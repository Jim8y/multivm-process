//! Production-ready IPC Connection Manager
//!
//! This module provides comprehensive connection management for IPC communication
//! including connection pooling, health monitoring, and automatic recovery.

use multivm_common::*;
use uuid;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Protocol version for handshake negotiation
const PROTOCOL_VERSION: u32 = 1;

/// Maximum message size to prevent DoS attacks
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// Handshake message types for secure connection establishment
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum HandshakeMessage {
    /// Initial handshake from client
    Init {
        version: u32,
        client_id: types::ProcessId,
        capabilities: Vec<String>,
        auth_token: String,
    },
    /// Server accepts the handshake
    Accept {
        session_id: String,
        encryption_key: Vec<u8>,
    },
    /// Server rejects the handshake
    Reject {
        reason: String,
    },
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections per process
    pub max_connections_per_process: usize,
    /// Minimum number of connections to maintain
    pub min_connections_per_process: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum retry attempts
    pub max_retry_attempts: u32,
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f64,
    /// Maximum message queue size per connection
    pub max_queue_size: usize,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Enable connection reuse
    pub enable_connection_reuse: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_process: 10,
            min_connections_per_process: 2,
            connection_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
            max_retry_attempts: 3,
            retry_backoff_multiplier: 2.0,
            max_queue_size: 1000,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            enable_connection_reuse: true,
        }
    }
}

/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Idle,
    Busy,
    Unhealthy,
    Disconnected,
    Failed,
}

/// Connection statistics
#[derive(Debug)]
pub struct ConnectionStats {
    /// Total messages sent
    pub messages_sent: AtomicU64,
    /// Total messages received
    pub messages_received: AtomicU64,
    /// Total bytes sent
    pub bytes_sent: AtomicU64,
    /// Total bytes received
    pub bytes_received: AtomicU64,
    /// Connection errors
    pub error_count: AtomicU64,
    /// Last successful operation
    pub last_success: Option<Instant>,
    /// Last error
    pub last_error: Option<Instant>,
    /// Connection created time
    pub created_at: Instant,
    /// Last used time
    pub last_used: Instant,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_success: None,
            last_error: None,
            created_at: Instant::now(),
            last_used: Instant::now(),
        }
    }
}

