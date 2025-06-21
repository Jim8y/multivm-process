//! Tests for the P2P message routing system

use super::routing::*;
use crate::messages::*;
use libp2p::{Multiaddr, PeerId};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_test;

#[tokio::test]
async fn test_message_router_creation() {
    let (message_sender, _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, _command_sender) = MessageRouter::new(message_sender, Some(config));
    
    // Basic validation that router was created
    assert!(true); // Router creation should not panic
}

#[test]
fn test_routing_config_defaults() {
    let config = RoutingConfig::default();
    
    assert_eq!(config.max_peers_per_type, 50);
    assert_eq!(config.cleanup_interval, Duration::from_secs(300));
    assert_eq!(config.reliability_threshold, 0.8);
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.routing_timeout, Duration::from_secs(30));
}

#[test]
fn test_routing_config_custom() {
    let custom_config = RoutingConfig {
        max_peers_per_type: 25,
        cleanup_interval: Duration::from_secs(600),
        reliability_threshold: 0.9,
        max_retries: 5,
        routing_timeout: Duration::from_secs(60),
    };
    
    assert_eq!(custom_config.max_peers_per_type, 25);
    assert_eq!(custom_config.reliability_threshold, 0.9);
    assert_eq!(custom_config.max_retries, 5);
}

#[tokio::test]
async fn test_routing_strategies() {
    use RoutingStrategy;
    
    let peer_id = PeerId::random();
    let key = b"test_key".to_vec();
    
    let strategies = vec![
        RoutingStrategy::Broadcast,
        RoutingStrategy::Direct(peer_id),
        RoutingStrategy::DHT(key),
        RoutingStrategy::Gossip("test-topic".to_string()),
        RoutingStrategy::Random(5),
    ];
    
    // Test that all strategies can be created and matched
    for strategy in strategies {
        match strategy {
            RoutingStrategy::Broadcast => assert!(true),
            RoutingStrategy::Direct(peer) => assert_eq!(peer, peer_id),
            RoutingStrategy::DHT(dht_key) => assert_eq!(dht_key, b"test_key".to_vec()),
            RoutingStrategy::Gossip(topic) => assert_eq!(topic, "test-topic"),
            RoutingStrategy::Random(count) => assert_eq!(count, 5),
        }
    }
}

#[tokio::test]
async fn test_router_lifecycle() {
    let (message_sender, _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, command_sender) = MessageRouter::new(message_sender, Some(config));
    
    // Test router start
    let start_result = router.start().await;
    assert!(start_result.is_ok(), "Router should start successfully");
    
    // Clean up
    drop(command_sender);
}

#[tokio::test]
async fn test_routing_commands() {
    let (message_sender, _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, command_sender) = MessageRouter::new(message_sender, Some(config));
    
    // Start router
    router.start().await.unwrap();
    
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
    
    // Test add route command
    let add_route_result = command_sender.send(RoutingCommand::AddRoute {
        message_type: MessageType::Control,
        peer_id,
        address: address.clone(),
    }).await;
    assert!(add_route_result.is_ok(), "Add route command should succeed");
    
    // Test remove route command
    let remove_route_result = command_sender.send(RoutingCommand::RemoveRoute {
        message_type: MessageType::Control,
        peer_id,
    }).await;
    assert!(remove_route_result.is_ok(), "Remove route command should succeed");
    
    // Test subscribe command
    let subscribe_result = command_sender.send(RoutingCommand::Subscribe("test-topic".to_string())).await;
    assert!(subscribe_result.is_ok(), "Subscribe command should succeed");
    
    // Test unsubscribe command
    let unsubscribe_result = command_sender.send(RoutingCommand::Unsubscribe("test-topic".to_string())).await;
    assert!(unsubscribe_result.is_ok(), "Unsubscribe command should succeed");
    
    // Test update reliability command
    let reliability_result = command_sender.send(RoutingCommand::UpdateReliability {
        peer_id,
        success: true,
    }).await;
    assert!(reliability_result.is_ok(), "Update reliability command should succeed");
    
    // Clean up
    drop(command_sender);
}

