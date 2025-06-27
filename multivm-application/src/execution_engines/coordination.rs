//! Cross-VM Coordination
//!
//! This module provides coordination between different virtual machine execution engines,
//! enabling cross-chain operations and state synchronization.

use crate::execution_engines::GlobalExecutionConfig;
use multivm_common::{types::BlockchainType, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Cross-VM coordination events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoordinationEvent {
    /// Block processed on a specific blockchain
    BlockProcessed {
        blockchain_type: BlockchainType,
        block_number: u64,
        block_hash: Vec<u8>,
        timestamp: u64,
    },
    /// Cross-VM transaction initiated
    CrossVmTransactionStarted {
        transaction_id: String,
        source_chain: BlockchainType,
        target_chain: BlockchainType,
        timestamp: u64,
    },
    /// Cross-VM transaction completed
    CrossVmTransactionCompleted {
        transaction_id: String,
        success: bool,
        error_message: Option<String>,
        timestamp: u64,
    },
    /// State synchronization event
    StateSynchronized {
        blockchain_type: BlockchainType,
        block_number: u64,
        state_root: Vec<u8>,
        timestamp: u64,
    },
}

/// Cross-VM transaction tracking
#[derive(Debug, Clone)]
pub struct CrossVmTransaction {
    pub id: String,
    pub source_chain: BlockchainType,
    pub target_chain: BlockchainType,
    pub status: TransactionStatus,
    pub created_at: Instant,
    pub updated_at: Instant,
    pub retry_count: u32,
    pub error_message: Option<String>,
}

/// Status of a cross-VM transaction
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Retrying,
}

/// Coordination statistics
#[derive(Debug, Clone)]
pub struct CoordinationStats {
    pub events_processed: u64,
    pub cross_vm_transactions_total: u64,
    pub cross_vm_transactions_successful: u64,
    pub cross_vm_transactions_failed: u64,
    pub last_synchronization: Option<Instant>,
    pub active_transactions: usize,
}

/// Cross-VM coordinator for managing interactions between execution engines
#[derive(Debug)]
pub struct CrossVmCoordinator {
    _config: GlobalExecutionConfig,
    active_transactions: Arc<RwLock<HashMap<String, CrossVmTransaction>>>,
    event_history: Arc<RwLock<Vec<CoordinationEvent>>>,
    stats: Arc<Mutex<CoordinationStats>>,
    is_running: Arc<RwLock<bool>>,
}

