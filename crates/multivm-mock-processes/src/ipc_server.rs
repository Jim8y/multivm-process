//! IPC Server for Mock Processes
//!
//! Provides a JSON-RPC server that can communicate over TCP or Unix sockets.

use crate::{MockProcessError, MockProcessResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

/// IPC message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID
    pub id: Option<Value>,
    /// Method name
    pub method: String,
    /// Method parameters
    pub params: Value,
}

/// IPC response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID (matches request)
    pub id: Option<Value>,
    /// Result on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Handler function type
pub type HandlerFn = Box<dyn Fn(Value) -> Value + Send + Sync>;

/// IPC server implementation
pub struct IpcServer {
    /// Server configuration
    endpoint: String,
    /// Request handlers
    handlers: Arc<RwLock<HashMap<String, HandlerFn>>>,
    /// Server state
    running: Arc<RwLock<bool>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl IpcServer {
    /// Create a new IPC server
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            shutdown_tx: None,
        }
    }

    /// Register a method handler
    pub async fn register_handler<F>(&self, method: &str, handler: F)
    where
        F: Fn(Value) -> Value + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers.insert(method.to_string(), Box::new(handler));
    }

    /// Start the IPC server
    pub async fn start(&mut self) -> MockProcessResult<()> {
        if *self.running.read().await {
            return Err(MockProcessError::AlreadyRunning);
        }

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let listener = TcpListener::bind(&self.endpoint).await?;
        info!("IPC server listening on {}", self.endpoint);

        *self.running.write().await = true;

        let handlers = Arc::clone(&self.handlers);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                debug!("New connection from {}", addr);
                                let handlers = Arc::clone(&handlers);
                                tokio::spawn(handle_connection(stream, handlers));
                            }
                            Err(e) => {
                                error!("Failed to accept connection: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("IPC server shutting down");
                        break;
                    }
                }
            }

            *running.write().await = false;
        });

        Ok(())
    }

    /// Stop the IPC server
    pub async fn stop(&mut self) -> MockProcessResult<()> {
        if !*self.running.read().await {
            return Err(MockProcessError::NotRunning);
        }

        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        // Wait for server to stop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    /// Check if server is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// Handle a client connection
async fn handle_connection(stream: TcpStream, handlers: Arc<RwLock<HashMap<String, HandlerFn>>>) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("Client disconnected");
                break;
            }
            Ok(_) => {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(&line) {
                    let response = handle_request(msg, &handlers).await;

                    if let Ok(response_str) = serde_json::to_string(&response) {
                        let _ = writer.write_all(response_str.as_bytes()).await;
                        let _ = writer.write_all(b"\n").await;
                        let _ = writer.flush().await;
                    }
                }
            }
            Err(e) => {
                error!("Failed to read from client: {}", e);
                break;
            }
        }
    }
}

/// Handle a JSON-RPC request
async fn handle_request(
    msg: IpcMessage,
    handlers: &Arc<RwLock<HashMap<String, HandlerFn>>>,
) -> IpcResponse {
    let handlers = handlers.read().await;

    if let Some(handler) = handlers.get(&msg.method) {
        let result = handler(msg.params);

        IpcResponse {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(result),
            error: None,
        }
    } else {
        IpcResponse {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method '{}' not found", msg.method),
                data: None,
            }),
        }
    }
}

/// Common JSON-RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ipc_server_creation() {
        let server = IpcServer::new("127.0.0.1:0".to_string());
        assert!(!server.is_running().await);
    }

    #[test]
    fn test_ipc_message_serialization() {
        let msg = IpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "eth_sendTransaction".to_string(),
            params: Value::Array(vec![]),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("eth_sendTransaction"));
    }
}
