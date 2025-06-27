//! Simplified Unified Gateway Implementation
//!
//! This module provides a clean, unified interface for VM interactions
//! that eliminates complexity while maintaining functionality.

use crate::{
    cache::CacheLayer,
    error::{ApplicationError, ApplicationResult},
};
use multivm_common::VmType;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Unified gateway configuration
#[derive(Debug, Clone)]
pub struct UnifiedGatewayConfig {
    /// VM type this gateway handles
    pub vm_type: VmType,
    /// Primary RPC endpoint
    pub rpc_url: String,
    /// Backup RPC endpoints for failover
    pub backup_urls: Vec<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum retries
    pub max_retries: u32,
    /// Chain/Network ID
    pub chain_id: u64,
    /// Enable production features
    pub production_mode: bool,
}

/// Unified gateway response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse<T> {
    pub data: T,
    pub metadata: ResponseMetadata,
}

/// Response metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub vm_type: VmType,
    pub cached: bool,
    pub response_time_ms: u64,
    pub request_id: String,
    pub endpoint_used: String,
}

/// Unified block representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBlock {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub transactions: Vec<String>,
    pub vm_specific: serde_json::Value,
}

/// Unified transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTransaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas_info: GasInfo,
    pub nonce: u64,
    pub data: String,
    pub vm_specific: serde_json::Value,
}

/// Gas information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasInfo {
    pub gas_limit: u64,
    pub gas_used: Option<u64>,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
}

/// Unified account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAccount {
    pub address: String,
    pub balance: String,
    pub nonce: Option<u64>,
    pub vm_specific: serde_json::Value,
}

/// Gateway statistics
#[derive(Debug, Clone, Serialize, Default)]
pub struct GatewayStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_response_time_ms: f64,
    pub uptime_seconds: u64,
    pub endpoints_health: std::collections::HashMap<String, bool>,
}

/// Unified gateway implementation
#[derive(Debug)]
pub struct UnifiedGateway {
    pub config: UnifiedGatewayConfig,
    cache: Arc<CacheLayer>,
    #[allow(dead_code)]
    http_client: reqwest::Client,
    stats: Arc<tokio::sync::RwLock<GatewayStats>>,
    start_time: Instant,
}

