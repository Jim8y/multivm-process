//! Real Reth execution engine implementation
//!
//! This module provides the production-ready Reth node integration via the Engine API.
//! It replaces the mock implementation with actual Reth node communication for
//! production-grade EVM transaction execution and state management.

use crate::engine::{RethBlock, RethEngineError, RethExecutionResult};
use reqwest::Client;
use serde_json::{json, Value};
use alloy_consensus;
use alloy_rlp;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use rand::Rng;

/// Real Reth execution engine that connects to actual Reth nodes
pub struct RealRethEngine {
    /// Configuration
    pub(crate) data_dir: PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) engine_port: u16,
    pub(crate) chain_id: u64,

    /// Process management
    pub(crate) reth_process: Arc<RwLock<Option<Child>>>,

    /// Network clients
    pub(crate) rpc_client: Arc<RwLock<Option<Client>>>,
    pub(crate) engine_client: Arc<RwLock<Option<Client>>>,
    pub(crate) jwt_secret: Arc<RwLock<Option<String>>>,

    /// State tracking
    pub(crate) current_block: Arc<RwLock<u64>>,
    pub(crate) blocks_processed: Arc<RwLock<u64>>,
    pub(crate) _start_time: Instant,
    pub(crate) is_running: Arc<RwLock<bool>>,

    /// Connection configuration
    pub(crate) connection_config: ConnectionConfig,
}

/// Configuration for Reth node connections
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub request_timeout: Duration,
    pub health_check_interval: Duration,
    pub connection_pool_size: u32,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            request_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(10),
            connection_pool_size: 10,
        }
    }
}

impl RealRethEngine {
    /// Create a new real Reth execution engine
    pub async fn new(
        data_dir: PathBuf,
        rpc_port: u16,
        chain_id: u64,
    ) -> Result<Self, RethEngineError> {
        Self::new_with_config(data_dir, rpc_port, chain_id, ConnectionConfig::default()).await
    }

