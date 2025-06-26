//! Mock Solana process with IPC support
//!
//! This mock process implements the IPC protocol to simulate a Solana node
//! for testing the MultiVM system.

use multivm_common::{
    types_rpc::RpcResponse,
    BlockchainType, EngineState, HealthStatus, ProcessId, {IpcCommand, IpcMessage, IpcResponse},
};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Mock Solana state
struct MockSolanaState {
    blocks_processed: u64,
    transactions_processed: u64,
    latest_block_hash: String,
    #[allow(dead_code)]
    start_time: std::time::Instant,
}

impl MockSolanaState {
    fn new() -> Self {
        Self {
            blocks_processed: 0,
            transactions_processed: 0,
            latest_block_hash: "11111111111111111111111111111111".to_string(),
            start_time: std::time::Instant::now(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting mock Solana process");

    let socket_path = "/tmp/multivm-solana.sock";

    // Remove existing socket
    if Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path)?;
    }

    // Create Unix socket listener
    let listener = UnixListener::bind(socket_path)?;
    info!("Mock Solana listening on: {}", socket_path);

    // Initialize state
    let state = Arc::new(RwLock::new(MockSolanaState::new()));

    // Accept connections
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state_clone).await {
                        error!("Connection handler error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    state: Arc<RwLock<MockSolanaState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("New connection established");

    // Handle handshake first
    let mut handshake_buf = vec![0u8; 64];
    match stream.read(&mut handshake_buf).await {
        Ok(n) => {
            let handshake = String::from_utf8_lossy(&handshake_buf[..n]);
            if handshake.starts_with("MULTIVM_HANDSHAKE:") {
                // Send handshake acknowledgment
                stream.write_all(b"MULTIVM_HANDSHAKE_ACK").await?;
                debug!("Handshake completed");
            } else {
                error!("Invalid handshake: {}", handshake);
                return Err("Invalid handshake".into());
            }
        }
        Err(e) => {
            error!("Failed to read handshake: {}", e);
            return Err(e.into());
        }
    }

    loop {
        // Read the raw message (no length prefix in this protocol)
        let mut msg_buf = vec![0u8; 4096];
        match stream.read(&mut msg_buf).await {
            Ok(0) => {
                debug!("Connection closed");
                return Ok(());
            }
            Ok(n) => {
                msg_buf.truncate(n);

                // Deserialize message
                let message: IpcMessage = match bincode::deserialize(&msg_buf) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to deserialize message: {}", e);
                        continue;
                    }
                };

                debug!("Received command: {:?}", message.command);

                // Process command
                let response = process_command(message.command, &state).await;

                // Send response directly (no length prefix needed)
                let response_bytes = bincode::serialize(&response)?;
                stream.write_all(&response_bytes).await?;
                stream.flush().await?;
            }
            Err(e) => {
                error!("Failed to read from stream: {}", e);
                return Err(e.into());
            }
        }
    }
}

async fn process_command(command: IpcCommand, state: &Arc<RwLock<MockSolanaState>>) -> IpcResponse {
    match command {
        IpcCommand::HealthCheck => {
            info!("Mock Solana health check");
            IpcResponse::HealthCheck
        }
        IpcCommand::Shutdown {
            graceful: _,
            timeout: _,
        } => {
            info!("Mock Solana shutting down");
            IpcResponse::Ack
        }
        IpcCommand::GetHealth => {
            let _state = state.read().await;
            let health = HealthStatus::Healthy; // Mock processes are always healthy
            IpcResponse::Health { status: health }
        }
        IpcCommand::ProcessBlock {
            block_data_bytes,
            blockchain_type: _,
            expect_response: _,
        } => {
            let mut state = state.write().await;

            info!("Processing SVM block ({} bytes)", block_data_bytes.len());

            // Simulate processing
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            state.blocks_processed += 1;
            state.transactions_processed += 3; // Mock 3 transactions per block
            state.latest_block_hash = format!("{:064x}", state.blocks_processed);

            // Create a simple result structure
            let result = serde_json::json!({
                "block_height": state.blocks_processed,
                "block_hash": state.latest_block_hash.clone(),
                "transactions_processed": 3,
                "gas_used": 15000,
                "status": "success",
                "logs": vec![format!("Processed SVM block {}", state.blocks_processed)],
            });

            let result_bytes = serde_json::to_vec(&result).unwrap_or_default();

            info!(
                "SVM block {} processed successfully",
                state.blocks_processed
            );

            IpcResponse::BlockProcessed {
                result_bytes,
                blockchain_type: BlockchainType::Solana,
                success: true,
            }
        }
        IpcCommand::GetState => {
            let state = state.read().await;
            let engine_state = EngineState {
                process_id: ProcessId::Solana,
                blockchain_type: BlockchainType::Solana,
                current_block: Some(state.blocks_processed),
                state_root: state.latest_block_hash.as_bytes().to_vec(),
                is_syncing: false,
                peer_count: 0,
                rpc_endpoints: vec!["http://127.0.0.1:8899".to_string()],
                data_directory: "/tmp/mock-solana".to_string(),
                chain_id: 103,
            };
            IpcResponse::State {
                state: engine_state,
            }
        }
        IpcCommand::Ping => IpcResponse::Pong,
        IpcCommand::RequestNextBlock {
            current_block: _,
            blockchain_type: _,
        } => IpcResponse::NextBlock {
            block_data_bytes: None,
            blockchain_type: None,
        },
        IpcCommand::RpcCall { call } => {
            let response = RpcResponse {
                result: Some(serde_json::json!({
                    "method": call.method,
                    "mock": true,
                })),
                error: None,
                id: call.id,
            };
            IpcResponse::RpcResponse { response }
        }
        _ => {
            warn!("Unhandled command: {:?}", command);
            IpcResponse::Error {
                code: -32601,
                message: "Method not found".to_string(),
                details: None,
            }
        }
    }
}
