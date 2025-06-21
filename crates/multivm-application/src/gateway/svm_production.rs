//! Production SVM API Gateway implementation with real Solana node connection
//!
//! This module provides a production-ready gateway for interacting with
//! Solana blockchain through JSON-RPC.

use crate::{
    cache::CacheLayer,
    error::{ApplicationError, ApplicationResult},
};
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Production SVM API Gateway for interacting with real Solana nodes
#[derive(Clone)]
pub struct ProductionSvmGateway {
    /// Cache layer for performance
    cache: Arc<CacheLayer>,

    /// JSON-RPC client for Solana RPC
    rpc_client: Arc<HttpClient>,

    /// Configuration
    config: SvmGatewayConfig,

    /// Health status
    health_status: Arc<RwLock<HealthStatus>>,
}

/// SVM Gateway configuration
#[derive(Debug, Clone)]
pub struct SvmGatewayConfig {
    /// JSON-RPC endpoint
    pub rpc_url: String,

    /// WebSocket endpoint for subscriptions
    pub ws_url: Option<String>,

    /// Request timeout
    pub request_timeout: Duration,

    /// Maximum request retries
    pub max_retries: u32,

    /// Commitment level for transactions
    pub commitment: CommitmentLevel,

    /// Enable preflight checks
    pub preflight_checks: bool,
}

/// Solana commitment levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommitmentLevel {
    Processed,
    Confirmed,
    Finalized,
}

/// Health status of the gateway
#[derive(Debug, Clone)]
struct HealthStatus {
    is_healthy: bool,
    last_slot: Option<u64>,
    last_error: Option<String>,
    last_check: std::time::Instant,
}

/// Solana block representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmBlock {
    pub slot: u64,
    pub blockhash: String,
    pub parent_slot: u64,
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
    pub transactions: Vec<SvmTransactionWithMeta>,
    pub rewards: Vec<SvmReward>,
}

/// Solana transaction with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmTransactionWithMeta {
    pub transaction: SvmTransaction,
    pub meta: Option<SvmTransactionMeta>,
}

/// Solana transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmTransaction {
    pub signatures: Vec<String>,
    pub message: SvmMessage,
}

/// Solana message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmMessage {
    pub header: MessageHeader,
    pub account_keys: Vec<String>,
    pub recent_blockhash: String,
    pub instructions: Vec<SvmInstruction>,
}

/// Message header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub num_required_signatures: u8,
    pub num_readonly_signed_accounts: u8,
    pub num_readonly_unsigned_accounts: u8,
}

/// Solana instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmInstruction {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: String,
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmTransactionMeta {
    pub err: Option<Value>,
    pub fee: u64,
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub inner_instructions: Option<Vec<InnerInstruction>>,
    pub log_messages: Option<Vec<String>>,
    pub pre_token_balances: Option<Vec<TokenBalance>>,
    pub post_token_balances: Option<Vec<TokenBalance>>,
    pub compute_units_consumed: Option<u64>,
}

/// Inner instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnerInstruction {
    pub index: u8,
    pub instructions: Vec<SvmInstruction>,
}

/// Token balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub account_index: u8,
    pub mint: String,
    pub ui_token_amount: UiTokenAmount,
    pub owner: String,
}

/// UI token amount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTokenAmount {
    pub ui_amount: Option<f64>,
    pub decimals: u8,
    pub amount: String,
    pub ui_amount_string: String,
}

/// Block reward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmReward {
    pub pubkey: String,
    pub lamports: i64,
    pub post_balance: u64,
    pub reward_type: Option<String>,
    pub commission: Option<u8>,
}

/// Account info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmAccount {
    pub lamports: u64,
    pub owner: String,
    pub data: AccountData,
    pub executable: bool,
    pub rent_epoch: u64,
}

/// Account data encoding
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AccountData {
    Base64(String),
    Base58(String),
    Json(Value),
}

/// Transaction signature status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureStatus {
    pub slot: u64,
    pub confirmations: Option<u64>,
    pub err: Option<Value>,
    pub confirmation_status: Option<String>,
}

/// Program accounts filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramAccountsFilter {
    pub memcmp: Option<MemcmpFilter>,
    pub datasize: Option<u64>,
}

/// Memory comparison filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemcmpFilter {
    pub offset: usize,
    pub bytes: String,
}

