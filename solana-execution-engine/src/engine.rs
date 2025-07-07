use crate::config::{SolanaConfig, SolanaConnectionConfig, SolanaEngineConfig};
use crate::engine_helper::compute_block_hash;
use crate::error::SolanaEngineError;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::Signature;
use solana_sdk::{hash::Hash, slot_history::Slot, transaction::Transaction};
use std::time::Instant;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::process::{Child, Command as TokioCommand};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Solana block data type for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaBlockData {
    /// The slot number (not related to Solana Validator)
    pub slot: Slot,
    /// Block hash (not related to Solana Validator)
    pub block_hash: Hash,
    /// Transactions in this block
    pub transactions: Vec<SolanaTransaction>,
    /// Block time (not related to Solana Validator)
    pub block_time: Option<i64>,
    /// Parent slot (not related to Solana Validator)
    pub parent_slot: Slot,
    /// Previous block hash (not related to Solana Validator)
    pub previous_blockhash: Hash,
}

/// Type alias for Solana transaction to maintain naming consistency
pub type SolanaTransaction = Transaction;

/// Solana execution engine that connects to actual Solana validators
pub struct SolanaEngine {
    pub(crate) internal_client: Arc<RwLock<Option<RpcClient>>>,

    /// State tracking
    pub(crate) current_slot: Arc<RwLock<Slot>>,
    pub(crate) current_blockhash: Arc<RwLock<Hash>>,
    slots_processed: Arc<RwLock<u64>>,
    is_running: Arc<RwLock<bool>>,

    /// Process management
    solana_process: Arc<RwLock<Option<Child>>>,

    /// Engine RPC server
    pub(crate) rpc_proxy_server:
        Arc<RwLock<Option<crate::engine_rpc_server::SolanaEngineRpcServer>>>,

    /// Connection configuration
    pub(crate) solana_connection_config: SolanaConnectionConfig,
    /// Solana execution engine configuration
    pub(crate) solana_config: SolanaConfig,
    /// Solana engine configuration
    pub(crate) solana_engine_config: SolanaEngineConfig,
}

impl SolanaEngine {
    /// Create a new Solana execution engine
    pub async fn new_default() -> Result<Self, SolanaEngineError> {
        Self::new_with_config(
            SolanaEngineConfig::default(),
            SolanaConnectionConfig::default(),
            SolanaConfig::default(),
        )
        .await
    }

    /// Create a new Solana execution engine with custom configuration
    pub async fn new_with_config(
        solana_engine_config: SolanaEngineConfig,
        solana_connection_config: SolanaConnectionConfig,
        solana_config: SolanaConfig,
    ) -> Result<Self, SolanaEngineError> {
        info!(
            "Solana Private Validator Ledger path: {}",
            solana_config.ledger_path.display()
        );
        info!(
            "Solana Private Validator RPC port: {}",
            solana_config.rpc_port
        );
        info!(
            "Solana Private Validator WebSocket port: {}",
            solana_config.ws_port
        );

        Ok(Self {
            solana_process: Arc::new(RwLock::new(None)),
            internal_client: Arc::new(RwLock::new(None)),
            current_slot: Arc::new(RwLock::new(0)),
            slots_processed: Arc::new(RwLock::new(0)),
            is_running: Arc::new(RwLock::new(false)),
            current_blockhash: Arc::new(RwLock::new(Hash::default())),
            rpc_proxy_server: Arc::new(RwLock::new(None)),
            solana_connection_config,
            solana_config,
            solana_engine_config,
        })
    }

    /// Initialize the Solana engine
    pub async fn initialize(&mut self) -> Result<(), SolanaEngineError> {
        info!("Initializing Solana execution engine");

        // Start the Solana validator process
        self.start_solana_solana_process().await?;

        // Initialize RPC clients
        self.init_rpc_clients().await?;

        // Start the RPC proxy server
        self.start_rpc_proxy_server().await?;

        // Start health monitoring
        self.start_health_monitoring().await;

        *self.is_running.write().await = true;

        info!("Solana execution engine initialized successfully");
        Ok(())
    }

