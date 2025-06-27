//! Test utilities and common testing functionality
//! 
//! This module provides shared utilities, fixtures, and helper functions
//! that can be used across all tests in the MultiVM project.

use multivm_common::{MultivmConfig, MultivmResult, ProcessId, VmType};
use std::sync::Once;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Mutex;

static INIT: Once = Once::new();

/// Initialize test environment (logging, etc.)
pub fn init_test_environment() {
    INIT.call_once(|| {
        // Initialize test logging
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug");
        }
        
        // Set test mode environment variable
        std::env::set_var("MULTIVM_TEST_MODE", "1");
        
        // Initialize tracing for tests
        let subscriber = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        
        // Ignore error if already initialized
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

/// Test configuration builder with sensible defaults
pub struct TestConfigBuilder {
    config: MultivmConfig,
    temp_dir: TempDir,
}

impl TestConfigBuilder {
    /// Create a new test configuration builder
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut config = MultivmConfig::default();
        
        // Set test-specific defaults
        config.system.data_dir = temp_dir.path().to_path_buf();
        config.database.path = temp_dir.path().join("test.db");
        config.server.rest.port = 0; // Random port
        config.server.graphql.port = 0;
        config.server.websocket.port = 0;
        config.server.admin.port = 0;
        config.cache.redis.enabled = false; // Use memory cache in tests
        
        Self { config, temp_dir }
    }
    
    /// Set a custom data directory
    pub fn with_data_dir(mut self, path: &str) -> Self {
        self.config.system.data_dir = self.temp_dir.path().join(path);
        self
    }
    
    /// Enable specific VMs
    pub fn with_svm_enabled(mut self, enabled: bool) -> Self {
        self.config.blockchain.solana.enable_health_checks = enabled;
        self
    }
    
    pub fn with_evm_enabled(mut self, enabled: bool) -> Self {
        self.config.blockchain.ethereum.enable_health_checks = enabled;
        self
    }
    
    /// Set custom ports for testing
    pub fn with_rest_port(mut self, port: u16) -> Self {
        self.config.server.rest.port = port;
        self
    }
    
    /// Build the final configuration
    pub fn build(self) -> (MultivmConfig, TempDir) {
        (self.config, self.temp_dir)
    }
}

impl Default for TestConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test fixture for creating mock blockchain addresses
pub struct AddressFixtures;

impl AddressFixtures {
    /// Generate a valid Solana address for testing
    pub fn solana_address(seed: u8) -> multivm_account_mapping::addresses::SolanaAddress {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        bytes[31] = seed; // Also set the last byte for uniqueness
        multivm_account_mapping::addresses::SolanaAddress(bytes)
    }
    
    /// Generate a valid Ethereum address for testing
    pub fn ethereum_address(seed: u8) -> multivm_account_mapping::addresses::EthereumAddress {
        let mut bytes = [0u8; 20];
        bytes[0] = seed;
        bytes[19] = seed; // Also set the last byte for uniqueness
        multivm_account_mapping::addresses::EthereumAddress(bytes)
    }
    
    /// Generate multiple unique addresses for testing
    pub fn multiple_solana_addresses(count: usize) -> Vec<multivm_account_mapping::addresses::SolanaAddress> {
        (0..count).map(|i| Self::solana_address(i as u8)).collect()
    }
    
    pub fn multiple_ethereum_addresses(count: usize) -> Vec<multivm_account_mapping::addresses::EthereumAddress> {
        (0..count).map(|i| Self::ethereum_address(i as u8)).collect()
    }
}

/// Test fixture for creating IPC messages
pub struct IpcFixtures;

impl IpcFixtures {
    /// Create a basic ping message
    pub fn ping_message() -> multivm_common::IpcMessage {
        multivm_common::IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            multivm_common::IpcCommand::Ping
        )
    }
    
    /// Create a health check message
    pub fn health_check_message() -> multivm_common::IpcMessage {
        multivm_common::IpcMessage::new(
            ProcessId::Main,
            ProcessId::Ethereum,
            multivm_common::IpcCommand::GetHealth
        )
    }
    
    /// Create a message with timeout
    pub fn message_with_timeout(timeout: Duration) -> multivm_common::IpcMessage {
        multivm_common::IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            multivm_common::IpcCommand::Ping
        ).with_timeout(timeout)
    }
    
    /// Create multiple messages for testing
    pub fn multiple_messages(count: usize) -> Vec<multivm_common::IpcMessage> {
        (0..count).map(|i| {
            let dest = if i % 2 == 0 { ProcessId::Solana } else { ProcessId::Ethereum };
            multivm_common::IpcMessage::new(
                ProcessId::Main,
                dest,
                multivm_common::IpcCommand::Ping
            )
        }).collect()
    }
}

/// Test fixture for creating network messages
pub struct NetworkFixtures;