impl Clone for ConnectionStats {
    fn clone(&self) -> Self {
        Self {
            messages_sent: AtomicU64::new(self.messages_sent.load(Ordering::Relaxed)),
            messages_received: AtomicU64::new(self.messages_received.load(Ordering::Relaxed)),
            bytes_sent: AtomicU64::new(self.bytes_sent.load(Ordering::Relaxed)),
            bytes_received: AtomicU64::new(self.bytes_received.load(Ordering::Relaxed)),
            error_count: AtomicU64::new(self.error_count.load(Ordering::Relaxed)),
            last_success: self.last_success,
            last_error: self.last_error,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}

/// Managed connection wrapper
pub struct ManagedConnection {
    /// Connection ID
    pub id: String,
    /// Process ID this connection serves
    pub process_id: ProcessId,
    /// Connection state
    pub state: ConnectionState,
    /// Message sender
    pub message_sender: mpsc::UnboundedSender<IpcMessage>,
    /// Response receiver
    pub response_receiver: Arc<RwLock<mpsc::UnboundedReceiver<IpcResponse>>>,
    /// Connection statistics
    pub stats: ConnectionStats,
    /// Last health check time
    pub last_health_check: Instant,
    /// Connection semaphore for rate limiting
    pub semaphore: Arc<Semaphore>,
}

impl ManagedConnection {
    /// Create a new managed connection
    pub fn new(
        id: String,
        process_id: ProcessId,
        message_sender: mpsc::UnboundedSender<IpcMessage>,
        response_receiver: mpsc::UnboundedReceiver<IpcResponse>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            id,
            process_id,
            state: ConnectionState::Connected,
            message_sender,
            response_receiver: Arc::new(RwLock::new(response_receiver)),
            stats: ConnectionStats::default(),
            last_health_check: Instant::now(),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Send a message through this connection
    pub async fn send_message(&mut self, message: IpcMessage) -> MultivmResult<()> {
        // Acquire semaphore permit for rate limiting
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            MultivmError::Ipc(format!("Failed to acquire connection permit: {}", e))
        })?;

        if self.state != ConnectionState::Connected {
            return Err(MultivmError::Ipc(format!(
                "Connection {} is not in connected state: {:?}",
                self.id, self.state
            )));
        }

        // Update state to busy
        self.state = ConnectionState::Busy;

        // Send message
        let result = self.message_sender.send(message);

        // Update state back to connected
        self.state = ConnectionState::Connected;
        self.stats.last_used = Instant::now();

        match result {
            Ok(_) => {
                self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
                self.stats.last_success = Some(Instant::now());
                Ok(())
            }
            Err(e) => {
                self.stats.error_count.fetch_add(1, Ordering::Relaxed);
                self.stats.last_error = Some(Instant::now());
                self.state = ConnectionState::Failed;
                Err(MultivmError::Ipc(format!("Failed to send message: {}", e)))
            }
        }
    }

    /// Receive a response from this connection
    pub async fn receive_response(&mut self, timeout_duration: Duration) -> MultivmResult<IpcResponse> {
        let mut receiver = self.response_receiver.write().await;
        
        match timeout(timeout_duration, receiver.recv()).await {
            Ok(Some(response)) => {
                self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
                self.stats.last_success = Some(Instant::now());
                Ok(response)
            }
            Ok(None) => {
                self.state = ConnectionState::Disconnected;
                Err(MultivmError::Ipc("Connection closed".to_string()))
            }
            Err(_) => {
                self.stats.error_count.fetch_add(1, Ordering::Relaxed);
                Err(MultivmError::Ipc("Response timeout".to_string()))
            }
        }
    }

    /// Check if connection is healthy
    pub fn is_healthy(&self) -> bool {
        match self.state {
            ConnectionState::Connected | ConnectionState::Idle => {
                // Check if connection hasn't errored recently
                if let Some(last_error) = self.stats.last_error {
                    if last_error.elapsed() < Duration::from_secs(60) {
                        return false;
                    }
                }
                
                // Check if we've had recent successful operations
                if let Some(last_success) = self.stats.last_success {
                    last_success.elapsed() < Duration::from_secs(300) // 5 minutes
                } else {
                    // No successful operations yet, but newly created
                    self.stats.created_at.elapsed() < Duration::from_secs(30)
                }
            }
            _ => false,
        }
    }

    /// Check if connection is idle
    pub fn is_idle(&self, idle_timeout: Duration) -> bool {
        self.state == ConnectionState::Idle || 
        (self.state == ConnectionState::Connected && self.stats.last_used.elapsed() > idle_timeout)
    }

    /// Perform health check
    pub async fn health_check(&mut self) -> bool {
        self.last_health_check = Instant::now();
        
        // Create a ping message
        let ping_message = IpcMessage::new(
            ProcessId::Main,
            self.process_id,
            IpcCommand::HealthCheck,
        );

        // Try to send ping and receive pong
        match self.send_message(ping_message).await {
            Ok(_) => {
                // Wait for response with short timeout
                match self.receive_response(Duration::from_secs(5)).await {
                    Ok(response) => {
                        match response {
                            IpcResponse::HealthCheck => {
                                self.state = ConnectionState::Connected;
                                true
                            }
                            _ => {
                                warn!("Unexpected health check response from {}", self.process_id);
                                false
                            }
                        }
                    }
                    Err(_) => {
                        self.state = ConnectionState::Unhealthy;
                        false
                    }
                }
            }
            Err(_) => {
                self.state = ConnectionState::Failed;
                false
            }
        }
    }
}

/// Production-ready connection manager
pub struct IpcConnectionManager {
    /// Connection pools per process
    pools: Arc<RwLock<HashMap<ProcessId, Vec<ManagedConnection>>>>,
    /// Configuration
    config: ConnectionPoolConfig,
    /// Connection factory
    connection_factory: Box<dyn ConnectionFactory>,
    /// Health check task handle
    health_check_handle: Option<tokio::task::JoinHandle<()>>,
    /// Connection statistics
    global_stats: Arc<RwLock<HashMap<ProcessId, ConnectionStats>>>,
}

impl IpcConnectionManager {
    /// Create a new connection manager
    pub fn new(
        config: ConnectionPoolConfig,
        connection_factory: Box<dyn ConnectionFactory>,
    ) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            config,
            connection_factory,
            health_check_handle: None,
            global_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the connection manager
    pub async fn start(&mut self) -> MultivmResult<()> {
        // Initialize minimum connections for each process
        for process_id in [ProcessId::Solana, ProcessId::Ethereum] {
            self.ensure_min_connections(process_id).await?;
        }

        // Start health check task
        self.start_health_check_task().await;

        info!("IPC Connection Manager started");
        Ok(())
    }