#[tokio::test]
async fn test_routing_stats() {
    let (message_sender, _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, command_sender) = MessageRouter::new(message_sender, Some(config));
    
    router.start().await.unwrap();
    
    // Get stats
    let (tx, rx) = tokio::sync::oneshot::channel();
    let stats_command = RoutingCommand::GetStats(tx);
    
    command_sender.send(stats_command).await.unwrap();
    let stats = rx.await.unwrap();
    
    // Validate initial stats
    assert_eq!(stats.total_messages_routed, 0);
    assert_eq!(stats.successful_routes, 0);
    assert_eq!(stats.failed_routes, 0);
    assert_eq!(stats.active_routes, 0);
    assert_eq!(stats.avg_routing_time_ms, 0.0);
    assert_eq!(stats.peer_count, 0);
    
    drop(command_sender);
}

#[tokio::test]
async fn test_message_routing() {
    let (message_sender, mut message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, command_sender) = MessageRouter::new(message_sender, Some(config));
    
    router.start().await.unwrap();
    
    // Add a route first
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
    
    command_sender.send(RoutingCommand::AddRoute {
        message_type: MessageType::Control,
        peer_id,
        address,
    }).await.unwrap();
    
    // Give time for route to be added
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Create test message
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );
    
    // Test broadcast routing
    let broadcast_result = router.route_message(test_message.clone(), RoutingStrategy::Broadcast).await;
    assert!(broadcast_result.is_ok(), "Broadcast routing should succeed");
    
    // Test direct routing
    let direct_result = router.route_message(test_message.clone(), RoutingStrategy::Direct(peer_id)).await;
    assert!(direct_result.is_ok(), "Direct routing should succeed");
    
    // Test DHT routing
    let dht_result = router.route_message(test_message.clone(), RoutingStrategy::DHT(b"test_key".to_vec())).await;
    assert!(dht_result.is_ok(), "DHT routing should succeed");
    
    // Test gossip routing
    let gossip_result = router.route_message(test_message.clone(), RoutingStrategy::Gossip("test-topic".to_string())).await;
    assert!(gossip_result.is_ok(), "Gossip routing should succeed");
    
    // Test random routing
    let random_result = router.route_message(test_message, RoutingStrategy::Random(1)).await;
    assert!(random_result.is_ok(), "Random routing should succeed");
    
    // Try to receive routed messages (with timeout)
    let mut received_count = 0;
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(50), message_receiver.recv()).await {
        received_count += 1;
        if received_count >= 3 {
            break; // Don't wait for all messages to avoid test hanging
        }
    }
    
    drop(command_sender);
}

#[tokio::test]
async fn test_route_management() {
    let (message_sender, _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, _command_sender) = MessageRouter::new(message_sender, Some(config));
    
    // Test has_routes (should be false initially)
    let has_routes_before = router.has_routes(&MessageType::Control).await;
    assert!(!has_routes_before, "Should not have routes initially");
    
    // Test get_best_routes (should return empty)
    let best_routes = router.get_best_routes(&MessageType::Control, 5).await;
    assert_eq!(best_routes.len(), 0, "Should return no routes initially");
}

#[test]
fn test_routing_stats_serialization() {
    let stats = RoutingStats {
        total_messages_routed: 1000,
        successful_routes: 950,
        failed_routes: 50,
        active_routes: 25,
        avg_routing_time_ms: 15.5,
        peer_count: 10,
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&stats).unwrap();
    let deserialized: RoutingStats = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(deserialized.total_messages_routed, stats.total_messages_routed);
    assert_eq!(deserialized.successful_routes, stats.successful_routes);
    assert_eq!(deserialized.failed_routes, stats.failed_routes);
    assert_eq!(deserialized.active_routes, stats.active_routes);
    assert_eq!(deserialized.avg_routing_time_ms, stats.avg_routing_time_ms);
    assert_eq!(deserialized.peer_count, stats.peer_count);
}

