//! Complete P2P module usage example
//!
//! This example demonstrates how to use the MultiVM P2P module with all
//! its features including consensus integration, authentication, and
//! enhanced transport layers.

use libp2p::identity::Keypair;
use multivm_p2p::{
    config::{NetworkConfig, P2PConfig},
    consensus_integration::{ConsensusBlock, ConsensusIntegration, ConsensusTransaction},
    core::network::P2PNetwork,
    protocol::messages::{
        ControlMessage, MessagePayload, MessageSource, MessageTarget, NetworkMessage, Priority,
        VmType,
    },
    security::auth::{AuthConfig, AuthManager},
    transport::transport::{TransportEvent, UnifiedTransport},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting complete P2P example");

    // 1. Set up authentication
    let auth_config = AuthConfig::default();
    let auth_manager = AuthManager::new(auth_config)?;

    // Generate admin API key
    let (api_key_id, api_key) = auth_manager
        .generate_api_key(
            "admin_key",
            vec!["admin".to_string(), "read".to_string(), "write".to_string()],
        )
        .await?;

    info!(
        "Generated admin API key: {} (value: {})",
        api_key_id, api_key
    );

    // Generate JWT token
    let jwt_token = auth_manager.generate_jwt_token(
        "admin_user",
        vec!["admin".to_string()],
        "administrator",
    )?;

    info!("Generated JWT token: {}", jwt_token);

    // Test authentication
    let auth_result = auth_manager.authenticate_jwt(&jwt_token, "127.0.0.1").await;
    if auth_result.success {
        info!(
            "JWT authentication successful for user: {:?}",
            auth_result.identifier
        );
    } else {
        warn!("JWT authentication failed: {:?}", auth_result.error);
    }

    // 2. Set up enhanced transport
    let transport_config = EnhancedTransportConfig::default();
    let keypair = Keypair::generate_ed25519();
    let (transport, mut transport_events) =
        EnhancedTransportLayer::new(transport_config, keypair.clone());

    // Start transport maintenance
    transport.start_maintenance().await?;
    info!("Enhanced transport layer initialized");

    // 3. Set up P2P network
    let network_config = NetworkConfig::default();
    let network = P2PNetwork::new(network_config, keypair).await?;
    let network_arc = Arc::new(RwLock::new(network));

    // Start the network
    {
        let mut network_guard = network_arc.write().await;
        network_guard.start().await?;
        info!("P2P network started");

        // Subscribe to topics
        network_guard.subscribe_topic("consensus").await?;
        network_guard.subscribe_topic("transactions").await?;
        network_guard.subscribe_topic("cross_vm").await?;
        info!("Subscribed to P2P topics");
    }

    // 4. Set up consensus integration
    let (consensus_integration, mut consensus_events, consensus_sender) =
        ConsensusIntegration::new(network_arc.clone());

    consensus_integration.start().await?;
    info!("Consensus integration started");

    // 5. Spawn background tasks to handle events

    // Handle transport events
    let transport_task = tokio::spawn(async move {
        while let Some(event) = transport_events.recv().await {
            match event {
                TransportEvent::ConnectionEstablished {
                    peer_id,
                    address,
                    protocol,
                } => {
                    info!(
                        "Transport connection established: {} via {:?} at {}",
                        peer_id, protocol, address
                    );
                }
                TransportEvent::ConnectionClosed { peer_id, reason } => {
                    info!(
                        "Transport connection closed: {} (reason: {})",
                        peer_id, reason
                    );
                }
                TransportEvent::MessageReceived { peer_id, message } => {
                    info!(
                        "Transport message received from {}: {}",
                        peer_id, message.id
                    );
                }
                _ => {
                    // Handle other transport events
                }
            }
        }
    });

    // Handle consensus events
    let consensus_task = tokio::spawn(async move {
        while let Some(event) = consensus_events.recv().await {
            match event {
                multivm_p2p::consensus_integration::P2PConsensusEvent::BlockReceived(block) => {
                    info!(
                        "Received block {} at height {} for {:?}",
                        block.hash, block.height, block.vm_type
                    );
                }
                multivm_p2p::consensus_integration::P2PConsensusEvent::TransactionReceived(tx) => {
                    info!("Received transaction {} for {:?}", tx.hash, tx.vm_type);
                }
                multivm_p2p::consensus_integration::P2PConsensusEvent::StateSyncResponse {
                    vm_type,
                    blocks,
                    from_height,
                    to_height,
                } => {
                    info!(
                        "Received state sync for {:?}: {} blocks from {} to {}",
                        vm_type,
                        blocks.len(),
                        from_height,
                        to_height
                    );
                }
                _ => {
                    // Handle other consensus events
                }
            }
        }
    });

    // 6. Simulate some network activity

    info!("Simulating network activity...");

    // Send some test messages
    {
        let network_guard = network_arc.read().await;

        // Send an EVM transaction
        let evm_message = NetworkMessage {
            id: "evm_tx_001".to_string(),
            source: MessageSource::EVM,
            target: MessageTarget::Broadcast,
            payload: MessagePayload::EvmTransaction(vec![0x60, 0x60, 0x40, 0x52]), // Simple bytecode
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            priority: Priority::High,
            ttl: 300,
            metadata: HashMap::new(),
        };

        network_guard.broadcast(evm_message).await?;
        info!("Broadcast EVM transaction");

        // Send an SVM transaction
        let svm_message = NetworkMessage {
            id: "svm_tx_001".to_string(),
            source: MessageSource::SVM,
            target: MessageTarget::Broadcast,
            payload: MessagePayload::SvmTransaction(vec![1, 2, 3, 4, 5, 6, 7, 8]),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            priority: Priority::High,
            ttl: 300,
            metadata: HashMap::new(),
        };

        network_guard.broadcast(svm_message).await?;
        info!("Broadcast SVM transaction");

        // Send a cross-VM transfer
        let cross_vm_message = NetworkMessage {
            id: "cross_vm_001".to_string(),
            source: MessageSource::MultiVM,
            target: MessageTarget::CrossVM,
            payload: MessagePayload::CrossVmTransfer {
                from_vm: "EVM".to_string(),
                to_vm: "SVM".to_string(),
                amount: 1000000,
                recipient: "SvmAddress123".to_string(),
            },
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            priority: Priority::Critical,
            ttl: 600,
            metadata: HashMap::new(),
        };

        network_guard.broadcast(cross_vm_message).await?;
        info!("Broadcast cross-VM transfer");
    }

    // 7. Simulate consensus activity

    // Create a test block
    let test_block = ConsensusBlock {
        height: 100,
        hash: "block_abc123".to_string(),
        previous_hash: "block_def456".to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        transactions: vec![
            ConsensusTransaction {
                hash: "tx_111".to_string(),
                data: vec![1, 2, 3, 4],
                vm_type: VmType::Evm,
                gas_limit: 21000,
                fee: 1000,
                nonce: 1,
            },
            ConsensusTransaction {
                hash: "tx_222".to_string(),
                data: vec![5, 6, 7, 8],
                vm_type: VmType::Svm,
                gas_limit: 5000,
                fee: 500,
                nonce: 2,
            },
        ],
        proposer: "validator_001".to_string(),
        vm_type: VmType::Evm,
        cross_vm_operations: vec![],
    };

    // Send block creation event to consensus integration
    let block_event = multivm_p2p::consensus_integration::ConsensusEvent::BlockCreated(test_block);
    consensus_sender.send(block_event)?;
    info!("Sent block creation event to consensus integration");

    // 8. Check network health and statistics

    tokio::time::sleep(Duration::from_secs(2)).await;

    {
        let network_guard = network_arc.read().await;
        let health_report = network_guard.health_check().await?;
        info!("Network health: {:?}", health_report.status);
        info!("Connected peers: {}", health_report.connected_peers);
        info!("Message queue size: {}", health_report.message_queue_size);
    }

    // Get transport metrics
    let transport_metrics = transport.get_metrics().await;
    info!("Transport metrics:");
    info!(
        "  Total connections: {}",
        transport_metrics.total_connections
    );
    info!(
        "  Active connections: {}",
        transport_metrics.active_connections
    );
    info!("  Messages sent: {}", transport_metrics.messages_sent);
    info!(
        "  Messages received: {}",
        transport_metrics.messages_received
    );
    info!("  Bytes sent: {}", transport_metrics.bytes_sent);
    info!("  Bytes received: {}", transport_metrics.bytes_received);

    // Get authentication statistics
    let auth_stats = auth_manager.get_auth_stats().await;
    info!("Authentication statistics:");
    info!("  Total attempts: {}", auth_stats.total_attempts);
    info!("  Successful attempts: {}", auth_stats.successful_attempts);
    info!("  Failed attempts: {}", auth_stats.failed_attempts);
    info!("  Active API keys: {}", auth_stats.active_api_keys);

    // 9. Clean shutdown

    info!("Waiting for background tasks...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Stop the network
    {
        let mut network_guard = network_arc.write().await;
        network_guard.stop().await?;
        info!("P2P network stopped");
    }

    // Cancel background tasks
    transport_task.abort();
    consensus_task.abort();

    info!("Complete P2P example finished successfully");

    Ok(())
}

/// Helper function to demonstrate advanced P2P features
async fn demonstrate_advanced_features(
    network: Arc<RwLock<P2PNetwork>>,
    auth_manager: &AuthManager,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Demonstrating advanced P2P features");

    // Demonstrate peer discovery simulation
    {
        let network_guard = network.read().await;

        // In a real implementation, this would connect to actual peers
        // For demo purposes, we'll just show the API
        let mock_peer_address = "/ip4/127.0.0.1/tcp/8001".parse()?;
        info!("Would attempt to connect to peer at: {}", mock_peer_address);
    }

    // Demonstrate authentication with different methods
    let jwt_token = auth_manager.generate_jwt_token(
        "peer_001",
        vec!["peer".to_string(), "sync".to_string()],
        "peer",
    )?;

    let auth_result = auth_manager
        .authenticate_jwt(&jwt_token, "192.168.1.100")
        .await;
    if auth_result.success {
        info!("Peer authentication successful");
    }

    // Demonstrate message filtering and priority handling
    {
        let network_guard = network.read().await;

        for priority in [
            Priority::Low,
            Priority::Normal,
            Priority::High,
            Priority::Critical,
        ] {
            let message = NetworkMessage {
                id: format!("priority_{:?}_msg", priority),
                source: MessageSource::MultiVmLayer,
                target: MessageTarget::Broadcast,
                payload: MessagePayload::Control(ControlMessage::StatusRequest),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                priority,
                ttl: 300,
                metadata: HashMap::new(),
            };

            network_guard.broadcast(message).await?;
        }

        info!("Sent messages with different priorities");
    }

    Ok(())
}

/// Helper function to simulate network stress testing
async fn stress_test_network(
    network: Arc<RwLock<P2PNetwork>>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting network stress test");

    let start_time = std::time::Instant::now();
    let message_count = 1000;

    {
        let network_guard = network.read().await;

        for i in 0..message_count {
            let message = NetworkMessage {
                id: format!("stress_msg_{:04}", i),
                source: MessageSource::MultiVM,
                target: MessageTarget::Broadcast,
                payload: MessagePayload::Text(format!("Stress test message number {}", i)),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                priority: Priority::Medium,
                ttl: 300,
                metadata: HashMap::new(),
            };

            network_guard.broadcast(message).await?;

            // Small delay to avoid overwhelming the network
            if i % 100 == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }

    let elapsed = start_time.elapsed();
    let messages_per_second = message_count as f64 / elapsed.as_secs_f64();

    info!("Stress test completed:");
    info!("  Messages sent: {}", message_count);
    info!("  Time elapsed: {:?}", elapsed);
    info!("  Throughput: {:.2} messages/second", messages_per_second);

    Ok(())
}
