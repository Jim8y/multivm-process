//! Solana execution engine configuration
//! 
//! This module provides configuration structures for the real Solana validator integration,
//! including RPC settings, connection parameters, and performance tuning options.

use crate::MultivmError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Solana execution engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    /// Whether the Solana engine is enabled
    pub enabled: bool,
    
    /// Data directory for Solana validator
    pub data_dir: PathBuf,
    
    /// RPC port for Solana JSON-RPC
    pub rpc_port: u16,
    
    /// WebSocket port for Solana subscriptions
    pub ws_port: u16,
    
    /// Cluster type (mainnet-beta, testnet, devnet, localnet)
    pub cluster: SolanaCluster,
    
    /// Connection configuration
    pub connection: SolanaConnectionConfig,
    
    /// Performance configuration
    pub performance: SolanaPerformanceConfig,
    
    /// Security configuration
    pub security: SolanaSecurityConfig,
    
    /// Whether to use mock mode instead of real Solana validator
    pub mock_mode: bool,
}

/// Solana cluster types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolanaCluster {
    #[serde(rename = "mainnet-beta")]
    MainnetBeta,
    #[serde(rename = "testnet")]
    Testnet,
    #[serde(rename = "devnet")]
    Devnet,
    #[serde(rename = "localnet")]
    Localnet,
}

/// Connection configuration for Solana integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConnectionConfig {
    /// Maximum number of retry attempts for failed requests
    pub max_retries: u32,
    
    /// Delay between retry attempts
    #[serde(with = "duration_serde")]
    pub retry_delay: Duration,
    
    /// Request timeout for RPC calls
    #[serde(with = "duration_serde")]
    pub request_timeout: Duration,
    
    /// Health check interval for monitoring
    #[serde(with = "duration_serde")]
    pub health_check_interval: Duration,
    
    /// Connection pool size for HTTP clients
    pub connection_pool_size: u32,
    
    /// Keep-alive timeout for TCP connections
    #[serde(with = "duration_serde")]
    pub tcp_keepalive: Duration,
    
    /// Commitment level for transactions (processed, confirmed, finalized)
    pub commitment_level: CommitmentLevel,
    
    /// Enable WebSocket subscriptions
    pub enable_websockets: bool,
}

/// Solana commitment levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitmentLevel {
    #[serde(rename = "processed")]
    Processed,
    #[serde(rename = "confirmed")]
    Confirmed,
    #[serde(rename = "finalized")]
    Finalized,
}

/// Performance configuration for Solana integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaPerformanceConfig {
    /// Maximum compute units per transaction
    pub max_compute_units_per_tx: u64,
    
    /// Slot time in milliseconds (Solana's block time)
    pub slot_time_ms: u64,
    
    /// Transaction cache size
    pub transaction_cache_size: u32,
    
    /// Account cache size
    pub account_cache_size: u32,
    
    /// Enable parallel transaction processing
    pub enable_parallel_execution: bool,
    
    /// Transaction pool size
    pub transaction_pool_size: u32,
    
    /// Maximum transactions per slot
    pub max_transactions_per_slot: u32,
    
    /// Enable transaction batching
    pub enable_transaction_batching: bool,
    
    /// Batch size for transaction processing
    pub transaction_batch_size: u32,
}

/// Security configuration for Solana integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaSecurityConfig {
    /// Enable CORS for RPC endpoints
    pub enable_cors: bool,
    
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
    
    /// Enable authentication for RPC endpoints
    pub enable_rpc_auth: bool,
    
    /// Rate limiting configuration
    pub rate_limit_requests_per_second: u32,
    
    /// Enable transaction signature verification
    pub verify_signatures: bool,
    
    /// Enable account rent enforcement
    pub enforce_rent: bool,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            data_dir: PathBuf::from("./data/solana"),
            rpc_port: 8899,
            ws_port: 8900,
            cluster: SolanaCluster::Localnet,
            connection: SolanaConnectionConfig::default(),
            performance: SolanaPerformanceConfig::default(),
            security: SolanaSecurityConfig::default(),
            mock_mode: false,
        }
    }
}

impl Default for SolanaConnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            request_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(10),
            connection_pool_size: 10,
            tcp_keepalive: Duration::from_secs(60),
            commitment_level: CommitmentLevel::Confirmed,
            enable_websockets: true,
        }
    }
}

impl Default for SolanaPerformanceConfig {
    fn default() -> Self {
        Self {
            max_compute_units_per_tx: 1_400_000, // Solana's current limit
            slot_time_ms: 400, // Solana's target slot time
            transaction_cache_size: 10000,
            account_cache_size: 50000,
            enable_parallel_execution: true,
            transaction_pool_size: 100000,
            max_transactions_per_slot: 20000,
            enable_transaction_batching: true,
            transaction_batch_size: 100,
        }
    }
}

impl Default for SolanaSecurityConfig {
    fn default() -> Self {
        Self {
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            enable_rpc_auth: false,
            rate_limit_requests_per_second: 100,
            verify_signatures: true,
            enforce_rent: true,
        }
    }
}

