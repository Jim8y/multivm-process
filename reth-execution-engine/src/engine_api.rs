//! Engine API client implementation
//!
//! This module provides a comprehensive client for the Ethereum Engine API,
//! implementing the latest specifications including engine_newPayloadV3,
//! engine_forkchoiceUpdatedV3, and engine_getPayloadV3.

use crate::engine::RethEngineError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// Enhanced retry configuration for Engine API calls
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub timeout: Duration,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            timeout: Duration::from_secs(60),
            jitter: true,
        }
    }
}

/// Engine API metrics and monitoring
#[derive(Debug, Default)]
pub struct EngineApiMetrics {
    pub requests_total: AtomicU64,
    pub requests_successful: AtomicU64,
    pub requests_failed: AtomicU64,
    pub retries_total: AtomicU64,
    pub average_response_time_ms: AtomicU64,
    pub payload_submissions: AtomicU64,
    pub forkchoice_updates: AtomicU64,
    pub payload_retrievals: AtomicU64,
    pub blob_bundles_processed: AtomicU64,
    pub withdrawals_processed: AtomicU64,
}

/// Enhanced Engine API client with advanced features
pub struct EngineApiClient {
    client: Client,
    engine_url: String,
    jwt_secret: Arc<RwLock<Option<String>>>,
    retry_config: RetryConfig,
    metrics: EngineApiMetrics,
    connection_semaphore: Arc<Semaphore>,
    /// JWT token cache to avoid regenerating tokens frequently
    jwt_token_cache: Arc<RwLock<Option<(String, Instant)>>>,
    /// JWT token expiry duration (default 60 seconds)
    jwt_expiry_duration: Duration,
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
            _ => Err(RethEngineError::Rpc(format!("Unknown payload status: {s}"))),
        }
    }
}

/// Enhanced newPayload response with detailed information
#[derive(Debug, Clone)]
pub struct NewPayloadResponse {
    pub status: PayloadStatus,
    pub latest_valid_hash: Option<String>,
    pub validation_error: Option<String>,
    pub processing_time: Duration,
    pub blob_gas_used: Option<u64>,
}

/// Enhanced forkchoiceUpdated response with withdrawal support
#[derive(Debug, Clone)]
pub struct ForkchoiceUpdatedResponse {
    pub payload_status: PayloadStatus,
    pub payload_id: Option<String>,
    pub latest_valid_hash: Option<String>,
    pub validation_error: Option<String>,
    pub processing_time: Duration,
    pub withdrawals_processed: u32,
}

/// Enhanced getPayload response with comprehensive blob bundle support
#[derive(Debug, Clone)]
pub struct GetPayloadResponse {
    pub execution_payload: Value,
    pub block_value: Option<String>,
    pub blobs_bundle: Option<BlobsBundle>,
    pub should_override_builder: Option<bool>,
    pub processing_time: Duration,
}

/// Comprehensive blob bundle representation
#[derive(Debug, Clone)]
pub struct BlobsBundle {
    pub commitments: Vec<String>,
    pub proofs: Vec<String>,
    pub blobs: Vec<String>,
    pub blob_count: usize,
    pub total_size_bytes: u64,
}

/// Withdrawal processing information
#[derive(Debug, Clone)]
pub struct WithdrawalRequest {
    pub index: u64,
    pub validator_index: u64,
    pub address: String,
    pub amount: u64,
}

/// Enhanced payload attributes with withdrawal support
#[derive(Debug, Clone)]
pub struct PayloadAttributes {
    pub timestamp: u64,
    pub prev_randao: String,
    pub suggested_fee_recipient: String,
    pub withdrawals: Option<Vec<WithdrawalRequest>>,
    pub parent_beacon_block_root: Option<String>,
}

impl EngineApiClient {
    /// Create a new enhanced Engine API client
    pub fn new(
        engine_url: String,
        jwt_secret: Arc<RwLock<Option<String>>>,
        retry_config: RetryConfig,
        max_concurrent_requests: usize,
    ) -> Result<Self, RethEngineError> {
        let client = Client::builder()
            .timeout(retry_config.timeout)
            .pool_max_idle_per_host(20)
            .pool_idle_timeout(Duration::from_secs(60))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .http2_prior_knowledge()
            .http2_keep_alive_interval(Duration::from_secs(30))
            .build()
            .map_err(|e| RethEngineError::Rpc(format!("Failed to create HTTP client: {e}")))?;

        info!(
            "Created Engine API client for {} with {} max concurrent requests",
            engine_url, max_concurrent_requests
        );

        Ok(Self {
            client,
            engine_url,
            jwt_secret,
            retry_config,
            metrics: EngineApiMetrics::default(),
            connection_semaphore: Arc::new(Semaphore::new(max_concurrent_requests)),
            jwt_token_cache: Arc::new(RwLock::new(None)),
            jwt_expiry_duration: Duration::from_secs(60),
        })
    }

