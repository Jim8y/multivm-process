//! Configuration for the MultiVM RPC service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

/// Main configuration for the RPC service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcServiceConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Backend node configurations
    pub backends: BackendConfig,
    /// Caching configuration
    pub cache: CacheConfig,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Observability configuration
    pub observability: crate::observability::ObservabilityConfig,
    /// Account mapping configuration
    pub account_mapping: AccountMappingConfig,
    /// Logging configuration
    pub logging: crate::logging::LoggingConfig,
    /// Health check configuration
    pub health: crate::health::HealthConfig,
    /// Performance optimization configuration
    pub performance: crate::performance::PerformanceConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_address: SocketAddr,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum concurrent connections
    pub max_connections: u32,
    /// Enable CORS
    pub cors_enabled: bool,
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
    /// Enable compression
    pub compression_enabled: bool,
    /// Enable WebSocket support
    pub websocket_enabled: bool,
}

/// Backend node configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Ethereum (Reth) nodes
    pub ethereum: Vec<NodeConfig>,
    /// Solana nodes
    pub solana: Vec<NodeConfig>,
    /// Connection pool settings
    pub connection_pool: ConnectionPoolConfig,
}

/// Individual node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node name/identifier
    pub name: String,
    /// RPC endpoint URL
    pub url: Url,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum concurrent requests
    pub max_concurrent_requests: u32,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Priority (higher = preferred)
    pub priority: u8,
    /// Authentication credentials
    pub auth: Option<NodeAuth>,
}

/// Node authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAuth {
    /// Bearer token
    pub bearer_token: Option<String>,
    /// Basic auth username
    pub username: Option<String>,
    /// Basic auth password
    pub password: Option<String>,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// Maximum connections per node
    pub max_connections_per_node: u32,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Connection keep-alive
    pub keep_alive: Duration,
}

/// Caching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    /// Cache TTL by method
    pub method_ttl: HashMap<String, Duration>,
    /// Maximum cache size (entries)
    pub max_size: u64,
    /// Enable persistent cache
    pub persistent: bool,
    /// Cache directory (for persistent cache)
    pub cache_dir: Option<PathBuf>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Requests per second per client
    pub requests_per_second: u32,
    /// Burst size
    pub burst_size: u32,
    /// Rate limit by IP address
    pub by_ip: bool,
    /// Rate limit by API key
    pub by_api_key: bool,
    /// Whitelist of unlimited clients
    pub whitelist: Vec<String>,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,
    /// Valid API keys
    pub api_keys: HashMap<String, ApiKeyConfig>,
    /// JWT secret (for future use)
    pub jwt_secret: Option<String>,
}

/// API key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Key name/description
    pub name: String,
    /// Allowed methods (empty = all)
    pub allowed_methods: Vec<String>,
    /// Rate limit override
    pub rate_limit: Option<u32>,
    /// Enabled status
    pub enabled: bool,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Metrics server bind address
    pub bind_address: Option<SocketAddr>,
    /// Export interval
    pub export_interval: Duration,
    /// Enable detailed metrics
    pub detailed: bool,
}

/// Account mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMappingConfig {
    /// Enable account mapping integration
    pub enabled: bool,
    /// Account mapping service URL
    pub service_url: Option<Url>,
    /// Cache account mappings
    pub cache_mappings: bool,
    /// Mapping cache TTL
    pub cache_ttl: Duration,
}

impl Default for RpcServiceConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            backends: BackendConfig::default(),
            cache: CacheConfig::default(),
            rate_limit: RateLimitConfig::default(),
            auth: AuthConfig::default(),
            observability: crate::observability::ObservabilityConfig::default(),
            account_mapping: AccountMappingConfig::default(),
            logging: crate::logging::LoggingConfig::default(),
            health: crate::health::HealthConfig::default(),
            performance: crate::performance::PerformanceConfig::default(),
        }
    }
}

