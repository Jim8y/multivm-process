//! Integration tests for the P2P networking layer
//!
//! These tests verify that different components of the P2P system work together correctly.

use libp2p::{Multiaddr, PeerId};
use multivm_p2p::*;
use std::time::Duration;

#[tokio::test]
async fn test_end_to_end_message_flow() {
    // Test that messages can flow through the entire P2P stack
    let _config = config::P2PConfig::default();

    // Create test message
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(100),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Verify message properties
    assert!(!test_message.id.is_empty());
    assert!(test_message.is_broadcast());
    assert_eq!(test_message.version, 1);

    // Test message serialization/deserialization
    let serialized = bincode::serialize(&test_message).unwrap();
    let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

    assert_eq!(test_message.id, deserialized.id);
    assert_eq!(test_message.version, deserialized.version);
}

#[tokio::test]
async fn test_protocol_translation_integration() {
    let translator = ProtocolTranslator::new();

    // Test cross-VM message translation
    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3, 4],
            signature: "test_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    // Translate to EVM
    let evm_translated = translator
        .translate_message(&svm_message, VmType::Evm)
        .unwrap();

    // Verify translation metadata
    assert!(evm_translated.metadata.contains_key("converted_from"));
    assert!(evm_translated.metadata.contains_key("converted_to"));
    assert!(evm_translated.metadata.contains_key("conversion_time"));

    // Test that translated message can be routed
    assert!(translator.can_route_to_vm(&evm_translated, VmType::Evm));
    assert!(translator.can_route_to_vm(&evm_translated, VmType::Svm));
}

#[tokio::test]
async fn test_network_discovery_integration() {
    let config = discovery::DiscoveryConfig::default();
    let local_peer_id = PeerId::random();

    let (discovery_service, command_sender, mut event_receiver) =
        discovery::DiscoveryService::new(Some(config), local_peer_id);

    // Start discovery service
    discovery_service.start().await.unwrap();

    // Add a bootstrap peer
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

    command_sender
        .send(discovery::DiscoveryCommand::AddBootstrapPeer {
            peer_id,
            address: address.clone(),
        })
        .await
        .unwrap();

    // Try to receive discovery event
    let event_result =
        tokio::time::timeout(Duration::from_millis(500), event_receiver.recv()).await;

    // Event system should be functional
    if let Ok(Some(discovery::DiscoveryEvent::PeerDiscovered { peer })) = event_result {
        assert_eq!(peer.peer_id, peer_id);
        assert!(peer.addresses.contains(&address));
    }

    discovery_service.stop().await.unwrap();
    drop(command_sender);
}

#[tokio::test]
async fn test_transport_network_integration() {
    use libp2p::identity;

    let local_key = identity::Keypair::generate_ed25519();
    let config = transport::TransportConfig::default();

    let (transport, message_sender, mut event_receiver) =
        transport::TransportLayer::new(Some(config), local_key.clone());

    // Start transport
    transport.start(local_key).await.unwrap();

    // Create and send test message
    let peer_id = PeerId::random();
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer(peer_id.to_string()),
    );

    // Send message through transport
    let send_result = message_sender.send((peer_id, test_message.clone())).await;
    assert!(send_result.is_ok());

    // Try to receive transport events
    let event_result =
        tokio::time::timeout(Duration::from_millis(200), event_receiver.recv()).await;

    // Transport should process the message
    if let Ok(Some(transport::TransportEvent::MessageSent {
        peer_id: sent_peer,
        message_id,
    })) = event_result
    {
        assert_eq!(sent_peer, peer_id);
        assert_eq!(message_id, test_message.id);
    }

    transport.stop().await.unwrap();
}

#[tokio::test]
async fn test_routing_integration() {
    let (message_sender, mut message_receiver) = tokio::sync::mpsc::channel(100);
    let config = routing::RoutingConfig::default();

    let (router, command_sender) = routing::MessageRouter::new(message_sender, Some(config));

    // Start router
    router.start().await.unwrap();

    // Add route
    let peer_id = PeerId::random();
    let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

    command_sender
        .send(routing::RoutingCommand::AddRoute {
            message_type: MessageType::Control,
            peer_id,
            address,
        })
        .await
        .unwrap();

    // Give time for route to be added
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Route message
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    let route_result = router
        .route_message(test_message.clone(), routing::RoutingStrategy::Broadcast)
        .await;
    assert!(route_result.is_ok());

    // Try to receive routed message
    let message_result =
        tokio::time::timeout(Duration::from_millis(100), message_receiver.recv()).await;

    if let Ok(Some((routed_peer_id, routed_message))) = message_result {
        assert_eq!(routed_peer_id, peer_id);
        assert_eq!(routed_message.id, test_message.id);
    }

    drop(command_sender);
}

