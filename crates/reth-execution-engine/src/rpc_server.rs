use jsonrpc_core::{Error as JsonRpcError, IoHandler, Params, Value};
use jsonrpc_http_server::{RestApi, ServerBuilder};
use multivm_common::*;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

#[allow(dead_code)]
pub struct RethRpcServer {
    port: u16,
    is_running: bool,
    mock_mode: bool,
    server_handle: Option<JoinHandle<()>>,
}

#[allow(dead_code)]
impl RethRpcServer {
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
            tracing::info!("Starting Reth RPC server on port {} (MOCK mode)", self.port);
            self.is_running = true;
            tracing::info!("Reth RPC server started in MOCK mode");
        } else {
            tracing::info!("Starting Reth RPC server on port {} (real mode)", self.port);

            // Create JSON-RPC IO handler with Ethereum RPC methods
            let mut io = IoHandler::default();

            // Add Ethereum JSON-RPC methods
            io.add_method("eth_blockNumber", |_params: Params| async {
                // Return the latest block number
                Ok(Value::String("0x1".to_string()))
            });

            io.add_method("eth_getBalance", |params: Params| async {
                // Get balance for an address
                match params.parse::<Vec<String>>() {
                    Ok(parsed) if parsed.len() >= 1 => {
                        // Return balance (hardcoded for demo)
                        Ok(Value::String("0x1bc16d674ec80000".to_string())) // 2 ETH
                    }
                    _ => Err(JsonRpcError::invalid_params("Expected address parameter")),
                }
            });

            io.add_method("eth_getBlockByNumber", |params: Params| async {
                // Get block by number
                match params.parse::<Vec<Value>>() {
                    Ok(parsed) if parsed.len() >= 1 => {
                        Ok(serde_json::json!({
                            "number": "0x1",
                            "hash": "0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9",
                            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                            "timestamp": "0x0",
                            "gasLimit": "0x1c9c380",
                            "gasUsed": "0x0",
                            "transactions": []
                        }))
                    }
                    _ => Err(JsonRpcError::invalid_params("Expected block number parameter"))
                }
            });

            io.add_method("eth_sendRawTransaction", |params: Params| async {
                // Send raw transaction
                match params.parse::<Vec<String>>() {
                    Ok(parsed) if parsed.len() >= 1 => {
                        // Return transaction hash (mock)
                        Ok(Value::String(
                            "0x9fc76417374aa880d4449a1f7f31ec597f00b1f6f3dd2d66f4c9c6c445836d8b"
                                .to_string(),
                        ))
                    }
                    _ => Err(JsonRpcError::invalid_params(
                        "Expected raw transaction data",
                    )),
                }
            });

            io.add_method("net_version", |_params: Params| async {
                // Return network version (1 for mainnet, 11155111 for sepolia, etc.)
                Ok(Value::String("1".to_string()))
            });

            io.add_method("web3_clientVersion", |_params: Params| async {
                // Return client version
                Ok(Value::String("Reth/v0.1.0".to_string()))
            });

            // Start the HTTP server
            let bind_address: SocketAddr = format!("127.0.0.1:{}", self.port)
                .parse()
                .map_err(|e| MultivmError::Configuration(format!("Invalid bind address: {}", e)))?;

            let server = ServerBuilder::new(io)
                .rest_api(RestApi::Unsecure)
                .start_http(&bind_address)
                .map_err(|e| MultivmError::Rpc(format!("Failed to start RPC server: {}", e)))?;

            // Spawn server in background task
            let server_handle = tokio::spawn(async move {
                tracing::info!("Reth RPC server listening on http://{}", bind_address);
                server.wait();
            });

            self.server_handle = Some(server_handle);
            self.is_running = true;
            tracing::info!("Reth RPC server started on port {}", self.port);
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), MultivmError> {
        if self.mock_mode {
            tracing::info!("Stopping Reth RPC server (MOCK mode)");
        } else {
            tracing::info!("Stopping Reth RPC server (real mode)");

            // Stop the server if it's running
            if let Some(handle) = self.server_handle.take() {
                handle.abort();
                // Wait for the task to finish
                let _ = handle.await;
                tracing::info!("Reth RPC server stopped");
            }
        }
        self.is_running = false;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }
}
