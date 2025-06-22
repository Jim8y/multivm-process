//! Mock Solana process with IPC support
//!
//! This mock process implements the IPC protocol to simulate a Solana node
//! for testing the MultiVM system.

use multivm_common::{
    BlockchainType, EngineState, HealthStatus, ProcessId, RpcResponse,
    ipc::{IpcCommand, IpcMessage, IpcResponse},
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

    loop {
        // Read message length
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(_) => {
                debug!("Connection closed");
                return Ok(());
            }
        }

        let msg_len = u32::from_le_bytes(len_buf) as usize;
        if msg_len > 1024 * 1024 {
            error!("Message too large: {}", msg_len);
            continue;
        }

        // Read message
        let mut msg_buf = vec![0u8; msg_len];
        stream.read_exact(&mut msg_buf).await?;

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

        // For simplicity, just send the response as a serialized IpcResponse
        // In a real implementation, this would be handled by a proper message router

        let response_bytes = bincode::serialize(&response)?;
        let response_len = (response_bytes.len() as u32).to_le_bytes();

        stream.write_all(&response_len).await?;
        stream.write_all(&response_bytes).await?;
        stream.flush().await?;
    }
}

async fn process_command(
    command: IpcCommand,
    state: &Arc<RwLock<MockSolanaState>>,
) -> IpcResponse {
    match command {
        IpcCommand::HealthCheck => {
            info!("Mock Solana health check");
            IpcResponse::HealthCheck
        }
        IpcCommand::Shutdown { graceful: _, timeout: _ } => {
            info!("Mock Solana shutting down");
            IpcResponse::Ack
        }
        IpcCommand::GetHealth => {
            let state = state.read().await;
            let health = HealthStatus {
                process_id: ProcessId::Solana,
                is_healthy: true,
                last_block_processed: Some(state.blocks_processed),
                blocks_processed_total: state.blocks_processed,
                uptime: state.start_time.elapsed(),
                memory_usage: 100_000_000, // 100MB mock
                cpu_usage_percent: 5.0,
                rpc_active: true,
                errors_count: 0,
                last_error: None,
                timestamp: std::time::SystemTime::now(),
            };
            IpcResponse::Health { status: health }
        }
        IpcCommand::ProcessBlock { block_data_bytes, blockchain_type: _, expect_response: _ } => {
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

            info!("SVM block {} processed successfully", state.blocks_processed);

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
            IpcResponse::State { state: engine_state }
        }
        IpcCommand::Ping => {
            IpcResponse::Pong
        }
        IpcCommand::RequestNextBlock { current_block: _, blockchain_type: _ } => {
            IpcResponse::NextBlock {
                block_data_bytes: None,
                blockchain_type: None,
            }
        }
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