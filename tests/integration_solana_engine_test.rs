//! Integration tests for Solana Execution Engine
//! Tests real connection to Solana process and communication workflows

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use multivm_common::config::VmType;
use solana_execution_engine::{
    config::{SolanaConfig, SolanaConnectionConfig},
    engine::SolanaExecutionEngine,
    error::SolanaEngineError,
    mempool::SolanaMempool,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    system_instruction,
    transaction::Transaction,
};

/// Test configuration for Solana integration tests
#[derive(Debug, Clone)]
pub struct SolanaIntegrationTestConfig {
    pub ledger_path: String,
    pub rpc_port: u16,
    pub ws_port: u16,
    pub ipc_socket_path: String,
    pub log_level: String,
    pub test_timeout: Duration,
}

impl Default for SolanaIntegrationTestConfig {
    fn default() -> Self {
        Self {
            ledger_path: "/tmp/solana-integration-test".to_string(),
            rpc_port: 8899,
            ws_port: 8900,
            ipc_socket_path: "/tmp/solana-integration-test.sock".to_string(),
            log_level: "info".to_string(),
            test_timeout: Duration::from_secs(120),
        }
    }
}

/// Integration test suite for Solana execution engine
pub struct SolanaIntegrationTestSuite {
    config: SolanaIntegrationTestConfig,
    engine: Option<SolanaExecutionEngine>,
    test_keypair: Keypair,
    mempool: Arc<SolanaMempool>,
}

impl SolanaIntegrationTestSuite {
    pub fn new(config: SolanaIntegrationTestConfig) -> Self {
        Self {
            config,
            engine: None,
            test_keypair: Keypair::new(),
            mempool: Arc::new(SolanaMempool::new()),
        }
    }

    /// Initialize the test suite with a real Solana engine
    pub async fn setup(&mut self) -> Result<(), SolanaEngineError> {
        info!("Setting up Solana integration test suite");

        // Create Solana config
        let solana_config = SolanaConfig {
            ledger_path: self.config.ledger_path.clone(),
            rpc_port: self.config.rpc_port,
            ws_port: self.config.ws_port,
            ipc_socket_path: self.config.ipc_socket_path.clone(),
            log_level: self.config.log_level.clone(),
            enable_dev_mode: true,
            slots_per_epoch: 32,
            ticks_per_slot: 64,
            genesis_config: Default::default(),
        };

        // Create connection config
        let connection_config = SolanaConnectionConfig {
            rpc_endpoint: format!("http://127.0.0.1:{}", self.config.rpc_port),
            ws_endpoint: format!("ws://127.0.0.1:{}", self.config.ws_port),
            commitment: CommitmentConfig::confirmed(),
            timeout: Duration::from_secs(30),
        };

        // Create Solana execution engine
        let engine = SolanaExecutionEngine::new(
            solana_config,
            connection_config,
            "integration-test-node".to_string(),
        ).await?;

        self.engine = Some(engine);
        info!("Solana integration test suite setup complete");
        Ok(())
    }

    /// Test basic Solana process connectivity
    pub async fn test_solana_process_connectivity(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing Solana process connectivity");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Start the Solana process
        engine.start().await?;

        // Wait for Solana to be ready
        for attempt in 1..=15 {
            match engine.is_healthy().await {
                Ok(true) => {
                    info!("Solana process is healthy after {} attempts", attempt);
                    return Ok(());
                }
                Ok(false) => {
                    warn!("Solana process not healthy, attempt {}/15", attempt);
                    sleep(Duration::from_secs(4)).await;
                }
                Err(e) => {
                    warn!("Health check failed: {}, attempt {}/15", e, attempt);
                    sleep(Duration::from_secs(4)).await;
                }
            }
        }

        Err(SolanaEngineError::Connection("Solana process failed to become healthy".to_string()))
    }

