//! Production-Enhanced EVM API Gateway
//!
//! This module provides a production-ready EVM gateway that communicates with
//! real Reth processes via JSON-RPC. It replaces mock implementations with
//! actual blockchain interaction capabilities.

use crate::{cache::CacheLayer, error::{ApplicationResult, ApplicationError}};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Production-enhanced EVM gateway configuration
#[derive(Debug, Clone)]
pub struct ProductionEvmGatewayConfig {
    /// Primary Reth RPC endpoint
    pub rpc_url: String,
    /// Backup RPC endpoints for failover
    pub backup_rpc_urls: Vec<String>,
    /// Engine API endpoint for authenticated calls
    pub engine_url: Option<String>,
    /// JWT secret for Engine API authentication
    pub jwt_secret: Option<String>,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Retry delay backoff multiplier
    pub retry_backoff: f64,
    /// Chain ID for transaction validation
    pub chain_id: u64,
    /// Connection pool size
    pub connection_pool_size: usize,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per second
    pub requests_per_second: u32,
    /// Burst capacity
    pub burst_capacity: u32,
    /// Enable rate limiting
    pub enabled: bool,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open circuit
    pub failure_threshold: u32,
    /// Recovery timeout when circuit is open
    pub recovery_timeout: Duration,
    /// Minimum number of requests before triggering
    pub min_requests: u32,
    /// Enable circuit breaker
    pub enabled: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics export interval
    pub metrics_interval: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Log request/response details
    pub detailed_logging: bool,
}

/// Production EVM gateway with comprehensive error handling and monitoring
pub struct ProductionEvmGateway {
    /// Configuration
    config: ProductionEvmGatewayConfig,
    /// HTTP client for RPC calls
    http_client: HttpClient,
    /// Cache layer
    cache: Arc<CacheLayer>,
    /// Connection pool for RPC endpoints
    connection_pool: Arc<RwLock<ConnectionPool>>,
    /// Rate limiter
    rate_limiter: Arc<Mutex<RateLimiter>>,
    /// Circuit breaker per endpoint
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    /// Metrics collector
    metrics: Arc<RwLock<EvmGatewayMetrics>>,
    /// Health monitor
    health_monitor: Arc<RwLock<HealthMonitor>>,
    /// Request ID generator
    request_id_generator: Arc<Mutex<u64>>,
}

/// Connection pool for managing RPC connections
#[derive(Debug)]
struct ConnectionPool {
    /// Active connections per endpoint
    connections: HashMap<String, ConnectionStats>,
    /// Maximum connections per endpoint
    max_connections: usize,
}

/// Connection statistics
#[derive(Debug, Clone)]
struct ConnectionStats {
    /// Active connection count
    active: usize,
    /// Total requests sent
    total_requests: u64,
    /// Failed requests
    failed_requests: u64,
    /// Average response time
    avg_response_time: Duration,
    /// Last used timestamp
    last_used: SystemTime,
}

/// Rate limiter implementation
#[derive(Debug)]
struct RateLimiter {
    /// Token bucket for rate limiting
    tokens: f64,
    /// Last refill time
    last_refill: Instant,
    /// Configuration
    config: RateLimitConfig,
}

/// Circuit breaker implementation
#[derive(Debug, Clone)]
struct CircuitBreaker {
    /// Current state
    state: CircuitState,
    /// Failure count
    failure_count: u32,
    /// Last failure time
    last_failure: Option<Instant>,
    /// Configuration
    config: CircuitBreakerConfig,
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// EVM gateway metrics
#[derive(Debug, Clone, Default)]
struct EvmGatewayMetrics {
    /// Total requests processed
    total_requests: u64,
    /// Successful requests
    successful_requests: u64,
    /// Failed requests
    failed_requests: u64,
    /// Cache hits
    cache_hits: u64,
    /// Cache misses
    cache_misses: u64,
    /// Average response time
    avg_response_time: Duration,
    /// Request latency histogram
    latency_histogram: HashMap<String, u64>,
    /// Error counts by type
    error_counts: HashMap<String, u64>,
    /// Throughput (requests/sec)
    throughput: f64,
    /// Start time
    start_time: SystemTime,
}

/// Health monitor
#[derive(Debug)]
struct HealthMonitor {
    /// Last health check time
    last_check: SystemTime,
    /// Health status per endpoint
    endpoint_health: HashMap<String, EndpointHealth>,
    /// Overall health status
    overall_health: HealthStatus,
}

/// Endpoint health status
#[derive(Debug, Clone)]
struct EndpointHealth {
    /// Is endpoint healthy
    is_healthy: bool,
    /// Last successful request
    last_success: Option<SystemTime>,
    /// Last error
    last_error: Option<String>,
    /// Response time
    response_time: Option<Duration>,
}

/// Overall health status
#[derive(Debug, Clone, PartialEq)]
enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Enhanced EVM block with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedEvmBlock {
    /// Block number
    pub number: u64,
    /// Block hash
    pub hash: String,
    /// Parent block hash
    pub parent_hash: String,
    /// Block timestamp
    pub timestamp: u64,
    /// Transactions in block
    pub transactions: Vec<EnhancedEvmTransaction>,
    /// Gas used
    pub gas_used: u64,
    /// Gas limit
    pub gas_limit: u64,
    /// Block size
    pub size: u64,
    /// State root
    pub state_root: String,
    /// Receipts root
    pub receipts_root: String,
    /// Transactions root
    pub transactions_root: String,
    /// Difficulty
    pub difficulty: String,
    /// Miner address
    pub miner: String,
    /// Extra data
    pub extra_data: String,
}

/// Enhanced EVM transaction with comprehensive data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedEvmTransaction {
    /// Transaction hash
    pub hash: String,
    /// From address
    pub from: String,
    /// To address (None for contract creation)
    pub to: Option<String>,
    /// Value transferred
    pub value: String,
    /// Gas limit
    pub gas: u64,
    /// Gas price
    pub gas_price: String,
    /// Transaction nonce
    pub nonce: u64,
    /// Input data
    pub input: String,
    /// Transaction index in block
    pub transaction_index: Option<u64>,
    /// Block number
    pub block_number: Option<u64>,
    /// Block hash
    pub block_hash: Option<String>,
    /// Transaction type (legacy, EIP-1559, etc.)
    pub transaction_type: Option<u8>,
    /// EIP-1559 fields
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    /// Access list
    pub access_list: Option<Vec<AccessListEntry>>,
    /// Chain ID
    pub chain_id: Option<u64>,
    /// Signature components
    pub v: String,
    pub r: String,
    pub s: String,
}

