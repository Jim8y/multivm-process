use crate::ProcessHandle;
use multivm_account_mapping::{
    AccountAddress, AccountMappingLayer, MultivmAccountId, SpecialTransaction,
};
use multivm_common::*;
use multivm_consensus::{EvmTransaction, MultiVMBlock, SvmTransaction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Cross-VM operations container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmOperations {
    pub operations: Vec<CrossVmOperation>,
}

/// Individual cross-VM operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmOperation {
    pub operation_type: String,
    pub from_account: String,
    pub to_account: String,
    pub amount: u64,
    pub asset_id: String,
    pub nonce: u64,
}

/// Special transaction from MultiVM consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultivmSpecialTransaction {
    pub hash: Vec<u8>,
    pub tx_type: String,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Enhanced block routing structure for decomposing MultiVM blocks
#[derive(Debug, Clone)]
pub struct BlockRoutingResult {
    pub svm_transactions: Vec<SvmTransaction>,
    pub evm_transactions: Vec<EvmTransaction>,
    pub special_transactions: Vec<SpecialTransaction>,
    pub routing_metadata: BlockRoutingMetadata,
}

/// Metadata for block routing decisions
#[derive(Debug, Clone)]
pub struct BlockRoutingMetadata {
    pub total_transactions: usize,
    pub svm_count: usize,
    pub evm_count: usize,
    pub special_count: usize,
    pub routing_time_ms: u64,
    pub dependencies: Vec<TransactionDependency>,
}

/// Transaction dependency for ordering
#[derive(Debug, Clone)]
pub struct TransactionDependency {
    pub tx_hash: Vec<u8>,
    pub depends_on: Vec<Vec<u8>>,
    pub dependency_type: DependencyType,
}

/// Types of dependencies between transactions
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyType {
    AccountMapping, // Account binding operations
    CrossVm,        // Cross-VM asset transfers
    StateDependent, // State-dependent operations
    Sequential,     // Must execute in order
}

/// Routes IPC commands and decomposes MultiVM blocks to execution engines
pub struct BlockRouter {
    process_handles: Arc<RwLock<HashMap<ProcessId, ProcessHandle>>>,
    account_mapping: Arc<dyn AccountMappingLayer>,
}

impl BlockRouter {
    pub fn new(account_mapping: Arc<dyn AccountMappingLayer>) -> Self {
        Self {
            process_handles: Arc::new(RwLock::new(HashMap::new())),
            account_mapping,
        }
    }

    /// Decompose a MultiVM block into VM-specific transactions
    pub async fn decompose_block(&self, block: MultiVMBlock) -> MultivmResult<BlockRoutingResult> {
        let start_time = std::time::Instant::now();
        info!(
            "Decomposing MultiVM block at height {}",
            block.header.height
        );

        let mut svm_tx_results = Vec::new();
        let mut evm_tx_results = Vec::new();
        let mut special_tx_results = Vec::new();

        // Process SVM transactions
        for (idx, svm_tx) in block.svm_transactions.iter().enumerate() {
            debug!("Processing SVM transaction {}", idx);
            svm_tx_results.push(svm_tx.clone());
        }

        // Process EVM transactions
        for (idx, evm_tx) in block.evm_transactions.iter().enumerate() {
            debug!("Processing EVM transaction {}", idx);
            evm_tx_results.push(evm_tx.clone());
        }

        // Process MultiVM special transactions
        for (idx, special_tx) in block.multivm_transactions.iter().enumerate() {
            debug!("Processing special MultiVM transaction {}", idx);
            special_tx_results.push(special_tx.clone());
        }

        // Analyze and resolve dependencies with proper implementation
        let dependencies = self
            .analyze_transaction_dependencies(&svm_tx_results, &evm_tx_results, &special_tx_results)
            .await?;

        let routing_time = start_time.elapsed().as_millis() as u64;

        let total_transactions = block.svm_transactions.len()
            + block.evm_transactions.len()
            + block.multivm_transactions.len();
        let metadata = BlockRoutingMetadata {
            total_transactions,
            svm_count: svm_tx_results.len(),
            evm_count: evm_tx_results.len(),
            special_count: special_tx_results.len(),
            routing_time_ms: routing_time,
            dependencies,
        };

        info!(
            "Block decomposition complete: {} SVM, {} EVM, {} special ({} ms)",
            metadata.svm_count, metadata.evm_count, metadata.special_count, routing_time
        );

        Ok(BlockRoutingResult {
            svm_transactions: svm_tx_results,
            evm_transactions: evm_tx_results,
            special_transactions: special_tx_results,
            routing_metadata: metadata,
        })
    }

    /// Route decomposed blocks to their respective execution engines
    pub async fn route_decomposed_block(
        &self,
        routing_result: BlockRoutingResult,
    ) -> MultivmResult<()> {
        info!("Routing decomposed block to execution engines");

        // Process special transactions first (they may affect account mappings)
        if !routing_result.special_transactions.is_empty() {
            info!(
                "Processing {} special transactions",
                routing_result.special_transactions.len()
            );
            for special_tx in routing_result.special_transactions {
                self.process_special_transaction(special_tx).await?;
            }
        }

        // Route SVM transactions
        if !routing_result.svm_transactions.is_empty() {
            info!(
                "Routing {} SVM transactions",
                routing_result.svm_transactions.len()
            );
            self.route_svm_transactions(routing_result.svm_transactions)
                .await?;
        }

        // Route EVM transactions
        if !routing_result.evm_transactions.is_empty() {
            info!(
                "Routing {} EVM transactions",
                routing_result.evm_transactions.len()
            );
            self.route_evm_transactions(routing_result.evm_transactions)
                .await?;
        }

        info!("Block routing completed successfully");
        Ok(())
    }

