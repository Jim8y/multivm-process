//! MultiVM Consensus Layer
//!
//! This crate provides a unified consensus layer for the MultiVM architecture,
//! supporting multiple consensus algorithms and ensuring cross-VM state consistency.

// Only allow dead code and warnings in debug builds
#![cfg_attr(debug_assertions, allow(
    dead_code,
    unused_variables,
    clippy::op_ref,
    clippy::unused_enumerate_index,
    clippy::useless_vec
))]

pub mod block;
pub mod crypto;
pub mod error;
pub mod fork_detection;
pub mod malachite;
pub mod manager;
pub mod messages;
pub mod metrics;
pub mod network_recovery;
pub mod process_integration;
pub mod state;
pub mod synchronization;
pub mod traits;

// Tests are included in individual modules

// Re-export core types
pub use block::{
    BlockHash, BlockHeader, EvmSignature, EvmTransaction, MultiVMBlock,
    StateChange as BlockStateChange, StateHash, StateTransition, SvmTransaction, TransactionHash,
    TransactionId,
};
pub use error::*;
pub use manager::*;
pub use messages::*;
pub use state::*;
pub use traits::{
    BlockProposer, ConsensusEngine, ConsensusStats, CrossVMState, CrossVMStateCoordinator, NodeId,
    StateChange, StateChangeType, StateCheckpoint, ValidationResult,
};

// Re-export Malachite consensus implementation
pub use malachite::{MalachiteConfig, MalachiteConsensus, ValidatorInfo};

// Re-export process integration types
pub use process_integration::{
    ExecutionState, ExecutionStats, ProcessConsensusConfig, ProcessConsensusCoordinator,
    ProcessConsensusMetrics, ProcessExecutionConfig,
};

// Re-export cryptographic types
pub use crypto::{ConsensusSignature, ProductionSigningScheme, ValidatorPublicKey};

// Re-export fork detection types
pub use fork_detection::{
    ForkDetectionConfig, ForkDetectionError, ForkDetectionManager, ForkDetectionMetrics,
    ForkDetector, ForkInfo, ForkReason, ForkResolutionStrategy, ForkStatus,
};

// Re-export network recovery types
pub use network_recovery::{
    NetworkHealth, NetworkHealthStatus, NetworkRecovery, NetworkRecoveryConfig,
    NetworkRecoveryError, NetworkRecoveryManager, NetworkRecoveryMetrics, PartitionIndicator,
    RecoveryPhase, RecoveryStatus,
};

// Re-export synchronization types
pub use synchronization::{
    BlockSyncConfig, BlockSyncError, BlockSyncManager, BlockSyncMetrics, BlockSyncStatus,
    BlockSynchronizer, SyncRequest, SyncResponse,
};

// Re-export metrics types
pub use metrics::{
    AggregatedMetrics, ComponentMetrics, ConsensusMetricsCollector, ConsensusPerformance,
    ErrorSummary, JsonExporter, MetricsExporter, NetworkStatus, PrometheusExporter, SyncStatus,
    SystemHealth,
};

// Common imports
// use multivm_account_mapping::special_tx::SpecialTransaction;

/// Version information for the consensus layer
pub const CONSENSUS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum block size in bytes
pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1MB

/// Maximum number of transactions per block
pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 1000;

/// Default consensus timeout in milliseconds
pub const DEFAULT_CONSENSUS_TIMEOUT: u64 = 5000;

/// Default heartbeat interval in milliseconds
pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 1000;
