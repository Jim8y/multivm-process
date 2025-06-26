//! Local definitions to avoid multivm-common dependency conflicts
//! This file contains minimal trait and type definitions needed for the Solana execution engine

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// MultiVM error types (minimal version)
#[derive(Debug, Error)]
pub enum MultivmError {
    #[error("Configuration error: {component} - {message}")]
    Configuration { component: String, message: String },
    
    #[error("Process error: {process_id} - {message}")]
    Process { process_id: String, message: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type MultivmResult<T> = Result<T, MultivmError>;

/// Blockchain types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockchainType {
    Solana,
    Ethereum,
    MultiVM,
}

/// Engine state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

/// Health status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Processing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetrics {
    pub blocks_processed: u64,
    pub transactions_processed: u64,
    pub average_block_time: Duration,
    pub last_block_timestamp: Option<std::time::SystemTime>,
    pub error_count: u64,
}

impl Default for ProcessingMetrics {
    fn default() -> Self {
        Self {
            blocks_processed: 0,
            transactions_processed: 0,
            average_block_time: Duration::from_secs(1),
            last_block_timestamp: None,
            error_count: 0,
        }
    }
}

/// RPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub endpoint: String,
    pub timeout: Duration,
    pub max_retries: u32,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8899".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

/// Main execution engine trait
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Start the execution engine
    async fn start(&mut self) -> Result<(), Self::Error>;

    /// Stop the execution engine
    async fn stop(&mut self) -> Result<(), Self::Error>;

    /// Get current engine state
    fn get_state(&self) -> EngineState;

    /// Check if engine is ready
    async fn is_ready(&self) -> bool;

    /// Process a transaction
    async fn process_transaction(&mut self, transaction: Vec<u8>) -> Result<Vec<u8>, Self::Error>;

    /// Process a block
    async fn process_block(&mut self, block: Vec<u8>) -> Result<Vec<u8>, Self::Error>;

    /// Get current block height
    async fn get_block_height(&self) -> Result<u64, Self::Error>;

    /// Get health status
    async fn get_health(&self) -> Result<HealthStatus, Self::Error>;

    /// Shutdown with timeout
    async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), Self::Error>;

    /// Get processing metrics
    async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error>;

    /// Get latest block ID
    async fn get_latest_block_id(&self) -> Result<u64, Self::Error>;

    /// Reset to specific block
    async fn reset_to_block(&mut self, block_id: u64) -> Result<(), Self::Error>;
}