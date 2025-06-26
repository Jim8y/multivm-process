//! Engine API client implementation
//!
//! This module provides a comprehensive client for the Ethereum Engine API,
//! implementing the latest specifications including engine_newPayloadV3,
//! engine_forkchoiceUpdatedV3, and engine_getPayloadV3.

use crate::engine::RethEngineError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Engine API client for communicating with Reth nodes
pub struct EngineApiClient {
    client: Client,
    engine_url: String,
    jwt_secret: Arc<RwLock<Option<String>>>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

/// Engine API payload status
#[derive(Debug, Clone, PartialEq)]
pub enum PayloadStatus {
    Valid,
    Invalid,
    Syncing,
    Accepted,
    InvalidBlockHash,
    InvalidTerminalBlock,
}

impl PayloadStatus {
    fn from_str(s: &str) -> Result<Self, RethEngineError> {
        match s {
            "VALID" => Ok(PayloadStatus::Valid),
            "INVALID" => Ok(PayloadStatus::Invalid),
            "SYNCING" => Ok(PayloadStatus::Syncing),
            "ACCEPTED" => Ok(PayloadStatus::Accepted),
            "INVALID_BLOCK_HASH" => Ok(PayloadStatus::InvalidBlockHash),
            "INVALID_TERMINAL_BLOCK" => Ok(PayloadStatus::InvalidTerminalBlock),
            _ => Err(RethEngineError::Rpc(format!("Unknown payload status: {}", s))),
        }
    }
}

/// Engine API response for newPayload
#[derive(Debug, Clone)]
pub struct NewPayloadResponse {
    pub status: PayloadStatus,
    pub latest_valid_hash: Option<String>,
    pub validation_error: Option<String>,
}

/// Engine API response for forkchoiceUpdated
#[derive(Debug, Clone)]
pub struct ForkchoiceUpdatedResponse {
    pub payload_status: PayloadStatus,
    pub payload_id: Option<String>,
    pub latest_valid_hash: Option<String>,
    pub validation_error: Option<String>,
}

/// Engine API response for getPayload
#[derive(Debug, Clone)]
pub struct GetPayloadResponse {
    pub execution_payload: Value,
    pub block_value: Option<String>,
    pub blobs_bundle: Option<Value>,
    pub should_override_builder: Option<bool>,
}

impl EngineApiClient {
    /// Create a new Engine API client
    pub fn new(
        engine_url: String,
        jwt_secret: Arc<RwLock<Option<String>>>,
        request_timeout: Duration,
        max_retries: u32,
        retry_delay: Duration,
    ) -> Result<Self, RethEngineError> {
        let client = Client::builder()
            .timeout(request_timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(|e| RethEngineError::Rpc(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            engine_url,
            jwt_secret,
            request_timeout,
            max_retries,
            retry_delay,
        })
    }

    /// Submit a new payload via engine_newPayloadV3
    pub async fn new_payload_v3(
        &self,
        execution_payload: &Value,
        expected_blob_versioned_hashes: &[String],
        parent_beacon_block_root: &str,
    ) -> Result<NewPayloadResponse, RethEngineError> {
        let method = "engine_newPayloadV3";
        let params = json!([execution_payload, expected_blob_versioned_hashes, parent_beacon_block_root]);

        let response = self.make_authenticated_request(method, params).await?;
        self.parse_new_payload_response(response)
    }

    /// Submit a new payload via engine_newPayloadV2 (fallback)
    pub async fn new_payload_v2(
        &self,
        execution_payload: &Value,
    ) -> Result<NewPayloadResponse, RethEngineError> {
        let method = "engine_newPayloadV2";
        let params = json!([execution_payload]);

        let response = self.make_authenticated_request(method, params).await?;
        self.parse_new_payload_response(response)
    }

    /// Update fork choice via engine_forkchoiceUpdatedV3
    pub async fn forkchoice_updated_v3(
        &self,
        forkchoice_state: &Value,
        payload_attributes: Option<&Value>,
    ) -> Result<ForkchoiceUpdatedResponse, RethEngineError> {
        let method = "engine_forkchoiceUpdatedV3";
        let params = json!([forkchoice_state, payload_attributes]);

        let response = self.make_authenticated_request(method, params).await?;
        self.parse_forkchoice_updated_response(response)
    }

    /// Update fork choice via engine_forkchoiceUpdatedV2 (fallback)
    pub async fn forkchoice_updated_v2(
        &self,
        forkchoice_state: &Value,
        payload_attributes: Option<&Value>,
    ) -> Result<ForkchoiceUpdatedResponse, RethEngineError> {
        let method = "engine_forkchoiceUpdatedV2";
        let params = json!([forkchoice_state, payload_attributes]);

        let response = self.make_authenticated_request(method, params).await?;
        self.parse_forkchoice_updated_response(response)
    }

    /// Get payload via engine_getPayloadV3
    pub async fn get_payload_v3(&self, payload_id: &str) -> Result<GetPayloadResponse, RethEngineError> {
        let method = "engine_getPayloadV3";
        let params = json!([payload_id]);

        let response = self.make_authenticated_request(method, params).await?;
        self.parse_get_payload_response(response)
    }

    /// Get payload via engine_getPayloadV2 (fallback)
    pub async fn get_payload_v2(&self, payload_id: &str) -> Result<GetPayloadResponse, RethEngineError> {
        let method = "engine_getPayloadV2";
        let params = json!([payload_id]);

        let response = self.make_authenticated_request(method, params).await?;
        self.parse_get_payload_response(response)
    }

    /// Exchange capabilities with the Engine API
    pub async fn exchange_capabilities(&self, capabilities: &[String]) -> Result<Vec<String>, RethEngineError> {
        let method = "engine_exchangeCapabilities";
        let params = json!([capabilities]);

        let response = self.make_authenticated_request(method, params).await?;
        
        if let Some(result) = response.get("result") {
            if let Some(caps) = result.as_array() {
                let capabilities: Result<Vec<String>, _> = caps
                    .iter()
                    .map(|v| v.as_str().ok_or_else(|| {
                        RethEngineError::Rpc("Invalid capability format".to_string())
                    }).map(|s| s.to_string()))
                    .collect();
                
                return capabilities.map_err(|e| e);
            }
        }

        Err(RethEngineError::Rpc("Invalid capabilities response".to_string()))
    }

    /// Make an authenticated request to the Engine API
    async fn make_authenticated_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, RethEngineError> {
        let jwt_secret = self.jwt_secret.read().await
            .as_ref()
            .ok_or_else(|| RethEngineError::Configuration("JWT secret not loaded".to_string()))?
            .clone();

        for attempt in 1..=self.max_retries {
            // Create fresh JWT token for each attempt
            let jwt_token = self.create_jwt_token(&jwt_secret)?;

            let request_id = format!("{}_{}", method, attempt);
            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params
            });

            debug!("Engine API request: {} (attempt {})", method, attempt);

            match self.client
                .post(&self.engine_url)
                .header("Authorization", format!("Bearer {}", jwt_token))
                .header("Content-Type", "application/json")
                .json(&rpc_request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    match response.json::<Value>().await {
                        Ok(result) => {
                            debug!("Engine API response: {} succeeded", method);
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Failed to parse Engine API response on attempt {}: {}", attempt, e);
                            if attempt == self.max_retries {
                                return Err(RethEngineError::Rpc(format!(
                                    "Failed to parse Engine API response: {}", e
                                )));
                            }
                        }
                    }
                }
                Ok(response) => {
                    let status_code = response.status();
                    warn!("Engine API returned error status on attempt {}: {}", attempt, status_code);
                    
                    // Try to get error details
                    if let Ok(error_body) = response.text().await {
                        warn!("Engine API error body: {}", error_body);
                    }
                    
                    if attempt == self.max_retries {
                        return Err(RethEngineError::Rpc(format!(
                            "Engine API returned error status: {}",
                            status_code
                        )));
                    }
                }
                Err(e) => {
                    warn!("Engine API request failed on attempt {}: {}", attempt, e);
                    if attempt == self.max_retries {
                        return Err(RethEngineError::Rpc(format!("Engine API request failed: {}", e)));
                    }
                }
            }

