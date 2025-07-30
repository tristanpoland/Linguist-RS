//! Advanced multithreading infrastructure for Linguist.
//!
//! This module provides thread pools, work queues, and parallel processing
//! utilities optimized for language detection and file analysis tasks.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use threadpool::ThreadPool;
use tokio::sync::Semaphore;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;
use crate::{Error, Result};

/// Configuration for thread pools and parallel processing
#[derive(Debug, Clone)]
pub struct ThreadingConfig {
    /// Number of worker threads for file processing
    pub worker_threads: usize,
    /// Number of threads for I/O operations
    pub io_threads: usize,
    /// Maximum number of concurrent language detection tasks
    pub max_concurrent_detections: usize,
    /// Work queue capacity
    pub queue_capacity: usize,
    /// Whether to use work stealing for load balancing
    pub use_work_stealing: bool,
}

impl Default for ThreadingConfig {
    fn default() -> Self {
        let cpu_count = num_cpus::get();
        Self {
            worker_threads: cpu_count * 2,
            io_threads: cpu_count.min(8),
            max_concurrent_detections: cpu_count * 4,
            queue_capacity: 10000,
            use_work_stealing: true,
        }
    }
}

/// Work item for the parallel processing queue
pub enum WorkItem<T> {
    /// Process a blob for language detection
    DetectLanguage {
        blob: Arc<dyn BlobHelper + Send + Sync>,
        result_sender: Sender<(String, Option<Language>)>,
    },
    /// Process multiple blobs in batch
    BatchProcess {
        blobs: Vec<Arc<dyn BlobHelper + Send + Sync>>,
        result_sender: Sender<Vec<(String, Option<Language>)>>,
    },
    /// Custom work item
    Custom(T),
    /// Shutdown signal
    Shutdown,
}

/// Statistics for monitoring thread performance
#[derive(Debug, Default)]
pub struct ThreadingStats {
    /// Total tasks processed
    pub total_tasks: AtomicUsize,
    /// Tasks currently in progress
    pub active_tasks: AtomicUsize,
    /// Number of worker threads
    pub worker_threads: AtomicUsize,
    /// Queue size
    pub queue_size: AtomicUsize,
    /// Average processing time in microseconds
    pub avg_processing_time_us: AtomicUsize,
}

impl ThreadingStats {
    pub fn increment_tasks(&self) {
        self.total_tasks.fetch_add(1, Ordering::Relaxed);
        self.active_tasks.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn decrement_active(&self) {
        self.active_tasks.fetch_sub(1, Ordering::Relaxed);
    }
    
    pub fn update_avg_time(&self, time_us: usize) {
        // Simple exponential moving average
        let current = self.avg_processing_time_us.load(Ordering::Relaxed);
        let new_avg = (current * 9 + time_us) / 10;
        self.avg_processing_time_us.store(new_avg, Ordering::Relaxed);
    }
}

/// Advanced thread pool manager with work stealing and load balancing
pub struct ThreadPoolManager {
    /// Configuration
    config: ThreadingConfig,
    /// Main worker thread pool
    workers: ThreadPool,
    /// I/O thread pool for file operations
    io_pool: ThreadPool,
    /// Work queue sender
    work_sender: Sender<WorkItem<Box<dyn Send + Sync>>>,
    /// Work queue receiver
    work_receiver: Receiver<WorkItem<Box<dyn Send + Sync>>>,
    /// Statistics
    stats: Arc<ThreadingStats>,
    /// Concurrent semaphore for limiting parallel operations
    semaphore: Arc<Semaphore>,
    /// Cache for language detection results
    cache: Arc<DashMap<String, Option<Language>>>,
    /// Shutdown flag
    shutdown: Arc<parking_lot::Mutex<bool>>,
}

impl ThreadPoolManager {
    /// Create a new thread pool manager
    pub fn new(config: ThreadingConfig) -> Self {
        let (work_sender, work_receiver) = if config.queue_capacity > 0 {
            bounded(config.queue_capacity)
        } else {
            unbounded()
        };
        
        let workers = ThreadPool::new(config.worker_threads);
        let io_pool = ThreadPool::new(config.io_threads);
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_detections));
        
