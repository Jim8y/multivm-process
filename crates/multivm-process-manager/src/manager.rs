use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex, RwLock};

use multivm_common::{
    error::MultivmError,
    ipc::{IpcCommand, IpcResponse},
    traits::ProcessManager as ProcessManagerTrait,
    types::{BlockchainType, HealthStatus, ProcessId},
    MultivmConfig, MultivmResult, SystemEvent,
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

use crate::{BlockRouter, HealthMonitor, ProcessHandle, SystemResourceMonitor};
use multivm_account_mapping::MemoryStorage;

/// The main process manager that coordinates all blockchain engines
#[derive(Clone)]
pub struct MultivmProcessManager {
    inner: Arc<MultivmProcessManagerInner>,
}

struct MultivmProcessManagerInner {
    config: MultivmConfig,
    processes: Arc<RwLock<HashMap<ProcessId, ProcessHandle>>>,
    health_monitor: HealthMonitor,
    block_router: BlockRouter,
    resource_monitor: Arc<Mutex<SystemResourceMonitor>>,
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

        // Validate configuration
        config
            .system
            .validate()
            .map_err(|e| MultivmError::Configuration(format!("Invalid system config: {}", e)))?;

        // Create data directories
        std::fs::create_dir_all(&config.system.data_dir).map_err(|e| {
            MultivmError::Configuration(format!("Failed to create data directory: {}", e))
        })?;

        // Initialize components
        let health_monitor = HealthMonitor::new(config.system.health_check_interval);
        let account_mapping = Arc::new(MemoryStorage::new());
        let block_router = BlockRouter::new(account_mapping);
        let resource_monitor = Arc::new(Mutex::new(SystemResourceMonitor::new(
            config.system.resource_limits.clone(),
        )));

        let inner = MultivmProcessManagerInner {
            config,
            processes: Arc::new(RwLock::new(HashMap::new())),
            health_monitor,
            block_router,
            resource_monitor,
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
        if self.inner.config.solana.enabled {
            self.start_solana_engine().await?;
        }

        // Start Ethereum engine if enabled
        if self.inner.config.ethereum.enabled {
            self.start_ethereum_engine().await?;
        }

        // Start health monitoring
        self.start_health_monitoring().await?;

        // Start resource monitoring
        self.start_resource_monitoring().await?;

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
                return Err(MultivmError::InvalidState(
                    "No shutdown channel available".to_string(),
                ));
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
                .unwrap_or_else(|_| HealthStatus {
                    process_id: *process_id,
                    is_healthy: false,
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
            overall_healthy: process_health.values().all(|h| h.is_healthy),
            process_health,
            system_uptime,
            total_blocks_processed: metrics.blocks_processed.values().sum(),
            active_processes: processes.len(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Restart a specific process
    pub async fn restart_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        tracing::info!("Restarting process: {}", process_id);

        // Get the current process handle
        let processes = self.inner.processes.read().await;
        let handle = processes
            .get(&process_id)
            .cloned()
            .ok_or_else(|| MultivmError::Process(format!("Process not found: {}", process_id)))?;
        drop(processes);

        // Stop the current process
        if let Err(e) = self
            .stop_process_internal(&handle, true, Some(Duration::from_secs(10)))
            .await
        {
            tracing::warn!("Failed to gracefully stop process {}: {}", process_id, e);
        }

        // Remove the old handle
        self.inner.processes.write().await.remove(&process_id);

        // Start a new instance
        let new_handle = match process_id {
            ProcessId::Solana => {
                tracing::info!("Starting new SVM engine process");
                ProcessHandle::start_solana_engine(
                    &self.inner.config.solana,
                    &self.inner.config.ipc,
                )
                .await?
            }
            ProcessId::Ethereum => {
                tracing::info!("Starting new EVM engine process");
                ProcessHandle::start_ethereum_engine(
                    &self.inner.config.ethereum,
                    &self.inner.config.ipc,
                )
                .await?
            }
            _ => {
                return Err(MultivmError::UnsupportedOperation(format!(
                    "Cannot restart process type: {}",
                    process_id
                )));
            }
        };

        // Store the new handle
        self.inner
            .processes
            .write()
            .await
            .insert(process_id, new_handle);

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
            Some(self.inner.config.system.shutdown_timeout)
        } else {
            Some(Duration::from_secs(5))
        };

        // Stop IPC server
        if let Some(handle) = self.inner.ipc_server.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Stop all engine processes
        let processes = self.inner.processes.read().await;
        for (process_id, handle) in processes.iter() {
            tracing::info!("Stopping {} process", process_id);
            if let Err(e) = self.stop_process_internal(handle, graceful, timeout).await {
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
        let mut processes = self.inner.processes.write().await;
        processes.insert(process_id, handle);

        tracing::info!("Process registered successfully: {}", process_id);
        Ok(())
    }

    /// Unregister a process from the manager
    pub async fn unregister_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        tracing::info!("Unregistering process: {}", process_id);

        let mut processes = self.inner.processes.write().await;
        if let Some(_handle) = processes.remove(&process_id) {
            tracing::info!("Process unregistered successfully: {}", process_id);
        } else {
            tracing::warn!("Process not found for unregistration: {}", process_id);
        }

        Ok(())
    }

    /// Start the Solana execution engine
    async fn start_solana_engine(&self) -> MultivmResult<()> {
        tracing::info!("Starting Solana execution engine");

        let process_handle =
            ProcessHandle::start_solana_engine(&self.inner.config.solana, &self.inner.config.ipc)
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
            &self.inner.config.ethereum,
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
                    let status = handle.health_check().await;
                    if !status.is_healthy {
                        tracing::warn!(
                            "Process {} is unhealthy: {:?}",
                            process_id,
                            status.last_error
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

        // Send shutdown signal to IPC server if running
        if let Some(sender) = self.inner.shutdown_sender.lock().await.take() {
            let _ = sender.send(());
        }

        tracing::info!("MultivmProcessManager stopped");
        Ok(())
    }
}

impl MultivmProcessManagerInner {
    /// Run the IPC server loop
    async fn run_ipc_server(&self) -> MultivmResult<()> {
        // This would implement the actual IPC server logic
        // For now, just a placeholder that runs indefinitely
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            // IPC message handling would go here
        }
    }
}

/// System health status
#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub overall_healthy: bool,
    pub process_health: HashMap<ProcessId, HealthStatus>,
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
            ProcessId::Main => Err(MultivmError::UnsupportedOperation(
                "Cannot start main process".to_string(),
            )),
        }
    }

    async fn stop_process(&self, process_id: ProcessId, graceful: bool) -> MultivmResult<()> {
        let processes = self.inner.processes.read().await;
        if let Some(handle) = processes.get(&process_id) {
            self.stop_process_internal(
                handle,
                graceful,
                Some(self.inner.config.system.shutdown_timeout),
            )
            .await
        } else {
            Err(MultivmError::Process(format!(
                "Process {} not found",
                process_id
            )))
        }
    }

    async fn restart_process(&self, process_id: ProcessId) -> MultivmResult<()> {
        // Stop the process first
        self.stop_process(process_id, true).await?;

        // Wait a bit before restarting
        tokio::time::sleep(self.inner.config.system.process_restart_delay).await;

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
            self.inner.health_monitor.check_process_health(handle).await
        } else {
            Err(MultivmError::Process(format!(
                "Process {} not found",
                process_id
            )))
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
            Err(MultivmError::Process(format!(
                "Process {} not found",
                process_id
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> MultivmConfig {
        let temp_dir = TempDir::new().unwrap();
        let mut config = MultivmConfig::default();
        config.system.data_dir = temp_dir.path().to_path_buf();
        config.solana.enabled = false; // Disable for testing
        config.ethereum.enabled = false; // Disable for testing
        config
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let config = create_test_config();
        let manager = MultivmProcessManager::new(config).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_health_status() {
        let config = create_test_config();
        let manager = MultivmProcessManager::new(config).await.unwrap();
        let health = manager.get_health_status().await.unwrap();
        assert_eq!(health.active_processes, 0);
    }
}
