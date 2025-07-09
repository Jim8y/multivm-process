//! JWT (JSON Web Token) authentication protocol implementation
//!
//! This module implements authenticated communication with execution engines
//! using JWT tokens for secure API access.

use super::{
    AuthContext, CommunicationError, CommunicationProtocol, CommunicationRequest,
    CommunicationResponse, ConnectionStatus, EngineType, HealthStatus, ProtocolType,
};
use crate::{MultivmError, MultivmResult};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for JWT protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtProtocolConfig {
    /// API endpoint URL
    pub endpoint_url: String,
    /// JWT secret for token generation/validation
    pub jwt_secret: String,
    /// Token expiration time
    pub token_expiry: Duration,
    /// Token refresh threshold (refresh when remaining time < threshold)
    pub refresh_threshold: Duration,
    /// HTTP client configuration
    pub http_config: HttpClientConfig,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Health check configuration
    pub health_check_config: HealthCheckConfig,
    /// API key for fallback authentication
    pub api_key: Option<String>,
    /// Additional authentication headers
    pub auth_headers: HashMap<String, String>,
}

/// HTTP client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpClientConfig {
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// User agent string
    pub user_agent: String,
    /// Whether to use HTTP/2
    pub use_http2: bool,
    /// Accept invalid certificates (for testing)
    pub accept_invalid_certs: bool,
    /// Default headers
    pub default_headers: HashMap<String, String>,
}

/// Retry configuration
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
    /// Whether to retry on authentication failures
    pub retry_auth_failures: bool,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check endpoint path
    pub health_endpoint: String,
    /// Health check interval
    pub interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Enable automatic health checks
    pub enable_auto_check: bool,
}

impl Default for JwtProtocolConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            jwt_secret: "default_secret_change_in_production".to_string(),
            token_expiry: Duration::from_secs(3600), // 1 hour
            refresh_threshold: Duration::from_secs(300), // 5 minutes
            http_config: HttpClientConfig::default(),
            retry_config: RetryConfig::default(),
            health_check_config: HealthCheckConfig::default(),
            api_key: None,
            auth_headers: HashMap::new(),
        }
    }
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            user_agent: "MultiVM-JWT-Client/1.0".to_string(),
            use_http2: true,
            accept_invalid_certs: false,
            default_headers: HashMap::new(),
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
            retry_auth_failures: true,
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            health_endpoint: "/health".to_string(),
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            enable_auto_check: true,
        }
    }
}

/// JWT token information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TokenInfo {
    /// The JWT token string
    token: String,
    /// Token expiration time
    expires_at: SystemTime,
    /// Token creation time
    created_at: SystemTime,
}

impl TokenInfo {
    /// Check if token needs refresh
    fn needs_refresh(&self, threshold: Duration) -> bool {
        match self.expires_at.duration_since(SystemTime::now()) {
            Ok(remaining) => remaining < threshold,
            Err(_) => true, // Token already expired
        }
    }

    /// Check if token is expired
    fn is_expired(&self) -> bool {
        SystemTime::now() >= self.expires_at
    }
}

/// JWT protocol implementation
pub struct JwtProtocol {
    /// Engine type this protocol communicates with
    engine_type: EngineType,
    /// Protocol configuration
    config: JwtProtocolConfig,
    /// HTTP client
    client: Client,
    /// Current JWT token
    current_token: Arc<RwLock<Option<TokenInfo>>>,
    /// Connection status
    connection_status: Arc<RwLock<ConnectionStatus>>,
    /// Health check task handle
    health_check_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl JwtProtocol {
    /// Create a new JWT protocol instance
    pub async fn new(engine_type: EngineType, config: JwtProtocolConfig) -> MultivmResult<Self> {
        // Validate JWT secret
        if config.jwt_secret == "default_secret_change_in_production" {
            warn!("Using default JWT secret - this is insecure for production!");
        }

        // Only validate JWT secret length if it's not empty (empty means using API key auth)
        if !config.jwt_secret.is_empty() && config.jwt_secret.len() < 32 {
            return Err(MultivmError::Configuration {
                component: "jwt_protocol".to_string(),
                message: "JWT secret must be at least 32 characters for security".to_string(),
                validation_errors: Some(vec!["jwt_secret".to_string()]),
            });
        }

        let client = Self::build_http_client(&config.http_config).await?;

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
            current_token: Arc::new(RwLock::new(None)),
            connection_status: Arc::new(RwLock::new(connection_status)),
            health_check_handle: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Build HTTP client with configuration
    async fn build_http_client(config: &HttpClientConfig) -> MultivmResult<Client> {
        let mut builder = ClientBuilder::new()
            .timeout(config.request_timeout)
            .connect_timeout(config.connect_timeout)
            .user_agent(&config.user_agent);

        if config.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }

        // Configure connection pooling
        builder = builder
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90));

