//! Error types for the consensus module

use thiserror::Error;

/// Result type alias for consensus operations
pub type Result<T> = std::result::Result<T, ConsensusError>;

/// Main error type for consensus operations
#[derive(Error, Debug)]
pub enum ConsensusError {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),
    
    /// Storage-related errors
    #[error("Storage error: {0}")]
    Storage(String),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    /// Leadership-related errors
    #[error("Not leader")]
    NotLeader,
    
    /// Node is shutting down
    #[error("Node is shutting down")]
    ShuttingDown,
    
    /// Timeout occurred
    #[error("Operation timed out")]
    Timeout,
    
    /// Invalid state transition
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: crate::NodeState,
        to: crate::NodeState,
    },
    
    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Channel send errors
    #[error("Channel send error")]
    ChannelSend,
    
    /// Channel receive errors
    #[error("Channel receive error")]
    ChannelReceive,
    
    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),
    
    /// Other error
    #[error("{0}")]
    Other(String),
}