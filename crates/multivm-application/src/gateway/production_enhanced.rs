//! Enhanced production gateway implementation with full special transaction support
//!
//! This module provides a complete production-ready gateway that integrates with
//! the special transaction service and other production components.

use crate::error::{ApplicationError, ApplicationResult};
use crate::gateway::cache::CacheManager;
use crate::gateway::special_transactions::{
    SpecialTransactionService, DetailedSpecialTransaction, TransactionQuery,
    TransactionStatus as SpecialTxStatus, SortBy, SortOrder,
};
use async_trait::async_trait;
use multivm_common::{ExecutionResult, MultivmResult, TransactionStatus};
use multivm_consensus::MultiVMBlock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use std::collections::HashMap;

/// Enhanced production gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedGatewayConfig {
    /// Base configuration
    pub base: ProductionGatewayConfig,
    
    /// Special transaction service configuration
    pub special_tx_config: SpecialTransactionServiceConfig,
    
    /// Query configuration
    pub query_config: QueryConfig,
    
    /// Monitoring configuration
    pub monitoring_config: MonitoringConfig,
}

/// Special transaction service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTransactionServiceConfig {
    /// Redis URL for transaction storage
    pub redis_url: String,
    
    /// Enable transaction indexing
    pub enable_indexing: bool,
    
    /// Transaction pool sync interval
    pub pool_sync_interval: Duration,
    
    /// Maximum pending transactions
    pub max_pending_transactions: usize,
}

/// Query configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// Default query limit
    pub default_limit: usize,
    
    /// Maximum query limit
    pub max_limit: usize,
    
    /// Query timeout
    pub query_timeout: Duration,
    
    /// Enable query caching
    pub enable_query_cache: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable detailed metrics
    pub enable_metrics: bool,
    
    /// Metrics push interval
    pub metrics_interval: Duration,
    
    /// Enable distributed tracing
    pub enable_tracing: bool,
    
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
}

/// Alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Maximum pending transactions before alert
    pub max_pending_transactions: usize,
    
    /// Maximum transaction processing time
    pub max_processing_time: Duration,
    
    /// Minimum success rate percentage
    pub min_success_rate: f64,
}

/// Enhanced production gateway implementation
pub struct EnhancedProductionGateway {
    /// Configuration
    config: EnhancedGatewayConfig,
    
    /// Cache manager
    cache: Option<Arc<CacheManager>>,
    
    /// Consensus client
    consensus_client: Arc<dyn ConsensusClient>,
    
    /// Account mapping client
    account_mapping_client: Arc<dyn AccountMappingClient>,
    
    /// Special transaction service
    special_tx_service: Arc<SpecialTransactionService>,
    
    /// Health monitor
    health_monitor: Arc<RwLock<HealthMonitor>>,
    
    /// Query processor
    query_processor: Arc<QueryProcessor>,
    
    /// Metrics collector
    metrics_collector: Arc<MetricsCollector>,
}

/// Production gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionGatewayConfig {
    pub cache_enabled: bool,
    pub cache_ttl_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub request_timeout_ms: u64,
    pub health_check_interval_ms: u64,
}

/// Production gateway trait
#[async_trait]
pub trait ProductionGateway: Send + Sync {
    async fn get_block(&self, block_id: &str) -> MultivmResult<Option<MultiVMBlock>>;
    async fn get_latest_block(&self) -> MultivmResult<MultiVMBlock>;
    async fn submit_special_transaction(
        &self,
        transaction: SpecialTransaction,
    ) -> MultivmResult<TransactionResult>;
    async fn get_special_transaction(
        &self,
        tx_id: &str,
    ) -> MultivmResult<Option<SpecialTransactionInfo>>;
    async fn query_special_transactions(
        &self,
        query: SpecialTransactionQuery,
    ) -> MultivmResult<QueryResult<SpecialTransactionInfo>>;
    async fn get_account_binding(
        &self,
        multivm_id: &str,
    ) -> MultivmResult<Option<AccountBinding>>;
    async fn get_consensus_status(&self) -> MultivmResult<ConsensusStatus>;
    async fn get_system_health(&self) -> MultivmResult<SystemHealth>;
}

