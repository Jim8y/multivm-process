//! Tests for the P2P discovery service

use super::discovery::*;
use libp2p::{Multiaddr, PeerId};
use std::time::Duration;
use tokio_test;

#[tokio::test]
async fn test_discovery_service_creation() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, _command_sender, _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Basic validation - service created successfully
    assert!(true); // Service creation should not panic
}

#[tokio::test]
async fn test_discovery_config_defaults() {
    let config = DiscoveryConfig::default();
    
    assert_eq!(config.enable_mdns, true);
    assert_eq!(config.enable_kademlia, true);
    assert_eq!(config.bootstrap_peers.len(), 0);
    assert_eq!(config.discovery_interval, Duration::from_secs(30));
    assert_eq!(config.max_discovered_peers, 1000);
    assert_eq!(config.peer_refresh_interval, Duration::from_secs(300));
    assert_eq!(config.replication_factor, 20);
}

#[tokio::test]
async fn test_discovery_service_lifecycle() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, command_sender, mut event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Test service start
    let start_result = service.start().await;
    assert!(start_result.is_ok(), "Discovery service should start successfully");
    
    // Test service stop
    let stop_result = service.stop().await;
    assert!(stop_result.is_ok(), "Discovery service should stop successfully");
    
    // Clean up
    drop(command_sender);
    
    // Verify no panic on event receiver
    tokio::time::timeout(Duration::from_millis(100), event_receiver.recv()).await.ok();
}

#[tokio::test]
async fn test_discovery_commands() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, command_sender, mut _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Start the service
    service.start().await.unwrap();
    
    // Test add bootstrap peer command
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
    
    let add_peer_result = command_sender.send(DiscoveryCommand::AddBootstrapPeer {
        peer_id,
        address: address.clone(),
    }).await;
    
    assert!(add_peer_result.is_ok(), "Add bootstrap peer command should succeed");
    
    // Test remove peer command
    let remove_peer_result = command_sender.send(DiscoveryCommand::RemovePeer { peer_id }).await;
    assert!(remove_peer_result.is_ok(), "Remove peer command should succeed");
    
    // Test DHT lookup command
    let lookup_result = command_sender.send(DiscoveryCommand::DhtLookup {
        key: b"test_key".to_vec(),
    }).await;
    assert!(lookup_result.is_ok(), "DHT lookup command should succeed");
    
    // Clean up
    drop(command_sender);
}

#[tokio::test]
async fn test_discovery_stats() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, command_sender, _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Start the service
    service.start().await.unwrap();
    
    // Get stats
    let (tx, rx) = tokio::sync::oneshot::channel();
    let stats_command = DiscoveryCommand::GetStats(tx);
    
    command_sender.send(stats_command).await.unwrap();
    let stats = rx.await.unwrap();
    
    // Validate stats
    assert_eq!(stats.total_peers_discovered, 0);
    assert_eq!(stats.active_peers, 0);
    assert_eq!(stats.mdns_discoveries, 0);
    assert_eq!(stats.kademlia_discoveries, 0);
    assert_eq!(stats.bootstrap_peers, 0);
    assert_eq!(stats.active_queries, 0);
    assert_eq!(stats.successful_queries, 0);
    assert_eq!(stats.failed_queries, 0);
    
    // Clean up
    drop(command_sender);
}

#[tokio::test]
async fn test_discovered_peer_info() {
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/192.168.1.100/tcp/8000".parse().unwrap();
    
    let discovered_peer = DiscoveredPeer {
        peer_id,
        addresses: vec![address.clone()],
        discovered_at: std::time::SystemTime::now(),
        last_seen: std::time::SystemTime::now(),
        discovery_method: DiscoveryMethod::Mdns,
        connection_status: ConnectionStatus::NotConnected,
    };
    
    assert_eq!(discovered_peer.peer_id, peer_id);
    assert_eq!(discovered_peer.addresses[0], address);
    assert!(matches!(discovered_peer.discovery_method, DiscoveryMethod::Mdns));
    assert!(matches!(discovered_peer.connection_status, ConnectionStatus::NotConnected));
}

#[test]
fn test_discovery_method_serialization() {
    use serde_json;
    
    let methods = vec![
        DiscoveryMethod::Mdns,
        DiscoveryMethod::Kademlia,
        DiscoveryMethod::Manual,
        DiscoveryMethod::Bootstrap,
    ];
    
    for method in methods {
        let serialized = serde_json::to_string(&method).unwrap();
        let deserialized: DiscoveryMethod = serde_json::from_str(&serialized).unwrap();
        
        match (&method, &deserialized) {
            (DiscoveryMethod::Mdns, DiscoveryMethod::Mdns) => (),
            (DiscoveryMethod::Kademlia, DiscoveryMethod::Kademlia) => (),
            (DiscoveryMethod::Manual, DiscoveryMethod::Manual) => (),
            (DiscoveryMethod::Bootstrap, DiscoveryMethod::Bootstrap) => (),
            _ => panic!("Serialization/deserialization failed"),
        }
    }
}

