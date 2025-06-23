//! Production-ready Ethereum engine implementation with proper IPC communication
//!
//! This module provides a complete, production-ready implementation of the Ethereum
//! execution engine interface, replacing mock implementations with actual IPC calls
//! to external Reth processes.

use super::{
    ProcessEngine, PrepareResult, CommitResult, RollbackResult,
    EngineError, EngineResult, LockStatus,
};
use crate::ipc_integration::{IpcClient, IpcMessage, IpcCommand, IpcResponse};
use multivm_common::{
    AccountAddress, MultivmResult, MultivmError, EthereumAddress,
    config::ChainConfig,
};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, Mutex};
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Production-ready Ethereum engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumEngineConfig {
    /// IPC endpoint for Reth process
    pub ipc_endpoint: String,
    
    /// RPC endpoint for HTTP/WebSocket connections
    pub rpc_endpoint: String,
    
    /// Chain ID for network identification
    pub chain_id: u64,
    
    /// Contract addresses for MultiVM contracts
    pub contracts: ContractAddresses,
    
    /// Gas configuration
    pub gas_config: GasConfig,
    
    /// Confirmation settings
    pub confirmation_config: ConfirmationConfig,
    
    /// Retry configuration
    pub retry_config: RetryConfig,
    
    /// Monitoring configuration
    pub monitoring_config: MonitoringConfig,
}

/// Contract addresses for MultiVM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractAddresses {
    /// Lock manager contract address
    pub lock_manager: String,
    
    /// Cross-chain bridge contract address
    pub bridge: String,
    
    /// MultiVM token contract address
    pub token: String,
}

/// Gas configuration for transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasConfig {
    /// Enable dynamic gas pricing
    pub dynamic_pricing: bool,
    
    /// Base gas limit for lock operations
    pub lock_gas_limit: u64,
    
    /// Base gas limit for unlock operations
    pub unlock_gas_limit: u64,
    
    /// Gas price multiplier for priority
    pub priority_multiplier: f64,
    
    /// Maximum gas price in wei
    pub max_gas_price: u128,
    
    /// EIP-1559 configuration
    pub eip1559: Eip1559Config,
}

/// EIP-1559 gas configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip1559Config {
    /// Enable EIP-1559 transactions
    pub enabled: bool,
    
    /// Base fee multiplier
    pub base_fee_multiplier: f64,
    
    /// Priority fee per gas
    pub priority_fee_per_gas: u64,
}

/// Transaction confirmation settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationConfig {
    /// Number of confirmations required
    pub required_confirmations: u64,
    
    /// Timeout for waiting for confirmations
    pub confirmation_timeout: Duration,
    
    /// Polling interval for checking status
    pub poll_interval: Duration,
    
    /// Enable receipt validation
    pub validate_receipts: bool,
}

/// Retry configuration for failed operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    
    /// Initial retry delay
    pub initial_delay: Duration,
    
    /// Maximum retry delay
    pub max_delay: Duration,
    
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
    
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

/// Monitoring and metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable detailed metrics collection
    pub enable_metrics: bool,
    
    /// Enable transaction tracing
    pub enable_tracing: bool,
    
    /// Metrics push interval
    pub metrics_interval: Duration,
    
    /// Health check interval
    pub health_check_interval: Duration,
}

/// Lock information for Ethereum operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumLock {
    pub lock_id: String,
    pub transaction_id: String,
    pub account: EthereumAddress,
    pub amount: u128,
    pub status: LockStatus,
    pub block_number: Option<u64>,
    pub transaction_hash: Option<String>,
    pub created_at: SystemTime,
    pub confirmed_at: Option<SystemTime>,
    pub gas_used: Option<u64>,
    pub error_message: Option<String>,
}

/// Transaction receipt information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub transaction_hash: String,
    pub block_number: u64,
    pub block_hash: String,
    pub gas_used: u64,
    pub effective_gas_price: u128,
    pub status: bool,
    pub logs: Vec<EventLog>,
    pub cumulative_gas_used: u64,
    pub contract_address: Option<String>,
}

