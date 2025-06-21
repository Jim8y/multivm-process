use thiserror::Error;

/// Main error type for the multi-VM system
#[derive(Error, Debug, Clone)]
pub enum MultivmError {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("IPC communication error: {0}")]
    Ipc(String),

    #[error("Solana engine error: {0}")]
    Solana(String),

    #[error("Ethereum engine error: {0}")]
    Ethereum(String),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Timeout error: operation timed out after {timeout:?}")]
    Timeout { timeout: std::time::Duration },

    #[error("Resource limit exceeded: {resource} exceeded limit {limit}")]
    ResourceLimit { resource: String, limit: String },

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Block processing error: {0}")]
    BlockProcessing(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Unknown error: {0}")]
    Unknown(String),

    /// Account mapping layer error
    #[error("Account mapping error: {0}")]
    AccountMapping(String),

    #[error("Consensus error: {0}")]
    ConsensusError(String),
}

// Conversion from common error types
impl From<std::io::Error> for MultivmError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<serde_json::Error> for MultivmError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

// bincode dependency removed - use serde_json for serialization
// impl From<bincode::Error> for MultivmError {
//     fn from(err: bincode::Error) -> Self {
//         Self::Serialization(err.to_string())
//     }
// }

impl From<tokio::time::error::Elapsed> for MultivmError {
    fn from(_err: tokio::time::error::Elapsed) -> Self {
        Self::Timeout {
            timeout: std::time::Duration::from_secs(0), // Default timeout
        }
    }
}

// Error categories for better error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Errors that can be retried
    Recoverable,
    /// Errors that require intervention
    Fatal,
    /// Temporary errors that may resolve themselves
    Transient,
    /// Configuration errors
    Configuration,
    /// Resource exhaustion errors
    Resource,
}

impl MultivmError {
    /// Categorize the error for handling decisions
    pub fn category(&self) -> ErrorCategory {
        match self {
            MultivmError::Configuration(_) => ErrorCategory::Configuration,
            MultivmError::Process(_) => ErrorCategory::Fatal,
            MultivmError::Ipc(_) => ErrorCategory::Transient,
            MultivmError::Solana(_) => ErrorCategory::Recoverable,
            MultivmError::Ethereum(_) => ErrorCategory::Recoverable,
            MultivmError::Rpc(_) => ErrorCategory::Transient,
            MultivmError::Storage(_) => ErrorCategory::Fatal,
            MultivmError::Network(_) => ErrorCategory::Transient,
            MultivmError::Serialization(_) => ErrorCategory::Fatal,
            MultivmError::Timeout { .. } => ErrorCategory::Transient,
            MultivmError::ResourceLimit { .. } => ErrorCategory::Resource,
            MultivmError::InvalidState(_) => ErrorCategory::Fatal,
            MultivmError::UnsupportedOperation(_) => ErrorCategory::Configuration,
            MultivmError::BlockProcessing(_) => ErrorCategory::Recoverable,
            MultivmError::PermissionDenied(_) => ErrorCategory::Configuration,
            MultivmError::Io(_) => ErrorCategory::Transient,
            MultivmError::Unknown(_) => ErrorCategory::Fatal,
            MultivmError::AccountMapping(_) => ErrorCategory::Fatal,
            MultivmError::AuthenticationFailed(_) => ErrorCategory::Configuration,
            MultivmError::RateLimited(_) => ErrorCategory::Resource,
            MultivmError::EncryptionError(_) => ErrorCategory::Fatal,
            MultivmError::ConsensusError(_) => ErrorCategory::Recoverable,
        }
    }

