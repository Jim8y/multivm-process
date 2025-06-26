//! Lock Ordering Protocol for MultiVM
//!
//! This module defines a consistent lock ordering to prevent deadlocks.
//! All locks must be acquired in the order defined here.

use multivm_common::error::{MultivmError, MultivmResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, Mutex};
use tokio::time::{timeout, Duration, Instant};
use tracing::{debug, warn, error};

/// Lock ordering levels (lower numbers must be acquired first)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockLevel {
    /// Coordinator state - highest level lock
    CoordinatorState = 1,
    /// Process manager processes
    Processes = 2,
    /// Account mappings
    AccountMappings = 3,
    /// Cross-VM transfers
    CrossVmTransfers = 4,
    /// Connection pools
    ConnectionPools = 5,
    /// Transaction queues
    TransactionQueues = 6,
    /// IPC message handlers
    IpcHandlers = 7,
    /// Resource monitors
    ResourceMonitors = 8,
}

/// Lock timeout configuration
#[derive(Debug, Clone)]
pub struct LockTimeoutConfig {
    /// Default timeout for lock acquisition
    pub default_timeout: Duration,
    /// Timeout for critical operations
    pub critical_timeout: Duration,
    /// Timeout for background operations
    pub background_timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Backoff multiplier for retries
    pub backoff_multiplier: f64,
}

impl Default for LockTimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(5),
            critical_timeout: Duration::from_secs(10),
            background_timeout: Duration::from_secs(2),
            max_retries: 3,
            backoff_multiplier: 1.5,
        }
    }
}

/// Global lock timeout configuration
static LOCK_CONFIG: std::sync::OnceLock<LockTimeoutConfig> = std::sync::OnceLock::new();

/// Initialize lock configuration
pub fn init_lock_config(config: LockTimeoutConfig) {
    LOCK_CONFIG.set(config).ok();
}

/// Get lock configuration
pub fn get_lock_config() -> &'static LockTimeoutConfig {
    LOCK_CONFIG.get_or_init(LockTimeoutConfig::default)
}

/// Lock acquisition statistics
#[derive(Debug, Default)]
pub struct LockStats {
    pub acquisitions: AtomicU64,
    pub timeouts: AtomicU64,
    pub contentions: AtomicU64,
    pub total_wait_time_ms: AtomicU64,
    pub max_wait_time_ms: AtomicU64,
}

/// Global lock statistics
static LOCK_STATS: std::sync::OnceLock<Arc<Mutex<HashMap<LockLevel, LockStats>>>> = std::sync::OnceLock::new();

/// Get lock statistics
pub async fn get_lock_stats() -> HashMap<LockLevel, LockStats> {
    let stats_map = LOCK_STATS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
    let guard = stats_map.lock().await;
    
    let mut result = HashMap::new();
    for (level, stats) in guard.iter() {
        result.insert(*level, LockStats {
            acquisitions: AtomicU64::new(stats.acquisitions.load(Ordering::Relaxed)),
            timeouts: AtomicU64::new(stats.timeouts.load(Ordering::Relaxed)),
            contentions: AtomicU64::new(stats.contentions.load(Ordering::Relaxed)),
            total_wait_time_ms: AtomicU64::new(stats.total_wait_time_ms.load(Ordering::Relaxed)),
            max_wait_time_ms: AtomicU64::new(stats.max_wait_time_ms.load(Ordering::Relaxed)),
        });
    }
    result
}

/// Update lock statistics
async fn update_lock_stats(level: LockLevel, wait_time: Duration, timed_out: bool) {
    let stats_map = LOCK_STATS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
    let mut guard = stats_map.lock().await;
    let stats = guard.entry(level).or_insert_with(LockStats::default);
    
    if timed_out {
        stats.timeouts.fetch_add(1, Ordering::Relaxed);
    } else {
        stats.acquisitions.fetch_add(1, Ordering::Relaxed);
    }
    
    let wait_ms = wait_time.as_millis() as u64;
    stats.total_wait_time_ms.fetch_add(wait_ms, Ordering::Relaxed);
    
    // Update max wait time
    let current_max = stats.max_wait_time_ms.load(Ordering::Relaxed);
    if wait_ms > current_max {
        stats.max_wait_time_ms.store(wait_ms, Ordering::Relaxed);
    }
    
    if wait_time > Duration::from_millis(100) {
        stats.contentions.fetch_add(1, Ordering::Relaxed);
    }
}

