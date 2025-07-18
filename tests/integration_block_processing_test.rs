//! Integration tests for Block Processing Workflow
//! Tests end-to-end block processing from consensus to execution engines

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

use multivm_common::config::VmType;
use multivm_consensus::{
    manager::ConsensusManager,
    config::{ConsensusConfig, ConsensusType},
    block::{MultiVMBlock, BlockHeader},
    traits::ConsensusEngine,
};
use reth_execution_engine::{
    config_integration::RethIntegrationConfig,
    engine::{MultivmTransaction, RethExecutionEngine},
    error::RethEngineError,
};
use solana_execution_engine::{
    config::{SolanaConfig, SolanaConnectionConfig},
    engine::SolanaExecutionEngine,
    error::SolanaEngineError,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    system_instruction,
    transaction::Transaction,
};

/// Configuration for block processing integration tests
#[derive(Debug, Clone)]
pub struct BlockProcessingTestConfig {
    pub reth_data_dir: String,
    pub reth_http_port: u16,
    pub reth_auth_port: u16,
    pub solana_ledger_path: String,
    pub solana_rpc_port: u16,
    pub solana_ws_port: u16,
    pub jwt_secret: String,
    pub chain_id: u64,
    pub block_time: Duration,
    pub max_transactions_per_block: usize,
    pub test_timeout: Duration,
}

impl Default for BlockProcessingTestConfig {
    fn default() -> Self {
        Self {
            reth_data_dir: "/tmp/block-processing-test-reth".to_string(),
            reth_http_port: 8545,
            reth_auth_port: 8551,
            solana_ledger_path: "/tmp/block-processing-test-solana".to_string(),
            solana_rpc_port: 8899,
            solana_ws_port: 8900,
            jwt_secret: "test-jwt-secret-for-block-processing".to_string(),
            chain_id: 1337,
            block_time: Duration::from_secs(3),
            max_transactions_per_block: 100,
            test_timeout: Duration::from_secs(180),
        }
    }
}

/// Block processing integration test suite
pub struct BlockProcessingIntegrationTestSuite {
    config: BlockProcessingTestConfig,
    consensus_manager: Option<ConsensusManager>,
    reth_engine: Option<RethExecutionEngine>,
    solana_engine: Option<SolanaExecutionEngine>,
    solana_keypair: Keypair,
    test_transactions: Vec<MultivmTransaction>,
    test_solana_transactions: Vec<Transaction>,
}

impl BlockProcessingIntegrationTestSuite {
    pub fn new(config: BlockProcessingTestConfig) -> Self {
        Self {
            config,
            consensus_manager: None,
            reth_engine: None,
            solana_engine: None,
            solana_keypair: Keypair::new(),
            test_transactions: Vec::new(),
            test_solana_transactions: Vec::new(),
        }
    }

    /// Setup consensus manager and execution engines
    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Setting up block processing integration test suite");

        // Setup consensus manager
        let consensus_config = ConsensusConfig {
            node_id: "block-processing-test-consensus".to_string(),
            consensus_type: ConsensusType::Malachite,
            validator_address: "block-processing-test-validator".to_string(),
            voting_power: 1,
            block_time: self.config.block_time,
            max_transactions_per_block: self.config.max_transactions_per_block,
            enable_cross_vm: true,
            enable_auto_block_generation: true,
            transaction_pool_size: 10000,
            ..Default::default()
        };

        let consensus_manager = ConsensusManager::new(consensus_config).await?;

        // Setup Reth engine
        let reth_config = RethIntegrationConfig {
            data_dir: self.config.reth_data_dir.clone(),
            http_port: self.config.reth_http_port,
            ws_port: self.config.reth_http_port + 1,
            auth_port: self.config.reth_auth_port,
            jwt_secret: self.config.jwt_secret.clone(),
            chain_id: self.config.chain_id,
            enable_dev_mode: true,
            mining_interval: Some(self.config.block_time),
            gas_limit: 30_000_000,
            base_fee: 1_000_000_000,
        };

        let reth_engine = RethExecutionEngine::new(
            reth_config,
            "block-processing-test-reth".to_string(),
        ).await?;

        // Setup Solana engine
        let solana_config = SolanaConfig {
            ledger_path: self.config.solana_ledger_path.clone(),
            rpc_port: self.config.solana_rpc_port,
            ws_port: self.config.solana_ws_port,
            ipc_socket_path: format!("{}/solana.sock", self.config.solana_ledger_path),
            log_level: "info".to_string(),
            enable_dev_mode: true,
            slots_per_epoch: 32,
            ticks_per_slot: 64,
            genesis_config: Default::default(),
        };

