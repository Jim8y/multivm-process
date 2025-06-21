//! Basic example showing how to use the MultiVM P2P networking layer
//!
//! Run with: `cargo run --example basic_p2p_usage`

use multivm_p2p::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create P2P configuration
    let config = network::NetworkConfig {
        max_peers: 100,
        enable_mdns: true, // Enable local peer discovery
        ..Default::default()
    };

    // Add bootstrap peers (if any)
    // config.bootstrap_peers.push("/ip4/1.2.3.4/tcp/9000/p2p/QmPeer123...".parse()?);

    // Create the P2P network
    let mut network = network::P2PNetwork::new(config)
        .await
        .map_err(|e| format!("Failed to create network: {}", e))?;

    println!("Local peer ID: {}", network.local_peer_id());

    // Start the network
    network.start().await?;
    println!("P2P network started");

    // Subscribe to essential topics
    network.subscribe("multivm-broadcast").await?;
    network.subscribe("cross-vm-transactions").await?;
    network.subscribe("network-discovery").await?;
    println!("Subscribed to essential topics");

    // Example 1: Send a cross-VM state sync message
    let cross_vm_msg = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x123456789abcdef".to_string(),
            vm_type: VmType::Svm,
            height: 1000,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    network.broadcast(cross_vm_msg).await?;
    println!("Broadcasted cross-VM transaction");

    // Example 2: Send a node status heartbeat
    let heartbeat = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(3600),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    network.broadcast(heartbeat).await?;
    println!("Sent heartbeat");

    // Example 3: Check network health
    let health = network.health_check().await?;
    println!("\nNetwork Health Report:");
    println!("  Status: {:?}", health.status);
    println!("  Connected peers: {}", health.connected_peers);
    println!("  Subscribed topics: {}", health.subscribed_topics);
    println!("  Message throughput: {}", health.message_throughput);

    if !health.issues.is_empty() {
        println!("  Issues detected:");
        for issue in &health.issues {
            println!("    - {}", issue);
        }

        // Attempt self-healing
        println!("\nAttempting self-healing...");
        let healing_actions = network.self_heal().await?;
        for action in healing_actions {
            println!("  - {}", action);
        }
    }

    // Example 4: Get network statistics
    let stats = network.get_network_stats().await?;
    println!("\nNetwork Statistics:");
    println!("  Messages sent: {}", stats.messages_sent);
    println!("  Messages received: {}", stats.messages_received);
    println!("  Bytes sent: {}", stats.bytes_sent);
    println!("  Bytes received: {}", stats.bytes_received);

    // Example 5: Protocol translation
    let translator = ProtocolTranslator::new();

    // Create an SVM message
    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3, 4],
            signature: "svm_tx_sig".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    // Translate to EVM format
    let _evm_translated = translator
        .translate_message(&svm_message, VmType::Evm)
        .map_err(|e| format!("Failed to translate message: {}", e))?;
    println!("\nTranslated SVM message to EVM format");

    // Keep running for a while to see network activity
    println!("\nNetwork running... Press Ctrl+C to stop");
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Graceful shutdown
    network.stop().await?;
    println!("P2P network stopped");

    Ok(())
}
