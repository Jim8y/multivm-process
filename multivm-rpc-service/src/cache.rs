//! Caching system for RPC responses

use crate::{
    error::{RpcError, RpcResult},
    types::{CacheEntry, JsonRpcRequest, JsonRpcResponse},
};
use moka::future::Cache;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing::{debug, trace, warn};

/// RPC response cache
pub struct RpcCache {
    /// Main cache storage
    cache: Cache<String, CacheEntry>,
    /// Method-specific TTL configuration
    method_ttls: HashMap<String, Duration>,
    /// Default TTL for methods not in configuration
    default_ttl: Duration,
    /// Enable/disable caching
    enabled: bool,
}

impl RpcCache {
    /// Create new RPC cache
    pub fn new(max_size: u64, default_ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_size)
            .time_to_live(default_ttl)
            .build();

        Self {
            cache,
            method_ttls: HashMap::new(),
            default_ttl,
            enabled: true,
        }
    }

    /// Create cache with method-specific TTLs
    pub fn with_method_ttls(
        max_size: u64,
        default_ttl: Duration,
        method_ttls: HashMap<String, Duration>,
    ) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_size)
            .time_to_live(default_ttl)
            .build();

        Self {
            cache,
            method_ttls,
            default_ttl,
            enabled: true,
        }
    }

    /// Check if request is cacheable
    pub fn is_cacheable(&self, request: &JsonRpcRequest) -> bool {
        if !self.enabled {
            return false;
        }

        // Only cache read-only methods
        if !request.is_read_only() {
            return false;
        }

        // Check specific methods that should not be cached
        match request.method.as_str() {
            // Ethereum methods that should not be cached
            "eth_sendTransaction" | "eth_sendRawTransaction" | "eth_sign" |
            "eth_signTransaction" | "eth_newFilter" | "eth_newBlockFilter" |
            "eth_newPendingTransactionFilter" | "eth_uninstallFilter" |
            "eth_getFilterChanges" | "eth_getFilterLogs" |
            // Solana methods that should not be cached
            "sendTransaction" | "simulateTransaction" |
            // Subscription methods
            "accountSubscribe" | "accountUnsubscribe" |
            "logsSubscribe" | "logsUnsubscribe" |
            "programSubscribe" | "programUnsubscribe" |
            "signatureSubscribe" | "signatureUnsubscribe" |
            "slotSubscribe" | "slotUnsubscribe" |
            "slotsUpdatesSubscribe" | "slotsUpdatesUnsubscribe" |
            "rootSubscribe" | "rootUnsubscribe" |
            "voteSubscribe" | "voteUnsubscribe" => false,
            _ => true,
        }
    }

    /// Get cached response
    pub async fn get(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        if !self.is_cacheable(request) {
            return None;
        }

        let cache_key = request.cache_key();
        
        match self.cache.get(&cache_key).await {
            Some(entry) => {
                if entry.is_expired() {
                    debug!("Cache entry expired for method: {}", request.method);
                    self.cache.invalidate(&cache_key).await;
                    None
                } else {
                    trace!("Cache hit for method: {}", request.method);
                    Some(JsonRpcResponse::success(
                        request.id.clone(),
                        entry.data,
                    ))
                }
            }
            None => {
                trace!("Cache miss for method: {}", request.method);
                None
            }
        }
    }

    /// Store response in cache
    pub async fn put(&self, request: &JsonRpcRequest, response: &JsonRpcResponse) -> RpcResult<()> {
        if !self.is_cacheable(request) {
            return Ok(());
        }

        // Only cache successful responses
        if response.error.is_some() {
            return Ok(());
        }

        let cache_key = request.cache_key();
        let ttl = self.get_method_ttl(&request.method);

        let entry = CacheEntry {
            data: response.result.clone().unwrap_or(Value::Null),
            cached_at: SystemTime::now(),
            ttl,
            request_hash: cache_key.clone(),
        };

        debug!("Caching response for method: {} (TTL: {:?})", request.method, ttl);
        self.cache.insert(cache_key, entry).await;

        Ok(())
    }

    /// Get TTL for specific method
    fn get_method_ttl(&self, method: &str) -> Duration {
        self.method_ttls
            .get(method)
            .copied()
            .unwrap_or(self.default_ttl)
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, request: &JsonRpcRequest) {
        let cache_key = request.cache_key();
        self.cache.invalidate(&cache_key).await;
        debug!("Invalidated cache for method: {}", request.method);
    }

    /// Invalidate all cache entries for a method
    pub async fn invalidate_method(&self, method: &str) {
        // Note: This is inefficient for large caches
        // In production, consider using cache tags or separate caches per method
        warn!("Invalidating all entries for method: {} (this may be slow)", method);
        
        // For now, we'll just clear the entire cache if needed
        // A more sophisticated implementation would track method->keys mapping
    }

    /// Clear entire cache
    pub async fn clear(&self) {
        self.cache.invalidate_all();
        debug!("Cleared entire cache");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let entry_count = self.cache.entry_count();
        let weighted_size = self.cache.weighted_size();

        CacheStats {
            entry_count,
            weighted_size,
            hit_rate: 0.0, // moka doesn't provide hit rate directly
            enabled: self.enabled,
            default_ttl: self.default_ttl,
            method_count: self.method_ttls.len(),
        }
    }

    /// Update cache configuration
    pub fn update_config(&mut self, enabled: bool, method_ttls: HashMap<String, Duration>) {
        self.enabled = enabled;
        self.method_ttls = method_ttls;
        debug!("Updated cache configuration: enabled={}, methods={}", enabled, self.method_ttls.len());
    }

    /// Enable/disable cache
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        debug!("Cache enabled: {}", enabled);
    }
    
    /// Check if cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Add method TTL
    pub fn add_method_ttl(&mut self, method: String, ttl: Duration) {
        self.method_ttls.insert(method.clone(), ttl);
        debug!("Added cache TTL for method {}: {:?}", method, ttl);
    }

    /// Remove method TTL
    pub fn remove_method_ttl(&mut self, method: &str) {
        self.method_ttls.remove(method);
        debug!("Removed cache TTL for method: {}", method);
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: u64,
    pub weighted_size: u64,
    pub hit_rate: f64,
    pub enabled: bool,
    pub default_ttl: Duration,
    pub method_count: usize,
}

