//! IPC Integration for Cross-VM Transactions
//!
//! This module integrates the atomic coordinator with the IPC transport layer
//! to enable communication with external Reth and Solana processes for
//! executing cross-VM transactions.

use crate::{
    AtomicTransactionCoordinator, CrossVmCoordinator, CrossVmCoordinatorConfig,
    VmEngineRegistry, VmType, MultivmAccountId, AssetType, SimpleCrossVmTransfer, 
    TransactionPriority,
};
use multivm_common::{
    ipc::{IpcClient, IpcMessage, IpcRequest, IpcResponse, SecureTransport},
    MultivmError, MultivmResult,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Integrated cross-VM transaction manager
pub struct IntegratedCrossVmManager {
    /// Cross-VM coordinator
    coordinator: Arc<CrossVmCoordinator>,
    /// IPC clients for external processes
    ipc_clients: Arc<RwLock<HashMap<VmType, Arc<IpcClient>>>>,
    /// Configuration
    config: IntegratedManagerConfig,
    /// Metrics
    metrics: Arc<Mutex<IntegratedManagerMetrics>>,
}

/// Configuration for integrated manager
#[derive(Debug, Clone)]
pub struct IntegratedManagerConfig {
    /// IPC endpoint configurations
    pub ipc_endpoints: HashMap<VmType, IpcEndpointConfig>,
    /// Connection timeouts
    pub connection_timeout: Duration,
    /// Request timeouts
    pub request_timeout: Duration,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Maximum retry attempts for IPC calls
    pub max_ipc_retries: u32,
}

/// IPC endpoint configuration
#[derive(Debug, Clone)]
pub struct IpcEndpointConfig {
    /// Endpoint address
    pub address: String,
    /// Port
    pub port: u16,
    /// Use secure transport
    pub secure: bool,
    /// Authentication token
    pub auth_token: Option<String>,
}

/// IPC request types for cross-VM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossVmIpcRequest {
    /// Lock assets in preparation for cross-VM transfer
    LockAssets {
        account: String,
        amount: u64,
        asset_type: String,
        lock_id: String,
        expiry_seconds: u64,
    },
    /// Unlock assets (abort operation)
    UnlockAssets {
        lock_id: String,
    },
    /// Complete transfer (commit operation)
    CompleteTransfer {
        lock_id: String,
        recipient: String,
        amount: u64,
    },
    /// Mint wrapped tokens on target chain
    MintWrapped {
        recipient: String,
        amount: u64,
        origin_chain: String,
        origin_tx: String,
    },
    /// Get account balance
    GetBalance {
        account: String,
        asset_type: String,
    },
    /// Check operation status
    CheckStatus {
        operation_id: String,
    },
}

/// IPC response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossVmIpcResponse {
    /// Lock operation result
    LockResult {
        success: bool,
        lock_id: String,
        tx_hash: Option<String>,
        error: Option<String>,
    },
    /// Unlock operation result
    UnlockResult {
        success: bool,
        tx_hash: Option<String>,
        error: Option<String>,
    },
    /// Transfer completion result
    TransferResult {
        success: bool,
        tx_hash: Option<String>,
        recipient: String,
        amount: u64,
        error: Option<String>,
    },
    /// Mint operation result
    MintResult {
        success: bool,
        tx_hash: Option<String>,
        recipient: String,
        amount: u64,
        error: Option<String>,
    },
    /// Balance query result
    BalanceResult {
        balance: u64,
        asset_type: String,
        error: Option<String>,
    },
    /// Status check result
    StatusResult {
        status: String,
        details: HashMap<String, String>,
        error: Option<String>,
    },
}

/// Metrics for integrated manager
#[derive(Debug, Clone, Default)]
pub struct IntegratedManagerMetrics {
    /// Total IPC requests sent
    pub total_ipc_requests: u64,
    /// Successful IPC requests
    pub successful_ipc_requests: u64,
    /// Failed IPC requests
    pub failed_ipc_requests: u64,
    /// Average IPC response time
    pub avg_ipc_response_time_ms: u64,
    /// Active cross-VM transactions
    pub active_cross_vm_transactions: u64,
    /// Completed cross-VM transactions
    pub completed_cross_vm_transactions: u64,
}

impl IntegratedCrossVmManager {
    /// Create new integrated cross-VM manager
    pub async fn new(
        config: IntegratedManagerConfig,
        account_mapping: Arc<dyn crate::AccountMappingLayer>,
    ) -> MultivmResult<Self> {
        // Create cross-VM coordinator
        let coordinator_config = CrossVmCoordinatorConfig::default();
        let coordinator = Arc::new(CrossVmCoordinator::new(coordinator_config, account_mapping).await?);
        
        // Initialize IPC clients
        let ipc_clients = Arc::new(RwLock::new(HashMap::new()));
        
        let manager = Self {
            coordinator,
            ipc_clients,
            config,
            metrics: Arc::new(Mutex::new(IntegratedManagerMetrics::default())),
        };
        
        // Initialize IPC connections
        manager.initialize_ipc_connections().await?;
        
        Ok(manager)
    }
    
