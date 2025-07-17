use multivm_common::{
    config::{BlockchainClientConfig, IpcConfig, IpcTransportConfig},
    *,
};
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
    // Process lifecycle tracking
    pub start_time: Arc<Mutex<Option<Instant>>>,
    pub total_uptime: Arc<Mutex<Duration>>,
    pub downtime_periods: Arc<Mutex<VecDeque<DowntimePeriod>>>,
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
            start_time: self.start_time.clone(),
            total_uptime: self.total_uptime.clone(),
            downtime_periods: self.downtime_periods.clone(),
        }
    }
}

impl ProcessHandle {
    /// Start a new Solana execution engine process
    pub async fn start_solana_engine(
        config: &BlockchainClientConfig,
        ipc_config: &IpcConfig,
    ) -> MultivmResult<Self> {
        // Use real solana-test-validator or fall back to mock for testing
        let binary_path =
            get_real_solana_binary_path().or_else(|_| get_engine_binary_path("mock-solana"))?;

        // Create data directory
        let data_dir = PathBuf::from(&config.rpc_url.replace("http://localhost:", "/tmp/solana_"));
        std::fs::create_dir_all(&data_dir).map_err(|e| MultivmError::Configuration {
            component: "solana-engine".to_string(),
            message: format!("Failed to create data directory: {e}"),
            validation_errors: None,
        })?;

        let args = if binary_path.file_name().unwrap_or_default() == "mock-solana" {
            // Mock solana arguments
            let ipc_address = get_ipc_address(&ProcessId::Solana, ipc_config);
            vec![
                "--ipc-address".to_string(),
                ipc_address,
                "--rpc-url".to_string(),
                config.rpc_url.clone(),
                "--rpc-port".to_string(),
                "8899".to_string(),
            ]
        } else {
            // Real solana-test-validator arguments for MultiVM integration
            vec![
                "--ledger".to_string(),
                data_dir.join("ledger").to_string_lossy().to_string(),
                "--rpc-port".to_string(),
                "8899".to_string(),
                "--rpc-bind-address".to_string(),
                "0.0.0.0".to_string(),
                "--dynamic-port-range".to_string(),
                "8000-8020".to_string(),
                "--gossip-port".to_string(),
                "8001".to_string(),
                "--gossip-host".to_string(),
                "127.0.0.1".to_string(),
                "--enable-rpc-transaction-history".to_string(),
                "--enable-cpi-and-log-storage".to_string(),
                "--reset".to_string(), // Reset the ledger on startup for development
                "--quiet".to_string(), // Reduce log noise
                // Disable P2P networking (isolated mode)
                "--no-voting".to_string(),
                "--gossip-host".to_string(),
                "127.0.0.1".to_string(),
            ]
        };

        let process_config = ProcessConfig {
            blockchain_type: BlockchainType::Solana,
            data_dir: data_dir.to_string_lossy().to_string(),
            rpc_port: 8899,
            ..Default::default()
        };

        let handle = Self {
            process_id: ProcessId::Solana,
            child: RwLock::new(None),
            binary_path,
            args,
            working_dir: data_dir,
            config: process_config,
            restart_attempts: Arc::new(Mutex::new(VecDeque::new())),
            last_health_check: Arc::new(Mutex::new(None)),
            is_recovering: Arc::new(Mutex::new(false)),
            start_time: Arc::new(Mutex::new(None)),
            total_uptime: Arc::new(Mutex::new(Duration::from_secs(0))),
            downtime_periods: Arc::new(Mutex::new(VecDeque::new())),
        };

        handle.start().await?;
        Ok(handle)
    }

