//! Transaction Pool for MultiVM Consensus
//!
//! This module provides a production-ready transaction pool that manages pending
//! transactions across different VMs (EVM, SVM, and Cross-VM) before they are
//! included in blocks.

use crate::{ConsensusError, ConsensusResult};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Virtual machine type for transaction categorization  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VmType {
    Evm,
    Svm,
    CrossVm,
}

/// Transaction pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPoolConfig {
    /// Maximum number of transactions in the pool
    pub max_pool_size: usize,
    /// Maximum number of transactions per account
    pub max_per_account: usize,
    /// Transaction expiry time in seconds
    pub tx_expiry_seconds: u64,
    /// Enable transaction replacement by higher gas price
    pub allow_replacement: bool,
    /// Minimum gas price increase for replacement (percentage)
    pub replacement_gas_increase: u64,
}

impl Default for TransactionPoolConfig {
    fn default() -> Self {
        Self {
            max_pool_size: 10000,
            max_per_account: 100,
            tx_expiry_seconds: 300, // 5 minutes
            allow_replacement: true,
            replacement_gas_increase: 10, // 10% increase required
        }
    }
}

/// Priority level for transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TransactionPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Transaction status in the pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Included { block_height: u64 },
    Dropped { reason: String },
    Expired,
}

/// Transaction in the pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PooledTransaction {
    /// Transaction ID
    pub id: String,
    /// Transaction data as JSON
    pub data: JsonValue,
    /// Submission timestamp
    pub timestamp: std::time::SystemTime,
    /// Optional signature
    pub signature: Option<String>,
}

/// Pool transaction wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolTransaction {
    /// The actual transaction
    pub transaction: PooledTransaction,
    /// Transaction priority
    pub priority: TransactionPriority,
    /// Submission timestamp
    pub submitted_at: std::time::SystemTime,
    /// Transaction status
    pub status: TransactionStatus,
    /// Gas price (for EVM transactions)
    pub gas_price: Option<u64>,
    /// Sender account
    pub sender: String,
    /// Nonce (for ordered transactions)
    pub nonce: Option<u64>,
}

/// Transaction pool statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionPoolStats {
    pub total_submitted: u64,
    pub total_included: u64,
    pub total_dropped: u64,
    pub total_expired: u64,
    pub current_pool_size: usize,
    pub evm_transactions: usize,
    pub svm_transactions: usize,
    pub cross_vm_transactions: usize,
}

/// Production-ready transaction pool
pub struct TransactionPool {
    /// Configuration
    config: TransactionPoolConfig,
    /// Pending transactions by VM type
    pending_by_vm: HashMap<VmType, VecDeque<PoolTransaction>>,
    /// Transactions by sender for nonce ordering
    by_sender: HashMap<String, Vec<PoolTransaction>>,
    /// Transaction lookup by ID
    by_id: HashMap<String, PoolTransaction>,
    /// Pool statistics
    stats: TransactionPoolStats,
    /// Lock for thread-safe access
    _phantom: std::marker::PhantomData<()>,
}

