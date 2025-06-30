//! Simple P2P node example that demonstrates basic functionality
//!
//! This example creates a basic P2P node, sends some messages, and shows statistics.
//! It's designed to work with the refactored P2P manager architecture.
//!
//! Run with: `cargo run --example simple_node`

use libp2p::identity::Keypair;
use multivm_p2p::{
    config::P2PConfig,
    core::manager::{ManagerHandle, P2PManager},
    protocol::messages::*,
};
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see what's happening
    tracing_subscriber::fmt()
        .with_env_filter("multivm_p2p=info")
        .init();

    info!("Starting simple P2P node example");

    // Step 1: Create configuration
    let mut config = P2PConfig::default();
    config.network.listen_addresses = vec!["/ip4/127.0.0.1/tcp/0".to_string()];
    config.network.max_connections = 10;
    config.security.rate_limiting.enabled = false; // Disable for simplicity

    // Step 2: Generate keypair
    let keypair = Keypair::generate_ed25519();
    let peer_id = libp2p::PeerId::from(keypair.public());
    info!("Generated peer ID: {}", peer_id);

    // Step 3: Create and start P2P manager
    let (mut p2p_manager, command_rx) = P2PManager::new(config, keypair);
    let handle = p2p_manager.get_handle();
    p2p_manager.start(command_rx).await?;
    info!("P2P manager started successfully");

    // Step 4: Send various types of messages
    send_example_messages(&handle).await?;

    // Step 5: Display statistics
    show_statistics(&handle).await?;

    // Step 6: Graceful shutdown
    info!("Shutting down P2P node");
    p2p_manager.stop().await?;
    info!("Shutdown complete");

    Ok(())
}

async fn send_example_messages(handle: &ManagerHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("Sending example messages...");

    // 1. Send a heartbeat message
    let heartbeat = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(60),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );
    handle.send_message(heartbeat, Priority::Low).await?;
    info!("✓ Sent heartbeat message");

    // 2. Send a cross-VM state sync message
    let state_sync = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            vm_type: VmType::Svm,
            height: 12345,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );
    handle.send_message(state_sync, Priority::High).await?;
    info!("✓ Sent cross-VM state sync message");

    // 3. Send an SVM transaction message
    let svm_transaction = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: Box::new(vec![1, 2, 3, 4, 5]),
            signature: "svm_signature_example".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );
    handle
        .send_message(svm_transaction, Priority::Normal)
        .await?;
    info!("✓ Sent SVM transaction message");

    // 4. Send an EVM transaction message
    let evm_transaction = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: Box::new(vec![6, 7, 8, 9, 10]),
            tx_hash: "0xevm_transaction_hash_example".to_string(),
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );
    handle
        .send_message(evm_transaction, Priority::Normal)
        .await?;
    info!("✓ Sent EVM transaction message");

    // 5. Send a discovery announcement
    let discovery = NetworkMessage::new(
        MessagePayload::Discovery(DiscoveryMessage::Announce {
            capabilities: NodeCapabilities {
                supported_vms: vec![VmType::Svm, VmType::Evm],
                protocol_versions: vec![1, 2],
                features: vec![
                    "cross-vm".to_string(),
                    "state-sync".to_string(),
                    "fast-finality".to_string(),
                ],
                limits: ResourceLimits {
                    max_connections: 100,
                    max_message_size: 1024 * 1024,
                    rate_limit: 100.0,
                },
            },
            addresses: vec!["127.0.0.1:8000".to_string()],
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );
    handle.send_message(discovery, Priority::Normal).await?;
    info!("✓ Sent discovery announcement");

    // 6. Send a custom consensus message
    let consensus_msg = NetworkMessage::new(
        MessagePayload::Custom(serde_json::json!({
            "type": "consensus_vote",
            "round": 42,
            "block_hash": "0xabcdef1234567890",
            "validator": "node_001",
            "vote": "approve"
        })),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    )
    .with_metadata("priority", "critical")
    .with_metadata("retry_count", "0");

    handle
        .send_message(consensus_msg, Priority::Critical)
        .await?;
    info!("✓ Sent custom consensus message");

    // Add a small delay to allow message processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(())
}

async fn show_statistics(handle: &ManagerHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("Retrieving P2P statistics...");

    match handle.get_stats().await {
        Ok(stats) => {
            info!("=== P2P Node Statistics ===");
            info!("Uptime: {:?}", stats.uptime);
            info!("Connected peers: {}", stats.peers_connected);
            info!("Messages sent: {}", stats.messages_sent);
            info!("Messages received: {}", stats.messages_received);
            info!("Bytes sent: {}", stats.bytes_sent);
            info!("Bytes received: {}", stats.bytes_received);

            if stats.messages_sent > 0 {
                info!("✓ Successfully sent {} messages", stats.messages_sent);
            } else {
                warn!("⚠ No messages were sent");
            }
        }
        Err(e) => {
            warn!("Failed to retrieve statistics: {}", e);
        }
    }

    Ok(())
}