impl NetworkFixtures {
    /// Create a basic control message
    pub fn control_message() -> multivm_p2p::messages::NetworkMessage {
        multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Control(
                multivm_p2p::messages::ControlMessage::Ping
            ),
            multivm_p2p::messages::MessageSource::NetworkLayer,
            multivm_p2p::messages::MessageTarget::Broadcast,
        )
    }
    
    /// Create an SVM transaction message
    pub fn svm_transaction_message(data: Vec<u8>) -> multivm_p2p::messages::NetworkMessage {
        multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Svm(
                multivm_p2p::messages::SvmMessage::Transaction {
                    transaction_data: Box::new(data),
                    signature: "test_signature".to_string(),
                }
            ),
            multivm_p2p::messages::MessageSource::SvmExecution,
            multivm_p2p::messages::MessageTarget::Broadcast,
        )
    }
    
    /// Create an EVM transaction message
    pub fn evm_transaction_message(data: Vec<u8>) -> multivm_p2p::messages::NetworkMessage {
        multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Evm(
                multivm_p2p::messages::EvmMessage::Transaction {
                    transaction_data: Box::new(data),
                    tx_hash: "0x1234567890abcdef".to_string(),
                }
            ),
            multivm_p2p::messages::MessageSource::EvmExecution,
            multivm_p2p::messages::MessageTarget::Broadcast,
        )
    }
    
    /// Create a discovery message
    pub fn discovery_message() -> multivm_p2p::messages::NetworkMessage {
        multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Discovery(
                multivm_p2p::messages::DiscoveryMessage::Request
            ),
            multivm_p2p::messages::MessageSource::NetworkLayer,
            multivm_p2p::messages::MessageTarget::Broadcast,
        )
    }
}

/// Utilities for async testing
pub struct AsyncTestUtils;

impl AsyncTestUtils {
    /// Run an async test with a timeout
    pub async fn with_timeout<F, T>(
        duration: Duration,
        future: F,
    ) -> Result<T, tokio::time::error::Elapsed>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::time::timeout(duration, future).await
    }
    
    /// Create a test runtime for synchronous tests that need async functionality
    pub fn create_test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2) // Use fewer threads in tests
            .build()
            .expect("Failed to create test runtime")
    }
    
    /// Sleep for a short duration (useful for timing tests)
    pub async fn short_delay() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    /// Sleep for a medium duration
    pub async fn medium_delay() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Mock implementations for testing
pub struct MockUtils;

impl MockUtils {
    /// Create a mock error for testing error handling
    pub fn create_network_error(message: &str) -> multivm_common::MultivmError {
        multivm_common::MultivmError::Network {
            message: message.to_string(),
            endpoint: Some("http://localhost:8899".to_string()),
            retry_after: Some(Duration::from_secs(1)),
        }
    }
    
    /// Create a mock process error
    pub fn create_process_error(process_id: &str, exit_code: Option<i32>) -> multivm_common::MultivmError {
        multivm_common::MultivmError::Process {
            process_id: process_id.to_string(),
            message: "Mock process error".to_string(),
            exit_code,
        }
    }
    
    /// Create a mock configuration error
    pub fn create_config_error(component: &str) -> multivm_common::MultivmError {
        multivm_common::MultivmError::Configuration {
            component: component.to_string(),
            message: "Mock configuration error".to_string(),
            validation_errors: Some(vec!["Invalid value".to_string()]),
        }
    }
}

/// Performance testing utilities
pub struct PerformanceUtils;

impl PerformanceUtils {
    /// Measure the time taken to execute a function
    pub fn measure_time<F, T>(func: F) -> (T, Duration)
    where
        F: FnOnce() -> T,
    {
        let start = std::time::Instant::now();
        let result = func();
        let elapsed = start.elapsed();
        (result, elapsed)
    }
    
    /// Measure the time taken to execute an async function
    pub async fn measure_time_async<F, T>(func: F) -> (T, Duration)
    where
        F: std::future::Future<Output = T>,
    {
        let start = std::time::Instant::now();
        let result = func.await;
        let elapsed = start.elapsed();
        (result, elapsed)
    }
    
    /// Assert that an operation completes within a time limit
    pub fn assert_duration_within(duration: Duration, max_duration: Duration) {
        assert!(
            duration <= max_duration,
            "Operation took {:?}, expected at most {:?}",
            duration,
            max_duration
        );
    }
    
    /// Generate load for performance testing
    pub async fn generate_concurrent_load<F, T>(
        operation: F,
        concurrent_count: usize,
    ) -> Vec<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>> + Send + Sync + 'static,
        T: Send + 'static,
    {
        let mut handles = Vec::new();
        
        for _ in 0..concurrent_count {
            let op = operation();
            let handle = tokio::spawn(op);
            handles.push(handle);
        }
        
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.expect("Task should complete"));
        }
        
        results
    }
}

/// Resource cleanup utilities
pub struct CleanupUtils;

impl CleanupUtils {
    /// Ensure a temporary directory is cleaned up properly
    pub fn cleanup_temp_dir(temp_dir: TempDir) {
        // The TempDir will be automatically cleaned up when dropped,
        // but we can do explicit cleanup here if needed
        drop(temp_dir);
    }
    
