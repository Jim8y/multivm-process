//! Transaction Batcher Component
//!
//! This module implements the transaction batching functionality for the MultiVM system.
//! It collects transactions from memory pools, prioritizes them, and builds candidate blocks
//! for consensus processing.

use multivm_common::{MultivmError, MultivmResult};
use multivm_consensus::block::{MultiVMBlock, SvmTransaction, EvmTransaction};
use multivm_account_mapping::SpecialTransaction;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::cmp::Ordering;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

/// Transaction batcher configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatcherConfig {
    /// Maximum batch size (number of transactions)
    pub max_batch_size: usize,
    /// Maximum block size (bytes)
    pub max_block_size: usize,
    /// Batch timeout duration
    pub batch_timeout: Duration,
    /// Priority threshold
    pub priority_threshold: u64,
    /// Base gas price
    pub base_gas_price: u64,
    /// Maximum wait time
    pub max_wait_time: Duration,
    /// Pre-validation enabled
    pub enable_prevalidation: bool,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            max_block_size: 1024 * 1024, // 1MB
            batch_timeout: Duration::from_millis(500),
            priority_threshold: 1000000000, // 1 gwei
            base_gas_price: 1000000000,     // 1 gwei
            max_wait_time: Duration::from_secs(5),
            enable_prevalidation: true,
        }
    }
}

/// Transaction priority wrapper
#[derive(Debug, Clone)]
pub struct PrioritizedTransaction {
    /// Transaction content
    pub transaction: Transaction,
    /// Priority score
    pub priority: u64,
    /// Arrival time
    pub arrival_time: Instant,
    /// Pre-validation status
    pub prevalidation_status: PrevalidationStatus,
}

/// Unified transaction type
#[derive(Debug, Clone)]
pub enum Transaction {
    /// Solana VM transaction
    Svm(SvmTransaction),
    /// Ethereum VM transaction
    Evm(EvmTransaction),
    /// Cross-VM special transaction
    MultiVm(SpecialTransaction),
}

/// Pre-validation status
#[derive(Debug, Clone, PartialEq)]
pub enum PrevalidationStatus {
    /// Not validated
    Pending,
    /// Validation passed
    Valid,
    /// Validation failed
    Invalid(String),
    /// Validation skipped
    Skipped,
}

/// Memory pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolStats {
    /// Number of SVM transactions
    pub svm_transactions: usize,
    /// Number of EVM transactions
    pub evm_transactions: usize,
    /// Number of MultiVM transactions
    pub multivm_transactions: usize,
    /// Total number of transactions
    pub total_transactions: usize,
    /// Average priority
    pub average_priority: f64,
    /// Memory usage (bytes)
    pub memory_usage: usize,
    /// Age of oldest transaction
    pub oldest_transaction_age: Duration,
}

/// Batch build result
#[derive(Debug)]
pub struct BatchResult {
    /// Built block
    pub block: MultiVMBlock,
    /// Number of included transactions
    pub transaction_count: usize,
    /// Block size
    pub block_size: usize,
    /// Build duration
    pub build_duration: Duration,
    /// Discarded transactions (validation failed)
    pub discarded_transactions: Vec<(Transaction, String)>,
}

/// Transaction batcher main structure
pub struct TransactionBatcher {
    /// Configuration
    config: BatcherConfig,
    /// SVM transaction priority queue
    svm_priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
    /// EVM transaction priority queue
    evm_priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
    /// MultiVM transaction priority queue
    multivm_priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
    /// Transaction index (for deduplication)
    transaction_index: Arc<RwLock<HashMap<String, Instant>>>,
    /// Statistics
    stats: Arc<RwLock<MempoolStats>>,
    /// Batch request receiver
    batch_receiver: mpsc::Receiver<BatchRequest>,
    /// Batch result sender
    result_sender: mpsc::Sender<BatchResult>,
    /// Transaction input receiver
    transaction_receiver: mpsc::Receiver<Transaction>,
    /// Running status
    is_running: Arc<RwLock<bool>>,
}

/// Batch request
#[derive(Debug)]
pub struct BatchRequest {
    /// Requested block height
    pub block_height: u64,
    /// Parent block hash
    pub parent_hash: String,
    /// Proposer node ID
    pub proposer: String,
    /// Response channel
    pub response: tokio::sync::oneshot::Sender<MultivmResult<BatchResult>>,
}

use std::sync::Arc;

