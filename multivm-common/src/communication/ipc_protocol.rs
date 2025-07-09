//! IPC (Inter-Process Communication) protocol implementation
//!
//! This module implements communication with execution engines via IPC using
//! the existing IPC infrastructure with Unix sockets and TCP.

use super::{
    CommunicationProtocol, CommunicationRequest, CommunicationResponse, ConnectionStatus,
    EngineType, HealthStatus as CommHealthStatus, ProtocolType,
};
use crate::ipc::{client::IpcClient, IpcCommand, IpcMessage, IpcResponse, IpcTransport};
use crate::types::{BlockchainType, HealthStatus, MessageId, ProcessId, RpcCall};
use crate::{MultivmError, MultivmResult};
use async_trait::async_trait;
use bincode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Transport wrapper that implements IpcTransport
struct TransportWrapper {
    inner: Box<dyn IpcTransport + Send + Sync>,
}

#[async_trait]
impl IpcTransport for TransportWrapper {
    async fn send(&self, message: IpcMessage) -> MultivmResult<()> {
        self.inner.send(message).await
    }

    async fn receive(&self) -> MultivmResult<IpcMessage> {
        self.inner.receive().await
    }

    async fn send_response(
        &self,
        message_id: MessageId,
        response: IpcResponse,
    ) -> MultivmResult<()> {
        self.inner.send_response(message_id, response).await
    }

    async fn close(&self) -> MultivmResult<()> {
        self.inner.close().await
    }
}

/// Unix socket transport implementation
struct UnixSocketTransport {
    stream: Arc<Mutex<tokio::net::UnixStream>>,
}

/// TCP socket transport implementation  
struct TcpSocketTransport {
    stream: Arc<Mutex<tokio::net::TcpStream>>,
}

#[cfg(unix)]
#[async_trait]
impl IpcTransport for UnixSocketTransport {
    async fn send(&self, message: IpcMessage) -> MultivmResult<()> {
        use tokio::io::AsyncWriteExt;

        let serialized = bincode::serialize(&message).map_err(|e| MultivmError::Serialization {
            message: format!("Failed to serialize IPC message: {e}"),
            data_type: Some("IpcMessage".to_string()),
        })?;

        let mut stream = self.stream.lock().await;

        // Send message length first (4 bytes)
        let len = serialized.len() as u32;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to write message length: {e}"),
                endpoint: Some("unix_socket".to_string()),
                retry_after: None,
            })?;

        // Send message data
        stream
            .write_all(&serialized)
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to write message data: {e}"),
                endpoint: Some("unix_socket".to_string()),
                retry_after: None,
            })?;

        stream.flush().await.map_err(|e| MultivmError::Network {
            message: format!("Failed to flush stream: {e}"),
            endpoint: Some("unix_socket".to_string()),
            retry_after: None,
        })?;

        Ok(())
    }

    async fn receive(&self) -> MultivmResult<IpcMessage> {
        use tokio::io::AsyncReadExt;

        let mut stream = self.stream.lock().await;

        // Read message length (4 bytes)
        let mut len_bytes = [0u8; 4];
        stream
            .read_exact(&mut len_bytes)
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to read message length: {e}"),
                endpoint: Some("unix_socket".to_string()),
                retry_after: None,
            })?;

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Validate message length
        if len > 10 * 1024 * 1024 {
            // 10MB max
            return Err(MultivmError::Validation {
                field: "message_length".to_string(),
                message: format!("Message too large: {len} bytes"),
                value: Some(len.to_string()),
            });
        }

        // Read message data
        let mut buffer = vec![0u8; len];
        stream
            .read_exact(&mut buffer)
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to read message data: {e}"),
                endpoint: Some("unix_socket".to_string()),
                retry_after: None,
            })?;

        let message = bincode::deserialize(&buffer).map_err(|e| MultivmError::Serialization {
            message: format!("Failed to deserialize IPC message: {e}"),
            data_type: Some("IpcMessage".to_string()),
        })?;

        Ok(message)
    }

    async fn send_response(
        &self,
        message_id: MessageId,
        response: IpcResponse,
    ) -> MultivmResult<()> {
        // Create a response message with the original message ID
        // In the real IPC protocol, responses are sent as regular messages
        // with the response encoded in the command field
        let response_command = match response {
            IpcResponse::Ack => IpcCommand::Ping, // Ack is implicit
            IpcResponse::Health { .. } => IpcCommand::GetHealth,
            IpcResponse::State { .. } => IpcCommand::GetState,
            IpcResponse::Pong => IpcCommand::Ping,
            _ => IpcCommand::Ping, // Default
        };

        let message = IpcMessage {
            id: message_id,
            source: ProcessId::Main,
            destination: ProcessId::Main,
            command: response_command,
            timestamp: SystemTime::now(),
            timeout: None,
        };

        // Send the response message
        self.send(message).await
    }

    async fn close(&self) -> MultivmResult<()> {
        Ok(())
    }
}

