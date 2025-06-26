//! Simplified Solana execution engine implementation

use crate::common::{ExecutionEngine, EngineState, HealthStatus, MultivmError, ProcessingMetrics};
use async_trait::async_trait;
use std::time::Duration;
use tracing::{info, warn};

/// Simplified Solana execution engine
pub struct SimpleSolanaEngine {
    state: EngineState,
    mock_mode: bool,
    blocks_processed: u64,
    transactions_processed: u64,
}

impl SimpleSolanaEngine {
    pub fn new() -> Self {
        Self {
            state: EngineState::Stopped,
            mock_mode: true,
            blocks_processed: 0,
            transactions_processed: 0,
        }
    }
}

#[async_trait]
impl ExecutionEngine for SimpleSolanaEngine {
    type Error = MultivmError;

    async fn start(&mut self) -> Result<(), Self::Error> {
        info!("Starting Solana execution engine (mock mode)");
        self.state = EngineState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        info!("Stopping Solana execution engine");
        self.state = EngineState::Stopped;
        Ok(())
    }

    fn get_state(&self) -> EngineState {
        self.state.clone()
    }

    async fn is_ready(&self) -> bool {
        matches!(self.state, EngineState::Running)
    }

    async fn process_transaction(&mut self, _transaction: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        if !self.is_ready().await {
            return Err(MultivmError::Other("Engine not ready".to_string()));
        }

        self.transactions_processed += 1;
        info!("Processed transaction (mock mode)");
        
        // Return mock transaction result
        Ok(b"mock_tx_result".to_vec())
    }

    async fn process_block(&mut self, _block: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        if !self.is_ready().await {
            return Err(MultivmError::Other("Engine not ready".to_string()));
        }

        self.blocks_processed += 1;
        info!("Processed block {} (mock mode)", self.blocks_processed);
        
        // Return mock block result
        Ok(b"mock_block_result".to_vec())
    }

    async fn get_block_height(&self) -> Result<u64, Self::Error> {
        Ok(self.blocks_processed)
    }

    async fn get_health(&self) -> Result<HealthStatus, Self::Error> {
        match self.state {
            EngineState::Running => Ok(HealthStatus::Healthy),
            EngineState::Error => Ok(HealthStatus::Unhealthy),
            _ => Ok(HealthStatus::Unknown),
        }
    }

    async fn shutdown(&mut self, _timeout: Option<Duration>) -> Result<(), Self::Error> {
        warn!("Shutting down Solana execution engine");
        self.state = EngineState::Stopped;
        Ok(())
    }

    async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error> {
        Ok(ProcessingMetrics {
            blocks_processed: self.blocks_processed,
            transactions_processed: self.transactions_processed,
            average_block_time: Duration::from_secs(1),
            last_block_timestamp: Some(std::time::SystemTime::now()),
            error_count: 0,
        })
    }

    async fn get_latest_block_id(&self) -> Result<u64, Self::Error> {
        Ok(self.blocks_processed)
    }

    async fn reset_to_block(&mut self, block_id: u64) -> Result<(), Self::Error> {
        info!("Resetting to block {} (mock mode)", block_id);
        self.blocks_processed = block_id;
        Ok(())
    }
}

impl Default for SimpleSolanaEngine {
    fn default() -> Self {
        Self::new()
    }
}