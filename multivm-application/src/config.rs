//! Application configuration that extends the unified MultiVM configuration
//!
//! This module provides application-specific configuration while leveraging
//! the unified configuration system from multivm-common.

use crate::api::rest::middleware::manager::MiddlewareConfig;
use crate::error::{ApplicationError, ApplicationResult};
use multivm_common::MultivmConfig;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Application configuration that extends the base MultiVM configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApplicationConfig {
    /// Base MultiVM configuration
    #[serde(flatten)]
    pub base: MultivmConfig,

    /// Application-specific server configuration
    pub server: ServerConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Monitoring configuration
    pub monitoring: MonitoringConfig,

    /// Feature flags
    pub features: FeatureConfig,

    /// Middleware configuration
    pub middleware: MiddlewareConfig,

    /// Execution engine configuration
    pub execution_engines: crate::execution_engines::ExecutionEngineConfig,
}

/// Server configuration for all API endpoints
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// REST API server configuration
    pub rest: RestServerConfig,

    /// GraphQL server configuration
    pub graphql: GraphQLServerConfig,

    /// WebSocket server configuration
    pub websocket: WebSocketServerConfig,

    /// Admin interface configuration
    pub admin: AdminServerConfig,
}

/// REST API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_cors: bool,
    pub cors_origins: Vec<String>,
    pub request_timeout_seconds: u64,
    pub max_body_size_bytes: usize,
    pub enable_compression: bool,
}

/// GraphQL server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_playground: bool,
    pub enable_introspection: bool,
    pub max_query_depth: u32,
    pub max_query_complexity: u32,
    pub query_timeout_seconds: u64,
}

/// WebSocket server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub connection_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub max_subscriptions_per_connection: usize,
}

/// Admin interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_ui: bool,
    pub require_auth: bool,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: u32,
    pub enable_api_keys: bool,
    pub api_key_validation: ApiKeyValidation,
    pub admin_api_key: Option<String>,
    /// JWT secret management configuration
    pub jwt_secret_management: JwtSecretManagementConfig,
}

/// API key validation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKeyValidation {
    Database,
    Environment,
    Redis,
}

/// JWT secret management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtSecretManagementConfig {
    /// Enable advanced secret management
    pub enabled: bool,
    /// Secret rotation interval in hours
    pub rotation_interval_hours: u32,
    /// How long to keep old secrets for token validation (hours)
    pub key_retention_hours: u32,
    /// Storage backend for secrets
    pub storage_backend: JwtStorageBackend,
    /// Auto-rotate secrets when needed
    pub auto_rotate: bool,
    /// Auto-cleanup expired secrets
    pub auto_cleanup: bool,
    /// File path for file storage
    pub file_storage_path: Option<String>,
}

/// JWT storage backend options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JwtStorageBackend {
    /// Use legacy single secret from config/env
    Legacy,
    /// File-based storage
    File,
    /// Environment variables
    Environment,
    /// HashiCorp Vault
    Vault,
    /// AWS Secrets Manager
    AwsSecretsManager,
    /// Azure Key Vault
    AzureKeyVault,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub strategy: CacheStrategy,
    pub default_ttl_seconds: u64,
    pub redis: RedisCacheConfig,
    pub memory: MemoryCacheConfig,
}

/// Cache strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    WriteThrough,
    WriteBack,
    WriteAround,
}

/// Redis cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCacheConfig {
    pub url: String,
    pub key_prefix: String,
    pub connection_timeout_seconds: u64,
    pub command_timeout_seconds: u64,
    pub max_connections: u32,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout: u64,
    pub hot_cache_max_size: usize,
}

/// Memory cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCacheConfig {
    pub max_items: usize,
    pub max_memory_bytes: usize,
    pub cleanup_interval_seconds: u64,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_metrics: bool,
    pub enable_health_checks: bool,
    pub metrics_port: u16,
    pub health_check_port: u16,
    pub prometheus_endpoint: Option<String>,
    pub log_level: String,
}

/// Feature flags configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub enable_graphql_subscriptions: bool,
    pub enable_websocket_streaming: bool,
    pub enable_cross_vm_operations: bool,
    pub enable_admin_interface: bool,
    pub enable_experimental: bool,
    pub enable_request_batching: bool,
    pub enable_compression: bool,
}