impl PrioritizedTransaction {
    /// Create new prioritized transaction
    pub fn new(transaction: Transaction) -> Self {
        let priority = Self::calculate_priority(&transaction);
        Self {
            transaction,
            priority,
            arrival_time: Instant::now(),
            prevalidation_status: PrevalidationStatus::Pending,
        }
    }

    /// Calculate transaction priority
    pub fn calculate_priority(transaction: &Transaction) -> u64 {
        match transaction {
            Transaction::Svm(svm_tx) => {
                // SVM priority based on fee
                svm_tx.fee
            }
            Transaction::Evm(evm_tx) => {
                // EVM priority based on gas price
                evm_tx.gas_price
            }
            Transaction::MultiVm(_) => {
                // MultiVM transactions get highest priority
                u64::MAX
            }
        }
    }

    /// Get transaction hash
    pub fn get_hash(&self) -> String {
        match &self.transaction {
            Transaction::Svm(svm_tx) => svm_tx.id.to_string(),
            Transaction::Evm(evm_tx) => evm_tx.hash.clone(),
            Transaction::MultiVm(mv_tx) => format!("multivm_{}", mv_tx.id),
        }
    }

    /// Get transaction size (bytes)
    pub fn get_size(&self) -> usize {
        match &self.transaction {
            Transaction::Svm(svm_tx) => svm_tx.size_bytes(),
            Transaction::Evm(evm_tx) => evm_tx.size_bytes(),
            Transaction::MultiVm(_) => 256, // Estimated size
        }
    }
}

impl Ord for PrioritizedTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        // First sort by priority
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => {
                // When priorities are equal, sort by arrival time (earlier arrivals have priority)
                other.arrival_time.cmp(&self.arrival_time)
            }
            other => other,
        }
    }
}

impl PartialOrd for PrioritizedTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PrioritizedTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.arrival_time == other.arrival_time
    }
}

impl Eq for PrioritizedTransaction {}

impl TransactionBatcher {
    /// Create new transaction batcher
    pub fn new(config: BatcherConfig) -> (Self, TransactionBatcherHandle) {
        let (batch_sender, batch_receiver) = mpsc::channel(100);
        let (result_sender, result_receiver) = mpsc::channel(100);
        let (transaction_sender, transaction_receiver) = mpsc::channel(10000);

        let batcher = Self {
            config,
            svm_priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            evm_priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            multivm_priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            transaction_index: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(MempoolStats::default())),
            batch_receiver,
            result_sender,
            transaction_receiver,
            is_running: Arc::new(RwLock::new(false)),
        };

        let handle = TransactionBatcherHandle {
            batch_sender,
            result_receiver: Mutex::new(result_receiver),
            transaction_sender,
            stats: batcher.stats.clone(),
            is_running: batcher.is_running.clone(),
        };