/// Special transaction types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SpecialTransaction {
    AccountBinding {
        source_chain: String,
        source_address: String,
        target_chain: String,
        target_address: String,
        proof: Vec<u8>,
    },
    CrossVmTransfer {
        from_chain: String,
        from_address: String,
        to_chain: String,
        to_address: String,
        amount: String,
        asset: String,
    },
    UpdateBinding {
        multivm_id: String,
        updates: serde_json::Value,
    },
}

/// Transaction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub transaction_id: String,
    pub status: TransactionStatus,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub estimated_confirmation_time: Option<Duration>,
}

/// Special transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTransactionInfo {
    pub transaction_id: String,
    pub transaction_type: String,
    pub status: TransactionStatus,
    pub block_height: Option<u64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub details: serde_json::Value,
    pub gas_used: Option<u64>,
    pub fee_paid: Option<u128>,
    pub confirmations: u32,
}

/// Special transaction query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTransactionQuery {
    /// Filter by account
    pub account: Option<String>,
    
    /// Filter by status
    pub status: Option<TransactionStatus>,
    
    /// Filter by transaction type
    pub transaction_type: Option<String>,
    
    /// Filter by time range
    pub time_range: Option<TimeRange>,
    
    /// Pagination
    pub pagination: PaginationParams,
    
    /// Sort order
    pub sort: SortParams,
}

/// Time range filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub offset: usize,
    pub limit: usize,
}

/// Sort parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortParams {
    pub field: String,
    pub order: SortOrder,
}

/// Query result with pagination info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

/// Account binding information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBinding {
    pub multivm_id: String,
    pub bindings: Vec<ChainBinding>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub verification_status: VerificationStatus,
}

/// Chain binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBinding {
    pub chain: String,
    pub address: String,
    pub verified: bool,
    pub verification_proof: Option<Vec<u8>>,
}

/// Verification status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Unverified,
    Pending,
    Verified,
    Failed,
}

/// Consensus status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusStatus {
    pub is_syncing: bool,
    pub current_height: u64,
    pub highest_block: u64,
    pub connected_peers: usize,
    pub validator_count: usize,
    pub is_validator: bool,
}

/// System health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: HealthStatus,
    pub components: Vec<ComponentHealth>,
    pub uptime_seconds: u64,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Consensus client trait
#[async_trait]
pub trait ConsensusClient: Send + Sync {
    async fn get_block(&self, block_id: &str) -> MultivmResult<Option<MultiVMBlock>>;
    async fn get_latest_block(&self) -> MultivmResult<MultiVMBlock>;
    async fn get_status(&self) -> MultivmResult<ConsensusStatus>;
    async fn submit_transaction(&self, tx: Vec<u8>) -> MultivmResult<String>;
}

/// Account mapping client trait
#[async_trait]
pub trait AccountMappingClient: Send + Sync {
    async fn get_binding(&self, multivm_id: &str) -> MultivmResult<Option<AccountBinding>>;
    async fn create_binding(&self, binding: AccountBinding) -> MultivmResult<String>;
    async fn update_binding(&self, multivm_id: &str, updates: serde_json::Value) -> MultivmResult<()>;
}

/// Health monitor
struct HealthMonitor {
    component_status: HashMap<String, ComponentHealth>,
    start_time: std::time::Instant,
    last_check: SystemTime,
}

/// Query processor for advanced queries
struct QueryProcessor {
    config: QueryConfig,
    cache: Option<Arc<CacheManager>>,
}

/// Metrics collector
struct MetricsCollector {
    config: MonitoringConfig,
    metrics: Arc<RwLock<GatewayMetrics>>,
}

/// Gateway metrics
#[derive(Debug, Default)]
struct GatewayMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    special_tx_submitted: u64,
    special_tx_completed: u64,
    average_response_time_ms: u64,
    cache_hits: u64,
    cache_misses: u64,
}

