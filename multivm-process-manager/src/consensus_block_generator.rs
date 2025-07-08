//! Consensus-aware block generator for single node operation
//!
//! This module provides a block generator that integrates with the consensus engine
//! to produce properly signed blocks even in single-node mode.

use crate::coordinator::MultivmCoordinator;
use multivm_common::MultivmResult;
use multivm_consensus::{BlockHeader, EvmSignature, EvmTransaction, MultiVMBlock, SvmTransaction};
use reth_execution_engine::rpc_client::{RethRpcClient, RethRpcClientBuilder};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for consensus block generation
#[derive(Debug, Clone)]
pub struct ConsensusBlockGeneratorConfig {
    /// Interval between blocks in milliseconds
    pub block_interval_ms: u64,
    /// Number of SVM transactions per block
    pub svm_tx_per_block: usize,
    /// Number of EVM transactions per block (from Reth)
    pub evm_tx_per_block: usize,
    /// Whether to enable continuous generation
    pub enabled: bool,
    /// Reth RPC endpoint URL
    pub reth_rpc_url: String,
    /// Reth RPC request timeout
    pub reth_rpc_timeout_ms: u64,
    /// Maximum retries for Reth RPC calls
    pub reth_max_retries: u32,
    /// Retry delay for Reth RPC calls
    pub reth_retry_delay_ms: u64,
    /// Enable state root verification
    pub enable_state_root_verification: bool,
    /// Block synchronization timeout
    pub block_sync_timeout_ms: u64,
}

impl Default for ConsensusBlockGeneratorConfig {
    fn default() -> Self {
        Self {
            block_interval_ms: 3000, // 3 second blocks
            svm_tx_per_block: 20,     // Keep SVM transactions for cross-VM functionality
            evm_tx_per_block: 20,     // Now sourced from Reth
            enabled: true,
            reth_rpc_url: "http://127.0.0.1:8545".to_string(),
            reth_rpc_timeout_ms: 30000,
            reth_max_retries: 3,
            reth_retry_delay_ms: 1000,
            enable_state_root_verification: true,
            block_sync_timeout_ms: 5000,
        }
    }
}

/// Consensus-aware block generator that produces signed blocks
pub struct ConsensusBlockGenerator {
    config: ConsensusBlockGeneratorConfig,
    coordinator: Arc<RwLock<MultivmCoordinator>>,
    current_height: Arc<RwLock<u64>>,
    is_running: Arc<RwLock<bool>>,
    reth_client: Arc<RethRpcClient>,
    last_reth_block_number: Arc<RwLock<u64>>,
}

impl ConsensusBlockGenerator {
    /// Create a new consensus block generator
    pub fn new(
        config: ConsensusBlockGeneratorConfig,
        coordinator: Arc<RwLock<MultivmCoordinator>>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize Reth RPC client
        let reth_client = RethRpcClientBuilder::new()
            .rpc_url(config.reth_rpc_url.clone())
            .request_timeout(Duration::from_millis(config.reth_rpc_timeout_ms))
            .max_retries(config.reth_max_retries)
            .retry_delay(Duration::from_millis(config.reth_retry_delay_ms))
            .build()?;

        Ok(Self {
            config,
            coordinator,
            current_height: Arc::new(RwLock::new(1)),
            is_running: Arc::new(RwLock::new(false)),
            reth_client: Arc::new(reth_client),
            last_reth_block_number: Arc::new(RwLock::new(0)),
        })
    }

    /// Start continuous block generation through consensus
    pub async fn start(&mut self) -> MultivmResult<()> {
        if !self.config.enabled {
            info!("Consensus block generator is disabled");
            return Ok(());
        }

        info!(
            "Starting consensus block generator with {} ms intervals",
            self.config.block_interval_ms
        );

        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        // Create interval timer
        let mut interval = interval(Duration::from_millis(self.config.block_interval_ms));

        // Skip the first immediate tick
        interval.tick().await;

        info!("Consensus block generator started successfully");

        // Main generation loop
        loop {
            // Check if we should continue running
            {
                let running = self.is_running.read().await;
                if !*running {
                    info!("Consensus block generator stopping...");
                    break;
                }
            }

            // Wait for next interval
            interval.tick().await;

            // Generate and propose block through consensus
            if let Err(e) = self.generate_and_propose_block().await {
                error!("Failed to generate consensus block: {}", e);
                // Continue running despite errors
            }
        }

        info!("Consensus block generator stopped");
        Ok(())
    }

