//! Unified Configuration System
//!
//! This module provides a single, consistent configuration system that
//! eliminates duplication and provides clear defaults for all components.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main configuration structure for all MultiVM components
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultivmConfig {
    /// System-wide settings
    pub system: SystemConfig,
    /// Server configuration
    pub server: ServerConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Blockchain client configuration
    pub blockchain: BlockchainConfig,
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    /// Network configuration
    pub network: NetworkConfig,
    /// IPC configuration
    pub ipc: IpcConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// System-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub data_dir: PathBuf,
    pub log_level: String,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub max_processes: u32,
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
}

/// GraphQL server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphqlServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_playground: bool,
    pub query_timeout_seconds: u64,
    pub query_complexity_limit: u32,
}

/// WebSocket server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketServerConfig {
    pub host: String,
    pub port: u16,
    pub connection_timeout_seconds: u64,
    pub max_connections: u32,
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
    pub enable_ssl: bool,
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
}

/// In-memory cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCacheConfig {
    pub max_items: u64,
    pub eviction_policy: EvictionPolicy,
}

/// Cache eviction policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Fifo,
}

/// Blockchain clients configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    pub solana: BlockchainClientConfig,
    pub ethereum: BlockchainClientConfig,
}

/// Individual blockchain client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainClientConfig {
    pub rpc_url: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_backoff_seconds: u64,
    pub enable_health_checks: bool,
}

/// Consensus mechanism configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub algorithm: String,
    pub block_time_milliseconds: u64,
    pub validator_count: u32,
    pub enable_single_node: bool,
    pub finality_depth: u32,
}

/// Network P2P configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub enable_p2p: bool,
    pub listen_host: String,
    pub listen_port: u16,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
}

/// Inter-process communication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    pub transport: IpcTransportConfig,
    pub message_timeout: Duration,
    pub enable_encryption: bool,
}

/// IPC transport type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcTransport {
    UnixSocket,
    Tcp,
}

/// IPC transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcTransportConfig {
    UnixSocket { path: PathBuf },
    TcpSocket { host: String, port: u16 },
}

impl Default for IpcTransportConfig {
    fn default() -> Self {
        Self::UnixSocket {
            path: PathBuf::from("/tmp/multivm.sock"),
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_authentication: bool,
    pub enable_encryption: bool,
    pub enable_rate_limiting: bool,
    pub max_requests_per_minute: u32,
    pub jwt_expiration_hours: u32,
    pub jwt_secret: String,
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
}

/// Log format enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

/// VM type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmType {
    Evm,
    Svm,
}

impl std::fmt::Display for VmType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmType::Evm => write!(f, "EVM"),
            VmType::Svm => write!(f, "SVM"),
        }
    }
}

/// Ethereum-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub enable_tracing: bool,
}

impl Default for EthereumConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            chain_id: 1,
            gas_limit: 21000,
            gas_price: 20_000_000_000, // 20 gwei
            enable_tracing: false,
        }
    }
}

/// Solana-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub commitment: String,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub enable_rpc_logging: bool,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8899".to_string(),
            commitment: "confirmed".to_string(),
            max_retries: 3,
            timeout_seconds: 30,
            enable_rpc_logging: false,
        }
    }
}

// Default implementations

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            log_level: "info".to_string(),
            enable_metrics: true,
            enable_tracing: true,
            max_processes: 20,
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
            enable_ssl: true,
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

impl Default for BlockchainConfig {
    fn default() -> Self {
        Self {
            solana: BlockchainClientConfig {
                rpc_url: "http://localhost:8899".to_string(),
                timeout_seconds: 30,
                max_retries: 3,
                retry_backoff_seconds: 2,
                enable_health_checks: true,
            },
            ethereum: BlockchainClientConfig {
                rpc_url: "http://localhost:8545".to_string(),
                timeout_seconds: 30,
                max_retries: 3,
                retry_backoff_seconds: 2,
                enable_health_checks: true,
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
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enable_p2p: true,
            listen_host: "0.0.0.0".to_string(),
            listen_port: 26656,
            max_connections: 100,
            connection_timeout_seconds: 10,
        }
    }
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            transport: IpcTransportConfig::default(),
            message_timeout: Duration::from_secs(30),
            enable_encryption: true,
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
            jwt_secret: Self::generate_jwt_secret(),
        }
    }
}

