//! Solana Engine RPC Server Implementation
//!
//! This module provides an Engine RPC server that forwards external RPC requests
//! to the internal Solana validator process. It acts as a transparent proxy,
//! forwarding all requests and responses without modification.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::engine::SolanaEngine;
use crate::SolanaEngineError;

/// Engine RPC server for forwarding requests to internal Solana validator
pub struct SolanaEngineRpcServer {
    /// HTTP client for forwarding requests
    http_client: Client,
    /// Internal Solana RPC URL
    internal_rpc_url: String,
    /// Server host and port
    server_host: String,
    server_port: u16,
    /// Server handle for shutdown
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SolanaEngineRpcServer {
    /// Create a new Engine RPC server
    pub fn new(server_host: String, server_port: u16, internal_rpc_url: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client,
            internal_rpc_url,
            server_host,
            server_port,
            server_handle: None,
        }
    }

    /// Start the Engine RPC server using hyper
    pub async fn start(&mut self) -> Result<(), SolanaEngineError> {
        let server_addr = format!("{}:{}", self.server_host, self.server_port);
        let http_client = self.http_client.clone();
        let internal_rpc_url = self.internal_rpc_url.clone();

        // Start the HTTP server using hyper
        let handle = tokio::spawn(async move {
            Self::run_proxy_server(server_addr, http_client, internal_rpc_url).await;
        });

        self.server_handle = Some(handle);

        Ok(())
    }

    /// Run the proxy server
    async fn run_proxy_server(server_addr: String, http_client: Client, internal_rpc_url: String) {
        // Parse the server address
        let addr: SocketAddr = server_addr.parse().expect("Invalid server address");

        // Create a service function that handles requests
        let make_service = make_service_fn(move |_conn| {
            let http_client = http_client.clone();
            let internal_rpc_url = internal_rpc_url.clone();

            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    Self::handle_request(req, http_client.clone(), internal_rpc_url.clone())
                }))
            }
        });

        // Create the server
        let server = Server::bind(&addr).serve(make_service);

        info!("Engine RPC server listening on http://{}", addr);

        // Run the server
        if let Err(e) = server.await {
            error!("Engine RPC server error: {}", e);
        }
    }

    /// Check if the request contains a forbidden method
    fn check_forbidden_method(body_bytes: &[u8]) -> Option<Response<Body>> {
        let json_str = std::str::from_utf8(body_bytes).ok()?;
        let json_value = serde_json::from_str::<Value>(json_str).ok()?;
        let method = json_value.get("method")?.as_str()?;

        if matches!(
            method,
            "requestAirdrop" | "sendTransaction" | "simulateTransaction"
        ) {
            warn!("Blocked forbidden RPC method: {}", method);
            Some(
                Response::builder()
                    .status(403)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"error":{"code":-25041,"message":"Method not allowed"}}"#,
                    ))
                    .unwrap(),
            )
        } else {
            debug!("Allowing RPC method: {}", method);
            None
        }
    }

    /// Handle incoming HTTP requests
    async fn handle_request(
        req: Request<Body>,
        http_client: Client,
        internal_rpc_url: String,
    ) -> Result<Response<Body>, Infallible> {
        // Read the request body as raw bytes
        let body_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .map_err(|e| {
                error!("Failed to read request body: {}", e);
                e
            })
            .unwrap_or_else(|_| hyper::body::Bytes::new());

        if body_bytes.is_empty() {
            return Ok(Response::builder()
                .status(400)
                .body(Body::from("Bad Request"))
                .unwrap());
        }

        // Check for forbidden methods
        if let Some(forbidden_response) = Self::check_forbidden_method(&body_bytes) {
            return Ok(forbidden_response);
        }

        debug!("Forwarding raw RPC request ({} bytes)", body_bytes.len());

        // Forward the raw request body to the internal Solana RPC
        let response = match http_client
            .post(&internal_rpc_url)
            .header("Content-Type", "application/json")
            .body(body_bytes.to_vec())
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to forward request to internal RPC: {}", e);
                return Ok(Response::builder()
                    .status(500)
                    .body(Body::from("Internal Server Error"))
                    .unwrap());
            }
        };

        // Get the response status and body
        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| {
                error!("Failed to read response from internal RPC: {}", e);
                e
            })
            .unwrap_or_else(|_| "Internal Server Error".to_string());

        if response_body == "Internal Server Error" {
            return Ok(Response::builder()
                .status(500)
                .body(Body::from("Internal Server Error"))
                .unwrap());
        }

        debug!(
            "Received response from internal RPC ({} bytes, status: {})",
            response_body.len(),
            status
        );

        // Return the response with the same status code
        Ok(Response::builder()
            .status(status.as_u16())
            .header("Content-Type", "application/json")
            .body(Body::from(response_body))
            .unwrap())
    }

    /// Stop the Engine RPC server
    pub async fn stop(&mut self) -> Result<(), SolanaEngineError> {
        if let Some(handle) = self.server_handle.take() {
            debug!("Stopping Solana Engine RPC server");
            handle.abort();
            debug!("Solana Engine RPC server stopped successfully");
        }
        Ok(())
    }

    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        self.server_handle.is_some()
    }
}

/// Extension trait for SolanaEngine to manage the RPC proxy server
impl SolanaEngine {
    /// Get the RPC server host from configuration
    pub fn get_rpc_server_host(&self) -> &str {
        &self.solana_engine_config.rpc_server_host
    }

    /// Get the RPC server port from configuration
    pub fn get_rpc_server_port(&self) -> u16 {
        self.solana_engine_config.rpc_server_port
    }

    /// Get the internal RPC port from configuration
    pub fn get_internal_rpc_port(&self) -> u16 {
        self.solana_config.rpc_port
    }

    /// Start the Engine RPC server
    pub async fn start_rpc_proxy_server(&mut self) -> Result<(), SolanaEngineError> {
        let internal_rpc_url = format!(
            "http://{}:{}",
            self.get_rpc_server_host(),
            self.get_internal_rpc_port()
        );

        let mut engine_rpc_server = SolanaEngineRpcServer::new(
            self.get_rpc_server_host().to_string(),
            self.get_rpc_server_port(),
            internal_rpc_url,
        );

        engine_rpc_server.start().await?;

        // Store the engine RPC server instance for later shutdown
        *self.rpc_proxy_server.write().await = Some(engine_rpc_server);

        Ok(())
    }

    /// Stop the Engine RPC server
    pub async fn stop_rpc_proxy_server(&mut self) -> Result<(), SolanaEngineError> {
        match self.rpc_proxy_server.write().await.take() {
            Some(mut server) => server.stop().await,
            None => Ok(()),
        }
    }
}
