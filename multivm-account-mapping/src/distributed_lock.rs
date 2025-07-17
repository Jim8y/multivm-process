//! Distributed locking for preventing race conditions in account binding

use crate::error::{AccountMappingError, AccountMappingResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Distributed lock manager for account operations
#[async_trait::async_trait]
pub trait DistributedLockManager: Send + Sync {
    /// Acquire a lock for a given key
    async fn acquire_lock(&self, key: &str, ttl: Duration) -> AccountMappingResult<LockGuard>;

    /// Release a lock
    async fn release_lock(&self, lock: LockGuard) -> AccountMappingResult<()>;

    /// Check if a lock exists
    async fn is_locked(&self, key: &str) -> AccountMappingResult<bool>;
}

/// Lock guard that automatically releases on drop
#[derive(Debug, Clone)]
pub struct LockGuard {
    pub key: String,
    pub lock_id: String,
    pub acquired_at: SystemTime,
    pub ttl: Duration,
}

impl LockGuard {
    /// Check if the lock is still valid
    pub fn is_valid(&self) -> bool {
        if let Ok(elapsed) = self.acquired_at.elapsed() {
            elapsed < self.ttl
        } else {
            false
        }
    }
}

/// In-memory implementation for development/testing
pub struct InMemoryLockManager {
    locks: Arc<RwLock<HashMap<String, LockEntry>>>,
}

#[derive(Debug, Clone)]
struct LockEntry {
    lock_id: String,
    acquired_at: SystemTime,
    ttl: Duration,
}

impl InMemoryLockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Clean up expired locks
    async fn cleanup_expired(&self) {
        let mut locks = self.locks.write().await;
        locks.retain(|_, entry| {
            if let Ok(elapsed) = entry.acquired_at.elapsed() {
                elapsed < entry.ttl
            } else {
                true
            }
        });
    }
}

#[async_trait::async_trait]
impl DistributedLockManager for InMemoryLockManager {
    async fn acquire_lock(&self, key: &str, ttl: Duration) -> AccountMappingResult<LockGuard> {
        // Clean up expired locks first
        self.cleanup_expired().await;

        let mut locks = self.locks.write().await;

        // Check if lock already exists and is valid
        if let Some(existing) = locks.get(key) {
            if let Ok(elapsed) = existing.acquired_at.elapsed() {
                if elapsed < existing.ttl {
                    return Err(AccountMappingError::LockContention {
                        resource: key.to_string(),
                        holder: existing.lock_id.clone(),
                    });
                }
            }
        }

        // Create new lock
        let lock_id = format!("{}-{}", uuid::Uuid::new_v4(), key);
        let acquired_at = SystemTime::now();

        let entry = LockEntry {
            lock_id: lock_id.clone(),
            acquired_at,
            ttl,
        };

        locks.insert(key.to_string(), entry);

        Ok(LockGuard {
            key: key.to_string(),
            lock_id,
            acquired_at,
            ttl,
        })
    }

    async fn release_lock(&self, lock: LockGuard) -> AccountMappingResult<()> {
        let mut locks = self.locks.write().await;

        if let Some(existing) = locks.get(&lock.key) {
            if existing.lock_id == lock.lock_id {
                locks.remove(&lock.key);
                Ok(())
            } else {
                Err(AccountMappingError::InvalidLock {
                    reason: "Lock ID mismatch".to_string(),
                })
            }
        } else {
            // Lock already expired or released
            Ok(())
        }
    }

    async fn is_locked(&self, key: &str) -> AccountMappingResult<bool> {
        self.cleanup_expired().await;
        let locks = self.locks.read().await;
        Ok(locks.contains_key(key))
    }
}

/// Redis-based distributed lock manager (production)
#[cfg(feature = "redis")]
pub struct RedisLockManager {
    client: redis_crate::aio::ConnectionManager,
    key_prefix: String,
}

#[cfg(feature = "redis")]
impl RedisLockManager {
    pub async fn new(redis_url: &str, key_prefix: String) -> AccountMappingResult<Self> {
        use redis_crate::AsyncCommands;

        let client =
            redis_crate::Client::open(redis_url).map_err(|e| AccountMappingError::Internal {
                message: format!("Failed to create Redis client: {}", e),
            })?;

        let connection =
            client
                .get_connection_manager()
                .await
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Failed to connect to Redis: {}", e),
                })?;

        Ok(Self {
            client: connection,
            key_prefix,
        })
    }

    fn make_key(&self, key: &str) -> String {
        format!("{}:lock:{}", self.key_prefix, key)
    }
}

