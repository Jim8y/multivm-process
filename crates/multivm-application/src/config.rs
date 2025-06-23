use crate::error::{ApplicationError, ApplicationResult};
use hex;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationConfig {
    /// Server configuration
    pub server: ServerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,

    /// Monitoring and metrics configuration
    pub monitoring: MonitoringConfig,

    /// VM client configurations
    pub vm_clients: VmClientConfig,

    /// Feature flags
    pub features: FeatureConfig,

    /// Performance tuning
    pub performance: PerformanceConfig,
}

/// Server configuration for REST, GraphQL, and WebSocket
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestServerConfig {
    /// Host address to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Request timeout
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,

    /// Maximum request body size in bytes
    pub max_body_size: usize,

    /// Enable CORS
    pub enable_cors: bool,

    /// Allowed origins for CORS
    pub cors_origins: Vec<String>,

    /// Enable request logging
    pub enable_logging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLServerConfig {
    /// Host address to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Enable GraphQL Playground
    pub enable_playground: bool,

    /// Maximum query depth
    pub max_query_depth: u32,

    /// Maximum query complexity
    pub max_query_complexity: u32,

    /// Query timeout
    #[serde(with = "humantime_serde")]
    pub query_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketServerConfig {
    /// Host address to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,

    /// Heartbeat interval
    #[serde(with = "humantime_serde")]
    pub heartbeat_interval: Duration,

    /// Maximum subscriptions per connection
    pub max_subscriptions_per_connection: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminServerConfig {
    /// Host address to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Enable admin UI
    pub enable_ui: bool,

    /// Admin authentication required
    pub require_auth: bool,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Minimum number of connections in the pool
    pub min_connections: u32,

    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,

    /// Idle timeout for connections
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,

    /// Maximum lifetime for connections
    #[serde(with = "humantime_serde")]
    pub max_lifetime: Duration,

    /// Enable query logging
    pub enable_query_logging: bool,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Redis configuration
    pub redis: RedisConfig,

    /// In-memory cache configuration
    pub memory: MemoryCacheConfig,

    /// Cache strategy
    pub strategy: CacheStrategy,

    /// Default TTL for cached items
    #[serde(with = "humantime_serde")]
    pub default_ttl: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    pub url: String,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,

    /// Command timeout
    #[serde(with = "humantime_serde")]
    pub command_timeout: Duration,

    /// Key prefix for namespacing
    pub key_prefix: String,

    /// Circuit breaker failure threshold
    pub circuit_breaker_threshold: u32,

    /// Circuit breaker timeout
    #[serde(with = "humantime_serde")]
    pub circuit_breaker_timeout: Duration,

    /// Hot cache max size in bytes
    pub hot_cache_max_size: u64,

    /// Enable connection pooling
    pub enable_connection_pooling: bool,

    /// Enable pipelining for batch operations
    pub enable_pipelining: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCacheConfig {
    /// Maximum number of items to store
    pub max_items: usize,

    /// Maximum memory usage in bytes
    pub max_memory_bytes: usize,

    /// Cleanup interval
    #[serde(with = "humantime_serde")]
    pub cleanup_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    WriteThrough,
    WriteBack,
    WriteAround,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT secret key
    pub jwt_secret: String,

    /// JWT token expiration time
    #[serde(with = "humantime_serde")]
    pub jwt_expiration: Duration,

    /// Enable API key authentication
    pub enable_api_keys: bool,

    /// API key validation method
    pub api_key_validation: ApiKeyValidation,

    /// Admin API key (for bootstrap)
    pub admin_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKeyValidation {
    Database,
    Environment,
    Redis,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Enable rate limiting
    pub enabled: bool,

    /// Default requests per minute
    pub default_rpm: u32,

    /// Default requests per hour
    pub default_rph: u32,

    /// Default requests per day
    pub default_rpd: u32,

    /// Rate limit storage
    pub storage: RateLimitStorage,

    /// Burst allowance
    pub burst_allowance: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitStorage {
    Memory,
    Redis,
}

/// Monitoring and observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Metrics server configuration
    pub metrics: MetricsConfig,

    /// Health check configuration
    pub health_check: HealthCheckConfig,

    /// Tracing configuration
    pub tracing: TracingConfig,

    /// Log level
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Host for metrics server
    pub host: String,

    /// Port for metrics server
    pub port: u16,

    /// Metrics export format
    pub format: MetricsFormat,

    /// Collection interval
    #[serde(with = "humantime_serde")]
    pub collection_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricsFormat {
    Prometheus,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Host for health check endpoint
    pub host: String,

    /// Port for health check endpoint
    pub port: u16,

    /// Health check interval
    #[serde(with = "humantime_serde")]
    pub check_interval: Duration,

    /// Timeout for individual health checks
    #[serde(with = "humantime_serde")]
    pub check_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: bool,

    /// Tracing service endpoint
    pub endpoint: Option<String>,

    /// Service name for tracing
    pub service_name: String,

    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
}

/// VM client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmClientConfig {
    /// Solana client configuration
    pub solana: SolanaClientConfig,

    /// Reth client configuration
    pub reth: RethClientConfig,

    /// MultiVM client configuration
    pub multivm: MultivmClientConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaClientConfig {
    /// Solana RPC endpoint
    pub rpc_url: String,

    /// WebSocket endpoint
    pub ws_url: String,

    /// Request timeout
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// Retry configuration
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RethClientConfig {
    /// Reth RPC endpoint
    pub rpc_url: String,

    /// WebSocket endpoint
    pub ws_url: String,

    /// JWT secret for engine API
    pub jwt_secret: String,

    /// Request timeout
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// Retry configuration
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultivmClientConfig {
    /// Consensus client endpoint
    pub consensus_endpoint: String,

    /// Account mapping endpoint
    pub account_mapping_endpoint: String,

    /// Process manager endpoint
    pub process_manager_endpoint: String,

    /// Request timeout
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// Retry configuration
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,

    /// Initial retry delay
    #[serde(with = "humantime_serde")]
    pub initial_delay: Duration,

    /// Maximum retry delay
    #[serde(with = "humantime_serde")]
    pub max_delay: Duration,

    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
}

/// Feature flags configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Enable GraphQL subscriptions
    pub enable_graphql_subscriptions: bool,

    /// Enable WebSocket streaming
    pub enable_websocket_streaming: bool,

    /// Enable cross-VM operations
    pub enable_cross_vm_operations: bool,

    /// Enable admin interface
    pub enable_admin_interface: bool,

    /// Enable experimental features
    pub enable_experimental: bool,

    /// Enable request batching
    pub enable_request_batching: bool,

    /// Enable response compression
    pub enable_compression: bool,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Worker thread count
    pub worker_threads: Option<usize>,

    /// Blocking thread count
    pub blocking_threads: Option<usize>,

    /// Stack size for threads
    pub thread_stack_size: Option<usize>,

    /// Request buffer size
    pub request_buffer_size: usize,

    /// Response buffer size
    pub response_buffer_size: usize,

    /// Connection pool size
    pub connection_pool_size: usize,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            cache: CacheConfig::default(),
            auth: AuthConfig::default(),
            rate_limiting: RateLimitingConfig::default(),
            monitoring: MonitoringConfig::default(),
            vm_clients: VmClientConfig::default(),
            features: FeatureConfig::default(),
            performance: PerformanceConfig::default(),
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
            request_timeout: Duration::from_secs(30),
            max_body_size: 1024 * 1024, // 1MB
            enable_cors: false,         // Disabled by default for security
            cors_origins: vec![],       // Empty by default, configure as needed
            enable_logging: true,
        }
    }
}

impl Default for GraphQLServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8081,
            enable_playground: false, // Disabled by default for security
            max_query_depth: 15,
            max_query_complexity: 1000,
            query_timeout: Duration::from_secs(30),
        }
    }
}

impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8082,
            max_connections: 10000,
            connection_timeout: Duration::from_secs(60),
            heartbeat_interval: Duration::from_secs(30),
            max_subscriptions_per_connection: 100,
        }
    }
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(), // Only localhost by default
            port: 8083,
            enable_ui: true,
            require_auth: true,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/multivm".to_string(),
            max_connections: 100,
            min_connections: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600), // 10 minutes
            max_lifetime: Duration::from_secs(3600), // 1 hour
            enable_query_logging: false,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis: RedisConfig::default(),
            memory: MemoryCacheConfig::default(),
            strategy: CacheStrategy::WriteThrough,
            default_ttl: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 50,
            connection_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(5),
            key_prefix: "multivm:app:".to_string(),
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(60),
            hot_cache_max_size: 10 * 1024 * 1024, // 10MB
            enable_connection_pooling: true,
            enable_pipelining: true,
        }
    }
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_items: 10000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

