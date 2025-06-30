//! Simplified Solana execution engine implementation

use async_trait::async_trait;
use multivm_common::{
    BlockchainType, EngineState, ExecutionEngine, HealthStatus, MultivmError, ProcessId,
    ProcessingMetrics,
};
use std::time::Duration;
use tracing::{info, warn};

/// Simple block type for the simplified engine
#[derive(Debug, Clone)]
pub struct SimpleBlock {
    pub data: Vec<u8>,
    pub block_id: u64,
}

/// Simple execution result
#[derive(Debug, Clone)]
pub struct SimpleExecutionResult {
    pub block_id: u64,
    pub success: bool,
    pub transactions_processed: u64,
}

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
            state: EngineState {
                process_id: ProcessId::Solana,
                blockchain_type: BlockchainType::Solana,
                current_block: None,
                state_root: vec![0u8; 32],
                is_syncing: false,
                peer_count: 0,
                rpc_endpoints: vec!["http://127.0.0.1:8899".to_string()],
                data_directory: "./data/solana".to_string(),
                chain_id: 103, // Solana devnet
            },
            mock_mode: true,
            blocks_processed: 0,
            transactions_processed: 0,
        }
    }
}

#[async_trait]
impl ExecutionEngine for SimpleSolanaEngine {
    type BlockType = SimpleBlock;
    type ExecutionResult = SimpleExecutionResult;
    type Error = MultivmError;

    async fn process_block(
        &mut self,
        block: Self::BlockType,
    ) -> Result<Self::ExecutionResult, Self::Error> {
        info!("Processing block {} (mock mode)", block.block_id);

        self.blocks_processed += 1;
        self.transactions_processed += 1; // Assume 1 transaction per block for simplicity

        // Update state
        self.state.current_block = Some(block.block_id);

        Ok(SimpleExecutionResult {
            block_id: block.block_id,
            success: true,
            transactions_processed: 1,
        })
    }

    async fn get_health(&self) -> Result<HealthStatus, Self::Error> {
        Ok(HealthStatus::Healthy)
    }

    async fn get_state(&self) -> Result<EngineState, Self::Error> {
        Ok(self.state.clone())
    }

    async fn start_rpc_server(
        &self,
        _config: multivm_common::types_rpc::RpcConfig,
    ) -> Result<(), Self::Error> {
        info!("Starting Solana RPC server (mock mode)");
        Ok(())
    }

    async fn stop_rpc_server(&self) -> Result<(), Self::Error> {
        info!("Stopping Solana RPC server (mock mode)");
        Ok(())
    }

    async fn initialize(&mut self) -> Result<(), Self::Error> {
        info!("Initializing Solana execution engine (mock mode)");
        Ok(())
    }

    async fn shutdown(&mut self, _timeout: Option<Duration>) -> Result<(), Self::Error> {
        warn!("Shutting down Solana execution engine");
        Ok(())
    }

    fn blockchain_type(&self) -> BlockchainType {
        BlockchainType::Solana
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error> {
        Ok(ProcessingMetrics {
            cpu_time: Duration::from_millis(100),
            memory_usage_bytes: 128 * 1024 * 1024, // 128MB
            disk_reads: self.blocks_processed * 10,
            disk_writes: self.blocks_processed * 5,
            network_bytes: 0,
            compute_units_used: self.transactions_processed * 5000,
            transaction_count: self.transactions_processed,
            account_updates: self.transactions_processed,
            total_requests: self.blocks_processed,
            successful_requests: self.blocks_processed,
            failed_requests: 0,
            average_response_time_ms: 100.0,
            peak_memory_usage_mb: 128,
            cpu_usage_percent: 5.0,
        })
    }

    async fn get_latest_block_id(&self) -> Result<u64, Self::Error> {
        Ok(self.blocks_processed)
    }

    async fn reset_to_block(&mut self, block_id: u64) -> Result<(), Self::Error> {
        info!("Resetting to block {} (mock mode)", block_id);
        self.blocks_processed = block_id;
        self.state.current_block = Some(block_id);
        Ok(())
    }
}

impl Default for SimpleSolanaEngine {
    fn default() -> Self {
        Self::new()
    }
}