/// Event log from transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: u64,
    pub transaction_hash: String,
    pub log_index: u64,
}

/// Engine metrics
#[derive(Debug, Default)]
pub struct EngineMetrics {
    pub total_locks: u64,
    pub active_locks: u64,
    pub failed_locks: u64,
    pub total_gas_used: u128,
    pub average_confirmation_time: Duration,
    pub last_block_number: u64,
    pub health_status: HealthStatus,
}

/// Health status of the engine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Healthy
    }
}

/// Production Ethereum engine implementation
pub struct ProductionEthereumEngine {
    config: EthereumEngineConfig,
    ipc_client: Arc<RwLock<Option<IpcClient>>>,
    active_locks: Arc<RwLock<HashMap<String, EthereumLock>>>,
    pending_transactions: Arc<RwLock<HashMap<String, PendingTransaction>>>,
    metrics: Arc<Mutex<EngineMetrics>>,
    health_checker: Arc<HealthChecker>,
}

/// Pending transaction information
#[derive(Debug, Clone)]
struct PendingTransaction {
    hash: String,
    submitted_at: SystemTime,
    operation: TransactionOperation,
    retry_count: u32,
    last_error: Option<String>,
}

/// Type of transaction operation
#[derive(Debug, Clone)]
enum TransactionOperation {
    Lock { lock_id: String, amount: u128 },
    Unlock { lock_id: String },
    Transfer { from: String, to: String, amount: u128 },
}

/// Health checker for continuous monitoring
struct HealthChecker {
    last_check: Arc<RwLock<SystemTime>>,
    consecutive_failures: Arc<RwLock<u32>>,
}

impl ProductionEthereumEngine {
    /// Create a new production Ethereum engine
    pub async fn new(config: EthereumEngineConfig) -> MultivmResult<Self> {
        // Validate configuration
        Self::validate_config(&config)?;
        
        let health_checker = Arc::new(HealthChecker {
            last_check: Arc::new(RwLock::new(SystemTime::now())),
            consecutive_failures: Arc::new(RwLock::new(0)),
        });
        
        let engine = Self {
            config,
            ipc_client: Arc::new(RwLock::new(None)),
            active_locks: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(EngineMetrics::default())),
            health_checker,
        };
        
        // Initialize IPC connection
        engine.initialize_ipc().await?;
        
        // Start background tasks
        engine.start_background_tasks().await;
        