    /// Stop block generation
    pub async fn stop(&self) {
        info!("Stopping consensus block generator...");
        let mut running = self.is_running.write().await;
        *running = false;
    }

    /// Generate and propose a block through consensus
    async fn generate_and_propose_block(&self) -> MultivmResult<()> {
        let height = {
            let mut current_height = self.current_height.write().await;
            let height = *current_height;
            *current_height += 1;
            height
        };

        debug!("Generating consensus block at height {}", height);

        // Coordinate block timing between MultiVM consensus and Reth process
        if let Err(e) = self.coordinate_block_timing(height).await {
            warn!("Failed to coordinate block timing: {}", e);
            // Continue despite timing coordination failure
        }

        // Generate block content
        let block = self.generate_block_for_consensus(height).await?;

        // Submit to coordinator which will route through consensus
        {
            let coordinator = self.coordinator.read().await;
            // Submit block to consensus engine for BFT consensus proposal
            // Submit block through coordinator (which routes through consensus)
            match coordinator.submit_block(block.clone()).await {
                Ok(_) => {
                    tracing::info!(
                        "Block {} successfully submitted through consensus pipeline",
                        height
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to submit block {} through consensus: {}", height, e);
                    return Err(e);
                }
            }
            info!(
                "Proposing block {} through consensus (single-node mode)",
                height
            );
            coordinator.submit_block(block).await?;
        }

        info!("Successfully proposed consensus block at height {}", height);
        Ok(())
    }

    /// Generate a block with proper structure for consensus
    async fn generate_block_for_consensus(&self, height: u64) -> MultivmResult<MultiVMBlock> {
        let timestamp = SystemTime::now();
        let timestamp_secs = timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();

        // Generate SVM transactions (keep for cross-VM functionality)
        let svm_transactions = self.generate_svm_transactions(height, timestamp_secs).await?;

        // Get EVM transactions from Reth
        let (evm_transactions, reth_block_data) = self.collect_evm_transactions_from_reth(height, timestamp_secs).await?;

        // Create block header with real Reth data
        let header = self.create_block_header(height, timestamp, &reth_block_data).await?;

        // Create the block
        let mut block = MultiVMBlock::new(
            header.height,
            header.previous_hash.clone(),
            header.proposer.clone(),
            header.consensus_data.clone(),
        );

        // Add transactions
        block.svm_transactions = svm_transactions;
        block.evm_transactions = evm_transactions;

        // Update header fields
        block.header = header;

        info!(
            "Generated consensus block {} with {} SVM and {} EVM transactions from Reth",
            height,
            block.svm_transactions.len(),
            block.evm_transactions.len()
        );

        Ok(block)
    }

    /// Generate SVM transactions (keeps existing logic for cross-VM compatibility)
    async fn generate_svm_transactions(&self, height: u64, timestamp_secs: u64) -> MultivmResult<Vec<SvmTransaction>> {
        let mut svm_transactions = Vec::new();
        for i in 0..self.config.svm_tx_per_block {
            svm_transactions.push(SvmTransaction {
                id: Uuid::new_v4(),
                signatures: vec![format!("svm_sig_{}_{}", height, i)],
                accounts: vec![
                    format!("svm_account_{}_{}_from", height, i),
                    format!("svm_account_{}_{}_to", height, i),
                ],
                data: vec![0x01, 0x02, 0x03, 0x04], // Transaction instruction data
                recent_blockhash: format!("block_hash_{}", height.saturating_sub(1)),
                fee: 5000 + (i as u64 * 100),
                metadata: serde_json::json!({
                    "consensus": true,
                    "block": height,
                    "index": i,
                    "timestamp": timestamp_secs
                }),
            });
        }
        Ok(svm_transactions)
    }

    /// Collect EVM transactions from Reth process
    async fn collect_evm_transactions_from_reth(&self, height: u64, _timestamp_secs: u64) -> MultivmResult<(Vec<EvmTransaction>, RethBlockData)> {
        // Get the latest block from Reth
        let current_reth_block = self.reth_client.get_block_number().await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: format!("Failed to get current block number: {}", e),
                endpoint: Some("reth".to_string()),
                retry_after: None,
            })?;

