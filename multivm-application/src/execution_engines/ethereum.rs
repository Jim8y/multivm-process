//! Ethereum Execution Engine Integration
//!
//! This module provides integration with the Reth execution engine for Ethereum
//! transaction processing and state management within the MultiVM system.

use crate::execution_engines::EthereumEngineConfig;
use multivm_common::{
    EngineState, ExecutionEngine, HealthStatus, MultivmResult, ProcessingMetrics,
};
use reth_execution_engine::engine::{Block as RethBlock, RethExecutionEngine, RethExecutionResult};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Ethereum execution engine wrapper for MultiVM integration
pub struct EthereumExecutionWrapper {
    engine: Arc<RwLock<RethExecutionEngine>>,
    config: EthereumEngineConfig,
    metrics: Arc<RwLock<EthereumMetrics>>,
}

/// Metrics specific to Ethereum execution
#[derive(Debug, Default)]
pub struct EthereumMetrics {
    pub blocks_processed: u64,
    pub transactions_processed: u64,
    pub gas_used_total: u64,
    pub average_block_time_ms: f64,
    pub last_block_number: u64,
    pub engine_restarts: u64,
}

impl EthereumExecutionWrapper {
    /// Create a new Ethereum execution wrapper
    pub async fn new(config: EthereumEngineConfig) -> MultivmResult<Self> {
        info!("Creating Ethereum execution wrapper");

        let data_dir = std::path::PathBuf::from(&config.data_dir);

        let engine = RethExecutionEngine::new_with_mode(
            data_dir,
            config.rpc_port,
            config.chain_id,
            config.mock_mode,
        )
        .await
        .map_err(|e| multivm_common::MultivmError::Process {
            process_id: "ethereum-wrapper".to_string(),
            message: format!("Failed to create Reth engine: {}", e),
            exit_code: None,
        })?;

        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
            config,
            metrics: Arc::new(RwLock::new(EthereumMetrics::default())),
        })
    }

    /// Initialize the Ethereum execution engine
    pub async fn initialize(&self) -> MultivmResult<()> {
        info!("Initializing Ethereum execution engine");

        let mut engine = self.engine.write().await;
        engine
            .initialize()
            .await
            .map_err(|e| multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to initialize Reth engine: {}", e),
                exit_code: None,
            })?;

        info!("Ethereum execution engine initialized successfully");
        Ok(())
    }

    /// Process an Ethereum block
    pub async fn process_block(&self, block: RethBlock) -> MultivmResult<RethExecutionResult> {
        let start_time = std::time::Instant::now();

        debug!("Processing Ethereum block {}", block.number);

        let mut engine = self.engine.write().await;
        let result = engine.process_block(block.clone()).await.map_err(|e| {
            multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to process block: {}", e),
                exit_code: None,
            }
        })?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.blocks_processed += 1;
            metrics.transactions_processed += block.body.transactions.len() as u64;
            metrics.gas_used_total += result.gas_used;
            metrics.last_block_number = block.number;

            let block_time_ms = start_time.elapsed().as_millis() as f64;
            if metrics.blocks_processed == 1 {
                metrics.average_block_time_ms = block_time_ms;
            } else {
                // Exponential moving average
                metrics.average_block_time_ms =
                    metrics.average_block_time_ms * 0.9 + block_time_ms * 0.1;
            }
        }

        info!(
            "Successfully processed Ethereum block {} with {} transactions in {:?}",
            block.number,
            block.body.transactions.len(),
            start_time.elapsed()
        );

        Ok(result)
    }

    /// Get health status of the Ethereum engine
    pub async fn get_health_status(&self) -> MultivmResult<HealthStatus> {
        let engine = self.engine.read().await;
        engine
            .get_health()
            .await
            .map_err(|e| multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to get health status: {}", e),
                exit_code: None,
            })
    }

    /// Get engine state
    pub async fn get_engine_state(&self) -> MultivmResult<EngineState> {
        let engine = self.engine.read().await;
        engine
            .get_state()
            .await
            .map_err(|e| multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to get engine state: {}", e),
                exit_code: None,
            })
    }

    /// Get processing metrics
    pub async fn get_processing_metrics(&self) -> MultivmResult<ProcessingMetrics> {
        let engine = self.engine.read().await;
        engine
            .get_metrics()
            .await
            .map_err(|e| multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to get processing metrics: {}", e),
                exit_code: None,
            })
    }

    /// Get Ethereum-specific metrics
    pub async fn get_ethereum_metrics(&self) -> EthereumMetrics {
        self.metrics.read().await.clone()
    }

    /// Check if the engine is ready
    pub async fn is_ready(&self) -> bool {
        let engine = self.engine.read().await;
        engine.is_ready().await
    }

    /// Get the latest block ID
    pub async fn get_latest_block_id(&self) -> MultivmResult<u64> {
        let engine = self.engine.read().await;
        engine
            .get_latest_block_id()
            .await
            .map_err(|e| multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to get latest block ID: {}", e),
                exit_code: None,
            })
    }

    /// Reset the engine to a specific block
    pub async fn reset_to_block(&self, block_id: u64) -> MultivmResult<()> {
        info!("Resetting Ethereum engine to block {}", block_id);

        let mut engine = self.engine.write().await;
        engine.reset_to_block(block_id).await.map_err(|e| {
            multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to reset to block: {}", e),
                exit_code: None,
            }
        })?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.last_block_number = block_id;
            // Note: In a full implementation, we might need to recalculate other metrics
        }

        info!("Successfully reset Ethereum engine to block {}", block_id);
        Ok(())
    }

    /// Restart the Ethereum engine
    pub async fn restart(&self) -> MultivmResult<()> {
        warn!("Restarting Ethereum execution engine");

        // Shutdown current engine
        {
            let mut engine = self.engine.write().await;
            if let Err(e) = engine.shutdown(Some(Duration::from_secs(10))).await {
                error!("Error during engine shutdown: {}", e);
            }
        }

        // Create new engine instance
        let data_dir = std::path::PathBuf::from(&self.config.data_dir);
        let new_engine = RethExecutionEngine::new_with_mode(
            data_dir,
            self.config.rpc_port,
            self.config.chain_id,
            self.config.mock_mode,
        )
        .await
        .map_err(|e| multivm_common::MultivmError::Process {
            process_id: "ethereum-wrapper".to_string(),
            message: format!("Failed to create new Reth engine: {}", e),
            exit_code: None,
        })?;

        // Replace the engine
        {
            let mut engine_guard = self.engine.write().await;
            *engine_guard = new_engine;
        }

        // Initialize the new engine
        {
            let mut engine = self.engine.write().await;
            engine
                .initialize()
                .await
                .map_err(|e| multivm_common::MultivmError::Process {
                    process_id: "ethereum-wrapper".to_string(),
                    message: format!("Failed to initialize restarted engine: {}", e),
                    exit_code: None,
                })?;
        }

        // Update restart metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.engine_restarts += 1;
        }

        info!("Successfully restarted Ethereum execution engine");
        Ok(())
    }

    /// Shutdown the Ethereum engine
    pub async fn shutdown(&self, timeout: Option<Duration>) -> MultivmResult<()> {
        info!("Shutting down Ethereum execution engine");

        let mut engine = self.engine.write().await;
        engine
            .shutdown(timeout)
            .await
            .map_err(|e| multivm_common::MultivmError::Process {
                process_id: "ethereum-wrapper".to_string(),
                message: format!("Failed to shutdown engine: {}", e),
                exit_code: None,
            })?;

        info!("Ethereum execution engine shutdown completed");
        Ok(())
    }

    /// Create a test Ethereum block for development/testing
    pub fn create_test_block(&self, block_number: u64, transaction_count: usize) -> RethBlock {
        reth_execution_engine::engine::generate_mock_reth_block(block_number, transaction_count)
    }

    /// Get configuration
    pub fn get_config(&self) -> &EthereumEngineConfig {
        &self.config
    }
}

