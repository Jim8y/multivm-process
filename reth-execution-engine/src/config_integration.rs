//! Configuration integration for MultiVM Reth execution engine
//!
//! This module provides configuration loading and integration with the MultiVM
//! setup scripts and configuration files.

use crate::engine::RethEngineError;
use crate::real_engine::{ConnectionConfig, RealRethEngine};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

/// Configuration structure that matches the setup script parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RethMultiVMConfig {
    /// Node configuration
    pub node: NodeConfig,
    /// Reth-specific configuration
    pub reth: RethConfig,
    /// RPC configuration
    pub rpc: RpcConfig,
    /// Engine API configuration
    pub engine_api: EngineApiConfig,
    /// MultiVM integration configuration
    pub multivm: MultiVMConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub name: String,
    pub description: String,
    pub chain_id: u64,
    pub network_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RethConfig {
    pub binary_path: String,
    pub data_dir: String,
    pub log_level: String,
    pub chain: String,
    pub full_node: bool,
    pub dev_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub api_modules: Vec<String>,
    pub max_connections: u32,
    pub max_request_size_bytes: u64,
    pub max_response_size_bytes: u64,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineApiConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub jwt_secret_path: String,
    pub jwt_expiry_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVMConfig {
    pub coordinator_host: String,
    pub coordinator_port: u16,
    pub ipc_enabled: bool,
    pub ipc_path: String,
    pub ipc_timeout_seconds: u64,
    pub max_cross_vm_transactions_per_block: u32,
    pub cross_vm_transaction_timeout_seconds: u64,
    pub authentication: AuthenticationConfig,
    pub coordination: CoordinationConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    pub jwt_shared_secret_path: String,
    pub token_expiry_seconds: u64,
    pub require_authentication: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationConfig {
    pub block_time_ms: u64,
    pub max_transactions_per_block: u32,
    pub transaction_pool_size: u32,
    pub health_check_interval_seconds: u64,
    pub health_check_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub metrics_path: String,
    pub track_transaction_processing_time: bool,
    pub track_block_processing_time: bool,
    pub track_cross_vm_operations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_admin_api: bool,
    pub allow_unsafe_methods: bool,
    pub enable_personal_api: bool,
    pub rate_limiting: RateLimitingConfig,
    pub allowed_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub output_file: String,
    pub max_file_size_mb: u32,
    pub max_files: u32,
    pub log_rotation: bool,
}

impl Default for RethMultiVMConfig {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                name: "multivm-reth-node".to_string(),
                description: "Reth execution engine for MultiVM blockchain".to_string(),
                chain_id: 1337,
                network_id: 1337,
            },
            reth: RethConfig {
                binary_path: "reth".to_string(),
                data_dir: "./reth-data".to_string(),
                log_level: "info".to_string(),
                chain: "dev".to_string(),
                full_node: true,
                dev_mode: true,
            },
            rpc: RpcConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8545,
                cors_origins: vec!["*".to_string()],
                api_modules: vec![
                    "eth".to_string(),
                    "net".to_string(),
                    "web3".to_string(),
                    "debug".to_string(),
                    "trace".to_string(),
                ],
                max_connections: 100,
                max_request_size_bytes: 15728640,  // 15MB
                max_response_size_bytes: 15728640, // 15MB
                request_timeout_seconds: 30,
            },
            engine_api: EngineApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8551,
                jwt_secret_path: "./reth-data/jwt.hex".to_string(),
                jwt_expiry_seconds: 300,
            },
            multivm: MultiVMConfig {
                coordinator_host: "127.0.0.1".to_string(),
                coordinator_port: 8080,
                ipc_enabled: true,
                ipc_path: "/tmp/multivm-reth.sock".to_string(),
                ipc_timeout_seconds: 30,
                max_cross_vm_transactions_per_block: 100,
                cross_vm_transaction_timeout_seconds: 60,
                authentication: AuthenticationConfig {
                    jwt_shared_secret_path: "./reth-data/jwt.hex".to_string(),
                    token_expiry_seconds: 300,
                    require_authentication: true,
                },
                coordination: CoordinationConfig {
                    block_time_ms: 3000, // 3 second blocks
                    max_transactions_per_block: 1000,
                    transaction_pool_size: 10000,
                    health_check_interval_seconds: 10,
                    health_check_timeout_seconds: 5,
                },
                metrics: MetricsConfig {
                    enabled: true,
                    host: "127.0.0.1".to_string(),
                    port: 9090,
                    metrics_path: "/metrics".to_string(),
                    track_transaction_processing_time: true,
                    track_block_processing_time: true,
                    track_cross_vm_operations: true,
                },
            },
            security: SecurityConfig {
                enable_admin_api: false,
                allow_unsafe_methods: false,
                enable_personal_api: false,
                rate_limiting: RateLimitingConfig {
                    enabled: true,
                    requests_per_minute: 1000,
                    burst_size: 100,
                },
                allowed_ips: vec![],
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                output_file: "./reth-data/logs/reth.log".to_string(),
                max_file_size_mb: 100,
                max_files: 10,
                log_rotation: true,
            },
        }
    }
}

