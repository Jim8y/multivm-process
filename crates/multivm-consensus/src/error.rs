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
        }
    }
}

/// Convert to MultivmError for compatibility with other layers
impl From<ConsensusError> for MultivmError {
    fn from(err: ConsensusError) -> Self {
        match err {
            ConsensusError::Network(msg) => MultivmError::Network(msg),
            ConsensusError::Storage(msg) => MultivmError::Storage(msg),
            ConsensusError::Configuration(msg) => MultivmError::Configuration(msg),
            ConsensusError::Timeout { timeout } => MultivmError::Timeout {
                timeout: std::time::Duration::from_millis(timeout),
            },
            other => MultivmError::Unknown(format!("Consensus error: {}", other)),
        }
    }
}

/// Convert from MultivmError
impl From<MultivmError> for ConsensusError {
    fn from(err: MultivmError) -> Self {
        match err {
            MultivmError::Network(msg) => ConsensusError::Network(msg),
            MultivmError::Storage(msg) => ConsensusError::Storage(msg),
            MultivmError::Configuration(msg) => ConsensusError::Configuration(msg),
            MultivmError::Timeout { timeout } => ConsensusError::Timeout {
                timeout: timeout.as_millis() as u64,
            },
            other => ConsensusError::Internal(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let error = ConsensusError::InvalidBlock("test".to_string());
        assert_eq!(error.category(), "validation");
        assert!(!error.is_recoverable());
        assert!(!error.is_critical());

        let error = ConsensusError::ForkDetected { height: 100 };
        assert_eq!(error.category(), "fork");
        assert!(!error.is_recoverable());
        assert!(error.is_critical());

        let error = ConsensusError::Timeout { timeout: 5000 };
        assert_eq!(error.category(), "timeout");
        assert!(error.is_recoverable());
        assert!(!error.is_critical());
    }

    #[test]
    fn test_error_conversion() {
        let consensus_error = ConsensusError::Network("test error".to_string());
        let multivm_error: MultivmError = consensus_error.into();

        match multivm_error {
            MultivmError::Network(msg) => assert!(msg.contains("test error")),
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_insufficient_votes_error() {
        let error = ConsensusError::InsufficientVotes {
            required: 5,
            actual: 3,
        };

        assert!(error.is_recoverable());
        assert!(!error.is_critical());
        assert_eq!(error.category(), "consensus");

        let error_str = error.to_string();
        assert!(error_str.contains("required 5"));
        assert!(error_str.contains("got 3"));
    }

    #[test]
    fn test_view_change_error() {
        let error = ConsensusError::ViewChangeInProgress {
            old_view: 1,
            new_view: 2,
        };

        assert!(error.is_recoverable());
        assert!(!error.is_critical());
        assert_eq!(error.category(), "view_change");
    }
}
