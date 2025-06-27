//! Simplified error types for the account mapping layer
//!
//! This module uses the unified error system from multivm-common
//! while providing account mapping specific error variants.

use multivm_common::MultivmError;
use thiserror::Error;

/// Account mapping specific errors that extend the base MultivmError
#[derive(Error, Debug, Clone)]
pub enum AccountMappingError {
    #[error("Account not found: {address}")]
    AccountNotFound { address: String },

    #[error("Invalid account address: {address}")]
    InvalidAddress { address: String },

    #[error("Account already bound: {address}")]
    AccountAlreadyBound { address: String },

    #[error("Binding proof validation failed: {reason}")]
    InvalidBindingProof { reason: String },

    #[error("Cross-VM operation not supported: {operation}")]
    UnsupportedOperation { operation: String },

    #[error("Transfer failed: {reason}")]
    TransferFailed { reason: String },

    #[error("Invalid binding: {reason}")]
    InvalidBinding { reason: String },

    #[error("Storage error: {message}")]
    Storage { message: String },

    #[error("IPC communication error: {message}")]
    IpcError { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },

    #[error("Unsupported account type: {account_type}")]
    UnsupportedAccountType { account_type: String },

    #[error("Invalid proof: {message}")]
    InvalidProof { message: String },
}

impl From<AccountMappingError> for MultivmError {
    fn from(err: AccountMappingError) -> Self {
        match err {
            AccountMappingError::AccountNotFound { address } => MultivmError::NotFound {
                resource: "account".to_string(),
                resource_id: Some(address),
            },
            AccountMappingError::InvalidAddress { address } => MultivmError::Validation {
                field: "address".to_string(),
                message: "Invalid address format".to_string(),
                value: Some(address),
            },
            AccountMappingError::AccountAlreadyBound { address } => MultivmError::InvalidState {
                message: format!("Account {address} is already bound"),
                current_state: Some("bound".to_string()),
                expected_state: Some("unbound".to_string()),
            },
            AccountMappingError::InvalidBindingProof { reason } => MultivmError::Validation {
                field: "binding_proof".to_string(),
                message: reason,
                value: None,
            },
            AccountMappingError::UnsupportedOperation { operation } => {
                MultivmError::NotImplemented {
                    feature: operation,
                    alternatives: None,
                }
            }
            AccountMappingError::TransferFailed { reason } => MultivmError::VmEngine {
                vm_type: "cross_vm".to_string(),
                message: format!("Transfer failed: {reason}"),
                block_info: None,
                transaction_info: None,
            },
            AccountMappingError::InvalidBinding { reason } => MultivmError::Validation {
                field: "binding".to_string(),
                message: reason,
                value: None,
            },
            AccountMappingError::Storage { message } => MultivmError::Storage {
                operation: "account_mapping".to_string(),
                message,
                path: None,
            },
            AccountMappingError::IpcError { message } => MultivmError::Ipc {
                endpoint: "account_mapping".to_string(),
                message,
                retry_count: None,
            },
            AccountMappingError::Internal { message } => MultivmError::Internal {
                component: "account_mapping".to_string(),
                message,
                error_code: None,
            },
            AccountMappingError::UnsupportedAccountType { account_type } => {
                MultivmError::NotImplemented {
                    feature: format!("Account type: {account_type}"),
                    alternatives: None,
                }
            }
            AccountMappingError::InvalidProof { message } => MultivmError::Validation {
                field: "proof".to_string(),
                message,
                value: None,
            },
        }
    }
}

impl From<MultivmError> for AccountMappingError {
    fn from(err: MultivmError) -> Self {
        match err {
            MultivmError::NotFound {
                resource,
                resource_id,
            } => AccountMappingError::AccountNotFound {
                address: resource_id.unwrap_or(resource),
            },
            MultivmError::Validation {
                field,
                message,
                value,
            } => {
                if field == "address" {
                    AccountMappingError::InvalidAddress {
                        address: value.unwrap_or_default(),
                    }
                } else if field == "binding_proof" {
                    AccountMappingError::InvalidBindingProof { reason: message }
                } else {
                    AccountMappingError::InvalidBinding { reason: message }
                }
            }
            MultivmError::Storage { message, .. } => AccountMappingError::Storage { message },
            MultivmError::Ipc { message, .. } => AccountMappingError::IpcError { message },
            _ => AccountMappingError::Internal {
                message: err.to_string(),
            },
        }
    }
}