impl RethMultiVMConfig {
    /// Load configuration from a TOML file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, RethEngineError> {
        info!("Loading MultiVM Reth configuration from: {:?}", path);

        let content = std::fs::read_to_string(path).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to read config file: {e}"))
        })?;

        let config: RethMultiVMConfig = toml::from_str(&content)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to parse config: {e}")))?;

        info!("Configuration loaded successfully");
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables (for setup script integration)
    pub fn load_from_env() -> Result<Self, RethEngineError> {
        info!("Loading MultiVM Reth configuration from environment variables");

        let mut config = Self::default();

        // Override with environment variables if present
        if let Ok(data_dir) = std::env::var("RETH_DATA_DIR") {
            config.reth.data_dir = data_dir.clone();
            config.engine_api.jwt_secret_path = format!("{}/jwt.hex", data_dir);
            config.multivm.authentication.jwt_shared_secret_path = format!("{}/jwt.hex", data_dir);
            config.logging.output_file = format!("{}/logs/reth.log", data_dir);
        }

        if let Ok(rpc_port) = std::env::var("RETH_HTTP_PORT") {
            config.rpc.port = rpc_port
                .parse()
                .map_err(|e| RethEngineError::Configuration(format!("Invalid RPC port: {e}")))?;
        }

        if let Ok(engine_port) = std::env::var("RETH_ENGINE_PORT") {
            config.engine_api.port = engine_port
                .parse()
                .map_err(|e| RethEngineError::Configuration(format!("Invalid Engine port: {e}")))?;
        }

        if let Ok(chain_id) = std::env::var("RETH_CHAIN_ID") {
            let chain_id_num: u64 = chain_id
                .parse()
                .map_err(|e| RethEngineError::Configuration(format!("Invalid chain ID: {e}")))?;
            config.node.chain_id = chain_id_num;
            config.node.network_id = chain_id_num;
        }

        if let Ok(jwt_path) = std::env::var("JWT_SECRET_PATH") {
            config.engine_api.jwt_secret_path = jwt_path.clone();
            config.multivm.authentication.jwt_shared_secret_path = jwt_path;
        }

        if let Ok(ipc_path) = std::env::var("MULTIVM_IPC_PATH") {
            config.multivm.ipc_path = ipc_path;
        }

        if let Ok(jwt_expiry) = std::env::var("JWT_EXPIRY_SECONDS") {
            let expiry: u64 = jwt_expiry
                .parse()
                .map_err(|e| RethEngineError::Configuration(format!("Invalid JWT expiry: {e}")))?;
            config.engine_api.jwt_expiry_seconds = expiry;
            config.multivm.authentication.token_expiry_seconds = expiry;
        }

        // Set chain name based on chain ID
        config.reth.chain = match config.node.chain_id {
            1 => "mainnet",
            11155111 => "sepolia",
            17000 => "holesky",
            _ => "dev",
        }
        .to_string();

        info!("Configuration loaded from environment variables");
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), RethEngineError> {
        info!("Saving configuration to: {:?}", path);

        let content = toml::to_string_pretty(self).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to serialize config: {e}"))
        })?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RethEngineError::Configuration(format!("Failed to create config directory: {e}"))
            })?;
        }

        std::fs::write(path, content).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to write config file: {e}"))
        })?;

        info!("Configuration saved successfully");
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), RethEngineError> {
        // Validate ports
        if self.rpc.port == self.engine_api.port {
            return Err(RethEngineError::Configuration(
                "RPC and Engine API ports must be different".to_string(),
            ));
        }

        // Validate paths
        let data_dir = PathBuf::from(&self.reth.data_dir);
        let jwt_path = PathBuf::from(&self.engine_api.jwt_secret_path);

        // JWT path should be under data directory (security best practice)
        if jwt_path.parent() != Some(&data_dir) {
            warn!("JWT secret path is not under data directory - this may be a security risk");
        }

        // Validate timeouts
        if self.multivm.coordination.health_check_timeout_seconds
            > self.multivm.coordination.health_check_interval_seconds
        {
            return Err(RethEngineError::Configuration(
                "Health check timeout cannot be greater than interval".to_string(),
            ));
        }

        // Validate transaction limits
        if self.multivm.max_cross_vm_transactions_per_block
            > self.multivm.coordination.max_transactions_per_block
        {
            return Err(RethEngineError::Configuration(
                "Cross-VM transaction limit cannot exceed total transaction limit".to_string(),
            ));
        }

        info!("Configuration validation passed");
        Ok(())
    }

    /// Convert to RealRethEngine ConnectionConfig
    pub fn to_connection_config(&self) -> ConnectionConfig {
        ConnectionConfig {
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            request_timeout: Duration::from_secs(self.rpc.request_timeout_seconds),
            health_check_interval: Duration::from_secs(
                self.multivm.coordination.health_check_interval_seconds,
            ),
            connection_pool_size: self.rpc.max_connections,
        }
    }

    /// Get the data directory as PathBuf
    pub fn data_dir_path(&self) -> PathBuf {
        PathBuf::from(&self.reth.data_dir)
    }

    /// Get the JWT secret file path
    pub fn jwt_secret_path(&self) -> PathBuf {
        PathBuf::from(&self.engine_api.jwt_secret_path)
    }

    /// Get the IPC socket path
    pub fn ipc_socket_path(&self) -> PathBuf {
        PathBuf::from(&self.multivm.ipc_path)
    }

    /// Create a RealRethEngine from this configuration
    pub async fn create_reth_engine(&self) -> Result<RealRethEngine, RethEngineError> {
        RealRethEngine::new_with_config(
            self.data_dir_path(),
            self.rpc.port,
            self.node.chain_id,
            self.to_connection_config(),
        )
        .await
    }

    /// Print configuration summary
    pub fn print_summary(&self) {
        info!("=== MultiVM Reth Configuration Summary ===");
        info!(
            "Node: {} (Chain ID: {})",
            self.node.name, self.node.chain_id
        );
        info!("Data Directory: {}", self.reth.data_dir);
        info!("RPC: {}:{}", self.rpc.host, self.rpc.port);
        info!(
            "Engine API: {}:{}",
            self.engine_api.host, self.engine_api.port
        );
        info!("JWT Secret: {}", self.engine_api.jwt_secret_path);
        info!("IPC Socket: {}", self.multivm.ipc_path);
        info!(
            "Chain: {} (dev_mode: {})",
            self.reth.chain, self.reth.dev_mode
        );
        info!("==========================================");
    }
}