    /// Check if this error should trigger a retry
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::Recoverable | ErrorCategory::Transient
        )
    }

    /// Check if this error is fatal and should stop the process
    pub fn is_fatal(&self) -> bool {
        matches!(self.category(), ErrorCategory::Fatal)
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            MultivmError::Configuration(msg) => {
                format!("Configuration issue: {}. Please check your settings.", msg)
            }
            MultivmError::Process(msg) => {
                format!("Process error: {}. The system may need to restart.", msg)
            }
            MultivmError::Timeout { timeout } => {
                format!("Operation timed out after {:?}. Please try again.", timeout)
            }
            MultivmError::ResourceLimit { resource, limit } => {
                format!(
                    "Resource limit exceeded: {} has reached the limit of {}. \
                     Please free up resources or increase limits.",
                    resource, limit
                )
            }
            MultivmError::Network(msg) => {
                format!(
                    "Network connectivity issue: {}. Please check your connection.",
                    msg
                )
            }
            _ => self.to_string(),
        }
    }

    /// Create a context wrapper for the error
    pub fn with_context(self, context: &str) -> Self {
        match self {
            MultivmError::Unknown(msg) => MultivmError::Unknown(format!("{}: {}", context, msg)),
            other => other,
        }
    }
}

/// Result type alias for convenience
pub type MultivmResult<T> = std::result::Result<T, MultivmError>;

/// Error context for tracking error chains
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub component: String,
    pub timestamp: std::time::SystemTime,
    pub additional_info: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    pub fn new(operation: &str, component: &str) -> Self {
        Self {
            operation: operation.to_string(),
            component: component.to_string(),
            timestamp: std::time::SystemTime::now(),
            additional_info: std::collections::HashMap::new(),
        }
    }

    pub fn with_info(mut self, key: &str, value: &str) -> Self {
        self.additional_info
            .insert(key.to_string(), value.to_string());
        self
    }
}

/// Trait for adding context to errors
pub trait ErrorContextExt<T> {
    fn with_context(self, context: ErrorContext) -> MultivmResult<T>;
    fn with_operation(self, operation: &str, component: &str) -> MultivmResult<T>;
}

impl<T, E> ErrorContextExt<T> for std::result::Result<T, E>
where
    E: Into<MultivmError>,
{
    fn with_context(self, context: ErrorContext) -> MultivmResult<T> {
        self.map_err(|e| {
            let mut error = e.into();
            error = error.with_context(&format!("{}::{}", context.component, context.operation));
            error
        })
    }

    fn with_operation(self, operation: &str, component: &str) -> MultivmResult<T> {
        self.with_context(ErrorContext::new(operation, component))
    }
}

/// Macro for creating contextual errors
#[macro_export]
macro_rules! multivm_error {
    ($variant:ident, $msg:expr) => {
        MultivmError::$variant($msg.to_string())
    };
    ($variant:ident, $fmt:expr, $($arg:tt)*) => {
        MultivmError::$variant(format!($fmt, $($arg)*))
    };
}

/// Macro for early return with context
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        if !($cond) {
            return Err($err);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let config_err = MultivmError::Configuration("test".to_string());
        assert_eq!(config_err.category(), ErrorCategory::Configuration);
        assert!(!config_err.is_retryable());

        let timeout_err = MultivmError::Timeout {
            timeout: std::time::Duration::from_secs(5),
        };
        assert_eq!(timeout_err.category(), ErrorCategory::Transient);
        assert!(timeout_err.is_retryable());
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let multivm_err: MultivmError = io_err.into();

        match multivm_err {
            MultivmError::Io(msg) => assert!(msg.contains("file not found")),
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_error_context() {
        let ctx = ErrorContext::new("test_operation", "test_component")
            .with_info("key1", "value1")
            .with_info("key2", "value2");

        assert_eq!(ctx.operation, "test_operation");
        assert_eq!(ctx.component, "test_component");
        assert_eq!(ctx.additional_info.len(), 2);
    }

    #[test]
    fn test_user_message() {
        let timeout_err = MultivmError::Timeout {
            timeout: std::time::Duration::from_secs(30),
        };

        let user_msg = timeout_err.user_message();
        assert!(user_msg.contains("30s"));
        assert!(user_msg.contains("try again"));
    }

    #[test]
    fn test_multivm_error_macro() {
        let err = multivm_error!(Configuration, "Invalid setting: {}", "test_setting");
        match err {
            MultivmError::Configuration(msg) => {
                assert_eq!(msg, "Invalid setting: test_setting");
            }
            _ => panic!("Expected Configuration error"),
        }
    }
}