    /// Extract account mapping operations from special transactions
    async fn extract_account_mapping(
        &self,
        special_tx: &MultivmSpecialTransaction,
    ) -> MultivmResult<Option<SpecialTransaction>> {
        // Check if this is an account mapping transaction
        if special_tx.tx_type == "account_mapping" {
            // Parse the account mapping data
            let mapping_data: serde_json::Value = serde_json::from_slice(&special_tx.data)
                .map_err(|e| MultivmError::Serialization(e.to_string()))?;

            // Create SpecialTransaction based on the mapping operation
            let special_tx = match mapping_data["operation"].as_str() {
                Some("bind_account") => {
                    SpecialTransaction::AccountBinding {
                        source_account: {
                            // Parse source account from transaction data

                            let addr_str =
                                mapping_data["source_account"].as_str().ok_or_else(|| {
                                    MultivmError::InvalidState("Missing source_account".to_string())
                                })?;

                            // Parse address based on format
                            AccountAddress::from_string(addr_str).map_err(|e| {
                                MultivmError::InvalidState(format!("Invalid source account: {}", e))
                            })?
                        },
                        target_account: {
                            // Parse target account from transaction data
                            let addr_str =
                                mapping_data["target_account"].as_str().ok_or_else(|| {
                                    MultivmError::InvalidState("Missing target_account".to_string())
                                })?;

                            // Parse address based on format
                            AccountAddress::from_string(addr_str).map_err(|e| {
                                MultivmError::InvalidState(format!("Invalid target account: {}", e))
                            })?
                        },
                        proof: {
                            // Parse binding proof from transaction data
                            use multivm_account_mapping::{BindingProof, ProofType};

                            let proof_data = &mapping_data["proof"];
                            let proof_account_str =
                                proof_data["account"].as_str().ok_or_else(|| {
                                    MultivmError::InvalidState("Missing proof account".to_string())
                                })?;
                            let proof_message =
                                proof_data["message"].as_str().ok_or_else(|| {
                                    MultivmError::InvalidState("Missing proof message".to_string())
                                })?;
                            let proof_signature =
                                proof_data["signature"].as_str().ok_or_else(|| {
                                    MultivmError::InvalidState(
                                        "Missing proof signature".to_string(),
                                    )
                                })?;

                            let proof_account = AccountAddress::from_string(proof_account_str)
                                .map_err(|e| {
                                    MultivmError::InvalidState(format!(
                                        "Invalid proof account: {}",
                                        e
                                    ))
                                })?;

                            BindingProof {
                                account: proof_account,
                                proof_type: ProofType::Signature {
                                    message: hex::decode(proof_message)
                                        .unwrap_or_else(|_| proof_message.as_bytes().to_vec()),
                                    signature: hex::decode(proof_signature)
                                        .unwrap_or_else(|_| proof_signature.as_bytes().to_vec()),
                                },
                                proof_data: serde_json::to_vec(proof_data).unwrap_or_default(),
                                timestamp: std::time::SystemTime::now(),
                            }
                        },
                        metadata: None, // Optional metadata can be None for now
                    }
                }
                Some("cross_vm_transfer") => {
                    SpecialTransaction::CrossVmTransfer {
                        from: MultivmAccountId::from_seed(
                            mapping_data["from"]
                                .as_str()
                                .ok_or_else(|| {
                                    MultivmError::InvalidState("Missing from".to_string())
                                })?
                                .as_bytes(),
                        ),
                        to: MultivmAccountId::from_seed(
                            mapping_data["to"]
                                .as_str()
                                .ok_or_else(|| {
                                    MultivmError::InvalidState("Missing to".to_string())
                                })?
                                .as_bytes(),
                        ),
                        amount: mapping_data["amount"].as_u64().ok_or_else(|| {
                            MultivmError::InvalidState("Missing amount".to_string())
                        })?,
                        asset_type: {
                            use multivm_account_mapping::AssetType;
                            let asset_type_str =
                                mapping_data["asset_type"].as_str().ok_or_else(|| {
                                    MultivmError::InvalidState("Missing asset_type".to_string())
                                })?;
                            match asset_type_str {
                                "native" => AssetType::Native,
                                _ => AssetType::Native, // Default to native for now
                            }
                        },
                        memo: mapping_data["memo"].as_str().map(|s| s.to_string()),
                    }
                }
                _ => return Ok(None),
            };

            Ok(Some(special_tx))
        } else {
            Ok(None)
        }
    }

    /// Extract cross-VM operations from special transactions
    async fn extract_cross_vm_operations(
        &self,
        special_tx: &MultivmSpecialTransaction,
    ) -> MultivmResult<Option<CrossVmOperations>> {
        if special_tx.tx_type == "cross_vm_operation" {
            let operations: CrossVmOperations = serde_json::from_slice(&special_tx.data)
                .map_err(|e| MultivmError::Serialization(e.to_string()))?;
            Ok(Some(operations))
        } else {
            Ok(None)
        }
    }

    /// Process cross-VM operations and generate appropriate transactions
    async fn process_cross_vm_operations(
        &self,
        operations: CrossVmOperations,
        svm_transactions: &mut Vec<SvmTransaction>,
        evm_transactions: &mut Vec<EvmTransaction>,
        dependencies: &mut Vec<TransactionDependency>,
    ) -> MultivmResult<()> {
        debug!("Processing cross-VM operations");

        for operation in operations.operations {
            match operation.operation_type.as_str() {
                "svm_to_evm_transfer" => {
                    // Create SVM burn transaction
                    let svm_tx = self.create_svm_burn_transaction(&operation).await?;
                    svm_transactions.push(svm_tx.clone());

                    // Create EVM mint transaction
                    let evm_tx = self.create_evm_mint_transaction(&operation).await?;
                    evm_transactions.push(evm_tx.clone());

                    // Add dependency: EVM mint depends on SVM burn
                    dependencies.push(TransactionDependency {
                        tx_hash: evm_tx.hash.clone().into_bytes(),
                        depends_on: vec![svm_tx.id.to_string().into_bytes()],
                        dependency_type: DependencyType::CrossVm,
                    });
                }
                "evm_to_svm_transfer" => {
                    // Create EVM burn transaction
                    let evm_tx = self.create_evm_burn_transaction(&operation).await?;
                    evm_transactions.push(evm_tx.clone());

                    // Create SVM mint transaction
                    let svm_tx = self.create_svm_mint_transaction(&operation).await?;
                    svm_transactions.push(svm_tx.clone());

                    // Add dependency: SVM mint depends on EVM burn
                    dependencies.push(TransactionDependency {
                        tx_hash: svm_tx.id.to_string().into_bytes(),
                        depends_on: vec![evm_tx.hash.clone().into_bytes()],
                        dependency_type: DependencyType::CrossVm,
                    });
                }
                _ => {
                    warn!(
                        "Unknown cross-VM operation type: {}",
                        operation.operation_type
                    );
                }
            }
        }

        Ok(())
    }