// Default implementations

impl Default for RestServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: false,
            cors_origins: vec![],
            request_timeout_seconds: 30,
            max_body_size_bytes: 16 * 1024 * 1024, // 16MB
            enable_compression: true,
        }
    }
}

impl Default for GraphQLServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8081,
            enable_playground: false,
            enable_introspection: false,
            max_query_depth: 15,
            max_query_complexity: 1000,
            query_timeout_seconds: 30,
        }
    }
}

impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8082,
            max_connections: 10000,
            connection_timeout_seconds: 60,
            heartbeat_interval_seconds: 30,
            max_subscriptions_per_connection: 100,
        }
    }
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8083,
            enable_ui: true,
            require_auth: true,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: generate_secure_jwt_secret(),
            jwt_expiration_hours: 24,
            enable_api_keys: true,
            api_key_validation: ApiKeyValidation::Database,
            admin_api_key: None,
            jwt_secret_management: JwtSecretManagementConfig::default(),
        }
    }
}

impl Default for JwtSecretManagementConfig {
    fn default() -> Self {
        Self {
            enabled: false,                  // Disabled by default for backward compatibility
            rotation_interval_hours: 24 * 7, // Weekly rotation
            key_retention_hours: 24 * 30,    // Keep for 30 days
            storage_backend: JwtStorageBackend::File,
            auto_rotate: true,
            auto_cleanup: true,
            file_storage_path: Some("jwt_secrets.json".to_string()),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            strategy: CacheStrategy::WriteThrough,
            default_ttl_seconds: 300,
            redis: RedisCacheConfig::default(),
            memory: MemoryCacheConfig::default(),
        }
    }
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            key_prefix: "multivm:app:".to_string(),
            connection_timeout_seconds: 5,
            command_timeout_seconds: 5,
            max_connections: 50,
            circuit_breaker_threshold: 10,
            circuit_breaker_timeout: 60,
            hot_cache_max_size: 1000,
        }
    }
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_items: 10000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            cleanup_interval_seconds: 60,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            enable_health_checks: true,
            metrics_port: 9090,
            health_check_port: 9091,
            prometheus_endpoint: None,
            log_level: "info".to_string(),
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            enable_graphql_subscriptions: true,
            enable_websocket_streaming: true,
            enable_cross_vm_operations: true,
            enable_admin_interface: true,
            enable_experimental: false,
            enable_request_batching: true,
            enable_compression: true,
        }
    }
}

