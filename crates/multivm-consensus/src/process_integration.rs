//! Process Integration for Malachite Consensus
//!
//! This module integrates the Malachite BFT consensus engine with the MultiVM
//! process coordinator. MultiVM does NOT execute transactions - it coordinates
//! external Reth and Solana processes for execution.
//!
//! ## Architecture
//!
//! Malachite Consensus → Process Coordinator → External Processes
//!                                             ├─ Reth (Ethereum)
//!                                             └─ Solana
//!
//! The consensus layer ensures agreement on transaction ordering, while the
//! process coordinator handles execution through external processes.

use crate::{
    block::MultiVMBlock,
    malachite::{MalachiteConfig, MalachiteConsensus},
    ConsensusError, ConsensusResult,
};
use multivm_common::MultivmResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

/// Process-aware consensus coordinator
///
/// This coordinator bridges the Malachite consensus engine with the external
/// process execution layer. It ensures that consensus decisions are executed
/// by the appropriate external processes (Reth for Ethereum, Solana for Solana).
pub struct ProcessConsensusCoordinator {
    /// Malachite consensus engine
    consensus_engine: Option<MalachiteConsensus>,
    /// Channel to send blocks to consensus
    block_sender: Option<mpsc::Sender<MultiVMBlock>>,
    /// Channel to receive committed blocks from consensus
    commit_receiver: Option<mpsc::Receiver<MultiVMBlock>>,
    /// Process execution coordinator
    process_executor: Arc<ProcessExecutionCoordinator>,
    /// Configuration
    config: ProcessConsensusConfig,
    /// Metrics
    metrics: Arc<RwLock<ProcessConsensusMetrics>>,
}

/// Configuration for process-aware consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConsensusConfig {
    /// Malachite consensus configuration
    pub consensus_config: MalachiteConfig,
    /// Process execution configuration
    pub execution_config: ProcessExecutionConfig,
    /// Enable transaction batching
    pub enable_batching: bool,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
}

/// Process execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExecutionConfig {
    /// Reth process endpoint
    pub reth_endpoint: String,
    /// Solana process endpoint
    pub solana_endpoint: String,
    /// Execution timeout per transaction
    pub execution_timeout_ms: u64,
    /// Maximum retries for failed executions
    pub max_retries: u32,
}

/// Metrics for process consensus
#[derive(Debug, Clone, Default)]
pub struct ProcessConsensusMetrics {
    /// Blocks received for consensus
    pub blocks_received: u64,
    /// Blocks committed by consensus
    pub blocks_committed: u64,
    /// Blocks executed by processes
    pub blocks_executed: u64,
    /// Reth transactions executed
    pub reth_transactions: u64,
    /// Solana transactions executed
    pub solana_transactions: u64,
    /// Execution failures
    pub execution_failures: u64,
    /// Average execution time
    pub avg_execution_time_ms: u64,
}

/// Process execution coordinator
///
/// Handles the execution of consensus-approved blocks through external processes
pub struct ProcessExecutionCoordinator {
    /// Reth process client (mocked)
    reth_client: Arc<RwLock<MockProcessClient>>,
    /// Solana process client (mocked)
    solana_client: Arc<RwLock<MockProcessClient>>,
    /// Configuration
    config: ProcessExecutionConfig,
    /// Execution state tracking
    execution_state: Arc<RwLock<ExecutionState>>,
}

/// Execution state tracking
#[derive(Debug, Default, Clone)]
pub struct ExecutionState {
    /// Last executed block height
    pub last_executed_height: u64,
    /// Pending executions
    pub pending_executions: HashMap<u64, BlockExecution>,
    /// Execution history
    pub execution_history: Vec<ExecutionRecord>,
}

/// Block execution details
#[derive(Debug, Clone)]
pub struct BlockExecution {
    /// Block height
    pub height: u64,
    /// Block hash
    pub block_hash: String,
    /// Transactions to execute
    pub transactions: Vec<TransactionExecution>,
    /// Execution status
    pub status: ExecutionStatus,
    /// Start time
    pub start_time: std::time::Instant,
}

/// Individual transaction execution
#[derive(Debug, Clone)]
pub struct TransactionExecution {
    /// Transaction ID
    pub tx_id: String,
    /// Target process (Reth or Solana)
    pub target_process: TargetProcess,
    /// Transaction data
    pub tx_data: Vec<u8>,
    /// Execution result
    pub result: Option<ExecutionResult>,
}

/// Target process for execution
#[derive(Debug, Clone, PartialEq)]
pub enum TargetProcess {
    Reth,
    Solana,
}

/// Execution status
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Executing,
    Completed,
    Failed(String),
}

