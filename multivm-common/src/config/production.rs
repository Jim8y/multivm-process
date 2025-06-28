//! Production Configuration
//!
//! This module contains production-specific configurations that are optimized
//! for performance, reliability, and security in production environments.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Production configuration for the entire multivm system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionConfig {
    /// VM configurations
    pub vms: ProductionVmConfigs,
    /// Network configuration
    pub network: ProductionNetworkConfig,
    /// Security configuration
    pub security: ProductionSecurityConfig,
    /// Performance configuration
    pub performance: ProductionPerformanceConfig,
    /// Monitoring configuration
    pub monitoring: ProductionMonitoringConfig,
}

/// Production VM configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionVmConfigs {
    /// EVM configuration
    pub evm: ProductionEvmConfig,
    /// SVM configuration
    pub svm: ProductionSvmConfig,
}

/// Production EVM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionEvmConfig {
    /// Primary RPC endpoint
    pub rpc_url: String,
    /// Backup RPC endpoints
    pub backup_rpc_urls: Vec<String>,
    /// Engine API endpoint
    pub engine_url: Option<String>,
    /// JWT secret for Engine API
    pub jwt_secret: Option<String>,
    /// Chain ID
    pub chain_id: u64,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum retries
    pub max_retries: u32,
    /// Connection pool size
    pub connection_pool_size: usize,
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Requests per second limit
    pub rate_limit_rps: u32,
}

/// Production SVM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionSvmConfig {
    /// Primary RPC endpoint
    pub rpc_url: String,
    /// WebSocket endpoint
    pub ws_url: Option<String>,
    /// Backup RPC endpoints
    pub backup_rpc_urls: Vec<String>,
    /// Commitment level
    pub commitment: CommitmentLevel,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum retries
    pub max_retries: u32,
    /// Enable preflight checks
    pub preflight_checks: bool,
    /// Connection pool size
    pub connection_pool_size: usize,
}

/// Commitment levels for SVM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommitmentLevel {
    Processed,
    Confirmed,
    Finalized,
}

/// Production network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionNetworkConfig {
    /// P2P configuration
    pub p2p: ProductionP2pConfig,
    /// IPC configuration
    pub ipc: ProductionIpcConfig,
    /// Load balancing configuration
    pub load_balancing: LoadBalancingConfig,
}

/// Production P2P configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionP2pConfig {
    /// Listen address
    pub listen_addr: String,
    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
    /// Maximum peers
    pub max_peers: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Enable discovery
    pub enable_discovery: bool,
}

/// Production IPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionIpcConfig {
    /// IPC socket path
    pub socket_path: String,
    /// Maximum connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Buffer size
    pub buffer_size: usize,
}

/// Load balancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    /// Load balancing strategy
    pub strategy: LoadBalancingStrategy,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Failure threshold
    pub failure_threshold: u32,
    /// Recovery timeout
    pub recovery_timeout: Duration,
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    HealthBased,
}

/// Production security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionSecurityConfig {
    /// Enable TLS
    pub enable_tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS key path
    pub tls_key_path: Option<String>,
    /// API key configuration
    pub api_keys: ApiKeyConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,
    /// CORS configuration
    pub cors: CorsConfig,
}

/// API key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Enable API key authentication
    pub enabled: bool,
    /// Valid API keys
    pub keys: Vec<String>,
    /// Key rotation interval
    pub rotation_interval: Duration,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Requests per second per IP
    pub requests_per_second: u32,
    /// Burst capacity
    pub burst_capacity: u32,
    /// Whitelist IPs
    pub whitelist: Vec<String>,
}

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Enable CORS
    pub enabled: bool,
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
}

/// Production performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionPerformanceConfig {
    /// Cache configuration
    pub cache: ProductionCacheConfig,
    /// Thread pool configuration
    pub thread_pool: ThreadPoolConfig,
    /// Memory configuration
    pub memory: MemoryConfig,
}

/// Production cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionCacheConfig {
    /// Redis configuration
    pub redis: ProductionRedisConfig,
    /// Memory cache configuration
    pub memory: ProductionMemoryCacheConfig,
    /// Cache strategy
    pub strategy: CacheStrategy,
}

