use crate::{
    BlockchainType, EngineState, HealthStatus, MultivmError, ProcessingMetrics, RpcConfig,
};
use async_trait::async_trait;
use std::time::Duration;

/// Core generic execution engine trait
/// Each concrete engine can define its own block type and result type
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Block type processed by the engine (e.g., reth_primitives::Block or SolanaBlockData)
    type BlockType: Send + Sync;

    /// Execution result type returned by the engine
    type ExecutionResult: Send + Sync;

    /// Engine-specific error type
    type Error: Send + Sync + std::error::Error;

    /// Process a block and update engine state
    async fn process_block(
        &mut self,
        block: Self::BlockType,
    ) -> Result<Self::ExecutionResult, Self::Error>;

    /// Get current health status of the engine
    async fn get_health(&self) -> Result<HealthStatus, Self::Error>;

    /// Get current state of the engine
    async fn get_state(&self) -> Result<EngineState, Self::Error>;

    /// Start RPC server for this engine
    async fn start_rpc_server(&self, config: RpcConfig) -> Result<(), Self::Error>;

    /// Stop RPC server
    async fn stop_rpc_server(&self) -> Result<(), Self::Error>;

    /// Initialize engine with configuration
    async fn initialize(&mut self) -> Result<(), Self::Error>;

    /// Gracefully shutdown the engine
    async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), Self::Error>;

    /// Get blockchain type handled by this engine
    fn blockchain_type(&self) -> BlockchainType;

    /// Check if engine is ready to process blocks
    async fn is_ready(&self) -> bool;

    /// Get current processing metrics
    async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error>;
}

/// Simplified block provider trait - also generic
#[async_trait]
pub trait BlockProvider<T>: Send + Sync
where
    T: Send + Sync,
{
    /// Get next block for specified blockchain
    async fn get_next_block(
        &self,
        blockchain_type: BlockchainType,
        current_block: Option<u64>,
    ) -> Result<Option<T>, MultivmError>;

    /// Check if there are pending blocks available
    async fn has_pending_blocks(
        &self,
        blockchain_type: BlockchainType,
    ) -> Result<bool, MultivmError>;

    /// Get latest block number/slot for the blockchain
    async fn get_latest_block_id(
        &self,
        blockchain_type: BlockchainType,
    ) -> Result<u64, MultivmError>;

    /// Start block provider service
    async fn start(&mut self) -> Result<(), MultivmError>;

    /// Stop block provider service
    async fn stop(&mut self) -> Result<(), MultivmError>;
}