    /// Start the Solana Private Validator process with simplified configuration
    pub async fn start_solana_solana_process(&self) -> Result<(), SolanaEngineError> {
        info!("Starting Solana Private Validator process");

        // Get the path to the solana-private-validator binary
        let binary_path = self.get_solana_private_validator_path()?;

        // Create ledger directory
        std::fs::create_dir_all(&self.solana_config.ledger_path).map_err(|e| {
            SolanaEngineError::Configuration(format!("Failed to create ledger directory: {e}"))
        })?;

        let mut cmd = TokioCommand::new(binary_path);
        cmd
            // Gossip configuration
            .arg("--gossip-host")
            .arg(&self.solana_engine_config.rpc_server_host)
            .arg("--gossip-port")
            .arg(self.solana_config.gossip_port.to_string())
            // RPC configuration
            .arg("--rpc-port")
            .arg(self.solana_config.rpc_port.to_string())
            // Configuration and ledger paths
            .arg("--ledger")
            .arg(&self.solana_config.ledger_path)
            // Timing configuration
            .arg("--ticks-per-slot")
            .arg(self.solana_config.ticks_per_slot.to_string())
            .arg("--deterministic");

        if self.solana_config.reset {
            cmd.arg("--reset");
        }

        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);

        debug!("Solana Private Validator command: {:?}", cmd);
        info!(
            "Validator output will be logged to: {}",
            self.solana_config
                .ledger_path
                .join("validator.log")
                .display()
        );

        let child = cmd.spawn().map_err(|e| {
            SolanaEngineError::Process(format!("Failed to start Solana Private Validator: {e}"))
        })?;

        let pid = child.id();
        *self.solana_process.write().await = Some(child);

        info!(
            "Started Solana Private Validator process with PID: {:?}",
            pid
        );

