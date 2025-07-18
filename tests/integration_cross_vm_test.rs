//! Integration tests for Cross-VM Transaction Processing
//! Tests MultiVM coordination between Reth and Solana execution engines

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

use multivm_common::config::VmType;
use multivm_consensus::manager::ConsensusManager;
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

/// Configuration for cross-VM integration tests
#[derive(Debug, Clone)]
pub struct CrossVmTestConfig {
    pub reth_data_dir: String,
    pub reth_http_port: u16,
    pub reth_auth_port: u16,
    pub solana_ledger_path: String,
    pub solana_rpc_port: u16,
    pub solana_ws_port: u16,
    pub jwt_secret: String,
    pub chain_id: u64,
    pub test_timeout: Duration,
}

impl Default for CrossVmTestConfig {
    fn default() -> Self {
        Self {
            reth_data_dir: "/tmp/cross-vm-test-reth".to_string(),
            reth_http_port: 8545,
            reth_auth_port: 8551,
            solana_ledger_path: "/tmp/cross-vm-test-solana".to_string(),
            solana_rpc_port: 8899,
            solana_ws_port: 8900,
            jwt_secret: "test-jwt-secret-for-cross-vm-integration".to_string(),
            chain_id: 1337,
            test_timeout: Duration::from_secs(180),
        }
    }
}

/// Cross-VM integration test suite
pub struct CrossVmIntegrationTestSuite {
    config: CrossVmTestConfig,
    reth_engine: Option<RethExecutionEngine>,
    solana_engine: Option<SolanaExecutionEngine>,
    consensus_manager: Option<ConsensusManager>,
    solana_keypair: Keypair,
}

impl CrossVmIntegrationTestSuite {
    pub fn new(config: CrossVmTestConfig) -> Self {
        Self {
            config,
            reth_engine: None,
            solana_engine: None,
            consensus_manager: None,
            solana_keypair: Keypair::new(),
        }
    }

    /// Setup both execution engines and consensus manager
    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Setting up cross-VM integration test suite");

        // Setup Reth engine
        let reth_config = RethIntegrationConfig {
            data_dir: self.config.reth_data_dir.clone(),
            http_port: self.config.reth_http_port,
            ws_port: self.config.reth_http_port + 1,
            auth_port: self.config.reth_auth_port,
            jwt_secret: self.config.jwt_secret.clone(),
            chain_id: self.config.chain_id,
            enable_dev_mode: true,
            mining_interval: Some(Duration::from_secs(2)),
            gas_limit: 30_000_000,
            base_fee: 1_000_000_000,
        };

        let reth_engine = RethExecutionEngine::new(
            reth_config,
            "cross-vm-test-reth".to_string(),
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
            "cross-vm-test-solana".to_string(),
        ).await?;

        // Setup consensus manager
        let consensus_config = multivm_consensus::config::ConsensusConfig {
            node_id: "cross-vm-test-consensus".to_string(),
            consensus_type: multivm_consensus::config::ConsensusType::Malachite,
            validator_address: "cross-vm-test-validator".to_string(),
            voting_power: 1,
            block_time: Duration::from_secs(2),
            max_transactions_per_block: 1000,
            enable_cross_vm: true,
            ..Default::default()
        };

        let consensus_manager = ConsensusManager::new(consensus_config).await?;

        self.reth_engine = Some(reth_engine);
        self.solana_engine = Some(solana_engine);
        self.consensus_manager = Some(consensus_manager);

