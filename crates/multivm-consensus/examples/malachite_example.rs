//! Example demonstrating Malachite consensus integration in MultiVM
//!
//! This example shows how to initialize and use Malachite consensus
//! for cross-VM transaction processing and block production.

use multivm_consensus::{
    malachite::{ConsensusParams, MalachiteConfig, NetworkConfig, ValidatorInfo},
    manager::{
        AlgorithmConfig, ConsensusAlgorithmType, ConsensusManagerConfig, MultiVMConsensusManager,
        NetworkConfig as ManagerNetworkConfig,
    },
    state::StateManagerConfig,
    ConsensusResult,
};
use tracing::info;

#[tokio::main]
async fn main() -> ConsensusResult<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Malachite consensus example");

    // Create Malachite configuration
    let malachite_config = MalachiteConfig {
        node_id: "example-node-1".to_string(),
        network_config: NetworkConfig {
            listen_addr: "127.0.0.1:26656".to_string(),
            peers: vec!["127.0.0.1:26657".to_string(), "127.0.0.1:26658".to_string()],
        },
        consensus_params: ConsensusParams {
            block_time_ms: 1000,
            max_block_size: 1024 * 1024, // 1MB
            timeout_propose_ms: 3000,
            timeout_prevote_ms: 1000,
            timeout_precommit_ms: 1000,
        },
        validators: vec![
            ValidatorInfo {
                public_key: "validator1_pubkey".to_string(),
                voting_power: 100,
            },
            ValidatorInfo {
                public_key: "validator2_pubkey".to_string(),
                voting_power: 100,
            },
            ValidatorInfo {
                public_key: "validator3_pubkey".to_string(),
                voting_power: 100,
            },
        ],
    };

    // Create consensus manager configuration
    let manager_config = ConsensusManagerConfig {
        node_id: Some("example-node-1".to_string()),
        algorithm: ConsensusAlgorithmType::Malachite,
        algorithm_config: AlgorithmConfig::Malachite(malachite_config),
        state_manager_config: StateManagerConfig::default(),
        block_proposal_interval_ms: 1000,
        max_transactions_per_block: 1000,
        enable_auto_proposal: true,
        network_config: ManagerNetworkConfig {
            node_id: "example-node-1".to_string(),
            listen_address: "127.0.0.1:8080".to_string(),
            bootstrap_nodes: vec!["127.0.0.1:8081".to_string(), "127.0.0.1:8082".to_string()],
            enable_encryption: false,
        },
    };

    // Initialize the consensus manager
    let mut consensus_manager = MultiVMConsensusManager::new(manager_config).await?;

    // Subscribe to consensus events
    let mut event_receiver = consensus_manager.subscribe_events();

    // Start the consensus manager
    consensus_manager.start().await?;
    info!("Consensus manager started successfully");

    // Demonstrate consensus statistics
    let stats = consensus_manager.get_consensus_stats().await?;
    info!("Consensus stats: {:?}", stats);

    // Example of processing a cross-VM transaction from the transaction pool
    use multivm_account_mapping::{AssetType, MultivmAccountId, SpecialTransaction};

    let cross_vm_tx = SpecialTransaction::CrossVmTransfer {
        from: MultivmAccountId::from_seed(b"sender_account"),
        to: MultivmAccountId::from_seed(b"receiver_account"),
        amount: 1000,
        asset_type: AssetType::Native,
        memo: Some("Example cross-VM transfer".to_string()),
    };

    info!("Processing cross-VM transaction");
    consensus_manager
        .process_cross_vm_transaction(cross_vm_tx)
        .await?;

    // Handle consensus events for a short period
    let event_handler = tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            info!("Consensus event: {:?}", event);
        }
    });

    // Let the consensus run for a bit
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Stop the consensus manager
    consensus_manager.stop().await?;
    info!("Consensus manager stopped");

    // Stop the event handler
    event_handler.abort();

    info!("Malachite consensus example completed");
    Ok(())
}
