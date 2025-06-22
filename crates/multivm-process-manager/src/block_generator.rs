//! Block generator for continuous block production in mock mode

use crate::coordinator::MultivmCoordinator;
use multivm_common::MultivmResult;
use multivm_consensus::{BlockHeader, EvmSignature, EvmTransaction, MultiVMBlock, SvmTransaction};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{interval, Interval};
use tracing::{debug, error, info};

/// Configuration for block generation
#[derive(Debug, Clone)]
pub struct BlockGeneratorConfig {
    /// Interval between blocks in milliseconds
    pub block_interval_ms: u64,
    /// Number of SVM transactions per block
    pub svm_tx_per_block: usize,
    /// Number of EVM transactions per block  
    pub evm_tx_per_block: usize,
    /// Whether to enable continuous generation
    pub enabled: bool,
}

impl Default for BlockGeneratorConfig {
    fn default() -> Self {
        Self {
            block_interval_ms: 1000, // 1 second blocks
            svm_tx_per_block: 5,
            evm_tx_per_block: 5,
            enabled: true,
        }
    }
}

/// Block generator service that continuously produces blocks for testing
pub struct BlockGenerator {
    config: BlockGeneratorConfig,
    coordinator: Arc<RwLock<MultivmCoordinator>>,
    current_height: Arc<RwLock<u64>>,
    is_running: Arc<RwLock<bool>>,
    interval: Option<Interval>,
}

impl BlockGenerator {
    /// Create a new block generator
    pub fn new(config: BlockGeneratorConfig, coordinator: Arc<RwLock<MultivmCoordinator>>) -> Self {
        Self {
            config,
            coordinator,
            current_height: Arc::new(RwLock::new(1)),
            is_running: Arc::new(RwLock::new(false)),
            interval: None,
        }
    }

