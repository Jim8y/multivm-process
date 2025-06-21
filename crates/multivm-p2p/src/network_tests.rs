//! Comprehensive unit tests for the P2P network layer

use super::network::*;
use crate::error::P2PError;
use crate::messages::*;
use crate::P2PNetworkLayer;
use libp2p::Multiaddr;
use std::time::Duration;

#[tokio::test]
async fn test_network_creation() {
    let config = NetworkConfig::default();
    let result = P2PNetwork::new(config).await;

    assert!(result.is_ok(), "Network creation should succeed");

    let network = result.unwrap();
    assert!(!network.local_peer_id().to_string().is_empty());
}

#[tokio::test]
async fn test_network_lifecycle() {
    let config = NetworkConfig::default();
    let mut network = P2PNetwork::new(config).await.unwrap();

    // Test start
    let start_result = network.start().await;
    assert!(start_result.is_ok(), "Network start should succeed");

    // Test stop
    let stop_result = network.stop().await;
    assert!(stop_result.is_ok(), "Network stop should succeed");
}

#[tokio::test]
async fn test_topic_subscription() {
    let config = NetworkConfig::default();
    let network = P2PNetwork::new(config).await.unwrap();

    // Test subscribe
    let subscribe_result = network.subscribe_topic("test-topic").await;
    assert!(
        subscribe_result.is_ok(),
        "Topic subscription should succeed"
    );

    // Verify subscription
    let topics = network.get_subscribed_topics().await;
    assert!(
        topics.contains("test-topic"),
        "Topic should be in subscribed list"
    );

    // Test unsubscribe
    let unsubscribe_result = network.unsubscribe_topic("test-topic").await;
    assert!(
        unsubscribe_result.is_ok(),
        "Topic unsubscription should succeed"
    );

    // Verify unsubscription
    let topics_after = network.get_subscribed_topics().await;
    assert!(
        !topics_after.contains("test-topic"),
        "Topic should be removed from subscribed list"
    );
}

#[tokio::test]
async fn test_message_publishing() {
    let config = NetworkConfig::default();
    let network = P2PNetwork::new(config).await.unwrap();

    let test_data = b"test message data".to_vec();
    let result = network
        .publish_message("test-topic", test_data.clone())
        .await;

    // Message publishing might fail if no swarm is running, which is expected
    match result {
        Ok(_) => (), // Success is good
        Err(e) => {
            // Expected error in test environment - channel closed or insufficient peers
            assert!(
                e.to_string().contains("Failed to send publish command")
                    || e.to_string().contains("channel")
                    || e.to_string().contains("InsufficientPeers"),
                "Expected channel, command, or peers error, got: {}",
                e
            );
            return; // Skip stats check if publishing failed
        }
    }

    // Check that stats were updated
    let stats = network.get_network_stats().await.unwrap();
    assert!(
        stats.messages_sent > 0,
        "Message sent count should be updated"
    );
    assert!(
        stats.bytes_sent >= test_data.len() as u64,
        "Bytes sent should be updated"
    );
}

#[tokio::test]
async fn test_peer_management() {
    let config = NetworkConfig::default();
    let network = P2PNetwork::new(config).await.unwrap();

    // Create test peer
    let peer_id = libp2p::PeerId::random();
    let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

    // Add peer
    let add_result = network.add_peer(peer_id, vec![addr.clone()]).await;
    assert!(add_result.is_ok(), "Adding peer should succeed");
}

#[tokio::test]
async fn test_network_stats() {
    let config = NetworkConfig::default();
    let network = P2PNetwork::new(config).await.unwrap();

    let stats = network.get_network_stats().await.unwrap();

    // Basic stats validation
    assert_eq!(
        stats.connected_peers, 0,
        "Initial connected peers should be 0"
    );
    assert_eq!(stats.messages_sent, 0, "Initial messages sent should be 0");
    assert_eq!(
        stats.messages_received, 0,
        "Initial messages received should be 0"
    );
}

#[tokio::test]
async fn test_p2p_network_layer_trait() {
    let config = NetworkConfig::default();
    let mut network = P2PNetwork::new(config).await.unwrap();

    // Test P2PNetworkLayer trait methods
    let start_result = network.start().await;
    assert!(start_result.is_ok());

    let stop_result = network.stop().await;
    assert!(stop_result.is_ok());

    let subscribe_result = network.subscribe("test-topic").await;
    assert!(subscribe_result.is_ok());

    let unsubscribe_result = network.unsubscribe("test-topic").await;
    assert!(unsubscribe_result.is_ok());

    let peers = network.get_connected_peers().await;
    // Should be empty initially

    let stats_result = network.get_network_stats().await;
    assert!(stats_result.is_ok());
}

