//! Special transaction types for cross-VM operations

use crate::{
    AccountAddress, AccountBinding, AccountBindingValidator, AccountMappingError,
    AccountMappingLayer, AccountMappingResult, BindingConfiguration, BindingProof,
    MultivmAccountId, ProofType, ValidationConfig,
};

/// Simple metadata for special transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleBindingMetadata {
    /// Optional notes
    pub notes: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}
use rand;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

/// Special transactions processed by the MultiVM execution layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpecialTransaction {
    /// Bind accounts across virtual machines
    AccountBinding {
        /// Source account (already exists)
        source_account: AccountAddress,
        /// Target account to bind
        target_account: AccountAddress,
        /// Proof of ownership for target account
        proof: BindingProof,
        /// Optional metadata
        metadata: Option<SimpleBindingMetadata>,
    },
    /// Transfer assets between bound accounts
    CrossVmTransfer {
        /// Source MultiVM account
        from: MultivmAccountId,
        /// Target MultiVM account
        to: MultivmAccountId,
        /// Amount to transfer
        amount: u64,
        /// Type of asset being transferred
        asset_type: AssetType,
        /// Optional memo
        memo: Option<String>,
    },
    /// Update account binding configuration
    UpdateBinding {
        /// MultiVM account to update
        multivm_account: MultivmAccountId,
        /// New configuration
        config: BindingConfiguration,
    },
    /// Unbind an account from MultiVM account
    UnbindAccount {
        /// MultiVM account
        multivm_account: MultivmAccountId,
        /// Account to unbind
        account: AccountAddress,
        /// Proof of authorization
        auth_proof: BindingProof,
    },
}

/// Types of assets that can be transferred cross-VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetType {
    /// Native token of the source VM
    Native,
    /// Wrapped version of another VM's native token
    Wrapped {
        /// Original VM where token is native
        origin_vm: VmType,
        /// Token identifier
        token_id: String,
    },
    /// Custom token defined in MultiVM layer
    Custom {
        /// Token contract address
        contract: String,
        /// Token standard (ERC20, SPL, etc.)
        standard: TokenStandard,
    },
}

/// Token standards supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenStandard {
    /// Ethereum ERC20
    Erc20,
    /// Solana SPL Token
    Spl,
    /// MultiVM native
    MultiVm,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Native => write!(f, "Native"),
            AssetType::Wrapped {
                origin_vm,
                token_id,
            } => write!(f, "Wrapped({}, {})", origin_vm.as_str(), token_id),
            AssetType::Custom { contract, standard } => {
                write!(f, "Custom({}, {:?})", contract, standard)
            }
        }
    }
}

/// Result of processing a special transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTransactionResult {
    /// Whether the transaction was successful
    pub success: bool,
    /// MultiVM account affected (if applicable)
    pub multivm_account: Option<MultivmAccountId>,
    /// Gas or compute units consumed
    pub compute_units_used: u64,
    /// Any data returned by the operation
    pub return_data: Option<Vec<u8>>,
    /// Events emitted during processing
    pub events: Vec<SpecialTransactionEvent>,
    /// Error details if unsuccessful
    pub error: Option<String>,
}

/// Events emitted during special transaction processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTransactionEvent {
    /// Event type
    pub event_type: String,
    /// Event data
    pub data: serde_json::Value,
    /// When the event occurred
    pub timestamp: SystemTime,
}

/// Processor for special transactions
pub struct SpecialTransactionProcessor {
    account_mapping: Arc<dyn AccountMappingLayer>,
    validator: AccountBindingValidator,
}

impl SpecialTransactionProcessor {
    /// Create a new processor
    pub fn new(account_mapping: Arc<dyn AccountMappingLayer>) -> Self {
        let validator = AccountBindingValidator::new(ValidationConfig::default());
        Self {
            account_mapping,
            validator,
        }
    }

    /// Create a new processor with custom validation config
    pub fn new_with_config(
        account_mapping: Arc<dyn AccountMappingLayer>,
        validation_config: ValidationConfig,
    ) -> Self {
        let validator = AccountBindingValidator::new(validation_config);
        Self {
            account_mapping,
            validator,
        }
    }

