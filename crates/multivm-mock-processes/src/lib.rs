//! Mock Processes for MultiVM Testing
//!
//! This crate provides mock implementations of Reth (Ethereum) and Solana processes
//! for testing the MultiVM architecture. These mocks simulate the behavior of real
//! blockchain execution engines through IPC communication.
//!
//! ## Architecture
//!
//! MultiVM acts as a coordinator that communicates with external processes:
//! - Mock Reth Process: Simulates Ethereum execution
//! - Mock Solana Process: Simulates Solana execution
//!
//! Communication happens through JSON-RPC over IPC (Unix sockets or TCP).

pub mod ipc_server;
pub mod state_manager;
pub mod transaction_processor;

pub use ipc_server::{IpcMessage, IpcResponse, IpcServer};

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for mock processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockProcessConfig {
    /// Process name
    pub name: String,
    /// IPC endpoint (e.g., "127.0.0.1:8545" or "/tmp/reth.sock")
    pub endpoint: String,
    /// Use TCP instead of Unix socket
    pub use_tcp: bool,
    /// Simulated processing delay
    pub processing_delay_ms: u64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Enable detailed logging
    pub verbose: bool,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
}

impl Default for MockProcessConfig {
    fn default() -> Self {
        Self {
            name: "mock-process".to_string(),
            endpoint: "127.0.0.1:8545".to_string(),
            use_tcp: true,
            processing_delay_ms: 100,
            success_rate: 0.95,
            verbose: false,
            max_concurrent_requests: 100,
        }
    }
}

/// Common trait for mock blockchain processes
#[async_trait::async_trait]
pub trait MockProcess: Send + Sync {
    /// Start the mock process
    async fn start(&mut self) -> Result<(), MockProcessError>;

    /// Stop the mock process
    async fn stop(&mut self) -> Result<(), MockProcessError>;

    /// Check if the process is running
    fn is_running(&self) -> bool;

    /// Get process statistics
    fn get_stats(&self) -> ProcessStats;
}

/// Statistics for mock processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    /// Total requests received
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: u64,
    /// Current pending requests
    pub pending_requests: usize,
    /// Process uptime
    pub uptime: Duration,
}

/// Errors that can occur in mock processes
#[derive(Debug, thiserror::Error)]
pub enum MockProcessError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Process already running")]
    AlreadyRunning,

    #[error("Process not running")]
    NotRunning,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Request processing failed: {0}")]
    ProcessingFailed(String),
}

/// Result type for mock process operations
pub type MockProcessResult<T> = Result<T, MockProcessError>;