#[async_trait]
impl IpcTransport for TcpSocketTransport {
    async fn send(&self, message: IpcMessage) -> MultivmResult<()> {
        use tokio::io::AsyncWriteExt;

        let serialized = bincode::serialize(&message).map_err(|e| MultivmError::Serialization {
            message: format!("Failed to serialize IPC message: {e}"),
            data_type: Some("IpcMessage".to_string()),
        })?;

        let mut stream = self.stream.lock().await;

        // Send message length first (4 bytes)
        let len = serialized.len() as u32;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to write message length: {e}"),
                endpoint: Some("tcp_socket".to_string()),
                retry_after: None,
            })?;

        // Send message data
        stream
            .write_all(&serialized)
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to write message data: {e}"),
                endpoint: Some("tcp_socket".to_string()),
                retry_after: None,
            })?;

        stream.flush().await.map_err(|e| MultivmError::Network {
            message: format!("Failed to flush stream: {e}"),
            endpoint: Some("tcp_socket".to_string()),
            retry_after: None,
        })?;

        Ok(())
    }

    async fn receive(&self) -> MultivmResult<IpcMessage> {
        use tokio::io::AsyncReadExt;

        let mut stream = self.stream.lock().await;

        // Read message length (4 bytes)
        let mut len_bytes = [0u8; 4];
        stream
            .read_exact(&mut len_bytes)
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to read message length: {e}"),
                endpoint: Some("tcp_socket".to_string()),
                retry_after: None,
            })?;

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Validate message length
        if len > 10 * 1024 * 1024 {
            // 10MB max
            return Err(MultivmError::Validation {
                field: "message_length".to_string(),
                message: format!("Message too large: {len} bytes"),
                value: Some(len.to_string()),
            });
        }

        // Read message data
        let mut buffer = vec![0u8; len];
        stream
            .read_exact(&mut buffer)
            .await
            .map_err(|e| MultivmError::Network {
                message: format!("Failed to read message data: {e}"),
                endpoint: Some("tcp_socket".to_string()),
                retry_after: None,
            })?;

        let message = bincode::deserialize(&buffer).map_err(|e| MultivmError::Serialization {
            message: format!("Failed to deserialize IPC message: {e}"),
            data_type: Some("IpcMessage".to_string()),
        })?;

        Ok(message)
    }

    async fn send_response(
        &self,
        message_id: MessageId,
        response: IpcResponse,
    ) -> MultivmResult<()> {
        // Create a response message with the original message ID
        // In the real IPC protocol, responses are sent as regular messages
        // with the response encoded in the command field
        let response_command = match response {
            IpcResponse::Ack => IpcCommand::Ping, // Ack is implicit
            IpcResponse::Health { .. } => IpcCommand::GetHealth,
            IpcResponse::State { .. } => IpcCommand::GetState,
            IpcResponse::Pong => IpcCommand::Ping,
            _ => IpcCommand::Ping, // Default
        };

        let message = IpcMessage {
            id: message_id,
            source: ProcessId::Main,
            destination: ProcessId::Main,
            command: response_command,
            timestamp: SystemTime::now(),
            timeout: None,
        };

        // Send the response message
        self.send(message).await
    }

    async fn close(&self) -> MultivmResult<()> {
        Ok(())
    }
}

/// Configuration for IPC protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcProtocolConfig {
    /// Target process ID for the execution engine
    pub target_process_id: ProcessId,
    /// Our process ID
    pub source_process_id: ProcessId,
    /// IPC transport configuration
    pub transport_config: IpcTransportConfig,
    /// Default request timeout
    pub default_timeout: Duration,
    /// Connection retry configuration
    pub retry_config: RetryConfig,
    /// Health check configuration
    pub health_check_config: HealthCheckConfig,
}

