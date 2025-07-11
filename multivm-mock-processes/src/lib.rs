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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_process_config_default() {
        let config = MockProcessConfig::default();
        assert_eq!(config.name, "mock-process");
        assert_eq!(config.endpoint, "127.0.0.1:8545");
        assert!(config.use_tcp);
        assert_eq!(config.processing_delay_ms, 100);
        assert!((config.success_rate - 0.95).abs() < f64::EPSILON);
        assert!(!config.verbose);
        assert_eq!(config.max_concurrent_requests, 100);
    }

    #[test]
    fn test_mock_process_config_custom() {
        let config = MockProcessConfig {
            name: "custom-mock".to_string(),
            endpoint: "/tmp/custom.sock".to_string(),
            use_tcp: false,
            processing_delay_ms: 200,
            success_rate: 0.99,
            verbose: true,
            max_concurrent_requests: 50,
        };

        assert_eq!(config.name, "custom-mock");
        assert_eq!(config.endpoint, "/tmp/custom.sock");
        assert!(!config.use_tcp);
        assert_eq!(config.processing_delay_ms, 200);
        assert!((config.success_rate - 0.99).abs() < f64::EPSILON);
        assert!(config.verbose);
        assert_eq!(config.max_concurrent_requests, 50);
    }

    #[test]
    fn test_process_stats_creation() {
        let stats = ProcessStats {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            avg_processing_time_ms: 150,
            pending_requests: 3,
            uptime: Duration::from_secs(3600),
        };

        assert_eq!(stats.total_requests, 100);
        assert_eq!(stats.successful_requests, 95);
        assert_eq!(stats.failed_requests, 5);
        assert_eq!(stats.avg_processing_time_ms, 150);
        assert_eq!(stats.pending_requests, 3);
        assert_eq!(stats.uptime.as_secs(), 3600);
    }

    #[test]
    fn test_mock_process_error_display() {
        let io_error = MockProcessError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));
        assert!(io_error.to_string().contains("IO error"));

        let ser_error = MockProcessError::Serialization("Invalid JSON".to_string());
        assert_eq!(ser_error.to_string(), "Serialization error: Invalid JSON");

        let already_running = MockProcessError::AlreadyRunning;
        assert_eq!(already_running.to_string(), "Process already running");

        let not_running = MockProcessError::NotRunning;
        assert_eq!(not_running.to_string(), "Process not running");

        let invalid_config = MockProcessError::InvalidConfig("Missing endpoint".to_string());
        assert_eq!(
            invalid_config.to_string(),
            "Invalid configuration: Missing endpoint"
        );

        let processing_failed = MockProcessError::ProcessingFailed("Timeout".to_string());
        assert_eq!(
            processing_failed.to_string(),
            "Request processing failed: Timeout"
        );
    }

    #[test]
    fn test_config_serialization() {
        let config = MockProcessConfig::default();

        // Serialize to JSON
        let json = serde_json::to_string(&config).unwrap();

        // Deserialize back
        let deserialized: MockProcessConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.name, deserialized.name);
        assert_eq!(config.endpoint, deserialized.endpoint);
        assert_eq!(config.use_tcp, deserialized.use_tcp);
    }

    #[test]
    fn test_stats_serialization() {
        let stats = ProcessStats {
            total_requests: 50,
            successful_requests: 48,
            failed_requests: 2,
            avg_processing_time_ms: 75,
            pending_requests: 1,
            uptime: Duration::from_secs(1800),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&stats).unwrap();

        // Deserialize back
        let deserialized: ProcessStats = serde_json::from_str(&json).unwrap();

        assert_eq!(stats.total_requests, deserialized.total_requests);
        assert_eq!(stats.successful_requests, deserialized.successful_requests);
        assert_eq!(stats.failed_requests, deserialized.failed_requests);
    }
}