    /// Clean up environment variables set during testing
    pub fn cleanup_env_vars() {
        std::env::remove_var("MULTIVM_TEST_MODE");
        std::env::remove_var("RUST_LOG");
    }
    
    /// Reset global state for tests
    pub fn reset_global_state() {
        // Reset any global state that might affect tests
        // This would include clearing caches, resetting counters, etc.
    }
}

/// Assertion utilities for common test patterns
pub struct AssertUtils;

impl AssertUtils {
    /// Assert that a result is an error of a specific type
    pub fn assert_error_type<T>(
        result: Result<T, multivm_common::MultivmError>,
        expected_error_type: &str,
    ) {
        match result {
            Err(error) => {
                let error_string = error.to_string();
                assert!(
                    error_string.contains(expected_error_type),
                    "Expected error containing '{}', got: {}",
                    expected_error_type,
                    error_string
                );
            }
            Ok(_) => panic!("Expected error, but got Ok result"),
        }
    }
    
    /// Assert that two values are approximately equal (for floating point comparisons)
    pub fn assert_approx_eq(a: f64, b: f64, tolerance: f64) {
        let diff = (a - b).abs();
        assert!(
            diff <= tolerance,
            "Values {} and {} differ by {}, which exceeds tolerance {}",
            a,
            b,
            diff,
            tolerance
        );
    }
    
    /// Assert that a duration is within an expected range
    pub fn assert_duration_in_range(
        duration: Duration,
        min: Duration,
        max: Duration,
    ) {
        assert!(
            duration >= min && duration <= max,
            "Duration {:?} is not within range {:?} to {:?}",
            duration,
            min,
            max
        );
    }
    
    /// Assert that a collection contains expected elements
    pub fn assert_contains<T: PartialEq + std::fmt::Debug>(
        collection: &[T],
        expected: &T,
    ) {
        assert!(
            collection.contains(expected),
            "Collection {:?} does not contain expected element {:?}",
            collection,
            expected
        );
    }
}

/// Test data generators
pub struct TestDataGenerators;

impl TestDataGenerators {
    /// Generate test data of specified size
    pub fn generate_bytes(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 256) as u8).collect()
    }
    
    /// Generate a test string of specified length
    pub fn generate_string(length: usize) -> String {
        "a".repeat(length)
    }
    
    /// Generate test JSON data
    pub fn generate_json_object(keys: usize) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for i in 0..keys {
            obj.insert(
                format!("key_{}", i),
                serde_json::Value::String(format!("value_{}", i))
            );
        }
        serde_json::Value::Object(obj)
    }
    
    /// Generate a large test payload for performance testing
    pub fn generate_large_payload(size_mb: usize) -> Vec<u8> {
        vec![0u8; size_mb * 1024 * 1024]
    }
}

/// Thread-safe test counter for generating unique values
pub struct TestCounter {
    counter: Mutex<u64>,
}

impl TestCounter {
    pub fn new() -> Self {
        Self {
            counter: Mutex::new(0),
        }
    }
    
    pub async fn next(&self) -> u64 {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        *counter
    }
    
    pub async fn next_string(&self) -> String {
        format!("test_{}", self.next().await)
    }
}

impl Default for TestCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_address_fixtures() {
        let addr1 = AddressFixtures::solana_address(1);
        let addr2 = AddressFixtures::solana_address(2);
        assert_ne!(addr1, addr2);
        
        let evm_addr1 = AddressFixtures::ethereum_address(1);
        let evm_addr2 = AddressFixtures::ethereum_address(2);
        assert_ne!(evm_addr1, evm_addr2);
    }
    
    #[test]
    fn test_config_builder() {
        let (config, _temp_dir) = TestConfigBuilder::new()
            .with_rest_port(8080)
            .with_svm_enabled(true)
            .build();
        
        assert_eq!(config.server.rest.port, 8080);
        assert!(config.blockchain.solana.enable_health_checks);
    }
    
    #[tokio::test]
    async fn test_async_utils() {
        let result = AsyncTestUtils::with_timeout(
            Duration::from_millis(100),
            async { "test" }
        ).await;
        
        assert_eq!(result.unwrap(), "test");
    }
    
    #[test]
    fn test_performance_utils() {
        let (result, duration) = PerformanceUtils::measure_time(|| {
            std::thread::sleep(Duration::from_millis(10));
            42
        });
        
        assert_eq!(result, 42);
        assert!(duration >= Duration::from_millis(10));
    }
    
    #[test]
    fn test_data_generators() {
        let bytes = TestDataGenerators::generate_bytes(100);
        assert_eq!(bytes.len(), 100);
        
        let string = TestDataGenerators::generate_string(50);
        assert_eq!(string.len(), 50);
        
        let json = TestDataGenerators::generate_json_object(3);
        assert!(json.is_object());
        assert_eq!(json.as_object().unwrap().len(), 3);
    }
}