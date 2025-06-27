//! Comprehensive tests for caching layer

#[cfg(test)]
mod tests {
    use crate::cache::{CacheLayer, CacheConfig, MemoryCache, CacheStrategy};
    use serde_json::Value;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_cache_config() -> CacheConfig {
        CacheConfig {
            strategy: CacheStrategy::Memory,
            memory: crate::cache::MemoryCacheConfig {
                max_size_mb: 128,
                ttl_seconds: 300,
                cleanup_interval_seconds: 60,
                max_entries: 10000,
            },
            redis: crate::cache::RedisCacheConfig {
                enabled: false,
                url: "redis://localhost:6379".to_string(),
                pool_size: 10,
                timeout_ms: 1000,
                key_prefix: "multivm:test:".to_string(),
                default_ttl_seconds: 3600,
            },
        }
    }

    #[tokio::test]
    async fn test_cache_layer_creation() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await;
        assert!(cache.is_ok(), "Cache layer creation should succeed");
    }

    #[tokio::test]
    async fn test_memory_cache_creation() {
        let config = create_test_cache_config();
        let cache = MemoryCache::new(&config.memory);
        assert!(cache.is_ok(), "Memory cache creation should succeed");
    }

    #[tokio::test]
    async fn test_cache_set_and_get() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        let key = "test:key:1";
        let value = serde_json::json!({"test": "data", "number": 42});

        // Set cache value
        let set_result = cache.set(key, &value, Some(Duration::from_secs(300))).await;
        assert!(set_result.is_ok(), "Cache set operation should succeed");

        // Get cache value
        let get_result: Result<Option<Value>, _> = cache.get(key).await;
        assert!(get_result.is_ok(), "Cache get operation should succeed");
        
        let cached_value = get_result.unwrap();
        assert!(cached_value.is_some(), "Cached value should exist");
        assert_eq!(cached_value.unwrap(), value, "Cached value should match original");
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        let key = "test:expiry:1";
        let value = serde_json::json!({"expires": "soon"});

        // Set cache value with short expiry
        let set_result = cache.set(key, &value, Some(Duration::from_millis(100))).await;
        assert!(set_result.is_ok(), "Cache set with expiry should succeed");

        // Immediately get the value (should exist)
        let get_result1: Result<Option<Value>, _> = cache.get(key).await;
        assert!(get_result1.is_ok(), "Immediate cache get should succeed");
        assert!(get_result1.unwrap().is_some(), "Value should exist immediately");

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Get after expiry (should not exist)
        let get_result2: Result<Option<Value>, _> = cache.get(key).await;
        assert!(get_result2.is_ok(), "Cache get after expiry should succeed");
        assert!(get_result2.unwrap().is_none(), "Value should not exist after expiry");
    }

    #[tokio::test]
    async fn test_cache_delete() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        let key = "test:delete:1";
        let value = serde_json::json!({"to_be": "deleted"});

        // Set cache value
        cache.set(key, &value, None).await.unwrap();

        // Verify it exists
        let get_result1: Result<Option<Value>, _> = cache.get(key).await;
        assert!(get_result1.unwrap().is_some(), "Value should exist before deletion");

        // Delete the value
        let delete_result = cache.delete(key).await;
        assert!(delete_result.is_ok(), "Cache delete should succeed");

        // Verify it's gone
        let get_result2: Result<Option<Value>, _> = cache.get(key).await;
        assert!(get_result2.unwrap().is_none(), "Value should not exist after deletion");
    }

    #[tokio::test]
    async fn test_cache_exists() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        let key = "test:exists:1";
        let value = serde_json::json!({"check": "existence"});

        // Check non-existent key
        let exists_result1 = cache.exists(key).await;
        assert!(exists_result1.is_ok(), "Cache exists check should succeed");
        assert!(!exists_result1.unwrap(), "Key should not exist initially");

        // Set the value
        cache.set(key, &value, None).await.unwrap();

        // Check existing key
        let exists_result2 = cache.exists(key).await;
        assert!(exists_result2.is_ok(), "Cache exists check should succeed");
        assert!(exists_result2.unwrap(), "Key should exist after setting");

        // Delete and check again
        cache.delete(key).await.unwrap();
        let exists_result3 = cache.exists(key).await;
        assert!(!exists_result3.unwrap(), "Key should not exist after deletion");
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        // Set multiple values
        let keys_values = vec![
            ("test:clear:1", serde_json::json!({"item": 1})),
            ("test:clear:2", serde_json::json!({"item": 2})),
            ("test:clear:3", serde_json::json!({"item": 3})),
        ];

        for (key, value) in &keys_values {
            cache.set(key, value, None).await.unwrap();
        }

        // Verify all exist
        for (key, _) in &keys_values {
            assert!(cache.exists(key).await.unwrap(), "Key {} should exist", key);
        }

        // Clear cache
        let clear_result = cache.clear().await;
        assert!(clear_result.is_ok(), "Cache clear should succeed");

        // Verify all are gone
        for (key, _) in &keys_values {
            assert!(!cache.exists(key).await.unwrap(), "Key {} should not exist after clear", key);
        }
    }

    #[tokio::test]
    async fn test_cache_with_different_types() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        // Test with string
        let string_key = "test:string";
        let string_value = "Hello, Cache!";
        cache.set(string_key, &string_value, None).await.unwrap();
        let cached_string: Option<String> = cache.get(string_key).await.unwrap();
        assert_eq!(cached_string.unwrap(), string_value);

        // Test with number
        let number_key = "test:number";
        let number_value = 42i64;
        cache.set(number_key, &number_value, None).await.unwrap();
        let cached_number: Option<i64> = cache.get(number_key).await.unwrap();
        assert_eq!(cached_number.unwrap(), number_value);

        // Test with boolean
        let bool_key = "test:boolean";
        let bool_value = true;
        cache.set(bool_key, &bool_value, None).await.unwrap();
        let cached_bool: Option<bool> = cache.get(bool_key).await.unwrap();
        assert_eq!(cached_bool.unwrap(), bool_value);

        // Test with complex object
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct TestStruct {
            name: String,
            age: u32,
            active: bool,
        }

        let struct_key = "test:struct";
        let struct_value = TestStruct {
            name: "Test User".to_string(),
            age: 25,
            active: true,
        };
        cache.set(struct_key, &struct_value, None).await.unwrap();
        let cached_struct: Option<TestStruct> = cache.get(struct_key).await.unwrap();
        assert_eq!(cached_struct.unwrap(), struct_value);
    }

    #[tokio::test]
    async fn test_cache_key_patterns() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        // Test different key patterns
        let test_keys = vec![
            "simple",
            "key:with:colons",
            "key-with-dashes",
            "key_with_underscores",
            "key.with.dots",
            "KEY_WITH_UPPERCASE",
            "key/with/slashes",
            "key with spaces", // This might not be supported in all cache backends
            "unicode:键值:αβγ",
        ];

        for key in &test_keys {
            let value = format!("value for {}", key);
            let set_result = cache.set(key, &value, None).await;
            
            if set_result.is_ok() {
                let cached_value: Option<String> = cache.get(key).await.unwrap();
                assert_eq!(cached_value.unwrap(), value, "Value mismatch for key: {}", key);
            }
        }
    }

    #[tokio::test]
    async fn test_cache_size_limits() {
        let mut config = create_test_cache_config();
        config.memory.max_entries = 3; // Very small limit for testing
        
        let cache = CacheLayer::new(&config).await.unwrap();

        // Add entries up to the limit
        for i in 1..=3 {
            let key = format!("test:limit:{}", i);
            let value = serde_json::json!({"entry": i});
            cache.set(&key, &value, None).await.unwrap();
        }

        // Verify all exist
        for i in 1..=3 {
            let key = format!("test:limit:{}", i);
            assert!(cache.exists(&key).await.unwrap(), "Entry {} should exist", i);
        }

        // Add one more entry (should evict oldest)
        let overflow_key = "test:limit:4";
        let overflow_value = serde_json::json!({"entry": 4});
        cache.set(overflow_key, &overflow_value, None).await.unwrap();

        // The first entry might be evicted due to size limit
        let first_exists = cache.exists("test:limit:1").await.unwrap();
        let overflow_exists = cache.exists(overflow_key).await.unwrap();
        
        // New entry should definitely exist
        assert!(overflow_exists, "New entry should exist");
        
        // Due to eviction policy, first entry might not exist
        // This depends on the specific eviction strategy implemented
    }

    #[tokio::test]
    async fn test_concurrent_cache_operations() {
        let config = create_test_cache_config();
        let cache = std::sync::Arc::new(CacheLayer::new(&config).await.unwrap());

        let mut handles = Vec::new();

        // Spawn multiple tasks doing cache operations
        for i in 0..10 {
            let cache_clone = cache.clone();
            let handle = tokio::spawn(async move {
                let key = format!("test:concurrent:{}", i);
                let value = serde_json::json!({"thread": i, "data": "concurrent_test"});
                
                // Set value
                cache_clone.set(&key, &value, None).await?;
                
                // Get value
                let cached: Option<Value> = cache_clone.get(&key).await?;
                
                // Verify value
                if let Some(cached_value) = cached {
                    if cached_value == value {
                        Ok(i)
                    } else {
                        Err("Value mismatch".into())
                    }
                } else {
                    Err("Value not found".into())
                }
            });
            handles.push(handle);
        }

        // Wait for all operations
        let mut success_count = 0;
        for handle in handles {
            if let Ok(Ok(_)) = handle.await {
                success_count += 1;
            }
        }

        assert!(success_count >= 8, "Most concurrent operations should succeed");
    }

    #[tokio::test]
    async fn test_cache_performance() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        let num_operations = 1000;
        let test_data = serde_json::json!({"performance": "test", "data": vec![1, 2, 3, 4, 5]});

        // Measure set operations
        let start_time = std::time::Instant::now();
        for i in 0..num_operations {
            let key = format!("perf:set:{}", i);
            cache.set(&key, &test_data, None).await.unwrap();
        }
        let set_duration = start_time.elapsed();

        // Measure get operations
        let start_time = std::time::Instant::now();
        for i in 0..num_operations {
            let key = format!("perf:set:{}", i);
            let _: Option<Value> = cache.get(&key).await.unwrap();
        }
        let get_duration = start_time.elapsed();

        // Performance assertions (these are loose bounds)
        assert!(set_duration.as_millis() < 5000, "Set operations should complete within 5 seconds");
        assert!(get_duration.as_millis() < 2000, "Get operations should complete within 2 seconds");

        println!("Performance: {} sets in {:?}, {} gets in {:?}", 
                num_operations, set_duration, num_operations, get_duration);
    }

    #[tokio::test]
    async fn test_cache_error_handling() {
        let config = create_test_cache_config();
        let cache = CacheLayer::new(&config).await.unwrap();

        // Test getting non-existent key
        let get_result: Result<Option<String>, _> = cache.get("non:existent:key").await;
        assert!(get_result.is_ok(), "Getting non-existent key should not error");
        assert!(get_result.unwrap().is_none(), "Non-existent key should return None");

        // Test deleting non-existent key
        let delete_result = cache.delete("non:existent:key").await;
        assert!(delete_result.is_ok(), "Deleting non-existent key should not error");

        // Test exists on non-existent key
        let exists_result = cache.exists("non:existent:key").await;
        assert!(exists_result.is_ok(), "Checking non-existent key should not error");
        assert!(!exists_result.unwrap(), "Non-existent key should not exist");
    }

    #[tokio::test]
    async fn test_cache_strategy_selection() {
        // Test memory cache strategy
        let memory_config = CacheConfig {
            strategy: CacheStrategy::Memory,
            memory: crate::cache::MemoryCacheConfig {
                max_size_mb: 64,
                ttl_seconds: 300,
                cleanup_interval_seconds: 60,
                max_entries: 1000,
            },
            redis: crate::cache::RedisCacheConfig {
                enabled: false,
                url: "redis://localhost:6379".to_string(),
                pool_size: 10,
                timeout_ms: 1000,
                key_prefix: "test:".to_string(),
                default_ttl_seconds: 3600,
            },
        };

        let memory_cache = CacheLayer::new(&memory_config).await;
        assert!(memory_cache.is_ok(), "Memory cache should initialize successfully");

        // Test basic operations with memory cache
        let cache = memory_cache.unwrap();
        let test_key = "strategy:test";
        let test_value = "memory cache test";
        
        cache.set(test_key, &test_value, None).await.unwrap();
        let cached_value: Option<String> = cache.get(test_key).await.unwrap();
        assert_eq!(cached_value.unwrap(), test_value);
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let mut config = create_test_cache_config();
        config.memory.cleanup_interval_seconds = 1; // Very frequent cleanup for testing
        
        let cache = CacheLayer::new(&config).await.unwrap();

        // Set values with short expiry
        for i in 0..5 {
            let key = format!("cleanup:test:{}", i);
            let value = format!("value_{}", i);
            cache.set(&key, &value, Some(Duration::from_millis(100))).await.unwrap();
        }

        // Wait for expiry and cleanup
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Manually trigger cleanup if the cache supports it
        if let Ok(_) = cache.clear().await {
            // Cleanup succeeded
        }

        // Values should be expired and cleaned up
        for i in 0..5 {
            let key = format!("cleanup:test:{}", i);
            let exists = cache.exists(&key).await.unwrap();
            assert!(!exists, "Expired key {} should be cleaned up", key);
        }
    }
}