//! Configuration structures for Solana execution engine
//!
//! This module contains all configuration-related structures and their default implementations
//! for the Solana execution engine integration with MultiVM.

use solana_sdk::commitment_config::CommitmentLevel;
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for Solana execution engine
#[derive(Debug, Clone)]
pub struct SolanaEngineConfig {
    /// RPC server host address
    pub rpc_server_host: String,
    /// RPC server port number
    pub rpc_server_port: u16,
}

impl Default for SolanaEngineConfig {
    fn default() -> Self {
        Self {
            rpc_server_host: "127.0.0.1".to_string(),
            rpc_server_port: 8888,
        }
    }
}

impl SolanaEngineConfig {
    /// Create a new SolanaEngineConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new SolanaEngineConfig with custom values
    pub fn new_with_config(rpc_server_host: String, rpc_server_port: u16) -> Self {
        Self {
            rpc_server_host,
            rpc_server_port,
        }
    }
}

/// Configuration for Solana execution engine
#[derive(Debug, Clone)]
pub struct SolanaConfig {
    /// Gossip port number
    pub gossip_port: u16,
    /// RPC port number
    pub rpc_port: u16,
    /// WebSocket port number
    pub ws_port: u16,
    /// Path to the ledger directory
    pub ledger_path: PathBuf,
    /// Number of ticks per slot
    pub ticks_per_slot: u32,
    /// Enable deterministic mode
    pub deterministic: bool,
    /// Reset the validator state on startup
    pub reset: bool,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        // Generate a random directory name for ledger_path
        let random_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let ledger_path = PathBuf::from(format!("/tmp/solana-private-ledger_{}", random_suffix));

        Self {
            gossip_port: 1024,
            rpc_port: 8899,
            ws_port: 8900,
            ledger_path,
            ticks_per_slot: 2,
            deterministic: true,
            reset: true,
        }
    }
}

impl SolanaConfig {
    /// Create a new SolanaConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for SolanaConfig
    pub fn builder() -> SolanaConfigBuilder {
        SolanaConfigBuilder::new()
    }
}

/// Builder for SolanaConfig
#[derive(Debug)]
pub struct SolanaConfigBuilder {
    config: SolanaConfig,
}

impl SolanaConfigBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            config: SolanaConfig::new(),
        }
    }

    /// Set the gossip port
    pub fn gossip_port(mut self, port: u16) -> Self {
        self.config.gossip_port = port;
        self
    }

    /// Set the RPC port
    pub fn rpc_port(mut self, port: u16) -> Self {
        self.config.rpc_port = port;
        self
    }

    /// Set the WebSocket port
    pub fn ws_port(mut self, port: u16) -> Self {
        self.config.ws_port = port;
        self
    }

    /// Set the ledger path
    pub fn ledger_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.ledger_path = path.into();
        self
    }

    /// Set the ticks per slot
    pub fn ticks_per_slot(mut self, ticks: u32) -> Self {
        self.config.ticks_per_slot = ticks;
        self
    }

    /// Set deterministic mode
    pub fn deterministic(mut self, deterministic: bool) -> Self {
        self.config.deterministic = deterministic;
        self
    }

    /// Set reset mode
    pub fn reset(mut self, reset: bool) -> Self {
        self.config.reset = reset;
        self
    }

    /// Build the SolanaConfig
    pub fn build(self) -> SolanaConfig {
        self.config
    }
}

/// Configuration for Solana validator connections
#[derive(Debug, Clone)]
pub struct SolanaConnectionConfig {
    /// Maximum number of retry attempts for failed operations
    pub max_retries: u32,
    /// Delay between retry attempts
    pub retry_delay: Duration,
    /// Timeout for individual requests
    pub request_timeout: Duration,
    /// Interval for health check operations
    pub health_check_interval: Duration,
    /// Size of the connection pool
    pub connection_pool_size: u32,
    /// Commitment level for transactions
    pub commitment_level: CommitmentLevel,
}

impl Default for SolanaConnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            request_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(10),
            connection_pool_size: 10,
            commitment_level: CommitmentLevel::Confirmed,
        }
    }
}