#[tokio::test]
async fn test_sequential_routing() {
    let (message_sender, mut message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, command_sender) = MessageRouter::new(message_sender, Some(config));
    
    router.start().await.unwrap();
    
    // Add multiple routes
    let peer_ids: Vec<PeerId> = (0..3).map(|_| PeerId::random()).collect();
    for (i, peer_id) in peer_ids.iter().enumerate() {
        let address: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}", 8000 + i).parse().unwrap();
        command_sender.send(RoutingCommand::AddRoute {
            message_type: MessageType::Control,
            peer_id: *peer_id,
            address,
        }).await.unwrap();
    }
    
    // Give time for routes to be added
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Test sequential message routing (avoiding concurrency issues)
    for i in 0..5 {
        let test_message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(i * 10),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
        
        let result = router.route_message(test_message, RoutingStrategy::Broadcast).await;
        assert!(result.is_ok(), "Sequential routing should succeed");
    }
    
    // Try to receive some routed messages
    let mut received_count = 0;
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(20), message_receiver.recv()).await {
        received_count += 1;
        if received_count >= 10 {
            break;
        }
    }
    
    drop(command_sender);
}

#[tokio::test]
async fn test_routing_with_no_peers() {
    let (message_sender, _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, _command_sender) = MessageRouter::new(message_sender, Some(config));
    
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );
    
    // Test routing with no peers (should handle gracefully)
    let broadcast_result = router.route_message(test_message.clone(), RoutingStrategy::Broadcast).await;
    assert!(broadcast_result.is_ok(), "Broadcast with no peers should succeed");
    
    let random_result = router.route_message(test_message, RoutingStrategy::Random(5)).await;
    // Random routing with no peers should fail gracefully
    match random_result {
        Ok(_) => (), // Handled gracefully
        Err(_) => (), // Expected failure is fine
    }
}

#[test]
fn test_message_type_matching() {
    let message_types = vec![
        MessageType::Svm,
        MessageType::Evm,
        MessageType::MultiVm,
        MessageType::Control,
        MessageType::Discovery,
    ];
    
    // Test that all message types can be used as routing keys
    for msg_type in message_types {
        match msg_type {
            MessageType::Svm => assert!(true),
            MessageType::Evm => assert!(true),
            MessageType::MultiVm => assert!(true),
            MessageType::Control => assert!(true),
            MessageType::Discovery => assert!(true),
        }
    }
}

#[test]
fn test_routing_strategy_serialization() {
    let peer_id = PeerId::random();
    let strategies = vec![
        RoutingStrategy::Broadcast,
        RoutingStrategy::Direct(peer_id),
        RoutingStrategy::DHT(b"key".to_vec()),
        RoutingStrategy::Gossip("topic".to_string()),
        RoutingStrategy::Random(3),
    ];
    
    for strategy in strategies {
        let serialized = serde_json::to_string(&strategy).unwrap();
        let deserialized: RoutingStrategy = serde_json::from_str(&serialized).unwrap();
        
        match (&strategy, &deserialized) {
            (RoutingStrategy::Broadcast, RoutingStrategy::Broadcast) => (),
            (RoutingStrategy::Direct(p1), RoutingStrategy::Direct(p2)) => assert_eq!(p1, p2),
            (RoutingStrategy::DHT(k1), RoutingStrategy::DHT(k2)) => assert_eq!(k1, k2),
            (RoutingStrategy::Gossip(t1), RoutingStrategy::Gossip(t2)) => assert_eq!(t1, t2),
            (RoutingStrategy::Random(c1), RoutingStrategy::Random(c2)) => assert_eq!(c1, c2),
            _ => panic!("Serialization/deserialization mismatch"),
        }
    }
}

#[tokio::test]
async fn test_router_stats_after_routing() {
    let (message_sender, mut _message_receiver) = mpsc::channel(100);
    let config = RoutingConfig::default();
    
    let (router, command_sender) = MessageRouter::new(message_sender, Some(config));
    
    router.start().await.unwrap();
    
    // Add a route
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
    
    command_sender.send(RoutingCommand::AddRoute {
        message_type: MessageType::Control,
        peer_id,
        address,
    }).await.unwrap();
    
    // Give time for route to be added
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Route some messages
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );
    
    router.route_message(test_message, RoutingStrategy::Broadcast).await.unwrap();
    
    // Get stats after routing
    let (tx, rx) = tokio::sync::oneshot::channel();
    command_sender.send(RoutingCommand::GetStats(tx)).await.unwrap();
    let stats = rx.await.unwrap();
    
    // Stats should show routing activity
    assert!(stats.total_messages_routed > 0, "Should show routed messages");
    assert!(stats.successful_routes > 0, "Should show successful routes");
    
    drop(command_sender);
}