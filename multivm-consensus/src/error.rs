//! Error types for the MultiVM consensus layer

use multivm_common::MultivmError;
use thiserror::Error;

/// Result type for consensus operations
pub type ConsensusResult<T> = Result<T, ConsensusError>;

/// Comprehensive error types for consensus operations
#[derive(Error, Debug, Clone)]
pub enum ConsensusError {
    /// Invalid block format or content
    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    /// Invalid transaction format or content
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    /// Validation failed for a block or transaction
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Consensus timeout occurred
    #[error("Consensus timeout: operation took longer than {timeout}ms")]
    Timeout { timeout: u64 },

    /// Invalid message format or content
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),

    /// Node is not the leader/proposer
    #[error("Not leader: {message}")]
    NotLeader { message: String },

    /// Invalid consensus state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// State management error
    #[error("State error: {0}")]
    StateError(String),

    /// Insufficient votes for consensus
    #[error("Insufficient votes: required {required}, got {actual}")]
    InsufficientVotes { required: usize, actual: usize },

    /// Insufficient votes for finalization with round info
    #[error("Insufficient votes for round {round}: received {received}, required {required}")]
    InsufficientVotesForRound {
        received: usize,
        required: usize,
        round: u64,
    },

    /// Fork detected in the blockchain
    #[error("Fork detected at height {height}")]
    ForkDetected { height: u64 },

    /// State synchronization failed
    #[error("State sync failed: {0}")]
    StateSyncFailed(String),

    /// Cryptographic operation failed
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Storage/persistence error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Resource exhaustion (memory, disk, etc.)
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Algorithm-specific error
    #[error("Algorithm error: {0}")]
    Algorithm(String),

    /// View change in progress
    #[error("View change in progress from {old_view} to {new_view}")]
    ViewChangeInProgress { old_view: u32, new_view: u32 },

    /// Duplicate message received
    #[error("Duplicate message: {message_type} from {sender}")]
    DuplicateMessage {
        message_type: String,
        sender: String,
    },

    /// Message verification failed
    #[error("Message verification failed: {0}")]
    MessageVerificationFailed(String),

    /// Internal system error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Already running error
    #[error("Consensus engine is already running")]
    AlreadyRunning,

    /// Network error (alias for Network for backward compatibility)
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Validator not found error
    #[error("Validator not found: {0}")]
    ValidatorNotFound(String),

    /// Invalid proposal error
    #[error("Invalid proposal: {0}")]
    InvalidProposal(String),

    /// Invalid round error
    #[error("Invalid round: expected {expected}, received {received}")]
    InvalidRound { expected: u64, received: u64 },

    /// Unauthorized validator error
    #[error("Unauthorized validator: {validator_id}")]
    UnauthorizedValidator { validator_id: String },

    /// Duplicate vote error
    #[error("Duplicate vote from validator {validator_id} in round {round}")]
    DuplicateVote { validator_id: String, round: u64 },
}

