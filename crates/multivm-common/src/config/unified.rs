//! Unified configuration structure for MultiVM
//!
//! This module defines the canonical configuration schema that all MultiVM
//! components should use. It provides a consistent structure and naming
//! convention across the entire system.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main configuration structure for MultiVM
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultivmUnifiedConfig {
    #[serde(default)]
    pub system: SystemConfig,

    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub database: DatabaseConfig,

    #[serde(default)]
    pub cache: CacheConfig,

    #[serde(default)]
    pub blockchain_clients: BlockchainClientsConfig,

    #[serde(default)]
    pub consensus: ConsensusConfig,

    #[serde(default)]
    pub network: NetworkConfig,

    #[serde(default)]
    pub ipc: IpcConfig,

    #[serde(default)]
    pub security: SecurityConfig,

    #[serde(default)]
    pub monitoring: MonitoringConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub execution_engines: ExecutionEnginesConfig,
}

/// System-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub data_dir: PathBuf,
    pub log_level: String,
    pub log_format: String,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub max_processes: u32,
    pub process_restart_delay_seconds: u64,
    pub health_check_interval_seconds: u64,
    pub shutdown_timeout_seconds: u64,
}

/// Server configuration for all HTTP/WebSocket endpoints
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub rest: RestServerConfig,
    pub graphql: GraphqlServerConfig,
    pub websocket: WebsocketServerConfig,
    pub admin: AdminServerConfig,
}

/// REST API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_cors: bool,
    pub request_timeout_seconds: u64,
    pub max_request_size_bytes: u64,
    pub enable_compression: bool,
}

/// GraphQL server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphqlServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_playground: bool,
    pub query_timeout_seconds: u64,
    pub query_complexity_limit: u32,
    pub enable_introspection: bool,
}

/// WebSocket server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketServerConfig {
    pub host: String,
    pub port: u16,
    pub connection_timeout_seconds: u64,
    pub max_connections: u32,
    pub ping_interval_seconds: u64,
}

/// Admin interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_ui: bool,
    pub require_auth: bool,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub connection_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub query_timeout_seconds: u64,
    pub enable_ssl: bool,
    pub migration_timeout_seconds: u64,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub strategy: CacheStrategy,
    pub default_ttl_seconds: u64,
    pub max_memory_bytes: u64,
    pub redis: RedisCacheConfig,
    pub memory: MemoryCacheConfig,
}

/// Cache strategy enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CacheStrategy {
    WriteThrough,
    WriteBack,
    WriteAround,
}

/// Redis cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCacheConfig {
    pub connection_url: String,
    pub key_prefix: String,
    pub connection_timeout_seconds: u64,
    pub command_timeout_seconds: u64,
    pub enable_cluster: bool,
}

/// In-memory cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCacheConfig {
    pub max_items: u64,
    pub eviction_policy: EvictionPolicy,
}

/// Cache eviction policy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Fifo,
}

/// Blockchain clients configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainClientsConfig {
    pub solana: BlockchainClientConfig,
    pub ethereum: BlockchainClientConfig,
    pub multivm: BlockchainClientConfig,
}

/// Individual blockchain client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainClientConfig {
    pub rpc_url: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_backoff_seconds: u64,
    pub enable_health_checks: bool,
    pub health_check_interval_seconds: u64,
}

/// Consensus mechanism configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub algorithm: String,
    pub block_time_milliseconds: u64,
    pub validator_count: u32,
    pub enable_single_node: bool,
    pub finality_depth: u32,
    pub max_block_size_bytes: u64,
    pub malachite: MalachiteConfig,
}

/// Malachite consensus specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MalachiteConfig {
    pub timeout_propose_milliseconds: u64,
    pub timeout_prevote_milliseconds: u64,
    pub timeout_precommit_milliseconds: u64,
    pub timeout_commit_milliseconds: u64,
    pub skip_timeout_commit: bool,
}

/// Network P2P configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub enable_p2p: bool,
    pub listen_host: String,
    pub listen_port: u16,
    pub external_address: String,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
    pub enable_upnp: bool,
    pub discovery: DiscoveryConfig,
}

/// Network discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub enable_mdns: bool,
    pub enable_kad: bool,
    pub bootstrap_nodes: Vec<String>,
}

/// Inter-process communication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    pub transport: IpcTransport,
    pub socket_path: PathBuf,
    pub tcp_host: String,
    pub tcp_port: u16,
    pub timeout_milliseconds: u64,
    pub enable_encryption: bool,
    pub max_message_size_bytes: u64,
}