impl EnhancedProductionGateway {
    /// Create new enhanced production gateway
    pub async fn new(
        config: EnhancedGatewayConfig,
        consensus_client: Arc<dyn ConsensusClient>,
        account_mapping_client: Arc<dyn AccountMappingClient>,
    ) -> MultivmResult<Self> {
        // Create cache if enabled
        let cache = if config.base.cache_enabled {
            Some(Arc::new(CacheManager::new(
                config.base.cache_ttl_seconds,
                10000, // max entries
            )))
        } else {
            None
        };

        // Create special transaction service
        let special_tx_config = crate::gateway::special_transactions::SpecialTransactionConfig {
            redis_url: config.special_tx_config.redis_url.clone(),
            pending_cache_ttl: Duration::from_secs(60),
            confirmed_cache_ttl: Duration::from_secs(3600),
            max_cache_size: 10000,
            query_timeout: config.query_config.query_timeout,
            enable_indexing: config.special_tx_config.enable_indexing,
            retention_period: Duration::from_secs(86400 * 30), // 30 days
        };
        
        let special_tx_service = Arc::new(
            SpecialTransactionService::new(special_tx_config).await?
        );

        // Create health monitor
        let health_monitor = Arc::new(RwLock::new(HealthMonitor {
            component_status: HashMap::new(),
            start_time: std::time::Instant::now(),
            last_check: SystemTime::now(),
        }));

        // Create query processor
        let query_processor = Arc::new(QueryProcessor {
            config: config.query_config.clone(),
            cache: cache.clone(),
        });

        // Create metrics collector
        let metrics_collector = Arc::new(MetricsCollector {
            config: config.monitoring_config.clone(),
            metrics: Arc::new(RwLock::new(GatewayMetrics::default())),
        });

        let gateway = Self {
            config,
            cache,
            consensus_client,
            account_mapping_client,
            special_tx_service,
            health_monitor,
            query_processor,
            metrics_collector,
        };

        // Start background tasks
        gateway.start_background_tasks().await;

        Ok(gateway)
    }

    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Health check task
        let health_monitor = Arc::clone(&self.health_monitor);
        let interval = Duration::from_millis(self.config.base.health_check_interval_ms);
        let consensus_client = Arc::clone(&self.consensus_client);
        let special_tx_service = Arc::clone(&self.special_tx_service);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                Self::perform_health_check(
                    &health_monitor,
                    &consensus_client,
                    &special_tx_service,
                ).await;
            }
        });

        // Metrics collection task
        if self.config.monitoring_config.enable_metrics {
            let metrics_collector = Arc::clone(&self.metrics_collector);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(
                    metrics_collector.config.metrics_interval
                );
                loop {
                    interval.tick().await;
                    metrics_collector.collect_and_push_metrics().await;
                }
            });
        }
    }

    /// Perform health check
    async fn perform_health_check(
        health_monitor: &Arc<RwLock<HealthMonitor>>,
        consensus_client: &Arc<dyn ConsensusClient>,
        special_tx_service: &Arc<SpecialTransactionService>,
    ) {
        let mut monitor = health_monitor.write().await;
        monitor.last_check = SystemTime::now();
        
        // Check consensus
        match consensus_client.get_status().await {
            Ok(_) => {
                monitor.component_status.insert(
                    "consensus".to_string(),
                    ComponentHealth {
                        name: "consensus".to_string(),
                        status: HealthStatus::Healthy,
                        message: None,
                        last_check: chrono::Utc::now(),
                    }
                );
            }
            Err(e) => {
                monitor.component_status.insert(
                    "consensus".to_string(),
                    ComponentHealth {
                        name: "consensus".to_string(),
                        status: HealthStatus::Unhealthy,
                        message: Some(e.to_string()),
                        last_check: chrono::Utc::now(),
                    }
                );
            }
        }
        
        // Check special transaction service
        match special_tx_service.get_statistics().await {
            Ok(stats) => {
                let status = if stats.pending > 1000 {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Healthy
                };
                
                monitor.component_status.insert(
                    "special_transactions".to_string(),
                    ComponentHealth {
                        name: "special_transactions".to_string(),
                        status,
                        message: Some(format!("Pending: {}", stats.pending)),
                        last_check: chrono::Utc::now(),
                    }
                );
            }
            Err(e) => {
                monitor.component_status.insert(
                    "special_transactions".to_string(),
                    ComponentHealth {
                        name: "special_transactions".to_string(),
                        status: HealthStatus::Unhealthy,
                        message: Some(e.to_string()),
                        last_check: chrono::Utc::now(),
                    }
                );
            }
        }
    }

    /// Execute with retry logic
    async fn with_retry<F, T>(&self, operation: F) -> MultivmResult<T>
    where
        F: Fn() -> futures::future::BoxFuture<'static, MultivmResult<T>>,
    {
        let mut last_error = None;
        
        for attempt in 0..self.config.base.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(
                    self.config.base.retry_delay_ms * (attempt as u64)
                )).await;
            }

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ApplicationError::Internal("All retry attempts failed".to_string()).into()
        }))
    }
}

