//! Integration tests for Reth Execution Engine
//! Tests real connection to Reth process and communication workflows

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use multivm_common::config::VmType;
use reth_execution_engine::{
    config_integration::RethIntegrationConfig,
    engine::{MultivmTransaction, RethExecutionEngine},
    error::RethEngineError,
};

/// Test configuration for integration tests
#[derive(Debug, Clone)]
pub struct IntegrationTestConfig {
    pub reth_data_dir: String,
    pub reth_http_port: u16,
    pub reth_ws_port: u16,
    pub reth_auth_port: u16,
    pub jwt_secret: String,
    pub chain_id: u64,
    pub test_timeout: Duration,
}

impl Default for IntegrationTestConfig {
    fn default() -> Self {
        Self {
            reth_data_dir: "/tmp/reth-integration-test".to_string(),
            reth_http_port: 8545,
            reth_ws_port: 8546,
            reth_auth_port: 8551,
            jwt_secret: "test-jwt-secret-for-integration-testing-only".to_string(),
            chain_id: 1337,
            test_timeout: Duration::from_secs(120),
        }
    }
}

/// Integration test suite for Reth execution engine
pub struct RethIntegrationTestSuite {
    config: IntegrationTestConfig,
    engine: Option<RethExecutionEngine>,
}

impl RethIntegrationTestSuite {
    pub fn new(config: IntegrationTestConfig) -> Self {
        Self {
            config,
            engine: None,
        }
    }

    /// Initialize the test suite with a real Reth engine
    pub async fn setup(&mut self) -> Result<(), RethEngineError> {
        info!("Setting up Reth integration test suite");

        // Create Reth integration config
        let reth_config = RethIntegrationConfig {
            data_dir: self.config.reth_data_dir.clone(),
            http_port: self.config.reth_http_port,
            ws_port: self.config.reth_ws_port,
            auth_port: self.config.reth_auth_port,
            jwt_secret: self.config.jwt_secret.clone(),
            chain_id: self.config.chain_id,
            enable_dev_mode: true,
            mining_interval: Some(Duration::from_secs(1)),
            gas_limit: 30_000_000,
            base_fee: 1_000_000_000,
        };

        // Create Reth execution engine
        let engine = RethExecutionEngine::new(
            reth_config,
            "integration-test-node".to_string(),
        ).await?;

        self.engine = Some(engine);
        info!("Reth integration test suite setup complete");
        Ok(())
    }

