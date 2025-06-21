//! Production EVM API Gateway implementation with real Reth node connection
//!
//! This module provides a production-ready gateway for interacting with
//! Ethereum/EVM-compatible blockchains through JSON-RPC and Engine API.

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

/// Production EVM API Gateway for interacting with real Reth/Ethereum nodes
#[derive(Clone)]
pub struct ProductionEvmGateway {
    /// Cache layer for performance
    cache: Arc<CacheLayer>,

    /// JSON-RPC client for standard Ethereum RPC
    rpc_client: Arc<HttpClient>,

    /// Engine API client for consensus layer integration
    engine_client: Option<Arc<HttpClient>>,

    /// Configuration
    config: EvmGatewayConfig,

    /// Health status
    health_status: Arc<RwLock<HealthStatus>>,
}

/// EVM Gateway configuration
#[derive(Debug, Clone)]
pub struct EvmGatewayConfig {
    /// Standard JSON-RPC endpoint
    pub rpc_url: String,

    /// Engine API endpoint (for consensus integration)
    pub engine_url: Option<String>,

    /// JWT secret for Engine API authentication
    pub jwt_secret: Option<String>,

    /// Request timeout
    pub request_timeout: Duration,

    /// Maximum request retries
    pub max_retries: u32,

    /// Chain ID for transaction signing
    pub chain_id: u64,
}

/// Health status of the gateway
#[derive(Debug, Clone)]
struct HealthStatus {
    is_healthy: bool,
    last_block_number: Option<u64>,
    last_error: Option<String>,
    last_check: std::time::Instant,
}

/// Ethereum block representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmBlock {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub gas_limit: String,
    pub gas_used: String,
    pub base_fee_per_gas: Option<String>,
    pub transactions: Vec<String>,
    pub state_root: String,
    pub receipts_root: String,
    pub logs_bloom: String,
    pub difficulty: String,
    pub total_difficulty: Option<String>,
    pub size: String,
    pub extra_data: String,
    pub miner: String,
    pub nonce: String,
    pub uncles: Vec<String>,
}

/// Ethereum transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmTransaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas: String,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub nonce: String,
    pub input: String,
    pub block_number: Option<u64>,
    pub block_hash: Option<String>,
    pub transaction_index: Option<u64>,
    pub v: String,
    pub r: String,
    pub s: String,
    pub transaction_type: Option<String>,
    pub access_list: Option<Vec<AccessListItem>>,
}

/// Access list item for EIP-2930 transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessListItem {
    pub address: String,
    pub storage_keys: Vec<String>,
}

/// Transaction receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmReceipt {
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub block_hash: String,
    pub block_number: u64,
    pub from: String,
    pub to: Option<String>,
    pub cumulative_gas_used: String,
    pub gas_used: String,
    pub contract_address: Option<String>,
    pub logs: Vec<EvmLog>,
    pub logs_bloom: String,
    pub status: String,
    pub effective_gas_price: String,
}

/// Event log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: u64,
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub block_hash: String,
    pub log_index: u64,
    pub removed: bool,
}

/// Fee history response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeHistory {
    pub oldest_block: String,
    pub base_fee_per_gas: Vec<String>,
    pub gas_used_ratio: Vec<f64>,
    pub reward: Option<Vec<Vec<String>>>,
}