        (batcher, handle)
    }

    /// Start transaction batcher
    pub async fn start(&mut self) -> MultivmResult<()> {
        info!("Starting transaction batcher");
        
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(MultivmError::InvalidState("Batcher already running".to_string()));
            }
            *running = true;
        }

        // Start main loop
        self.run().await
    }

    /// Main run loop
    async fn run(&mut self) -> MultivmResult<()> {
        let mut cleanup_timer = tokio::time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                // Process new transaction
                Some(transaction) = self.transaction_receiver.recv() => {
                    if let Err(e) = self.add_transaction(transaction).await {
                        error!("Error adding transaction: {}", e);
                    }
                }

                // Process batch request
                Some(batch_request) = self.batch_receiver.recv() => {
                    let result = self.build_batch(batch_request.block_height, 
                                                 batch_request.parent_hash, 
                                                 batch_request.proposer).await;
                    let _ = batch_request.response.send(result);
                }

                // Periodically clean up expired transactions
                _ = cleanup_timer.tick() => {
                    self.cleanup_expired_transactions().await;
                }

                // Check if should stop
                else => {
                    if !*self.is_running.read().await {
                        break;
                    }
                }
            }
        }

        info!("Transaction batcher stopped");
        Ok(())
    }

    /// Add new transaction to memory pool
    async fn add_transaction(&mut self, transaction: Transaction) -> MultivmResult<()> {
        let prioritized_tx = PrioritizedTransaction::new(transaction);
        let tx_hash = prioritized_tx.get_hash();

        // Check for duplicates
        {
            let mut index = self.transaction_index.write().await;
            if index.contains_key(&tx_hash) {
                debug!("Duplicate transaction ignored: {}", tx_hash);
                return Ok(());
            }
            index.insert(tx_hash.clone(), prioritized_tx.arrival_time);
        }

        // Pre-validation (if enabled)
        let mut prioritized_tx = prioritized_tx;
        if self.config.enable_prevalidation {
            prioritized_tx.prevalidation_status = self.prevalidate_transaction(&prioritized_tx.transaction).await;
            
            if let PrevalidationStatus::Invalid(reason) = &prioritized_tx.prevalidation_status {
                warn!("Transaction prevalidation failed: {} - {}", tx_hash, reason);
                return Ok(());
            }
        }

        // Add to corresponding priority queue
        match &prioritized_tx.transaction {
            Transaction::Svm(_) => {
                let mut queue = self.svm_priority_queue.lock().await;
                queue.push(prioritized_tx);
            }
            Transaction::Evm(_) => {
                let mut queue = self.evm_priority_queue.lock().await;
                queue.push(prioritized_tx);
            }
            Transaction::MultiVm(_) => {
                let mut queue = self.multivm_priority_queue.lock().await;
                queue.push(prioritized_tx);
            }
        }

        // Update statistics
        self.update_stats().await;

        debug!("Transaction added to mempool: {}", tx_hash);
        Ok(())
    }

    /// Build transaction batch
    async fn build_batch(&self, block_height: u64, parent_hash: String, proposer: String) -> MultivmResult<BatchResult> {
        let start_time = Instant::now();
        info!("Building batch for block height: {}", block_height);

        // Create new block
        let mut block = MultiVMBlock::new(block_height, parent_hash, proposer, vec![]);
        
        let mut transaction_count = 0;
        let mut current_size = 0;
        let mut discarded_transactions = Vec::new();

        // Collect MultiVM transactions (highest priority)
        self.collect_transactions_from_queue(
            &self.multivm_priority_queue,
            &mut block,
            &mut transaction_count,
            &mut current_size,
            &mut discarded_transactions,
        ).await?;

        // Collect EVM transactions
        if transaction_count < self.config.max_batch_size && current_size < self.config.max_block_size {
            self.collect_transactions_from_queue(
                &self.evm_priority_queue,
                &mut block,
                &mut transaction_count,
                &mut current_size,
                &mut discarded_transactions,
            ).await?;
        }

        // Collect SVM transactions
        if transaction_count < self.config.max_batch_size && current_size < self.config.max_block_size {
            self.collect_transactions_from_queue(
                &self.svm_priority_queue,
                &mut block,
                &mut transaction_count,
                &mut current_size,
                &mut discarded_transactions,
            ).await?;
        }

        // Complete block construction
        block.finalize();

        let build_duration = start_time.elapsed();
        let block_size = block.size_bytes();

        info!("Batch built: {} transactions, {} bytes, {:?} duration", 
              transaction_count, block_size, build_duration);

        Ok(BatchResult {
            block,
            transaction_count,
            block_size,
            build_duration,
            discarded_transactions,
        })
    }

    /// Collect transactions from queue
    async fn collect_transactions_from_queue(
        &self,
        queue: &Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
        block: &mut MultiVMBlock,
        transaction_count: &mut usize,
        current_size: &mut usize,
        discarded: &mut Vec<(Transaction, String)>,
    ) -> MultivmResult<()> {
        let mut queue_guard = queue.lock().await;
        let mut temp_transactions = Vec::new();

        // Take transactions from queue for processing
        while let Some(prioritized_tx) = queue_guard.pop() {
            if *transaction_count >= self.config.max_batch_size {
                temp_transactions.push(prioritized_tx);
                break;
            }

            let tx_size = prioritized_tx.get_size();
            if *current_size + tx_size > self.config.max_block_size {
                temp_transactions.push(prioritized_tx);
                break;
            }

            // Check priority threshold
            if prioritized_tx.priority < self.config.priority_threshold {
                temp_transactions.push(prioritized_tx);
                continue;
            }

            // Check if transaction is expired
            if prioritized_tx.arrival_time.elapsed() > self.config.max_wait_time {
                discarded.push((prioritized_tx.transaction, "Transaction expired".to_string()));
                continue;
            }

            // Add transaction to block
            match prioritized_tx.transaction {
                Transaction::Svm(svm_tx) => {
                    block.add_svm_transaction(svm_tx);
                }
                Transaction::Evm(evm_tx) => {
                    block.add_evm_transaction(evm_tx);
                }
                Transaction::MultiVm(mv_tx) => {
                    block.add_multivm_transaction(mv_tx);
                }
            }

            *transaction_count += 1;
            *current_size += tx_size;

            // Remove from index
            let tx_hash = prioritized_tx.get_hash();
            self.transaction_index.write().await.remove(&tx_hash);
        }

        // Put unprocessed transactions back in queue
        for tx in temp_transactions {
            queue_guard.push(tx);
        }

        Ok(())
    }

    /// Pre-validate transaction
    async fn prevalidate_transaction(&self, transaction: &Transaction) -> PrevalidationStatus {
        match transaction {
            Transaction::Svm(svm_tx) => {
                // SVM transaction pre-validation
                if svm_tx.signatures.is_empty() {
                    PrevalidationStatus::Invalid("No signatures".to_string())
                } else if svm_tx.accounts.is_empty() {
                    PrevalidationStatus::Invalid("No accounts".to_string())
                } else {
                    PrevalidationStatus::Valid
                }
            }
            Transaction::Evm(evm_tx) => {
                // EVM transaction pre-validation
                if evm_tx.gas_limit == 0 {
                    PrevalidationStatus::Invalid("Zero gas limit".to_string())
                } else if evm_tx.gas_price == 0 {
                    PrevalidationStatus::Invalid("Zero gas price".to_string())
                } else {
                    PrevalidationStatus::Valid
                }
            }
            Transaction::MultiVm(_) => {
                // MultiVM transaction pre-validation
                PrevalidationStatus::Valid
            }
        }
    }

    /// Clean up expired transactions
    async fn cleanup_expired_transactions(&self) {
        let cutoff_time = Instant::now() - self.config.max_wait_time;
        let mut expired_count = 0;

        // Clean up SVM queue
        {
            let mut queue = self.svm_priority_queue.lock().await;
            let transactions: Vec<_> = queue.drain().collect();
            for tx in transactions {
                if tx.arrival_time > cutoff_time {
                    queue.push(tx);
                } else {
                    expired_count += 1;
                    let tx_hash = tx.get_hash();
                    self.transaction_index.write().await.remove(&tx_hash);
                }
            }
        }

        // Clean up EVM queue
        {
            let mut queue = self.evm_priority_queue.lock().await;
            let transactions: Vec<_> = queue.drain().collect();
            for tx in transactions {
                if tx.arrival_time > cutoff_time {
                    queue.push(tx);
                } else {
                    expired_count += 1;
                    let tx_hash = tx.get_hash();
                    self.transaction_index.write().await.remove(&tx_hash);
                }
            }
        }

        // Clean up MultiVM queue
        {
            let mut queue = self.multivm_priority_queue.lock().await;
            let transactions: Vec<_> = queue.drain().collect();
            for tx in transactions {
                if tx.arrival_time > cutoff_time {
                    queue.push(tx);
                } else {
                    expired_count += 1;
                    let tx_hash = tx.get_hash();
                    self.transaction_index.write().await.remove(&tx_hash);
                }
            }
        }

        if expired_count > 0 {
            info!("Cleaned up {} expired transactions", expired_count);
            self.update_stats().await;
        }
    }

    /// Update statistics
    async fn update_stats(&self) {
        let svm_count = self.svm_priority_queue.lock().await.len();
        let evm_count = self.evm_priority_queue.lock().await.len();
        let multivm_count = self.multivm_priority_queue.lock().await.len();

        let total_count = svm_count + evm_count + multivm_count;

        // Calculate average priority and oldest transaction age
        let mut total_priority = 0u64;
        let mut oldest_time = Instant::now();

        // Check all queues
        for queue in [&self.svm_priority_queue, &self.evm_priority_queue, &self.multivm_priority_queue] {
            let queue_guard = queue.lock().await;
            for tx in queue_guard.iter() {
                total_priority += tx.priority;
                if tx.arrival_time < oldest_time {
                    oldest_time = tx.arrival_time;
                }
            }
        }

        let average_priority = if total_count > 0 {
            total_priority as f64 / total_count as f64
        } else {
            0.0
        };

        let oldest_transaction_age = oldest_time.elapsed();

        let mut stats = self.stats.write().await;
        stats.svm_transactions = svm_count;
        stats.evm_transactions = evm_count;
        stats.multivm_transactions = multivm_count;
        stats.total_transactions = total_count;
        stats.average_priority = average_priority;
        stats.memory_usage = total_count * 512; // Estimated memory usage
        stats.oldest_transaction_age = oldest_transaction_age;
    }

    /// Stop transaction batcher
    pub async fn stop(&mut self) -> MultivmResult<()> {
        info!("Stopping transaction batcher");
        *self.is_running.write().await = false;
        Ok(())
    }
}