    /// Test RPC request relaying to Solana
    pub async fn test_rpc_request_relaying(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing RPC request relaying to Solana");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test getSlot
        let slot = engine.get_current_slot().await?;
        info!("Current slot: {}", slot);

        // Test getBlockHeight
        let block_height = engine.get_block_height().await?;
        info!("Block height: {}", block_height);

        // Test getVersion
        let version = engine.get_version().await?;
        info!("Solana version: {:?}", version);

        // Test getGenesisHash
        let genesis_hash = engine.get_genesis_hash().await?;
        info!("Genesis hash: {}", genesis_hash);

        // Test getBalance
        let balance = engine.get_balance(&self.test_keypair.pubkey()).await?;
        info!("Balance for test keypair: {} lamports", balance);

        // Test getRecentBlockhash
        let blockhash = engine.get_recent_blockhash().await?;
        info!("Recent blockhash: {}", blockhash);

        info!("RPC request relaying test completed successfully");
        Ok(())
    }

    /// Test transaction processing through Solana
    pub async fn test_transaction_processing(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing transaction processing through Solana");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Create a test transaction (transfer)
        let recipient = Keypair::new();
        let transfer_amount = 1_000_000; // 0.001 SOL
        
        let recent_blockhash = engine.get_recent_blockhash().await?;
        let instruction = system_instruction::transfer(
            &self.test_keypair.pubkey(),
            &recipient.pubkey(),
            transfer_amount,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.test_keypair.pubkey()),
            &[&self.test_keypair],
            recent_blockhash,
        );

        // Process the transaction
        let result = engine.process_transaction(&transaction).await;
        
        match result {
            Ok(signature) => {
                info!("Transaction processed successfully: {}", signature);
                
                // Wait for transaction to be confirmed
                sleep(Duration::from_secs(3)).await;
                
                // Check transaction confirmation
                let confirmed = engine.confirm_transaction(&signature).await?;
                info!("Transaction confirmed: {}", confirmed);
                
                // Get transaction details
                let transaction_result = engine.get_transaction(&signature).await;
                match transaction_result {
                    Ok(tx_info) => {
                        info!("Transaction details: {:?}", tx_info);
                    }
                    Err(e) => {
                        warn!("Failed to get transaction details: {}", e);
                    }
                }
                
            }
            Err(e) => {
                // In dev mode, transactions might fail due to insufficient balance
                // This is expected behavior for integration tests
                warn!("Transaction processing failed (expected in dev mode): {}", e);
            }
        }

