//! Process handle implementation with support for multiple communication protocols
//!
//! This module extends the ProcessHandle to use the new communication protocol
//! abstraction instead of direct IPC connections.

use multivm_common::{
    communication::{
        config::CommunicationConfig, CommunicationManager, CommunicationRequest,
        CommunicationResponse, EngineType, ProtocolType,
    },
    config::{BlockchainClientConfig, IpcConfig},
    ipc::{IpcCommand, IpcResponse},
    *,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Extended process handle with protocol support
pub struct ProcessHandleWithProtocols {
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
    // Communication protocol manager
    pub communication_manager: Arc<RwLock<CommunicationManager>>,
    pub engine_type: EngineType,
    pub preferred_protocol: ProtocolType,
}

impl ProcessHandleWithProtocols {
    /// Create a new process handle with protocol support
    pub async fn new(
        process_id: ProcessId,
        binary_path: PathBuf,
        args: Vec<String>,
        working_dir: PathBuf,
        config: ProcessConfig,
        communication_config: Option<CommunicationConfig>,
    ) -> MultivmResult<Self> {
        // Determine engine type from process ID
        let engine_type = match process_id {
            ProcessId::Ethereum => EngineType::Ethereum,
            ProcessId::Solana => EngineType::Solana,
            _ => {
                return Err(MultivmError::Configuration {
                    component: "process_handle".to_string(),
                    message: format!(
                        "Unsupported process ID for protocol communication: {:?}",
                        process_id
                    ),
                    validation_errors: None,
                })
            }
        };

        // Create communication manager
        let comm_config = communication_config.unwrap_or_default();
        let communication_manager = comm_config.create_manager().await?;

        // Get preferred protocol from config
        let preferred_protocol = comm_config
            .protocol_preferences
            .get(&engine_type)
            .and_then(|prefs| prefs.first())
            .cloned()
            .unwrap_or(ProtocolType::Ipc);

        Ok(Self {
            process_id,
            child: RwLock::new(None),
            binary_path,
            args,
            working_dir,
            config,
            restart_attempts: Arc::new(Mutex::new(VecDeque::new())),
            last_health_check: Arc::new(Mutex::new(None)),
            is_recovering: Arc::new(Mutex::new(false)),
            start_time: Arc::new(Mutex::new(None)),
            total_uptime: Arc::new(Mutex::new(Duration::from_secs(0))),
            downtime_periods: Arc::new(Mutex::new(VecDeque::new())),
            communication_manager: Arc::new(RwLock::new(communication_manager)),
            engine_type,
            preferred_protocol,
        })
    }

    /// Start a new Solana execution engine process with protocol support
    pub async fn start_solana_engine(
        config: &BlockchainClientConfig,
        ipc_config: &IpcConfig,
        communication_config: Option<CommunicationConfig>,
    ) -> MultivmResult<Self> {
        let binary_path = get_engine_binary_path("mock-solana")?;
        let ipc_address = get_ipc_address(&ProcessId::Solana, ipc_config);

        let mut args = vec![
            "--ipc-address".to_string(),
            ipc_address,
            "--rpc-url".to_string(),
            config.rpc_url.clone(),
        ];

        args.push("--rpc-port".to_string());
        args.push("8899".to_string());

        let process_config = ProcessConfig {
            blockchain_type: BlockchainType::Solana,
            data_dir: "/tmp/solana".to_string(),
            rpc_port: 8899,
            ..Default::default()
        };

        let handle = Self::new(
            ProcessId::Solana,
            binary_path,
            args,
            std::path::PathBuf::from("/tmp/solana"),
            process_config,
            communication_config,
        )
        .await?;

        handle.start().await?;
        Ok(handle)
    }

    /// Start a new Ethereum execution engine process with protocol support
    pub async fn start_ethereum_engine(
        config: &BlockchainClientConfig,
        ipc_config: &IpcConfig,
        communication_config: Option<CommunicationConfig>,
    ) -> MultivmResult<Self> {
        let binary_path = get_engine_binary_path("mock-reth")?;
        let ipc_address = get_ipc_address(&ProcessId::Ethereum, ipc_config);

        let mut args = vec![
            "--ipc-address".to_string(),
            ipc_address,
            "--rpc-url".to_string(),
            config.rpc_url.clone(),
        ];

        args.push("--rpc-port".to_string());
        args.push("8545".to_string());

        let process_config = ProcessConfig {
            blockchain_type: BlockchainType::Ethereum,
            data_dir: "/tmp/ethereum".to_string(),
            rpc_port: 8545,
            ..Default::default()
        };

        let handle = Self::new(
            ProcessId::Ethereum,
            binary_path,
            args,
            std::path::PathBuf::from("/tmp/ethereum"),
            process_config,
            communication_config,
        )
        .await?;

        handle.start().await?;
        Ok(handle)
    }

    /// Start the process
    async fn start(&self) -> MultivmResult<()> {
        info!(
            "Starting {} process with {} protocol",
            self.process_id, self.preferred_protocol
        );

        // Check if we're in test mode
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        if is_test_mode && !self.binary_path.exists() {
            info!(
                "Running in test mode - skipping actual process spawn for {}",
                self.process_id
            );
            *self.start_time.lock().await = Some(Instant::now());
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

        *self.start_time.lock().await = Some(Instant::now());
        self.mark_downtime_ended().await;

        info!("Started {} process with PID: {:?}", self.process_id, pid);

        // Wait a bit for the process to initialize
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Connect communication protocols
        self.connect_protocols().await?;

        Ok(())
    }

    /// Connect all configured protocols
    async fn connect_protocols(&self) -> MultivmResult<()> {
        let mut manager = self.communication_manager.write().await;

        // Try to connect preferred protocol first
        if let Some(protocol) =
            manager.get_protocol_mut(&self.preferred_protocol, &self.engine_type)
        {
            match protocol.connect().await {
                Ok(()) => {
                    info!(
                        "Connected {} protocol to {} engine",
                        self.preferred_protocol, self.engine_type
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to connect {} protocol: {}",
                        self.preferred_protocol, e
                    );
                }
            }
        }

        // Try to connect other protocols as fallback
        for protocol_type in [ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt] {
            if protocol_type == self.preferred_protocol {
                continue; // Already tried
            }

            if let Some(protocol) = manager.get_protocol_mut(&protocol_type, &self.engine_type) {
                match protocol.connect().await {
                    Ok(()) => {
                        info!(
                            "Connected {} protocol to {} engine (fallback)",
                            protocol_type, self.engine_type
                        );
                    }
                    Err(e) => {
                        debug!("Failed to connect {} protocol: {}", protocol_type, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Send a command to the process using protocol abstraction
    pub async fn send_command(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        debug!("Sending command to {}: {:?}", self.process_id, command);

        // Check if we're in test mode
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        if is_test_mode {
            debug!(
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

        // Convert IPC command to communication request
        let request = self.ipc_command_to_request(command)?;

        // Send using communication manager with fallback
        let manager = self.communication_manager.read().await;
        let response = manager
            .send_with_fallback(&self.engine_type, request)
            .await?;

        // Convert response back to IPC response
        self.response_to_ipc(response)
    }

    /// Convert IPC command to communication request
    fn ipc_command_to_request(&self, command: IpcCommand) -> MultivmResult<CommunicationRequest> {
        let (method, params) = match command {
            IpcCommand::GetHealth => ("get_health", serde_json::Value::Null),
            IpcCommand::GetState => ("get_state", serde_json::Value::Null),
            IpcCommand::ProcessBlock {
                block_data_bytes,
                blockchain_type,
                expect_response,
            } => {
                let encoded =
                    base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &**block_data_bytes);
                (
                    "process_block",
                    serde_json::json!({
                        "block_data": encoded,
                        "blockchain_type": blockchain_type,
                        "expect_response": expect_response
                    }),
                )
            }
            IpcCommand::RpcCall { call } => (
                "rpc_call",
                serde_json::json!({
                    "method": call.method,
                    "params": call.params
                }),
            ),
            IpcCommand::Shutdown { graceful, timeout } => (
                "shutdown",
                serde_json::json!({
                    "graceful": graceful,
                    "timeout_seconds": timeout.map(|d| d.as_secs())
                }),
            ),
            _ => {
                return Err(MultivmError::UnsupportedOperation {
                    operation: "Unknown IPC command".to_string(),
                    alternatives: Some(vec![
                        "get_health".to_string(),
                        "get_state".to_string(),
                        "process_block".to_string(),
                        "rpc_call".to_string(),
                        "shutdown".to_string(),
                    ]),
                })
            }
        };

        Ok(CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: method.to_string(),
            params,
            timeout: Some(Duration::from_secs(30)),
            auth_context: None,
        })
    }

    /// Convert communication response to IPC response
    fn response_to_ipc(&self, response: CommunicationResponse) -> MultivmResult<IpcResponse> {
        if let Some(error) = response.error {
            return Err(MultivmError::Ipc {
                endpoint: self.process_id.to_string(),
                message: error.message,
                retry_count: None,
            });
        }

        if let Some(result) = response.result {
            // Try to parse common response types
            if let Some(status_str) = result.get("status").and_then(|v| v.as_str()) {
                if status_str == "healthy" {
                    return Ok(IpcResponse::Health {
                        status: HealthStatus::Healthy,
                    });
                }
            }

            // Default to Ack for successful responses
            Ok(IpcResponse::Ack)
        } else {
            Ok(IpcResponse::Ack)
        }
    }

    /// Check if the process is still running
    pub async fn is_running(&self) -> bool {
        let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

        if is_test_mode {
            return self.start_time.lock().await.is_some();
        }

        match self.child.write().await.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => false, // Process has exited
                Ok(None) => true,     // Still running
                Err(_) => false,      // Error checking status
            },
            None => false,
        }
    }

    /// Stop the process gracefully
    pub async fn stop_gracefully(&self, timeout: Option<Duration>) -> MultivmResult<()> {
        info!("Stopping {} process gracefully", self.process_id);

        // Disconnect all protocols first
        let mut manager = self.communication_manager.write().await;
        for protocol_type in [ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt] {
            if let Some(protocol) = manager.get_protocol_mut(&protocol_type, &self.engine_type) {
                let _ = protocol.disconnect().await;
            }
        }

        // Send shutdown command
        if let Err(e) = self
            .send_command(IpcCommand::Shutdown {
                graceful: true,
                timeout,
            })
            .await
        {
            warn!("Failed to send shutdown command: {}", e);
        }

        // Wait for process to exit
        let timeout_duration = timeout.unwrap_or(Duration::from_secs(30));
        let start_time = std::time::Instant::now();

        while self.is_running().await && start_time.elapsed() < timeout_duration {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // If still running, force kill
        if self.is_running().await {
            warn!(
                "Process {} did not shut down gracefully, force killing",
                self.process_id
            );
            self.kill().await?;
        }

        // Update downtime tracking
        self.mark_downtime_started().await;

        Ok(())
    }

    /// Force kill the process
    pub async fn kill(&self) -> MultivmResult<()> {
        if let Some(mut child) = self.child.write().await.take() {
            child.kill().await.map_err(|e| MultivmError::Process {
                process_id: format!("{:?}", self.process_id),
                message: format!("Failed to kill process: {e}"),
                exit_code: None,
            })?;
        }
        Ok(())
    }

    /// Mark the start of a downtime period
    async fn mark_downtime_started(&self) {
        let mut downtime_periods = self.downtime_periods.lock().await;
        downtime_periods.push_back(DowntimePeriod {
            start_time: Instant::now(),
            end_time: None,
            reason: None,
        });
    }

    /// Mark the end of a downtime period
    async fn mark_downtime_ended(&self) {
        let mut downtime_periods = self.downtime_periods.lock().await;
        if let Some(last_period) = downtime_periods.back_mut() {
            if last_period.end_time.is_none() {
                last_period.end_time = Some(Instant::now());
            }
        }
    }

    /// Get health status using the best available protocol
    pub async fn get_health_status(&self) -> MultivmResult<HealthInfo> {
        let manager = self.communication_manager.read().await;

        // Try each protocol in preference order
        let preferences = manager
            .get_protocol_preferences(&self.engine_type)
            .cloned()
            .unwrap_or_else(|| vec![ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt]);

        for protocol_type in preferences {
            if let Some(protocol) = manager.get_protocol(&protocol_type, &self.engine_type) {
                if protocol.is_connected() {
                    match protocol.health_check().await {
                        Ok(health_status) => {
                            return Ok(HealthInfo {
                                process_id: self.process_id,
                                status: if health_status.is_healthy {
                                    HealthStatus::Healthy
                                } else {
                                    HealthStatus::Unhealthy
                                },
                                last_block_processed: None,
                                blocks_processed_total: 0,
                                uptime: self.total_uptime.lock().await.clone(),
                                memory_usage: 0,
                                cpu_usage_percent: 0.0,
                                rpc_active: protocol_type == ProtocolType::Rpc,
                                errors_count: 0,
                                last_error: None,
                                timestamp: health_status.timestamp,
                            });
                        }
                        Err(e) => {
                            debug!("Health check failed with {}: {}", protocol_type, e);
                        }
                    }
                }
            }
        }

        // All protocols failed
        Ok(HealthInfo {
            process_id: self.process_id,
            status: HealthStatus::Unhealthy,
            last_block_processed: None,
            blocks_processed_total: 0,
            uptime: Duration::ZERO,
            memory_usage: 0,
            cpu_usage_percent: 0.0,
            rpc_active: false,
            errors_count: 1,
            last_error: Some("All communication protocols failed".to_string()),
            timestamp: std::time::SystemTime::now(),
        })
    }
}

// Import types and functions from the process module
use crate::process::{DowntimePeriod, ProcessConfig, RestartAttempt};

// Helper function to get engine binary path (duplicated since original is private)
fn get_engine_binary_path(binary_name: &str) -> MultivmResult<PathBuf> {
    let is_test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok() || cfg!(test);

    let workspace_binary = PathBuf::from("target").join("debug").join(binary_name);
    if workspace_binary.exists() {
        return Ok(workspace_binary.canonicalize().unwrap_or(workspace_binary));
    }

    let release_binary = PathBuf::from("target").join("release").join(binary_name);
    if release_binary.exists() {
        return Ok(release_binary.canonicalize().unwrap_or(release_binary));
    }

    if is_test_mode {
        return Ok(PathBuf::from("/tmp").join(binary_name));
    }

    Err(MultivmError::Process {
        process_id: binary_name.to_string(),
        message: "Binary not found".to_string(),
        exit_code: None,
    })
}

// Helper function to get IPC address (duplicated since original is private)
fn get_ipc_address(process_id: &ProcessId, ipc_config: &IpcConfig) -> String {
    match &ipc_config.transport {
        multivm_common::config::IpcTransportConfig::TcpSocket { host, port } => {
            let process_port = match process_id {
                ProcessId::Solana => port + 1,
                ProcessId::Ethereum => port + 2,
                ProcessId::Main => *port,
            };
            format!("{host}:{process_port}")
        }
        multivm_common::config::IpcTransportConfig::UnixSocket { path } => {
            path.to_string_lossy().to_string()
        }
    }
}