impl ConsensusError {
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            ConsensusError::InvalidBlock(_) => false,
            ConsensusError::InvalidTransaction(_) => false,
            ConsensusError::ValidationFailed(_) => false,
            ConsensusError::Timeout { .. } => true,
            ConsensusError::Network(_) => true,
            ConsensusError::NotLeader { .. } => true,
            ConsensusError::InvalidState(_) => false,
            ConsensusError::StateError(_) => true,
            ConsensusError::InsufficientVotes { .. } => true,
            ConsensusError::ForkDetected { .. } => false,
            ConsensusError::StateSyncFailed(_) => true,
            ConsensusError::Crypto(_) => false,
            ConsensusError::Configuration(_) => false,
            ConsensusError::Storage(_) => true,
            ConsensusError::ResourceExhausted(_) => true,
            ConsensusError::Algorithm(_) => false,
            ConsensusError::ViewChangeInProgress { .. } => true,
            ConsensusError::DuplicateMessage { .. } => true,
            ConsensusError::MessageVerificationFailed(_) => false,
            ConsensusError::Internal(_) => false,
            ConsensusError::AlreadyRunning => true,
            ConsensusError::InvalidMessage(_) => false,
            ConsensusError::NetworkError(_) => true,
            ConsensusError::SerializationError(_) => false,
            ConsensusError::ValidatorNotFound(_) => true,
            ConsensusError::InsufficientVotesForRound { .. } => true,
            ConsensusError::InvalidProposal(_) => false,
            ConsensusError::InvalidRound { .. } => false,
            ConsensusError::UnauthorizedValidator { .. } => false,
            ConsensusError::DuplicateVote { .. } => true,
        }
    }

    /// Check if the error is critical (requires immediate attention)
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            ConsensusError::ForkDetected { .. }
                | ConsensusError::Crypto(_)
                | ConsensusError::ResourceExhausted(_)
                | ConsensusError::Internal(_)
        )
    }

    /// Get error category for metrics and logging
    pub fn category(&self) -> &'static str {
        match self {
            ConsensusError::InvalidBlock(_) => "validation",
            ConsensusError::InvalidTransaction(_) => "validation",
            ConsensusError::ValidationFailed(_) => "validation",
            ConsensusError::Timeout { .. } => "timeout",
            ConsensusError::Network(_) => "network",
            ConsensusError::NotLeader { .. } => "leadership",
            ConsensusError::InvalidState(_) => "state",
            ConsensusError::StateError(_) => "state",
            ConsensusError::InsufficientVotes { .. } => "consensus",
            ConsensusError::ForkDetected { .. } => "fork",
            ConsensusError::StateSyncFailed(_) => "sync",
            ConsensusError::Crypto(_) => "crypto",
            ConsensusError::Configuration(_) => "config",
            ConsensusError::Storage(_) => "storage",
            ConsensusError::ResourceExhausted(_) => "resources",
            ConsensusError::Algorithm(_) => "algorithm",
            ConsensusError::ViewChangeInProgress { .. } => "view_change",
            ConsensusError::DuplicateMessage { .. } => "duplicate",
            ConsensusError::MessageVerificationFailed(_) => "verification",
            ConsensusError::Internal(_) => "internal",
            ConsensusError::AlreadyRunning => "state",
            ConsensusError::InvalidMessage(_) => "validation",
            ConsensusError::NetworkError(_) => "network",
            ConsensusError::SerializationError(_) => "serialization",
            ConsensusError::ValidatorNotFound(_) => "validation",
            ConsensusError::InsufficientVotesForRound { .. } => "consensus",
            ConsensusError::InvalidProposal(_) => "validation",
            ConsensusError::InvalidRound { .. } => "validation",
            ConsensusError::UnauthorizedValidator { .. } => "security",
            ConsensusError::DuplicateVote { .. } => "duplicate",
        }
    }
}

/// Convert to MultivmError for compatibility with other layers
impl From<ConsensusError> for MultivmError {
    fn from(err: ConsensusError) -> Self {
        match err {
            ConsensusError::Network(msg) => MultivmError::Network {
                message: msg,
                endpoint: None,
                retry_after: None,
            },
            ConsensusError::Storage(msg) => MultivmError::Storage {
                operation: "consensus".to_string(),
                message: msg,
                path: None,
            },
            ConsensusError::Configuration(msg) => MultivmError::Configuration {
                component: "consensus".to_string(),
                message: msg,
                validation_errors: None,
            },
            ConsensusError::Timeout { timeout } => MultivmError::Timeout {
                operation: "consensus".to_string(),
                timeout: std::time::Duration::from_millis(timeout),
                partial_result: None,
            },
            other => MultivmError::Unknown {
                message: format!("Consensus error: {other}"),
                error_source: None,
            },
        }
    }
}

/// Convert from MultivmError
impl From<MultivmError> for ConsensusError {
    fn from(err: MultivmError) -> Self {
        match err {
            MultivmError::Network { message, .. } => ConsensusError::Network(message),
            MultivmError::Storage { message, .. } => ConsensusError::Storage(message),
            MultivmError::Configuration { message, .. } => ConsensusError::Configuration(message),
            MultivmError::Timeout { timeout, .. } => ConsensusError::Timeout {
                timeout: timeout.as_millis() as u64,
            },
            other => ConsensusError::Internal(other.to_string()),
        }
    }
}