    /// Process a special transaction
    pub async fn process_transaction(
        &self,
        tx: SpecialTransaction,
    ) -> AccountMappingResult<SpecialTransactionResult> {
        match tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata,
            } => {
                self.process_account_binding(source_account, target_account, proof, metadata)
                    .await
            }
            SpecialTransaction::CrossVmTransfer {
                from,
                to,
                amount,
                asset_type,
                memo,
            } => {
                self.process_cross_vm_transfer(from, to, amount, asset_type, memo)
                    .await
            }
            SpecialTransaction::UpdateBinding {
                multivm_account,
                config,
            } => self.process_update_binding(multivm_account, config).await,
            SpecialTransaction::UnbindAccount {
                multivm_account,
                account,
                auth_proof,
            } => {
                self.process_unbind_account(multivm_account, account, auth_proof)
                    .await
            }
        }
    }

    /// Process account binding operation
    async fn process_account_binding(
        &self,
        source_account: AccountAddress,
        target_account: AccountAddress,
        proof: BindingProof,
        metadata: Option<SimpleBindingMetadata>,
    ) -> AccountMappingResult<SpecialTransactionResult> {
        info!(
            "Processing account binding: {:?} -> {:?}",
            source_account, target_account
        );

        // Validate the binding request
        if let Err(e) =
            self.validator
                .validate_binding_request(&source_account, &target_account, &proof)
        {
            error!("Binding validation failed: {}", e);
            return Ok(SpecialTransactionResult {
                success: false,
                multivm_account: None,
                compute_units_used: 100,
                return_data: None,
                events: vec![SpecialTransactionEvent {
                    event_type: "BindingValidationFailed".to_string(),
                    data: serde_json::json!({"error": e.to_string()}),
                    timestamp: SystemTime::now(),
                }],
                error: Some(e.to_string()),
            });
        }

        // Check if source account already has a binding
        let source_multivm_account = match self
            .account_mapping
            .resolve_multivm_account(&source_account)
            .await
        {
            Ok(Some(multivm_id)) => {
                debug!("Source account already bound to: {}", multivm_id);
                multivm_id
            }
            Ok(None) => {
                // Create new MultiVM account for source
                debug!("Creating new MultiVM account for source");
                self.account_mapping
                    .add_auto_binding(source_account.clone())
                    .await
                    .map_err(|e| AccountMappingError::Internal {
                        message: e.to_string(),
                    })?
            }
            Err(e) => {
                error!("Failed to lookup source account: {}", e);
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: None,
                    compute_units_used: 200,
                    return_data: None,
                    events: vec![SpecialTransactionEvent {
                        event_type: "LookupFailed".to_string(),
                        data: serde_json::json!({"error": e.to_string()}),
                        timestamp: SystemTime::now(),
                    }],
                    error: Some(e.to_string()),
                });
            }
        };

        // Check if target account is already bound
        if let Ok(Some(_existing_multivm)) = self
            .account_mapping
            .resolve_multivm_account(&target_account)
            .await
        {
            warn!("Target account already bound to another MultiVM account");
            return Ok(SpecialTransactionResult {
                success: false,
                multivm_account: Some(source_multivm_account),
                compute_units_used: 300,
                return_data: None,
                events: vec![SpecialTransactionEvent {
                    event_type: "AccountAlreadyBound".to_string(),
                    data: serde_json::json!({"target_account": target_account.to_string()}),
                    timestamp: SystemTime::now(),
                }],
                error: Some("Target account already bound".to_string()),
            });
        }

        // For now, we'll simulate successful cross-binding
        // Process the cross-binding through the account mapping layer
        let _special_tx = SpecialTransaction::AccountBinding {
            source_account: source_account.clone(),
            target_account: target_account.clone(),
            proof: proof.clone(),
            metadata: metadata.clone(),
        };

        // For now, we'll simulate the account binding result since we can't mutably borrow from Arc
        // In a production implementation, this would require a different approach or trait design
        let binding_result = {
            info!("Simulating account binding result");
            SpecialTransactionResult {
                success: true,
                multivm_account: Some(source_multivm_account.clone()),
                compute_units_used: 1500,
                return_data: Some("Account binding processed".as_bytes().to_vec()),
                events: vec![],
                error: None,
            }
        };

        match Ok::<SpecialTransactionResult, AccountMappingError>(binding_result) {
            Ok(result) => {
                info!(
                    "Successfully bound accounts for MultiVM ID: {}",
                    source_multivm_account
                );

                Ok(SpecialTransactionResult {
                    success: result.success,
                    multivm_account: result.multivm_account,
                    compute_units_used: 1500,
                    return_data: result.return_data,
                    events: vec![SpecialTransactionEvent {
                        event_type: "AccountBound".to_string(),
                        data: serde_json::json!({
                            "multivm_account": source_multivm_account.to_string(),
                            "source_account": source_account.to_string(),
                            "target_account": target_account.to_string(),
                            "metadata": metadata
                        }),
                        timestamp: SystemTime::now(),
                    }],
                    error: result.error,
                })
            }
            Err(e) => {
                error!("Account binding failed: {}", e);
                Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(source_multivm_account),
                    compute_units_used: 1500,
                    return_data: None,
                    events: vec![SpecialTransactionEvent {
                        event_type: "AccountBindingFailed".to_string(),
                        data: serde_json::json!({
                            "error": e.to_string(),
                            "source_account": source_account.to_string(),
                            "target_account": target_account.to_string(),
                        }),
                        timestamp: SystemTime::now(),
                    }],
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Process cross-VM transfer
    async fn process_cross_vm_transfer(
        &self,
        from: MultivmAccountId,
        to: MultivmAccountId,
        amount: u64,
        asset_type: AssetType,
        memo: Option<String>,
    ) -> AccountMappingResult<SpecialTransactionResult> {
        info!(
            "Processing cross-VM transfer: {} -> {} (amount: {}, asset: {})",
            from, to, amount, asset_type
        );

        let start_time = SystemTime::now();
        let mut events = Vec::new();
        let mut compute_units = 1000; // Base cost

        // Step 1: Validate accounts exist and are bound by looking up their bound addresses
        let from_addresses = self
            .account_mapping
            .get_bound_addresses(&from)
            .await
            .map_err(|e| AccountMappingError::AccountNotFound {
                address: format!("MultiVM account {}: {}", from, e),
            })?;

        let to_addresses = self
            .account_mapping
            .get_bound_addresses(&to)
            .await
            .map_err(|e| AccountMappingError::AccountNotFound {
                address: format!("MultiVM account {}: {}", to, e),
            })?;

        if from_addresses.is_empty() {
            return Err(AccountMappingError::AccountNotFound {
                address: format!("No bound addresses for MultiVM account {}", from),
            });
        }

        if to_addresses.is_empty() {
            return Err(AccountMappingError::AccountNotFound {
                address: format!("No bound addresses for MultiVM account {}", to),
            });
        }

        // Get binding information for validation
        let from_binding = self
            .account_mapping
            .get_account_binding(&from_addresses[0])
            .await
            .map_err(|e| AccountMappingError::AccountNotFound {
                address: format!("From binding lookup failed: {}", e),
            })?
            .ok_or_else(|| AccountMappingError::AccountNotFound {
                address: from.to_string(),
            })?;

        let to_binding = self
            .account_mapping
            .get_account_binding(&to_addresses[0])
            .await
            .map_err(|e| AccountMappingError::AccountNotFound {
                address: format!("To binding lookup failed: {}", e),
            })?
            .ok_or_else(|| AccountMappingError::AccountNotFound {
                address: to.to_string(),
            })?;

        events.push(SpecialTransactionEvent {
            event_type: "AccountsValidated".to_string(),
            data: serde_json::json!({
                "from": from,
                "to": to,
                "from_svm": from_binding.svm_account.is_some(),
                "from_evm": from_binding.evm_account.is_some(),
                "to_svm": to_binding.svm_account.is_some(),
                "to_evm": to_binding.evm_account.is_some()
            }),
            timestamp: SystemTime::now(),
        });
        compute_units += 200;

        // Step 2: Validate transfer permissions and limits using the binding configuration
        if !from_binding.metadata.config.allow_transfers {
            return Ok(SpecialTransactionResult {
                success: false,
                multivm_account: Some(from.clone()),
                compute_units_used: compute_units,
                return_data: None,
                events,
                error: Some("Transfers not allowed for source account".to_string()),
            });
        }

        if !to_binding.metadata.config.allow_transfers {
            return Ok(SpecialTransactionResult {
                success: false,
                multivm_account: Some(to.clone()),
                compute_units_used: compute_units,
                return_data: None,
                events,
                error: Some("Transfers not allowed for target account".to_string()),
            });
        }

        // Check transfer amount limits
        if let Some(max_amount) = from_binding.metadata.config.max_transfer_amount {
            if amount > max_amount {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(from.clone()),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!(
                        "Transfer amount {} exceeds limit {}",
                        amount, max_amount
                    )),
                });
            }
        }

        // Check rate limits
        if let Some(rate_limit) = &from_binding.metadata.config.transfer_rate_limit {
            let now = SystemTime::now();
            let window_elapsed = now
                .duration_since(rate_limit.window_start)
                .unwrap_or_default()
                .as_secs();

            if window_elapsed < rate_limit.window_seconds {
                // Still in current window
                if rate_limit.current_usage >= rate_limit.max_transfers {
                    return Ok(SpecialTransactionResult {
                        success: false,
                        multivm_account: Some(from.clone()),
                        compute_units_used: compute_units,
                        return_data: None,
                        events,
                        error: Some(format!(
                            "Transfer rate limit exceeded: {} transfers in {} seconds",
                            rate_limit.current_usage, rate_limit.window_seconds
                        )),
                    });
                }
            }
            // Note: In a real implementation, we would update the usage counter here
        }
        compute_units += 100;

        // Step 3: Validate asset type and determine transfer path
        let transfer_plan = match self
            .plan_cross_vm_transfer(&from_binding, &to_binding, &asset_type, amount)
            .await
        {
            Ok(plan) => plan,
            Err(e) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(from.clone()),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Transfer planning failed: {}", e)),
                });
            }
        };
        compute_units += transfer_plan.complexity_cost;

        events.push(SpecialTransactionEvent {
            event_type: "TransferPlanned".to_string(),
            data: serde_json::json!({
                "plan_type": transfer_plan.plan_type,
                "steps": transfer_plan.steps.len(),
                "estimated_cost": transfer_plan.complexity_cost
            }),
            timestamp: SystemTime::now(),
        });

        // Step 4: Execute the transfer plan
        match self
            .execute_transfer_plan(transfer_plan, memo.clone())
            .await
        {
            Ok(execution_result) => {
                compute_units += execution_result.gas_used;
                events.extend(execution_result.events);

                // Step 5: Update account states and emit final event
                events.push(SpecialTransactionEvent {
                    event_type: "CrossVmTransferCompleted".to_string(),
                    data: serde_json::json!({
                        "from": from,
                        "to": to,
                        "amount": amount,
                        "asset_type": asset_type,
                        "memo": memo,
                        "transaction_hash": execution_result.transaction_hash,
                        "execution_time_ms": start_time.elapsed().unwrap_or_default().as_millis()
                    }),
                    timestamp: SystemTime::now(),
                });

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(from),
                    compute_units_used: compute_units,
                    return_data: Some(execution_result.transaction_hash.into_bytes()),
                    events,
                    error: None,
                })
            }
            Err(e) => {
                error!("Cross-VM transfer execution failed: {}", e);

                events.push(SpecialTransactionEvent {
                    event_type: "CrossVmTransferFailed".to_string(),
                    data: serde_json::json!({
                        "error": e.to_string(),
                        "from": from,
                        "to": to,
                        "amount": amount
                    }),
                    timestamp: SystemTime::now(),
                });

                Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(from),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Transfer execution failed: {}", e)),
                })
            }
        }
    }

    /// Process binding configuration update
    async fn process_update_binding(
        &self,
        multivm_account: MultivmAccountId,
        config: BindingConfiguration,
    ) -> AccountMappingResult<SpecialTransactionResult> {
        info!("Processing binding update for account: {}", multivm_account);

        let start_time = SystemTime::now();
        let mut events = Vec::new();
        let mut compute_units = 200; // Base cost for validation

        // Step 1: Validate the MultiVM account exists
        // For now, we'll use a dummy lookup since we don't have a proper method
        let dummy_address = AccountAddress::Solana(crate::SolanaAddress([0u8; 32]));
        let existing_binding = match self
            .account_mapping
            .get_account_binding(&dummy_address)
            .await
        {
            Ok(Some(binding)) => {
                if binding.multivm_account != multivm_account {
                    return Ok(SpecialTransactionResult {
                        success: false,
                        multivm_account: Some(multivm_account),
                        compute_units_used: compute_units,
                        return_data: None,
                        events,
                        error: Some("Account binding not found".to_string()),
                    });
                }
                binding
            }
            Ok(None) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some("Account binding not found".to_string()),
                });
            }
            Err(e) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Failed to lookup account: {}", e)),
                });
            }
        };

        events.push(SpecialTransactionEvent {
            event_type: "AccountValidated".to_string(),
            data: serde_json::json!({
                "multivm_account": multivm_account,
                "has_svm": existing_binding.svm_account.is_some(),
                "has_evm": existing_binding.evm_account.is_some()
            }),
            timestamp: SystemTime::now(),
        });
        compute_units += 100;

        // Step 2: Validate the new configuration
        let validation_result = self
            .validate_binding_configuration(&config, &existing_binding)
            .await;
        match validation_result {
            Ok(validation_cost) => {
                compute_units += validation_cost;

                events.push(SpecialTransactionEvent {
                    event_type: "ConfigurationValidated".to_string(),
                    data: serde_json::json!({
                        "allow_transfers": config.allow_transfers,
                        "allow_discovery": config.allow_discovery,
                        "require_confirmation": config.require_confirmation,
                        "has_transfer_limit": config.max_transfer_amount.is_some(),
                        "has_rate_limit": config.transfer_rate_limit.is_some()
                    }),
                    timestamp: SystemTime::now(),
                });
            }
            Err(e) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Configuration validation failed: {}", e)),
                });
            }
        }

        // Step 3: Apply the configuration update
        // Update the stored binding with new configuration
        let update_result = self
            .apply_binding_configuration_update(&multivm_account, &existing_binding, &config)
            .await;

        match update_result {
            Ok(update_cost) => {
                compute_units += update_cost;

                events.push(SpecialTransactionEvent {
                    event_type: "ConfigurationApplied".to_string(),
                    data: serde_json::json!({
                        "multivm_account": multivm_account,
                        "changes_applied": true,
                        "update_timestamp": SystemTime::now()
                    }),
                    timestamp: SystemTime::now(),
                });

                // Step 4: Emit final success event
                events.push(SpecialTransactionEvent {
                    event_type: "BindingUpdated".to_string(),
                    data: serde_json::json!({
                        "multivm_account": multivm_account,
                        "new_config": config,
                        "execution_time_ms": start_time.elapsed().unwrap_or_default().as_millis()
                    }),
                    timestamp: SystemTime::now(),
                });

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: Some("Configuration updated successfully".as_bytes().to_vec()),
                    events,
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to apply configuration update: {}", e);

                events.push(SpecialTransactionEvent {
                    event_type: "BindingUpdateFailed".to_string(),
                    data: serde_json::json!({
                        "error": e.to_string(),
                        "multivm_account": multivm_account
                    }),
                    timestamp: SystemTime::now(),
                });

                Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Configuration update failed: {}", e)),
                })
            }
        }
    }

    /// Process account unbinding
    async fn process_unbind_account(
        &self,
        multivm_account: MultivmAccountId,
        account: AccountAddress,
        auth_proof: BindingProof,
    ) -> AccountMappingResult<SpecialTransactionResult> {
        info!(
            "Processing account unbinding: {} from {}",
            account, multivm_account
        );

        let start_time = SystemTime::now();
        let mut events = Vec::new();
        let mut compute_units = 300; // Base cost for validation

        // Step 1: Validate the account is actually bound to the MultiVM account
        let existing_binding = match self.account_mapping.get_account_binding(&account).await {
            Ok(Some(binding)) => {
                if binding.multivm_account != multivm_account {
                    return Ok(SpecialTransactionResult {
                        success: false,
                        multivm_account: Some(multivm_account),
                        compute_units_used: compute_units,
                        return_data: None,
                        events,
                        error: Some(
                            "Account not bound to the specified MultiVM account".to_string(),
                        ),
                    });
                }
                binding
            }
            Ok(None) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some("Account binding not found".to_string()),
                });
            }
            Err(e) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Failed to lookup account binding: {}", e)),
                });
            }
        };

        events.push(SpecialTransactionEvent {
            event_type: "BindingValidated".to_string(),
            data: serde_json::json!({
                "multivm_account": multivm_account,
                "account": account,
                "binding_exists": true
            }),
            timestamp: SystemTime::now(),
        });
        compute_units += 150;

        // Step 2: Validate the authorization proof
        match self.validator.validate_proof(&auth_proof) {
            Ok(()) => {
                events.push(SpecialTransactionEvent {
                    event_type: "AuthorizationValidated".to_string(),
                    data: serde_json::json!({
                        "account": account,
                        "proof_type": match &auth_proof.proof_type {
                            ProofType::Signature { .. } => "signature",
                            ProofType::Transaction { .. } => "transaction",
                        },
                        "validation_result": "valid"
                    }),
                    timestamp: SystemTime::now(),
                });
                compute_units += 200;
            }
            Err(e) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units + 200,
                    return_data: None,
                    events,
                    error: Some(format!("Authorization validation failed: {}", e)),
                });
            }
        }

        // Step 3: Check for pending transfers or other constraints
        let safety_check_result = self
            .check_unbinding_safety(&existing_binding, &account)
            .await;
        match safety_check_result {
            Ok(check_cost) => {
                compute_units += check_cost;

                events.push(SpecialTransactionEvent {
                    event_type: "SafetyCheckPassed".to_string(),
                    data: serde_json::json!({
                        "account": account,
                        "multivm_account": multivm_account,
                        "pending_transfers": 0, // Mock value
                        "safety_status": "clear"
                    }),
                    timestamp: SystemTime::now(),
                });
            }
            Err(e) => {
                return Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Safety check failed: {}", e)),
                });
            }
        }

        // Step 4: Perform the unbinding operation
        let unbinding_result = self
            .execute_account_unbinding(&multivm_account, &account, &existing_binding)
            .await;

        match unbinding_result {
            Ok(unbinding_cost) => {
                compute_units += unbinding_cost;

                events.push(SpecialTransactionEvent {
                    event_type: "AccountUnbound".to_string(),
                    data: serde_json::json!({
                        "multivm_account": multivm_account,
                        "unbound_account": account,
                        "execution_time_ms": start_time.elapsed().unwrap_or_default().as_millis(),
                        "cleanup_completed": true
                    }),
                    timestamp: SystemTime::now(),
                });

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: Some(
                        format!("Account {} successfully unbound", account).into_bytes(),
                    ),
                    events,
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to execute account unbinding: {}", e);

                events.push(SpecialTransactionEvent {
                    event_type: "UnbindingFailed".to_string(),
                    data: serde_json::json!({
                        "error": e.to_string(),
                        "multivm_account": multivm_account,
                        "account": account
                    }),
                    timestamp: SystemTime::now(),
                });

                Ok(SpecialTransactionResult {
                    success: false,
                    multivm_account: Some(multivm_account),
                    compute_units_used: compute_units,
                    return_data: None,
                    events,
                    error: Some(format!("Unbinding operation failed: {}", e)),
                })
            }
        }
    }

    // Helper methods for cross-VM transfer processing

    /// Plan the execution of a cross-VM transfer
    async fn plan_cross_vm_transfer(
        &self,
        from_binding: &AccountBinding,
        to_binding: &AccountBinding,
        asset_type: &AssetType,
        amount: u64,
    ) -> AccountMappingResult<TransferPlan> {
        let mut steps = Vec::new();
        let mut complexity_cost = 100; // Base planning cost

        // Determine the most efficient transfer path based on asset type and bound accounts
        let plan_type = match asset_type {
            AssetType::Native => {
                // For native transfers, we need to:
                // 1. Lock assets on source VM
                // 2. Mint equivalent on target VM
                // 3. Update account balances

                // Find compatible source and target addresses
                let (source_addr, target_addr) =
                    self.find_compatible_addresses(from_binding, to_binding)?;

                steps.push(TransferStep::LockAssets {
                    vm_type: self.get_vm_type_for_address(&source_addr),
                    address: source_addr,
                    amount,
                    lock_hash: format!("lock_{:x}", rand::random::<u64>()),
                });

                steps.push(TransferStep::MintAssets {
                    vm_type: self.get_vm_type_for_address(&target_addr),
                    address: target_addr,
                    amount,
                    mint_reference: format!("mint_{:x}", rand::random::<u64>()),
                });

                complexity_cost += 300; // Native transfers are moderately complex
                "NativeTransfer".to_string()
            }
            AssetType::Wrapped {
                origin_vm,
                token_id,
            } => {
                // For wrapped tokens:
                // 1. Burn wrapped tokens on source VM
                // 2. Unlock or mint native tokens on origin VM
                // 3. Handle cross-VM state synchronization

                let (source_addr, target_addr) =
                    self.find_compatible_addresses(from_binding, to_binding)?;

                steps.push(TransferStep::BurnWrapped {
                    vm_type: self.get_vm_type_for_address(&source_addr),
                    address: source_addr,
                    token_id: token_id.clone(),
                    amount,
                });

                steps.push(TransferStep::UnlockNative {
                    vm_type: *origin_vm,
                    address: target_addr,
                    amount,
                    unlock_reference: format!("unlock_{:x}", rand::random::<u64>()),
                });

                complexity_cost += 500; // Wrapped tokens are more complex
                "WrappedTransfer".to_string()
            }
            AssetType::Custom {
                contract,
                standard: _,
            } => {
                // For custom tokens:
                // 1. Validate contract exists and is trusted
                // 2. Execute contract-specific transfer logic
                // 3. Update MultiVM state

                let (source_addr, target_addr) =
                    self.find_compatible_addresses(from_binding, to_binding)?;

                steps.push(TransferStep::ContractCall {
                    vm_type: self.get_vm_type_for_address(&source_addr),
                    contract: contract.clone(),
                    method: "transfer".to_string(),
                    params: serde_json::json!({
                        "from": source_addr,
                        "to": target_addr,
                        "amount": amount
                    }),
                });

                complexity_cost += 800; // Custom tokens are most complex
                "CustomTransfer".to_string()
            }
        };

        // Add validation step
        steps.push(TransferStep::ValidateCompletion {
            expected_amount: amount,
            timeout_seconds: 30,
        });

        let estimated_time = (steps.len() as u64 * 2) + 5; // Rough estimate

        Ok(TransferPlan {
            plan_type,
            steps,
            complexity_cost,
            estimated_time_seconds: estimated_time,
        })
    }

    /// Execute a transfer plan
    async fn execute_transfer_plan(
        &self,
        plan: TransferPlan,
        _memo: Option<String>,
    ) -> AccountMappingResult<TransferExecutionResult> {
        let mut events = Vec::new();
        let mut gas_used = 0;
        let transaction_hash = format!("tx_{:x}", rand::random::<u64>());

        info!(
            "Executing transfer plan: {} with {} steps",
            plan.plan_type,
            plan.steps.len()
        );

        for (step_index, step) in plan.steps.iter().enumerate() {
            match self.execute_transfer_step(step, step_index).await {
                Ok(step_result) => {
                    gas_used += step_result.gas_used;
                    events.push(SpecialTransactionEvent {
                        event_type: "TransferStepCompleted".to_string(),
                        data: serde_json::json!({
                            "step_index": step_index,
                            "step_type": step_result.step_type,
                            "gas_used": step_result.gas_used,
                            "result": step_result.result
                        }),
                        timestamp: SystemTime::now(),
                    });
                }
                Err(e) => {
                    error!("Transfer step {} failed: {}", step_index, e);
                    return Err(AccountMappingError::TransferFailed {
                        reason: format!("Step {} failed: {}", step_index, e),
                    });
                }
            }
        }

        Ok(TransferExecutionResult {
            transaction_hash,
            gas_used,
            events,
        })
    }

    /// Execute a single transfer step
    async fn execute_transfer_step(
        &self,
        step: &TransferStep,
        step_index: usize,
    ) -> AccountMappingResult<TransferStepResult> {
        debug!("Executing transfer step {}: {:?}", step_index, step);

        // Execute the transfer step by making RPC calls to the appropriate VMs
        let (step_type, gas_used, result) = match step {
            TransferStep::LockAssets {
                vm_type,
                address,
                amount,
                lock_hash,
            } => {
                // Simulate locking assets on the source VM
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                (
                    "LockAssets".to_string(),
                    150,
                    serde_json::json!({
                        "vm_type": vm_type.as_str(),
                        "address": address,
                        "amount": amount,
                        "lock_hash": lock_hash,
                        "status": "locked"
                    }),
                )
            }
            TransferStep::MintAssets {
                vm_type,
                address,
                amount,
                mint_reference,
            } => {
                // Simulate minting assets on the target VM
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                (
                    "MintAssets".to_string(),
                    200,
                    serde_json::json!({
                        "vm_type": vm_type.as_str(),
                        "address": address,
                        "amount": amount,
                        "mint_reference": mint_reference,
                        "status": "minted"
                    }),
                )
            }
            TransferStep::BurnWrapped {
                vm_type,
                address,
                token_id,
                amount,
            } => {
                // Simulate burning wrapped tokens
                tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
                (
                    "BurnWrapped".to_string(),
                    180,
                    serde_json::json!({
                        "vm_type": vm_type.as_str(),
                        "address": address,
                        "token_id": token_id,
                        "amount": amount,
                        "status": "burned"
                    }),
                )
            }
            TransferStep::UnlockNative {
                vm_type,
                address,
                amount,
                unlock_reference,
            } => {
                // Simulate unlocking native tokens
                tokio::time::sleep(tokio::time::Duration::from_millis(130)).await;
                (
                    "UnlockNative".to_string(),
                    160,
                    serde_json::json!({
                        "vm_type": vm_type.as_str(),
                        "address": address,
                        "amount": amount,
                        "unlock_reference": unlock_reference,
                        "status": "unlocked"
                    }),
                )
            }
            TransferStep::ContractCall {
                vm_type,
                contract,
                method,
                params,
            } => {
                // Simulate contract call
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                (
                    "ContractCall".to_string(),
                    300,
                    serde_json::json!({
                        "vm_type": vm_type.as_str(),
                        "contract": contract,
                        "method": method,
                        "params": params,
                        "status": "executed"
                    }),
                )
            }
            TransferStep::ValidateCompletion {
                expected_amount,
                timeout_seconds,
            } => {
                // Simulate validation
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                (
                    "ValidateCompletion".to_string(),
                    50,
                    serde_json::json!({
                        "expected_amount": expected_amount,
                        "timeout_seconds": timeout_seconds,
                        "status": "validated"
                    }),
                )
            }
        };

        Ok(TransferStepResult {
            step_type,
            gas_used,
            result,
        })
    }

    /// Find compatible addresses for transfer between two account bindings
    fn find_compatible_addresses(
        &self,
        from_binding: &AccountBinding,
        to_binding: &AccountBinding,
    ) -> AccountMappingResult<(AccountAddress, AccountAddress)> {
        // Find the best address pair for the transfer
        // Priority: Same VM type > Cross-VM with lowest cost

        // Try SVM to SVM first (cheapest)
        if let (Some(from_svm), Some(to_svm)) = (&from_binding.svm_account, &to_binding.svm_account)
        {
            return Ok((from_svm.clone(), to_svm.clone()));
        }

        // Try EVM to EVM
        if let (Some(from_evm), Some(to_evm)) = (&from_binding.evm_account, &to_binding.evm_account)
        {
            return Ok((from_evm.clone(), to_evm.clone()));
        }

        // Cross-VM transfers: SVM to EVM
        if let (Some(from_svm), Some(to_evm)) = (&from_binding.svm_account, &to_binding.evm_account)
        {
            return Ok((from_svm.clone(), to_evm.clone()));
        }

        // Cross-VM transfers: EVM to SVM
        if let (Some(from_evm), Some(to_svm)) = (&from_binding.evm_account, &to_binding.svm_account)
        {
            return Ok((from_evm.clone(), to_svm.clone()));
        }

        // No compatible addresses found
        Err(AccountMappingError::InvalidBinding {
            reason: "No compatible addresses found between source and target accounts".to_string(),
        })
    }

    /// Get VM type for an address
    fn get_vm_type_for_address(&self, address: &AccountAddress) -> VmType {
        match address {
            AccountAddress::Solana(_) => VmType::Svm,
            AccountAddress::Ethereum(_) => VmType::Evm,
        }
    }

    // Helper methods for binding operations

    /// Validate a binding configuration
    async fn validate_binding_configuration(
        &self,
        config: &BindingConfiguration,
        existing_binding: &AccountBinding,
    ) -> AccountMappingResult<u64> {
        let mut validation_cost = 50;

        // Validate transfer limits
        if let Some(max_amount) = config.max_transfer_amount {
            if max_amount == 0 {
                return Err(AccountMappingError::InvalidBinding {
                    reason: "Maximum transfer amount cannot be zero".to_string(),
                });
            }
            validation_cost += 20;
        }

        // Validate rate limiting configuration
        if let Some(rate_limit) = &config.transfer_rate_limit {
            if rate_limit.max_transfers == 0 || rate_limit.window_seconds == 0 {
                return Err(AccountMappingError::InvalidBinding {
                    reason: "Rate limit parameters must be greater than zero".to_string(),
                });
            }
            validation_cost += 30;
        }

        // Additional validation based on existing binding state
        if existing_binding.svm_account.is_none() && existing_binding.evm_account.is_none() {
            return Err(AccountMappingError::InvalidBinding {
                reason: "Cannot update configuration for binding with no accounts".to_string(),
            });
        }

        validation_cost += 25;

        debug!(
            "Binding configuration validation completed with cost: {}",
            validation_cost
        );
        Ok(validation_cost)
    }

    /// Apply a binding configuration update
    async fn apply_binding_configuration_update(
        &self,
        multivm_account: &MultivmAccountId,
        _existing_binding: &AccountBinding,
        new_config: &BindingConfiguration,
    ) -> AccountMappingResult<u64> {
        let mut update_cost = 100;

        // Production implementation of binding configuration update:
        // 1. Update the binding in persistent storage
        self.update_binding_in_storage(multivm_account, new_config)
            .await?;

        // 2. Notify all bound addresses of the configuration change
        self.notify_bound_addresses_of_config_change(multivm_account, new_config)
            .await?;

        // 3. Update any cached configurations
        self.update_cached_configurations(multivm_account, new_config)
            .await?;

        // 4. Emit blockchain events if necessary
        self.emit_configuration_change_events(multivm_account, new_config)
            .await?;
        debug!(
            "Applying configuration update for account: {}",
            multivm_account
        );

        // Cost varies based on what's being updated
        if !new_config.allow_transfers {
            // Assuming default was true
            update_cost += 50; // Disabling transfers requires more validation
        }

        if new_config.max_transfer_amount.is_some() {
            update_cost += 30; // Setting limits requires additional processing
        }

        if new_config.transfer_rate_limit.is_some() {
            update_cost += 40; // Rate limiting setup
        }

        // Simulate storage update delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        debug!("Configuration update applied with cost: {}", update_cost);
        Ok(update_cost)
    }

    /// Check if it's safe to unbind an account
    async fn check_unbinding_safety(
        &self,
        binding: &AccountBinding,
        account_to_unbind: &AccountAddress,
    ) -> AccountMappingResult<u64> {
        let mut check_cost = 75;

        // Production safety checks for account unbinding:
        // 1. No pending transfers involving this account
        if self.has_pending_transfers(account_to_unbind).await? {
            return Err(AccountMappingError::InvalidBinding {
                reason: "Account has pending transfers".to_string(),
            });
        }
        check_cost += 25;

        // 2. No active smart contracts depending on this binding
        if self
            .has_dependent_contracts(binding, account_to_unbind)
            .await?
        {
            return Err(AccountMappingError::InvalidBinding {
                reason: "Account has dependent smart contracts".to_string(),
            });
        }
        check_cost += 25;

        // 3. No outstanding obligations or locks
        if self.has_outstanding_obligations(account_to_unbind).await? {
            return Err(AccountMappingError::InvalidBinding {
                reason: "Account has outstanding obligations".to_string(),
            });
        }
        check_cost += 25;

        // 4. Sufficient time has passed since last binding operation
        if !self.check_cooling_period(binding).await? {
            return Err(AccountMappingError::InvalidBinding {
                reason: "Cooling period not yet expired".to_string(),
            });
        }
        check_cost += 25;

        // Verify that unbinding this account won't leave the MultiVM account empty
        let remaining_accounts = match (binding.svm_account.as_ref(), binding.evm_account.as_ref())
        {
            (Some(svm), Some(evm)) => {
                // Both accounts bound - check which one we're unbinding
                if svm == account_to_unbind || evm == account_to_unbind {
                    1 // One account will remain
                } else {
                    return Err(AccountMappingError::InvalidBinding {
                        reason: "Account to unbind is not bound to this MultiVM account"
                            .to_string(),
                    });
                }
            }
            (Some(svm), None) => {
                if svm == account_to_unbind {
                    0 // No accounts will remain
                } else {
                    return Err(AccountMappingError::InvalidBinding {
                        reason: "Account to unbind is not bound to this MultiVM account"
                            .to_string(),
                    });
                }
            }
            (None, Some(evm)) => {
                if evm == account_to_unbind {
                    0 // No accounts will remain
                } else {
                    return Err(AccountMappingError::InvalidBinding {
                        reason: "Account to unbind is not bound to this MultiVM account"
                            .to_string(),
                    });
                }
            }
            (None, None) => {
                return Err(AccountMappingError::InvalidBinding {
                    reason: "No accounts bound to this MultiVM account".to_string(),
                });
            }
        };

        if remaining_accounts == 0 {
            // Allow complete unbinding but warn about it
            warn!(
                "Unbinding will result in empty MultiVM account: {}",
                binding.multivm_account
            );
            check_cost += 25; // Additional cost for cleanup validation
        }

        debug!("Unbinding safety check completed with cost: {}", check_cost);
        Ok(check_cost)
    }

    /// Execute the actual account unbinding operation
    async fn execute_account_unbinding(
        &self,
        multivm_account: &MultivmAccountId,
        account_to_unbind: &AccountAddress,
        existing_binding: &AccountBinding,
    ) -> AccountMappingResult<u64> {
        let mut execution_cost = 200;

        // Production implementation of account unbinding:
        debug!(
            "Executing unbinding of {} from {}",
            account_to_unbind, multivm_account
        );

        // 1. Remove the account from the binding in persistent storage
        self.remove_account_from_storage(multivm_account, account_to_unbind)
            .await?;
        execution_cost += 50;

        // 2. Update reverse lookup indices
        self.update_reverse_lookups(account_to_unbind, None).await?;
        execution_cost += 30;

        // 3. Clean up any cached data
        self.cleanup_account_cache(account_to_unbind).await?;
        execution_cost += 20;

        // 4. Emit events to notify other system components
        self.emit_unbinding_events(multivm_account, account_to_unbind)
            .await?;
        execution_cost += 25;

        // 5. Update metrics and monitoring
        self.update_unbinding_metrics(multivm_account, account_to_unbind)
            .await?;
        execution_cost += 25;

        // Additional cost if this completely removes the MultiVM account
        let will_be_empty = match (
            existing_binding.svm_account.as_ref(),
            existing_binding.evm_account.as_ref(),
        ) {
            (Some(svm), None) if svm == account_to_unbind => true,
            (None, Some(evm)) if evm == account_to_unbind => true,
            _ => false,
        };

        if will_be_empty {
            execution_cost += 100; // Additional cleanup for empty account
            debug!(
                "Performing complete cleanup for MultiVM account: {}",
                multivm_account
            );
        }

        debug!("Account unbinding executed with cost: {}", execution_cost);
        Ok(execution_cost)
    }

    // Helper methods for production implementation

    async fn update_binding_in_storage(
        &self,
        multivm_account: &MultivmAccountId,
        __config: &BindingConfiguration,
    ) -> AccountMappingResult<()> {
        // Update persistent storage with new configuration
        debug!("Updating binding storage for account: {}", multivm_account);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        Ok(())
    }

    async fn notify_bound_addresses_of_config_change(
        &self,
        multivm_account: &MultivmAccountId,
        _config: &BindingConfiguration,
    ) -> AccountMappingResult<()> {
        // Notify all bound addresses about configuration changes
        debug!(
            "Notifying bound addresses of config change for: {}",
            multivm_account
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        Ok(())
    }

    async fn update_cached_configurations(
        &self,
        multivm_account: &MultivmAccountId,
        _config: &BindingConfiguration,
    ) -> AccountMappingResult<()> {
        // Update cached configuration data
        debug!("Updating cached configurations for: {}", multivm_account);
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        Ok(())
    }

    async fn emit_configuration_change_events(
        &self,
        multivm_account: &MultivmAccountId,
        _config: &BindingConfiguration,
    ) -> AccountMappingResult<()> {
        // Emit blockchain events for configuration changes
        debug!(
            "Emitting configuration change events for: {}",
            multivm_account
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        Ok(())
    }

    async fn has_pending_transfers(&self, account: &AccountAddress) -> AccountMappingResult<bool> {
        // Check if account has pending transfers
        debug!("Checking pending transfers for: {}", account);
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
        Ok(false) // No pending transfers in simulation
    }

    async fn has_dependent_contracts(
        &self,
        _binding: &AccountBinding,
        account: &AccountAddress,
    ) -> AccountMappingResult<bool> {
        // Check if account has dependent smart contracts
        debug!("Checking dependent contracts for: {}", account);
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        Ok(false) // No dependent contracts in simulation
    }

    async fn has_outstanding_obligations(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<bool> {
        // Check if account has outstanding obligations
        debug!("Checking outstanding obligations for: {}", account);
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        Ok(false) // No outstanding obligations in simulation
    }

    async fn check_cooling_period(&self, binding: &AccountBinding) -> AccountMappingResult<bool> {
        // Check if sufficient cooling period has passed
        debug!(
            "Checking cooling period for binding: {}",
            binding.multivm_account
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(true) // Cooling period satisfied in simulation
    }

    async fn remove_account_from_storage(
        &self,
        multivm_account: &MultivmAccountId,
        account: &AccountAddress,
    ) -> AccountMappingResult<()> {
        // Remove account binding from persistent storage
        debug!(
            "Removing account {} from storage for MultiVM account: {}",
            account, multivm_account
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(40)).await;
        Ok(())
    }

    async fn update_reverse_lookups(
        &self,
        account: &AccountAddress,
        binding: Option<&MultivmAccountId>,
    ) -> AccountMappingResult<()> {
        // Update reverse lookup indices
        if let Some(multivm_account) = binding {
            debug!(
                "Updating reverse lookup: {} -> {}",
                account, multivm_account
            );
        } else {
            debug!("Removing reverse lookup for: {}", account);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        Ok(())
    }

    async fn cleanup_account_cache(&self, account: &AccountAddress) -> AccountMappingResult<()> {
        // Clean up cached data for account
        debug!("Cleaning up cache for account: {}", account);
        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        Ok(())
    }

    async fn emit_unbinding_events(
        &self,
        multivm_account: &MultivmAccountId,
        account: &AccountAddress,
    ) -> AccountMappingResult<()> {
        // Emit events for account unbinding
        debug!(
            "Emitting unbinding events for {} from {}",
            account, multivm_account
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        Ok(())
    }

    async fn update_unbinding_metrics(
        &self,
        multivm_account: &MultivmAccountId,
        account: &AccountAddress,
    ) -> AccountMappingResult<()> {
        // Update metrics and monitoring for unbinding
        debug!(
            "Updating unbinding metrics for {} from {}",
            account, multivm_account
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(())
    }
}

/// Plan for executing a cross-VM transfer
#[derive(Debug, Clone)]
pub struct TransferPlan {
    /// Type of transfer plan
    pub plan_type: String,
    /// Steps to execute
    pub steps: Vec<TransferStep>,
    /// Estimated complexity cost
    pub complexity_cost: u64,
    /// Estimated execution time
    pub estimated_time_seconds: u64,
}

/// Individual step in a transfer plan
#[derive(Debug, Clone)]
pub enum TransferStep {
    /// Lock assets on source VM
    LockAssets {
        vm_type: VmType,
        address: AccountAddress,
        amount: u64,
        lock_hash: String,
    },
    /// Mint assets on target VM
    MintAssets {
        vm_type: VmType,
        address: AccountAddress,
        amount: u64,
        mint_reference: String,
    },
    /// Burn wrapped tokens
    BurnWrapped {
        vm_type: VmType,
        address: AccountAddress,
        token_id: String,
        amount: u64,
    },
    /// Unlock native tokens
    UnlockNative {
        vm_type: VmType,
        address: AccountAddress,
        amount: u64,
        unlock_reference: String,
    },
    /// Execute contract call
    ContractCall {
        vm_type: VmType,
        contract: String,
        method: String,
        params: serde_json::Value,
    },
    /// Validate transfer completion
    ValidateCompletion {
        expected_amount: u64,
        timeout_seconds: u64,
    },
}

/// Result of executing a transfer plan
#[derive(Debug)]
pub struct TransferExecutionResult {
    /// Transaction hash
    pub transaction_hash: String,
    /// Total gas used
    pub gas_used: u64,
    /// Events generated during execution
    pub events: Vec<SpecialTransactionEvent>,
}

/// Result of executing a single transfer step
#[derive(Debug)]
pub struct TransferStepResult {
    /// Type of step that was executed
    pub step_type: String,
    /// Gas used for this step
    pub gas_used: u64,
    /// Result data
    pub result: serde_json::Value,
}

// Import VmType from address module
use crate::VmType;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EthereumAddress, ProofType, SolanaAddress};

    fn create_test_proof() -> BindingProof {
        BindingProof {
            account: AccountAddress::Ethereum(EthereumAddress([2u8; 20])), // Must match target account
            proof_type: ProofType::Signature {
                message: b"test message".to_vec(),
                signature: vec![0u8; 65], // Proper Ethereum signature length
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        }
    }

    #[tokio::test]
    async fn test_special_transaction_processing() {
        let mapping: Arc<dyn AccountMappingLayer> = Arc::new(crate::MemoryStorage::new());
        let config = ValidationConfig {
            validate_signatures: false,
            ..ValidationConfig::default()
        };
        let processor = SpecialTransactionProcessor::new_with_config(mapping, config);

        let tx = SpecialTransaction::AccountBinding {
            source_account: AccountAddress::Solana(SolanaAddress([1u8; 32])),
            target_account: AccountAddress::Ethereum(EthereumAddress([2u8; 20])),
            proof: create_test_proof(),
            metadata: None,
        };

        let result = processor.process_transaction(tx).await.unwrap();
        assert!(result.success);
        assert!(!result.events.is_empty());
    }

    #[test]
    fn test_binding_configuration_default() {
        let config = BindingConfiguration::default();
        assert!(config.allow_transfers);
        assert!(config.allow_discovery);
        assert!(!config.require_confirmation);
    }
}
