//! Network-specific error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("TLS error: {0}")]
    Tls(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Message too large: {size} bytes (max: {max})")]
    MessageTooLarge { size: usize, max: usize },
    
    #[error("Invalid message format")]
    InvalidMessage,
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Shutdown")]
    Shutdown,
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Encryption/Decryption error: {0}")]
    Encryption(String),
}

pub type Result<T> = std::result::Result<T, NetworkError>;

impl From<NetworkError> for consensus::ConsensusError {
    fn from(err: NetworkError) -> Self {
        consensus::ConsensusError::Network(err.to_string())
    }
}