impl Clone for EthereumMetrics {
    fn clone(&self) -> Self {
        Self {
            blocks_processed: self.blocks_processed,
            transactions_processed: self.transactions_processed,
            gas_used_total: self.gas_used_total,
            average_block_time_ms: self.average_block_time_ms,
            last_block_number: self.last_block_number,
            engine_restarts: self.engine_restarts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_wrapper() -> (EthereumExecutionWrapper, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = EthereumEngineConfig {
            enabled: true,
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            rpc_port: 18545, // Use different port for testing
            chain_id: 1337,
            mock_mode: true, // Always use mock mode for tests
            auto_start: false,
        };

        let wrapper = EthereumExecutionWrapper::new(config).await.unwrap();
        (wrapper, temp_dir)
    }

    #[tokio::test]
    async fn test_ethereum_wrapper_creation() {
        let (_wrapper, _temp_dir) = create_test_wrapper().await;
        // Test passes if wrapper is created successfully
    }

    #[tokio::test]
    async fn test_ethereum_wrapper_initialization() {
        let (wrapper, _temp_dir) = create_test_wrapper().await;

        wrapper.initialize().await.unwrap();

        // Check that engine is ready after initialization
        assert!(wrapper.is_ready().await);
    }

    #[tokio::test]
    async fn test_block_processing() {
        let (wrapper, _temp_dir) = create_test_wrapper().await;

        wrapper.initialize().await.unwrap();

        let test_block = wrapper.create_test_block(1, 3);
        let result = wrapper.process_block(test_block).await.unwrap();

        assert_eq!(result.block_number, 1);
        assert_eq!(result.transactions_count, 3);
        assert!(result.success);

        // Check metrics were updated
        let metrics = wrapper.get_ethereum_metrics().await;
        assert_eq!(metrics.blocks_processed, 1);
        assert_eq!(metrics.transactions_processed, 3);
        assert_eq!(metrics.last_block_number, 1);
    }

    #[tokio::test]
    async fn test_health_status() {
        let (wrapper, _temp_dir) = create_test_wrapper().await;

        wrapper.initialize().await.unwrap();

        let health = wrapper.get_health_status().await.unwrap();
        assert_eq!(health, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_reset_to_block() {
        let (wrapper, _temp_dir) = create_test_wrapper().await;

        wrapper.initialize().await.unwrap();

        // Process a block first
        let test_block = wrapper.create_test_block(5, 2);
        wrapper.process_block(test_block).await.unwrap();

        // Reset to an earlier block
        wrapper.reset_to_block(3).await.unwrap();

        let metrics = wrapper.get_ethereum_metrics().await;
        assert_eq!(metrics.last_block_number, 3);
    }
}