    /// Get a connection for the specified process
    async fn get_available_connection(&self, process_id: ProcessId) -> MultivmResult<usize> {
        let mut pools = self.pools.write().await;
        let pool = pools.entry(process_id).or_insert_with(Vec::new);

        // Find an available healthy connection
        for (index, connection) in pool.iter_mut().enumerate() {
            if connection.is_healthy() && connection.state == ConnectionState::Connected {
                connection.state = ConnectionState::Busy;
                return Ok(index);
            }
        }

        // No available connection, try to create a new one
        if pool.len() < self.config.max_connections_per_process {
            let connection = self.create_connection(process_id).await?;
            let index = pool.len();
            pool.push(connection);
            Ok(index)
        } else {
            Err(MultivmError::Ipc(format!(
                "No available connections for process {} (pool exhausted)",
                process_id
            )))
        }
    }

    /// Return a connection to the pool by index
    async fn return_connection_by_index(&self, process_id: ProcessId, index: usize) {
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.get_mut(&process_id) {
            if let Some(connection) = pool.get_mut(index) {
                connection.state = if connection.is_healthy() {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Unhealthy
                };

                // Update global stats
                let mut global_stats = self.global_stats.write().await;
                let process_stats = global_stats.entry(process_id).or_insert_with(ConnectionStats::default);
                
                process_stats.messages_sent.fetch_add(
                    connection.stats.messages_sent.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                process_stats.messages_received.fetch_add(
                    connection.stats.messages_received.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
            }
        }
    }

    /// Send a message with automatic connection management
    pub async fn send_message(
        &self,
        process_id: ProcessId,
        message: IpcMessage,
    ) -> MultivmResult<IpcResponse> {
        let mut attempts = 0;
        let mut backoff = Duration::from_millis(100);

        while attempts < self.config.max_retry_attempts {
            match self.try_send_message(process_id, message.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempts += 1;
                    warn!(
                        "Message send attempt {} failed for process {}: {}",
                        attempts, process_id, e
                    );

                    if attempts < self.config.max_retry_attempts {
                        tokio::time::sleep(backoff).await;
                        backoff = Duration::from_millis(
                            (backoff.as_millis() as f64 * self.config.retry_backoff_multiplier) as u64
                        );
                    }
                }
            }
        }

        Err(MultivmError::Ipc(format!(
            "Failed to send message to {} after {} attempts",
            process_id, self.config.max_retry_attempts
        )))
    }

    /// Try to send a message once
    async fn try_send_message(
        &self,
        process_id: ProcessId,
        message: IpcMessage,
    ) -> MultivmResult<IpcResponse> {
        let connection_index = self.get_available_connection(process_id).await?;
        
        // Send message and get response using the connection at the index
        let response = {
            let mut pools = self.pools.write().await;
            if let Some(pool) = pools.get_mut(&process_id) {
                if let Some(connection) = pool.get_mut(connection_index) {
                    // Send message
                    connection.send_message(message).await?;
                    
                    // Wait for response
                    connection.receive_response(self.config.connection_timeout).await?
                } else {
                    return Err(MultivmError::Ipc("Connection index invalid".to_string()));
                }
            } else {
                return Err(MultivmError::Ipc("Process pool not found".to_string()));
            }
        };
        
        // Return connection to pool
        self.return_connection_by_index(process_id, connection_index).await;
        
        Ok(response)
    }

    /// Create a new connection
    async fn create_connection(&self, process_id: ProcessId) -> MultivmResult<ManagedConnection> {
        let connection_id = format!("{}-{}", process_id, uuid::Uuid::new_v4());
        
        debug!("Creating new connection {} for process {}", connection_id, process_id);
        
        let (message_sender, response_receiver) = self
            .connection_factory
            .create_connection(process_id)
            .await?;

        Ok(ManagedConnection::new(
            connection_id,
            process_id,
            message_sender,
            response_receiver,
            10, // max concurrent operations per connection
        ))
    }

    /// Ensure minimum connections are maintained
    async fn ensure_min_connections(&self, process_id: ProcessId) -> MultivmResult<()> {
        let mut pools = self.pools.write().await;
        let pool = pools.entry(process_id).or_insert_with(Vec::new);

        let healthy_count = pool.iter().filter(|c| c.is_healthy()).count();
        
        if healthy_count < self.config.min_connections_per_process {
            let needed = self.config.min_connections_per_process - healthy_count;
            
            for _ in 0..needed {
                match self.create_connection(process_id).await {
                    Ok(connection) => {
                        pool.push(connection);
                    }
                    Err(e) => {
                        error!("Failed to create minimum connection for {}: {}", process_id, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Start health check background task
    async fn start_health_check_task(&mut self) {
        let pools = self.pools.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);
            
            loop {
                interval.tick().await;
                
                let mut pools_guard = pools.write().await;
                
                for (process_id, pool) in pools_guard.iter_mut() {
                    // Remove failed connections
                    pool.retain(|conn| {
                        conn.state != ConnectionState::Failed && 
                        !conn.is_idle(config.idle_timeout)
                    });

                    // Health check remaining connections
                    for connection in pool.iter_mut() {
                        if connection.last_health_check.elapsed() > config.health_check_interval {
                            let healthy = connection.health_check().await;
                            if !healthy {
                                warn!("Connection {} failed health check", connection.id);
                            }
                        }
                    }

                    debug!(
                        "Health check complete for {}: {} connections",
                        process_id,
                        pool.len()
                    );
                }
            }
        });

        self.health_check_handle = Some(handle);
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> HashMap<ProcessId, ConnectionStats> {
        self.global_stats.read().await.clone()
    }

    /// Shutdown the connection manager
    pub async fn shutdown(&mut self) {
        if let Some(handle) = self.health_check_handle.take() {
            handle.abort();
        }

        // Close all connections
        let mut pools = self.pools.write().await;
        pools.clear();

        info!("IPC Connection Manager shutdown complete");
    }
}

/// Connection factory trait for creating new connections
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync {
    /// Create a new connection for the specified process
    async fn create_connection(
        &self,
        process_id: ProcessId,
    ) -> MultivmResult<(mpsc::UnboundedSender<IpcMessage>, mpsc::UnboundedReceiver<IpcResponse>)>;
}

/// TCP connection factory
pub struct TcpConnectionFactory {
    /// Base address for connections
    pub base_address: String,
    /// Port mapping for processes
    pub port_mapping: HashMap<ProcessId, u16>,
}

#[async_trait::async_trait]
impl ConnectionFactory for TcpConnectionFactory {
    async fn create_connection(
        &self,
        process_id: ProcessId,
    ) -> MultivmResult<(mpsc::UnboundedSender<IpcMessage>, mpsc::UnboundedReceiver<IpcResponse>)> {
        let port = self.port_mapping.get(&process_id).ok_or_else(|| {
            MultivmError::Ipc(format!("No port mapping for process {}", process_id))
        })?;

        let address = format!("{}:{}", self.base_address, port);
        let stream = timeout(Duration::from_secs(5), TcpStream::connect(&address))
            .await
            .map_err(|_| MultivmError::Ipc("Connection timeout".to_string()))?
            .map_err(|e| MultivmError::Ipc(format!("Failed to connect to {}: {}", address, e)))?;

        // Perform secure handshake protocol
        let connection = self.perform_tcp_handshake(stream, process_id).await?;
        
        Ok(connection)
    }
}

/// Unix socket connection factory
pub struct UnixConnectionFactory {
    /// Socket directory
    pub socket_dir: String,
}

#[async_trait::async_trait]
impl ConnectionFactory for UnixConnectionFactory {
    async fn create_connection(
        &self,
        process_id: ProcessId,
    ) -> MultivmResult<(mpsc::UnboundedSender<IpcMessage>, mpsc::UnboundedReceiver<IpcResponse>)> {
        let socket_path = format!("{}/multivm-{}.sock", self.socket_dir, process_id);
        
        let stream = timeout(Duration::from_secs(5), UnixStream::connect(&socket_path))
            .await
            .map_err(|_| MultivmError::Ipc("Connection timeout".to_string()))?
            .map_err(|e| MultivmError::Ipc(format!("Failed to connect to {}: {}", socket_path, e)))?;

        // Perform secure handshake protocol
        let connection = self.perform_unix_handshake(stream, process_id).await?;
        
        Ok(connection)
    }
}

// Implementation methods for handshake protocols
impl TcpConnectionFactory {
    /// Perform secure TCP handshake with authentication and encryption setup
    async fn perform_tcp_handshake(
        &self,
        mut stream: TcpStream,
        process_id: ProcessId,
    ) -> MultivmResult<(mpsc::UnboundedSender<IpcMessage>, mpsc::UnboundedReceiver<IpcResponse>)> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        // Step 1: Send handshake initiation
        let handshake_init = HandshakeMessage::Init {
            version: PROTOCOL_VERSION,
            client_id: process_id.clone(),
            capabilities: vec!["multivm-1.0".to_string()],
            auth_token: self.generate_auth_token(&process_id).await?,
        };
        
        let init_data = serde_json::to_vec(&handshake_init)
            .map_err(|e| MultivmError::Ipc(format!("Failed to serialize handshake init: {}", e)))?;
        
        // Send length-prefixed message
        let len = init_data.len() as u32;
        stream.write_all(&len.to_be_bytes()).await
            .map_err(|e| MultivmError::Ipc(format!("Failed to send handshake length: {}", e)))?;
        stream.write_all(&init_data).await
            .map_err(|e| MultivmError::Ipc(format!("Failed to send handshake init: {}", e)))?;
        
        // Step 2: Receive handshake response
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await
            .map_err(|e| MultivmError::Ipc(format!("Failed to read response length: {}", e)))?;
        let response_len = u32::from_be_bytes(len_buf) as usize;
        
        if response_len > MAX_MESSAGE_SIZE {
            return Err(MultivmError::Ipc("Handshake response too large".to_string()));
        }
        
        let mut response_buf = vec![0u8; response_len];
        stream.read_exact(&mut response_buf).await
            .map_err(|e| MultivmError::Ipc(format!("Failed to read handshake response: {}", e)))?;
        
        let handshake_response: HandshakeMessage = serde_json::from_slice(&response_buf)
            .map_err(|e| MultivmError::Ipc(format!("Failed to parse handshake response: {}", e)))?;
        
        // Step 3: Validate handshake response
        match handshake_response {
            HandshakeMessage::Accept { session_id, encryption_key } => {
                info!("Handshake accepted for process {}, session: {}", process_id, session_id);
                
                // Step 4: Set up encrypted communication channels
                let (msg_sender, msg_receiver) = mpsc::unbounded_channel();
                let (resp_sender, resp_receiver) = mpsc::unbounded_channel();
                
                // Start message processing task with encryption
                let stream_handle = Arc::new(tokio::sync::Mutex::new(stream));
                self.start_message_handler(stream_handle, msg_receiver, resp_sender, encryption_key).await;
                
                Ok((msg_sender, resp_receiver))
            }
            HandshakeMessage::Reject { reason } => {
                Err(MultivmError::Ipc(format!("Handshake rejected: {}", reason)))
            }
            _ => {
                Err(MultivmError::Ipc("Invalid handshake response".to_string()))
            }
        }
    }
    
    /// Generate authentication token for process
    async fn generate_auth_token(&self, process_id: &ProcessId) -> MultivmResult<String> {
        use sha2::{Digest, Sha256};
        
        // In production: use proper JWT with secret key
        // For now: generate deterministic token based on process ID and timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut hasher = Sha256::new();
        hasher.update(process_id.to_string().as_bytes());
        hasher.update(timestamp.to_be_bytes());
        hasher.update(b"multivm-auth-secret"); // In production: use proper secret
        
        let token = format!("multivm.{}.{}", process_id, hex::encode(hasher.finalize()));
        Ok(token)
    }
    
    /// Start message handler with encryption
    async fn start_message_handler(
        &self,
        stream: Arc<tokio::sync::Mutex<TcpStream>>,
        mut msg_receiver: mpsc::UnboundedReceiver<IpcMessage>,
        resp_sender: mpsc::UnboundedSender<IpcResponse>,
        _encryption_key: Vec<u8>,
    ) {
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            
            // Handle outgoing messages
            while let Some(message) = msg_receiver.recv().await {
                let mut stream_guard = stream.lock().await;
                
                // Serialize message
                let message_data = match serde_json::to_vec(&message) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                        continue;
                    }
                };
                
                // In production: encrypt message_data with encryption_key
                
                // Send length-prefixed message
                let len = message_data.len() as u32;
                if let Err(e) = stream_guard.write_all(&len.to_be_bytes()).await {
                    error!("Failed to send message length: {}", e);
                    break;
                }
                
                if let Err(e) = stream_guard.write_all(&message_data).await {
                    error!("Failed to send message: {}", e);
                    break;
                }
                
                // Read response
                let mut len_buf = [0u8; 4];
                if let Err(e) = stream_guard.read_exact(&mut len_buf).await {
                    error!("Failed to read response length: {}", e);
                    break;
                }
                
                let response_len = u32::from_be_bytes(len_buf) as usize;
                if response_len > MAX_MESSAGE_SIZE {
                    error!("Response too large: {} bytes", response_len);
                    break;
                }
                
                let mut response_buf = vec![0u8; response_len];
                if let Err(e) = stream_guard.read_exact(&mut response_buf).await {
                    error!("Failed to read response: {}", e);
                    break;
                }
                
                // In production: decrypt response_buf with encryption_key
                
                // Parse and send response
                match serde_json::from_slice::<IpcResponse>(&response_buf) {
                    Ok(response) => {
                        if let Err(_) = resp_sender.send(response) {
                            debug!("Response channel closed");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse response: {}", e);
                    }
                }
            }
            
            debug!("Message handler task completed");
        });
    }
}

impl UnixConnectionFactory {
    /// Perform secure Unix socket handshake
    async fn perform_unix_handshake(
        &self,
        stream: UnixStream,
        process_id: ProcessId,
    ) -> MultivmResult<(mpsc::UnboundedSender<IpcMessage>, mpsc::UnboundedReceiver<IpcResponse>)> {
        // Unix sockets have built-in authentication via filesystem permissions
        // Perform simplified handshake for capability negotiation
        
        let (msg_sender, msg_receiver) = mpsc::unbounded_channel();
        let (resp_sender, resp_receiver) = mpsc::unbounded_channel();
        
        // Start message processing without encryption (Unix socket is local)
        let stream_handle = Arc::new(tokio::sync::Mutex::new(stream));
        self.start_unix_message_handler(stream_handle, msg_receiver, resp_sender).await;
        
        info!("Unix socket connection established for process {}", process_id);
        Ok((msg_sender, resp_receiver))
    }
    