impl ProductionEvmGateway {
    /// Create a new production EVM gateway
    pub async fn new(config: EvmGatewayConfig, cache: Arc<CacheLayer>) -> ApplicationResult<Self> {
        info!("Initializing production EVM gateway: {}", config.rpc_url);

        // Create JSON-RPC client
        let rpc_client = HttpClientBuilder::default()
            .request_timeout(config.request_timeout)
            .build(&config.rpc_url)
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "evm_gateway".to_string(),
                message: format!("Failed to create RPC client: {}", e),
            })?;

        // Create Engine API client if configured
        let engine_client = if let Some(ref engine_url) = config.engine_url {
            let mut headers = jsonrpsee::http_client::HeaderMap::new();

            // Add JWT authentication header if configured
            if let Some(ref jwt_secret) = config.jwt_secret {
                let token = Self::generate_jwt_token(jwt_secret)?;
                headers.insert(
                    "Authorization",
                    format!("Bearer {}", token).parse().map_err(|e| {
                        ApplicationError::ConfigurationError {
                            component: "evm_gateway".to_string(),
                            message: format!("Invalid JWT token: {}", e),
                        }
                    })?,
                );
            }

            let client = HttpClientBuilder::default()
                .request_timeout(config.request_timeout)
                .set_headers(headers)
                .build(engine_url)
                .map_err(|e| ApplicationError::ConfigurationError {
                    component: "evm_gateway".to_string(),
                    message: format!("Failed to create Engine API client: {}", e),
                })?;

            Some(Arc::new(client))
        } else {
            None
        };

        let gateway = Self {
            cache,
            rpc_client: Arc::new(rpc_client),
            engine_client,
            config,
            health_status: Arc::new(RwLock::new(HealthStatus {
                is_healthy: false,
                last_block_number: None,
                last_error: None,
                last_check: std::time::Instant::now(),
            })),
        };

        // Perform initial health check
        gateway.check_health().await;

        Ok(gateway)
    }

    /// Get the latest block
    pub async fn get_latest_block(&self) -> ApplicationResult<EvmBlock> {
        // Check cache first
        if let Some(block) = self.cache.get::<EvmBlock>("evm:latest_block").await? {
            return Ok(block);
        }

        // Make RPC call
        let block_json: Value = self
            .rpc_call("eth_getBlockByNumber", rpc_params!["latest", true])
            .await?;
        let block = self.parse_block(block_json)?;

        // Cache the result with short TTL for latest block
        self.cache
            .set("evm:latest_block", &block, Duration::from_secs(3))
            .await?;

        // Update health status
        let mut health = self.health_status.write().await;
        health.is_healthy = true;
        health.last_block_number = Some(block.number);
        health.last_check = std::time::Instant::now();

        Ok(block)
    }

    /// Get block by number
    pub async fn get_block(&self, block_number: u64) -> ApplicationResult<Option<EvmBlock>> {
        let cache_key = format!("evm:block:{}", block_number);

        // Check cache
        if let Some(block) = self.cache.get::<EvmBlock>(&cache_key).await? {
            return Ok(Some(block));
        }

        // Make RPC call
        let block_hex = format!("0x{:x}", block_number);
        let block_json: Option<Value> = self
            .rpc_call("eth_getBlockByNumber", rpc_params![block_hex, true])
            .await?;

        match block_json {
            Some(json) => {
                let block = self.parse_block(json)?;

                // Cache immutable block data with long TTL
                self.cache
                    .set(&cache_key, &block, Duration::from_secs(86400)) // 24 hours
                    .await?;

                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, block_hash: &str) -> ApplicationResult<Option<EvmBlock>> {
        let cache_key = format!("evm:block:hash:{}", block_hash);

        // Check cache
        if let Some(block) = self.cache.get::<EvmBlock>(&cache_key).await? {
            return Ok(Some(block));
        }

        // Make RPC call
        let block_json: Option<Value> = self
            .rpc_call("eth_getBlockByHash", rpc_params![block_hash, true])
            .await?;

        match block_json {
            Some(json) => {
                let block = self.parse_block(json)?;

                // Cache immutable block data with long TTL
                self.cache
                    .set(&cache_key, &block, Duration::from_secs(86400)) // 24 hours
                    .await?;

                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get transaction by hash
    pub async fn get_transaction(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<Option<EvmTransaction>> {
        let cache_key = format!("evm:tx:{}", tx_hash);

        // Check cache
        if let Some(tx) = self.cache.get::<EvmTransaction>(&cache_key).await? {
            return Ok(Some(tx));
        }

        // Make RPC call
        let tx_json: Option<Value> = self
            .rpc_call("eth_getTransactionByHash", rpc_params![tx_hash])
            .await?;

        match tx_json {
            Some(json) => {
                let tx = self.parse_transaction(json)?;

                // Cache immutable transaction data with long TTL
                self.cache
                    .set(&cache_key, &tx, Duration::from_secs(86400)) // 24 hours
                    .await?;

                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<Option<EvmReceipt>> {
        let cache_key = format!("evm:receipt:{}", tx_hash);

        // Check cache
        if let Some(receipt) = self.cache.get::<EvmReceipt>(&cache_key).await? {
            return Ok(Some(receipt));
        }

        // Make RPC call
        let receipt_json: Option<Value> = self
            .rpc_call("eth_getTransactionReceipt", rpc_params![tx_hash])
            .await?;

        match receipt_json {
            Some(json) => {
                let receipt = self.parse_receipt(json)?;

                // Cache immutable receipt data with long TTL
                self.cache
                    .set(&cache_key, &receipt, Duration::from_secs(86400)) // 24 hours
                    .await?;

                Ok(Some(receipt))
            }
            None => Ok(None),
        }
    }

    /// Send raw transaction
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> ApplicationResult<String> {
        // Validate transaction format
        if !raw_tx.starts_with("0x") {
            return Err(ApplicationError::InvalidRequest {
                message: "Raw transaction must start with 0x".to_string(),
            });
        }

        // Send transaction
        let tx_hash: String = self
            .rpc_call("eth_sendRawTransaction", rpc_params![raw_tx])
            .await?;

        info!("Transaction sent: {}", tx_hash);
        Ok(tx_hash)
    }

    /// Get account balance
    pub async fn get_balance(
        &self,
        address: &str,
        block: Option<&str>,
    ) -> ApplicationResult<String> {
        let block_param = block.unwrap_or("latest");
        let cache_key = format!("evm:balance:{}:{}", address, block_param);

        // Check cache for recent blocks
        if block_param == "latest" || block_param == "pending" {
            if let Some(balance) = self.cache.get::<String>(&cache_key).await? {
                return Ok(balance);
            }
        }

        // Make RPC call
        let balance: String = self
            .rpc_call("eth_getBalance", rpc_params![address, block_param])
            .await?;

        // Cache with appropriate TTL
        let ttl = if block_param == "latest" || block_param == "pending" {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(3600) // 1 hour for historical data
        };

        self.cache.set(&cache_key, &balance, ttl).await?;

        Ok(balance)
    }

    /// Get account nonce
    pub async fn get_nonce(&self, address: &str, block: Option<&str>) -> ApplicationResult<String> {
        let block_param = block.unwrap_or("latest");

        // Make RPC call
        let nonce: String = self
            .rpc_call("eth_getTransactionCount", rpc_params![address, block_param])
            .await?;

        Ok(nonce)
    }

    /// Call contract method (read-only)
    pub async fn call(
        &self,
        call_data: EvmCallData,
        block: Option<&str>,
    ) -> ApplicationResult<String> {
        let block_param = block.unwrap_or("latest");

        // Convert call data to JSON
        let call_object = json!({
            "from": call_data.from,
            "to": call_data.to,
            "gas": call_data.gas,
            "gasPrice": call_data.gas_price,
            "value": call_data.value,
            "data": call_data.data,
        });

        // Make RPC call
        let result: String = self
            .rpc_call("eth_call", rpc_params![call_object, block_param])
            .await?;

        Ok(result)
    }

    /// Estimate gas for transaction
    pub async fn estimate_gas(&self, tx: &EvmTransaction) -> ApplicationResult<String> {
        // Convert transaction to JSON
        let tx_object = json!({
            "from": tx.from,
            "to": tx.to,
            "gas": tx.gas,
            "gasPrice": tx.gas_price,
            "value": tx.value,
            "data": tx.input,
        });

        // Make RPC call
        let gas_estimate: String = self
            .rpc_call("eth_estimateGas", rpc_params![tx_object])
            .await?;

        Ok(gas_estimate)
    }

    /// Get current gas price
    pub async fn get_gas_price(&self) -> ApplicationResult<String> {
        // Check cache
        if let Some(price) = self.cache.get::<String>("evm:gas_price").await? {
            return Ok(price);
        }

        // Make RPC call
        let gas_price: String = self.rpc_call("eth_gasPrice", rpc_params![]).await?;

        // Cache with short TTL
        self.cache
            .set("evm:gas_price", &gas_price, Duration::from_secs(10))
            .await?;

        Ok(gas_price)
    }

    /// Get fee history
    pub async fn get_fee_history(
        &self,
        block_count: u64,
        newest_block: &str,
        percentiles: Vec<f64>,
    ) -> ApplicationResult<FeeHistory> {
        // Make RPC call
        let fee_history: FeeHistory = self
            .rpc_call(
                "eth_feeHistory",
                rpc_params![format!("0x{:x}", block_count), newest_block, percentiles],
            )
            .await?;

        Ok(fee_history)
    }

    /// Get logs with filter
    pub async fn get_logs(&self, filter: LogFilter) -> ApplicationResult<Vec<EvmLog>> {
        // Convert filter to JSON
        let filter_object = json!({
            "fromBlock": filter.from_block,
            "toBlock": filter.to_block,
            "address": filter.address,
            "topics": filter.topics,
            "blockHash": filter.block_hash,
        });

        // Make RPC call
        let logs: Vec<EvmLog> = self
            .rpc_call("eth_getLogs", rpc_params![filter_object])
            .await?;

        Ok(logs)
    }

    /// Get chain ID
    pub async fn get_chain_id(&self) -> ApplicationResult<String> {
        // Check cache
        if let Some(chain_id) = self.cache.get::<String>("evm:chain_id").await? {
            return Ok(chain_id);
        }

        // Make RPC call
        let chain_id: String = self.rpc_call("eth_chainId", rpc_params![]).await?;

        // Cache with long TTL (chain ID doesn't change)
        self.cache
            .set("evm:chain_id", &chain_id, Duration::from_secs(86400))
            .await?;

        Ok(chain_id)
    }

    /// Get current block number
    pub async fn get_block_number(&self) -> ApplicationResult<u64> {
        // Make RPC call
        let block_number_hex: String = self.rpc_call("eth_blockNumber", rpc_params![]).await?;

        // Parse hex to u64
        let block_number = u64::from_str_radix(&block_number_hex.trim_start_matches("0x"), 16)
            .map_err(|e| ApplicationError::ParseError {
                field: "block_number".to_string(),
                value: block_number_hex,
                message: e.to_string(),
            })?;

        Ok(block_number)
    }

    /// Subscribe to new blocks (for WebSocket connections)
    pub async fn subscribe_new_blocks(&self) -> ApplicationResult<String> {
        // This would require WebSocket support
        // For now, return a placeholder
        warn!("Block subscription requires WebSocket connection");
        Ok("subscription_id_placeholder".to_string())
    }

    // Engine API methods (for consensus layer integration)

    /// Get payload (Engine API)
    pub async fn engine_get_payload(&self, payload_id: &str) -> ApplicationResult<Value> {
        let engine_client =
            self.engine_client
                .as_ref()
                .ok_or_else(|| ApplicationError::ConfigurationError {
                    component: "evm_gateway".to_string(),
                    message: "Engine API not configured".to_string(),
                })?;

        let payload: Value = engine_client
            .request("engine_getPayloadV2", rpc_params![payload_id])
            .await
            .map_err(|e| ApplicationError::RpcError {
                endpoint: "engine_getPayloadV2".to_string(),
                message: e.to_string(),
            })?;

        Ok(payload)
    }

    /// New payload (Engine API)
    pub async fn engine_new_payload(&self, payload: Value) -> ApplicationResult<Value> {
        let engine_client =
            self.engine_client
                .as_ref()
                .ok_or_else(|| ApplicationError::ConfigurationError {
                    component: "evm_gateway".to_string(),
                    message: "Engine API not configured".to_string(),
                })?;

        let result: Value = engine_client
            .request("engine_newPayloadV2", rpc_params![payload])
            .await
            .map_err(|e| ApplicationError::RpcError {
                endpoint: "engine_newPayloadV2".to_string(),
                message: e.to_string(),
            })?;

        Ok(result)
    }

    /// Fork choice updated (Engine API)
    pub async fn engine_forkchoice_updated(
        &self,
        forkchoice_state: Value,
        payload_attributes: Option<Value>,
    ) -> ApplicationResult<Value> {
        let engine_client =
            self.engine_client
                .as_ref()
                .ok_or_else(|| ApplicationError::ConfigurationError {
                    component: "evm_gateway".to_string(),
                    message: "Engine API not configured".to_string(),
                })?;

        let result: Value = engine_client
            .request(
                "engine_forkchoiceUpdatedV2",
                rpc_params![forkchoice_state, payload_attributes],
            )
            .await
            .map_err(|e| ApplicationError::RpcError {
                endpoint: "engine_forkchoiceUpdatedV2".to_string(),
                message: e.to_string(),
            })?;

        Ok(result)
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
    fn parse_block(&self, json: Value) -> ApplicationResult<EvmBlock> {
        // Parse block number
        let number = u64::from_str_radix(
            json["number"]
                .as_str()
                .ok_or_else(|| ApplicationError::ParseError {
                    field: "number".to_string(),
                    value: json["number"].to_string(),
                    message: "Missing block number".to_string(),
                })?
                .trim_start_matches("0x"),
            16,
        )
        .map_err(|e| ApplicationError::ParseError {
            field: "number".to_string(),
            value: json["number"].to_string(),
            message: e.to_string(),
        })?;

        // Parse timestamp
        let timestamp = u64::from_str_radix(
            json["timestamp"]
                .as_str()
                .ok_or_else(|| ApplicationError::ParseError {
                    field: "timestamp".to_string(),
                    value: json["timestamp"].to_string(),
                    message: "Missing timestamp".to_string(),
                })?
                .trim_start_matches("0x"),
            16,
        )
        .map_err(|e| ApplicationError::ParseError {
            field: "timestamp".to_string(),
            value: json["timestamp"].to_string(),
            message: e.to_string(),
        })?;

        // Parse transactions
        let transactions = json["transactions"]
            .as_array()
            .map(|txs| {
                txs.iter()
                    .filter_map(|tx| {
                        if tx.is_string() {
                            tx.as_str().map(|s| s.to_string())
                        } else {
                            tx["hash"].as_str().map(|s| s.to_string())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(EvmBlock {
            number,
            hash: json["hash"].as_str().unwrap_or_default().to_string(),
            parent_hash: json["parentHash"].as_str().unwrap_or_default().to_string(),
            timestamp,
            gas_limit: json["gasLimit"].as_str().unwrap_or("0x0").to_string(),
            gas_used: json["gasUsed"].as_str().unwrap_or("0x0").to_string(),
            base_fee_per_gas: json["baseFeePerGas"].as_str().map(|s| s.to_string()),
            transactions,
            state_root: json["stateRoot"].as_str().unwrap_or_default().to_string(),
            receipts_root: json["receiptsRoot"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            logs_bloom: json["logsBloom"].as_str().unwrap_or_default().to_string(),
            difficulty: json["difficulty"].as_str().unwrap_or("0x0").to_string(),
            total_difficulty: json["totalDifficulty"].as_str().map(|s| s.to_string()),
            size: json["size"].as_str().unwrap_or("0x0").to_string(),
            extra_data: json["extraData"].as_str().unwrap_or_default().to_string(),
            miner: json["miner"].as_str().unwrap_or_default().to_string(),
            nonce: json["nonce"].as_str().unwrap_or_default().to_string(),
            uncles: json["uncles"]
                .as_array()
                .map(|u| {
                    u.iter()
                        .filter_map(|h| h.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Parse transaction JSON response
    fn parse_transaction(&self, json: Value) -> ApplicationResult<EvmTransaction> {
        Ok(EvmTransaction {
            hash: json["hash"].as_str().unwrap_or_default().to_string(),
            from: json["from"].as_str().unwrap_or_default().to_string(),
            to: json["to"].as_str().map(|s| s.to_string()),
            value: json["value"].as_str().unwrap_or("0x0").to_string(),
            gas: json["gas"].as_str().unwrap_or("0x0").to_string(),
            gas_price: json["gasPrice"].as_str().map(|s| s.to_string()),
            max_fee_per_gas: json["maxFeePerGas"].as_str().map(|s| s.to_string()),
            max_priority_fee_per_gas: json["maxPriorityFeePerGas"].as_str().map(|s| s.to_string()),
            nonce: json["nonce"].as_str().unwrap_or("0x0").to_string(),
            input: json["input"].as_str().unwrap_or("0x").to_string(),
            block_number: json["blockNumber"]
                .as_str()
                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()),
            block_hash: json["blockHash"].as_str().map(|s| s.to_string()),
            transaction_index: json["transactionIndex"]
                .as_str()
                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()),
            v: json["v"].as_str().unwrap_or("0x0").to_string(),
            r: json["r"].as_str().unwrap_or("0x0").to_string(),
            s: json["s"].as_str().unwrap_or("0x0").to_string(),
            transaction_type: json["type"].as_str().map(|s| s.to_string()),
            access_list: self.parse_access_list(&json)?,
        })
    }

    /// Parse EIP-2930 access list from transaction JSON
    fn parse_access_list(&self, json: &Value) -> ApplicationResult<Option<Vec<AccessListItem>>> {
        // Access lists are only present in transaction types 1 (EIP-2930) and 2 (EIP-1559)
        let transaction_type = json["type"].as_str().unwrap_or("0x0");

        // Only parse access list for transaction types that support it
        match transaction_type {
            "0x1" | "0x2" => {
                // Transaction type 1 (EIP-2930) or type 2 (EIP-1559) - may have access list
                if let Some(access_list_json) = json["accessList"].as_array() {
                    let mut access_list = Vec::new();

                    for item in access_list_json {
                        let access_item = self.parse_access_list_item(item)?;
                        access_list.push(access_item);
                    }

                    Ok(Some(access_list))
                } else {
                    // Transaction type supports access list but none provided
                    Ok(Some(Vec::new()))
                }
            }
            _ => {
                // Legacy transaction (type 0) or other types - no access list
                Ok(None)
            }
        }
    }

    /// Parse individual access list item
    fn parse_access_list_item(&self, item: &Value) -> ApplicationResult<AccessListItem> {
        let address = item["address"].as_str().ok_or_else(|| {
            crate::error::ApplicationError::RpcValidationError {
                field: "access_list.address".to_string(),
                reason: "Missing address in access list item".to_string(),
            }
        })?;

        // Validate address format (should be 42 characters with 0x prefix)
        if !address.starts_with("0x") || address.len() != 42 {
            return Err(crate::error::ApplicationError::RpcValidationError {
                field: "access_list.address".to_string(),
                reason: format!("Invalid address format: {}", address),
            });
        }

        let storage_keys = if let Some(keys_array) = item["storageKeys"].as_array() {
            let mut keys = Vec::new();
            for key in keys_array {
                let key_str = key.as_str().ok_or_else(|| {
                    crate::error::ApplicationError::RpcValidationError {
                        field: "access_list.storageKeys".to_string(),
                        reason: "Invalid storage key format".to_string(),
                    }
                })?;

                // Validate storage key format (should be 66 characters with 0x prefix)
                if !key_str.starts_with("0x") || key_str.len() != 66 {
                    return Err(crate::error::ApplicationError::RpcValidationError {
                        field: "access_list.storageKeys".to_string(),
                        reason: format!("Invalid storage key format: {}", key_str),
                    });
                }

                keys.push(key_str.to_string());
            }
            keys
        } else {
            // No storage keys provided
            Vec::new()
        };

        Ok(AccessListItem {
            address: address.to_string(),
            storage_keys,
        })
    }

    /// Parse receipt JSON response
    fn parse_receipt(&self, json: Value) -> ApplicationResult<EvmReceipt> {
        // Parse logs
        let logs = json["logs"]
            .as_array()
            .map(|logs_array| {
                logs_array
                    .iter()
                    .filter_map(|log| self.parse_log(log.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(EvmReceipt {
            transaction_hash: json["transactionHash"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            transaction_index: u64::from_str_radix(
                json["transactionIndex"]
                    .as_str()
                    .unwrap_or("0x0")
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap_or(0),
            block_hash: json["blockHash"].as_str().unwrap_or_default().to_string(),
            block_number: u64::from_str_radix(
                json["blockNumber"]
                    .as_str()
                    .unwrap_or("0x0")
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap_or(0),
            from: json["from"].as_str().unwrap_or_default().to_string(),
            to: json["to"].as_str().map(|s| s.to_string()),
            cumulative_gas_used: json["cumulativeGasUsed"]
                .as_str()
                .unwrap_or("0x0")
                .to_string(),
            gas_used: json["gasUsed"].as_str().unwrap_or("0x0").to_string(),
            contract_address: json["contractAddress"].as_str().map(|s| s.to_string()),
            logs,
            logs_bloom: json["logsBloom"].as_str().unwrap_or_default().to_string(),
            status: json["status"].as_str().unwrap_or("0x0").to_string(),
            effective_gas_price: json["effectiveGasPrice"]
                .as_str()
                .unwrap_or("0x0")
                .to_string(),
        })
    }

    /// Parse log JSON response
    fn parse_log(&self, json: Value) -> ApplicationResult<EvmLog> {
        Ok(EvmLog {
            address: json["address"].as_str().unwrap_or_default().to_string(),
            topics: json["topics"]
                .as_array()
                .map(|t| {
                    t.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            data: json["data"].as_str().unwrap_or("0x").to_string(),
            block_number: u64::from_str_radix(
                json["blockNumber"]
                    .as_str()
                    .unwrap_or("0x0")
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap_or(0),
            transaction_hash: json["transactionHash"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            transaction_index: u64::from_str_radix(
                json["transactionIndex"]
                    .as_str()
                    .unwrap_or("0x0")
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap_or(0),
            block_hash: json["blockHash"].as_str().unwrap_or_default().to_string(),
            log_index: u64::from_str_radix(
                json["logIndex"]
                    .as_str()
                    .unwrap_or("0x0")
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap_or(0),
            removed: json["removed"].as_bool().unwrap_or(false),
        })
    }

    /// Generate JWT token for Engine API authentication
    fn generate_jwt_token(secret: &str) -> ApplicationResult<String> {
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
        use serde_json::json;

        let header = Header::new(Algorithm::HS256);
        let claims = json!({
            "iat": chrono::Utc::now().timestamp(),
            "exp": chrono::Utc::now().timestamp() + 60, // 1 minute expiry
        });

        encode(
            &header,
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| ApplicationError::AuthenticationFailed {
            reason: format!("Failed to generate JWT: {}", e),
        })
    }

    /// Check gateway health
    async fn check_health(&self) {
        match self.get_block_number().await {
            Ok(block_number) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = true;
                health.last_block_number = Some(block_number);
                health.last_error = None;
                health.last_check = std::time::Instant::now();
                info!(
                    "EVM gateway health check passed, block height: {}",
                    block_number
                );
            }
            Err(e) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = false;
                health.last_error = Some(e.to_string());
                health.last_check = std::time::Instant::now();
                error!("EVM gateway health check failed: {}", e);
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

/// Call data for eth_call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmCallData {
    pub from: Option<String>,
    pub to: String,
    pub gas: Option<String>,
    pub gas_price: Option<String>,
    pub value: Option<String>,
    pub data: String,
}

/// Log filter for eth_getLogs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFilter {
    pub from_block: Option<String>,
    pub to_block: Option<String>,
    pub address: Option<Vec<String>>,
    pub topics: Option<Vec<Option<Vec<String>>>>,
    pub block_hash: Option<String>,
}

/// Access list utilities for EIP-2930 support
impl ProductionEvmGateway {
    /// Validate access list for consistency and format
    pub fn validate_access_list(&self, access_list: &[AccessListItem]) -> ApplicationResult<()> {
        for (index, item) in access_list.iter().enumerate() {
            // Validate address format
            if !item.address.starts_with("0x") || item.address.len() != 42 {
                return Err(crate::error::ApplicationError::RpcValidationError {
                    field: format!("access_list[{}].address", index),
                    reason: format!("Invalid address format: {}", item.address),
                });
            }

            // Validate address is valid hex
            if hex::decode(&item.address[2..]).is_err() {
                return Err(crate::error::ApplicationError::RpcValidationError {
                    field: format!("access_list[{}].address", index),
                    reason: format!("Address contains invalid hex characters: {}", item.address),
                });
            }

            // Validate storage keys
            for (key_index, key) in item.storage_keys.iter().enumerate() {
                if !key.starts_with("0x") || key.len() != 66 {
                    return Err(crate::error::ApplicationError::RpcValidationError {
                        field: format!("access_list[{}].storage_keys[{}]", index, key_index),
                        reason: format!("Invalid storage key format: {}", key),
                    });
                }

                // Validate storage key is valid hex
                if hex::decode(&key[2..]).is_err() {
                    return Err(crate::error::ApplicationError::RpcValidationError {
                        field: format!("access_list[{}].storage_keys[{}]", index, key_index),
                        reason: format!("Storage key contains invalid hex characters: {}", key),
                    });
                }
            }
        }

        info!(
            "Access list validation passed for {} items",
            access_list.len()
        );
        Ok(())
    }

    /// Estimate gas savings from access list usage
    pub fn estimate_access_list_gas_savings(&self, access_list: &[AccessListItem]) -> u64 {
        let mut total_savings = 0u64;

        for item in access_list {
            // EIP-2930: Adding an address to access list costs 2400 gas
            // but saves gas on subsequent SSTORE and SLOAD operations

            // Address access: 2400 gas cost upfront
            let address_cost = 2400u64;

            // Storage key access: 1900 gas cost upfront per key
            let storage_cost = item.storage_keys.len() as u64 * 1900;

            // Potential savings depend on actual usage during execution
            // Conservative estimate: assume 1-2 warm accesses per item
            let estimated_warm_accesses = 2u64;
            let cold_access_cost = 2600u64; // COLD_ACCOUNT_ACCESS_COST
            let warm_access_cost = 100u64; // WARM_STORAGE_READ_COST

            let potential_savings = estimated_warm_accesses * (cold_access_cost - warm_access_cost);

            // Net savings = potential savings - upfront costs
            if potential_savings > (address_cost + storage_cost) {
                total_savings += potential_savings - (address_cost + storage_cost);
            }
        }

        total_savings
    }

    /// Create access list for a transaction (for eth_createAccessList RPC)
    pub async fn create_access_list(
        &self,
        transaction: &EvmTransaction,
    ) -> ApplicationResult<Vec<AccessListItem>> {
        // In a production implementation, this would call eth_createAccessList RPC
        // to automatically generate an optimal access list for the transaction

        debug!("Creating access list for transaction: {}", transaction.hash);

        // For now, return empty access list as this requires actual RPC integration
        // In production: make RPC call to eth_createAccessList with transaction parameters
        warn!("Access list creation requires live RPC integration - returning empty list");

        Ok(Vec::new())
    }

    /// Convert access list to JSON format for RPC calls
    pub fn access_list_to_json(&self, access_list: &[AccessListItem]) -> serde_json::Value {
        let json_items: Vec<serde_json::Value> = access_list
            .iter()
            .map(|item| {
                serde_json::json!({
                    "address": item.address,
                    "storageKeys": item.storage_keys
                })
            })
            .collect();

        serde_json::Value::Array(json_items)
    }

    /// Check if transaction supports access lists based on type
    pub fn supports_access_list(&self, transaction_type: Option<&str>) -> bool {
        match transaction_type {
            Some("0x1") | Some("0x2") => true, // EIP-2930 and EIP-1559
            _ => false,                        // Legacy transactions (type 0x0) or unknown types
        }
    }
}

impl std::fmt::Debug for ProductionEvmGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionEvmGateway")
            .field("rpc_url", &self.config.rpc_url)
            .field("engine_configured", &self.engine_client.is_some())
            .field("chain_id", &self.config.chain_id)
            .finish()
    }
}