#[async_trait]
impl ProductionGateway for EnhancedProductionGateway {
    async fn get_block(&self, block_id: &str) -> MultivmResult<Option<MultiVMBlock>> {
        // Update metrics
        self.metrics_collector.record_request().await;
        
        // Check cache first
        if let Some(cache) = &self.cache {
            let cache_key = format!("block:{}", block_id);
            if let Some(cached) = cache.get::<MultiVMBlock>(&cache_key).await? {
                debug!("Block {} found in cache", block_id);
                self.metrics_collector.record_cache_hit().await;
                return Ok(Some(cached));
            }
            self.metrics_collector.record_cache_miss().await;
        }

        // Fetch from consensus
        let block = self.consensus_client.get_block(block_id).await?;
        
        // Cache the result
        if let (Some(cache), Some(ref block)) = (&self.cache, &block) {
            let cache_key = format!("block:{}", block_id);
            cache.set(&cache_key, block.clone()).await?;
        }

        Ok(block)
    }

    async fn get_latest_block(&self) -> MultivmResult<MultiVMBlock> {
        self.metrics_collector.record_request().await;
        self.consensus_client.get_latest_block().await
    }

    async fn submit_special_transaction(
        &self,
        transaction: SpecialTransaction,
    ) -> MultivmResult<TransactionResult> {
        self.metrics_collector.record_request().await;
        self.metrics_collector.record_special_tx_submitted().await;
        
        // Convert to common format
        let special_tx = match transaction {
            SpecialTransaction::AccountBinding { 
                source_chain, 
                source_address, 
                target_chain, 
                target_address, 
                proof 
            } => {
                multivm_common::SpecialTransaction {
                    transaction_type: multivm_common::SpecialTransactionType::AccountBinding {
                        account_a: self.parse_account_address(&source_chain, &source_address)?,
                        account_b: self.parse_account_address(&target_chain, &target_address)?,
                    },
                    nonce: self.generate_nonce(),
                    signature: None,
                }
            }
            SpecialTransaction::CrossVmTransfer {
                from_chain,
                from_address,
                to_chain,
                to_address,
                amount,
                asset,
            } => {
                multivm_common::SpecialTransaction {
                    transaction_type: multivm_common::SpecialTransactionType::CrossVMTransfer {
                        from: self.parse_account_address(&from_chain, &from_address)?,
                        to: self.parse_account_address(&to_chain, &to_address)?,
                        amount: amount.parse::<u128>()
                            .map_err(|_| ApplicationError::InvalidInput("Invalid amount".to_string()))?,
                    },
                    nonce: self.generate_nonce(),
                    signature: None,
                }
            }
            _ => return Err(ApplicationError::UnsupportedOperation(
                "Transaction type not supported".to_string()
            ).into()),
        };
        
        // Submit to special transaction service
        let tx_id = self.special_tx_service.submit_transaction(special_tx).await?;
        
        Ok(TransactionResult {
            transaction_id: tx_id,
            status: TransactionStatus::Pending,
            block_height: None,
            block_hash: None,
            timestamp: chrono::Utc::now(),
            estimated_confirmation_time: Some(Duration::from_secs(30)),
        })
    }

    async fn get_special_transaction(
        &self,
        tx_id: &str,
    ) -> MultivmResult<Option<SpecialTransactionInfo>> {
        self.metrics_collector.record_request().await;
        
        // Query from special transaction service
        match self.special_tx_service.get_transaction(tx_id).await? {
            Some(detailed_tx) => {
                Ok(Some(self.convert_to_special_tx_info(detailed_tx)))
            }
            None => Ok(None),
        }
    }