/// Production Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionRedisConfig {
    /// Redis URL
    pub url: String,
    /// Maximum connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Command timeout
    pub command_timeout: Duration,
    /// Key prefix
    pub key_prefix: String,
    /// Enable clustering
    pub enable_clustering: bool,
    /// Cluster nodes
    pub cluster_nodes: Vec<String>,
}

/// Production memory cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMemoryCacheConfig {
    /// Maximum items
    pub max_items: usize,
    /// Maximum memory in bytes
    pub max_memory_bytes: usize,
    /// Cleanup interval
    pub cleanup_interval: Duration,
    /// Enable LRU eviction
    pub enable_lru: bool,
}

/// Cache strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    WriteThrough,
    WriteBack,
    WriteAround,
    Hybrid,
}

/// Thread pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadPoolConfig {
    /// Core threads
    pub core_threads: usize,
    /// Maximum threads
    pub max_threads: usize,
    /// Keep alive time
    pub keep_alive: Duration,
    /// Queue size
    pub queue_size: usize,
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum heap size in bytes
    pub max_heap_size: usize,
    /// GC threshold
    pub gc_threshold: f64,
    /// Enable memory profiling
    pub enable_profiling: bool,
}

/// Production monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMonitoringConfig {
    /// Metrics configuration
    pub metrics: MetricsConfig,
    /// Logging configuration
    pub logging: ProductionLoggingConfig,
    /// Tracing configuration
    pub tracing: TracingConfig,
    /// Health check configuration
    pub health_checks: HealthCheckConfig,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Metrics endpoint
    pub endpoint: String,
    /// Collection interval
    pub collection_interval: Duration,
    /// Retention period
    pub retention_period: Duration,
    /// Enable Prometheus export
    pub enable_prometheus: bool,
}

/// Production logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionLoggingConfig {
    /// Log level
    pub level: LogLevel,
    /// Log format
    pub format: LogFormat,
    /// Log output
    pub output: LogOutput,
    /// Enable structured logging
    pub structured: bool,
    /// Log rotation configuration
    pub rotation: LogRotationConfig,
}

/// Log levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Log formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Json,
    Plain,
    Compact,
}

/// Log output destinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogOutput {
    Stdout,
    File(String),
    Syslog,
    Multiple(Vec<LogOutput>),
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Enable rotation
    pub enabled: bool,
    /// Maximum file size
    pub max_size: usize,
    /// Maximum number of files
    pub max_files: usize,
    /// Rotation interval
    pub rotation_interval: Duration,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,
    /// Tracing endpoint
    pub endpoint: String,
    /// Sample rate
    pub sample_rate: f64,
    /// Enable jaeger export
    pub enable_jaeger: bool,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Enable health checks
    pub enabled: bool,
    /// Health check interval
    pub interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Health check endpoint
    pub endpoint: String,
}

impl Default for ProductionConfig {
    fn default() -> Self {
        Self {
            vms: ProductionVmConfigs::default(),
            network: ProductionNetworkConfig::default(),
            security: ProductionSecurityConfig::default(),
            performance: ProductionPerformanceConfig::default(),
            monitoring: ProductionMonitoringConfig::default(),
        }
    }
}

impl Default for ProductionVmConfigs {
    fn default() -> Self {
        Self {
            evm: ProductionEvmConfig::default(),
            svm: ProductionSvmConfig::default(),
        }
    }
}

impl Default for ProductionEvmConfig {
    fn default() -> Self {
        Self {
            rpc_url: std::env::var("ETHEREUM_RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string()),
            backup_rpc_urls: vec![],
            engine_url: None,
            jwt_secret: None,
            chain_id: 31337,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            connection_pool_size: 10,
            enable_rate_limiting: true,
            rate_limit_rps: 100,
        }
    }
}

impl Default for ProductionSvmConfig {
    fn default() -> Self {
        Self {
            rpc_url: std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string()),
            ws_url: std::env::var("SOLANA_WS_URL").ok().or_else(|| Some("ws://localhost:8900".to_string())),
            backup_rpc_urls: vec![],
            commitment: CommitmentLevel::Confirmed,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            preflight_checks: true,
            connection_pool_size: 10,
        }
    }
}