impl RpcServiceConfig {
    /// Validate configuration for production use
    pub fn validate_production(&self) -> Result<(), String> {
        // Run basic validation first
        self.validate()?;

        // Production-specific validations
        if !self.auth.enabled {
            return Err("Authentication must be enabled in production".to_string());
        }
        
        if !self.rate_limit.enabled {
            return Err("Rate limiting must be enabled in production".to_string());
        }

        if !self.observability.enabled {
            return Err("Observability must be enabled in production".to_string());
        }

        if !self.cache.enabled {
            return Err("Caching should be enabled in production for better performance".to_string());
        }

        // Validate logging configuration for production
        if matches!(self.logging.target, crate::logging::LogTarget::Console) {
            return Err("Production should use file or both logging targets, not console only".to_string());
        }

        if !self.logging.json_format {
            return Err("Production should use JSON log format for better parsing".to_string());
        }

        // Validate health check configuration
        if !self.health.enable_system_metrics {
            return Err("System metrics should be enabled in production".to_string());
        }

        if !self.health.enable_network_checks {
            return Err("Network checks should be enabled in production".to_string());
        }

        // Validate API keys
        if self.auth.api_keys.is_empty() {
            return Err("At least one API key must be configured in production".to_string());
        }

        // Validate backend configurations
        if self.backends.ethereum.is_empty() && self.backends.solana.is_empty() {
            return Err("At least one backend (Ethereum or Solana) must be configured".to_string());
        }

        for eth_node in &self.backends.ethereum {
            if eth_node.timeout.as_secs() > 30 {
                return Err("Ethereum node timeouts should not exceed 30 seconds in production".to_string());
            }
            if eth_node.max_concurrent_requests < 10 {
                return Err("Ethereum nodes should support at least 10 concurrent requests in production".to_string());
            }
        }

        for sol_node in &self.backends.solana {
            if sol_node.timeout.as_secs() > 30 {
                return Err("Solana node timeouts should not exceed 30 seconds in production".to_string());
            }
            if sol_node.max_concurrent_requests < 10 {
                return Err("Solana nodes should support at least 10 concurrent requests in production".to_string());
            }
        }

        Ok(())
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8545".parse().unwrap(),
            request_timeout: Duration::from_secs(30),
            max_connections: 1000,
            cors_enabled: true,
            cors_origins: vec!["*".to_string()],
            compression_enabled: true,
            websocket_enabled: true,
        }
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            ethereum: vec![NodeConfig {
                name: "local-reth".to_string(),
                url: "http://localhost:8551".parse().unwrap(),
                timeout: Duration::from_secs(10),
                max_concurrent_requests: 100,
                health_check_interval: Duration::from_secs(30),
                priority: 100,
                auth: None,
            }],
            solana: vec![NodeConfig {
                name: "local-solana".to_string(),
                url: "http://localhost:8899".parse().unwrap(),
                timeout: Duration::from_secs(10),
                max_concurrent_requests: 100,
                health_check_interval: Duration::from_secs(30),
                priority: 100,
                auth: None,
            }],
            connection_pool: ConnectionPoolConfig::default(),
        }
    }
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_node: 50,
            idle_timeout: Duration::from_secs(300),
            keep_alive: Duration::from_secs(60),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        let mut method_ttl = HashMap::new();
        
        // Ethereum method TTLs
        method_ttl.insert("eth_blockNumber".to_string(), Duration::from_secs(1));
        method_ttl.insert("eth_getBlockByNumber".to_string(), Duration::from_secs(60));
        method_ttl.insert("eth_getBlockByHash".to_string(), Duration::from_secs(300));
        method_ttl.insert("eth_getTransactionByHash".to_string(), Duration::from_secs(300));
        method_ttl.insert("eth_getBalance".to_string(), Duration::from_secs(10));
        method_ttl.insert("eth_call".to_string(), Duration::from_secs(5));
        
        // Solana method TTLs
        method_ttl.insert("getSlot".to_string(), Duration::from_secs(1));
        method_ttl.insert("getBlock".to_string(), Duration::from_secs(60));
        method_ttl.insert("getTransaction".to_string(), Duration::from_secs(300));
        method_ttl.insert("getBalance".to_string(), Duration::from_secs(10));
        method_ttl.insert("getAccountInfo".to_string(), Duration::from_secs(10));

        Self {
            enabled: true,
            method_ttl,
            max_size: 10000,
            persistent: false,
            cache_dir: None,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 100,
            burst_size: 200,
            by_ip: true,
            by_api_key: true,
            whitelist: vec![],
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_keys: HashMap::new(),
            jwt_secret: None,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: Some("127.0.0.1:9090".parse().unwrap()),
            export_interval: Duration::from_secs(10),
            detailed: false,
        }
    }
}

