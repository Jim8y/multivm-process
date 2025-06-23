//! Lock Ordering Protocol for MultiVM
//!
//! This module defines a consistent lock ordering to prevent deadlocks.
//! All locks must be acquired in the order defined here.

use multivm_common::error::{MultivmError, MultivmResult};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time::{timeout, Duration};

/// Lock ordering levels (lower numbers must be acquired first)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
}

/// Default lock timeout
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// Acquire a write lock with timeout
pub async fn acquire_write_lock<T>(
    lock: &Arc<RwLock<T>>,
    level: LockLevel,
    timeout_duration: Option<Duration>,
) -> MultivmResult<RwLockWriteGuard<'_, T>> {
    let timeout_duration = timeout_duration.unwrap_or(DEFAULT_LOCK_TIMEOUT);

    timeout(timeout_duration, lock.write())
        .await
        .map_err(|_| MultivmError::LockTimeout {
            lock_name: format!("{:?}", level),
            timeout: timeout_duration,
        })
}

/// Acquire a read lock with timeout
pub async fn acquire_read_lock<T>(
    lock: &Arc<RwLock<T>>,
    level: LockLevel,
    timeout_duration: Option<Duration>,
) -> MultivmResult<RwLockReadGuard<'_, T>> {
    let timeout_duration = timeout_duration.unwrap_or(DEFAULT_LOCK_TIMEOUT);

    timeout(timeout_duration, lock.read())
        .await
        .map_err(|_| MultivmError::LockTimeout {
            lock_name: format!("{:?}", level),
            timeout: timeout_duration,
        })
}

/// Lock guard that ensures proper ordering
pub struct OrderedLockGuard {
    current_level: LockLevel,
}

impl OrderedLockGuard {
    pub fn new(level: LockLevel) -> Self {
        Self {
            current_level: level,
        }
    }

    /// Check if acquiring a new lock would violate ordering
    pub fn can_acquire(&self, new_level: LockLevel) -> bool {
        new_level > self.current_level
    }

    /// Validate lock acquisition order
    pub fn validate_acquisition(&self, new_level: LockLevel) -> MultivmResult<()> {
        if !self.can_acquire(new_level) {
            return Err(MultivmError::InvalidState(format!(
                "Lock ordering violation: attempting to acquire {:?} while holding {:?}",
                new_level, self.current_level
            )));
        }
        Ok(())
    }
}

/// Macro to acquire multiple locks in the correct order
#[macro_export]
macro_rules! acquire_locks {
    ($($lock:expr => $level:expr),+ $(,)?) => {{
        // Sort locks by level to ensure correct ordering
        let mut locks = vec![$( ($lock, $level) ),+];
        locks.sort_by_key(|&(_, level)| level);

        let mut guards = Vec::new();
        for (lock, level) in locks {
            let guard = $crate::lock_ordering::acquire_write_lock(
                lock,
                level,
                None
            ).await?;
            guards.push(guard);
        }
        guards
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_ordering() {
        let guard = OrderedLockGuard::new(LockLevel::CoordinatorState);

        // Should allow acquiring lower level locks
        assert!(guard.can_acquire(LockLevel::Processes));
        assert!(guard.can_acquire(LockLevel::AccountMappings));

        // Should not allow acquiring same or higher level locks
        assert!(!guard.can_acquire(LockLevel::CoordinatorState));
    }

    #[tokio::test]
    async fn test_lock_timeout() {
        let lock = Arc::new(RwLock::new(42));

        // Acquire write lock
        let _guard = lock.write().await;

        // Try to acquire with timeout - should fail
        let result = acquire_write_lock(
            &lock,
            LockLevel::CoordinatorState,
            Some(Duration::from_millis(100)),
        )
        .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MultivmError::LockTimeout { .. } => {}
            _ => panic!("Expected LockTimeout error"),
        }
    }
}
