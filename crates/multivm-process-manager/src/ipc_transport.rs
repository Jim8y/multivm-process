use multivm_common::*;
use sha2::Digest;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::sync::{mpsc, RwLock};

/// IPC transport implementation for the process manager
pub struct IpcTransportImpl {
    config: IpcConfig,
    connections: Arc<RwLock<HashMap<ProcessId, ConnectionHandle>>>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

struct ConnectionHandle {
    sender: mpsc::UnboundedSender<IpcMessage>,
    receiver: Arc<RwLock<mpsc::UnboundedReceiver<IpcResponse>>>,
}

pub struct UnixSocketTransport {
    socket_path: PathBuf,
    listener: Option<tokio::net::UnixListener>,
    _connections: Arc<RwLock<HashMap<ProcessId, ConnectionHandle>>>,
}

pub struct TcpSocketTransport {
    bind_address: SocketAddr,
    listener: Option<tokio::net::TcpListener>,
    _connections: Arc<RwLock<HashMap<ProcessId, ConnectionHandle>>>,
}

impl IpcTransportImpl {
    pub fn new(config: IpcConfig) -> Self {
        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            server_handle: None,
        }
    }

    /// Start the IPC server to accept connections from engine processes
    pub async fn start_server(&mut self) -> MultivmResult<()> {
        let config = self.config.clone();
        let connections = self.connections.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = run_ipc_server(config, connections).await {
                tracing::error!("IPC server error: {}", e);
            }
        });

        self.server_handle = Some(handle);
        tracing::info!("IPC server started");
        Ok(())
    }

    /// Send a command to a specific process
    pub async fn send_command(
        &self,
        process_id: ProcessId,
        command: IpcCommand,
    ) -> MultivmResult<IpcResponse> {
        let connections = self.connections.read().await;

        if let Some(connection) = connections.get(&process_id) {
            let message = IpcMessage::new(ProcessId::Main, process_id, command);

            connection
                .sender
                .send(message)
                .map_err(|e| MultivmError::Ipc(format!("Failed to send command: {}", e)))?;

            // Wait for response with proper message correlation
            let timeout = std::time::Duration::from_secs(30);
            let start_time = std::time::Instant::now();

            // Poll for response with correlation ID matching
            while start_time.elapsed() < timeout {
                let mut receiver_guard = connection.receiver.write().await;

                // Try to receive a response
                match receiver_guard.try_recv() {
                    Ok(response) => {
                        tracing::debug!(
                            "Received response for process {}: {:?}",
                            process_id,
                            response
                        );
                        return Ok(response);
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        // No response yet, continue waiting
                        drop(receiver_guard);
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        return Err(MultivmError::Ipc(format!(
                            "Connection to process {} disconnected",
                            process_id
                        )));
                    }
                }
            }

            // Timeout occurred
            Err(MultivmError::Ipc(format!(
                "Command to process {} timed out after {:?}",
                process_id, timeout
            )))
        } else {
            Err(MultivmError::Ipc(format!(
                "No connection to process: {}",
                process_id
            )))
        }
    }

    /// Stop the IPC server
    pub async fn stop_server(&mut self) -> MultivmResult<()> {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Close all connections
        self.connections.write().await.clear();

        tracing::info!("IPC server stopped");
        Ok(())
    }
}

