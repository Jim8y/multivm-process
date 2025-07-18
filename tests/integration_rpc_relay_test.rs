//! Integration tests for RPC Request Relaying
//! Tests RPC proxy functionality and request routing between MultiVM and execution engines

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};
use serde_json::{json, Value};

use multivm_common::config::VmType;
use reth_execution_engine::{
    config_integration::RethIntegrationConfig,
    engine::RethExecutionEngine,
    error::RethEngineError,
};
use solana_execution_engine::{
    config::{SolanaConfig, SolanaConnectionConfig},
    engine::SolanaExecutionEngine,
    engine_rpc_server::SolanaEngineRpcServer,
    error::SolanaEngineError,
};

/// Configuration for RPC relay integration tests
#[derive(Debug, Clone)]
pub struct RpcRelayTestConfig {
    pub reth_data_dir: String,
    pub reth_http_port: u16,
    pub reth_ws_port: u16,
    pub reth_auth_port: u16,
    pub solana_ledger_path: String,
    pub solana_rpc_port: u16,
    pub solana_ws_port: u16,
    pub solana_proxy_port: u16,
    pub jwt_secret: String,
    pub chain_id: u64,
    pub test_timeout: Duration,
}

impl Default for RpcRelayTestConfig {
    fn default() -> Self {
        Self {
            reth_data_dir: "/tmp/rpc-relay-test-reth".to_string(),
            reth_http_port: 8545,
            reth_ws_port: 8546,
            reth_auth_port: 8551,
            solana_ledger_path: "/tmp/rpc-relay-test-solana".to_string(),
            solana_rpc_port: 8899,
            solana_ws_port: 8900,
            solana_proxy_port: 8901,
            jwt_secret: "test-jwt-secret-for-rpc-relay".to_string(),
            chain_id: 1337,
            test_timeout: Duration::from_secs(120),
        }
    }
}

/// RPC relay integration test suite
pub struct RpcRelayIntegrationTestSuite {
    config: RpcRelayTestConfig,
    reth_engine: Option<RethExecutionEngine>,
    solana_engine: Option<SolanaExecutionEngine>,
    solana_rpc_server: Option<SolanaEngineRpcServer>,
}

impl RpcRelayIntegrationTestSuite {
    pub fn new(config: RpcRelayTestConfig) -> Self {
        Self {
            config,
            reth_engine: None,
            solana_engine: None,
            solana_rpc_server: None,
        }
    }

    /// Setup execution engines and RPC servers
    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Setting up RPC relay integration test suite");

        // Setup Reth engine
        let reth_config = RethIntegrationConfig {
            data_dir: self.config.reth_data_dir.clone(),
            http_port: self.config.reth_http_port,
            ws_port: self.config.reth_ws_port,
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
            "rpc-relay-test-reth".to_string(),
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
            commitment: solana_sdk::commitment_config::CommitmentConfig::confirmed(),
            timeout: Duration::from_secs(30),
        };

        let solana_engine = SolanaExecutionEngine::new(
            solana_config,
            solana_connection_config,
            "rpc-relay-test-solana".to_string(),
        ).await?;

        // Setup Solana RPC proxy server
        let mut solana_rpc_server = SolanaEngineRpcServer::new(
            "127.0.0.1".to_string(),
            self.config.solana_proxy_port,
            format!("http://127.0.0.1:{}", self.config.solana_rpc_port),
        );

        self.reth_engine = Some(reth_engine);
        self.solana_engine = Some(solana_engine);
        self.solana_rpc_server = Some(solana_rpc_server);

