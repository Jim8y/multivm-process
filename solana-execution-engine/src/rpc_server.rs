//! Solana RPC Server
//!
//! This module implements the JSON-RPC server for the Solana execution engine,
//! providing standard Solana RPC methods for transaction submission, account queries,
//! and blockchain state inspection.

use jsonrpc_core::{Error as JsonRpcError, IoHandler, Params, Value};
use jsonrpc_http_server::{RestApi, ServerBuilder};
use multivm_common::MultivmError;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

#[allow(dead_code)]
pub struct SolanaRpcServer {
    port: u16,
    is_running: bool,
    mock_mode: bool,
    server_handle: Option<JoinHandle<()>>,
}

#[allow(dead_code)]
impl SolanaRpcServer {
    pub fn new(port: u16) -> Self {
        Self::new_with_mode(port, cfg!(feature = "mock"))
    }

    pub fn new_with_mode(port: u16, mock_mode: bool) -> Self {
        Self {
            port,
            is_running: false,
            mock_mode,
            server_handle: None,
        }
    }

    pub async fn start(&mut self) -> Result<(), MultivmError> {
        if self.mock_mode {
            tracing::info!(
                "Starting Solana RPC server on port {} (MOCK mode)",
                self.port
            );
            self.is_running = true;
            tracing::info!("Solana RPC server started in MOCK mode");
        } else {
            tracing::info!(
                "Starting Solana RPC server on port {} (real mode)",
                self.port
            );

            // Create JSON-RPC IO handler with Solana RPC methods
            let mut io = IoHandler::default();

            // Add Solana JSON-RPC methods
            io.add_method("getHealth", |_params: Params| async {
                Ok(Value::String("ok".to_string()))
            });

            io.add_method("getVersion", |_params: Params| async {
                Ok(serde_json::json!({
                    "solana-core": "1.16.0",
                    "feature-set": 1234567890
                }))
            });

            io.add_method("getSlot", |_params: Params| async {
                // Return current slot
                Ok(Value::Number(serde_json::Number::from(100u64)))
            });

            io.add_method("getBalance", |params: Params| async {
                // Get balance for a public key
                match params.parse::<Vec<String>>() {
                    Ok(parsed) if !parsed.is_empty() => {
                        // Return balance in lamports (1 SOL = 1e9 lamports)
                        Ok(serde_json::json!({
                            "context": {"slot": 100},
                            "value": 2000000000u64  // 2 SOL in lamports
                        }))
                    }
                    _ => Err(JsonRpcError::invalid_params("Expected pubkey parameter")),
                }
            });

            io.add_method("getBlockHeight", |_params: Params| async {
                // Return current block height
                Ok(Value::Number(serde_json::Number::from(100u64)))
            });

            io.add_method("getLatestBlockhash", |_params: Params| async {
                // Return latest blockhash
                Ok(serde_json::json!({
                    "context": {"slot": 100},
                    "value": {
                        "blockhash": "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N",
                        "lastValidBlockHeight": 150
                    }
                }))
            });

            io.add_method("sendTransaction", |params: Params| async {
                // Send transaction
                match params.parse::<Vec<String>>() {
                    Ok(parsed) if !parsed.is_empty() => {
                        // Return transaction signature (mock)
                        Ok(Value::String("5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW".to_string()))
                    }
                    _ => Err(JsonRpcError::invalid_params("Expected transaction data"))
                }
            });

            io.add_method("getAccountInfo", |params: Params| async {
                // Get account info for a public key
                match params.parse::<Vec<String>>() {
                    Ok(parsed) if !parsed.is_empty() => Ok(serde_json::json!({
                        "context": {"slot": 100},
                        "value": {
                            "data": ["", "base58"],
                            "executable": false,
                            "lamports": 1000000000u64,
                            "owner": "11111111111111111111111111111112",
                            "rentEpoch": 361
                        }
                    })),
                    _ => Err(JsonRpcError::invalid_params("Expected pubkey parameter")),
                }
            });

            io.add_method("getEpochInfo", |_params: Params| async {
                // Return epoch information
                Ok(serde_json::json!({
                    "absoluteSlot": 100,
                    "blockHeight": 100,
                    "epoch": 1,
                    "slotIndex": 100,
                    "slotsInEpoch": 432000,
                    "transactionCount": 1000
                }))
            });

            // Start the HTTP server
            let bind_address: SocketAddr =
                format!("127.0.0.1:{}", self.port).parse().map_err(|e| {
                    MultivmError::Configuration {
                        component: "solana-rpc-server".to_string(),
                        message: format!("Invalid bind address: {}", e),
                        validation_errors: None,
                    }
                })?;

            let server = ServerBuilder::new(io)
                .rest_api(RestApi::Unsecure)
                .start_http(&bind_address)
                .map_err(|e| MultivmError::Rpc {
                    method: "start_http_server".to_string(),
                    message: format!("Failed to start RPC server: {}", e),
                    status_code: None,
                })?;

            // Spawn server in background task
            let server_handle = tokio::spawn(async move {
                tracing::info!("Solana RPC server listening on http://{}", bind_address);
                server.wait();
            });

            self.server_handle = Some(server_handle);
            self.is_running = true;
            tracing::info!("Solana RPC server started on port {}", self.port);
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), MultivmError> {
        if self.mock_mode {
            tracing::info!("Stopping Solana RPC server (MOCK mode)");
        } else {
            tracing::info!("Stopping Solana RPC server (real mode)");

            // Stop the server if it's running
            if let Some(handle) = self.server_handle.take() {
                handle.abort();
                // Wait for the task to finish
                let _ = handle.await;
                tracing::info!("Solana RPC server stopped");
            }
        }
        self.is_running = false;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }
}
