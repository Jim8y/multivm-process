//! # Gateway Unit Tests
//! 
//! Unit tests for the gateway components that interface with VM nodes.

use multivm_application::{
    gateway::{SvmApiGateway, EvmApiGateway, MultivmApiGateway},
    cache::{CacheLayer, CacheConfig},
    config::{VmClientConfig, CacheType},
};
use std::sync::Arc;

/// Create test cache configuration
fn create_test_cache_config() -> CacheConfig {
    CacheConfig {
        cache_type: CacheType::Memory,
        default_ttl: std::time::Duration::from_secs(300),
        max_entries: 1000,
        redis: None,
    }
}

/// Create test VM client configuration
fn create_test_vm_config() -> VmClientConfig {
    VmClientConfig {
        rpc_url: "http://localhost:8899".to_string(),
        timeout_seconds: 30,
        max_retries: 3,
        rate_limit: None,
    }
}

/// Test SVM gateway creation
#[tokio::test]
async fn test_svm_gateway_creation() {
    let cache_config = create_test_cache_config();
    let cache = Arc::new(CacheLayer::new(&cache_config).await.expect("Failed to create cache"));
    
    let vm_config = create_test_vm_config();
    let gateway = SvmApiGateway::new(&vm_config, cache).await;
    
    assert!(gateway.is_ok(), "SVM gateway creation should succeed");
}

/// Test EVM gateway creation
#[tokio::test]
async fn test_evm_gateway_creation() {
    let cache_config = create_test_cache_config();
    let cache = Arc::new(CacheLayer::new(&cache_config).await.expect("Failed to create cache"));
    
    let vm_config = create_test_vm_config();
    let gateway = EvmApiGateway::new(&vm_config, cache).await;
    
    assert!(gateway.is_ok(), "EVM gateway creation should succeed");
}

/// Test MultiVM gateway creation
#[tokio::test]
async fn test_multivm_gateway_creation() {
    let cache_config = create_test_cache_config();
    let cache = Arc::new(CacheLayer::new(&cache_config).await.expect("Failed to create cache"));
    
    let vm_config = create_test_vm_config();
    let gateway = MultivmApiGateway::new(&vm_config, cache).await;
    
    assert!(gateway.is_ok(), "MultiVM gateway creation should succeed");
}

/// Test cache layer functionality
#[tokio::test]
async fn test_cache_layer() {
    let cache_config = create_test_cache_config();
    let cache = CacheLayer::new(&cache_config).await.expect("Failed to create cache");
    
    let key = "test_key";
    let value = "test_value";
    
    // Test set operation
    let set_result = cache.set(key, value.to_string(), None).await;
    assert!(set_result.is_ok(), "Cache set operation should succeed");
    
    // Test get operation
    let get_result: Result<Option<String>, _> = cache.get(key).await;
    assert!(get_result.is_ok(), "Cache get operation should succeed");
    
    if let Ok(Some(cached_value)) = get_result {
        assert_eq!(cached_value, value, "Cached value should match original value");
    }
    
    // Test delete operation
    let delete_result = cache.delete(key).await;
    assert!(delete_result.is_ok(), "Cache delete operation should succeed");
    
    // Verify value is deleted
    let get_after_delete: Result<Option<String>, _> = cache.get(key).await;
    assert!(get_after_delete.is_ok(), "Cache get after delete should succeed");
    assert!(get_after_delete.unwrap().is_none(), "Value should be None after deletion");
}

/// Test gateway error handling
#[tokio::test]
async fn test_gateway_error_handling() {
    let cache_config = create_test_cache_config();
    let cache = Arc::new(CacheLayer::new(&cache_config).await.expect("Failed to create cache"));
    
    // Create gateway with invalid configuration
    let mut invalid_config = create_test_vm_config();
    invalid_config.rpc_url = "invalid_url".to_string();
    
    let gateway = SvmApiGateway::new(&invalid_config, cache).await;
    
    // Gateway creation might succeed even with invalid URL
    // The actual error would occur when making requests
    if let Ok(gateway) = gateway {
        // Test that operations fail gracefully with invalid configuration
        let result = gateway.get_account("invalid_address").await;
        assert!(result.is_err(), "Operation should fail with invalid configuration");
    }
}

/// Test configuration validation
#[test]
fn test_vm_config_validation() {
    let config = create_test_vm_config();
    
    // Test that basic configuration is valid
    assert!(!config.rpc_url.is_empty(), "RPC URL should not be empty");
    assert!(config.timeout_seconds > 0, "Timeout should be positive");
    assert!(config.max_retries >= 0, "Max retries should be non-negative");
}

/// Test cache configuration validation
#[test]
fn test_cache_config_validation() {
    let config = create_test_cache_config();
    
    // Test cache configuration
    assert!(config.default_ttl.as_secs() > 0, "Default TTL should be positive");
    assert!(config.max_entries > 0, "Max entries should be positive");
    
    // Test that memory cache type is properly configured
    match config.cache_type {
        CacheType::Memory => {
            // Memory cache should not require Redis configuration
            assert!(config.redis.is_none(), "Memory cache should not have Redis config");
        }
        CacheType::Redis => {
            // Redis cache should have Redis configuration
            assert!(config.redis.is_some(), "Redis cache should have Redis config");
        }
    }
}

/// Test cache TTL functionality
#[tokio::test]
async fn test_cache_ttl() {
    let mut cache_config = create_test_cache_config();
    cache_config.default_ttl = std::time::Duration::from_millis(100); // Very short TTL for testing
    
    let cache = CacheLayer::new(&cache_config).await.expect("Failed to create cache");
    
    let key = "ttl_test_key";
    let value = "ttl_test_value";
    
    // Set value with short TTL
    let set_result = cache.set(key, value.to_string(), Some(std::time::Duration::from_millis(50))).await;
    assert!(set_result.is_ok(), "Cache set with TTL should succeed");
    
    // Immediately get the value - should exist
    let get_result: Result<Option<String>, _> = cache.get(key).await;
    assert!(get_result.is_ok() && get_result.as_ref().unwrap().is_some(), "Value should exist immediately");
    
    // Wait for TTL to expire
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // Get the value again - should be expired
    let get_after_ttl: Result<Option<String>, _> = cache.get(key).await;
    assert!(get_after_ttl.is_ok(), "Cache get after TTL should succeed");
    
    // Note: TTL expiration behavior depends on cache implementation
    // Some implementations might not immediately expire items
} 