//! Advanced example showing message routing and protocol handling
//!
//! Run with: `cargo run --example advanced_routing`

use multivm_p2p::messages::*;
use multivm_p2p::routing::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MultiVM P2P Advanced Routing Example");

    // Example 1: Create different types of messages for routing
    let messages = vec![
        // SVM message
        NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Block {
                block_data: Box::new(vec![1, 2, 3]),
                block_hash: "svm_block_hash".to_string(),
                height: 100,
            }),
            MessageSource::SvmExecution,
            MessageTarget::Protocol("svm".to_string()),
        ),
        // EVM message
        NetworkMessage::new(
            MessagePayload::Evm(EvmMessage::Block {
                block_data: Box::new(vec![4, 5, 6]),
                block_hash: "0xevm_block_hash".to_string(),
                block_number: 200,
            }),
            MessageSource::EvmExecution,
            MessageTarget::Protocol("evm".to_string()),
        ),
        // MultiVM message
        NetworkMessage::new(
            MessagePayload::MultiVm(MultiVmMessage::StateSync {
                state_root: "0xstate_root".to_string(),
                vm_type: VmType::Svm,
                height: 150,
            }),
            MessageSource::MultiVmLayer,
            MessageTarget::Broadcast,
        ),
        // Control message
        NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        ),
        // Discovery message
        NetworkMessage::new(
            MessagePayload::Discovery(DiscoveryMessage::Announce {
                capabilities: NodeCapabilities {
                    supported_vms: vec![VmType::Svm, VmType::Evm],
                    protocol_versions: vec![1],
                    features: vec!["routing".to_string()],
                    limits: ResourceLimits::default(),
                },
                addresses: vec!["127.0.0.1:9000".to_string()],
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        ),
    ];

    // Example 2: Demonstrate message routing logic
    println!("\nMessage Routing Analysis:");
    for (i, msg) in messages.iter().enumerate() {
        let msg_type = msg.infer_type();
        let routing_strategy = determine_routing_strategy(&msg_type);

        println!("Message {}: {:?}", i + 1, msg.id);
        println!("  Type: {msg_type:?}");
        println!("  Target: {:?}", msg.target);
        println!("  Routing Strategy: {routing_strategy:?}");
        println!("  Size: {} bytes", msg.estimated_size());
        println!();
    }

    // Example 3: Message filtering and prioritization
    println!("Message Filtering:");
    let high_priority_messages: Vec<_> = messages
        .iter()
        .filter(|msg| {
            matches!(
                msg.infer_type(),
                MessageType::Control | MessageType::Discovery
            )
        })
        .collect();

    println!("High priority messages: {}", high_priority_messages.len());

    let vm_specific_messages: Vec<_> = messages
        .iter()
        .filter(|msg| matches!(msg.infer_type(), MessageType::Svm | MessageType::Evm))
        .collect();

    println!("VM-specific messages: {}", vm_specific_messages.len());

    // Example 4: Create targeted messages
    println!("\nTargeted Message Examples:");

    let peer_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer("specific_peer_123".to_string()),
    );

    println!("Peer message target: {:?}", peer_message.target_peer());

    let local_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: Box::new(vec![9, 10, 11]),
            signature: "local_tx_sig".to_string(),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Local(VmType::Svm),
    );

    println!("Local message for VM: {:?}", local_message.is_local());

    println!("\nAdvanced routing examples completed successfully!");

    Ok(())
}

/// Determine routing strategy based on message type
fn determine_routing_strategy(msg_type: &MessageType) -> RoutingStrategy {
    match msg_type {
        MessageType::Control => RoutingStrategy::Broadcast,
        MessageType::Discovery => RoutingStrategy::Broadcast,
        MessageType::Svm => RoutingStrategy::Gossip("svm_nodes".to_string()),
        MessageType::Evm => RoutingStrategy::Gossip("evm_nodes".to_string()),
        MessageType::MultiVm => RoutingStrategy::Broadcast,
    }
}
