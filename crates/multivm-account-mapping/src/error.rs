//! Error types for the account mapping layer

use thiserror::Error;

/// Errors that can occur in the account mapping layer
#[derive(Error, Debug)]
pub enum AccountMappingError {
    #[error("Account not found: {address}")]
    AccountNotFound { address: String },

    #[error("Invalid account address: {address}")]
    InvalidAddress { address: String },

    #[error("Account already bound: {address}")]
    AccountAlreadyBound { address: String },

    #[error("Binding proof validation failed: {reason}")]
    InvalidBindingProof { reason: String },

    #[error("Invalid proof: {reason}")]
    InvalidProof { reason: String },

    #[error("Cross-VM operation not supported: {operation}")]
    UnsupportedOperation { operation: String },

    #[error("Unsupported account type: {account_type}")]
    UnsupportedAccountType { account_type: String },

    #[error("Transfer failed: {reason}")]
    TransferFailed { reason: String },

    #[error("Invalid binding: {reason}")]
    InvalidBinding { reason: String },

    #[error("Storage error: {source}")]
    Storage {
        #[from]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: bincode::Error,
    },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl From<AccountMappingError> for multivm_common::error::MultivmError {
    fn from(err: AccountMappingError) -> Self {
        multivm_common::error::MultivmError::AccountMapping(err.to_string())
    }
}

pub type AccountMappingResult<T> = Result<T, AccountMappingError>;