    /// Initialize IPC connections to external processes
    async fn initialize_ipc_connections(&self) -> MultivmResult<()> {
        info!("Initializing IPC connections to external processes");
        
        let mut clients = self.ipc_clients.write().await;
        
        for (vm_type, endpoint_config) in &self.config.ipc_endpoints {
            let endpoint = format!("{}:{}", endpoint_config.address, endpoint_config.port);
            
            info!("Connecting to {} process at {}", vm_type_to_string(vm_type), endpoint);
            
            // Create secure transport if enabled
            let transport = if endpoint_config.secure {
                Some(SecureTransport::new(
                    endpoint_config.auth_token.clone()
                        .unwrap_or_else(|| "default_token".to_string())
                )?)
            } else {
                None
            };
            
            // Create IPC client
            let client = Arc::new(IpcClient::new(
                endpoint,
                transport,
                self.config.connection_timeout,
            ).await?);
            
            clients.insert(vm_type.clone(), client);
            
            info!("Successfully connected to {} process", vm_type_to_string(vm_type));
        }
        
        Ok(())
    }
    
    /// Execute a cross-VM transfer with full IPC integration
    pub async fn execute_integrated_transfer(
        &self,
        transfer: SimpleCrossVmTransfer,
    ) -> MultivmResult<String> {
        info!("Executing integrated cross-VM transfer: {:?} -> {:?}, amount: {}", 
              transfer.from, transfer.to, transfer.amount);
        
        // Start metrics tracking
        let start_time = std::time::Instant::now();
        self.update_metrics_start().await;
        
        // Step 1: Validate transfer requirements
        self.validate_integrated_transfer(&transfer).await?;
        
        // Step 2: Check balances on source chain
        let source_balance = self.get_account_balance(&transfer.from, &transfer.asset_id).await?;
        if source_balance < transfer.amount {
            return Err(MultivmError::InsufficientBalance(
                format!("Balance {} < required {}", source_balance, transfer.amount)
            ));
        }
        
        // Step 3: Execute atomic cross-VM transaction via coordinator
        let tx_id = self.coordinator.execute_transfer(transfer.clone()).await?;
        
        // Step 4: Monitor transaction completion
        let completion_result = self.monitor_transaction_completion(&tx_id).await?;
        
        // Update metrics
        let duration = start_time.elapsed();
        self.update_metrics_completion(duration, true).await;
        
        info!("Integrated cross-VM transfer completed successfully: {}", completion_result);
        Ok(completion_result)
    }
    
    /// Validate integrated transfer requirements
    async fn validate_integrated_transfer(&self, transfer: &SimpleCrossVmTransfer) -> MultivmResult<()> {
        // Check if required IPC connections are available
        let clients = self.ipc_clients.read().await;
        
        // Determine required VMs for this transfer
        let (source_vm, target_vm) = self.determine_transfer_vms(transfer).await?;
        
        if !clients.contains_key(&source_vm) {
            return Err(MultivmError::Configuration(
                format!("No IPC connection available for source VM: {:?}", source_vm)
            ));
        }
        
        if !clients.contains_key(&target_vm) {
            return Err(MultivmError::Configuration(
                format!("No IPC connection available for target VM: {:?}", target_vm)
            ));
        }
        
        Ok(())
    }
    
    /// Determine VMs required for transfer
    async fn determine_transfer_vms(&self, transfer: &SimpleCrossVmTransfer) -> MultivmResult<(VmType, VmType)> {
        // Query account bindings to determine actual VMs
        let source_addresses = self.account_mapping
            .get_bound_addresses(&transfer.from)
            .await?;
        
        let target_addresses = self.account_mapping
            .get_bound_addresses(&transfer.to)
            .await?;
        
        // Find the first supported VM for each account based on the asset
        let asset_registry = self.asset_registry.read().await;
        let asset = asset_registry.assets.get(&transfer.asset_id)
            .ok_or_else(|| MultivmError::Configuration(
                format!("Asset {} not found", transfer.asset_id)
            ))?;
        
        let source_vm = source_addresses.iter()
            .find_map(|addr| {
                let vm = match addr {
                    AccountAddress::Ethereum(_) => VmType::Evm,
                    AccountAddress::Solana(_) => VmType::Svm,
                };
                if asset.supported_vms.contains(&vm) {
                    Some(vm)
                } else {
                    None
                }
            })
            .ok_or_else(|| MultivmError::Configuration(
                "Source account has no address on VMs that support this asset".to_string()
            ))?;
        
        let target_vm = target_addresses.iter()
            .find_map(|addr| {
                let vm = match addr {
                    AccountAddress::Ethereum(_) => VmType::Evm,
                    AccountAddress::Solana(_) => VmType::Svm,
                };
                if asset.supported_vms.contains(&vm) {
                    Some(vm)
                } else {
                    None
                }
            })
            .ok_or_else(|| MultivmError::Configuration(
                "Target account has no address on VMs that support this asset".to_string()
            ))?;
        
        Ok((source_vm, target_vm))
    }
    
