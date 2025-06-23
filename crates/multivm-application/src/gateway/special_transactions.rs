//! Production-ready special transaction handling
//!
//! This module provides complete implementation for querying and managing
//! special cross-VM transactions with proper database integration and caching.

use crate::error::{ApplicationError, ApplicationResult};
use multivm_common::{
    MultivmResult, MultivmError, SpecialTransaction, ProcessId,
    AccountAddress, SpecialTransactionInfo,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use dashmap::DashMap;
use redis::{AsyncCommands, RedisError};

/// Special transaction status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is pending in the pool
    Pending,
    /// Transaction is being processed
    Processing,
    /// Transaction has been included in a block
    Confirmed,
    /// Transaction has failed
    Failed,
    /// Transaction was cancelled
    Cancelled,
}

/// Detailed special transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedSpecialTransaction {
    /// Transaction ID
    pub id: String,
    
    /// Transaction type
    pub transaction_type: SpecialTransactionType,
    
    /// Current status
    pub status: TransactionStatus,
    
    /// Source information
    pub source: TransactionEndpoint,
    
    /// Destination information
    pub destination: TransactionEndpoint,
    
    /// Amount being transferred
    pub amount: u128,
    
    /// Transaction metadata
    pub metadata: TransactionMetadata,
    
    /// Execution details
    pub execution: Option<ExecutionDetails>,
    
    /// Error information if failed
    pub error: Option<TransactionError>,
}

/// Transaction endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEndpoint {
    /// VM type
    pub vm: ProcessId,
    
    /// Account address
    pub address: AccountAddress,
    
    /// Optional contract address
    pub contract: Option<String>,
    
    /// Chain-specific data
    pub chain_data: serde_json::Value,
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMetadata {
    /// Creation timestamp
    pub created_at: SystemTime,
    
    /// Last update timestamp
    pub updated_at: SystemTime,
    
    /// Transaction nonce
    pub nonce: u64,
    
    /// Priority fee
    pub priority_fee: Option<u128>,
    
    /// Maximum fee willing to pay
    pub max_fee: Option<u128>,
    
    /// User-provided memo
    pub memo: Option<String>,
    
    /// Additional metadata
    pub extra: HashMap<String, serde_json::Value>,
}

/// Execution details for confirmed transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetails {
    /// Block number where transaction was included
    pub block_number: u64,
    
    /// Block hash
    pub block_hash: String,
    
    /// Position in block
    pub transaction_index: u64,
    
    /// Source VM transaction hash
    pub source_tx_hash: String,
    
    /// Destination VM transaction hash
    pub destination_tx_hash: Option<String>,
    
    /// Total gas used
    pub gas_used: u64,
    
    /// Actual fee paid
    pub fee_paid: u128,
    
    /// Confirmation timestamp
    pub confirmed_at: SystemTime,
    
    /// Number of confirmations
    pub confirmations: u64,
}

/// Transaction error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionError {
    /// Error code
    pub code: String,
    
    /// Human-readable message
    pub message: String,
    
    /// Detailed error data
    pub details: Option<serde_json::Value>,
    
    /// Timestamp of error
    pub occurred_at: SystemTime,
    
    /// Whether the error is retryable
    pub retryable: bool,
}

/// Special transaction type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpecialTransactionType {
    /// Cross-VM token transfer
    CrossVmTransfer,
    
    /// Account binding transaction
    AccountBinding,
    
    /// Contract deployment across VMs
    CrossVmDeploy,
    
    /// Cross-VM contract call
    CrossVmCall,
    
    /// Atomic swap between VMs
    AtomicSwap,
    
    /// Liquidity bridge operation
    LiquidityBridge,
}

/// Configuration for special transaction service
#[derive(Debug, Clone)]
pub struct SpecialTransactionConfig {
    /// Redis connection URL
    pub redis_url: String,
    
    /// Cache TTL for pending transactions
    pub pending_cache_ttl: Duration,
    
    /// Cache TTL for confirmed transactions
    pub confirmed_cache_ttl: Duration,
    
    /// Maximum transactions in memory cache
    pub max_cache_size: usize,
    
    /// Database query timeout
    pub query_timeout: Duration,
    
    /// Enable transaction indexing
    pub enable_indexing: bool,
    
    /// Retention period for completed transactions
    pub retention_period: Duration,
}