/// Supply info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supply {
    pub total: u64,
    pub circulating: u64,
    pub non_circulating: u64,
    pub non_circulating_accounts: Vec<String>,
}

/// Epoch info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochInfo {
    pub epoch: u64,
    pub slot_index: u64,
    pub slots_in_epoch: u64,
    pub absolute_slot: u64,
    pub block_height: Option<u64>,
    pub transaction_count: Option<u64>,
}

impl ProductionSvmGateway {
    /// Create a new production SVM gateway
    pub async fn new(config: SvmGatewayConfig, cache: Arc<CacheLayer>) -> ApplicationResult<Self> {
        info!("Initializing production SVM gateway: {}", config.rpc_url);

        // Create JSON-RPC client
        let rpc_client = HttpClientBuilder::default()
            .request_timeout(config.request_timeout)
            .build(&config.rpc_url)
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "svm_gateway".to_string(),
                message: format!("Failed to create RPC client: {}", e),
            })?;

        let gateway = Self {
            cache,
            rpc_client: Arc::new(rpc_client),
            config,
            health_status: Arc::new(RwLock::new(HealthStatus {
                is_healthy: false,
                last_slot: None,
                last_error: None,
                last_check: std::time::Instant::now(),
            })),
        };

        // Perform initial health check
        gateway.check_health().await;

        Ok(gateway)
    }

    /// Get the latest slot
    pub async fn get_slot(&self) -> ApplicationResult<u64> {
        // Check cache first
        if let Some(slot) = self.cache.get::<u64>("svm:latest_slot").await? {
            return Ok(slot);
        }

        // Make RPC call
        let slot: u64 = self
            .rpc_call("getSlot", rpc_params![self.get_commitment_config()])
            .await?;

        // Cache the result with short TTL
        self.cache
            .set("svm:latest_slot", &slot, Duration::from_secs(1))
            .await?;

        // Update health status
        let mut health = self.health_status.write().await;
        health.is_healthy = true;
        health.last_slot = Some(slot);
        health.last_check = std::time::Instant::now();

        Ok(slot)
    }

    /// Get block by slot
    pub async fn get_block(&self, slot: u64) -> ApplicationResult<Option<SvmBlock>> {
        let cache_key = format!("svm:block:{}", slot);

        // Check cache
        if let Some(block) = self.cache.get::<SvmBlock>(&cache_key).await? {
            return Ok(Some(block));
        }

        // Make RPC call
        let encoding = json!({
            "encoding": "json",
            "transactionDetails": "full",
            "rewards": true,
            "maxSupportedTransactionVersion": 0
        });

        match self
            .rpc_call::<Option<Value>>("getBlock", rpc_params![slot, encoding])
            .await
        {
            Ok(Some(block_json)) => {
                let block = self.parse_block(slot, block_json)?;

                // Cache immutable block data with long TTL
                self.cache
                    .set(&cache_key, &block, Duration::from_secs(86400)) // 24 hours
                    .await?;

                Ok(Some(block))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get latest blockhash
    pub async fn get_latest_blockhash(&self) -> ApplicationResult<(String, u64)> {
        // Check cache first
        if let Some(result) = self
            .cache
            .get::<(String, u64)>("svm:latest_blockhash")
            .await?
        {
            return Ok(result);
        }

        // Make RPC call
        let response: Value = self
            .rpc_call(
                "getLatestBlockhash",
                rpc_params![self.get_commitment_config()],
            )
            .await?;

        let blockhash = response["blockhash"]
            .as_str()
            .ok_or_else(|| ApplicationError::ParseError {
                field: "blockhash".to_string(),
                value: response.to_string(),
                message: "Missing blockhash".to_string(),
            })?
            .to_string();

        let last_valid_block_height =
            response["lastValidBlockHeight"].as_u64().ok_or_else(|| {
                ApplicationError::ParseError {
                    field: "lastValidBlockHeight".to_string(),
                    value: response.to_string(),
                    message: "Missing lastValidBlockHeight".to_string(),
                }
            })?;

        let result = (blockhash, last_valid_block_height);

        // Cache with short TTL
        self.cache
            .set("svm:latest_blockhash", &result, Duration::from_secs(5))
            .await?;

        Ok(result)
    }

    /// Send transaction
    pub async fn send_transaction(&self, transaction: &str) -> ApplicationResult<String> {
        // Validate transaction format
        if transaction.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                message: "Transaction data cannot be empty".to_string(),
            });
        }

        let encoding = json!({
            "encoding": "base64",
            "skipPreflight": !self.config.preflight_checks,
            "preflightCommitment": self.get_commitment_string(),
            "maxRetries": self.config.max_retries,
        });

        // Send transaction
        let signature: String = self
            .rpc_call("sendTransaction", rpc_params![transaction, encoding])
            .await?;

        info!("Transaction sent: {}", signature);
        Ok(signature)
    }

    /// Get transaction by signature
    pub async fn get_transaction(
        &self,
        signature: &str,
    ) -> ApplicationResult<Option<SvmTransactionWithMeta>> {
        let cache_key = format!("svm:tx:{}", signature);

        // Check cache
        if let Some(tx) = self.cache.get::<SvmTransactionWithMeta>(&cache_key).await? {
            return Ok(Some(tx));
        }

        let encoding = json!({
            "encoding": "json",
            "commitment": self.get_commitment_string(),
            "maxSupportedTransactionVersion": 0
        });

        // Make RPC call
        match self
            .rpc_call::<Option<Value>>("getTransaction", rpc_params![signature, encoding])
            .await
        {
            Ok(Some(tx_json)) => {
                let tx = self.parse_transaction(tx_json)?;

                // Cache immutable transaction data with long TTL
                self.cache
                    .set(&cache_key, &tx, Duration::from_secs(86400)) // 24 hours
                    .await?;

                Ok(Some(tx))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get signature status
    pub async fn get_signature_status(
        &self,
        signature: &str,
    ) -> ApplicationResult<Option<SignatureStatus>> {
        let response: Value = self
            .rpc_call(
                "getSignatureStatuses",
                rpc_params![[signature], json!({"searchTransactionHistory": true})],
            )
            .await?;

        if let Some(statuses) = response.as_array() {
            if let Some(status) = statuses.get(0) {
                if !status.is_null() {
                    let status: SignatureStatus =
                        serde_json::from_value(status.clone()).map_err(|e| {
                            ApplicationError::ParseError {
                                field: "signature_status".to_string(),
                                value: status.to_string(),
                                message: e.to_string(),
                            }
                        })?;
                    return Ok(Some(status));
                }
            }
        }

        Ok(None)
    }

    /// Get account info
    pub async fn get_account_info(&self, pubkey: &str) -> ApplicationResult<Option<SvmAccount>> {
        let cache_key = format!("svm:account:{}", pubkey);

        // Check cache for recent data
        if let Some(account) = self.cache.get::<SvmAccount>(&cache_key).await? {
            return Ok(Some(account));
        }

        let encoding = json!({
            "encoding": "base64",
            "commitment": self.get_commitment_string()
        });

        // Make RPC call
        match self
            .rpc_call::<Option<Value>>("getAccountInfo", rpc_params![pubkey, encoding])
            .await
        {
            Ok(Some(account_json)) => {
                let account = self.parse_account(account_json)?;

                // Cache with medium TTL
                self.cache
                    .set(&cache_key, &account, Duration::from_secs(60))
                    .await?;

                Ok(Some(account))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get balance
    pub async fn get_balance(&self, pubkey: &str) -> ApplicationResult<u64> {
        let cache_key = format!("svm:balance:{}", pubkey);

        // Check cache
        if let Some(balance) = self.cache.get::<u64>(&cache_key).await? {
            return Ok(balance);
        }

        // Make RPC call
        let balance: u64 = self
            .rpc_call(
                "getBalance",
                rpc_params![pubkey, self.get_commitment_config()],
            )
            .await?;

        // Cache with short TTL
        self.cache
            .set(&cache_key, &balance, Duration::from_secs(5))
            .await?;

        Ok(balance)
    }

    /// Get program accounts
    pub async fn get_program_accounts(
        &self,
        program_id: &str,
        filters: Vec<ProgramAccountsFilter>,
    ) -> ApplicationResult<Vec<(String, SvmAccount)>> {
        let config = json!({
            "encoding": "base64",
            "commitment": self.get_commitment_string(),
            "filters": filters
        });

        // Make RPC call
        let accounts: Vec<Value> = self
            .rpc_call("getProgramAccounts", rpc_params![program_id, config])
            .await?;

        // Parse accounts
        let mut result = Vec::new();
        for account_value in accounts {
            let pubkey = account_value["pubkey"]
                .as_str()
                .ok_or_else(|| ApplicationError::ParseError {
                    field: "pubkey".to_string(),
                    value: account_value.to_string(),
                    message: "Missing pubkey".to_string(),
                })?
                .to_string();

            let account = self.parse_account(account_value["account"].clone())?;
            result.push((pubkey, account));
        }

        Ok(result)
    }

    /// Request airdrop (devnet/testnet only)
    pub async fn request_airdrop(&self, pubkey: &str, lamports: u64) -> ApplicationResult<String> {
        // Make RPC call
        let signature: String = self
            .rpc_call("requestAirdrop", rpc_params![pubkey, lamports])
            .await?;

        info!("Airdrop requested: {} lamports to {}", lamports, pubkey);
        Ok(signature)
    }

    /// Get supply info
    pub async fn get_supply(&self) -> ApplicationResult<Supply> {
        // Check cache
        if let Some(supply) = self.cache.get::<Supply>("svm:supply").await? {
            return Ok(supply);
        }

        // Make RPC call
        let response: Value = self
            .rpc_call("getSupply", rpc_params![self.get_commitment_config()])
            .await?;

        let supply = Supply {
            total: response["total"].as_u64().unwrap_or(0),
            circulating: response["circulating"].as_u64().unwrap_or(0),
            non_circulating: response["nonCirculating"].as_u64().unwrap_or(0),
            non_circulating_accounts: response["nonCirculatingAccounts"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        };

        // Cache with medium TTL
        self.cache
            .set("svm:supply", &supply, Duration::from_secs(300))
            .await?;

        Ok(supply)
    }

    /// Get epoch info
    pub async fn get_epoch_info(&self) -> ApplicationResult<EpochInfo> {
        // Check cache
        if let Some(epoch_info) = self.cache.get::<EpochInfo>("svm:epoch_info").await? {
            return Ok(epoch_info);
        }

        // Make RPC call
        let response: EpochInfo = self
            .rpc_call("getEpochInfo", rpc_params![self.get_commitment_config()])
            .await?;

        // Cache with short TTL
        self.cache
            .set("svm:epoch_info", &response, Duration::from_secs(30))
            .await?;

        Ok(response)
    }

    /// Get minimum balance for rent exemption
    pub async fn get_minimum_balance_for_rent_exemption(
        &self,
        data_len: usize,
    ) -> ApplicationResult<u64> {
        let cache_key = format!("svm:rent_exemption:{}", data_len);

        // Check cache
        if let Some(lamports) = self.cache.get::<u64>(&cache_key).await? {
            return Ok(lamports);
        }

        // Make RPC call
        let lamports: u64 = self
            .rpc_call("getMinimumBalanceForRentExemption", rpc_params![data_len])
            .await?;

        // Cache with long TTL (rent exemption doesn't change often)
        self.cache
            .set(&cache_key, &lamports, Duration::from_secs(3600))
            .await?;

        Ok(lamports)
    }

    /// Simulate transaction
    pub async fn simulate_transaction(&self, transaction: &str) -> ApplicationResult<Value> {
        let config = json!({
            "encoding": "base64",
            "commitment": self.get_commitment_string(),
            "sigVerify": true,
            "replaceRecentBlockhash": true,
            "accounts": {
                "encoding": "base64"
            }
        });

        // Make RPC call
        let result: Value = self
            .rpc_call("simulateTransaction", rpc_params![transaction, config])
            .await?;

        Ok(result)
    }

    /// Get fee for message
    pub async fn get_fee_for_message(&self, message: &str) -> ApplicationResult<u64> {
        // Make RPC call
        let fee: Option<u64> = self
            .rpc_call(
                "getFeeForMessage",
                rpc_params![message, self.get_commitment_config()],
            )
            .await?;

        fee.ok_or_else(|| ApplicationError::InvalidRequest {
            message: "Unable to calculate fee for message".to_string(),
        })
    }

    /// Get recent prioritization fees
    pub async fn get_recent_prioritization_fees(
        &self,
        accounts: Vec<String>,
    ) -> ApplicationResult<Vec<Value>> {
        // Make RPC call
        let fees: Vec<Value> = self
            .rpc_call("getRecentPrioritizationFees", rpc_params![accounts])
            .await?;

        Ok(fees)
    }

    // Helper methods

    /// Make JSON-RPC call with retry logic
    async fn rpc_call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: jsonrpsee::core::params::ArrayParams,
    ) -> ApplicationResult<T> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
                debug!("Retrying RPC call {} (attempt {})", method, attempt + 1);
            }

            match self.rpc_client.request(method, params.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("RPC call {} failed: {}", method, e);
                    last_error = Some(e);
                }
            }
        }

        // Update health status on failure
        let mut health = self.health_status.write().await;
        health.is_healthy = false;
        health.last_error = Some(format!(
            "RPC call {} failed after {} retries",
            method, self.config.max_retries
        ));
        health.last_check = std::time::Instant::now();

        Err(ApplicationError::RpcError {
            endpoint: method.to_string(),
            message: last_error.unwrap().to_string(),
        })
    }

    /// Parse block JSON response
    fn parse_block(&self, slot: u64, json: Value) -> ApplicationResult<SvmBlock> {
        // Parse transactions
        let transactions = json["transactions"]
            .as_array()
            .map(|txs| {
                txs.iter()
                    .filter_map(|tx| self.parse_transaction(tx.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        // Parse rewards
        let rewards = json["rewards"]
            .as_array()
            .map(|rewards| {
                rewards
                    .iter()
                    .filter_map(|r| serde_json::from_value(r.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(SvmBlock {
            slot,
            blockhash: json["blockhash"].as_str().unwrap_or_default().to_string(),
            parent_slot: json["parentSlot"].as_u64().unwrap_or(0),
            block_time: json["blockTime"].as_i64(),
            block_height: json["blockHeight"].as_u64(),
            transactions,
            rewards,
        })
    }

    /// Parse transaction JSON response
    fn parse_transaction(&self, json: Value) -> ApplicationResult<SvmTransactionWithMeta> {
        let transaction = serde_json::from_value(json["transaction"].clone()).map_err(|e| {
            ApplicationError::ParseError {
                field: "transaction".to_string(),
                value: json["transaction"].to_string(),
                message: e.to_string(),
            }
        })?;

        let meta = json
            .get("meta")
            .and_then(|m| serde_json::from_value(m.clone()).ok());

        Ok(SvmTransactionWithMeta { transaction, meta })
    }

    /// Parse account JSON response
    fn parse_account(&self, json: Value) -> ApplicationResult<SvmAccount> {
        Ok(SvmAccount {
            lamports: json["lamports"].as_u64().unwrap_or(0),
            owner: json["owner"].as_str().unwrap_or_default().to_string(),
            data: AccountData::Base64(json["data"][0].as_str().unwrap_or_default().to_string()),
            executable: json["executable"].as_bool().unwrap_or(false),
            rent_epoch: json["rentEpoch"].as_u64().unwrap_or(0),
        })
    }

    /// Get commitment configuration
    fn get_commitment_config(&self) -> Value {
        json!({
            "commitment": self.get_commitment_string()
        })
    }

    /// Get commitment string
    fn get_commitment_string(&self) -> &str {
        match self.config.commitment {
            CommitmentLevel::Processed => "processed",
            CommitmentLevel::Confirmed => "confirmed",
            CommitmentLevel::Finalized => "finalized",
        }
    }

    /// Check gateway health
    async fn check_health(&self) {
        match self.get_slot().await {
            Ok(slot) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = true;
                health.last_slot = Some(slot);
                health.last_error = None;
                health.last_check = std::time::Instant::now();
                info!("SVM gateway health check passed, slot: {}", slot);
            }
            Err(e) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = false;
                health.last_error = Some(e.to_string());
                health.last_check = std::time::Instant::now();
                error!("SVM gateway health check failed: {}", e);
            }
        }
    }

    /// Get gateway health status
    pub async fn is_healthy(&self) -> bool {
        let health = self.health_status.read().await;

        // Consider unhealthy if last check was more than 60 seconds ago
        if health.last_check.elapsed() > Duration::from_secs(60) {
            return false;
        }

        health.is_healthy
    }
}

impl Default for SvmGatewayConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            ws_url: None,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            commitment: CommitmentLevel::Confirmed,
            preflight_checks: true,
        }
    }
}

impl std::fmt::Debug for ProductionSvmGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionSvmGateway")
            .field("rpc_url", &self.config.rpc_url)
            .field("commitment", &self.config.commitment)
            .field("preflight_checks", &self.config.preflight_checks)
            .finish()
    }
}