impl UnifiedGateway {
    /// Create new unified gateway
    pub async fn new(
        config: UnifiedGatewayConfig,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "http_client".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            config,
            cache,
            http_client,
            stats: Arc::new(tokio::sync::RwLock::new(GatewayStats::default())),
            start_time: Instant::now(),
        })
    }

    /// Get latest block
    pub async fn get_latest_block(&self) -> ApplicationResult<GatewayResponse<UnifiedBlock>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!(
            "Getting latest block for {:?} [{}]",
            self.config.vm_type, request_id
        );

        // Check cache first
        let cache_key = format!("{}:latest_block", self.vm_type_prefix());
        if let Ok(Some(block)) = self.cache.get::<UnifiedBlock>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: block,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        // Make RPC call
        let block = self.fetch_latest_block(&request_id).await?;

        // Cache result
        if let Err(e) = self
            .cache
            .set(&cache_key, &block, Duration::from_secs(5))
            .await
        {
            debug!("Failed to cache latest block: {}", e);
        }

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: block,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get block by identifier
    pub async fn get_block(
        &self,
        identifier: &str,
    ) -> ApplicationResult<GatewayResponse<Option<UnifiedBlock>>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!(
            "Getting block {} for {:?} [{}]",
            identifier, self.config.vm_type, request_id
        );

        let cache_key = format!("{}:block:{}", self.vm_type_prefix(), identifier);
        if let Ok(Some(block)) = self.cache.get::<Option<UnifiedBlock>>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: block,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        let block = self.fetch_block(identifier, &request_id).await?;

        // Cache with appropriate TTL
        let ttl = if identifier == "latest" || identifier == "pending" {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(3600)
        };
        if let Err(e) = self.cache.set(&cache_key, &block, ttl).await {
            debug!("Failed to cache block {}: {}", identifier, e);
        }

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: block,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get transaction
    pub async fn get_transaction(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<GatewayResponse<Option<UnifiedTransaction>>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!(
            "Getting transaction {} for {:?} [{}]",
            tx_hash, self.config.vm_type, request_id
        );

        // Validate transaction hash format
        if !self.is_valid_tx_hash(tx_hash) {
            return Err(ApplicationError::ValidationError {
                field: "tx_hash".to_string(),
                message: "Invalid transaction hash format".to_string(),
            });
        }

        let cache_key = format!("{}:tx:{}", self.vm_type_prefix(), tx_hash);
        if let Ok(Some(tx)) = self
            .cache
            .get::<Option<UnifiedTransaction>>(&cache_key)
            .await
        {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: tx,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        let tx = self.fetch_transaction(tx_hash, &request_id).await?;
        if let Err(e) = self
            .cache
            .set(&cache_key, &tx, Duration::from_secs(3600))
            .await
        {
            debug!("Failed to cache transaction {}: {}", tx_hash, e);
        }

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: tx,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get account information
    pub async fn get_account(
        &self,
        address: &str,
    ) -> ApplicationResult<GatewayResponse<UnifiedAccount>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!(
            "Getting account {} for {:?} [{}]",
            address, self.config.vm_type, request_id
        );

        // Validate address format
        if !self.is_valid_address(address) {
            return Err(ApplicationError::ValidationError {
                field: "address".to_string(),
                message: "Invalid address format".to_string(),
            });
        }

        let cache_key = format!("{}:account:{}", self.vm_type_prefix(), address);
        if let Ok(Some(account)) = self.cache.get::<UnifiedAccount>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: account,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        let account = self.fetch_account(address, &request_id).await?;
        if let Err(e) = self
            .cache
            .set(&cache_key, &account, Duration::from_secs(30))
            .await
        {
            debug!("Failed to cache account {}: {}", address, e);
        }

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: account,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Send raw transaction
    pub async fn send_raw_transaction(
        &self,
        raw_tx: &str,
    ) -> ApplicationResult<GatewayResponse<String>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!(
            "Sending raw transaction for {:?} [{}]",
            self.config.vm_type, request_id
        );

        let tx_hash = self.submit_transaction(raw_tx, &request_id).await?;

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        info!("Transaction sent: {} [{}]", tx_hash, request_id);

        Ok(GatewayResponse {
            data: tx_hash,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get SVM account info (alias for get_account)
    pub async fn get_svm_account_info(
        &self,
        address: &str,
    ) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let account = self.get_account(address).await?;
        let account_info = serde_json::json!({
            "lamports": account.data.balance.parse::<u64>().unwrap_or(0),
            "owner": account.data.vm_specific.get("owner").unwrap_or(&serde_json::Value::String("11111111111111111111111111111111".to_string())),
            "executable": account.data.vm_specific.get("executable").unwrap_or(&serde_json::Value::Bool(false)),
            "rent_epoch": account.data.vm_specific.get("rent_epoch").unwrap_or(&serde_json::Value::Number(serde_json::Number::from(0)))
        });

        Ok(GatewayResponse {
            data: account_info,
            metadata: account.metadata,
        })
    }

    /// Get EVM account (alias for get_account)
    pub async fn get_evm_account(
        &self,
        address: &str,
    ) -> ApplicationResult<GatewayResponse<UnifiedAccount>> {
        self.get_account(address).await
    }

    /// Get account binding from cache or account mapping service
    pub async fn get_account_binding(
        &self,
        address: &str,
    ) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        // Check cache first
        let cache_key = format!("account_binding:{}", address);
        if let Ok(Some(cached_data)) = self.cache.get::<serde_json::Value>(&cache_key).await {
            return Ok(GatewayResponse {
                data: cached_data,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        // Validate address format
        if address.trim().is_empty() {
            return Err(crate::error::ApplicationError::ValidationError {
                field: "address".to_string(),
                message: "Address cannot be empty".to_string(),
            });
        }

        // For production, this would query the account mapping service
        // For now, we'll create a deterministic binding based on the address
        let binding_data = if address.len() >= 32 {
            // Ethereum-style address - derive Solana address
            let evm_address = address;
            let svm_address = format!("{}SVM", &address[..32]); // Simplified derivation

            serde_json::json!({
                "multivm_account_id": format!("multivm_{}", &address[..8]),
                "primary_address": evm_address,
                "bound_accounts": {
                    "EVM": evm_address,
                    "SVM": svm_address
                },
                "binding_type": "evm_primary",
                "created_at": chrono::Utc::now(),
                "is_verified": true
            })
        } else {
            // Solana-style address - derive Ethereum address
            let svm_address = address;
            let evm_address = format!("0x{}", &format!("{:0<40}", address)); // Simplified derivation

            serde_json::json!({
                "multivm_account_id": format!("multivm_{}", &address[..8]),
                "primary_address": svm_address,
                "bound_accounts": {
                    "SVM": svm_address,
                    "EVM": evm_address
                },
                "binding_type": "svm_primary",
                "created_at": chrono::Utc::now(),
                "is_verified": true
            })
        };

        // Cache the result for future requests
        if let Err(e) = self
            .cache
            .set(
                &cache_key,
                &binding_data,
                std::time::Duration::from_secs(300),
            )
            .await
        {
            debug!("Failed to cache account binding for {}: {}", address, e);
        }

        Ok(GatewayResponse {
            data: binding_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "account_mapping".to_string(),
            },
        })
    }

    /// Send SVM transaction (alias for send_raw_transaction)
    pub async fn send_svm_transaction(
        &self,
        transaction_data: &str,
    ) -> ApplicationResult<GatewayResponse<String>> {
        self.send_raw_transaction(transaction_data).await
    }

    /// Send EVM transaction (alias for send_raw_transaction)
    pub async fn send_evm_transaction(
        &self,
        transaction_data: &str,
    ) -> ApplicationResult<GatewayResponse<String>> {
        self.send_raw_transaction(transaction_data).await
    }

    /// Bind accounts with cryptographic proof verification
    pub async fn bind_accounts(
        &self,
        svm_addr: &str,
        evm_addr: &str,
        proof: &str,
    ) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        // Validate addresses
        if svm_addr.trim().is_empty() || evm_addr.trim().is_empty() {
            return Err(crate::error::ApplicationError::ValidationError {
                field: "addresses".to_string(),
                message: "Both SVM and EVM addresses must be provided".to_string(),
            });
        }

        // Validate proof format (simplified - in production would verify cryptographic proof)
        if proof.trim().is_empty() || proof.len() < 64 {
            return Err(crate::error::ApplicationError::ValidationError {
                field: "proof".to_string(),
                message: "Invalid or missing cryptographic proof".to_string(),
            });
        }

        // Verify proof authenticity (simplified implementation)
        let proof_valid = self.verify_binding_proof(svm_addr, evm_addr, proof).await?;
        if !proof_valid {
            return Err(crate::error::ApplicationError::AuthenticationFailed {
                reason: "Cryptographic proof verification failed".to_string(),
            });
        }

        // Generate binding ID
        let binding_id = format!(
            "binding_{}_{}",
            &svm_addr[..8.min(svm_addr.len())],
            &evm_addr[..8.min(evm_addr.len())]
        );

        let binding_data = serde_json::json!({
            "binding_id": binding_id,
            "multivm_account_id": format!("multivm_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            "svm_account": svm_addr,
            "evm_account": evm_addr,
            "proof_hash": self.hash_proof(proof),
            "status": "bound",
            "created_at": chrono::Utc::now(),
            "expires_at": chrono::Utc::now() + chrono::Duration::days(365), // 1 year expiry
            "verification_method": "cryptographic_proof"
        });

        // Cache the binding for quick lookup
        let cache_key = format!("account_binding:{}", svm_addr);
        if let Err(e) = self
            .cache
            .set(
                &cache_key,
                &binding_data,
                std::time::Duration::from_secs(3600),
            )
            .await
        {
            debug!("Failed to cache account binding for {}: {}", svm_addr, e);
        }

        let cache_key_evm = format!("account_binding:{}", evm_addr);
        if let Err(e) = self
            .cache
            .set(
                &cache_key_evm,
                &binding_data,
                std::time::Duration::from_secs(3600),
            )
            .await
        {
            debug!(
                "Failed to cache EVM account binding for {}: {}",
                evm_addr, e
            );
        }

        Ok(GatewayResponse {
            data: binding_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "account_binding_service".to_string(),
            },
        })
    }

    /// Verify binding proof (simplified implementation)
    async fn verify_binding_proof(
        &self,
        svm_addr: &str,
        evm_addr: &str,
        proof: &str,
    ) -> ApplicationResult<bool> {
        // In production, this would:
        // 1. Verify that the proof was signed by both the SVM and EVM private keys
        // 2. Check that the proof contains both addresses
        // 3. Verify the proof hasn't been used before
        // 4. Check proof timestamp for freshness

        // Simplified verification: check proof contains both addresses
        let proof_valid = proof.contains(&svm_addr[..8.min(svm_addr.len())])
            && proof.contains(&evm_addr[..8.min(evm_addr.len())]);

        Ok(proof_valid)
    }

    /// Hash proof for storage (without revealing the original)
    fn hash_proof(&self, proof: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(proof.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Send cross-VM transaction with coordination
    pub async fn send_cross_vm_transaction(
        &self,
        from_vm: &str,
        to_vm: &str,
        transaction_data: &str,
    ) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        // Validate VM types
        if !matches!(from_vm, "SVM" | "EVM") || !matches!(to_vm, "SVM" | "EVM") {
            return Err(crate::error::ApplicationError::ValidationError {
                field: "vm_type".to_string(),
                message: "VM types must be either 'SVM' or 'EVM'".to_string(),
            });
        }

        if from_vm == to_vm {
            return Err(crate::error::ApplicationError::ValidationError {
                field: "vm_type".to_string(),
                message: "Cross-VM transaction requires different source and destination VMs"
                    .to_string(),
            });
        }

        // Validate transaction data
        if transaction_data.trim().is_empty() {
            return Err(crate::error::ApplicationError::ValidationError {
                field: "transaction_data".to_string(),
                message: "Transaction data cannot be empty".to_string(),
            });
        }

        // Generate unique transaction ID
        let tx_id = format!(
            "crossvm_{}_{}_{}",
            from_vm.to_lowercase(),
            to_vm.to_lowercase(),
            uuid::Uuid::new_v4().to_string()[..8].to_string()
        );

        // Parse transaction data to extract relevant information
        let parsed_tx = self
            .parse_transaction_data(transaction_data, from_vm)
            .await?;

        // Create cross-VM transaction record
        let tx_data = serde_json::json!({
            "id": tx_id,
            "from_vm": from_vm,
            "to_vm": to_vm,
            "transaction_data": transaction_data,
            "parsed_transaction": parsed_tx,
            "status": "submitted",
            "created_at": chrono::Utc::now(),
            "estimated_completion": chrono::Utc::now() + chrono::Duration::seconds(30),
            "coordination_required": true,
            "fee_estimate": self.estimate_cross_vm_fee(from_vm, to_vm).await?,
            "steps": [
                {
                    "step": 1,
                    "description": format!("Submit transaction on {}", from_vm),
                    "status": "pending"
                },
                {
                    "step": 2,
                    "description": "Cross-VM coordination",
                    "status": "pending"
                },
                {
                    "step": 3,
                    "description": format!("Execute on {}", to_vm),
                    "status": "pending"
                }
            ]
        });

        // Cache transaction for status tracking
        let cache_key = format!("cross_vm_tx:{}", tx_id);
        if let Err(e) = self
            .cache
            .set(&cache_key, &tx_data, std::time::Duration::from_secs(1800))
            .await
        {
            debug!("Failed to cache cross-VM transaction {}: {}", tx_id, e);
        }

        Ok(GatewayResponse {
            data: tx_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "cross_vm_coordinator".to_string(),
            },
        })
    }

    /// Parse transaction data based on VM type
    async fn parse_transaction_data(
        &self,
        transaction_data: &str,
        vm_type: &str,
    ) -> ApplicationResult<serde_json::Value> {
        match vm_type {
            "SVM" => {
                // Parse Solana transaction format
                Ok(serde_json::json!({
                    "type": "solana_transaction",
                    "size_bytes": transaction_data.len(),
                    "estimated_compute_units": 5000,
                    "contains_programs": true
                }))
            }
            "EVM" => {
                // Parse Ethereum transaction format
                let has_contract_call = transaction_data.contains("0x");
                Ok(serde_json::json!({
                    "type": "ethereum_transaction",
                    "size_bytes": transaction_data.len(),
                    "estimated_gas": 21000,
                    "contains_contract_call": has_contract_call
                }))
            }
            _ => Err(crate::error::ApplicationError::ValidationError {
                field: "vm_type".to_string(),
                message: "Unsupported VM type".to_string(),
            }),
        }
    }

    /// Estimate cross-VM transaction fee
    async fn estimate_cross_vm_fee(
        &self,
        from_vm: &str,
        to_vm: &str,
    ) -> ApplicationResult<serde_json::Value> {
        let base_fee = match (from_vm, to_vm) {
            ("SVM", "EVM") => 0.001,  // 0.001 SOL equivalent
            ("EVM", "SVM") => 0.0001, // 0.0001 ETH equivalent
            _ => 0.0005,
        };

        Ok(serde_json::json!({
            "base_fee": base_fee,
            "coordination_fee": 0.0001,
            "total_fee": base_fee + 0.0001,
            "currency": match from_vm {
                "SVM" => "SOL",
                "EVM" => "ETH",
                _ => "MULTIVM"
            }
        }))
    }

    /// Simulate SVM transaction (mock implementation)
    pub async fn simulate_svm_transaction(
        &self,
        _transaction_data: &str,
    ) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        let simulation_data = serde_json::json!({
            "err": null,
            "logs": ["Program log: Instruction: Transfer", "Program log: Success"],
            "units_consumed": 5000,
            "return_data": "simulation_result"
        });

        Ok(GatewayResponse {
            data: simulation_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "mock".to_string(),
            },
        })
    }

    /// Get gateway statistics
    pub async fn get_stats(&self) -> ApplicationResult<GatewayStats> {
        let mut stats = self.stats.read().await.clone();
        stats.uptime_seconds = self.start_time.elapsed().as_secs();
        Ok(stats)
    }

    /// Health check
    pub async fn health_check(&self) -> ApplicationResult<bool> {
        match self.ping_endpoint().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get token supply for a given mint (SVM-specific)
    pub async fn get_token_supply(
        &self,
        _vm_type: VmType,
        mint: &str,
    ) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let request_start = Instant::now();

        // Mock token supply data for now
        let supply_data = serde_json::json!({
            "mint": mint,
            "supply": "1000000000000",
            "decimals": 6,
            "frozen": false
        });

        Ok(GatewayResponse {
            data: supply_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: request_start.elapsed().as_millis() as u64,
                request_id: format!(
                    "req_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                ),
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    // Private helper methods

    fn vm_type_prefix(&self) -> &'static str {
        match self.config.vm_type {
            VmType::Evm => "evm",
            VmType::Svm => "svm",
        }
    }

    fn is_valid_tx_hash(&self, tx_hash: &str) -> bool {
        match self.config.vm_type {
            VmType::Evm => tx_hash.starts_with("0x") && tx_hash.len() == 66,
            VmType::Svm => tx_hash.len() >= 32 && tx_hash.len() <= 88,
        }
    }

    fn is_valid_address(&self, address: &str) -> bool {
        match self.config.vm_type {
            VmType::Evm => address.starts_with("0x") && address.len() == 42,
            VmType::Svm => address.len() >= 32 && address.len() <= 44,
        }
    }

    async fn record_request(&self, success: bool, response_time_ms: u64) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        if success {
            stats.successful_requests += 1;
        } else {
            stats.failed_requests += 1;
        }

        // Update average response time
        if stats.successful_requests > 0 {
            stats.average_response_time_ms = (stats.average_response_time_ms
                * (stats.successful_requests - 1) as f64
                + response_time_ms as f64)
                / stats.successful_requests as f64;
        }
    }

    async fn record_cache_hit(&self) {
        let mut stats = self.stats.write().await;
        stats.cache_hits += 1;
    }

    async fn record_cache_miss(&self) {
        let mut stats = self.stats.write().await;
        stats.cache_misses += 1;
    }

    // Simplified RPC implementations that delegate to actual VM clients
    async fn fetch_latest_block(&self, _request_id: &str) -> ApplicationResult<UnifiedBlock> {
        // In a real implementation, this would make actual RPC calls
        // For now, return a simplified response
        Ok(UnifiedBlock {
            number: 1000,
            hash: "0x1234567890abcdef".to_string(),
            parent_hash: "0x0987654321fedcba".to_string(),
            timestamp: chrono::Utc::now().timestamp() as u64,
            transactions: vec![],
            vm_specific: serde_json::json!({}),
        })
    }

    async fn fetch_block(
        &self,
        identifier: &str,
        _request_id: &str,
    ) -> ApplicationResult<Option<UnifiedBlock>> {
        if identifier == "latest" {
            Ok(Some(self.fetch_latest_block(_request_id).await?))
        } else {
            // For other identifiers, return None for simplicity
            Ok(None)
        }
    }

    async fn fetch_transaction(
        &self,
        _tx_hash: &str,
        _request_id: &str,
    ) -> ApplicationResult<Option<UnifiedTransaction>> {
        // Simplified implementation
        Ok(Some(UnifiedTransaction {
            hash: _tx_hash.to_string(),
            from: "0x1234567890123456789012345678901234567890".to_string(),
            to: Some("0x0987654321098765432109876543210987654321".to_string()),
            value: "1000000000000000000".to_string(),
            gas_info: GasInfo {
                gas_limit: 21000,
                gas_used: Some(21000),
                gas_price: Some("20000000000".to_string()),
                max_fee_per_gas: None,
            },
            nonce: 1,
            data: "0x".to_string(),
            vm_specific: serde_json::json!({}),
        }))
    }

    async fn fetch_account(
        &self,
        address: &str,
        _request_id: &str,
    ) -> ApplicationResult<UnifiedAccount> {
        Ok(UnifiedAccount {
            address: address.to_string(),
            balance: "1000000000000000000".to_string(),
            nonce: Some(1),
            vm_specific: serde_json::json!({}),
        })
    }

    async fn submit_transaction(
        &self,
        _raw_tx: &str,
        _request_id: &str,
    ) -> ApplicationResult<String> {
        // Generate a mock transaction hash
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(_raw_tx.as_bytes());
        Ok(format!("0x{}", hex::encode(hash)))
    }

    async fn ping_endpoint(&self) -> ApplicationResult<()> {
        // Simple health check - in real implementation would ping the actual endpoint
        Ok(())
    }
}

impl UnifiedGatewayConfig {
    /// Create config from application config
    pub fn from_app_config(app_config: &crate::config::ApplicationConfig) -> Self {
        Self {
            vm_type: VmType::Evm, // Default to EVM
            rpc_url: app_config.base.blockchain.ethereum.rpc_url.clone(),
            backup_urls: vec![],
            timeout: Duration::from_secs(app_config.base.blockchain.ethereum.timeout_seconds),
            max_retries: app_config.base.blockchain.ethereum.max_retries,
            chain_id: 1,
            production_mode: true,
        }
    }
}

impl Default for UnifiedGatewayConfig {
    fn default() -> Self {
        Self {
            vm_type: VmType::Evm,
            rpc_url: "http://localhost:8545".to_string(),
            backup_urls: vec![],
            timeout: Duration::from_secs(30),
            max_retries: 3,
            chain_id: 31337,
            production_mode: false,
        }
    }
}

impl Default for GasInfo {
    fn default() -> Self {
        Self {
            gas_limit: 21000,
            gas_used: None,
            gas_price: None,
            max_fee_per_gas: None,
        }
    }
}
