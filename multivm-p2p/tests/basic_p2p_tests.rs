//! Basic P2P Module Tests
//!
//! Simple tests that work with the actual P2P implementation.

use multivm_p2p::core::network::NetworkConfig;
use multivm_p2p::{protocol::messages::*, P2PNetwork};
use std::time::Duration;

/// Test P2P network creation
#[tokio::test]
async fn test_p2p_network_creation() {
    let config = NetworkConfig::default();
    let network_result = P2PNetwork::new(config).await;

    assert!(
        network_result.is_ok(),
        "P2P Network should be created successfully"
    );
}

/// Test message creation and serialization
#[tokio::test]
async fn test_message_creation() {
    let msg = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Verify message has correct fields
    assert!(!msg.id.is_empty(), "Message should have unique ID");
    assert_eq!(msg.source, MessageSource::NetworkLayer);
    assert_eq!(msg.target, MessageTarget::Broadcast);
    assert_eq!(msg.version, 1);

    // Test message serialization
    let serialized = bincode::serialize(&msg);
    assert!(serialized.is_ok(), "Message should serialize successfully");

    // Test message deserialization
    let serialized_data = serialized.unwrap();
    let deserialized: Result<NetworkMessage, _> = bincode::deserialize(&serialized_data);
    assert!(
        deserialized.is_ok(),
        "Message should deserialize successfully"
    );
}

/// Test different message types
#[tokio::test]
async fn test_message_types() {
    // Test SVM message
    let svm_msg = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: Box::new(vec![1, 2, 3, 4]),
            signature: "test_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    // Test EVM message
    let evm_msg = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: Box::new(vec![5, 6, 7, 8]),
            tx_hash: "0xtest_hash".to_string(),
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );

    // Test MultiVM message
    let multivm_msg = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x1234".to_string(),
            vm_type: VmType::Svm,
            height: 12345,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    // Test serialization
    assert!(bincode::serialize(&svm_msg).is_ok());
    assert!(bincode::serialize(&evm_msg).is_ok());
    assert!(bincode::serialize(&multivm_msg).is_ok());
}

/// Test message metadata
#[tokio::test]
async fn test_message_metadata() {
    let mut msg = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Test adding metadata
    msg = msg.with_metadata("priority", "high");
    msg = msg.with_metadata("timestamp", "1234567890");

    assert_eq!(msg.metadata.get("priority"), Some(&"high".to_string()));
    assert_eq!(
        msg.metadata.get("timestamp"),
        Some(&"1234567890".to_string())
    );
    assert_eq!(msg.metadata.len(), 2);
}

/// Test network configuration
#[tokio::test]
async fn test_network_configuration() {
    let mut config = NetworkConfig::default();

    // Test custom configuration
    config.listen_addresses = vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()];
    config.max_peers = 100;
    config.connection_timeout = Duration::from_secs(30);

    let network_result = P2PNetwork::new(config).await;
    assert!(
        network_result.is_ok(),
        "Network should be created with custom config"
    );
}