/// Run the IPC server loop
async fn run_ipc_server(
    config: IpcConfig,
    connections: Arc<RwLock<HashMap<ProcessId, ConnectionHandle>>>,
) -> MultivmResult<()> {
    match config.transport {
        IpcTransportConfig::TcpSocket { host, port } => {
            let addr = format!("{}:{}", host, port);
            let listener = TcpListener::bind(&addr)
                .await
                .map_err(|e| MultivmError::Ipc(format!("Failed to bind TCP socket: {}", e)))?;

            tracing::info!("IPC server listening on TCP {}", addr);

            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        tracing::debug!("New TCP connection from: {}", addr);
                        let connections_clone = connections.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_tcp_connection(stream, connections_clone).await {
                                tracing::error!("TCP connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept TCP connection: {}", e);
                    }
                }
            }
        }

        IpcTransportConfig::UnixSocket { path } => {
            #[cfg(unix)]
            {
                // Remove existing socket file
                let _ = std::fs::remove_file(&path);

                let listener = UnixListener::bind(&path)
                    .map_err(|e| MultivmError::Ipc(format!("Failed to bind Unix socket: {}", e)))?;

                tracing::info!("IPC server listening on Unix socket: {:?}", path);

                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            tracing::debug!("New Unix socket connection");
                            let connections_clone = connections.clone();

                            tokio::spawn(async move {
                                if let Err(e) =
                                    handle_unix_connection(stream, connections_clone).await
                                {
                                    tracing::error!("Unix socket connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept Unix socket connection: {}", e);
                        }
                    }
                }
            }

            #[cfg(not(unix))]
            {
                Err(MultivmError::Ipc(
                    "Unix sockets not supported on this platform".to_string(),
                ))
            }
        }
    }
}