impl AuthConfig {
    /// Load authentication configuration with secure environment overrides
    pub fn load_secure() -> ApplicationResult<Self> {
        // Start with default configuration
        let mut config = Self::default();

        // Override JWT secret from environment (critical for production)
        if let Ok(jwt_secret) = std::env::var("MULTIVM_JWT_SECRET") {
            if jwt_secret.len() < 32 {
                return Err(ApplicationError::ConfigurationError {
                    component: "auth".to_string(),
                    message: "JWT secret must be at least 32 characters long".to_string(),
                });
            }
            config.jwt_secret = jwt_secret;
        } else if config.jwt_secret == "your-jwt-secret-key-must-be-at-least-32-characters-long" {
            // Production environments require proper JWT secret configuration
            let env = std::env::var("RUST_ENV").unwrap_or_default();
            if env == "production" || env == "prod" || env == "live" {
                return Err(ApplicationError::ConfigurationError {
                    component: "auth".to_string(),
                    message: "MULTIVM_JWT_SECRET environment variable must be set in production. Generate a cryptographically secure 256-bit key.".to_string(),
                });
            }

            // Issue warning for non-production environments using default secret
            if env != "test" && env != "development" {
                tracing::warn!(
                    "Using default JWT secret in environment '{}'. This is not recommended. Set MULTIVM_JWT_SECRET environment variable.",
                    env
                );
            }
        }

        // Load JWT expiration from environment
        if let Ok(exp_str) = std::env::var("MULTIVM_JWT_EXPIRATION") {
            if let Ok(exp_seconds) = exp_str.parse::<u64>() {
                config.jwt_expiration = Duration::from_secs(exp_seconds);
            }
        }

        // Load API key configuration from environment
        if let Ok(enable_str) = std::env::var("MULTIVM_ENABLE_API_KEYS") {
            config.enable_api_keys = enable_str.to_lowercase() == "true";
        }

        // Load API key validation method from environment
        if let Ok(validation_str) = std::env::var("MULTIVM_API_KEY_VALIDATION") {
            config.api_key_validation = match validation_str.to_lowercase().as_str() {
                "database" => ApiKeyValidation::Database,
                "environment" => ApiKeyValidation::Environment,
                "redis" => ApiKeyValidation::Redis,
                _ => {
                    return Err(ApplicationError::ConfigurationError {
                        component: "auth".to_string(),
                        message: format!("Invalid API key validation method: {}. Valid options: database, environment, redis", validation_str),
                    });
                }
            };
        }

        // Load admin API key from environment
        config.admin_api_key = std::env::var("MULTIVM_ADMIN_API_KEY").ok();

        Ok(config)
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: generate_secure_jwt_secret(),
            jwt_expiration: Duration::from_secs(3600), // 1 hour
            enable_api_keys: true,
            api_key_validation: ApiKeyValidation::Database,
            admin_api_key: None,
        }
    }
}

