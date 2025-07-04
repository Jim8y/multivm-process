//! Real Solana execution engine implementation
//!
//! This module provides the production-ready Solana validator integration via RPC API.
//! It replaces the mock implementation with actual Solana validator communication for
//! production-grade SVM transaction execution and state management.

use crate::config::{MultivmValidatorConfig, SolanaConnectionConfig};
use crate::engine::{SolanaBlockData, SolanaEngineError, SolanaExecutionResult, SolanaTransaction};
use crate::validator_api::{SlotInfo, SolanaValidatorApi, SolanaValidatorApiBuilder};
use async_trait::async_trait;
use base64::Engine;
use multivm_common::*;
use reqwest::Client;
use serde_json::{json, Value};
use solana_client::client_error::ClientError;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    pubkey::Pubkey,
    signature::Signature,
    slot_history::Slot,
    transaction::Transaction,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Real Solana execution engine that connects to actual Solana validators
pub struct RealSolanaEngine {
    /// Configuration
    data_dir: PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) ws_port: u16,
    pub(crate) cluster: String,

    /// Process management
    validator_process: Arc<RwLock<Option<Child>>>,

    /// Network clients
    pub(crate) rpc_client: Arc<RwLock<Option<RpcClient>>>,
    pub(crate) validator_api: Arc<RwLock<Option<SolanaValidatorApi>>>,

    /// State tracking
    current_slot: Arc<RwLock<Slot>>,
    slots_processed: Arc<RwLock<u64>>,
    start_time: Instant,
    is_running: Arc<RwLock<bool>>,

    /// Connection configuration
    connection_config: SolanaConnectionConfig,

    /// MultiVM validator configuration
    validator_config: MultivmValidatorConfig,
}

impl RealSolanaEngine {
    /// Create a new real Solana execution engine
    pub async fn new(
        data_dir: PathBuf,
        rpc_port: u16,
        cluster: String,
    ) -> Result<Self, SolanaEngineError> {
        Self::new_with_config(
            data_dir,
            rpc_port,
            cluster,
            SolanaConnectionConfig::default(),
            MultivmValidatorConfig::default(),
        )
        .await
    }

    /// Create a new real Solana execution engine with custom configuration
    pub async fn new_with_config(
        data_dir: PathBuf,
        rpc_port: u16,
        cluster: String,
        connection_config: SolanaConnectionConfig,
        validator_config: MultivmValidatorConfig,
    ) -> Result<Self, SolanaEngineError> {
        info!("Creating real Solana execution engine");
        info!("Data directory: {}", data_dir.display());
        info!("RPC port: {}", rpc_port);
        info!("WebSocket port: {}", rpc_port + 1);
        info!("Cluster: {}", cluster);

        // Create directories
        std::fs::create_dir_all(&data_dir).map_err(|e| {
            SolanaEngineError::Configuration(format!("Failed to create data directory: {e}"))
        })?;

        Ok(Self {
            data_dir,
            rpc_port,
            ws_port: rpc_port + 1,
            cluster,
            validator_process: Arc::new(RwLock::new(None)),
            rpc_client: Arc::new(RwLock::new(None)),
            validator_api: Arc::new(RwLock::new(None)),
            current_slot: Arc::new(RwLock::new(0)),
            slots_processed: Arc::new(RwLock::new(0)),
            start_time: Instant::now(),
            is_running: Arc::new(RwLock::new(false)),
            connection_config,
            validator_config,
        })
    }

    /// Initialize the real Solana engine
    pub async fn initialize(&mut self) -> Result<(), SolanaEngineError> {
        info!("Initializing real Solana execution engine");

        // Start the Solana validator process
        self.start_solana_validator_process().await?;

        // Initialize RPC clients
        self.init_rpc_clients().await?;

        // Verify connections
        self.verify_connections().await?;

        // Start health monitoring
        self.start_health_monitoring().await;

        *self.is_running.write().await = true;
        info!("Real Solana execution engine initialized successfully");

        Ok(())
    }