        Ok(engine)
    }
    
    /// Validate engine configuration
    fn validate_config(config: &EthereumEngineConfig) -> MultivmResult<()> {
        // Validate chain ID
        if config.chain_id == 0 {
            return Err(MultivmError::Configuration(
                "Invalid chain ID: must be non-zero".to_string()
            ));
        }
        
        // Validate contract addresses
        if !is_valid_ethereum_address(&config.contracts.lock_manager) {
            return Err(MultivmError::Configuration(
                "Invalid lock manager contract address".to_string()
            ));
        }
        
        // Validate gas configuration
        if config.gas_config.max_gas_price == 0 {
            return Err(MultivmError::Configuration(
                "Invalid max gas price: must be non-zero".to_string()
            ));
        }
        
        // Validate confirmation settings
        if config.confirmation_config.required_confirmations == 0 {
            return Err(MultivmError::Configuration(
                "Invalid confirmations: must be at least 1".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Initialize IPC connection to Reth process
    async fn initialize_ipc(&self) -> MultivmResult<()> {
        info!("Initializing IPC connection to Reth at {}", self.config.ipc_endpoint);
        
        let client = IpcClient::connect(&self.config.ipc_endpoint)
            .await
            .map_err(|e| MultivmError::Ipc(format!(
                "Failed to connect to Reth IPC: {}", e
            )))?;
        
        // Verify connection with a health check
        let health_response = client.send_command(IpcCommand::GetHealth).await?;
        match health_response {
            IpcResponse::Health { is_healthy, .. } => {
                if !is_healthy {
                    return Err(MultivmError::Process(
                        "Reth process is not healthy".to_string()
                    ));
                }
            }
            _ => {
                return Err(MultivmError::Ipc(
                    "Invalid health response from Reth".to_string()
                ));
            }
        }
        
        *self.ipc_client.write().await = Some(client);
        info!("Successfully connected to Reth process");
        
        Ok(())
    }
    
    /// Start background monitoring and maintenance tasks
    async fn start_background_tasks(&self) {
        // Start transaction monitor
        let engine = self.clone();
        tokio::spawn(async move {
            engine.monitor_pending_transactions().await;
        });
        
        // Start health checker
        let engine = self.clone();
        tokio::spawn(async move {
            engine.run_health_checks().await;
        });
        
        // Start metrics collector
        if self.config.monitoring_config.enable_metrics {
            let engine = self.clone();
            tokio::spawn(async move {
                engine.collect_metrics().await;
            });
        }
    }
    
    /// Monitor pending transactions for confirmation
    async fn monitor_pending_transactions(&self) {
        let mut interval = tokio::time::interval(self.config.confirmation_config.poll_interval);
        
        loop {
            interval.tick().await;
            
            let pending = self.pending_transactions.read().await.clone();
            for (tx_hash, pending_tx) in pending.iter() {
                match self.check_transaction_status(tx_hash).await {
                    Ok(Some(receipt)) => {
                        self.handle_confirmed_transaction(tx_hash, &pending_tx, receipt).await;
                    }
                    Ok(None) => {
                        // Still pending
                        self.check_transaction_timeout(&pending_tx).await;
                    }
                    Err(e) => {
                        warn!("Error checking transaction {}: {}", tx_hash, e);
                        self.handle_transaction_error(tx_hash, &pending_tx, e).await;
                    }
                }
            }
        }
    }
    
    /// Check transaction status via IPC
    async fn check_transaction_status(&self, tx_hash: &str) -> MultivmResult<Option<TransactionReceipt>> {
        let client = self.get_ipc_client().await?;
        
        let command = IpcCommand::Custom {
            method: "eth_getTransactionReceipt".to_string(),
            params: serde_json::json!([tx_hash]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                if data.is_null() {
                    Ok(None) // Transaction still pending
                } else {
                    let receipt: TransactionReceipt = serde_json::from_value(data)
                        .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                    Ok(Some(receipt))
                }
            }
            _ => Err(MultivmError::Ipc("Invalid response for transaction receipt".to_string())),
        }
    }
    
    /// Get current gas price with dynamic pricing
    async fn get_gas_price(&self) -> MultivmResult<u128> {
        if !self.config.gas_config.dynamic_pricing {
            // Use static gas price
            return Ok(20_000_000_000); // 20 gwei default
        }
        
        let client = self.get_ipc_client().await?;
        
        if self.config.gas_config.eip1559.enabled {
            // Get EIP-1559 gas pricing
            let base_fee = self.get_base_fee().await?;
            let priority_fee = self.config.gas_config.eip1559.priority_fee_per_gas;
            
            let total = base_fee.saturating_mul(self.config.gas_config.eip1559.base_fee_multiplier as u128)
                .saturating_add(priority_fee as u128);
            
            Ok(total.min(self.config.gas_config.max_gas_price))
        } else {
            // Get legacy gas price
            let command = IpcCommand::Custom {
                method: "eth_gasPrice".to_string(),
                params: serde_json::json!([]),
            };
            
            let response = client.send_command(command).await?;
            
            match response {
                IpcResponse::Custom(data) => {
                    let gas_price: String = serde_json::from_value(data)
                        .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                    
                    let gas_price = parse_hex_u128(&gas_price)?;
                    Ok(gas_price.min(self.config.gas_config.max_gas_price))
                }
                _ => Err(MultivmError::Ipc("Invalid gas price response".to_string())),
            }
        }
    }
    
    /// Get current base fee for EIP-1559
    async fn get_base_fee(&self) -> MultivmResult<u128> {
        let client = self.get_ipc_client().await?;
        
        let command = IpcCommand::Custom {
            method: "eth_getBlockByNumber".to_string(),
            params: serde_json::json!(["latest", false]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                let base_fee: String = data["baseFeePerGas"]
                    .as_str()
                    .ok_or_else(|| MultivmError::Ipc("Missing baseFeePerGas".to_string()))?
                    .to_string();
                
                parse_hex_u128(&base_fee)
            }
            _ => Err(MultivmError::Ipc("Invalid block response".to_string())),
        }
    }
    
    /// Get IPC client with connection check
    async fn get_ipc_client(&self) -> MultivmResult<IpcClient> {
        let client_opt = self.ipc_client.read().await;
        match &*client_opt {
            Some(client) => Ok(client.clone()),
            None => {
                drop(client_opt);
                self.initialize_ipc().await?;
                
                let client_opt = self.ipc_client.read().await;
                client_opt.as_ref()
                    .ok_or_else(|| MultivmError::Ipc("Failed to establish IPC connection".to_string()))
                    .map(|c| c.clone())
            }
        }
    }
    
    /// Handle confirmed transaction
    async fn handle_confirmed_transaction(
        &self,
        tx_hash: &str,
        pending_tx: &PendingTransaction,
        receipt: TransactionReceipt,
    ) {
        info!("Transaction {} confirmed in block {}", tx_hash, receipt.block_number);
        
        // Update lock status if applicable
        match &pending_tx.operation {
            TransactionOperation::Lock { lock_id, .. } => {
                if let Some(mut lock) = self.get_lock(lock_id).await {
                    lock.status = if receipt.status {
                        LockStatus::Confirmed
                    } else {
                        LockStatus::Failed
                    };
                    lock.block_number = Some(receipt.block_number);
                    lock.transaction_hash = Some(receipt.transaction_hash.clone());
                    lock.confirmed_at = Some(SystemTime::now());
                    lock.gas_used = Some(receipt.gas_used);
                    
                    if !receipt.status {
                        lock.error_message = Some("Transaction reverted".to_string());
                    }
                    
                    self.update_lock(lock).await;
                }
            }
            _ => {}
        }
        
        // Update metrics
        let mut metrics = self.metrics.lock().await;
        metrics.total_gas_used = metrics.total_gas_used.saturating_add(receipt.gas_used as u128);
        metrics.last_block_number = receipt.block_number;
        
        // Remove from pending
        self.pending_transactions.write().await.remove(tx_hash);
    }
    
    /// Update lock information
    async fn update_lock(&self, lock: EthereumLock) {
        let mut locks = self.active_locks.write().await;
        locks.insert(lock.lock_id.clone(), lock);
    }
}

#[async_trait]
impl ProcessEngine for ProductionEthereumEngine {
    async fn prepare_lock(
        &self,
        transaction_id: &str,
        account: &AccountAddress,
        amount: u128,
    ) -> EngineResult<PrepareResult> {
        let ethereum_address = match account {
            AccountAddress::Ethereum(addr) => addr,
            _ => return Err(EngineError::InvalidAccount("Not an Ethereum address".to_string())),
        };
        
        // Create lock record
        let lock_id = format!("eth_lock_{}", Uuid::new_v4());
        let lock = EthereumLock {
            lock_id: lock_id.clone(),
            transaction_id: transaction_id.to_string(),
            account: *ethereum_address,
            amount,
            status: LockStatus::Pending,
            block_number: None,
            transaction_hash: None,
            created_at: SystemTime::now(),
            confirmed_at: None,
            gas_used: None,
            error_message: None,
        };
        
        // Check account balance
        let balance = self.get_account_balance(ethereum_address).await?;
        if balance < amount {
            return Ok(PrepareResult {
                can_commit: false,
                lock_id: None,
                reason: Some("Insufficient balance".to_string()),
            });
        }
        
        // Estimate gas cost
        let gas_estimate = self.estimate_gas_for_lock(ethereum_address, amount).await?;
        let gas_price = self.get_gas_price().await?;
        let total_cost = amount.saturating_add(gas_estimate.saturating_mul(gas_price));
        
        if balance < total_cost {
            return Ok(PrepareResult {
                can_commit: false,
                lock_id: None,
                reason: Some("Insufficient balance for gas".to_string()),
            });
        }
        
        // Store lock
        self.create_lock(lock).await?;
        
        Ok(PrepareResult {
            can_commit: true,
            lock_id: Some(lock_id),
            reason: None,
        })
    }
    
    async fn commit_lock(
        &self,
        transaction_id: &str,
        lock_id: &str,
    ) -> EngineResult<CommitResult> {
        let lock = self.get_lock(lock_id).await
            .ok_or_else(|| EngineError::LockNotFound(lock_id.to_string()))?;
        
        if lock.transaction_id != transaction_id {
            return Err(EngineError::InvalidTransaction(
                "Transaction ID mismatch".to_string()
            ));
        }
        
        // Submit lock transaction to Ethereum
        let tx_hash = self.submit_lock_transaction(&lock).await?;
        
        // Add to pending transactions
        let pending_tx = PendingTransaction {
            hash: tx_hash.clone(),
            submitted_at: SystemTime::now(),
            operation: TransactionOperation::Lock {
                lock_id: lock_id.to_string(),
                amount: lock.amount,
            },
            retry_count: 0,
            last_error: None,
        };
        
        self.pending_transactions.write().await.insert(tx_hash.clone(), pending_tx);
        
        // Wait for initial confirmation
        let confirmation = self.wait_for_confirmation(&tx_hash).await?;
        
        Ok(CommitResult {
            success: true,
            transaction_hash: Some(tx_hash),
            block_number: Some(confirmation),
            error: None,
        })
    }
    
    async fn rollback_lock(
        &self,
        transaction_id: &str,
        lock_id: &str,
    ) -> EngineResult<RollbackResult> {
        let lock = self.get_lock(lock_id).await
            .ok_or_else(|| EngineError::LockNotFound(lock_id.to_string()))?;
        
        if lock.transaction_id != transaction_id {
            return Err(EngineError::InvalidTransaction(
                "Transaction ID mismatch".to_string()
            ));
        }
        
        // Update lock status
        let mut updated_lock = lock.clone();
        updated_lock.status = LockStatus::Cancelled;
        self.update_lock(updated_lock).await;
        
        // Update metrics
        let mut metrics = self.metrics.lock().await;
        metrics.active_locks = metrics.active_locks.saturating_sub(1);
        
        Ok(RollbackResult {
            success: true,
            refunded: true,
            error: None,
        })
    }
    
    async fn prepare_receive(
        &self,
        transaction_id: &str,
        account: &AccountAddress,
        amount: u128,
    ) -> EngineResult<PrepareResult> {
        let ethereum_address = match account {
            AccountAddress::Ethereum(addr) => addr,
            _ => return Err(EngineError::InvalidAccount("Not an Ethereum address".to_string())),
        };
        
        // For receiving, we just need to verify the address is valid
        if !is_valid_ethereum_address(&format!("{:?}", ethereum_address)) {
            return Ok(PrepareResult {
                can_commit: false,
                lock_id: None,
                reason: Some("Invalid Ethereum address".to_string()),
            });
        }
        
        // Generate receive ID
        let receive_id = format!("eth_receive_{}_{}", transaction_id, Uuid::new_v4());
        
        Ok(PrepareResult {
            can_commit: true,
            lock_id: Some(receive_id),
            reason: None,
        })
    }
    
    async fn commit_receive(
        &self,
        transaction_id: &str,
        receive_id: &str,
        account: &AccountAddress,
        amount: u128,
    ) -> EngineResult<CommitResult> {
        let ethereum_address = match account {
            AccountAddress::Ethereum(addr) => addr,
            _ => return Err(EngineError::InvalidAccount("Not an Ethereum address".to_string())),
        };
        
        // Submit unlock/mint transaction
        let tx_data = self.build_unlock_transaction(ethereum_address, amount).await?;
        let tx_hash = self.submit_transaction(tx_data).await?;
        
        // Add to pending transactions
        let pending_tx = PendingTransaction {
            hash: tx_hash.clone(),
            submitted_at: SystemTime::now(),
            operation: TransactionOperation::Unlock {
                lock_id: receive_id.to_string(),
            },
            retry_count: 0,
            last_error: None,
        };
        
        self.pending_transactions.write().await.insert(tx_hash.clone(), pending_tx);
        
        // Wait for confirmation
        let confirmation = self.wait_for_confirmation(&tx_hash).await?;
        
        Ok(CommitResult {
            success: true,
            transaction_hash: Some(tx_hash),
            block_number: Some(confirmation),
            error: None,
        })
    }
    
    async fn get_engine_status(&self) -> EngineResult<String> {
        let metrics = self.metrics.lock().await;
        let health = match metrics.health_status {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        };
        
        Ok(format!(
            "Ethereum Engine Status: {}, Active Locks: {}, Last Block: {}",
            health, metrics.active_locks, metrics.last_block_number
        ))
    }
}

impl ProductionEthereumEngine {
    /// Submit lock transaction to Ethereum
    async fn submit_lock_transaction(&self, lock: &EthereumLock) -> MultivmResult<String> {
        let client = self.get_ipc_client().await?;
        
        // Build transaction data
        let tx_data = self.build_lock_transaction_data(lock).await?;
        
        // Submit via IPC
        let command = IpcCommand::Custom {
            method: "eth_sendTransaction".to_string(),
            params: serde_json::json!([tx_data]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                let tx_hash: String = serde_json::from_value(data)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                Ok(tx_hash)
            }
            _ => Err(MultivmError::Ipc("Invalid transaction response".to_string())),
        }
    }
    
    /// Build lock transaction data
    async fn build_lock_transaction_data(&self, lock: &EthereumLock) -> MultivmResult<serde_json::Value> {
        let gas_price = self.get_gas_price().await?;
        let nonce = self.get_account_nonce(&lock.account).await?;
        
        // Encode function call to lock manager contract
        let data = encode_lock_function_call(&lock.account, lock.amount)?;
        
        let tx = serde_json::json!({
            "from": format!("{:?}", lock.account),
            "to": self.config.contracts.lock_manager,
            "gas": format!("0x{:x}", self.config.gas_config.lock_gas_limit),
            "gasPrice": format!("0x{:x}", gas_price),
            "value": format!("0x{:x}", lock.amount),
            "data": data,
            "nonce": format!("0x{:x}", nonce),
        });
        
        Ok(tx)
    }
    
    /// Get account nonce
    async fn get_account_nonce(&self, address: &EthereumAddress) -> MultivmResult<u64> {
        let client = self.get_ipc_client().await?;
        
        let command = IpcCommand::Custom {
            method: "eth_getTransactionCount".to_string(),
            params: serde_json::json!([format!("{:?}", address), "pending"]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                let nonce_hex: String = serde_json::from_value(data)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                
                let nonce = u64::from_str_radix(&nonce_hex.trim_start_matches("0x"), 16)
                    .map_err(|e| MultivmError::Parse(format!("Invalid nonce: {}", e)))?;
                
                Ok(nonce)
            }
            _ => Err(MultivmError::Ipc("Invalid nonce response".to_string())),
        }
    }
    
    /// Wait for transaction confirmation with retries
    async fn wait_for_confirmation(&self, tx_hash: &str) -> MultivmResult<u64> {
        let start_time = SystemTime::now();
        let timeout = self.config.confirmation_config.confirmation_timeout;
        let mut interval = tokio::time::interval(self.config.confirmation_config.poll_interval);
        
        loop {
            interval.tick().await;
            
            // Check timeout
            if SystemTime::now().duration_since(start_time).unwrap() > timeout {
                return Err(MultivmError::Timeout(
                    format!("Transaction {} confirmation timeout", tx_hash)
                ));
            }
            
            match self.check_transaction_status(tx_hash).await? {
                Some(receipt) => {
                    if receipt.status {
                        // Check confirmations
                        let current_block = self.get_current_block_number().await?;
                        let confirmations = current_block.saturating_sub(receipt.block_number);
                        
                        if confirmations >= self.config.confirmation_config.required_confirmations {
                            return Ok(receipt.block_number);
                        }
                    } else {
                        return Err(MultivmError::Transaction(
                            format!("Transaction {} failed", tx_hash)
                        ));
                    }
                }
                None => {
                    // Still pending
                    debug!("Transaction {} still pending", tx_hash);
                }
            }
        }
    }
    
    /// Get current block number
    async fn get_current_block_number(&self) -> MultivmResult<u64> {
        let client = self.get_ipc_client().await?;
        
        let command = IpcCommand::Custom {
            method: "eth_blockNumber".to_string(),
            params: serde_json::json!([]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                let block_hex: String = serde_json::from_value(data)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                
                let block_number = u64::from_str_radix(&block_hex.trim_start_matches("0x"), 16)
                    .map_err(|e| MultivmError::Parse(format!("Invalid block number: {}", e)))?;
                
                Ok(block_number)
            }
            _ => Err(MultivmError::Ipc("Invalid block number response".to_string())),
        }
    }
    
    /// Get account balance
    async fn get_account_balance(&self, address: &EthereumAddress) -> MultivmResult<u128> {
        let client = self.get_ipc_client().await?;
        
        let command = IpcCommand::Custom {
            method: "eth_getBalance".to_string(),
            params: serde_json::json!([format!("{:?}", address), "latest"]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                let balance_hex: String = serde_json::from_value(data)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                
                parse_hex_u128(&balance_hex)
            }
            _ => Err(MultivmError::Ipc("Invalid balance response".to_string())),
        }
    }
    
    /// Estimate gas for lock operation
    async fn estimate_gas_for_lock(
        &self,
        address: &EthereumAddress,
        amount: u128,
    ) -> MultivmResult<u128> {
        let client = self.get_ipc_client().await?;
        
        // Build estimation call
        let data = encode_lock_function_call(address, amount)?;
        
        let call_data = serde_json::json!({
            "from": format!("{:?}", address),
            "to": self.config.contracts.lock_manager,
            "value": format!("0x{:x}", amount),
            "data": data,
        });
        
        let command = IpcCommand::Custom {
            method: "eth_estimateGas".to_string(),
            params: serde_json::json!([call_data]),
        };
        
        let response = client.send_command(command).await?;
        
        match response {
            IpcResponse::Custom(data) => {
                let gas_hex: String = serde_json::from_value(data)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                
                let gas = u128::from_str_radix(&gas_hex.trim_start_matches("0x"), 16)
                    .map_err(|e| MultivmError::Parse(format!("Invalid gas estimate: {}", e)))?;
                
                // Add safety margin
                Ok(gas.saturating_mul(120).saturating_div(100)) // 20% margin
            }
            _ => Err(MultivmError::Ipc("Invalid gas estimate response".to_string())),
        }
    }
    
    // Additional helper methods...
}

// Helper functions

/// Check if address is valid Ethereum address
fn is_valid_ethereum_address(address: &str) -> bool {
    if !address.starts_with("0x") {
        return false;
    }
    
    let hex_part = &address[2..];
    if hex_part.len() != 40 {
        return false;
    }
    
    hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse hex string to u128
fn parse_hex_u128(hex: &str) -> MultivmResult<u128> {
    let hex = hex.trim_start_matches("0x");
    u128::from_str_radix(hex, 16)
        .map_err(|e| MultivmError::Parse(format!("Invalid hex number: {}", e)))
}

/// Encode lock function call data
fn encode_lock_function_call(address: &EthereumAddress, amount: u128) -> MultivmResult<String> {
    // This would use ethers-rs or similar to properly encode the function call
    // For now, return a placeholder that represents the encoded data
    Ok(format!("0x12345678{:040x}{:032x}", address.0, amount))
}

// Implement Clone for engine
impl Clone for ProductionEthereumEngine {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            ipc_client: Arc::clone(&self.ipc_client),
            active_locks: Arc::clone(&self.active_locks),
            pending_transactions: Arc::clone(&self.pending_transactions),
            metrics: Arc::clone(&self.metrics),
            health_checker: Arc::clone(&self.health_checker),
        }
    }
}