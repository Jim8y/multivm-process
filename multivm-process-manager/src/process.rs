use multivm_common::*;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Handle for managing a subprocess (engine process)
pub struct ProcessHandle {
    pub process_id: ProcessId,
    pub child: RwLock<Option<Child>>,
    pub binary_path: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    // Recovery-related fields
    pub config: ProcessConfig,
    pub restart_attempts: Arc<Mutex<VecDeque<RestartAttempt>>>,
    pub last_health_check: Arc<Mutex<Option<Instant>>>,
    pub is_recovering: Arc<Mutex<bool>>,
}

impl Clone for ProcessHandle {
    fn clone(&self) -> Self {
        Self {
            process_id: self.process_id,
            child: RwLock::new(None), // Don't clone the actual child process
            binary_path: self.binary_path.clone(),
            args: self.args.clone(),
            working_dir: self.working_dir.clone(),
            config: self.config.clone(),
            restart_attempts: self.restart_attempts.clone(),
            last_health_check: self.last_health_check.clone(),
            is_recovering: self.is_recovering.clone(),
        }
    }
}

impl ProcessHandle {
    /// Start a new Solana execution engine process
    pub async fn start_solana_engine(
        config: &SolanaConfig,
        ipc_config: &IpcConfig,
    ) -> MultivmResult<Self> {
        let binary_path = get_engine_binary_path("solana-execution-engine")?;
        let ipc_address = get_ipc_address(&ProcessId::Solana, ipc_config);

        let mut args = vec![
            "--ipc-address".to_string(),
            ipc_address,
            "--data-dir".to_string(),
            config.data_dir.to_string_lossy().to_string(),
        ];

        if let Some(ref rpc_config) = config.rpc_config {
            args.push("--rpc-port".to_string());
            args.push(rpc_config.port.to_string());
        }

        let process_config = ProcessConfig {
            blockchain_type: BlockchainType::Solana,
            data_dir: config.data_dir.to_string_lossy().to_string(),
            rpc_port: config
                .rpc_config
                .as_ref()
                .map(|rpc| rpc.port)
                .unwrap_or(8899),
            ..Default::default()
        };

        let handle = Self {
            process_id: ProcessId::Solana,
            child: RwLock::new(None),
            binary_path,
            args,
            working_dir: config.data_dir.clone(),
            config: process_config,
            restart_attempts: Arc::new(Mutex::new(VecDeque::new())),
            last_health_check: Arc::new(Mutex::new(None)),
            is_recovering: Arc::new(Mutex::new(false)),
        };

        handle.start().await?;
        Ok(handle)
    }

    /// Start a new Reth execution engine process
    pub async fn start_ethereum_engine(
        config: &EthereumConfig,
        ipc_config: &IpcConfig,
    ) -> MultivmResult<Self> {
        let binary_path = get_engine_binary_path("reth-execution-engine")?;
        let ipc_address = get_ipc_address(&ProcessId::Ethereum, ipc_config);

        let mut args = vec![
            "--ipc-address".to_string(),
            ipc_address,
            "--data-dir".to_string(),
            config.data_dir.to_string_lossy().to_string(),
        ];

        if let Some(ref rpc_config) = config.rpc_config {
            args.push("--rpc-port".to_string());
            args.push(rpc_config.port.to_string());
        }

        let process_config = ProcessConfig {
            blockchain_type: BlockchainType::Ethereum,
            data_dir: config.data_dir.to_string_lossy().to_string(),
            rpc_port: config
                .rpc_config
                .as_ref()
                .map(|rpc| rpc.port)
                .unwrap_or(8545),
            ..Default::default()
        };

        let handle = Self {
            process_id: ProcessId::Ethereum,
            child: RwLock::new(None),
            binary_path,
            args,
            working_dir: config.data_dir.clone(),
            config: process_config,
            restart_attempts: Arc::new(Mutex::new(VecDeque::new())),
            last_health_check: Arc::new(Mutex::new(None)),
            is_recovering: Arc::new(Mutex::new(false)),
        };

        handle.start().await?;
        Ok(handle)
    }

