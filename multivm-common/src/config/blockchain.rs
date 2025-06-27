//! Blockchain Configuration Module
//!
//! This module provides configuration structures for different blockchain engines
//! supported by MultiVM, including Ethereum (via Reth) and Solana configurations.
//! Each blockchain has its own specific settings for RPC endpoints, data directories,
//! and runtime parameters.

use crate::{MultivmError, RpcConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Solana engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub enabled: bool,
    pub data_dir: PathBuf,
    pub rpc_config: Option<RpcConfig>,
    pub ledger_path: PathBuf,
    pub accounts_path: PathBuf,
    pub chain_id: u64,
}

impl SolanaConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(ref rpc_config) = self.rpc_config {
            rpc_config.validate()?;
        }

        Ok(())
    }
}

impl Default for SolanaConfig {
    fn default() -> Self {
        let data_dir = PathBuf::from("./data/solana");
        Self {
            enabled: true,
            data_dir: data_dir.clone(),
            rpc_config: Some(RpcConfig {
                host: "127.0.0.1".to_string(),
                port: 8899,
                max_connections: 1000,
                request_timeout: Duration::from_secs(30),
                cors_origins: vec!["*".to_string()],
            }),
            ledger_path: data_dir.join("ledger"),
            accounts_path: data_dir.join("accounts"),
            chain_id: 1,
        }
    }
}

/// Ethereum engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumConfig {
    pub enabled: bool,
    pub data_dir: PathBuf,
    pub rpc_config: Option<RpcConfig>,
    pub chain_id: u64,
}

impl EthereumConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(ref rpc_config) = self.rpc_config {
            rpc_config.validate()?;
        }

        Ok(())
    }
}

impl Default for EthereumConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            data_dir: PathBuf::from("./data/ethereum"),
            rpc_config: Some(RpcConfig {
                host: "127.0.0.1".to_string(),
                port: 8545,
                max_connections: 1000,
                request_timeout: Duration::from_secs(30),
                cors_origins: vec!["*".to_string()],
            }),
            chain_id: 1,
        }
    }
}