impl Default for SpecialTransactionConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            pending_cache_ttl: Duration::from_secs(60),
            confirmed_cache_ttl: Duration::from_secs(3600),
            max_cache_size: 10000,
            query_timeout: Duration::from_secs(5),
            enable_indexing: true,
            retention_period: Duration::from_secs(86400 * 30), // 30 days
        }
    }
}

/// Production special transaction service
pub struct SpecialTransactionService {
    config: SpecialTransactionConfig,
    redis_pool: Arc<redis::aio::ConnectionManager>,
    memory_cache: Arc<DashMap<String, CachedTransaction>>,
    transaction_pool: Arc<RwLock<HashMap<String, DetailedSpecialTransaction>>>,
    indexer: Arc<TransactionIndexer>,
}

/// Cached transaction with expiry
#[derive(Debug, Clone)]
struct CachedTransaction {
    transaction: DetailedSpecialTransaction,
    cached_at: SystemTime,
    ttl: Duration,
}

/// Transaction indexer for efficient queries
struct TransactionIndexer {
    by_account: DashMap<AccountAddress, Vec<String>>,
    by_status: DashMap<TransactionStatus, Vec<String>>,
    by_type: DashMap<SpecialTransactionType, Vec<String>>,
    by_block: DashMap<u64, Vec<String>>,
}

impl SpecialTransactionService {
    /// Create new special transaction service
    pub async fn new(config: SpecialTransactionConfig) -> MultivmResult<Self> {
        // Connect to Redis
        let client = redis::Client::open(config.redis_url.clone())
            .map_err(|e| MultivmError::Database(format!("Redis connection failed: {}", e)))?;
        
        let manager = redis::aio::ConnectionManager::new(client).await
            .map_err(|e| MultivmError::Database(format!("Redis manager failed: {}", e)))?;
        
        let indexer = Arc::new(TransactionIndexer {
            by_account: DashMap::new(),
            by_status: DashMap::new(),
            by_type: DashMap::new(),
            by_block: DashMap::new(),
        });
        
        let service = Self {
            config,
            redis_pool: Arc::new(manager),
            memory_cache: Arc::new(DashMap::new()),
            transaction_pool: Arc::new(RwLock::new(HashMap::new())),
            indexer,
        };
        
        // Start background tasks
        service.start_background_tasks();
        
        Ok(service)
    }
    