/// IPC transport type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcTransport {
    UnixSocket,
    Tcp,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_authentication: bool,
    pub enable_encryption: bool,
    pub enable_rate_limiting: bool,
    pub max_requests_per_minute: u32,
    pub jwt_expiration_hours: u32,
    pub api_key_length: u32,
    pub secrets: SecretsConfig,
}

/// Configuration for environment variable secrets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    pub jwt_secret_env: String,
    pub database_password_env: String,
    pub redis_password_env: String,
    pub encryption_key_env: String,
}

/// Monitoring and observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_prometheus: bool,
    pub prometheus_host: String,
    pub prometheus_port: u16,
    pub enable_jaeger: bool,
    pub jaeger_endpoint: String,
    pub service_name: String,
    pub health_check: HealthCheckConfig,
}

/// Health check endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub enable_endpoint: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub enable_file_logging: bool,
    pub log_directory: PathBuf,
    pub max_file_size_bytes: u64,
    pub max_log_files: u32,
    pub enable_log_rotation: bool,
}

/// Log format enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

/// Execution engines configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEnginesConfig {
    pub enable_mock_solana: bool,
    pub enable_mock_ethereum: bool,
    pub solana: ExecutionEngineConfig,
    pub ethereum: ExecutionEngineConfig,
}

/// Individual execution engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEngineConfig {
    pub binary_path: PathBuf,
    pub config_path: PathBuf,
    pub data_dir: PathBuf,
    pub enable_rpc: bool,
    pub rpc_port: u16,
}

// Default implementations for all configuration structures

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("/opt/multivm/data"),
            log_level: "info".to_string(),
            log_format: "json".to_string(),
            enable_metrics: true,
            enable_tracing: true,
            max_processes: 20,
            process_restart_delay_seconds: 5,
            health_check_interval_seconds: 30,
            shutdown_timeout_seconds: 30,
        }
    }
}

impl Default for RestServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: false,
            request_timeout_seconds: 30,
            max_request_size_bytes: 16_777_216, // 16MB
            enable_compression: true,
        }
    }
}

impl Default for GraphqlServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8081,
            enable_playground: false,
            query_timeout_seconds: 30,
            query_complexity_limit: 1000,
            enable_introspection: false,
        }
    }
}

impl Default for WebsocketServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8082,
            connection_timeout_seconds: 60,
            max_connections: 1000,
            ping_interval_seconds: 30,
        }
    }
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8083,
            enable_ui: false,
            require_auth: true,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            connection_url: "postgresql://multivm:multivm@localhost:5432/multivm".to_string(),
            max_connections: 100,
            min_connections: 10,
            connection_timeout_seconds: 30,
            query_timeout_seconds: 60,
            enable_ssl: true,
            migration_timeout_seconds: 300,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            strategy: CacheStrategy::WriteThrough,
            default_ttl_seconds: 300,
            max_memory_bytes: 1_073_741_824, // 1GB
            redis: RedisCacheConfig::default(),
            memory: MemoryCacheConfig::default(),
        }
    }
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            connection_url: "redis://localhost:6379/0".to_string(),
            key_prefix: "multivm:".to_string(),
            connection_timeout_seconds: 5,
            command_timeout_seconds: 10,
            enable_cluster: false,
        }
    }
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_items: 100_000,
            eviction_policy: EvictionPolicy::Lru,
        }
    }
}

impl Default for BlockchainClientsConfig {
    fn default() -> Self {
        Self {
            solana: BlockchainClientConfig {
                rpc_url: "http://localhost:8899".to_string(),
                timeout_seconds: 30,
                max_retries: 3,
                retry_backoff_seconds: 2,
                enable_health_checks: true,
                health_check_interval_seconds: 60,
            },
            ethereum: BlockchainClientConfig {
                rpc_url: "http://localhost:8545".to_string(),
                timeout_seconds: 30,
                max_retries: 3,
                retry_backoff_seconds: 2,
                enable_health_checks: true,
                health_check_interval_seconds: 60,
            },
            multivm: BlockchainClientConfig {
                rpc_url: "http://localhost:9000".to_string(),
                timeout_seconds: 30,
                max_retries: 3,
                retry_backoff_seconds: 2,
                enable_health_checks: true,
                health_check_interval_seconds: 60,
            },
        }
    }
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: "malachite".to_string(),
            block_time_milliseconds: 2000,
            validator_count: 4,
            enable_single_node: false,
            finality_depth: 6,
            max_block_size_bytes: 2_097_152, // 2MB
            malachite: MalachiteConfig::default(),
        }
    }
}

