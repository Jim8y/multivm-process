//! Error types for the MultiVM RPC service

use multivm_common::MultivmError;
use thiserror::Error;

/// RPC service specific errors
#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Invalid RPC method: {method}")]
    InvalidMethod { method: String },

    #[error("Invalid RPC parameters: {reason}")]
    InvalidParams { reason: String },

    #[error("RPC relay error for {vm_type}: {message}")]
    RelayError { vm_type: String, message: String },

    #[error("Backend unavailable: {backend}")]
    BackendUnavailable { backend: String },

    #[error("Rate limit exceeded for client: {client_id}")]
    RateLimitExceeded { client_id: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Cache error: {message}")]
    CacheError { message: String },

    #[error("Proxy configuration error: {message}")]
    ProxyError { message: String },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Timeout error: {operation}")]
    TimeoutError { operation: String },

    #[error("Internal error: {message}")]
    Internal { message: String },

    #[error("Account mapping error: {error}")]
    AccountMapping {
        #[from]
        error: multivm_account_mapping::AccountMappingError,
    },

    #[error("MultiVM error: {error}")]
    Multivm {
        #[from] 
        error: MultivmError,
    },
}

impl From<serde_json::Error> for RpcError {
    fn from(err: serde_json::Error) -> Self {
        RpcError::SerializationError {
            message: err.to_string(),
        }
    }
}

impl From<reqwest::Error> for RpcError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            RpcError::TimeoutError {
                operation: "HTTP request".to_string(),
            }
        } else {
            RpcError::NetworkError {
                message: err.to_string(),
            }
        }
    }
}

impl From<std::io::Error> for RpcError {
    fn from(err: std::io::Error) -> Self {
        RpcError::Internal {
            message: err.to_string(),
        }
    }
}

impl From<jsonrpsee::core::ClientError> for RpcError {
    fn from(err: jsonrpsee::core::ClientError) -> Self {
        RpcError::RelayError {
            vm_type: "unknown".to_string(),
            message: err.to_string(),
        }
    }
}

/// Result type for RPC operations
pub type RpcResult<T> = Result<T, RpcError>;

/// Convert RpcError to JSON-RPC error response
impl RpcError {
    pub fn to_jsonrpc_error(&self) -> crate::types::JsonRpcError {
        use crate::types::JsonRpcError;
        use serde_json::Value;
        
        match self {
            RpcError::InvalidMethod { .. } => {
                JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                }
            }
            RpcError::InvalidParams { reason } => {
                JsonRpcError {
                    code: -32602,
                    message: "Invalid params".to_string(),
                    data: Some(Value::String(reason.clone())),
                }
            }
            RpcError::RateLimitExceeded { .. } => {
                JsonRpcError {
                    code: -32000,
                    message: "Rate limit exceeded".to_string(),
                    data: None,
                }
            }
            RpcError::AuthenticationFailed { .. } => {
                JsonRpcError {
                    code: -32001,
                    message: "Authentication failed".to_string(),
                    data: None,
                }
            }
            RpcError::BackendUnavailable { backend } => {
                JsonRpcError {
                    code: -32002,
                    message: "Backend unavailable".to_string(),
                    data: Some(Value::String(backend.clone())),
                }
            }
            RpcError::TimeoutError { operation } => {
                JsonRpcError {
                    code: -32003,
                    message: "Request timeout".to_string(),
                    data: Some(Value::String(operation.clone())),
                }
            }
            _ => {
                JsonRpcError {
                    code: -32000,
                    message: "Internal error".to_string(),
                    data: Some(Value::String(self.to_string())),
                }
            }
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            RpcError::NetworkError { .. } |
            RpcError::TimeoutError { .. } |
            RpcError::BackendUnavailable { .. }
        )
    }

    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            RpcError::InvalidMethod { .. } | RpcError::InvalidParams { .. } => "client_error",
            RpcError::RelayError { .. } => "relay_error",
            RpcError::BackendUnavailable { .. } => "backend_error",
            RpcError::RateLimitExceeded { .. } => "rate_limit",
            RpcError::AuthenticationFailed { .. } => "auth_error",
            RpcError::NetworkError { .. } | RpcError::TimeoutError { .. } => "network_error",
            _ => "internal_error",
        }
    }
}

/// Convenience macros for creating RPC errors
#[macro_export]
macro_rules! rpc_error {
    (InvalidMethod, $method:expr) => {
        RpcError::InvalidMethod {
            method: $method.to_string(),
        }
    };
    (InvalidParams, $reason:expr) => {
        RpcError::InvalidParams {
            reason: $reason.to_string(),
        }
    };
    (RelayError, $vm_type:expr, $message:expr) => {
        RpcError::RelayError {
            vm_type: $vm_type.to_string(),
            message: $message.to_string(),
        }
    };
    (BackendUnavailable, $backend:expr) => {
        RpcError::BackendUnavailable {
            backend: $backend.to_string(),
        }
    };
    (Internal, $message:expr) => {
        RpcError::Internal {
            message: $message.to_string(),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json");
        let rpc_err: RpcError = json_err.unwrap_err().into();
        
        match rpc_err {
            RpcError::SerializationError { .. } => {}
            _ => panic!("Expected SerializationError"),
        }
    }

    #[test]
    fn test_error_categories() {
        let errors = vec![
            (rpc_error!(InvalidMethod, "test"), "client_error"),
            (rpc_error!(RelayError, "eth", "test"), "relay_error"),
            (rpc_error!(BackendUnavailable, "reth"), "backend_error"),
        ];

        for (error, expected_category) in errors {
            assert_eq!(error.category(), expected_category);
        }
    }

    #[test]
    fn test_retryable_errors() {
        let retryable = RpcError::NetworkError {
            message: "Connection failed".to_string(),
        };
        assert!(retryable.is_retryable());

        let not_retryable = rpc_error!(InvalidMethod, "eth_invalid");
        assert!(!not_retryable.is_retryable());
    }
}