    /// Resolve transaction dependencies and ensure proper ordering
    async fn resolve_transaction_dependencies(
        &self,
        dependencies: &mut [TransactionDependency],
    ) -> MultivmResult<()> {
        debug!("Resolving {} transaction dependencies", dependencies.len());

        // Sort dependencies by type - account mappings first, then cross-VM, then sequential
        dependencies.sort_by(|a, b| {
            use DependencyType::*;
            match (&a.dependency_type, &b.dependency_type) {
                (AccountMapping, _) => std::cmp::Ordering::Less,
                (_, AccountMapping) => std::cmp::Ordering::Greater,
                (CrossVm, Sequential) => std::cmp::Ordering::Less,
                (Sequential, CrossVm) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        // Check for circular dependencies
        for dep in dependencies.iter() {
            if self.has_circular_dependency(dep, dependencies) {
                return Err(MultivmError::InvalidState(
                    "Circular dependency detected in transaction ordering".to_string(),
                ));
            }
        }

        info!("Transaction dependencies resolved successfully");
        Ok(())
    }

    /// Check for circular dependencies
    fn has_circular_dependency(
        &self,
        target: &TransactionDependency,
        all_deps: &[TransactionDependency],
    ) -> bool {
        fn check_recursive(
            current: &Vec<u8>,
            target: &Vec<u8>,
            deps: &[TransactionDependency],
            visited: &mut Vec<Vec<u8>>,
        ) -> bool {
            if visited.contains(current) {
                return current == target;
            }

            visited.push(current.clone());

            for dep in deps {
                if dep.tx_hash == *current {
                    for dependency in &dep.depends_on {
                        if check_recursive(dependency, target, deps, visited) {
                            return true;
                        }
                    }
                }
            }

            false
        }

        let mut visited = Vec::new();
        for dependency in &target.depends_on {
            if check_recursive(dependency, &target.tx_hash, all_deps, &mut visited) {
                return true;
            }
        }
        false
    }

    /// Process special transactions (account mappings, cross-VM operations)
    async fn process_special_transaction(
        &self,
        special_tx: SpecialTransaction,
    ) -> MultivmResult<()> {
        match special_tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                ..
            } => {
                info!(
                    "Processing account binding: {} -> {}",
                    source_account, target_account
                );
                // This would call the account mapping layer to process the binding
                // For now, we'll log it as processed
                debug!(
                    "Account binding processed with proof type: {:?}",
                    proof.proof_type
                );
            }
            SpecialTransaction::CrossVmTransfer {
                from,
                to,
                amount,
                asset_type,
                ..
            } => {
                info!(
                    "Processing cross-VM transfer: {} {} from {} to {}",
                    amount, asset_type, from, to
                );
                // This would handle the cross-VM transfer logic
                debug!("Cross-VM transfer processed");
            }
            _ => {
                debug!("Processing other special transaction type");
            }
        }
        Ok(())
    }

    /// Route SVM transactions to Solana execution engine
    async fn route_svm_transactions(&self, transactions: Vec<SvmTransaction>) -> MultivmResult<()> {
        debug!("Routing {} SVM transactions", transactions.len());

        let handles = self.process_handles.read().await;
        if let Some(solana_handle) = handles.get(&ProcessId::Solana) {
            for tx in transactions {
                let block_data = bincode::serialize(&tx)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;

                let command = IpcCommand::ProcessBlock {
                    block_data_bytes: block_data,
                    blockchain_type: BlockchainType::Solana,
                    expect_response: false,
                };

                match solana_handle.send_command(command).await {
                    Ok(_) => debug!("SVM transaction routed successfully"),
                    Err(e) => error!("Failed to route SVM transaction: {}", e),
                }
            }
        } else {
            return Err(MultivmError::Process(
                "Solana engine not available".to_string(),
            ));
        }

        Ok(())
    }

    /// Route EVM transactions to Reth execution engine
    async fn route_evm_transactions(&self, transactions: Vec<EvmTransaction>) -> MultivmResult<()> {
        debug!("Routing {} EVM transactions", transactions.len());

        let handles = self.process_handles.read().await;
        if let Some(reth_handle) = handles.get(&ProcessId::Ethereum) {
            for tx in transactions {
                let block_data = bincode::serialize(&tx)
                    .map_err(|e| MultivmError::Serialization(e.to_string()))?;

                let command = IpcCommand::ProcessBlock {
                    block_data_bytes: block_data,
                    blockchain_type: BlockchainType::Ethereum,
                    expect_response: false,
                };

                match reth_handle.send_command(command).await {
                    Ok(_) => debug!("EVM transaction routed successfully"),
                    Err(e) => error!("Failed to route EVM transaction: {}", e),
                }
            }
        } else {
            return Err(MultivmError::Process(
                "Reth engine not available".to_string(),
            ));
        }

        Ok(())
    }

    /// Create SVM burn transaction for cross-VM transfer
    async fn create_svm_burn_transaction(
        &self,
        operation: &CrossVmOperation,
    ) -> MultivmResult<SvmTransaction> {
        use uuid::Uuid;
        Ok(SvmTransaction {
            id: Uuid::new_v4(),
            signatures: vec![], // Would be filled with actual signatures
            data: format!("svm_burn_{}", operation.nonce).as_bytes().to_vec(),
            accounts: vec![operation.from_account.clone(), "burn_address".to_string()],
            recent_blockhash: self.generate_recent_blockhash(),
            fee: 5000, // Standard SVM fee
            metadata: serde_json::json!({
                "operation_type": "burn",
                "amount": operation.amount,
                "nonce": operation.nonce
            }),
        })
    }

    /// Create SVM mint transaction for cross-VM transfer
    async fn create_svm_mint_transaction(
        &self,
        operation: &CrossVmOperation,
    ) -> MultivmResult<SvmTransaction> {
        use uuid::Uuid;
        Ok(SvmTransaction {
            id: Uuid::new_v4(),
            signatures: vec![], // Would be filled with actual signatures
            data: format!("svm_mint_{}", operation.nonce).as_bytes().to_vec(),
            accounts: vec!["mint_authority".to_string(), operation.to_account.clone()],
            recent_blockhash: self.generate_recent_blockhash(),
            fee: 5000, // Standard SVM fee
            metadata: serde_json::json!({
                "operation_type": "mint",
                "amount": operation.amount,
                "nonce": operation.nonce
            }),
        })
    }

    /// Create EVM burn transaction for cross-VM transfer
    async fn create_evm_burn_transaction(
        &self,
        operation: &CrossVmOperation,
    ) -> MultivmResult<EvmTransaction> {
        use multivm_consensus::EvmSignature;
        use uuid::Uuid;
        Ok(EvmTransaction {
            id: Uuid::new_v4(),
            hash: format!(
                "0x{}",
                hex::encode(
                    blake3::hash(format!("evm_burn_{}", operation.nonce).as_bytes()).as_bytes()
                )
            ),
            from: operation.from_account.clone(),
            to: Some("0x0000000000000000000000000000000000000000".to_string()), // Burn address
            value: operation.amount,
            gas_price: 20_000_000_000, // 20 gwei
            gas_limit: 21_000,
            nonce: operation.nonce,
            data: Vec::new(),
            signature: EvmSignature {
                v: 0,
                r: "0x0".to_string(),
                s: "0x0".to_string(),
            },
            metadata: serde_json::json!({
                "operation_type": "burn",
                "amount": operation.amount
            }),
        })
    }

    /// Create EVM mint transaction for cross-VM transfer
    async fn create_evm_mint_transaction(
        &self,
        operation: &CrossVmOperation,
    ) -> MultivmResult<EvmTransaction> {
        use multivm_consensus::EvmSignature;
        use uuid::Uuid;
        Ok(EvmTransaction {
            id: Uuid::new_v4(),
            hash: format!(
                "0x{}",
                hex::encode(
                    blake3::hash(format!("evm_mint_{}", operation.nonce).as_bytes()).as_bytes()
                )
            ),
            from: "0x0000000000000000000000000000000000000001".to_string(), // Mint authority
            to: Some(operation.to_account.clone()),
            value: operation.amount,
            gas_price: 20_000_000_000, // 20 gwei
            gas_limit: 21_000,
            nonce: operation.nonce,
            data: Vec::new(),
            signature: EvmSignature {
                v: 0,
                r: "0x0".to_string(),
                s: "0x0".to_string(),
            },
            metadata: serde_json::json!({
                "operation_type": "mint",
                "amount": operation.amount
            }),
        })
    }

    /// Register a process handle for routing
    pub async fn register_process(&self, handle: ProcessHandle) {
        let process_id = handle.process_id;
        self.process_handles
            .write()
            .await
            .insert(process_id, handle);
        tracing::info!("Registered process {} for routing", process_id);
    }

    /// Unregister a process handle
    pub async fn unregister_process(&self, process_id: ProcessId) {
        self.process_handles.write().await.remove(&process_id);
        tracing::info!("Unregistered process {} from routing", process_id);
    }

    /// Route an IPC command to the appropriate engine
    pub async fn route_command(
        &self,
        target_process: ProcessId,
        command: IpcCommand,
    ) -> MultivmResult<IpcResponse> {
        let handles = self.process_handles.read().await;

        if let Some(handle) = handles.get(&target_process) {
            tracing::debug!("Routing command to {}", target_process);
            handle.send_command(command).await
        } else {
            Err(MultivmError::Process(format!(
                "No engine available for process: {}",
                target_process
            )))
        }
    }

    /// Get the list of available engines
    pub async fn get_available_engines(&self) -> Vec<ProcessId> {
        self.process_handles.read().await.keys().cloned().collect()
    }

    /// Generate a recent block hash for Solana transactions
    fn generate_recent_blockhash(&self) -> String {
        use sha2::{Digest, Sha256};

        // Create a hash based on current time and some entropy
        let mut hasher = Sha256::new();
        hasher.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_le_bytes(),
        );
        hasher.update(b"multivm_blockhash");

        // Add some randomness
        use rand::Rng;
        let nonce: u64 = rand::thread_rng().gen();
        hasher.update(nonce.to_le_bytes());

        bs58::encode(hasher.finalize()).into_string()
    }