/// Acquire a write lock with timeout and retry logic
pub async fn acquire_write_lock<T>(
    lock: &Arc<RwLock<T>>,
    level: LockLevel,
    timeout_duration: Option<Duration>,
) -> MultivmResult<RwLockWriteGuard<'_, T>> {
    let config = get_lock_config();
    let timeout_duration = timeout_duration.unwrap_or(config.default_timeout);
    let start_time = Instant::now();
    
    debug!("Acquiring write lock for {:?} with timeout {:?}", level, timeout_duration);
    
    let mut attempts = 0;
    let mut backoff = Duration::from_millis(10);
    
    while attempts < config.max_retries {
        let attempt_start = Instant::now();
        
        match timeout(timeout_duration, lock.write()).await {
            Ok(guard) => {
                let wait_time = start_time.elapsed();
                update_lock_stats(level, wait_time, false).await;
                
                debug!("Successfully acquired write lock for {:?} after {:?}", level, wait_time);
                return Ok(guard);
            }
            Err(_) => {
                attempts += 1;
                let wait_time = attempt_start.elapsed();
                
                warn!(
                    "Write lock acquisition timeout for {:?} (attempt {}/{}), waited {:?}",
                    level, attempts, config.max_retries, wait_time
                );
                
                if attempts < config.max_retries {
                    tokio::time::sleep(backoff).await;
                    backoff = Duration::from_millis(
                        (backoff.as_millis() as f64 * config.backoff_multiplier) as u64
                    );
                }
            }
        }
    }
    
    let total_wait_time = start_time.elapsed();
    update_lock_stats(level, total_wait_time, true).await;
    
    error!(
        "Failed to acquire write lock for {:?} after {} attempts, total wait time: {:?}",
        level, config.max_retries, total_wait_time
    );
    
    Err(MultivmError::Timeout {
        operation: format!("write_lock_acquisition_{:?}", level),
        timeout: timeout_duration,
        partial_result: Some(format!("Thread {:?}", thread::current().id())),
    })
}

/// Acquire a read lock with timeout and retry logic
pub async fn acquire_read_lock<T>(
    lock: &Arc<RwLock<T>>,
    level: LockLevel,
    timeout_duration: Option<Duration>,
) -> MultivmResult<RwLockReadGuard<'_, T>> {
    let config = get_lock_config();
    let timeout_duration = timeout_duration.unwrap_or(config.default_timeout);
    let start_time = Instant::now();
    
    debug!("Acquiring read lock for {:?} with timeout {:?}", level, timeout_duration);
    
    let mut attempts = 0;
    let mut backoff = Duration::from_millis(10);
    
    while attempts < config.max_retries {
        let attempt_start = Instant::now();
        
        match timeout(timeout_duration, lock.read()).await {
            Ok(guard) => {
                let wait_time = start_time.elapsed();
                update_lock_stats(level, wait_time, false).await;
                
                debug!("Successfully acquired read lock for {:?} after {:?}", level, wait_time);
                return Ok(guard);
            }
            Err(_) => {
                attempts += 1;
                let wait_time = attempt_start.elapsed();
                
                warn!(
                    "Read lock acquisition timeout for {:?} (attempt {}/{}), waited {:?}",
                    level, attempts, config.max_retries, wait_time
                );
                
                if attempts < config.max_retries {
                    tokio::time::sleep(backoff).await;
                    backoff = Duration::from_millis(
                        (backoff.as_millis() as f64 * config.backoff_multiplier) as u64
                    );
                }
            }
        }
    }
    
    let total_wait_time = start_time.elapsed();
    update_lock_stats(level, total_wait_time, true).await;
    
    error!(
        "Failed to acquire read lock for {:?} after {} attempts, total wait time: {:?}",
        level, config.max_retries, total_wait_time
    );
    
    Err(MultivmError::Timeout {
        operation: format!("read_lock_acquisition_{:?}", level),
        timeout: timeout_duration,
        partial_result: Some(format!("Thread {:?}", thread::current().id())),
    })
}