impl CrossVmCoordinator {
    /// Create a new cross-VM coordinator
    pub async fn new(config: GlobalExecutionConfig) -> MultivmResult<Self> {
        info!("Creating cross-VM coordinator");

        Ok(Self {
            _config: config,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(Mutex::new(CoordinationStats {
                events_processed: 0,
                cross_vm_transactions_total: 0,
                cross_vm_transactions_successful: 0,
                cross_vm_transactions_failed: 0,
                last_synchronization: None,
                active_transactions: 0,
            })),
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the coordination service
    pub async fn start(&self) -> MultivmResult<()> {
        info!("Starting cross-VM coordinator");

        *self.is_running.write().await = true;

        // Start background tasks
        self.start_transaction_monitor().await?;
        self.start_state_synchronizer().await?;

        info!("Cross-VM coordinator started successfully");
        Ok(())
    }

    /// Process a coordination event
    pub async fn process_event(&self, event: CoordinationEvent) -> MultivmResult<()> {
        debug!("Processing coordination event: {:?}", event);

        // Add event to history
        {
            let mut history = self.event_history.write().await;
            history.push(event.clone());

            // Keep only the last 1000 events
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        // Process specific event types
        match &event {
            CoordinationEvent::BlockProcessed {
                blockchain_type,
                block_number,
                ..
            } => {
                self.handle_block_processed(*blockchain_type, *block_number)
                    .await?;
            }
            CoordinationEvent::CrossVmTransactionStarted {
                transaction_id,
                source_chain,
                target_chain,
                ..
            } => {
                self.handle_cross_vm_transaction_started(
                    transaction_id.clone(),
                    *source_chain,
                    *target_chain,
                )
                .await?;
            }
            CoordinationEvent::CrossVmTransactionCompleted {
                transaction_id,
                success,
                error_message,
                ..
            } => {
                self.handle_cross_vm_transaction_completed(
                    transaction_id.clone(),
                    *success,
                    error_message.clone(),
                )
                .await?;
            }
            CoordinationEvent::StateSynchronized {
                blockchain_type,
                block_number,
                ..
            } => {
                self.handle_state_synchronized(*blockchain_type, *block_number)
                    .await?;
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.events_processed += 1;
        }

        Ok(())
    }

    /// Handle block processed event
    async fn handle_block_processed(
        &self,
        blockchain_type: BlockchainType,
        block_number: u64,
    ) -> MultivmResult<()> {
        debug!(
            "Handling block processed: {:?} block {}",
            blockchain_type, block_number
        );

        // Check for pending cross-VM transactions that might be affected
        let active_transactions = self.active_transactions.read().await;
        for transaction in active_transactions.values() {
            if transaction.source_chain == blockchain_type
                || transaction.target_chain == blockchain_type
            {
                debug!(
                    "Block {} on {:?} may affect cross-VM transaction {}",
                    block_number, blockchain_type, transaction.id
                );
                // In a full implementation, we would check if this block affects the transaction
            }
        }

        Ok(())
    }

    /// Handle cross-VM transaction started
    async fn handle_cross_vm_transaction_started(
        &self,
        transaction_id: String,
        source_chain: BlockchainType,
        target_chain: BlockchainType,
    ) -> MultivmResult<()> {
        info!(
            "Starting cross-VM transaction {} from {:?} to {:?}",
            transaction_id, source_chain, target_chain
        );

        let transaction = CrossVmTransaction {
            id: transaction_id.clone(),
            source_chain,
            target_chain,
            status: TransactionStatus::Pending,
            created_at: Instant::now(),
            updated_at: Instant::now(),
            retry_count: 0,
            error_message: None,
        };

        // Add to active transactions
        {
            let mut active_transactions = self.active_transactions.write().await;
            active_transactions.insert(transaction_id, transaction);
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.cross_vm_transactions_total += 1;
            stats.active_transactions = self.active_transactions.read().await.len();
        }

        Ok(())
    }

    /// Handle cross-VM transaction completed
    async fn handle_cross_vm_transaction_completed(
        &self,
        transaction_id: String,
        success: bool,
        error_message: Option<String>,
    ) -> MultivmResult<()> {
        info!(
            "Cross-VM transaction {} completed: success = {}",
            transaction_id, success
        );

        // Update transaction status
        {
            let mut active_transactions = self.active_transactions.write().await;
            if let Some(transaction) = active_transactions.get_mut(&transaction_id) {
                transaction.status = if success {
                    TransactionStatus::Completed
                } else {
                    TransactionStatus::Failed
                };
                transaction.updated_at = Instant::now();
                transaction.error_message = error_message;
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            if success {
                stats.cross_vm_transactions_successful += 1;
            } else {
                stats.cross_vm_transactions_failed += 1;
            }
        }

        // Remove completed transaction after some time (cleanup will handle this)
        Ok(())
    }

    /// Handle state synchronized event
    async fn handle_state_synchronized(
        &self,
        blockchain_type: BlockchainType,
        block_number: u64,
    ) -> MultivmResult<()> {
        debug!(
            "State synchronized for {:?} at block {}",
            blockchain_type, block_number
        );

        // Update synchronization timestamp
        {
            let mut stats = self.stats.lock().await;
            stats.last_synchronization = Some(Instant::now());
        }

        Ok(())
    }

    /// Start transaction monitoring background task
    async fn start_transaction_monitor(&self) -> MultivmResult<()> {
        let active_transactions = Arc::clone(&self.active_transactions);
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                if !*is_running.read().await {
                    break;
                }

                interval.tick().await;

                // Check for stale transactions
                let mut transactions_to_retry = Vec::new();
                let mut transactions_to_fail = Vec::new();

                {
                    let active = active_transactions.read().await;
                    let now = Instant::now();

                    for (id, transaction) in active.iter() {
                        let age = now.duration_since(transaction.created_at);

                        if age > Duration::from_secs(300) {
                            // Transaction is older than 5 minutes
                            if transaction.retry_count < 3 {
                                transactions_to_retry.push(id.clone());
                            } else {
                                transactions_to_fail.push(id.clone());
                            }
                        }
                    }
                }

                // Handle retries and failures
                if !transactions_to_retry.is_empty() || !transactions_to_fail.is_empty() {
                    let mut active = active_transactions.write().await;

                    for id in transactions_to_retry {
                        if let Some(transaction) = active.get_mut(&id) {
                            transaction.status = TransactionStatus::Retrying;
                            transaction.retry_count += 1;
                            transaction.updated_at = Instant::now();
                            warn!(
                                "Retrying cross-VM transaction {} (attempt {})",
                                id, transaction.retry_count
                            );
                        }
                    }

                    for id in transactions_to_fail {
                        if let Some(transaction) = active.get_mut(&id) {
                            transaction.status = TransactionStatus::Failed;
                            transaction.updated_at = Instant::now();
                            transaction.error_message = Some("Transaction timeout".to_string());
                            error!("Cross-VM transaction {} failed due to timeout", id);
                        }
                    }
                }

                // Cleanup completed transactions older than 1 hour
                {
                    let mut active = active_transactions.write().await;
                    let now = Instant::now();
                    let mut to_remove = Vec::new();

                    for (id, transaction) in active.iter() {
                        if matches!(
                            transaction.status,
                            TransactionStatus::Completed | TransactionStatus::Failed
                        ) {
                            let age = now.duration_since(transaction.updated_at);
                            if age > Duration::from_secs(3600) {
                                to_remove.push(id.clone());
                            }
                        }
                    }

                    for id in to_remove {
                        active.remove(&id);
                    }

                    // Update active transaction count
                    let mut stats_guard = stats.lock().await;
                    stats_guard.active_transactions = active.len();
                }
            }
        });

        Ok(())
    }

    /// Start state synchronization background task
    async fn start_state_synchronizer(&self) -> MultivmResult<()> {
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                if !*is_running.read().await {
                    break;
                }

                interval.tick().await;

                // In a full implementation, this would:
                // 1. Check state consistency across chains
                // 2. Trigger state synchronization if needed
                // 3. Handle state conflicts
                debug!("Performing state synchronization check");
            }
        });

        Ok(())
    }