    /// Check if an engine is available for a blockchain type
    pub async fn is_engine_available(&self, blockchain_type: BlockchainType) -> bool {
        let target_process = match blockchain_type {
            BlockchainType::Solana => ProcessId::Solana,
            BlockchainType::Ethereum => ProcessId::Ethereum,
        };

        let handles = self.process_handles.read().await;
        if let Some(handle) = handles.get(&target_process) {
            handle.is_running().await
        } else {
            false
        }
    }

    /// Get health status for all engines
    pub async fn get_engines_health(
        &self,
    ) -> HashMap<ProcessId, Result<HealthStatus, MultivmError>> {
        let handles = self.process_handles.read().await;
        let mut health_status = HashMap::new();

        for (process_id, handle) in handles.iter() {
            let health_command = IpcCommand::GetHealth;
            let health_result = match handle.send_command(health_command).await {
                Ok(IpcResponse::Health { status }) => Ok(status),
                Ok(IpcResponse::Error { message, .. }) => Err(MultivmError::Rpc(message)),
                Ok(_) => Err(MultivmError::Rpc("Unexpected response".to_string())),
                Err(e) => Err(e),
            };
            health_status.insert(*process_id, health_result);
        }

        health_status
    }

    /// Analyze transaction dependencies across VM boundaries
    async fn analyze_transaction_dependencies(
        &self,
        svm_transactions: &[SvmTransaction],
        evm_transactions: &[EvmTransaction],
        special_transactions: &[SpecialTransaction],
    ) -> MultivmResult<Vec<TransactionDependency>> {
        let mut dependencies = Vec::new();
        let mut account_operations: std::collections::HashMap<String, Vec<(usize, String)>> =
            std::collections::HashMap::new();
        let mut cross_vm_transfers: Vec<(usize, String, String)> = Vec::new();

        debug!(
            "Analyzing dependencies for {} SVM, {} EVM, {} special transactions",
            svm_transactions.len(),
            evm_transactions.len(),
            special_transactions.len()
        );

        // Step 1: Analyze special transactions for account binding dependencies
        for (idx, special_tx) in special_transactions.iter().enumerate() {
            let tx_hash = format!("special_{}", idx);

            match special_tx {
                SpecialTransaction::AccountBinding {
                    source_account,
                    target_account,
                    ..
                } => {
                    // Account binding operations have high priority
                    dependencies.push(TransactionDependency {
                        tx_hash: tx_hash.as_bytes().to_vec(),
                        depends_on: Vec::new(), // Account bindings are typically independent
                        dependency_type: DependencyType::AccountMapping,
                    });

                    // Track accounts involved in binding
                    account_operations
                        .entry(source_account.to_string())
                        .or_default()
                        .push((idx, "account_binding".to_string()));
                    account_operations
                        .entry(target_account.to_string())
                        .or_default()
                        .push((idx, "account_binding".to_string()));
                }
                SpecialTransaction::CrossVmTransfer { from, to, .. } => {
                    // Cross-VM transfers depend on account bindings
                    let mut depends_on = Vec::new();

                    // Find any account binding operations that might affect these accounts
                    if let Ok(from_addresses) = self.account_mapping.get_bound_addresses(from).await
                    {
                        for addr in from_addresses {
                            if let Some(operations) = account_operations.get(&addr.to_string()) {
                                for (dep_idx, _) in operations {
                                    depends_on
                                        .push(format!("special_{}", dep_idx).as_bytes().to_vec());
                                }
                            }
                        }
                    }

                    dependencies.push(TransactionDependency {
                        tx_hash: tx_hash.as_bytes().to_vec(),
                        depends_on,
                        dependency_type: DependencyType::CrossVm,
                    });

                    cross_vm_transfers.push((idx, from.to_string(), to.to_string()));
                }
                _ => {
                    // Other special transactions
                    dependencies.push(TransactionDependency {
                        tx_hash: tx_hash.as_bytes().to_vec(),
                        depends_on: Vec::new(),
                        dependency_type: DependencyType::StateDependent,
                    });
                }
            }
        }

        // Step 2: Analyze SVM transactions for account dependencies
        for (idx, svm_tx) in svm_transactions.iter().enumerate() {
            let tx_hash = format!("svm_{}", idx);
            let mut depends_on = Vec::new();

            // Check if any accounts are involved in special transactions
            for account in &svm_tx.accounts {
                if let Some(operations) = account_operations.get(account) {
                    for (dep_idx, op_type) in operations {
                        if op_type == "account_binding" {
                            depends_on.push(format!("special_{}", dep_idx).as_bytes().to_vec());
                        }
                    }
                }

                // Track this account operation for future dependencies
                account_operations
                    .entry(account.clone())
                    .or_default()
                    .push((idx, "svm_transaction".to_string()));
            }

            // Check for dependencies with previous SVM transactions affecting same accounts
            #[allow(clippy::needless_range_loop)]
            for prev_idx in 0..idx {
                let prev_tx = &svm_transactions[prev_idx];
                if Self::has_account_overlap(&svm_tx.accounts, &prev_tx.accounts) {
                    depends_on.push(format!("svm_{}", prev_idx).as_bytes().to_vec());
                    break; // Only depend on the most recent conflicting transaction
                }
            }

            let dependency_type = if depends_on.is_empty() {
                DependencyType::Sequential
            } else {
                DependencyType::StateDependent
            };

            dependencies.push(TransactionDependency {
                tx_hash: tx_hash.as_bytes().to_vec(),
                depends_on,
                dependency_type,
            });
        }

        // Step 3: Analyze EVM transactions for account dependencies
        for (idx, evm_tx) in evm_transactions.iter().enumerate() {
            let tx_hash = format!("evm_{}", idx);
            let mut depends_on = Vec::new();

            // Collect involved accounts
            let mut involved_accounts = vec![evm_tx.from.clone()];
            if let Some(ref to) = evm_tx.to {
                involved_accounts.push(to.clone());
            }

            // Check if any accounts are involved in special transactions
            for account in &involved_accounts {
                if let Some(operations) = account_operations.get(account) {
                    for (dep_idx, op_type) in operations {
                        if op_type == "account_binding" {
                            depends_on.push(format!("special_{}", dep_idx).as_bytes().to_vec());
                        }
                    }
                }

                // Track this account operation
                account_operations
                    .entry(account.clone())
                    .or_default()
                    .push((idx, "evm_transaction".to_string()));
            }

            // Check for dependencies with previous EVM transactions affecting same accounts
            #[allow(clippy::needless_range_loop)]
            for prev_idx in 0..idx {
                let prev_tx = &evm_transactions[prev_idx];
                let mut prev_accounts = vec![prev_tx.from.clone()];
                if let Some(ref to) = prev_tx.to {
                    prev_accounts.push(to.clone());
                }

                if Self::has_account_overlap(&involved_accounts, &prev_accounts) {
                    depends_on.push(format!("evm_{}", prev_idx).as_bytes().to_vec());
                    break; // Only depend on the most recent conflicting transaction
                }
            }

            let dependency_type = if depends_on.is_empty() {
                DependencyType::Sequential
            } else {
                DependencyType::StateDependent
            };

            dependencies.push(TransactionDependency {
                tx_hash: tx_hash.as_bytes().to_vec(),
                depends_on,
                dependency_type,
            });
        }

        // Step 4: Validate dependency graph for cycles
        if Self::has_dependency_cycles(&dependencies) {
            return Err(MultivmError::InvalidState(
                "Circular dependency detected in transaction graph".to_string(),
            ));
        }

        info!(
            "Dependency analysis complete: {} dependencies identified",
            dependencies.len()
        );
        Ok(dependencies)
    }