    /// Start the process
    async fn start(&self) -> MultivmResult<()> {
        tracing::info!("Starting {} process", self.process_id);

        // Ensure working directory exists
        std::fs::create_dir_all(&self.working_dir).map_err(|e| {
            MultivmError::Process(format!("Failed to create working directory: {}", e))
        })?;

        let mut cmd = Command::new(&self.binary_path);
        cmd.args(&self.args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true);

        let child = cmd
            .spawn()
            .map_err(|e| MultivmError::Process(format!("Failed to spawn process: {}", e)))?;

        let pid = child.id();
        *self.child.write().await = Some(child);

        tracing::info!("Started {} process with PID: {:?}", self.process_id, pid);
        Ok(())
    }

    /// Check if the process is still running
    pub async fn is_running(&self) -> bool {
        let mut child_guard = self.child.write().await;
        if let Some(child) = child_guard.as_mut() {
            match child.try_wait() {
                Ok(Some(_exit_status)) => {
                    // Process has exited
                    *child_guard = None;
                    false
                }
                Ok(None) => {
                    // Process is still running
                    true
                }
                Err(_) => {
                    // Error checking status, assume not running
                    *child_guard = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Send a command to the process via IPC
    pub async fn send_command(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        tracing::debug!("Sending command to {}: {:?}", self.process_id, command);

        // Check if process is running
        if !self.is_running().await {
            return Err(MultivmError::Process(format!(
                "Cannot send command to stopped process: {}",
                self.process_id
            )));
        }

        // Create IPC client for this process
        let ipc_client = match &self.process_id {
            ProcessId::Solana => {
                let socket_path = format!("/tmp/multivm-solana.sock");
                crate::ipc_transport::IpcClient::new_unix_socket(&socket_path).await?
            }
            ProcessId::Ethereum => {
                let socket_path = format!("/tmp/multivm-ethereum.sock");
                crate::ipc_transport::IpcClient::new_unix_socket(&socket_path).await?
            }
            ProcessId::Main => {
                let socket_path = format!("/tmp/multivm-main.sock");
                crate::ipc_transport::IpcClient::new_unix_socket(&socket_path).await?
            }
        };

        // Send command with timeout
        let timeout = Duration::from_secs(30);
        let response = tokio::time::timeout(timeout, ipc_client.send_command(command)).await
            .map_err(|_| MultivmError::Process("IPC command timed out".to_string()))?;

        match response {
            Ok(resp) => {
                tracing::debug!("Received response from {}: {:?}", self.process_id, resp);
                Ok(resp)
            }
            Err(e) => {
                tracing::error!("IPC command failed for {}: {}", self.process_id, e);
                Err(e)
            }
        }
    }

    /// Stop the process gracefully
    pub async fn stop_gracefully(&self, timeout: Option<Duration>) -> MultivmResult<()> {
        tracing::info!("Stopping {} process gracefully", self.process_id);

        // First, try to send a shutdown command
        if let Err(e) = self
            .send_command(IpcCommand::Shutdown {
                graceful: true,
                timeout,
            })
            .await
        {
            tracing::warn!("Failed to send shutdown command: {}", e);
        }

        // Wait for process to exit
        let timeout_duration = timeout.unwrap_or(Duration::from_secs(30));
        let start_time = std::time::Instant::now();

        while self.is_running().await && start_time.elapsed() < timeout_duration {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // If still running, force kill
        if self.is_running().await {
            tracing::warn!(
                "Process {} did not shut down gracefully, force killing",
                self.process_id
            );
            self.kill().await?;
        }

        tracing::info!("Process {} stopped", self.process_id);
        Ok(())
    }

    /// Force kill the process
    pub async fn kill(&self) -> MultivmResult<()> {
        tracing::info!("Force killing {} process", self.process_id);

        let mut child_guard = self.child.write().await;
        if let Some(child) = child_guard.as_mut() {
            if let Err(e) = child.kill().await {
                tracing::warn!("Failed to kill process: {}", e);
            }

            // Wait for the process to be killed
            if let Err(e) = child.wait().await {
                tracing::warn!("Error waiting for killed process: {}", e);
            }

            *child_guard = None;
        }

        Ok(())
    }

    /// Perform health check on the process
    pub async fn health_check(&self) -> HealthStatus {
        use std::time::{Duration, SystemTime};

        // Update last health check time
        *self.last_health_check.lock().await = Some(Instant::now());

        let is_running = self.is_running().await;
        let rpc_responsive = if is_running {
            self.check_rpc_responsiveness().await.unwrap_or(false)
        } else {
            false
        };

        // Calculate uptime (simplified)
        let uptime = self
            .last_health_check
            .lock()
            .await
            .map(|t| t.elapsed())
            .unwrap_or(Duration::from_secs(0));

        HealthStatus {
            process_id: self.process_id,
            is_healthy: is_running && rpc_responsive,
            last_block_processed: None, // Would be set by actual engine
            blocks_processed_total: 0,  // Would be tracked by actual engine
            uptime,
            memory_usage: 0,        // Would be calculated from process stats
            cpu_usage_percent: 0.0, // Would be calculated from process stats
            rpc_active: rpc_responsive,
            errors_count: 0, // Would be tracked by actual engine
            last_error: if is_running && rpc_responsive {
                None
            } else {
                Some("Process not running or not responsive".to_string())
            },
            timestamp: SystemTime::now(),
        }
    }

    /// Check RPC responsiveness
    async fn check_rpc_responsiveness(&self) -> MultivmResult<bool> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| MultivmError::Process(format!("Failed to create HTTP client: {}", e)))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.config.rpc_port);

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        match client.post(&rpc_url).json(&request_body).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false), // Assume not responsive if request fails
        }
    }

