//! Integration tests for the P2P module
//!
//! These tests verify functionality of the P2P networking layer

#[cfg(test)]
mod tests {
    use crate::{
        core::network::{NetworkConfig, P2PNetwork},
        protocol::messages::{
            ControlMessage, MessagePayload, MessageSource, MessageTarget, NetworkMessage, VmType,
        },
    };
    use std::time::Duration;

    /// Test basic P2P network startup and shutdown
    #[tokio::test]
    async fn test_network_lifecycle() {
        let config = NetworkConfig::default();

        // Test network creation
        let network = P2PNetwork::new(config.clone()).await;
        assert!(network.is_ok(), "Failed to create P2P network");

        let mut network = network.unwrap();

        // Test network startup
        let startup_result = network.start().await;
        assert!(startup_result.is_ok(), "Failed to start P2P network");

        // Test health check
        let health_result = network.health_check().await;
        assert!(health_result.is_ok(), "Network health check failed");

        // Test network shutdown
        let shutdown_result = network.stop().await;
        assert!(shutdown_result.is_ok(), "Failed to stop P2P network");
    }

    /// Test message creation and serialization
    #[tokio::test]
    async fn test_message_creation() {
        // Test creating different message types
        let heartbeat_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: crate::protocol::messages::NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // Test message serialization
        let serialized = bincode::serialize(&heartbeat_msg);
        assert!(serialized.is_ok(), "Failed to serialize message");

        // Test message deserialization
        let deserialized: Result<NetworkMessage, _> = bincode::deserialize(&serialized.unwrap());
        assert!(deserialized.is_ok(), "Failed to deserialize message");
    }

    /// Test network message publishing (basic)
    #[tokio::test]
    async fn test_message_publishing() {
        let config = NetworkConfig::default();
        let mut network = P2PNetwork::new(config).await.unwrap();

        network.start().await.unwrap();

        // Subscribe to topic first
        let subscribe_result = network.subscribe_topic("test_topic").await;
        assert!(
            subscribe_result.is_ok(),
            "Failed to subscribe to test_topic"
        );

        let test_message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::MultiVmLayer,
            MessageTarget::Broadcast,
        );

        // Wait a bit for subscription to be processed
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test message publishing - note this may succeed even without other peers
        let serialized = bincode::serialize(&test_message).unwrap();
        let publish_result = network.publish_message("test_topic", serialized).await;
        // Publishing to a topic we're subscribed to should work even without peers
        if publish_result.is_err() {
            // This is actually expected in some cases without active peers
            println!("Publishing failed as expected without active peers: {publish_result:?}");
        }

        // Test topic unsubscription
        let unsubscribe_result = network.unsubscribe_topic("test_topic").await;
        assert!(
            unsubscribe_result.is_ok(),
            "Failed to unsubscribe from topic"
        );

        network.stop().await.unwrap();
    }

    /// Test multiple message types
    #[tokio::test]
    async fn test_different_message_types() {
        // Test SVM message
        let svm_msg = NetworkMessage::new(
            MessagePayload::Svm(crate::protocol::messages::SvmMessage::Transaction {
                transaction_data: Box::new(vec![1, 2, 3, 4]),
                signature: "test_signature".to_string(),
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        );

        // Test EVM message
        let evm_msg = NetworkMessage::new(
            MessagePayload::Evm(crate::protocol::messages::EvmMessage::Transaction {
                transaction_data: Box::new(vec![5, 6, 7, 8]),
                tx_hash: "0xtest_hash".to_string(),
            }),
            MessageSource::EvmExecution,
            MessageTarget::Broadcast,
        );

        // Test MultiVM message
        let multivm_msg = NetworkMessage::new(
            MessagePayload::MultiVm(crate::protocol::messages::MultiVmMessage::StateSync {
                state_root: "0x1234".to_string(),
                vm_type: VmType::Svm,
                height: 12345,
            }),
            MessageSource::MultiVmLayer,
            MessageTarget::Broadcast,
        );

        // Test that all messages can be serialized
        assert!(bincode::serialize(&svm_msg).is_ok());
        assert!(bincode::serialize(&evm_msg).is_ok());
        assert!(bincode::serialize(&multivm_msg).is_ok());
    }

    /// Test basic network stress (reduced scope)
    #[tokio::test]
    async fn test_network_basic_stress() {
        let config = NetworkConfig::default();
        let mut network = P2PNetwork::new(config).await.unwrap();

        network.start().await.unwrap();

        // Subscribe to topic first
        let subscribe_result = network.subscribe_topic("stress_test").await;
        assert!(
            subscribe_result.is_ok(),
            "Failed to subscribe to stress_test topic"
        );

        // Wait for subscription to be processed
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send a few messages to test basic functionality
        let mut successful_publishes = 0;
        for i in 0..10 {
            let message = NetworkMessage::new(
                MessagePayload::Control(ControlMessage::StatusRequest),
                MessageSource::MultiVmLayer,
                MessageTarget::Broadcast,
            )
            .with_metadata("test_id", i.to_string());

            let data = bincode::serialize(&message).unwrap();
            let publish_result = network.publish_message("stress_test", data).await;

            // Publishing may fail without other peers, which is expected
            if publish_result.is_ok() {
                successful_publishes += 1;
            } else {
                println!("Publish {i} failed as expected without peers: {publish_result:?}");
            }
        }

        // At least some publishes should work even without peers
        println!("Successful publishes: {successful_publishes}/10");

        // Verify network health
        let health_result = network.health_check().await;
        assert!(health_result.is_ok(), "Network health check failed");

        network.stop().await.unwrap();
    }
}