    /// Submit a new payload via engine_newPayloadV3 with enhanced blob support
    pub async fn new_payload_v3(
        &self,
        execution_payload: &Value,
        expected_blob_versioned_hashes: &[String],
        parent_beacon_block_root: &str,
    ) -> Result<NewPayloadResponse, RethEngineError> {
        let start_time = Instant::now();
        self.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

        let method = "engine_newPayloadV3";
        let params = json!([
            execution_payload,
            expected_blob_versioned_hashes,
            parent_beacon_block_root
        ]);

        debug!(
            "Submitting newPayloadV3 with {} blob hashes",
            expected_blob_versioned_hashes.len()
        );

        let response = self.make_authenticated_request(method, params).await?;
        let mut payload_response = self.parse_new_payload_response(response)?;

        payload_response.processing_time = start_time.elapsed();
        payload_response.blob_gas_used = execution_payload
            .get("blobGasUsed")
            .and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok());

        self.metrics
            .payload_submissions
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .requests_successful
            .fetch_add(1, Ordering::Relaxed);

        // Update blob metrics if blobs were processed
        if !expected_blob_versioned_hashes.is_empty() {
            self.metrics
                .blob_bundles_processed
                .fetch_add(1, Ordering::Relaxed);
        }

        info!(
            "newPayloadV3 completed in {:?} with status: {:?}",
            payload_response.processing_time, payload_response.status
        );