            // Wait before retry
            if attempt < self.max_retries {
                tokio::time::sleep(self.retry_delay).await;
            }
        }

        unreachable!()
    }

    /// Parse newPayload response
    fn parse_new_payload_response(&self, response: Value) -> Result<NewPayloadResponse, RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API newPayload error: {}",
                error
            )));
        }

        let result = response.get("result")
            .ok_or_else(|| RethEngineError::Rpc("Missing result in newPayload response".to_string()))?;

        let status_str = result.get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing status in newPayload response".to_string()))?;

        let status = PayloadStatus::from_str(status_str)?;

        let latest_valid_hash = result.get("latestValidHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string());

        let validation_error = result.get("validationError")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());

        Ok(NewPayloadResponse {
            status,
            latest_valid_hash,
            validation_error,
        })
    }

    /// Parse forkchoiceUpdated response
    fn parse_forkchoice_updated_response(&self, response: Value) -> Result<ForkchoiceUpdatedResponse, RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API forkchoiceUpdated error: {}",
                error
            )));
        }

        let result = response.get("result")
            .ok_or_else(|| RethEngineError::Rpc("Missing result in forkchoiceUpdated response".to_string()))?;

        let payload_status_json = result.get("payloadStatus")
            .ok_or_else(|| RethEngineError::Rpc("Missing payloadStatus in forkchoiceUpdated response".to_string()))?;

        let status_str = payload_status_json.get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing status in payloadStatus".to_string()))?;

        let payload_status = PayloadStatus::from_str(status_str)?;

        let payload_id = result.get("payloadId")
            .and_then(|id| id.as_str())
            .map(|s| s.to_string());

        let latest_valid_hash = payload_status_json.get("latestValidHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string());

        let validation_error = payload_status_json.get("validationError")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());

        Ok(ForkchoiceUpdatedResponse {
            payload_status,
            payload_id,
            latest_valid_hash,
            validation_error,
        })
    }

    /// Parse getPayload response
    fn parse_get_payload_response(&self, response: Value) -> Result<GetPayloadResponse, RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API getPayload error: {}",
                error
            )));
        }

        let result = response.get("result")
            .ok_or_else(|| RethEngineError::Rpc("Missing result in getPayload response".to_string()))?;

        let execution_payload = result.get("executionPayload")
            .ok_or_else(|| RethEngineError::Rpc("Missing executionPayload in getPayload response".to_string()))?
            .clone();

        let block_value = result.get("blockValue")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let blobs_bundle = result.get("blobsBundle").cloned();

        let should_override_builder = result.get("shouldOverrideBuilder")
            .and_then(|b| b.as_bool());

        Ok(GetPayloadResponse {
            execution_payload,
            block_value,
            blobs_bundle,
            should_override_builder,
        })
    }

    /// Create JWT token for Engine API authentication
    fn create_jwt_token(&self, secret: &str) -> Result<String, RethEngineError> {
        use sha2::{Digest, Sha256};

        // Create JWT header
        let header = json!({
            "alg": "HS256",
            "typ": "JWT"
        });

        // Create JWT payload with current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RethEngineError::Configuration(format!("System time error: {}", e)))?
            .as_secs();

        let payload = json!({
            "iat": now,
            "exp": now + 60 // Token expires in 60 seconds
        });

        // Encode header and payload
        let header_b64 = URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&header)
                .map_err(|e| RethEngineError::Configuration(format!("Failed to serialize header: {}", e)))?
                .as_bytes(),
        );

        let payload_b64 = URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&payload)
                .map_err(|e| RethEngineError::Configuration(format!("Failed to serialize payload: {}", e)))?
                .as_bytes(),
        );

        // Create signature
        let message = format!("{}.{}", header_b64, payload_b64);
        let secret_bytes = hex::decode(secret).map_err(|e| {
            RethEngineError::Configuration(format!("Invalid JWT secret format: {}", e))
        })?;

        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(&secret_bytes).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to create HMAC: {}", e))
        })?;

        use hmac::Mac;
        mac.update(message.as_bytes());
        let signature = mac.finalize().into_bytes();

        let signature_b64 = URL_SAFE_NO_PAD.encode(&signature);

        // Combine into final JWT
        let jwt = format!("{}.{}.{}", header_b64, payload_b64, signature_b64);
        Ok(jwt)
    }

    /// Health check for Engine API
    pub async fn health_check(&self) -> Result<bool, RethEngineError> {
        match self.exchange_capabilities(&[]).await {
            Ok(_) => Ok(true),
            Err(e) => {
                warn!("Engine API health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get Engine API version information
    pub async fn get_version(&self) -> Result<String, RethEngineError> {
        // Try to determine version based on supported capabilities
        let capabilities = self.exchange_capabilities(&[
            "engine_newPayloadV3".to_string(),
            "engine_forkchoiceUpdatedV3".to_string(),
            "engine_getPayloadV3".to_string(),
        ]).await?;

        if capabilities.contains(&"engine_newPayloadV3".to_string()) {
            Ok("v3".to_string())
        } else if capabilities.contains(&"engine_newPayloadV2".to_string()) {
            Ok("v2".to_string())
        } else if capabilities.contains(&"engine_newPayloadV1".to_string()) {
            Ok("v1".to_string())
        } else {
            Ok("unknown".to_string())
        }
    }
}

/// Builder for EngineApiClient
pub struct EngineApiClientBuilder {
    engine_url: Option<String>,
    jwt_secret: Option<Arc<RwLock<Option<String>>>>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

impl EngineApiClientBuilder {
    pub fn new() -> Self {
        Self {
            engine_url: None,
            jwt_secret: None,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    pub fn engine_url(mut self, url: String) -> Self {
        self.engine_url = Some(url);
        self
    }

    pub fn jwt_secret(mut self, secret: Arc<RwLock<Option<String>>>) -> Self {
        self.jwt_secret = Some(secret);
        self
    }

    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    pub fn build(self) -> Result<EngineApiClient, RethEngineError> {
        let engine_url = self.engine_url
            .ok_or_else(|| RethEngineError::Configuration("Engine URL is required".to_string()))?;
        
        let jwt_secret = self.jwt_secret
            .ok_or_else(|| RethEngineError::Configuration("JWT secret is required".to_string()))?;

        EngineApiClient::new(
            engine_url,
            jwt_secret,
            self.request_timeout,
            self.max_retries,
            self.retry_delay,
        )
    }
}

impl Default for EngineApiClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}