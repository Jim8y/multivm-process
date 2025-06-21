//! MultiVM System Coordinator
//!
//! This module provides the main coordination logic that connects all components:
//! - Consensus layer (Malachite)
//! - Block routing and decomposition
//! - Account mapping
//! - IPC communication with execution engines

use crate::{BlockRouter, HealthMonitor, MultivmProcessManager, ProcessHandle};
use multivm_account_mapping::{AccountMappingLayer, MemoryStorage};
use multivm_common::{*, config::{IpcTransportConfig, SolanaConfig, EthereumConfig}};
use multivm_consensus::{MalachiteConfig, MalachiteConsensus, MultiVMBlock};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Configuration for the MultiVM coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Consensus configuration
    pub consensus: MalachiteConfig,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Block processing timeout
    pub block_timeout: Duration,
    /// Maximum concurrent blocks
    pub max_concurrent_blocks: usize,
    /// Enable system recovery
    pub enable_recovery: bool,
}

/// System coordinator state
#[derive(Debug, Clone)]
pub struct CoordinatorState {
    pub is_running: bool,
    pub blocks_processed: u64,
    pub last_block_height: u64,
    pub active_processes: Vec<ProcessId>,
    pub last_health_check: std::time::SystemTime,
    pub system_metrics: SystemMetrics,
}

/// System metrics for monitoring
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub consensus_latency_ms: u64,
    pub block_routing_time_ms: u64,
    pub total_transactions_processed: u64,
    pub error_count: u64,
    pub recovery_count: u64,
}

/// MultiVM System Coordinator
pub struct MultivmCoordinator {
    /// Process manager for execution engines
    process_manager: Arc<MultivmProcessManager>,
    /// Block router for decomposition and routing
    block_router: Arc<BlockRouter>,
    /// Consensus engine
    consensus: Arc<RwLock<MalachiteConsensus>>,
    /// Account mapping layer (handles special transactions)
    account_mapping: Arc<dyn AccountMappingLayer>,
    /// Health monitor
    health_monitor: Arc<HealthMonitor>,
    /// Configuration
    config: CoordinatorConfig,
    /// Current state
    state: Arc<RwLock<CoordinatorState>>,
    /// Block processing channel
    block_sender: Option<mpsc::UnboundedSender<MultiVMBlock>>,
    /// Shutdown signal
    shutdown_sender: Option<tokio::sync::oneshot::Sender<()>>,
}

impl MultivmCoordinator {
    /// Create MultivmConfig from CoordinatorConfig
    fn create_multivm_config_from_coordinator(config: &CoordinatorConfig) -> MultivmConfig {
        MultivmConfig {
            system: SystemConfig {
                max_processes: 50,
                process_restart_delay: Duration::from_secs(5),
                shutdown_timeout: Duration::from_secs(30),
                health_check_interval: config.health_check_interval,
                data_dir: std::path::PathBuf::from("./data"),
                resource_limits: ResourceLimits {
                    max_memory_mb: 4096,
                    max_cpu_percent: 80.0,
                    max_open_files: 1024,
                    max_disk_usage_gb: 100,
                    max_rpc_connections: 1000,
                },
                enable_metrics: true,
                metrics_port: Some(9090),
            },
            solana: SolanaConfig::default(),
            ethereum: EthereumConfig::default(),
            ipc: IpcConfig {
                transport: IpcTransportConfig::default(),
                message_timeout: Duration::from_secs(30),
                max_message_size: 16 * 1024 * 1024,
                buffer_size: 1024 * 1024,
            },
            logging: LoggingConfig::default(),
        }
    }