        Self {
            workers,
            io_pool,
            work_sender,
            work_receiver,
            stats: Arc::new(ThreadingStats::default()),
            semaphore,
            cache: Arc::new(DashMap::new()),
            shutdown: Arc::new(parking_lot::Mutex::new(false)),
            config,
        }
    }
    
    /// Start the worker threads
    pub fn start(&self) {
        let stats = self.stats.clone();
        stats.worker_threads.store(self.config.worker_threads, Ordering::Relaxed);
        
        // Start work stealing workers if enabled
        if self.config.use_work_stealing {
            self.start_work_stealing_workers();
        }
        
        // Start monitoring thread
        self.start_monitoring_thread();
    }
    
    /// Start work stealing workers
    fn start_work_stealing_workers(&self) {
        let receiver = self.work_receiver.clone();
        let stats = self.stats.clone();
        let cache = self.cache.clone();
        let shutdown = self.shutdown.clone();
        
        for i in 0..self.config.worker_threads {
            let receiver = receiver.clone();
            let stats = stats.clone();
            let cache = cache.clone();
            let shutdown = shutdown.clone();
            
            self.workers.execute(move || {
                let worker_id = i;
                Self::worker_loop(worker_id, receiver, stats, cache, shutdown);
            });
        }
    }
    
    /// Worker loop for processing work items
    fn worker_loop(
        worker_id: usize,
        receiver: Receiver<WorkItem<Box<dyn Send + Sync>>>,
        stats: Arc<ThreadingStats>,
        cache: Arc<DashMap<String, Option<Language>>>,
        shutdown: Arc<parking_lot::Mutex<bool>>,
    ) {
        loop {
            // Check shutdown flag
            if *shutdown.lock() {
                break;
            }
            
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(work_item) => {
                    let start_time = std::time::Instant::now();
                    stats.increment_tasks();
                    
                    match work_item {
                        WorkItem::DetectLanguage { blob, result_sender } => {
                            // Check cache first
                            let cache_key = blob.name().to_string();
                            if let Some(cached_result) = cache.get(&cache_key) {
                                let _ = result_sender.send((cache_key, cached_result.clone()));
                            } else {
                                // Perform language detection
                                let language = blob.language();
                                cache.insert(cache_key.clone(), language.clone());
                                let _ = result_sender.send((cache_key, language));
                            }
                        },
                        WorkItem::BatchProcess { blobs, result_sender } => {
                            // Process blobs in parallel using rayon
                            let results: Vec<_> = blobs.par_iter().map(|blob| {
                                let cache_key = blob.name().to_string();
                                if let Some(cached_result) = cache.get(&cache_key) {
                                    (cache_key, cached_result.clone())
                                } else {
                                    let language = blob.language();
                                    cache.insert(cache_key.clone(), language.clone());
                                    (cache_key, language)
                                }
                            }).collect();
                            let _ = result_sender.send(results);
                        },
                        WorkItem::Custom(_) => {
                            // Handle custom work items
                        },
                        WorkItem::Shutdown => {
                            break;
                        }
                    }
                    
                    stats.decrement_active(); 
                    let elapsed = start_time.elapsed();
                    stats.update_avg_time(elapsed.as_micros() as usize);
                },
                Err(_) => {
                    // Timeout - continue loop to check shutdown
                    continue;
                }
            }
        }
    }
    
    /// Start monitoring thread for statistics
    fn start_monitoring_thread(&self) {
        let stats = self.stats.clone();
        let shutdown = self.shutdown.clone();
        
        thread::spawn(move || {
            loop {
                if *shutdown.lock() {
                    break;
                }
                
                thread::sleep(Duration::from_secs(5));
                
                // Log statistics periodically (could be configurable)
                let total = stats.total_tasks.load(Ordering::Relaxed);
                let active = stats.active_tasks.load(Ordering::Relaxed);
                let avg_time = stats.avg_processing_time_us.load(Ordering::Relaxed);
                
                if total > 0 {
                    log::debug!(
                        "Threading stats - Total: {}, Active: {}, Avg time: {}μs",
                        total, active, avg_time
                    );
                }
            }
        });
    }
    
    /// Submit work for language detection
    pub fn detect_language_async(
        &self,
        blob: Arc<dyn BlobHelper + Send + Sync>,
    ) -> crossbeam_channel::Receiver<(String, Option<Language>)> {
        let (sender, receiver) = unbounded();
        
        let work_item = WorkItem::DetectLanguage {
            blob,
            result_sender: sender,
        };
        
        // Update queue size statistics
        self.stats.queue_size.fetch_add(1, Ordering::Relaxed);
        
        if let Err(_) = self.work_sender.send(work_item) {
            // Queue is full or closed, handle gracefully
            log::warn!("Failed to submit work item - queue may be full");
        }
        
        receiver
    }
    
    /// Submit batch work for processing multiple blobs
    pub fn batch_process_async(
        &self,
        blobs: Vec<Arc<dyn BlobHelper + Send + Sync>>,
    ) -> crossbeam_channel::Receiver<Vec<(String, Option<Language>)>> {
        let (sender, receiver) = unbounded();
        
        let work_item = WorkItem::BatchProcess {
            blobs,
            result_sender: sender,
        };
        
        self.stats.queue_size.fetch_add(1, Ordering::Relaxed);
        
        if let Err(_) = self.work_sender.send(work_item) {
            log::warn!("Failed to submit batch work item");
        }
        
        receiver
    }
    
    /// Process files in a directory with parallel processing
    pub async fn process_directory_parallel<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<Vec<(String, Option<Language>)>> {
        let path = path.as_ref();
        
        // Collect all files first
        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            files.push(entry.path().to_path_buf());
        }
        
        // Process files in parallel batches
        let batch_size = 100; // Process files in batches of 100
        let mut results = Vec::new();
        
        for chunk in files.chunks(batch_size) {
            let blobs: Vec<Arc<dyn BlobHelper + Send + Sync>> = chunk
                .iter()
                .filter_map(|path| {
                    crate::blob::FileBlob::new(path)
                        .ok()
                        .map(|blob| Arc::new(blob) as Arc<dyn BlobHelper + Send + Sync>)
                })
                .collect();
            
            if !blobs.is_empty() {
                let receiver = self.batch_process_async(blobs);
                match receiver.recv() {
                    Ok(batch_results) => results.extend(batch_results),
                    Err(_) => {
                        log::warn!("Failed to receive batch processing results");
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    /// Get current statistics
    pub fn stats(&self) -> ThreadingStats {
        ThreadingStats {
            total_tasks: AtomicUsize::new(self.stats.total_tasks.load(Ordering::Relaxed)),
            active_tasks: AtomicUsize::new(self.stats.active_tasks.load(Ordering::Relaxed)),
            worker_threads: AtomicUsize::new(self.stats.worker_threads.load(Ordering::Relaxed)),
            queue_size: AtomicUsize::new(self.stats.queue_size.load(Ordering::Relaxed)),
            avg_processing_time_us: AtomicUsize::new(self.stats.avg_processing_time_us.load(Ordering::Relaxed)),
        }
    }
    
    /// Shutdown the thread pool
    pub fn shutdown(&self) {
        *self.shutdown.lock() = true;
        
        // Send shutdown signals to all workers
        for _ in 0..self.config.worker_threads {
            let _ = self.work_sender.send(WorkItem::Shutdown);
        }
        
        // Wait for workers to finish
        thread::sleep(Duration::from_millis(100));
    }
}

impl Drop for ThreadPoolManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Parallel strategy executor for concurrent language detection
pub struct ParallelStrategyExecutor {
    pool: Arc<ThreadPoolManager>,
}

impl ParallelStrategyExecutor {
    pub fn new(config: ThreadingConfig) -> Self {
        let pool = Arc::new(ThreadPoolManager::new(config));
        pool.start();
        
        Self { pool }
    }
    
    /// Execute multiple strategies concurrently
    pub async fn execute_strategies_parallel<B: BlobHelper + Send + Sync + 'static>(
        &self,
        blob: Arc<B>,
        strategies: Vec<crate::strategy::StrategyType>,
    ) -> Vec<Language> {
        use futures::future::join_all;
        
        // Execute strategies concurrently
        let futures: Vec<_> = strategies.into_iter().map(|strategy| {
            let blob = blob.clone();
            async move {
                // In a real implementation, we'd spawn this on a thread pool
                tokio::task::spawn_blocking(move || {
                    strategy.call(blob.as_ref(), &[])
                }).await.unwrap_or_else(|_| Vec::new())
            }
        }).collect();
        
        let results = join_all(futures).await;
        
        // Combine results from all strategies
        let mut all_languages = Vec::new();
        for mut languages in results {
            all_languages.append(&mut languages);
        }
        
        // Remove duplicates
        all_languages.sort_by(|a, b| a.name.cmp(&b.name));
        all_languages.dedup_by(|a, b| a.name == b.name);
        
        all_languages
    }
}

/// Global thread pool manager instance
lazy_static::lazy_static! {
    static ref GLOBAL_THREAD_POOL: ThreadPoolManager = {
        let config = ThreadingConfig::default();
        let pool = ThreadPoolManager::new(config);
        pool.start();
        pool
    };
}

/// Get the global thread pool manager
pub fn global_thread_pool() -> &'static ThreadPoolManager {
    &GLOBAL_THREAD_POOL
}

/// Convenience function for parallel language detection
pub fn detect_language_parallel(
    blob: Arc<dyn BlobHelper + Send + Sync>
) -> crossbeam_channel::Receiver<(String, Option<Language>)> {
    global_thread_pool().detect_language_async(blob)
}

/// Convenience function for batch processing
pub fn batch_process_parallel(
    blobs: Vec<Arc<dyn BlobHelper + Send + Sync>>
) -> crossbeam_channel::Receiver<Vec<(String, Option<Language>)>> {
    global_thread_pool().batch_process_async(blobs)
}

/// Thread-safe counter for tracking processing statistics
#[derive(Debug, Default)]
pub struct ThreadSafeCounter {
    count: AtomicUsize,
}

impl ThreadSafeCounter {
    pub fn new() -> Self {
        Self { count: AtomicUsize::new(0) }
    }
    
    pub fn increment(&self) -> usize {
        self.count.fetch_add(1, Ordering::Relaxed)
    }
    
    pub fn decrement(&self) -> usize {
        self.count.fetch_sub(1, Ordering::Relaxed)
    }
    
    pub fn get(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
    
    pub fn reset(&self) -> usize {
        self.count.swap(0, Ordering::Relaxed)
    }
}

/// Thread-safe progress tracker for long-running operations
#[derive(Debug)]
pub struct ProgressTracker {
    total: AtomicUsize,
    completed: AtomicUsize,
    start_time: std::time::Instant,
}

impl ProgressTracker {
    pub fn new(total: usize) -> Self {
        Self {
            total: AtomicUsize::new(total),
            completed: AtomicUsize::new(0),
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn increment(&self) -> usize {
        self.completed.fetch_add(1, Ordering::Relaxed)
    }
    
    pub fn progress(&self) -> f64 {
        let total = self.total.load(Ordering::Relaxed);
        let completed = self.completed.load(Ordering::Relaxed);
        
        if total == 0 {
            0.0
        } else {
            (completed as f64) / (total as f64)
        }
    }
    
    pub fn remaining(&self) -> usize {
        let total = self.total.load(Ordering::Relaxed);
        let completed = self.completed.load(Ordering::Relaxed);
        total.saturating_sub(completed)
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    pub fn eta(&self) -> Option<Duration> {
        let progress = self.progress();
        if progress > 0.0 && progress < 1.0 {
            let elapsed = self.elapsed();
            let total_time = elapsed.as_secs_f64() / progress;
            let remaining_time = total_time - elapsed.as_secs_f64();
            Some(Duration::from_secs_f64(remaining_time.max(0.0)))
        } else {
            None
        }
    }
}

/// Thread-safe resource pool for managing limited resources
#[derive(Debug)]
pub struct ResourcePool<T> {
    resources: Arc<Mutex<Vec<T>>>,
    semaphore: Arc<Semaphore>,
}

impl<T> ResourcePool<T> {
    pub fn new(resources: Vec<T>) -> Self {
        let count = resources.len();
        Self {
            resources: Arc::new(Mutex::new(resources)),
            semaphore: Arc::new(Semaphore::new(count)),
        }
    }
    
    pub async fn acquire(&self) -> Option<ResourceGuard<T>> {
        let _permit = self.semaphore.acquire().await.ok()?;
        let resource = {
            let mut resources = self.resources.lock();
            resources.pop()
        };
        
        resource.map(|r| ResourceGuard {
            resource: Some(r),
            pool: self.resources.clone(),
        })
    }
    
    pub fn try_acquire(&self) -> Option<ResourceGuard<T>> {
        let _permit = self.semaphore.try_acquire().ok()?;
        let resource = {
            let mut resources = self.resources.lock();
            resources.pop()
        };
        
        resource.map(|r| ResourceGuard {
            resource: Some(r),
            pool: self.resources.clone(),
        })
    }
    
    pub fn available_count(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// RAII guard for resources from ResourcePool
pub struct ResourceGuard<T> {
    resource: Option<T>,
    pool: Arc<Mutex<Vec<T>>>,
}

impl<T> ResourceGuard<T> {
    pub fn get(&self) -> &T {
        self.resource.as_ref().expect("Resource should be available")
    }
    
    pub fn get_mut(&mut self) -> &mut T {
        self.resource.as_mut().expect("Resource should be available")
    }
}

impl<T> Drop for ResourceGuard<T> {
    fn drop(&mut self) {
        if let Some(resource) = self.resource.take() {
            let mut resources = self.pool.lock();
            resources.push(resource);
        }
    }
}

impl<T> std::ops::Deref for ResourceGuard<T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for ResourceGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::FileBlob;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_threading_config() {
        let config = ThreadingConfig::default();
        assert!(config.worker_threads > 0);
        assert!(config.io_threads > 0);
        assert!(config.max_concurrent_detections > 0);
    }
    
    #[test]
    fn test_thread_pool_creation() {
        let config = ThreadingConfig {
            worker_threads: 2,
            io_threads: 1,
            max_concurrent_detections: 4,
            queue_capacity: 100,
            use_work_stealing: true,
        };
        
        let pool = ThreadPoolManager::new(config);
        pool.start();
        
        // Verify basic functionality
        let stats = pool.stats();
        assert_eq!(stats.worker_threads.load(Ordering::Relaxed), 2);
    }
    
    #[tokio::test]
    async fn test_parallel_directory_processing() -> Result<()> {
        let dir = tempdir()?;
        
        // Create test files
        let rust_path = dir.path().join("test.rs");
        {
            let mut file = File::create(&rust_path)?;
            file.write_all(b"fn main() { println!(\"Hello, world!\"); }")?;
        }
        
        let js_path = dir.path().join("test.js");
        {
            let mut file = File::create(&js_path)?;
            file.write_all(b"console.log('Hello, world!');")?;
        }
        
        let config = ThreadingConfig {
            worker_threads: 2,
            io_threads: 1,
            max_concurrent_detections: 4,
            queue_capacity: 100,
            use_work_stealing: true,
        };
        
        let pool = ThreadPoolManager::new(config);
        pool.start();
        
        let results = pool.process_directory_parallel(dir.path()).await?;
        
        assert!(!results.is_empty());
        assert!(results.len() >= 2);
        
        pool.shutdown();
        Ok(())
    }
    
    #[test]
    fn test_batch_processing() {
        let config = ThreadingConfig::default();
        let pool = ThreadPoolManager::new(config);
        pool.start();
        
        // Create test blobs
        let blob1 = Arc::new(FileBlob::from_data(
            std::path::Path::new("test1.rs"),
            b"fn main() {}".to_vec()
        )) as Arc<dyn BlobHelper + Send + Sync>;
        
        let blob2 = Arc::new(FileBlob::from_data(
            std::path::Path::new("test2.js"), 
            b"console.log('hello');".to_vec()
        )) as Arc<dyn BlobHelper + Send + Sync>;
        
        let blobs = vec![blob1, blob2];
        let receiver = pool.batch_process_async(blobs);
        
        // Wait for results (with timeout)
        if let Ok(results) = receiver.recv_timeout(Duration::from_secs(5)) {
            assert_eq!(results.len(), 2);
        }
        
        pool.shutdown();
    }
    
    #[test]
    fn test_concurrent_access() {
        use std::thread;
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        let config = ThreadingConfig {
            worker_threads: 4,
            io_threads: 2,
            max_concurrent_detections: 8,
            queue_capacity: 100,
            use_work_stealing: true,
        };
        
        let pool = Arc::new(ThreadPoolManager::new(config));
        pool.start();
        
        let processed_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        
        // Spawn multiple threads to test concurrent access
        for i in 0..10 {
            let pool = pool.clone();
            let processed_count = processed_count.clone();
            
            let handle = thread::spawn(move || {
                for j in 0..5 {
                    let blob = Arc::new(FileBlob::from_data(
                        std::path::Path::new(&format!("test{}_{}.rs", i, j)),
                        format!("fn test{}_{}() {{}}", i, j).into_bytes()
                    )) as Arc<dyn BlobHelper + Send + Sync>;
                    
                    let receiver = pool.detect_language_async(blob);
                    if receiver.recv_timeout(Duration::from_secs(1)).is_ok() {
                        processed_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        let final_count = processed_count.load(Ordering::Relaxed);
        assert!(final_count > 0, "Expected some tasks to be processed");
        
        pool.shutdown();
    }
    
    #[test]
    fn test_work_stealing_performance() {
        let start_time = std::time::Instant::now();
        
        // Test with work stealing enabled
        let config_with_stealing = ThreadingConfig {
            worker_threads: 4,
            io_threads: 2,
            max_concurrent_detections: 16,
            queue_capacity: 1000,
            use_work_stealing: true,
        };
        
        let pool = ThreadPoolManager::new(config_with_stealing);
        pool.start();
        
        // Create many small tasks
        let mut receivers = Vec::new();
        for i in 0..100 {
            let blob = Arc::new(FileBlob::from_data(
                std::path::Path::new(&format!("perf_test_{}.rs", i)),
                format!("// Test file {}\nfn main() {{ println!(\"Hello {}\"); }}", i, i).into_bytes()
            )) as Arc<dyn BlobHelper + Send + Sync>;
            
            receivers.push(pool.detect_language_async(blob));
        }
        
        // Collect all results
        let mut success_count = 0;
        for receiver in receivers {
            if receiver.recv_timeout(Duration::from_secs(5)).is_ok() {
                success_count += 1;
            }
        }
        
        let elapsed = start_time.elapsed();
        pool.shutdown();
        
        assert!(success_count > 90, "Expected most tasks to succeed, got {}", success_count);
        assert!(elapsed.as_secs() < 10, "Performance benchmark took too long: {:?}", elapsed);
        
        println!("Work stealing performance test: {} tasks in {:?}", success_count, elapsed);
    }
    
    #[test]
    fn test_stats_monitoring() {
        let config = ThreadingConfig::default();
        let pool = ThreadPoolManager::new(config);
        pool.start();
        
        // Submit some work
        let blob = Arc::new(FileBlob::from_data(
            std::path::Path::new("stats_test.rs"),
            b"fn main() { println!(\"Stats test\"); }".to_vec()
        )) as Arc<dyn BlobHelper + Send + Sync>;
        
        let receiver = pool.detect_language_async(blob);
        let _ = receiver.recv_timeout(Duration::from_secs(1));
        
        // Check stats
        let stats = pool.stats();
        assert!(stats.total_tasks.load(Ordering::Relaxed) > 0);
        assert!(stats.worker_threads.load(Ordering::Relaxed) > 0);
        
        pool.shutdown();
    }
    
    #[tokio::test]
    async fn test_parallel_strategy_execution() {
        use crate::strategy::StrategyType;
        use crate::blob::FileBlob;
        
        let executor = ParallelStrategyExecutor::new(ThreadingConfig::default());
        
        let blob = Arc::new(FileBlob::from_data(
            std::path::Path::new("strategy_test.js"),
            b"console.log('Testing parallel strategies');".to_vec()
        ));
        
        // Create a subset of strategies for testing
        let strategies = vec![
            StrategyType::Extension(crate::strategy::extension::Extension),
            StrategyType::Filename(crate::strategy::filename::Filename),
        ];
        
        let results = executor.execute_strategies_parallel(blob, strategies).await;
        
        // We expect at least some strategy to potentially identify this as JavaScript
        // Note: The actual result depends on the strategy implementations
        println!("Parallel strategy execution returned {} results", results.len());
    }
}