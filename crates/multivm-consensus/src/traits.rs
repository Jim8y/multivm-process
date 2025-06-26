//! Core traits for the MultiVM consensus layer

use crate::ConsensusResult;
use async_trait::async_trait;
// use multivm_account_mapping::{
//     address::MultivmAccountId,
//     special_tx::SpecialTransaction
// };
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::time::SystemTime;

/// Core consensus engine trait that all consensus algorithms must implement
#[async_trait]
pub trait ConsensusEngine: Send + Sync + Debug {
    /// Block type specific to this consensus algorithm
    type Block: Clone + Send + Sync + Debug;

    /// Transaction type handled by this consensus
    type Transaction: Clone + Send + Sync + Debug;

    /// Configuration type for this consensus algorithm
    type Config: Clone + Send + Sync + Debug;

    /// Initialize the consensus engine with the given configuration
    async fn initialize(&mut self, config: Self::Config) -> ConsensusResult<()>;

    /// Start the consensus process
    async fn start(&mut self) -> ConsensusResult<()>;

    /// Stop the consensus process gracefully
    async fn stop(&mut self) -> ConsensusResult<()>;

    /// Check if the consensus engine is currently running
    fn is_running(&self) -> bool;

    /// Propose a new block with the given transactions
    async fn propose_block(
        &self,
        transactions: Vec<Self::Transaction>,
    ) -> ConsensusResult<Self::Block>;

    /// Validate a proposed block
    async fn validate_block(&self, block: &Self::Block) -> ConsensusResult<bool>;

    /// Commit a validated block to the blockchain
    async fn commit_block(&mut self, block: Self::Block) -> ConsensusResult<()>;

    /// Get the current blockchain height
    async fn get_current_height(&self) -> ConsensusResult<u64>;

    /// Get the latest committed block
    async fn get_latest_block(&self) -> ConsensusResult<Option<Self::Block>>;

    /// Get a block by its height
    async fn get_block_by_height(&self, height: u64) -> ConsensusResult<Option<Self::Block>>;

    /// Get current consensus statistics
    async fn get_consensus_stats(&self) -> ConsensusResult<ConsensusStats>;
}

/// Cross-VM state coordinator trait for managing state across different VMs
#[async_trait]
pub trait CrossVMStateCoordinator: Send + Sync + Debug {
    /// Apply a cross-VM transaction and return the resulting state changes
    // async fn apply_cross_vm_transaction(
    //     &mut self,
    //     transaction: &SpecialTransaction,
    // ) -> ConsensusResult<Vec<StateChange>>;

    /// Validate a cross-VM transaction before applying it
    // async fn validate_cross_vm_transaction(
    //     &self,
    //     transaction: &SpecialTransaction,
    // ) -> ConsensusResult<ValidationResult>;

    /// Get the current cross-VM state snapshot
    async fn get_cross_vm_state(&self) -> ConsensusResult<CrossVMState>;

    /// Synchronize state with other nodes
    async fn sync_state(&mut self, target_height: u64) -> ConsensusResult<()>;

    /// Create a state checkpoint at the current height
    async fn create_checkpoint(&self) -> ConsensusResult<StateCheckpoint>;

    /// Restore state from a checkpoint
    async fn restore_from_checkpoint(
        &mut self,
        checkpoint: &StateCheckpoint,
    ) -> ConsensusResult<()>;
}

/// Block proposer trait for nodes that can propose new blocks
#[async_trait]
pub trait BlockProposer: Send + Sync + Debug {
    type Block;
    type Transaction;

    /// Check if this node is eligible to propose a block at the current time
    async fn can_propose(&self) -> ConsensusResult<bool>;

    /// Create a new block proposal with the given transactions
    async fn create_proposal(
        &self,
        transactions: Vec<Self::Transaction>,
    ) -> ConsensusResult<Self::Block>;

    /// Get the next proposer in the rotation
    async fn get_next_proposer(&self) -> ConsensusResult<NodeId>;
}

/// Block validator trait for validating proposed blocks
#[async_trait]
pub trait BlockValidator: Send + Sync + Debug {
    type Block: Send + Sync;

    /// Validate the structure and content of a block
    async fn validate_structure(&self, block: &Self::Block) -> ConsensusResult<bool>;

    /// Validate the transactions within a block
    async fn validate_transactions(&self, block: &Self::Block) -> ConsensusResult<bool>;