impl SolanaConnectionConfig {
    /// Create a new SolanaConnectionConfig with custom parameters
    pub fn new(
        max_retries: u32,
        retry_delay: Duration,
        request_timeout: Duration,
        health_check_interval: Duration,
        connection_pool_size: u32,
        commitment_level: CommitmentLevel,
    ) -> Self {
        Self {
            max_retries,
            retry_delay,
            request_timeout,
            health_check_interval,
            connection_pool_size,
            commitment_level,
        }
    }

    /// Create a builder for SolanaConnectionConfig
    pub fn builder() -> SolanaConnectionConfigBuilder {
        SolanaConnectionConfigBuilder::default()
    }
}

/// Builder for SolanaConnectionConfig
#[derive(Debug, Default)]
pub struct SolanaConnectionConfigBuilder {
    max_retries: Option<u32>,
    retry_delay: Option<Duration>,
    request_timeout: Option<Duration>,
    health_check_interval: Option<Duration>,
    connection_pool_size: Option<u32>,
    commitment_level: Option<CommitmentLevel>,
}

impl SolanaConnectionConfigBuilder {
    /// Set the maximum number of retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Set the retry delay
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = Some(delay);
        self
    }

    /// Set the request timeout
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    /// Set the health check interval
    pub fn health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = Some(interval);
        self
    }

    /// Set the connection pool size
    pub fn connection_pool_size(mut self, size: u32) -> Self {
        self.connection_pool_size = Some(size);
        self
    }

    /// Set the commitment level
    pub fn commitment_level(mut self, level: CommitmentLevel) -> Self {
        self.commitment_level = Some(level);
        self
    }

    /// Build the SolanaConnectionConfig
    pub fn build(self) -> SolanaConnectionConfig {
        let default = SolanaConnectionConfig::default();
        SolanaConnectionConfig {
            max_retries: self.max_retries.unwrap_or(default.max_retries),
            retry_delay: self.retry_delay.unwrap_or(default.retry_delay),
            request_timeout: self.request_timeout.unwrap_or(default.request_timeout),
            health_check_interval: self
                .health_check_interval
                .unwrap_or(default.health_check_interval),
            connection_pool_size: self
                .connection_pool_size
                .unwrap_or(default.connection_pool_size),
            commitment_level: self.commitment_level.unwrap_or(default.commitment_level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_config_new() {
        let config = SolanaConfig::new();
        assert_eq!(config.gossip_port, 1024);
        assert_eq!(config.rpc_port, 8899);
        assert_eq!(config.ws_port, 8900);
        assert!(config.ledger_path.starts_with("/tmp"));
        assert_eq!(config.ticks_per_slot, 2);
        assert!(config.deterministic);
        assert!(config.reset);
    }

    #[test]
    fn test_solana_config_builder() {
        let custom_ledger_path = PathBuf::from("/tmp/custom_ledger");

        let config = SolanaConfig::builder()
            .gossip_port(2048)
            .rpc_port(9000)
            .ledger_path(&custom_ledger_path)
            .ticks_per_slot(4)
            .deterministic(false)
            .reset(false)
            .build();

        assert_eq!(config.gossip_port, 2048);
        assert_eq!(config.rpc_port, 9000);
        assert_eq!(config.ledger_path, custom_ledger_path);
        assert_eq!(config.ticks_per_slot, 4);
        assert!(!config.deterministic);
        assert!(!config.reset);
    }

    #[test]
    fn test_solana_connection_config_default() {
        let config = SolanaConnectionConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, Duration::from_millis(1000));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.connection_pool_size, 10);
    }

    #[test]
    fn test_solana_connection_config_builder() {
        let config = SolanaConnectionConfig::builder()
            .max_retries(5)
            .retry_delay(Duration::from_millis(500))
            .request_timeout(Duration::from_secs(60))
            .connection_pool_size(20)
            .build();

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.retry_delay, Duration::from_millis(500));
        assert_eq!(config.request_timeout, Duration::from_secs(60));
        assert_eq!(config.connection_pool_size, 20);
    }
}