/// Execution result from external process
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Success flag
    pub success: bool,
    /// Transaction hash from the process
    pub tx_hash: Option<String>,
    /// Gas/compute units used
    pub resources_used: u64,
    /// Error message if failed
    pub error: Option<String>,
}

/// Execution record for history
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    /// Block height
    pub height: u64,
    /// Total transactions
    pub total_transactions: usize,
    /// Successful executions
    pub successful: usize,
    /// Failed executions
    pub failed: usize,
    /// Execution duration
    pub duration_ms: u64,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Mock process client for testing
///
/// In production, this would be replaced with actual IPC/RPC clients
/// to communicate with external Reth and Solana processes
#[derive(Debug)]
pub struct MockProcessClient {
    /// Process name
    pub process_name: String,
    /// Simulated latency
    pub latency_ms: u64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}

impl ProcessConsensusCoordinator {
    /// Create new process consensus coordinator
    pub async fn new(config: ProcessConsensusConfig) -> ConsensusResult<Self> {
        info!("Creating process consensus coordinator");

        // Create process execution coordinator
        let process_executor = Arc::new(ProcessExecutionCoordinator::new(
            config.execution_config.clone(),
        )?);

        // Create Malachite consensus engine
        let (consensus_engine, block_sender, commit_receiver) =
            MalachiteConsensus::new(config.consensus_config.clone()).await?;

        Ok(Self {
            consensus_engine: Some(consensus_engine),
            block_sender: Some(block_sender),
            commit_receiver: Some(commit_receiver),
            process_executor,
            config,
            metrics: Arc::new(RwLock::new(ProcessConsensusMetrics::default())),
        })
    }

    /// Start the consensus coordinator
    pub async fn start(&mut self) -> ConsensusResult<()> {
        info!("Starting process consensus coordinator");

        // Start the Malachite consensus engine
        if let Some(engine) = &self.consensus_engine {
            engine.start_consensus().await?;
        }

        // Start the commit processing loop
        self.start_commit_processor().await?;

        Ok(())
    }

    /// Submit a block for consensus
    pub async fn submit_block(&self, block: MultiVMBlock) -> ConsensusResult<()> {
        if let Some(sender) = &self.block_sender {
            let mut metrics = self.metrics.write().await;
            metrics.blocks_received += 1;

            sender
                .send(block)
                .await
                .map_err(|e| ConsensusError::Internal(format!("Channel send error: {}", e)))?;
        }
        Ok(())
    }

    /// Start processing committed blocks
    async fn start_commit_processor(&mut self) -> ConsensusResult<()> {
        let mut commit_receiver = self
            .commit_receiver
            .take()
            .ok_or_else(|| ConsensusError::InvalidState("No commit receiver".to_string()))?;

        let process_executor = Arc::clone(&self.process_executor);
        let metrics = Arc::clone(&self.metrics);
        let enable_batching = self.config.enable_batching;
        let max_batch_size = self.config.max_batch_size;

        tokio::spawn(async move {
            info!("Started commit processor for executing consensus decisions");

            let mut batch = Vec::new();

            while let Some(block) = commit_receiver.recv().await {
                info!("Received committed block at height {}", block.header.height);

                // Update metrics
                {
                    let mut m = metrics.write().await;
                    m.blocks_committed += 1;
                }

                if enable_batching {
                    batch.push(block);

                    if batch.len() >= max_batch_size {
                        // Execute batch
                        if let Err(e) = process_executor.execute_block_batch(batch.clone()).await {
                            error!("Failed to execute block batch: {:?}", e);
                        }
                        batch.clear();
                    }
                } else {
                    // Execute immediately
                    if let Err(e) = process_executor.execute_block(block).await {
                        error!("Failed to execute block: {:?}", e);
                    }
                }

                // Update metrics
                {
                    let mut m = metrics.write().await;
                    m.blocks_executed += 1;
                }
            }
        });

        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> ProcessConsensusMetrics {
        self.metrics.read().await.clone()
    }

    /// Get execution state
    pub async fn get_execution_state(&self) -> ExecutionState {
        self.process_executor.get_execution_state().await
    }
}

impl ProcessExecutionCoordinator {
    /// Create new process execution coordinator
    pub fn new(config: ProcessExecutionConfig) -> ConsensusResult<Self> {
        // Create mock process clients
        // In production, these would be real IPC/RPC clients
        let reth_client = Arc::new(RwLock::new(MockProcessClient {
            process_name: "Reth".to_string(),
            latency_ms: 100,
            success_rate: 0.95,
        }));

        let solana_client = Arc::new(RwLock::new(MockProcessClient {
            process_name: "Solana".to_string(),
            latency_ms: 50,
            success_rate: 0.98,
        }));

        Ok(Self {
            reth_client,
            solana_client,
            config,
            execution_state: Arc::new(RwLock::new(ExecutionState::default())),
        })
    }

