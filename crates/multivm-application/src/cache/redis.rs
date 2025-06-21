//! Redis cache implementation

use crate::config::RedisConfig;
use crate::error::ApplicationResult;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(feature = "cache")]
use redis::{Client, Commands, Connection};
#[cfg(feature = "cache")]
use std::sync::Arc;
#[cfg(feature = "cache")]
use tokio::sync::Mutex;

/// Redis cache implementation
pub struct RedisCache {
    config: RedisConfig,
    #[cfg(feature = "cache")]
    client: Arc<Client>,
    #[cfg(feature = "cache")]
    connection: Arc<Mutex<Option<Connection>>>,
    #[cfg(not(feature = "cache"))]
    _phantom: std::marker::PhantomData<()>,
}

impl std::fmt::Debug for RedisCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisCache")
            .field("config", &self.config)
            .finish()
    }
}

/// Redis cache statistics
#[derive(Debug, Clone)]
pub struct RedisStats {
    pub hits: u64,
    pub misses: u64,
}

impl RedisCache {
    /// Create new Redis cache
    pub async fn new(config: &RedisConfig) -> ApplicationResult<Self> {
        tracing::info!("Initializing Redis cache: {}", config.url);

        #[cfg(feature = "cache")]
        {
            // Create Redis client with configuration
            let client = if config.url.is_empty() {
                // Use memory-only cache when Redis URL is not configured
                return Ok(Self {
                    config: config.clone(),
                    client: Arc::new(Client::open("redis://127.0.0.1:6379/").unwrap()), // Dummy client
                    connection: Arc::new(Mutex::new(None)), // No connection for memory mode
                });
            } else {
                Client::open(config.url.as_str())
            }.map_err(|e| crate::error::ApplicationError::CacheError {
                operation: "redis".to_string(),
                message: format!("Failed to create Redis client: {}", e),
            })?;

            // Test connection
            let mut conn = client.get_connection().map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "connect".to_string(),
                    message: format!("Failed to connect to Redis: {}", e),
                }
            })?;

            // Test with a ping
            let _: String = conn.get("__redis_connection_test__").unwrap_or_default();

            Ok(Self {
                config: config.clone(),
                client: Arc::new(client),
                connection: Arc::new(Mutex::new(Some(conn))),
            })
        }
        
        #[cfg(not(feature = "cache"))]
        {
            tracing::warn!("Redis cache feature not enabled, using mock implementation");
            Ok(Self {
                config: config.clone(),
                _phantom: std::marker::PhantomData,
            })
        }
    }

    /// Get value from Redis
    pub async fn get<T>(&self, key: &str) -> ApplicationResult<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis GET: {}", full_key);

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            match conn.get::<_, String>(&full_key) {
                Ok(value) => {
                    match serde_json::from_str::<T>(&value) {
                        Ok(deserialized) => Ok(Some(deserialized)),
                        Err(e) => {
                            tracing::warn!("Failed to deserialize cached value for key {}: {}", full_key, e);
                            Ok(None)
                        }
                    }
                }
                Err(e) if e.kind() == redis::ErrorKind::TypeError => {
                    // Key doesn't exist
                    Ok(None)
                }
                Err(e) => Err(crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: format!("Redis GET error for key {}: {}", full_key, e),
                }),
            }
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always returns None
            Ok(None)
        }
    }

    /// Set value in Redis
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> ApplicationResult<()>
    where
        T: Serialize,
    {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        let ttl_secs = ttl.map(|d| d.as_secs()).unwrap_or(3600);

        tracing::debug!("Redis SET: {} (TTL: {}s)", full_key, ttl_secs);

        #[cfg(feature = "cache")]
        {
            let serialized = serde_json::to_string(value).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "serialize".to_string(),
                    message: format!("Failed to serialize value for key {}: {}", full_key, e),
                }
            })?;

            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            if let Some(_ttl) = ttl {
                conn.set_ex::<_, _, ()>(&full_key, &serialized, ttl_secs)
            } else {
                conn.set::<_, _, ()>(&full_key, &serialized)
            }.map_err(|e| crate::error::ApplicationError::CacheError {
                operation: "redis".to_string(),
                message: format!("Redis SET error for key {}: {}", full_key, e),
            })?;
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always succeeds
            let _ = (value, ttl); // Silence unused variable warnings
        }

        Ok(())
    }

    /// Delete value from Redis
    pub async fn delete(&self, key: &str) -> ApplicationResult<()> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis DEL: {}", full_key);

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            conn.del::<_, ()>(&full_key).map_err(|e| crate::error::ApplicationError::CacheError {
                operation: "redis".to_string(),
                message: format!("Redis DEL error for key {}: {}", full_key, e),
            })?;
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always succeeds
        }

        Ok(())
    }

    /// Clear all values with the configured prefix
    pub async fn clear(&self) -> ApplicationResult<()> {
        tracing::warn!("Redis CLEAR: {}*", self.config.key_prefix);

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            let pattern = format!("{}*", self.config.key_prefix);
            let keys: Vec<String> = conn.keys(&pattern).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "keys".to_string(),
                    message: format!("Redis KEYS error for pattern {}: {}", pattern, e),
                }
            })?;

            if !keys.is_empty() {
                conn.del::<_, ()>(&keys).map_err(|e| crate::error::ApplicationError::CacheError {
                    operation: "del".to_string(),
                    message: format!("Redis DEL error for keys {:?}: {}", keys, e),
                })?;
            }
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always succeeds
        }

        Ok(())
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> ApplicationResult<bool> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis EXISTS: {}", full_key);

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            let exists: bool = conn.exists(&full_key).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "exists".to_string(),
                    message: format!("Redis EXISTS error for key {}: {}", full_key, e),
                }
            })?;

            Ok(exists)
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always returns false
            Ok(false)
        }
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> ApplicationResult<RedisStats> {
        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            // Get Redis INFO stats
            let info: String = redis::cmd("INFO").arg("stats").query(conn).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "info".to_string(),
                    message: format!("Redis INFO error: {}", e),
                }
            })?;

            // Parse hits and misses from INFO output
            let mut hits = 0;
            let mut misses = 0;

            for line in info.lines() {
                if line.starts_with("keyspace_hits:") {
                    hits = line.split(':').nth(1).unwrap_or("0").parse().unwrap_or(0);
                } else if line.starts_with("keyspace_misses:") {
                    misses = line.split(':').nth(1).unwrap_or("0").parse().unwrap_or(0);
                }
            }

            Ok(RedisStats { hits, misses })
        }

        #[cfg(not(feature = "cache"))]
        {
            Ok(RedisStats { hits: 0, misses: 0 })
        }
    }

    /// Batch get multiple keys
    pub async fn mget<T>(&self, keys: &[&str]) -> ApplicationResult<Vec<Option<T>>>
    where
        T: for<'de> Deserialize<'de>,
    {
        tracing::debug!("Redis MGET: {} keys", keys.len());

        if keys.is_empty() {
            return Ok(vec![]);
        }

        #[cfg(feature = "cache")]
        {
            let full_keys: Vec<String> = keys
                .iter()
                .map(|k| format!("{}{}", self.config.key_prefix, k))
                .collect();

            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            let values: Vec<Option<String>> = conn.get(&full_keys).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "mget".to_string(),
                    message: format!("Redis MGET error: {}", e),
                }
            })?;

            let result = values
                .into_iter()
                .map(|opt_val| {
                    opt_val.and_then(|val| serde_json::from_str::<T>(&val).ok())
                })
                .collect();

            Ok(result)
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - returns None for all keys
            Ok(keys.iter().map(|_| None).collect())
        }
    }

    /// Batch set multiple key-value pairs
    pub async fn mset<T>(
        &self,
        items: &[(&str, &T)],
        ttl: Option<Duration>,
    ) -> ApplicationResult<()>
    where
        T: Serialize,
    {
        tracing::debug!("Redis MSET: {} items", items.len());

        if items.is_empty() {
            return Ok(());
        }

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            for (key, value) in items {
                let full_key = format!("{}{}", self.config.key_prefix, key);
                let serialized = serde_json::to_string(value).map_err(|e| {
                    crate::error::ApplicationError::CacheError {
                        operation: "serialize".to_string(),
                    message: format!("Failed to serialize value for key {}: {}", full_key, e),
                    }
                })?;

                if let Some(ttl) = ttl {
                    conn.set_ex::<_, _, ()>(&full_key, &serialized, ttl.as_secs())
                } else {
                    conn.set::<_, _, ()>(&full_key, &serialized)
                }.map_err(|e| crate::error::ApplicationError::CacheError {
                    operation: "set".to_string(),
                    message: format!("Redis SET error for key {}: {}", full_key, e),
                })?;
            }
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always succeeds
            let _ = (items, ttl); // Silence unused variable warnings
        }

        Ok(())
    }

    /// Increment a counter
    pub async fn incr(&self, key: &str, delta: i64) -> ApplicationResult<i64> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis INCRBY: {} {}", full_key, delta);

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            let result: i64 = conn.incr(&full_key, delta).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "incrby".to_string(),
                    message: format!("Redis INCRBY error for key {}: {}", full_key, e),
                }
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - returns the delta
            Ok(delta)
        }
    }

    /// Set with expiration only if not exists
    pub async fn set_nx<T>(&self, key: &str, value: &T, ttl: Duration) -> ApplicationResult<bool>
    where
        T: Serialize,
    {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis SETNX: {} (TTL: {}s)", full_key, ttl.as_secs());

        #[cfg(feature = "cache")]
        {
            let serialized = serde_json::to_string(value).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "serialize".to_string(),
                    message: format!("Failed to serialize value for key {}: {}", full_key, e),
                }
            })?;

            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            // Use SET with NX and EX options
            let result: Option<String> = redis::cmd("SET")
                .arg(&full_key)
                .arg(&serialized)
                .arg("NX")
                .arg("EX")
                .arg(ttl.as_secs())
                .query(conn)
                .map_err(|e| crate::error::ApplicationError::CacheError {
                    operation: "setnx".to_string(),
                    message: format!("Redis SETNX error for key {}: {}", full_key, e),
                })?;

            Ok(result.is_some())
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always succeeds
            let _ = (value, ttl); // Silence unused variable warnings
            Ok(true)
        }
    }

    /// Get remaining TTL for a key
    pub async fn ttl(&self, key: &str) -> ApplicationResult<Option<Duration>> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis TTL: {}", full_key);

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            let ttl_secs: i64 = conn.ttl(&full_key).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "ttl".to_string(),
                    message: format!("Redis TTL error for key {}: {}", full_key, e),
                }
            })?;

            match ttl_secs {
                -2 => Ok(None), // Key doesn't exist
                -1 => Ok(None), // Key exists but has no expiration
                secs if secs > 0 => Ok(Some(Duration::from_secs(secs as u64))),
                _ => Ok(None),
            }
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - returns None
            Ok(None)
        }
    }

    /// Extend TTL for a key
    pub async fn expire(&self, key: &str, ttl: Duration) -> ApplicationResult<bool> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        tracing::debug!("Redis EXPIRE: {} {}s", full_key, ttl.as_secs());

        #[cfg(feature = "cache")]
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard.as_mut().ok_or_else(|| {
                crate::error::ApplicationError::CacheError {
                    operation: "redis".to_string(),
                    message: "Redis connection not available".to_string(),
                }
            })?;

            let result: bool = conn.expire(&full_key, ttl.as_secs() as i64).map_err(|e| {
                crate::error::ApplicationError::CacheError {
                    operation: "expire".to_string(),
                    message: format!("Redis EXPIRE error for key {}: {}", full_key, e),
                }
            })?;

            Ok(result)
        }

        #[cfg(not(feature = "cache"))]
        {
            // Mock implementation - always succeeds
            Ok(true)
        }
    }
}