impl Default for AccountMappingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_url: None,
            cache_mappings: true,
            cache_ttl: Duration::from_secs(300),
        }
    }
}

impl RpcServiceConfig {
    /// Load configuration from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate backend configurations
        if self.backends.ethereum.is_empty() && self.backends.solana.is_empty() {
            return Err("At least one backend node must be configured".to_string());
        }

        // Validate timeouts
        if self.server.request_timeout.as_secs() == 0 {
            return Err("Server request timeout must be greater than 0".to_string());
        }

        // Validate rate limits
        if self.rate_limit.enabled && self.rate_limit.requests_per_second == 0 {
            return Err("Rate limit requests per second must be greater than 0".to_string());
        }

        // Validate cache settings
        if self.cache.enabled && self.cache.max_size == 0 {
            return Err("Cache max size must be greater than 0".to_string());
        }

        // Validate health check settings
        if self.health.check_interval.as_secs() == 0 {
            return Err("Health check interval must be greater than 0".to_string());
        }
        if self.health.check_timeout.as_secs() == 0 {
            return Err("Health check timeout must be greater than 0".to_string());
        }
        if self.health.memory_warning_threshold <= 0.0 || self.health.memory_warning_threshold > 100.0 {
            return Err("Memory warning threshold must be between 0 and 100".to_string());
        }
        if self.health.memory_critical_threshold <= 0.0 || self.health.memory_critical_threshold > 100.0 {
            return Err("Memory critical threshold must be between 0 and 100".to_string());
        }
        if self.health.memory_warning_threshold >= self.health.memory_critical_threshold {
            return Err("Memory warning threshold must be less than critical threshold".to_string());
        }

        Ok(())
    }

    /// Create a development configuration
    pub fn dev_config() -> Self {
        let mut config = Self::default();
        config.auth.enabled = false;
        config.rate_limit.enabled = false;
        config.observability.enabled = false;
        config.cache.enabled = false;
        config
    }

    /// Create a production configuration
    pub fn prod_config() -> Self {
        let mut config = Self::default();
        config.auth.enabled = true;
        config.rate_limit.enabled = true;
        config.observability = crate::observability::ObservabilityConfig::production();
        config.cache.enabled = true;
        config.cache.persistent = true;
        config.logging = crate::logging::LoggingConfig::production();
        config.health = crate::health::HealthConfig::production();
        config.performance = crate::performance::PerformanceConfig::production();
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = RpcServiceConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_dev_config() {
        let config = RpcServiceConfig::dev_config();
        assert!(!config.auth.enabled);
        assert!(!config.rate_limit.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_prod_config() {
        let config = RpcServiceConfig::prod_config();
        assert!(config.auth.enabled);
        assert!(config.rate_limit.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = RpcServiceConfig::default();
        
        // Test empty backends
        config.backends.ethereum.clear();
        config.backends.solana.clear();
        assert!(config.validate().is_err());
        
        // Test zero timeout
        config = RpcServiceConfig::default();
        config.server.request_timeout = Duration::from_secs(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_production_validation() {
        // Default config should fail production validation
        let config = RpcServiceConfig::default();
        assert!(config.validate_production().is_err());

        // Production config should pass
        let prod_config = RpcServiceConfig::prod_config();
        // Add a dummy API key for the test
        let mut prod_config = prod_config;
        prod_config.auth.api_keys.insert(
            "test_key".to_string(),
            crate::config::ApiKeyConfig {
                name: "test".to_string(),
                enabled: true,
                allowed_methods: vec![],
                rate_limit: None,
            }
        );
        assert!(prod_config.validate_production().is_ok());

        // Test invalid health thresholds
        let mut bad_config = RpcServiceConfig::prod_config();
        bad_config.health.memory_warning_threshold = 95.0;
        bad_config.health.memory_critical_threshold = 90.0; // Warning > Critical
        assert!(bad_config.validate_production().is_err());
    }
}