/// IPC transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcTransportConfig {
    /// Whether to use Unix domain sockets (if supported)
    pub use_unix_sockets: bool,
    /// TCP host for fallback
    pub tcp_host: String,
    /// TCP port for the target process
    pub tcp_port: u16,
    /// Socket path for Unix domain sockets
    pub unix_socket_path: Option<String>,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Read timeout
    pub read_timeout: Duration,
    /// Write timeout
    pub write_timeout: Duration,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check interval
    pub interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Enable automatic health checks
    pub enable_auto_check: bool,
}

impl Default for IpcProtocolConfig {
    fn default() -> Self {
        Self {
            target_process_id: ProcessId::Main, // Default to Main
            source_process_id: ProcessId::Main, // Default to Main
            transport_config: IpcTransportConfig::default(),
            default_timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
            health_check_config: HealthCheckConfig::default(),
        }
    }
}

impl Default for IpcTransportConfig {
    fn default() -> Self {
        Self {
            use_unix_sockets: true,
            tcp_host: "127.0.0.1".to_string(),
            tcp_port: 0, // Will be set based on engine type
            unix_socket_path: None,
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(10),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            enable_auto_check: true,
        }
    }
}

/// IPC protocol implementation
pub struct IpcProtocol {
    /// Engine type this protocol communicates with
    engine_type: EngineType,
    /// Protocol configuration
    config: IpcProtocolConfig,
    /// IPC client for communication
    ipc_client: Arc<Mutex<Option<IpcClient<TransportWrapper>>>>,
    /// Connection status
    connection_status: Arc<RwLock<ConnectionStatus>>,
    /// Health check task handle
    health_check_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl IpcProtocol {
    /// Create a new IPC protocol instance
    pub fn new(engine_type: EngineType, config: IpcProtocolConfig) -> Self {
        let connection_status = ConnectionStatus {
            is_connected: false,
            last_success: None,
            latency_ms: None,
            success_count: 0,
            error_count: 0,
        };

        Self {
            engine_type,
            config,
            ipc_client: Arc::new(Mutex::new(None)),
            connection_status: Arc::new(RwLock::new(connection_status)),
            health_check_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Convert CommunicationRequest to IpcMessage
    async fn to_ipc_message(&self, request: CommunicationRequest) -> MultivmResult<IpcMessage> {
        let command = match request.method.as_str() {
            "get_health" => IpcCommand::GetHealth,
            "get_state" => IpcCommand::GetState,
            "process_block" => {
                // Extract block data from params
                let block_data = request
                    .params
                    .get("block_data")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MultivmError::Validation {
                        field: "block_data".to_string(),
                        message: "Missing block_data parameter".to_string(),
                        value: None,
                    })?;

                let blockchain_type = request
                    .params
                    .get("blockchain_type")
                    .and_then(|v| v.as_str())
                    .and_then(|s| match s {
                        "ethereum" => Some(BlockchainType::Ethereum),
                        "solana" => Some(BlockchainType::Solana),
                        _ => None,
                    })
                    .unwrap_or(BlockchainType::Ethereum);

                IpcCommand::ProcessBlock {
                    block_data_bytes: Box::new(block_data.as_bytes().to_vec()),
                    blockchain_type,
                    expect_response: request
                        .params
                        .get("expect_response")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                }
            }
            "rpc_call" => {
                let method = request
                    .params
                    .get("method")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MultivmError::Validation {
                        field: "method".to_string(),
                        message: "Missing method parameter for RPC call".to_string(),
                        value: None,
                    })?;

                let params = request
                    .params
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                let call = RpcCall {
                    method: method.to_string(),
                    params,
                    id: serde_json::json!(1), // Default RPC ID
                };

                IpcCommand::RpcCall { call }
            }
            "shutdown" => {
                let graceful = request
                    .params
                    .get("graceful")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let timeout = request
                    .params
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .map(Duration::from_secs);
                IpcCommand::Shutdown { graceful, timeout }
            }
            _ => {
                return Err(MultivmError::UnsupportedOperation {
                    operation: request.method,
                    alternatives: Some(vec![
                        "get_health".to_string(),
                        "get_state".to_string(),
                        "process_block".to_string(),
                        "rpc_call".to_string(),
                        "shutdown".to_string(),
                    ]),
                });
            }
        };

        let message_id = MessageId::new();

        Ok(IpcMessage {
            id: message_id,
            source: self.config.source_process_id,
            destination: self.config.target_process_id,
            command,
            timestamp: SystemTime::now(),
            timeout: request.timeout.or(Some(self.config.default_timeout)),
        })
    }

    /// Convert IpcResponse to CommunicationResponse
    fn from_ipc_response(
        &self,
        response: IpcResponse,
        request_id: String,
    ) -> CommunicationResponse {
        let (result, error) = match response {
            IpcResponse::Ack => (Some(serde_json::Value::Bool(true)), None),
            IpcResponse::Health { status } => (Some(serde_json::json!({ "status": status })), None),
            IpcResponse::State { state } => (Some(serde_json::json!({ "state": state })), None),
            IpcResponse::Pong => (Some(serde_json::Value::String("pong".to_string())), None),
            _ => {
                // For other response types, serialize them
                let data = serde_json::to_value(&response).unwrap_or(serde_json::Value::Null);
                (Some(data), None)
            }
        };

        let mut metadata = HashMap::new();
        metadata.insert("protocol".to_string(), "ipc".to_string());
        metadata.insert(
            "timestamp".to_string(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
        );

        CommunicationResponse {
            request_id,
            result,
            error,
            metadata,
        }
    }

    /// Initialize IPC transport and create client
    async fn initialize_transport(&self) -> MultivmResult<TransportWrapper> {
        info!("Initializing IPC transport for {} engine", self.engine_type);

        let transport_config = &self.config.transport_config;

        // Try Unix domain sockets first if enabled and supported
        #[cfg(unix)]
        if transport_config.use_unix_sockets {
            if let Some(socket_path) = &transport_config.unix_socket_path {
                match tokio::net::UnixStream::connect(socket_path).await {
                    Ok(stream) => {
                        info!("Connected via Unix domain socket: {}", socket_path);
                        let transport = UnixSocketTransport {
                            stream: Arc::new(Mutex::new(stream)),
                        };
                        return Ok(TransportWrapper {
                            inner: Box::new(transport),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to connect via Unix socket {}: {}", socket_path, e);
                    }
                }
            }
        }

        // Fallback to TCP
        let tcp_addr = format!(
            "{}:{}",
            transport_config.tcp_host, transport_config.tcp_port
        );
        info!("Connecting via TCP: {}", tcp_addr);

        match tokio::net::TcpStream::connect(&tcp_addr).await {
            Ok(stream) => {
                let transport = TcpSocketTransport {
                    stream: Arc::new(Mutex::new(stream)),
                };
                Ok(TransportWrapper {
                    inner: Box::new(transport),
                })
            }
            Err(e) => Err(MultivmError::Network {
                message: format!("Failed to establish TCP connection: {e}"),
                endpoint: Some(tcp_addr),
                retry_after: None,
            }),
        }
    }

    /// Send IPC message with retry logic
    async fn send_with_retry(&self, message: IpcMessage) -> MultivmResult<IpcResponse> {
        let retry_config = &self.config.retry_config;
        let mut delay = retry_config.base_delay;
        let mut last_error = None;

        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                debug!("Retry attempt {} for message {}", attempt, message.id);
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(
                    Duration::from_millis(
                        (delay.as_millis() as f64 * retry_config.backoff_multiplier) as u64,
                    ),
                    retry_config.max_delay,
                );
            }

            let mut client_guard = self.ipc_client.lock().await;

            // Create client if not exists
            if client_guard.is_none() {
                match self.initialize_transport().await {
                    Ok(transport) => {
                        let client = IpcClient::new(transport, self.config.source_process_id);
                        *client_guard = Some(client);
                    }
                    Err(e) => {
                        last_error = Some(e);
                        continue;
                    }
                }
            }

            if let Some(client) = client_guard.as_ref() {
                let timeout = message.timeout.unwrap_or(self.config.default_timeout);

                match client
                    .send_command(message.destination, message.command.clone(), Some(timeout))
                    .await
                {
                    Ok(response) => {
                        // Update connection status on success
                        let mut status = self.connection_status.write().await;
                        status.last_success = Some(SystemTime::now());
                        status.success_count += 1;
                        status.is_connected = true;
                        return Ok(response);
                    }
                    Err(e) => {
                        error!(engine_type = %self.engine_type, attempt = attempt + 1, error = %e, "IPC send failed");
                        last_error = Some(e);

                        // Clear client on failure
                        *client_guard = None;

                        // Update error count
                        let mut status = self.connection_status.write().await;
                        status.error_count += 1;
                        status.is_connected = false;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| MultivmError::Network {
            message: "All IPC retry attempts exhausted".to_string(),
            endpoint: Some("ipc_protocol".to_string()),
            retry_after: None,
        }))
    }

    /// Start automatic health check task
    async fn start_health_check_task(&self) {
        if !self.config.health_check_config.enable_auto_check {
            return;
        }

        let config = self.config.health_check_config.clone();
        let client = Arc::clone(&self.ipc_client);
        let status = Arc::clone(&self.connection_status);
        let source_process_id = self.config.source_process_id;
        let target_process_id = self.config.target_process_id;

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.interval);

            loop {
                interval.tick().await;

                let health_message = IpcMessage {
                    id: MessageId::new(),
                    source: source_process_id,
                    destination: target_process_id,
                    command: IpcCommand::GetHealth,
                    timestamp: SystemTime::now(),
                    timeout: Some(config.timeout),
                };

                let start_time = std::time::Instant::now();
                let client_guard = client.lock().await;

                if let Some(ipc_client) = client_guard.as_ref() {
                    match ipc_client
                        .send_command(
                            health_message.destination,
                            health_message.command,
                            Some(config.timeout),
                        )
                        .await
                    {
                        Ok(IpcResponse::Health {
                            status: health_status,
                        }) => {
                            let latency = start_time.elapsed().as_millis() as u64;
                            let mut status_guard = status.write().await;
                            status_guard.is_connected = health_status == HealthStatus::Healthy;
                            status_guard.latency_ms = Some(latency);
                            status_guard.last_success = Some(SystemTime::now());
                            debug!(
                                "Health check successful, latency: {}ms, status: {:?}",
                                latency, health_status
                            );
                        }
                        Ok(_) => {
                            warn!("Unexpected response to health check");
                        }
                        Err(e) => {
                            warn!("Health check failed: {}", e);
                            let mut status_guard = status.write().await;
                            status_guard.is_connected = false;
                            status_guard.error_count += 1;
                        }
                    }
                }
            }
        });

        *self.health_check_handle.lock().await = Some(task);
    }

    /// Stop health check task
    async fn stop_health_check_task(&self) {
        if let Some(handle) = self.health_check_handle.lock().await.take() {
            handle.abort();
        }
    }
}

#[async_trait]
impl CommunicationProtocol for IpcProtocol {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Ipc
    }

