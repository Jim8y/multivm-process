//! Application configuration that extends the unified MultiVM configuration
//!
//! This module provides application-specific configuration while leveraging
//! the unified configuration system from multivm-common.

use crate::error::{ApplicationError, ApplicationResult};
use multivm_common::MultivmConfig;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Application configuration that extends the base MultiVM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Server configuration for all API endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// API key validation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKeyValidation {
    Database,
    Environment,
    Redis,
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

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            base: MultivmConfig::default(),
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            cache: CacheConfig::default(),
            monitoring: MonitoringConfig::default(),
            features: FeatureConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            rest: RestServerConfig::default(),
            graphql: GraphQLServerConfig::default(),
            websocket: WebSocketServerConfig::default(),
            admin: AdminServerConfig::default(),
        }
    }
}

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
    /// Load configuration from file
    pub fn from_file(path: &str) -> ApplicationResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "config".to_string(),
                message: format!("Failed to read config file: {}", e),
            })?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "config".to_string(),
                message: format!("Failed to parse config: {}", e),
            })?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> ApplicationResult<Self> {
        let mut config = Self::default();

        // Load JWT secret from environment
        if let Ok(jwt_secret) = std::env::var("MULTIVM_JWT_SECRET") {
            config.auth.jwt_secret = jwt_secret;
        }

        // Load other environment variables as needed
        if let Ok(log_level) = std::env::var("MULTIVM_LOG_LEVEL") {
            config.monitoring.log_level = log_level;
        }

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> ApplicationResult<()> {
        // Validate base configuration
        self.base.validate().map_err(|e| ApplicationError::ConfigurationError {
            component: "base_config".to_string(),
            message: e.to_string(),
        })?;

        // Validate server configuration
        self.validate_server_config()?;

        // Validate authentication configuration
        self.validate_auth_config()?;

        Ok(())
    }

    fn validate_server_config(&self) -> ApplicationResult<()> {
        // Check for port conflicts
        let ports = vec![
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

        Ok(())
    }

    /// Get socket addresses for servers
    pub fn rest_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.rest.host, self.server.rest.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "rest_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }

    pub fn graphql_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.graphql.host, self.server.graphql.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "graphql_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }

    pub fn websocket_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.websocket.host, self.server.websocket.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "websocket_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }

    pub fn admin_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.admin.host, self.server.admin.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "admin_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }

    /// Convert to base MultivmConfig for compatibility
    pub fn to_base_config(&self) -> MultivmConfig {
        self.base.clone()
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
            if std::env::var("MULTIVM_JWT_SECRET").is_err() {
                return Err(ApplicationError::ConfigurationError {
                    component: "environment".to_string(),
                    message: "MULTIVM_JWT_SECRET must be set in production".to_string(),
                });
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