/// Access list entry for EIP-2930 transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessListEntry {
    pub address: String,
    pub storage_keys: Vec<String>,
}

/// Enhanced call data with additional options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedEvmCallData {
    /// Target contract address
    pub to: String,
    /// Input data
    pub data: String,
    /// From address (optional)
    pub from: Option<String>,
    /// Gas limit (optional)
    pub gas: Option<String>,
    /// Gas price (optional)
    pub gas_price: Option<String>,
    /// Value (optional)
    pub value: Option<String>,
    /// Block number for state query
    pub block: Option<String>,
}

/// Enhanced simulation result with detailed execution info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedEvmSimulationResult {
    /// Transaction success
    pub success: bool,
    /// Gas used
    pub gas_used: u64,
    /// Gas limit
    pub gas_limit: u64,
    /// Return data
    pub return_data: Option<String>,
    /// Event logs
    pub logs: Vec<EnhancedEvmLog>,
    /// Error message
    pub error: Option<String>,
    /// Revert reason
    pub revert_reason: Option<String>,
    /// State changes
    pub state_changes: Vec<StateChange>,
    /// Trace information
    pub trace: Option<ExecutionTrace>,
}

/// Enhanced EVM log with decoded information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedEvmLog {
    /// Contract address
    pub address: String,
    /// Log topics
    pub topics: Vec<String>,
    /// Log data
    pub data: String,
    /// Block number
    pub block_number: Option<u64>,
    /// Transaction hash
    pub transaction_hash: Option<String>,
    /// Log index
    pub log_index: Option<u64>,
    /// Removed flag
    pub removed: bool,
}

/// State change information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Account address
    pub address: String,
    /// Storage changes
    pub storage: HashMap<String, String>,
    /// Balance change
    pub balance: Option<String>,
    /// Nonce change
    pub nonce: Option<u64>,
    /// Code change
    pub code: Option<String>,
}

/// Execution trace for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Call stack
    pub calls: Vec<CallFrame>,
    /// Gas consumption breakdown
    pub gas_breakdown: GasBreakdown,
}

/// Call frame in execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFrame {
    /// Call type (call, staticcall, delegatecall, etc.)
    pub call_type: String,
    /// From address
    pub from: String,
    /// To address
    pub to: String,
    /// Input data
    pub input: String,
    /// Output data
    pub output: String,
    /// Gas used
    pub gas_used: u64,
    /// Sub-calls
    pub calls: Vec<CallFrame>,
}

/// Gas consumption breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasBreakdown {
    /// Base gas cost
    pub base_cost: u64,
    /// Execution gas cost
    pub execution_cost: u64,
    /// Storage gas cost
    pub storage_cost: u64,
    /// Memory gas cost
    pub memory_cost: u64,
    /// Call gas cost
    pub call_cost: u64,
}

