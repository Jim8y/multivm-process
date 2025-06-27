//! JSON-RPC client for Reth node communication
//!
//! This module provides a comprehensive RPC client for communicating with Reth nodes
//! via standard Ethereum JSON-RPC methods. It includes connection pooling, retry logic,
//! and proper error handling for production use.

use crate::engine::RethEngineError;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, warn};

/// JSON-RPC client for Reth node communication
pub struct RethRpcClient {
    client: Client,
    rpc_url: String,
    _request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

/// Transaction for RPC submission
#[derive(Debug, Clone)]
pub struct RpcTransaction {
    pub from: Option<String>,
    pub to: Option<String>,
    pub gas: Option<String>,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub value: Option<String>,
    pub data: Option<String>,
    pub nonce: Option<String>,
    pub transaction_type: Option<String>,
    pub access_list: Option<Vec<Value>>,
}

/// Block information from RPC
#[derive(Debug, Clone)]
pub struct RpcBlock {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee_per_gas: Option<u64>,
    pub transactions: Vec<Value>,
    pub state_root: String,
    pub receipts_root: String,
}

/// Transaction receipt from RPC
#[derive(Debug, Clone)]
pub struct RpcTransactionReceipt {
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub block_hash: String,
    pub block_number: u64,
    pub from: String,
    pub to: Option<String>,
    pub cumulative_gas_used: u64,
    pub gas_used: u64,
    pub contract_address: Option<String>,
    pub logs: Vec<Value>,
    pub status: u64,
    pub effective_gas_price: u64,
}

impl RethRpcClient {
    /// Create a new RPC client
    pub fn new(
        rpc_url: String,
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
            .map_err(|e| RethEngineError::Rpc(format!("Failed to create RPC client: {e}")))?;

        Ok(Self {
            client,
            rpc_url,
            _request_timeout: request_timeout,
            max_retries,
            retry_delay,
        })
    }

    /// Get chain ID
    pub async fn get_chain_id(&self) -> Result<u64, RethEngineError> {
        let response = self.make_request("eth_chainId", json!([])).await?;

        if let Some(chain_id_hex) = response.get("result").and_then(|r| r.as_str()) {
            let chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid chain ID: {e}")))?;
            Ok(chain_id)
        } else {
            Err(RethEngineError::Rpc(
                "Invalid chain ID response".to_string(),
            ))
        }
    }

    /// Get current block number
    pub async fn get_block_number(&self) -> Result<u64, RethEngineError> {
        let response = self.make_request("eth_blockNumber", json!([])).await?;

        if let Some(block_hex) = response.get("result").and_then(|r| r.as_str()) {
            let block_number = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid block number: {e}")))?;
            Ok(block_number)
        } else {
            Err(RethEngineError::Rpc(
                "Invalid block number response".to_string(),
            ))
        }
    }