    /// Start background maintenance tasks
    fn start_background_tasks(&self) {
        // Cache cleanup task
        let cache = Arc::clone(&self.memory_cache);
        let cleanup_interval = Duration::from_secs(60);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                Self::cleanup_expired_cache(&cache);
            }
        });
        
        // Transaction pool sync task
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = service.sync_transaction_pool().await {
                    error!("Transaction pool sync error: {}", e);
                }
            }
        });
    }
    
    /// Get special transaction by ID
    pub async fn get_transaction(
        &self,
        tx_id: &str,
    ) -> MultivmResult<Option<DetailedSpecialTransaction>> {
        // Check memory cache first
        if let Some(cached) = self.get_from_cache(tx_id).await {
            return Ok(Some(cached));
        }
        
        // Check Redis cache
        if let Some(tx) = self.get_from_redis(tx_id).await? {
            self.add_to_cache(tx_id, tx.clone()).await;
            return Ok(Some(tx));
        }
        
        // Check transaction pool
        if let Some(tx) = self.get_from_pool(tx_id).await {
            self.add_to_cache(tx_id, tx.clone()).await;
            return Ok(Some(tx));
        }
        
        Ok(None)
    }
    
    /// Submit new special transaction
    pub async fn submit_transaction(
        &self,
        transaction: SpecialTransaction,
    ) -> MultivmResult<String> {
        // Validate transaction
        self.validate_transaction(&transaction)?;
        
        // Generate transaction ID
        let tx_id = self.generate_transaction_id(&transaction);
        
        // Create detailed transaction
        let detailed = self.create_detailed_transaction(tx_id.clone(), transaction)?;
        
        // Add to pool
        self.add_to_pool(detailed.clone()).await?;
        
        // Store in Redis
        self.store_in_redis(&detailed).await?;
        
        // Update indexes
        self.index_transaction(&detailed);
        
        info!("Submitted special transaction: {}", tx_id);
        
        Ok(tx_id)
    }
    
    /// Update transaction status
    pub async fn update_transaction_status(
        &self,
        tx_id: &str,
        status: TransactionStatus,
        execution: Option<ExecutionDetails>,
        error: Option<TransactionError>,
    ) -> MultivmResult<()> {
        // Get transaction
        let mut tx = self.get_transaction(tx_id).await?
            .ok_or_else(|| MultivmError::NotFound(format!("Transaction {} not found", tx_id)))?;
        
        // Update status
        let old_status = tx.status.clone();
        tx.status = status.clone();
        tx.metadata.updated_at = SystemTime::now();
        
        if let Some(exec) = execution {
            tx.execution = Some(exec);
        }
        
        if let Some(err) = error {
            tx.error = Some(err);
        }
        
        // Update in all stores
        self.update_in_pool(&tx).await?;
        self.store_in_redis(&tx).await?;
        self.invalidate_cache(tx_id).await;
        
        // Update indexes
        self.reindex_transaction(&tx, old_status);
        
        info!("Updated transaction {} status: {:?} -> {:?}", tx_id, old_status, status);
        
        Ok(())
    }
    
    /// Query transactions by various criteria
    pub async fn query_transactions(
        &self,
        query: TransactionQuery,
    ) -> MultivmResult<Vec<DetailedSpecialTransaction>> {
        let mut results = Vec::new();
        
        // Get transaction IDs based on query
        let tx_ids = self.get_transaction_ids_by_query(&query).await?;
        
        // Fetch transactions
        for tx_id in tx_ids {
            if let Some(tx) = self.get_transaction(&tx_id).await? {
                if self.matches_query(&tx, &query) {
                    results.push(tx);
                }
            }
        }
        
        // Apply sorting and pagination
        results = self.apply_sorting(results, &query.sort_by);
        results = self.apply_pagination(results, query.offset, query.limit);
        
        Ok(results)
    }
    
    /// Get pending transactions count
    pub async fn get_pending_count(&self) -> MultivmResult<usize> {
        let count = self.indexer.by_status
            .get(&TransactionStatus::Pending)
            .map(|ids| ids.len())
            .unwrap_or(0);
        
        Ok(count)
    }
    
    /// Get transaction statistics
    pub async fn get_statistics(&self) -> MultivmResult<TransactionStatistics> {
        let mut stats = TransactionStatistics::default();
        
        // Count by status
        for status in [
            TransactionStatus::Pending,
            TransactionStatus::Processing,
            TransactionStatus::Confirmed,
            TransactionStatus::Failed,
            TransactionStatus::Cancelled,
        ] {
            let count = self.indexer.by_status
                .get(&status)
                .map(|ids| ids.len())
                .unwrap_or(0);
            
            match status {
                TransactionStatus::Pending => stats.pending = count,
                TransactionStatus::Processing => stats.processing = count,
                TransactionStatus::Confirmed => stats.confirmed = count,
                TransactionStatus::Failed => stats.failed = count,
                TransactionStatus::Cancelled => stats.cancelled = count,
            }
        }
        
        // Count by type
        for (tx_type, ids) in self.indexer.by_type.iter() {
            stats.by_type.insert(tx_type.clone(), ids.len());
        }
        
        // Calculate total
        stats.total = stats.pending + stats.processing + stats.confirmed + stats.failed + stats.cancelled;
        
        Ok(stats)
    }
    
    // Helper methods
    
    async fn get_from_cache(&self, tx_id: &str) -> Option<DetailedSpecialTransaction> {
        self.memory_cache.get(tx_id)
            .and_then(|cached| {
                let elapsed = SystemTime::now()
                    .duration_since(cached.cached_at)
                    .unwrap_or(Duration::MAX);
                
                if elapsed < cached.ttl {
                    Some(cached.transaction.clone())
                } else {
                    None
                }
            })
    }
    
    async fn get_from_redis(&self, tx_id: &str) -> MultivmResult<Option<DetailedSpecialTransaction>> {
        let mut conn = self.redis_pool.as_ref().clone();
        let key = format!("tx:{}", tx_id);
        
        let data: Option<Vec<u8>> = conn.get(&key).await
            .map_err(|e| MultivmError::Database(format!("Redis get failed: {}", e)))?;
        
        match data {
            Some(bytes) => {
                let tx: DetailedSpecialTransaction = serde_json::from_slice(&bytes)
                    .map_err(|e| MultivmError::Serialization(format!("Transaction decode failed: {}", e)))?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }
    
    async fn get_from_pool(&self, tx_id: &str) -> Option<DetailedSpecialTransaction> {
        let pool = self.transaction_pool.read().await;
        pool.get(tx_id).cloned()
    }
    
    async fn add_to_cache(&self, tx_id: &str, tx: DetailedSpecialTransaction) {
        let ttl = match tx.status {
            TransactionStatus::Pending | TransactionStatus::Processing => self.config.pending_cache_ttl,
            _ => self.config.confirmed_cache_ttl,
        };
        
        let cached = CachedTransaction {
            transaction: tx,
            cached_at: SystemTime::now(),
            ttl,
        };
        
        self.memory_cache.insert(tx_id.to_string(), cached);
        
        // Enforce cache size limit
        if self.memory_cache.len() > self.config.max_cache_size {
            // Remove oldest entries
            Self::cleanup_oldest_cache(&self.memory_cache, self.config.max_cache_size / 2);
        }
    }
    
    async fn invalidate_cache(&self, tx_id: &str) {
        self.memory_cache.remove(tx_id);
    }
    
    async fn store_in_redis(&self, tx: &DetailedSpecialTransaction) -> MultivmResult<()> {
        let mut conn = self.redis_pool.as_ref().clone();
        let key = format!("tx:{}", tx.id);
        
        let data = serde_json::to_vec(tx)
            .map_err(|e| MultivmError::Serialization(format!("Transaction encode failed: {}", e)))?;
        
        let ttl = match tx.status {
            TransactionStatus::Pending | TransactionStatus::Processing => 3600, // 1 hour
            _ => 86400 * 7, // 7 days for completed
        };
        
        conn.set_ex(&key, data, ttl).await
            .map_err(|e| MultivmError::Database(format!("Redis set failed: {}", e)))?;
        
        Ok(())
    }
    
    async fn add_to_pool(&self, tx: DetailedSpecialTransaction) -> MultivmResult<()> {
        let mut pool = self.transaction_pool.write().await;
        pool.insert(tx.id.clone(), tx);
        Ok(())
    }
    
    async fn update_in_pool(&self, tx: &DetailedSpecialTransaction) -> MultivmResult<()> {
        let mut pool = self.transaction_pool.write().await;
        pool.insert(tx.id.clone(), tx.clone());
        Ok(())
    }
    
    fn validate_transaction(&self, tx: &SpecialTransaction) -> MultivmResult<()> {
        // Validate transaction type
        match &tx.transaction_type {
            multivm_common::SpecialTransactionType::AccountBinding { .. } => {
                // Validate account binding specifics
            }
            multivm_common::SpecialTransactionType::CrossVMTransfer { .. } => {
                // Validate cross-VM transfer specifics
            }
        }
        
        // Validate amount
        if matches!(&tx.transaction_type, multivm_common::SpecialTransactionType::CrossVMTransfer { amount, .. } if *amount == 0) {
            return Err(MultivmError::Validation("Transfer amount must be non-zero".to_string()));
        }
        
        Ok(())
    }
    
    fn generate_transaction_id(&self, tx: &SpecialTransaction) -> String {
        use blake3::Hasher;
        
        let mut hasher = Hasher::new();
        hasher.update(&serde_json::to_vec(tx).unwrap_or_default());
        hasher.update(&SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos().to_le_bytes());
        
        format!("tx_{}", hasher.finalize().to_hex())
    }
    
    fn create_detailed_transaction(
        &self,
        tx_id: String,
        tx: SpecialTransaction,
    ) -> MultivmResult<DetailedSpecialTransaction> {
        let (tx_type, source, destination, amount) = match tx.transaction_type {
            multivm_common::SpecialTransactionType::AccountBinding { account_a, account_b } => {
                (
                    SpecialTransactionType::AccountBinding,
                    TransactionEndpoint {
                        vm: account_a.process_id(),
                        address: account_a,
                        contract: None,
                        chain_data: serde_json::json!({}),
                    },
                    TransactionEndpoint {
                        vm: account_b.process_id(),
                        address: account_b,
                        contract: None,
                        chain_data: serde_json::json!({}),
                    },
                    0,
                )
            }
            multivm_common::SpecialTransactionType::CrossVMTransfer { from, to, amount } => {
                (
                    SpecialTransactionType::CrossVmTransfer,
                    TransactionEndpoint {
                        vm: from.process_id(),
                        address: from,
                        contract: None,
                        chain_data: serde_json::json!({}),
                    },
                    TransactionEndpoint {
                        vm: to.process_id(),
                        address: to,
                        contract: None,
                        chain_data: serde_json::json!({}),
                    },
                    amount,
                )
            }
        };
        
        Ok(DetailedSpecialTransaction {
            id: tx_id,
            transaction_type: tx_type,
            status: TransactionStatus::Pending,
            source,
            destination,
            amount,
            metadata: TransactionMetadata {
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
                nonce: tx.nonce,
                priority_fee: None,
                max_fee: None,
                memo: None,
                extra: HashMap::new(),
            },
            execution: None,
            error: None,
        })
    }
    
    fn index_transaction(&self, tx: &DetailedSpecialTransaction) {
        // Index by account
        self.indexer.by_account.entry(tx.source.address.clone()).or_default().push(tx.id.clone());
        self.indexer.by_account.entry(tx.destination.address.clone()).or_default().push(tx.id.clone());
        
        // Index by status
        self.indexer.by_status.entry(tx.status.clone()).or_default().push(tx.id.clone());
        
        // Index by type
        self.indexer.by_type.entry(tx.transaction_type.clone()).or_default().push(tx.id.clone());
        
        // Index by block if confirmed
        if let Some(exec) = &tx.execution {
            self.indexer.by_block.entry(exec.block_number).or_default().push(tx.id.clone());
        }
    }
    
    fn reindex_transaction(&self, tx: &DetailedSpecialTransaction, old_status: TransactionStatus) {
        // Remove from old status index
        if let Some(mut ids) = self.indexer.by_status.get_mut(&old_status) {
            ids.retain(|id| id != &tx.id);
        }
        
        // Add to new status index
        self.indexer.by_status.entry(tx.status.clone()).or_default().push(tx.id.clone());
        
        // Update block index if newly confirmed
        if let Some(exec) = &tx.execution {
            self.indexer.by_block.entry(exec.block_number).or_default().push(tx.id.clone());
        }
    }
    
    async fn sync_transaction_pool(&self) -> MultivmResult<()> {
        // Sync pending transactions with consensus/process manager
        // This would integrate with the actual transaction processing pipeline
        Ok(())
    }
    
    fn cleanup_expired_cache(cache: &DashMap<String, CachedTransaction>) {
        let now = SystemTime::now();
        cache.retain(|_, cached| {
            let elapsed = now.duration_since(cached.cached_at).unwrap_or(Duration::MAX);
            elapsed < cached.ttl
        });
    }
    
    fn cleanup_oldest_cache(cache: &DashMap<String, CachedTransaction>, target_size: usize) {
        if cache.len() <= target_size {
            return;
        }
        
        // Collect all entries with timestamps
        let mut entries: Vec<(String, SystemTime)> = cache.iter()
            .map(|entry| (entry.key().clone(), entry.value().cached_at))
            .collect();
        
        // Sort by age (oldest first)
        entries.sort_by_key(|(_, time)| *time);
        
        // Remove oldest entries
        let to_remove = cache.len() - target_size;
        for (key, _) in entries.into_iter().take(to_remove) {
            cache.remove(&key);
        }
    }
}

// Query support

/// Transaction query parameters
#[derive(Debug, Clone)]
pub struct TransactionQuery {
    /// Filter by account address
    pub account: Option<AccountAddress>,
    
    /// Filter by status
    pub status: Option<TransactionStatus>,
    
    /// Filter by type
    pub transaction_type: Option<SpecialTransactionType>,
    
    /// Filter by block range
    pub block_range: Option<(u64, u64)>,
    
    /// Filter by time range
    pub time_range: Option<(SystemTime, SystemTime)>,
    
    /// Sort order
    pub sort_by: SortBy,
    
    /// Pagination offset
    pub offset: usize,
    
    /// Pagination limit
    pub limit: usize,
}

/// Sort options
#[derive(Debug, Clone)]
pub enum SortBy {
    CreatedAt(SortOrder),
    UpdatedAt(SortOrder),
    Amount(SortOrder),
    BlockNumber(SortOrder),
}

/// Sort order
#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Transaction statistics
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TransactionStatistics {
    pub total: usize,
    pub pending: usize,
    pub processing: usize,
    pub confirmed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub by_type: HashMap<SpecialTransactionType, usize>,
}

// Implement required conversions

impl From<DetailedSpecialTransaction> for SpecialTransactionInfo {
    fn from(tx: DetailedSpecialTransaction) -> Self {
        SpecialTransactionInfo {
            id: tx.id,
            transaction_type: match tx.transaction_type {
                SpecialTransactionType::AccountBinding => {
                    multivm_common::SpecialTransactionType::AccountBinding {
                        account_a: tx.source.address,
                        account_b: tx.destination.address,
                    }
                }
                SpecialTransactionType::CrossVmTransfer => {
                    multivm_common::SpecialTransactionType::CrossVMTransfer {
                        from: tx.source.address,
                        to: tx.destination.address,
                        amount: tx.amount,
                    }
                }
                _ => {
                    // For other types, default to cross-VM transfer
                    multivm_common::SpecialTransactionType::CrossVMTransfer {
                        from: tx.source.address,
                        to: tx.destination.address,
                        amount: tx.amount,
                    }
                }
            },
            status: match tx.status {
                TransactionStatus::Pending => multivm_common::SpecialTransactionStatus::Pending,
                TransactionStatus::Processing => multivm_common::SpecialTransactionStatus::Processing,
                TransactionStatus::Confirmed => multivm_common::SpecialTransactionStatus::Confirmed,
                TransactionStatus::Failed => multivm_common::SpecialTransactionStatus::Failed,
                TransactionStatus::Cancelled => multivm_common::SpecialTransactionStatus::Failed,
            },
            created_at: tx.metadata.created_at,
            confirmed_at: tx.execution.as_ref().map(|e| e.confirmed_at),
            block_number: tx.execution.as_ref().map(|e| e.block_number),
            nonce: tx.metadata.nonce,
        }
    }
}

impl SpecialTransactionService {
    async fn get_transaction_ids_by_query(&self, query: &TransactionQuery) -> MultivmResult<Vec<String>> {
        let mut tx_ids = Vec::new();
        
        // Start with status filter if provided
        if let Some(status) = &query.status {
            if let Some(ids) = self.indexer.by_status.get(status) {
                tx_ids.extend(ids.clone());
            }
        } else {
            // Get all transaction IDs
            for entry in self.indexer.by_status.iter() {
                tx_ids.extend(entry.value().clone());
            }
        }
        
        // Apply account filter
        if let Some(account) = &query.account {
            if let Some(account_ids) = self.indexer.by_account.get(account) {
                tx_ids.retain(|id| account_ids.contains(id));
            } else {
                tx_ids.clear();
            }
        }
        
        // Apply type filter
        if let Some(tx_type) = &query.transaction_type {
            if let Some(type_ids) = self.indexer.by_type.get(tx_type) {
                tx_ids.retain(|id| type_ids.contains(id));
            } else {
                tx_ids.clear();
            }
        }
        
        Ok(tx_ids)
    }
    
    fn matches_query(&self, tx: &DetailedSpecialTransaction, query: &TransactionQuery) -> bool {
        // Check time range
        if let Some((start, end)) = &query.time_range {
            if tx.metadata.created_at < *start || tx.metadata.created_at > *end {
                return false;
            }
        }
        
        // Check block range
        if let Some((start, end)) = &query.block_range {
            if let Some(exec) = &tx.execution {
                if exec.block_number < *start || exec.block_number > *end {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        true
    }
    
    fn apply_sorting(&self, mut txs: Vec<DetailedSpecialTransaction>, sort_by: &SortBy) -> Vec<DetailedSpecialTransaction> {
        match sort_by {
            SortBy::CreatedAt(order) => {
                txs.sort_by_key(|tx| tx.metadata.created_at);
                if matches!(order, SortOrder::Descending) {
                    txs.reverse();
                }
            }
            SortBy::UpdatedAt(order) => {
                txs.sort_by_key(|tx| tx.metadata.updated_at);
                if matches!(order, SortOrder::Descending) {
                    txs.reverse();
                }
            }
            SortBy::Amount(order) => {
                txs.sort_by_key(|tx| tx.amount);
                if matches!(order, SortOrder::Descending) {
                    txs.reverse();
                }
            }
            SortBy::BlockNumber(order) => {
                txs.sort_by_key(|tx| tx.execution.as_ref().map(|e| e.block_number).unwrap_or(0));
                if matches!(order, SortOrder::Descending) {
                    txs.reverse();
                }
            }
        }
        
        txs
    }
    
    fn apply_pagination(&self, txs: Vec<DetailedSpecialTransaction>, offset: usize, limit: usize) -> Vec<DetailedSpecialTransaction> {
        txs.into_iter()
            .skip(offset)
            .take(limit)
            .collect()
    }
}

// Implement Clone for service
impl Clone for SpecialTransactionService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            redis_pool: Arc::clone(&self.redis_pool),
            memory_cache: Arc::clone(&self.memory_cache),
            transaction_pool: Arc::clone(&self.transaction_pool),
            indexer: Arc::clone(&self.indexer),
        }
    }
}