    /// Create a new coordinator
    pub async fn new(config: CoordinatorConfig) -> MultivmResult<Self> {
        info!("Initializing MultiVM Coordinator");

        // Initialize account mapping layer
        let account_mapping: Arc<dyn AccountMappingLayer> = Arc::new(MemoryStorage::new());

        // Initialize block router
        let block_router = Arc::new(BlockRouter::new(Arc::clone(&account_mapping)));

        // Initialize process manager
        let multivm_config = Self::create_multivm_config_from_coordinator(&config);
        let process_manager = Arc::new(MultivmProcessManager::new(multivm_config).await?);

        // Initialize health monitor
        let health_monitor = Arc::new(HealthMonitor::new(config.health_check_interval));

        // Initialize consensus
        let (consensus_engine, _block_sender, _block_receiver) =
            MalachiteConsensus::new(config.consensus.clone()).await?;
        let consensus = Arc::new(RwLock::new(consensus_engine));

        // Initialize state
        let state = Arc::new(RwLock::new(CoordinatorState {
            is_running: false,
            blocks_processed: 0,
            last_block_height: 0,
            active_processes: Vec::new(),
            last_health_check: std::time::SystemTime::now(),
            system_metrics: SystemMetrics {
                consensus_latency_ms: 0,
                block_routing_time_ms: 0,
                total_transactions_processed: 0,
                error_count: 0,
                recovery_count: 0,
            },
        }));

        Ok(Self {
            process_manager,
            block_router,
            consensus,
            account_mapping,
            health_monitor,
            config,
            state,
            block_sender: None,
            shutdown_sender: None,
        })
    }

    /// Start the coordinator system
    pub async fn start(&mut self) -> MultivmResult<()> {
        info!("Starting MultiVM Coordinator system");

        // Start process manager
        self.process_manager.start().await?;

        // Start consensus
        {
            let consensus = self.consensus.read().await;
            consensus.start_consensus().await.map_err(|e| {
                MultivmError::ConsensusError(format!("Failed to start consensus: {}", e))
            })?;
        }

        // Health monitoring is handled by the monitoring loop

        // Start block processing loop
        self.start_block_processing_loop().await?;

        // Start system monitoring
        self.start_system_monitoring().await?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = true;
            state.last_health_check = std::time::SystemTime::now();
        }