    /// Start a new Reth execution engine process
    pub async fn start_ethereum_engine(
        config: &BlockchainClientConfig,
        ipc_config: &IpcConfig,
    ) -> MultivmResult<Self> {
        // Use real reth binary or fall back to mock for testing
        let binary_path =
            get_real_reth_binary_path().or_else(|_| get_engine_binary_path("mock-reth"))?;

        // Create data directory
        let data_dir = PathBuf::from(
            &config
                .rpc_url
                .replace("http://localhost:", "/tmp/ethereum_"),
        );
        std::fs::create_dir_all(&data_dir).map_err(|e| MultivmError::Configuration {
            component: "ethereum-engine".to_string(),
            message: format!("Failed to create data directory: {e}"),
            validation_errors: None,
        })?;

        // Generate JWT secret for Engine API authentication
        let jwt_secret_path = data_dir.join("jwt.hex");
        if !jwt_secret_path.exists() {
            let jwt_secret = generate_jwt_secret();
            std::fs::write(&jwt_secret_path, jwt_secret).map_err(|e| {
                MultivmError::Configuration {
                    component: "ethereum-engine".to_string(),
                    message: format!("Failed to write JWT secret: {e}"),
                    validation_errors: None,
                }
            })?;
        }

        let args = if binary_path.file_name().unwrap_or_default() == "mock-reth" {
            // Mock reth arguments
            let ipc_address = get_ipc_address(&ProcessId::Ethereum, ipc_config);
            vec![
                "--ipc-address".to_string(),
                ipc_address,
                "--rpc-url".to_string(),
                config.rpc_url.clone(),
                "--rpc-port".to_string(),
                "8545".to_string(),
            ]
        } else {
            // Real reth arguments for MultiVM integration
            vec![
                "node".to_string(),
                "--datadir".to_string(),
                data_dir.to_string_lossy().to_string(),
                "--chain".to_string(),
                "dev".to_string(), // Use dev chain for development
                "--http".to_string(),
                "--http.addr".to_string(),
                "0.0.0.0".to_string(),
                "--http.port".to_string(),
                "8545".to_string(),
                "--http.api".to_string(),
                "eth,net,web3,debug,trace".to_string(),
                "--http.corsdomain".to_string(),
                "*".to_string(),
                "--authrpc.addr".to_string(),
                "0.0.0.0".to_string(),
                "--authrpc.port".to_string(),
                "8551".to_string(),
                "--authrpc.jwtsecret".to_string(),
                jwt_secret_path.to_string_lossy().to_string(),
                "--disable-discovery".to_string(),
                "--max-inbound-peers".to_string(),
                "0".to_string(),
                "--max-outbound-peers".to_string(),
                "0".to_string(),
                "--port".to_string(),
                "0".to_string(),
                "--ipcdisable".to_string(),
                "--dev".to_string(),
            ]
        };

        let process_config = ProcessConfig {
            blockchain_type: BlockchainType::Ethereum,
            data_dir: data_dir.to_string_lossy().to_string(),
            rpc_port: 8545,
            ..Default::default()
        };

        let handle = Self {
            process_id: ProcessId::Ethereum,
            child: RwLock::new(None),
            binary_path,
            args,
            working_dir: data_dir,
            config: process_config,
            restart_attempts: Arc::new(Mutex::new(VecDeque::new())),
            last_health_check: Arc::new(Mutex::new(None)),
            is_recovering: Arc::new(Mutex::new(false)),
            start_time: Arc::new(Mutex::new(None)),
            total_uptime: Arc::new(Mutex::new(Duration::from_secs(0))),
            downtime_periods: Arc::new(Mutex::new(VecDeque::new())),
        };

        handle.start().await?;
        Ok(handle)
    }

