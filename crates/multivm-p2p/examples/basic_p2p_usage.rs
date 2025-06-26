//! Basic example showing how to use the MultiVM P2P networking layer
//!
//! Run with: `cargo run --example basic_p2p_usage`

use multivm_p2p::messages::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    println!("MultiVM P2P Basic Usage Example");

    // Example 1: Create a cross-VM state sync message
    let cross_vm_msg = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x123456789abcdef".to_string(),
            vm_type: VmType::Svm,
            height: 1000,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    println!("Created cross-VM message: {:?}", cross_vm_msg.id);
    println!("Message type: {:?}", cross_vm_msg.infer_type());
    println!("Is broadcast: {}", cross_vm_msg.is_broadcast());
    println!("Estimated size: {} bytes", cross_vm_msg.estimated_size());

    // Example 2: Create a node status heartbeat
    let heartbeat = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(3600),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    println!("\nCreated heartbeat message: {:?}", heartbeat.id);

    // Example 3: Create an SVM transaction message
    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3, 4],
            signature: "svm_tx_sig".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    println!("\nCreated SVM transaction message: {:?}", svm_message.id);

    // Example 4: Create an EVM transaction message
    let evm_message = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: vec![5, 6, 7, 8],
            tx_hash: "0xevm_tx_hash".to_string(),
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );

    println!("Created EVM transaction message: {:?}", evm_message.id);

    // Example 5: Create a discovery message
    let discovery_msg = NetworkMessage::new(
        MessagePayload::Discovery(DiscoveryMessage::Announce {
            capabilities: NodeCapabilities {
                supported_vms: vec![VmType::Svm, VmType::Evm],
                protocol_versions: vec![1, 2],
                features: vec!["cross-vm".to_string(), "account-binding".to_string()],
                limits: ResourceLimits::default(),
            },
            addresses: vec!["127.0.0.1:9000".to_string()],
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    println!("Created discovery message: {:?}", discovery_msg.id);

    // Example 6: Message with metadata
    let metadata_msg = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Peer("peer123".to_string()),
    )
    .with_metadata("priority", "high")
    .with_metadata("retry_count", "0");

    println!("\nCreated message with metadata: {:?}", metadata_msg.id);
    println!("Target peer: {:?}", metadata_msg.target_peer());
    println!("Is peer message: {}", metadata_msg.is_peer_message());

    println!("\nP2P message creation examples completed successfully!");

    Ok(())
}