    /// Get active cross-VM transactions
    pub async fn get_active_transactions(&self) -> HashMap<String, CrossVmTransaction> {
        self.active_transactions.read().await.clone()
    }

    /// Get coordination statistics
    pub async fn get_stats(&self) -> CoordinationStats {
        self.stats.lock().await.clone()
    }

    /// Get recent events
    pub async fn get_recent_events(&self, limit: usize) -> Vec<CoordinationEvent> {
        let history = self.event_history.read().await;
        let start = if history.len() > limit {
            history.len() - limit
        } else {
            0
        };
        history[start..].to_vec()
    }

    /// Check if coordinator is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Shutdown the coordinator
    pub async fn shutdown(&self) -> MultivmResult<()> {
        info!("Shutting down cross-VM coordinator");

        *self.is_running.write().await = false;

        info!("Cross-VM coordinator shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let config = GlobalExecutionConfig::default();
        let coordinator = CrossVmCoordinator::new(config).await.unwrap();
        assert!(!coordinator.is_running().await);
    }

    #[tokio::test]
    async fn test_event_processing() {
        let config = GlobalExecutionConfig::default();
        let coordinator = CrossVmCoordinator::new(config).await.unwrap();

        let event = CoordinationEvent::BlockProcessed {
            blockchain_type: BlockchainType::Ethereum,
            block_number: 100,
            block_hash: vec![1, 2, 3],
            timestamp: 1234567890,
        };

        coordinator.process_event(event).await.unwrap();

        let stats = coordinator.get_stats().await;
        assert_eq!(stats.events_processed, 1);
    }

    #[tokio::test]
    async fn test_cross_vm_transaction_lifecycle() {
        let config = GlobalExecutionConfig::default();
        let coordinator = CrossVmCoordinator::new(config).await.unwrap();

        // Start transaction
        let start_event = CoordinationEvent::CrossVmTransactionStarted {
            transaction_id: "test_tx_1".to_string(),
            source_chain: BlockchainType::Ethereum,
            target_chain: BlockchainType::Solana,
            timestamp: 1234567890,
        };
        coordinator.process_event(start_event).await.unwrap();

        // Complete transaction
        let complete_event = CoordinationEvent::CrossVmTransactionCompleted {
            transaction_id: "test_tx_1".to_string(),
            success: true,
            error_message: None,
            timestamp: 1234567891,
        };
        coordinator.process_event(complete_event).await.unwrap();

        let stats = coordinator.get_stats().await;
        assert_eq!(stats.cross_vm_transactions_total, 1);
        assert_eq!(stats.cross_vm_transactions_successful, 1);
        assert_eq!(stats.cross_vm_transactions_failed, 0);
    }
}