    /// Create a new real Reth execution engine with custom configuration
    pub async fn new_with_config(
        data_dir: PathBuf,
        rpc_port: u16,
        chain_id: u64,
        connection_config: ConnectionConfig,
    ) -> Result<Self, RethEngineError> {
        info!("Creating real Reth execution engine");
        info!("Data directory: {}", data_dir.display());
        info!("RPC port: {}", rpc_port);
        info!("Engine API port: {}", rpc_port + 1);
        info!("Chain ID: {}", chain_id);

        // Create directories
        std::fs::create_dir_all(&data_dir).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to create data directory: {e}"))
        })?;

        Ok(Self {
            data_dir,
            rpc_port,
            engine_port: rpc_port + 1,
            chain_id,
            reth_process: Arc::new(RwLock::new(None)),
            rpc_client: Arc::new(RwLock::new(None)),
            engine_client: Arc::new(RwLock::new(None)),
            jwt_secret: Arc::new(RwLock::new(None)),
            current_block: Arc::new(RwLock::new(0)),
            blocks_processed: Arc::new(RwLock::new(0)),
            _start_time: Instant::now(),
            is_running: Arc::new(RwLock::new(false)),
            connection_config,
        })
    }

    /// Initialize the real Reth engine
    pub async fn initialize(&mut self) -> Result<(), RethEngineError> {
        info!("Initializing real Reth execution engine");

        // Initialize database and JWT secret
        self.init_database_if_needed().await?;
        self.generate_jwt_secret().await?;

        // Start the Reth process
        self.start_reth_process().await?;

        // Initialize HTTP clients with connection pooling
        self.init_http_clients().await?;

        // Verify connections
        self.verify_connections().await?;

        // Start health monitoring
        self.start_health_monitoring().await;

        *self.is_running.write().await = true;
        info!("Real Reth execution engine initialized successfully");

        Ok(())
    }

    /// Start the Reth node process with simplified configuration
    async fn start_reth_process(&self) -> Result<(), RethEngineError> {
        info!("Starting Reth node process");

        let mut cmd = Command::new("reth");
        cmd.arg("node")
            // Data directory
            .arg("--datadir")
            .arg(&self.data_dir)
            // Engine API configuration
            .arg("--authrpc.jwtsecret")
            .arg(self.data_dir.join("jwt.hex"))
            .arg("--authrpc.addr")
            .arg("127.0.0.1")
            .arg("--authrpc.port")
            .arg(self.engine_port.to_string())
            // HTTP RPC configuration
            .arg("--http")
            .arg("--http.addr")
            .arg("127.0.0.1")
            .arg("--http.port")
            .arg(self.rpc_port.to_string())
            // Disable P2P networking for MultiVM
            .arg("--disable-discovery")
            .arg("--max-inbound-peers")
            .arg("0")
            .arg("--max-outbound-peers")
            .arg("0")
            .arg("--port")
            .arg("0")
            // Disable IPC
            .arg("--ipcdisable")
            // Use development mode to avoid genesis hash conflicts
            .arg("--dev")
            // Process management
            .kill_on_drop(true);

        debug!("Reth command: {:?}", cmd);

        let child = cmd
            .spawn()
            .map_err(|e| RethEngineError::Process(format!("Failed to start Reth node: {e}")))?;

        let pid = child.id();
        *self.reth_process.write().await = Some(child);

        info!("Started Reth node process with PID: {:?}", pid);
        info!("Reth HTTP RPC: http://127.0.0.1:{}", self.rpc_port);
        info!("Reth Engine API: http://127.0.0.1:{}", self.engine_port);

        // Wait for Reth to initialize
        tokio::time::sleep(Duration::from_secs(20)).await;

        Ok(())
    }

    /// Initialize HTTP clients with connection pooling and timeouts
    async fn init_http_clients(&self) -> Result<(), RethEngineError> {
        info!("Initializing HTTP clients");

        // Create RPC client with connection pooling
        let rpc_client = Client::builder()
            .timeout(self.connection_config.request_timeout)
            .pool_max_idle_per_host(self.connection_config.connection_pool_size as usize)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(|e| RethEngineError::Rpc(format!("Failed to create RPC client: {e}")))?;

        // Create Engine API client with authentication
        let engine_client = Client::builder()
            .timeout(self.connection_config.request_timeout)
            .pool_max_idle_per_host(self.connection_config.connection_pool_size as usize)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(|e| RethEngineError::Rpc(format!("Failed to create Engine client: {e}")))?;

        *self.rpc_client.write().await = Some(rpc_client);
        *self.engine_client.write().await = Some(engine_client);

        info!("HTTP clients initialized successfully");
        Ok(())
    }

    /// Verify connections to Reth node
    async fn verify_connections(&self) -> Result<(), RethEngineError> {
        info!("Verifying connections to Reth node");

        // Verify RPC connection
        self.verify_rpc_connection().await?;

        // Verify Engine API connection
        self.verify_engine_connection().await?;

        info!("All connections verified successfully");
        Ok(())
    }

    /// Verify RPC connection
    async fn verify_rpc_connection(&self) -> Result<(), RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        for attempt in 1..=self.connection_config.max_retries {
            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": "connection_test",
                "method": "eth_chainId",
                "params": []
            });

            match client.post(&rpc_url).json(&rpc_request).send().await {
                Ok(response) if response.status().is_success() => {
                    match response.json::<Value>().await {
                        Ok(result) => {
                            if let Some(chain_id_hex) =
                                result.get("result").and_then(|r| r.as_str())
                            {
                                let chain_id =
                                    u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16)
                                        .map_err(|e| {
                                            RethEngineError::Rpc(format!("Invalid chain ID: {e}"))
                                        })?;

                                info!("RPC connection verified, chain ID: {}", chain_id);
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse RPC response on attempt {}: {}", attempt, e)
                        }
                    }
                }
                Ok(response) => warn!(
                    "RPC returned error status on attempt {}: {}",
                    attempt,
                    response.status()
                ),
                Err(e) => warn!("RPC connection failed on attempt {}: {}", attempt, e),
            }

            if attempt < self.connection_config.max_retries {
                tokio::time::sleep(self.connection_config.retry_delay).await;
            }
        }

        Err(RethEngineError::Rpc(
            "Failed to verify RPC connection after retries".to_string(),
        ))
    }

    /// Verify Engine API connection
    async fn verify_engine_connection(&self) -> Result<(), RethEngineError> {
        let client_guard = self.engine_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("Engine client not initialized".to_string()))?;

        let jwt_secret = self
            .jwt_secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| RethEngineError::Configuration("JWT secret not loaded".to_string()))?
            .clone();

        let engine_url = format!("http://127.0.0.1:{}", self.engine_port);

        for attempt in 1..=self.connection_config.max_retries {
            // Create JWT token for authentication
            let jwt_token = self.create_jwt_token(&jwt_secret)?;

            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": "engine_test",
                "method": "engine_exchangeCapabilities",
                "params": [["engine_newPayloadV3", "engine_forkchoiceUpdatedV3", "engine_getPayloadV3"]]
            });

            match client
                .post(&engine_url)
                .header("Authorization", format!("Bearer {jwt_token}"))
                .json(&rpc_request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    info!("Engine API connection verified");
                    return Ok(());
                }
                Ok(response) => warn!(
                    "Engine API returned error status on attempt {}: {}",
                    attempt,
                    response.status()
                ),
                Err(e) => warn!("Engine API connection failed on attempt {}: {}", attempt, e),
            }

            if attempt < self.connection_config.max_retries {
                tokio::time::sleep(self.connection_config.retry_delay).await;
            }
        }

        Err(RethEngineError::Rpc(
            "Failed to verify Engine API connection after retries".to_string(),
        ))
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&self) {
        let rpc_client = self.rpc_client.clone();
        let engine_client = self.engine_client.clone();
        let jwt_secret = self.jwt_secret.clone();
        let rpc_port = self.rpc_port;
        let engine_port = self.engine_port;
        let interval = self.connection_config.health_check_interval;

        tokio::spawn(async move {
            let mut health_interval = tokio::time::interval(interval);
            loop {
                health_interval.tick().await;

                // Check RPC health
                if let Some(client) = rpc_client.read().await.as_ref() {
                    let rpc_url = format!("http://127.0.0.1:{rpc_port}");
                    let health_request = json!({
                        "jsonrpc": "2.0",
                        "id": "health_check",
                        "method": "eth_blockNumber",
                        "params": []
                    });

                    match client.post(&rpc_url).json(&health_request).send().await {
                        Ok(response) if response.status().is_success() => {
                            debug!("RPC health check: OK");
                        }
                        Ok(response) => {
                            warn!("RPC health check failed: {}", response.status());
                        }
                        Err(e) => {
                            error!("RPC health check error: {}", e);
                        }
                    }
                }

                // Check Engine API health with JWT authentication
                if let (Some(client), Some(secret)) = (
                    engine_client.read().await.as_ref(),
                    jwt_secret.read().await.as_ref(),
                ) {
                    if let Ok(jwt_token) = Self::create_jwt_token_static(secret) {
                        let engine_url = format!("http://127.0.0.1:{engine_port}");
                        let health_request = json!({
                            "jsonrpc": "2.0",
                            "id": "engine_health",
                            "method": "engine_exchangeCapabilities",
                            "params": [["engine_newPayloadV3", "engine_forkchoiceUpdatedV3"]]
                        });

                        match client
                            .post(&engine_url)
                            .header("Authorization", format!("Bearer {jwt_token}"))
                            .json(&health_request)
                            .send()
                            .await
                        {
                            Ok(response) if response.status().is_success() => {
                                debug!("Engine API health check: OK");
                            }
                            Ok(response) => {
                                warn!("Engine API health check failed: {}", response.status());
                            }
                            Err(e) => {
                                error!("Engine API health check error: {}", e);
                            }
                        }
                    }
                }
            }
        });

        info!("Health monitoring started");
    }

    /// Stop the Reth node process (following Solana pattern)
    pub async fn stop_reth_process(&self) -> Result<(), RethEngineError> {
        info!("Stopping Reth node process");

        let mut process_guard = self.reth_process.write().await;
        if let Some(mut child) = process_guard.take() {
            // Try graceful shutdown first (SIGTERM)
            if let Some(pid) = child.id() {
                info!("Sending SIGTERM to Reth process (PID: {})", pid);
                
                #[cfg(unix)]
                {
                    use tokio::process::Command;
                    let _ = Command::new("kill")
                        .arg("-TERM")
                        .arg(pid.to_string())
                        .output()
                        .await;
                }
                
                // Wait for graceful shutdown
                match tokio::time::timeout(Duration::from_secs(15), child.wait()).await {
                    Ok(Ok(status)) => {
                        info!("Reth process exited gracefully with status: {}", status);
                        return Ok(());
                    }
                    Ok(Err(e)) => {
                        warn!("Error waiting for Reth process to exit: {}", e);
                    }
                    Err(_) => {
                        warn!("Reth process did not exit gracefully within 15 seconds");
                    }
                }
            }

            // Force kill if graceful shutdown failed
            info!("Force killing Reth process");
            if let Err(e) = child.kill().await {
                warn!("Failed to force kill Reth process: {}", e);
            }

            // Wait for forced shutdown
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    info!("Reth process force killed with status: {}", status);
                }
                Ok(Err(e)) => {
                    error!("Error waiting for Reth process after force kill: {}", e);
                }
                Err(_) => {
                    error!("Reth process did not exit even after force kill");
                }
            }
        } else {
            info!("No Reth process to stop");
        }

        Ok(())
    }

    /// Shutdown with timeout (following Solana pattern)
    pub async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), RethEngineError> {
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        
        info!("Shutting down Reth execution engine with timeout: {:?}", timeout);
        
        // Mark as not running
        *self.is_running.write().await = false;

        // Stop the process
        match tokio::time::timeout(timeout, self.stop_reth_process()).await {
            Ok(result) => result,
            Err(_) => {
                error!("Shutdown timeout exceeded, force killing process");
                // Force kill any remaining process
                if let Some(mut child) = self.reth_process.write().await.take() {
                    let _ = child.kill().await;
                }
                Ok(())
            }
        }
    }

    /// Restart the Reth node process
    pub async fn restart_reth_process(&self) -> Result<(), RethEngineError> {
        info!("Restarting Reth node process");

        // Stop the existing process
        self.stop_reth_process().await?;

        // Wait a moment for cleanup
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Start a new process
        self.start_reth_process().await?;

        // Re-verify connections
        self.verify_connections().await?;

        info!("Reth node process restarted successfully");
        Ok(())
    }

    /// Check if the Reth process is running
    pub async fn is_reth_process_running(&self) -> bool {
        let mut process_guard = self.reth_process.write().await;
        if let Some(child) = process_guard.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => false, // Process has exited
                Ok(None) => true,     // Process is still running
                Err(_) => false,      // Error checking process
            }
        } else {
            false // No process
        }
    }

    /// Get Reth process PID
    pub async fn get_reth_process_pid(&self) -> Option<u32> {
        let process_guard = self.reth_process.read().await;
        process_guard.as_ref().and_then(|child| child.id())
    }

    /// Get engine status information
    pub async fn get_engine_status(&self) -> Result<Value, RethEngineError> {
        let current_block = *self.current_block.read().await;
        let blocks_processed = *self.blocks_processed.read().await;
        let is_running = *self.is_running.read().await;
        let process_running = self.is_reth_process_running().await;
        let process_pid = self.get_reth_process_pid().await;

        let status = json!({
            "engine_type": "reth",
            "is_running": is_running,
            "process_running": process_running,
            "process_pid": process_pid,
            "current_block": current_block,
            "blocks_processed": blocks_processed,
            "rpc_port": self.rpc_port,
            "engine_port": self.engine_port,
            "chain_id": self.chain_id,
            "chain_name": self.get_chain_name(),
            "data_dir": self.data_dir.display().to_string(),
            "connection_config": {
                "max_retries": self.connection_config.max_retries,
                "retry_delay_ms": self.connection_config.retry_delay.as_millis(),
                "request_timeout_ms": self.connection_config.request_timeout.as_millis(),
                "health_check_interval_ms": self.connection_config.health_check_interval.as_millis(),
                "connection_pool_size": self.connection_config.connection_pool_size
            }
        });

        Ok(status)
    }

    /// Process a block using the real Reth node
    pub async fn process_block_real(
        &mut self,
        block: RethBlock,
    ) -> Result<RethExecutionResult, RethEngineError> {
        let start_time = Instant::now();
        let block_number = block.number;
        let block_hash = block.hash_slow();

        info!(
            "Processing block {} with hash {:?} via real Reth node",
            block_number,
            hex::encode(block_hash)
        );

        // Submit block to Reth via Engine API
        self.submit_block_via_engine_api(&block).await?;

        // Update metrics
        let mut current_block = self.current_block.write().await;
        *current_block = block_number;

        let mut blocks_processed = self.blocks_processed.write().await;
        *blocks_processed += 1;

        let processing_time = start_time.elapsed();
        let transactions_count = block.body.transactions.len();
        let gas_used = block.header.gas_used;
        let state_root = block.header.state_root;

        info!(
            "Successfully processed block {} in {:?} with {} transactions, gas used: {}",
            block_number, processing_time, transactions_count, gas_used
        );

        Ok(RethExecutionResult {
            block_hash,
            block_number,
            gas_used,
            transactions_count,
            processing_time,
            state_root,
            success: true,
            error: None,
        })
    }

    /// Submit block to Reth via Engine API with proper error handling
    async fn submit_block_via_engine_api(&self, block: &RethBlock) -> Result<(), RethEngineError> {
        debug!("Submitting block {} via Engine API", block.number);

        // Create execution payload
        let execution_payload = self.create_execution_payload_v3(block)?;

        // Submit payload via engine_newPayloadV3
        let payload_response = self.submit_execution_payload_v3(&execution_payload).await?;
        debug!("Payload submission response: {:?}", payload_response);

        // Validate payload response
        self.validate_payload_response(&payload_response)?;

        // Update fork choice via engine_forkchoiceUpdatedV3
        let fork_choice_state = self.create_fork_choice_state(block)?;
        let fork_choice_response = self.update_fork_choice_v3(&fork_choice_state).await?;
        debug!("Fork choice response: {:?}", fork_choice_response);

        // Validate fork choice response
        self.validate_fork_choice_response(&fork_choice_response)?;

        info!(
            "Successfully submitted block {} via Engine API",
            block.number
        );
        Ok(())
    }

    /// Create execution payload V3 (latest Engine API version)
    fn create_execution_payload_v3(&self, block: &RethBlock) -> Result<Value, RethEngineError> {
        let transactions: Vec<String> = block
            .body
            .transactions
            .iter()
            .map(|tx| {
                // Use TxEnvelope's built-in encoding 
                let encoded = alloy_rlp::encode(tx).to_vec();
                format!("0x{}", hex::encode(encoded))
            })
            .collect();

        let payload = json!({
            "parentHash": format!("0x{}", hex::encode(block.header.parent_hash)),
            "feeRecipient": format!("0x{}", hex::encode(block.header.beneficiary)),
            "stateRoot": format!("0x{}", hex::encode(block.header.state_root)),
            "receiptsRoot": format!("0x{}", hex::encode(block.header.receipts_root)),
            "logsBloom": format!("0x{}", hex::encode(block.header.logs_bloom)),
            "prevRandao": format!("0x{}", hex::encode(block.header.mix_hash)),
            "blockNumber": format!("0x{:x}", block.header.number),
            "gasLimit": format!("0x{:x}", block.header.gas_limit),
            "gasUsed": format!("0x{:x}", block.header.gas_used),
            "timestamp": format!("0x{:x}", block.header.timestamp),
            "extraData": format!("0x{}", hex::encode(&block.header.extra_data)),
            "baseFeePerGas": format!("0x{:x}", block.header.base_fee_per_gas.unwrap_or(0)),
            "blockHash": format!("0x{}", hex::encode(block.hash_slow())),
            "transactions": transactions,
            "withdrawals": block.header.withdrawals_root.map(|_| json!([])),
            "blobGasUsed": block.header.blob_gas_used.map(|v| format!("0x{v:x}")),
            "excessBlobGas": block.header.excess_blob_gas.map(|v| format!("0x{v:x}")),
            "parentBeaconBlockRoot": block.header.parent_beacon_block_root.map(|root| format!("0x{}", hex::encode(root)))
        });

        Ok(payload)
    }

    /// Submit execution payload via engine_newPayloadV3
    async fn submit_execution_payload_v3(&self, payload: &Value) -> Result<Value, RethEngineError> {
        let client_guard = self.engine_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("Engine client not initialized".to_string()))?;

        let jwt_secret = self
            .jwt_secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| RethEngineError::Configuration("JWT secret not loaded".to_string()))?
            .clone();

        let jwt_token = self.create_jwt_token(&jwt_secret)?;

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": "engine_newPayloadV3",
            "method": "engine_newPayloadV3",
            "params": [payload, [], format!("0x{}", hex::encode([0u8; 32]))] // Empty versioned hashes and parent beacon block root
        });

        let engine_url = format!("http://127.0.0.1:{}", self.engine_port);

        for attempt in 1..=self.connection_config.max_retries {
            let result = client
                .post(&engine_url)
                .header("Authorization", format!("Bearer {jwt_token}"))
                .json(&rpc_request)
                .send()
                .await;

            match result {
                Ok(response) if response.status().is_success() => {
                    let result: Value = response.json().await.map_err(|e| {
                        RethEngineError::Rpc(format!("Failed to parse Engine API response: {e}"))
                    })?;
                    return Ok(result);
                }
                Ok(response) => {
                    let error_msg =
                        format!("Engine API returned error status: {}", response.status());
                    warn!(
                        "Engine API payload submission failed on attempt {}: {}",
                        attempt,
                        response.status()
                    );

                    if attempt == self.connection_config.max_retries {
                        return Err(RethEngineError::Rpc(error_msg));
                    }
                }
                Err(e) => {
                    let error_msg = format!("Engine API request failed: {e}");
                    warn!("Engine API request failed on attempt {}: {}", attempt, e);

                    if attempt == self.connection_config.max_retries {
                        return Err(RethEngineError::Rpc(error_msg));
                    }
                }
            }

            if attempt < self.connection_config.max_retries {
                tokio::time::sleep(self.connection_config.retry_delay).await;
            }
        }

        // This should never be reached due to the logic above, but just in case
        Err(RethEngineError::Rpc(
            "All retry attempts exhausted".to_string(),
        ))
    }

    /// Update fork choice via engine_forkchoiceUpdatedV3
    async fn update_fork_choice_v3(
        &self,
        fork_choice_state: &Value,
    ) -> Result<Value, RethEngineError> {
        let client_guard = self.engine_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("Engine client not initialized".to_string()))?;

        let jwt_secret = self
            .jwt_secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| RethEngineError::Configuration("JWT secret not loaded".to_string()))?
            .clone();

        let jwt_token = self.create_jwt_token(&jwt_secret)?;

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": "engine_forkchoiceUpdatedV3",
            "method": "engine_forkchoiceUpdatedV3",
            "params": [fork_choice_state, null]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.engine_port);

        for attempt in 1..=self.connection_config.max_retries {
            let result = client
                .post(&engine_url)
                .header("Authorization", format!("Bearer {jwt_token}"))
                .json(&rpc_request)
                .send()
                .await;

            match result {
                Ok(response) if response.status().is_success() => {
                    let result: Value = response.json().await.map_err(|e| {
                        RethEngineError::Rpc(format!("Failed to parse fork choice response: {e}"))
                    })?;
                    return Ok(result);
                }
                Ok(response) => {
                    let error_msg = format!(
                        "Fork choice update returned error status: {}",
                        response.status()
                    );
                    warn!(
                        "Fork choice update failed on attempt {}: {}",
                        attempt,
                        response.status()
                    );

                    if attempt == self.connection_config.max_retries {
                        return Err(RethEngineError::Rpc(error_msg));
                    }
                }
                Err(e) => {
                    let error_msg = format!("Fork choice update failed: {e}");
                    warn!("Fork choice request failed on attempt {}: {}", attempt, e);

                    if attempt == self.connection_config.max_retries {
                        return Err(RethEngineError::Rpc(error_msg));
                    }
                }
            }

            if attempt < self.connection_config.max_retries {
                tokio::time::sleep(self.connection_config.retry_delay).await;
            }
        }

        // This should never be reached due to the logic above, but just in case
        Err(RethEngineError::Rpc(
            "All fork choice retry attempts exhausted".to_string(),
        ))
    }

    /// Create fork choice state from block
    fn create_fork_choice_state(&self, block: &RethBlock) -> Result<Value, RethEngineError> {
        let fork_choice_state = json!({
            "headBlockHash": format!("0x{}", hex::encode(block.hash_slow())),
            "safeBlockHash": format!("0x{}", hex::encode(block.header.parent_hash)),
            "finalizedBlockHash": format!("0x{}", hex::encode(block.header.parent_hash))
        });

        Ok(fork_choice_state)
    }

    /// Validate payload response from Engine API
    fn validate_payload_response(&self, response: &Value) -> Result<(), RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API payload error: {error}"
            )));
        }

        if let Some(result) = response.get("result") {
            if let Some(status) = result.get("status").and_then(|s| s.as_str()) {
                match status {
                    "VALID" => Ok(()),
                    "INVALID" => Err(RethEngineError::Rpc(
                        "Engine API rejected payload as invalid".to_string(),
                    )),
                    "SYNCING" => Err(RethEngineError::Rpc(
                        "Engine API is syncing, cannot process payload".to_string(),
                    )),
                    _ => Err(RethEngineError::Rpc(format!(
                        "Unknown payload status: {status}"
                    ))),
                }
            } else {
                Err(RethEngineError::Rpc(
                    "Engine API response missing status field".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(
                "Engine API response missing result field".to_string(),
            ))
        }
    }

    /// Validate fork choice response from Engine API
    fn validate_fork_choice_response(&self, response: &Value) -> Result<(), RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API fork choice error: {error}"
            )));
        }

        if let Some(result) = response.get("result") {
            if let Some(payload_status) = result.get("payloadStatus") {
                if let Some(status) = payload_status.get("status").and_then(|s| s.as_str()) {
                    match status {
                        "VALID" => Ok(()),
                        "INVALID" => Err(RethEngineError::Rpc(
                            "Engine API rejected fork choice as invalid".to_string(),
                        )),
                        "SYNCING" => Err(RethEngineError::Rpc(
                            "Engine API is syncing, cannot update fork choice".to_string(),
                        )),
                        _ => Err(RethEngineError::Rpc(format!(
                            "Unknown fork choice status: {status}"
                        ))),
                    }
                } else {
                    Err(RethEngineError::Rpc(
                        "Fork choice response missing status field".to_string(),
                    ))
                }
            } else {
                Err(RethEngineError::Rpc(
                    "Fork choice response missing payloadStatus field".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(
                "Fork choice response missing result field".to_string(),
            ))
        }
    }

    // Note: Helper methods are implemented in real_engine_utils.rs
}