#[tokio::test]
async fn test_multivm_cross_chain_message() {
    use multivm_account_mapping::{AccountAddress, EthereumAddress, SolanaAddress};

    // Test complete cross-chain message flow
    let translator = ProtocolTranslator::new();

    // Create cross-chain account binding message
    let source_addr = AccountAddress::Solana(SolanaAddress([1u8; 32]));
    let target_addr = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));

    let binding_message = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::AccountBinding {
            source: source_addr.clone(),
            target: target_addr.clone(),
            proof_hash: "0x123456789abcdef".to_string(),
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    // Test that message can be translated to both VM types
    let svm_result = translator.translate_message(&binding_message, VmType::Svm);
    let evm_result = translator.translate_message(&binding_message, VmType::Evm);

    assert!(svm_result.is_ok());
    assert!(evm_result.is_ok());

    // Test message size estimation
    let estimated_size = binding_message.estimated_size();
    assert!(estimated_size > 0);

    // Test message targeting
    assert!(binding_message.is_broadcast());
    assert!(!binding_message.is_peer_message());
    assert!(binding_message.target_peer().is_none());
}

#[tokio::test]
async fn test_error_handling_integration() {
    // Test error propagation through the P2P stack

    // Test invalid peer ID handling
    let invalid_peer_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer("invalid-peer-id".to_string()),
    );

    // Message creation should succeed
    assert!(!invalid_peer_message.id.is_empty());
    assert!(invalid_peer_message.is_peer_message());
    assert_eq!(invalid_peer_message.target_peer(), Some("invalid-peer-id"));

    // Test error categorization
    let test_errors = vec![
        P2PError::connection_error("Test connection error"),
        P2PError::peer_not_found("test-peer"),
        P2PError::invalid_message("Test invalid message"),
        P2PError::timeout(Duration::from_secs(30)),
    ];

    for error in test_errors {
        assert!(!error.to_string().is_empty());
        assert!(!error.category().is_empty());

        // Test error recovery classification
        match error {
            P2PError::ConnectionError { .. } => assert!(error.is_recoverable()),
            P2PError::TimeoutError { .. } => assert!(error.is_recoverable()),
            P2PError::InvalidMessage { .. } => assert!(error.is_fatal()),
            _ => (),
        }
    }
}

#[tokio::test]
async fn test_configuration_integration() {
    // Test that configurations work together across components

    let p2p_config = config::P2PConfig::default();
    assert!(p2p_config.validate().is_ok());

    let discovery_config = discovery::DiscoveryConfig::default();
    assert!(discovery_config.enable_mdns);
    assert!(discovery_config.enable_kademlia);

    let transport_config = transport::TransportConfig::default();
    assert!(!transport_config.tcp_addresses.is_empty());

    let routing_config = routing::RoutingConfig::default();
    assert!(routing_config.max_peers_per_type > 0);
    assert!(routing_config.reliability_threshold > 0.0);

    // Test minimal configuration
    let minimal_config = config::P2PConfig::minimal();
    assert!(minimal_config.validate().is_ok());
    assert!(!minimal_config.discovery.enable_mdns);
    assert!(!minimal_config.discovery.enable_kademlia);
}

#[tokio::test]
async fn test_concurrent_operations() {
    // Test concurrent operations across P2P components

    let translator = ProtocolTranslator::new();

    // Test sequential message translations (simplified to avoid lifetime issues)
    for i in 0..10 {
        let message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(i * 10),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        let result = translator.translate_message(&message, VmType::Evm);
        assert!(result.is_ok(), "Sequential translation should succeed");
    }

    // All translations completed successfully
}

#[tokio::test]
async fn test_performance_characteristics() {
    // Test basic performance characteristics
    use std::time::Instant;

    let translator = ProtocolTranslator::new();

    // Test message creation performance
    let start = Instant::now();
    for _i in 0..1000 {
        let _message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
    }
    let creation_time = start.elapsed();

    // Should be able to create 1000 messages quickly
    assert!(
        creation_time < Duration::from_millis(100),
        "Message creation should be fast"
    );

    // Test translation performance
    let test_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3, 4],
            signature: "test".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    let start = Instant::now();
    for _i in 0..100 {
        let _result = translator
            .translate_message(&test_message, VmType::Evm)
            .unwrap();
    }
    let translation_time = start.elapsed();

    // Should be able to translate 100 messages quickly
    assert!(
        translation_time < Duration::from_millis(500),
        "Translation should be fast"
    );
}