impl Default for MempoolStats {
    fn default() -> Self {
        Self {
            svm_transactions: 0,
            evm_transactions: 0,
            multivm_transactions: 0,
            total_transactions: 0,
            average_priority: 0.0,
            memory_usage: 0,
            oldest_transaction_age: Duration::from_secs(0),
        }
    }
}

/// Transaction batcher operation handle
pub struct TransactionBatcherHandle {
    /// Batch request sender
    batch_sender: mpsc::Sender<BatchRequest>,
    /// Batch result receiver
    result_receiver: Mutex<mpsc::Receiver<BatchResult>>,
    /// Transaction input sender
    transaction_sender: mpsc::Sender<Transaction>,
    /// Statistics
    stats: Arc<RwLock<MempoolStats>>,
    /// Running status
    is_running: Arc<RwLock<bool>>,
}

impl TransactionBatcherHandle {
    /// Submit new transaction
    pub async fn submit_transaction(&self, transaction: Transaction) -> MultivmResult<()> {
        self.transaction_sender.send(transaction).await
            .map_err(|_| MultivmError::Communication("Failed to send transaction".to_string()))
    }

    /// Request batch construction
    pub async fn request_batch(&self, block_height: u64, parent_hash: String, proposer: String) -> MultivmResult<BatchResult> {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        
        let request = BatchRequest {
            block_height,
            parent_hash,
            proposer,
            response: response_sender,
        };

        self.batch_sender.send(request).await
            .map_err(|_| MultivmError::Communication("Failed to send batch request".to_string()))?;

        response_receiver.await
            .map_err(|_| MultivmError::Communication("Failed to receive batch response".to_string()))?
    }