        builder.build().map_err(|e| MultivmError::Configuration {
            component: "jwt_protocol".to_string(),
            message: format!("Failed to build HTTP client: {e}"),
            validation_errors: None,
        })
    }

    /// Generate a new JWT token
    async fn generate_token(&self) -> MultivmResult<TokenInfo> {
        let now = SystemTime::now();
        let expires_at = now + self.config.token_expiry;

        // JWT header
        let header = serde_json::json!({
            "alg": "HS256",
            "typ": "JWT"
        });

        // JWT payload
        let payload = serde_json::json!({
            "iss": "multivm",
            "sub": format!("{}_engine", self.engine_type.to_string().to_lowercase()),
            "aud": self.config.endpoint_url,
            "exp": expires_at.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "iat": now.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "jti": Uuid::new_v4().to_string(),
            "engine_type": self.engine_type.to_string(),
        });

        // Encode header and payload
        let header_b64 = BASE64.encode(serde_json::to_string(&header).unwrap());
        let payload_b64 = BASE64.encode(serde_json::to_string(&payload).unwrap());

        // Create signature
        let message = format!("{header_b64}.{payload_b64}");
        let signature = self.create_hmac_signature(&message)?;
        let signature_b64 = BASE64.encode(signature);

        // Combine into final JWT
        let token = format!("{header_b64}.{payload_b64}.{signature_b64}");

        Ok(TokenInfo {
            token,
            expires_at,
            created_at: now,
        })
    }

    /// Create HMAC signature
    fn create_hmac_signature(&self, message: &str) -> MultivmResult<Vec<u8>> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.config.jwt_secret.as_bytes()).map_err(|e| {
                MultivmError::AuthenticationFailed {
                    reason: format!("Failed to create HMAC: {e}"),
                    user_id: None,
                    required_permissions: None,
                }
            })?;

        mac.update(message.as_bytes());
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Get valid JWT token (refresh if necessary)
    async fn get_valid_token(&self) -> MultivmResult<String> {
        let token_guard = self.current_token.read().await;

        // Check if we have a valid token
        if let Some(token_info) = token_guard.as_ref() {
            if !token_info.needs_refresh(self.config.refresh_threshold) {
                return Ok(token_info.token.clone());
            }
        }

        drop(token_guard);

        // Need to generate new token
        let new_token = self.generate_token().await?;
        let token_string = new_token.token.clone();

        *self.current_token.write().await = Some(new_token);

        debug!("Generated new JWT token for {} engine", self.engine_type);
        Ok(token_string)
    }

    /// Prepare authenticated request headers
    async fn prepare_headers(
        &self,
        auth_context: Option<&AuthContext>,
    ) -> MultivmResult<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();

        // Add content type
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        // Add authentication
        if let Some(auth) = auth_context {
            // Use provided auth context
            if let Some(token) = &auth.token {
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(
                        |e| MultivmError::AuthenticationFailed {
                            reason: format!("Invalid authorization header: {e}"),
                            user_id: None,
                            required_permissions: None,
                        },
                    )?,
                );
            } else if let Some(api_key) = &auth.api_key {
                headers.insert(
                    reqwest::header::HeaderName::from_static("x-api-key"),
                    reqwest::header::HeaderValue::from_str(api_key).map_err(|e| {
                        MultivmError::AuthenticationFailed {
                            reason: format!("Invalid API key header: {e}"),
                            user_id: None,
                            required_permissions: None,
                        }
                    })?,
                );
            }

            // Add custom headers
            for (key, value) in &auth.headers {
                if let (Ok(header_name), Ok(header_value)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    reqwest::header::HeaderValue::from_str(value),
                ) {
                    headers.insert(header_name, header_value);
                }
            }
        } else {
            // Use JWT token or API key
            if !self.config.jwt_secret.is_empty() {
                let token = self.get_valid_token().await?;
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(
                        |e| MultivmError::AuthenticationFailed {
                            reason: format!("Invalid authorization header: {e}"),
                            user_id: None,
                            required_permissions: None,
                        },
                    )?,
                );
            } else if let Some(api_key) = &self.config.api_key {
                headers.insert(
                    reqwest::header::HeaderName::from_static("x-api-key"),
                    reqwest::header::HeaderValue::from_str(api_key).map_err(|e| {
                        MultivmError::AuthenticationFailed {
                            reason: format!("Invalid API key header: {e}"),
                            user_id: None,
                            required_permissions: None,
                        }
                    })?,
                );
            }
        }

        // Add default headers
        for (key, value) in &self.config.http_config.default_headers {
            if let (Ok(header_name), Ok(header_value)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                headers.insert(header_name, header_value);
            }
        }

        // Add configured auth headers
        for (key, value) in &self.config.auth_headers {
            if let (Ok(header_name), Ok(header_value)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                headers.insert(header_name, header_value);
            }
        }

        Ok(headers)
    }

    /// Send HTTP request with retry logic
    async fn send_with_retry(
        &self,
        request: &CommunicationRequest,
    ) -> MultivmResult<serde_json::Value> {
        let retry_config = &self.config.retry_config;
        let mut delay = retry_config.base_delay;
        let mut last_error = None;
        let mut auth_failure_count = 0;

        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                debug!("Retry attempt {} for JWT request {}", attempt, request.id);
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(
                    Duration::from_millis(
                        (delay.as_millis() as f64 * retry_config.backoff_multiplier) as u64,
                    ),
                    retry_config.max_delay,
                );
            }

            let start_time = std::time::Instant::now();

            // Prepare headers (this might refresh the token)
            let headers = match self.prepare_headers(request.auth_context.as_ref()).await {
                Ok(h) => h,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            // Construct the URL
            let url = if request.method.starts_with('/') {
                format!("{}{}", self.config.endpoint_url, request.method)
            } else {
                format!(
                    "{}/{}",
                    self.config.endpoint_url.trim_end_matches('/'),
                    request.method
                )
            };

            // Make the request
            let response = self
                .client
                .post(&url)
                .headers(headers)
                .json(&request.params)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let latency = start_time.elapsed().as_millis() as u64;
                    let status_code = resp.status();

                    if status_code.is_success() {
                        match resp.json::<serde_json::Value>().await {
                            Ok(response_data) => {
                                // Update connection status on success
                                let mut status = self.connection_status.write().await;
                                status.last_success = Some(SystemTime::now());
                                status.success_count += 1;
                                status.is_connected = true;
                                status.latency_ms = Some(latency);

                                return Ok(response_data);
                            }
                            Err(e) => {
                                last_error = Some(MultivmError::Network {
                                    message: format!("Failed to parse JSON response: {e}"),
                                    endpoint: Some("jwt_protocol".to_string()),
                                    retry_after: None,
                                });
                            }
                        }
                    } else if status_code == reqwest::StatusCode::UNAUTHORIZED {
                        auth_failure_count += 1;

                        if retry_config.retry_auth_failures && auth_failure_count <= 2 {
                            // Clear current token to force refresh
                            *self.current_token.write().await = None;

                            let error_text = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "Unauthorized".to_string());
                            warn!(
                                "Authentication failed, will retry with new token: {}",
                                error_text
                            );

                            last_error = Some(MultivmError::AuthenticationFailed {
                                reason: format!("HTTP 401: {error_text}"),
                                user_id: None,
                                required_permissions: None,
                            });
                            continue;
                        } else {
                            let error_text = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "Unauthorized".to_string());
                            return Err(MultivmError::AuthenticationFailed {
                                reason: format!(
                                    "Authentication failed after retries: {error_text}"
                                ),
                                user_id: None,
                                required_permissions: None,
                            });
                        }
                    } else {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());
                        last_error = Some(MultivmError::Network {
                            message: format!("HTTP error {status_code}: {error_text}"),
                            endpoint: Some("jwt_protocol".to_string()),
                            retry_after: None,
                        });
                    }
                }
                Err(e) => {
                    last_error = Some(MultivmError::Network {
                        message: format!("HTTP request failed: {e}"),
                        endpoint: Some("jwt_protocol".to_string()),
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
            message: "All JWT retry attempts exhausted".to_string(),
            endpoint: Some("jwt_protocol".to_string()),
            retry_after: None,
        }))
    }

    /// Start automatic health check task
    async fn start_health_check_task(&self) {
        if !self.config.health_check_config.enable_auto_check {
            return;
        }

        let config = self.config.clone();
        let client = self.client.clone();
        let status = Arc::clone(&self.connection_status);
        let current_token = Arc::clone(&self.current_token);

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_config.interval);

            loop {
                interval.tick().await;

                // Create a simple health check request
                let health_url = format!(
                    "{}{}",
                    config.endpoint_url, config.health_check_config.health_endpoint
                );

                let start_time = std::time::Instant::now();

                // Get current token if available
                let token_guard = current_token.read().await;
                let auth_header = if let Some(token_info) = token_guard.as_ref() {
                    if !token_info.is_expired() {
                        Some(format!("Bearer {}", token_info.token))
                    } else {
                        None
                    }
                } else {
                    None
                };
                drop(token_guard);

                let mut headers = reqwest::header::HeaderMap::new();
                if let Some(auth) = auth_header {
                    if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&auth) {
                        headers.insert(reqwest::header::AUTHORIZATION, header_value);
                    }
                }

                let request_result = client
                    .get(&health_url)
                    .headers(headers)
                    .timeout(config.health_check_config.timeout)
                    .send()
                    .await;

                match request_result {
                    Ok(response) if response.status().is_success() => {
                        let latency = start_time.elapsed().as_millis() as u64;
                        let mut status_guard = status.write().await;
                        status_guard.is_connected = true;
                        status_guard.latency_ms = Some(latency);
                        status_guard.last_success = Some(SystemTime::now());
                        debug!("JWT health check successful, latency: {}ms", latency);
                    }
                    _ => {
                        warn!("JWT health check failed");
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
}

#[async_trait]
impl CommunicationProtocol for JwtProtocol {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Jwt
    }

    fn engine_type(&self) -> EngineType {
        self.engine_type.clone()
    }

    async fn connect(&mut self) -> MultivmResult<()> {
        info!(
            "Connecting JWT protocol to {} engine at {}",
            self.engine_type, self.config.endpoint_url
        );

        // Generate initial token
        let _token = self.get_valid_token().await?;

        // Test connection with a health check
        let health_url = format!(
            "{}{}",
            self.config.endpoint_url, self.config.health_check_config.health_endpoint
        );

        let headers = self.prepare_headers(None).await?;

        match self
            .client
            .get(&health_url)
            .headers(headers)
            .timeout(self.config.health_check_config.timeout)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                info!("JWT protocol connected successfully");

                // Start health check task
                self.start_health_check_task().await;

                // Update connection status
                let mut status = self.connection_status.write().await;
                status.is_connected = true;
                status.last_success = Some(SystemTime::now());

                Ok(())
            }
            Ok(response) => {
                let status_code = response.status();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(MultivmError::Network {
                    message: format!("Connection test failed: HTTP {status_code}: {error_text}"),
                    endpoint: Some(self.config.endpoint_url.clone()),
                    retry_after: None,
                })
            }
            Err(e) => {
                error!("Failed to connect JWT protocol: {}", e);
                Err(MultivmError::Network {
                    message: format!("Connection failed: {e}"),
                    endpoint: Some(self.config.endpoint_url.clone()),
                    retry_after: None,
                })
            }
        }
    }

    async fn disconnect(&mut self) -> MultivmResult<()> {
        info!(
            "Disconnecting JWT protocol from {} engine",
            self.engine_type
        );

        // Stop health check task
        self.stop_health_check_task().await;

        // Clear current token
        *self.current_token.write().await = None;

        // Update connection status
        {
            let mut status = self.connection_status.write().await;
            status.is_connected = false;
        }

        info!("JWT protocol disconnected");
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
            "Sending JWT request: {} to {}",
            request.method, self.engine_type
        );

        let request_id = request.id.clone();
        let result = self.send_with_retry(&request).await;

        let (response_result, error) = match result {
            Ok(data) => (Some(data), None),
            Err(e) => {
                let error = CommunicationError {
                    code: -1,
                    message: e.to_string(),
                    data: None,
                };
                (None, Some(error))
            }
        };

        let mut metadata = HashMap::new();
        metadata.insert("protocol".to_string(), "jwt".to_string());
        metadata.insert("endpoint".to_string(), self.config.endpoint_url.clone());

        let response = CommunicationResponse {
            request_id,
            result: response_result,
            error,
            metadata,
        };

        debug!("JWT request completed");
        Ok(response)
    }

    async fn send_notification(&self, request: CommunicationRequest) -> MultivmResult<()> {
        debug!(
            "Sending JWT notification: {} to {}",
            request.method, self.engine_type
        );

        // For notifications, we send the request but don't care about the response
        self.send_with_retry(&request).await?;
        Ok(())
    }

    async fn get_connection_status(&self) -> MultivmResult<ConnectionStatus> {
        Ok(self.connection_status.read().await.clone())
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        let health_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: self.config.health_check_config.health_endpoint.clone(),
            params: serde_json::Value::Null,
            timeout: Some(self.config.health_check_config.timeout),
            auth_context: None,
        };

        let response = self.send_request(health_request).await?;

        let is_healthy = response.error.is_none();
        let capabilities = vec![
            "jwt".to_string(),
            "http".to_string(),
            "authenticated".to_string(),
        ];

        let mut metrics = HashMap::new();
        let status = self.connection_status.read().await;
        metrics.insert("success_count".to_string(), json!(status.success_count));
        metrics.insert("error_count".to_string(), json!(status.error_count));
        metrics.insert("endpoint_url".to_string(), json!(self.config.endpoint_url));

        if let Some(latency) = status.latency_ms {
            metrics.insert("latency_ms".to_string(), json!(latency));
        }

        // Add token information
        let token_guard = self.current_token.read().await;
        if let Some(token_info) = token_guard.as_ref() {
            let remaining = token_info
                .expires_at
                .duration_since(SystemTime::now())
                .unwrap_or_default()
                .as_secs();
            metrics.insert("token_expires_in_seconds".to_string(), json!(remaining));
            metrics.insert(
                "token_needs_refresh".to_string(),
                json!(token_info.needs_refresh(self.config.refresh_threshold)),
            );
        }

        Ok(HealthStatus {
            is_healthy,
            version: None,
            capabilities,
            metrics,
            timestamp: SystemTime::now(),
        })
    }

    async fn subscribe(&self, event_types: Vec<String>) -> MultivmResult<()> {
        debug!("Subscribing to events: {:?}", event_types);

        let subscribe_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/subscribe".to_string(),
            params: json!({ "event_types": event_types }),
            timeout: Some(self.config.http_config.request_timeout),
            auth_context: None,
        };

        self.send_request(subscribe_request).await?;
        Ok(())
    }

    async fn unsubscribe(&self, event_types: Vec<String>) -> MultivmResult<()> {
        debug!("Unsubscribing from events: {:?}", event_types);

        let unsubscribe_request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/unsubscribe".to_string(),
            params: json!({ "event_types": event_types }),
            timeout: Some(self.config.http_config.request_timeout),
            auth_context: None,
        };

        self.send_request(unsubscribe_request).await?;
        Ok(())
    }

    fn get_configuration(&self) -> serde_json::Value {
        // Don't expose sensitive information like JWT secret
        let mut config = serde_json::to_value(&self.config).unwrap_or(serde_json::Value::Null);
        if let Some(config_obj) = config.as_object_mut() {
            config_obj.remove("jwt_secret");
            config_obj.remove("api_key");
        }
        config
    }

    async fn update_configuration(&mut self, config: serde_json::Value) -> MultivmResult<()> {
        let new_config: JwtProtocolConfig =
            serde_json::from_value(config).map_err(|e| MultivmError::Configuration {
                component: "jwt_protocol".to_string(),
                message: format!("Invalid configuration: {e}"),
                validation_errors: None,
            })?;

        // Rebuild HTTP client if necessary
        let new_client = Self::build_http_client(&new_config.http_config).await?;

        self.config = new_config;
        self.client = new_client;

        // Clear current token to force regeneration with new config
        *self.current_token.write().await = None;

        info!("JWT protocol configuration updated");
        Ok(())
    }
}