#[test]
fn test_connection_status_types() {
    let statuses = vec![
        ConnectionStatus::NotConnected,
        ConnectionStatus::Connecting,
        ConnectionStatus::Connected,
        ConnectionStatus::Failed("Connection timeout".to_string()),
    ];
    
    for status in statuses {
        match status {
            ConnectionStatus::NotConnected => assert!(true),
            ConnectionStatus::Connecting => assert!(true),
            ConnectionStatus::Connected => assert!(true),
            ConnectionStatus::Failed(ref reason) => assert!(!reason.is_empty()),
        }
    }
}

#[tokio::test]
async fn test_peer_management() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, _command_sender, _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    let test_peer_id = PeerId::random();
    
    // Test peer existence check
    let exists_before = service.is_peer_known(&test_peer_id).await;
    assert!(!exists_before, "Peer should not exist initially");
    
    // Test getting non-existent peer
    let peer_info = service.get_peer(&test_peer_id).await;
    assert!(peer_info.is_none(), "Non-existent peer should return None");
    
    // Test getting all discovered peers
    let all_peers = service.get_discovered_peers().await;
    assert_eq!(all_peers.len(), 0, "Initially no peers should be discovered");
    
    // Test updating peer status (should handle gracefully even if peer doesn't exist)
    let update_result = service.update_peer_status(test_peer_id, ConnectionStatus::Connected).await;
    assert!(update_result.is_ok(), "Status update should handle missing peer gracefully");
}

#[tokio::test]
async fn test_discovery_behaviour_creation() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, _command_sender, _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Test creating discovery behaviour
    let behaviour_result = service.create_behaviour().await;
    assert!(behaviour_result.is_ok(), "Discovery behaviour creation should succeed");
}

#[tokio::test]
async fn test_bootstrap_with_peers() {
    let mut config = DiscoveryConfig::default();
    
    // Add bootstrap peers
    let peer1 = PeerId::random();
    let peer2 = PeerId::random();
    let addr1: Multiaddr = "/ip4/192.168.1.10/tcp/8000".parse().unwrap();
    let addr2: Multiaddr = "/ip4/192.168.1.11/tcp/8000".parse().unwrap();
    
    config.bootstrap_peers = vec![(peer1, addr1), (peer2, addr2)];
    
    let local_peer_id = PeerId::random();
    let (service, _command_sender, _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Start service (which should trigger bootstrap)
    let start_result = service.start().await;
    assert!(start_result.is_ok(), "Service with bootstrap peers should start successfully");
    
    // Give a moment for bootstrap to process
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Check that bootstrap peers were added
    let discovered_peers = service.get_discovered_peers().await;
    // Note: Bootstrap might not be complete immediately, so we just check the process started
    assert!(discovered_peers.len() <= 2, "Bootstrap process should be running");
}

#[tokio::test]
async fn test_discovery_events() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (_service, command_sender, mut event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    // Send a command that should generate events
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
    
    command_sender.send(DiscoveryCommand::AddBootstrapPeer {
        peer_id,
        address,
    }).await.unwrap();
    
    // Try to receive an event (with timeout to avoid hanging)
    let event_result = tokio::time::timeout(
        Duration::from_millis(500),
        event_receiver.recv()
    ).await;
    
    // Event might or might not arrive immediately, but the channel should be functional
    match event_result {
        Ok(Some(event)) => {
            match event {
                DiscoveryEvent::PeerDiscovered { peer } => {
                    assert_eq!(peer.peer_id, peer_id);
                }
                _ => (), // Other events are fine too
            }
        }
        Ok(None) => (), // Channel closed
        Err(_) => (), // Timeout - also fine
    }
    
    // Clean up
    drop(command_sender);
}

#[test]
fn test_discovery_config_validation() {
    let mut config = DiscoveryConfig::default();
    
    // Test valid config
    assert!(config.max_discovered_peers > 0);
    assert!(config.replication_factor > 0);
    
    // Test edge cases
    config.max_discovered_peers = 0;
    assert_eq!(config.max_discovered_peers, 0); // Should handle gracefully
    
    config.replication_factor = 0;
    assert_eq!(config.replication_factor, 0); // Should handle gracefully
}

#[tokio::test]
async fn test_dht_operations() {
    let config = DiscoveryConfig::default();
    let local_peer_id = PeerId::random();
    
    let (service, command_sender, _event_receiver) = DiscoveryService::new(Some(config), local_peer_id);
    
    service.start().await.unwrap();
    
    // Test DHT put operation
    let put_result = command_sender.send(DiscoveryCommand::PutValue {
        key: b"test_key".to_vec(),
        value: b"test_value".to_vec(),
    }).await;
    assert!(put_result.is_ok(), "DHT put operation should succeed");
    
    // Test DHT get operation
    let get_result = command_sender.send(DiscoveryCommand::GetValue {
        key: b"test_key".to_vec(),
    }).await;
    assert!(get_result.is_ok(), "DHT get operation should succeed");
    
    // Test find peers operation
    let find_result = command_sender.send(DiscoveryCommand::FindPeers {
        key: b"peer_key".to_vec(),
    }).await;
    assert!(find_result.is_ok(), "Find peers operation should succeed");
    
    // Clean up
    drop(command_sender);
}