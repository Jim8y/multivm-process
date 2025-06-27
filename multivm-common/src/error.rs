//! Unified error handling system for the MultiVM platform
//!
//! This module provides a consistent error handling framework with:
//! - Structured error types with detailed context
//! - Error categorization for recovery strategies
//! - Consistent error conversion patterns

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Main error type for the multi-VM system
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum MultivmError {
    /// Configuration-related errors
    #[error("Configuration error in {component}: {message}")]
    Configuration {
        component: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_errors: Option<Vec<String>>,
    },

    /// Process management errors
    #[error("Process error in {process_id}: {message}")]
    Process {
        process_id: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },

    /// IPC communication errors
    #[error("IPC communication error on {endpoint}: {message}")]
    Ipc {
        endpoint: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_count: Option<u32>,
    },

    /// Blockchain-specific execution errors
    #[error("{vm_type} engine error: {message}")]
    VmEngine {
        vm_type: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        block_info: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transaction_info: Option<String>,
    },

    /// RPC-related errors
    #[error("RPC error on {method}: {message}")]
    Rpc {
        method: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },

    /// Storage and persistence errors
    #[error("Storage error in {operation}: {message}")]
    Storage {
        operation: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },

    /// Network connectivity and communication errors
    #[error("Network error: {message}")]
    Network {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after: Option<Duration>,
    },

    /// Serialization and data format errors
    #[error("Serialization error: {message}")]
    Serialization {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data_type: Option<String>,
    },

    /// Operation timeout errors
    #[error("Timeout error: {operation} timed out after {timeout:?}")]
    Timeout {
        operation: String,
        timeout: Duration,
        #[serde(skip_serializing_if = "Option::is_none")]
        partial_result: Option<String>,
    },

    /// Resource exhaustion errors
    #[error("Resource limit exceeded: {resource} used {current}/{limit}")]
    ResourceLimit {
        resource: String,
        current: String,
        limit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggested_action: Option<String>,
    },

    /// Invalid state transition errors
    #[error("Invalid state transition: {message}")]
    InvalidState {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_state: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_state: Option<String>,
    },

    /// Authentication and authorization errors
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed {
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        required_permissions: Option<Vec<String>>,
    },

    /// Rate limiting errors
    #[error("Rate limited: {message}")]
    RateLimited {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after: Option<Duration>,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_rate: Option<f64>,
    },

    /// Account mapping layer errors
    #[error("Account mapping error: {message}")]
    AccountMapping {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_chain: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_chain: Option<String>,
    },

    /// Consensus mechanism errors
    #[error("Consensus error: {message}")]
    ConsensusError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        round: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validator_count: Option<u32>,
    },

    /// Validation errors
    #[error("Validation error: {field}: {message}")]
    Validation {
        field: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },

    /// Resource not found errors
    #[error("Not found: {resource}")]
    NotFound {
        resource: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        resource_id: Option<String>,
    },

    /// Not implemented functionality errors
    #[error("Not implemented: {feature}")]
    NotImplemented {
        feature: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alternatives: Option<Vec<String>>,
    },

    /// Internal system errors
    #[error("Internal error in {component}: {message}")]
    Internal {
        component: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
    },

    /// Encryption/decryption errors
    #[error("Encryption failed: {message}")]
    EncryptionFailed {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        algorithm: Option<String>,
    },

    /// Block processing errors
    #[error("Block processing error: {message}")]
    BlockProcessing {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        block_number: Option<u64>,
    },

    /// Unsupported operation errors
    #[error("Unsupported operation: {operation}")]
    UnsupportedOperation {
        operation: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alternatives: Option<Vec<String>>,
    },

    /// Catch-all for unexpected errors
    #[error("Unknown error: {message}")]
    Unknown {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_source: Option<String>,
    },
}