    /// Attempt to restart the process
    pub async fn restart(&self, reason: String) -> MultivmResult<()> {
        // Check if we're already recovering
        let mut is_recovering = self.is_recovering.lock().await;
        if *is_recovering {
            return Err(MultivmError::Process(
                "Process is already recovering".to_string(),
            ));
        }
        *is_recovering = true;

        let start_time = Instant::now();
        info!(
            "Attempting to restart {} process, reason: {}",
            self.process_id, reason
        );

        // Check restart attempts limit
        {
            let mut attempts = self.restart_attempts.lock().await;

            // Clean up old attempts (older than 1 hour)
            let cutoff_time = Instant::now() - Duration::from_secs(3600);
            while attempts
                .front()
                .map_or(false, |a| a.timestamp < cutoff_time)
            {
                attempts.pop_front();
            }

            // Check if we've exceeded the maximum attempts
            if attempts.len() >= self.config.max_restart_attempts as usize {
                *is_recovering = false;
                return Err(MultivmError::Process(format!(
                    "Maximum restart attempts ({}) exceeded for process {}",
                    self.config.max_restart_attempts, self.process_id
                )));
            }
        }

        let restart_result = self.perform_restart().await;
        let successful = restart_result.is_ok();

        // Record the restart attempt
        {
            let mut attempts = self.restart_attempts.lock().await;
            attempts.push_back(RestartAttempt {
                timestamp: start_time,
                reason: reason.clone(),
                successful,
            });
        }

        *is_recovering = false;

        if successful {
            info!("Successfully restarted {} process", self.process_id);
        } else {
            error!(
                "Failed to restart {} process: {:?}",
                self.process_id, restart_result
            );
        }

        restart_result
    }