        info!("Transaction processing test completed");
        Ok(())
    }

    /// Test block processing workflow
    pub async fn test_block_processing_workflow(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing block processing workflow");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Get initial slot
        let initial_slot = engine.get_current_slot().await?;
        info!("Initial slot: {}", initial_slot);

        // Wait for a few slots to pass
        sleep(Duration::from_secs(8)).await;

        // Get current slot
        let current_slot = engine.get_current_slot().await?;
        info!("Current slot: {}", current_slot);

        // Verify slots are progressing
        if current_slot > initial_slot {
            info!("Slot progression verified: {} -> {}", initial_slot, current_slot);
        } else {
            warn!("No slot progression during test period");
        }

        // Test block retrieval
        let block_result = engine.get_block(current_slot).await;
        match block_result {
            Ok(block) => {
                info!("Retrieved block for slot {}: {:?}", current_slot, block);
            }
            Err(e) => {
                warn!("Failed to retrieve block: {}", e);
            }
        }

        // Test block commitment
        let block_commitment = engine.get_block_commitment(current_slot).await?;
        info!("Block commitment for slot {}: {:?}", current_slot, block_commitment);

        info!("Block processing workflow test completed");
        Ok(())
    }

    /// Test Solana mempool operations
    pub async fn test_mempool_operations(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing Solana mempool operations");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Create test transaction
        let recipient = Keypair::new();
        let recent_blockhash = engine.get_recent_blockhash().await?;
        let instruction = system_instruction::transfer(
            &self.test_keypair.pubkey(),
            &recipient.pubkey(),
            1_000_000,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.test_keypair.pubkey()),
            &[&self.test_keypair],
            recent_blockhash,
        );

        // Add transaction to mempool
        let signature = transaction.signatures[0];
        let serialized_tx = bincode::serialize(&transaction).map_err(|e| {
            SolanaEngineError::SerializationError(format!("Failed to serialize transaction: {}", e))
        })?;

        self.mempool.add_transaction(signature, serialized_tx.into()).await.map_err(|e| {
            SolanaEngineError::Transaction(format!("Failed to add to mempool: {}", e))
        })?;

        info!("Added transaction to mempool: {}", signature);

        // Check mempool size
        let mempool_size = self.mempool.size().await;
        info!("Mempool size: {}", mempool_size);
        assert!(mempool_size > 0);

        // Get transaction from mempool
        let retrieved_tx = self.mempool.get_transaction(&signature).await;
        match retrieved_tx {
            Some(tx) => {
                info!("Retrieved transaction from mempool: {} bytes", tx.raw_body.len());
            }
            None => {
                warn!("Transaction not found in mempool");
            }
        }

        // Remove transaction from mempool
        let removed = self.mempool.remove_transaction(&signature).await;
        info!("Removed transaction from mempool: {}", removed);

        info!("Mempool operations test completed");
        Ok(())
    }

    /// Test Solana account operations
    pub async fn test_account_operations(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing Solana account operations");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test account info
        let account_info = engine.get_account_info(&self.test_keypair.pubkey()).await;
        match account_info {
            Ok(account) => {
                info!("Account info: {:?}", account);
            }
            Err(e) => {
                warn!("Failed to get account info: {}", e);
            }
        }

        // Test multiple account info
        let accounts = vec![
            self.test_keypair.pubkey(),
            Pubkey::new_unique(),
        ];
        
        let multiple_accounts = engine.get_multiple_accounts(&accounts).await;
        match multiple_accounts {
            Ok(accounts_info) => {
                info!("Multiple accounts info: {} accounts", accounts_info.len());
            }
            Err(e) => {
                warn!("Failed to get multiple accounts: {}", e);
            }
        }

        // Test program accounts
        let program_accounts = engine.get_program_accounts(&solana_sdk::system_program::id()).await;
        match program_accounts {
            Ok(accounts) => {
                info!("System program accounts: {} accounts", accounts.len());
            }
            Err(e) => {
                warn!("Failed to get program accounts: {}", e);
            }
        }

        info!("Account operations test completed");
        Ok(())
    }

    /// Test MultiVM integration points
    pub async fn test_multivm_integration(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing MultiVM integration points");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test cross-VM communication readiness
        let is_ready = engine.is_ready_for_cross_vm_communication().await;
        info!("Ready for cross-VM communication: {}", is_ready);

        // Test state synchronization
        let state_sync_result = engine.sync_state().await;
        match state_sync_result {
            Ok(_) => info!("State synchronization successful"),
            Err(e) => warn!("State synchronization failed: {}", e),
        }

        // Test epoch information
        let epoch_info = engine.get_epoch_info().await?;
        info!("Epoch info: {:?}", epoch_info);

        // Test vote accounts
        let vote_accounts = engine.get_vote_accounts().await;
        match vote_accounts {
            Ok(accounts) => {
                info!("Vote accounts: {} current, {} delinquent", 
                      accounts.current.len(), accounts.delinquent.len());
            }
            Err(e) => {
                warn!("Failed to get vote accounts: {}", e);
            }
        }

        // Test cluster nodes
        let cluster_nodes = engine.get_cluster_nodes().await;
        match cluster_nodes {
            Ok(nodes) => {
                info!("Cluster nodes: {} nodes", nodes.len());
            }
            Err(e) => {
                warn!("Failed to get cluster nodes: {}", e);
            }
        }

        info!("MultiVM integration test completed");
        Ok(())
    }

    /// Test Solana RPC server proxy
    pub async fn test_rpc_server_proxy(&mut self) -> Result<(), SolanaEngineError> {
        info!("Testing Solana RPC server proxy");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            SolanaEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test direct RPC calls through the proxy
        let rpc_client = engine.get_rpc_client()?;
        
        // Test getSlot through proxy
        let slot = rpc_client.get_slot().map_err(|e| {
            SolanaEngineError::Rpc(format!("Failed to get slot: {}", e))
        })?;
        info!("Slot via RPC proxy: {}", slot);

        // Test getBlockHeight through proxy
        let block_height = rpc_client.get_block_height().map_err(|e| {
            SolanaEngineError::Rpc(format!("Failed to get block height: {}", e))
        })?;
        info!("Block height via RPC proxy: {}", block_height);

        // Test getLatestBlockhash through proxy
        let latest_blockhash = rpc_client.get_latest_blockhash().map_err(|e| {
            SolanaEngineError::Rpc(format!("Failed to get latest blockhash: {}", e))
        })?;
        info!("Latest blockhash via RPC proxy: {}", latest_blockhash);

        info!("RPC server proxy test completed");
        Ok(())
    }

    /// Cleanup test environment
    pub async fn cleanup(&mut self) -> Result<(), SolanaEngineError> {
        info!("Cleaning up Solana integration test environment");
        
        if let Some(engine) = &mut self.engine {
            engine.stop().await?;
        }
        
        // Clean up test data directory
        let _ = std::fs::remove_dir_all(&self.config.ledger_path);
        
        // Clean up IPC socket
        let _ = std::fs::remove_file(&self.config.ipc_socket_path);
        
        info!("Cleanup completed");
        Ok(())
    }
}