    fn engine_type(&self) -> EngineType {
        self.engine_type.clone()
    }

    async fn connect(&mut self) -> MultivmResult<()> {
        info!(engine_type = %self.engine_type, "Connecting IPC protocol");

        // Initialize transport and create IPC client
        let transport = self.initialize_transport().await?;
        let client = IpcClient::new(transport, self.config.source_process_id);

        // Test connection with a ping
        let ping_message = IpcMessage {
            id: MessageId::new(),
            source: self.config.source_process_id,
            destination: self.config.target_process_id,
            command: IpcCommand::Ping,
            timestamp: SystemTime::now(),
            timeout: Some(Duration::from_secs(5)),
        };

        match client
            .send_command(
                ping_message.destination,
                ping_message.command,
                Some(Duration::from_secs(5)),
            )
            .await
        {
            Ok(IpcResponse::Pong) => {
                info!(engine_type = %self.engine_type, "IPC ping successful");
            }
            Ok(_) => {
                warn!(engine_type = %self.engine_type, "Unexpected response to ping");
            }
            Err(e) => {
                return Err(MultivmError::Network {
                    message: format!("Failed to establish IPC connection: {e}"),
                    endpoint: Some("ipc_protocol".to_string()),
                    retry_after: None,
                });
            }
        }

        *self.ipc_client.lock().await = Some(client);

        // Update connection status
        {
            let mut status = self.connection_status.write().await;
            status.is_connected = true;
            status.last_success = Some(SystemTime::now());
        }

        // Start health check task
        self.start_health_check_task().await;

        info!(engine_type = %self.engine_type, source = ?self.config.source_process_id, target = ?self.config.target_process_id, "IPC protocol connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> MultivmResult<()> {
        info!(
            "Disconnecting IPC protocol from {} engine",
            self.engine_type
        );

        // Stop health check task
        self.stop_health_check_task().await;

        // Clear IPC client
        if let Some(client) = self.ipc_client.lock().await.take() {
            // IpcClient doesn't have a close method, just drop it
            drop(client);
        }

        // Update connection status
        {
            let mut status = self.connection_status.write().await;
            status.is_connected = false;
        }

        info!("IPC protocol disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Use a non-blocking check
        if let Ok(status) = self.connection_status.try_read() {
            status.is_connected
        } else {
            false
        }
    }

    async fn send_request(
        &self,
        request: CommunicationRequest,
    ) -> MultivmResult<CommunicationResponse> {
        debug!(
            "Sending IPC request: {} to {}",
            request.method, self.engine_type
        );

        let ipc_message = self.to_ipc_message(request.clone()).await?;
        let request_id = request.id.clone();

        let start_time = std::time::Instant::now();
        let response_message = self.send_with_retry(ipc_message).await?;
        let latency = start_time.elapsed().as_millis() as u64;

        // Update latency in connection status
        {
            let mut status = self.connection_status.write().await;
            status.latency_ms = Some(latency);
        }

        let response = self.from_ipc_response(response_message, request_id);

        debug!("IPC request completed in {}ms", latency);
        Ok(response)
    }

    async fn send_notification(&self, request: CommunicationRequest) -> MultivmResult<()> {
        debug!(
            "Sending IPC notification: {} to {}",
            request.method, self.engine_type
        );

        let ipc_message = self.to_ipc_message(request).await?;

        // For notifications, we don't expect a response
        // This could be implemented by setting a flag in the message
        // or using a different transport method

        self.send_with_retry(ipc_message).await?;
        Ok(())
    }

    async fn get_connection_status(&self) -> MultivmResult<ConnectionStatus> {
        Ok(self.connection_status.read().await.clone())
    }

    async fn health_check(&self) -> MultivmResult<CommHealthStatus> {
        let health_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "get_health".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(self.config.health_check_config.timeout),
            auth_context: None,
        };

        let response = self.send_request(health_request).await?;

        let is_healthy = response.error.is_none();
        let mut capabilities = vec!["ipc".to_string()];
        let mut metrics = HashMap::new();

        if let Some(result) = response.result {
            if let Some(caps) = result.get("capabilities").and_then(|v| v.as_array()) {
                capabilities.extend(caps.iter().filter_map(|v| v.as_str().map(String::from)));
            }

            if let Some(metrics_obj) = result.get("metrics").and_then(|v| v.as_object()) {
                for (key, value) in metrics_obj {
                    metrics.insert(key.clone(), value.clone());
                }
            }
        }

        let status = self.connection_status.read().await;
        metrics.insert(
            "success_count".to_string(),
            serde_json::Value::Number(status.success_count.into()),
        );
        metrics.insert(
            "error_count".to_string(),
            serde_json::Value::Number(status.error_count.into()),
        );

        if let Some(latency) = status.latency_ms {
            metrics.insert(
                "latency_ms".to_string(),
                serde_json::Value::Number(latency.into()),
            );
        }

        Ok(CommHealthStatus {
            is_healthy,
            version: None, // Could be extracted from health response
            capabilities,
            metrics,
            timestamp: SystemTime::now(),
        })
    }

    async fn subscribe(&self, event_types: Vec<String>) -> MultivmResult<()> {
        // IPC subscription could be implemented using a Subscribe command
        debug!("Subscribing to events: {:?}", event_types);

        let subscribe_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "subscribe".to_string(),
            params: serde_json::json!({ "event_types": event_types }),
            timeout: Some(self.config.default_timeout),
            auth_context: None,
        };

        self.send_request(subscribe_request).await?;
        Ok(())
    }

    async fn unsubscribe(&self, event_types: Vec<String>) -> MultivmResult<()> {
        debug!("Unsubscribing from events: {:?}", event_types);

        let unsubscribe_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "unsubscribe".to_string(),
            params: serde_json::json!({ "event_types": event_types }),
            timeout: Some(self.config.default_timeout),
            auth_context: None,
        };

        self.send_request(unsubscribe_request).await?;
        Ok(())
    }

    fn get_configuration(&self) -> serde_json::Value {
        serde_json::to_value(&self.config).unwrap_or(serde_json::Value::Null)
    }

    async fn update_configuration(&mut self, config: serde_json::Value) -> MultivmResult<()> {
        let new_config: IpcProtocolConfig =
            serde_json::from_value(config).map_err(|e| MultivmError::Configuration {
                component: "ipc_protocol".to_string(),
                message: format!("Invalid configuration: {e}"),
                validation_errors: None,
            })?;

        self.config = new_config;

        // If we're connected, we might need to reconnect with new configuration
        if self.is_connected() {
            warn!("Configuration updated while connected - reconnection may be required");
        }

        Ok(())
    }
}
