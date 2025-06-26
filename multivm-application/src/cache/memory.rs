use crate::config::MemoryCacheConfig;
use crate::error::{ApplicationError, CacheResult};
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// In-memory cache implementation with LRU eviction
#[derive(Debug)]
pub struct MemoryCache {
    data: Arc<RwLock<HashMap<String, CacheItem>>>,
    config: MemoryCacheConfig,
    stats: Arc<RwLock<MemoryCacheStats>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Cache item with metadata
#[derive(Debug, Clone)]
struct CacheItem {
    data: Vec<u8>, // Serialized data
    created_at: SystemTime,
    ttl: Duration,
    last_accessed: SystemTime,
    size: usize,
}

/// Memory cache statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size: usize,
    pub item_count: usize,
}

impl MemoryCache {
    /// Create a new memory cache
    pub async fn new(config: &MemoryCacheConfig) -> CacheResult<Self> {
        let data = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(MemoryCacheStats::default()));

        let cache = Self {
            data: data.clone(),
            config: config.clone(),
            stats: stats.clone(),
            cleanup_handle: None,
        };

        // Start cleanup task
        let cleanup_handle = cache.start_cleanup_task().await?;

        Ok(Self {
            data,
            config: config.clone(),
            stats,
            cleanup_handle: Some(cleanup_handle),
        })
    }

    /// Get a value from cache
    pub async fn get<T>(&self, key: &str) -> CacheResult<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut data = self.data.write();
        let mut stats = self.stats.write();

        if let Some(item) = data.get_mut(key) {
            // Check if expired
            if item.is_expired() {
                data.remove(key);
                stats.misses += 1;
                return Ok(None);
            }

            // Update last accessed time
            item.last_accessed = SystemTime::now();
            stats.hits += 1;

            // Deserialize and return
            let value: T =
                bincode::deserialize(&item.data).map_err(|e| ApplicationError::CacheError {
                    operation: "deserialize".to_string(),
                    message: e.to_string(),
                })?;

            Ok(Some(value))
        } else {
            stats.misses += 1;
            Ok(None)
        }
    }

    /// Set a value in cache
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> CacheResult<()>
    where
        T: Serialize,
    {
        // Serialize the value
        let data = bincode::serialize(value).map_err(|e| ApplicationError::CacheError {
            operation: "serialize".to_string(),
            message: e.to_string(),
        })?;

        let size = data.len();
        let item = CacheItem {
            data,
            created_at: SystemTime::now(),
            ttl: ttl.unwrap_or(Duration::from_secs(300)), // Default 5 min TTL
            last_accessed: SystemTime::now(),
            size,
        };

        let mut cache_data = self.data.write();
        let mut stats = self.stats.write();

        // Check if we need to evict items
        self.maybe_evict(&mut cache_data, &mut stats, size)?;

        // Insert the new item
        let old_size = cache_data.get(key).map(|item| item.size).unwrap_or(0);
        cache_data.insert(key.to_string(), item);

        // Update statistics
        stats.size = stats.size - old_size + size;
        if old_size == 0 {
            stats.item_count += 1;
        }

        Ok(())
    }

    /// Delete a value from cache
    pub async fn delete(&self, key: &str) -> CacheResult<()> {
        let mut data = self.data.write();
        let mut stats = self.stats.write();

        if let Some(item) = data.remove(key) {
            stats.size -= item.size;
            stats.item_count -= 1;
        }

        Ok(())
    }

    /// Check if a key exists in cache
    pub async fn exists(&self, key: &str) -> CacheResult<bool> {
        let mut data = self.data.write();

        if let Some(item) = data.get(key) {
            if item.is_expired() {
                let item_size = item.size;
                data.remove(key);
                let mut stats = self.stats.write();
                stats.size -= item_size;
                stats.item_count -= 1;
                Ok(false)
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    /// Clear all items from cache
    pub async fn clear(&self) -> CacheResult<()> {
        let mut data = self.data.write();
        let mut stats = self.stats.write();

        data.clear();
        stats.size = 0;
        stats.item_count = 0;

        Ok(())
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheResult<MemoryCacheStats> {
        Ok(self.stats.read().clone())
    }

    /// Remove expired items
    pub async fn cleanup_expired(&self) -> CacheResult<usize> {
        let mut data = self.data.write();
        let mut stats = self.stats.write();

        let initial_count = data.len();
        let _initial_size = stats.size;

        data.retain(|_, item| !item.is_expired());

        // Recalculate size
        let new_size: usize = data.values().map(|item| item.size).sum();
        stats.size = new_size;
        stats.item_count = data.len();

        let removed_count = initial_count - data.len();
        Ok(removed_count)
    }

    // Private helper methods

    fn maybe_evict(
        &self,
        data: &mut HashMap<String, CacheItem>,
        stats: &mut MemoryCacheStats,
        new_item_size: usize,
    ) -> CacheResult<()> {
        // Check memory limit
        while stats.size + new_item_size > self.config.max_memory_bytes {
            if let Some(lru_key) = self.find_lru_key(data) {
                if let Some(item) = data.remove(&lru_key) {
                    stats.size -= item.size;
                    stats.item_count -= 1;
                    stats.evictions += 1;
                }
            } else {
                break; // No items to evict
            }
        }

        // Check item count limit
        while data.len() >= self.config.max_items {
            if let Some(lru_key) = self.find_lru_key(data) {
                if let Some(item) = data.remove(&lru_key) {
                    stats.size -= item.size;
                    stats.item_count -= 1;
                    stats.evictions += 1;
                }
            } else {
                break; // No items to evict
            }
        }

        Ok(())
    }

    fn find_lru_key(&self, data: &HashMap<String, CacheItem>) -> Option<String> {
        data.iter()
            .min_by_key(|(_, item)| item.last_accessed)
            .map(|(key, _)| key.clone())
    }

    async fn start_cleanup_task(&self) -> CacheResult<tokio::task::JoinHandle<()>> {
        let data = self.data.clone();
        let stats = self.stats.clone();
        let cleanup_interval = Duration::from_secs(self.config.cleanup_interval_seconds);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Remove expired items
                let mut data_guard = data.write();
                let mut stats_guard = stats.write();

                let initial_count = data_guard.len();
                let _initial_size = stats_guard.size;

                data_guard.retain(|_, item| !item.is_expired());

                // Recalculate size and count
                let new_size: usize = data_guard.values().map(|item| item.size).sum();
                stats_guard.size = new_size;
                stats_guard.item_count = data_guard.len();

                let removed_count = initial_count - data_guard.len();
                if removed_count > 0 {
                    tracing::debug!("Cleaned up {} expired cache items", removed_count);
                }
            }
        });

        Ok(handle)
    }
}

impl CacheItem {
    /// Check if the cache item is expired
    fn is_expired(&self) -> bool {
        self.created_at.elapsed().unwrap_or(Duration::MAX) > self.ttl
    }

    /// Get remaining TTL
    #[allow(dead_code)]
    fn remaining_ttl(&self) -> Duration {
        let elapsed = self.created_at.elapsed().unwrap_or(Duration::ZERO);
        if elapsed >= self.ttl {
            Duration::ZERO
        } else {
            self.ttl - elapsed
        }
    }
}

impl Drop for MemoryCache {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