impl ApplicationConfig {
    /// Create application config from unified config
    pub fn from_unified_config(unified_config: MultivmConfig) -> ApplicationResult<Self> {
        let app_config = ApplicationConfig {
            base: unified_config.clone(),
            server: ServerConfig {
                rest: RestServerConfig {
                    host: unified_config.server.rest.host,
                    port: unified_config.server.rest.port,
                    enable_cors: unified_config.server.rest.enable_cors,
                    cors_origins: vec!["*".to_string()], // Allow all for testnet
                    request_timeout_seconds: unified_config.server.rest.request_timeout_seconds,
                    max_body_size_bytes: unified_config.server.rest.max_request_size_bytes as usize,
                    enable_compression: true,
                },
                graphql: GraphQLServerConfig {
                    host: unified_config.server.graphql.host,
                    port: unified_config.server.graphql.port,
                    enable_playground: unified_config.server.graphql.enable_playground,
                    enable_introspection: true,
                    max_query_depth: unified_config.server.graphql.query_complexity_limit,
                    max_query_complexity: unified_config.server.graphql.query_complexity_limit,
                    query_timeout_seconds: unified_config.server.graphql.query_timeout_seconds,
                },
                websocket: WebSocketServerConfig {
                    host: unified_config.server.websocket.host,
                    port: unified_config.server.websocket.port,
                    max_connections: unified_config.server.websocket.max_connections as usize,
                    connection_timeout_seconds: unified_config
                        .server
                        .websocket
                        .connection_timeout_seconds,
                    heartbeat_interval_seconds: 30,
                    max_subscriptions_per_connection: 100,
                },
                admin: AdminServerConfig {
                    host: unified_config.server.admin.host,
                    port: unified_config.server.admin.port,
                    enable_ui: unified_config.server.admin.enable_ui,
                    require_auth: unified_config.server.admin.require_auth,
                },
            },
            auth: AuthConfig::default(),
            cache: CacheConfig::default(),
            monitoring: MonitoringConfig {
                enable_metrics: unified_config.monitoring.enable_prometheus,
                enable_health_checks: true,
                metrics_port: unified_config.monitoring.prometheus_port,
                health_check_port: unified_config.monitoring.health_check.port,
                prometheus_endpoint: Some(format!(
                    "{}:{}",
                    unified_config.monitoring.prometheus_host,
                    unified_config.monitoring.prometheus_port
                )),
                log_level: unified_config.logging.level,
            },
            features: FeatureConfig::default(),
            middleware: crate::api::rest::middleware::manager::MiddlewareConfig::default(),
            execution_engines: crate::execution_engines::ExecutionEngineConfig {
                ethereum: crate::execution_engines::EthereumEngineConfig {
                    enabled: true,
                    data_dir: format!("{}/ethereum", unified_config.system.data_dir.display()),
                    rpc_port: 8545,
                    chain_id: 1337, // Use dev chain ID
                    mock_mode: std::env::var("MULTIVM_ETHEREUM_MOCK_MODE")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false), // Default to real execution, use env var to override
                    auto_start: true,
                },
                solana: crate::execution_engines::SolanaEngineConfig {
                    enabled: true,
                    data_dir: format!("{}/solana", unified_config.system.data_dir.display()),
                    rpc_port: 8899,
                    cluster: "localnet".to_string(),
                    mock_mode: std::env::var("MULTIVM_SOLANA_MOCK_MODE")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(true), // Keep Solana in mock mode by default
                    auto_start: true,
                },
                global: crate::execution_engines::GlobalExecutionConfig {
                    max_concurrent_blocks: 10,
                    block_timeout_seconds: 30,
                    health_check_interval_seconds: 30,
                    enable_cross_vm_coordination: true,
                },
            },
        };
        Ok(app_config)
    }

    /// Load configuration from file
    pub fn from_file(path: &str) -> ApplicationResult<Self> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ApplicationError::ConfigurationError {
                component: "config".to_string(),
                message: format!("Failed to read config file: {e}"),
            })?;

        let config: Self =
            toml::from_str(&content).map_err(|e| ApplicationError::ConfigurationError {
                component: "config".to_string(),
                message: format!("Failed to parse config: {e}"),
            })?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> ApplicationResult<Self> {
        let mut config = Self::default();

        // Authentication configuration
        if let Ok(jwt_secret) = std::env::var("MULTIVM_JWT_SECRET") {
            config.auth.jwt_secret = jwt_secret;
        }
        if let Ok(jwt_expiration) = std::env::var("MULTIVM_JWT_EXPIRATION_HOURS") {
            if let Ok(hours) = jwt_expiration.parse::<u32>() {
                config.auth.jwt_expiration_hours = hours;
            }
        }
        if let Ok(enable_api_keys) = std::env::var("MULTIVM_ENABLE_API_KEYS") {
            config.auth.enable_api_keys = enable_api_keys.to_lowercase() == "true";
        }

        // Server configuration
        if let Ok(rest_port) = std::env::var("MULTIVM_REST_PORT") {
            if let Ok(port) = rest_port.parse::<u16>() {
                config.server.rest.port = port;
            }
        }
        if let Ok(graphql_port) = std::env::var("MULTIVM_GRAPHQL_PORT") {
            if let Ok(port) = graphql_port.parse::<u16>() {
                config.server.graphql.port = port;
            }
        }
        if let Ok(ws_port) = std::env::var("MULTIVM_WEBSOCKET_PORT") {
            if let Ok(port) = ws_port.parse::<u16>() {
                config.server.websocket.port = port;
            }
        }
        if let Ok(admin_port) = std::env::var("MULTIVM_ADMIN_PORT") {
            if let Ok(port) = admin_port.parse::<u16>() {
                config.server.admin.port = port;
            }
        }

        // Monitoring configuration
        if let Ok(log_level) = std::env::var("MULTIVM_LOG_LEVEL") {
            config.monitoring.log_level = log_level;
        }
        if let Ok(enable_metrics) = std::env::var("MULTIVM_ENABLE_METRICS") {
            config.monitoring.enable_metrics = enable_metrics.to_lowercase() == "true";
        }

        // Cache configuration
        if let Ok(redis_url) = std::env::var("MULTIVM_REDIS_URL") {
            config.cache.redis.url = redis_url;
        }
        if let Ok(cache_ttl) = std::env::var("MULTIVM_CACHE_TTL_SECONDS") {
            if let Ok(ttl) = cache_ttl.parse::<u64>() {
                config.cache.default_ttl_seconds = ttl;
            }
        }

        // Feature flags
        if let Ok(enable_experimental) = std::env::var("MULTIVM_ENABLE_EXPERIMENTAL") {
            config.features.enable_experimental = enable_experimental.to_lowercase() == "true";
        }

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> ApplicationResult<()> {
        // Validate base configuration
        self.base
            .validate()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "base_config".to_string(),
                message: e.to_string(),
            })?;

        // Validate server configuration
        self.validate_server_config()?;

        // Validate authentication configuration
        self.validate_auth_config()?;

        // Validate cache configuration
        self.validate_cache_config()?;

        // Validate monitoring configuration
        self.validate_monitoring_config()?;

        // Validate resource limits
        self.validate_resource_limits()?;

        Ok(())
    }

    fn validate_server_config(&self) -> ApplicationResult<()> {
        // Validate port ranges
        let all_ports = vec![
            ("REST", self.server.rest.port),
            ("GraphQL", self.server.graphql.port),
            ("WebSocket", self.server.websocket.port),
            ("Admin", self.server.admin.port),
            ("Metrics", self.monitoring.metrics_port),
            ("Health", self.monitoring.health_check_port),
        ];

        for (name, port) in &all_ports {
            if *port == 0 {
                return Err(ApplicationError::ConfigurationError {
                    component: "server".to_string(),
                    message: format!("{name} port {port} is invalid. Must be between 1 and 65535"),
                });
            }
        }

        // Check for port conflicts
        let ports = [
            ("REST", self.server.rest.port),
            ("GraphQL", self.server.graphql.port),
            ("WebSocket", self.server.websocket.port),
            ("Admin", self.server.admin.port),
            ("Metrics", self.monitoring.metrics_port),
            ("Health", self.monitoring.health_check_port),
        ];

        for i in 0..ports.len() {
            for j in i + 1..ports.len() {
                if ports[i].1 == ports[j].1 {
                    return Err(ApplicationError::ConfigurationError {
                        component: "server".to_string(),
                        message: format!(
                            "Port conflict: {} and {} both use port {}",
                            ports[i].0, ports[j].0, ports[i].1
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    fn validate_auth_config(&self) -> ApplicationResult<()> {
        if self.auth.jwt_secret.len() < 32 {
            return Err(ApplicationError::ConfigurationError {
                component: "auth".to_string(),
                message: "JWT secret must be at least 32 characters long".to_string(),
            });
        }

        // Validate JWT expiration
        if self.auth.jwt_expiration_hours == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "auth".to_string(),
                message: "JWT expiration hours must be greater than 0".to_string(),
            });
        }

        if self.auth.jwt_expiration_hours > 720 {
            // 30 days
            return Err(ApplicationError::ConfigurationError {
                component: "auth".to_string(),
                message: "JWT expiration hours should not exceed 720 (30 days)".to_string(),
            });
        }

        Ok(())
    }

    fn validate_cache_config(&self) -> ApplicationResult<()> {
        // Validate cache TTL
        if self.cache.default_ttl_seconds == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "cache".to_string(),
                message: "Default cache TTL must be greater than 0".to_string(),
            });
        }

        // Validate Redis configuration if using Redis
        if !self.cache.redis.url.is_empty() {
            // Basic URL validation
            if !self.cache.redis.url.starts_with("redis://")
                && !self.cache.redis.url.starts_with("rediss://")
            {
                return Err(ApplicationError::ConfigurationError {
                    component: "cache".to_string(),
                    message: "Redis URL must start with redis:// or rediss://".to_string(),
                });
            }
        }

        // Validate memory cache limits
        if self.cache.memory.max_items == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "cache".to_string(),
                message: "Memory cache max_items must be greater than 0".to_string(),
            });
        }

        if self.cache.memory.max_memory_bytes == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "cache".to_string(),
                message: "Memory cache max_memory_bytes must be greater than 0".to_string(),
            });
        }

        Ok(())
    }

    fn validate_monitoring_config(&self) -> ApplicationResult<()> {
        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.monitoring.log_level.to_lowercase().as_str()) {
            return Err(ApplicationError::ConfigurationError {
                component: "monitoring".to_string(),
                message: format!(
                    "Invalid log level '{}'. Must be one of: trace, debug, info, warn, error",
                    self.monitoring.log_level
                ),
            });
        }

        Ok(())
    }

    fn validate_resource_limits(&self) -> ApplicationResult<()> {
        // Validate request timeout
        if self.server.rest.request_timeout_seconds == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "server".to_string(),
                message: "Request timeout must be greater than 0".to_string(),
            });
        }

        if self.server.rest.request_timeout_seconds > 300 {
            // 5 minutes
            return Err(ApplicationError::ConfigurationError {
                component: "server".to_string(),
                message: "Request timeout should not exceed 300 seconds (5 minutes)".to_string(),
            });
        }

        // Validate WebSocket limits
        if self.server.websocket.max_connections == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "websocket".to_string(),
                message: "Max WebSocket connections must be greater than 0".to_string(),
            });
        }

        if self.server.websocket.max_subscriptions_per_connection == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "websocket".to_string(),
                message: "Max subscriptions per connection must be greater than 0".to_string(),
            });
        }

        // Validate GraphQL limits
        if self.server.graphql.max_query_depth == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "graphql".to_string(),
                message: "Max query depth must be greater than 0".to_string(),
            });
        }

        if self.server.graphql.max_query_complexity == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "graphql".to_string(),
                message: "Max query complexity must be greater than 0".to_string(),
            });
        }

        Ok(())
    }

    /// Get socket addresses for servers
    pub fn rest_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.rest.host, self.server.rest.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "rest_server".to_string(),
                message: format!("Invalid socket address: {e}"),
            })
    }

    pub fn graphql_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.graphql.host, self.server.graphql.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "graphql_server".to_string(),
                message: format!("Invalid socket address: {e}"),
            })
    }

    pub fn websocket_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!(
            "{}:{}",
            self.server.websocket.host, self.server.websocket.port
        )
        .parse()
        .map_err(|e| ApplicationError::ConfigurationError {
            component: "websocket_server".to_string(),
            message: format!("Invalid socket address: {e}"),
        })
    }

    pub fn admin_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.admin.host, self.server.admin.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "admin_server".to_string(),
                message: format!("Invalid socket address: {e}"),
            })
    }

    /// Convert to base MultivmConfig for compatibility
    pub fn to_base_config(&self) -> MultivmConfig {
        self.base.clone()
    }

    /// Create JWT authentication with configured secret management
    pub fn create_jwt_auth(&self) -> crate::error::ApplicationResult<crate::auth::JwtAuth> {
        use crate::auth::secret_manager::{StorageBackend, StorageConfig};
        use crate::auth::{JwtAuth, SecretManagerConfig};
        use std::time::Duration;

        let expiration = Duration::from_secs(self.auth.jwt_expiration_hours as u64 * 3600);

        if !self.auth.jwt_secret_management.enabled {
            // Use legacy simple mode
            return JwtAuth::simple(&self.auth.jwt_secret, expiration);
        }

        // Create secret manager config
        let storage_backend = match self.auth.jwt_secret_management.storage_backend {
            JwtStorageBackend::Legacy => StorageBackend::Environment,
            JwtStorageBackend::File => StorageBackend::File,
            JwtStorageBackend::Environment => StorageBackend::Environment,
            JwtStorageBackend::Vault => StorageBackend::Vault,
            JwtStorageBackend::AwsSecretsManager => StorageBackend::AwsSecretsManager,
            JwtStorageBackend::AzureKeyVault => StorageBackend::AzureKeyVault,
        };

        let storage_config = StorageConfig {
            file_path: self.auth.jwt_secret_management.file_storage_path.clone(),
            vault_config: None, // Would be configured separately
            aws_config: None,   // Would be configured separately
            azure_config: None, // Would be configured separately
        };

        let secret_manager_config = SecretManagerConfig {
            rotation_interval_hours: self.auth.jwt_secret_management.rotation_interval_hours,
            key_retention_hours: self.auth.jwt_secret_management.key_retention_hours,
            min_secret_length: 64,
            storage_backend,
            storage_config,
        };

        JwtAuth::new(secret_manager_config, expiration)
    }

    /// Create middleware manager with configured settings
    pub fn create_middleware_manager(
        &self,
    ) -> ApplicationResult<crate::api::rest::middleware::MiddlewareManager> {
        use crate::api::rest::middleware::MiddlewareManager;
        MiddlewareManager::new(self.middleware.clone())
    }
}