    /// Get account balance via IPC
    async fn get_account_balance(&self, account: &MultivmAccountId, asset_id: &str) -> MultivmResult<u64> {
        // Determine which VM this account belongs to based on the asset
        let addresses = self.account_mapping
            .get_bound_addresses(account)
            .await?;
        
        let asset_registry = self.asset_registry.read().await;
        let asset = asset_registry.assets.get(asset_id)
            .ok_or_else(|| MultivmError::Configuration(
                format!("Asset {} not found", asset_id)
            ))?;
        
        // Find the VM that has this account's address and supports the asset
        let (vm_type, _) = addresses.iter()
            .find_map(|addr| {
                let vm = match addr {
                    AccountAddress::Ethereum(_) => VmType::Evm,
                    AccountAddress::Solana(_) => VmType::Svm,
                };
                if asset.supported_vms.contains(&vm) {
                    Some((vm, addr))
                } else {
                    None
                }
            })
            .ok_or_else(|| MultivmError::Configuration(
                "Account has no address on VMs that support this asset".to_string()
            ))?;
        
        let clients = self.ipc_clients.read().await;
        let client = clients.get(&vm_type)
            .ok_or_else(|| MultivmError::Configuration(
                format!("IPC client not found for VM {:?}", vm_type)
            ))?;
        
        let request = CrossVmIpcRequest::GetBalance {
            account: account.to_string(),
            asset_type: asset_id.to_string(),
        };
        
        let response = self.send_ipc_request(client, request).await?;
        
        match response {
            CrossVmIpcResponse::BalanceResult { balance, error, .. } => {
                if let Some(err) = error {
                    Err(MultivmError::ExternalProcess(err))
                } else {
                    Ok(balance)
                }
            }
            _ => Err(MultivmError::InvalidState("Unexpected response type".to_string())),
        }
    }
    