        Ok(payload_response)
    }

    /// Submit a new payload via engine_newPayloadV2 (fallback)
    pub async fn new_payload_v2(
        &self,
        execution_payload: &Value,
    ) -> Result<NewPayloadResponse, RethEngineError> {
        let start_time = Instant::now();
        self.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

        let method = "engine_newPayloadV2";
        let params = json!([execution_payload]);

        let response = self.make_authenticated_request(method, params).await?;
        let mut payload_response = self.parse_new_payload_response(response)?;
        payload_response.processing_time = start_time.elapsed();

        self.metrics
            .payload_submissions
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .requests_successful
            .fetch_add(1, Ordering::Relaxed);

        Ok(payload_response)
    }

    /// Update fork choice via engine_forkchoiceUpdatedV3 with withdrawal processing
    pub async fn forkchoice_updated_v3(
        &self,
        forkchoice_state: &Value,
        payload_attributes: Option<&PayloadAttributes>,
    ) -> Result<ForkchoiceUpdatedResponse, RethEngineError> {
        let start_time = Instant::now();
        self.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

        let method = "engine_forkchoiceUpdatedV3";

        // Convert PayloadAttributes to JSON with withdrawal support
        let payload_attrs_json = if let Some(attrs) = payload_attributes {
            Some(self.serialize_payload_attributes(attrs)?)
        } else {
            None
        };

        let params = json!([forkchoice_state, payload_attrs_json]);

        // Log withdrawal information if present
        let withdrawal_count = payload_attributes
            .and_then(|attrs| attrs.withdrawals.as_ref())
            .map(|w| w.len())
            .unwrap_or(0);

        if withdrawal_count > 0 {
            info!(
                "Processing {} withdrawals in forkchoiceUpdatedV3",
                withdrawal_count
            );
        }

        let response = self.make_authenticated_request(method, params).await?;
        let mut fc_response = self.parse_forkchoice_updated_response(response)?;

        fc_response.processing_time = start_time.elapsed();
        fc_response.withdrawals_processed = withdrawal_count as u32;

        self.metrics
            .forkchoice_updates
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .requests_successful
            .fetch_add(1, Ordering::Relaxed);

        // Update withdrawal metrics
        if withdrawal_count > 0 {
            self.metrics
                .withdrawals_processed
                .fetch_add(withdrawal_count as u64, Ordering::Relaxed);
        }

        info!(
            "forkchoiceUpdatedV3 completed in {:?} with {} withdrawals processed",
            fc_response.processing_time, withdrawal_count
        );

        Ok(fc_response)
    }

    /// Update fork choice via engine_forkchoiceUpdatedV2 (fallback)
    pub async fn forkchoice_updated_v2(
        &self,
        forkchoice_state: &Value,
        payload_attributes: Option<&Value>,
    ) -> Result<ForkchoiceUpdatedResponse, RethEngineError> {
        let start_time = Instant::now();
        self.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

        let method = "engine_forkchoiceUpdatedV2";
        let params = json!([forkchoice_state, payload_attributes]);

        let response = self.make_authenticated_request(method, params).await?;
        let mut fc_response = self.parse_forkchoice_updated_response(response)?;
        fc_response.processing_time = start_time.elapsed();

        self.metrics
            .forkchoice_updates
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .requests_successful
            .fetch_add(1, Ordering::Relaxed);

        Ok(fc_response)
    }

    /// Get payload via engine_getPayloadV3 with comprehensive blob bundle handling
    pub async fn get_payload_v3(
        &self,
        payload_id: &str,
    ) -> Result<GetPayloadResponse, RethEngineError> {
        let start_time = Instant::now();
        self.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

        let method = "engine_getPayloadV3";
        let params = json!([payload_id]);

        debug!("Requesting payload with ID: {}", payload_id);

        let response = self.make_authenticated_request(method, params).await?;
        let mut payload_response = self.parse_get_payload_response(response)?;
        payload_response.processing_time = start_time.elapsed();

        self.metrics
            .payload_retrievals
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .requests_successful
            .fetch_add(1, Ordering::Relaxed);

        // Update blob bundle metrics
        if let Some(ref blobs_bundle) = payload_response.blobs_bundle {
            self.metrics
                .blob_bundles_processed
                .fetch_add(1, Ordering::Relaxed);
            info!(
                "Retrieved payload with {} blobs ({} bytes total)",
                blobs_bundle.blob_count, blobs_bundle.total_size_bytes
            );
        }

        info!(
            "getPayloadV3 completed in {:?} for payload {}",
            payload_response.processing_time, payload_id
        );

        Ok(payload_response)
    }

    /// Get payload via engine_getPayloadV2 (fallback)
    pub async fn get_payload_v2(
        &self,
        payload_id: &str,
    ) -> Result<GetPayloadResponse, RethEngineError> {
        let start_time = Instant::now();
        self.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

        let method = "engine_getPayloadV2";
        let params = json!([payload_id]);

        let response = self.make_authenticated_request(method, params).await?;
        let mut payload_response = self.parse_get_payload_response(response)?;
        payload_response.processing_time = start_time.elapsed();

        self.metrics
            .payload_retrievals
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .requests_successful
            .fetch_add(1, Ordering::Relaxed);

        Ok(payload_response)
    }

    /// Exchange capabilities with the Engine API
    pub async fn exchange_capabilities(
        &self,
        capabilities: &[String],
    ) -> Result<Vec<String>, RethEngineError> {
        let method = "engine_exchangeCapabilities";
        let params = json!([capabilities]);

        let response = self.make_authenticated_request(method, params).await?;

        if let Some(result) = response.get("result") {
            if let Some(caps) = result.as_array() {
                let capabilities: Result<Vec<String>, _> = caps
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| {
                                RethEngineError::Rpc("Invalid capability format".to_string())
                            })
                            .map(|s| s.to_string())
                    })
                    .collect();

                return capabilities;
            }
        }

        Err(RethEngineError::Rpc(
            "Invalid capabilities response".to_string(),
        ))
    }

    /// Enhanced authenticated request with intelligent retry logic
    async fn make_authenticated_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, RethEngineError> {
        // Acquire semaphore permit for connection limiting
        let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
            RethEngineError::Rpc("Connection semaphore acquisition failed".to_string())
        })?;

        let jwt_secret = self
            .jwt_secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| RethEngineError::Configuration("JWT secret not loaded".to_string()))?
            .clone();

        let mut current_delay = self.retry_config.base_delay;
        let mut last_error = None;

        for attempt in 1..=self.retry_config.max_retries {
            // Create fresh JWT token for each attempt (handles expiration)
            let jwt_token = self.create_jwt_token(&jwt_secret)?;

            let request_id = format!(
                "{method}_{attempt}_{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            );

            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params
            });

            debug!(
                "Engine API request: {} (attempt {}/{})",
                method, attempt, self.retry_config.max_retries
            );

            let request_start = Instant::now();

            match self
                .client
                .post(&self.engine_url)
                .header("Authorization", format!("Bearer {jwt_token}"))
                .header("Content-Type", "application/json")
                .header("User-Agent", "MultiVM-RethEngine/1.0")
                .json(&rpc_request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    match response.json::<Value>().await {
                        Ok(result) => {
                            let duration = request_start.elapsed();
                            debug!(
                                "Engine API {} succeeded in {:?} (attempt {})",
                                method, duration, attempt
                            );

                            // Update average response time
                            self.update_average_response_time(duration);

                            return Ok(result);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse Engine API response on attempt {}: {}",
                                attempt, e
                            );
                            last_error = Some(RethEngineError::Rpc(format!(
                                "Failed to parse Engine API response: {e}"
                            )));
                        }
                    }
                }
                Ok(response) => {
                    let status_code = response.status();
                    let error_body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());

                    warn!(
                        "Engine API {} returned error status {} on attempt {}: {}",
                        method, status_code, attempt, error_body
                    );

                    last_error = Some(RethEngineError::Rpc(format!(
                        "Engine API returned error status {status_code}: {error_body}"
                    )));

                    // Don't retry on client errors (4xx)
                    if status_code.is_client_error() {
                        break;
                    }
                }
                Err(e) => {
                    warn!(
                        "Engine API {} request failed on attempt {}: {}",
                        method, attempt, e
                    );
                    last_error = Some(RethEngineError::Rpc(format!(
                        "Engine API request failed: {e}"
                    )));
                }
            }

            // Update retry metrics
            self.metrics.retries_total.fetch_add(1, Ordering::Relaxed);

            // Don't wait after the last attempt
            if attempt < self.retry_config.max_retries {
                let delay = if self.retry_config.jitter {
                    self.add_jitter(current_delay)
                } else {
                    current_delay
                };

                debug!(
                    "Retrying {} in {:?} (attempt {}/{})",
                    method,
                    delay,
                    attempt + 1,
                    self.retry_config.max_retries
                );
                tokio::time::sleep(delay).await;

                // Exponential backoff with max delay cap
                current_delay = std::cmp::min(
                    Duration::from_millis(
                        (current_delay.as_millis() as f64 * self.retry_config.backoff_multiplier)
                            as u64,
                    ),
                    self.retry_config.max_delay,
                );
            }
        }

        // Update failure metrics
        self.metrics.requests_failed.fetch_add(1, Ordering::Relaxed);

        error!(
            "All {} retry attempts exhausted for method: {}",
            self.retry_config.max_retries, method
        );
        Err(last_error.unwrap_or_else(|| {
            RethEngineError::Rpc(format!("All retry attempts exhausted for method: {method}"))
        }))
    }

    /// Add jitter to delay for better retry distribution
    fn add_jitter(&self, delay: Duration) -> Duration {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let jitter_factor = rng.gen_range(0.5..1.5);
        Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64)
    }

    /// Update average response time metric
    fn update_average_response_time(&self, duration: Duration) {
        let current_avg = self
            .metrics
            .average_response_time_ms
            .load(Ordering::Relaxed);
        let new_value = duration.as_millis() as u64;

        // Simple exponential moving average
        let updated_avg = if current_avg == 0 {
            new_value
        } else {
            (current_avg * 9 + new_value) / 10
        };

        self.metrics
            .average_response_time_ms
            .store(updated_avg, Ordering::Relaxed);
    }

    /// Serialize payload attributes with withdrawal support
    fn serialize_payload_attributes(
        &self,
        attrs: &PayloadAttributes,
    ) -> Result<Value, RethEngineError> {
        let mut payload_attrs = json!({
            "timestamp": format!("0x{:x}", attrs.timestamp),
            "prevRandao": attrs.prev_randao,
            "suggestedFeeRecipient": attrs.suggested_fee_recipient
        });

        // Add withdrawals if present
        if let Some(ref withdrawals) = attrs.withdrawals {
            let withdrawals_json: Vec<Value> = withdrawals
                .iter()
                .map(|w| {
                    json!({
                        "index": format!("0x{:x}", w.index),
                        "validatorIndex": format!("0x{:x}", w.validator_index),
                        "address": w.address,
                        "amount": format!("0x{:x}", w.amount)
                    })
                })
                .collect();

            payload_attrs["withdrawals"] = json!(withdrawals_json);
        }

        // Add parent beacon block root if present
        if let Some(ref root) = attrs.parent_beacon_block_root {
            payload_attrs["parentBeaconBlockRoot"] = json!(root);
        }

        Ok(payload_attrs)
    }

    /// Parse newPayload response with enhanced error details
    fn parse_new_payload_response(
        &self,
        response: Value,
    ) -> Result<NewPayloadResponse, RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API newPayload error: {error}"
            )));
        }

        let result = response.get("result").ok_or_else(|| {
            RethEngineError::Rpc("Missing result in newPayload response".to_string())
        })?;

        let status_str = result
            .get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| {
                RethEngineError::Rpc("Missing status in newPayload response".to_string())
            })?;

        let status = PayloadStatus::from_str(status_str)?;

        let latest_valid_hash = result
            .get("latestValidHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string());

        let validation_error = result
            .get("validationError")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());

        Ok(NewPayloadResponse {
            status,
            latest_valid_hash,
            validation_error,
            processing_time: Duration::default(), // Will be set by caller
            blob_gas_used: None,                  // Will be set by caller
        })
    }

    /// Parse forkchoiceUpdated response
    fn parse_forkchoice_updated_response(
        &self,
        response: Value,
    ) -> Result<ForkchoiceUpdatedResponse, RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API forkchoiceUpdated error: {error}"
            )));
        }

        let result = response.get("result").ok_or_else(|| {
            RethEngineError::Rpc("Missing result in forkchoiceUpdated response".to_string())
        })?;

        let payload_status_json = result.get("payloadStatus").ok_or_else(|| {
            RethEngineError::Rpc("Missing payloadStatus in forkchoiceUpdated response".to_string())
        })?;

        let status_str = payload_status_json
            .get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing status in payloadStatus".to_string()))?;

        let payload_status = PayloadStatus::from_str(status_str)?;

        let payload_id = result
            .get("payloadId")
            .and_then(|id| id.as_str())
            .map(|s| s.to_string());

        let latest_valid_hash = payload_status_json
            .get("latestValidHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string());

        let validation_error = payload_status_json
            .get("validationError")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());

        Ok(ForkchoiceUpdatedResponse {
            payload_status,
            payload_id,
            latest_valid_hash,
            validation_error,
            processing_time: Duration::default(), // Will be set by caller
            withdrawals_processed: 0,             // Will be set by caller
        })
    }

    /// Parse getPayload response
    fn parse_get_payload_response(
        &self,
        response: Value,
    ) -> Result<GetPayloadResponse, RethEngineError> {
        if let Some(error) = response.get("error") {
            return Err(RethEngineError::Rpc(format!(
                "Engine API getPayload error: {error}"
            )));
        }

        let result = response.get("result").ok_or_else(|| {
            RethEngineError::Rpc("Missing result in getPayload response".to_string())
        })?;

        let execution_payload = result
            .get("executionPayload")
            .ok_or_else(|| {
                RethEngineError::Rpc("Missing executionPayload in getPayload response".to_string())
            })?
            .clone();

        let block_value = result
            .get("blockValue")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse blob bundle with enhanced validation
        let blobs_bundle = if let Some(bundle_json) = result.get("blobsBundle") {
            Some(self.parse_blobs_bundle(bundle_json)?)
        } else {
            None
        };

        let should_override_builder = result
            .get("shouldOverrideBuilder")
            .and_then(|b| b.as_bool());

        Ok(GetPayloadResponse {
            execution_payload,
            block_value,
            blobs_bundle,
            should_override_builder,
            processing_time: Duration::default(), // Will be set by caller
        })
    }

    /// Parse blobs bundle with comprehensive validation and metrics
    fn parse_blobs_bundle(&self, bundle_json: &Value) -> Result<BlobsBundle, RethEngineError> {
        let commitments = bundle_json
            .get("commitments")
            .and_then(|c| c.as_array())
            .ok_or_else(|| RethEngineError::Rpc("Missing commitments in blobs bundle".to_string()))?
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect::<Vec<String>>();

        let proofs = bundle_json
            .get("proofs")
            .and_then(|p| p.as_array())
            .ok_or_else(|| RethEngineError::Rpc("Missing proofs in blobs bundle".to_string()))?
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect::<Vec<String>>();

        let blobs = bundle_json
            .get("blobs")
            .and_then(|b| b.as_array())
            .ok_or_else(|| RethEngineError::Rpc("Missing blobs in blobs bundle".to_string()))?
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect::<Vec<String>>();

        // Validate blob bundle consistency
        if commitments.len() != proofs.len() || commitments.len() != blobs.len() {
            return Err(RethEngineError::Rpc(format!(
                "Inconsistent blob bundle sizes: {} commitments, {} proofs, {} blobs",
                commitments.len(),
                proofs.len(),
                blobs.len()
            )));
        }

        let blob_count = blobs.len();
        let total_size_bytes = blobs.iter().map(|blob| blob.len() as u64).sum();

        debug!(
            "Parsed blob bundle with {} blobs, total size: {} bytes",
            blob_count, total_size_bytes
        );

        Ok(BlobsBundle {
            commitments,
            proofs,
            blobs,
            blob_count,
            total_size_bytes,
        })
    }

    /// Create JWT token for Engine API authentication with caching
    fn create_jwt_token(&self, secret: &str) -> Result<String, RethEngineError> {
        // Check if we have a valid cached token
        if let Some(cached_token) = self.get_cached_jwt_token() {
            return Ok(cached_token);
        }

        // Generate new token and cache it
        let token = self.generate_fresh_jwt_token(secret)?;
        self.cache_jwt_token(token.clone());
        Ok(token)
    }

    /// Check for cached JWT token that's still valid
    fn get_cached_jwt_token(&self) -> Option<String> {
        if let Ok(cache) = self.jwt_token_cache.try_read() {
            if let Some((token, created_at)) = cache.as_ref() {
                // Check if token is still valid (with 30 second buffer before expiry)
                let age = created_at.elapsed();
                if age < self.jwt_expiry_duration.saturating_sub(Duration::from_secs(30)) {
                    return Some(token.clone());
                }
            }
        }
        None
    }

    /// Cache a JWT token with timestamp
    fn cache_jwt_token(&self, token: String) {
        if let Ok(mut cache) = self.jwt_token_cache.try_write() {
            *cache = Some((token, Instant::now()));
        }
    }

    /// Generate a fresh JWT token for Engine API authentication
    fn generate_fresh_jwt_token(&self, secret: &str) -> Result<String, RethEngineError> {
        use sha2::Sha256;

        // Create JWT header
        let header = json!({
            "alg": "HS256",
            "typ": "JWT"
        });

        // Create JWT payload with current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RethEngineError::Configuration(format!("System time error: {e}")))?
            .as_secs();

        let payload = json!({
            "iat": now,
            "exp": now + 300 // Token expires in 5 minutes (longer than default for retries)
        });

        // Encode header and payload
        let header_b64 = URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&header)
                .map_err(|e| {
                    RethEngineError::Configuration(format!("Failed to serialize header: {e}"))
                })?
                .as_bytes(),
        );

        let payload_b64 = URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&payload)
                .map_err(|e| {
                    RethEngineError::Configuration(format!("Failed to serialize payload: {e}"))
                })?
                .as_bytes(),
        );

        // Create signature
        let message = format!("{header_b64}.{payload_b64}");
        let secret_bytes = hex::decode(secret).map_err(|e| {
            RethEngineError::Configuration(format!("Invalid JWT secret format: {e}"))
        })?;

        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(&secret_bytes)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to create HMAC: {e}")))?;

        use hmac::Mac;
        mac.update(message.as_bytes());
        let signature = mac.finalize().into_bytes();

        let signature_b64 = URL_SAFE_NO_PAD.encode(signature);

        // Combine into final JWT
        let jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");
        Ok(jwt)
    }

    /// Enhanced health check with detailed diagnostics
    pub async fn health_check(&self) -> Result<EngineHealthStatus, RethEngineError> {
        let start_time = Instant::now();

        match self.exchange_capabilities(&[]).await {
            Ok(capabilities) => {
                let response_time = start_time.elapsed();
                Ok(EngineHealthStatus {
                    is_healthy: true,
                    response_time,
                    capabilities,
                    error_message: None,
                })
            }
            Err(e) => {
                warn!("Engine API health check failed: {}", e);
                let response_time = start_time.elapsed();
                Ok(EngineHealthStatus {
                    is_healthy: false,
                    response_time,
                    capabilities: vec![],
                    error_message: Some(e.to_string()),
                })
            }
        }
    }

    /// Get Engine API version information
    pub async fn get_version(&self) -> Result<String, RethEngineError> {
        // Try to determine version based on supported capabilities
        let capabilities = self
            .exchange_capabilities(&[
                "engine_newPayloadV3".to_string(),
                "engine_forkchoiceUpdatedV3".to_string(),
                "engine_getPayloadV3".to_string(),
            ])
            .await?;

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

    /// Get comprehensive Engine API metrics
    pub fn get_metrics(&self) -> EngineApiMetricsSnapshot {
        EngineApiMetricsSnapshot {
            requests_total: self.metrics.requests_total.load(Ordering::Relaxed),
            requests_successful: self.metrics.requests_successful.load(Ordering::Relaxed),
            requests_failed: self.metrics.requests_failed.load(Ordering::Relaxed),
            retries_total: self.metrics.retries_total.load(Ordering::Relaxed),
            average_response_time_ms: self
                .metrics
                .average_response_time_ms
                .load(Ordering::Relaxed),
            payload_submissions: self.metrics.payload_submissions.load(Ordering::Relaxed),
            forkchoice_updates: self.metrics.forkchoice_updates.load(Ordering::Relaxed),
            payload_retrievals: self.metrics.payload_retrievals.load(Ordering::Relaxed),
            blob_bundles_processed: self.metrics.blob_bundles_processed.load(Ordering::Relaxed),
            withdrawals_processed: self.metrics.withdrawals_processed.load(Ordering::Relaxed),
            success_rate: self.calculate_success_rate(),
        }
    }

    /// Calculate success rate percentage
    fn calculate_success_rate(&self) -> f64 {
        let total = self.metrics.requests_total.load(Ordering::Relaxed);
        let successful = self.metrics.requests_successful.load(Ordering::Relaxed);

        if total == 0 {
            0.0
        } else {
            (successful as f64 / total as f64) * 100.0
        }
    }

    /// Reset metrics (useful for testing or periodic resets)
    pub fn reset_metrics(&self) {
        self.metrics.requests_total.store(0, Ordering::Relaxed);
        self.metrics.requests_successful.store(0, Ordering::Relaxed);
        self.metrics.requests_failed.store(0, Ordering::Relaxed);
        self.metrics.retries_total.store(0, Ordering::Relaxed);
        self.metrics
            .average_response_time_ms
            .store(0, Ordering::Relaxed);
        self.metrics.payload_submissions.store(0, Ordering::Relaxed);
        self.metrics.forkchoice_updates.store(0, Ordering::Relaxed);
        self.metrics.payload_retrievals.store(0, Ordering::Relaxed);
        self.metrics
            .blob_bundles_processed
            .store(0, Ordering::Relaxed);
        self.metrics
            .withdrawals_processed
            .store(0, Ordering::Relaxed);
    }
}