    /// Execute a single block through external processes
    pub async fn execute_block(&self, block: MultiVMBlock) -> MultivmResult<()> {
        let start_time = std::time::Instant::now();
        let total_txs = block.transaction_count();
        info!(
            "Executing block {} with {} transactions",
            block.header.height, total_txs
        );

        // Create block execution record
        let mut block_execution = BlockExecution {
            height: block.header.height,
            block_hash: block.calculate_hash(),
            transactions: Vec::new(),
            status: ExecutionStatus::Executing,
            start_time,
        };

        // Process each transaction
        let mut successful = 0;
        let mut failed = 0;

        // Process EVM transactions through Reth
        for tx in &block.evm_transactions {
            let tx_execution = TransactionExecution {
                tx_id: tx.id.to_string(),
                target_process: TargetProcess::Reth,
                tx_data: tx.data.clone(),
                result: None,
            };

            // Execute transaction through Reth process
            let result = self.execute_reth_transaction(&tx_execution).await;

            match result {
                Ok(exec_result) => {
                    if exec_result.success {
                        successful += 1;
                    } else {
                        failed += 1;
                    }
                    block_execution.transactions.push(TransactionExecution {
                        result: Some(exec_result),
                        ..tx_execution
                    });
                }
                Err(e) => {
                    error!("Transaction execution failed: {:?}", e);
                    failed += 1;
                }
            }
        }

        // Process SVM transactions through Solana
        for tx in &block.svm_transactions {
            let tx_execution = TransactionExecution {
                tx_id: tx.id.to_string(),
                target_process: TargetProcess::Solana,
                tx_data: tx.data.clone(),
                result: None,
            };

            // Execute transaction through Solana process
            let result = self.execute_solana_transaction(&tx_execution).await;

            match result {
                Ok(exec_result) => {
                    if exec_result.success {
                        successful += 1;
                    } else {
                        failed += 1;
                    }
                    block_execution.transactions.push(TransactionExecution {
                        result: Some(exec_result),
                        ..tx_execution
                    });
                }
                Err(e) => {
                    error!("Transaction execution failed: {:?}", e);
                    failed += 1;
                }
            }
        }

        // Process MultiVM special transactions (cross-VM operations)
        for tx in &block.multivm_transactions {
            // For special transactions, we may need to coordinate between both processes
            info!("Processing MultiVM special transaction: {:?}", tx);
            // In production, this would handle cross-VM atomic operations
            successful += 1;
        }

        // Update execution state
        let duration_ms = start_time.elapsed().as_millis() as u64;
        block_execution.status = if failed == 0 {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed(format!("{} transactions failed", failed))
        };

        // Record execution
        let mut state = self.execution_state.write().await;
        state.last_executed_height = block.header.height;
        state.execution_history.push(ExecutionRecord {
            height: block.header.height,
            total_transactions: total_txs,
            successful,
            failed,
            duration_ms,
            timestamp: chrono::Utc::now(),
        });

        // Update metrics for specific transaction types
        // This would be done through the metrics system in production

        info!(
            "Block {} execution complete: {} successful, {} failed, took {}ms",
            block.header.height, successful, failed, duration_ms
        );

        Ok(())
    }

    /// Execute a batch of blocks
    pub async fn execute_block_batch(&self, blocks: Vec<MultiVMBlock>) -> MultivmResult<()> {
        info!("Executing batch of {} blocks", blocks.len());

        for block in blocks {
            self.execute_block(block).await?;
        }

        Ok(())
    }

    /// Execute transaction through Reth process
    async fn execute_reth_transaction(
        &self,
        tx: &TransactionExecution,
    ) -> MultivmResult<ExecutionResult> {
        let client = self.reth_client.read().await;

        // Simulate IPC call to Reth process
        debug!("[MOCK] Sending transaction to Reth process: {}", tx.tx_id);
        tokio::time::sleep(tokio::time::Duration::from_millis(client.latency_ms)).await;

        // Simulate execution result
        let success = rand::random::<f64>() < client.success_rate;

        Ok(ExecutionResult {
            success,
            tx_hash: if success {
                Some(format!(
                    "0x{}",
                    hex::encode(&tx.tx_data[..8.min(tx.tx_data.len())])
                ))
            } else {
                None
            },
            resources_used: if success { 21000 } else { 0 }, // Mock gas units
            error: if !success {
                Some("Mock Reth execution failure".to_string())
            } else {
                None
            },
        })
    }