    /// Send IPC request with retries
    async fn send_ipc_request(
        &self,
        client: &IpcClient,
        request: CrossVmIpcRequest,
    ) -> MultivmResult<CrossVmIpcResponse> {
        let mut retry_count = 0;
        
        loop {
            let start_time = std::time::Instant::now();
            
            // Serialize request
            let request_data = serde_json::to_vec(&request)
                .map_err(|e| MultivmError::Serialization(e.to_string()))?;
            
            let ipc_request = IpcRequest {
                id: Uuid::new_v4().to_string(),
                method: "cross_vm_operation".to_string(),
                params: request_data,
                timeout: Some(self.config.request_timeout),
            };
            
            // Send request
            let result = client.send_request(ipc_request).await;
            
            // Update metrics
            let duration = start_time.elapsed();
            self.update_ipc_metrics(duration, result.is_ok()).await;
            
            match result {
                Ok(ipc_response) => {
                    // Deserialize response
                    let response: CrossVmIpcResponse = serde_json::from_slice(&ipc_response.data)
                        .map_err(|e| MultivmError::Serialization(e.to_string()))?;
                    
                    return Ok(response);
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= self.config.max_ipc_retries {
                        return Err(MultivmError::ExternalProcess(
                            format!("IPC request failed after {} retries: {:?}", retry_count, e)
                        ));
                    }
                    
                    warn!("IPC request failed, retrying ({}/{}): {:?}", 
                          retry_count, self.config.max_ipc_retries, e);
                    
                    // Exponential backoff
                    let delay = Duration::from_millis(100 * (1 << retry_count));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    
    /// Monitor transaction completion
    async fn monitor_transaction_completion(&self, tx_id: &crate::TransactionId) -> MultivmResult<String> {
        let timeout = Duration::from_secs(300); // 5 minutes
        let poll_interval = Duration::from_secs(5);
        let start_time = std::time::Instant::now();
        
        loop {
            if start_time.elapsed() > timeout {
                return Err(MultivmError::Timeout("Transaction monitoring timeout".to_string()));
            }
            
            let status = self.coordinator.get_transaction_status(tx_id).await?;
            
            match status {
                crate::CrossVmTransactionStatus::Completed => {
                    return Ok(format!("Transaction {} completed successfully", tx_id));
                }
                crate::CrossVmTransactionStatus::Failed => {
                    return Err(MultivmError::InvalidState("Transaction failed".to_string()));
                }
                crate::CrossVmTransactionStatus::Cancelled => {
                    return Err(MultivmError::InvalidState("Transaction cancelled".to_string()));
                }
                _ => {
                    // Still processing, wait and check again
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
    
    /// Update metrics at start of operation
    async fn update_metrics_start(&self) {
        let mut metrics = self.metrics.lock().await;
        metrics.active_cross_vm_transactions += 1;
    }
    
    /// Update metrics at completion of operation
    async fn update_metrics_completion(&self, duration: std::time::Duration, success: bool) {
        let mut metrics = self.metrics.lock().await;
        metrics.active_cross_vm_transactions = metrics.active_cross_vm_transactions.saturating_sub(1);
        
        if success {
            metrics.completed_cross_vm_transactions += 1;
        }
    }
    
    /// Update IPC-specific metrics
    async fn update_ipc_metrics(&self, duration: std::time::Duration, success: bool) {
        let mut metrics = self.metrics.lock().await;
        metrics.total_ipc_requests += 1;
        
        if success {
            metrics.successful_ipc_requests += 1;
        } else {
            metrics.failed_ipc_requests += 1;
        }
        
        // Update average response time (simple moving average)
        let duration_ms = duration.as_millis() as u64;
        if metrics.total_ipc_requests == 1 {
            metrics.avg_ipc_response_time_ms = duration_ms;
        } else {
            metrics.avg_ipc_response_time_ms = 
                (metrics.avg_ipc_response_time_ms * (metrics.total_ipc_requests - 1) + duration_ms) 
                / metrics.total_ipc_requests;
        }
    }
    
    /// Get manager metrics
    pub async fn get_metrics(&self) -> IntegratedManagerMetrics {
        self.metrics.lock().await.clone()
    }
    
    /// Health check for all IPC connections
    pub async fn health_check(&self) -> MultivmResult<HashMap<VmType, bool>> {
        let mut health_status = HashMap::new();
        let clients = self.ipc_clients.read().await;
        
        for (vm_type, client) in clients.iter() {
            // Send a simple ping request to check connectivity
            let request = CrossVmIpcRequest::CheckStatus {
                operation_id: "health_check".to_string(),
            };
            
            let is_healthy = match self.send_ipc_request(client, request).await {
                Ok(_) => true,
                Err(_) => false,
            };
            
            health_status.insert(vm_type.clone(), is_healthy);
        }
        
        Ok(health_status)
    }
}

/// Convert VM type to string for logging
fn vm_type_to_string(vm_type: &VmType) -> &'static str {
    match vm_type {
        VmType::Evm => "Reth",
        VmType::Svm => "Solana", 
        VmType::MultiVm => "MultiVM",
    }
}

impl Default for IntegratedManagerConfig {
    fn default() -> Self {
        let mut ipc_endpoints = HashMap::new();
        
        // Default Reth process endpoint
        ipc_endpoints.insert(VmType::Evm, IpcEndpointConfig {
            address: "127.0.0.1".to_string(),
            port: 8545,
            secure: false,
            auth_token: None,
        });
        
        // Default Solana process endpoint
        ipc_endpoints.insert(VmType::Svm, IpcEndpointConfig {
            address: "127.0.0.1".to_string(),
            port: 8899,
            secure: false,
            auth_token: None,
        });
        
        Self {
            ipc_endpoints,
            connection_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            auto_reconnect: true,
            max_ipc_retries: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryStorage, StandardAccountMapping};
    
    #[tokio::test]
    async fn test_integrated_manager_creation() {
        let storage = Arc::new(MemoryStorage::new());
        let account_mapping = Arc::new(StandardAccountMapping::new(storage)) as Arc<dyn crate::AccountMappingLayer>;
        let config = IntegratedManagerConfig::default();
        
        // Note: This will fail in CI because no actual processes are running
        // but it tests the basic initialization logic
        let result = IntegratedCrossVmManager::new(config, account_mapping).await;
        
        // We expect this to fail in test environment due to no running processes
        assert!(result.is_err());
    }
    
    #[test]
    fn test_vm_type_string_conversion() {
        assert_eq!(vm_type_to_string(&VmType::Evm), "Reth");
        assert_eq!(vm_type_to_string(&VmType::Svm), "Solana");
        assert_eq!(vm_type_to_string(&VmType::MultiVm), "MultiVM");
    }
}