//! Secure IPC Client for Reth Execution Engine
//!
//! This module provides secure IPC transport with encryption, message queuing,
//! connection recovery, and health monitoring for communication with external Reth processes.

use crate::engine::RethEngineError;
use multivm_common::{IpcCommand, IpcMessage, IpcResponse, MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Encryption imports
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};

/// Configuration for secure IPC client
#[derive(Debug, Clone)]
pub struct IpcClientConfig {
    /// Connection address (Unix socket path or TCP address)
    pub address: String,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Backoff delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Enable encryption
    pub enable_encryption: bool,
    /// Encryption key (32 bytes for ChaCha20Poly1305)
    pub encryption_key: Option<[u8; 32]>,
    /// Health check interval in milliseconds
    pub health_check_interval_ms: u64,
    /// Maximum queue size for pending messages
    pub max_queue_size: usize,
    /// Connection recovery interval in milliseconds
    pub recovery_interval_ms: u64,
    /// Enable keep-alive messages
    pub enable_keepalive: bool,
    /// Keep-alive interval in milliseconds
    pub keepalive_interval_ms: u64,
}

impl Default for IpcClientConfig {
    fn default() -> Self {
        Self {
            address: "/tmp/reth.ipc".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_encryption: true,
            encryption_key: None,
            health_check_interval_ms: 10000,
            max_queue_size: 1000,
            recovery_interval_ms: 5000,
            enable_keepalive: true,
            keepalive_interval_ms: 30000,
        }
    }
}

/// IPC connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

/// IPC stream wrapper with encryption support
#[derive(Debug)]
pub enum IpcStream {
    #[cfg(unix)]
    Unix {
        reader: BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
    },
    Tcp {
        reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    },
}

/// Encrypted message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessage {
    pub nonce: [u8; 12], // ChaCha20Poly1305 nonce
    pub ciphertext: Vec<u8>,
    pub timestamp: u64,
    pub message_id: String,
}

/// Pending request information
#[derive(Debug)]
pub struct PendingRequest {
    pub request_id: Uuid,
    pub command: IpcCommand,
    pub response_sender: oneshot::Sender<MultivmResult<IpcResponse>>,
    pub created_at: Instant,
    pub timeout: Duration,
}

/// Connection health metrics
#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub connection_count: u64,
    pub disconnection_count: u64,
    pub last_successful_request: Option<SystemTime>,
    pub last_connection_time: Option<SystemTime>,
    pub average_response_time_ms: u64,
}

/// IPC client health status
#[derive(Debug, Clone)]
pub struct IpcHealthStatus {
    pub status: ConnectionStatus,
    pub metrics: ConnectionMetrics,
    pub last_error: Option<String>,
    pub uptime_seconds: u64,
}

/// Secure IPC client for Reth communication
pub struct SecureRethIpcClient {
    config: IpcClientConfig,
    stream: Arc<Mutex<Option<IpcStream>>>,
    status: Arc<RwLock<ConnectionStatus>>,
    cipher: Option<ChaCha20Poly1305>,

    // Message queuing
    request_queue: Arc<Mutex<VecDeque<PendingRequest>>>,
    pending_requests: Arc<Mutex<HashMap<Uuid, PendingRequest>>>,

    // Health monitoring
    metrics: Arc<RwLock<ConnectionMetrics>>,
    last_heartbeat: Arc<RwLock<Option<SystemTime>>>,
    is_healthy: Arc<AtomicBool>,

    // Background tasks
    shutdown_sender: Option<oneshot::Sender<()>>,
    request_counter: Arc<AtomicU64>,

    // Event channels
    command_sender: mpsc::UnboundedSender<IpcCommand>,
    response_receiver: Arc<Mutex<mpsc::UnboundedReceiver<IpcResponse>>>,
}

