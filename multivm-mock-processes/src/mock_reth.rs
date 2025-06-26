//! Mock Reth process with IPC support
//!
//! This mock process implements the IPC protocol to simulate a Reth (Ethereum) node
//! for testing the MultiVM system.

use multivm_common::{
    types_rpc::RpcResponse,
    BlockchainType, EngineState, HealthStatus, ProcessId, {IpcCommand, IpcMessage, IpcResponse},
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Mock Reth state
struct MockRethState {
    blocks_processed: u64,
    transactions_processed: u64,
    latest_block_hash: String,
    #[allow(dead_code)]
    start_time: std::time::Instant,
    nonce_tracker: HashMap<String, u64>,
}

impl MockRethState {
    fn new() -> Self {
        Self {
            blocks_processed: 0,
            transactions_processed: 0,
            latest_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            start_time: std::time::Instant::now(),
            nonce_tracker: HashMap::new(),
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

    info!("Starting mock Reth process");

    let socket_path = "/tmp/multivm-ethereum.sock";

    // Remove existing socket
    if Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path)?;
    }

    // Create Unix socket listener
    let listener = UnixListener::bind(socket_path)?;
    info!("Mock Reth listening on: {}", socket_path);

    // Initialize state
    let state = Arc::new(RwLock::new(MockRethState::new()));

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
    state: Arc<RwLock<MockRethState>>,
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

async fn process_command(command: IpcCommand, state: &Arc<RwLock<MockRethState>>) -> IpcResponse {
    match command {
        IpcCommand::HealthCheck => {
            info!("Mock Reth health check");
            IpcResponse::HealthCheck
        }
        IpcCommand::Shutdown {
            graceful: _,
            timeout: _,
        } => {
            info!("Mock Reth shutting down");
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

            info!("Processing EVM block ({} bytes)", block_data_bytes.len());

            // Simulate processing
            tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;

            state.blocks_processed += 1;
            state.transactions_processed += 3; // Mock 3 transactions per block
            state.latest_block_hash = format!("0x{:064x}", state.blocks_processed);

            // Track some mock nonces
            for i in 0..3 {
                let addr = format!("0x{:040x}", state.blocks_processed * 1000 + i);
                *state.nonce_tracker.entry(addr).or_insert(0) += 1;
            }

            // Create a simple result structure
            let result = serde_json::json!({
                "block_height": state.blocks_processed,
                "block_hash": state.latest_block_hash.clone(),
                "transactions_processed": 3,
                "gas_used": 63000,
                "status": "success",
                "logs": vec![format!("Processed EVM block {}", state.blocks_processed)],
            });

            let result_bytes = serde_json::to_vec(&result).unwrap_or_default();

            info!(
                "EVM block {} processed successfully",
                state.blocks_processed
            );

            IpcResponse::BlockProcessed {
                result_bytes: Box::new(result_bytes),
                blockchain_type: BlockchainType::Ethereum,
                success: true,
            }
        }
        IpcCommand::GetState => {
            let state = state.read().await;
            let engine_state = EngineState {
                process_id: ProcessId::Ethereum,
                blockchain_type: BlockchainType::Ethereum,
                current_block: Some(state.blocks_processed),
                state_root: hex::decode(&state.latest_block_hash[2..]).unwrap_or_default(),
                is_syncing: false,
                peer_count: 0,
                rpc_endpoints: vec!["http://127.0.0.1:8545".to_string()],
                data_directory: "/tmp/mock-reth".to_string(),
                chain_id: 1,
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
                    "chainId": "0x1",
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