impl SolanaConfig {
    /// Validate the Solana configuration
    pub fn validate(&self) -> Result<(), MultivmError> {
        if !self.enabled {
            return Ok(());
        }

        // Validate ports
        if self.rpc_port == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "RPC port must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.ws_port == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "WebSocket port must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.rpc_port == self.ws_port {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "RPC port and WebSocket port must be different".to_string(),
                validation_errors: None,
            });
        }

        // Validate connection config
        self.connection.validate()?;

        // Validate performance config
        self.performance.validate()?;

        // Validate security config
        self.security.validate()?;

        Ok(())
    }

    /// Get the cluster name for Solana configuration
    pub fn get_cluster_name(&self) -> &str {
        match self.cluster {
            SolanaCluster::MainnetBeta => "mainnet-beta",
            SolanaCluster::Testnet => "testnet",
            SolanaCluster::Devnet => "devnet",
            SolanaCluster::Localnet => "localnet",
        }
    }

    /// Get the default RPC URL for the cluster
    pub fn get_default_rpc_url(&self) -> String {
        match self.cluster {
            SolanaCluster::MainnetBeta => "https://api.mainnet-beta.solana.com".to_string(),
            SolanaCluster::Testnet => "https://api.testnet.solana.com".to_string(),
            SolanaCluster::Devnet => "https://api.devnet.solana.com".to_string(),
            SolanaCluster::Localnet => format!("http://127.0.0.1:{}", self.rpc_port),
        }
    }

    /// Get the default WebSocket URL for the cluster
    pub fn get_default_ws_url(&self) -> String {
        match self.cluster {
            SolanaCluster::MainnetBeta => "wss://api.mainnet-beta.solana.com".to_string(),
            SolanaCluster::Testnet => "wss://api.testnet.solana.com".to_string(),
            SolanaCluster::Devnet => "wss://api.devnet.solana.com".to_string(),
            SolanaCluster::Localnet => format!("ws://127.0.0.1:{}", self.ws_port),
        }
    }

    /// Create a configuration for mainnet-beta
    pub fn mainnet_beta() -> Self {
        Self {
            cluster: SolanaCluster::MainnetBeta,
            data_dir: PathBuf::from("./data/solana-mainnet"),
            ..Default::default()
        }
    }

    /// Create a configuration for testnet
    pub fn testnet() -> Self {
        Self {
            cluster: SolanaCluster::Testnet,
            data_dir: PathBuf::from("./data/solana-testnet"),
            ..Default::default()
        }
    }

    /// Create a configuration for devnet
    pub fn devnet() -> Self {
        Self {
            cluster: SolanaCluster::Devnet,
            data_dir: PathBuf::from("./data/solana-devnet"),
            ..Default::default()
        }
    }

    /// Create a configuration for development (localnet)
    pub fn development() -> Self {
        Self {
            cluster: SolanaCluster::Localnet,
            data_dir: PathBuf::from("./data/solana-dev"),
            mock_mode: false, // Use real Solana implementation for production readiness
            ..Default::default()
        }
    }
}

impl SolanaConnectionConfig {
    /// Validate the connection configuration
    pub fn validate(&self) -> Result<(), MultivmError> {
        if self.max_retries == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "max_retries must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.retry_delay.as_millis() == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "retry_delay must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.request_timeout.as_secs() == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "request_timeout must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.health_check_interval.as_secs() == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "health_check_interval must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.connection_pool_size == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "connection_pool_size must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        Ok(())
    }
}

impl SolanaPerformanceConfig {
    /// Validate the performance configuration
    pub fn validate(&self) -> Result<(), MultivmError> {
        if self.max_compute_units_per_tx == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "max_compute_units_per_tx must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.max_compute_units_per_tx > 1_400_000 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "max_compute_units_per_tx exceeds Solana's limit of 1,400,000".to_string(),
                validation_errors: None,
            });
        }

        if self.slot_time_ms == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "slot_time_ms must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.transaction_cache_size == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "transaction_cache_size must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.account_cache_size == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "account_cache_size must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.transaction_pool_size == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "transaction_pool_size must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.max_transactions_per_slot == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "max_transactions_per_slot must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.transaction_batch_size == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "transaction_batch_size must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        Ok(())
    }
}

impl SolanaSecurityConfig {
    /// Validate the security configuration
    pub fn validate(&self) -> Result<(), MultivmError> {
        if self.rate_limit_requests_per_second == 0 {
            return Err(MultivmError::Configuration {
                component: "solana".to_string(),
                message: "rate_limit_requests_per_second must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        Ok(())
    }
}

impl CommitmentLevel {
    /// Convert to Solana RPC commitment string
    pub fn as_str(&self) -> &'static str {
        match self {
            CommitmentLevel::Processed => "processed",
            CommitmentLevel::Confirmed => "confirmed",
            CommitmentLevel::Finalized => "finalized",
        }
    }
}

impl SolanaCluster {
    /// Get the genesis hash for the cluster
    pub fn genesis_hash(&self) -> &'static str {
        match self {
            SolanaCluster::MainnetBeta => "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
            SolanaCluster::Testnet => "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY",
            SolanaCluster::Devnet => "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
            SolanaCluster::Localnet => "11111111111111111111111111111111", // System program ID for localnet
        }
    }
}

/// Serde module for Duration serialization/deserialization
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

