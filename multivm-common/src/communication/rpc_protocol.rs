//! RPC (Remote Procedure Call) protocol implementation
//!
//! This module implements communication with execution engines via JSON-RPC
//! over HTTP and WebSocket connections.

use super::{
    CommunicationError, CommunicationProtocol, CommunicationRequest, CommunicationResponse,
    ConnectionStatus, EngineType, HealthStatus, ProtocolType,
};
use crate::{MultivmError, MultivmResult};
use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for RPC protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcProtocolConfig {
    /// RPC endpoint URL
    pub endpoint_url: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Health check configuration
    pub health_check_config: HealthCheckConfig,
    /// HTTP headers to include with requests
    pub default_headers: HashMap<String, String>,
    /// User agent string
    pub user_agent: String,
    /// Whether to use HTTP/2
    pub use_http2: bool,
    /// TLS configuration
    pub tls_config: TlsConfig,
}

/// Retry configuration for RPC calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Jitter to add to retry delays
    pub jitter: bool,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check method name
    pub health_method: String,
    /// Health check interval
    pub interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Enable automatic health checks
    pub enable_auto_check: bool,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Accept invalid certificates (for testing)
    pub accept_invalid_certs: bool,
    /// Accept invalid hostnames (for testing)
    pub accept_invalid_hostnames: bool,
    /// Path to CA certificate file
    pub ca_cert_path: Option<String>,
    /// Path to client certificate file
    pub client_cert_path: Option<String>,
    /// Path to client private key file
    pub client_key_path: Option<String>,
}

impl Default for RpcProtocolConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "http://127.0.0.1:8545".to_string(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_connections: 10,
            retry_config: RetryConfig::default(),
            health_check_config: HealthCheckConfig::default(),
            default_headers: HashMap::new(),
            user_agent: "MultiVM-RPC-Client/1.0".to_string(),
            use_http2: true,
            tls_config: TlsConfig::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            health_method: "eth_chainId".to_string(), // Default for Ethereum
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            enable_auto_check: true,
        }
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
        }
    }
}

/// JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    params: Value,
}

/// JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// RPC protocol implementation
pub struct RpcProtocol {
    /// Engine type this protocol communicates with
    engine_type: EngineType,
    /// Protocol configuration
    config: RpcProtocolConfig,
    /// HTTP client
    client: Client,
    /// Connection status
    connection_status: Arc<RwLock<ConnectionStatus>>,
    /// Health check task handle
    health_check_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RpcProtocol {
    /// Create a new RPC protocol instance
    pub async fn new(engine_type: EngineType, config: RpcProtocolConfig) -> MultivmResult<Self> {
        let client = Self::build_http_client(&config).await?;

        let connection_status = ConnectionStatus {
            is_connected: false,
            last_success: None,
            latency_ms: None,
            success_count: 0,
            error_count: 0,
        };

        Ok(Self {
            engine_type,
            config,
            client,
            connection_status: Arc::new(RwLock::new(connection_status)),
            health_check_handle: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Build HTTP client with configuration
    async fn build_http_client(config: &RpcProtocolConfig) -> MultivmResult<Client> {
        let mut builder = ClientBuilder::new()
            .timeout(config.request_timeout)
            .connect_timeout(config.connect_timeout)
            .user_agent(&config.user_agent);

        // Configure TLS
        if config.tls_config.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }
        // Note: danger_accept_invalid_hostnames is not available in reqwest 0.11
        // This would need to be handled differently or upgraded to a newer version

        // Add client certificates if configured
        if let (Some(cert_path), Some(key_path)) = (
            &config.tls_config.client_cert_path,
            &config.tls_config.client_key_path,
        ) {
            let _cert =
                tokio::fs::read(cert_path)
                    .await
                    .map_err(|e| MultivmError::Configuration {
                        component: "rpc_protocol".to_string(),
                        message: format!("Failed to read client certificate: {e}"),
                        validation_errors: Some(vec![cert_path.clone()]),
                    })?;

            let _key =
                tokio::fs::read(key_path)
                    .await
                    .map_err(|e| MultivmError::Configuration {
                        component: "rpc_protocol".to_string(),
                        message: format!("Failed to read client private key: {e}"),
                        validation_errors: Some(vec![key_path.clone()]),
                    })?;

            // Client certificate authentication requires native-tls or rustls identity support
            // This is a known limitation with reqwest 0.11
            return Err(MultivmError::UnsupportedOperation {
                operation: "Client certificate authentication".to_string(),
                alternatives: Some(vec![
                    "Use JWT authentication".to_string(),
                    "Use API key authentication".to_string(),
                ]),
            });
        }

        // Configure connection pooling
        builder = builder
            .pool_max_idle_per_host(config.max_connections)
            .pool_idle_timeout(Duration::from_secs(90));

        builder.build().map_err(|e| MultivmError::Configuration {
            component: "rpc_protocol".to_string(),
            message: format!("Failed to build HTTP client: {e}"),
            validation_errors: None,
        })
    }

    /// Create JSON-RPC request
    fn create_rpc_request(&self, request: &CommunicationRequest) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(request.id),
            method: request.method.clone(),
            params: request.params.clone(),
        }
    }

    /// Send HTTP request with retry logic
    async fn send_with_retry(&self, rpc_request: JsonRpcRequest) -> MultivmResult<JsonRpcResponse> {
        let retry_config = &self.config.retry_config;
        let mut delay = retry_config.base_delay;
        let mut last_error = None;

        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                debug!(
                    "Retry attempt {} for RPC request {}",
                    attempt, rpc_request.id
                );
                tokio::time::sleep(delay).await;

                if retry_config.jitter {
                    delay = self.add_jitter(delay);
                }

                delay = std::cmp::min(
                    Duration::from_millis(
                        (delay.as_millis() as f64 * retry_config.backoff_multiplier) as u64,
                    ),
                    retry_config.max_delay,
                );
            }

            let start_time = std::time::Instant::now();

            // Prepare headers
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );

            // Add default headers
            for (key, value) in &self.config.default_headers {
                if let (Ok(header_name), Ok(header_value)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    reqwest::header::HeaderValue::from_str(value),
                ) {
                    headers.insert(header_name, header_value);
                }
            }

            // Make the request
            let response = self
                .client
                .post(&self.config.endpoint_url)
                .headers(headers)
                .json(&rpc_request)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let latency = start_time.elapsed().as_millis() as u64;

                    if resp.status().is_success() {
                        match resp.json::<JsonRpcResponse>().await {
                            Ok(rpc_response) => {
                                // Update connection status on success
                                let mut status = self.connection_status.write().await;
                                status.last_success = Some(SystemTime::now());
                                status.success_count += 1;
                                status.is_connected = true;
                                status.latency_ms = Some(latency);

                                return Ok(rpc_response);
                            }
                            Err(e) => {
                                last_error = Some(MultivmError::Network {
                                    message: format!("Failed to parse JSON response: {e}"),
                                    endpoint: Some("rpc_protocol".to_string()),
                                    retry_after: None,
                                });
                            }
                        }
                    } else {
                        let status_code = resp.status();
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());

                        // Handle rate limiting
                        if status_code == reqwest::StatusCode::TOO_MANY_REQUESTS {
                            last_error = Some(MultivmError::RateLimited {
                                message: format!("Rate limited: {}", error_text),
                                retry_after: Some(Duration::from_secs(60)), // Default retry after 60s
                                current_rate: None,
                            });

                            // Wait before retrying
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        } else {
                            last_error = Some(MultivmError::Network {
                                message: format!("HTTP error {}: {}", status_code, error_text),
                                endpoint: Some(self.config.endpoint_url.clone()),
                                retry_after: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(MultivmError::Network {
                        message: format!("HTTP request failed: {e}"),
                        endpoint: Some("rpc_protocol".to_string()),
                        retry_after: None,
                    });
                }
            }

            // Update error count
            let mut status = self.connection_status.write().await;
            status.error_count += 1;
            status.is_connected = false;
        }