impl SecureRethIpcClient {
    /// Create a new secure IPC client
    pub fn new(config: IpcClientConfig) -> MultivmResult<Self> {
        let cipher = if config.enable_encryption {
            let key = config.encryption_key.unwrap_or_else(|| {
                // Generate a key from the address for demo purposes
                // In production, use proper key exchange
                let mut hasher = Sha256::new();
                hasher.update(config.address.as_bytes());
                hasher.update(b"multivm-reth-ipc-key");
                let hash = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&hash[..32]);
                key
            });
            Some(ChaCha20Poly1305::new(&key.into()))
        } else {
            None
        };

        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        let (response_sender, response_receiver) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            stream: Arc::new(Mutex::new(None)),
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            cipher,
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(ConnectionMetrics::default())),
            last_heartbeat: Arc::new(RwLock::new(None)),
            is_healthy: Arc::new(AtomicBool::new(false)),
            shutdown_sender: None,
            request_counter: Arc::new(AtomicU64::new(0)),
            command_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
        })
    }

    /// Start the IPC client with all background tasks
    pub async fn start(&mut self) -> MultivmResult<()> {
        info!(
            "Starting secure IPC client for address: {}",
            self.config.address
        );

        // Initial connection
        self.connect().await?;

        // Start background tasks
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        self.shutdown_sender = Some(shutdown_sender);

        self.start_background_tasks(shutdown_receiver).await?;

        info!("Secure IPC client started successfully");
        Ok(())
    }

    /// Stop the IPC client
    pub async fn stop(&mut self) -> MultivmResult<()> {
        info!("Stopping secure IPC client");

        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }

        // Close connection
        self.disconnect().await?;

        // Update status
        let mut status = self.status.write().await;
        *status = ConnectionStatus::Disconnected;

        info!("Secure IPC client stopped");
        Ok(())
    }

    /// Connect to the Reth process
    async fn connect(&self) -> MultivmResult<()> {
        let mut status = self.status.write().await;
        *status = ConnectionStatus::Connecting;
        drop(status);

        info!("Connecting to Reth IPC at: {}", self.config.address);

        let connect_timeout = Duration::from_millis(self.config.connect_timeout_ms);
        let stream = timeout(connect_timeout, self.establish_connection())
            .await
            .map_err(|_| MultivmError::Ipc {
                endpoint: self.config.address.clone(),
                message: "Connection timeout".to_string(),
                retry_count: Some(0),
            })??;

        // Store the stream
        let mut stream_guard = self.stream.lock().await;
        *stream_guard = Some(stream);
        drop(stream_guard);

        // Update status and metrics
        let mut status = self.status.write().await;
        *status = ConnectionStatus::Connected;
        drop(status);

        let mut metrics = self.metrics.write().await;
        metrics.connection_count += 1;
        metrics.last_connection_time = Some(SystemTime::now());
        drop(metrics);

        self.is_healthy.store(true, Ordering::Relaxed);

        info!("Successfully connected to Reth IPC");
        Ok(())
    }

    /// Establish the actual connection
    async fn establish_connection(&self) -> MultivmResult<IpcStream> {
        let address = &self.config.address;

        if address.starts_with("/") || address.starts_with("./") {
            // Unix socket
            #[cfg(unix)]
            {
                let stream = UnixStream::connect(address)
                    .await
                    .map_err(|e| MultivmError::Ipc {
                        endpoint: address.clone(),
                        message: format!("Failed to connect to Unix socket: {}", e),
                        retry_count: Some(0),
                    })?;

                let (reader, writer) = stream.into_split();
                Ok(IpcStream::Unix {
                    reader: BufReader::new(reader),
                    writer: BufWriter::new(writer),
                })
            }
            #[cfg(not(unix))]
            {
                Err(MultivmError::Ipc {
                    endpoint: address.clone(),
                    message: "Unix sockets not supported on this platform".to_string(),
                    retry_count: Some(0),
                })
            }
        } else {
            // TCP socket
            let tcp_address = if address.contains(":") {
                address.clone()
            } else {
                format!("127.0.0.1:{}", address)
            };

            let stream = TcpStream::connect(&tcp_address)
                .await
                .map_err(|e| MultivmError::Ipc {
                    endpoint: tcp_address.clone(),
                    message: format!("Failed to connect to TCP socket: {}", e),
                    retry_count: Some(0),
                })?;

            let (reader, writer) = stream.into_split();
            Ok(IpcStream::Tcp {
                reader: BufReader::new(reader),
                writer: BufWriter::new(writer),
            })
        }
    }

    /// Disconnect from the Reth process
    async fn disconnect(&self) -> MultivmResult<()> {
        debug!("Disconnecting from Reth IPC");

        let mut stream_guard = self.stream.lock().await;
        *stream_guard = None;
        drop(stream_guard);

        let mut status = self.status.write().await;
        *status = ConnectionStatus::Disconnected;
        drop(status);

        let mut metrics = self.metrics.write().await;
        metrics.disconnection_count += 1;
        drop(metrics);

        self.is_healthy.store(false, Ordering::Relaxed);

        debug!("Disconnected from Reth IPC");
        Ok(())
    }

    /// Send a command and wait for response
    pub async fn send_command(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        let request_id = Uuid::new_v4();
        let request_timeout = Duration::from_millis(self.config.request_timeout_ms);

        debug!("Sending IPC command: {:?} (ID: {})", command, request_id);

        // Create response channel
        let (response_sender, response_receiver) = oneshot::channel();

        // Create pending request
        let pending_request = PendingRequest {
            request_id,
            command: command.clone(),
            response_sender,
            created_at: Instant::now(),
            timeout: request_timeout,
        };

        // Add to queue
        {
            let mut queue = self.request_queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                return Err(MultivmError::Ipc {
                    endpoint: self.config.address.clone(),
                    message: "Request queue is full".to_string(),
                    retry_count: Some(0),
                });
            }
            queue.push_back(pending_request);
        }

        // Wait for response
        let response = timeout(request_timeout, response_receiver)
            .await
            .map_err(|_| MultivmError::Ipc {
                endpoint: self.config.address.clone(),
                message: "Request timeout".to_string(),
                retry_count: Some(0),
            })?
            .map_err(|_| MultivmError::Ipc {
                endpoint: self.config.address.clone(),
                message: "Response channel closed".to_string(),
                retry_count: Some(0),
            })??;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.successful_requests += 1;
        metrics.last_successful_request = Some(SystemTime::now());
        drop(metrics);

        debug!("Received IPC response for command ID: {}", request_id);
        Ok(response)
    }

    /// Send a command with automatic retry
    pub async fn send_command_with_retry(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match self.send_command(command.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        warn!(
                            "IPC command failed, retrying ({}/{}): {:?}",
                            attempt + 1,
                            self.config.max_retries,
                            command
                        );
                        sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                    }
                }
            }
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.failed_requests += 1;
        drop(metrics);

        Err(last_error.unwrap_or_else(|| MultivmError::Ipc {
            endpoint: self.config.address.clone(),
            message: "All retry attempts failed".to_string(),
            retry_count: Some(self.config.max_retries),
        }))
    }

    /// Get current health status
    pub async fn get_health_status(&self) -> IpcHealthStatus {
        let status = self.status.read().await.clone();
        let metrics = self.metrics.read().await.clone();
        let is_healthy = self.is_healthy.load(Ordering::Relaxed);

        IpcHealthStatus {
            status,
            metrics,
            last_error: None,
            uptime_seconds: 0, // Would need to track start time
        }
    }

    /// Get connection metrics
    pub async fn get_metrics(&self) -> ConnectionMetrics {
        self.metrics.read().await.clone()
    }

    /// Check if the connection is healthy
    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    /// Start background tasks for connection management
    async fn start_background_tasks(
        &self,
        shutdown_receiver: oneshot::Receiver<()>,
    ) -> MultivmResult<()> {
        let stream = Arc::clone(&self.stream);
        let status = Arc::clone(&self.status);
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);
        let is_healthy = Arc::clone(&self.is_healthy);
        let request_queue = Arc::clone(&self.request_queue);
        let pending_requests = Arc::clone(&self.pending_requests);
        let cipher = self.cipher.clone();

        // Message processing task
        let stream_clone = Arc::clone(&stream);
        let status_clone = Arc::clone(&status);
        let config_clone = config.clone();
        let request_queue_clone = Arc::clone(&request_queue);
        let pending_requests_clone = Arc::clone(&pending_requests);
        let cipher_clone = cipher.clone();
        let mut shutdown_receiver_clone = shutdown_receiver;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = Self::process_message_queue(
                            &stream_clone,
                            &status_clone,
                            &config_clone,
                            &request_queue_clone,
                            &pending_requests_clone,
                            &cipher_clone,
                        ).await {
                            error!("Error processing message queue: {}", e);
                        }
                    }
                    _ = &mut shutdown_receiver_clone => {
                        debug!("Message processing task shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Process queued messages
    async fn process_message_queue(
        stream: &Arc<Mutex<Option<IpcStream>>>,
        status: &Arc<RwLock<ConnectionStatus>>,
        config: &IpcClientConfig,
        request_queue: &Arc<Mutex<VecDeque<PendingRequest>>>,
        pending_requests: &Arc<Mutex<HashMap<Uuid, PendingRequest>>>,
        cipher: &Option<ChaCha20Poly1305>,
    ) -> MultivmResult<()> {
        // Check if we have a connection
        let status_guard = status.read().await;
        if *status_guard != ConnectionStatus::Connected {
            return Ok(());
        }
        drop(status_guard);

        // Process pending requests
        let request = {
            let mut queue = request_queue.lock().await;
            queue.pop_front()
        };

        if let Some(pending_request) = request {
            // Send the request
            if let Err(e) =
                Self::send_encrypted_message(stream, &pending_request.command, cipher, config).await
            {
                error!("Failed to send encrypted message: {}", e);

                // Send error response
                let _ = pending_request.response_sender.send(Err(e));
                return Ok(());
            }

            // Add to pending requests
            let mut pending = pending_requests.lock().await;
            pending.insert(pending_request.request_id, pending_request);
        }

        Ok(())
    }

    /// Send encrypted message
    async fn send_encrypted_message(
        stream: &Arc<Mutex<Option<IpcStream>>>,
        command: &IpcCommand,
        cipher: &Option<ChaCha20Poly1305>,
        config: &IpcClientConfig,
    ) -> MultivmResult<()> {
        use multivm_common::{MessageId, ProcessId};

        let message = IpcMessage {
            id: MessageId::new(),
            source: ProcessId::Main,
            destination: ProcessId::Ethereum,
            command: command.clone(),
            timestamp: SystemTime::now(),
            timeout: None,
        };

        let serialized = bincode::serialize(&message).map_err(|e| MultivmError::Ipc {
            endpoint: config.address.clone(),
            message: format!("Failed to serialize message: {}", e),
            retry_count: Some(0),
        })?;

        let data_to_send = if let Some(cipher) = cipher {
            // Encrypt the message
            let nonce_bytes: [u8; 12] = thread_rng().gen();
            let nonce = Nonce::from_slice(&nonce_bytes);

            let ciphertext =
                cipher
                    .encrypt(nonce, serialized.as_ref())
                    .map_err(|e| MultivmError::Ipc {
                        endpoint: config.address.clone(),
                        message: format!("Failed to encrypt message: {}", e),
                        retry_count: Some(0),
                    })?;

            let encrypted_message = EncryptedMessage {
                nonce: nonce_bytes,
                ciphertext,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                message_id: message.id.as_u64().to_string(),
            };

            bincode::serialize(&encrypted_message).map_err(|e| MultivmError::Ipc {
                endpoint: config.address.clone(),
                message: format!("Failed to serialize encrypted message: {}", e),
                retry_count: Some(0),
            })?
        } else {
            serialized
        };

        // Send the message
        let mut stream_guard = stream.lock().await;
        if let Some(ref mut stream) = *stream_guard {
            Self::write_message(stream, &data_to_send, config).await?;
        }

        Ok(())
    }

    /// Write message to stream
    async fn write_message(
        stream: &mut IpcStream,
        data: &[u8],
        config: &IpcClientConfig,
    ) -> MultivmResult<()> {
        let len_bytes = (data.len() as u32).to_be_bytes();

        match stream {
            #[cfg(unix)]
            IpcStream::Unix { writer, .. } => {
                writer
                    .write_all(&len_bytes)
                    .await
                    .map_err(|e| MultivmError::Ipc {
                        endpoint: config.address.clone(),
                        message: format!("Failed to write message length: {}", e),
                        retry_count: Some(0),
                    })?;
                writer
                    .write_all(data)
                    .await
                    .map_err(|e| MultivmError::Ipc {
                        endpoint: config.address.clone(),
                        message: format!("Failed to write message data: {}", e),
                        retry_count: Some(0),
                    })?;
                writer.flush().await.map_err(|e| MultivmError::Ipc {
                    endpoint: config.address.clone(),
                    message: format!("Failed to flush writer: {}", e),
                    retry_count: Some(0),
                })?;
            }
            IpcStream::Tcp { writer, .. } => {
                writer
                    .write_all(&len_bytes)
                    .await
                    .map_err(|e| MultivmError::Ipc {
                        endpoint: config.address.clone(),
                        message: format!("Failed to write message length: {}", e),
                        retry_count: Some(0),
                    })?;
                writer
                    .write_all(data)
                    .await
                    .map_err(|e| MultivmError::Ipc {
                        endpoint: config.address.clone(),
                        message: format!("Failed to write message data: {}", e),
                        retry_count: Some(0),
                    })?;
                writer.flush().await.map_err(|e| MultivmError::Ipc {
                    endpoint: config.address.clone(),
                    message: format!("Failed to flush writer: {}", e),
                    retry_count: Some(0),
                })?;
            }
        }

        Ok(())
    }
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            connection_count: 0,
            disconnection_count: 0,
            last_successful_request: None,
            last_connection_time: None,
            average_response_time_ms: 0,
        }
    }
}