/// Generate a cryptographically secure JWT secret
fn generate_secure_jwt_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    hex::encode(secret)
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_rpm: 1000,
            default_rph: 10000,
            default_rpd: 100000,
            storage: RateLimitStorage::Memory,
            burst_allowance: 100,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics: MetricsConfig::default(),
            health_check: HealthCheckConfig::default(),
            tracing: TracingConfig::default(),
            log_level: "info".to_string(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9090,
            format: MetricsFormat::Prometheus,
            collection_interval: Duration::from_secs(15),
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9091,
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "multivm-application".to_string(),
            sampling_rate: 0.1,
        }
    }
}

impl Default for VmClientConfig {
    fn default() -> Self {
        Self {
            solana: SolanaClientConfig::default(),
            reth: RethClientConfig::default(),
            multivm: MultivmClientConfig::default(),
        }
    }
}

impl Default for SolanaClientConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8899".to_string(),
            ws_url: "ws://localhost:8900".to_string(),
            timeout: Duration::from_secs(30),
            retry: RetryConfig::default(),
        }
    }
}

impl Default for RethClientConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            ws_url: "ws://localhost:8546".to_string(),
            jwt_secret: "your-jwt-secret-must-be-at-least-32-characters-long".to_string(),
            timeout: Duration::from_secs(30),
            retry: RetryConfig::default(),
        }
    }
}

impl Default for MultivmClientConfig {
    fn default() -> Self {
        Self {
            consensus_endpoint: "http://localhost:8100".to_string(),
            account_mapping_endpoint: "http://localhost:8101".to_string(),
            process_manager_endpoint: "http://localhost:8102".to_string(),
            timeout: Duration::from_secs(30),
            retry: RetryConfig::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
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

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            worker_threads: None,    // Use default (CPU count)
            blocking_threads: None,  // Use default
            thread_stack_size: None, // Use default
            request_buffer_size: 8192,
            response_buffer_size: 8192,
            connection_pool_size: 100,
        }
    }
}