    /// Validate the block's consensus-specific data
    async fn validate_consensus_data(&self, block: &Self::Block) -> ConsensusResult<bool>;

    /// Perform full validation of a block
    async fn validate_full(&self, block: &Self::Block) -> ConsensusResult<ValidationResult> {
        if !self.validate_structure(block).await? {
            return Ok(ValidationResult::Invalid(
                "Invalid block structure".to_string(),
            ));
        }

        if !self.validate_transactions(block).await? {
            return Ok(ValidationResult::Invalid(
                "Invalid transactions".to_string(),
            ));
        }

        if !self.validate_consensus_data(block).await? {
            return Ok(ValidationResult::Invalid(
                "Invalid consensus data".to_string(),
            ));
        }

        Ok(ValidationResult::Valid)
    }
}

/// Configuration trait for consensus algorithms
pub trait ConsensusConfig: Clone + Send + Sync + Debug {
    /// Validate the configuration
    fn validate(&self) -> Result<(), String>;
}

/// Node identifier type
pub type NodeId = String;

/// Consensus statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusStats {
    /// Current blockchain height
    pub current_height: u64,
    /// Current consensus round
    pub current_round: u32,
    /// Total number of committed blocks
    pub total_blocks: u64,
    /// Total number of processed transactions
    pub total_transactions: u64,
    /// Average block time in milliseconds
    pub avg_block_time: u64,
    /// Number of active consensus participants
    pub active_nodes: u32,
    /// Current consensus algorithm
    pub algorithm: String,
    /// Consensus uptime in seconds
    pub uptime: u64,
    /// Last block timestamp
    pub last_block_time: SystemTime,
}

/// Cross-VM state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVMState {
    /// Current height of the state
    pub height: u64,
    /// Hash of the SVM state root
    pub svm_state_root: String,
    /// Hash of the EVM state root
    pub evm_state_root: String,
    /// MultiVM account bindings
    pub account_bindings: Vec<ConsensusAccountBinding>,
    /// Global nonce for cross-VM operations
    pub global_nonce: u64,
    /// Timestamp of this state
    pub timestamp: SystemTime,
}

/// Consensus-specific account binding state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusAccountBinding {
    /// MultiVM account identifier
    // pub multivm_account: MultivmAccountId,
    /// Associated VM addresses
    pub bound_addresses: Vec<String>,
    /// Binding metadata
    pub metadata: serde_json::Value,
}

/// State change representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Type of state change
    pub change_type: StateChangeType,
    /// Account or entity affected
    pub target: String,
    /// Previous value (if applicable)
    pub previous_value: Option<serde_json::Value>,
    /// New value
    pub new_value: serde_json::Value,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Types of state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateChangeType {
    /// Account balance change
    BalanceUpdate,
    /// Account binding creation
    BindingCreated,
    /// Account binding update
    BindingUpdated,
    /// Account binding removal
    BindingRemoved,
    /// Contract state change
    ContractState,
    /// Custom state change
    Custom(String),
}

/// Validation result for transactions and blocks
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Validation passed
    Valid,
    /// Validation failed with reason
    Invalid(String),
    /// Validation pending (needs more information)
    Pending,
}

/// State checkpoint for recovery and synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    /// Height at which checkpoint was created
    pub height: u64,
    /// Hash of the complete state
    pub state_hash: String,
    /// Cross-VM state at checkpoint
    pub cross_vm_state: CrossVMState,
    /// Timestamp of checkpoint creation
    pub timestamp: SystemTime,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl Default for ConsensusStats {
    fn default() -> Self {
        Self {
            current_height: 0,
            current_round: 0,
            total_blocks: 0,
            total_transactions: 0,
            avg_block_time: 0,
            active_nodes: 0,
            algorithm: "unknown".to_string(),
            uptime: 0,
            last_block_time: SystemTime::now(),
        }
    }
}

impl ValidationResult {
    /// Check if the validation result indicates success
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    /// Check if the validation result indicates failure
    pub fn is_invalid(&self) -> bool {
        matches!(self, ValidationResult::Invalid(_))
    }

    /// Check if the validation result is pending
    pub fn is_pending(&self) -> bool {
        matches!(self, ValidationResult::Pending)
    }

    /// Get the error message if validation failed
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ValidationResult::Invalid(msg) => Some(msg),
            _ => None,
        }
    }
}