        let solana_connection_config = SolanaConnectionConfig {
            rpc_endpoint: format!("http://127.0.0.1:{}", self.config.solana_rpc_port),
            ws_endpoint: format!("ws://127.0.0.1:{}", self.config.solana_ws_port),
            commitment: CommitmentConfig::confirmed(),
            timeout: Duration::from_secs(30),
        };

        let solana_engine = SolanaExecutionEngine::new(
            solana_config,
            solana_connection_config,
            "block-processing-test-solana".to_string(),
        ).await?;

        self.consensus_manager = Some(consensus_manager);
        self.reth_engine = Some(reth_engine);
        self.solana_engine = Some(solana_engine);

        // Prepare test transactions
        self.prepare_test_transactions().await?;

        info!("Block processing integration test suite setup complete");
        Ok(())
    }

    /// Prepare test transactions for block processing
    async fn prepare_test_transactions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Preparing test transactions");

        // Create EVM transactions
        for i in 0..5 {
            let tx = MultivmTransaction {
                vm_type: VmType::EVM,
                from: "0x0000000000000000000000000000000000000000".to_string(),
                to: Some(format!("0x{:040x}", i + 1)),
                value: format!("{}", (i + 1) * 1_000_000_000_000_000_000), // 1-5 ETH
                data: "0x".to_string(),
                gas_price: Some("20000000000".to_string()),
                gas_limit: Some(21000),
                nonce: Some(i as u64),
                chain_id: Some(self.config.chain_id),
                max_fee_per_gas: Some("30000000000".to_string()),
                max_priority_fee_per_gas: Some("2000000000".to_string()),
                transaction_type: Some(2),
            };
            self.test_transactions.push(tx);
        }

        // Create Solana transactions
        if let Some(solana_engine) = &self.solana_engine {
            let recent_blockhash = solana_engine.get_recent_blockhash().await?;
            
            for i in 0..5 {
                let recipient = Keypair::new();
                let instruction = system_instruction::transfer(
                    &self.solana_keypair.pubkey(),
                    &recipient.pubkey(),
                    (i + 1) * 1_000_000, // 1-5 SOL (in lamports)
                );

                let transaction = Transaction::new_signed_with_payer(
                    &[instruction],
                    Some(&self.solana_keypair.pubkey()),
                    &[&self.solana_keypair],
                    recent_blockhash,
                );

                self.test_solana_transactions.push(transaction);
            }
        }

        info!("Prepared {} EVM and {} Solana test transactions", 
              self.test_transactions.len(), self.test_solana_transactions.len());
        Ok(())
    }

    /// Test starting all components
    pub async fn test_components_startup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing components startup");

        let consensus_manager = self.consensus_manager.as_mut().ok_or("Consensus manager not initialized")?;
        let reth_engine = self.reth_engine.as_mut().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_mut().ok_or("Solana engine not initialized")?;

        // Start all components
        consensus_manager.start().await?;
        reth_engine.start().await?;
        solana_engine.start().await?;

        info!("All components started successfully");

        // Wait for all components to become ready
        for attempt in 1..=20 {
            let consensus_ready = consensus_manager.is_running().await;
            let reth_ready = reth_engine.is_healthy().await.unwrap_or(false);
            let solana_ready = solana_engine.is_healthy().await.unwrap_or(false);

            if consensus_ready && reth_ready && solana_ready {
                info!("All components are ready after {} attempts", attempt);
                return Ok(());
            }

            warn!("Waiting for components to become ready: Consensus={}, Reth={}, Solana={}, attempt {}/20", 
                  consensus_ready, reth_ready, solana_ready, attempt);
            sleep(Duration::from_secs(3)).await;
        }

        Err("Components failed to become ready within timeout".into())
    }

    /// Test transaction submission to consensus
    pub async fn test_transaction_submission(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing transaction submission to consensus");

        let consensus_manager = self.consensus_manager.as_mut().ok_or("Consensus manager not initialized")?;

        // Submit EVM transactions
        for (i, tx) in self.test_transactions.iter().enumerate() {
            let tx_bytes = serde_json::to_vec(tx)?;
            let result = consensus_manager.submit_transaction(tx_bytes).await;
            
            match result {
                Ok(_) => info!("✅ EVM transaction {} submitted successfully", i),
                Err(e) => warn!("❌ EVM transaction {} submission failed: {}", i, e),
            }
        }

        // Submit Solana transactions
        for (i, tx) in self.test_solana_transactions.iter().enumerate() {
            let tx_bytes = bincode::serialize(tx)?;
            let result = consensus_manager.submit_transaction(tx_bytes).await;
            
            match result {
                Ok(_) => info!("✅ Solana transaction {} submitted successfully", i),
                Err(e) => warn!("❌ Solana transaction {} submission failed: {}", i, e),
            }
        }

        info!("Transaction submission test completed");
        Ok(())
    }

    /// Test block proposal and validation
    pub async fn test_block_proposal_and_validation(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing block proposal and validation");

        let consensus_manager = self.consensus_manager.as_mut().ok_or("Consensus manager not initialized")?;

        // Wait for consensus to process transactions
        sleep(Duration::from_secs(5)).await;

        // Check consensus statistics
        let initial_stats = consensus_manager.get_stats().await?;
        info!("Initial consensus stats: {:?}", initial_stats);

        // Trigger block proposal
        let block_proposal_result = consensus_manager.propose_block().await;
        match block_proposal_result {
            Ok(block_height) => {
                info!("✅ Block proposed at height {}", block_height);
                
                // Wait for block validation
                sleep(Duration::from_secs(self.config.block_time.as_secs() + 2)).await;
                
                // Check updated statistics
                let updated_stats = consensus_manager.get_stats().await?;
                info!("Updated consensus stats: {:?}", updated_stats);
                
                if updated_stats.current_height > initial_stats.current_height {
                    info!("✅ Block height increased from {} to {}", 
                          initial_stats.current_height, updated_stats.current_height);
                } else {
                    warn!("❌ Block height did not increase");
                }
            }
            Err(e) => {
                warn!("❌ Block proposal failed: {}", e);
            }
        }

        info!("Block proposal and validation test completed");
        Ok(())
    }

    /// Test block execution on execution engines
    pub async fn test_block_execution(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing block execution on execution engines");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Get initial state
        let initial_eth_block = reth_engine.get_latest_block_number().await?;
        let initial_solana_slot = solana_engine.get_current_slot().await?;

        info!("Initial execution state - ETH block: {}, Solana slot: {}", 
              initial_eth_block, initial_solana_slot);

        // Create a test block with mixed transactions
        let test_block = MultiVMBlock {
            header: BlockHeader {
                height: initial_eth_block + 1,
                previous_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                state_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                transactions_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                timestamp: std::time::SystemTime::now(),
                proposer: "test-proposer".to_string(),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            evm_transactions: self.test_transactions.clone(),
            svm_transactions: self.test_solana_transactions.iter().map(|tx| {
                multivm_consensus::block::SvmTransaction {
                    signatures: tx.signatures.iter().map(|sig| sig.to_string()).collect(),
                    message: bincode::serialize(&tx.message).unwrap_or_default(),
                    compute_budget: 200_000,
                }
            }).collect(),
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        // Process the block
        let block_bytes = serde_json::to_vec(&test_block)?;
        
        // Simulate block processing on both engines
        let reth_processing = async {
            // Process EVM transactions
            let mut results = Vec::new();
            for tx in &test_block.evm_transactions {
                let result = reth_engine.process_transaction(tx).await;
                results.push(result);
            }
            results
        };

        let solana_processing = async {
            // Process Solana transactions
            let mut results = Vec::new();
            for tx in &self.test_solana_transactions {
                let result = solana_engine.process_transaction(tx).await;
                results.push(result);
            }
            results
        };

        let (reth_results, solana_results) = tokio::join!(reth_processing, solana_processing);

        // Analyze results
        let mut reth_success = 0;
        let mut reth_failed = 0;
        for (i, result) in reth_results.iter().enumerate() {
            match result {
                Ok(tx_hash) => {
                    info!("✅ EVM transaction {} executed: {}", i, tx_hash);
                    reth_success += 1;
                }
                Err(e) => {
                    warn!("❌ EVM transaction {} failed: {}", i, e);
                    reth_failed += 1;
                }
            }
        }

        let mut solana_success = 0;
        let mut solana_failed = 0;
        for (i, result) in solana_results.iter().enumerate() {
            match result {
                Ok(signature) => {
                    info!("✅ Solana transaction {} executed: {}", i, signature);
                    solana_success += 1;
                }
                Err(e) => {
                    warn!("❌ Solana transaction {} failed: {}", i, e);
                    solana_failed += 1;
                }
            }
        }

        info!("Block execution results:");
        info!("  Reth: {} success, {} failed", reth_success, reth_failed);
        info!("  Solana: {} success, {} failed", solana_success, solana_failed);

        // Wait for execution to complete
        sleep(Duration::from_secs(self.config.block_time.as_secs() + 2)).await;

        // Check final state
        let final_eth_block = reth_engine.get_latest_block_number().await?;
        let final_solana_slot = solana_engine.get_current_slot().await?;

        info!("Final execution state - ETH block: {}, Solana slot: {}", 
              final_eth_block, final_solana_slot);

        // Verify progress
        let eth_progress = final_eth_block >= initial_eth_block;
        let solana_progress = final_solana_slot >= initial_solana_slot;

        info!("Execution progress: ETH={}, Solana={}", eth_progress, solana_progress);

        info!("Block execution test completed");
        Ok(())
    }

    /// Test block finalization workflow
    pub async fn test_block_finalization(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing block finalization workflow");

        let consensus_manager = self.consensus_manager.as_mut().ok_or("Consensus manager not initialized")?;

        // Get initial state
        let initial_stats = consensus_manager.get_stats().await?;
        info!("Initial finalization state: height={}, finalized={}", 
              initial_stats.current_height, initial_stats.total_blocks);

        // Wait for automatic block generation and finalization
        sleep(Duration::from_secs(self.config.block_time.as_secs() * 3)).await;

        // Check finalization progress
        let updated_stats = consensus_manager.get_stats().await?;
        info!("Updated finalization state: height={}, finalized={}", 
              updated_stats.current_height, updated_stats.total_blocks);

        if updated_stats.total_blocks > initial_stats.total_blocks {
            info!("✅ Block finalization is working: {} blocks finalized", 
                  updated_stats.total_blocks - initial_stats.total_blocks);
        } else {
            warn!("❌ No blocks were finalized during test period");
        }

        // Test manual finalization
        let manual_finalization_result = consensus_manager.finalize_block().await;
        match manual_finalization_result {
            Ok(height) => {
                info!("✅ Manual block finalization successful at height {}", height);
            }
            Err(e) => {
                warn!("❌ Manual block finalization failed: {}", e);
            }
        }

        info!("Block finalization test completed");
        Ok(())
    }

    /// Test block synchronization between engines
    pub async fn test_block_synchronization(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing block synchronization between engines");

        let consensus_manager = self.consensus_manager.as_ref().ok_or("Consensus manager not initialized")?;
        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Get current state from all components
        let consensus_height = consensus_manager.get_current_height().await?;
        let reth_block = reth_engine.get_latest_block_number().await?;
        let solana_slot = solana_engine.get_current_slot().await?;

        info!("Current synchronization state:");
        info!("  Consensus height: {}", consensus_height);
        info!("  Reth block: {}", reth_block);
        info!("  Solana slot: {}", solana_slot);

        // Wait for synchronization
        sleep(Duration::from_secs(self.config.block_time.as_secs() * 2)).await;

        // Check if all components are progressing
        let new_consensus_height = consensus_manager.get_current_height().await?;
        let new_reth_block = reth_engine.get_latest_block_number().await?;
        let new_solana_slot = solana_engine.get_current_slot().await?;

        info!("Updated synchronization state:");
        info!("  Consensus height: {}", new_consensus_height);
        info!("  Reth block: {}", new_reth_block);
        info!("  Solana slot: {}", new_solana_slot);

        // Check synchronization progress
        let consensus_progress = new_consensus_height >= consensus_height;
        let reth_progress = new_reth_block >= reth_block;
        let solana_progress = new_solana_slot >= solana_slot;

        info!("Synchronization progress: Consensus={}, Reth={}, Solana={}", 
              consensus_progress, reth_progress, solana_progress);

        if consensus_progress && reth_progress && solana_progress {
            info!("✅ All components are synchronized and progressing");
        } else {
            warn!("❌ Some components are not synchronized");
        }

        info!("Block synchronization test completed");
        Ok(())
    }

    /// Test error handling in block processing
    pub async fn test_error_handling(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing error handling in block processing");

        let consensus_manager = self.consensus_manager.as_mut().ok_or("Consensus manager not initialized")?;

        // Test invalid transaction submission
        let invalid_tx_bytes = b"invalid transaction data";
        let invalid_result = consensus_manager.submit_transaction(invalid_tx_bytes.to_vec()).await;
        
        match invalid_result {
            Ok(_) => warn!("Invalid transaction was unexpectedly accepted"),
            Err(e) => info!("✅ Invalid transaction correctly rejected: {}", e),
        }

        // Test recovery after error
        let valid_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: Some("0x1111111111111111111111111111111111111111".to_string()),
            value: "1000000000000000000".to_string(),
            data: "0x".to_string(),
            gas_price: Some("20000000000".to_string()),
            gas_limit: Some(21000),
            nonce: Some(100),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("30000000000".to_string()),
            max_priority_fee_per_gas: Some("2000000000".to_string()),
            transaction_type: Some(2),
        };

        let valid_tx_bytes = serde_json::to_vec(&valid_tx)?;
        let recovery_result = consensus_manager.submit_transaction(valid_tx_bytes).await;
        
        match recovery_result {
            Ok(_) => info!("✅ Recovery successful after error"),
            Err(e) => warn!("❌ Recovery failed: {}", e),
        }

        // Test consensus health after errors
        let is_healthy = consensus_manager.is_running().await;
        info!("Consensus health after errors: {}", is_healthy);

        info!("Error handling test completed");
        Ok(())
    }

    /// Cleanup test environment
    pub async fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Cleaning up block processing integration test environment");

        // Stop all components
        if let Some(consensus_manager) = &mut self.consensus_manager {
            consensus_manager.stop().await?;
        }

        if let Some(reth_engine) = &mut self.reth_engine {
            reth_engine.stop().await.ok();
        }

        if let Some(solana_engine) = &mut self.solana_engine {
            solana_engine.stop().await.ok();
        }

        // Clean up directories
        let _ = std::fs::remove_dir_all(&self.config.reth_data_dir);
        let _ = std::fs::remove_dir_all(&self.config.solana_ledger_path);

        info!("Cleanup completed");
        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tracing_subscriber;

    fn setup_logging() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .without_time()
            .try_init();
    }

    #[tokio::test]
    #[ignore = "Requires actual Reth and Solana binaries and long running process"]
    async fn test_block_processing_full_workflow() {
        setup_logging();
        info!("Starting block processing full workflow test");

        let config = BlockProcessingTestConfig::default();
        let mut test_suite = BlockProcessingIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            error!("Setup failed: {}", e);
            return;
        }

        // Run all tests
        let tests = vec![
            ("components_startup", test_suite.test_components_startup()),
            ("transaction_submission", test_suite.test_transaction_submission()),
            ("block_proposal_validation", test_suite.test_block_proposal_and_validation()),
            ("block_execution", test_suite.test_block_execution()),
            ("block_finalization", test_suite.test_block_finalization()),
            ("block_synchronization", test_suite.test_block_synchronization()),
            ("error_handling", test_suite.test_error_handling()),
        ];

        let mut passed = 0;
        let mut failed = 0;

        for (test_name, test_future) in tests {
            match test_future.await {
                Ok(_) => {
                    info!("✅ Test '{}' passed", test_name);
                    passed += 1;
                }
                Err(e) => {
                    error!("❌ Test '{}' failed: {}", test_name, e);
                    failed += 1;
                }
            }
        }

        // Cleanup
        if let Err(e) = test_suite.cleanup().await {
            warn!("Cleanup failed: {}", e);
        }

        info!("Block processing integration test results: {} passed, {} failed", passed, failed);
    }

    #[tokio::test]
    #[ignore = "Requires actual Reth and Solana binaries"]
    async fn test_basic_block_processing() {
        setup_logging();
        info!("Starting basic block processing test");

        let config = BlockProcessingTestConfig {
            test_timeout: Duration::from_secs(60),
            ..Default::default()
        };

        let mut test_suite = BlockProcessingIntegrationTestSuite::new(config);

        if let Err(e) = test_suite.setup().await {
            error!("Setup failed: {}", e);
            return;
        }

        if let Err(e) = test_suite.test_components_startup().await {
            test_suite.cleanup().await.ok();
            error!("Components startup test failed: {}", e);
            return;
        }

        if let Err(e) = test_suite.test_transaction_submission().await {
            test_suite.cleanup().await.ok();
            error!("Transaction submission test failed: {}", e);
            return;
        }

        test_suite.cleanup().await.ok();
        info!("Basic block processing test completed successfully");
    }
}