use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::lock_ordering::{
    acquire_write_lock_safe, get_lock_config, init_lock_config, LockLevel, LockTimeoutConfig,
};
use multivm_common::{
    error::MultivmError,
    traits::ProcessManager as ProcessManagerTrait,
    types::{BlockchainType, HealthInfo, HealthStatus, ProcessId, RpcError, RpcResponse},
    types_rpc::RpcConfig,
    EngineState, IpcCommand, IpcResponse, MultivmConfig, MultivmResult, SystemEvent,
};

/// Events emitted by the process manager
#[derive(Debug, Clone)]
pub enum ProcessManagerEvent {
    ProcessStarted {
        process_id: ProcessId,
    },
    ProcessStopped {
        process_id: ProcessId,
    },
    ProcessFailed {
        process_id: ProcessId,
        error: String,
    },
    ProcessRestarted {
        process_id: ProcessId,
    },
    HealthCheckFailed {
        process_id: ProcessId,
        error: String,
    },
}

use crate::{
    resource_monitor::SystemResourceMonitor,
    zombie_reaper::{ZombieReaper, ZombieReaperConfig},
    /*BlockRouter,*/ HealthMonitor, ProcessHandle,
};
use std::path::PathBuf;

// Temporary compatibility types until we fully migrate to unified config
#[derive(Debug, Clone)]
pub struct SolanaExecutionConfig {
    pub enabled: bool,
    pub data_dir: PathBuf,
    pub rpc_config: Option<RpcConfig>,
    pub ledger_path: PathBuf,
    pub accounts_path: PathBuf,
    pub chain_id: u64,
}

impl Default for SolanaExecutionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            data_dir: PathBuf::from("/tmp/solana"),
            rpc_config: Some(RpcConfig {
                host: "127.0.0.1".to_string(),
                port: 8899,
                timeout_seconds: 30,
                rate_limit_requests_per_minute: Some(1000),
                cors_origins: vec!["*".to_string()],
                max_connections: 100,
            }),
            ledger_path: PathBuf::from("/tmp/solana/ledger"),
            accounts_path: PathBuf::from("/tmp/solana/accounts"),
            chain_id: 1,
        }
    }
}

// Use the ResourceLimits from multivm_common instead of defining our own
use multivm_common::ResourceLimits;

/// The main process manager that coordinates all blockchain engines
#[derive(Clone)]
pub struct MultivmProcessManager {
    inner: Arc<MultivmProcessManagerInner>,
}

struct MultivmProcessManagerInner {
    config: MultivmConfig,
    processes: Arc<RwLock<HashMap<ProcessId, ProcessHandle>>>,
    health_monitor: HealthMonitor,
    block_router: crate::block_router::BlockRouter,
    resource_monitor: Arc<Mutex<SystemResourceMonitor>>,
    zombie_reaper: ZombieReaper,
    ipc_server: Mutex<Option<tokio::task::JoinHandle<()>>>,
    event_handlers: RwLock<Vec<Box<dyn EventHandler>>>,
    shutdown_sender: Mutex<Option<oneshot::Sender<()>>>,
    metrics: Arc<Mutex<SystemMetrics>>,
    start_time: std::time::Instant,
    event_sender: Option<tokio::sync::mpsc::UnboundedSender<ProcessManagerEvent>>,
}