// Conversion from common error types
impl From<std::io::Error> for AccountMappingError {
    fn from(err: std::io::Error) -> Self {
        AccountMappingError::Storage {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for AccountMappingError {
    fn from(err: serde_json::Error) -> Self {
        AccountMappingError::Internal {
            message: format!("JSON serialization error: {err}"),
        }
    }
}

impl From<bincode::Error> for AccountMappingError {
    fn from(err: bincode::Error) -> Self {
        AccountMappingError::Internal {
            message: format!("Binary serialization error: {err}"),
        }
    }
}

/// Result type for account mapping operations
pub type AccountMappingResult<T> = Result<T, AccountMappingError>;

/// Convenience macros for creating account mapping errors
#[macro_export]
macro_rules! account_mapping_error {
    (NotFound, $address:expr) => {
        AccountMappingError::AccountNotFound {
            address: $address.to_string(),
        }
    };
    (InvalidAddress, $address:expr) => {
        AccountMappingError::InvalidAddress {
            address: $address.to_string(),
        }
    };
    (AlreadyBound, $address:expr) => {
        AccountMappingError::AccountAlreadyBound {
            address: $address.to_string(),
        }
    };
    (InvalidProof, $reason:expr) => {
        AccountMappingError::InvalidBindingProof {
            reason: $reason.to_string(),
        }
    };
    (Unsupported, $operation:expr) => {
        AccountMappingError::UnsupportedOperation {
            operation: $operation.to_string(),
        }
    };
    (TransferFailed, $reason:expr) => {
        AccountMappingError::TransferFailed {
            reason: $reason.to_string(),
        }
    };
    (Storage, $message:expr) => {
        AccountMappingError::Storage {
            message: $message.to_string(),
        }
    };
    (Internal, $message:expr) => {
        AccountMappingError::Internal {
            message: $message.to_string(),
        }
    };
}

/// Utility functions for error handling
pub mod utils {
    use super::*;

    /// Check if an error is retryable
    pub fn is_retryable(error: &AccountMappingError) -> bool {
        matches!(
            error,
            AccountMappingError::IpcError { .. } | AccountMappingError::Storage { .. }
        )
    }

    /// Get a user-friendly error message
    pub fn user_message(error: &AccountMappingError) -> String {
        match error {
            AccountMappingError::AccountNotFound { address } => {
                format!("Account {address} was not found")
            }
            AccountMappingError::InvalidAddress { address } => {
                format!("The address {address} is not valid")
            }
            AccountMappingError::AccountAlreadyBound { address } => {
                format!("Account {address} is already bound to another account")
            }
            AccountMappingError::InvalidBindingProof { reason } => {
                format!("The binding proof is invalid: {reason}")
            }
            AccountMappingError::UnsupportedOperation { operation } => {
                format!("The operation {operation} is not supported")
            }
            AccountMappingError::TransferFailed { reason } => {
                format!("Transfer failed: {reason}")
            }
            AccountMappingError::InvalidBinding { reason } => {
                format!("Invalid binding: {reason}")
            }
            AccountMappingError::Storage { message } => {
                format!("Storage error: {message}")
            }
            AccountMappingError::IpcError { message } => {
                format!("Communication error: {message}")
            }
            AccountMappingError::Internal { message } => {
                format!("Internal error: {message}")
            }
            AccountMappingError::UnsupportedAccountType { account_type } => {
                format!("Account type {account_type} is not supported")
            }
            AccountMappingError::InvalidProof { message } => {
                format!("Invalid proof: {message}")
            }
        }
    }

    /// Convert an error to an HTTP status code
    pub fn to_http_status(error: &AccountMappingError) -> u16 {
        match error {
            AccountMappingError::AccountNotFound { .. } => 404,
            AccountMappingError::InvalidAddress { .. } => 400,
            AccountMappingError::AccountAlreadyBound { .. } => 409,
            AccountMappingError::InvalidBindingProof { .. } => 400,
            AccountMappingError::UnsupportedOperation { .. } => 501,
            AccountMappingError::TransferFailed { .. } => 500,
            AccountMappingError::InvalidBinding { .. } => 400,
            AccountMappingError::Storage { .. } => 500,
            AccountMappingError::IpcError { .. } => 503,
            AccountMappingError::Internal { .. } => 500,
            AccountMappingError::UnsupportedAccountType { .. } => 501,
            AccountMappingError::InvalidProof { .. } => 400,
        }
    }
}