    /// Perform the actual restart process
    async fn perform_restart(&self) -> MultivmResult<()> {
        // Stop the current process gracefully
        if self.is_running().await {
            info!(
                "Stopping current {} process before restart",
                self.process_id
            );
            if let Err(e) = self.stop_gracefully(Some(Duration::from_secs(30))).await {
                warn!("Failed to stop process gracefully: {}, force killing", e);
                self.kill().await?;
            }
        }

        // Wait a bit before restarting
        tokio::time::sleep(self.config.restart_delay).await;

        // Start the process again
        info!("Starting {} process after restart", self.process_id);
        self.start().await?;

        // Wait for startup and verify it's working
        let startup_deadline = Instant::now() + self.config.startup_timeout;

        while Instant::now() < startup_deadline {
            if self.is_running().await {
                // Give it a moment to fully initialize
                tokio::time::sleep(Duration::from_secs(2)).await;

                // Check if it's responsive
                let health = self.health_check().await;
                if health.is_healthy {
                    info!(
                        "Process {} successfully restarted and is healthy",
                        self.process_id
                    );
                    return Ok(());
                } else {
                    debug!(
                        "Process {} still unhealthy after restart: {:?}",
                        self.process_id, health.last_error
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(MultivmError::Process(format!(
            "Process {} failed to become healthy within startup timeout",
            self.process_id
        )))
    }

    /// Get restart statistics
    pub async fn get_restart_stats(&self) -> (usize, Option<Instant>) {
        let attempts = self.restart_attempts.lock().await;
        let count = attempts.len();
        let last_restart = attempts.back().map(|a| a.timestamp);
        (count, last_restart)
    }

    /// Check if the process needs recovery
    pub async fn needs_recovery(&self) -> bool {
        let is_recovering = *self.is_recovering.lock().await;
        if is_recovering {
            return false; // Already recovering
        }

        let health = self.health_check().await;
        !health.is_healthy
    }
}

/// Get the path to an engine binary
fn get_engine_binary_path(binary_name: &str) -> MultivmResult<PathBuf> {
    // First, try to find it in the current workspace target directory
    let workspace_binary = PathBuf::from("target").join("debug").join(binary_name);

    if workspace_binary.exists() {
        return Ok(workspace_binary.canonicalize().unwrap_or(workspace_binary));
    }

    // Try release directory
    let release_binary = PathBuf::from("target").join("release").join(binary_name);

    if release_binary.exists() {
        return Ok(release_binary.canonicalize().unwrap_or(release_binary));
    }

    // Try to find the workspace root and search from there
    let mut current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    // Walk up the directory tree to find the correct workspace root with target directory
    while current_dir.parent().is_some() {
        let cargo_toml = current_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            // Check if this directory has a target folder with our binaries
            let debug_binary = current_dir.join("target").join("debug").join(binary_name);
            if debug_binary.exists() {
                return Ok(debug_binary.canonicalize().unwrap_or(debug_binary));
            }
            
            let release_binary = current_dir.join("target").join("release").join(binary_name);
            if release_binary.exists() {
                return Ok(release_binary.canonicalize().unwrap_or(release_binary));
            }
            
            // If this Cargo.toml doesn't have our binaries, continue searching up
        }
        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    // Try system PATH
    if let Ok(path) = which::which(binary_name) {
        return Ok(path);
    }

    Err(MultivmError::Process(format!(
        "Could not find binary: {}",
        binary_name
    )))
}

/// Get the IPC address for a process
fn get_ipc_address(process_id: &ProcessId, ipc_config: &IpcConfig) -> String {
    match &ipc_config.transport {
        IpcTransportConfig::UnixSocket { path } => {
            let base_path = path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/tmp"));
            let socket_name = format!("multivm-{}.sock", process_id.to_string().to_lowercase());
            base_path.join(socket_name).to_string_lossy().to_string()
        }
        IpcTransportConfig::TcpSocket { host, port } => {
            let process_port = match process_id {
                ProcessId::Solana => port + 1,
                ProcessId::Ethereum => port + 2,
                ProcessId::Main => *port,
            };
            format!("{}:{}", host, process_port)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessConfig {
    pub blockchain_type: BlockchainType,
    pub data_dir: String,
    pub rpc_port: u16,
    pub max_restart_attempts: u32,
    pub restart_delay: Duration,
    pub health_check_interval: Duration,
    pub startup_timeout: Duration,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            blockchain_type: BlockchainType::Ethereum,
            data_dir: "./data".to_string(),
            rpc_port: 8545,
            max_restart_attempts: 5,
            restart_delay: Duration::from_secs(10),
            health_check_interval: Duration::from_secs(30),
            startup_timeout: Duration::from_secs(120),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RestartAttempt {
    pub timestamp: Instant,
    pub reason: String,
    pub successful: bool,
}