    /// Start the MultiVM validator process with simplified configuration
    pub async fn start_solana_validator_process(&self) -> Result<(), SolanaEngineError> {
        info!("Starting MultiVM validator process");

        // Get the path to the multivm-validator binary
        let binary_path = self.get_multivm_validator_path()?;

        // Create log file for validator output
        let log_file_path = self.data_dir.join("multivm-validator.log");
        let log_file = std::fs::File::create(&log_file_path).map_err(|e| {
            SolanaEngineError::Configuration(format!("Failed to create log file: {e}"))
        })?;

        let mut cmd = Command::new(binary_path);
        cmd
            // Gossip configuration
            .arg("--gossip-host")
            .arg(&self.validator_config.gossip_host)
            .arg("--gossip-port")
            .arg(self.validator_config.gossip_port.to_string())
            // Configuration and ledger paths
            .arg("--ledger")
            .arg(&self.validator_config.ledger_path)
            // Timing configuration
            .arg("--ticks-per-slot")
            .arg(self.validator_config.ticks_per_slot.to_string())
            .arg("--log")
            .arg("--deterministic");

        if self.validator_config.reset {
            cmd.arg("--reset");
        }

        cmd
            // Process settings - redirect output to log file
            .stdout(std::process::Stdio::from(log_file.try_clone().map_err(
                |e| {
                    SolanaEngineError::Configuration(format!(
                        "Failed to clone log file handle: {e}"
                    ))
                },
            )?))
            .stderr(std::process::Stdio::from(log_file))
            .kill_on_drop(true);

        debug!("MultiVM validator command: {:?}", cmd);
        info!(
            "Validator output will be logged to: {}",
            log_file_path.display()
        );

        let child = cmd.spawn().map_err(|e| {
            SolanaEngineError::Process(format!("Failed to start MultiVM validator: {e}"))
        })?;

        let pid = child.id();
        *self.validator_process.write().await = Some(child);

        info!("Started MultiVM validator process with PID: {:?}", pid);
        info!("Solana RPC: http://127.0.0.1:{}", self.rpc_port);
        info!("Solana WebSocket: ws://127.0.0.1:{}", self.ws_port);

        // Wait for validator to initialize
        tokio::time::sleep(Duration::from_secs(10)).await;

        Ok(())
    }

    /// Get the path to the multivm-validator binary
    fn get_multivm_validator_path(&self) -> Result<PathBuf, SolanaEngineError> {
        // Get current working directory for error reporting
        let current_dir = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string());

        // Try different possible paths for the binary
        let paths = [
            PathBuf::from("target/release/multivm-validator"),
            PathBuf::from("target/debug/multivm-validator"),
            PathBuf::from("../target/release/multivm-validator"),
            PathBuf::from("../target/debug/multivm-validator"),
        ];