    /// Start continuous block generation
    pub async fn start(&mut self) -> MultivmResult<()> {
        if !self.config.enabled {
            info!("Block generator is disabled");
            return Ok(());
        }

        info!(
            "Starting block generator with {} ms intervals",
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

        info!("Block generator started successfully");

        // Main generation loop
        loop {
            // Check if we should continue running
            {
                let running = self.is_running.read().await;
                if !*running {
                    info!("Block generator stopping...");
                    break;
                }
            }

            // Wait for next interval
            interval.tick().await;

            // Generate and submit block
            if let Err(e) = self.generate_and_submit_block().await {
                error!("Failed to generate block: {}", e);
                // Continue running despite errors
            }
        }

        info!("Block generator stopped");
        Ok(())
    }

    /// Stop block generation
    pub async fn stop(&self) {
        info!("Stopping block generator...");
        let mut running = self.is_running.write().await;
        *running = false;
    }

    /// Generate and submit a single block
    async fn generate_and_submit_block(&self) -> MultivmResult<()> {
        let height = {
            let mut current_height = self.current_height.write().await;
            let height = *current_height;
            *current_height += 1;
            height
        };

        debug!("Generating block at height {}", height);

        // Generate block content
        let block = self.generate_mock_block(height).await?;

        // Submit to coordinator
        {
            let coordinator = self.coordinator.read().await;
            coordinator.submit_block(block).await?;
        }

        debug!("Successfully submitted block at height {}", height);
        Ok(())
    }

    /// Generate a mock block with random transactions
    async fn generate_mock_block(&self, height: u64) -> MultivmResult<MultiVMBlock> {
        let _timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Generate SVM transactions
        let mut svm_transactions = Vec::new();
        for i in 0..self.config.svm_tx_per_block {
            svm_transactions.push(SvmTransaction {
                id: uuid::Uuid::new_v4(),
                signatures: vec![format!("svm_mock_sig_{}_{}", height, i)],
                accounts: vec![
                    format!("svm_account_{}_{}_from", height, i),
                    format!("svm_account_{}_{}_to", height, i),
                ],
                data: vec![0x01, 0x02, 0x03, 0x04], // Mock instruction data
                recent_blockhash: format!("block_hash_{}", height.saturating_sub(1)),
                fee: 5000 + (i as u64 * 100),
                metadata: serde_json::json!({"mock": true, "block": height, "index": i}),
            });
        }

        // Generate EVM transactions
        let mut evm_transactions = Vec::new();
        for i in 0..self.config.evm_tx_per_block {
            evm_transactions.push(EvmTransaction {
                id: uuid::Uuid::new_v4(),
                hash: format!("evm_mock_hash_{}_{}", height, i),
                from: format!("0x{:040x}", height * 1000 + i as u64),
                to: Some(format!("0x{:040x}", height * 1000 + i as u64 + 1)),
                value: 1000000000000000000u64, // 1 ETH in wei
                gas_limit: 21000,
                gas_price: 20000000000u64, // 20 gwei
                data: vec![],
                nonce: i as u64,
                signature: EvmSignature {
                    v: 27,
                    r: format!("0x{:064x}", height * 1000 + i as u64),
                    s: format!("0x{:064x}", height * 1000 + i as u64 + 500),
                },
                metadata: serde_json::json!({"mock": true, "block": height, "index": i}),
            });
        }

        // Create block header
        let header = BlockHeader {
            height,
            timestamp: SystemTime::now(),
            previous_hash: if height > 1 {
                format!("block_hash_{}", height - 1)
            } else {
                "genesis".to_string()
            },
            state_root: format!("state_root_{}", height),
            transactions_root: format!("tx_root_{}", height),
            proposer: "mock_validator".to_string(),
            consensus_data: vec![],
            version: 1,
            extra_data: vec![],
        };

        // Create block
        let block = MultiVMBlock {
            header,
            svm_transactions,
            evm_transactions,
            multivm_transactions: vec![], // No special transactions for now
            state_transitions: vec![],    // No state transitions for mock mode
        };

        info!(
            "Generated mock block {} with {} SVM and {} EVM transactions",
            height, self.config.svm_tx_per_block, self.config.evm_tx_per_block
        );

        Ok(block)
    }

    /// Get current block height
    pub async fn get_current_height(&self) -> u64 {
        *self.current_height.read().await
    }

    /// Check if the generator is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Update configuration
    pub async fn update_config(&mut self, config: BlockGeneratorConfig) {
        self.config = config;
        info!("Block generator configuration updated");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator::CoordinatorConfig;
    use multivm_consensus::MalachiteConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn test_block_generator_creation() {
        let config = BlockGeneratorConfig::default();
        let coordinator_config = CoordinatorConfig {
            consensus: MalachiteConfig::default(),
            health_check_interval: Duration::from_secs(30),
            block_timeout: Duration::from_secs(60),
            max_concurrent_blocks: 10,
            enable_recovery: true,
        };

        let coordinator = MultivmCoordinator::new(coordinator_config).await.unwrap();
        let coordinator = Arc::new(RwLock::new(coordinator));

        let generator = BlockGenerator::new(config, coordinator);
        assert!(!generator.is_running().await);
        assert_eq!(generator.get_current_height().await, 1);
    }

    #[tokio::test]
    async fn test_mock_block_generation() {
        let config = BlockGeneratorConfig {
            block_interval_ms: 100,
            svm_tx_per_block: 3,
            evm_tx_per_block: 2,
            enabled: true,
        };

        let coordinator_config = CoordinatorConfig {
            consensus: MalachiteConfig::default(),
            health_check_interval: Duration::from_secs(30),
            block_timeout: Duration::from_secs(60),
            max_concurrent_blocks: 10,
            enable_recovery: true,
        };

        let coordinator = MultivmCoordinator::new(coordinator_config).await.unwrap();
        let coordinator = Arc::new(RwLock::new(coordinator));

        let generator = BlockGenerator::new(config, coordinator);
        let block = generator.generate_mock_block(1).await.unwrap();

        assert_eq!(block.header.height, 1);
        assert_eq!(block.svm_transactions.len(), 3);
        assert_eq!(block.evm_transactions.len(), 2);
        assert_eq!(block.multivm_transactions.len(), 0);
    }
}
