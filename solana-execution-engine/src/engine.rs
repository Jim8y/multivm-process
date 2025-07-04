//! Solana Execution Engine Implementation
//!
//! This module provides the core execution engine for processing Solana transactions
//! within the MultiVM system. It manages the Solana runtime, transaction processing,
//! and state management.

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

// Common types from multivm-common
use multivm_common::types_rpc::RpcConfig;
use multivm_common::{
    BlockchainType, EngineState, ExecutionEngine, HealthStatus, MultivmError, ProcessId,
    ProcessingMetrics,
};

use solana_sdk::{hash::Hash, slot_history::Slot, transaction::Transaction};

use crate::real_engine::RealSolanaEngine;

/// Solana execution engine error types
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

/// Solana block data type for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaBlockData {
    /// The slot number (not related to Solana Validator)
    pub slot: Slot,
    /// Block hash (not related to Solana Validator)
    pub block_hash: Hash,
    /// Parent slot (not related to Solana Validator)
    pub parent_slot: Slot,
    /// Transactions in this block
    pub transactions: Vec<SolanaTransaction>,
    /// Block time (not related to Solana Validator)
    pub block_time: Option<i64>,
    /// Previous block hash (not related to Solana Validator)
    pub previous_blockhash: Hash,
}

/// Type alias for Solana transaction to maintain naming consistency
pub type SolanaTransaction = Transaction;

/// Solana execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaExecutionResult {
    /// The slot that was processed
    pub slot: Slot,
    /// Hash of the executed block
    pub block_hash: Hash,
    /// New state root after execution
    pub state_root: Hash,
    /// Number of transactions processed
    pub transaction_count: usize,
    /// Compute units used
    pub compute_units_used: u64,
    /// Processing time
    pub processing_time: Duration,
    /// Success flag
    pub success: bool,
    /// Error message if any
    pub error: Option<String>,
}

/// Configuration for the Solana engine
#[derive(Debug, Clone)]
pub struct SolanaConfig {
    /// Path for account storage
    pub data_dir: PathBuf,

    /// RPC bind address
    pub rpc_addr: String,

    /// RPC port
    pub rpc_port: u16,

    /// Maximum compute units per block
    #[allow(dead_code)]
    pub max_compute_units: u64,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data/solana"),
            rpc_addr: "127.0.0.1".to_string(),
            rpc_port: 8899,
            max_compute_units: 1_000_000,
        }
    }
}