    /// Execute transaction through Solana process
    async fn execute_solana_transaction(
        &self,
        tx: &TransactionExecution,
    ) -> MultivmResult<ExecutionResult> {
        let client = self.solana_client.read().await;

        // Simulate IPC call to Solana process
        debug!("[MOCK] Sending transaction to Solana process: {}", tx.tx_id);
        tokio::time::sleep(tokio::time::Duration::from_millis(client.latency_ms)).await;

        // Simulate execution result
        let success = rand::random::<f64>() < client.success_rate;

        Ok(ExecutionResult {
            success,
            tx_hash: if success {
                Some(format!(
                    "sol_{}",
                    bs58::encode(&tx.tx_data[..8.min(tx.tx_data.len())]).into_string()
                ))
            } else {
                None
            },
            resources_used: if success { 5000 } else { 0 }, // Mock compute units
            error: if !success {
                Some("Mock Solana execution failure".to_string())
            } else {
                None
            },
        })
    }

    /// Get current execution state
    pub async fn get_execution_state(&self) -> ExecutionState {
        let state = self.execution_state.read().await;
        ExecutionState {
            last_executed_height: state.last_executed_height,
            pending_executions: state.pending_executions.clone(),
            execution_history: state.execution_history.clone(),
        }
    }
}

impl Default for ProcessConsensusConfig {
    fn default() -> Self {
        Self {
            consensus_config: MalachiteConfig::default(),
            execution_config: ProcessExecutionConfig::default(),
            enable_batching: false,
            max_batch_size: 10,
            batch_timeout_ms: 1000,
        }
    }
}

impl Default for ProcessExecutionConfig {
    fn default() -> Self {
        Self {
            reth_endpoint: "127.0.0.1:8545".to_string(),
            solana_endpoint: "127.0.0.1:8899".to_string(),
            execution_timeout_ms: 30000,
            max_retries: 3,
        }
    }
}

/// Helper to determine execution state
impl ExecutionState {
    /// Get execution statistics
    pub fn get_stats(&self) -> ExecutionStats {
        let total_blocks = self.execution_history.len();
        let total_transactions: usize = self
            .execution_history
            .iter()
            .map(|r| r.total_transactions)
            .sum();
        let total_successful: usize = self.execution_history.iter().map(|r| r.successful).sum();
        let total_failed: usize = self.execution_history.iter().map(|r| r.failed).sum();
        let avg_duration_ms = if total_blocks > 0 {
            self.execution_history
                .iter()
                .map(|r| r.duration_ms)
                .sum::<u64>()
                / total_blocks as u64
        } else {
            0
        };

        ExecutionStats {
            total_blocks,
            total_transactions,
            total_successful,
            total_failed,
            success_rate: if total_transactions > 0 {
                total_successful as f64 / total_transactions as f64
            } else {
                0.0
            },
            avg_block_execution_time_ms: avg_duration_ms,
        }
    }
}

/// Execution statistics
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub total_blocks: usize,
    pub total_transactions: usize,
    pub total_successful: usize,
    pub total_failed: usize,
    pub success_rate: f64,
    pub avg_block_execution_time_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_consensus_coordinator_creation() {
        let config = ProcessConsensusConfig::default();
        let coordinator = ProcessConsensusCoordinator::new(config).await;
        assert!(coordinator.is_ok());
    }

    #[tokio::test]
    async fn test_process_execution_coordinator() {
        let config = ProcessExecutionConfig::default();
        let executor = ProcessExecutionCoordinator::new(config).unwrap();

        // Create test block
        let mut block = MultiVMBlock::new(1, "0x0".to_string(), "test-node".to_string(), vec![]);

        // Add EVM transaction
        block.add_evm_transaction(crate::block::EvmTransaction::new(
            "0x1234".to_string(),
            Some("0x5678".to_string()),
            1000,
            21000,
            20,
            vec![1, 2, 3, 4],
            1,
        ));

        // Add SVM transaction
        block.add_svm_transaction(crate::block::SvmTransaction::new(
            vec!["sig1".to_string()],
            vec![5, 6, 7],
            vec!["account1".to_string()],
        ));

        // Execute block
        let result = executor.execute_block(block).await;
        assert!(result.is_ok());

        // Check execution state
        let state = executor.get_execution_state().await;
        assert_eq!(state.last_executed_height, 1);
        assert_eq!(state.execution_history.len(), 1);

        let stats = state.get_stats();
        assert_eq!(stats.total_blocks, 1);
        assert_eq!(stats.total_transactions, 2);
    }
}
