//! Transaction Processor for Mock Processes
//!
//! Handles transaction processing and storage for mock blockchain processes.

use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Transaction processor for mock processes
#[derive(Debug, Clone)]
pub struct TransactionProcessor {
    /// Pending transactions
    pending_transactions: HashMap<String, Transaction>,
    /// Processed transactions
    processed_transactions: HashMap<String, ProcessedTransaction>,
    /// Transaction receipts
    receipts: HashMap<String, TransactionReceipt>,
}

/// Transaction data
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Transaction hash
    pub hash: String,
    /// Transaction data (JSON)
    pub data: Value,
    /// Submission time
    pub submitted_at: Instant,
    /// Processing status
    pub status: TransactionStatus,
}

/// Transaction status
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Confirmed,
    Failed(String),
}

/// Processed transaction
#[derive(Debug, Clone)]
pub struct ProcessedTransaction {
    /// Original transaction
    pub transaction: Transaction,
    /// Processing time
    pub processing_time: Duration,
    /// Block number
    pub block_number: u64,
    /// Transaction index in block
    pub transaction_index: u32,
}

/// Transaction receipt
#[derive(Debug, Clone)]
pub struct TransactionReceipt {
    /// Transaction hash
    pub transaction_hash: String,
    /// Block number
    pub block_number: u64,
    /// Gas used
    pub gas_used: u64,
    /// Success status
    pub success: bool,
    /// Logs
    pub logs: Vec<String>,
}

impl TransactionProcessor {
    /// Create a new transaction processor
    pub fn new() -> Self {
        Self {
            pending_transactions: HashMap::new(),
            processed_transactions: HashMap::new(),
            receipts: HashMap::new(),
        }
    }

    /// Add a new transaction
    pub fn add_transaction(&mut self, hash: String, data: Value) {
        let transaction = Transaction {
            hash: hash.clone(),
            data,
            submitted_at: Instant::now(),
            status: TransactionStatus::Pending,
        };

        self.pending_transactions.insert(hash, transaction);
    }

    /// Get a transaction by hash
    pub fn get_transaction(&self, hash: &str) -> Option<&Transaction> {
        self.pending_transactions.get(hash).or_else(|| {
            self.processed_transactions
                .get(hash)
                .map(|pt| &pt.transaction)
        })
    }

    /// Mark transaction as processing
    pub fn start_processing(&mut self, hash: &str) -> bool {
        if let Some(tx) = self.pending_transactions.get_mut(hash) {
            tx.status = TransactionStatus::Processing;
            true
        } else {
            false
        }
    }

    /// Complete transaction processing
    pub fn complete_processing(
        &mut self,
        hash: &str,
        block_number: u64,
        transaction_index: u32,
        gas_used: u64,
        success: bool,
    ) -> bool {
        if let Some(mut tx) = self.pending_transactions.remove(hash) {
            let processing_time = tx.submitted_at.elapsed();

            if success {
                tx.status = TransactionStatus::Confirmed;
            } else {
                tx.status = TransactionStatus::Failed("Execution failed".to_string());
            }

            // Create processed transaction
            let processed = ProcessedTransaction {
                transaction: tx.clone(),
                processing_time,
                block_number,
                transaction_index,
            };

            // Create receipt
            let receipt = TransactionReceipt {
                transaction_hash: hash.to_string(),
                block_number,
                gas_used,
                success,
                logs: vec![],
            };

            self.processed_transactions
                .insert(hash.to_string(), processed);
            self.receipts.insert(hash.to_string(), receipt);

            true
        } else {
            false
        }
    }

    /// Get transaction receipt
    pub fn get_receipt(&self, hash: &str) -> Option<&TransactionReceipt> {
        self.receipts.get(hash)
    }

    /// Get pending transactions
    pub fn get_pending_transactions(&self) -> Vec<&Transaction> {
        self.pending_transactions.values().collect()
    }

    /// Get pending transaction count
    pub fn pending_count(&self) -> usize {
        self.pending_transactions.len()
    }

    /// Get processed transaction count
    pub fn processed_count(&self) -> usize {
        self.processed_transactions.len()
    }

    /// Clean up old transactions
    pub fn cleanup_old_transactions(&mut self, older_than: Duration) {
        let now = Instant::now();

        // Remove old pending transactions
        self.pending_transactions
            .retain(|_, tx| now.duration_since(tx.submitted_at) < older_than);

        // Remove old processed transactions (keep receipts longer)
        self.processed_transactions.retain(|hash, pt| {
            if now.duration_since(pt.transaction.submitted_at) > older_than * 2 {
                self.receipts.remove(hash);
                false
            } else {
                true
            }
        });
    }

    /// Get statistics
    pub fn get_stats(&self) -> TransactionStats {
        let mut total_processing_time = Duration::from_secs(0);
        let mut confirmed_count = 0;
        let mut failed_count = 0;

        for pt in self.processed_transactions.values() {
            total_processing_time += pt.processing_time;
            match &pt.transaction.status {
                TransactionStatus::Confirmed => confirmed_count += 1,
                TransactionStatus::Failed(_) => failed_count += 1,
                _ => {}
            }
        }

        let avg_processing_time = if self.processed_transactions.is_empty() {
            Duration::from_secs(0)
        } else {
            total_processing_time / self.processed_transactions.len() as u32
        };

        TransactionStats {
            pending_count: self.pending_transactions.len(),
            processed_count: self.processed_transactions.len(),
            confirmed_count,
            failed_count,
            avg_processing_time,
        }
    }
}

/// Transaction statistics
#[derive(Debug, Clone)]
pub struct TransactionStats {
    pub pending_count: usize,
    pub processed_count: usize,
    pub confirmed_count: usize,
    pub failed_count: usize,
    pub avg_processing_time: Duration,
}

impl Default for TransactionProcessor {
    fn default() -> Self {
        Self::new()
    }
}