// Implement Default for all other structs...
impl Default for ProductionNetworkConfig {
    fn default() -> Self {
        Self {
            p2p: ProductionP2pConfig::default(),
            ipc: ProductionIpcConfig::default(),
            load_balancing: LoadBalancingConfig::default(),
        }
    }
}

impl Default for ProductionP2pConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:30303".to_string(),
            bootstrap_nodes: vec![],
            max_peers: 50,
            connection_timeout: Duration::from_secs(10),
            enable_discovery: true,
        }
    }
}

impl Default for ProductionIpcConfig {
    fn default() -> Self {
        Self {
            socket_path: "/tmp/multivm.sock".to_string(),
            max_connections: 100,
            connection_timeout: Duration::from_secs(5),
            buffer_size: 8192,
        }
    }
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::HealthBased,
            health_check_interval: Duration::from_secs(30),
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(60),
        }
    }
}

impl Default for ProductionSecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: true,
            tls_cert_path: None,
            tls_key_path: None,
            api_keys: ApiKeyConfig::default(),
            rate_limiting: RateLimitingConfig::default(),
            cors: CorsConfig::default(),
        }
    }
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keys: vec![],
            rotation_interval: Duration::from_secs(86400), // 24 hours
        }
    }
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 100,
            burst_capacity: 10,
            whitelist: vec![],
        }
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
            allowed_headers: vec!["Content-Type".to_string()],
        }
    }
}

impl Default for ProductionPerformanceConfig {
    fn default() -> Self {
        Self {
            cache: ProductionCacheConfig::default(),
            thread_pool: ThreadPoolConfig::default(),
            memory: MemoryConfig::default(),
        }
    }
}

impl Default for ProductionCacheConfig {
    fn default() -> Self {
        Self {
            redis: ProductionRedisConfig::default(),
            memory: ProductionMemoryCacheConfig::default(),
            strategy: CacheStrategy::WriteThrough,
        }
    }
}

impl Default for ProductionRedisConfig {
    fn default() -> Self {
        Self {
            url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            max_connections: 10,
            connection_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(5),
            key_prefix: "multivm:".to_string(),
            enable_clustering: false,
            cluster_nodes: vec![],
        }
    }
}

impl Default for ProductionMemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_items: 10000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            cleanup_interval: Duration::from_secs(300),
            enable_lru: true,
        }
    }
}

impl Default for ThreadPoolConfig {
    fn default() -> Self {
        Self {
            core_threads: num_cpus::get(),
            max_threads: num_cpus::get() * 2,
            keep_alive: Duration::from_secs(60),
            queue_size: 1000,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_heap_size: 1024 * 1024 * 1024, // 1GB
            gc_threshold: 0.8,
            enable_profiling: false,
        }
    }
}

impl Default for ProductionMonitoringConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            logging: ProductionLoggingConfig::default(),
            tracing: TracingConfig::default(),
            health_checks: HealthCheckConfig::default(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: std::env::var("METRICS_ENDPOINT").unwrap_or_else(|_| "0.0.0.0:9090".to_string()),
            collection_interval: Duration::from_secs(60),
            retention_period: Duration::from_secs(86400 * 7), // 7 days
            enable_prometheus: true,
        }
    }
}

impl Default for ProductionLoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Json,
            output: LogOutput::Stdout,
            structured: true,
            rotation: LogRotationConfig::default(),
        }
    }
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: 100 * 1024 * 1024, // 100MB
            max_files: 10,
            rotation_interval: Duration::from_secs(86400), // 24 hours
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: std::env::var("JAEGER_ENDPOINT").unwrap_or_else(|_| "http://localhost:14268/api/traces".to_string()),
            sample_rate: 0.1,
            enable_jaeger: true,
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            endpoint: "0.0.0.0:8080/health".to_string(),
        }
    }
}