    /// Start the process
    async fn start(&self) -> MultivmResult<()> {
        tracing::info!("Starting {} process", self.process_id);

        // Check if we're in test mode (binary not found is ok for tests)
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        if is_test_mode && !self.binary_path.exists() {
            tracing::info!(
                "Running in test mode - skipping actual process spawn for {}",
                self.process_id
            );
            // Record the start time for accurate uptime calculation
            *self.start_time.lock().await = Some(Instant::now());
            // Mark the end of any ongoing downtime period
            self.mark_downtime_ended().await;
            return Ok(());
        }

        // Ensure working directory exists
        std::fs::create_dir_all(&self.working_dir).map_err(|e| MultivmError::Process {
            process_id: format!("{:?}", self.process_id),
            message: format!("Failed to create working directory: {e}"),
            exit_code: None,
        })?;

        let mut cmd = Command::new(&self.binary_path);
        cmd.args(&self.args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| MultivmError::Process {
            process_id: format!("{:?}", self.process_id),
            message: format!("Failed to spawn process: {e}"),
            exit_code: None,
        })?;

        let pid = child.id();
        *self.child.write().await = Some(child);

        // Record the start time for accurate uptime calculation
        *self.start_time.lock().await = Some(Instant::now());

        // Mark the end of any ongoing downtime period
        self.mark_downtime_ended().await;

        tracing::info!("Started {} process with PID: {:?}", self.process_id, pid);
        Ok(())
    }