impl SecurityConfig {
    fn generate_jwt_secret() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let secret: [u8; 32] = rng.gen();
        hex::encode(secret)
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_prometheus: true,
            prometheus_host: "0.0.0.0".to_string(),
            prometheus_port: 9090,
            enable_jaeger: false,
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
            log_directory: PathBuf::from("./logs"),
            max_file_size_bytes: 104_857_600, // 100MB
            max_log_files: 10,
        }
    }
}

impl MultivmConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &std::path::Path) -> crate::MultivmResult<Self> {
        let content =
            std::fs::read_to_string(path).map_err(|e| crate::MultivmError::Configuration {
                component: "config".to_string(),
                message: format!("Failed to read config file: {e}"),
                validation_errors: None,
            })?;

        let config: Self =
            toml::from_str(&content).map_err(|e| crate::MultivmError::Configuration {
                component: "config".to_string(),
                message: format!("Failed to parse config: {e}"),
                validation_errors: None,
            })?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file(&self, path: &std::path::Path) -> crate::MultivmResult<()> {
        let content =
            toml::to_string_pretty(self).map_err(|e| crate::MultivmError::Configuration {
                component: "config".to_string(),
                message: format!("Failed to serialize config: {e}"),
                validation_errors: None,
            })?;

        std::fs::write(path, content).map_err(|e| crate::MultivmError::Configuration {
            component: "config".to_string(),
            message: format!("Failed to write config file: {e}"),
            validation_errors: None,
        })?;

        Ok(())
    }

    /// Validate configuration for consistency and completeness
    pub fn validate(&self) -> crate::MultivmResult<()> {
        use std::collections::HashSet;

        // Check for port conflicts
        let mut used_ports = HashSet::new();
        let ports = [
            ("REST", self.server.rest.port),
            ("GraphQL", self.server.graphql.port),
            ("WebSocket", self.server.websocket.port),
            ("Admin", self.server.admin.port),
            ("Prometheus", self.monitoring.prometheus_port),
            ("Health", self.monitoring.health_check.port),
            ("Network", self.network.listen_port),
        ];

        for (name, port) in &ports {
            if !used_ports.insert(port) {
                return Err(crate::MultivmError::Configuration {
                    component: "config".to_string(),
                    message: format!("Port conflict: {name} port {port} is already in use"),
                    validation_errors: None,
                });
            }
        }

        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.system.log_level.as_str()) {
            return Err(crate::MultivmError::Configuration {
                component: "config".to_string(),
                message: format!(
                    "Invalid log level '{}'. Must be one of: {:?}",
                    self.system.log_level, valid_log_levels
                ),
                validation_errors: None,
            });
        }

        // Validate JWT secret length
        if self.security.jwt_secret.len() < 32 {
            return Err(crate::MultivmError::Configuration {
                component: "security".to_string(),
                message: "JWT secret must be at least 32 characters long".to_string(),
                validation_errors: None,
            });
        }

        Ok(())
    }

    /// Get duration values as std::time::Duration
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.system.shutdown_timeout_seconds)
    }

    /// Get socket addresses for servers
    pub fn rest_socket_addr(&self) -> String {
        format!("{}:{}", self.server.rest.host, self.server.rest.port)
    }

    pub fn graphql_socket_addr(&self) -> String {
        format!("{}:{}", self.server.graphql.host, self.server.graphql.port)
    }

    pub fn websocket_socket_addr(&self) -> String {
        format!(
            "{}:{}",
            self.server.websocket.host, self.server.websocket.port
        )
    }

    pub fn admin_socket_addr(&self) -> String {
        format!("{}:{}", self.server.admin.host, self.server.admin.port)
    }
}