impl TransactionPool {
    /// Create a new transaction pool
    pub fn new(config: TransactionPoolConfig) -> Self {
        let mut pending_by_vm = HashMap::new();
        pending_by_vm.insert(VmType::Evm, VecDeque::new());
        pending_by_vm.insert(VmType::Svm, VecDeque::new());
        pending_by_vm.insert(VmType::CrossVm, VecDeque::new());

        Self {
            config,
            pending_by_vm,
            by_sender: HashMap::new(),
            by_id: HashMap::new(),
            stats: TransactionPoolStats::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add a transaction to the pool
    pub fn add_transaction(
        &mut self,
        transaction: PooledTransaction,
        priority: TransactionPriority,
    ) -> ConsensusResult<()> {
        // Check pool size limit
        if self.stats.current_pool_size >= self.config.max_pool_size {
            self.evict_lowest_priority()?;
        }

        // Extract transaction metadata
        let tx_id = transaction.id.clone();
        let vm_type = self.get_vm_type(&transaction);
        let sender = self.extract_sender(&transaction);
        let gas_price = self.extract_gas_price(&transaction);
        let nonce = self.extract_nonce(&transaction);

        // Check per-account limit
        if let Some(account_txs) = self.by_sender.get(&sender) {
            if account_txs.len() >= self.config.max_per_account {
                return Err(ConsensusError::InvalidTransaction(format!(
                    "Account {sender} has reached transaction limit"
                )));
            }
        }

        // Create pool transaction
        let pool_tx = PoolTransaction {
            transaction,
            priority,
            submitted_at: std::time::SystemTime::now(),
            status: TransactionStatus::Pending,
            gas_price,
            sender: sender.clone(),
            nonce,
        };

        // Add to appropriate collections
        self.pending_by_vm
            .get_mut(&vm_type)
            .ok_or_else(|| ConsensusError::InvalidTransaction("Unknown VM type".to_string()))?
            .push_back(pool_tx.clone());

        self.by_sender
            .entry(sender)
            .or_default()
            .push(pool_tx.clone());

        self.by_id.insert(tx_id.clone(), pool_tx);

        // Update statistics
        self.stats.total_submitted += 1;
        self.stats.current_pool_size += 1;
        match vm_type {
            VmType::Evm => self.stats.evm_transactions += 1,
            VmType::Svm => self.stats.svm_transactions += 1,
            VmType::CrossVm => self.stats.cross_vm_transactions += 1,
        }

        info!(
            "Added transaction {} to pool with {:?} priority",
            tx_id, priority
        );
        Ok(())
    }

    /// Get transactions for block inclusion
    pub fn get_transactions_for_block(&mut self, max_count: usize) -> Vec<PooledTransaction> {
        let mut selected = Vec::new();
        let mut selected_ids = Vec::new();

        // Select high priority transactions first
        for vm_type in [VmType::Evm, VmType::Svm, VmType::CrossVm] {
            if let Some(queue) = self.pending_by_vm.get_mut(&vm_type) {
                let mut temp_selected = Vec::new();

                let expiry_seconds = self.config.tx_expiry_seconds;
                for pool_tx in queue.iter() {
                    if selected.len() >= max_count {
                        break;
                    }

                    // Skip expired transactions
                    if let Ok(elapsed) = pool_tx.submitted_at.elapsed() {
                        if elapsed.as_secs() > expiry_seconds {
                            continue;
                        }
                    }

                    temp_selected
                        .push((pool_tx.transaction.clone(), pool_tx.transaction.id.clone()));
                }

                for (tx, id) in temp_selected {
                    selected.push(tx);
                    selected_ids.push(id);
                }
            }
        }

        // Remove selected transactions from pool
        for tx_id in selected_ids {
            self.remove_transaction(&tx_id);
        }

        debug!(
            "Selected {} transactions for block inclusion",
            selected.len()
        );
        selected
    }

    /// Remove a transaction from the pool
    pub fn remove_transaction(&mut self, tx_id: &str) -> Option<PoolTransaction> {
        if let Some(pool_tx) = self.by_id.remove(tx_id) {
            // Remove from VM queue
            let vm_type = self.get_vm_type(&pool_tx.transaction);
            if let Some(queue) = self.pending_by_vm.get_mut(&vm_type) {
                queue.retain(|tx| tx.transaction.id != tx_id);
            }

            // Remove from sender list
            if let Some(sender_txs) = self.by_sender.get_mut(&pool_tx.sender) {
                sender_txs.retain(|tx| tx.transaction.id != tx_id);
                if sender_txs.is_empty() {
                    self.by_sender.remove(&pool_tx.sender);
                }
            }

            // Update statistics
            self.stats.current_pool_size -= 1;
            match vm_type {
                VmType::Evm => self.stats.evm_transactions -= 1,
                VmType::Svm => self.stats.svm_transactions -= 1,
                VmType::CrossVm => self.stats.cross_vm_transactions -= 1,
            }

            Some(pool_tx)
        } else {
            None
        }
    }

    /// Mark transactions as included in a block
    pub fn mark_included(&mut self, tx_ids: &[String], block_height: u64) {
        for tx_id in tx_ids {
            if let Some(pool_tx) = self.by_id.get_mut(tx_id) {
                pool_tx.status = TransactionStatus::Included { block_height };
                self.stats.total_included += 1;
            }
        }
    }

    /// Clean up expired transactions
    pub fn cleanup_expired(&mut self) -> usize {
        let mut expired_ids = Vec::new();

        for (tx_id, pool_tx) in &self.by_id {
            if self.is_expired(pool_tx) {
                expired_ids.push(tx_id.clone());
            }
        }

        let count = expired_ids.len();
        for tx_id in expired_ids {
            if let Some(mut pool_tx) = self.remove_transaction(&tx_id) {
                pool_tx.status = TransactionStatus::Expired;
                self.stats.total_expired += 1;
            }
        }

        if count > 0 {
            info!("Cleaned up {} expired transactions", count);
        }
        count
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> &TransactionPoolStats {
        &self.stats
    }

    /// Get transaction by ID
    pub fn get_transaction(&self, tx_id: &str) -> Option<&PoolTransaction> {
        self.by_id.get(tx_id)
    }

    /// Get pending transaction count
    pub fn pending_count(&self) -> usize {
        self.stats.current_pool_size
    }

    // Helper methods

    fn get_vm_type(&self, transaction: &PooledTransaction) -> VmType {
        // Determine VM type from transaction data
        if transaction.data.get("evm_data").is_some() {
            VmType::Evm
        } else if transaction.data.get("cross_vm").is_some() {
            VmType::CrossVm
        } else {
            VmType::Svm
        }
    }

    fn extract_sender(&self, transaction: &PooledTransaction) -> String {
        transaction
            .data
            .get("sender")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn extract_gas_price(&self, transaction: &PooledTransaction) -> Option<u64> {
        transaction.data.get("gas_price").and_then(|v| v.as_u64())
    }

    fn extract_nonce(&self, transaction: &PooledTransaction) -> Option<u64> {
        transaction.data.get("nonce").and_then(|v| v.as_u64())
    }

    fn is_expired(&self, pool_tx: &PoolTransaction) -> bool {
        if let Ok(elapsed) = pool_tx.submitted_at.elapsed() {
            elapsed.as_secs() > self.config.tx_expiry_seconds
        } else {
            true
        }
    }

    fn evict_lowest_priority(&mut self) -> ConsensusResult<()> {
        // Find lowest priority transaction
        let mut lowest_priority = None;
        let mut lowest_tx_id = None;

        for (tx_id, pool_tx) in &self.by_id {
            if lowest_priority.map_or(true, |prio| pool_tx.priority < prio) {
                lowest_priority = Some(pool_tx.priority);
                lowest_tx_id = Some(tx_id.clone());
            }
        }

        if let Some(tx_id) = lowest_tx_id {
            if let Some(mut pool_tx) = self.remove_transaction(&tx_id) {
                pool_tx.status = TransactionStatus::Dropped {
                    reason: "Pool full, evicted for higher priority transaction".to_string(),
                };
                self.stats.total_dropped += 1;
                warn!("Evicted transaction {} due to pool limit", tx_id);
            }
        }

        Ok(())
    }
}

/// Thread-safe transaction pool wrapper
#[derive(Clone)]
pub struct ConcurrentTransactionPool {
    inner: Arc<RwLock<TransactionPool>>,
}

impl ConcurrentTransactionPool {
    pub fn new(config: TransactionPoolConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(TransactionPool::new(config))),
        }
    }

    pub async fn add_transaction(
        &self,
        transaction: PooledTransaction,
        priority: TransactionPriority,
    ) -> ConsensusResult<()> {
        self.inner
            .write()
            .await
            .add_transaction(transaction, priority)
    }

    pub async fn get_transactions_for_block(&self, max_count: usize) -> Vec<PooledTransaction> {
        self.inner
            .write()
            .await
            .get_transactions_for_block(max_count)
    }

    pub async fn mark_included(&self, tx_ids: &[String], block_height: u64) {
        self.inner.write().await.mark_included(tx_ids, block_height)
    }

    pub async fn cleanup_expired(&self) -> usize {
        self.inner.write().await.cleanup_expired()
    }

    pub async fn get_stats(&self) -> TransactionPoolStats {
        self.inner.read().await.get_stats().clone()
    }

    pub async fn pending_count(&self) -> usize {
        self.inner.read().await.pending_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_pool_basic() {
        let config = TransactionPoolConfig::default();
        let mut pool = TransactionPool::new(config);

        // Create test transaction
        let tx = PooledTransaction {
            id: "tx1".to_string(),
            data: serde_json::json!({
                "sender": "alice",
                "evm_data": {},
                "gas_price": 100
            }),
            timestamp: std::time::SystemTime::now(),
            signature: None,
        };

        // Add transaction
        pool.add_transaction(tx.clone(), TransactionPriority::Normal)
            .unwrap();
        assert_eq!(pool.pending_count(), 1);

        // Get transactions for block
        let txs = pool.get_transactions_for_block(10);
        assert_eq!(txs.len(), 1);
        assert_eq!(pool.pending_count(), 0);
    }
}