    async fn query_special_transactions(
        &self,
        query: SpecialTransactionQuery,
    ) -> MultivmResult<QueryResult<SpecialTransactionInfo>> {
        self.metrics_collector.record_request().await;
        
        // Build internal query
        let internal_query = self.query_processor.build_internal_query(query)?;
        
        // Execute query
        let transactions = self.special_tx_service.query_transactions(internal_query).await?;
        
        // Convert results
        let items: Vec<SpecialTransactionInfo> = transactions
            .into_iter()
            .map(|tx| self.convert_to_special_tx_info(tx))
            .collect();
        
        let total = items.len();
        let has_more = total > query.pagination.limit;
        
        Ok(QueryResult {
            items: items.into_iter().take(query.pagination.limit).collect(),
            total,
            offset: query.pagination.offset,
            limit: query.pagination.limit,
            has_more,
        })
    }

    async fn get_account_binding(
        &self,
        multivm_id: &str,
    ) -> MultivmResult<Option<AccountBinding>> {
        self.metrics_collector.record_request().await;
        self.account_mapping_client.get_binding(multivm_id).await
    }

    async fn get_consensus_status(&self) -> MultivmResult<ConsensusStatus> {
        self.metrics_collector.record_request().await;
        self.consensus_client.get_status().await
    }

    async fn get_system_health(&self) -> MultivmResult<SystemHealth> {
        self.metrics_collector.record_request().await;
        
        let monitor = self.health_monitor.read().await;
        let uptime = monitor.start_time.elapsed().as_secs();
        
        let components: Vec<ComponentHealth> = monitor.component_status
            .values()
            .cloned()
            .collect();
        
        let status = if components.iter().all(|c| c.status == HealthStatus::Healthy) {
            HealthStatus::Healthy
        } else if components.iter().any(|c| c.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };
        
        Ok(SystemHealth {
            status,
            components,
            uptime_seconds: uptime,
            last_check: chrono::Utc::now(),
        })
    }
}

impl EnhancedProductionGateway {
    /// Convert detailed transaction to API format
    fn convert_to_special_tx_info(&self, tx: DetailedSpecialTransaction) -> SpecialTransactionInfo {
        SpecialTransactionInfo {
            transaction_id: tx.id,
            transaction_type: format!("{:?}", tx.transaction_type),
            status: self.convert_tx_status(tx.status),
            block_height: tx.execution.as_ref().map(|e| e.block_number),
            created_at: chrono::DateTime::from(tx.metadata.created_at),
            updated_at: chrono::DateTime::from(tx.metadata.updated_at),
            details: serde_json::json!({
                "source": tx.source,
                "destination": tx.destination,
                "amount": tx.amount,
                "metadata": tx.metadata,
            }),
            gas_used: tx.execution.as_ref().map(|e| e.gas_used),
            fee_paid: tx.execution.as_ref().map(|e| e.fee_paid),
            confirmations: tx.execution.as_ref().map(|e| e.confirmations as u32).unwrap_or(0),
        }
    }
    
    /// Convert transaction status
    fn convert_tx_status(&self, status: SpecialTxStatus) -> TransactionStatus {
        match status {
            SpecialTxStatus::Pending => TransactionStatus::Pending,
            SpecialTxStatus::Processing => TransactionStatus::Pending,
            SpecialTxStatus::Confirmed => TransactionStatus::Confirmed,
            SpecialTxStatus::Failed => TransactionStatus::Failed,
            SpecialTxStatus::Cancelled => TransactionStatus::Failed,
        }
    }
    