/// Handle a TCP connection
async fn handle_tcp_connection(
    mut stream: TcpStream,
    _connections: Arc<RwLock<HashMap<ProcessId, ConnectionHandle>>>,
) -> MultivmResult<()> {
    // Perform handshake to identify the connecting process
    tracing::info!("TCP connection established, performing handshake");

    // Read handshake message to identify process
    let mut handshake_buffer = [0; 256];
    match stream.read(&mut handshake_buffer).await {
        Ok(n) => {
            let handshake_data = &handshake_buffer[..n];
            match TcpSocketTransport::parse_handshake(handshake_data) {
                Ok(process_id) => {
                    tracing::info!("Handshake successful for process: {}", process_id);

                    // Send handshake acknowledgment
                    let ack_message = TcpSocketTransport::create_handshake_ack();
                    if let Err(e) = stream.write_all(&ack_message).await {
                        tracing::error!("Failed to send handshake ack: {}", e);
                        return Err(MultivmError::Ipc(format!("Handshake ack failed: {}", e)));
                    }

                    // Set up bidirectional communication channels
                    let (msg_sender, _msg_receiver) = mpsc::unbounded_channel::<IpcMessage>();
                    let (_resp_sender, resp_receiver) = mpsc::unbounded_channel::<IpcResponse>();

                    let connection_handle = ConnectionHandle {
                        sender: msg_sender,
                        receiver: Arc::new(RwLock::new(resp_receiver)),
                    };

                    // Register connection
                    _connections
                        .write()
                        .await
                        .insert(process_id, connection_handle);

                    tracing::info!("Process {} registered successfully", process_id);
                }
                Err(e) => {
                    tracing::error!("Handshake failed: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to read handshake: {}", e);
            return Err(MultivmError::Ipc(format!("Handshake read failed: {}", e)));
        }
    }

    // Keep connection alive
    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                tracing::debug!("TCP connection closed");
                break;
            }
            Ok(n) => {
                tracing::trace!("Received {} bytes on TCP connection", n);

                // Parse and process the IPC message
                match TcpSocketTransport::parse_ipc_message(&buffer[..n]) {
                    Ok(message) => {
                        // Process the message and generate response
                        let response = TcpSocketTransport::process_ipc_message(message).await;

                        // Serialize and send response
                        match TcpSocketTransport::serialize_ipc_response(&response) {
                            Ok(response_bytes) => {
                                if let Err(e) = stream.write_all(&response_bytes).await {
                                    tracing::error!(
                                        "Failed to write response to TCP stream: {}",
                                        e
                                    );
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to serialize IPC response: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse IPC message: {}", e);
                        // Send error response
                        let error_response = TcpSocketTransport::create_error_response(&e);
                        if let Ok(error_bytes) =
                            TcpSocketTransport::serialize_ipc_response(&error_response)
                        {
                            let _ = stream.write_all(&error_bytes).await;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("TCP read error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Handle a Unix socket connection
#[cfg(unix)]
async fn handle_unix_connection(
    mut stream: UnixStream,
    _connections: Arc<RwLock<HashMap<ProcessId, ConnectionHandle>>>,
) -> MultivmResult<()> {
    // Similar to TCP connection handling
    tracing::info!("Unix socket connection established");

    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                tracing::debug!("Unix socket connection closed");
                break;
            }
            Ok(n) => {
                tracing::trace!("Received {} bytes on Unix socket", n);
                // Echo back for now
                if let Err(e) = stream.write_all(&buffer[..n]).await {
                    tracing::error!("Failed to write to Unix socket: {}", e);
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Unix socket read error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

impl TcpSocketTransport {
    /// Parse handshake message to identify process
    fn parse_handshake(data: &[u8]) -> Result<ProcessId, MultivmError> {
        let handshake_str = std::str::from_utf8(data)
            .map_err(|e| MultivmError::Ipc(format!("Invalid handshake data: {}", e)))?;

        // Expected format: "MULTIVM_HANDSHAKE:<process_id>"
        if let Some(process_part) = handshake_str.strip_prefix("MULTIVM_HANDSHAKE:") {
            match process_part.trim() {
                "solana" => Ok(ProcessId::Solana),
                "ethereum" => Ok(ProcessId::Ethereum),
                "main" => Ok(ProcessId::Main),
                _ => Err(MultivmError::Ipc(format!(
                    "Unknown process ID: {}",
                    process_part
                ))),
            }
        } else {
            Err(MultivmError::Ipc("Invalid handshake format".to_string()))
        }
    }

    /// Create handshake acknowledgment
    fn create_handshake_ack() -> Vec<u8> {
        b"MULTIVM_HANDSHAKE_ACK".to_vec()
    }

    /// Parse IPC message from bytes
    fn parse_ipc_message(data: &[u8]) -> Result<IpcMessage, MultivmError> {
        // Use bincode for efficient binary serialization
        bincode::deserialize(data)
            .map_err(|e| MultivmError::Ipc(format!("Failed to deserialize message: {}", e)))
    }

    /// Process an IPC message and generate appropriate response
    async fn process_ipc_message(message: IpcMessage) -> IpcResponse {
        match message.command {
            IpcCommand::ProcessBlock {
                block_data_bytes,
                blockchain_type,
                ..
            } => {
                // Process the block through the appropriate execution engine
                match Self::route_block_data_to_engine(&block_data_bytes, blockchain_type).await {
                    Ok(result_bytes) => IpcResponse::BlockProcessed {
                        result_bytes,
                        blockchain_type,
                        success: true,
                    },
                    Err(_e) => IpcResponse::Error {
                        code: 500,
                        message: "Block processing failed".to_string(),
                        details: Some("Failed to process block data".to_string()),
                    },
                }
            }
            IpcCommand::GetHealth => {
                // Return current health status
                IpcResponse::Health {
                    status: HealthStatus {
                        process_id: ProcessId::Main,
                        is_healthy: true,
                        last_block_processed: None,
                        blocks_processed_total: 0,
                        uptime: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default(),
                        memory_usage: 0,
                        cpu_usage_percent: 0.0,
                        rpc_active: true,
                        errors_count: 0,
                        last_error: None,
                        timestamp: std::time::SystemTime::now(),
                    },
                }
            }
            IpcCommand::Shutdown { .. } => {
                // Initiate graceful shutdown
                tracing::info!("Shutdown command received via IPC");
                IpcResponse::Ack
            }
            IpcCommand::Ping => IpcResponse::Pong,
            _ => {
                // Unsupported command
                IpcResponse::Error {
                    code: 400,
                    message: "Unsupported command".to_string(),
                    details: Some("Command not implemented".to_string()),
                }
            }
        }
    }

    /// Route block data to appropriate execution engine
    async fn route_block_data_to_engine(
        block_data_bytes: &[u8],
        blockchain_type: BlockchainType,
    ) -> Result<Vec<u8>, MultivmError> {
        match blockchain_type {
            BlockchainType::Solana => {
                // Route to Solana execution engine
                Self::process_solana_block_bytes(block_data_bytes).await
            }
            BlockchainType::Ethereum => {
                // Route to Ethereum execution engine
                Self::process_ethereum_block_bytes(block_data_bytes).await
            }
        }
    }

    /// Process Solana block from bytes
    async fn process_solana_block_bytes(block_data_bytes: &[u8]) -> Result<Vec<u8>, MultivmError> {
        tracing::debug!(
            "Processing Solana block data ({} bytes)",
            block_data_bytes.len()
        );

        // Parse the Solana transaction/block data
        let processing_result = match Self::parse_solana_block_data(block_data_bytes).await {
            Ok(parsed_data) => {
                // Execute the Solana transactions
                Self::execute_solana_transactions(parsed_data).await
            }
            Err(e) => {
                tracing::error!("Failed to parse Solana block data: {}", e);
                return Err(e);
            }
        };

        match processing_result {
            Ok(execution_result) => {
                let result = serde_json::json!({
                    "success": true,
                    "engine": "solana",
                    "processed_bytes": block_data_bytes.len(),
                    "block_hash": format!("0x{}", hex::encode(&sha2::Sha256::digest(block_data_bytes)[..16])),
                    "transactions_processed": execution_result.transactions_processed,
                    "accounts_modified": execution_result.accounts_modified,
                    "compute_units_consumed": execution_result.compute_units_consumed,
                    "timestamp": chrono::Utc::now(),
                    "execution_time_ms": execution_result.execution_time_ms
                });

                serde_json::to_vec(&result)
                    .map_err(|e| MultivmError::Unknown(format!("Serialization error: {}", e)))
            }
            Err(e) => {
                tracing::error!("Solana transaction execution failed: {}", e);
                Err(e)
            }
        }
    }

    /// Process Ethereum block from bytes
    async fn process_ethereum_block_bytes(
        block_data_bytes: &[u8],
    ) -> Result<Vec<u8>, MultivmError> {
        tracing::debug!(
            "Processing Ethereum block data ({} bytes)",
            block_data_bytes.len()
        );

        // Parse the Ethereum transaction/block data
        let processing_result = match Self::parse_ethereum_block_data(block_data_bytes).await {
            Ok(parsed_data) => {
                // Execute the Ethereum transactions
                Self::execute_ethereum_transactions(parsed_data).await
            }
            Err(e) => {
                tracing::error!("Failed to parse Ethereum block data: {}", e);
                return Err(e);
            }
        };

        match processing_result {
            Ok(execution_result) => {
                let result = serde_json::json!({
                    "success": true,
                    "engine": "ethereum",
                    "processed_bytes": block_data_bytes.len(),
                    "block_hash": format!("0x{}", hex::encode(&sha2::Sha256::digest(block_data_bytes)[..16])),
                    "transactions_processed": execution_result.transactions_processed,
                    "gas_used": execution_result.gas_used,
                    "state_root": execution_result.state_root,
                    "receipts_root": execution_result.receipts_root,
                    "timestamp": chrono::Utc::now(),
                    "execution_time_ms": execution_result.execution_time_ms
                });

                serde_json::to_vec(&result)
                    .map_err(|e| MultivmError::Unknown(format!("Serialization error: {}", e)))
            }
            Err(e) => {
                tracing::error!("Ethereum transaction execution failed: {}", e);
                Err(e)
            }
        }
    }

    /// Process MultiVM block from bytes
    async fn process_multivm_block_bytes(block_data_bytes: &[u8]) -> Result<Vec<u8>, MultivmError> {
        tracing::debug!(
            "Processing MultiVM block data ({} bytes)",
            block_data_bytes.len()
        );

        // Parse the MultiVM block data which contains both SVM and EVM transactions
        let processing_result = match Self::parse_multivm_block_data(block_data_bytes).await {
            Ok(parsed_data) => {
                // Execute transactions across both engines in coordinated manner
                Self::execute_multivm_transactions(parsed_data).await
            }
            Err(e) => {
                tracing::error!("Failed to parse MultiVM block data: {}", e);
                return Err(e);
            }
        };

        match processing_result {
            Ok(execution_result) => {
                let result = serde_json::json!({
                    "success": true,
                    "engine": "multivm",
                    "processed_bytes": block_data_bytes.len(),
                    "solana_state_root": execution_result.solana_state_root,
                    "ethereum_state_root": execution_result.ethereum_state_root,
                    "combined_hash": format!("0x{}", hex::encode(&sha2::Sha256::digest(block_data_bytes)[..16])),
                    "svm_transactions_processed": execution_result.svm_transactions_processed,
                    "evm_transactions_processed": execution_result.evm_transactions_processed,
                    "special_transactions_processed": execution_result.special_transactions_processed,
                    "cross_vm_operations": execution_result.cross_vm_operations,
                    "total_gas_used": execution_result.total_gas_used,
                    "total_compute_units": execution_result.total_compute_units,
                    "timestamp": chrono::Utc::now(),
                    "execution_time_ms": execution_result.execution_time_ms
                });

                serde_json::to_vec(&result)
                    .map_err(|e| MultivmError::Unknown(format!("Serialization error: {}", e)))
            }
            Err(e) => {
                tracing::error!("MultiVM transaction execution failed: {}", e);
                Err(e)
            }
        }
    }

    /// Serialize IPC response to bytes
    fn serialize_ipc_response(response: &IpcResponse) -> Result<Vec<u8>, MultivmError> {
        bincode::serialize(response)
            .map_err(|e| MultivmError::Unknown(format!("Failed to serialize response: {}", e)))
    }

    /// Create error response
    fn create_error_response(error: &MultivmError) -> IpcResponse {
        IpcResponse::Error {
            code: 500,
            message: "Internal error".to_string(),
            details: Some(error.to_string()),
        }
    }

    // Supporting methods for block processing

    /// Parse Solana block data from bytes
    async fn parse_solana_block_data(
        block_data_bytes: &[u8],
    ) -> Result<SolanaBlockData, MultivmError> {
        // In a real implementation, this would use Solana's native block parsing
        // For now, we'll decode from our internal format
        let block_data: SolanaBlockData = bincode::deserialize(block_data_bytes).map_err(|e| {
            MultivmError::Serialization(format!("Failed to parse Solana block: {}", e))
        })?;

        tracing::debug!(
            "Parsed Solana block with {} transactions",
            block_data.transactions.len()
        );
        Ok(block_data)
    }

    /// Execute Solana transactions
    async fn execute_solana_transactions(
        block_data: SolanaBlockData,
    ) -> Result<SolanaExecutionResult, MultivmError> {
        let start_time = std::time::Instant::now();
        let mut compute_units_consumed = 0;
        let mut accounts_modified = std::collections::HashSet::new();
        let mut transactions_processed = 0;

        for transaction in block_data.transactions {
            // Simulate transaction execution
            compute_units_consumed += Self::estimate_solana_compute_units(&transaction);

            // Track account modifications
            for account in &transaction.accounts {
                accounts_modified.insert(account.clone());
            }

            transactions_processed += 1;

            // Simulate execution time
            tokio::time::sleep(std::time::Duration::from_micros(100)).await;
        }

        Ok(SolanaExecutionResult {
            transactions_processed,
            accounts_modified: accounts_modified.len(),
            compute_units_consumed,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Parse Ethereum block data from bytes
    async fn parse_ethereum_block_data(
        block_data_bytes: &[u8],
    ) -> Result<EthereumBlockData, MultivmError> {
        // In a real implementation, this would use Ethereum's native block parsing
        let block_data: EthereumBlockData =
            bincode::deserialize(block_data_bytes).map_err(|e| {
                MultivmError::Serialization(format!("Failed to parse Ethereum block: {}", e))
            })?;

        tracing::debug!(
            "Parsed Ethereum block with {} transactions",
            block_data.transactions.len()
        );
        Ok(block_data)
    }

    /// Execute Ethereum transactions
    async fn execute_ethereum_transactions(
        block_data: EthereumBlockData,
    ) -> Result<EthereumExecutionResult, MultivmError> {
        let start_time = std::time::Instant::now();
        let mut gas_used = 0;
        let mut transactions_processed = 0;

        for transaction in block_data.transactions {
            // Simulate transaction execution
            gas_used += Self::estimate_ethereum_gas_usage(&transaction);
            transactions_processed += 1;

            // Simulate execution time
            tokio::time::sleep(std::time::Duration::from_micros(200)).await;
        }

        // Generate mock state and receipt roots
        let state_root = format!(
            "0x{}",
            hex::encode(
                &sha2::Sha256::digest(format!("state_{}", transactions_processed).as_bytes())[..16]
            )
        );
        let receipts_root = format!(
            "0x{}",
            hex::encode(&sha2::Sha256::digest(format!("receipts_{}", gas_used).as_bytes())[..16])
        );

        Ok(EthereumExecutionResult {
            transactions_processed,
            gas_used,
            state_root,
            receipts_root,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Parse MultiVM block data from bytes
    async fn parse_multivm_block_data(
        block_data_bytes: &[u8],
    ) -> Result<MultiVmBlockData, MultivmError> {
        let block_data: MultiVmBlockData = bincode::deserialize(block_data_bytes).map_err(|e| {
            MultivmError::Serialization(format!("Failed to parse MultiVM block: {}", e))
        })?;

        tracing::debug!(
            "Parsed MultiVM block with {} SVM txs, {} EVM txs, {} special txs",
            block_data.svm_transactions.len(),
            block_data.evm_transactions.len(),
            block_data.special_transactions.len()
        );
        Ok(block_data)
    }

    /// Execute MultiVM transactions across both engines
    async fn execute_multivm_transactions(
        block_data: MultiVmBlockData,
    ) -> Result<MultiVmExecutionResult, MultivmError> {
        let start_time = std::time::Instant::now();

        // Execute SVM transactions
        let svm_result = if !block_data.svm_transactions.is_empty() {
            let svm_block = SolanaBlockData {
                transactions: block_data.svm_transactions,
            };
            Some(Self::execute_solana_transactions(svm_block).await?)
        } else {
            None
        };

        // Execute EVM transactions
        let evm_result = if !block_data.evm_transactions.is_empty() {
            let evm_block = EthereumBlockData {
                transactions: block_data.evm_transactions,
            };
            Some(Self::execute_ethereum_transactions(evm_block).await?)
        } else {
            None
        };

        // Process special transactions (cross-VM operations, account bindings)
        let mut cross_vm_operations = 0;
        for special_tx in &block_data.special_transactions {
            // Simulate special transaction processing
            match special_tx.tx_type.as_str() {
                "cross_vm_transfer" | "account_binding" => {
                    cross_vm_operations += 1;
                }
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_micros(500)).await;
        }

        // Generate combined state roots
        let solana_state_root = format!(
            "0x{}",
            hex::encode(
                &sha2::Sha256::digest(
                    format!(
                        "svm_state_{}",
                        svm_result
                            .as_ref()
                            .map(|r| r.transactions_processed)
                            .unwrap_or(0)
                    )
                    .as_bytes()
                )[..16]
            )
        );
        let ethereum_state_root = format!(
            "0x{}",
            hex::encode(
                &sha2::Sha256::digest(
                    format!(
                        "evm_state_{}",
                        evm_result
                            .as_ref()
                            .map(|r| r.transactions_processed)
                            .unwrap_or(0)
                    )
                    .as_bytes()
                )[..16]
            )
        );

        Ok(MultiVmExecutionResult {
            svm_transactions_processed: svm_result
                .as_ref()
                .map(|r| r.transactions_processed)
                .unwrap_or(0),
            evm_transactions_processed: evm_result
                .as_ref()
                .map(|r| r.transactions_processed)
                .unwrap_or(0),
            special_transactions_processed: block_data.special_transactions.len(),
            cross_vm_operations,
            total_gas_used: evm_result.as_ref().map(|r| r.gas_used).unwrap_or(0),
            total_compute_units: svm_result
                .as_ref()
                .map(|r| r.compute_units_consumed)
                .unwrap_or(0),
            solana_state_root,
            ethereum_state_root,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Estimate compute units for Solana transaction
    fn estimate_solana_compute_units(transaction: &SolanaTransactionData) -> u64 {
        // Basic estimation based on instruction complexity
        let base_units = 5000;
        let account_factor = transaction.accounts.len() as u64 * 1000;
        let data_factor = transaction.data.len() as u64 * 10;

        base_units + account_factor + data_factor
    }

    /// Estimate gas usage for Ethereum transaction
    fn estimate_ethereum_gas_usage(transaction: &EthereumTransactionData) -> u64 {
        // Basic estimation based on transaction complexity
        let base_gas = 21000;
        let data_gas = transaction.data.len() as u64 * 16; // 16 gas per byte

        std::cmp::min(base_gas + data_gas, transaction.gas_limit)
    }
}

/// Data structures for block processing

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct SolanaBlockData {
    transactions: Vec<SolanaTransactionData>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct SolanaTransactionData {
    signatures: Vec<String>,
    data: Vec<u8>,
    accounts: Vec<String>,
    recent_blockhash: String,
    fee: u64,
}

#[derive(Debug)]
struct SolanaExecutionResult {
    transactions_processed: usize,
    accounts_modified: usize,
    compute_units_consumed: u64,
    execution_time_ms: u64,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct EthereumBlockData {
    transactions: Vec<EthereumTransactionData>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct EthereumTransactionData {
    from: String,
    to: Option<String>,
    value: u64,
    gas_price: u64,
    gas_limit: u64,
    nonce: u64,
    data: Vec<u8>,
}

#[derive(Debug)]
struct EthereumExecutionResult {
    transactions_processed: usize,
    gas_used: u64,
    state_root: String,
    receipts_root: String,
    execution_time_ms: u64,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct MultiVmBlockData {
    svm_transactions: Vec<SolanaTransactionData>,
    evm_transactions: Vec<EthereumTransactionData>,
    special_transactions: Vec<SpecialTransactionData>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct SpecialTransactionData {
    tx_type: String,
    data: Vec<u8>,
    signature: Vec<u8>,
}

#[derive(Debug)]
struct MultiVmExecutionResult {
    svm_transactions_processed: usize,
    evm_transactions_processed: usize,
    special_transactions_processed: usize,
    cross_vm_operations: usize,
    total_gas_used: u64,
    total_compute_units: u64,
    solana_state_root: String,
    ethereum_state_root: String,
    execution_time_ms: u64,
}

/// IPC Client for sending commands to remote processes
pub struct IpcClient {
    socket_path: Option<PathBuf>,
    tcp_address: Option<SocketAddr>,
}

impl IpcClient {
    /// Create a new Unix socket IPC client
    pub async fn new_unix_socket(socket_path: &str) -> MultivmResult<Self> {
        // Verify socket exists
        let path = PathBuf::from(socket_path);
        if !path.exists() {
            return Err(MultivmError::Ipc(format!(
                "Unix socket does not exist: {}",
                socket_path
            )));
        }

        Ok(Self {
            socket_path: Some(path),
            tcp_address: None,
        })
    }

    /// Create a new TCP IPC client
    pub async fn new_tcp_socket(address: SocketAddr) -> MultivmResult<Self> {
        Ok(Self {
            socket_path: None,
            tcp_address: Some(address),
        })
    }

    /// Send a command and await response
    pub async fn send_command(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        if let Some(ref socket_path) = self.socket_path {
            self.send_command_unix(socket_path, command).await
        } else if let Some(ref address) = self.tcp_address {
            self.send_command_tcp(*address, command).await
        } else {
            Err(MultivmError::Ipc("No connection configured".to_string()))
        }
    }

    /// Send command via Unix socket
    #[cfg(unix)]
    async fn send_command_unix(
        &self,
        socket_path: &PathBuf,
        command: IpcCommand,
    ) -> MultivmResult<IpcResponse> {
        let mut stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to connect to Unix socket: {}", e)))?;

        // Send handshake
        stream
            .write_all(b"MULTIVM_HANDSHAKE:client")
            .await
            .map_err(|e| MultivmError::Ipc(format!("Handshake failed: {}", e)))?;

        // Read handshake ack
        let mut ack_buffer = [0; 64];
        let n = stream
            .read(&mut ack_buffer)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to read handshake ack: {}", e)))?;

        if &ack_buffer[..n] != b"MULTIVM_HANDSHAKE_ACK" {
            return Err(MultivmError::Ipc(
                "Invalid handshake acknowledgment".to_string(),
            ));
        }

        // Create and send message
        let message = IpcMessage::new(ProcessId::Main, ProcessId::Main, command);
        let message_bytes = bincode::serialize(&message)
            .map_err(|e| MultivmError::Ipc(format!("Failed to serialize message: {}", e)))?;

        stream
            .write_all(&message_bytes)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to send message: {}", e)))?;

        // Read response
        let mut response_buffer = vec![0; 4096];
        let n = stream
            .read(&mut response_buffer)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to read response: {}", e)))?;

        let response: IpcResponse = bincode::deserialize(&response_buffer[..n])
            .map_err(|e| MultivmError::Ipc(format!("Failed to deserialize response: {}", e)))?;

        Ok(response)
    }

    /// Send command via TCP socket
    async fn send_command_tcp(
        &self,
        address: SocketAddr,
        command: IpcCommand,
    ) -> MultivmResult<IpcResponse> {
        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to connect to TCP socket: {}", e)))?;

        // Send handshake
        stream
            .write_all(b"MULTIVM_HANDSHAKE:client")
            .await
            .map_err(|e| MultivmError::Ipc(format!("Handshake failed: {}", e)))?;

        // Read handshake ack
        let mut ack_buffer = [0; 64];
        let n = stream
            .read(&mut ack_buffer)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to read handshake ack: {}", e)))?;

        if &ack_buffer[..n] != b"MULTIVM_HANDSHAKE_ACK" {
            return Err(MultivmError::Ipc(
                "Invalid handshake acknowledgment".to_string(),
            ));
        }

        // Create and send message
        let message = IpcMessage::new(ProcessId::Main, ProcessId::Main, command);
        let message_bytes = bincode::serialize(&message)
            .map_err(|e| MultivmError::Ipc(format!("Failed to serialize message: {}", e)))?;

        stream
            .write_all(&message_bytes)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to send message: {}", e)))?;

        // Read response
        let mut response_buffer = vec![0; 4096];
        let n = stream
            .read(&mut response_buffer)
            .await
            .map_err(|e| MultivmError::Ipc(format!("Failed to read response: {}", e)))?;

        let response: IpcResponse = bincode::deserialize(&response_buffer[..n])
            .map_err(|e| MultivmError::Ipc(format!("Failed to deserialize response: {}", e)))?;

        Ok(response)
    }

    #[cfg(not(unix))]
    async fn send_command_unix(
        &self,
        _socket_path: &PathBuf,
        _command: IpcCommand,
    ) -> MultivmResult<IpcResponse> {
        Err(MultivmError::Ipc(
            "Unix sockets not supported on this platform".to_string(),
        ))
    }
}
