//! Common error types for the MultiVM system

use thiserror::Error;

/// Core error type for MultiVM system
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    /// Network error
    #[error("Network error: {0}")]
    Network(String),
    
    /// Storage error  
    #[error("Storage error: {0}")]
    Storage(String),
    
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),
    
    /// Operation timeout
    #[error("Operation timeout")]
    Timeout,
    
    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    /// Consensus error
    #[error("Consensus error: {0}")]
    Consensus(String),
    
    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Result type for MultiVM operations
pub type Result<T> = std::result::Result<T, Error>;

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Error::Timeout
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(err: std::net::AddrParseError) -> Self {
        Error::InvalidInput(format!("Invalid network address: {}", err))
    }
}


/// Add context to errors for better debugging
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context(self, msg: &str) -> Result<T>;
    
    /// Add context with a closure for lazy evaluation
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Other(format!("{}: {}", msg, base_error))
        })
    }
    
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            Error::Other(format!("{}: {}", f(), base_error))
        })
    }
}