    /// Parse account address from chain and address string
    fn parse_account_address(
        &self,
        chain: &str,
        address: &str,
    ) -> MultivmResult<multivm_common::AccountAddress> {
        match chain.to_lowercase().as_str() {
            "ethereum" | "eth" | "evm" => {
                // Parse Ethereum address
                let addr_bytes = hex::decode(address.trim_start_matches("0x"))
                    .map_err(|_| ApplicationError::InvalidInput("Invalid Ethereum address".to_string()))?;
                if addr_bytes.len() != 20 {
                    return Err(ApplicationError::InvalidInput("Invalid Ethereum address length".to_string()).into());
                }
                let mut eth_addr = [0u8; 20];
                eth_addr.copy_from_slice(&addr_bytes);
                Ok(multivm_common::AccountAddress::Ethereum(
                    multivm_common::EthereumAddress(eth_addr)
                ))
            }
            "solana" | "sol" | "svm" => {
                // Parse Solana address
                let addr_bytes = bs58::decode(address)
                    .into_vec()
                    .map_err(|_| ApplicationError::InvalidInput("Invalid Solana address".to_string()))?;
                if addr_bytes.len() != 32 {
                    return Err(ApplicationError::InvalidInput("Invalid Solana address length".to_string()).into());
                }
                let mut sol_addr = [0u8; 32];
                sol_addr.copy_from_slice(&addr_bytes);
                Ok(multivm_common::AccountAddress::Solana(
                    multivm_common::SolanaAddress(sol_addr)
                ))
            }
            _ => Err(ApplicationError::InvalidInput(
                format!("Unsupported chain: {}", chain)
            ).into()),
        }
    }
    
    /// Generate nonce
    fn generate_nonce(&self) -> u64 {
        use std::time::UNIX_EPOCH;
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}

impl QueryProcessor {
    /// Build internal query from API query
    fn build_internal_query(
        &self,
        query: SpecialTransactionQuery,
    ) -> MultivmResult<TransactionQuery> {
        let sort_by = match query.sort.field.as_str() {
            "created_at" => SortBy::CreatedAt(query.sort.order),
            "updated_at" => SortBy::UpdatedAt(query.sort.order),
            "amount" => SortBy::Amount(query.sort.order),
            "block_number" => SortBy::BlockNumber(query.sort.order),
            _ => SortBy::CreatedAt(SortOrder::Descending),
        };
        
        Ok(TransactionQuery {
            account: query.account.and_then(|addr| {
                // Parse account address - simplified for now
                None
            }),
            status: query.status.map(|s| match s {
                TransactionStatus::Pending => SpecialTxStatus::Pending,
                TransactionStatus::Confirmed => SpecialTxStatus::Confirmed,
                TransactionStatus::Failed => SpecialTxStatus::Failed,
            }),
            transaction_type: None, // Would map from string
            block_range: None,
            time_range: query.time_range.map(|tr| {
                (tr.start.into(), tr.end.into())
            }),
            sort_by,
            offset: query.pagination.offset,
            limit: query.pagination.limit.min(self.config.max_limit),
        })
    }
}

impl MetricsCollector {
    async fn record_request(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
    }
    
    async fn record_cache_hit(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_hits += 1;
    }
    
    async fn record_cache_miss(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_misses += 1;
    }
    
    async fn record_special_tx_submitted(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.special_tx_submitted += 1;
    }
    
    async fn collect_and_push_metrics(&self) {
        let metrics = self.metrics.read().await;
        info!("Gateway metrics - Requests: {}, Success: {}, Cache Hit Rate: {:.2}%",
              metrics.total_requests,
              metrics.successful_requests,
              if metrics.cache_hits + metrics.cache_misses > 0 {
                  (metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64) * 100.0
              } else {
                  0.0
              }
        );
    }
}

impl Default for EnhancedGatewayConfig {
    fn default() -> Self {
        Self {
            base: ProductionGatewayConfig {
                cache_enabled: true,
                cache_ttl_seconds: 300,
                max_retries: 3,
                retry_delay_ms: 1000,
                request_timeout_ms: 30000,
                health_check_interval_ms: 60000,
            },
            special_tx_config: SpecialTransactionServiceConfig {
                redis_url: "redis://127.0.0.1:6379".to_string(),
                enable_indexing: true,
                pool_sync_interval: Duration::from_secs(30),
                max_pending_transactions: 10000,
            },
            query_config: QueryConfig {
                default_limit: 50,
                max_limit: 1000,
                query_timeout: Duration::from_secs(10),
                enable_query_cache: true,
            },
            monitoring_config: MonitoringConfig {
                enable_metrics: true,
                metrics_interval: Duration::from_secs(60),
                enable_tracing: true,
                alert_thresholds: AlertThresholds {
                    max_pending_transactions: 5000,
                    max_processing_time: Duration::from_secs(300),
                    min_success_rate: 0.95,
                },
            },
        }
    }
}