/// Engine health status information
#[derive(Debug, Clone)]
pub struct EngineHealthStatus {
    pub is_healthy: bool,
    pub response_time: Duration,
    pub capabilities: Vec<String>,
    pub error_message: Option<String>,
}

/// Snapshot of Engine API metrics
#[derive(Debug, Clone)]
pub struct EngineApiMetricsSnapshot {
    pub requests_total: u64,
    pub requests_successful: u64,
    pub requests_failed: u64,
    pub retries_total: u64,
    pub average_response_time_ms: u64,
    pub payload_submissions: u64,
    pub forkchoice_updates: u64,
    pub payload_retrievals: u64,
    pub blob_bundles_processed: u64,
    pub withdrawals_processed: u64,
    pub success_rate: f64,
}

/// Enhanced builder for EngineApiClient
pub struct EngineApiClientBuilder {
    engine_url: Option<String>,
    jwt_secret: Option<Arc<RwLock<Option<String>>>>,
    retry_config: RetryConfig,
    max_concurrent_requests: usize,
}

impl EngineApiClientBuilder {
    pub fn new() -> Self {
        Self {
            engine_url: None,
            jwt_secret: None,
            retry_config: RetryConfig::default(),
            max_concurrent_requests: 50,
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

    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    pub fn build(self) -> Result<EngineApiClient, RethEngineError> {
        let engine_url = self
            .engine_url
            .ok_or_else(|| RethEngineError::Configuration("Engine URL is required".to_string()))?;

        let jwt_secret = self
            .jwt_secret
            .ok_or_else(|| RethEngineError::Configuration("JWT secret is required".to_string()))?;

        EngineApiClient::new(
            engine_url,
            jwt_secret,
            self.retry_config,
            self.max_concurrent_requests,
        )
    }
}

impl Default for EngineApiClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
