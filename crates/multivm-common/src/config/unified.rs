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

        // Deep merge system config
        if override_config.system.data_dir != SystemConfig::default().data_dir {
            self.system.data_dir = override_config.system.data_dir;
        }
        if override_config.system.log_level != SystemConfig::default().log_level {
            self.system.log_level = override_config.system.log_level;
        }
        if override_config.system.log_format != SystemConfig::default().log_format {
            self.system.log_format = override_config.system.log_format;
        }
        if override_config.system.enable_metrics != SystemConfig::default().enable_metrics {
            self.system.enable_metrics = override_config.system.enable_metrics;
        }
        if override_config.system.enable_tracing != SystemConfig::default().enable_tracing {
            self.system.enable_tracing = override_config.system.enable_tracing;
        }
        if override_config.system.max_processes != SystemConfig::default().max_processes {
            self.system.max_processes = override_config.system.max_processes;
        }
        if override_config.system.process_restart_delay_seconds
            != SystemConfig::default().process_restart_delay_seconds
        {
            self.system.process_restart_delay_seconds =
                override_config.system.process_restart_delay_seconds;
        }
        if override_config.system.health_check_interval_seconds
            != SystemConfig::default().health_check_interval_seconds
        {
            self.system.health_check_interval_seconds =
                override_config.system.health_check_interval_seconds;
        }
        if override_config.system.shutdown_timeout_seconds
            != SystemConfig::default().shutdown_timeout_seconds
        {
            self.system.shutdown_timeout_seconds = override_config.system.shutdown_timeout_seconds;
        }

        // Deep merge server config
        if override_config.server.rest.host != RestServerConfig::default().host {
            self.server.rest.host = override_config.server.rest.host;
        }
        if override_config.server.rest.port != RestServerConfig::default().port {
            self.server.rest.port = override_config.server.rest.port;
        }
        if override_config.server.rest.enable_cors != RestServerConfig::default().enable_cors {
            self.server.rest.enable_cors = override_config.server.rest.enable_cors;
        }
        if override_config.server.rest.request_timeout_seconds
            != RestServerConfig::default().request_timeout_seconds
        {
            self.server.rest.request_timeout_seconds =
                override_config.server.rest.request_timeout_seconds;
        }
        if override_config.server.rest.max_request_size_bytes
            != RestServerConfig::default().max_request_size_bytes
        {
            self.server.rest.max_request_size_bytes =
                override_config.server.rest.max_request_size_bytes;
        }
        if override_config.server.rest.enable_compression
            != RestServerConfig::default().enable_compression
        {
            self.server.rest.enable_compression = override_config.server.rest.enable_compression;
        }

        // Deep merge GraphQL server config
        if override_config.server.graphql.host != GraphqlServerConfig::default().host {
            self.server.graphql.host = override_config.server.graphql.host;
        }
        if override_config.server.graphql.port != GraphqlServerConfig::default().port {
            self.server.graphql.port = override_config.server.graphql.port;
        }
        if override_config.server.graphql.enable_playground
            != GraphqlServerConfig::default().enable_playground
        {
            self.server.graphql.enable_playground =
                override_config.server.graphql.enable_playground;
        }
        if override_config.server.graphql.query_timeout_seconds
            != GraphqlServerConfig::default().query_timeout_seconds
        {
            self.server.graphql.query_timeout_seconds =
                override_config.server.graphql.query_timeout_seconds;
        }
        if override_config.server.graphql.query_complexity_limit
            != GraphqlServerConfig::default().query_complexity_limit
        {
            self.server.graphql.query_complexity_limit =
                override_config.server.graphql.query_complexity_limit;
        }
        if override_config.server.graphql.enable_introspection
            != GraphqlServerConfig::default().enable_introspection
        {
            self.server.graphql.enable_introspection =
                override_config.server.graphql.enable_introspection;
        }

        // Deep merge WebSocket server config
        if override_config.server.websocket.host != WebsocketServerConfig::default().host {
            self.server.websocket.host = override_config.server.websocket.host;
        }
        if override_config.server.websocket.port != WebsocketServerConfig::default().port {
            self.server.websocket.port = override_config.server.websocket.port;
        }
        if override_config.server.websocket.connection_timeout_seconds
            != WebsocketServerConfig::default().connection_timeout_seconds
        {
            self.server.websocket.connection_timeout_seconds =
                override_config.server.websocket.connection_timeout_seconds;
        }
        if override_config.server.websocket.max_connections
            != WebsocketServerConfig::default().max_connections
        {
            self.server.websocket.max_connections =
                override_config.server.websocket.max_connections;
        }
        if override_config.server.websocket.ping_interval_seconds
            != WebsocketServerConfig::default().ping_interval_seconds
        {
            self.server.websocket.ping_interval_seconds =
                override_config.server.websocket.ping_interval_seconds;
        }

        // Deep merge Admin server config
        if override_config.server.admin.host != AdminServerConfig::default().host {
            self.server.admin.host = override_config.server.admin.host;
        }
        if override_config.server.admin.port != AdminServerConfig::default().port {
            self.server.admin.port = override_config.server.admin.port;
        }
        if override_config.server.admin.enable_ui != AdminServerConfig::default().enable_ui {
            self.server.admin.enable_ui = override_config.server.admin.enable_ui;
        }
        if override_config.server.admin.require_auth != AdminServerConfig::default().require_auth {
            self.server.admin.require_auth = override_config.server.admin.require_auth;
        }

        // Deep merge database config
        if override_config.database.connection_url != DatabaseConfig::default().connection_url {
            self.database.connection_url = override_config.database.connection_url;
        }
        if override_config.database.max_connections != DatabaseConfig::default().max_connections {
            self.database.max_connections = override_config.database.max_connections;
        }
        if override_config.database.min_connections != DatabaseConfig::default().min_connections {
            self.database.min_connections = override_config.database.min_connections;
        }
        if override_config.database.connection_timeout_seconds
            != DatabaseConfig::default().connection_timeout_seconds
        {
            self.database.connection_timeout_seconds =
                override_config.database.connection_timeout_seconds;
        }
        if override_config.database.query_timeout_seconds
            != DatabaseConfig::default().query_timeout_seconds
        {
            self.database.query_timeout_seconds = override_config.database.query_timeout_seconds;
        }
        if override_config.database.enable_ssl != DatabaseConfig::default().enable_ssl {
            self.database.enable_ssl = override_config.database.enable_ssl;
        }
        if override_config.database.migration_timeout_seconds
            != DatabaseConfig::default().migration_timeout_seconds
        {
            self.database.migration_timeout_seconds =
                override_config.database.migration_timeout_seconds;
        }

        // Deep merge cache config
        if !matches!(override_config.cache.strategy, CacheStrategy::WriteThrough) {
            self.cache.strategy = override_config.cache.strategy;
        }
        if override_config.cache.default_ttl_seconds != CacheConfig::default().default_ttl_seconds {
            self.cache.default_ttl_seconds = override_config.cache.default_ttl_seconds;
        }
        if override_config.cache.max_memory_bytes != CacheConfig::default().max_memory_bytes {
            self.cache.max_memory_bytes = override_config.cache.max_memory_bytes;
        }

        // Deep merge Redis cache config
        if override_config.cache.redis.connection_url != RedisCacheConfig::default().connection_url
        {
            self.cache.redis.connection_url = override_config.cache.redis.connection_url;
        }
        if override_config.cache.redis.key_prefix != RedisCacheConfig::default().key_prefix {
            self.cache.redis.key_prefix = override_config.cache.redis.key_prefix;
        }
        if override_config.cache.redis.connection_timeout_seconds
            != RedisCacheConfig::default().connection_timeout_seconds
        {
            self.cache.redis.connection_timeout_seconds =
                override_config.cache.redis.connection_timeout_seconds;
        }
        if override_config.cache.redis.command_timeout_seconds
            != RedisCacheConfig::default().command_timeout_seconds
        {
            self.cache.redis.command_timeout_seconds =
                override_config.cache.redis.command_timeout_seconds;
        }
        if override_config.cache.redis.enable_cluster != RedisCacheConfig::default().enable_cluster
        {
            self.cache.redis.enable_cluster = override_config.cache.redis.enable_cluster;
        }

        // Deep merge memory cache config
        if override_config.cache.memory.max_items != MemoryCacheConfig::default().max_items {
            self.cache.memory.max_items = override_config.cache.memory.max_items;
        }
        if !matches!(
            override_config.cache.memory.eviction_policy,
            EvictionPolicy::Lru
        ) {
            self.cache.memory.eviction_policy =
                override_config.cache.memory.eviction_policy.clone();
        }

        // Deep merge blockchain clients config
        if override_config.blockchain_clients.solana.rpc_url
            != BlockchainClientsConfig::default().solana.rpc_url
        {
            self.blockchain_clients.solana.rpc_url =
                override_config.blockchain_clients.solana.rpc_url;
        }
        if override_config.blockchain_clients.ethereum.rpc_url
            != BlockchainClientsConfig::default().ethereum.rpc_url
        {
            self.blockchain_clients.ethereum.rpc_url =
                override_config.blockchain_clients.ethereum.rpc_url;
        }
        if override_config.blockchain_clients.multivm.rpc_url
            != BlockchainClientsConfig::default().multivm.rpc_url
        {
            self.blockchain_clients.multivm.rpc_url =
                override_config.blockchain_clients.multivm.rpc_url;
        }

        // Deep merge consensus config
        if override_config.consensus.algorithm != ConsensusConfig::default().algorithm {
            self.consensus.algorithm = override_config.consensus.algorithm;
        }
        if override_config.consensus.block_time_milliseconds
            != ConsensusConfig::default().block_time_milliseconds
        {
            self.consensus.block_time_milliseconds =
                override_config.consensus.block_time_milliseconds;
        }
        if override_config.consensus.validator_count != ConsensusConfig::default().validator_count {
            self.consensus.validator_count = override_config.consensus.validator_count;
        }
        if override_config.consensus.enable_single_node
            != ConsensusConfig::default().enable_single_node
        {
            self.consensus.enable_single_node = override_config.consensus.enable_single_node;
        }
        if override_config.consensus.finality_depth != ConsensusConfig::default().finality_depth {
            self.consensus.finality_depth = override_config.consensus.finality_depth;
        }
        if override_config.consensus.max_block_size_bytes
            != ConsensusConfig::default().max_block_size_bytes
        {
            self.consensus.max_block_size_bytes = override_config.consensus.max_block_size_bytes;
        }

        // Deep merge Malachite config
        if override_config
            .consensus
            .malachite
            .timeout_propose_milliseconds
            != MalachiteConfig::default().timeout_propose_milliseconds
        {
            self.consensus.malachite.timeout_propose_milliseconds = override_config
                .consensus
                .malachite
                .timeout_propose_milliseconds;
        }
        if override_config
            .consensus
            .malachite
            .timeout_prevote_milliseconds
            != MalachiteConfig::default().timeout_prevote_milliseconds
        {
            self.consensus.malachite.timeout_prevote_milliseconds = override_config
                .consensus
                .malachite
                .timeout_prevote_milliseconds;
        }
        if override_config
            .consensus
            .malachite
            .timeout_precommit_milliseconds
            != MalachiteConfig::default().timeout_precommit_milliseconds
        {
            self.consensus.malachite.timeout_precommit_milliseconds = override_config
                .consensus
                .malachite
                .timeout_precommit_milliseconds;
        }
        if override_config
            .consensus
            .malachite
            .timeout_commit_milliseconds
            != MalachiteConfig::default().timeout_commit_milliseconds
        {
            self.consensus.malachite.timeout_commit_milliseconds = override_config
                .consensus
                .malachite
                .timeout_commit_milliseconds;
        }
        if override_config.consensus.malachite.skip_timeout_commit
            != MalachiteConfig::default().skip_timeout_commit
        {
            self.consensus.malachite.skip_timeout_commit =
                override_config.consensus.malachite.skip_timeout_commit;
        }

        // Deep merge network config
        if override_config.network.enable_p2p != NetworkConfig::default().enable_p2p {
            self.network.enable_p2p = override_config.network.enable_p2p;
        }
        if override_config.network.listen_host != NetworkConfig::default().listen_host {
            self.network.listen_host = override_config.network.listen_host;
        }
        if override_config.network.listen_port != NetworkConfig::default().listen_port {
            self.network.listen_port = override_config.network.listen_port;
        }
        if !override_config.network.external_address.is_empty() {
            self.network.external_address = override_config.network.external_address;
        }
        if override_config.network.max_connections != NetworkConfig::default().max_connections {
            self.network.max_connections = override_config.network.max_connections;
        }
        if override_config.network.connection_timeout_seconds
            != NetworkConfig::default().connection_timeout_seconds
        {
            self.network.connection_timeout_seconds =
                override_config.network.connection_timeout_seconds;
        }
        if override_config.network.enable_upnp != NetworkConfig::default().enable_upnp {
            self.network.enable_upnp = override_config.network.enable_upnp;
        }

        // Deep merge discovery config
        if override_config.network.discovery.enable_mdns != DiscoveryConfig::default().enable_mdns {
            self.network.discovery.enable_mdns = override_config.network.discovery.enable_mdns;
        }
        if override_config.network.discovery.enable_kad != DiscoveryConfig::default().enable_kad {
            self.network.discovery.enable_kad = override_config.network.discovery.enable_kad;
        }
        if !override_config.network.discovery.bootstrap_nodes.is_empty() {
            self.network.discovery.bootstrap_nodes =
                override_config.network.discovery.bootstrap_nodes;
        }

        // Deep merge IPC config
        if !matches!(override_config.ipc.transport, IpcTransport::UnixSocket) {
            self.ipc.transport = override_config.ipc.transport;
        }
        if override_config.ipc.socket_path != IpcConfig::default().socket_path {
            self.ipc.socket_path = override_config.ipc.socket_path;
        }
        if override_config.ipc.tcp_host != IpcConfig::default().tcp_host {
            self.ipc.tcp_host = override_config.ipc.tcp_host;
        }
        if override_config.ipc.tcp_port != IpcConfig::default().tcp_port {
            self.ipc.tcp_port = override_config.ipc.tcp_port;
        }
        if override_config.ipc.timeout_milliseconds != IpcConfig::default().timeout_milliseconds {
            self.ipc.timeout_milliseconds = override_config.ipc.timeout_milliseconds;
        }
        if override_config.ipc.enable_encryption != IpcConfig::default().enable_encryption {
            self.ipc.enable_encryption = override_config.ipc.enable_encryption;
        }
        if override_config.ipc.max_message_size_bytes != IpcConfig::default().max_message_size_bytes
        {
            self.ipc.max_message_size_bytes = override_config.ipc.max_message_size_bytes;
        }

        // Deep merge security config
        if override_config.security.enable_authentication
            != SecurityConfig::default().enable_authentication
        {
            self.security.enable_authentication = override_config.security.enable_authentication;
        }
        if override_config.security.enable_encryption != SecurityConfig::default().enable_encryption
        {
            self.security.enable_encryption = override_config.security.enable_encryption;
        }
        if override_config.security.enable_rate_limiting
            != SecurityConfig::default().enable_rate_limiting
        {
            self.security.enable_rate_limiting = override_config.security.enable_rate_limiting;
        }
        if override_config.security.max_requests_per_minute
            != SecurityConfig::default().max_requests_per_minute
        {
            self.security.max_requests_per_minute =
                override_config.security.max_requests_per_minute;
        }
        if override_config.security.jwt_expiration_hours
            != SecurityConfig::default().jwt_expiration_hours
        {
            self.security.jwt_expiration_hours = override_config.security.jwt_expiration_hours;
        }
        if override_config.security.api_key_length != SecurityConfig::default().api_key_length {
            self.security.api_key_length = override_config.security.api_key_length;
        }

        // Deep merge secrets config
        if override_config.security.secrets.jwt_secret_env
            != SecretsConfig::default().jwt_secret_env
        {
            self.security.secrets.jwt_secret_env = override_config.security.secrets.jwt_secret_env;
        }
        if override_config.security.secrets.database_password_env
            != SecretsConfig::default().database_password_env
        {
            self.security.secrets.database_password_env =
                override_config.security.secrets.database_password_env;
        }
        if override_config.security.secrets.redis_password_env
            != SecretsConfig::default().redis_password_env
        {
            self.security.secrets.redis_password_env =
                override_config.security.secrets.redis_password_env;
        }
        if override_config.security.secrets.encryption_key_env
            != SecretsConfig::default().encryption_key_env
        {
            self.security.secrets.encryption_key_env =
                override_config.security.secrets.encryption_key_env;
        }

        // Deep merge monitoring config
        if override_config.monitoring.enable_prometheus
            != MonitoringConfig::default().enable_prometheus
        {
            self.monitoring.enable_prometheus = override_config.monitoring.enable_prometheus;
        }
        if override_config.monitoring.prometheus_host != MonitoringConfig::default().prometheus_host
        {
            self.monitoring.prometheus_host = override_config.monitoring.prometheus_host;
        }
        if override_config.monitoring.prometheus_port != MonitoringConfig::default().prometheus_port
        {
            self.monitoring.prometheus_port = override_config.monitoring.prometheus_port;
        }
        if override_config.monitoring.enable_jaeger != MonitoringConfig::default().enable_jaeger {
            self.monitoring.enable_jaeger = override_config.monitoring.enable_jaeger;
        }
        if override_config.monitoring.jaeger_endpoint != MonitoringConfig::default().jaeger_endpoint
        {
            self.monitoring.jaeger_endpoint = override_config.monitoring.jaeger_endpoint;
        }
        if override_config.monitoring.service_name != MonitoringConfig::default().service_name {
            self.monitoring.service_name = override_config.monitoring.service_name;
        }

        // Deep merge health check config
        if override_config.monitoring.health_check.enable_endpoint
            != HealthCheckConfig::default().enable_endpoint
        {
            self.monitoring.health_check.enable_endpoint =
                override_config.monitoring.health_check.enable_endpoint;
        }
        if override_config.monitoring.health_check.host != HealthCheckConfig::default().host {
            self.monitoring.health_check.host = override_config.monitoring.health_check.host;
        }
        if override_config.monitoring.health_check.port != HealthCheckConfig::default().port {
            self.monitoring.health_check.port = override_config.monitoring.health_check.port;
        }
        if override_config.monitoring.health_check.path != HealthCheckConfig::default().path {
            self.monitoring.health_check.path = override_config.monitoring.health_check.path;
        }

        // Deep merge logging config
        if override_config.logging.level != LoggingConfig::default().level {
            self.logging.level = override_config.logging.level;
        }
        if !matches!(override_config.logging.format, LogFormat::Json) {
            self.logging.format = override_config.logging.format;
        }
        if override_config.logging.enable_file_logging
            != LoggingConfig::default().enable_file_logging
        {
            self.logging.enable_file_logging = override_config.logging.enable_file_logging;
        }
        if override_config.logging.log_directory != LoggingConfig::default().log_directory {
            self.logging.log_directory = override_config.logging.log_directory;
        }
        if override_config.logging.max_file_size_bytes
            != LoggingConfig::default().max_file_size_bytes
        {
            self.logging.max_file_size_bytes = override_config.logging.max_file_size_bytes;
        }
        if override_config.logging.max_log_files != LoggingConfig::default().max_log_files {
            self.logging.max_log_files = override_config.logging.max_log_files;
        }
        if override_config.logging.enable_log_rotation
            != LoggingConfig::default().enable_log_rotation
        {
            self.logging.enable_log_rotation = override_config.logging.enable_log_rotation;
        }

        // Deep merge execution engines config
        if override_config.execution_engines.enable_mock_solana
            != ExecutionEnginesConfig::default().enable_mock_solana
        {
            self.execution_engines.enable_mock_solana =
                override_config.execution_engines.enable_mock_solana;
        }
        if override_config.execution_engines.enable_mock_ethereum
            != ExecutionEnginesConfig::default().enable_mock_ethereum
        {
            self.execution_engines.enable_mock_ethereum =
                override_config.execution_engines.enable_mock_ethereum;
        }

        // Deep merge Solana execution engine config
        if override_config.execution_engines.solana.binary_path
            != ExecutionEnginesConfig::default().solana.binary_path
        {
            self.execution_engines.solana.binary_path =
                override_config.execution_engines.solana.binary_path;
        }
        if override_config.execution_engines.solana.config_path
            != ExecutionEnginesConfig::default().solana.config_path
        {
            self.execution_engines.solana.config_path =
                override_config.execution_engines.solana.config_path;
        }
        if override_config.execution_engines.solana.data_dir
            != ExecutionEnginesConfig::default().solana.data_dir
        {
            self.execution_engines.solana.data_dir =
                override_config.execution_engines.solana.data_dir;
        }
        if override_config.execution_engines.solana.enable_rpc
            != ExecutionEnginesConfig::default().solana.enable_rpc
        {
            self.execution_engines.solana.enable_rpc =
                override_config.execution_engines.solana.enable_rpc;
        }
        if override_config.execution_engines.solana.rpc_port
            != ExecutionEnginesConfig::default().solana.rpc_port
        {
            self.execution_engines.solana.rpc_port =
                override_config.execution_engines.solana.rpc_port;
        }

        // Deep merge Ethereum execution engine config
        if override_config.execution_engines.ethereum.binary_path
            != ExecutionEnginesConfig::default().ethereum.binary_path
        {
            self.execution_engines.ethereum.binary_path =
                override_config.execution_engines.ethereum.binary_path;
        }
        if override_config.execution_engines.ethereum.config_path
            != ExecutionEnginesConfig::default().ethereum.config_path
        {
            self.execution_engines.ethereum.config_path =
                override_config.execution_engines.ethereum.config_path;
        }
        if override_config.execution_engines.ethereum.data_dir
            != ExecutionEnginesConfig::default().ethereum.data_dir
        {
            self.execution_engines.ethereum.data_dir =
                override_config.execution_engines.ethereum.data_dir;
        }
        if override_config.execution_engines.ethereum.enable_rpc
            != ExecutionEnginesConfig::default().ethereum.enable_rpc
        {
            self.execution_engines.ethereum.enable_rpc =
                override_config.execution_engines.ethereum.enable_rpc;
        }
        if override_config.execution_engines.ethereum.rpc_port
            != ExecutionEnginesConfig::default().ethereum.rpc_port
        {
            self.execution_engines.ethereum.rpc_port =
                override_config.execution_engines.ethereum.rpc_port;
        }

        Ok(())
    }

    /// Validate configuration for consistency and completeness
    pub fn validate(&self) -> crate::MultivmResult<()> {
        use std::collections::HashSet;

        // Check for port conflicts
        let mut used_ports = HashSet::new();

        // Add all server ports
        if !used_ports.insert(self.server.rest.port) {
            return Err(crate::MultivmError::Configuration(format!(
                "REST API port {} is already in use",
                self.server.rest.port
            )));
        }
        if !used_ports.insert(self.server.graphql.port) {
            return Err(crate::MultivmError::Configuration(format!(
                "GraphQL port {} is already in use",
                self.server.graphql.port
            )));
        }
        if !used_ports.insert(self.server.websocket.port) {
            return Err(crate::MultivmError::Configuration(format!(
                "WebSocket port {} is already in use",
                self.server.websocket.port
            )));
        }
        if !used_ports.insert(self.server.admin.port) {
            return Err(crate::MultivmError::Configuration(format!(
                "Admin port {} is already in use",
                self.server.admin.port
            )));
        }

        // Add network port
        if self.network.enable_p2p && !used_ports.insert(self.network.listen_port) {
            return Err(crate::MultivmError::Configuration(format!(
                "P2P port {} is already in use",
                self.network.listen_port
            )));
        }

        // Add IPC port if using TCP
        if matches!(self.ipc.transport, IpcTransport::Tcp) && !used_ports.insert(self.ipc.tcp_port)
        {
            return Err(crate::MultivmError::Configuration(format!(
                "IPC TCP port {} is already in use",
                self.ipc.tcp_port
            )));
        }

        // Add monitoring ports
        if self.monitoring.enable_prometheus && !used_ports.insert(self.monitoring.prometheus_port)
        {
            return Err(crate::MultivmError::Configuration(format!(
                "Prometheus port {} is already in use",
                self.monitoring.prometheus_port
            )));
        }
        if self.monitoring.health_check.enable_endpoint
            && !used_ports.insert(self.monitoring.health_check.port)
        {
            return Err(crate::MultivmError::Configuration(format!(
                "Health check port {} is already in use",
                self.monitoring.health_check.port
            )));
        }

        // Add execution engine RPC ports
        if self.execution_engines.solana.enable_rpc
            && !used_ports.insert(self.execution_engines.solana.rpc_port)
        {
            return Err(crate::MultivmError::Configuration(format!(
                "Solana RPC port {} is already in use",
                self.execution_engines.solana.rpc_port
            )));
        }
        if self.execution_engines.ethereum.enable_rpc
            && !used_ports.insert(self.execution_engines.ethereum.rpc_port)
        {
            return Err(crate::MultivmError::Configuration(format!(
                "Ethereum RPC port {} is already in use",
                self.execution_engines.ethereum.rpc_port
            )));
        }

        // Validate all port ranges
        for &port in &used_ports {
            if port < 1024 {
                return Err(crate::MultivmError::Configuration(format!(
                    "Port {} is below minimum allowed value (1024)",
                    port
                )));
            }
        }

        // Check IPC socket path parent directory exists
        if matches!(self.ipc.transport, IpcTransport::UnixSocket) {
            let ipc_path = std::path::Path::new(&self.ipc.socket_path);
            if let Some(parent) = ipc_path.parent() {
                if !parent.exists() {
                    return Err(crate::MultivmError::Configuration(format!(
                        "IPC socket directory '{}' does not exist",
                        parent.display()
                    )));
                }
            }
        }

        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.system.log_level.as_str()) {
            return Err(crate::MultivmError::Configuration(format!(
                "Invalid log level '{}'. Must be one of: {:?}",
                self.system.log_level, valid_log_levels
            )));
        }
        if !valid_log_levels.contains(&self.logging.level.as_str()) {
            return Err(crate::MultivmError::Configuration(format!(
                "Invalid logging level '{}'. Must be one of: {:?}",
                self.logging.level, valid_log_levels
            )));
        }

        // Check data directory exists or can be created
        let data_path = std::path::Path::new(&self.system.data_dir);
        if !data_path.exists() {
            // Try to create it
            if let Err(e) = std::fs::create_dir_all(data_path) {
                return Err(crate::MultivmError::Configuration(format!(
                    "Cannot create data directory '{}': {}",
                    self.system.data_dir.display(),
                    e
                )));
            }
        }

        // Check log directory if file logging is enabled
        if self.logging.enable_file_logging {
            let log_path = std::path::Path::new(&self.logging.log_directory);
            if !log_path.exists() {
                if let Err(e) = std::fs::create_dir_all(log_path) {
                    return Err(crate::MultivmError::Configuration(format!(
                        "Cannot create log directory '{}': {}",
                        self.logging.log_directory.display(),
                        e
                    )));
                }
            }
        }

        // Validate execution engine paths
        if !self.execution_engines.solana.data_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.execution_engines.solana.data_dir) {
                return Err(crate::MultivmError::Configuration(format!(
                    "Cannot create Solana data directory '{}': {}",
                    self.execution_engines.solana.data_dir.display(),
                    e
                )));
            }
        }
        if !self.execution_engines.ethereum.data_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.execution_engines.ethereum.data_dir) {
                return Err(crate::MultivmError::Configuration(format!(
                    "Cannot create Ethereum data directory '{}': {}",
                    self.execution_engines.ethereum.data_dir.display(),
                    e
                )));
            }
        }

        // Validate network configuration
        if self.network.enable_p2p {
            if self.network.listen_host.is_empty() {
                return Err(crate::MultivmError::Configuration(
                    "P2P listen host cannot be empty".to_string(),
                ));
            }
            if self.network.max_connections == 0 {
                return Err(crate::MultivmError::Configuration(
                    "P2P max connections must be greater than 0".to_string(),
                ));
            }
            if self.network.connection_timeout_seconds == 0 {
                return Err(crate::MultivmError::Configuration(
                    "P2P connection timeout must be greater than 0".to_string(),
                ));
            }
        }

        // Validate database configuration
        if self.database.connection_url.is_empty() {
            return Err(crate::MultivmError::Configuration(
                "Database connection URL cannot be empty".to_string(),
            ));
        }
        if self.database.max_connections < self.database.min_connections {
            return Err(crate::MultivmError::Configuration(
                "Database max connections must be >= min connections".to_string(),
            ));
        }
        if self.database.connection_timeout_seconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Database connection timeout must be greater than 0".to_string(),
            ));
        }

        // Validate cache configuration
        if self.cache.default_ttl_seconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Cache default TTL must be greater than 0".to_string(),
            ));
        }
        if self.cache.max_memory_bytes == 0 {
            return Err(crate::MultivmError::Configuration(
                "Cache max memory must be greater than 0".to_string(),
            ));
        }
        if self.cache.memory.max_items == 0 {
            return Err(crate::MultivmError::Configuration(
                "Memory cache max items must be greater than 0".to_string(),
            ));
        }

        // Validate blockchain client configurations
        let clients = [
            ("Solana", &self.blockchain_clients.solana),
            ("Ethereum", &self.blockchain_clients.ethereum),
            ("MultiVM", &self.blockchain_clients.multivm),
        ];

        for (name, client) in &clients {
            if client.rpc_url.is_empty() {
                return Err(crate::MultivmError::Configuration(format!(
                    "{} RPC URL cannot be empty",
                    name
                )));
            }
            // Basic URL validation
            if !client.rpc_url.starts_with("http://")
                && !client.rpc_url.starts_with("https://")
                && !client.rpc_url.starts_with("ws://")
                && !client.rpc_url.starts_with("wss://")
            {
                return Err(crate::MultivmError::Configuration(format!(
                    "{} RPC URL must start with http://, https://, ws://, or wss://",
                    name
                )));
            }
            if client.timeout_seconds == 0 {
                return Err(crate::MultivmError::Configuration(format!(
                    "{} client timeout must be greater than 0",
                    name
                )));
            }
            if client.max_retries == 0 {
                return Err(crate::MultivmError::Configuration(format!(
                    "{} client must have at least 1 retry",
                    name
                )));
            }
        }

        // Validate consensus configuration
        if self.consensus.algorithm.is_empty() {
            return Err(crate::MultivmError::Configuration(
                "Consensus algorithm cannot be empty".to_string(),
            ));
        }
        if self.consensus.block_time_milliseconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Block time must be greater than 0".to_string(),
            ));
        }
        if self.consensus.validator_count == 0 && !self.consensus.enable_single_node {
            return Err(crate::MultivmError::Configuration(
                "Validator count must be greater than 0 when single node mode is disabled"
                    .to_string(),
            ));
        }
        if self.consensus.finality_depth == 0 {
            return Err(crate::MultivmError::Configuration(
                "Finality depth must be greater than 0".to_string(),
            ));
        }
        if self.consensus.max_block_size_bytes == 0 {
            return Err(crate::MultivmError::Configuration(
                "Max block size must be greater than 0".to_string(),
            ));
        }

        // Validate Malachite consensus timeouts
        if self.consensus.malachite.timeout_propose_milliseconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Malachite propose timeout must be greater than 0".to_string(),
            ));
        }
        if self.consensus.malachite.timeout_prevote_milliseconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Malachite prevote timeout must be greater than 0".to_string(),
            ));
        }
        if self.consensus.malachite.timeout_precommit_milliseconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Malachite precommit timeout must be greater than 0".to_string(),
            ));
        }
        if self.consensus.malachite.timeout_commit_milliseconds == 0 {
            return Err(crate::MultivmError::Configuration(
                "Malachite commit timeout must be greater than 0".to_string(),
            ));
        }

        // Validate security configuration
        if self.security.enable_rate_limiting && self.security.max_requests_per_minute == 0 {
            return Err(crate::MultivmError::Configuration(
                "Rate limiting enabled but max requests per minute is 0".to_string(),
            ));
        }
        if self.security.jwt_expiration_hours == 0 {
            return Err(crate::MultivmError::Configuration(
                "JWT expiration hours must be greater than 0".to_string(),
            ));
        }
        if self.security.api_key_length < 16 {
            return Err(crate::MultivmError::Configuration(
                "API key length must be at least 16 characters".to_string(),
            ));
        }

        // Validate environment variable names
        if self.security.secrets.jwt_secret_env.is_empty() {
            return Err(crate::MultivmError::Configuration(
                "JWT secret environment variable name cannot be empty".to_string(),
            ));
        }
        if self.security.secrets.database_password_env.is_empty() {
            return Err(crate::MultivmError::Configuration(
                "Database password environment variable name cannot be empty".to_string(),
            ));
        }
        if self.security.secrets.redis_password_env.is_empty() {
            return Err(crate::MultivmError::Configuration(
                "Redis password environment variable name cannot be empty".to_string(),
            ));
        }
        if self.security.secrets.encryption_key_env.is_empty() {
            return Err(crate::MultivmError::Configuration(
                "Encryption key environment variable name cannot be empty".to_string(),
            ));
        }

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
