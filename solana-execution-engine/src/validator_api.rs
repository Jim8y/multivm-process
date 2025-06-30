//! Solana validator API client for advanced operations
//!
//! This module provides specialized API methods for interacting with Solana validators,
//! including slot management, account subscriptions, and validator-specific operations.

use crate::engine::SolanaEngineError;
use reqwest::Client;
use serde_json::{json, Value};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    slot_history::Slot,
};
use std::time::Duration;
use tracing::{debug, warn};

/// Solana validator API client for advanced operations
pub struct SolanaValidatorApi {
    client: Client,
    rpc_url: String,
    ws_url: Option<String>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

/// Slot information with additional metadata
#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub slot: Slot,
    pub parent: Slot,
    pub root: Slot,
    pub confirmed_slot: Option<Slot>,
    pub finalized_slot: Option<Slot>,
}

/// Account information with subscription support
#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub pubkey: String,
    pub lamports: u64,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub data_encoding: String,
}

/// Transaction status with detailed information
#[derive(Debug, Clone)]
pub struct TransactionStatus {
    pub signature: String,
    pub slot: Slot,
    pub confirmations: Option<u64>,
    pub err: Option<Value>,
    pub confirmation_status: ConfirmationStatus,
    pub block_time: Option<i64>,
    pub compute_units_consumed: Option<u64>,
    pub fee: u64,
}

/// Confirmation status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationStatus {
    Processed,
    Confirmed,
    Finalized,
}

/// Validator performance metrics
#[derive(Debug, Clone)]
pub struct ValidatorPerformance {
    pub identity: String,
    pub vote_account: String,
    pub commission: u8,
    pub last_vote: Slot,
    pub root_slot: Slot,
    pub credits: u64,
    pub epoch_credits: Vec<(u64, u64, u64)>, // (epoch, credits, prev_credits)
    pub activated_stake: u64,
    pub delinquent: bool,
}

/// Epoch information
#[derive(Debug, Clone)]
pub struct EpochInfo {
    pub epoch: u64,
    pub slot_index: u64,
    pub slots_in_epoch: u64,
    pub absolute_slot: Slot,
    pub block_height: Option<u64>,
    pub transaction_count: Option<u64>,
}