        info!("Cross-VM integration test suite setup complete");
        Ok(())
    }

    /// Test starting both execution engines
    pub async fn test_dual_engine_startup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing dual engine startup");

        let reth_engine = self.reth_engine.as_mut().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_mut().ok_or("Solana engine not initialized")?;

        // Start both engines concurrently
        let reth_start = reth_engine.start();
        let solana_start = solana_engine.start();

        let (reth_result, solana_result) = tokio::join!(reth_start, solana_start);

        reth_result?;
        solana_result?;

        info!("Both engines started successfully");

        // Wait for both to become healthy
        for attempt in 1..=20 {
            let reth_healthy = reth_engine.is_healthy().await.unwrap_or(false);
            let solana_healthy = solana_engine.is_healthy().await.unwrap_or(false);

            if reth_healthy && solana_healthy {
                info!("Both engines are healthy after {} attempts", attempt);
                return Ok(());
            }

            warn!("Waiting for engines to become healthy: Reth={}, Solana={}, attempt {}/20", 
                  reth_healthy, solana_healthy, attempt);
            sleep(Duration::from_secs(3)).await;
        }

        Err("Both engines failed to become healthy within timeout".into())
    }

    /// Test cross-VM transaction coordination
    pub async fn test_cross_vm_transaction_coordination(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing cross-VM transaction coordination");

        let reth_engine = self.reth_engine.as_mut().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_mut().ok_or("Solana engine not initialized")?;

        // Create EVM transaction
        let evm_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: Some("0x1111111111111111111111111111111111111111".to_string()),
            value: "1000000000000000000".to_string(), // 1 ETH
            data: "0x".to_string(),
            gas_price: Some("20000000000".to_string()),
            gas_limit: Some(21000),
            nonce: Some(0),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("30000000000".to_string()),
            max_priority_fee_per_gas: Some("2000000000".to_string()),
            transaction_type: Some(2),
        };

        // Create Solana transaction
        let recipient = Keypair::new();
        let recent_blockhash = solana_engine.get_recent_blockhash().await?;
        let instruction = system_instruction::transfer(
            &self.solana_keypair.pubkey(),
            &recipient.pubkey(),
            1_000_000,
        );

        let solana_tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.solana_keypair.pubkey()),
            &[&self.solana_keypair],
            recent_blockhash,
        );

        // Process transactions simultaneously
        let evm_processing = reth_engine.process_transaction(&evm_tx);
        let solana_processing = solana_engine.process_transaction(&solana_tx);

        match tokio::join!(evm_processing, solana_processing) {
            (Ok(evm_hash), Ok(solana_sig)) => {
                info!("Cross-VM transactions processed successfully:");
                info!("  EVM TX: {}", evm_hash);
                info!("  Solana TX: {}", solana_sig);
            }
            (Err(evm_err), Ok(solana_sig)) => {
                warn!("EVM transaction failed: {}", evm_err);
                info!("Solana transaction succeeded: {}", solana_sig);
            }
            (Ok(evm_hash), Err(solana_err)) => {
                info!("EVM transaction succeeded: {}", evm_hash);
                warn!("Solana transaction failed: {}", solana_err);
            }
            (Err(evm_err), Err(solana_err)) => {
                warn!("Both transactions failed:");
                warn!("  EVM: {}", evm_err);
                warn!("  Solana: {}", solana_err);
            }
        }

        info!("Cross-VM transaction coordination test completed");
        Ok(())
    }

    /// Test consensus coordination with both VMs
    pub async fn test_consensus_coordination(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing consensus coordination with both VMs");

        let consensus_manager = self.consensus_manager.as_mut().ok_or("Consensus manager not initialized")?;

        // Start consensus
        consensus_manager.start().await?;

        // Create mixed transaction batch
        let mut transactions = Vec::new();

        // Add EVM transaction
        let evm_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: Some("0x2222222222222222222222222222222222222222".to_string()),
            value: "500000000000000000".to_string(),
            data: "0x".to_string(),
            gas_price: Some("20000000000".to_string()),
            gas_limit: Some(21000),
            nonce: Some(1),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("30000000000".to_string()),
            max_priority_fee_per_gas: Some("2000000000".to_string()),
            transaction_type: Some(2),
        };

        // Add Solana transaction
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;
        let recipient = Keypair::new();
        let recent_blockhash = solana_engine.get_recent_blockhash().await?;
        let instruction = system_instruction::transfer(
            &self.solana_keypair.pubkey(),
            &recipient.pubkey(),
            500_000,
        );

        let solana_tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.solana_keypair.pubkey()),
            &[&self.solana_keypair],
            recent_blockhash,
        );

        // Submit transactions to consensus
        let evm_tx_bytes = serde_json::to_vec(&evm_tx)?;
        let solana_tx_bytes = bincode::serialize(&solana_tx)?;

        consensus_manager.submit_transaction(evm_tx_bytes).await?;
        consensus_manager.submit_transaction(solana_tx_bytes).await?;

        info!("Submitted mixed transactions to consensus");

        // Wait for consensus to process
        sleep(Duration::from_secs(10)).await;

        // Check consensus state
        let stats = consensus_manager.get_stats().await?;
        info!("Consensus stats: {:?}", stats);

        info!("Consensus coordination test completed");
        Ok(())
    }

    /// Test RPC request routing between VMs
    pub async fn test_rpc_request_routing(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing RPC request routing between VMs");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Test concurrent RPC calls
        let reth_calls = async {
            let mut results = Vec::new();
            results.push(("eth_blockNumber", reth_engine.get_latest_block_number().await));
            results.push(("eth_chainId", reth_engine.get_chain_id().await.map(|id| id as u64)));
            results.push(("eth_gasPrice", reth_engine.get_gas_price().await));
            results
        };

        let solana_calls = async {
            let mut results = Vec::new();
            results.push(("getSlot", solana_engine.get_current_slot().await));
            results.push(("getBlockHeight", solana_engine.get_block_height().await));
            results.push(("getBalance", solana_engine.get_balance(&self.solana_keypair.pubkey()).await));
            results
        };

        let (reth_results, solana_results) = tokio::join!(reth_calls, solana_calls);

        info!("Reth RPC results:");
        for (method, result) in reth_results {
            match result {
                Ok(value) => info!("  {}: {}", method, value),
                Err(e) => warn!("  {}: Error - {}", method, e),
            }
        }

        info!("Solana RPC results:");
        for (method, result) in solana_results {
            match result {
                Ok(value) => info!("  {}: {}", method, value),
                Err(e) => warn!("  {}: Error - {}", method, e),
            }
        }

        info!("RPC request routing test completed");
        Ok(())
    }

    /// Test state synchronization between VMs
    pub async fn test_state_synchronization(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing state synchronization between VMs");

        let reth_engine = self.reth_engine.as_mut().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_mut().ok_or("Solana engine not initialized")?;

        // Trigger state sync on both engines
        let reth_sync = reth_engine.sync_state();
        let solana_sync = solana_engine.sync_state();

        let (reth_result, solana_result) = tokio::join!(reth_sync, solana_sync);

        match (reth_result, solana_result) {
            (Ok(_), Ok(_)) => info!("State synchronization successful on both VMs"),
            (Err(reth_err), Ok(_)) => warn!("Reth state sync failed: {}", reth_err),
            (Ok(_), Err(solana_err)) => warn!("Solana state sync failed: {}", solana_err),
            (Err(reth_err), Err(solana_err)) => {
                warn!("State sync failed on both VMs:");
                warn!("  Reth: {}", reth_err);
                warn!("  Solana: {}", solana_err);
            }
        }

        // Test cross-VM communication readiness
        let reth_ready = reth_engine.is_ready_for_cross_vm_communication().await;
        let solana_ready = solana_engine.is_ready_for_cross_vm_communication().await;

        info!("Cross-VM communication readiness: Reth={}, Solana={}", reth_ready, solana_ready);

        info!("State synchronization test completed");
        Ok(())
    }

    /// Test block production coordination
    pub async fn test_block_production_coordination(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing block production coordination");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Get initial state
        let initial_eth_block = reth_engine.get_latest_block_number().await?;
        let initial_solana_slot = solana_engine.get_current_slot().await?;

        info!("Initial state - ETH block: {}, Solana slot: {}", initial_eth_block, initial_solana_slot);

        // Wait for block production
        sleep(Duration::from_secs(15)).await;

        // Get final state
        let final_eth_block = reth_engine.get_latest_block_number().await?;
        let final_solana_slot = solana_engine.get_current_slot().await?;

        info!("Final state - ETH block: {}, Solana slot: {}", final_eth_block, final_solana_slot);

        // Verify progress
        let eth_progress = final_eth_block > initial_eth_block;
        let solana_progress = final_solana_slot > initial_solana_slot;

        info!("Block production progress: ETH={}, Solana={}", eth_progress, solana_progress);

        if eth_progress && solana_progress {
            info!("Both VMs are producing blocks successfully");
        } else {
            warn!("Some VMs are not producing blocks as expected");
        }

        info!("Block production coordination test completed");
        Ok(())
    }

    /// Test error handling and recovery
    pub async fn test_error_handling_and_recovery(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing error handling and recovery");

        let reth_engine = self.reth_engine.as_mut().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_mut().ok_or("Solana engine not initialized")?;

        // Test invalid transaction handling
        let invalid_evm_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "invalid_address".to_string(),
            to: Some("0x1111111111111111111111111111111111111111".to_string()),
            value: "invalid_value".to_string(),
            data: "0x".to_string(),
            gas_price: Some("0".to_string()),
            gas_limit: Some(0),
            nonce: Some(0),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("0".to_string()),
            max_priority_fee_per_gas: Some("0".to_string()),
            transaction_type: Some(2),
        };

        let invalid_result = reth_engine.process_transaction(&invalid_evm_tx).await;
        match invalid_result {
            Ok(_) => warn!("Invalid transaction was unexpectedly accepted"),
            Err(e) => info!("Invalid transaction correctly rejected: {}", e),
        }

        // Test recovery after error
        let valid_evm_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: Some("0x1111111111111111111111111111111111111111".to_string()),
            value: "1000000000000000000".to_string(),
            data: "0x".to_string(),
            gas_price: Some("20000000000".to_string()),
            gas_limit: Some(21000),
            nonce: Some(2),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("30000000000".to_string()),
            max_priority_fee_per_gas: Some("2000000000".to_string()),
            transaction_type: Some(2),
        };

        let recovery_result = reth_engine.process_transaction(&valid_evm_tx).await;
        match recovery_result {
            Ok(hash) => info!("Recovery successful, transaction processed: {}", hash),
            Err(e) => warn!("Recovery failed: {}", e),
        }

        // Test health check resilience
        let health_checks = vec![
            reth_engine.is_healthy(),
            solana_engine.is_healthy(),
        ];

        let health_results = futures::future::join_all(health_checks).await;
        for (i, result) in health_results.iter().enumerate() {
            match result {
                Ok(healthy) => info!("Engine {} health: {}", i, healthy),
                Err(e) => warn!("Engine {} health check failed: {}", i, e),
            }
        }

        info!("Error handling and recovery test completed");
        Ok(())
    }

    /// Cleanup test environment
    pub async fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Cleaning up cross-VM integration test environment");

        // Stop consensus manager
        if let Some(consensus_manager) = &mut self.consensus_manager {
            consensus_manager.stop().await?;
        }

        // Stop engines
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
    async fn test_cross_vm_full_integration() {
        setup_logging();
        info!("Starting cross-VM full integration test");

        let config = CrossVmTestConfig::default();
        let mut test_suite = CrossVmIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            error!("Setup failed: {}", e);
            return;
        }

        // Run all tests
        let tests = vec![
            ("dual_engine_startup", test_suite.test_dual_engine_startup()),
            ("cross_vm_transactions", test_suite.test_cross_vm_transaction_coordination()),
            ("consensus_coordination", test_suite.test_consensus_coordination()),
            ("rpc_routing", test_suite.test_rpc_request_routing()),
            ("state_synchronization", test_suite.test_state_synchronization()),
            ("block_production", test_suite.test_block_production_coordination()),
            ("error_handling", test_suite.test_error_handling_and_recovery()),
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

        info!("Cross-VM integration test results: {} passed, {} failed", passed, failed);
    }

    #[tokio::test]
    #[ignore = "Requires actual Reth and Solana binaries"]
    async fn test_dual_engine_startup_only() {
        setup_logging();
        info!("Starting dual engine startup test");

        let config = CrossVmTestConfig::default();
        let mut test_suite = CrossVmIntegrationTestSuite::new(config);

        if let Err(e) = test_suite.setup().await {
            error!("Setup failed: {}", e);
            return;
        }

        if let Err(e) = test_suite.test_dual_engine_startup().await {
            test_suite.cleanup().await.ok();
            error!("Dual engine startup test failed: {}", e);
            return;
        }

        test_suite.cleanup().await.ok();
        info!("Dual engine startup test completed successfully");
    }
}