    /// Get gas price
    pub async fn get_gas_price(&self) -> Result<u64, RethEngineError> {
        let response = self.make_request("eth_gasPrice", json!([])).await?;

        if let Some(gas_price_hex) = response.get("result").and_then(|r| r.as_str()) {
            let gas_price = u64::from_str_radix(gas_price_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid gas price: {e}")))?;
            Ok(gas_price)
        } else {
            Err(RethEngineError::Rpc(
                "Invalid gas price response".to_string(),
            ))
        }
    }

    /// Get balance of an address
    pub async fn get_balance(
        &self,
        address: &str,
        block: Option<&str>,
    ) -> Result<u64, RethEngineError> {
        let block_param = block.unwrap_or("latest");
        let response = self
            .make_request("eth_getBalance", json!([address, block_param]))
            .await?;

        if let Some(balance_hex) = response.get("result").and_then(|r| r.as_str()) {
            let balance = u64::from_str_radix(balance_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid balance: {e}")))?;
            Ok(balance)
        } else {
            Err(RethEngineError::Rpc("Invalid balance response".to_string()))
        }
    }

    /// Get transaction count (nonce) for an address
    pub async fn get_transaction_count(
        &self,
        address: &str,
        block: Option<&str>,
    ) -> Result<u64, RethEngineError> {
        let block_param = block.unwrap_or("latest");
        let response = self
            .make_request("eth_getTransactionCount", json!([address, block_param]))
            .await?;

        if let Some(count_hex) = response.get("result").and_then(|r| r.as_str()) {
            let count = u64::from_str_radix(count_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid transaction count: {e}")))?;
            Ok(count)
        } else {
            Err(RethEngineError::Rpc(
                "Invalid transaction count response".to_string(),
            ))
        }
    }

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, transaction: &RpcTransaction) -> Result<u64, RethEngineError> {
        let tx_object = self.transaction_to_json(transaction);
        let response = self
            .make_request("eth_estimateGas", json!([tx_object]))
            .await?;

        if let Some(gas_hex) = response.get("result").and_then(|r| r.as_str()) {
            let gas_estimate = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid gas estimate: {e}")))?;
            Ok(gas_estimate)
        } else {
            Err(RethEngineError::Rpc(
                "Invalid gas estimation response".to_string(),
            ))
        }
    }

    /// Send raw transaction
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> Result<String, RethEngineError> {
        let response = self
            .make_request("eth_sendRawTransaction", json!([raw_tx]))
            .await?;

        if let Some(tx_hash) = response.get("result").and_then(|r| r.as_str()) {
            Ok(tx_hash.to_string())
        } else if let Some(error) = response.get("error") {
            Err(RethEngineError::Rpc(format!(
                "Transaction rejected: {error}"
            )))
        } else {
            Err(RethEngineError::Rpc(
                "Invalid transaction response".to_string(),
            ))
        }
    }

    /// Get block by number
    pub async fn get_block_by_number(
        &self,
        block_number: u64,
        full_transactions: bool,
    ) -> Result<Option<RpcBlock>, RethEngineError> {
        let block_hex = format!("0x{block_number:x}");
        let response = self
            .make_request(
                "eth_getBlockByNumber",
                json!([block_hex, full_transactions]),
            )
            .await?;

        if let Some(block_data) = response.get("result") {
            if block_data.is_null() {
                return Ok(None);
            }

            let block = self.parse_block(block_data)?;
            Ok(Some(block))
        } else {
            Err(RethEngineError::Rpc("Invalid block response".to_string()))
        }
    }

    /// Get block by hash
    pub async fn get_block_by_hash(
        &self,
        block_hash: &str,
        full_transactions: bool,
    ) -> Result<Option<RpcBlock>, RethEngineError> {
        let response = self
            .make_request("eth_getBlockByHash", json!([block_hash, full_transactions]))
            .await?;

        if let Some(block_data) = response.get("result") {
            if block_data.is_null() {
                return Ok(None);
            }

            let block = self.parse_block(block_data)?;
            Ok(Some(block))
        } else {
            Err(RethEngineError::Rpc("Invalid block response".to_string()))
        }
    }

    /// Get transaction by hash
    pub async fn get_transaction_by_hash(
        &self,
        tx_hash: &str,
    ) -> Result<Option<Value>, RethEngineError> {
        let response = self
            .make_request("eth_getTransactionByHash", json!([tx_hash]))
            .await?;

        if let Some(tx_data) = response.get("result") {
            if tx_data.is_null() {
                Ok(None)
            } else {
                Ok(Some(tx_data.clone()))
            }
        } else {
            Err(RethEngineError::Rpc(
                "Invalid transaction response".to_string(),
            ))
        }
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<RpcTransactionReceipt>, RethEngineError> {
        let response = self
            .make_request("eth_getTransactionReceipt", json!([tx_hash]))
            .await?;

        if let Some(receipt_data) = response.get("result") {
            if receipt_data.is_null() {
                return Ok(None);
            }

            let receipt = self.parse_transaction_receipt(receipt_data)?;
            Ok(Some(receipt))
        } else {
            Err(RethEngineError::Rpc("Invalid receipt response".to_string()))
        }
    }

    /// Call a contract method
    pub async fn call(
        &self,
        transaction: &RpcTransaction,
        block: Option<&str>,
    ) -> Result<String, RethEngineError> {
        let tx_object = self.transaction_to_json(transaction);
        let block_param = block.unwrap_or("latest");
        let response = self
            .make_request("eth_call", json!([tx_object, block_param]))
            .await?;

        if let Some(result) = response.get("result").and_then(|r| r.as_str()) {
            Ok(result.to_string())
        } else if let Some(error) = response.get("error") {
            Err(RethEngineError::Rpc(format!(
                "Contract call failed: {error}"
            )))
        } else {
            Err(RethEngineError::Rpc("Invalid call response".to_string()))
        }
    }

    /// Get logs
    pub async fn get_logs(&self, filter: &Value) -> Result<Vec<Value>, RethEngineError> {
        let response = self.make_request("eth_getLogs", json!([filter])).await?;

        if let Some(logs) = response.get("result").and_then(|r| r.as_array()) {
            Ok(logs.clone())
        } else {
            Err(RethEngineError::Rpc("Invalid logs response".to_string()))
        }
    }

    /// Get network version
    pub async fn get_network_version(&self) -> Result<String, RethEngineError> {
        let response = self.make_request("net_version", json!([])).await?;

        if let Some(version) = response.get("result").and_then(|r| r.as_str()) {
            Ok(version.to_string())
        } else {
            Err(RethEngineError::Rpc(
                "Invalid network version response".to_string(),
            ))
        }
    }

    /// Get peer count
    pub async fn get_peer_count(&self) -> Result<u64, RethEngineError> {
        let response = self.make_request("net_peerCount", json!([])).await?;

        if let Some(count_hex) = response.get("result").and_then(|r| r.as_str()) {
            let count = u64::from_str_radix(count_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid peer count: {e}")))?;
            Ok(count)
        } else {
            Err(RethEngineError::Rpc(
                "Invalid peer count response".to_string(),
            ))
        }
    }

    /// Check if node is syncing
    pub async fn is_syncing(&self) -> Result<bool, RethEngineError> {
        let response = self.make_request("eth_syncing", json!([])).await?;

        if let Some(result) = response.get("result") {
            if result.is_boolean() {
                Ok(result.as_bool().unwrap_or(false))
            } else if result.is_object() {
                // Syncing object returned, node is syncing
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(RethEngineError::Rpc("Invalid syncing response".to_string()))
        }
    }

    /// Get client version
    pub async fn get_client_version(&self) -> Result<String, RethEngineError> {
        let response = self.make_request("web3_clientVersion", json!([])).await?;

        if let Some(version) = response.get("result").and_then(|r| r.as_str()) {
            Ok(version.to_string())
        } else {
            Err(RethEngineError::Rpc(
                "Invalid client version response".to_string(),
            ))
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, RethEngineError> {
        match self.get_chain_id().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Make a JSON-RPC request with retry logic
    async fn make_request(&self, method: &str, params: Value) -> Result<Value, RethEngineError> {
        for attempt in 1..=self.max_retries {
            let request_id = format!("{method}_{attempt}");
            let rpc_request = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params
            });

            debug!("RPC request: {} (attempt {})", method, attempt);

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
                            debug!("RPC response: {} succeeded", method);
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Failed to parse RPC response on attempt {}: {}", attempt, e);
                            if attempt == self.max_retries {
                                return Err(RethEngineError::Rpc(format!(
                                    "Failed to parse RPC response: {e}"
                                )));
                            }
                        }
                    }
                }
                Ok(response) => {
                    warn!(
                        "RPC returned error status on attempt {}: {}",
                        attempt,
                        response.status()
                    );
                    if attempt == self.max_retries {
                        return Err(RethEngineError::Rpc(format!(
                            "RPC returned error status: {}",
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    warn!("RPC request failed on attempt {}: {}", attempt, e);
                    if attempt == self.max_retries {
                        return Err(RethEngineError::Rpc(format!("RPC request failed: {e}")));
                    }
                }
            }

            // Wait before retry
            if attempt < self.max_retries {
                tokio::time::sleep(self.retry_delay).await;
            }
        }

        // This should never be reached due to the logic above, but just in case
        Err(RethEngineError::Rpc(format!(
            "All retry attempts exhausted for RPC method: {method}"
        )))
    }

    /// Convert RpcTransaction to JSON object
    fn transaction_to_json(&self, transaction: &RpcTransaction) -> Value {
        let mut tx_object = json!({});

        if let Some(from) = &transaction.from {
            tx_object["from"] = json!(from);
        }
        if let Some(to) = &transaction.to {
            tx_object["to"] = json!(to);
        }
        if let Some(gas) = &transaction.gas {
            tx_object["gas"] = json!(gas);
        }
        if let Some(gas_price) = &transaction.gas_price {
            tx_object["gasPrice"] = json!(gas_price);
        }
        if let Some(max_fee) = &transaction.max_fee_per_gas {
            tx_object["maxFeePerGas"] = json!(max_fee);
        }
        if let Some(max_priority_fee) = &transaction.max_priority_fee_per_gas {
            tx_object["maxPriorityFeePerGas"] = json!(max_priority_fee);
        }
        if let Some(value) = &transaction.value {
            tx_object["value"] = json!(value);
        }
        if let Some(data) = &transaction.data {
            tx_object["data"] = json!(data);
        }
        if let Some(nonce) = &transaction.nonce {
            tx_object["nonce"] = json!(nonce);
        }
        if let Some(tx_type) = &transaction.transaction_type {
            tx_object["type"] = json!(tx_type);
        }
        if let Some(access_list) = &transaction.access_list {
            tx_object["accessList"] = json!(access_list);
        }

        tx_object
    }

    /// Parse block data from JSON
    fn parse_block(&self, block_data: &Value) -> Result<RpcBlock, RethEngineError> {
        let number_hex = block_data
            .get("number")
            .and_then(|n| n.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing block number".to_string()))?;
        let number = u64::from_str_radix(number_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid block number: {e}")))?;

        let hash = block_data
            .get("hash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing block hash".to_string()))?
            .to_string();

        let parent_hash = block_data
            .get("parentHash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing parent hash".to_string()))?
            .to_string();

        let timestamp_hex = block_data
            .get("timestamp")
            .and_then(|t| t.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing timestamp".to_string()))?;
        let timestamp = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid timestamp: {e}")))?;

        let gas_limit_hex = block_data
            .get("gasLimit")
            .and_then(|g| g.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing gas limit".to_string()))?;
        let gas_limit = u64::from_str_radix(gas_limit_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid gas limit: {e}")))?;

        let gas_used_hex = block_data
            .get("gasUsed")
            .and_then(|g| g.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing gas used".to_string()))?;
        let gas_used = u64::from_str_radix(gas_used_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid gas used: {e}")))?;

        let base_fee_per_gas = block_data
            .get("baseFeePerGas")
            .and_then(|b| b.as_str())
            .map(|hex| u64::from_str_radix(hex.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| RethEngineError::Rpc(format!("Invalid base fee: {e}")))?;

        let transactions = block_data
            .get("transactions")
            .and_then(|t| t.as_array())
            .ok_or_else(|| RethEngineError::Rpc("Missing transactions".to_string()))?
            .clone();

        let state_root = block_data
            .get("stateRoot")
            .and_then(|s| s.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing state root".to_string()))?
            .to_string();

        let receipts_root = block_data
            .get("receiptsRoot")
            .and_then(|r| r.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing receipts root".to_string()))?
            .to_string();

        Ok(RpcBlock {
            number,
            hash,
            parent_hash,
            timestamp,
            gas_limit,
            gas_used,
            base_fee_per_gas,
            transactions,
            state_root,
            receipts_root,
        })
    }

    /// Parse transaction receipt from JSON
    fn parse_transaction_receipt(
        &self,
        receipt_data: &Value,
    ) -> Result<RpcTransactionReceipt, RethEngineError> {
        let transaction_hash = receipt_data
            .get("transactionHash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing transaction hash".to_string()))?
            .to_string();

        let transaction_index_hex = receipt_data
            .get("transactionIndex")
            .and_then(|i| i.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing transaction index".to_string()))?;
        let transaction_index =
            u64::from_str_radix(transaction_index_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid transaction index: {e}")))?;

        let block_hash = receipt_data
            .get("blockHash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing block hash".to_string()))?
            .to_string();

        let block_number_hex = receipt_data
            .get("blockNumber")
            .and_then(|n| n.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing block number".to_string()))?;
        let block_number = u64::from_str_radix(block_number_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid block number: {e}")))?;

        let from = receipt_data
            .get("from")
            .and_then(|f| f.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing from address".to_string()))?
            .to_string();

        let to = receipt_data
            .get("to")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let cumulative_gas_used_hex = receipt_data
            .get("cumulativeGasUsed")
            .and_then(|g| g.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing cumulative gas used".to_string()))?;
        let cumulative_gas_used =
            u64::from_str_radix(cumulative_gas_used_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid cumulative gas used: {e}")))?;

        let gas_used_hex = receipt_data
            .get("gasUsed")
            .and_then(|g| g.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing gas used".to_string()))?;
        let gas_used = u64::from_str_radix(gas_used_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid gas used: {e}")))?;

        let contract_address = receipt_data
            .get("contractAddress")
            .and_then(|a| a.as_str())
            .map(|s| s.to_string());

        let logs = receipt_data
            .get("logs")
            .and_then(|l| l.as_array())
            .ok_or_else(|| RethEngineError::Rpc("Missing logs".to_string()))?
            .clone();

        let status_hex = receipt_data
            .get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing status".to_string()))?;
        let status = u64::from_str_radix(status_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid status: {e}")))?;

        let effective_gas_price_hex = receipt_data
            .get("effectiveGasPrice")
            .and_then(|p| p.as_str())
            .ok_or_else(|| RethEngineError::Rpc("Missing effective gas price".to_string()))?;
        let effective_gas_price =
            u64::from_str_radix(effective_gas_price_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid effective gas price: {e}")))?;

        Ok(RpcTransactionReceipt {
            transaction_hash,
            transaction_index,
            block_hash,
            block_number,
            from,
            to,
            cumulative_gas_used,
            gas_used,
            contract_address,
            logs,
            status,
            effective_gas_price,
        })
    }
}

/// Builder for RethRpcClient
pub struct RethRpcClientBuilder {
    rpc_url: Option<String>,
    request_timeout: Duration,
    max_retries: u32,
    retry_delay: Duration,
}

impl RethRpcClientBuilder {
    pub fn new() -> Self {
        Self {
            rpc_url: None,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    pub fn rpc_url(mut self, url: String) -> Self {
        self.rpc_url = Some(url);
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

    pub fn build(self) -> Result<RethRpcClient, RethEngineError> {
        let rpc_url = self
            .rpc_url
            .ok_or_else(|| RethEngineError::Configuration("RPC URL is required".to_string()))?;

        RethRpcClient::new(
            rpc_url,
            self.request_timeout,
            self.max_retries,
            self.retry_delay,
        )
    }
}

impl Default for RethRpcClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