/// Generate a cryptographically secure JWT secret
fn generate_secure_jwt_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    hex::encode(secret)
}

/// Utility functions for configuration management
pub mod utils {
    use super::*;

    /// Create a development configuration
    pub fn create_dev_config() -> ApplicationConfig {
        let mut config = ApplicationConfig::default();

        // Enable development features
        config.features.enable_experimental = true;
        config.server.graphql.enable_playground = true;
        config.server.graphql.enable_introspection = true;
        config.monitoring.log_level = "debug".to_string();

        config
    }

    /// Create a production configuration
    pub fn create_prod_config() -> ApplicationConfig {
        let mut config = ApplicationConfig::default();

        // Disable development features
        config.features.enable_experimental = false;
        config.server.graphql.enable_playground = false;
        config.server.graphql.enable_introspection = false;
        config.monitoring.log_level = "info".to_string();

        // Tighten security
        config.server.rest.enable_cors = false;
        config.server.admin.require_auth = true;

        config
    }

    /// Validate environment variables
    pub fn validate_env_vars() -> ApplicationResult<()> {
        // Check for required environment variables in production
        let env = std::env::var("RUST_ENV").unwrap_or_default();
        if env == "production" || env == "prod" {
            // Critical security requirements for production
            if std::env::var("MULTIVM_JWT_SECRET").is_err() {
                return Err(ApplicationError::ConfigurationError {
                    component: "environment".to_string(),
                    message: "MULTIVM_JWT_SECRET must be set in production".to_string(),
                });
            }

            // Check JWT secret strength
            if let Ok(jwt_secret) = std::env::var("MULTIVM_JWT_SECRET") {
                if jwt_secret.len() < 32 {
                    return Err(ApplicationError::ConfigurationError {
                        component: "environment".to_string(),
                        message: "MULTIVM_JWT_SECRET must be at least 32 characters in production"
                            .to_string(),
                    });
                }
            }

            // Warn about insecure defaults
            if std::env::var("MULTIVM_ADMIN_PORT").is_err() {
                tracing::warn!("MULTIVM_ADMIN_PORT not set - using default. Consider setting explicitly for production.");
            }

            if std::env::var("MULTIVM_REDIS_URL").is_err() {
                tracing::warn!("MULTIVM_REDIS_URL not set - using default localhost. Set for production deployment.");
            }
        }

        // Type aliases for compatibility with existing code
        #[allow(dead_code)]
        pub type RedisConfig = RedisCacheConfig;
        #[allow(dead_code)]
        pub type HealthCheckConfig = MonitoringConfig;
        #[allow(dead_code)]
        pub type MetricsConfig = MonitoringConfig;
        #[allow(dead_code)]
        pub type TracingConfig = MonitoringConfig;

        /// Rate limiting configuration
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct RateLimitingConfig {
            pub requests_per_minute: u32,
            pub burst_size: u32,
            pub enable_rate_limiting: bool,
            pub enabled: bool,
            pub default_rpm: u32,
        }

        impl Default for RateLimitingConfig {
            fn default() -> Self {
                Self {
                    requests_per_minute: 1000,
                    burst_size: 100,
                    enable_rate_limiting: true,
                    enabled: true,
                    default_rpm: 1000,
                }
            }
        }

        Ok(())
    }
}