impl Default for MalachiteConfig {
    fn default() -> Self {
        Self {
            timeout_propose_milliseconds: 3000,
            timeout_prevote_milliseconds: 1000,
            timeout_precommit_milliseconds: 1000,
            timeout_commit_milliseconds: 1000,
            skip_timeout_commit: false,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enable_p2p: true,
            listen_host: "0.0.0.0".to_string(),
            listen_port: 26656,
            external_address: String::new(),
            max_connections: 100,
            connection_timeout_seconds: 10,
            enable_upnp: false,
            discovery: DiscoveryConfig::default(),
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            enable_kad: true,
            bootstrap_nodes: Vec::new(),
        }
    }
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            transport: IpcTransport::UnixSocket,
            socket_path: PathBuf::from("/tmp/multivm.sock"),
            tcp_host: "127.0.0.1".to_string(),
            tcp_port: 9090,
            timeout_milliseconds: 10000,
            enable_encryption: true,
            max_message_size_bytes: 16_777_216, // 16MB
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_authentication: true,
            enable_encryption: true,
            enable_rate_limiting: true,
            max_requests_per_minute: 1000,
            jwt_expiration_hours: 24,
            api_key_length: 32,
            secrets: SecretsConfig::default(),
        }
    }
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            jwt_secret_env: "MULTIVM_JWT_SECRET".to_string(),
            database_password_env: "MULTIVM_DB_PASSWORD".to_string(),
            redis_password_env: "MULTIVM_REDIS_PASSWORD".to_string(),
            encryption_key_env: "MULTIVM_ENCRYPTION_KEY".to_string(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_prometheus: true,
            prometheus_host: "0.0.0.0".to_string(),
            prometheus_port: 9090,
            enable_jaeger: true,
            jaeger_endpoint: "http://localhost:14268/api/traces".to_string(),
            service_name: "multivm".to_string(),
            health_check: HealthCheckConfig::default(),
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enable_endpoint: true,
            host: "0.0.0.0".to_string(),
            port: 8090,
            path: "/health".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Json,
            enable_file_logging: true,
            log_directory: PathBuf::from("/var/log/multivm"),
            max_file_size_bytes: 104_857_600, // 100MB
            max_log_files: 10,
            enable_log_rotation: true,
        }
    }
}

impl Default for ExecutionEnginesConfig {
    fn default() -> Self {
        Self {
            enable_mock_solana: true,
            enable_mock_ethereum: true,
            solana: ExecutionEngineConfig {
                binary_path: PathBuf::from("/usr/local/bin/solana-validator"),
                config_path: PathBuf::from("/opt/multivm/solana/validator.json"),
                data_dir: PathBuf::from("/opt/multivm/data/solana"),
                enable_rpc: true,
                rpc_port: 8899,
            },
            ethereum: ExecutionEngineConfig {
                binary_path: PathBuf::from("/usr/local/bin/reth"),
                config_path: PathBuf::from("/opt/multivm/ethereum/reth.toml"),
                data_dir: PathBuf::from("/opt/multivm/data/ethereum"),
                enable_rpc: true,
                rpc_port: 8545,
            },
        }
    }
}

// Utility methods for configuration management
impl MultivmUnifiedConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &std::path::Path) -> crate::MultivmResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::MultivmError::Configuration(format!("Failed to read config file: {}", e))
        })?;

        let config: Self = toml::from_str(&content).map_err(|e| {
            crate::MultivmError::Configuration(format!("Failed to parse config: {}", e))
        })?;

        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file(&self, path: &std::path::Path) -> crate::MultivmResult<()> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            crate::MultivmError::Configuration(format!("Failed to serialize config: {}", e))
        })?;

        std::fs::write(path, content).map_err(|e| {
            crate::MultivmError::Configuration(format!("Failed to write config file: {}", e))
        })?;

        Ok(())
    }

    /// Merge with environment-specific overrides
    pub fn merge_from_file(&mut self, path: &std::path::Path) -> crate::MultivmResult<()> {
        let override_config = Self::from_file(path)?;
        // TODO: Implement deep merge logic for configuration overrides
        *self = override_config;
        Ok(())
    }

    /// Validate configuration for consistency and completeness
    pub fn validate(&self) -> crate::MultivmResult<()> {
        // TODO: Implement comprehensive validation logic
        // Check port conflicts, path accessibility, etc.
        Ok(())
    }

    /// Get duration values as std::time::Duration
    pub fn process_restart_delay(&self) -> Duration {
        Duration::from_secs(self.system.process_restart_delay_seconds)
    }

    pub fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.system.health_check_interval_seconds)
    }

    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.system.shutdown_timeout_seconds)
    }
}
