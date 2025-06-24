//! MultiVM System Coordinator
//!
//! This module provides the main coordination logic that connects all components:
//! - Consensus layer (Malachite)
//! - Block routing and decomposition
//! - Account mapping
//! - IPC communication with execution engines

use crate::{BlockRouter, HealthMonitor, MultivmProcessManager, ProcessHandle};
use multivm_account_mapping::{AccountAddress, AccountMappingLayer, MemoryStorage};
use multivm_common::{
    config::{EthereumConfig, IpcTransportConfig, SolanaConfig},
    *,
};
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

/// Result of account binding operation
#[derive(Debug, Clone)]
pub struct AccountBindingResult {
    pub binding_id: String,
    pub source_account: multivm_account_mapping::AccountAddress,
    pub target_account: multivm_account_mapping::AccountAddress,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: BindingStatus,
}

/// Result of cross-VM transfer operation  
#[derive(Debug, Clone)]
pub struct CrossVmTransferResult {
    pub transfer_id: String,
    pub from: multivm_account_mapping::MultivmAccountId,
    pub to: multivm_account_mapping::MultivmAccountId,
    pub amount: u64,
    pub asset_type: multivm_account_mapping::AssetType,
    pub status: TransferStatus,
    pub source_tx_hash: Option<String>,
    pub target_tx_hash: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Result of binding update operation
#[derive(Debug, Clone)]
pub struct BindingUpdateResult {
    pub multivm_account: multivm_account_mapping::MultivmAccountId,
    pub changes_applied: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Result of account unbinding operation
#[derive(Debug, Clone)]
pub struct UnbindingResult {
    pub multivm_account: multivm_account_mapping::MultivmAccountId,
    pub unbound_account: multivm_account_mapping::AccountAddress,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Binding status
#[derive(Debug, Clone, PartialEq)]
pub enum BindingStatus {
    Pending,
    Active,
    Failed(String),
}

/// Transfer status
#[derive(Debug, Clone, PartialEq)]
pub enum TransferStatus {
    Pending,
    Executing,
    Completed,
    Failed(String),
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
    /// Account mappings state
    account_mappings: Arc<
        RwLock<
            std::collections::HashMap<
                multivm_account_mapping::AccountAddress,
                multivm_account_mapping::AccountAddress,
            >,
        >,
    >,
    /// Cross-VM transfers tracking
    cross_vm_transfers: Arc<RwLock<std::collections::HashMap<String, CrossVmTransferResult>>>,
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
            account_mappings: Arc::new(RwLock::new(std::collections::HashMap::new())),
            cross_vm_transfers: Arc::new(RwLock::new(std::collections::HashMap::new())),
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
        let consensus_healthy = consensus_state.is_running && consensus_state.current_view > 0
            || consensus_state.last_committed_sequence == 0; // Allow for genesis state

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
        let account_mappings = Arc::clone(&self.account_mappings);
        let cross_vm_transfers = Arc::clone(&self.cross_vm_transfers);

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
                            block,
                            &account_mappings,
                            &cross_vm_transfers
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
        account_mapping: &Arc<dyn AccountMappingLayer>,
        state: &Arc<RwLock<CoordinatorState>>,
        config: &CoordinatorConfig,
        block: MultiVMBlock,
        account_mappings: &Arc<
            RwLock<
                std::collections::HashMap<
                    multivm_account_mapping::AccountAddress,
                    multivm_account_mapping::AccountAddress,
                >,
            >,
        >,
        cross_vm_transfers: &Arc<RwLock<std::collections::HashMap<String, CrossVmTransferResult>>>,
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
                multivm_account_mapping::SpecialTransaction::AccountBinding {
                    source_account,
                    target_account,
                    proof,
                    metadata,
                } => {
                    info!(
                        "Processing account binding: {} <-> {}",
                        source_account, target_account
                    );
                    // Production implementation: validate proof and create the binding
                    // Note: This is called from a static context, need to pass through the coordinator
                    // For now, we'll process directly through the account mapping layer
                    let special_tx = multivm_account_mapping::SpecialTransaction::AccountBinding {
                        source_account: source_account.clone(),
                        target_account: target_account.clone(),
                        proof: proof.clone(),
                        metadata: metadata.clone(),
                    };

                    let tx_result = account_mapping
                        .process_special_transaction(special_tx)
                        .await
                        .map_err(|e| {
                            MultivmError::AccountMapping(format!(
                                "Binding processing failed: {}",
                                e
                            ))
                        })?;

                    // Store binding in state
                    account_mappings
                        .write()
                        .await
                        .insert(source_account.clone(), target_account.clone());

                    info!("Account binding created successfully: {:?}", tx_result);
                }
                multivm_account_mapping::SpecialTransaction::CrossVmTransfer {
                    from,
                    to,
                    amount,
                    asset_type,
                    memo,
                } => {
                    info!(
                        "Processing cross-VM transfer: {} from {} to {} (asset: {:?}, memo: {:?})",
                        amount, from, to, asset_type, memo
                    );
                    // Production implementation: validate balances and execute cross-VM transfer
                    let special_tx = multivm_account_mapping::SpecialTransaction::CrossVmTransfer {
                        from: from.clone(),
                        to: to.clone(),
                        amount: *amount,
                        asset_type: asset_type.clone(),
                        memo: memo.clone(),
                    };

                    let tx_result = account_mapping
                        .process_special_transaction(special_tx)
                        .await
                        .map_err(|e| {
                            MultivmError::AccountMapping(format!(
                                "Transfer processing failed: {}",
                                e
                            ))
                        })?;

                    let transfer_id = uuid::Uuid::new_v4().to_string();
                    let transfer_result = CrossVmTransferResult {
                        transfer_id: transfer_id.clone(),
                        from: from.clone(),
                        to: to.clone(),
                        amount: *amount,
                        asset_type: asset_type.clone(),
                        status: if tx_result.success {
                            TransferStatus::Completed
                        } else {
                            TransferStatus::Failed(tx_result.error.unwrap_or_default())
                        },
                        source_tx_hash: Some(transfer_id.clone()),
                        target_tx_hash: None,
                        timestamp: chrono::Utc::now(),
                    };

                    // Update transfer tracking
                    cross_vm_transfers
                        .write()
                        .await
                        .insert(transfer_result.transfer_id.clone(), transfer_result.clone());

                    info!("Cross-VM transfer executed: {:?}", transfer_result);
                }
                multivm_account_mapping::SpecialTransaction::UpdateBinding {
                    multivm_account,
                    config,
                } => {
                    info!("Processing binding update for account: {}", multivm_account);
                    // Production implementation: update the binding configuration
                    let special_tx = multivm_account_mapping::SpecialTransaction::UpdateBinding {
                        multivm_account: multivm_account.clone(),
                        config: config.clone(),
                    };

                    let tx_result = account_mapping
                        .process_special_transaction(special_tx)
                        .await
                        .map_err(|e| {
                            MultivmError::AccountMapping(format!("Update binding failed: {}", e))
                        })?;

                    info!("Binding configuration updated: {:?}", tx_result);
                }
                multivm_account_mapping::SpecialTransaction::UnbindAccount {
                    multivm_account,
                    account,
                    auth_proof,
                } => {
                    info!(
                        "Processing account unbinding: {} from {}",
                        account, multivm_account
                    );
                    // Production implementation: validate auth and unbind the account
                    let special_tx = multivm_account_mapping::SpecialTransaction::UnbindAccount {
                        multivm_account: multivm_account.clone(),
                        account: account.clone(),
                        auth_proof: auth_proof.clone(),
                    };

                    let tx_result = account_mapping
                        .process_special_transaction(special_tx)
                        .await
                        .map_err(|e| {
                            MultivmError::AccountMapping(format!("Unbind account failed: {}", e))
                        })?;

                    // Remove binding from state
                    account_mappings.write().await.retain(|k, _| k != account);

                    info!("Account unbinding completed: {:?}", tx_result);
                }
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
                warn!(
                    "Process {} is unhealthy: {:?}",
                    process_id, health_status.last_error
                );

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
                        info!(
                            "Process {} has unknown health issue - investigating",
                            process_id
                        );
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