trait EventHandler: Send + Sync {
    fn handle_event(
        &self,
        event: &SystemEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>;
}

#[derive(Debug, Default)]
struct SystemMetrics {
    blocks_processed: HashMap<BlockchainType, u64>,
    total_processing_time: HashMap<BlockchainType, Duration>,
    error_count: HashMap<ProcessId, u64>,
    restart_count: HashMap<ProcessId, u64>,
    last_health_check: HashMap<ProcessId, std::time::Instant>,
}

impl MultivmProcessManager {
    /// Create a new process manager
    pub async fn new(config: MultivmConfig) -> MultivmResult<Self> {
        tracing::info!("Creating new MultivmProcessManager");

        // Initialize lock configuration for proper concurrency handling
        let lock_config = LockTimeoutConfig {
            default_timeout: Duration::from_secs(10),
            critical_timeout: Duration::from_secs(30),
            background_timeout: Duration::from_secs(5),
            max_retries: 3,
            backoff_multiplier: 1.5,
        };
        init_lock_config(lock_config);

        // Validate configuration
        config.validate().map_err(|e| MultivmError::Configuration {
            component: "system_config".to_string(),
            message: format!("Invalid system config: {e}"),
            validation_errors: Some(vec![]),
        })?;

        // Create data directories
        std::fs::create_dir_all(&config.system.data_dir).map_err(|e| {
            MultivmError::Configuration {
                component: "data_directory".to_string(),
                message: format!("Failed to create data directory: {e}"),
                validation_errors: Some(vec![]),
            }
        })?;

        // Initialize components
        let health_monitor = HealthMonitor::new(std::time::Duration::from_secs(30));
        // Initialize account mapping for block router
        let account_mapping = Arc::new(multivm_account_mapping::storage::MemoryStorage::new());
        let block_router = crate::block_router::BlockRouter::new(account_mapping);
        let resource_monitor = Arc::new(Mutex::new(SystemResourceMonitor::new(
            ResourceLimits::default(),
        )));

        // Initialize zombie reaper
        let zombie_reaper_config = ZombieReaperConfig::default();
        let zombie_reaper = ZombieReaper::new(zombie_reaper_config);

        let inner = MultivmProcessManagerInner {
            config,
            processes: Arc::new(RwLock::new(HashMap::new())),
            health_monitor,
            block_router,
            resource_monitor,
            zombie_reaper,
            ipc_server: Mutex::new(None),
            event_handlers: RwLock::new(Vec::new()),
            shutdown_sender: Mutex::new(None),
            metrics: Arc::new(Mutex::new(SystemMetrics::default())),
            start_time: std::time::Instant::now(),
            event_sender: None, // Can be set later via set_event_sender
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Start all configured engine processes
    pub async fn start(&self) -> MultivmResult<()> {
        tracing::info!("Starting MultivmProcessManager");

        let (shutdown_tx, _shutdown_rx) = oneshot::channel();
        *self.inner.shutdown_sender.lock().await = Some(shutdown_tx);

        // Start Solana engine if enabled
        if self.inner.config.blockchain.solana.enable_health_checks {
            self.start_solana_engine().await?;
        }

        // Start Ethereum engine if enabled
        if self.inner.config.blockchain.ethereum.enable_health_checks {
            self.start_ethereum_engine().await?;
        }

        // Start health monitoring
        self.start_health_monitoring().await?;

        // Start resource monitoring
        self.start_resource_monitoring().await?;

        // Start zombie process reaper
        self.start_zombie_reaper().await?;

        // Start IPC server
        self.start_ipc_server().await?;

        // Emit system started event
        self.emit_event(SystemEvent::ProcessStarted {
            process_id: ProcessId::Main,
        })
        .await;

        tracing::info!("MultivmProcessManager started successfully");
        Ok(())
    }

    /// Main run loop for the process manager
    pub async fn run(&self) -> MultivmResult<()> {
        tracing::info!("Starting main event loop");

        // Wait for shutdown signal
        let shutdown_rx = {
            let mut sender = self.inner.shutdown_sender.lock().await;
            if let Some(tx) = sender.take() {
                let (new_tx, rx) = oneshot::channel();
                *sender = Some(new_tx);
                // Move the old sender to trigger shutdown when dropped
                drop(tx);
                rx
            } else {
                return Err(MultivmError::InvalidState {
                    message: "No shutdown channel available".to_string(),
                    current_state: Some("shutdown_channel_none".to_string()),
                    expected_state: Some("shutdown_channel_available".to_string()),
                });
            }
        };

        match shutdown_rx.await {
            Ok(_) => {
                tracing::info!("Received shutdown signal");
                self.shutdown(true).await?;
            }
            Err(_) => {
                tracing::warn!("Shutdown sender dropped, initiating shutdown");
                self.shutdown(false).await?;
            }
        }

        Ok(())
    }

    /// Get system health status
    pub async fn get_health_status(&self) -> MultivmResult<SystemHealthStatus> {
        let processes = self.inner.processes.read().await;
        let mut process_health = HashMap::new();

        for (process_id, handle) in processes.iter() {
            let health = self
                .inner
                .health_monitor
                .check_process_health(handle)
                .await
                .unwrap_or_else(|_| HealthInfo {
                    process_id: *process_id,
                    status: HealthStatus::Unhealthy,
                    last_block_processed: None,
                    blocks_processed_total: 0,
                    uptime: Duration::ZERO,
                    memory_usage: 0,
                    cpu_usage_percent: 0.0,
                    rpc_active: false,
                    errors_count: 1,
                    last_error: Some("Health check failed".to_string()),
                    timestamp: std::time::SystemTime::now(),
                });
            process_health.insert(*process_id, health);
        }

        let metrics = self.inner.metrics.lock().await;
        let system_uptime = self.inner.start_time.elapsed();

        Ok(SystemHealthStatus {
            overall_healthy: process_health.values().all(|h| h.status.is_operational()),
            process_health,
            system_uptime,
            total_blocks_processed: metrics.blocks_processed.values().sum(),
            active_processes: processes.len(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Restart a specific process with proper concurrency control
    pub async fn restart_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        tracing::info!("Restarting process: {}", process_id);

        // Use proper lock ordering and timeout to avoid deadlocks
        let handle = {
            let mut processes = acquire_write_lock_safe(
                &self.inner.processes,
                LockLevel::Processes,
                Some(get_lock_config().critical_timeout),
            )
            .await?;

            // Get and remove the current process handle atomically
            processes
                .remove(&process_id)
                .ok_or_else(|| MultivmError::Process {
                    process_id: format!("{process_id:?}"),
                    message: format!("Process not found: {process_id}"),
                    exit_code: None,
                })?
        };

        // Stop the current process outside of the lock to avoid holding it during I/O
        if let Err(e) = self
            .stop_process_internal(&handle, true, Some(Duration::from_secs(10)))
            .await
        {
            tracing::warn!("Failed to gracefully stop process {}: {}", process_id, e);
        }

        // Start a new instance outside of the lock
        let new_handle = match process_id {
            ProcessId::Solana => {
                tracing::info!("Starting new SVM engine process");
                // Use the unified blockchain config for Solana
                let solana_config = &self.inner.config.blockchain.solana;
                ProcessHandle::start_solana_engine(solana_config, &self.inner.config.ipc).await?
            }
            ProcessId::Ethereum => {
                tracing::info!("Starting new EVM engine process");
                ProcessHandle::start_ethereum_engine(
                    &self.inner.config.blockchain.ethereum,
                    &self.inner.config.ipc,
                )
                .await?
            }
            _ => {
                // Re-insert the old handle on error with proper lock ordering
                let mut processes = acquire_write_lock_safe(
                    &self.inner.processes,
                    LockLevel::Processes,
                    Some(get_lock_config().default_timeout),
                )
                .await?;
                processes.insert(process_id, handle);
                return Err(MultivmError::UnsupportedOperation {
                    operation: format!("Cannot restart process type: {process_id}"),
                    alternatives: Some(vec![
                        "Use ProcessId::Solana or ProcessId::Ethereum".to_string()
                    ]),
                });
            }
        };

        // Store the new handle with proper lock ordering
        {
            let mut processes = acquire_write_lock_safe(
                &self.inner.processes,
                LockLevel::Processes,
                Some(get_lock_config().default_timeout),
            )
            .await?;
            processes.insert(process_id, new_handle);
        }

        // Notify about successful restart
        if let Some(event_sender) = &self.inner.event_sender {
            let _ = event_sender.send(ProcessManagerEvent::ProcessRestarted { process_id });
        }

        tracing::info!("Successfully restarted process: {}", process_id);
        Ok(())
    }

    /// Graceful shutdown of all processes
    pub async fn shutdown(&self, graceful: bool) -> MultivmResult<()> {
        tracing::info!(
            "Shutting down MultivmProcessManager (graceful: {})",
            graceful
        );

        let timeout = if graceful {
            Some(self.inner.config.shutdown_timeout())
        } else {
            Some(Duration::from_secs(5))
        };

        // Stop IPC server first to prevent new connections
        if let Some(handle) = self.inner.ipc_server.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Take ownership of all processes to prevent new registrations during shutdown
        let mut processes = self.inner.processes.write().await;
        let process_handles: Vec<(ProcessId, ProcessHandle)> = processes.drain().collect();
        drop(processes); // Release lock early

        // Stop all engine processes
        for (process_id, handle) in process_handles {
            tracing::info!("Stopping {} process", process_id);
            if let Err(e) = self.stop_process_internal(&handle, graceful, timeout).await {
                tracing::error!("Failed to stop {} process: {}", process_id, e);
            }
        }

        // Emit shutdown event
        self.emit_event(SystemEvent::ProcessStopped {
            process_id: ProcessId::Main,
            exit_code: Some(0),
        })
        .await;

        tracing::info!("MultivmProcessManager shutdown complete");
        Ok(())
    }

    /// Register a process handle with the manager
    pub async fn register_process(&self, handle: ProcessHandle) -> MultivmResult<()> {
        tracing::info!("Registering process: {}", handle.process_id);

        let process_id = handle.process_id;

        // Get the actual PID if the process is running
        if let Some(child) = handle.child.read().await.as_ref() {
            if let Some(pid) = child.id() {
                // Register with zombie reaper to prevent accidental reaping
                self.inner
                    .zombie_reaper
                    .register_multivm_process(pid, process_id)
                    .await;
            }
        }

        let mut processes = self.inner.processes.write().await;
        processes.insert(process_id, handle);

        tracing::info!("Process registered successfully: {}", process_id);
        Ok(())
    }

    /// Unregister a process from the manager
    pub async fn unregister_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        tracing::info!("Unregistering process: {}", process_id);

        let mut processes = self.inner.processes.write().await;
        if let Some(handle) = processes.remove(&process_id) {
            // Unregister from zombie reaper if it has a PID
            if let Some(child) = handle.child.read().await.as_ref() {
                if let Some(pid) = child.id() {
                    self.inner
                        .zombie_reaper
                        .unregister_multivm_process(pid)
                        .await;
                }
            }
            tracing::info!("Process unregistered successfully: {}", process_id);
        } else {
            tracing::warn!("Process not found for unregistration: {}", process_id);
        }

        Ok(())
    }

    /// Start the Solana execution engine
    async fn start_solana_engine(&self) -> MultivmResult<()> {
        tracing::info!("Starting Solana execution engine");

        // Use the blockchain client config directly
        let process_handle = ProcessHandle::start_solana_engine(
            &self.inner.config.blockchain.solana,
            &self.inner.config.ipc,
        )
        .await?;

        self.inner
            .processes
            .write()
            .await
            .insert(ProcessId::Solana, process_handle);

        self.emit_event(SystemEvent::ProcessStarted {
            process_id: ProcessId::Solana,
        })
        .await;

        Ok(())
    }

    /// Start the Ethereum execution engine
    async fn start_ethereum_engine(&self) -> MultivmResult<()> {
        tracing::info!("Starting Ethereum execution engine");

        let process_handle = ProcessHandle::start_ethereum_engine(
            &self.inner.config.blockchain.ethereum,
            &self.inner.config.ipc,
        )
        .await?;

        self.inner
            .processes
            .write()
            .await
            .insert(ProcessId::Ethereum, process_handle);

        self.emit_event(SystemEvent::ProcessStarted {
            process_id: ProcessId::Ethereum,
        })
        .await;

        Ok(())
    }

    /// Start health monitoring task
    async fn start_health_monitoring(&self) -> MultivmResult<()> {
        let inner = self.inner.clone();
        let processes_handle = Arc::clone(&inner.processes);
        let health_monitor = inner.health_monitor.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30)); // Health check every 30 seconds

            loop {
                interval.tick().await;

                // Perform health checks on all processes
                let processes = processes_handle.read().await;
                for (process_id, handle) in processes.iter() {
                    let health_info = health_monitor
                        .check_process_health(handle)
                        .await
                        .unwrap_or_else(|_| HealthInfo {
                            process_id: *process_id,
                            status: HealthStatus::Unhealthy,
                            last_block_processed: None,
                            blocks_processed_total: 0,
                            uptime: Duration::ZERO,
                            memory_usage: 0,
                            cpu_usage_percent: 0.0,
                            rpc_active: false,
                            errors_count: 1,
                            last_error: Some("Health check failed".to_string()),
                            timestamp: std::time::SystemTime::now(),
                        });

                    if !health_info.status.is_operational() {
                        tracing::warn!(
                            "Process {} is unhealthy: status={:?}",
                            process_id,
                            health_info.status
                        );

                        // Update health check timestamp
                        health_monitor.update_last_check(*process_id);
                    } else {
                        tracing::debug!("Process {} is healthy", process_id);
                    }
                }
                drop(processes);

                // System health is implicitly checked through individual process health
            }
        });

        tracing::info!("Health monitoring started");
        Ok(())
    }

