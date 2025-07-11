pub mod memory;
pub mod production;
pub mod redis;
pub mod strategy;

pub use memory::MemoryCache;
pub use production::ProductionRedisCache;
pub use redis::RedisCache;
pub use strategy::CacheStrategy;

use crate::config::CacheConfig;
use crate::error::CacheResult;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Multi-level cache layer
#[derive(Debug)]
pub struct CacheLayer {
    memory_cache: Arc<MemoryCache>,
    redis_cache: Option<Arc<RedisCache>>,
    #[cfg(feature = "cache")]
    production_cache: Option<Arc<ProductionRedisCache>>,
    config: CacheConfig,
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub data: T,
    pub created_at: std::time::SystemTime,
    pub ttl: Duration,
}

/// Cache statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    pub memory_hits: u64,
    pub memory_misses: u64,
    pub redis_hits: u64,
    pub redis_misses: u64,
    pub total_requests: u64,
    pub hit_ratio: f64,
    pub memory_size: usize,
    pub redis_connected: bool,
}

impl CacheLayer {
    /// Create a new cache layer
    pub async fn new(config: &CacheConfig) -> CacheResult<Self> {
        // Initialize memory cache
        let memory_cache = Arc::new(MemoryCache::new(&config.memory).await?);

        // Initialize Redis cache
        let redis_cache = if !config.redis.url.is_empty() {
            Some(Arc::new(RedisCache::new(&config.redis).await?))
        } else {
            None
        };

        // Initialize production cache if enabled
        #[cfg(feature = "cache")]
        let production_cache = if !config.redis.url.is_empty() {
            Some(Arc::new(ProductionRedisCache::new(&config.redis).await?))
        } else {
            None
        };

        Ok(Self {
            memory_cache,
            redis_cache,
            #[cfg(feature = "cache")]
            production_cache,
            config: config.clone(),
        })
    }

