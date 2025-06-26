//! JSON-RPC client for Solana validator communication
//! 
//! This module provides a comprehensive RPC client for communicating with Solana validators
//! via standard Solana JSON-RPC methods. It includes connection pooling, retry logic,
//! and proper error handling for production use.

use crate::engine::SolanaEngineError;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, warn};
use solana_sdk::{
    signature::Signature,
    slot_history::Slot,
    pubkey::Pubkey,
    commitment_config::{CommitmentConfig, CommitmentLevel},
};

/// JSON-RPC client for Solana validator communication
pub struct SolanaRpcClient {
    client: Client,
    rpc_url: String,
    ws_url: Option<String>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
    commitment: CommitmentConfig,
}

/// Transaction for RPC submission
#[derive(Debug, Clone)]
pub struct SolanaRpcTransaction {
    pub from: Option<String>,
    pub to: Option<String>,
    pub lamports: Option<u64>,
    pub data: Option<Vec<u8>>,
    pub recent_blockhash: Option<String>,
    pub signatures: Vec<String>,
    pub compute_units: Option<u64>,
    pub compute_unit_price: Option<u64>,
}

/// Block information from RPC
#[derive(Debug, Clone)]
pub struct SolanaRpcBlock {
    pub slot: Slot,
    pub block_hash: String,
    pub parent_slot: Slot,
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
    pub transactions: Vec<Value>,
    pub rewards: Vec<Value>,
    pub previous_blockhash: String,
}

/// Transaction receipt from RPC
#[derive(Debug, Clone)]
pub struct SolanaRpcTransactionReceipt {
    pub signature: String,
    pub slot: Slot,
    pub block_time: Option<i64>,
    pub confirmation_status: String,
    pub err: Option<Value>,
    pub memo: Option<String>,
    pub compute_units_consumed: Option<u64>,
    pub fee: u64,
}

/// Account information from RPC
#[derive(Debug, Clone)]
pub struct SolanaAccountInfo {
    pub pubkey: String,
    pub lamports: u64,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
}

/// Slot information
#[derive(Debug, Clone)]
pub struct SolanaSlotInfo {
    pub slot: Slot,
    pub parent: Slot,
    pub root: Slot,
}