/// Lock guard that ensures proper ordering and tracks acquisition
pub struct OrderedLockGuard {
    current_level: LockLevel,
    acquired_at: Instant,
    thread_id: thread::ThreadId,
}

impl OrderedLockGuard {
    pub fn new(level: LockLevel) -> Self {
        Self {
            current_level: level,
            acquired_at: Instant::now(),
            thread_id: thread::current().id(),
        }
    }

    /// Check if acquiring a new lock would violate ordering
    pub fn can_acquire(&self, new_level: LockLevel) -> bool {
        new_level > self.current_level
    }

    /// Validate lock acquisition order
    pub fn validate_acquisition(&self, new_level: LockLevel) -> MultivmResult<()> {
        if !self.can_acquire(new_level) {
            error!(
                "Lock ordering violation: thread {:?} attempting to acquire {:?} while holding {:?} (held for {:?})",
                self.thread_id, new_level, self.current_level, self.acquired_at.elapsed()
            );
            return Err(MultivmError::InvalidState {
                message: format!(
                    "Lock ordering violation: attempting to acquire {:?} while holding {:?}",
                    new_level, self.current_level
                ),
                current_state: Some(format!("{:?}", self.current_level)),
                expected_state: Some(format!("{:?}", new_level)),
            });
        }
        Ok(())
    }
    
    /// Get the current lock level
    pub fn current_level(&self) -> LockLevel {
        self.current_level
    }
    
    /// Get how long this lock has been held
    pub fn held_duration(&self) -> Duration {
        self.acquired_at.elapsed()
    }
}

impl Drop for OrderedLockGuard {
    fn drop(&mut self) {
        let held_duration = self.acquired_at.elapsed();
        if held_duration > Duration::from_millis(100) {
            warn!(
                "Lock {:?} held for {:?} by thread {:?}",
                self.current_level, held_duration, self.thread_id
            );
        }
    }
}

/// Deadlock detection context
#[derive(Debug)]
pub struct DeadlockDetector {
    active_locks: Arc<Mutex<HashMap<thread::ThreadId, Vec<LockLevel>>>>,
}

impl DeadlockDetector {
    pub fn new() -> Self {
        Self {
            active_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Register lock acquisition
    pub async fn register_lock(&self, level: LockLevel) -> MultivmResult<()> {
        let thread_id = thread::current().id();
        let mut locks = self.active_locks.lock().await;
        
        let thread_locks = locks.entry(thread_id).or_insert_with(Vec::new);
        
        // Check for potential deadlock
        if let Some(&last_level) = thread_locks.last() {
            if level <= last_level {
                error!(
                    "Potential deadlock detected: thread {:?} acquiring {:?} after {:?}",
                    thread_id, level, last_level
                );
                return Err(MultivmError::InvalidState {
                    message: format!(
                        "Potential deadlock: acquiring {:?} after {:?}",
                        level, last_level
                    ),
                    current_state: Some(format!("{:?}", last_level)),
                    expected_state: Some(format!("{:?}", level)),
                });
            }
        }
        
        thread_locks.push(level);
        Ok(())
    }
    
    /// Unregister lock release
    pub async fn unregister_lock(&self, level: LockLevel) {
        let thread_id = thread::current().id();
        let mut locks = self.active_locks.lock().await;
        
        if let Some(thread_locks) = locks.get_mut(&thread_id) {
            thread_locks.retain(|&l| l != level);
            if thread_locks.is_empty() {
                locks.remove(&thread_id);
            }
        }
    }
    
    /// Get current lock state for debugging
    pub async fn get_lock_state(&self) -> HashMap<thread::ThreadId, Vec<LockLevel>> {
        self.active_locks.lock().await.clone()
    }
}

/// Global deadlock detector
static DEADLOCK_DETECTOR: std::sync::OnceLock<DeadlockDetector> = std::sync::OnceLock::new();

/// Get the global deadlock detector
pub fn get_deadlock_detector() -> &'static DeadlockDetector {
    DEADLOCK_DETECTOR.get_or_init(DeadlockDetector::new)
}

/// Acquire write lock with deadlock detection
pub async fn acquire_write_lock_safe<T>(
    lock: &Arc<RwLock<T>>,
    level: LockLevel,
    timeout_duration: Option<Duration>,
) -> MultivmResult<RwLockWriteGuard<'_, T>> {
    let detector = get_deadlock_detector();
    detector.register_lock(level).await?;
    