impl ApplicationConfig {
    /// Load configuration from file
    pub fn from_file(path: &str) -> ApplicationResult<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("MULTIVM_APP").separator("__"))
            .build()
            .map_err(ApplicationError::from)?;

        settings.try_deserialize().map_err(ApplicationError::from)
    }

    /// Load configuration with environment variable overrides
    pub fn from_env() -> ApplicationResult<Self> {
        let settings = config::Config::builder()
            .add_source(config::Environment::with_prefix("MULTIVM_APP").separator("__"))
            .build()
            .map_err(ApplicationError::from)?;

        let mut config: Self = settings.try_deserialize().map_err(ApplicationError::from)?;

        // Override with secure authentication configuration
        config.auth = AuthConfig::load_secure()?;

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> ApplicationResult<()> {
        // Validate server configurations
        self.validate_server_config()?;

        // Validate database configuration
        self.validate_database_config()?;

        // Validate cache configuration
        self.validate_cache_config()?;

        // Validate authentication configuration
        self.validate_auth_config()?;

        // Validate VM client configurations
        self.validate_vm_client_config()?;

        Ok(())
    }

    fn validate_server_config(&self) -> ApplicationResult<()> {
        // Check for port conflicts
        let ports = vec![
            ("REST", self.server.rest.port),
            ("GraphQL", self.server.graphql.port),
            ("WebSocket", self.server.websocket.port),
            ("Admin", self.server.admin.port),
            ("Metrics", self.monitoring.metrics.port),
            ("Health", self.monitoring.health_check.port),
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

    fn validate_database_config(&self) -> ApplicationResult<()> {
        if self.database.max_connections <= self.database.min_connections {
            return Err(ApplicationError::ConfigurationError {
                component: "database".to_string(),
                message: "max_connections must be greater than min_connections".to_string(),
            });
        }

        Ok(())
    }

    fn validate_cache_config(&self) -> ApplicationResult<()> {
        if self.cache.memory.max_items == 0 {
            return Err(ApplicationError::ConfigurationError {
                component: "cache".to_string(),
                message: "Memory cache max_items must be greater than 0".to_string(),
            });
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

    fn validate_vm_client_config(&self) -> ApplicationResult<()> {
        // Validate URLs
        if !self.vm_clients.solana.rpc_url.starts_with("http") {
            return Err(ApplicationError::ConfigurationError {
                component: "vm_clients".to_string(),
                message: "Solana RPC URL must start with http or https".to_string(),
            });
        }

        if !self.vm_clients.reth.rpc_url.starts_with("http") {
            return Err(ApplicationError::ConfigurationError {
                component: "vm_clients".to_string(),
                message: "Reth RPC URL must start with http or https".to_string(),
            });
        }

        Ok(())
    }

    /// Get socket address for REST server
    pub fn rest_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.rest.host, self.server.rest.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "rest_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }

    /// Get socket address for GraphQL server
    pub fn graphql_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.graphql.host, self.server.graphql.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "graphql_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }

    /// Get socket address for WebSocket server
    pub fn websocket_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!(
            "{}:{}",
            self.server.websocket.host, self.server.websocket.port
        )
        .parse()
        .map_err(|e| ApplicationError::ConfigurationError {
            component: "websocket_server".to_string(),
            message: format!("Invalid socket address: {}", e),
        })
    }

    /// Get socket address for admin server
    pub fn admin_socket_addr(&self) -> ApplicationResult<SocketAddr> {
        format!("{}:{}", self.server.admin.host, self.server.admin.port)
            .parse()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "admin_server".to_string(),
                message: format!("Invalid socket address: {}", e),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ApplicationConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_port_conflict_validation() {
        let mut config = ApplicationConfig::default();
        config.server.graphql.port = config.server.rest.port; // Create conflict
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_socket_addr_parsing() {
        let config = ApplicationConfig::default();
        assert!(config.rest_socket_addr().is_ok());
        assert!(config.graphql_socket_addr().is_ok());
        assert!(config.websocket_socket_addr().is_ok());
        assert!(config.admin_socket_addr().is_ok());
    }

    #[test]
    fn test_jwt_secret_validation() {
        let mut config = ApplicationConfig::default();
        config.auth.jwt_secret = "short".to_string(); // Too short
        assert!(config.validate().is_err());
    }
}