impl CacheStats {
    /// Get cache utilization percentage
    pub fn utilization_percentage(&self, max_size: u64) -> f64 {
        if max_size == 0 {
            return 0.0;
        }
        (self.entry_count as f64 / max_size as f64) * 100.0
    }
}

/// Cache invalidation strategies
pub enum InvalidationStrategy {
    /// Time-based expiration (TTL)
    TimeToLive(Duration),
    /// Manual invalidation
    Manual,
    /// Event-based invalidation
    EventBased,
}

/// Smart cache that can invalidate based on blockchain events
pub struct SmartCache {
    cache: RpcCache,
    /// Track block-dependent methods
    block_dependent_methods: HashMap<String, InvalidationStrategy>,
}

impl SmartCache {
    pub fn new(max_size: u64, default_ttl: Duration) -> Self {
        let mut block_dependent_methods = HashMap::new();
        
        // Ethereum block-dependent methods
        block_dependent_methods.insert(
            "eth_blockNumber".to_string(),
            InvalidationStrategy::TimeToLive(Duration::from_secs(1)),
        );
        block_dependent_methods.insert(
            "eth_getBalance".to_string(),
            InvalidationStrategy::TimeToLive(Duration::from_secs(10)),
        );
        block_dependent_methods.insert(
            "eth_getTransactionCount".to_string(),
            InvalidationStrategy::TimeToLive(Duration::from_secs(10)),
        );

        // Solana slot-dependent methods
        block_dependent_methods.insert(
            "getSlot".to_string(),
            InvalidationStrategy::TimeToLive(Duration::from_secs(1)),
        );
        block_dependent_methods.insert(
            "getBalance".to_string(),
            InvalidationStrategy::TimeToLive(Duration::from_secs(10)),
        );

        Self {
            cache: RpcCache::new(max_size, default_ttl),
            block_dependent_methods,
        }
    }

    /// Invalidate cache based on new block/slot
    pub async fn invalidate_on_block_update(&self, vm_type: crate::types::VmType) {
        match vm_type {
            crate::types::VmType::Ethereum => {
                debug!("Invalidating Ethereum block-dependent cache entries");
                // In a real implementation, you'd invalidate specific entries
                // For now, we'll just log the event
            }
            crate::types::VmType::Solana => {
                debug!("Invalidating Solana slot-dependent cache entries");
                // In a real implementation, you'd invalidate specific entries
            }
            crate::types::VmType::MultiVm => {
                debug!("Invalidating MultiVM cache entries");
            }
        }
    }

    /// Get the underlying cache
    pub fn cache(&self) -> &RpcCache {
        &self.cache
    }

    /// Get mutable access to the underlying cache
    pub fn cache_mut(&mut self) -> &mut RpcCache {
        &mut self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let mut cache = RpcCache::new(100, Duration::from_secs(60));

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let response = JsonRpcResponse::success(
            Some(json!(1)),
            json!("0x123"),
        );

        // Test cache miss
        assert!(cache.get(&request).await.is_none());

        // Test cache put
        cache.put(&request, &response).await.unwrap();

        // Test cache hit
        let cached_response = cache.get(&request).await;
        assert!(cached_response.is_some());
        assert_eq!(cached_response.unwrap().result, Some(json!("0x123")));
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let mut method_ttls = HashMap::new();
        method_ttls.insert("test_method".to_string(), Duration::from_millis(10));

        let mut cache = RpcCache::with_method_ttls(100, Duration::from_secs(60), method_ttls);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test_method".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let response = JsonRpcResponse::success(
            Some(json!(1)),
            json!("test_value"),
        );

        // Cache the response
        cache.put(&request, &response).await.unwrap();

        // Should be available immediately
        assert!(cache.get(&request).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be expired now
        assert!(cache.get(&request).await.is_none());
    }

    #[test]
    fn test_cacheability() {
        let cache = RpcCache::new(100, Duration::from_secs(60));

        // Read-only method should be cacheable
        let read_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_getBalance".to_string(),
            params: None,
            id: Some(json!(1)),
        };
        assert!(cache.is_cacheable(&read_request));

        // Write method should not be cacheable
        let write_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendTransaction".to_string(),
            params: None,
            id: Some(json!(1)),
        };
        assert!(!cache.is_cacheable(&write_request));

        // Subscription method should not be cacheable
        let sub_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "accountSubscribe".to_string(),
            params: None,
            id: Some(json!(1)),
        };
        assert!(!cache.is_cacheable(&sub_request));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = RpcCache::new(100, Duration::from_secs(60));
        let stats = cache.stats().await;
        
        assert_eq!(stats.entry_count, 0);
        assert!(stats.enabled);
        assert_eq!(stats.default_ttl, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_smart_cache() {
        let smart_cache = SmartCache::new(100, Duration::from_secs(60));
        
        // Test that it has the underlying cache
        assert!(smart_cache.cache().enabled);
        
        // Test block invalidation (should not panic)
        smart_cache.invalidate_on_block_update(crate::types::VmType::Ethereum).await;
        smart_cache.invalidate_on_block_update(crate::types::VmType::Solana).await;
    }
}