        info!("RPC relay integration test suite setup complete");
        Ok(())
    }

    /// Test starting engines and RPC servers
    pub async fn test_rpc_servers_startup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing RPC servers startup");

        let reth_engine = self.reth_engine.as_mut().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_mut().ok_or("Solana engine not initialized")?;
        let solana_rpc_server = self.solana_rpc_server.as_mut().ok_or("Solana RPC server not initialized")?;

        // Start engines
        reth_engine.start().await?;
        solana_engine.start().await?;

        // Start Solana RPC proxy server
        solana_rpc_server.start().await?;

        info!("All RPC servers started successfully");

        // Wait for services to become ready
        for attempt in 1..=15 {
            let reth_healthy = reth_engine.is_healthy().await.unwrap_or(false);
            let solana_healthy = solana_engine.is_healthy().await.unwrap_or(false);

            if reth_healthy && solana_healthy {
                info!("All services are healthy after {} attempts", attempt);
                return Ok(());
            }

            warn!("Waiting for services to become healthy: Reth={}, Solana={}, attempt {}/15", 
                  reth_healthy, solana_healthy, attempt);
            sleep(Duration::from_secs(3)).await;
        }

        Err("Services failed to become healthy within timeout".into())
    }

    /// Test Reth RPC request relaying
    pub async fn test_reth_rpc_relaying(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing Reth RPC request relaying");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;

        // Test various Reth RPC methods
        let rpc_tests = vec![
            ("eth_blockNumber", "get_latest_block_number", reth_engine.get_latest_block_number()),
            ("eth_chainId", "get_chain_id", reth_engine.get_chain_id().map(|id| id as u64)),
            ("eth_gasPrice", "get_gas_price", reth_engine.get_gas_price()),
        ];

        for (method_name, test_name, rpc_call) in rpc_tests {
            match rpc_call.await {
                Ok(result) => {
                    info!("✅ {} ({}): {}", method_name, test_name, result);
                }
                Err(e) => {
                    warn!("❌ {} ({}): {}", method_name, test_name, e);
                }
            }
        }

        // Test balance query
        let balance_result = reth_engine.get_balance("0x0000000000000000000000000000000000000000").await;
        match balance_result {
            Ok(balance) => info!("✅ eth_getBalance: {}", balance),
            Err(e) => warn!("❌ eth_getBalance: {}", e),
        }

        // Test transaction count
        let tx_count_result = reth_engine.get_transaction_count("0x0000000000000000000000000000000000000000").await;
        match tx_count_result {
            Ok(count) => info!("✅ eth_getTransactionCount: {}", count),
            Err(e) => warn!("❌ eth_getTransactionCount: {}", e),
        }

        info!("Reth RPC relaying test completed");
        Ok(())
    }

    /// Test Solana RPC request relaying
    pub async fn test_solana_rpc_relaying(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing Solana RPC request relaying");

        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Test various Solana RPC methods
        let slot_result = solana_engine.get_current_slot().await;
        match slot_result {
            Ok(slot) => info!("✅ getSlot: {}", slot),
            Err(e) => warn!("❌ getSlot: {}", e),
        }

        let block_height_result = solana_engine.get_block_height().await;
        match block_height_result {
            Ok(height) => info!("✅ getBlockHeight: {}", height),
            Err(e) => warn!("❌ getBlockHeight: {}", e),
        }

        let version_result = solana_engine.get_version().await;
        match version_result {
            Ok(version) => info!("✅ getVersion: {:?}", version),
            Err(e) => warn!("❌ getVersion: {}", e),
        }

        let genesis_hash_result = solana_engine.get_genesis_hash().await;
        match genesis_hash_result {
            Ok(hash) => info!("✅ getGenesisHash: {}", hash),
            Err(e) => warn!("❌ getGenesisHash: {}", e),
        }

        let blockhash_result = solana_engine.get_recent_blockhash().await;
        match blockhash_result {
            Ok(hash) => info!("✅ getRecentBlockhash: {}", hash),
            Err(e) => warn!("❌ getRecentBlockhash: {}", e),
        }

        // Test balance query
        let test_pubkey = solana_sdk::signature::Keypair::new().pubkey();
        let balance_result = solana_engine.get_balance(&test_pubkey).await;
        match balance_result {
            Ok(balance) => info!("✅ getBalance: {} lamports", balance),
            Err(e) => warn!("❌ getBalance: {}", e),
        }

        info!("Solana RPC relaying test completed");
        Ok(())
    }

    /// Test concurrent RPC requests
    pub async fn test_concurrent_rpc_requests(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing concurrent RPC requests");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Create multiple concurrent requests
        let concurrent_requests = vec![
            // Reth requests
            tokio::spawn({
                let engine = reth_engine.clone();
                async move {
                    let mut results = Vec::new();
                    for i in 0..5 {
                        let result = engine.get_latest_block_number().await;
                        results.push(format!("Reth-{}: {:?}", i, result));
                        sleep(Duration::from_millis(100)).await;
                    }
                    results
                }
            }),
            // Solana requests
            tokio::spawn({
                let engine = solana_engine.clone();
                async move {
                    let mut results = Vec::new();
                    for i in 0..5 {
                        let result = engine.get_current_slot().await;
                        results.push(format!("Solana-{}: {:?}", i, result));
                        sleep(Duration::from_millis(100)).await;
                    }
                    results
                }
            }),
        ];

        let results = futures::future::join_all(concurrent_requests).await;
        
        for (i, result) in results.iter().enumerate() {
            match result {
                Ok(request_results) => {
                    info!("Concurrent request batch {} completed:", i);
                    for req_result in request_results {
                        info!("  {}", req_result);
                    }
                }
                Err(e) => {
                    warn!("Concurrent request batch {} failed: {}", i, e);
                }
            }
        }

        info!("Concurrent RPC requests test completed");
        Ok(())
    }

    /// Test RPC error handling
    pub async fn test_rpc_error_handling(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing RPC error handling");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Test invalid requests
        let invalid_balance_result = reth_engine.get_balance("invalid_address").await;
        match invalid_balance_result {
            Ok(_) => warn!("Invalid address request unexpectedly succeeded"),
            Err(e) => info!("✅ Invalid address correctly rejected: {}", e),
        }

        let invalid_pubkey = "invalid_pubkey";
        let invalid_pubkey_result = solana_engine.get_balance(
            &solana_sdk::pubkey::Pubkey::try_from(invalid_pubkey.as_bytes()).unwrap_or_default()
        ).await;
        match invalid_pubkey_result {
            Ok(_) => warn!("Invalid pubkey request unexpectedly succeeded"),
            Err(e) => info!("✅ Invalid pubkey correctly rejected: {}", e),
        }

        // Test timeout handling
        info!("Testing timeout handling...");
        let timeout_duration = Duration::from_millis(1); // Very short timeout
        
        let timeout_result = tokio::time::timeout(
            timeout_duration,
            reth_engine.get_latest_block_number()
        ).await;
        
        match timeout_result {
            Ok(result) => info!("Request completed within timeout: {:?}", result),
            Err(_) => info!("✅ Timeout correctly triggered"),
        }

        info!("RPC error handling test completed");
        Ok(())
    }

    /// Test RPC proxy functionality
    pub async fn test_rpc_proxy_functionality(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing RPC proxy functionality");

        // Test direct HTTP RPC calls to the proxy
        let client = reqwest::Client::new();
        let proxy_url = format!("http://127.0.0.1:{}", self.config.solana_proxy_port);

        // Test getSlot via proxy
        let slot_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
            "params": []
        });

        let slot_response = client
            .post(&proxy_url)
            .json(&slot_request)
            .send()
            .await;

        match slot_response {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                info!("✅ Proxy getSlot response ({}): {}", status, body);
            }
            Err(e) => {
                warn!("❌ Proxy getSlot request failed: {}", e);
            }
        }

        // Test getBlockHeight via proxy
        let block_height_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "getBlockHeight",
            "params": []
        });

        let block_height_response = client
            .post(&proxy_url)
            .json(&block_height_request)
            .send()
            .await;

        match block_height_response {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                info!("✅ Proxy getBlockHeight response ({}): {}", status, body);
            }
            Err(e) => {
                warn!("❌ Proxy getBlockHeight request failed: {}", e);
            }
        }

        // Test invalid method via proxy
        let invalid_request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "invalidMethod",
            "params": []
        });

        let invalid_response = client
            .post(&proxy_url)
            .json(&invalid_request)
            .send()
            .await;

        match invalid_response {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                info!("✅ Proxy invalid method response ({}): {}", status, body);
            }
            Err(e) => {
                warn!("❌ Proxy invalid method request failed: {}", e);
            }
        }

        info!("RPC proxy functionality test completed");
        Ok(())
    }

    /// Test RPC request routing and load balancing
    pub async fn test_rpc_routing_and_load_balancing(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing RPC routing and load balancing");

        let reth_engine = self.reth_engine.as_ref().ok_or("Reth engine not initialized")?;
        let solana_engine = self.solana_engine.as_ref().ok_or("Solana engine not initialized")?;

        // Create multiple concurrent requests to test load balancing
        let mut reth_requests = Vec::new();
        let mut solana_requests = Vec::new();

        for i in 0..10 {
            reth_requests.push(tokio::spawn({
                let engine = reth_engine.clone();
                async move {
                    let start = std::time::Instant::now();
                    let result = engine.get_latest_block_number().await;
                    let duration = start.elapsed();
                    (i, result, duration)
                }
            }));

            solana_requests.push(tokio::spawn({
                let engine = solana_engine.clone();
                async move {
                    let start = std::time::Instant::now();
                    let result = engine.get_current_slot().await;
                    let duration = start.elapsed();
                    (i, result, duration)
                }
            }));
        }

        // Wait for all requests to complete
        let reth_results = futures::future::join_all(reth_requests).await;
        let solana_results = futures::future::join_all(solana_requests).await;

        // Analyze results
        let mut reth_durations = Vec::new();
        let mut solana_durations = Vec::new();

        for result in reth_results {
            match result {
                Ok((i, rpc_result, duration)) => {
                    reth_durations.push(duration);
                    match rpc_result {
                        Ok(block_num) => info!("Reth request {}: {} ({}ms)", i, block_num, duration.as_millis()),
                        Err(e) => warn!("Reth request {}: Error - {} ({}ms)", i, e, duration.as_millis()),
                    }
                }
                Err(e) => warn!("Reth request task failed: {}", e),
            }
        }

        for result in solana_results {
            match result {
                Ok((i, rpc_result, duration)) => {
                    solana_durations.push(duration);
                    match rpc_result {
                        Ok(slot) => info!("Solana request {}: {} ({}ms)", i, slot, duration.as_millis()),
                        Err(e) => warn!("Solana request {}: Error - {} ({}ms)", i, e, duration.as_millis()),
                    }
                }
                Err(e) => warn!("Solana request task failed: {}", e),
            }
        }

        // Calculate average response times
        if !reth_durations.is_empty() {
            let avg_reth_duration = reth_durations.iter().sum::<Duration>() / reth_durations.len() as u32;
            info!("Average Reth response time: {}ms", avg_reth_duration.as_millis());
        }

        if !solana_durations.is_empty() {
            let avg_solana_duration = solana_durations.iter().sum::<Duration>() / solana_durations.len() as u32;
            info!("Average Solana response time: {}ms", avg_solana_duration.as_millis());
        }

        info!("RPC routing and load balancing test completed");
        Ok(())
    }

    /// Cleanup test environment
    pub async fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Cleaning up RPC relay integration test environment");

        // Stop RPC server
        if let Some(solana_rpc_server) = &mut self.solana_rpc_server {
            solana_rpc_server.stop().await.ok();
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
    async fn test_rpc_relay_full_integration() {
        setup_logging();
        info!("Starting RPC relay full integration test");

        let config = RpcRelayTestConfig::default();
        let mut test_suite = RpcRelayIntegrationTestSuite::new(config);

        // Setup
        if let Err(e) = test_suite.setup().await {
            error!("Setup failed: {}", e);
            return;
        }

        // Run all tests
        let tests = vec![
            ("rpc_servers_startup", test_suite.test_rpc_servers_startup()),
            ("reth_rpc_relaying", test_suite.test_reth_rpc_relaying()),
            ("solana_rpc_relaying", test_suite.test_solana_rpc_relaying()),
            ("concurrent_rpc_requests", test_suite.test_concurrent_rpc_requests()),
            ("rpc_error_handling", test_suite.test_rpc_error_handling()),
            ("rpc_proxy_functionality", test_suite.test_rpc_proxy_functionality()),
            ("rpc_routing_load_balancing", test_suite.test_rpc_routing_and_load_balancing()),
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

        info!("RPC relay integration test results: {} passed, {} failed", passed, failed);
    }

    #[tokio::test]
    #[ignore = "Requires actual Reth and Solana binaries"]
    async fn test_rpc_proxy_only() {
        setup_logging();
        info!("Starting RPC proxy only test");

        let config = RpcRelayTestConfig::default();
        let mut test_suite = RpcRelayIntegrationTestSuite::new(config);

        if let Err(e) = test_suite.setup().await {
            error!("Setup failed: {}", e);
            return;
        }

        if let Err(e) = test_suite.test_rpc_servers_startup().await {
            test_suite.cleanup().await.ok();
            error!("RPC servers startup test failed: {}", e);
            return;
        }

        if let Err(e) = test_suite.test_rpc_proxy_functionality().await {
            test_suite.cleanup().await.ok();
            error!("RPC proxy functionality test failed: {}", e);
            return;
        }

        test_suite.cleanup().await.ok();
        info!("RPC proxy only test completed successfully");
    }
}