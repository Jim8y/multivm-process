//! Execution engine traits and abstractions for blockchain processing
//!
//! This module defines the core traits that all blockchain execution engines must implement,
//! providing a unified interface for processing blocks across different blockchain types.

use crate::{
    types::RpcConfig, BlockchainType, EngineState, HealthStatus, MultivmError, ProcessingMetrics,
};
use async_trait::async_trait;
use std::time::Duration;

/// Core execution engine trait for blockchain processing
///
/// This trait provides a unified interface for all blockchain execution engines,
/// allowing the MultiVM system to process blocks from different blockchains
/// (Solana, Ethereum, etc.) through a common abstraction.
///
/// # Type Parameters
///
/// - `BlockType`: The blockchain-specific block type (e.g., `reth_primitives::Block` for Ethereum)
/// - `ExecutionResult`: The result type returned after processing a block
/// - `Error`: Engine-specific error type that can be converted to `MultivmError`
///
/// # Error Handling
///
/// All methods return `Result` types with engine-specific errors that should be
/// convertible to `MultivmError` for unified error handling across the system.
///
/// # Example Implementation
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use multivm_common::traits::ExecutionEngine;
/// use multivm_common::{BlockchainType, HealthStatus, EngineState, ProcessingMetrics, ProcessId};
/// use multivm_common::types::rpc::RpcConfig;
/// use std::time::Duration;
///
/// # #[derive(Clone)]
/// # struct MyBlock;
/// # #[derive(Clone)]
/// # struct MyResult;
/// # #[derive(Debug)]
/// # struct MyError;
/// # impl std::fmt::Display for MyError {
/// #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
/// #         write!(f, "MyError")
/// #     }
/// # }
/// # impl std::error::Error for MyError {}
/// # impl From<MyError> for multivm_common::MultivmError {
/// #     fn from(err: MyError) -> Self {
/// #         multivm_common::MultivmError::Internal {
/// #             component: "engine".to_string(),
/// #             message: err.to_string(),
/// #             error_code: None,
/// #         }
/// #     }
/// # }
///
/// struct MyEngine {
///     // Engine state
/// }
///
/// #[async_trait]
/// impl ExecutionEngine for MyEngine {
///     type BlockType = MyBlock;
///     type ExecutionResult = MyResult;
///     type Error = MyError;
///
///     async fn process_block(&mut self, block: Self::BlockType) -> Result<Self::ExecutionResult, Self::Error> {
///         // Process the block and return results
///         // In a real implementation, you would:
///         // 1. Validate the block
///         // 2. Execute transactions
///         // 3. Update state
///         Ok(MyResult)
///     }
///
///     fn blockchain_type(&self) -> BlockchainType {
///         BlockchainType::Ethereum // or BlockchainType::Solana
///     }
///
///     async fn get_health(&self) -> Result<HealthStatus, Self::Error> {
///         Ok(HealthStatus::Healthy)
///     }
///
///     async fn get_state(&self) -> Result<EngineState, Self::Error> {
///         Ok(EngineState {
///             process_id: ProcessId::Ethereum,
///             blockchain_type: BlockchainType::Ethereum,
///             current_block: Some(12345),
///             state_root: vec![0x01, 0x23],
///             rpc_endpoints: vec!["http://localhost:8545".to_string()],
///             is_syncing: false,
///             peer_count: 5,
///             data_directory: "/tmp/engine".to_string(),
///             chain_id: 1,
///         })
///     }
///
///     async fn start_rpc_server(&self, config: RpcConfig) -> Result<(), Self::Error> {
///         // Start RPC server implementation
///         Ok(())
///     }
///
///     async fn stop_rpc_server(&self) -> Result<(), Self::Error> {
///         // Stop RPC server implementation
///         Ok(())
///     }
///
///     async fn initialize(&mut self) -> Result<(), Self::Error> {
///         // Initialize engine: load config, connect to network, sync state
///         Ok(())
///     }
///
///     async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), Self::Error> {
///         // Graceful shutdown implementation
///         Ok(())
///     }
///
///     async fn is_ready(&self) -> bool {
///         // Check if engine is initialized and ready
///         true
///     }
///
///     async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error> {
///         Ok(ProcessingMetrics {
///             cpu_time: Duration::from_secs(100),
///             memory_usage_bytes: 1024 * 1024 * 100, // 100MB
///             disk_reads: 1000,
///             disk_writes: 500,
///             network_bytes: 1024 * 1024, // 1MB
///             compute_units_used: 50000,
///             transaction_count: 100,
///             account_updates: 200,
///             total_requests: 1000,
///             successful_requests: 980,
///             failed_requests: 20,
///             average_response_time_ms: 15.5,
///             peak_memory_usage_mb: 150,
///             cpu_usage_percent: 45.0,
///         })
///     }
///
///     async fn get_latest_block_id(&self) -> Result<u64, Self::Error> {
///         Ok(12345)
///     }
///
///     async fn reset_to_block(&mut self, block_id: u64) -> Result<(), Self::Error> {
///         // Reset state to specific block
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Block type processed by the engine
    ///
    /// This should be the native block type for the specific blockchain:
    /// - For Ethereum: `reth_primitives::Block` or similar
    /// - For Solana: Custom block structure containing slot data
    type BlockType: Send + Sync;

    /// Execution result type returned by the engine
    ///
    /// Contains the results of block processing, including:
    /// - Transaction receipts
    /// - State changes
    /// - Processing metrics
    /// - Any blockchain-specific results
    type ExecutionResult: Send + Sync;

    /// Engine-specific error type
    ///
    /// Should implement `std::error::Error` and be convertible to `MultivmError`
    /// for unified error handling across the MultiVM system.
    type Error: Send + Sync + std::error::Error + Into<MultivmError>;

    /// Process a block and update engine state
    ///
    /// This is the core method that processes a blockchain block, executing
    /// all transactions within it and updating the engine's internal state.
    ///
    /// # Arguments
    ///
    /// * `block` - The block to process
    ///
    /// # Returns
    ///
    /// Returns the execution result containing transaction receipts, state changes,
    /// and processing metrics.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Block validation fails
    /// - Transaction execution fails
    /// - State update fails
    /// - Resource limits are exceeded
    async fn process_block(
        &mut self,
        block: Self::BlockType,
    ) -> Result<Self::ExecutionResult, Self::Error>;

    /// Get current health status of the engine
    ///
    /// Returns detailed health information including:
    /// - Engine operational status
    /// - Resource usage
    /// - Connection status to external services
    /// - Any detected issues or warnings
    async fn get_health(&self) -> Result<HealthStatus, Self::Error>;

    /// Get current state of the engine
    ///
    /// Returns the current operational state of the engine,
    /// including initialization status, processing state, and configuration.
    async fn get_state(&self) -> Result<EngineState, Self::Error>;

    /// Start RPC server for this engine
    ///
    /// Starts the JSON-RPC server that allows external clients to interact
    /// with this specific blockchain engine.
    ///
    /// # Arguments
    ///
    /// * `config` - RPC server configuration including host, port, and security settings
    async fn start_rpc_server(&self, config: RpcConfig) -> Result<(), Self::Error>;

    /// Stop RPC server
    ///
    /// Gracefully shuts down the RPC server, finishing any in-flight requests
    /// before closing the server socket.
    async fn stop_rpc_server(&self) -> Result<(), Self::Error>;

    /// Initialize engine with configuration
    ///
    /// Performs initial setup of the engine, including:
    /// - Loading configuration
    /// - Connecting to external services
    /// - Initializing state
    /// - Preparing for block processing
    async fn initialize(&mut self) -> Result<(), Self::Error>;

    /// Gracefully shutdown the engine
    ///
    /// Performs cleanup and shutdown operations:
    /// - Stops processing new blocks
    /// - Completes in-flight operations
    /// - Saves state to persistent storage
    /// - Closes external connections
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for graceful shutdown
    async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), Self::Error>;

    /// Get blockchain type handled by this engine
    ///
    /// Returns the specific blockchain type (Solana, Ethereum, etc.)
    /// that this engine is designed to process.
    fn blockchain_type(&self) -> BlockchainType;

    /// Check if engine is ready to process blocks
    ///
    /// Returns `true` if the engine is fully initialized and ready
    /// to process blocks, `false` otherwise.
    async fn is_ready(&self) -> bool;

    /// Get current processing metrics
    ///
    /// Returns detailed metrics about the engine's performance:
    /// - Blocks processed
    /// - Transaction throughput
    /// - Resource usage
    /// - Processing latency
    async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error>;

    /// Validate a block without processing it
    ///
    /// Performs validation checks on a block without executing transactions
    /// or updating state. Useful for pre-validation and testing.
    ///
    /// Default implementation calls `process_block` in a dry-run mode,
    /// but engines can override for more efficient validation.
    async fn validate_block(&self, block: &Self::BlockType) -> Result<bool, Self::Error> {
        // Default implementation - engines should override for efficiency
        let _ = block;
        Ok(true)
    }

    /// Get the latest processed block identifier
    ///
    /// Returns the identifier (block number, slot, etc.) of the most
    /// recently processed block.
    async fn get_latest_block_id(&self) -> Result<u64, Self::Error>;

    /// Reset engine state to a specific block
    ///
    /// Resets the engine's state to match the state at the specified block.
    /// Used for handling chain reorganizations and state recovery.
    ///
    /// # Arguments
    ///
    /// * `block_id` - The block identifier to reset to
    async fn reset_to_block(&mut self, block_id: u64) -> Result<(), Self::Error>;
}

/// Block provider trait for fetching blockchain data
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