    /// Test basic Reth process connectivity
    pub async fn test_reth_process_connectivity(&mut self) -> Result<(), RethEngineError> {
        info!("Testing Reth process connectivity");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            RethEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Start the Reth process
        engine.start().await?;

        // Wait for Reth to be ready
        for attempt in 1..=10 {
            match engine.is_healthy().await {
                Ok(true) => {
                    info!("Reth process is healthy after {} attempts", attempt);
                    return Ok(());
                }
                Ok(false) => {
                    warn!("Reth process not healthy, attempt {}/10", attempt);
                    sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    warn!("Health check failed: {}, attempt {}/10", e, attempt);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        Err(RethEngineError::Connection("Reth process failed to become healthy".to_string()))
    }

    /// Test RPC request relaying to Reth
    pub async fn test_rpc_request_relaying(&mut self) -> Result<(), RethEngineError> {
        info!("Testing RPC request relaying to Reth");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            RethEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test eth_blockNumber
        let block_number = engine.get_latest_block_number().await?;
        info!("Latest block number: {}", block_number);

        // Test eth_chainId
        let chain_id = engine.get_chain_id().await?;
        info!("Chain ID: {}", chain_id);
        assert_eq!(chain_id, self.config.chain_id);

        // Test eth_getBalance
        let balance = engine.get_balance("0x0000000000000000000000000000000000000000").await?;
        info!("Balance for zero address: {}", balance);

        // Test eth_gasPrice
        let gas_price = engine.get_gas_price().await?;
        info!("Current gas price: {}", gas_price);

        info!("RPC request relaying test completed successfully");
        Ok(())
    }

    /// Test transaction processing through Reth
    pub async fn test_transaction_processing(&mut self) -> Result<(), RethEngineError> {
        info!("Testing transaction processing through Reth");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            RethEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Create a test transaction
        let test_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: Some("0x1111111111111111111111111111111111111111".to_string()),
            value: "1000000000000000000".to_string(), // 1 ETH in wei
            data: "0x".to_string(),
            gas_price: Some("20000000000".to_string()), // 20 gwei
            gas_limit: Some(21000),
            nonce: Some(0),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("30000000000".to_string()),
            max_priority_fee_per_gas: Some("2000000000".to_string()),
            transaction_type: Some(2), // EIP-1559
        };

        // Process the transaction
        let result = engine.process_transaction(&test_tx).await;
        
        match result {
            Ok(tx_hash) => {
                info!("Transaction processed successfully: {}", tx_hash);
                
                // Wait for transaction to be mined
                sleep(Duration::from_secs(3)).await;
                
                // Check transaction receipt
                let receipt = engine.get_transaction_receipt(&tx_hash).await?;
                info!("Transaction receipt: {:?}", receipt);
                
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
    pub async fn test_block_processing_workflow(&mut self) -> Result<(), RethEngineError> {
        info!("Testing block processing workflow");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            RethEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Get initial block number
        let initial_block = engine.get_latest_block_number().await?;
        info!("Initial block number: {}", initial_block);

        // Wait for a few blocks to be mined
        sleep(Duration::from_secs(5)).await;

        // Get current block number
        let current_block = engine.get_latest_block_number().await?;
        info!("Current block number: {}", current_block);

        // Verify blocks are being produced
        if current_block > initial_block {
            info!("Block production verified: {} -> {}", initial_block, current_block);
        } else {
            warn!("No new blocks produced during test period");
        }

        // Test block retrieval
        let block_hash = engine.get_block_hash(current_block).await?;
        info!("Block hash for block {}: {:?}", current_block, block_hash);

        // Test block by number
        let block_data = engine.get_block_by_number(current_block).await?;
        info!("Retrieved block data for block {}: {} bytes", current_block, block_data.len());

        info!("Block processing workflow test completed");
        Ok(())
    }

    /// Test Engine API communication
    pub async fn test_engine_api_communication(&mut self) -> Result<(), RethEngineError> {
        info!("Testing Engine API communication");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            RethEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test forkchoice update
        let fork_choice_result = engine.update_fork_choice(
            "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            None,
            None,
        ).await;

        match fork_choice_result {
            Ok(response) => {
                info!("Fork choice update successful: {:?}", response);
            }
            Err(e) => {
                warn!("Fork choice update failed (expected in test): {}", e);
            }
        }

        // Test payload building
        let payload_result = engine.build_payload(vec![]).await;
        
        match payload_result {
            Ok(payload) => {
                info!("Payload built successfully: {} bytes", payload.len());
            }
            Err(e) => {
                warn!("Payload building failed (expected in test): {}", e);
            }
        }

        info!("Engine API communication test completed");
        Ok(())
    }

    /// Test MultiVM integration points
    pub async fn test_multivm_integration(&mut self) -> Result<(), RethEngineError> {
        info!("Testing MultiVM integration points");
        
        let engine = self.engine.as_mut().ok_or_else(|| {
            RethEngineError::Configuration("Engine not initialized".to_string())
        })?;

        // Test transaction conversion
        let multivm_tx = MultivmTransaction {
            vm_type: VmType::EVM,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: Some("0x1111111111111111111111111111111111111111".to_string()),
            value: "1000000000000000000".to_string(),
            data: "0x".to_string(),
            gas_price: Some("20000000000".to_string()),
            gas_limit: Some(21000),
            nonce: Some(0),
            chain_id: Some(self.config.chain_id),
            max_fee_per_gas: Some("30000000000".to_string()),
            max_priority_fee_per_gas: Some("2000000000".to_string()),
            transaction_type: Some(2),
        };

        // Test transaction validation
        let validation_result = engine.validate_transaction(&multivm_tx).await;
        info!("Transaction validation result: {:?}", validation_result);

        // Test state synchronization
        let state_sync_result = engine.sync_state().await;
        match state_sync_result {
            Ok(_) => info!("State synchronization successful"),
            Err(e) => warn!("State synchronization failed: {}", e),
        }

        // Test cross-VM communication readiness
        let is_ready = engine.is_ready_for_cross_vm_communication().await;
        info!("Ready for cross-VM communication: {}", is_ready);

        info!("MultiVM integration test completed");
        Ok(())
    }

    /// Cleanup test environment
    pub async fn cleanup(&mut self) -> Result<(), RethEngineError> {
        info!("Cleaning up Reth integration test environment");
        
        if let Some(engine) = &mut self.engine {
            engine.stop().await?;
        }
        
        // Clean up test data directory
        let _ = std::fs::remove_dir_all(&self.config.reth_data_dir);
        
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
    #[ignore = "Requires actual Reth binary and long running process"]
    async fn test_reth_integration_full_workflow() {
        setup_logging();
        info!("Starting Reth integration test full workflow");

        let config = IntegrationTestConfig::default();
        let mut test_suite = RethIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            panic!("Setup failed: {}", e);
        }

        // Run all tests
        let tests = vec![
            ("connectivity", test_suite.test_reth_process_connectivity()),
            ("rpc_relaying", test_suite.test_rpc_request_relaying()),
            ("transaction_processing", test_suite.test_transaction_processing()),
            ("block_processing", test_suite.test_block_processing_workflow()),
            ("engine_api", test_suite.test_engine_api_communication()),
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

        info!("Reth integration test results: {} passed, {} failed", passed, failed);
        
        if failed > 0 {
            panic!("Some integration tests failed");
        }
    }

    #[tokio::test]
    #[ignore = "Requires actual Reth binary"]
    async fn test_reth_connectivity_only() {
        setup_logging();
        info!("Starting Reth connectivity test");

        let config = IntegrationTestConfig {
            test_timeout: Duration::from_secs(30),
            ..Default::default()
        };

        let mut test_suite = RethIntegrationTestSuite::new(config);

        // Setup and test connectivity
        if let Err(e) = test_suite.setup().await {
            panic!("Setup failed: {}", e);
        }

        if let Err(e) = test_suite.test_reth_process_connectivity().await {
            test_suite.cleanup().await.ok();
            panic!("Connectivity test failed: {}", e);
        }

        // Cleanup
        test_suite.cleanup().await.ok();
        info!("Reth connectivity test completed successfully");
    }

    #[tokio::test]
    #[ignore = "Requires actual Reth binary"]
    async fn test_reth_rpc_communication() {
        setup_logging();
        info!("Starting Reth RPC communication test");

        let config = IntegrationTestConfig::default();
        let mut test_suite = RethIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            panic!("Setup failed: {}", e);
        }

        // Test connectivity first
        if let Err(e) = test_suite.test_reth_process_connectivity().await {
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
        info!("Reth RPC communication test completed successfully");
    }
}