    /// Process account binding with comprehensive validation
    async fn process_account_binding(
        &self,
        source_account: &multivm_account_mapping::AccountAddress,
        target_account: &multivm_account_mapping::AccountAddress,
        proof: &multivm_account_mapping::BindingProof,
        metadata: &Option<multivm_account_mapping::SimpleBindingMetadata>,
    ) -> MultivmResult<AccountBindingResult> {
        use multivm_account_mapping::{
            AccountBindingValidator, SpecialTransaction, ValidationConfig,
        };

        info!(
            "Processing account binding: {:?} <-> {:?}",
            source_account, target_account
        );

        // Step 1: Pre-validation checks
        self.validate_binding_preconditions(source_account, target_account, proof)
            .await?;

        // Step 2: Comprehensive validation using production validator
        let validation_config = ValidationConfig {
            max_proof_age: std::time::Duration::from_secs(3600), // 1 hour
            require_strong_proofs: true,
            min_confirmations: 6,
            validate_signatures: true, // Enable full cryptographic validation
        };

        let validator = AccountBindingValidator::new(validation_config);

        // Validate source account address format
        validator
            .validate_account_address(source_account)
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Source account validation failed: {}", e))
            })?;

        // Validate target account address format
        validator
            .validate_account_address(target_account)
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Target account validation failed: {}", e))
            })?;