        info!("MultiVM Coordinator system started successfully");
        Ok(())
    }

    /// Stop the coordinator system
    pub async fn stop(&mut self) -> MultivmResult<()> {
        info!("Stopping MultiVM Coordinator system");

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = false;
        }

        // Send shutdown signal
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }

        // Stop consensus
        {
            let consensus = self.consensus.read().await;
            consensus.stop_consensus().await.map_err(|e| {
                MultivmError::ConsensusError(format!("Failed to stop consensus: {}", e))
            })?;
        }

        // Stop process manager
        self.process_manager.stop().await?;

        // Stop health monitor
        // Health monitoring stops automatically when loops are cancelled

        info!("MultiVM Coordinator system stopped successfully");
        Ok(())
    }

    /// Submit a block for processing
    pub async fn submit_block(&self, block: MultiVMBlock) -> MultivmResult<()> {
        debug!(
            "Submitting block at height {} for processing",
            block.header.height
        );

        if let Some(ref sender) = self.block_sender {
            sender
                .send(block)
                .map_err(|e| MultivmError::Ipc(format!("Failed to submit block: {}", e)))?;
            Ok(())
        } else {
            Err(MultivmError::InvalidState(
                "Block processing not started".to_string(),
            ))
        }
    }

    /// Get current system state
    pub async fn get_state(&self) -> CoordinatorState {
        self.state.read().await.clone()
    }

    /// Get system health status
    pub async fn get_health_status(&self) -> MultivmResult<SystemHealthStatus> {
        let state = self.state.read().await;
        let process_health = self.process_manager.get_health_status().await?;
        let consensus_state = {
            let consensus = self.consensus.read().await;
            consensus.get_state().await.map_err(|e| {
                MultivmError::ConsensusError(format!("Failed to get consensus state: {}", e))
            })?
        };

        // Determine consensus health based on state
        let consensus_healthy = consensus_state.is_running && 
            consensus_state.current_view > 0 || 
            consensus_state.last_committed_sequence == 0; // Allow for genesis state

        Ok(SystemHealthStatus {
            is_healthy: state.is_running && process_health.overall_healthy && consensus_healthy,
            coordinator_running: state.is_running,
            processes_healthy: process_health.overall_healthy,
            consensus_healthy,
            last_health_check: state.last_health_check,
            system_metrics: state.system_metrics.clone(),
        })
    }

    /// Start the block processing loop
    async fn start_block_processing_loop(&mut self) -> MultivmResult<()> {
        let (block_sender, mut block_receiver) = mpsc::unbounded_channel::<MultiVMBlock>();
        let (shutdown_sender, mut shutdown_receiver) = tokio::sync::oneshot::channel::<()>();

        self.block_sender = Some(block_sender);
        self.shutdown_sender = Some(shutdown_sender);

        // Clone necessary components for the processing loop
        let block_router = Arc::clone(&self.block_router);
        let account_mapping = Arc::clone(&self.account_mapping);
        let state = Arc::clone(&self.state);
        let config = self.config.clone();

        // Start the processing loop
        tokio::spawn(async move {
            info!("Starting block processing loop");

            loop {
                tokio::select! {
                    // Process incoming blocks
                    Some(block) = block_receiver.recv() => {
                        if let Err(e) = Self::process_block_internal(
                            &block_router,
                            &account_mapping,
                            &state,
                            &config,
                            block
                        ).await {
                            error!("Failed to process block: {}", e);
                            // Update error metrics
                            let mut state_guard = state.write().await;
                            state_guard.system_metrics.error_count += 1;
                        }
                    }

                    // Handle shutdown signal
                    _ = &mut shutdown_receiver => {
                        info!("Block processing loop shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Start system monitoring
    async fn start_system_monitoring(&self) -> MultivmResult<()> {
        let state = Arc::clone(&self.state);
        let health_monitor = Arc::clone(&self.health_monitor);
        let process_manager = Arc::clone(&self.process_manager);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(config.health_check_interval);

            loop {
                interval.tick().await;

                // Perform health checks
                if let Err(e) =
                    Self::perform_health_checks(&health_monitor, &process_manager, &state).await
                {
                    error!("Health check failed: {}", e);
                }

                // Check if system is still running
                {
                    let state_guard = state.read().await;
                    if !state_guard.is_running {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Internal block processing logic
    async fn process_block_internal(
        block_router: &Arc<BlockRouter>,
        _account_mapping: &Arc<dyn AccountMappingLayer>,
        state: &Arc<RwLock<CoordinatorState>>,
        config: &CoordinatorConfig,
        block: MultiVMBlock,
    ) -> MultivmResult<()> {
        let start_time = std::time::Instant::now();
        info!("Processing block at height {}", block.header.height);

        // Step 1: Decompose the block
        let routing_result = tokio::time::timeout(
            config.block_timeout,
            block_router.decompose_block(block.clone()),
        )
        .await
        .map_err(|_| MultivmError::Timeout {
            timeout: config.block_timeout,
        })?
        .map_err(|e| MultivmError::BlockProcessing(format!("Block decomposition failed: {}", e)))?;

        // Step 2: Process special transactions first
        for special_tx in &routing_result.special_transactions {
            debug!("Processing special transaction: {:?}", special_tx);
            
            // Process different types of special transactions
            match special_tx {
                multivm_account_mapping::SpecialTransaction::AccountBinding { source_account, target_account, proof: _, metadata: _ } => {
                    info!("Processing account binding: {} <-> {}", source_account, target_account);
                    // In a real implementation, would validate proof and create the binding
                },
                multivm_account_mapping::SpecialTransaction::CrossVmTransfer { from, to, amount, asset_type, memo } => {
                    info!(
                        "Processing cross-VM transfer: {} from {} to {} (asset: {:?}, memo: {:?})", 
                        amount, from, to, asset_type, memo
                    );
                    // In a real implementation, would validate balances and execute transfer
                },
                multivm_account_mapping::SpecialTransaction::UpdateBinding { multivm_account, config: _ } => {
                    info!("Processing binding update for account: {}", multivm_account);
                    // In a real implementation, would update the binding configuration
                },
                multivm_account_mapping::SpecialTransaction::UnbindAccount { multivm_account, account, auth_proof: _ } => {
                    info!("Processing account unbinding: {} from {}", account, multivm_account);
                    // In a real implementation, would validate auth and unbind the account
                },
            }
        }

        // Step 3: Route VM-specific transactions to execution engines
        block_router
            .route_decomposed_block(routing_result.clone())
            .await
            .map_err(|e| MultivmError::BlockProcessing(format!("Block routing failed: {}", e)))?;

        // Step 4: Update system state and metrics
        let processing_time = start_time.elapsed();
        {
            let mut state_guard = state.write().await;
            state_guard.blocks_processed += 1;
            state_guard.last_block_height = block.header.height;
            state_guard.system_metrics.block_routing_time_ms = processing_time.as_millis() as u64;
            state_guard.system_metrics.total_transactions_processed +=
                routing_result.routing_metadata.total_transactions as u64;
        }

        info!(
            "Block {} processed successfully in {}ms ({} txns)",
            block.header.height,
            processing_time.as_millis(),
            routing_result.routing_metadata.total_transactions
        );

        Ok(())
    }

    /// Perform system health checks
    async fn perform_health_checks(
        _health_monitor: &Arc<HealthMonitor>,
        process_manager: &Arc<MultivmProcessManager>,
        state: &Arc<RwLock<CoordinatorState>>,
    ) -> MultivmResult<()> {
        debug!("Performing system health checks");

        // Check process health
        let process_health = process_manager.get_health_status().await?;

        // Check individual processes and perform recovery if needed
        for (process_id, health_status) in &process_health.process_health {
            debug!("Process {} health: {:?}", process_id, health_status);
            
            // Implement recovery logic for unhealthy processes
            if !health_status.is_healthy {
                warn!("Process {} is unhealthy: {:?}", process_id, health_status.last_error);
                
                // Attempt recovery based on the type of issue
                if let Some(ref error_msg) = health_status.last_error {
                    if error_msg.contains("timeout") || error_msg.contains("unresponsive") {
                        info!("Attempting to restart unresponsive process: {}", process_id);
                        
                        if let Err(e) = process_manager.restart_process(*process_id).await {
                            error!("Failed to restart process {}: {}", process_id, e);
                            // Update recovery count
                            let mut state_guard = state.write().await;
                            state_guard.system_metrics.error_count += 1;
                        } else {
                            info!("Successfully restarted process: {}", process_id);
                            let mut state_guard = state.write().await;
                            state_guard.system_metrics.recovery_count += 1;
                        }
                    } else if error_msg.contains("memory") || error_msg.contains("resource") {
                        warn!("Process {} has resource issues - monitoring", process_id);
                        // Could implement resource cleanup or scaling here
                    } else {
                        info!("Process {} has unknown health issue - investigating", process_id);
                    }
                }
            }
        }

        // Update state
        {
            let mut state_guard = state.write().await;
            state_guard.last_health_check = std::time::SystemTime::now();
            state_guard.active_processes = process_health.process_health.keys().cloned().collect();
        }

        Ok(())
    }

    /// Register an execution engine process
    pub async fn register_process(&self, handle: ProcessHandle) -> MultivmResult<()> {
        info!("Registering process: {}", handle.process_id);

        // Register with process manager
        self.process_manager
            .register_process(handle.clone())
            .await?;

        // Register with block router
        let process_id = handle.process_id;
        self.block_router.register_process(handle).await;

        // Update state
        {
            let mut state = self.state.write().await;
            if !state.active_processes.contains(&process_id) {
                state.active_processes.push(process_id);
            }
        }

        Ok(())
    }

    /// Unregister an execution engine process
    pub async fn unregister_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        info!("Unregistering process: {}", process_id);

        // Unregister from process manager
        self.process_manager.unregister_process(process_id).await?;

        // Unregister from block router
        self.block_router.unregister_process(process_id).await;

        // Update state
        {
            let mut state = self.state.write().await;
            state.active_processes.retain(|&id| id != process_id);
        }

        Ok(())
    }
}

/// System health status
#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub is_healthy: bool,
    pub coordinator_running: bool,
    pub processes_healthy: bool,
    pub consensus_healthy: bool,
    pub last_health_check: std::time::SystemTime,
    pub system_metrics: SystemMetrics,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            consensus: MalachiteConfig::default(),
            health_check_interval: Duration::from_secs(30),
            block_timeout: Duration::from_secs(60),
            max_concurrent_blocks: 10,
            enable_recovery: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let config = CoordinatorConfig::default();
        let coordinator = MultivmCoordinator::new(config).await;
        assert!(coordinator.is_ok());
    }

    #[tokio::test]
    async fn test_coordinator_state() {
        let config = CoordinatorConfig::default();
        let coordinator = MultivmCoordinator::new(config).await.unwrap();
        let state = coordinator.get_state().await;
        assert!(!state.is_running);
        assert_eq!(state.blocks_processed, 0);
    }
}