    /// Start resource monitoring task
    async fn start_resource_monitoring(&self) -> MultivmResult<()> {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;

                if let Err(e) = inner
                    .resource_monitor
                    .lock()
                    .await
                    .check_system_resources()
                    .await
                {
                    tracing::error!("Resource monitoring error: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Start the zombie process reaper
    async fn start_zombie_reaper(&self) -> MultivmResult<()> {
        tracing::info!("Starting zombie process reaper");
        self.inner.zombie_reaper.start().await?;
        Ok(())
    }

    /// Start IPC server for inter-process communication
    async fn start_ipc_server(&self) -> MultivmResult<()> {
        let inner = self.inner.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = inner.run_ipc_server().await {
                tracing::error!("IPC server error: {}", e);
            }
        });

        *self.inner.ipc_server.lock().await = Some(handle);
        Ok(())
    }

    /// Internal method to stop a process
    async fn stop_process_internal(
        &self,
        handle: &ProcessHandle,
        graceful: bool,
        timeout: Option<Duration>,
    ) -> MultivmResult<()> {
        if graceful {
            handle.stop_gracefully(timeout).await
        } else {
            handle.kill().await
        }
    }

    /// Get zombie reaper statistics
    pub async fn get_zombie_reaper_stats(&self) -> crate::zombie_reaper::ZombieReaperStats {
        self.inner.zombie_reaper.get_stats().await
    }

    /// Manually trigger a zombie process scan
    pub async fn scan_for_zombies(&self) -> MultivmResult<()> {
        self.inner.zombie_reaper.manual_scan().await
    }

    /// Emit a system event to all registered handlers
    async fn emit_event(&self, event: SystemEvent) {
        let handlers = self.inner.event_handlers.read().await;
        for handler in handlers.iter() {
            handler.handle_event(&event).await;
        }
    }

    /// Stop the process manager and all running processes
    pub async fn stop(&self) -> MultivmResult<()> {
        tracing::info!("Stopping MultivmProcessManager");

        // Stop all running processes
        let processes = self.inner.processes.read().await;
        for (process_id, handle) in processes.iter() {
            tracing::info!("Stopping process: {}", process_id);
            if let Err(e) = handle.stop_gracefully(Some(Duration::from_secs(5))).await {
                tracing::warn!("Failed to gracefully stop process {}: {}", process_id, e);
                // Force kill if graceful stop fails
                let _ = handle.kill().await;
            }
        }
        drop(processes);

        // Stop zombie reaper
        if let Err(e) = self.inner.zombie_reaper.stop().await {
            tracing::warn!("Failed to stop zombie reaper: {}", e);
        }

        // Send shutdown signal to IPC server if running
        if let Some(sender) = self.inner.shutdown_sender.lock().await.take() {
            let _ = sender.send(());
        }

        tracing::info!("MultivmProcessManager stopped");
        Ok(())
    }

    /// Route a MultiVM block to appropriate execution engines
    pub async fn route_block(&self, block: multivm_consensus::MultiVMBlock) -> MultivmResult<()> {
        info!("Routing MultiVM block at height {}", block.header.height);

        // Decompose the block using the block router
        let routing_result = self.inner.block_router.decompose_block(block).await?;

        // Process SVM transactions
        if !routing_result.svm_transactions.is_empty() {
            if let Some(solana_handle) = self.inner.processes.read().await.get(&ProcessId::Solana) {
                for svm_tx in routing_result.svm_transactions {
                    self.send_transaction_to_solana(solana_handle, svm_tx)
                        .await?;
                }
            } else {
                warn!("No Solana process available for SVM transactions");
            }
        }

        // Process EVM transactions
        if !routing_result.evm_transactions.is_empty() {
            if let Some(ethereum_handle) =
                self.inner.processes.read().await.get(&ProcessId::Ethereum)
            {
                for evm_tx in routing_result.evm_transactions {
                    self.send_transaction_to_ethereum(ethereum_handle, evm_tx)
                        .await?;
                }
            } else {
                warn!("No Ethereum process available for EVM transactions");
            }
        }

        // Process special transactions (account binding, cross-VM operations)
        if !routing_result.special_transactions.is_empty() {
            for special_tx in routing_result.special_transactions {
                self.handle_special_transaction(special_tx).await?;
            }
        }

        info!(
            "Block routing completed: {} SVM, {} EVM, {} special transactions",
            routing_result.routing_metadata.svm_count,
            routing_result.routing_metadata.evm_count,
            routing_result.routing_metadata.special_count
        );

        Ok(())
    }

    /// Send an SVM transaction to Solana process
    async fn send_transaction_to_solana(
        &self,
        _handle: &ProcessHandle,
        _transaction: multivm_consensus::SvmTransaction,
    ) -> MultivmResult<()> {
        // TODO: Implement actual IPC communication to Solana process
        debug!("Sending SVM transaction to Solana process");
        Ok(())
    }

    /// Send an EVM transaction to Ethereum process  
    async fn send_transaction_to_ethereum(
        &self,
        _handle: &ProcessHandle,
        _transaction: multivm_consensus::EvmTransaction,
    ) -> MultivmResult<()> {
        // TODO: Implement actual IPC communication to Ethereum process
        debug!("Sending EVM transaction to Ethereum process");
        Ok(())
    }

    /// Handle special transactions (account binding, cross-VM operations)
    async fn handle_special_transaction(
        &self,
        _transaction: multivm_account_mapping::special_tx::SpecialTransaction,
    ) -> MultivmResult<()> {
        // TODO: Implement special transaction handling
        debug!("Handling special transaction");
        Ok(())
    }
}

impl MultivmProcessManagerInner {
    /// Run the IPC server loop
    async fn run_ipc_server(&self) -> MultivmResult<()> {
        info!("Starting IPC server for process management");

        // Create a channel for IPC commands
        let (_tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<IpcCommand>();

        // Start IPC message handling loop
        while let Some(command) = rx.recv().await {
            if let Err(e) = self.handle_ipc_command(command).await {
                error!("Error handling IPC command: {}", e);
            }
        }

        info!("IPC server shutdown");
        Ok(())
    }

    /// Handle incoming IPC commands
    async fn handle_ipc_command(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        debug!("Handling IPC command: {:?}", command);

        match command {
            IpcCommand::ProcessBlock {
                block_data_bytes,
                blockchain_type,
                expect_response,
            } => {
                // Route the block to appropriate execution engines
                match bincode::deserialize::<multivm_consensus::MultiVMBlock>(&block_data_bytes) {
                    Ok(block) => {
                        // Use the existing route_block method from MultivmProcessManager
                        // This is where we'd call the block router
                        Ok(IpcResponse::BlockProcessed {
                            result_bytes: Box::new(vec![]), // TODO: Implement actual result
                            blockchain_type,
                            success: true,
                        })
                    }
                    Err(e) => Ok(IpcResponse::Error {
                        code: -32700,
                        message: format!("Failed to deserialize block: {}", e),
                        details: None,
                    }),
                }
            }
            IpcCommand::GetHealth => {
                // Get overall system health
                let system_health = HealthStatus::Healthy; // TODO: Implement actual system health check
                Ok(IpcResponse::Health {
                    status: system_health,
                })
            }
            IpcCommand::GetState => {
                // Get overall system state
                Ok(IpcResponse::State {
                    state: EngineState {
                        process_id: ProcessId::Main,
                        blockchain_type: BlockchainType::Ethereum, // Default to Ethereum
                        current_block: None,
                        state_root: vec![0; 32],
                        is_syncing: false,
                        peer_count: 0,
                        rpc_endpoints: vec![],
                        data_directory: self.config.system.data_dir.to_string_lossy().to_string(),
                        chain_id: 1,
                    },
                })
            }
            IpcCommand::Shutdown { graceful, timeout } => {
                // Handle shutdown request
                if let Some(sender) = self.shutdown_sender.lock().await.take() {
                    let _ = sender.send(());
                }
                Ok(IpcResponse::Ack)
            }
            IpcCommand::Ping => Ok(IpcResponse::Pong),
            IpcCommand::RpcCall { call } => {
                // Route RPC call to appropriate engine
                Ok(IpcResponse::RpcResponse {
                    response: RpcResponse {
                        result: None,
                        error: Some(RpcError {
                            code: -32601,
                            message: "Method not found".to_string(),
                            data: None,
                        }),
                        id: call.id,
                    },
                })
            }
            IpcCommand::RequestNextBlock {
                current_block,
                blockchain_type,
            } => {
                Ok(IpcResponse::NextBlock {
                    block_data_bytes: Some(Box::new(vec![])), // TODO: Implement block fetching
                    blockchain_type: Some(blockchain_type),
                })
            }
            IpcCommand::ConfigureRpc { enable, port } => {
                // TODO: Implement RPC configuration
                Ok(IpcResponse::Ack)
            }
            IpcCommand::UpdateConfig { config_data } => {
                // TODO: Implement configuration update
                Ok(IpcResponse::Ack)
            }
            IpcCommand::HealthCheck => {
                // Similar to GetHealth
                let system_health = HealthStatus::Healthy;
                Ok(IpcResponse::Health {
                    status: system_health,
                })
            }
        }
    }
}

/// System health status
#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub overall_healthy: bool,
    pub process_health: HashMap<ProcessId, HealthInfo>,
    pub system_uptime: Duration,
    pub total_blocks_processed: u64,
    pub active_processes: usize,
    pub timestamp: std::time::SystemTime,
}

#[async_trait::async_trait]
impl ProcessManagerTrait for MultivmProcessManager {
    async fn start_process(&self, process_id: ProcessId, _config: Vec<u8>) -> MultivmResult<()> {
        match process_id {
            ProcessId::Solana => self.start_solana_engine().await,
            ProcessId::Ethereum => self.start_ethereum_engine().await,
            ProcessId::Main => Err(MultivmError::UnsupportedOperation {
                operation: "Cannot start main process".to_string(),
                alternatives: Some(vec![
                    "Use start_solana_engine or start_ethereum_engine".to_string()
                ]),
            }),
        }
    }