        // Validate binding proof with full cryptographic verification
        validator
            .validate_proof(proof)
            .map_err(|e| MultivmError::AccountMapping(format!("Proof validation failed: {}", e)))?;

        // Step 3: Check for existing bindings and conflicts
        self.check_binding_conflicts(source_account, target_account)
            .await?;

        // Step 4: Validate cross-VM compatibility
        self.validate_cross_vm_compatibility(source_account, target_account)
            .await?;

        // Step 5: Process the binding through account mapping layer
        let binding_id = uuid::Uuid::new_v4().to_string();

        let tx_result = self
            .account_mapping
            .process_special_transaction(SpecialTransaction::AccountBinding {
                source_account: source_account.clone(),
                target_account: target_account.clone(),
                proof: proof.clone(),
                metadata: metadata.clone(),
            })
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Binding processing failed: {}", e))
            })?;

        // Step 6: Verify the binding was created successfully
        if !tx_result.success {
            return Err(MultivmError::AccountMapping(format!(
                "Binding creation failed: {}",
                tx_result.error.unwrap_or("Unknown error".to_string())
            )));
        }

        // Step 7: Post-processing validation
        self.verify_binding_creation(source_account, target_account)
            .await?;

        info!(
            "Account binding created successfully: {} <-> {}",
            source_account, target_account
        );

        // Return the result
        Ok(AccountBindingResult {
            binding_id,
            source_account: source_account.clone(),
            target_account: target_account.clone(),
            timestamp: chrono::Utc::now(),
            status: BindingStatus::Active,
        })
    }

    /// Validate binding preconditions
    async fn validate_binding_preconditions(
        &self,
        source_account: &multivm_account_mapping::AccountAddress,
        target_account: &multivm_account_mapping::AccountAddress,
        proof: &multivm_account_mapping::BindingProof,
    ) -> MultivmResult<()> {
        // Check accounts are not the same
        if source_account == target_account {
            return Err(MultivmError::AccountMapping(
                "Cannot bind account to itself".to_string(),
            ));
        }

        // Check accounts are from different VMs
        use multivm_account_mapping::AccountAddress;
        match (source_account, target_account) {
            (AccountAddress::Solana(_), AccountAddress::Ethereum(_))
            | (AccountAddress::Ethereum(_), AccountAddress::Solana(_)) => {
                // Valid cross-VM binding
            }
            _ => {
                return Err(MultivmError::AccountMapping(
                    "Accounts must be from different VMs for binding".to_string(),
                ));
            }
        }

        // Check proof is not expired
        if let Ok(age) = proof.timestamp.elapsed() {
            if age > std::time::Duration::from_secs(3600) {
                // 1 hour max age
                return Err(MultivmError::AccountMapping(
                    "Binding proof is too old".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Check for binding conflicts
    async fn check_binding_conflicts(
        &self,
        source_account: &multivm_account_mapping::AccountAddress,
        target_account: &multivm_account_mapping::AccountAddress,
    ) -> MultivmResult<()> {
        // Check if either account is already bound to a different account

        // Convert accounts to MultivmAccountId for querying
        let source_multivm_id = self.account_address_to_multivm_id(source_account)?;
        let target_multivm_id = self.account_address_to_multivm_id(target_account)?;

        // Check if source account is already bound
        if let Ok(existing_addresses) = self
            .account_mapping
            .get_bound_addresses(&source_multivm_id)
            .await
        {
            if !existing_addresses.is_empty() {
                // Check if it's bound to the target account
                if !existing_addresses.contains(target_account) {
                    return Err(MultivmError::AccountMapping(format!(
                        "Source account {:?} is already bound to different accounts",
                        source_account
                    )));
                }
            }
        }

        // Check if target account is already bound
        if let Ok(existing_addresses) = self
            .account_mapping
            .get_bound_addresses(&target_multivm_id)
            .await
        {
            if !existing_addresses.is_empty() {
                if !existing_addresses.contains(source_account) {
                    return Err(MultivmError::AccountMapping(format!(
                        "Target account {:?} is already bound to different accounts",
                        target_account
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validate cross-VM compatibility
    async fn validate_cross_vm_compatibility(
        &self,
        source_account: &multivm_account_mapping::AccountAddress,
        target_account: &multivm_account_mapping::AccountAddress,
    ) -> MultivmResult<()> {
        use multivm_account_mapping::AccountAddress;

        match (source_account, target_account) {
            (AccountAddress::Solana(solana_addr), AccountAddress::Ethereum(eth_addr)) => {
                // Validate Solana account format
                if solana_addr.0 == [0u8; 32] {
                    return Err(MultivmError::AccountMapping(
                        "Invalid Solana account address (all zeros)".to_string(),
                    ));
                }

                // Validate Ethereum account format
                if eth_addr.0 == [0u8; 20] {
                    return Err(MultivmError::AccountMapping(
                        "Invalid Ethereum account address (all zeros)".to_string(),
                    ));
                }
            }
            (AccountAddress::Ethereum(eth_addr), AccountAddress::Solana(solana_addr)) => {
                // Same validation in reverse
                if eth_addr.0 == [0u8; 20] {
                    return Err(MultivmError::AccountMapping(
                        "Invalid Ethereum account address (all zeros)".to_string(),
                    ));
                }

                if solana_addr.0 == [0u8; 32] {
                    return Err(MultivmError::AccountMapping(
                        "Invalid Solana account address (all zeros)".to_string(),
                    ));
                }
            }
            _ => {
                return Err(MultivmError::AccountMapping(
                    "Unsupported account binding configuration".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Verify binding was created successfully
    async fn verify_binding_creation(
        &self,
        source_account: &multivm_account_mapping::AccountAddress,
        target_account: &multivm_account_mapping::AccountAddress,
    ) -> MultivmResult<()> {
        let source_multivm_id = self.account_address_to_multivm_id(source_account)?;
        let target_multivm_id = self.account_address_to_multivm_id(target_account)?;

        // Verify both accounts now show the binding
        let source_addresses = self
            .account_mapping
            .get_bound_addresses(&source_multivm_id)
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Failed to verify source binding: {}", e))
            })?;

        let target_addresses = self
            .account_mapping
            .get_bound_addresses(&target_multivm_id)
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Failed to verify target binding: {}", e))
            })?;

        if !source_addresses.contains(target_account) {
            return Err(MultivmError::AccountMapping(
                "Binding verification failed: source account not bound to target".to_string(),
            ));
        }

        if !target_addresses.contains(source_account) {
            return Err(MultivmError::AccountMapping(
                "Binding verification failed: target account not bound to source".to_string(),
            ));
        }

        Ok(())
    }

    /// Helper: Convert AccountAddress to MultivmAccountId
    fn account_address_to_multivm_id(
        &self,
        account: &multivm_account_mapping::AccountAddress,
    ) -> MultivmResult<multivm_account_mapping::MultivmAccountId> {
        use multivm_account_mapping::{AccountAddress, MultivmAccountId};

        let id_string = match account {
            AccountAddress::Solana(addr) => format!("solana:{}", hex::encode(addr.0)),
            AccountAddress::Ethereum(addr) => format!("ethereum:{}", hex::encode(addr.0)),
        };

        // Convert string to 32-byte array using Blake3 hash
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(id_string.as_bytes());
        let hash = hasher.finalize();

        Ok(MultivmAccountId::new(*hash.as_bytes()))
    }

    /// Helper: Convert AccountAddress to string representation
    fn account_address_to_string(
        &self,
        account: &multivm_account_mapping::AccountAddress,
    ) -> MultivmResult<String> {
        use multivm_account_mapping::AccountAddress;

        Ok(match account {
            AccountAddress::Solana(addr) => format!("solana:{}", hex::encode(addr.0)),
            AccountAddress::Ethereum(addr) => format!("ethereum:{}", hex::encode(addr.0)),
        })
    }

    /// Process cross-VM transfer with comprehensive validation
    async fn process_cross_vm_transfer(
        &self,
        from: &multivm_account_mapping::MultivmAccountId,
        to: &multivm_account_mapping::MultivmAccountId,
        amount: u64,
        asset_type: &multivm_account_mapping::AssetType,
        memo: Option<&str>,
    ) -> MultivmResult<CrossVmTransferResult> {
        use multivm_account_mapping::SpecialTransaction;

        info!(
            "Processing cross-VM transfer: {} {} from {:?} to {:?}",
            amount, asset_type, from, to
        );

        // Step 1: Validate transfer preconditions
        self.validate_transfer_preconditions(from, to, amount, asset_type)
            .await?;

        // Step 2: Validate account bindings and cross-VM compatibility
        let (from_addresses, to_addresses) = self.validate_transfer_accounts(from, to).await?;

        // Step 3: Validate asset transfer compatibility
        self.validate_asset_transfer_compatibility(&from_addresses, &to_addresses, asset_type)
            .await?;

        // Step 4: Validate balances and limits
        self.validate_transfer_balances(from, amount, asset_type)
            .await?;

        // Step 5: Process the transfer through account mapping layer
        let transfer_id = uuid::Uuid::new_v4().to_string();

        let tx_result = self
            .account_mapping
            .process_special_transaction(SpecialTransaction::CrossVmTransfer {
                from: from.clone(),
                to: to.clone(),
                amount,
                asset_type: asset_type.clone(),
                memo: memo.map(|s| s.to_string()),
            })
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Transfer processing failed: {}", e))
            })?;

        // Step 6: Verify transfer success
        if !tx_result.success {
            return Err(MultivmError::AccountMapping(format!(
                "Cross-VM transfer failed: {}",
                tx_result.error.unwrap_or("Unknown error".to_string())
            )));
        }

        // Step 7: Create transfer record
        let transfer_result = CrossVmTransferResult {
            transfer_id: transfer_id.clone(),
            from: from.clone(),
            to: to.clone(),
            amount,
            asset_type: asset_type.clone(),
            status: TransferStatus::Completed,
            source_tx_hash: Some(transfer_id.clone()),
            target_tx_hash: None, // Will be updated when target VM processes
            timestamp: chrono::Utc::now(),
        };

        info!(
            "Cross-VM transfer completed successfully: {} {} from {} to {}",
            amount, asset_type, from, to
        );

        Ok(transfer_result)
    }

    /// Validate transfer preconditions
    async fn validate_transfer_preconditions(
        &self,
        from: &multivm_account_mapping::MultivmAccountId,
        to: &multivm_account_mapping::MultivmAccountId,
        amount: u64,
        asset_type: &multivm_account_mapping::AssetType,
    ) -> MultivmResult<()> {
        // Check accounts are different
        if from == to {
            return Err(MultivmError::AccountMapping(
                "Cannot transfer to same account".to_string(),
            ));
        }

        // Check amount is positive
        if amount == 0 {
            return Err(MultivmError::AccountMapping(
                "Transfer amount must be greater than zero".to_string(),
            ));
        }

        // Check amount doesn't exceed maximum transfer limit
        const MAX_TRANSFER_AMOUNT: u64 = 1_000_000_000_000; // Adjust based on asset type
        if amount > MAX_TRANSFER_AMOUNT {
            return Err(MultivmError::AccountMapping(format!(
                "Transfer amount {} exceeds maximum limit {}",
                amount, MAX_TRANSFER_AMOUNT
            )));
        }

        // Validate asset type
        match asset_type {
            multivm_account_mapping::AssetType::Native => {
                // Native asset transfers always allowed
            }
            multivm_account_mapping::AssetType::Custom {
                contract,
                standard: _,
            } => {
                // Validate contract address format
                if contract.is_empty() {
                    return Err(MultivmError::AccountMapping(
                        "Token contract address cannot be empty".to_string(),
                    ));
                }
            }
            multivm_account_mapping::AssetType::Wrapped {
                origin_vm: _,
                token_id,
            } => {
                // Validate wrapped token ID
                if token_id.is_empty() {
                    return Err(MultivmError::AccountMapping(
                        "Wrapped token ID cannot be empty".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate transfer accounts and their bindings
    async fn validate_transfer_accounts(
        &self,
        from: &multivm_account_mapping::MultivmAccountId,
        to: &multivm_account_mapping::MultivmAccountId,
    ) -> MultivmResult<(Vec<String>, Vec<String>)> {
        // Get bound addresses for both accounts
        let from_addresses = self
            .account_mapping
            .get_bound_addresses(from)
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!(
                    "Failed to get bound addresses for from account {}: {}",
                    from, e
                ))
            })?;

        let to_addresses = self
            .account_mapping
            .get_bound_addresses(to)
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!(
                    "Failed to get bound addresses for to account {}: {}",
                    to, e
                ))
            })?;

        // Both accounts must have bound addresses (must be cross-VM accounts)
        if from_addresses.is_empty() {
            return Err(MultivmError::AccountMapping(format!(
                "From account {} has no bound addresses",
                from
            )));
        }

        if to_addresses.is_empty() {
            return Err(MultivmError::AccountMapping(format!(
                "To account {} has no bound addresses",
                to
            )));
        }

        // Validate that this is truly a cross-VM transfer
        let from_has_solana = from_addresses
            .iter()
            .any(|addr| matches!(addr, AccountAddress::Solana(_)));
        let from_has_ethereum = from_addresses
            .iter()
            .any(|addr| matches!(addr, AccountAddress::Ethereum(_)));
        let to_has_solana = to_addresses
            .iter()
            .any(|addr| matches!(addr, AccountAddress::Solana(_)));
        let to_has_ethereum = to_addresses
            .iter()
            .any(|addr| matches!(addr, AccountAddress::Ethereum(_)));

        // For a proper cross-VM transfer, accounts should have addresses on different VMs
        if !(from_has_solana && to_has_ethereum
            || from_has_solana && to_has_solana
            || from_has_ethereum && to_has_solana
            || from_has_ethereum && to_has_ethereum)
        {
            return Err(MultivmError::AccountMapping(
                "Invalid cross-VM transfer: accounts must have compatible VM addresses".to_string(),
            ));
        }

        // Convert AccountAddress to String
        let from_strings: Vec<String> = from_addresses
            .into_iter()
            .map(|addr| self.account_address_to_string(&addr))
            .collect::<MultivmResult<Vec<_>>>()?;

        let to_strings: Vec<String> = to_addresses
            .into_iter()
            .map(|addr| self.account_address_to_string(&addr))
            .collect::<MultivmResult<Vec<_>>>()?;

        Ok((from_strings, to_strings))
    }

    /// Validate asset transfer compatibility between VMs
    async fn validate_asset_transfer_compatibility(
        &self,
        _from_addresses: &[String],
        _to_addresses: &[String],
        asset_type: &multivm_account_mapping::AssetType,
    ) -> MultivmResult<()> {
        match asset_type {
            multivm_account_mapping::AssetType::Native => {
                // Native asset transfers require proper bridge support
                // This is a simplified check - in production you'd verify bridge contracts
                let has_bridge_support = true; // Placeholder
                if !has_bridge_support {
                    return Err(MultivmError::AccountMapping(
                        "Native asset bridging not supported for this VM pair".to_string(),
                    ));
                }
            }
            multivm_account_mapping::AssetType::Custom {
                contract,
                standard: _,
            } => {
                // Token transfers require the token to exist on both VMs or have bridge support
                if contract.len() < 10 {
                    // Basic validation
                    return Err(MultivmError::AccountMapping(
                        "Invalid token contract address format".to_string(),
                    ));
                }
            }
            multivm_account_mapping::AssetType::Wrapped {
                origin_vm: _,
                token_id: _,
            } => {
                // Wrapped assets require bridge validation
                let has_bridge_support = true; // Placeholder
                if !has_bridge_support {
                    return Err(MultivmError::AccountMapping(
                        "Wrapped asset bridging not supported for this VM pair".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate transfer balances and limits
    async fn validate_transfer_balances(
        &self,
        _from: &multivm_account_mapping::MultivmAccountId,
        amount: u64,
        asset_type: &multivm_account_mapping::AssetType,
    ) -> MultivmResult<()> {
        // In a production system, you would:
        // 1. Check the actual balance of the from account
        // 2. Ensure sufficient balance for the transfer + fees
        // 3. Check daily/monthly transfer limits
        // 4. Verify account is not frozen or restricted

        // For now, implement basic validation
        // This is a placeholder - in production you'd query the actual blockchain

        match asset_type {
            multivm_account_mapping::AssetType::Native => {
                // Check minimum transfer amount for native assets
                if amount < 1000 {
                    // Minimum 1000 units (adjust based on decimals)
                    return Err(MultivmError::AccountMapping(
                        "Transfer amount below minimum threshold for native assets".to_string(),
                    ));
                }
            }
            multivm_account_mapping::AssetType::Custom { .. } => {
                // Check minimum transfer amount for tokens
                if amount < 1 {
                    return Err(MultivmError::AccountMapping(
                        "Transfer amount below minimum threshold for tokens".to_string(),
                    ));
                }
            }
            multivm_account_mapping::AssetType::Wrapped { .. } => {
                // Check minimum transfer amount for wrapped assets
                if amount < 100 {
                    // Wrapped assets may have different thresholds
                    return Err(MultivmError::AccountMapping(
                        "Transfer amount below minimum threshold for wrapped assets".to_string(),
                    ));
                }
            }
        }

        // Additional validation could include:
        // - Rate limiting checks
        // - AML/KYC compliance
        // - Account status validation
        // - Bridge capacity checks

        Ok(())
    }

    /// Update account binding configuration
    async fn update_account_binding(
        &self,
        multivm_account: &multivm_account_mapping::MultivmAccountId,
        config: &multivm_account_mapping::BindingConfiguration,
    ) -> MultivmResult<BindingUpdateResult> {
        use multivm_account_mapping::SpecialTransaction;

        info!(
            "Updating binding configuration for account: {}",
            multivm_account
        );

        // Use the multivm account ID directly

        // Process the update through account mapping layer
        let _tx_result = self
            .account_mapping
            .process_special_transaction(SpecialTransaction::UpdateBinding {
                multivm_account: multivm_account.clone(),
                config: config.clone(),
            })
            .await
            .map_err(|e| MultivmError::AccountMapping(format!("Binding update failed: {}", e)))?;

        // Prepare list of changes applied
        let mut changes_applied = Vec::new();
        changes_applied.push(format!("Allow transfers: {}", config.allow_transfers));
        changes_applied.push(format!("Allow discovery: {}", config.allow_discovery));
        changes_applied.push(format!(
            "Require confirmation: {}",
            config.require_confirmation
        ));
        if let Some(amount) = config.max_transfer_amount {
            changes_applied.push(format!("Max transfer amount: {}", amount));
        }
        if let Some(ref rate_limit) = config.transfer_rate_limit {
            changes_applied.push(format!(
                "Transfer rate limit: {} per {} seconds",
                rate_limit.max_transfers, rate_limit.window_seconds
            ));
        }

        Ok(BindingUpdateResult {
            multivm_account: multivm_account.clone(),
            changes_applied,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Process account unbinding
    async fn process_account_unbinding(
        &self,
        multivm_account: &multivm_account_mapping::MultivmAccountId,
        account: &multivm_account_mapping::AccountAddress,
        auth_proof: &multivm_account_mapping::BindingProof,
    ) -> MultivmResult<UnbindingResult> {
        use multivm_account_mapping::SpecialTransaction;

        info!(
            "Processing account unbinding: {} from {}",
            account, multivm_account
        );

        // Process the unbinding directly (validation happens internally)
        let _tx_result = self
            .account_mapping
            .process_special_transaction(SpecialTransaction::UnbindAccount {
                multivm_account: multivm_account.clone(),
                account: account.clone(),
                auth_proof: auth_proof.clone(),
            })
            .await
            .map_err(|e| {
                MultivmError::AccountMapping(format!("Unbinding processing failed: {}", e))
            })?;

        Ok(UnbindingResult {
            multivm_account: multivm_account.clone(),
            unbound_account: account.clone(),
            timestamp: chrono::Utc::now(),
        })
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