impl SolanaValidatorApi {
    /// Create a new validator API client
    pub fn new(
        rpc_url: String,
        ws_url: Option<String>,
        request_timeout: Duration,
        max_retries: u32,
        retry_delay: Duration,
    ) -> Result<Self, SolanaEngineError> {
        let client = Client::builder()
            .timeout(request_timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(|e| {
                SolanaEngineError::Rpc(format!("Failed to create validator API client: {e}"))
            })?;

        Ok(Self {
            client,
            rpc_url,
            ws_url,
            request_timeout,
            max_retries,
            retry_delay,
        })
    }

    /// Get detailed slot information
    pub async fn get_slot_info(&self) -> Result<SlotInfo, SolanaEngineError> {
        // Get current slot
        let slot_response = self.make_request("getSlot", json!([])).await?;
        let slot = slot_response
            .get("result")
            .and_then(|r| r.as_u64())
            .ok_or_else(|| SolanaEngineError::Rpc("Invalid slot response".to_string()))?;

        // Get slot leaders to find parent
        let leaders_response = self
            .make_request("getSlotLeaders", json!([slot.saturating_sub(1), 2]))
            .await?;
        let parent = slot.saturating_sub(1);

        // Get confirmed and finalized slots
        let confirmed_slot = self
            .get_slot_with_commitment(CommitmentLevel::Confirmed)
            .await
            .ok();
        let finalized_slot = self
            .get_slot_with_commitment(CommitmentLevel::Finalized)
            .await
            .ok();

        // For root slot, use finalized slot or a conservative estimate
        let root = finalized_slot.unwrap_or(slot.saturating_sub(32));

        Ok(SlotInfo {
            slot,
            parent,
            root,
            confirmed_slot,
            finalized_slot,
        })
    }

    /// Get slot with specific commitment level
    pub async fn get_slot_with_commitment(
        &self,
        commitment: CommitmentLevel,
    ) -> Result<Slot, SolanaEngineError> {
        let commitment_config = CommitmentConfig { commitment };
        let response = self
            .make_request("getSlot", json!([commitment_config]))
            .await?;

        if let Some(slot) = response.get("result").and_then(|r| r.as_u64()) {
            Ok(slot)
        } else {
            Err(SolanaEngineError::Rpc("Invalid slot response".to_string()))
        }
    }

    /// Get epoch information
    pub async fn get_epoch_info(&self) -> Result<EpochInfo, SolanaEngineError> {
        let response = self.make_request("getEpochInfo", json!([])).await?;

        if let Some(result) = response.get("result") {
            let epoch = result
                .get("epoch")
                .and_then(|e| e.as_u64())
                .ok_or_else(|| SolanaEngineError::Rpc("Missing epoch".to_string()))?;

            let slot_index = result
                .get("slotIndex")
                .and_then(|s| s.as_u64())
                .ok_or_else(|| SolanaEngineError::Rpc("Missing slot index".to_string()))?;

            let slots_in_epoch = result
                .get("slotsInEpoch")
                .and_then(|s| s.as_u64())
                .ok_or_else(|| SolanaEngineError::Rpc("Missing slots in epoch".to_string()))?;

            let absolute_slot = result
                .get("absoluteSlot")
                .and_then(|s| s.as_u64())
                .ok_or_else(|| SolanaEngineError::Rpc("Missing absolute slot".to_string()))?;

            let block_height = result.get("blockHeight").and_then(|h| h.as_u64());
            let transaction_count = result.get("transactionCount").and_then(|t| t.as_u64());

            Ok(EpochInfo {
                epoch,
                slot_index,
                slots_in_epoch,
                absolute_slot,
                block_height,
                transaction_count,
            })
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid epoch info response".to_string(),
            ))
        }
    }

    /// Get validator performance metrics
    pub async fn get_vote_accounts(&self) -> Result<Vec<ValidatorPerformance>, SolanaEngineError> {
        let response = self.make_request("getVoteAccounts", json!([])).await?;

        if let Some(result) = response.get("result") {
            let mut validators = Vec::new();

            // Process current validators
            if let Some(current) = result.get("current").and_then(|c| c.as_array()) {
                for validator in current {
                    if let Ok(perf) = self.parse_validator_performance(validator, false) {
                        validators.push(perf);
                    }
                }
            }

            // Process delinquent validators
            if let Some(delinquent) = result.get("delinquent").and_then(|d| d.as_array()) {
                for validator in delinquent {
                    if let Ok(perf) = self.parse_validator_performance(validator, true) {
                        validators.push(perf);
                    }
                }
            }

            Ok(validators)
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid vote accounts response".to_string(),
            ))
        }
    }

    /// Get cluster nodes information
    pub async fn get_cluster_nodes(&self) -> Result<Vec<Value>, SolanaEngineError> {
        let response = self.make_request("getClusterNodes", json!([])).await?;

        if let Some(result) = response.get("result").and_then(|r| r.as_array()) {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid cluster nodes response".to_string(),
            ))
        }
    }

    /// Get supply information
    pub async fn get_supply(&self) -> Result<Value, SolanaEngineError> {
        let response = self.make_request("getSupply", json!([])).await?;

        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid supply response".to_string(),
            ))
        }
    }

    /// Get inflation rate
    pub async fn get_inflation_rate(&self) -> Result<Value, SolanaEngineError> {
        let response = self.make_request("getInflationRate", json!([])).await?;

        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid inflation rate response".to_string(),
            ))
        }
    }

    /// Get recent performance samples
    pub async fn get_recent_performance_samples(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<Value>, SolanaEngineError> {
        let params = if let Some(limit) = limit {
            json!([limit])
        } else {
            json!([])
        };

        let response = self
            .make_request("getRecentPerformanceSamples", params)
            .await?;

        if let Some(result) = response.get("result").and_then(|r| r.as_array()) {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid performance samples response".to_string(),
            ))
        }
    }

    /// Get block production information
    pub async fn get_block_production(&self) -> Result<Value, SolanaEngineError> {
        let response = self.make_request("getBlockProduction", json!([])).await?;

        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid block production response".to_string(),
            ))
        }
    }

    /// Get leader schedule
    pub async fn get_leader_schedule(
        &self,
        slot: Option<Slot>,
    ) -> Result<Value, SolanaEngineError> {
        let params = if let Some(slot) = slot {
            json!([slot])
        } else {
            json!([])
        };

        let response = self.make_request("getLeaderSchedule", params).await?;

        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid leader schedule response".to_string(),
            ))
        }
    }

    /// Get slot leaders
    pub async fn get_slot_leaders(
        &self,
        start_slot: Slot,
        limit: u64,
    ) -> Result<Vec<String>, SolanaEngineError> {
        let response = self
            .make_request("getSlotLeaders", json!([start_slot, limit]))
            .await?;

        if let Some(result) = response.get("result").and_then(|r| r.as_array()) {
            let leaders: Result<Vec<String>, _> = result
                .iter()
                .map(|leader| {
                    leader
                        .as_str()
                        .ok_or_else(|| SolanaEngineError::Rpc("Invalid leader format".to_string()))
                        .map(|s| s.to_string())
                })
                .collect();

            leaders
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid slot leaders response".to_string(),
            ))
        }
    }

    /// Get first available block
    pub async fn get_first_available_block(&self) -> Result<Slot, SolanaEngineError> {
        let response = self
            .make_request("getFirstAvailableBlock", json!([]))
            .await?;

        if let Some(slot) = response.get("result").and_then(|r| r.as_u64()) {
            Ok(slot)
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid first available block response".to_string(),
            ))
        }
    }

    /// Get blocks in range
    pub async fn get_blocks(
        &self,
        start_slot: Slot,
        end_slot: Option<Slot>,
    ) -> Result<Vec<Slot>, SolanaEngineError> {
        let params = if let Some(end) = end_slot {
            json!([start_slot, end])
        } else {
            json!([start_slot])
        };

        let response = self.make_request("getBlocks", params).await?;

        if let Some(result) = response.get("result").and_then(|r| r.as_array()) {
            let blocks: Result<Vec<Slot>, _> = result
                .iter()
                .map(|block| {
                    block.as_u64().ok_or_else(|| {
                        SolanaEngineError::Rpc("Invalid block slot format".to_string())
                    })
                })
                .collect();

            blocks
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid blocks response".to_string(),
            ))
        }
    }

    /// Get blocks with limit
    pub async fn get_blocks_with_limit(
        &self,
        start_slot: Slot,
        limit: u64,
    ) -> Result<Vec<Slot>, SolanaEngineError> {
        let response = self
            .make_request("getBlocksWithLimit", json!([start_slot, limit]))
            .await?;

        if let Some(result) = response.get("result").and_then(|r| r.as_array()) {
            let blocks: Result<Vec<Slot>, _> = result
                .iter()
                .map(|block| {
                    block.as_u64().ok_or_else(|| {
                        SolanaEngineError::Rpc("Invalid block slot format".to_string())
                    })
                })
                .collect();

            blocks
        } else {
            Err(SolanaEngineError::Rpc(
                "Invalid blocks with limit response".to_string(),
            ))
        }
    }

    /// Health check for validator API
    pub async fn health_check(&self) -> Result<bool, SolanaEngineError> {
        match self.get_slot_info().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Make a JSON-RPC request with retry logic
    async fn make_request(&self, method: &str, params: Value) -> Result<Value, SolanaEngineError> {
        for attempt in 1..=self.max_retries {
            let request_id = format!("{method}_{attempt}");
            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params
            });

            debug!(
                "Solana validator API request: {} (attempt {})",
                method, attempt
            );

            match self
                .client
                .post(&self.rpc_url)
                .header("Content-Type", "application/json")
                .json(&rpc_request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    match response.json::<Value>().await {
                        Ok(result) => {
                            debug!("Solana validator API response: {} succeeded", method);
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse Solana validator API response on attempt {}: {}",
                                attempt, e
                            );
                            if attempt == self.max_retries {
                                return Err(SolanaEngineError::Rpc(format!(
                                    "Failed to parse API response: {e}"
                                )));
                            }
                        }
                    }
                }
                Ok(response) => {
                    warn!(
                        "Solana validator API returned error status on attempt {}: {}",
                        attempt,
                        response.status()
                    );
                    if attempt == self.max_retries {
                        return Err(SolanaEngineError::Rpc(format!(
                            "API returned error status: {}",
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    warn!(
                        "Solana validator API request failed on attempt {}: {}",
                        attempt, e
                    );
                    if attempt == self.max_retries {
                        return Err(SolanaEngineError::Rpc(format!("API request failed: {e}")));
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

    /// Parse validator performance from JSON
    fn parse_validator_performance(
        &self,
        validator_data: &Value,
        delinquent: bool,
    ) -> Result<ValidatorPerformance, SolanaEngineError> {
        let identity = validator_data
            .get("nodePubkey")
            .and_then(|i| i.as_str())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing validator identity".to_string()))?
            .to_string();

        let vote_account = validator_data
            .get("votePubkey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing vote account".to_string()))?
            .to_string();

        let commission = validator_data
            .get("commission")
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as u8;

        let last_vote = validator_data
            .get("lastVote")
            .and_then(|l| l.as_u64())
            .unwrap_or(0);

        let root_slot = validator_data
            .get("rootSlot")
            .and_then(|r| r.as_u64())
            .unwrap_or(0);

        let credits = validator_data
            .get("credits")
            .and_then(|c| c.as_u64())
            .unwrap_or(0);

        let epoch_credits = if let Some(credits_array) = validator_data
            .get("epochCredits")
            .and_then(|e| e.as_array())
        {
            credits_array
                .iter()
                .filter_map(|credit| {
                    if let Some(credit_array) = credit.as_array() {
                        if credit_array.len() >= 3 {
                            let epoch = credit_array[0].as_u64()?;
                            let credits = credit_array[1].as_u64()?;
                            let prev_credits = credit_array[2].as_u64()?;
                            Some((epoch, credits, prev_credits))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        };

        let activated_stake = validator_data
            .get("activatedStake")
            .and_then(|s| s.as_u64())
            .unwrap_or(0);

        Ok(ValidatorPerformance {
            identity,
            vote_account,
            commission,
            last_vote,
            root_slot,
            credits,
            epoch_credits,
            activated_stake,
            delinquent,
        })
    }
}

impl ConfirmationStatus {
    /// Convert from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "processed" => ConfirmationStatus::Processed,
            "confirmed" => ConfirmationStatus::Confirmed,
            "finalized" => ConfirmationStatus::Finalized,
            _ => ConfirmationStatus::Processed, // Default to processed
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfirmationStatus::Processed => "processed",
            ConfirmationStatus::Confirmed => "confirmed",
            ConfirmationStatus::Finalized => "finalized",
        }
    }
}

/// Builder for SolanaValidatorApi
pub struct SolanaValidatorApiBuilder {
    rpc_url: Option<String>,
    ws_url: Option<String>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

impl SolanaValidatorApiBuilder {
    pub fn new() -> Self {
        Self {
            rpc_url: None,
            ws_url: None,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    pub fn rpc_url(mut self, url: String) -> Self {
        self.rpc_url = Some(url);
        self
    }

    pub fn ws_url(mut self, url: String) -> Self {
        self.ws_url = Some(url);
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

    pub fn build(self) -> Result<SolanaValidatorApi, SolanaEngineError> {
        let rpc_url = self
            .rpc_url
            .ok_or_else(|| SolanaEngineError::Configuration("RPC URL is required".to_string()))?;

        SolanaValidatorApi::new(
            rpc_url,
            self.ws_url,
            self.request_timeout,
            self.max_retries,
            self.retry_delay,
        )
    }
}

impl Default for SolanaValidatorApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}
