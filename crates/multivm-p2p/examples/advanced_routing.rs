//! Advanced example showing P2P message routing and protocol handling
//!
//! Run with: `cargo run --example advanced_routing`

use libp2p::{Multiaddr, PeerId};
use multivm_p2p::*;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with more detail
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();

    // Create custom routing configuration
    let routing_config = routing::RoutingConfig::default();

    // Create message router
    let (tx, mut rx) = mpsc::channel(100);
    let (router, command_sender) = routing::MessageRouter::new(tx, Some(routing_config));

    // Start the router
    router.start().await?;
    println!("Message router started");

    // Add some routes for different message types
    let peer1 = PeerId::random();
    let addr1: Multiaddr = "/ip4/10.0.0.1/tcp/9000".parse()?;

    let peer2 = PeerId::random();
    let addr2: Multiaddr = "/ip4/10.0.0.2/tcp/9000".parse()?;

    let peer3 = PeerId::random();
    let addr3: Multiaddr = "/ip4/10.0.0.3/tcp/9000".parse()?;

    // Route SVM messages to peer1
    command_sender
        .send(routing::RoutingCommand::AddRoute {
            message_type: MessageType::Svm,
            peer_id: peer1,
            address: addr1.clone(),
        })
        .await?;

    // Route EVM messages to peer2
    command_sender
        .send(routing::RoutingCommand::AddRoute {
            message_type: MessageType::Evm,
            peer_id: peer2,
            address: addr2.clone(),
        })
        .await?;

    // Route MultiVM messages to peer3
    command_sender
        .send(routing::RoutingCommand::AddRoute {
            message_type: MessageType::MultiVm,
            peer_id: peer3,
            address: addr3.clone(),
        })
        .await?;

    println!("Added routing rules for different VM types");

    // Example 1: Route messages based on type
    let messages = vec![
        NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Block {
                block_data: vec![1, 2, 3],
                block_hash: "svm_block_123".to_string(),
                height: 1000,
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        ),
        NetworkMessage::new(
            MessagePayload::Evm(EvmMessage::Block {
                block_data: vec![4, 5, 6],
                block_hash: "0xevm_block_456".to_string(),
                block_number: 2000,
            }),
            MessageSource::EvmExecution,
            MessageTarget::Broadcast,
        ),
        NetworkMessage::new(
            MessagePayload::MultiVm(MultiVmMessage::StateSync {
                state_root: "0xstate_root_789".to_string(),
                vm_type: VmType::Svm,
                height: 3000,
            }),
            MessageSource::MultiVmLayer,
            MessageTarget::Broadcast,
        ),
    ];

    // Route messages using different strategies
    for (i, message) in messages.into_iter().enumerate() {
        let msg_type = message.infer_type();
        println!("\nRouting message {} with type {:?}", i + 1, msg_type);

        // Use appropriate routing strategy
        let strategy = match msg_type {
            MessageType::Control => routing::RoutingStrategy::Broadcast,
            MessageType::Discovery => routing::RoutingStrategy::Broadcast,
            _ => routing::RoutingStrategy::Direct(match msg_type {
                MessageType::Svm => peer1,
                MessageType::Evm => peer2,
                MessageType::MultiVm => peer3,
                _ => peer1,
            }),
        };

        router.route_message(message, strategy).await?;
    }

    // Example 2: Handle routed messages
    println!("\nReceiving routed messages:");
    let mut count = 0;
    while let Ok(Some((peer_id, message))) =
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
    {
        count += 1;
        println!(
            "  Message {} routed to peer {}: {:?}",
            count,
            peer_id,
            message.infer_type()
        );
    }

    // Example 3: Get routing statistics
    let stats = router.get_routing_stats().await;
    println!("\nRouting Statistics:");
    println!("  Total messages routed: {}", stats.total_messages_routed);
    println!("  Total routing failures: {}", stats.failed_routes);
    println!("  Active routing entries: {}", stats.active_routes);

    // Example 4: Advanced routing with custom peer selection
    let custom_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Route with broadcast strategy
    router
        .route_message(custom_message.clone(), routing::RoutingStrategy::Broadcast)
        .await?;
    println!("\nRouted control message with broadcast strategy");

    // Example 5: Remove routes
    command_sender
        .send(routing::RoutingCommand::RemoveRoute {
            message_type: MessageType::Svm,
            peer_id: peer1,
        })
        .await?;
    println!("\nRemoved SVM route to peer1");

    // Final stats
    let final_stats = router.get_routing_stats().await;
    println!("\nFinal routing statistics:");
    println!("  Total routed: {}", final_stats.successful_routes);
    println!("  Total failed: {}", final_stats.failed_routes);

    println!("\nRouting example completed");
    Ok(())
}