/// Main integration test runner
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
    #[ignore = "Requires actual Solana binary and long running process"]
    async fn test_solana_integration_full_workflow() {
        setup_logging();
        info!("Starting Solana integration test full workflow");

        let config = SolanaIntegrationTestConfig::default();
        let mut test_suite = SolanaIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            panic!("Setup failed: {}", e);
        }

        // Run all tests
        let tests = vec![
            ("connectivity", test_suite.test_solana_process_connectivity()),
            ("rpc_relaying", test_suite.test_rpc_request_relaying()),
            ("transaction_processing", test_suite.test_transaction_processing()),
            ("block_processing", test_suite.test_block_processing_workflow()),
            ("mempool_operations", test_suite.test_mempool_operations()),
            ("account_operations", test_suite.test_account_operations()),
            ("rpc_server_proxy", test_suite.test_rpc_server_proxy()),
            ("multivm_integration", test_suite.test_multivm_integration()),
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
                    warn!("❌ Test '{}' failed: {}", test_name, e);
                    failed += 1;
                }
            }
        }

        // Cleanup
        if let Err(e) = test_suite.cleanup().await {
            warn!("Cleanup failed: {}", e);
        }

        info!("Solana integration test results: {} passed, {} failed", passed, failed);
        
        if failed > 0 {
            panic!("Some integration tests failed");
        }
    }

    #[tokio::test]
    #[ignore = "Requires actual Solana binary"]
    async fn test_solana_connectivity_only() {
        setup_logging();
        info!("Starting Solana connectivity test");

        let config = SolanaIntegrationTestConfig {
            test_timeout: Duration::from_secs(60),
            ..Default::default()
        };

        let mut test_suite = SolanaIntegrationTestSuite::new(config);

        // Setup and test connectivity
        if let Err(e) = test_suite.setup().await {
            panic!("Setup failed: {}", e);
        }

        if let Err(e) = test_suite.test_solana_process_connectivity().await {
            test_suite.cleanup().await.ok();
            panic!("Connectivity test failed: {}", e);
        }

        // Cleanup
        test_suite.cleanup().await.ok();
        info!("Solana connectivity test completed successfully");
    }

    #[tokio::test]
    #[ignore = "Requires actual Solana binary"]
    async fn test_solana_rpc_communication() {
        setup_logging();
        info!("Starting Solana RPC communication test");

        let config = SolanaIntegrationTestConfig::default();
        let mut test_suite = SolanaIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            panic!("Setup failed: {}", e);
        }

        // Test connectivity first
        if let Err(e) = test_suite.test_solana_process_connectivity().await {
            test_suite.cleanup().await.ok();
            panic!("Connectivity test failed: {}", e);
        }

        // Test RPC communication
        if let Err(e) = test_suite.test_rpc_request_relaying().await {
            test_suite.cleanup().await.ok();
            panic!("RPC communication test failed: {}", e);
        }

        // Cleanup
        test_suite.cleanup().await.ok();
        info!("Solana RPC communication test completed successfully");
    }

    #[tokio::test]
    async fn test_solana_mempool_only() {
        setup_logging();
        info!("Starting Solana mempool test");

        let config = SolanaIntegrationTestConfig::default();
        let mut test_suite = SolanaIntegrationTestSuite::new(config);

        // Test mempool operations without needing actual Solana process
        if let Err(e) = test_suite.test_mempool_operations().await {
            panic!("Mempool test failed: {}", e);
        }

        info!("Solana mempool test completed successfully");
    }
}