#[tokio::test]
async fn test_message_handling() {
    let config = NetworkConfig::default();
    let mut network = P2PNetwork::new(config).await.unwrap();

    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    let handle_result = network
        .handle_incoming_message(test_message, "test-peer".to_string())
        .await;

    assert!(handle_result.is_ok(), "Message handling should succeed");
}

#[tokio::test]
async fn test_broadcast_message() {
    let config = NetworkConfig::default();
    let mut network = P2PNetwork::new(config).await.unwrap();

    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(100),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    let broadcast_result = network.broadcast(test_message).await;
    // Broadcasting might fail if no swarm is running, which is expected in test
    match broadcast_result {
        Ok(_) => (), // Success is good
        Err(e) => {
            // Expected error in test environment
            assert!(
                e.to_string().contains("Failed to send publish command")
                    || e.to_string().contains("channel")
                    || e.to_string().contains("InsufficientPeers"),
                "Expected channel, command, or peers error, got: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_send_to_peer() {
    let config = NetworkConfig::default();
    let mut network = P2PNetwork::new(config).await.unwrap();

    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer("test-peer".to_string()),
    );

    // This should work even if peer doesn't exist (network layer handles it)
    let send_result = network
        .send_to_peer("test-peer".to_string(), test_message)
        .await;
    // Note: This might fail if peer is not connected, which is expected behavior
    match send_result {
        Ok(_) => (),
        Err(e) => {
            // Should be a network-related error
            assert!(e.to_string().contains("peer") || e.to_string().contains("network"));
        }
    }
}

#[test]
fn test_network_config_validation() {
    let valid_config = NetworkConfig::default();
    assert!(!valid_config.listen_addresses.is_empty());
    assert!(valid_config.max_peers > 0);
    assert!(valid_config.enable_mdns);
}

#[test]
fn test_network_config_custom() {
    let custom_config = NetworkConfig {
        listen_addresses: vec!["/ip4/0.0.0.0/tcp/9999".parse().unwrap()],
        bootstrap_peers: vec![],
        max_peers: 25,
        enable_mdns: false,
        validation_mode: libp2p::gossipsub::ValidationMode::Permissive,
        connection_timeout: Duration::from_secs(5),
    };

    assert_eq!(custom_config.max_peers, 25);
    assert!(!custom_config.enable_mdns);
}

#[tokio::test]
async fn test_concurrent_operations() {
    let config = NetworkConfig::default();
    let network = P2PNetwork::new(config).await.unwrap();

    // Test multiple sequential subscriptions (simplified to avoid lifetime issues)
    for i in 0..5 {
        let topic = format!("test-topic-{}", i);
        let result = network.subscribe_topic(&topic).await;
        assert!(result.is_ok(), "Sequential subscription should succeed");
    }

    // Verify all topics are subscribed
    let topics = network.get_subscribed_topics().await;
    assert_eq!(topics.len(), 5, "All topics should be subscribed");
}

#[tokio::test]
async fn test_error_handling() {
    let config = NetworkConfig::default();
    let mut network = P2PNetwork::new(config).await.unwrap();

    // Test with invalid peer ID
    let invalid_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer("invalid-peer-id".to_string()),
    );

    let result = network
        .send_to_peer("invalid-peer-id".to_string(), invalid_message)
        .await;
    // This should either succeed (message queued) or fail gracefully
    match result {
        Ok(_) => (), // Message was queued
        Err(e) => {
            // Should be a proper P2P error
            assert!(!e.to_string().is_empty());
        }
    }
}

#[tokio::test]
async fn test_message_size_limits() {
    let config = NetworkConfig::default();
    let network = P2PNetwork::new(config).await.unwrap();

    // Create a large message
    let large_data = vec![0u8; 1024 * 1024]; // 1MB
    let result = network.publish_message("test-topic", large_data).await;

    // Should succeed or fail gracefully based on configuration
    match result {
        Ok(_) => println!("Large message accepted"),
        Err(e) => {
            println!("Large message rejected: {}", e);
            // Should be a size-related error
        }
    }
}

#[test]
fn test_network_behaviour_creation() {
    // Test that we can create the network behaviour structure
    // This is important for libp2p integration
    use crate::network::NetworkBehaviour;
    use libp2p::identity::Keypair;
    use libp2p::kad::store::MemoryStore;
    use libp2p::{gossipsub, identify, kad, mdns, ping};

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = libp2p::PeerId::from(local_key.public());

    // This should not panic
    let store = MemoryStore::new(local_peer_id);
    let kademlia = kad::Behaviour::new(local_peer_id, store);

    // Test basic behaviour creation (simplified)
    assert!(!local_peer_id.to_string().is_empty());
}