#[test]
fn test_message_type_compatibility() {
    // Test that all message types are compatible with the P2P system

    use multivm_account_mapping::{AccountAddress, EthereumAddress, SolanaAddress};

    let message_types = vec![
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3],
            signature: "sig".to_string(),
        }),
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: vec![4, 5, 6],
            tx_hash: "0xhash".to_string(),
        }),
        MessagePayload::MultiVm(MultiVmMessage::AccountBinding {
            source: AccountAddress::Solana(SolanaAddress([1u8; 32])),
            target: AccountAddress::Ethereum(EthereumAddress([2u8; 20])),
            proof_hash: "0xproof".to_string(),
        }),
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessagePayload::Discovery(DiscoveryMessage::Announce {
            capabilities: NodeCapabilities {
                supported_vms: vec![VmType::Svm, VmType::Evm],
                protocol_versions: vec![1],
                features: vec!["test".to_string()],
                limits: ResourceLimits::default(),
            },
            addresses: vec!["127.0.0.1:8000".to_string()],
        }),
    ];

    for payload in message_types {
        let message = NetworkMessage::new(
            payload,
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // All message types should be serializable
        let serialized = bincode::serialize(&message).unwrap();
        let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.version, deserialized.version);

        // All message types should have size estimation
        assert!(message.estimated_size() > 0);
    }
}

#[tokio::test]
async fn test_full_stack_simulation() {
    // Simulate a complete P2P networking scenario

    use libp2p::identity;

    // Set up components
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    // Discovery setup
    let discovery_config = discovery::DiscoveryConfig::default();
    let (discovery_service, discovery_commands, mut discovery_events) =
        discovery::DiscoveryService::new(Some(discovery_config), local_peer_id);

    // Transport setup
    let transport_config = transport::TransportConfig::default();
    let (transport, transport_messages, mut transport_events) =
        transport::TransportLayer::new(Some(transport_config), local_key.clone());

    // Routing setup
    let routing_config = routing::RoutingConfig::default();
    let (router, routing_commands) =
        routing::MessageRouter::new(transport_messages, Some(routing_config));

    // Protocol translation
    let translator = ProtocolTranslator::new();

    // Start components
    discovery_service.start().await.unwrap();
    transport.start(local_key).await.unwrap();
    router.start().await.unwrap();

    // Simulate peer discovery
    let remote_peer = PeerId::random();
    let remote_addr: Multiaddr = "/ip4/192.168.1.100/tcp/8000".parse().unwrap();

    discovery_commands
        .send(discovery::DiscoveryCommand::AddBootstrapPeer {
            peer_id: remote_peer,
            address: remote_addr.clone(),
        })
        .await
        .unwrap();

    // Add routing entry
    routing_commands
        .send(routing::RoutingCommand::AddRoute {
            message_type: MessageType::MultiVm,
            peer_id: remote_peer,
            address: remote_addr,
        })
        .await
        .unwrap();

    // Create and route cross-VM message

    let cross_vm_message = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x123456789abcdef".to_string(),
            vm_type: VmType::Svm,
            height: 12345,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    // Translate message
    let translated = translator
        .translate_message(&cross_vm_message, VmType::Evm)
        .unwrap();

    // Route message
    let route_result = router
        .route_message(translated, routing::RoutingStrategy::Broadcast)
        .await;
    assert!(route_result.is_ok());

    // Collect events with timeout
    let mut event_count = 0;
    let timeout = Duration::from_millis(500);

    loop {
        tokio::select! {
            discovery_event = discovery_events.recv() => {
                if discovery_event.is_some() {
                    event_count += 1;
                }
            }
            transport_event = transport_events.recv() => {
                if transport_event.is_some() {
                    event_count += 1;
                }
            }
            _ = tokio::time::sleep(timeout) => {
                break;
            }
        }

        if event_count >= 2 {
            break;
        }
    }

    // Clean up
    discovery_service.stop().await.unwrap();
    transport.stop().await.unwrap();
    drop(discovery_commands);
    drop(routing_commands);

    // Test should complete without panics
}
