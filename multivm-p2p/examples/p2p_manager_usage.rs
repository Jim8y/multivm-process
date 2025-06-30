//! Complete example showing P2P Manager usage with the new refactored architecture
//!
//! This example demonstrates:
//! - Setting up a P2P node with the refactored manager
//! - Sending and receiving messages
//! - Security features (authentication, encryption)
//! - Monitoring and statistics
//!
//! Run with: `cargo run --example p2p_manager_usage`

use libp2p::{identity::Keypair, PeerId};
use multivm_p2p::{
    config::{AuthConfig, AuthMethod, NetworkConfig, P2PConfig, RateLimitConfig, SecurityConfig},
    core::manager::{ManagerHandle, P2PManager},
    error::P2PResult,
    protocol::messages::*,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> P2PResult<()> {
    // Initialize logging
    // Initialize basic tracing - in production you'd use proper configuration
    tracing_subscriber::fmt::init();
    info!("Starting P2P Manager usage example");

    // Example 1: Create a basic P2P configuration
    let config = create_example_config();

    // Example 2: Generate keypair for this node
    let keypair = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(keypair.public());
    info!("Local peer ID: {}", local_peer_id);

    // Example 3: Create and start P2P manager
    let (mut p2p_manager, command_rx) = P2PManager::new(config, keypair);
    let handle = p2p_manager.get_handle();
    info!("P2P Manager created successfully");

    // Start the manager
    p2p_manager.start(command_rx).await?;
    info!("P2P Manager started");

    // Example 4: Send different types of messages
    demonstrate_message_sending(&handle).await?;

    // Example 5: Demonstrate security features
    demonstrate_security_features(&handle).await?;

    // Example 6: Monitor statistics
    demonstrate_monitoring(&handle).await?;

    // Example 7: Clean shutdown
    info!("Shutting down P2P Manager");
    p2p_manager.stop().await?;
    info!("P2P Manager shutdown complete");

    Ok(())
}

/// Create a comprehensive P2P configuration
fn create_example_config() -> P2PConfig {
    P2PConfig {
        network: NetworkConfig {
            peer_id: None, // Will be generated from keypair
            listen_addresses: vec![
                "/ip4/127.0.0.1/tcp/0".to_string(), // Random port
            ],
            external_addresses: vec![],
            max_connections: 50,
            connection_timeout: Duration::from_secs(10),
            keep_alive_interval: Duration::from_secs(30),
            enable_nat_traversal: true,
            enable_relay: false,
            enable_autonat: true,
            max_message_size: 1024 * 1024, // 1MB
        },
        transport: Default::default(),
        discovery: Default::default(),
        protocol: Default::default(),
        security: SecurityConfig {
            enable_noise: true,
            noise: Default::default(),
            rate_limiting: RateLimitConfig {
                enabled: true,
                max_requests_per_second: 100.0,
                burst_size: 20,
                window_duration: Duration::from_secs(1),
                penalty_duration: Duration::from_secs(60),
            },
            authentication: AuthConfig {
                enabled: true,
                method: AuthMethod::Ed25519,
                trusted_peers: vec![],
                timeout: Duration::from_secs(30),
            },
            firewall: Default::default(),
        },
        logging: Default::default(),
        rate_limiting: RateLimitConfig {
            enabled: true,
            max_requests_per_second: 100.0,
            burst_size: 20,
            window_duration: Duration::from_secs(1),
            penalty_duration: Duration::from_secs(60),
        },
        auth: AuthConfig {
            enabled: true,
            method: AuthMethod::Ed25519,
            trusted_peers: vec![],
            timeout: Duration::from_secs(30),
        },
    }
}

/// Demonstrate different types of message sending
async fn demonstrate_message_sending(handle: &ManagerHandle) -> P2PResult<()> {
    info!("Demonstrating message sending...");

    // 1. Cross-VM state synchronization message
    let state_sync_msg = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x1234567890abcdef".to_string(),
            vm_type: VmType::Svm,
            height: 12345,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    handle.send_message(state_sync_msg, Priority::High).await?;
    info!("Sent cross-VM state sync message");

    // 2. Consensus proposal message
    let consensus_msg = NetworkMessage::new(
        MessagePayload::Custom(serde_json::json!({
            "type": "consensus_proposal",
            "round": 10,
            "proposer": "node_1",
            "block_hash": "0xabcdef"
        })),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    handle
        .send_message(consensus_msg, Priority::Critical)
        .await?;
    info!("Sent consensus proposal message");

    // 3. Node heartbeat
    let heartbeat = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(3600),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    handle.send_message(heartbeat, Priority::Low).await?;
    info!("Sent heartbeat message");

    // 4. Peer discovery announcement
    let discovery_msg = NetworkMessage::new(
        MessagePayload::Discovery(DiscoveryMessage::Announce {
            capabilities: NodeCapabilities {
                supported_vms: vec![VmType::Svm, VmType::Evm],
                protocol_versions: vec![1, 2],
                features: vec![
                    "cross-vm".to_string(),
                    "account-binding".to_string(),
                    "fast-sync".to_string(),
                ],
                limits: ResourceLimits {
                    max_connections: 100,
                    max_message_size: 1024 * 1024,
                    rate_limit: 100.0,
                },
            },
            addresses: vec!["127.0.0.1:9000".to_string(), "127.0.0.1:9001".to_string()],
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    handle.send_message(discovery_msg, Priority::Normal).await?;
    info!("Sent discovery announcement");

    Ok(())
}

/// Demonstrate security features
async fn demonstrate_security_features(handle: &ManagerHandle) -> P2PResult<()> {
    info!("Demonstrating security features...");

    // Security features are automatically handled by the SecurityCoordinator
    // This is more of a demonstration of security-related messages

    // 1. Authentication challenge
    let auth_challenge = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer("example_peer".to_string()),
    )
    .with_metadata("requires_auth", "true")
    .with_metadata("challenge_type", "ed25519");

    info!("Created authentication challenge message");

    // 2. Security status message
    let security_status = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(7200),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    )
    .with_metadata("security_level", "high")
    .with_metadata("encryption_enabled", "true");

    handle
        .send_message(security_status, Priority::Normal)
        .await?;
    info!("Sent security status message");

    // Note: Actual encryption, authentication, and rate limiting are handled
    // transparently by the SecurityCoordinator within the P2PManager

    Ok(())
}

/// Demonstrate monitoring and statistics
async fn demonstrate_monitoring(handle: &ManagerHandle) -> P2PResult<()> {
    info!("Demonstrating monitoring and statistics...");

    // Get current statistics
    let stats = handle.get_stats().await?;

    info!("=== P2P Manager Statistics ===");
    info!("Uptime: {:?}", stats.uptime);
    info!("Connected peers: {}", stats.peers_connected);
    info!("Messages sent: {}", stats.messages_sent);
    info!("Messages received: {}", stats.messages_received);
    info!("Bytes sent: {}", stats.bytes_sent);
    info!("Bytes received: {}", stats.bytes_received);

    // Send a few more messages to update stats
    for i in 0..5 {
        let test_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        )
        .with_metadata("test_sequence", &i.to_string());

        handle.send_message(test_msg, Priority::Low).await?;
        sleep(Duration::from_millis(100)).await;
    }

    // Get updated statistics
    let updated_stats = handle.get_stats().await?;
    info!("=== Updated Statistics ===");
    info!(
        "Messages sent: {} (+{})",
        updated_stats.messages_sent,
        updated_stats.messages_sent - stats.messages_sent
    );

    // Note: In a real application, you would also monitor:
    // - Security metrics (ban counts, rate limit violations)
    // - Performance metrics (latency, throughput)
    // - Health status of components
    // These would be available through the SecurityCoordinator and MonitoringCoordinator

    Ok(())
}