        Err(last_error.unwrap_or_else(|| MultivmError::Network {
            message: "All RPC retry attempts exhausted".to_string(),
            endpoint: Some("rpc_protocol".to_string()),
            retry_after: None,
        }))
    }

    /// Add jitter to delay
    fn add_jitter(&self, delay: Duration) -> Duration {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let jitter_factor = rng.gen_range(0.5..1.5);
        Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64)
    }

    /// Convert JSON-RPC response to CommunicationResponse
    fn convert_response(
        &self,
        rpc_response: JsonRpcResponse,
        request_id: String,
    ) -> CommunicationResponse {
        let (result, error) = if let Some(rpc_error) = rpc_response.error {
            let error = CommunicationError {
                code: rpc_error.code,
                message: rpc_error.message,
                data: rpc_error.data,
            };
            (None, Some(error))
        } else {
            (rpc_response.result, None)
        };

        let mut metadata = HashMap::new();
        metadata.insert("protocol".to_string(), "json-rpc".to_string());
        metadata.insert("jsonrpc".to_string(), rpc_response.jsonrpc);
        metadata.insert("rpc_id".to_string(), rpc_response.id.to_string());

        CommunicationResponse {
            request_id,
            result,
            error,
            metadata,
        }
    }

    /// Start automatic health check task
    async fn start_health_check_task(&self) {
        if !self.config.health_check_config.enable_auto_check {
            return;
        }

        let config = self.config.health_check_config.clone();
        let client = self.client.clone();
        let endpoint_url = self.config.endpoint_url.clone();
        let status = Arc::clone(&self.connection_status);
        let default_headers = self.config.default_headers.clone();

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.interval);

            loop {
                interval.tick().await;

                let health_request = JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: json!(Uuid::new_v4().to_string()),
                    method: config.health_method.clone(),
                    params: json!([]),
                };

                let start_time = std::time::Instant::now();

                // Prepare headers
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::CONTENT_TYPE,
                    reqwest::header::HeaderValue::from_static("application/json"),
                );

                for (key, value) in &default_headers {
                    if let (Ok(header_name), Ok(header_value)) = (
                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                        reqwest::header::HeaderValue::from_str(value),
                    ) {
                        headers.insert(header_name, header_value);
                    }
                }

                let request_result = client
                    .post(&endpoint_url)
                    .headers(headers)
                    .json(&health_request)
                    .timeout(config.timeout)
                    .send()
                    .await;

                match request_result {
                    Ok(response) if response.status().is_success() => {
                        match response.json::<JsonRpcResponse>().await {
                            Ok(rpc_response) if rpc_response.error.is_none() => {
                                let latency = start_time.elapsed().as_millis() as u64;
                                let mut status_guard = status.write().await;
                                status_guard.is_connected = true;
                                status_guard.latency_ms = Some(latency);
                                status_guard.last_success = Some(SystemTime::now());
                                debug!("RPC health check successful, latency: {}ms", latency);
                            }
                            Ok(_) | Err(_) => {
                                warn!("RPC health check failed: Invalid response");
                                let mut status_guard = status.write().await;
                                status_guard.is_connected = false;
                                status_guard.error_count += 1;
                            }
                        }
                    }
                    _ => {
                        warn!("RPC health check failed: Request error");
                        let mut status_guard = status.write().await;
                        status_guard.is_connected = false;
                        status_guard.error_count += 1;
                    }
                }
            }
        });

        *self.health_check_handle.lock().await = Some(task);
    }

    /// Stop health check task
    async fn stop_health_check_task(&self) {
        if let Some(handle) = self.health_check_handle.lock().await.take() {
            handle.abort();
        }
    }

    /// Get health method based on engine type
    fn get_default_health_method(&self) -> String {
        match self.engine_type {
            EngineType::Ethereum => "eth_chainId".to_string(),
            EngineType::Solana => "getVersion".to_string(),
        }
    }
}