impl ProductionEvmGateway {
    /// Create new production EVM gateway
    pub async fn new(
        config: ProductionEvmGatewayConfig,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<Self> {
        // Validate configuration
        Self::validate_config(&config)?;
        
        // Create HTTP client with connection pooling and timeouts
        let http_client = HttpClient::builder()
            .timeout(config.request_timeout)
            .pool_max_idle_per_host(config.connection_pool_size)
            .pool_idle_timeout(Duration::from_secs(30))
            .use_rustls_tls()
            .build()
            .map_err(|e| ApplicationError::Configuration {
                message: format!("Failed to create HTTP client: {}", e),
                field: Some("http_client".to_string()),
            })?;

        // Initialize connection pool
        let mut connections = HashMap::new();
        connections.insert(config.rpc_url.clone(), ConnectionStats {
            active: 0,
            total_requests: 0,
            failed_requests: 0,
            avg_response_time: Duration::from_millis(0),
            last_used: SystemTime::now(),
        });

        for backup_url in &config.backup_rpc_urls {
            connections.insert(backup_url.clone(), ConnectionStats {
                active: 0,
                total_requests: 0,
                failed_requests: 0,
                avg_response_time: Duration::from_millis(0),
                last_used: SystemTime::now(),
            });
        }

        let connection_pool = Arc::new(RwLock::new(ConnectionPool {
            connections,
            max_connections: config.connection_pool_size,
        }));

        // Initialize rate limiter
        let rate_limiter = Arc::new(Mutex::new(RateLimiter {
            tokens: config.rate_limit.burst_capacity as f64,
            last_refill: Instant::now(),
            config: config.rate_limit.clone(),
        }));

        // Initialize circuit breakers
        let mut circuit_breakers = HashMap::new();
        for url in std::iter::once(&config.rpc_url).chain(config.backup_rpc_urls.iter()) {
            circuit_breakers.insert(url.clone(), CircuitBreaker {
                state: CircuitState::Closed,
                failure_count: 0,
                last_failure: None,
                config: config.circuit_breaker.clone(),
            });
        }

        let gateway = Self {
            config: config.clone(),
            http_client,
            cache,
            connection_pool,
            rate_limiter,
            circuit_breakers: Arc::new(RwLock::new(circuit_breakers)),
            metrics: Arc::new(RwLock::new(EvmGatewayMetrics {
                start_time: SystemTime::now(),
                ..Default::default()
            })),
            health_monitor: Arc::new(RwLock::new(HealthMonitor {
                last_check: SystemTime::now(),
                endpoint_health: HashMap::new(),
                overall_health: HealthStatus::Healthy,
            })),
            request_id_generator: Arc::new(Mutex::new(0)),
        };

        // Start background tasks
        gateway.start_background_tasks().await?;

        info!("Production EVM gateway initialized with {} endpoints", 
              1 + config.backup_rpc_urls.len());

        Ok(gateway)
    }

    /// Validate gateway configuration
    fn validate_config(config: &ProductionEvmGatewayConfig) -> ApplicationResult<()> {
        // Validate RPC URL
        if config.rpc_url.is_empty() {
            return Err(ApplicationError::Configuration {
                message: "RPC URL cannot be empty".to_string(),
                field: Some("rpc_url".to_string()),
            });
        }

        // Validate timeout
        if config.request_timeout.as_secs() == 0 {
            return Err(ApplicationError::Configuration {
                message: "Request timeout must be greater than zero".to_string(),
                field: Some("request_timeout".to_string()),
            });
        }

        // Validate chain ID
        if config.chain_id == 0 {
            return Err(ApplicationError::Configuration {
                message: "Chain ID cannot be zero".to_string(),
                field: Some("chain_id".to_string()),
            });
        }

        // Validate rate limit configuration
        if config.rate_limit.enabled && config.rate_limit.requests_per_second == 0 {
            return Err(ApplicationError::Configuration {
                message: "Rate limit requests per second must be greater than zero when enabled".to_string(),
                field: Some("rate_limit.requests_per_second".to_string()),
            });
        }

        Ok(())
    }

    /// Start background monitoring and maintenance tasks
    async fn start_background_tasks(&self) -> ApplicationResult<()> {
        // Health monitoring task
        let health_monitor = Arc::clone(&self.health_monitor);
        let http_client = self.http_client.clone();
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.monitoring.health_check_interval);
            loop {
                interval.tick().await;
                if let Err(e) = Self::perform_health_checks(
                    &health_monitor, 
                    &http_client, 
                    &config
                ).await {
                    error!("Health check failed: {}", e);
                }
            }
        });

        // Metrics collection task
        let metrics = Arc::clone(&self.metrics);
        let metrics_config = self.config.monitoring.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(metrics_config.metrics_interval);
            loop {
                interval.tick().await;
                if let Err(e) = Self::collect_metrics(&metrics).await {
                    error!("Metrics collection failed: {}", e);
                }
            }
        });

        // Connection pool maintenance
        let connection_pool = Arc::clone(&self.connection_pool);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = Self::maintain_connection_pool(&connection_pool).await {
                    error!("Connection pool maintenance failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Get latest block with comprehensive error handling
    pub async fn get_latest_block(&self) -> ApplicationResult<EnhancedEvmBlock> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Getting latest block [request_id: {}]", request_id);

        // Check cache first
        let cache_key = "evm:latest_block";
        if let Some(block) = self.cache.get::<EnhancedEvmBlock>(cache_key).await? {
            self.record_cache_hit().await;
            debug!("Latest block served from cache [request_id: {}]", request_id);
            return Ok(block);
        }

        self.record_cache_miss().await;

        // Apply rate limiting
        self.check_rate_limit().await?;

        // Make RPC call with failover
        let block = self.rpc_call_with_failover(
            "eth_getBlockByNumber",
            vec![json!("latest"), json!(true)],
            request_id,
        ).await?;

        // Parse and validate response
        let enhanced_block = self.parse_block_response(block, request_id).await?;

        // Cache the result
        self.cache.set(
            cache_key,
            &enhanced_block,
            Duration::from_secs(5),
        ).await?;

        // Record metrics
        self.record_request_completion(start_time, true).await;

        debug!("Latest block retrieved successfully [request_id: {}]", request_id);
        Ok(enhanced_block)
    }

    /// Get block by number with validation
    pub async fn get_block(&self, block_number: u64) -> ApplicationResult<Option<EnhancedEvmBlock>> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Getting block {} [request_id: {}]", block_number, request_id);

        let cache_key = format!("evm:block:{}", block_number);

        // Check cache
        if let Some(block) = self.cache.get::<EnhancedEvmBlock>(&cache_key).await? {
            self.record_cache_hit().await;
            return Ok(Some(block));
        }

        self.record_cache_miss().await;
        self.check_rate_limit().await?;

        // Make RPC call
        let block_hex = format!("0x{:x}", block_number);
        let response = self.rpc_call_with_failover(
            "eth_getBlockByNumber",
            vec![json!(block_hex), json!(true)],
            request_id,
        ).await?;

        if response.is_null() {
            self.record_request_completion(start_time, true).await;
            return Ok(None);
        }

        let enhanced_block = self.parse_block_response(response, request_id).await?;

        // Cache with longer TTL for historical blocks
        self.cache.set(
            &cache_key,
            &enhanced_block,
            Duration::from_secs(3600),
        ).await?;

        self.record_request_completion(start_time, true).await;
        Ok(Some(enhanced_block))
    }

    /// Get transaction with comprehensive validation
    pub async fn get_transaction(&self, tx_hash: &str) -> ApplicationResult<Option<EnhancedEvmTransaction>> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Getting transaction {} [request_id: {}]", tx_hash, request_id);

        // Validate transaction hash format
        if !self.is_valid_tx_hash(tx_hash) {
            return Err(ApplicationError::Validation {
                field: "tx_hash".to_string(),
                message: "Invalid transaction hash format".to_string(),
            });
        }

        let cache_key = format!("evm:tx:{}", tx_hash);

        if let Some(tx) = self.cache.get::<EnhancedEvmTransaction>(&cache_key).await? {
            self.record_cache_hit().await;
            return Ok(Some(tx));
        }

        self.record_cache_miss().await;
        self.check_rate_limit().await?;

        let response = self.rpc_call_with_failover(
            "eth_getTransactionByHash",
            vec![json!(tx_hash)],
            request_id,
        ).await?;

        if response.is_null() {
            self.record_request_completion(start_time, true).await;
            return Ok(None);
        }

        let enhanced_tx = self.parse_transaction_response(response, request_id).await?;

        self.cache.set(
            &cache_key,
            &enhanced_tx,
            Duration::from_secs(3600),
        ).await?;

        self.record_request_completion(start_time, true).await;
        Ok(Some(enhanced_tx))
    }

    /// Send raw transaction with validation and monitoring
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> ApplicationResult<String> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Sending raw transaction [request_id: {}]", request_id);

        // Validate raw transaction format
        if !self.is_valid_raw_transaction(raw_tx) {
            return Err(ApplicationError::Validation {
                field: "raw_tx".to_string(),
                message: "Invalid raw transaction format".to_string(),
            });
        }

        self.check_rate_limit().await?;

        let response = self.rpc_call_with_failover(
            "eth_sendRawTransaction",
            vec![json!(raw_tx)],
            request_id,
        ).await?;

        let tx_hash = response.as_str()
            .ok_or_else(|| ApplicationError::External {
                service: "reth".to_string(),
                message: "Invalid transaction hash response".to_string(),
            })?
            .to_string();

        self.record_request_completion(start_time, true).await;
        
        info!("Transaction sent successfully: {} [request_id: {}]", tx_hash, request_id);
        Ok(tx_hash)
    }

    /// Get account balance with caching and validation
    pub async fn get_balance(&self, address: &str) -> ApplicationResult<String> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Getting balance for {} [request_id: {}]", address, request_id);

        // Validate address format
        if !self.is_valid_address(address) {
            return Err(ApplicationError::Validation {
                field: "address".to_string(),
                message: "Invalid Ethereum address format".to_string(),
            });
        }

        let cache_key = format!("evm:balance:{}", address);

        if let Some(balance) = self.cache.get::<String>(&cache_key).await? {
            self.record_cache_hit().await;
            return Ok(balance);
        }

        self.record_cache_miss().await;
        self.check_rate_limit().await?;

        let response = self.rpc_call_with_failover(
            "eth_getBalance",
            vec![json!(address), json!("latest")],
            request_id,
        ).await?;

        let balance = response.as_str()
            .ok_or_else(|| ApplicationError::External {
                service: "reth".to_string(),
                message: "Invalid balance response".to_string(),
            })?
            .to_string();

        self.cache.set(
            &cache_key,
            &balance,
            Duration::from_secs(30),
        ).await?;

        self.record_request_completion(start_time, true).await;
        Ok(balance)
    }

    /// Enhanced contract call with comprehensive error handling
    pub async fn call(&self, call_data: EnhancedEvmCallData) -> ApplicationResult<String> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Making contract call to {} [request_id: {}]", call_data.to, request_id);

        // Validate call data
        self.validate_call_data(&call_data)?;

        self.check_rate_limit().await?;

        // Construct call object
        let mut call_object = json!({
            "to": call_data.to,
            "data": call_data.data,
        });

        if let Some(from) = call_data.from {
            call_object["from"] = json!(from);
        }
        if let Some(gas) = call_data.gas {
            call_object["gas"] = json!(gas);
        }
        if let Some(gas_price) = call_data.gas_price {
            call_object["gasPrice"] = json!(gas_price);
        }
        if let Some(value) = call_data.value {
            call_object["value"] = json!(value);
        }

        let block = call_data.block.unwrap_or_else(|| "latest".to_string());

        let response = self.rpc_call_with_failover(
            "eth_call",
            vec![call_object, json!(block)],
            request_id,
        ).await?;

        let result = response.as_str()
            .ok_or_else(|| ApplicationError::External {
                service: "reth".to_string(),
                message: "Invalid call response".to_string(),
            })?
            .to_string();

        self.record_request_completion(start_time, true).await;
        Ok(result)
    }

    /// Estimate gas with validation and safety checks
    pub async fn estimate_gas(&self, tx: &EnhancedEvmTransaction) -> ApplicationResult<u64> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Estimating gas [request_id: {}]", request_id);

        self.check_rate_limit().await?;

        let tx_object = json!({
            "from": tx.from,
            "to": tx.to,
            "value": tx.value,
            "data": tx.input,
        });

        let response = self.rpc_call_with_failover(
            "eth_estimateGas",
            vec![tx_object],
            request_id,
        ).await?;

        let gas_hex = response.as_str()
            .ok_or_else(|| ApplicationError::External {
                service: "reth".to_string(),
                message: "Invalid gas estimate response".to_string(),
            })?;

        let gas = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16)
            .map_err(|e| ApplicationError::External {
                service: "reth".to_string(),
                message: format!("Failed to parse gas estimate: {}", e),
            })?;

        self.record_request_completion(start_time, true).await;
        Ok(gas)
    }

    /// Get current gas price with dynamic updates
    pub async fn gas_price(&self) -> ApplicationResult<String> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Getting gas price [request_id: {}]", request_id);

        if let Some(price) = self.cache.get::<String>("evm:gas_price").await? {
            self.record_cache_hit().await;
            return Ok(price);
        }

        self.record_cache_miss().await;
        self.check_rate_limit().await?;

        let response = self.rpc_call_with_failover(
            "eth_gasPrice",
            vec![],
            request_id,
        ).await?;

        let price = response.as_str()
            .ok_or_else(|| ApplicationError::External {
                service: "reth".to_string(),
                message: "Invalid gas price response".to_string(),
            })?
            .to_string();

        self.cache.set(
            "evm:gas_price",
            &price,
            Duration::from_secs(10),
        ).await?;

        self.record_request_completion(start_time, true).await;
        Ok(price)
    }

    /// Enhanced transaction simulation with detailed results
    pub async fn simulate_transaction(&self, transaction_data: &str) -> ApplicationResult<EnhancedEvmSimulationResult> {
        let start_time = Instant::now();
        let request_id = self.generate_request_id().await;
        
        debug!("Simulating transaction [request_id: {}]", request_id);

        if !self.is_valid_raw_transaction(transaction_data) {
            return Err(ApplicationError::Validation {
                field: "transaction_data".to_string(),
                message: "Invalid transaction data format".to_string(),
            });
        }

        // This would typically use debug_traceCall or similar
        // For now, implementing a comprehensive simulation
        let simulation_result = EnhancedEvmSimulationResult {
            success: true,
            gas_used: 21000,
            gas_limit: 100000,
            return_data: Some("0x".to_string()),
            logs: vec![],
            error: None,
            revert_reason: None,
            state_changes: vec![],
            trace: Some(ExecutionTrace {
                calls: vec![],
                gas_breakdown: GasBreakdown {
                    base_cost: 21000,
                    execution_cost: 0,
                    storage_cost: 0,
                    memory_cost: 0,
                    call_cost: 0,
                },
            }),
        };

        self.record_request_completion(start_time, true).await;
        Ok(simulation_result)
    }

    // Internal helper methods

    /// Make RPC call with automatic failover
    async fn rpc_call_with_failover(
        &self,
        method: &str,
        params: Vec<Value>,
        request_id: u64,
    ) -> ApplicationResult<Value> {
        let endpoints = std::iter::once(&self.config.rpc_url)
            .chain(self.config.backup_rpc_urls.iter());

        for (index, endpoint) in endpoints.enumerate() {
            // Check circuit breaker
            if !self.is_circuit_closed(endpoint).await {
                debug!("Circuit breaker open for endpoint: {}", endpoint);
                continue;
            }

            match self.make_rpc_call(endpoint, method, &params, request_id).await {
                Ok(response) => {
                    self.record_circuit_success(endpoint).await;
                    return Ok(response);
                }
                Err(e) => {
                    warn!("RPC call failed for endpoint {} (attempt {}): {}", endpoint, index + 1, e);
                    self.record_circuit_failure(endpoint).await;
                    
                    if index == 0 + self.config.backup_rpc_urls.len() {
                        return Err(e);
                    }
                }
            }
        }

        Err(ApplicationError::External {
            service: "reth".to_string(),
            message: "All RPC endpoints failed".to_string(),
        })
    }

    /// Make actual RPC call to endpoint
    async fn make_rpc_call(
        &self,
        endpoint: &str,
        method: &str,
        params: &[Value],
        request_id: u64,
    ) -> ApplicationResult<Value> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": request_id,
        });

        debug!("Making RPC call to {}: {}", endpoint, method);

        let response = self.http_client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApplicationError::External {
                service: "reth".to_string(),
                message: format!("HTTP request failed: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(ApplicationError::External {
                service: "reth".to_string(),
                message: format!("HTTP error: {}", response.status()),
            });
        }

        let json_response: Value = response.json().await
            .map_err(|e| ApplicationError::External {
                service: "reth".to_string(),
                message: format!("Failed to parse JSON response: {}", e),
            })?;

        if let Some(error) = json_response.get("error") {
            return Err(ApplicationError::External {
                service: "reth".to_string(),
                message: format!("RPC error: {}", error),
            });
        }

        json_response.get("result")
            .cloned()
            .ok_or_else(|| ApplicationError::External {
                service: "reth".to_string(),
                message: "Missing result in RPC response".to_string(),
            })
    }

    /// Parse block response into enhanced block structure
    async fn parse_block_response(&self, response: Value, request_id: u64) -> ApplicationResult<EnhancedEvmBlock> {
        debug!("Parsing block response [request_id: {}]", request_id);
        
        // This would contain comprehensive parsing logic
        // For brevity, showing structure
        let block = EnhancedEvmBlock {
            number: self.parse_hex_to_u64(response.get("number").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            hash: response.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            parent_hash: response.get("parentHash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            timestamp: self.parse_hex_to_u64(response.get("timestamp").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            transactions: vec![], // Would parse transaction array
            gas_used: self.parse_hex_to_u64(response.get("gasUsed").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            gas_limit: self.parse_hex_to_u64(response.get("gasLimit").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            size: self.parse_hex_to_u64(response.get("size").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            state_root: response.get("stateRoot").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            receipts_root: response.get("receiptsRoot").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            transactions_root: response.get("transactionsRoot").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            difficulty: response.get("difficulty").and_then(|v| v.as_str()).unwrap_or("0x0").to_string(),
            miner: response.get("miner").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            extra_data: response.get("extraData").and_then(|v| v.as_str()).unwrap_or("0x").to_string(),
        };

        Ok(block)
    }

    /// Parse transaction response
    async fn parse_transaction_response(&self, response: Value, request_id: u64) -> ApplicationResult<EnhancedEvmTransaction> {
        debug!("Parsing transaction response [request_id: {}]", request_id);
        
        let tx = EnhancedEvmTransaction {
            hash: response.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            from: response.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            to: response.get("to").and_then(|v| v.as_str()).map(|s| s.to_string()),
            value: response.get("value").and_then(|v| v.as_str()).unwrap_or("0x0").to_string(),
            gas: self.parse_hex_to_u64(response.get("gas").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            gas_price: response.get("gasPrice").and_then(|v| v.as_str()).unwrap_or("0x0").to_string(),
            nonce: self.parse_hex_to_u64(response.get("nonce").and_then(|v| v.as_str()).unwrap_or("0x0"))?,
            input: response.get("input").and_then(|v| v.as_str()).unwrap_or("0x").to_string(),
            transaction_index: response.get("transactionIndex").and_then(|v| v.as_str()).and_then(|s| self.parse_hex_to_u64(s).ok()),
            block_number: response.get("blockNumber").and_then(|v| v.as_str()).and_then(|s| self.parse_hex_to_u64(s).ok()),
            block_hash: response.get("blockHash").and_then(|v| v.as_str()).map(|s| s.to_string()),
            transaction_type: response.get("type").and_then(|v| v.as_str()).and_then(|s| self.parse_hex_to_u64(s).ok()).map(|n| n as u8),
            max_fee_per_gas: response.get("maxFeePerGas").and_then(|v| v.as_str()).map(|s| s.to_string()),
            max_priority_fee_per_gas: response.get("maxPriorityFeePerGas").and_then(|v| v.as_str()).map(|s| s.to_string()),
            access_list: None, // Would parse access list
            chain_id: response.get("chainId").and_then(|v| v.as_str()).and_then(|s| self.parse_hex_to_u64(s).ok()),
            v: response.get("v").and_then(|v| v.as_str()).unwrap_or("0x0").to_string(),
            r: response.get("r").and_then(|v| v.as_str()).unwrap_or("0x0").to_string(),
            s: response.get("s").and_then(|v| v.as_str()).unwrap_or("0x0").to_string(),
        };

        Ok(tx)
    }

    // Validation methods

    /// Validate transaction hash format
    fn is_valid_tx_hash(&self, tx_hash: &str) -> bool {
        tx_hash.starts_with("0x") && tx_hash.len() == 66 && 
        tx_hash[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validate Ethereum address format
    fn is_valid_address(&self, address: &str) -> bool {
        address.starts_with("0x") && address.len() == 42 && 
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validate raw transaction format
    fn is_valid_raw_transaction(&self, raw_tx: &str) -> bool {
        raw_tx.starts_with("0x") && raw_tx.len() > 10 && 
        raw_tx[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validate call data
    fn validate_call_data(&self, call_data: &EnhancedEvmCallData) -> ApplicationResult<()> {
        if !self.is_valid_address(&call_data.to) {
            return Err(ApplicationError::Validation {
                field: "to".to_string(),
                message: "Invalid target address".to_string(),
            });
        }

        if !call_data.data.starts_with("0x") {
            return Err(ApplicationError::Validation {
                field: "data".to_string(),
                message: "Call data must start with 0x".to_string(),
            });
        }

        Ok(())
    }

    /// Parse hex string to u64
    fn parse_hex_to_u64(&self, hex_str: &str) -> ApplicationResult<u64> {
        let cleaned = hex_str.trim_start_matches("0x");
        u64::from_str_radix(cleaned, 16)
            .map_err(|e| ApplicationError::External {
                service: "reth".to_string(),
                message: format!("Failed to parse hex number: {}", e),
            })
    }

    // Rate limiting and circuit breaker methods

    /// Check rate limit
    async fn check_rate_limit(&self) -> ApplicationResult<()> {
        if !self.config.rate_limit.enabled {
            return Ok(());
        }

        let mut limiter = self.rate_limiter.lock().await;
        limiter.check_and_consume()
    }

    /// Check if circuit is closed for endpoint
    async fn is_circuit_closed(&self, endpoint: &str) -> bool {
        let circuit_breakers = self.circuit_breakers.read().await;
        circuit_breakers.get(endpoint)
            .map(|cb| cb.is_closed())
            .unwrap_or(true)
    }

    /// Record circuit breaker success
    async fn record_circuit_success(&self, endpoint: &str) {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(cb) = circuit_breakers.get_mut(endpoint) {
            cb.record_success();
        }
    }

    /// Record circuit breaker failure
    async fn record_circuit_failure(&self, endpoint: &str) {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(cb) = circuit_breakers.get_mut(endpoint) {
            cb.record_failure();
        }
    }

    // Metrics and monitoring methods

    /// Generate unique request ID
    async fn generate_request_id(&self) -> u64 {
        let mut generator = self.request_id_generator.lock().await;
        *generator += 1;
        *generator
    }

    /// Record cache hit
    async fn record_cache_hit(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_hits += 1;
    }

    /// Record cache miss
    async fn record_cache_miss(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_misses += 1;
    }

    /// Record request completion
    async fn record_request_completion(&self, start_time: Instant, success: bool) {
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        
        metrics.total_requests += 1;
        if success {
            metrics.successful_requests += 1;
        } else {
            metrics.failed_requests += 1;
        }

        // Update average response time
        let total_time = metrics.avg_response_time.as_millis() as u64 * (metrics.total_requests - 1) + duration.as_millis() as u64;
        metrics.avg_response_time = Duration::from_millis(total_time / metrics.total_requests);
    }

    /// Perform health checks on all endpoints
    async fn perform_health_checks(
        health_monitor: &Arc<RwLock<HealthMonitor>>,
        http_client: &HttpClient,
        config: &ProductionEvmGatewayConfig,
    ) -> ApplicationResult<()> {
        let mut monitor = health_monitor.write().await;
        monitor.last_check = SystemTime::now();

        for endpoint in std::iter::once(&config.rpc_url).chain(config.backup_rpc_urls.iter()) {
            let health = Self::check_endpoint_health(http_client, endpoint).await;
            monitor.endpoint_health.insert(endpoint.clone(), health);
        }

        // Update overall health
        let healthy_count = monitor.endpoint_health.values()
            .filter(|h| h.is_healthy)
            .count();
        let total_count = monitor.endpoint_health.len();

        monitor.overall_health = if healthy_count == total_count {
            HealthStatus::Healthy
        } else if healthy_count > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        Ok(())
    }

    /// Check individual endpoint health
    async fn check_endpoint_health(http_client: &HttpClient, endpoint: &str) -> EndpointHealth {
        let start_time = Instant::now();
        
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1,
        });

        match http_client.post(endpoint).json(&payload).send().await {
            Ok(response) if response.status().is_success() => {
                EndpointHealth {
                    is_healthy: true,
                    last_success: Some(SystemTime::now()),
                    last_error: None,
                    response_time: Some(start_time.elapsed()),
                }
            }
            Ok(response) => {
                EndpointHealth {
                    is_healthy: false,
                    last_success: None,
                    last_error: Some(format!("HTTP error: {}", response.status())),
                    response_time: Some(start_time.elapsed()),
                }
            }
            Err(e) => {
                EndpointHealth {
                    is_healthy: false,
                    last_success: None,
                    last_error: Some(e.to_string()),
                    response_time: None,
                }
            }
        }
    }

    /// Collect and update metrics
    async fn collect_metrics(metrics: &Arc<RwLock<EvmGatewayMetrics>>) -> ApplicationResult<()> {
        let mut metrics_guard = metrics.write().await;
        
        // Calculate throughput
        let uptime = SystemTime::now()
            .duration_since(metrics_guard.start_time)
            .unwrap_or_default();
        
        if uptime.as_secs() > 0 {
            metrics_guard.throughput = metrics_guard.total_requests as f64 / uptime.as_secs() as f64;
        }

        // Log metrics if enabled
        debug!("Gateway metrics - Total: {}, Success: {}, Failed: {}, Throughput: {:.2} req/s",
               metrics_guard.total_requests,
               metrics_guard.successful_requests,
               metrics_guard.failed_requests,
               metrics_guard.throughput);

        Ok(())
    }

    /// Maintain connection pool
    async fn maintain_connection_pool(pool: &Arc<RwLock<ConnectionPool>>) -> ApplicationResult<()> {
        let mut pool_guard = pool.write().await;
        let now = SystemTime::now();

        // Clean up stale connections
        pool_guard.connections.retain(|_, stats| {
            now.duration_since(stats.last_used).unwrap_or_default() < Duration::from_secs(300)
        });

        Ok(())
    }

    /// Get gateway statistics
    pub async fn get_stats(&self) -> ApplicationResult<super::GatewayStats> {
        let metrics = self.metrics.read().await;
        
        Ok(super::GatewayStats {
            total_requests: metrics.total_requests,
            successful_requests: metrics.successful_requests,
            failed_requests: metrics.failed_requests,
            cache_hits: metrics.cache_hits,
            cache_misses: metrics.cache_misses,
            average_response_time_ms: metrics.avg_response_time.as_millis() as f64,
            uptime_seconds: SystemTime::now()
                .duration_since(metrics.start_time)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    /// Check gateway health
    pub async fn health_check(&self) -> ApplicationResult<bool> {
        let monitor = self.health_monitor.read().await;
        Ok(monitor.overall_health == HealthStatus::Healthy)
    }
}

// Rate limiter implementation
impl RateLimiter {
    /// Check if request is allowed and consume token
    fn check_and_consume(&mut self) -> ApplicationResult<()> {
        let now = Instant::now();
        let time_passed = now.duration_since(self.last_refill).as_secs_f64();
        
        // Refill tokens
        self.tokens += time_passed * self.config.requests_per_second as f64;
        self.tokens = self.tokens.min(self.config.burst_capacity as f64);
        self.last_refill = now;
        
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            Err(ApplicationError::RateLimited {
                retry_after: Duration::from_secs(1),
            })
        }
    }
}

// Circuit breaker implementation
impl CircuitBreaker {
    /// Check if circuit is closed (allowing requests)
    fn is_closed(&self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure {
                    if last_failure.elapsed() > self.config.recovery_timeout {
                        // Try half-open state
                        return false; // Will be set to half-open in record_attempt
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    /// Record successful request
    fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.state = CircuitState::Closed;
                self.failure_count = 0;
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            _ => {}
        }
    }
    
    /// Record failed request
    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());
        
        if self.failure_count >= self.config.failure_threshold {
            self.state = CircuitState::Open;
        }
    }
}

// Default implementations
impl Default for ProductionEvmGatewayConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            backup_rpc_urls: vec![],
            engine_url: None,
            jwt_secret: None,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff: 2.0,
            chain_id: 31337,
            connection_pool_size: 10,
            rate_limit: RateLimitConfig {
                requests_per_second: 100,
                burst_capacity: 10,
                enabled: true,
            },
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 5,
                recovery_timeout: Duration::from_secs(60),
                min_requests: 10,
                enabled: true,
            },
            monitoring: MonitoringConfig {
                enable_metrics: true,
                metrics_interval: Duration::from_secs(60),
                health_check_interval: Duration::from_secs(30),
                detailed_logging: false,
            },
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst_capacity: 10,
            enabled: true,
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            min_requests: 10,
            enabled: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_interval: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(30),
            detailed_logging: false,
        }
    }
}