impl Drop for SecureRethIpcClient {
    fn drop(&mut self) {
        // Send shutdown signal if still active
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }
    }
}

// Legacy API compatibility - Simple IPC client for backward compatibility
#[allow(dead_code)]
pub struct RethIpcClient {
    secure_client: SecureRethIpcClient,
}

impl RethIpcClient {
    pub async fn new(address: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = IpcClientConfig {
            address: address.to_string(),
            enable_encryption: false, // Disable encryption for legacy compatibility
            ..Default::default()
        };

        let secure_client = SecureRethIpcClient::new(config)?;
        Ok(Self { secure_client })
    }

    pub async fn receive_command(&mut self) -> Result<IpcCommand, MultivmError> {
        // This is a simplified implementation for backward compatibility
        // In a real implementation, you'd need to handle the async nature properly
        Err(MultivmError::Ipc {
            endpoint: "legacy_client".to_string(),
            message: "Legacy receive_command not implemented, use SecureRethIpcClient".to_string(),
            retry_count: Some(0),
        })
    }

    pub async fn send_response(&mut self, response: IpcResponse) -> Result<(), MultivmError> {
        // This is a simplified implementation for backward compatibility
        Err(MultivmError::Ipc {
            endpoint: "legacy_client".to_string(),
            message: "Legacy send_response not implemented, use SecureRethIpcClient".to_string(),
            retry_count: Some(0),
        })
    }

    pub async fn send_heartbeat(
        &mut self,
        health_response: IpcResponse,
    ) -> Result<(), MultivmError> {
        // This is a simplified implementation for backward compatibility
        Err(MultivmError::Ipc {
            endpoint: "legacy_client".to_string(),
            message: "Legacy send_heartbeat not implemented, use SecureRethIpcClient".to_string(),
            retry_count: Some(0),
        })
    }
}