        // Wait for validator to initialize with progress bar
        info!("Waiting for Solana to initialize...");
        let pb = ProgressBar::new(15);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}s {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message("Initializing Solana");

        for i in 0..15 {
            pb.set_position(i);
            pb.set_message(format!("remaining {}s ...", 15 - i));
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        pb.set_position(15);
        pb.finish_with_message("Complete!");

        Ok(())
    }

    /// Get the path to the solana-private-validator binary
    fn get_solana_private_validator_path(&self) -> Result<PathBuf, SolanaEngineError> {
        // Get current working directory for error reporting
        let current_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string());

        // Try different possible paths for the binary
        let paths = [
            PathBuf::from("target/release/solana-private-validator"),
            PathBuf::from("target/debug/solana-private-validator"),
            PathBuf::from("../target/release/solana-private-validator"),
            PathBuf::from("../target/debug/solana-private-validator"),
        ];

        for path in &paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // If none found, return error with current directory info
        Err(SolanaEngineError::Configuration(
            format!(
                "solana-private-validator binary not found. Please build it first. Searched in directory: {} (tried paths: {})",
                current_dir,
                paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")
            ),
        ))
    }

    /// Initialize RPC clients
    async fn init_rpc_clients(&self) -> Result<(), SolanaEngineError> {
        info!("Initializing Solana RPC clients");

        let rpc_url = format!(
            "http://{}:{}",
            self.solana_engine_config.rpc_server_host, self.solana_config.rpc_port
        );

        // Create RPC client
        let commitment = CommitmentConfig {
            commitment: self.solana_connection_config.commitment_level,
        };

        let rpc_client = RpcClient::new_with_commitment(rpc_url.clone(), commitment);

        *self.internal_client.write().await = Some(rpc_client);

        info!("Solana RPC clients initialized successfully");
        Ok(())
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&self) {
        let rpc_client = self.internal_client.clone();
        let interval = self.solana_connection_config.health_check_interval;

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
            }
        });

        info!("Health monitoring started");
    }

    /// Replay a block using the Solana validator
    /// Blocks must be received in sequential order (slot n+1 after slot n)
    /// Returns true if replay was successful, false otherwise
    pub async fn replay_block(
        &mut self,
        block: SolanaBlockData,
    ) -> Result<bool, SolanaEngineError> {
        let start_time = Instant::now();
        let slot = block.slot;
        let block_hash = block.block_hash;

        // Validate block sequence - blocks must be received in order
        let current_slot = *self.current_slot.read().await;
        let expected_slot = current_slot + 1;

        if slot != expected_slot {
            return Err(SolanaEngineError::Configuration(format!(
                "Block sequence error: expected slot {}, but received slot {}. Blocks must be received in sequential order.",
                expected_slot, slot
            )));
        }

        // Validate block hash by computing it ourselves
        let previous_blockhash = *self.current_blockhash.read().await;
        let block_time = block.block_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        let computed_hash =
            compute_block_hash(&block.transactions, slot, previous_blockhash, block_time);

        if computed_hash != block_hash {
            return Err(SolanaEngineError::Configuration(format!(
                "Block hash verification failed for slot {}: expected {:?}, but computed {:?}",
                slot, block_hash, computed_hash
            )));
        }

        info!(
            "Replaying Solana block for slot {} with verified hash {:?} via validator",
            slot, block_hash
        );

        // Submit block to Solana via RPC
        match self.submit_block_to_validator(&block).await {
            Ok(_) => {
                // Update metrics
                let mut current_slot = self.current_slot.write().await;
                *current_slot = slot;

                let mut slots_processed = self.slots_processed.write().await;
                *slots_processed += 1;

                let processing_time = start_time.elapsed();
                let transactions_count = block.transactions.len();

                info!(
                    "Successfully replayed Solana block {} in {:?} with {} transactions",
                    slot, processing_time, transactions_count
                );

                Ok(true)
            }
            Err(e) => {
                error!("Failed to replay block {}: {}", slot, e);
                Ok(false)
            }
        }
    }

    /// Submit block to Solana validator via RPC
    async fn submit_block_to_validator(
        &self,
        block: &SolanaBlockData,
    ) -> Result<(), SolanaEngineError> {
        debug!("Submitting Solana block for slot {} via RPC", block.slot);

        let client_guard = self.internal_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        let mut successful_txs = 0;
        let mut failed_txs = 0;

        // Submit each transaction to the validator
        for (i, tx_data) in block.transactions.iter().enumerate() {
            debug!("Submitting transaction {} for slot {}", i, block.slot);

            self.submit_transaction_to_validator(client, tx_data)
                .await
                .map(|signature| {
                    debug!(
                        "Transaction {} submitted successfully with signature: {}",
                        i, signature
                    );
                    successful_txs += 1;
                })
                .unwrap_or_else(|e| {
                    warn!("Failed to submit transaction {}: {}", i, e);
                    failed_txs += 1;
                    // Continue with other transactions rather than failing the entire block
                });
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

    /// Submit a transaction to the Solana Private Validator
    async fn submit_transaction_to_validator(
        &self,
        client: &RpcClient,
        transaction: &SolanaTransaction,
    ) -> Result<Signature, SolanaEngineError> {
        debug!(
            "Submitting Solana transaction with signatures: {:?}",
            transaction.signatures
        );

        // Submit transaction to the Solana Private Validator
        let signature = client
            .send_and_confirm_transaction(transaction)
            .await
            .map_err(|e| {
                error!("Failed to submit transaction: {}", e);
                SolanaEngineError::Transaction(format!("Transaction submission failed: {e}"))
            })?;

        info!("Successfully submitted transaction: {}", signature);
        Ok(signature)
    }

    /// Gracefully shutdown the Solana engine
    pub async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), SolanaEngineError> {
        info!("Shutting down Solana execution engine");

        *self.is_running.write().await = false;

        // Stop the RPC proxy server first
        self.stop_rpc_proxy_server().await?;

        // Clear clients
        *self.internal_client.write().await = None;

        // Stop the Solana Private Validator process
        if let Some(mut child) = self.solana_process.write().await.take() {
            info!("Terminating Solana Private Validator");

            // Try graceful shutdown first
            child.kill().await.unwrap_or_else(|e| {
                warn!("Failed to kill Solana Private Validator: {}", e);
            });

            // Wait for it to exit
            let wait_timeout = timeout.unwrap_or(Duration::from_secs(10));
            let status = tokio::time::timeout(wait_timeout, child.wait()).await??;
            info!("Solana Private Validator exited with status: {:?}", status);
        }

        Ok(())
    }
    /// Create a block by submitting multiple signed transactions to the validator via RPC
    ///
    /// This method takes a mutable slice of signed Solana transactions and submits them
    /// to the validator one by one in the order they appear in the slice, then creates
    /// and returns a SolanaBlockData containing the submitted transactions.
    ///
    /// # Arguments
    /// * `transactions` - A mutable slice of signed Transaction objects to submit
    ///
    /// # Returns
    /// * `Ok(SolanaBlockData)` - A block containing the successfully submitted transactions
    /// * `Err(SolanaEngineError)` - If RPC client is not initialized or other errors occur
    ///
    /// # Example
    /// ```rust
    /// let mut transactions = vec![signed_tx1, signed_tx2, signed_tx3];
    /// let block = engine.create_block(&mut transactions).await?;
    /// ```
    pub async fn create_block(
        &self,
        transactions: &mut [Transaction],
    ) -> Result<SolanaBlockData, SolanaEngineError> {
        info!(
            "Submitting {} transactions to validator via RPC",
            transactions.len()
        );

        if transactions.is_empty() {
            // Return an empty block if no transactions
            let current_slot = *self.current_slot.read().await;
            let next_slot = current_slot + 1;
            let previous_blockhash = *self.current_blockhash.read().await;

            // Get timestamp once for consistency
            let block_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let block_hash = compute_block_hash(&[], next_slot, previous_blockhash, block_time);

            // Update current slot and previous block hash
            *self.current_slot.write().await = next_slot;
            *self.current_blockhash.write().await = block_hash;

            return Ok(SolanaBlockData {
                slot: next_slot,
                block_hash,
                parent_slot: current_slot,
                transactions: Vec::new(),
                block_time: Some(block_time),
                previous_blockhash,
            });
        }

        let client_guard = self.internal_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        let mut signatures = Vec::with_capacity(transactions.len());
        let mut successful_transactions = Vec::new();
        let mut successful_count = 0;
        let mut failed_count = 0;

        // Submit each transaction in sequence
        for (index, transaction) in transactions.iter().enumerate() {
            match client.send_and_confirm_transaction(transaction).await {
                Ok(signature) => {
                    info!(
                        "Transaction {} submitted successfully with signature: {}",
                        index + 1,
                        signature
                    );
                    signatures.push(signature.to_string());
                    successful_transactions.push(transaction.clone());
                    successful_count += 1;
                }
                Err(e) => {
                    error!("Failed to submit transaction {}: {}", index + 1, e);
                    failed_count += 1;
                    // Continue with other transactions rather than failing entirely
                }
            }
        }

        info!(
            "Successfully submitted {} transactions to validator ({} failed)",
            successful_count, failed_count
        );

        // Get current slot and create next slot
        let current_slot = *self.current_slot.read().await;
        let next_slot = current_slot + 1;

        // Get previous block hash
        let previous_blockhash = *self.current_blockhash.read().await;

        // Get timestamp once for consistency
        let block_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Create block hash from successful transactions
        let block_hash = compute_block_hash(
            &successful_transactions,
            next_slot,
            previous_blockhash,
            block_time,
        );

        // Create and return the block
        let block = SolanaBlockData {
            slot: next_slot,
            block_hash,
            parent_slot: current_slot,
            transactions: successful_transactions,
            block_time: Some(block_time),
            previous_blockhash,
        };

        // Update current slot and previous block hash for next block
        *self.current_slot.write().await = next_slot;
        *self.current_blockhash.write().await = block_hash;

        info!(
            "Created block for slot {} with {} transactions and hash: {:?}",
            block.slot,
            block.transactions.len(),
            block.block_hash
        );

        Ok(block)
    }
}