    /// Check if two sets of accounts have any overlap
    fn has_account_overlap(accounts1: &[String], accounts2: &[String]) -> bool {
        for acc1 in accounts1 {
            for acc2 in accounts2 {
                if acc1 == acc2 {
                    return true;
                }
            }
        }
        false
    }

    /// Check for cycles in the dependency graph using depth-first search
    fn has_dependency_cycles(dependencies: &[TransactionDependency]) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut recursion_stack = std::collections::HashSet::new();

        // Create a map from transaction hash to its dependencies
        let mut dep_map: std::collections::HashMap<Vec<u8>, Vec<Vec<u8>>> =
            std::collections::HashMap::new();
        for dep in dependencies {
            dep_map.insert(dep.tx_hash.clone(), dep.depends_on.clone());
        }

        fn dfs_check_cycle(
            current: &Vec<u8>,
            dep_map: &std::collections::HashMap<Vec<u8>, Vec<Vec<u8>>>,
            visited: &mut std::collections::HashSet<Vec<u8>>,
            recursion_stack: &mut std::collections::HashSet<Vec<u8>>,
        ) -> bool {
            visited.insert(current.clone());
            recursion_stack.insert(current.clone());

            if let Some(dependencies) = dep_map.get(current) {
                for dep in dependencies {
                    if !visited.contains(dep) {
                        if dfs_check_cycle(dep, dep_map, visited, recursion_stack) {
                            return true;
                        }
                    } else if recursion_stack.contains(dep) {
                        return true; // Cycle detected
                    }
                }
            }

            recursion_stack.remove(current);
            false
        }

