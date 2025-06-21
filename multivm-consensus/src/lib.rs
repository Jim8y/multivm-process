//! MultiVM Consensus Layer
//!
//! This crate provides a unified consensus layer for the MultiVM architecture,
//! supporting multiple consensus algorithms and ensuring cross-VM state consistency.

pub mod block;
pub mod error;
pub mod malachite;
pub mod manager;
pub mod messages;
pub mod state;
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
pub use malachite::{MalachiteConfig, MalachiteConsensus};

// Common imports
use multivm_account_mapping::SpecialTransaction;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(!CONSENSUS_VERSION.is_empty());
        assert!(MAX_BLOCK_SIZE > 0);
        assert!(MAX_TRANSACTIONS_PER_BLOCK > 0);
        assert!(DEFAULT_CONSENSUS_TIMEOUT > 0);
        assert!(DEFAULT_HEARTBEAT_INTERVAL > 0);
    }
}