    async fn stop_process(&self, process_id: ProcessId, graceful: bool) -> MultivmResult<()> {
        let processes = self.inner.processes.read().await;
        if let Some(handle) = processes.get(&process_id) {
            self.stop_process_internal(handle, graceful, Some(self.inner.config.shutdown_timeout()))
                .await
        } else {
            Err(MultivmError::Process {
                process_id: format!("{process_id:?}"),
                message: format!("Process {process_id} not found"),
                exit_code: None,
            })
        }
    }

    async fn restart_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        // Stop the process first
        self.stop_process(process_id, true).await?;

        // Wait a bit before restarting
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        // Start the process again
        self.start_process(process_id, vec![]).await?;

        // Update restart count
        self.inner
            .metrics
            .lock()
            .await
            .restart_count
            .entry(process_id)
            .and_modify(|c| *c += 1)
            .or_insert(1);

        Ok(())
    }

    async fn is_process_running(&self, process_id: ProcessId) -> MultivmResult<bool> {
        let processes = self.inner.processes.read().await;
        if let Some(handle) = processes.get(&process_id) {
            Ok(handle.is_running().await)
        } else {
            Ok(false)
        }
    }

    async fn get_process_health(&self, process_id: ProcessId) -> MultivmResult<HealthStatus> {
        let processes = self.inner.processes.read().await;
        if let Some(handle) = processes.get(&process_id) {
            let health_info = self
                .inner
                .health_monitor
                .check_process_health(handle)
                .await?;
            Ok(health_info.status)
        } else {
            Err(MultivmError::Process {
                process_id: format!("{process_id:?}"),
                message: format!("Process {process_id} not found"),
                exit_code: None,
            })
        }
    }

    async fn send_command(
        &self,
        process_id: ProcessId,
        command: IpcCommand,
    ) -> MultivmResult<IpcResponse> {
        let processes = self.inner.processes.read().await;
        if let Some(handle) = processes.get(&process_id) {
            handle.send_command(command).await
        } else {
            Err(MultivmError::Process {
                process_id: format!("{process_id:?}"),
                message: format!("Process {process_id} not found"),
                exit_code: None,
            })
        }
    }
}