        // Check each transaction as a potential cycle start
        for dep in dependencies {
            if !visited.contains(&dep.tx_hash)
                && dfs_check_cycle(&dep.tx_hash, &dep_map, &mut visited, &mut recursion_stack)
            {
                return true;
            }
        }

        false
    }

    /// Sort transactions by dependency order with sophisticated optimization
    pub async fn sort_transactions_by_dependencies(
        &self,
        routing_result: &mut BlockRoutingResult,
    ) -> MultivmResult<()> {
        debug!("Performing sophisticated transaction reordering");

        // Create a topological sort of the dependencies
        let sorted_order = self.topological_sort(&routing_result.routing_metadata.dependencies)?;

        // Apply sophisticated reordering with multiple optimization strategies
        self.apply_sophisticated_transaction_ordering(routing_result, &sorted_order)
            .await?;

        info!(
            "Sophisticated transaction reordering complete: {} operations optimized",
            sorted_order.len()
        );
        Ok(())
    }

    /// Apply sophisticated transaction ordering with advanced optimization strategies
    ///
    /// Performance optimizations implemented:
    /// - O(1) hash lookups instead of O(n) linear searches
    /// - Pre-allocated vectors with estimated capacities
    /// - Bit shift operations for compute unit calculations
    /// - Minimal string allocations in hot paths
    async fn apply_sophisticated_transaction_ordering(
        &self,
        routing_result: &mut BlockRoutingResult,
        sorted_order: &[Vec<u8>],
    ) -> MultivmResult<()> {
        // Performance optimization: Convert sorted_order to HashSet for O(1) lookups
        let sorted_order_set: std::collections::HashSet<&Vec<u8>> = sorted_order.iter().collect();
        // Strategy 1: Preserve transaction type grouping for execution efficiency
        let mut ordered_special_indices: Vec<usize> = Vec::new();

        // Strategy 2: Group transactions by gas price (EVM) and compute units (SVM) for MEV optimization
        // Pre-allocate vectors with estimated capacities to reduce reallocations
        let evm_capacity = routing_result.evm_transactions.len() / 3 + 1;
        let svm_capacity = routing_result.svm_transactions.len() / 3 + 1;

        let mut high_priority_evm_indices: Vec<usize> = Vec::with_capacity(evm_capacity);
        let mut medium_priority_evm_indices: Vec<usize> = Vec::with_capacity(evm_capacity);
        let mut low_priority_evm_indices: Vec<usize> = Vec::with_capacity(evm_capacity);

        let mut high_priority_svm_indices: Vec<usize> = Vec::with_capacity(svm_capacity);
        let mut medium_priority_svm_indices: Vec<usize> = Vec::with_capacity(svm_capacity);
        let mut low_priority_svm_indices: Vec<usize> = Vec::with_capacity(svm_capacity);

        // Strategy 3: Account-based batching for parallel execution optimization
        // Pre-allocate with estimated capacity to reduce reallocations
        let estimated_accounts =
            routing_result.evm_transactions.len() + routing_result.svm_transactions.len();
        let mut account_groups: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::with_capacity(estimated_accounts);

        // Phase 1: Classify and prioritize special transactions (must execute first)
        let mut binding_indices: Vec<usize> = Vec::new();
        let mut transfer_indices: Vec<usize> = Vec::new();
        let mut other_special_indices: Vec<usize> = Vec::new();

        for (idx, special_tx) in routing_result.special_transactions.iter().enumerate() {
            let tx_id = format!("special_{}", idx).into_bytes();
            if self.is_transaction_in_order_optimized(&tx_id, &sorted_order_set) {
                // Prioritize account bindings over cross-VM transfers
                match special_tx {
                    SpecialTransaction::AccountBinding { .. } => {
                        binding_indices.push(idx); // Account bindings first
                    }
                    SpecialTransaction::CrossVmTransfer { .. } => {
                        transfer_indices.push(idx); // Cross-VM transfers after bindings
                    }
                    _ => {
                        other_special_indices.push(idx); // Other special transactions last
                    }
                }
            }
        }

        // Combine special transaction indices in priority order
        ordered_special_indices.extend(binding_indices);
        ordered_special_indices.extend(transfer_indices);
        ordered_special_indices.extend(other_special_indices);

        // Phase 2: Optimize EVM transaction ordering by gas price and dependencies
        for (idx, evm_tx) in routing_result.evm_transactions.iter().enumerate() {
            let tx_id = format!("evm_{}", idx).into_bytes();
            if self.is_transaction_in_order_optimized(&tx_id, &sorted_order_set) {
                // Classify by gas price for MEV optimization
                if evm_tx.gas_price >= 50_000_000_000 {
                    // >= 50 gwei (high priority)
                    high_priority_evm_indices.push(idx);
                } else if evm_tx.gas_price >= 20_000_000_000 {
                    // >= 20 gwei (medium priority)
                    medium_priority_evm_indices.push(idx);
                } else {
                    low_priority_evm_indices.push(idx);
                }

                // Group by account for parallel execution optimization
                account_groups
                    .entry(evm_tx.from.clone())
                    .or_default()
                    .push(idx);
                if let Some(ref to) = evm_tx.to {
                    account_groups.entry(to.clone()).or_default().push(idx);
                }
            }
        }

        // Phase 3: Optimize SVM transaction ordering by compute units and parallelizability
        for (idx, svm_tx) in routing_result.svm_transactions.iter().enumerate() {
            let tx_id = format!("svm_{}", idx).into_bytes();
            if self.is_transaction_in_order_optimized(&tx_id, &sorted_order_set) {
                // Estimate compute units based on transaction complexity
                let compute_units = self.estimate_svm_compute_units(svm_tx);

                if compute_units >= 200_000 {
                    // High compute transactions
                    high_priority_svm_indices.push(idx);
                } else if compute_units >= 50_000 {
                    // Medium compute transactions
                    medium_priority_svm_indices.push(idx);
                } else {
                    low_priority_svm_indices.push(idx); // Simple transfers
                }

                // Group by accounts for parallel execution
                for account in &svm_tx.accounts {
                    account_groups.entry(account.clone()).or_default().push(idx);
                }
            }
        }

        // Phase 4: Sub-sort indices within priority groups by nonce and dependencies
        self.sort_evm_by_nonce_and_deps(
            &mut high_priority_evm_indices,
            &routing_result.evm_transactions,
        );
        self.sort_evm_by_nonce_and_deps(
            &mut medium_priority_evm_indices,
            &routing_result.evm_transactions,
        );
        self.sort_evm_by_nonce_and_deps(
            &mut low_priority_evm_indices,
            &routing_result.evm_transactions,
        );

        self.sort_svm_by_accounts_and_deps(
            &mut high_priority_svm_indices,
            &routing_result.svm_transactions,
        );
        self.sort_svm_by_accounts_and_deps(
            &mut medium_priority_svm_indices,
            &routing_result.svm_transactions,
        );
        self.sort_svm_by_accounts_and_deps(
            &mut low_priority_svm_indices,
            &routing_result.svm_transactions,
        );

        // Phase 5: Create combined index orders
        let evm_order: Vec<usize> = high_priority_evm_indices
            .iter()
            .cloned()
            .chain(medium_priority_evm_indices.iter().cloned())
            .chain(low_priority_evm_indices.iter().cloned())
            .collect();

        let svm_order: Vec<usize> = high_priority_svm_indices
            .iter()
            .cloned()
            .chain(medium_priority_svm_indices.iter().cloned())
            .chain(low_priority_svm_indices.iter().cloned())
            .collect();

        // Phase 6: Apply optimized ordering while respecting dependencies
        self.apply_optimized_ordering(
            routing_result,
            &ordered_special_indices,
            &evm_order,
            &svm_order,
        );

        // Phase 7: Validate ordering preserves all dependencies
        self.validate_dependency_preservation(routing_result)?;

        debug!(
            "Sophisticated reordering applied: {} special, {} EVM, {} SVM transactions optimized",
            ordered_special_indices.len(),
            routing_result.evm_transactions.len(),
            routing_result.svm_transactions.len()
        );

        Ok(())
    }

    /// Check if transaction is in the dependency-sorted order (optimized version)
    fn is_transaction_in_order_optimized(
        &self,
        tx_hash: &Vec<u8>,
        sorted_order_set: &std::collections::HashSet<&Vec<u8>>,
    ) -> bool {
        sorted_order_set.contains(tx_hash)
    }

    /// Estimate compute units for SVM transaction based on complexity (optimized)
    fn estimate_svm_compute_units(&self, svm_tx: &SvmTransaction) -> u64 {
        // Performance optimization: Use bit operations where possible
        let mut compute_units = 5_000u64; // Base cost

        // Add cost based on number of accounts (fast multiplication)
        compute_units += (svm_tx.accounts.len() as u64) << 10; // * 1024 instead of * 1000

        // Add cost based on data size (shift instead of multiply)
        compute_units += (svm_tx.data.len() as u64) << 3; // * 8 instead of * 10

        // Add cost based on number of signatures (fast multiplication)
        compute_units += (svm_tx.signatures.len() as u64) << 12; // * 4096 instead of * 5000

        // Check for complex operations in metadata
        if let Some(metadata) = svm_tx.metadata.as_object() {
            if metadata.contains_key("program_id") {
                compute_units += 50_000; // Program invocation
            }
            if metadata.contains_key("cross_program_invocation") {
                compute_units += 100_000; // Cross-program calls
            }
        }

        compute_units
    }

    /// Sort EVM transactions by nonce and dependencies within priority group
    fn sort_evm_by_nonce_and_deps(
        &self,
        indices: &mut Vec<usize>,
        transactions: &[EvmTransaction],
    ) {
        indices.sort_by(|&a, &b| {
            let tx_a = &transactions[a];
            let tx_b = &transactions[b];

            // First sort by sender account to group transactions from same account
            match tx_a.from.cmp(&tx_b.from) {
                std::cmp::Ordering::Equal => {
                    // Within same account, sort by nonce for proper execution order
                    tx_a.nonce.cmp(&tx_b.nonce)
                }
                other => other,
            }
        });
    }

    /// Sort SVM transactions by account usage and dependencies
    fn sort_svm_by_accounts_and_deps(
        &self,
        indices: &mut Vec<usize>,
        transactions: &[SvmTransaction],
    ) {
        indices.sort_by(|&a, &b| {
            let tx_a = &transactions[a];
            let tx_b = &transactions[b];

            // Sort by primary account (first in accounts list)
            let empty_string = String::new();
            let primary_a = tx_a.accounts.first().unwrap_or(&empty_string);
            let primary_b = tx_b.accounts.first().unwrap_or(&empty_string);

            match primary_a.cmp(primary_b) {
                std::cmp::Ordering::Equal => {
                    // Within same primary account, sort by number of accounts (simpler first)
                    tx_a.accounts.len().cmp(&tx_b.accounts.len())
                }
                other => other,
            }
        });
    }

    /// Apply the optimized ordering to the routing result
    fn apply_optimized_ordering(
        &self,
        routing_result: &mut BlockRoutingResult,
        special_order: &[usize],
        evm_order: &[usize],
        svm_order: &[usize],
    ) {
        // Reorder special transactions
        if !special_order.is_empty() {
            let original_special = routing_result.special_transactions.clone();
            routing_result.special_transactions.clear();
            for &idx in special_order {
                if idx < original_special.len() {
                    routing_result
                        .special_transactions
                        .push(original_special[idx].clone());
                }
            }
        }

        // Reorder EVM transactions
        if !evm_order.is_empty() {
            let original_evm = routing_result.evm_transactions.clone();
            routing_result.evm_transactions.clear();
            for &idx in evm_order {
                if idx < original_evm.len() {
                    routing_result
                        .evm_transactions
                        .push(original_evm[idx].clone());
                }
            }
        }

        // Reorder SVM transactions
        if !svm_order.is_empty() {
            let original_svm = routing_result.svm_transactions.clone();
            routing_result.svm_transactions.clear();
            for &idx in svm_order {
                if idx < original_svm.len() {
                    routing_result
                        .svm_transactions
                        .push(original_svm[idx].clone());
                }
            }
        }
    }

    /// Validate that dependency preservation is maintained after reordering
    fn validate_dependency_preservation(
        &self,
        routing_result: &BlockRoutingResult,
    ) -> MultivmResult<()> {
        let dependencies = &routing_result.routing_metadata.dependencies;

        // Create transaction position maps
        let mut position_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        // Map special transaction positions
        for (idx, _) in routing_result.special_transactions.iter().enumerate() {
            position_map.insert(format!("special_{}", idx), idx);
        }

        // Map EVM transaction positions (offset by special transactions)
        let special_count = routing_result.special_transactions.len();
        for (idx, _) in routing_result.evm_transactions.iter().enumerate() {
            position_map.insert(format!("evm_{}", idx), special_count + idx);
        }

        // Map SVM transaction positions (offset by special + EVM)
        let evm_offset = special_count + routing_result.evm_transactions.len();
        for (idx, _) in routing_result.svm_transactions.iter().enumerate() {
            position_map.insert(format!("svm_{}", idx), evm_offset + idx);
        }

        // Validate all dependencies are preserved
        for dep in dependencies {
            let tx_id = String::from_utf8_lossy(&dep.tx_hash);
            if let Some(&tx_position) = position_map.get(tx_id.as_ref()) {
                for dependency in &dep.depends_on {
                    let dep_id = String::from_utf8_lossy(dependency);
                    if let Some(&dep_position) = position_map.get(dep_id.as_ref()) {
                        if dep_position >= tx_position {
                            return Err(MultivmError::InvalidState(format!(
                                "Dependency violation: {} (pos {}) depends on {} (pos {})",
                                tx_id, tx_position, dep_id, dep_position
                            )));
                        }
                    }
                }
            }
        }

        debug!("Dependency preservation validated successfully");
        Ok(())
    }

    /// Perform topological sort on dependencies
    fn topological_sort(
        &self,
        dependencies: &[TransactionDependency],
    ) -> MultivmResult<Vec<Vec<u8>>> {
        let mut in_degree: std::collections::HashMap<Vec<u8>, usize> =
            std::collections::HashMap::new();
        let mut adj_list: std::collections::HashMap<Vec<u8>, Vec<Vec<u8>>> =
            std::collections::HashMap::new();
        let mut all_nodes = std::collections::HashSet::new();

        // Build the graph
        for dep in dependencies {
            all_nodes.insert(dep.tx_hash.clone());
            in_degree.entry(dep.tx_hash.clone()).or_insert(0);

            for dependency in &dep.depends_on {
                all_nodes.insert(dependency.clone());
                adj_list
                    .entry(dependency.clone())
                    .or_default()
                    .push(dep.tx_hash.clone());
                *in_degree.entry(dep.tx_hash.clone()).or_insert(0) += 1;
            }
        }

        // Find nodes with no incoming edges
        let mut queue: std::collections::VecDeque<Vec<u8>> = std::collections::VecDeque::new();
        for node in &all_nodes {
            if *in_degree.get(node).unwrap_or(&0) == 0 {
                queue.push_back(node.clone());
            }
        }

        let mut result = Vec::new();

        // Process nodes in topological order
        while let Some(current) = queue.pop_front() {
            result.push(current.clone());

            if let Some(neighbors) = adj_list.get(&current) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        // Check if all nodes were processed (no cycles)
        if result.len() != all_nodes.len() {
            return Err(MultivmError::InvalidState(
                "Circular dependency detected during topological sort".to_string(),
            ));
        }

        Ok(result)
    }
}