#[async_trait]
impl CommunicationProtocol for RpcProtocol {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Rpc
    }

    fn engine_type(&self) -> EngineType {
        self.engine_type.clone()
    }

    async fn connect(&mut self) -> MultivmResult<()> {
        info!(
            "Connecting RPC protocol to {} engine at {}",
            self.engine_type, self.config.endpoint_url
        );

        // Test connection with a simple health check
        let health_method = self.get_default_health_method();
        let test_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: health_method,
            params: json!([]),
            timeout: Some(self.config.health_check_config.timeout),
            auth_context: None,
        };

        // Try to send a test request
        match self.send_request(test_request).await {
            Ok(_) => {
                info!("RPC protocol connected successfully");

                // Start health check task
                self.start_health_check_task().await;

                Ok(())
            }
            Err(e) => {
                error!("Failed to connect RPC protocol: {}", e);
                Err(e)
            }
        }
    }

    async fn disconnect(&mut self) -> MultivmResult<()> {
        info!(
            "Disconnecting RPC protocol from {} engine",
            self.engine_type
        );

        // Stop health check task
        self.stop_health_check_task().await;

        // Update connection status
        {
            let mut status = self.connection_status.write().await;
            status.is_connected = false;
        }

        info!("RPC protocol disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        if let Ok(status) = self.connection_status.try_read() {
            status.is_connected
        } else {
            false
        }
    }

    async fn send_request(
        &self,
        request: CommunicationRequest,
    ) -> MultivmResult<CommunicationResponse> {
        debug!(
            "Sending RPC request: {} to {}",
            request.method, self.engine_type
        );

        let rpc_request = self.create_rpc_request(&request);
        let request_id = request.id.clone();

        let rpc_response = self.send_with_retry(rpc_request).await?;
        let response = self.convert_response(rpc_response, request_id);

        debug!("RPC request completed");
        Ok(response)
    }

    async fn send_notification(&self, request: CommunicationRequest) -> MultivmResult<()> {
        debug!(
            "Sending RPC notification: {} to {}",
            request.method, self.engine_type
        );

        // For JSON-RPC notifications, we set id to null and don't wait for response
        let mut rpc_request = self.create_rpc_request(&request);
        rpc_request.id = json!(null);

        // Send the request but don't wait for response
        let _response = self.send_with_retry(rpc_request).await?;
        Ok(())
    }

    async fn get_connection_status(&self) -> MultivmResult<ConnectionStatus> {
        Ok(self.connection_status.read().await.clone())
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        let health_method = self.get_default_health_method();
        let health_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: health_method,
            params: json!([]),
            timeout: Some(self.config.health_check_config.timeout),
            auth_context: None,
        };

        let response = self.send_request(health_request).await?;

        let is_healthy = response.error.is_none();
        let mut capabilities = vec!["json-rpc".to_string()];
        let mut metrics = HashMap::new();

        // Add protocol-specific capabilities
        match self.engine_type {
            EngineType::Ethereum => {
                capabilities.extend_from_slice(&[
                    "eth_chainId".to_string(),
                    "eth_blockNumber".to_string(),
                    "eth_getBalance".to_string(),
                    "eth_sendTransaction".to_string(),
                ]);
            }
            EngineType::Solana => {
                capabilities.extend_from_slice(&[
                    "getVersion".to_string(),
                    "getSlot".to_string(),
                    "getBalance".to_string(),
                    "sendTransaction".to_string(),
                ]);
            }
        }

        // Add metrics from connection status
        let status = self.connection_status.read().await;
        metrics.insert("success_count".to_string(), json!(status.success_count));
        metrics.insert("error_count".to_string(), json!(status.error_count));
        metrics.insert("endpoint_url".to_string(), json!(self.config.endpoint_url));

        if let Some(latency) = status.latency_ms {
            metrics.insert("latency_ms".to_string(), json!(latency));
        }

        // Try to extract version from the health check response
        let version = response
            .result
            .as_ref()
            .and_then(|r| match self.engine_type {
                EngineType::Ethereum => r.as_str().map(|s| format!("Chain ID: {s}")),
                EngineType::Solana => r
                    .get("solana-core")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });

        Ok(HealthStatus {
            is_healthy,
            version,
            capabilities,
            metrics,
            timestamp: SystemTime::now(),
        })
    }

    async fn subscribe(&self, event_types: Vec<String>) -> MultivmResult<()> {
        debug!("Subscribing to events: {:?}", event_types);

        // Different engines have different subscription methods
        let method = match self.engine_type {
            EngineType::Ethereum => "eth_subscribe",
            EngineType::Solana => "logsSubscribe", // or other Solana subscription methods
        };

        let subscribe_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: method.to_string(),
            params: json!(event_types),
            timeout: Some(self.config.request_timeout),
            auth_context: None,
        };

        self.send_request(subscribe_request).await?;
        Ok(())
    }

    async fn unsubscribe(&self, event_types: Vec<String>) -> MultivmResult<()> {
        debug!("Unsubscribing from events: {:?}", event_types);

        let method = match self.engine_type {
            EngineType::Ethereum => "eth_unsubscribe",
            EngineType::Solana => "logsUnsubscribe",
        };

        let unsubscribe_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: method.to_string(),
            params: json!(event_types),
            timeout: Some(self.config.request_timeout),
            auth_context: None,
        };

        self.send_request(unsubscribe_request).await?;
        Ok(())
    }

    fn get_configuration(&self) -> serde_json::Value {
        serde_json::to_value(&self.config).unwrap_or(serde_json::Value::Null)
    }

    async fn update_configuration(&mut self, config: serde_json::Value) -> MultivmResult<()> {
        let new_config: RpcProtocolConfig =
            serde_json::from_value(config).map_err(|e| MultivmError::Configuration {
                component: "rpc_protocol".to_string(),
                message: format!("Invalid configuration: {e}"),
                validation_errors: None,
            })?;

        // Rebuild HTTP client if necessary
        let new_client = Self::build_http_client(&new_config).await?;

        self.config = new_config;
        self.client = new_client;

        info!("RPC protocol configuration updated");
        Ok(())
    }
}