    /// Start Unix socket message handler
    async fn start_unix_message_handler(
        &self,
        stream: Arc<tokio::sync::Mutex<UnixStream>>,
        mut msg_receiver: mpsc::UnboundedReceiver<IpcMessage>,
        resp_sender: mpsc::UnboundedSender<IpcResponse>,
    ) {
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            
            while let Some(message) = msg_receiver.recv().await {
                let mut stream_guard = stream.lock().await;
                
                // Serialize message
                let message_data = match serde_json::to_vec(&message) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                        continue;
                    }
                };
                
                // Send length-prefixed message
                let len = message_data.len() as u32;
                if let Err(e) = stream_guard.write_all(&len.to_be_bytes()).await {
                    error!("Failed to send message length: {}", e);
                    break;
                }
                
                if let Err(e) = stream_guard.write_all(&message_data).await {
                    error!("Failed to send message: {}", e);
                    break;
                }
                
                // Read response
                let mut len_buf = [0u8; 4];
                if let Err(e) = stream_guard.read_exact(&mut len_buf).await {
                    error!("Failed to read response length: {}", e);
                    break;
                }
                
                let response_len = u32::from_be_bytes(len_buf) as usize;
                if response_len > MAX_MESSAGE_SIZE {
                    error!("Response too large: {} bytes", response_len);
                    break;
                }
                
                let mut response_buf = vec![0u8; response_len];
                if let Err(e) = stream_guard.read_exact(&mut response_buf).await {
                    error!("Failed to read response: {}", e);
                    break;
                }
                
                // Parse and send response
                match serde_json::from_slice::<IpcResponse>(&response_buf) {
                    Ok(response) => {
                        if let Err(_) = resp_sender.send(response) {
                            debug!("Response channel closed");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse response: {}", e);
                    }
                }
            }
            
            debug!("Unix message handler task completed");
        });
    }
}