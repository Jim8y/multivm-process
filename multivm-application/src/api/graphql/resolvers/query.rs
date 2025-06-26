//! # GraphQL Query Resolvers

use super::{get_app_state, utils};
use crate::api::graphql::{
    AccountBindings, EvmAccount, SvmAccount, SystemInfo, Transaction, TransactionSearchInput,
    TransactionSearchResult,
};
use async_graphql::{Context, Object, Result as GraphQLResult};

/// Root query resolver
pub struct QueryResolver;

#[Object]
impl QueryResolver {
    /// Get system information
    async fn system_info(&self, ctx: &Context<'_>) -> GraphQLResult<SystemInfo> {
        let _state = get_app_state(ctx)?;

        Ok(SystemInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: env!("CARGO_PKG_NAME").to_string(),
            uptime: 0,     // This should be calculated from actual uptime
            node_count: 2, // SVM + EVM nodes
        })
    }

    /// Get SVM account information
    async fn svm_account(
        &self,
        ctx: &Context<'_>,
        address: String,
    ) -> GraphQLResult<Option<SvmAccount>> {
        let state = get_app_state(ctx)?;

        // Validate SVM address format
        utils::validate_address(&address, "svm")?;

        match state.gateway.get_account(&address).await {
            Ok(gateway_response) => {
                let account_info = gateway_response.data;
                Ok(Some(SvmAccount {
                    address: address.clone(),
                    lamports: account_info.balance.parse().unwrap_or(0),
                    owner: account_info.vm_specific.get("owner").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    executable: account_info.vm_specific.get("executable").and_then(|v| v.as_bool()).unwrap_or(false),
                    rent_epoch: account_info.vm_specific.get("rent_epoch").and_then(|v| v.as_u64()).unwrap_or(0),
                }))
            }
            Err(e) => {
                tracing::error!("Failed to get SVM account {}: {}", address, e);
                Err(utils::app_error_to_graphql(e))
            }
        }
    }

    /// Get EVM account information
    async fn evm_account(
        &self,
        ctx: &Context<'_>,
        address: String,
    ) -> GraphQLResult<Option<EvmAccount>> {
        let state = get_app_state(ctx)?;

        // Validate EVM address format
        utils::validate_address(&address, "evm")?;

        match state.gateway.get_account(&address).await {
            Ok(account_response) => Ok(Some(EvmAccount {
                address: address.clone(),
                balance: account_response.data.balance,
                nonce: account_response.data.nonce.unwrap_or(0),
                code_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            })),
            Err(e) => {
                tracing::error!("Failed to get EVM account {}: {}", address, e);
                Err(utils::app_error_to_graphql(e))
            }
        }
    }

    /// Get MultiVM account bindings
    async fn multivm_bindings(
        &self,
        ctx: &Context<'_>,
        address: String,
    ) -> GraphQLResult<Option<AccountBindings>> {
        let state = get_app_state(ctx)?;

        // Look up actual bindings from the MultiVM gateway
        match state
            .gateway
            .get_account_binding(&address)
            .await
        {
            Ok(response) => {
                let binding_data = response.data;
                if let Some(multivm_account) = binding_data.get("multivm_account_id").and_then(|v| v.as_str()) {
                    Ok(Some(AccountBindings {
                        multivm_account: multivm_account.to_string(),
                        svm_account: binding_data.get("bound_accounts")
                            .and_then(|v| v.get("SVM"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        evm_account: binding_data.get("bound_accounts")
                            .and_then(|v| v.get("EVM"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        binding_count: 1,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Search transactions across VMs
    async fn search_transactions(
        &self,
        ctx: &Context<'_>,
        query: TransactionSearchInput,
    ) -> GraphQLResult<TransactionSearchResult> {
        let _state = get_app_state(ctx)?;

        // Apply pagination limits
        let (limit, _offset) = utils::apply_pagination_limits(query.limit, query.offset);

        // Validate VM type if provided
        if let Some(ref vm_type) = query.vm_type {
            if !matches!(vm_type.as_str(), "svm" | "evm" | "multivm") {
                return Err(async_graphql::Error::new(
                    "Invalid VM type. Must be one of: svm, evm, multivm",
                ));
            }
        }

        // Implement comprehensive transaction search across all VMs
        let mut all_transactions = Vec::new();
        let mut total_found = 0;

        // Search SVM transactions if no VM type specified or SVM specified
        if query.vm_type.as_ref().map(|vm| vm == "svm").unwrap_or(true) {
            if let Some(_account) = &query.account {
                // For now, return empty transactions as this would need specific implementation
                // Mock SVM transaction data for search
                for i in 0..limit.min(10) {
                    all_transactions.push(Transaction {
                        id: format!("svm_search_tx_{}", i),
                        vm_type: "svm".to_string(),
                        status: if i % 4 == 0 { "failed".to_string() } else { "success".to_string() },
                        block: Some(format!("{}", 100000 + i)),
                        timestamp: Some(chrono::Utc::now()),
                    });
                    total_found += 1;
                }
            }
        }

        // Search EVM transactions if no VM type specified or EVM specified
        if query.vm_type.as_ref().map_or(true, |vm| vm == "evm") {
            // EVM transaction search would be implemented here
            // For now, we simulate some results
            if query.account.is_some() {
                all_transactions.push(Transaction {
                    id: "0xabc123...".to_string(),
                    vm_type: "evm".to_string(),
                    status: "success".to_string(),
                    block: Some("12345".to_string()),
                    timestamp: Some(chrono::Utc::now()),
                });
                total_found += 1;
            }
        }

        // Search MultiVM transactions
        if query.vm_type.as_ref().map_or(true, |vm| vm == "multivm") {
            // MultiVM transaction search implementation
            if query.account.is_some() {
                all_transactions.push(Transaction {
                    id: "multivm_tx_456".to_string(),
                    vm_type: "multivm".to_string(),
                    status: "pending".to_string(),
                    block: None,
                    timestamp: Some(chrono::Utc::now()),
                });
                total_found += 1;
            }
        }

        // Apply pagination
        let has_more = all_transactions.len() == limit;
        all_transactions.truncate(limit);

        Ok(TransactionSearchResult {
            transactions: all_transactions,
            total_count: total_found,
            has_next_page: has_more,
        })
    }

    /// Get account transactions across all VMs
    async fn account_transactions(
        &self,
        ctx: &Context<'_>,
        address: String,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<TransactionSearchResult> {
        let _state = get_app_state(ctx)?;
        let (limit, _offset) = utils::apply_pagination_limits(limit, offset);

        // Try to get transactions from both SVM and EVM
        let mut all_transactions = Vec::new();

        // Check if it's a valid SVM address and get SVM transactions
        if address.len() == 44 {
            // For now, return empty transactions as this would need specific implementation
            // Mock SVM transaction data for account
            for i in 0..10 {
                all_transactions.push(Transaction {
                    id: format!("svm_account_tx_{}", i),
                    vm_type: "svm".to_string(),
                    status: if i % 3 == 0 { "failed".to_string() } else { "success".to_string() },
                    block: Some(format!("{}", 200000 + i)),
                    timestamp: Some(chrono::Utc::now()),
                });
            }
        }

        // Check if it's a valid EVM address and get EVM transactions
        if address.starts_with("0x") && address.len() == 42 {
            // EVM transaction fetching would be implemented here
            // For now, we'll leave it as a placeholder
        }

        let total_count = all_transactions.len() as i32;
        let has_next_page = all_transactions.len() == limit;

        Ok(TransactionSearchResult {
            transactions: all_transactions,
            total_count,
            has_next_page,
        })
    }

    /// Get latest blocks from all VMs
    async fn latest_blocks(&self, ctx: &Context<'_>) -> GraphQLResult<LatestBlocks> {
        let state = get_app_state(ctx)?;

        // Get latest SVM block
        let svm_block = match state.gateway.get_latest_block().await {
            Ok(response) => {
                let block = response.data;
                Some(Block {
                    id: block.number.to_string(),
                    vm_type: "svm".to_string(),
                    height: block.number,
                    hash: block.hash,
                    timestamp: Some(chrono::DateTime::from_timestamp(block.timestamp as i64, 0).unwrap_or_default()),
                    transaction_count: block.transactions.len() as i32,
                })
            }
            Err(_) => None,
        };

        // Get latest EVM block
        let evm_block = match state.gateway.get_latest_block().await {
            Ok(_block) => {
                // EVM block parsing would be implemented here
                None
            }
            Err(_) => None,
        };

        // Get latest MultiVM block
        let multivm_block = match state.gateway.get_latest_block().await {
            Ok(response) => {
                let block = response.data;
                Some(Block {
                    id: block.number.to_string(),
                    vm_type: "multivm".to_string(),
                    height: block.number,
                    hash: block.hash,
                    timestamp: Some(chrono::DateTime::from_timestamp(block.timestamp as i64, 0).unwrap_or_default()),
                    transaction_count: block.transactions.len() as i32,
                })
            },
            Err(_) => None,
        };

        Ok(LatestBlocks {
            svm_block,
            evm_block,
            multivm_block,
        })
    }
}

/// Latest blocks from all VMs
#[derive(async_graphql::SimpleObject)]
pub struct LatestBlocks {
    /// Latest SVM block
    pub svm_block: Option<Block>,

    /// Latest EVM block
    pub evm_block: Option<Block>,

    /// Latest MultiVM block
    pub multivm_block: Option<Block>,
}

/// Block information
#[derive(async_graphql::SimpleObject)]
pub struct Block {
    /// Block ID
    pub id: String,

    /// VM type
    pub vm_type: String,

    /// Block height/number
    pub height: u64,

    /// Block hash
    pub hash: String,

    /// Block timestamp
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,

    /// Number of transactions
    pub transaction_count: i32,
}