    match acquire_write_lock(lock, level, timeout_duration).await {
        Ok(guard) => Ok(guard),
        Err(e) => {
            detector.unregister_lock(level).await;
            Err(e)
        }
    }
}

/// Acquire read lock with deadlock detection
pub async fn acquire_read_lock_safe<T>(
    lock: &Arc<RwLock<T>>,
    level: LockLevel,
    timeout_duration: Option<Duration>,
) -> MultivmResult<RwLockReadGuard<'_, T>> {
    let detector = get_deadlock_detector();
    detector.register_lock(level).await?;
    
    match acquire_read_lock(lock, level, timeout_duration).await {
        Ok(guard) => Ok(guard),
        Err(e) => {
            detector.unregister_lock(level).await;
            Err(e)
        }
    }
}

/// Macro to acquire multiple locks in the correct order with deadlock detection
#[macro_export]
macro_rules! acquire_locks_safe {
    ($($lock:expr => $level:expr),+ $(,)?) => {{
        // Sort locks by level to ensure correct ordering
        let mut locks = vec![$( ($lock, $level) ),+];
        locks.sort_by_key(|&(_, level)| level);

        let mut guards = Vec::new();
        for (lock, level) in locks {
            let guard = $crate::lock_ordering::acquire_write_lock_safe(
                lock,
                level,
                None
            ).await?;
            guards.push(guard);
        }
        guards
    }};
}

/// Macro to acquire multiple read locks in the correct order
#[macro_export]
macro_rules! acquire_read_locks_safe {
    ($($lock:expr => $level:expr),+ $(,)?) => {{
        // Sort locks by level to ensure correct ordering
        let mut locks = vec![$( ($lock, $level) ),+];
        locks.sort_by_key(|&(_, level)| level);

        let mut guards = Vec::new();
        for (lock, level) in locks {
            let guard = $crate::lock_ordering::acquire_read_lock_safe(
                lock,
                level,
                None
            ).await?;
            guards.push(guard);
        }
        guards
    }};
}

/// Lock scope guard that automatically releases locks on drop
pub struct LockScope {
    levels: Vec<LockLevel>,
    detector: &'static DeadlockDetector,
}

impl LockScope {
    pub async fn new() -> Self {
        Self {
            levels: Vec::new(),
            detector: get_deadlock_detector(),
        }
    }
    
    pub async fn acquire_write<'a, T>(
        &mut self,
        lock: &'a Arc<RwLock<T>>,
        level: LockLevel,
        timeout: Option<Duration>,
    ) -> MultivmResult<RwLockWriteGuard<'a, T>> {
        self.detector.register_lock(level).await?;
        match acquire_write_lock(lock, level, timeout).await {
            Ok(guard) => {
                self.levels.push(level);
                Ok(guard)
            }
            Err(e) => {
                self.detector.unregister_lock(level).await;
                Err(e)
            }
        }
    }
    
    pub async fn acquire_read<'a, T>(
        &mut self,
        lock: &'a Arc<RwLock<T>>,
        level: LockLevel,
        timeout: Option<Duration>,
    ) -> MultivmResult<RwLockReadGuard<'a, T>> {
        self.detector.register_lock(level).await?;
        match acquire_read_lock(lock, level, timeout).await {
            Ok(guard) => {
                self.levels.push(level);
                Ok(guard)
            }
            Err(e) => {
                self.detector.unregister_lock(level).await;
                Err(e)
            }
        }
    }
}

impl Drop for LockScope {
    fn drop(&mut self) {
        // Note: We can't await in Drop, so we spawn a task
        let levels = std::mem::take(&mut self.levels);
        let detector = self.detector;
        
        tokio::spawn(async move {
            for level in levels {
                detector.unregister_lock(level).await;
            }
        });
    }
}