impl SolanaRpcClient {
    /// Create a new RPC client
    pub fn new(
        rpc_url: String,
        ws_url: Option<String>,
        request_timeout: Duration,
        max_retries: u32,
        retry_delay: Duration,
        commitment: CommitmentConfig,
    ) -> Result<Self, SolanaEngineError> {
        let client = Client::builder()
            .timeout(request_timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(|e| SolanaEngineError::Rpc(format!("Failed to create RPC client: {}", e)))?;

        Ok(Self {
            client,
            rpc_url,
            ws_url,
            request_timeout,
            max_retries,
            retry_delay,
            commitment,
        })
    }

    /// Get current slot
    pub async fn get_slot(&self) -> Result<Slot, SolanaEngineError> {
        let response = self.make_request("getSlot", json!([self.commitment])).await?;
        
        if let Some(slot) = response.get("result").and_then(|r| r.as_u64()) {
            Ok(slot)
        } else {
            Err(SolanaEngineError::Rpc("Invalid slot response".to_string()))
        }
    }

    /// Get block height
    pub async fn get_block_height(&self) -> Result<u64, SolanaEngineError> {
        let response = self.make_request("getBlockHeight", json!([self.commitment])).await?;
        
        if let Some(height) = response.get("result").and_then(|r| r.as_u64()) {
            Ok(height)
        } else {
            Err(SolanaEngineError::Rpc("Invalid block height response".to_string()))
        }
    }

    /// Get balance of an account
    pub async fn get_balance(&self, pubkey: &str) -> Result<u64, SolanaEngineError> {
        let response = self.make_request("getBalance", json!([pubkey, self.commitment])).await?;
        
        if let Some(balance_obj) = response.get("result") {
            if let Some(value) = balance_obj.get("value").and_then(|v| v.as_u64()) {
                Ok(value)
            } else {
                Err(SolanaEngineError::Rpc("Invalid balance response format".to_string()))
            }
        } else {
            Err(SolanaEngineError::Rpc("Invalid balance response".to_string()))
        }
    }

    /// Get account information
    pub async fn get_account_info(&self, pubkey: &str) -> Result<Option<SolanaAccountInfo>, SolanaEngineError> {
        let response = self.make_request("getAccountInfo", json!([pubkey, {"encoding": "base64", "commitment": self.commitment.commitment}])).await?;
        
        if let Some(result) = response.get("result") {
            if result.get("value").is_none() || result.get("value").unwrap().is_null() {
                return Ok(None);
            }
            
            let account_data = result.get("value").unwrap();
            let account_info = self.parse_account_info(pubkey, account_data)?;
            Ok(Some(account_info))
        } else {
            Err(SolanaEngineError::Rpc("Invalid account info response".to_string()))
        }
    }

    /// Send transaction
    pub async fn send_transaction(&self, transaction: &str) -> Result<String, SolanaEngineError> {
        let response = self.make_request("sendTransaction", json!([transaction, {"encoding": "base64"}])).await?;
        
        if let Some(signature) = response.get("result").and_then(|r| r.as_str()) {
            Ok(signature.to_string())
        } else if let Some(error) = response.get("error") {
            Err(SolanaEngineError::Rpc(format!("Transaction rejected: {}", error)))
        } else {
            Err(SolanaEngineError::Rpc("Invalid transaction response".to_string()))
        }
    }

    /// Send and confirm transaction
    pub async fn send_and_confirm_transaction(&self, transaction: &str) -> Result<String, SolanaEngineError> {
        // First send the transaction
        let signature = self.send_transaction(transaction).await?;
        
        // Then wait for confirmation
        self.confirm_transaction(&signature).await?;
        
        Ok(signature)
    }

    /// Confirm transaction
    pub async fn confirm_transaction(&self, signature: &str) -> Result<(), SolanaEngineError> {
        let max_attempts = 30; // 30 seconds with 1 second intervals
        
        for _ in 0..max_attempts {
            match self.get_signature_status(signature).await? {
                Some(status) => {
                    if status.confirmation_status == "finalized" || status.confirmation_status == "confirmed" {
                        if status.err.is_some() {
                            return Err(SolanaEngineError::Transaction(format!(
                                "Transaction failed: {:?}", status.err
                            )));
                        }
                        return Ok(());
                    }
                }
                None => {
                    // Transaction not found yet, continue waiting
                }
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        
        Err(SolanaEngineError::Transaction("Transaction confirmation timeout".to_string()))
    }

    /// Get signature status
    pub async fn get_signature_status(&self, signature: &str) -> Result<Option<SolanaRpcTransactionReceipt>, SolanaEngineError> {
        let response = self.make_request("getSignatureStatuses", json!([[signature], {"searchTransactionHistory": true}])).await?;
        
        if let Some(result) = response.get("result") {
            if let Some(value) = result.get("value").and_then(|v| v.as_array()) {
                if let Some(status) = value.get(0) {
                    if status.is_null() {
                        return Ok(None);
                    }
                    
                    let receipt = self.parse_transaction_receipt(signature, status)?;
                    return Ok(Some(receipt));
                }
            }
        }
        
        Err(SolanaEngineError::Rpc("Invalid signature status response".to_string()))
    }

    /// Get block by slot
    pub async fn get_block(&self, slot: Slot) -> Result<Option<SolanaRpcBlock>, SolanaEngineError> {
        let response = self.make_request("getBlock", json!([slot, {"encoding": "json", "transactionDetails": "full", "rewards": false}])).await?;
        
        if let Some(block_data) = response.get("result") {
            if block_data.is_null() {
                return Ok(None);
            }
            
            let block = self.parse_block(slot, block_data)?;
            Ok(Some(block))
        } else {
            Err(SolanaEngineError::Rpc("Invalid block response".to_string()))
        }
    }

    /// Get latest blockhash
    pub async fn get_latest_blockhash(&self) -> Result<String, SolanaEngineError> {
        let response = self.make_request("getLatestBlockhash", json!([self.commitment])).await?;
        
        if let Some(result) = response.get("result") {
            if let Some(blockhash) = result.get("value").and_then(|v| v.get("blockhash")).and_then(|b| b.as_str()) {
                Ok(blockhash.to_string())
            } else {
                Err(SolanaEngineError::Rpc("Invalid blockhash response format".to_string()))
            }
        } else {
            Err(SolanaEngineError::Rpc("Invalid blockhash response".to_string()))
        }
    }

    /// Get transaction count
    pub async fn get_transaction_count(&self) -> Result<u64, SolanaEngineError> {
        let response = self.make_request("getTransactionCount", json!([self.commitment])).await?;
        
        if let Some(count) = response.get("result").and_then(|r| r.as_u64()) {
            Ok(count)
        } else {
            Err(SolanaEngineError::Rpc("Invalid transaction count response".to_string()))
        }
    }

    /// Get version information
    pub async fn get_version(&self) -> Result<Value, SolanaEngineError> {
        let response = self.make_request("getVersion", json!([])).await?;
        
        if let Some(version) = response.get("result") {
            Ok(version.clone())
        } else {
            Err(SolanaEngineError::Rpc("Invalid version response".to_string()))
        }
    }

    /// Get health status
    pub async fn get_health(&self) -> Result<String, SolanaEngineError> {
        let response = self.make_request("getHealth", json!([])).await?;
        
        if let Some(health) = response.get("result").and_then(|r| r.as_str()) {
            Ok(health.to_string())
        } else {
            // Health endpoint returns "ok" as a string, not in result field
            if response.as_str() == Some("ok") {
                Ok("ok".to_string())
            } else {
                Err(SolanaEngineError::Rpc("Invalid health response".to_string()))
            }
        }
    }

    /// Estimate compute units for a transaction
    pub async fn simulate_transaction(&self, transaction: &str) -> Result<Value, SolanaEngineError> {
        let response = self.make_request("simulateTransaction", json!([transaction, {"encoding": "base64", "commitment": self.commitment.commitment}])).await?;
        
        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else {
            Err(SolanaEngineError::Rpc("Invalid simulation response".to_string()))
        }
    }

    /// Get minimum balance for rent exemption
    pub async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64, SolanaEngineError> {
        let response = self.make_request("getMinimumBalanceForRentExemption", json!([data_len])).await?;
        
        if let Some(balance) = response.get("result").and_then(|r| r.as_u64()) {
            Ok(balance)
        } else {
            Err(SolanaEngineError::Rpc("Invalid rent exemption response".to_string()))
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, SolanaEngineError> {
        match self.get_health().await {
            Ok(status) => Ok(status == "ok"),
            Err(_) => Ok(false),
        }
    }

    /// Make a JSON-RPC request with retry logic
    async fn make_request(&self, method: &str, params: Value) -> Result<Value, SolanaEngineError> {
        for attempt in 1..=self.max_retries {
            let request_id = format!("{}_{}", method, attempt);
            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params
            });

            debug!("Solana RPC request: {} (attempt {})", method, attempt);

            match self.client
                .post(&self.rpc_url)
                .header("Content-Type", "application/json")
                .json(&rpc_request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    match response.json::<Value>().await {
                        Ok(result) => {
                            debug!("Solana RPC response: {} succeeded", method);
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Failed to parse Solana RPC response on attempt {}: {}", attempt, e);
                            if attempt == self.max_retries {
                                return Err(SolanaEngineError::Rpc(format!(
                                    "Failed to parse RPC response: {}", e
                                )));
                            }
                        }
                    }
                }
                Ok(response) => {
                    warn!("Solana RPC returned error status on attempt {}: {}", attempt, response.status());
                    if attempt == self.max_retries {
                        return Err(SolanaEngineError::Rpc(format!(
                            "RPC returned error status: {}",
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    warn!("Solana RPC request failed on attempt {}: {}", attempt, e);
                    if attempt == self.max_retries {
                        return Err(SolanaEngineError::Rpc(format!("RPC request failed: {}", e)));
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

    /// Parse account information from JSON
    fn parse_account_info(&self, pubkey: &str, account_data: &Value) -> Result<SolanaAccountInfo, SolanaEngineError> {
        let lamports = account_data.get("lamports")
            .and_then(|l| l.as_u64())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing lamports".to_string()))?;

        let owner = account_data.get("owner")
            .and_then(|o| o.as_str())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing owner".to_string()))?
            .to_string();

        let executable = account_data.get("executable")
            .and_then(|e| e.as_bool())
            .unwrap_or(false);

        let rent_epoch = account_data.get("rentEpoch")
            .and_then(|r| r.as_u64())
            .unwrap_or(0);

        let data = if let Some(data_array) = account_data.get("data").and_then(|d| d.as_array()) {
            if let Some(data_str) = data_array.get(0).and_then(|d| d.as_str()) {
                base64::decode(data_str).map_err(|e| {
                    SolanaEngineError::Rpc(format!("Failed to decode account data: {}", e))
                })?
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok(SolanaAccountInfo {
            pubkey: pubkey.to_string(),
            lamports,
            owner,
            executable,
            rent_epoch,
            data,
        })
    }

    /// Parse transaction receipt from JSON
    fn parse_transaction_receipt(&self, signature: &str, status_data: &Value) -> Result<SolanaRpcTransactionReceipt, SolanaEngineError> {
        let slot = status_data.get("slot")
            .and_then(|s| s.as_u64())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing slot in transaction status".to_string()))?;

        let confirmation_status = status_data.get("confirmationStatus")
            .and_then(|c| c.as_str())
            .unwrap_or("processed")
            .to_string();

        let err = status_data.get("err").cloned();

        Ok(SolanaRpcTransactionReceipt {
            signature: signature.to_string(),
            slot,
            block_time: None, // Not available in signature status
            confirmation_status,
            err,
            memo: None,
            compute_units_consumed: None,
            fee: 0, // Not available in signature status
        })
    }

    /// Parse block data from JSON
    fn parse_block(&self, slot: Slot, block_data: &Value) -> Result<SolanaRpcBlock, SolanaEngineError> {
        let block_hash = block_data.get("blockhash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing block hash".to_string()))?
            .to_string();

        let parent_slot = block_data.get("parentSlot")
            .and_then(|p| p.as_u64())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing parent slot".to_string()))?;

        let block_time = block_data.get("blockTime")
            .and_then(|t| t.as_i64());

        let block_height = block_data.get("blockHeight")
            .and_then(|h| h.as_u64());

        let transactions = block_data.get("transactions")
            .and_then(|t| t.as_array())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing transactions".to_string()))?
            .clone();

        let rewards = block_data.get("rewards")
            .and_then(|r| r.as_array())
            .unwrap_or(&vec![])
            .clone();

        let previous_blockhash = block_data.get("previousBlockhash")
            .and_then(|p| p.as_str())
            .ok_or_else(|| SolanaEngineError::Rpc("Missing previous blockhash".to_string()))?
            .to_string();

        Ok(SolanaRpcBlock {
            slot,
            block_hash,
            parent_slot,
            block_time,
            block_height,
            transactions,
            rewards,
            previous_blockhash,
        })
    }
}

/// Builder for SolanaRpcClient
pub struct SolanaRpcClientBuilder {
    rpc_url: Option<String>,
    ws_url: Option<String>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
    commitment: CommitmentConfig,
}

impl SolanaRpcClientBuilder {
    pub fn new() -> Self {
        Self {
            rpc_url: None,
            ws_url: None,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            commitment: CommitmentConfig::confirmed(),
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

    pub fn commitment(mut self, commitment: CommitmentConfig) -> Self {
        self.commitment = commitment;
        self
    }

    pub fn build(self) -> Result<SolanaRpcClient, SolanaEngineError> {
        let rpc_url = self.rpc_url
            .ok_or_else(|| SolanaEngineError::Configuration("RPC URL is required".to_string()))?;

        SolanaRpcClient::new(
            rpc_url,
            self.ws_url,
            self.request_timeout,
            self.max_retries,
            self.retry_delay,
            self.commitment,
        )
    }
}

impl Default for SolanaRpcClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}