#[cfg(feature = "redis")]
#[async_trait::async_trait]
impl DistributedLockManager for RedisLockManager {
    async fn acquire_lock(&self, key: &str, ttl: Duration) -> AccountMappingResult<LockGuard> {
        use redis_crate::AsyncCommands;

        let lock_key = self.make_key(key);
        let lock_id = format!("{}-{}", uuid::Uuid::new_v4(), key);
        let ttl_ms = ttl.as_millis() as u64;

        // Try to acquire lock with SET NX EX
        let mut conn = self.client.clone();
        let result: bool = conn
            .set_options(
                &lock_key,
                &lock_id,
                redis_crate::SetOptions::default()
                    .conditional_set(redis_crate::ExistenceCheck::NX)
                    .with_expiration(redis_crate::SetExpiry::PX(ttl_ms as usize)),
            )
            .await
            .map_err(|e| AccountMappingError::Internal {
                message: format!("Redis lock acquisition failed: {}", e),
            })?;

        if result {
            Ok(LockGuard {
                key: key.to_string(),
                lock_id,
                acquired_at: SystemTime::now(),
                ttl,
            })
        } else {
            Err(AccountMappingError::LockContention {
                resource: key.to_string(),
                holder: "unknown".to_string(),
            })
        }
    }

    async fn release_lock(&self, lock: LockGuard) -> AccountMappingResult<()> {
        use redis_crate::AsyncCommands;

        let lock_key = self.make_key(&lock.key);
        let mut conn = self.client.clone();

        // Use Lua script to ensure atomic check-and-delete
        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
        "#;

        let result: i32 = redis_crate::Script::new(script)
            .key(&lock_key)
            .arg(&lock.lock_id)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AccountMappingError::Internal {
                message: format!("Redis lock release failed: {}", e),
            })?;

        if result == 1 {
            Ok(())
        } else {
            // Lock already expired or held by someone else
            Ok(())
        }
    }

    async fn is_locked(&self, key: &str) -> AccountMappingResult<bool> {
        use redis_crate::AsyncCommands;

        let lock_key = self.make_key(key);
        let mut conn = self.client.clone();

        let exists: bool =
            conn.exists(&lock_key)
                .await
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Redis exists check failed: {}", e),
                })?;

        Ok(exists)
    }
}

/// Lock manager wrapper with retry logic
pub struct RetryableLockManager<T: DistributedLockManager> {
    inner: T,
    max_retries: u32,
    retry_delay: Duration,
}

impl<T: DistributedLockManager> RetryableLockManager<T> {
    pub fn new(inner: T, max_retries: u32, retry_delay: Duration) -> Self {
        Self {
            inner,
            max_retries,
            retry_delay,
        }
    }

    pub async fn with_lock<F, R>(&self, key: &str, ttl: Duration, f: F) -> AccountMappingResult<R>
    where
        F: FnOnce() -> AccountMappingResult<R>,
    {
        let mut retries = 0;

        loop {
            match self.inner.acquire_lock(key, ttl).await {
                Ok(lock) => {
                    let result = f();
                    let _ = self.inner.release_lock(lock).await;
                    return result;
                }
                Err(AccountMappingError::LockContention { .. }) if retries < self.max_retries => {
                    retries += 1;
                    tokio::time::sleep(self.retry_delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[async_trait::async_trait]
impl<T: DistributedLockManager> DistributedLockManager for RetryableLockManager<T> {
    async fn acquire_lock(&self, key: &str, ttl: Duration) -> AccountMappingResult<LockGuard> {
        let mut retries = 0;

        loop {
            match self.inner.acquire_lock(key, ttl).await {
                Ok(lock) => return Ok(lock),
                Err(AccountMappingError::LockContention { .. }) if retries < self.max_retries => {
                    retries += 1;
                    tokio::time::sleep(self.retry_delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn release_lock(&self, lock: LockGuard) -> AccountMappingResult<()> {
        self.inner.release_lock(lock).await
    }

    async fn is_locked(&self, key: &str) -> AccountMappingResult<bool> {
        self.inner.is_locked(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_lock() {
        let manager = InMemoryLockManager::new();

        // Acquire lock
        let lock = manager
            .acquire_lock("test-key", Duration::from_secs(60))
            .await
            .unwrap();

        assert!(lock.is_valid());

        // Try to acquire same lock (should fail)
        let result = manager
            .acquire_lock("test-key", Duration::from_secs(60))
            .await;

        assert!(result.is_err());

        // Release lock
        manager.release_lock(lock).await.unwrap();

        // Now we should be able to acquire it again
        let lock2 = manager
            .acquire_lock("test-key", Duration::from_secs(60))
            .await
            .unwrap();

        manager.release_lock(lock2).await.unwrap();
    }

    #[tokio::test]
    async fn test_lock_expiration() {
        let manager = InMemoryLockManager::new();

        // Acquire lock with short TTL
        let _lock = manager
            .acquire_lock("test-key", Duration::from_millis(100))
            .await
            .unwrap();

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should be able to acquire lock again
        let lock2 = manager
            .acquire_lock("test-key", Duration::from_secs(60))
            .await
            .unwrap();

        manager.release_lock(lock2).await.unwrap();
    }
}