    /// Get memory pool statistics
    pub async fn get_stats(&self) -> MempoolStats {
        self.stats.read().await.clone()
    }

    /// Check if running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multivm_consensus::block::{SvmTransaction, EvmTransaction};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_batcher_creation() {
        let config = BatcherConfig::default();
        let (mut batcher, _handle) = TransactionBatcher::new(config);
        
        assert!(!batcher.is_running.read().await);
    }

    #[tokio::test]
    async fn test_transaction_priority() {
        let svm_tx = SvmTransaction::new(
            vec!["sig1".to_string()],
            vec![1, 2, 3],
            vec!["account1".to_string()],
        );
        let mut svm_tx = svm_tx;
        svm_tx.fee = 1000;

        let evm_tx = EvmTransaction::new(
            "0x1234".to_string(),
            Some("0x5678".to_string()),
            1000,
            21000,
            2000, // Higher gas price
            vec![],
            1,
        );

        let prioritized_svm = PrioritizedTransaction::new(Transaction::Svm(svm_tx));
        let prioritized_evm = PrioritizedTransaction::new(Transaction::Evm(evm_tx));

        // EVM should have higher priority due to higher gas price
        assert!(prioritized_evm > prioritized_svm);
    }

    #[tokio::test]
    async fn test_prevalidation() {
        let config = BatcherConfig::default();
        let (batcher, _handle) = TransactionBatcher::new(config);

        // Valid SVM transaction
        let svm_tx = SvmTransaction::new(
            vec!["sig1".to_string()],
            vec![1, 2, 3],
            vec!["account1".to_string()],
        );
        let status = batcher.prevalidate_transaction(&Transaction::Svm(svm_tx)).await;
        assert_eq!(status, PrevalidationStatus::Valid);

        // Invalid EVM transaction (zero gas)
        let evm_tx = EvmTransaction::new(
            "0x1234".to_string(),
            Some("0x5678".to_string()),
            1000,
            0, // Zero gas limit
            20,
            vec![],
            1,
        );
        let status = batcher.prevalidate_transaction(&Transaction::Evm(evm_tx)).await;
        assert!(matches!(status, PrevalidationStatus::Invalid(_)));
    }
} 