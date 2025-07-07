use multivm_common::MultivmError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SolanaEngineError {
    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("RPC communication error: {0}")]
    Rpc(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Block processing error: {0}")]
    #[allow(dead_code)]
    BlockProcessing(String),

    #[error("Invalid block data: {0}")]
    #[allow(dead_code)]
    InvalidBlock(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Transaction error: {0}")]
    Transaction(String),
}

impl From<MultivmError> for SolanaEngineError {
    fn from(err: MultivmError) -> Self {
        match err {
            MultivmError::Configuration { message, .. } => {
                SolanaEngineError::Configuration(message)
            }
            MultivmError::Process { message, .. } => SolanaEngineError::Process(message),
            MultivmError::Rpc { message, .. } => SolanaEngineError::Rpc(message),
            MultivmError::Serialization { message, .. } => {
                SolanaEngineError::Serialization(message)
            }
            _ => SolanaEngineError::Runtime(err.to_string()),
        }
    }
}

// Add conversion from solana_client::client_error::ClientError to SolanaEngineError
impl From<solana_client::client_error::ClientError> for SolanaEngineError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        SolanaEngineError::Rpc(format!("Solana client error: {}", err))
    }
}

// Add conversion from tokio::time::error::Elapsed to SolanaEngineError
impl From<tokio::time::error::Elapsed> for SolanaEngineError {
    fn from(err: tokio::time::error::Elapsed) -> Self {
        SolanaEngineError::Process(format!("Operation timed out: {}", err))
    }
}

impl From<SolanaEngineError> for MultivmError {
    fn from(err: SolanaEngineError) -> Self {
        match err {
            SolanaEngineError::Configuration(msg) => MultivmError::Configuration {
                component: "solana-engine".to_string(),
                message: msg,
                validation_errors: None,
            },
            SolanaEngineError::Process(msg) => MultivmError::Process {
                process_id: "solana-engine".to_string(),
                message: msg,
                exit_code: None,
            },
            SolanaEngineError::Rpc(msg) => MultivmError::Rpc {
                method: "solana-rpc".to_string(),
                message: msg,
                status_code: None,
            },
            SolanaEngineError::Serialization(msg) => MultivmError::Serialization {
                message: msg,
                data_type: Some("solana-data".to_string()),
            },
            SolanaEngineError::Transaction(msg) => MultivmError::Process {
                process_id: "solana-transaction".to_string(),
                message: msg,
                exit_code: None,
            },
            SolanaEngineError::Runtime(msg) => MultivmError::Process {
                process_id: "solana-runtime".to_string(),
                message: msg,
                exit_code: None,
            },
            SolanaEngineError::Io(e) => MultivmError::Process {
                process_id: "solana-io".to_string(),
                message: e.to_string(),
                exit_code: None,
            },
            SolanaEngineError::BlockProcessing(msg) => MultivmError::Process {
                process_id: "solana-block-processing".to_string(),
                message: msg,
                exit_code: None,
            },
            SolanaEngineError::InvalidBlock(msg) => MultivmError::Process {
                process_id: "solana-block-validation".to_string(),
                message: msg,
                exit_code: None,
            },
        }
    }
}
