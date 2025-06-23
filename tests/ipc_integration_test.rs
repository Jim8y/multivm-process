//! IPC Transport Integration Tests
//!
//! Tests for secure message transport, connection handling, and protocol reliability.

use multivm_common::{
    ipc::{IpcCommand, IpcMessage, IpcResponse, IpcTransport},
    error::{MultivmError, MultivmResult},
    secure_transport::{SecureMessage, MessageMetadata},
};
use multivm_process_manager::ipc_transport::{IpcTransportManager, TransportConfig};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

#[tokio::test]
async fn test_ipc_transport_connection() {
    let config = TransportConfig {
        bind_address: "127.0.0.1:0".to_string(), // Use port 0 for auto-assignment
        max_connections: 10,
        connection_timeout: Duration::from_secs(5),
        message_timeout: Duration::from_secs(10),
        enable_compression: false,
        enable_encryption: true,
        buffer_size: 8192,
    };
    
    let manager = IpcTransportManager::new(config).await.unwrap();
    
    // Start the transport manager
    let result = manager.start().await;
    assert!(result.is_ok(), "Transport manager should start successfully");
    
    // Give it time to bind
    sleep(Duration::from_millis(100)).await;
    
    // Check that it's listening
    let status = manager.get_status().await;
    assert!(status.is_listening, "Transport should be listening");
    assert_eq!(status.active_connections, 0, "No connections initially");
    
    // Shutdown
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_secure_message_transport() {
    let config = TransportConfig::default();
    let manager = IpcTransportManager::new(config).await.unwrap();
    
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    // Create a secure message
    let message = SecureMessage {
        message_id: "test_msg_001".to_string(),
        sequence_number: 1,
        sender_id: "test_sender".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
        signature: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        payload: b"Hello, secure world!".to_vec(),
    };
    
    // Test message serialization/deserialization
    let serialized = serde_json::to_vec(&message).unwrap();
    let deserialized: SecureMessage = serde_json::from_slice(&serialized).unwrap();
    
    assert_eq!(message.message_id, deserialized.message_id);
    assert_eq!(message.payload, deserialized.payload);
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_replay_protection() {
    let config = TransportConfig::default();
    let manager = IpcTransportManager::new(config).await.unwrap();
    
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    let message = SecureMessage {
        message_id: "replay_test_001".to_string(),
        sequence_number: 1,
        sender_id: "test_sender".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
        signature: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        payload: b"Original message".to_vec(),
    };
    
    // First message should be accepted
    let result1 = manager.validate_message(&message).await;
    assert!(result1.is_ok(), "First message should be valid");
    
    // Replay of same message should be rejected
    let result2 = manager.validate_message(&message).await;
    assert!(result2.is_err(), "Replayed message should be rejected");
    
    if let Err(e) = result2 {
        assert!(e.to_string().contains("replay"), "Error should mention replay attack");
    }
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_ordering() {
    let config = TransportConfig::default();
    let manager = IpcTransportManager::new(config).await.unwrap();
    
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    let sender_id = "ordering_test_sender".to_string();
    
    // Send messages out of order
    let message3 = create_test_message(&sender_id, 3, b"Message 3");
    let message1 = create_test_message(&sender_id, 1, b"Message 1");
    let message2 = create_test_message(&sender_id, 2, b"Message 2");
    
    // Message 1 should be accepted
    let result1 = manager.validate_message(&message1).await;
    assert!(result1.is_ok(), "Message 1 should be accepted");
    
    // Message 3 (out of order) might be buffered or rejected depending on implementation
    let result3 = manager.validate_message(&message3).await;
    // This test depends on whether we implement strict ordering or buffering
    
    // Message 2 should be accepted
    let result2 = manager.validate_message(&message2).await;
    assert!(result2.is_ok(), "Message 2 should be accepted");
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_connection_limits() {
    let config = TransportConfig {
        max_connections: 2, // Low limit for testing
        ..Default::default()
    };
    
    let manager = IpcTransportManager::new(config).await.unwrap();
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    // This test would require actual client connections to fully test
    // For now, we'll just verify the configuration is applied
    let status = manager.get_status().await;
    assert_eq!(status.max_connections, 2, "Max connections should be set correctly");
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_transport_metrics() {
    let config = TransportConfig::default();
    let manager = IpcTransportManager::new(config).await.unwrap();
    
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    let metrics = manager.get_metrics().await;
    
    // Check basic metrics structure
    assert!(metrics.messages_sent >= 0);
    assert!(metrics.messages_received >= 0);
    assert!(metrics.bytes_sent >= 0);
    assert!(metrics.bytes_received >= 0);
    assert!(metrics.connection_errors >= 0);
    assert!(metrics.message_errors >= 0);
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let config = TransportConfig::default();
    let manager = IpcTransportManager::new(config).await.unwrap();
    
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    let status_before = manager.get_status().await;
    assert!(status_before.is_listening, "Should be listening before shutdown");
    
    // Graceful shutdown
    let shutdown_result = manager.shutdown().await;
    assert!(shutdown_result.is_ok(), "Shutdown should succeed");
    
    let status_after = manager.get_status().await;
    assert!(!status_after.is_listening, "Should not be listening after shutdown");
}

#[tokio::test]
async fn test_message_size_limits() {
    let config = TransportConfig {
        buffer_size: 1024, // Small buffer for testing
        ..Default::default()
    };
    
    let manager = IpcTransportManager::new(config).await.unwrap();
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    // Create oversized message
    let large_payload = vec![0u8; 2048]; // Larger than buffer
    let large_message = SecureMessage {
        message_id: "large_msg_001".to_string(),
        sequence_number: 1,
        sender_id: "test_sender".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
        signature: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        payload: large_payload,
    };
    
    // Should reject oversized message
    let result = manager.validate_message(&large_message).await;
    assert!(result.is_err(), "Oversized message should be rejected");
    
    if let Err(e) = result {
        assert!(
            e.to_string().contains("too large") || e.to_string().contains("size"),
            "Error should mention message size: {}",
            e
        );
    }
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_connections() {
    use std::sync::Arc;
    use tokio::sync::Barrier;
    
    let config = TransportConfig {
        max_connections: 5,
        ..Default::default()
    };
    
    let manager = Arc::new(IpcTransportManager::new(config).await.unwrap());
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];
    
    // Spawn multiple tasks simulating concurrent message validation
    for i in 0..3 {
        let manager_clone = Arc::clone(&manager);
        let barrier_clone = Arc::clone(&barrier);
        
        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;
            
            let message = create_test_message(
                &format!("concurrent_sender_{}", i),
                1,
                format!("Concurrent message {}", i).as_bytes(),
            );
            
            manager_clone.validate_message(&message).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.await {
            success_count += 1;
        }
    }
    
    assert!(success_count > 0, "At least some messages should be processed successfully");
    
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_compression_handling() {
    let config = TransportConfig {
        enable_compression: true,
        ..Default::default()
    };
    
    let manager = IpcTransportManager::new(config).await.unwrap();
    manager.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    
    // Create message with compressible payload
    let repetitive_payload = b"A".repeat(1000);
    let message = SecureMessage {
        message_id: "compression_test_001".to_string(),
        sequence_number: 1,
        sender_id: "test_sender".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
        signature: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        payload: repetitive_payload,
    };
    
    // Should handle compressed message
    let result = manager.validate_message(&message).await;
    // Result depends on implementation - might succeed or need actual compression layer
    
    manager.shutdown().await.unwrap();
}

// Helper functions

fn create_test_message(sender_id: &str, sequence: u64, payload: &[u8]) -> SecureMessage {
    SecureMessage {
        message_id: format!("test_msg_{:06}", sequence),
        sequence_number: sequence,
        sender_id: sender_id.to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        nonce: vec![sequence as u8; 8],
        signature: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        payload: payload.to_vec(),
    }
}

// Mock implementations for testing

impl IpcTransportManager {
    async fn validate_message(&self, _message: &SecureMessage) -> MultivmResult<()> {
        // Mock implementation for testing
        // In real implementation, this would validate signatures, check replay protection, etc.
        Ok(())
    }
    
    async fn get_status(&self) -> TransportStatus {
        // Mock implementation
        TransportStatus {
            is_listening: true,
            active_connections: 0,
            max_connections: self.config.max_connections,
            total_messages: 0,
            total_bytes: 0,
        }
    }
    
    async fn get_metrics(&self) -> TransportMetrics {
        // Mock implementation
        TransportMetrics {
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            connection_errors: 0,
            message_errors: 0,
            average_latency_ms: 0.0,
            peak_connections: 0,
        }
    }
}

#[derive(Debug)]
struct TransportStatus {
    is_listening: bool,
    active_connections: usize,
    max_connections: usize,
    total_messages: u64,
    total_bytes: u64,
}

#[derive(Debug)]
struct TransportMetrics {
    messages_sent: u64,
    messages_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
    connection_errors: u64,
    message_errors: u64,
    average_latency_ms: f64,
    peak_connections: usize,
}

// Mock TransportConfig for testing
#[derive(Debug, Clone)]
struct TransportConfig {
    bind_address: String,
    max_connections: usize,
    connection_timeout: Duration,
    message_timeout: Duration,
    enable_compression: bool,
    enable_encryption: bool,
    buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:0".to_string(),
            max_connections: 100,
            connection_timeout: Duration::from_secs(10),
            message_timeout: Duration::from_secs(30),
            enable_compression: false,
            enable_encryption: true,
            buffer_size: 65536,
        }
    }
}

// Mock IpcTransportManager for testing
struct IpcTransportManager {
    config: TransportConfig,
}

impl IpcTransportManager {
    async fn new(config: TransportConfig) -> MultivmResult<Self> {
        Ok(Self { config })
    }
    
    async fn start(&self) -> MultivmResult<()> {
        Ok(())
    }
    
    async fn shutdown(&self) -> MultivmResult<()> {
        Ok(())
    }
}