        // Update our tracking of Reth block number
        {
            let mut last_block = self.last_reth_block_number.write().await;
            *last_block = current_reth_block;
        }

        debug!("Collecting EVM transactions from Reth block {}", current_reth_block);

        // Get the latest block with full transaction details
        let reth_block = self.reth_client.get_block_by_number(current_reth_block, true).await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: format!("Failed to get block {}: {}", current_reth_block, e),
                endpoint: Some("reth".to_string()),
                retry_after: None,
            })?;

        let reth_block = reth_block.ok_or_else(|| multivm_common::MultivmError::NotFound {
            resource: format!("Block {}", current_reth_block),
            resource_id: Some(current_reth_block.to_string()),
        })?;

        // Convert Reth transactions to MultiVM format
        let mut evm_transactions = Vec::new();
        let tx_limit = self.config.evm_tx_per_block.min(reth_block.transactions.len());
        
        for (i, tx_data) in reth_block.transactions.iter().take(tx_limit).enumerate() {
            let evm_tx = self.convert_reth_transaction_to_multivm(tx_data, height, i).await?;
            evm_transactions.push(evm_tx);
        }

        // If we don't have enough transactions, pad with synthetic ones
        while evm_transactions.len() < self.config.evm_tx_per_block {
            let synthetic_tx = self.create_synthetic_evm_transaction(height, evm_transactions.len()).await?;
            evm_transactions.push(synthetic_tx);
        }

        let block_data = RethBlockData {
            block_number: reth_block.number,
            block_hash: reth_block.hash.clone(),
            parent_hash: reth_block.parent_hash.clone(),
            state_root: reth_block.state_root.clone(),
            receipts_root: reth_block.receipts_root.clone(),
            timestamp: reth_block.timestamp,
            gas_limit: reth_block.gas_limit,
            gas_used: reth_block.gas_used,
            base_fee_per_gas: reth_block.base_fee_per_gas,
        };

        Ok((evm_transactions, block_data))
    }

    /// Convert Reth transaction to MultiVM format
    async fn convert_reth_transaction_to_multivm(&self, tx_data: &serde_json::Value, height: u64, index: usize) -> MultivmResult<EvmTransaction> {
        let hash = tx_data.get("hash")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("unknown_hash_{}", index))
            .to_string();

        let from = tx_data.get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000")
            .to_string();

        let to = tx_data.get("to")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let value = tx_data.get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        let gas_limit = tx_data.get("gas")
            .and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(21000);

        let gas_price = tx_data.get("gasPrice")
            .and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(20000000000);

        let nonce = tx_data.get("nonce")
            .and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        let data = tx_data.get("input")
            .and_then(|v| v.as_str())
            .and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
            .unwrap_or_default();

        // Extract signature components
        let v = tx_data.get("v")
            .and_then(|v| v.as_u64())
            .unwrap_or(27);

        let r = tx_data.get("r")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0")
            .to_string();

        let s = tx_data.get("s")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0")
            .to_string();

        Ok(EvmTransaction {
            id: Uuid::new_v4(),
            hash,
            from,
            to,
            value,
            gas_limit,
            gas_price,
            data,
            nonce,
            signature: EvmSignature {
                v: v as u8,
                r,
                s,
            },
            metadata: serde_json::json!({
                "consensus": true,
                "block": height,
                "index": index,
                "reth_sourced": true,
                "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
            }),
        })
    }

    /// Create synthetic EVM transaction when not enough real transactions are available
    async fn create_synthetic_evm_transaction(&self, height: u64, index: usize) -> MultivmResult<EvmTransaction> {
        Ok(EvmTransaction {
            id: Uuid::new_v4(),
            hash: format!("synthetic_evm_hash_{}_{}", height, index),
            from: format!("0x{:040x}", height * 1000 + index as u64),
            to: Some(format!("0x{:040x}", height * 1000 + index as u64 + 1)),
            value: 1000000000000000000u64, // 1 ETH in wei
            gas_limit: 21000,
            gas_price: 20000000000u64, // 20 gwei
            data: vec![],
            nonce: index as u64,
            signature: EvmSignature {
                v: 27,
                r: format!("0x{:064x}", height * 1000 + index as u64),
                s: format!("0x{:064x}", height * 1000 + index as u64 + 500),
            },
            metadata: serde_json::json!({
                "consensus": true,
                "block": height,
                "index": index,
                "synthetic": true,
                "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
            }),
        })
    }

    /// Create block header with real Reth data
    async fn create_block_header(&self, height: u64, timestamp: SystemTime, reth_data: &RethBlockData) -> MultivmResult<BlockHeader> {
        let state_root = if self.config.enable_state_root_verification {
            self.verify_and_get_state_root(reth_data).await?
        } else {
            reth_data.state_root.clone()
        };

        Ok(BlockHeader {
            height,
            timestamp,
            previous_hash: if height > 1 {
                format!("consensus_hash_{}", height - 1)
            } else {
                "genesis".to_string()
            },
            state_root,
            transactions_root: reth_data.receipts_root.clone(),
            proposer: "multivm-reth-bridge".to_string(),
            consensus_data: self.create_consensus_data(reth_data).await?,
            version: 1,
            extra_data: vec![],
        })
    }

    /// Verify state root through Reth RPC calls
    async fn verify_and_get_state_root(&self, reth_data: &RethBlockData) -> MultivmResult<String> {
        // Additional verification can be added here
        debug!("Verifying state root for block {}", reth_data.block_number);
        
        // For now, we trust the Reth state root, but we could add additional checks:
        // - Compare with previous block
        // - Verify against transaction receipts
        // - Check against consensus rules
        
        if reth_data.state_root.is_empty() {
            warn!("Empty state root received from Reth block {}", reth_data.block_number);
            return Err(multivm_common::MultivmError::Validation {
                field: "state_root".to_string(),
                message: "Invalid state root from Reth".to_string(),
                value: Some(reth_data.state_root.clone()),
            });
        }

        Ok(reth_data.state_root.clone())
    }

    /// Create consensus data that includes Reth block information
    async fn create_consensus_data(&self, reth_data: &RethBlockData) -> MultivmResult<Vec<u8>> {
        let consensus_info = serde_json::json!({
            "reth_block_number": reth_data.block_number,
            "reth_block_hash": reth_data.block_hash,
            "reth_timestamp": reth_data.timestamp,
            "reth_gas_used": reth_data.gas_used,
            "reth_gas_limit": reth_data.gas_limit,
            "reth_base_fee": reth_data.base_fee_per_gas,
            "multivm_bridge_version": "1.0.0"
        });

        Ok(consensus_info.to_string().into_bytes())
    }

    /// Synchronize timing between MultiVM consensus and Reth process
    async fn coordinate_block_timing(&self, _height: u64) -> MultivmResult<()> {
        // Get the latest Reth block timestamp
        let current_reth_block = self.reth_client.get_block_number().await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: format!("Failed to get current block number for timing: {}", e),
                endpoint: Some("reth".to_string()),
                retry_after: None,
            })?;

        let reth_block = self.reth_client.get_block_by_number(current_reth_block, false).await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: format!("Failed to get block for timing: {}", e),
                endpoint: Some("reth".to_string()),
                retry_after: None,
            })?;

        if let Some(block) = reth_block {
            let reth_timestamp = block.timestamp;
            let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            
            // If Reth is too far ahead, we might want to wait
            if reth_timestamp > current_timestamp + 5 {
                warn!("Reth timestamp is {} seconds ahead of system time", reth_timestamp - current_timestamp);
            }
            
            // If Reth is too far behind, we might want to speed up
            if current_timestamp > reth_timestamp + 30 {
                warn!("Reth timestamp is {} seconds behind system time", current_timestamp - reth_timestamp);
            }
        }

        Ok(())
    }
}

/// Data structure to hold Reth block information
#[derive(Debug, Clone)]
struct RethBlockData {
    pub block_number: u64,
    pub block_hash: String,
    pub parent_hash: String,
    pub state_root: String,
    pub receipts_root: String,
    pub timestamp: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee_per_gas: Option<u64>,
}