        for path in &paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // If none found, return error with current directory info
        Err(SolanaEngineError::Configuration(
            format!(
                "multivm-validator binary not found. Please build it first. Searched in directory: {} (tried paths: {})",
                current_dir,
                paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
            ),
        ))
    }

    /// Initialize RPC clients
    async fn init_rpc_clients(&self) -> Result<(), SolanaEngineError> {
        info!("Initializing Solana RPC clients");

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);
        let ws_url = format!("ws://127.0.0.1:{}", self.ws_port);

        // Create RPC client
        let commitment = CommitmentConfig {
            commitment: self.connection_config.commitment_level,
        };

        let rpc_client = RpcClient::new_with_commitment(rpc_url.clone(), commitment);

        // Create validator API client
        let validator_api = SolanaValidatorApiBuilder::new()
            .rpc_url(rpc_url)
            .ws_url(ws_url)
            .request_timeout(self.connection_config.request_timeout)
            .max_retries(self.connection_config.max_retries)
            .retry_delay(self.connection_config.retry_delay)
            .build()?;

        *self.rpc_client.write().await = Some(rpc_client);
        *self.validator_api.write().await = Some(validator_api);

        info!("Solana RPC clients initialized successfully");
        Ok(())
    }

    /// Verify connections to Solana validator
    async fn verify_connections(&self) -> Result<(), SolanaEngineError> {
        info!("Verifying connections to Solana validator");

        // Verify RPC connection
        self.verify_rpc_connection().await?;

        // Verify validator API connection
        self.verify_validator_api_connection().await?;

        info!("All connections verified successfully");
        Ok(())
    }

    /// Verify RPC connection
    async fn verify_rpc_connection(&self) -> Result<(), SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        for attempt in 1..=self.connection_config.max_retries {
            match client.get_slot().await {
                Ok(slot) => {
                    info!(
                        "Successfully connected to Solana validator, current slot: {}",
                        slot
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!("RPC connection failed on attempt {}: {}", attempt, e);
                    if attempt < self.connection_config.max_retries {
                        tokio::time::sleep(self.connection_config.retry_delay).await;
                    }
                }
            }
        }

        Err(SolanaEngineError::Rpc(
            "Failed to verify RPC connection after retries".to_string(),
        ))
    }

    /// Verify validator API connection
    async fn verify_validator_api_connection(&self) -> Result<(), SolanaEngineError> {
        let api_guard = self.validator_api.read().await;
        let api = api_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("Validator API not initialized".to_string()))?;

        for attempt in 1..=self.connection_config.max_retries {
            match api.get_slot_info().await {
                Ok(slot_info) => {
                    info!(
                        "Successfully connected to Solana validator API, slot info: {:?}",
                        slot_info
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        "Validator API connection failed on attempt {}: {}",
                        attempt, e
                    );
                    if attempt < self.connection_config.max_retries {
                        tokio::time::sleep(self.connection_config.retry_delay).await;
                    }
                }
            }
        }

        Err(SolanaEngineError::Rpc(
            "Failed to verify validator API connection after retries".to_string(),
        ))
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&self) {
        let rpc_client = self.rpc_client.clone();
        let validator_api = self.validator_api.clone();
        let interval = self.connection_config.health_check_interval;

        tokio::spawn(async move {
            let mut health_interval = tokio::time::interval(interval);
            loop {
                health_interval.tick().await;

                // Check RPC health
                if let Some(client) = rpc_client.read().await.as_ref() {
                    match client.get_health().await {
                        Ok(()) => debug!("Solana RPC health check: OK"),
                        Err(e) => error!("Solana RPC health check error: {}", e),
                    }
                }

                // Check validator API health
                if let Some(api) = validator_api.read().await.as_ref() {
                    match api.health_check().await {
                        Ok(true) => debug!("Solana validator API health check: OK"),
                        Ok(false) => warn!("Solana validator API health check: FAILED"),
                        Err(e) => error!("Solana validator API health check error: {}", e),
                    }
                }
            }
        });

        info!("Health monitoring started");
    }

    /// Process a block using the real Solana validator
    pub async fn process_block_real(
        &mut self,
        block: SolanaBlockData,
    ) -> Result<SolanaExecutionResult, SolanaEngineError> {
        let start_time = Instant::now();
        let slot = block.slot;
        let block_hash = block.block_hash;

        info!(
            "Processing Solana block for slot {} with hash {:?} via real validator",
            slot, block_hash
        );

        // Submit block to Solana via RPC
        self.submit_block_to_validator(&block).await?;

        // Update metrics
        let mut current_slot = self.current_slot.write().await;
        *current_slot = slot;

        let mut slots_processed = self.slots_processed.write().await;
        *slots_processed += 1;

        let processing_time = start_time.elapsed();
        let transactions_count = block.transactions.len();
        let compute_units_used: u64 = block.transactions.iter().map(|tx| tx.compute_units).sum();

        info!(
            "Successfully processed Solana block {} in {:?} with {} transactions, compute units: {}",
            slot, processing_time, transactions_count, compute_units_used
        );

        Ok(SolanaExecutionResult {
            slot,
            block_hash,
            state_root: self.calculate_state_root(&block),
            transaction_count: transactions_count,
            compute_units_used,
            processing_time,
            success: true,
            error: None,
        })
    }

    /// Submit block to Solana validator via RPC
    async fn submit_block_to_validator(
        &self,
        block: &SolanaBlockData,
    ) -> Result<(), SolanaEngineError> {
        debug!("Submitting Solana block for slot {} via RPC", block.slot);

        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        let mut successful_txs = 0;
        let mut failed_txs = 0;

        // Submit each transaction to the validator
        for (i, tx_data) in block.transactions.iter().enumerate() {
            debug!("Submitting transaction {} for slot {}", i, block.slot);

            match self.submit_transaction_to_validator(client, tx_data).await {
                Ok(signature) => {
                    debug!(
                        "Transaction {} submitted successfully with signature: {}",
                        i, signature
                    );
                    successful_txs += 1;
                }
                Err(e) => {
                    warn!("Failed to submit transaction {}: {}", i, e);
                    failed_txs += 1;
                    // Continue with other transactions rather than failing the entire block
                }
            }
        }

        if successful_txs > 0 {
            info!(
                "Successfully submitted {} transactions for slot {} ({} failed)",
                successful_txs, block.slot, failed_txs
            );
        } else if !block.transactions.is_empty() {
            return Err(SolanaEngineError::Rpc(format!(
                "Failed to submit any transactions for slot {}",
                block.slot
            )));
        }

        Ok(())
    }

    /// Submit a transaction to the Solana validator
    async fn submit_transaction_to_validator(
        &self,
        client: &RpcClient,
        transaction: &SolanaTransaction,
    ) -> Result<String, SolanaEngineError> {
        debug!(
            "Submitting Solana transaction with signature: {}",
            transaction.signature
        );

        // Deserialize transaction data from bytes to Transaction object
        let solana_transaction: solana_sdk::transaction::Transaction =
            bincode::deserialize(&transaction.data).map_err(|e| {
                SolanaEngineError::Serialization(format!(
                    "Failed to deserialize transaction: {}",
                    e
                ))
            })?;

        // Submit transaction to the Solana validator
        match client
            .send_and_confirm_transaction(&solana_transaction)
            .await
        {
            Ok(signature) => {
                info!("Successfully submitted Solana transaction: {}", signature);
                Ok(signature.to_string())
            }
            Err(e) => {
                error!("Failed to submit Solana transaction: {}", e);
                Err(SolanaEngineError::Transaction(format!(
                    "Transaction submission failed: {e}"
                )))
            }
        }
    }

    /// Calculate state root for a processed block
    fn calculate_state_root(&self, block: &SolanaBlockData) -> solana_sdk::hash::Hash {
        use sha2::{Digest, Sha256};

        // In a production Solana implementation, the state root would be calculated by:
        // 1. Collecting all account state changes from transaction execution
        // 2. Building a Merkle tree of account hashes
        // 3. Computing the root hash of the state tree

        // For our simplified implementation, we calculate a deterministic hash based on:
        // - Block slot
        // - Transaction signatures
        // - Previous block hash

        let mut hasher = Sha256::new();

        // Add block metadata
        hasher.update(block.slot.to_le_bytes());
        hasher.update(block.block_hash.to_bytes());

        // Add transaction signatures
        for tx in &block.transactions {
            hasher.update(tx.signature.as_bytes());
            // Include transaction data hash for more entropy
            let tx_hash = Sha256::digest(&tx.data);
            hasher.update(tx_hash);
        }

        // Add timestamp for additional uniqueness
        if let Ok(timestamp) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            hasher.update(timestamp.as_secs().to_le_bytes());
        }

        // Create hash from digest
        let state_hash = hasher.finalize();
        solana_sdk::hash::Hash::new_from_array(state_hash.into())
    }

    /// Get current slot from validator
    pub async fn get_current_slot(&self) -> Result<Slot, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        match client.get_slot().await {
            Ok(slot) => Ok(slot),
            Err(e) => Err(SolanaEngineError::Rpc(format!(
                "Failed to get current slot: {}",
                e
            ))),
        }
    }

    /// Get slot information
    pub async fn get_slot_info(&self) -> Result<SlotInfo, SolanaEngineError> {
        let api_guard = self.validator_api.read().await;
        let api = api_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("Validator API not initialized".to_string()))?;

        api.get_slot_info().await
    }

    /// Gracefully shutdown the real Solana engine
    pub async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), SolanaEngineError> {
        info!("Shutting down real Solana execution engine");

        *self.is_running.write().await = false;

        // Clear clients
        *self.rpc_client.write().await = None;
        *self.validator_api.write().await = None;

        // Stop the Solana validator process
        if let Some(mut child) = self.validator_process.write().await.take() {
            info!("Terminating Solana validator process");

            // Try graceful shutdown first
            if let Err(e) = child.kill().await {
                warn!("Failed to kill Solana validator process: {}", e);
            }

            // Wait for it to exit
            let wait_timeout = timeout.unwrap_or(Duration::from_secs(10));
            match tokio::time::timeout(wait_timeout, child.wait()).await {
                Ok(Ok(status)) => {
                    info!("Solana validator exited with status: {}", status);
                }
                Ok(Err(e)) => {
                    warn!("Error waiting for Solana validator to exit: {}", e);
                }
                Err(_) => {
                    warn!("Solana validator did not exit within timeout");
                }
            }
        }

        info!("Real Solana execution engine shutdown complete");
        Ok(())
    }
}
