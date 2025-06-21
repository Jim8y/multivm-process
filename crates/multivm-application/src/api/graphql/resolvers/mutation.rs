//! GraphQL Mutation Resolvers

use super::{get_app_state, utils};
use crate::api::graphql::{AccountBindingInput, AccountBindingResult, TransactionResult};
use async_graphql::{Context, Object, Result as GraphQLResult};
use sha2::Digest;

/// Root mutation resolver
pub struct MutationResolver;

#[Object]
impl MutationResolver {
    /// Send SVM transaction
    async fn send_svm_transaction(
        &self,
        ctx: &Context<'_>,
        transaction_data: String,
    ) -> GraphQLResult<TransactionResult> {
        let state = get_app_state(ctx)?;

        // Validate transaction data is not empty
        if transaction_data.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "Transaction data cannot be empty",
            ));
        }

        match state.svm_gateway.send_transaction(&transaction_data).await {
            Ok(response) => Ok(TransactionResult {
                success: true,
                transaction_id: response.data,
                error: None,
            }),
            Err(e) => {
                tracing::error!("Failed to send SVM transaction: {}", e);
                Ok(TransactionResult {
                    success: false,
                    transaction_id: String::new(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Send EVM transaction
    async fn send_evm_transaction(
        &self,
        ctx: &Context<'_>,
        transaction_data: String,
    ) -> GraphQLResult<TransactionResult> {
        let state = get_app_state(ctx)?;

        // Validate transaction data is not empty
        if transaction_data.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "Transaction data cannot be empty",
            ));
        }

        match state
            .evm_gateway
            .send_raw_transaction(&transaction_data)
            .await
        {
            Ok(hash) => Ok(TransactionResult {
                success: true,
                transaction_id: hash,
                error: None,
            }),
            Err(e) => {
                tracing::error!("Failed to send EVM transaction: {}", e);
                Ok(TransactionResult {
                    success: false,
                    transaction_id: String::new(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Bind accounts across VMs
    async fn bind_accounts(
        &self,
        ctx: &Context<'_>,
        input: AccountBindingInput,
    ) -> GraphQLResult<AccountBindingResult> {
        let state = get_app_state(ctx)?;

        // Validate that at least one account is provided
        if input.svm_account.is_none() && input.evm_account.is_none() {
            return Err(async_graphql::Error::new(
                "At least one account (SVM or EVM) must be provided for binding",
            ));
        }

        // Validate SVM account format if provided
        if let Some(ref svm_account) = input.svm_account {
            utils::validate_address(svm_account, "svm")?;
        }

        // Validate EVM account format if provided
        if let Some(ref evm_account) = input.evm_account {
            utils::validate_address(evm_account, "evm")?;
        }

        let svm_addr = input.svm_account.clone().unwrap_or_default();
        let evm_addr = input.evm_account.clone().unwrap_or_default();

        match state
            .multivm_gateway
            .bind_accounts(svm_addr.clone(), evm_addr.clone())
            .await
        {
            Ok(binding_id) => Ok(AccountBindingResult {
                success: true,
                multivm_account: format!("{}:{}", svm_addr, evm_addr),
                binding_id,
                error: None,
            }),
            Err(e) => {
                tracing::error!("Failed to bind accounts: {}", e);
                Ok(AccountBindingResult {
                    success: false,
                    multivm_account: String::new(),
                    binding_id: String::new(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Unbind accounts
    async fn unbind_accounts(
        &self,
        ctx: &Context<'_>,
        binding_id: String,
    ) -> GraphQLResult<UnbindResult> {
        let state = get_app_state(ctx)?;

        // Validate binding ID is not empty
        if binding_id.trim().is_empty() {
            return Err(async_graphql::Error::new("Binding ID cannot be empty"));
        }

        match state.multivm_gateway.unbind_accounts(&binding_id).await {
            Ok(_) => Ok(UnbindResult {
                success: true,
                message: "Accounts unbound successfully".to_string(),
                error: None,
            }),
            Err(e) => {
                tracing::error!("Failed to unbind accounts with ID {}: {}", binding_id, e);
                Ok(UnbindResult {
                    success: false,
                    message: String::new(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Send cross-VM transaction
    async fn send_cross_vm_transaction(
        &self,
        ctx: &Context<'_>,
        input: CrossVmTransactionInput,
    ) -> GraphQLResult<CrossVmTransactionResult> {
        let state = get_app_state(ctx)?;

        // Validate VM types
        if !matches!(input.from_vm.as_str(), "svm" | "evm") {
            return Err(async_graphql::Error::new(
                "from_vm must be either 'svm' or 'evm'",
            ));
        }

        if !matches!(input.to_vm.as_str(), "svm" | "evm") {
            return Err(async_graphql::Error::new(
                "to_vm must be either 'svm' or 'evm'",
            ));
        }

        if input.from_vm == input.to_vm {
            return Err(async_graphql::Error::new(
                "from_vm and to_vm cannot be the same",
            ));
        }

        // Validate transaction data is not empty
        if input.transaction_data.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "Transaction data cannot be empty",
            ));
        }

        // Parse the transaction data to extract cross-VM transaction components
        let cross_vm_data: Result<serde_json::Value, _> = serde_json::from_str(&input.transaction_data);
        let (svm_tx, evm_tx) = match cross_vm_data {
            Ok(data) => {
                // Extract source and target transaction data
                let svm_part = data.get("svm_transaction")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&input.transaction_data);
                    
                let default_evm_tx = format!("0x{}", hex::encode(&input.transaction_data.as_bytes()[..std::cmp::min(32, input.transaction_data.len())]));
                let evm_part = data.get("evm_transaction")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&default_evm_tx);

                if input.from_vm == "svm" {
                    (svm_part.to_string(), evm_part.to_string())
                } else {
                    (svm_part.to_string(), evm_part.to_string())
                }
            }
            Err(_) => {
                // Fallback to simple transaction data splitting
                if input.from_vm == "svm" {
                    (
                        input.transaction_data.clone(),
                        format!("0x{}", hex::encode(sha2::Sha256::digest(input.transaction_data.as_bytes()))),
                    )
                } else {
                    (
                        format!("svm_tx_{}", uuid::Uuid::new_v4()),
                        input.transaction_data.clone(),
                    )
                }
            }
        };

        match state
            .multivm_gateway
            .send_cross_vm_transaction(svm_tx, evm_tx)
            .await
        {
            Ok(transaction_id) => Ok(CrossVmTransactionResult {
                success: true,
                transaction_id,
                status: "pending".to_string(),
                error: None,
            }),
            Err(e) => {
                tracing::error!("Failed to send cross-VM transaction: {}", e);
                Ok(CrossVmTransactionResult {
                    success: false,
                    transaction_id: String::new(),
                    status: "failed".to_string(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Simulate transaction
    async fn simulate_transaction(
        &self,
        ctx: &Context<'_>,
        vm_type: String,
        transaction_data: String,
    ) -> GraphQLResult<SimulationResult> {
        let state = get_app_state(ctx)?;

        // Validate VM type
        if !matches!(vm_type.as_str(), "svm" | "evm") {
            return Err(async_graphql::Error::new(
                "vm_type must be either 'svm' or 'evm'",
            ));
        }

        // Validate transaction data is not empty
        if transaction_data.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "Transaction data cannot be empty",
            ));
        }

        match vm_type.as_str() {
            "svm" => {
                match state
                    .svm_gateway
                    .simulate_transaction(&transaction_data)
                    .await
                {
                    Ok(response) => {
                        let result = &response.data;
                        Ok(SimulationResult {
                            success: result.get("err").map_or(true, |e| e.is_null()),
                            logs: result
                                .get("logs")
                                .and_then(|l| l.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            units_consumed: result.get("units_consumed").and_then(|u| u.as_u64()),
                            return_data: result
                                .get("return_data")
                                .and_then(|d| d.as_str())
                                .map(|s| s.to_string()),
                            error: result.get("err").and_then(|e| {
                                if !e.is_null() {
                                    Some(e.to_string())
                                } else {
                                    None
                                }
                            }),
                        })
                    }
                    Err(e) => {
                        tracing::error!("Failed to simulate SVM transaction: {}", e);
                        Ok(SimulationResult {
                            success: false,
                            logs: vec![],
                            units_consumed: None,
                            return_data: None,
                            error: Some(e.to_string()),
                        })
                    }
                }
            }
            "evm" => {
                match state
                    .evm_gateway
                    .simulate_transaction(&transaction_data)
                    .await
                {
                    Ok(simulation_result) => {
                        Ok(SimulationResult {
                            success: simulation_result.success,
                            logs: simulation_result.logs.iter().map(|log| {
                                format!("Event from {}: {} topics, data: {}", 
                                    log.address, 
                                    log.topics.len(), 
                                    &log.data[..std::cmp::min(20, log.data.len())]
                                )
                            }).collect(),
                            units_consumed: Some(simulation_result.gas_used),
                            return_data: simulation_result.return_data,
                            error: simulation_result.error,
                        })
                    }
                    Err(e) => {
                        tracing::error!("Failed to simulate EVM transaction: {}", e);
                        Ok(SimulationResult {
                            success: false,
                            logs: vec![],
                            units_consumed: None,
                            return_data: None,
                            error: Some(e.to_string()),
                        })
                    }
                }
            }
            _ => unreachable!(), // Already validated above
        }
    }
}

/// Unbind result
#[derive(async_graphql::SimpleObject)]
pub struct UnbindResult {
    /// Whether the operation was successful
    pub success: bool,

    /// Success message
    pub message: String,

    /// Error message if failed
    pub error: Option<String>,
}

/// Cross-VM transaction input
#[derive(async_graphql::InputObject)]
pub struct CrossVmTransactionInput {
    /// Source VM type
    pub from_vm: String,

    /// Target VM type
    pub to_vm: String,

    /// Transaction data
    pub transaction_data: String,
}

/// Cross-VM transaction result
#[derive(async_graphql::SimpleObject)]
pub struct CrossVmTransactionResult {
    /// Whether the operation was successful
    pub success: bool,

    /// Transaction ID
    pub transaction_id: String,

    /// Transaction status
    pub status: String,

    /// Error message if failed
    pub error: Option<String>,
}

/// Transaction simulation result
#[derive(async_graphql::SimpleObject)]
pub struct SimulationResult {
    /// Whether the simulation was successful
    pub success: bool,

    /// Simulation logs
    pub logs: Vec<String>,

    /// Compute units consumed (SVM only)
    pub units_consumed: Option<u64>,

    /// Return data from the simulation
    pub return_data: Option<String>,

    /// Error message if failed
    pub error: Option<String>,
}