/// Error categories for better error handling
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
            MultivmError::Configuration { .. } => ErrorCategory::Configuration,
            MultivmError::Process { .. } => ErrorCategory::Fatal,
            MultivmError::Ipc { .. } => ErrorCategory::Transient,
            MultivmError::VmEngine { .. } => ErrorCategory::Recoverable,
            MultivmError::Rpc { .. } => ErrorCategory::Transient,
            MultivmError::Storage { .. } => ErrorCategory::Fatal,
            MultivmError::Network { .. } => ErrorCategory::Transient,
            MultivmError::Serialization { .. } => ErrorCategory::Fatal,
            MultivmError::Timeout { .. } => ErrorCategory::Transient,
            MultivmError::ResourceLimit { .. } => ErrorCategory::Resource,
            MultivmError::InvalidState { .. } => ErrorCategory::Fatal,
            MultivmError::AuthenticationFailed { .. } => ErrorCategory::Configuration,
            MultivmError::RateLimited { .. } => ErrorCategory::Resource,
            MultivmError::AccountMapping { .. } => ErrorCategory::Fatal,
            MultivmError::ConsensusError { .. } => ErrorCategory::Recoverable,
            MultivmError::Validation { .. } => ErrorCategory::Configuration,
            MultivmError::NotFound { .. } => ErrorCategory::Fatal,
            MultivmError::NotImplemented { .. } => ErrorCategory::Configuration,
            MultivmError::EncryptionFailed { .. } => ErrorCategory::Fatal,
            MultivmError::BlockProcessing { .. } => ErrorCategory::Recoverable,
            MultivmError::UnsupportedOperation { .. } => ErrorCategory::Configuration,
            MultivmError::Internal { .. } => ErrorCategory::Fatal,
            MultivmError::Unknown { .. } => ErrorCategory::Fatal,
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

    /// Get retry information for recoverable errors
    pub fn retry_info(&self) -> Option<Duration> {
        match self {
            MultivmError::Network { retry_after, .. } => *retry_after,
            MultivmError::RateLimited { retry_after, .. } => *retry_after,
            MultivmError::Timeout { .. } if self.is_retryable() => Some(Duration::from_secs(1)),
            MultivmError::Ipc { .. } if self.is_retryable() => Some(Duration::from_millis(500)),
            _ => None,
        }
    }

    /// Get HTTP status code for API responses
    pub fn http_status(&self) -> u16 {
        match self {
            MultivmError::Validation { .. } => 400, // Bad Request
            MultivmError::AuthenticationFailed { .. } => 401, // Unauthorized
            MultivmError::NotFound { .. } => 404,   // Not Found
            MultivmError::RateLimited { .. } => 429, // Too Many Requests
            MultivmError::Internal { .. } | MultivmError::Unknown { .. } => 500, // Internal Server Error
            MultivmError::Timeout { .. } => 504,                                 // Gateway Timeout
            _ => 500, // Default to Internal Server Error
        }
    }

    /// Create error with context
    pub fn with_context(self, context: &str) -> Self {
        match self {
            MultivmError::Unknown {
                message,
                error_source,
            } => MultivmError::Unknown {
                message: format!("{context}: {message}"),
                error_source,
            },
            other => other,
        }
    }
}

// Conversion implementations for external error types
impl From<std::io::Error> for MultivmError {
    fn from(err: std::io::Error) -> Self {
        Self::Storage {
            operation: "io_operation".to_string(),
            message: err.to_string(),
            path: None,
        }
    }
}

impl From<serde_json::Error> for MultivmError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            message: err.to_string(),
            data_type: Some("json".to_string()),
        }
    }
}

impl From<tokio::time::error::Elapsed> for MultivmError {
    fn from(_err: tokio::time::error::Elapsed) -> Self {
        Self::Timeout {
            operation: "unknown".to_string(),
            timeout: Duration::from_secs(30),
            partial_result: None,
        }
    }
}

impl From<toml::de::Error> for MultivmError {
    fn from(err: toml::de::Error) -> Self {
        Self::Configuration {
            component: "config".to_string(),
            message: format!("TOML parsing error: {err}"),
            validation_errors: None,
        }
    }
}

impl From<toml::ser::Error> for MultivmError {
    fn from(err: toml::ser::Error) -> Self {
        Self::Configuration {
            component: "config".to_string(),
            message: format!("TOML serialization error: {err}"),
            validation_errors: None,
        }
    }
}

/// Result type alias for convenience
pub type MultivmResult<T> = std::result::Result<T, MultivmError>;

/// Convenience macros for creating errors
#[macro_export]
macro_rules! multivm_error {
    (Configuration, $component:expr, $msg:expr) => {
        MultivmError::Configuration {
            component: $component.to_string(),
            message: $msg.to_string(),
            validation_errors: None,
        }
    };
    (Process, $process_id:expr, $msg:expr) => {
        MultivmError::Process {
            process_id: $process_id.to_string(),
            message: $msg.to_string(),
            exit_code: None,
        }
    };
    (VmEngine, $vm_type:expr, $msg:expr) => {
        MultivmError::VmEngine {
            vm_type: $vm_type.to_string(),
            message: $msg.to_string(),
            block_info: None,
            transaction_info: None,
        }
    };
    (Network, $msg:expr) => {
        MultivmError::Network {
            message: $msg.to_string(),
            endpoint: None,
            retry_after: None,
        }
    };
    (Storage, $operation:expr, $msg:expr) => {
        MultivmError::Storage {
            operation: $operation.to_string(),
            message: $msg.to_string(),
            path: None,
        }
    };
    (Validation, $field:expr, $msg:expr) => {
        MultivmError::Validation {
            field: $field.to_string(),
            message: $msg.to_string(),
            value: None,
        }
    };
    (NotFound, $resource:expr) => {
        MultivmError::NotFound {
            resource: $resource.to_string(),
            resource_id: None,
        }
    };
    (Internal, $component:expr, $msg:expr) => {
        MultivmError::Internal {
            component: $component.to_string(),
            message: $msg.to_string(),
            error_code: None,
        }
    };
}

/// Macro for early return with validation
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        if !($cond) {
            return Err($err);
        }
    };
}
