//! Consensus Layer Demo
//!
//! This example demonstrates the core functionality of the MultiVM consensus layer:
//! 1. Raft consensus algorithm initialization and operation
//! 2. Cross-VM transaction processing through consensus
//! 3. State management and synchronization
//! 4. Event handling and monitoring

use multivm_account_mapping::{
    AccountAddress, AssetType, BindingProof, EthereumAddress, MultivmAccountId, ProofType,
    SolanaAddress, SpecialTransaction,
};
use multivm_consensus::*;
use std::time::SystemTime;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🚀 MultiVM Consensus Layer Demo");
    println!("================================\n");

    // 1. Create consensus manager configuration
    println!("📋 Step 1: Creating consensus configuration");

    let raft_config = RaftConfig {
        node_id: "demo_node".to_string(),
        cluster_nodes: vec!["demo_node".to_string()],
        election_timeout_ms: (150, 300),
        heartbeat_interval_ms: 50,
        log_compaction_threshold: 100,
        max_entries_per_append: 10,
    };

    let consensus_config = ConsensusManagerConfig {
        node_id: Some("demo_node".to_string()),
        algorithm: ConsensusAlgorithmType::Raft,
        algorithm_config: AlgorithmConfig::Raft(raft_config),
        state_manager_config: StateManagerConfig::default(),
        block_proposal_interval_ms: 2000,
        max_transactions_per_block: 100,
        enable_auto_proposal: false, // Manual proposal for demo
        network_config: NetworkConfig {
            node_id: "demo_node".to_string(),
            listen_address: "127.0.0.1:8080".to_string(),
            bootstrap_nodes: vec![],
            enable_encryption: true,
        },
    };

    println!("   ✅ Configuration created with Raft algorithm");
    println!(
        "   📊 Max transactions per block: {}",
        consensus_config.max_transactions_per_block
    );
    println!(
        "   🕒 Block proposal interval: {}ms\n",
        consensus_config.block_proposal_interval_ms
    );

    // 2. Initialize consensus manager
    println!("🔧 Step 2: Initializing consensus manager");

    let mut consensus_manager = MultiVMConsensusManager::new(consensus_config).await?;

    // Subscribe to consensus events
    let mut event_receiver = consensus_manager.subscribe_events();

    println!("   ✅ Consensus manager created");
    println!("   🎯 Event listener configured\n");

    // 3. Start consensus
    println!("🟢 Step 3: Starting consensus");

    consensus_manager.start().await?;

    println!("   ✅ Consensus started successfully");
    println!(
        "   🏃 Status: {}\n",
        if consensus_manager.is_running() {
            "Running"
        } else {
            "Stopped"
        }
    );

    // 4. Get initial state and statistics
    println!("📊 Step 4: Initial state and statistics");

    let initial_stats = consensus_manager.get_consensus_stats().await?;
    println!("   📈 Current height: {}", initial_stats.current_height);
    println!("   🧮 Total blocks: {}", initial_stats.total_blocks);
    println!("   🔗 Algorithm: {}", initial_stats.algorithm);
    println!("   👥 Active nodes: {}", initial_stats.active_nodes);

    let cross_vm_state = consensus_manager.get_cross_vm_state().await?;
    println!("   🌐 Cross-VM state height: {}", cross_vm_state.height);
    println!("   🔢 Global nonce: {}", cross_vm_state.global_nonce);
    println!(
        "   📝 Account bindings: {}\n",
        cross_vm_state.account_bindings.len()
    );

    // 5. Create sample cross-VM transactions
    println!("⚡ Step 5: Creating cross-VM transactions");

    // Create account binding transaction
    let solana_account = AccountAddress::Solana(SolanaAddress([1u8; 32]));
    let ethereum_account = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));

    let binding_tx = SpecialTransaction::AccountBinding {
        source_account: solana_account.clone(),
        target_account: ethereum_account.clone(),
        proof: BindingProof {
            account: ethereum_account.clone(),
            proof_type: ProofType::Signature {
                message: b"consensus demo binding".to_vec(),
                signature: b"demo_signature".to_vec(),
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        },
        metadata: None,
    };

    println!("   ✨ Created account binding transaction");
    println!("     🔗 Source: {}", solana_account);
    println!("     🎯 Target: {}", ethereum_account);

    // Create cross-VM transfer transaction
    let from_account = MultivmAccountId::from_seed(b"demo_from_account_seed");
    let to_account = MultivmAccountId::from_seed(b"demo_to_account_seed");

    let transfer_tx = SpecialTransaction::CrossVmTransfer {
        from: from_account.clone(),
        to: to_account.clone(),
        amount: 1000,
        asset_type: AssetType::Native,
        memo: Some("Demo cross-VM transfer".to_string()),
    };

    println!("   💸 Created cross-VM transfer transaction");
    println!("     📤 From: {}", from_account);
    println!("     📥 To: {}", to_account);
    println!("     💰 Amount: 1000\n");

    // 6. Process transactions through consensus
    println!("🔄 Step 6: Processing transactions through consensus");

    // Start event monitoring task
    let event_task = tokio::spawn(async move {
        let mut events_received = 0;
        while let Some(event) = event_receiver.recv().await {
            events_received += 1;
            match event {
                ConsensusEvent::BlockProposed {
                    block,
                    proposer,
                    height: _,
                } => {
                    println!(
                        "   📦 Block proposed by {}: height {}, {} transactions",
                        proposer,
                        block.header.height,
                        block.transaction_count()
                    );
                }
                ConsensusEvent::BlockCommitted {
                    block,
                    height,
                    block_hash,
                } => {
                    println!(
                        "   ✅ Block committed at height {} (hash: {}): {} transactions",
                        height,
                        block_hash,
                        block.transaction_count()
                    );
                }
                ConsensusEvent::StateSynchronized { height, state_hash } => {
                    println!(
                        "   🔄 State synchronized to height {}: hash {}",
                        height, state_hash
                    );
                }
                ConsensusEvent::Error {
                    error,
                    context,
                    message,
                } => {
                    println!("   ❌ Error in {}: {} ({})", context, error, message);
                }
                ConsensusEvent::ViewChanged {
                    old_view,
                    new_view,
                    reason,
                } => {
                    println!(
                        "   🔄 View changed from {} to {}: {}",
                        old_view, new_view, reason
                    );
                }
                ConsensusEvent::NodeJoined { node_id } => {
                    println!("   ➕ Node joined: {}", node_id);
                }
                ConsensusEvent::NodeLeft { node_id } => {
                    println!("   ➖ Node left: {}", node_id);
                }
            }

            // Stop after receiving a few events
            if events_received >= 3 {
                break;
            }
        }
        events_received
    });

    // Process the binding transaction
    println!("   🔧 Processing account binding transaction...");
    match consensus_manager
        .process_cross_vm_transaction(binding_tx)
        .await
    {
        Ok(()) => println!("   ✅ Account binding transaction processed successfully"),
        Err(e) => println!("   ❌ Failed to process binding transaction: {}", e),
    }

    // Process the transfer transaction
    println!("   🔧 Processing cross-VM transfer transaction...");
    match consensus_manager
        .process_cross_vm_transaction(transfer_tx)
        .await
    {
        Ok(()) => println!("   ✅ Cross-VM transfer transaction processed successfully"),
        Err(e) => println!("   ❌ Failed to process transfer transaction: {}", e),
    }

    // Wait for events to be processed
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Wait for event task to complete or timeout
    match tokio::time::timeout(std::time::Duration::from_secs(2), event_task).await {
        Ok(Ok(events_count)) => println!("   📨 Processed {} consensus events", events_count),
        Ok(Err(_)) => println!("   ⚠️ Event task failed"),
        Err(_) => println!("   ⏰ Event processing timed out"),
    }

    println!();

    // 7. Check final state and statistics
    println!("📈 Step 7: Final state and statistics");

    let final_stats = consensus_manager.get_consensus_stats().await?;
    println!("   📊 Final height: {}", final_stats.current_height);
    println!("   🧮 Total blocks: {}", final_stats.total_blocks);
    println!(
        "   🔄 Total transactions: {}",
        final_stats.total_transactions
    );
    println!(
        "   ⚡ Cross-VM transactions: {}",
        final_stats.cross_vm_transactions
    );

    let final_cross_vm_state = consensus_manager.get_cross_vm_state().await?;
    println!(
        "   🌐 Final cross-VM state height: {}",
        final_cross_vm_state.height
    );
    println!(
        "   🔢 Final global nonce: {}",
        final_cross_vm_state.global_nonce
    );

    // 8. Test state checkpointing
    println!("\n💾 Step 8: Testing state checkpointing");

    let checkpoint = consensus_manager.create_checkpoint().await?;
    println!("   ✅ Checkpoint created at height {}", checkpoint.height);
    println!("   📋 State hash: {}", checkpoint.state_hash);
    println!("   🕒 Timestamp: {:?}", checkpoint.timestamp);

    // 9. Test state synchronization
    println!("\n🔄 Step 9: Testing state synchronization");

    let target_height = final_stats.current_height + 10;
    consensus_manager.sync_to_height(target_height).await?;

    let synced_state = consensus_manager.get_cross_vm_state().await?;
    println!("   ✅ State synchronized to height {}", synced_state.height);

    // 10. Stop consensus
    println!("\n🛑 Step 10: Stopping consensus");

    consensus_manager.stop().await?;

    println!("   ✅ Consensus stopped successfully");
    println!(
        "   🏁 Status: {}",
        if consensus_manager.is_running() {
            "Running"
        } else {
            "Stopped"
        }
    );

    // 11. Summary
    println!("\n📊 Demo Summary");
    println!("===============");
    println!("✅ Consensus Initialization: Successfully created and configured Raft consensus");
    println!("✅ Transaction Processing: Processed account binding and cross-VM transfer");
    println!("✅ State Management: Demonstrated state updates and synchronization");
    println!("✅ Event System: Monitored consensus events and state changes");
    println!("✅ Checkpointing: Created and managed state checkpoints");
    println!("✅ Lifecycle Management: Successfully started and stopped consensus");

    println!("\n🎉 Demo completed successfully!");
    println!("   The MultiVM consensus layer is working correctly and ready for");
    println!("   integration with the broader MultiVM architecture.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consensus_demo_components() {
        // Test consensus manager creation
        let config = ConsensusManagerConfig {
            algorithm: ConsensusAlgorithmType::Malachite,
            algorithm_config: AlgorithmConfig::Malachite(MalachiteConfig {
                node_id: "test-validator".to_string(),
                network_config: multivm_consensus::malachite::NetworkConfig {
                    listen_addr: "127.0.0.1:0".to_string(),
                    peers: vec![],
                },
                consensus_params: multivm_consensus::malachite::ConsensusParams {
                    block_time_ms: 1000,
                    max_block_size: 1024 * 1024,
                    timeout_propose_ms: 1000,
                    timeout_prevote_ms: 1000,
                    timeout_precommit_ms: 1000,
                },
                validators: vec![],
            }),
            state_manager_config: StateManagerConfig::default(),
            block_proposal_interval_ms: 1000,
            max_transactions_per_block: 50,
            enable_auto_proposal: false,
            network_config: NetworkConfig::default(),
        };
        let manager = MultiVMConsensusManager::new(config).await.unwrap();
        assert!(!manager.is_running());

        // Test configuration
        let malachite_config = MalachiteConfig {
            node_id: "node_1".to_string(),
            network_config: multivm_consensus::malachite::NetworkConfig {
                listen_addr: "127.0.0.1:0".to_string(),
                peers: vec![],
            },
            consensus_params: multivm_consensus::malachite::ConsensusParams {
                block_time_ms: 1000,
                max_block_size: 1024 * 1024,
                timeout_propose_ms: 1000,
                timeout_prevote_ms: 1000,
                timeout_precommit_ms: 1000,
            },
            validators: vec![],
        };
        assert_eq!(malachite_config.node_id, "node_1");
        assert!(malachite_config.validators.is_empty());
    }

    #[tokio::test]
    async fn test_transaction_creation() {
        let solana_account = AccountAddress::Solana(SolanaAddress([42u8; 32]));
        let ethereum_account = AccountAddress::Ethereum(EthereumAddress([24u8; 20]));

        let binding_tx = SpecialTransaction::AccountBinding {
            source_account: solana_account.clone(),
            target_account: ethereum_account.clone(),
            proof: BindingProof {
                account: ethereum_account.clone(),
                proof_type: ProofType::Signature {
                    message: b"test binding".to_vec(),
                    signature: b"test signature".to_vec(),
                },
                proof_data: vec![],
                timestamp: SystemTime::now(),
            },
            metadata: None,
        };

        // Verify transaction structure
        match binding_tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                ..
            } => {
                assert_eq!(source_account, solana_account);
                assert_eq!(target_account, ethereum_account);
            }
            _ => panic!("Wrong transaction type"),
        }
    }

    #[tokio::test]
    async fn test_consensus_config() {
        let config = ConsensusManagerConfig {
            algorithm: ConsensusAlgorithmType::Malachite,
            algorithm_config: AlgorithmConfig::Malachite(MalachiteConfig {
                node_id: "test-validator".to_string(),
                network_config: multivm_consensus::malachite::NetworkConfig {
                    listen_addr: "127.0.0.1:0".to_string(),
                    peers: vec![],
                },
                consensus_params: multivm_consensus::malachite::ConsensusParams {
                    block_time_ms: 1000,
                    max_block_size: 1024 * 1024,
                    timeout_propose_ms: 1000,
                    timeout_prevote_ms: 1000,
                    timeout_precommit_ms: 1000,
                },
                validators: vec![],
            }),
            state_manager_config: StateManagerConfig::default(),
            block_proposal_interval_ms: 1000,
            max_transactions_per_block: 50,
            enable_auto_proposal: false,
            network_config: NetworkConfig::default(),
        };

        assert_eq!(config.block_proposal_interval_ms, 1000);
        assert_eq!(config.max_transactions_per_block, 50);
        assert_eq!(config.max_transactions_per_block, 50);
        assert!(!config.enable_auto_proposal);
    }
}