/// Helper function to load configuration from standard locations
pub fn load_configuration() -> Result<RethMultiVMConfig, RethEngineError> {
    // Try environment variables first (for setup script integration)
    if std::env::var("RETH_DATA_DIR").is_ok() {
        info!("Loading configuration from environment variables");
        return RethMultiVMConfig::load_from_env();
    }

    // Try standard configuration file locations
    let config_paths = [
        PathBuf::from("configs/reth-multivm.toml"),
        PathBuf::from("./reth-multivm.toml"),
        PathBuf::from("/etc/multivm/reth.toml"),
    ];

    for path in &config_paths {
        if path.exists() {
            info!("Found configuration file: {:?}", path);
            return RethMultiVMConfig::load_from_file(path);
        }
    }

    // Fallback to default configuration
    warn!("No configuration file found, using defaults");
    let config = RethMultiVMConfig::default();
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config_validation() {
        let config = RethMultiVMConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = RethMultiVMConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: RethMultiVMConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.node.chain_id, parsed.node.chain_id);
    }

    #[test]
    fn test_config_file_round_trip() {
        let config = RethMultiVMConfig::default();
        let temp_file = NamedTempFile::new().unwrap();

        config
            .save_to_file(&temp_file.path().to_path_buf())
            .unwrap();
        let loaded = RethMultiVMConfig::load_from_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.node.chain_id, loaded.node.chain_id);
        assert_eq!(config.rpc.port, loaded.rpc.port);
    }

    #[test]
    fn test_validation_errors() {
        let mut config = RethMultiVMConfig::default();

        // Test port conflict
        config.engine_api.port = config.rpc.port;
        assert!(config.validate().is_err());

        // Reset and test timeout validation
        config.engine_api.port = 8551;
        config.multivm.coordination.health_check_timeout_seconds = 100;
        config.multivm.coordination.health_check_interval_seconds = 50;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_env_var_loading() {
        std::env::set_var("RETH_DATA_DIR", "/tmp/test-reth");
        std::env::set_var("RETH_HTTP_PORT", "8546");
        std::env::set_var("RETH_CHAIN_ID", "1337");

        let config = RethMultiVMConfig::load_from_env().unwrap();
        assert_eq!(config.reth.data_dir, "/tmp/test-reth");
        assert_eq!(config.rpc.port, 8546);
        assert_eq!(config.node.chain_id, 1337);

        // Cleanup
        std::env::remove_var("RETH_DATA_DIR");
        std::env::remove_var("RETH_HTTP_PORT");
        std::env::remove_var("RETH_CHAIN_ID");
    }
}