    /// Get a value from cache
    pub async fn get<T>(&self, key: &str) -> CacheResult<Option<T>>
    where
        T: Clone + Serialize + DeserializeOwned,
    {
        // Try memory cache first
        if let Some(value) = self.memory_cache.get::<T>(key).await? {
            return Ok(Some(value));
        }

        // Try production cache first if available
        #[cfg(feature = "cache")]
        if let Some(production_cache) = &self.production_cache {
            if let Some(value) = production_cache.get::<T>(key).await? {
                // Store in memory cache for faster access next time
                let ttl = Duration::from_secs(self.config.default_ttl_seconds);
                self.memory_cache.set(key, &value, Some(ttl)).await?;
                return Ok(Some(value));
            }
        }

        // Try regular Redis cache if available
        if let Some(redis_cache) = &self.redis_cache {
            if let Some(value) = redis_cache.get::<T>(key).await? {
                // Store in memory cache for faster access next time
                let ttl = Duration::from_secs(self.config.default_ttl_seconds);
                self.memory_cache.set(key, &value, Some(ttl)).await?;
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    /// Set a value in cache
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Duration) -> CacheResult<()>
    where
        T: Clone + Serialize + Send + Sync + 'static,
    {
        match &self.config.strategy {
            crate::config::CacheStrategy::WriteThrough => {
                // Write to both caches simultaneously
                self.memory_cache.set(key, value, Some(ttl)).await?;
                if let Some(redis_cache) = &self.redis_cache {
                    redis_cache.set(key, value, Some(ttl)).await?;
                }
            }
            crate::config::CacheStrategy::WriteBack => {
                // Write to memory cache immediately, Redis later
                self.memory_cache.set(key, value, Some(ttl)).await?;

                // Schedule write-back to Redis (asynchronous)
                if let Some(redis_cache) = &self.redis_cache {
                    let redis_cache = redis_cache.clone();
                    let key = key.to_string();
                    let value = value.clone();

                    tokio::spawn(async move {
                        if let Err(e) = redis_cache.set(&key, &value, Some(ttl)).await {
                            tracing::warn!("Write-back to Redis failed for key '{}': {}", key, e);
                        }
                    });
                }
            }
            crate::config::CacheStrategy::WriteAround => {
                // Write only to Redis, bypass memory cache
                if let Some(redis_cache) = &self.redis_cache {
                    redis_cache.set(key, value, Some(ttl)).await?;
                } else {
                    self.memory_cache.set(key, value, Some(ttl)).await?;
                }
            }
        }

        Ok(())
    }

    /// Delete a value from cache
    pub async fn delete(&self, key: &str) -> CacheResult<()> {
        self.memory_cache.delete(key).await?;
        if let Some(redis_cache) = &self.redis_cache {
            redis_cache.delete(key).await?;
        }
        Ok(())
    }

    /// Check if a key exists in cache
    pub async fn exists(&self, key: &str) -> CacheResult<bool> {
        if self.memory_cache.exists(key).await? {
            return Ok(true);
        }

        if let Some(redis_cache) = &self.redis_cache {
            return redis_cache.exists(key).await;
        }

        Ok(false)
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheResult<CacheStats> {
        let memory_stats = self.memory_cache.get_stats().await?;
        let redis_stats: Option<crate::cache::redis::RedisStats> =
            if let Some(redis_cache) = &self.redis_cache {
                Some(redis_cache.get_stats().await?)
            } else {
                None
            };

        let total_requests = memory_stats.hits
            + memory_stats.misses
            + redis_stats.as_ref().map(|s| s.hits + s.misses).unwrap_or(0);

        let total_hits = memory_stats.hits + redis_stats.as_ref().map(|s| s.hits).unwrap_or(0);

        let hit_ratio = if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(CacheStats {
            memory_hits: memory_stats.hits,
            memory_misses: memory_stats.misses,
            redis_hits: redis_stats.as_ref().map(|s| s.hits).unwrap_or(0),
            redis_misses: redis_stats.as_ref().map(|s| s.misses).unwrap_or(0),
            total_requests,
            hit_ratio,
            memory_size: memory_stats.size,
            redis_connected: redis_stats.is_some(),
        })
    }

    /// Clear all caches
    pub async fn clear(&self) -> CacheResult<()> {
        self.memory_cache.clear().await?;
        if let Some(redis_cache) = &self.redis_cache {
            redis_cache.clear().await?;
        }
        Ok(())
    }

    /// Set with default TTL
    pub async fn set_default<T>(&self, key: &str, value: &T) -> CacheResult<()>
    where
        T: Clone + Serialize + Send + Sync + 'static,
    {
        self.set(
            key,
            value,
            Duration::from_secs(self.config.default_ttl_seconds),
        )
        .await
    }

    /// Get TTL for specific data types
    fn get_ttl_for_type(data_type: &str) -> Duration {
        match data_type {
            "tx" | "block" => Duration::from_secs(3600), // Immutable data: 1 hour
            "system" => Duration::from_secs(30),         // Dynamic data: 30 seconds
            _ => Duration::from_secs(300),               // Default: 5 minutes
        }
    }

    /// Cache typed value with automatic key prefix
    pub async fn cache_typed<T>(&self, data_type: &str, id: &str, value: &T) -> CacheResult<()>
    where
        T: Clone + Serialize + Send + Sync + 'static,
    {
        let key = format!("{data_type}:{id}");
        let ttl = Self::get_ttl_for_type(data_type);
        self.set(&key, value, ttl).await
    }

    /// Get typed value with automatic key prefix
    pub async fn get_typed<T>(&self, data_type: &str, id: &str) -> CacheResult<Option<T>>
    where
        T: Clone + Serialize + DeserializeOwned,
    {
        let key = format!("{data_type}:{id}");
        self.get(&key).await
    }

    /// Flush any pending writes to persistent storage
    pub async fn flush(&self) -> CacheResult<()> {
        // For write-back strategy, this would flush pending writes
        // For now, just ensure memory cache is persisted if needed
        if let Some(_redis_cache) = &self.redis_cache {
            // In a real implementation, we would flush any write-back queue
            // For now, just log that flush was called
            tracing::debug!("Cache flush requested");
        }
        Ok(())
    }
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry
    pub fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            created_at: std::time::SystemTime::now(),
            ttl,
        }
    }

    /// Check if the entry is expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().unwrap_or(Duration::MAX) > self.ttl
    }

    /// Get remaining TTL
    pub fn remaining_ttl(&self) -> Duration {
        let elapsed = self.created_at.elapsed().unwrap_or(Duration::ZERO);
        if elapsed >= self.ttl {
            Duration::ZERO
        } else {
            self.ttl - elapsed
        }
    }
}