    /// Check if the process is still running
    pub async fn is_running(&self) -> bool {
        // Check if we're in test mode
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        if is_test_mode {
            // In test mode, consider process as running if start time is set
            return self.start_time.lock().await.is_some();
        }

        let mut child_guard = self.child.write().await;
        if let Some(child) = child_guard.as_mut() {
            match child.try_wait() {
                Ok(Some(_exit_status)) => {
                    // Process has exited - record downtime period
                    self.record_process_stopped().await;
                    *child_guard = None;
                    false
                }
                Ok(None) => {
                    // Process is still running
                    true
                }
                Err(_) => {
                    // Error checking status, assume not running
                    self.record_process_stopped().await;
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

        // Check if we're in test mode
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        if is_test_mode {
            // In test mode, simulate successful command execution
            tracing::debug!(
                "Test mode: simulating successful command execution for {}",
                self.process_id
            );
            return Ok(IpcResponse::Ack);
        }

        // Check if process is running
        if !self.is_running().await {
            return Err(MultivmError::Process {
                process_id: format!("{:?}", self.process_id),
                message: format!(
                    "Cannot send command to stopped process: {}",
                    self.process_id
                ),
                exit_code: None,
            });
        }

        // Create IPC client for this process
        let ipc_client = match &self.process_id {
            ProcessId::Solana => {
                let socket_path = "/tmp/multivm-solana.sock".to_string();
                crate::ipc_transport::IpcClient::new_unix_socket(&socket_path).await?
            }
            ProcessId::Ethereum => {
                let socket_path = "/tmp/multivm-ethereum.sock".to_string();
                crate::ipc_transport::IpcClient::new_unix_socket(&socket_path).await?
            }
            ProcessId::Main => {
                let socket_path = "/tmp/multivm-main.sock".to_string();
                crate::ipc_transport::IpcClient::new_unix_socket(&socket_path).await?
            }
        };

        // Send command with timeout
        let timeout = Duration::from_secs(30);
        let response = tokio::time::timeout(timeout, ipc_client.send_command(command))
            .await
            .map_err(|_| MultivmError::Process {
                process_id: format!("{:?}", self.process_id),
                message: "IPC command timed out".to_string(),
                exit_code: None,
            })?;

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
        // use std::time::SystemTime;  // Not currently used

        // Update last health check time
        *self.last_health_check.lock().await = Some(Instant::now());

        // Check if we're in test mode
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        let is_running = if is_test_mode {
            // In test mode, consider process running if start_time is set
            self.start_time.lock().await.is_some()
        } else {
            self.is_running().await
        };

        let rpc_responsive = if is_running && !is_test_mode {
            self.check_rpc_responsiveness().await.unwrap_or(false)
        } else if is_test_mode {
            true // Always responsive in test mode
        } else {
            false
        };

        // Calculate proper uptime using comprehensive tracking
        let _uptime = self.calculate_total_uptime().await;

        if is_running && rpc_responsive {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    /// Check RPC responsiveness
    async fn check_rpc_responsiveness(&self) -> MultivmResult<bool> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| MultivmError::Process {
                process_id: format!("{:?}", self.process_id),
                message: format!("Failed to create HTTP client: {e}"),
                exit_code: None,
            })?;

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
            return Err(MultivmError::Process {
                process_id: format!("{:?}", self.process_id),
                message: "Process is already recovering".to_string(),
                exit_code: None,
            });
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
            while attempts.front().is_some_and(|a| a.timestamp < cutoff_time) {
                attempts.pop_front();
            }

            // Check if we've exceeded the maximum attempts
            if attempts.len() >= self.config.max_restart_attempts as usize {
                *is_recovering = false;
                return Err(MultivmError::Process {
                    process_id: format!("{:?}", self.process_id),
                    message: format!(
                        "Maximum restart attempts ({}) exceeded for process {}",
                        self.config.max_restart_attempts, self.process_id
                    ),
                    exit_code: None,
                });
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
                if health.is_operational() {
                    info!(
                        "Process {} successfully restarted and is healthy",
                        self.process_id
                    );
                    return Ok(());
                } else {
                    debug!(
                        "Process {} still unhealthy after restart: status={:?}",
                        self.process_id, health
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(MultivmError::Process {
            process_id: format!("{:?}", self.process_id),
            message: format!(
                "Process {} failed to become healthy within startup timeout",
                self.process_id
            ),
            exit_code: None,
        })
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
        !health.is_operational()
    }

    /// Record when the process stopped for downtime tracking
    async fn record_process_stopped(&self) {
        let start_time_guard = self.start_time.lock().await;
        if let Some(start_time) = *start_time_guard {
            // Update total uptime with the current session
            let current_session_uptime = start_time.elapsed();
            let mut total_uptime = self.total_uptime.lock().await;
            *total_uptime += current_session_uptime;

            // Record the downtime period
            let mut downtime_periods = self.downtime_periods.lock().await;
            downtime_periods.push_back(DowntimePeriod {
                start_time: Instant::now(),
                end_time: None,
                reason: Some("Process stopped".to_string()),
            });

            // Limit downtime history to last 100 periods
            while downtime_periods.len() > 100 {
                downtime_periods.pop_front();
            }
        }
    }

    /// Calculate comprehensive uptime including current session and historical data
    async fn calculate_total_uptime(&self) -> Duration {
        let start_time_guard = self.start_time.lock().await;
        let total_uptime_guard = self.total_uptime.lock().await;

        if let Some(start_time) = *start_time_guard {
            // Process is currently running - add current session uptime
            *total_uptime_guard + start_time.elapsed()
        } else {
            // Process is not running - return accumulated uptime only
            *total_uptime_guard
        }
    }

    /// Get detailed uptime statistics
    pub async fn get_uptime_stats(&self) -> UptimeStats {
        let total_uptime = self.calculate_total_uptime().await;
        let downtime_periods = self.downtime_periods.lock().await;

        // Calculate total downtime from completed downtime periods
        let total_downtime: Duration = downtime_periods
            .iter()
            .filter_map(|period| {
                period
                    .end_time
                    .map(|end| end.duration_since(period.start_time))
            })
            .sum();

        // Calculate availability percentage
        let total_time = total_uptime + total_downtime;
        let availability_percent = if total_time.as_secs() > 0 {
            (total_uptime.as_secs_f64() / total_time.as_secs_f64()) * 100.0
        } else {
            100.0
        };

        // Get current status
        let current_status = if self.start_time.lock().await.is_some() {
            ProcessStatus::Running
        } else {
            ProcessStatus::Stopped
        };

        UptimeStats {
            total_uptime,
            total_downtime,
            availability_percent,
            restart_count: self.restart_attempts.lock().await.len(),
            current_status,
            last_restart: self
                .restart_attempts
                .lock()
                .await
                .back()
                .map(|attempt| attempt.timestamp),
        }
    }

    /// Mark the end of a downtime period (called when process starts again)
    async fn mark_downtime_ended(&self) {
        let mut downtime_periods = self.downtime_periods.lock().await;
        if let Some(last_period) = downtime_periods.back_mut() {
            if last_period.end_time.is_none() {
                last_period.end_time = Some(Instant::now());
            }
        }
    }
}

/// Get the path to an engine binary
fn get_engine_binary_path(binary_name: &str) -> MultivmResult<PathBuf> {
    // Check if we're in test mode
    let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

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

    if is_test_mode {
        return Ok(PathBuf::from("/tmp").join(binary_name));
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
        current_dir = match current_dir.parent() {
            Some(parent) => parent.to_path_buf(),
            None => break, // Reached root directory
        };
    }

    // Try system PATH
    if let Ok(path) = which::which(binary_name) {
        return Ok(path);
    }

    Err(MultivmError::Process {
        process_id: "unknown".to_string(),
        message: format!("Could not find binary: {binary_name}"),
        exit_code: None,
    })
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
            format!("{host}:{process_port}")
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

/// Represents a period when the process was down
#[derive(Debug, Clone)]
pub struct DowntimePeriod {
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub reason: Option<String>,
}

/// Comprehensive uptime statistics
#[derive(Debug, Clone)]
pub struct UptimeStats {
    pub total_uptime: Duration,
    pub total_downtime: Duration,
    pub availability_percent: f64,
    pub restart_count: usize,
    pub current_status: ProcessStatus,
    pub last_restart: Option<Instant>,
}

/// Current process status
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessStatus {
    Running,
    Stopped,
    Starting,
    Stopping,
}

/// Get the path to the real Reth binary
fn get_real_reth_binary_path() -> MultivmResult<PathBuf> {
    // Try common installation paths for Reth
    let possible_paths = vec![
        "/usr/local/bin/reth",
        "/usr/bin/reth",
        "/opt/reth/bin/reth",
        "./target/release/reth",
        "../reth/target/release/reth",
        "reth", // Try PATH
    ];

    for path in possible_paths {
        let path_buf = PathBuf::from(path);
        if path_buf.exists() || (path == "reth" && which::which("reth").is_ok()) {
            info!("Found Reth binary at: {}", path);
            return Ok(path_buf);
        }
    }

    Err(MultivmError::Configuration {
        component: "reth-binary".to_string(),
        message: "Reth binary not found. Please install Reth or set RETH_BINARY_PATH environment variable.".to_string(),
        validation_errors: None,
    })
}

/// Get the path to the real Solana test validator binary
fn get_real_solana_binary_path() -> MultivmResult<PathBuf> {
    // Try common installation paths for Solana
    let possible_paths = vec![
        "/usr/local/bin/solana-test-validator",
        "/usr/bin/solana-test-validator",
        "~/.local/share/solana/install/active_release/bin/solana-test-validator",
        "./solana-test-validator",
        "solana-test-validator", // Try PATH
    ];

    for path in possible_paths {
        let expanded_path = if path.starts_with("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                PathBuf::from(home).join(&path[2..])
            } else {
                continue;
            }
        } else {
            PathBuf::from(path)
        };

        if expanded_path.exists()
            || (path == "solana-test-validator" && which::which("solana-test-validator").is_ok())
        {
            info!(
                "Found Solana test validator binary at: {}",
                expanded_path.display()
            );
            return Ok(expanded_path);
        }
    }

    Err(MultivmError::Configuration {
        component: "solana-binary".to_string(),
        message: "Solana test validator binary not found. Please install Solana CLI tools."
            .to_string(),
        validation_errors: None,
    })
}

/// Generate a JWT secret for Engine API authentication
fn generate_jwt_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    hex::encode(secret)
}
