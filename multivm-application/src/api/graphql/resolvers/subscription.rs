//! GraphQL Subscription Resolvers

use async_graphql::{Context, Subscription};
use futures_util::Stream;
use std::time::Duration;

/// Root subscription resolver
pub struct SubscriptionResolver;

#[Subscription]
impl SubscriptionResolver {
    /// Subscribe to new blocks from all VMs
    async fn new_blocks(&self, ctx: &Context<'_>) -> impl Stream<Item = NewBlockEvent> {
        // Get application state to access gateways
        let state = ctx
            .data::<std::sync::Arc<crate::ApplicationState>>()
            .expect("ApplicationState not found in context")
            .clone();

        async_stream::stream! {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            let mut last_svm_block = 0u64;
            let mut last_evm_block = 0u64;

            loop {
                interval.tick().await;

                // Check for new SVM blocks
                if let Ok(svm_block) = state.gateway.get_latest_block().await {
                    let block_slot = svm_block.data.number;
                    if block_slot > last_svm_block {
                        last_svm_block = block_slot;
                        yield NewBlockEvent {
                            vm_type: "svm".to_string(),
                            block_number: block_slot,
                            block_hash: svm_block.data.hash,
                            timestamp: chrono::Utc::now(),
                        };
                    }
                }

                // Check for new EVM blocks (mock implementation)
                if let Ok(_evm_block) = state.gateway.get_latest_block().await {
                    // For now, simulate EVM block progression
                    last_evm_block += 1;
                    yield NewBlockEvent {
                        vm_type: "evm".to_string(),
                        block_number: last_evm_block,
                        block_hash: format!("0x{:064x}", last_evm_block),
                        timestamp: chrono::Utc::now(),
                    };
                }

                // Check for new MultiVM blocks
                if let Ok(multivm_block) = state.gateway.get_latest_block().await {
                    yield NewBlockEvent {
                        vm_type: "multivm".to_string(),
                        block_number: multivm_block.data.number,
                        block_hash: multivm_block.data.hash,
                        timestamp: chrono::Utc::now(),
                    };
                }
            }
        }
    }

    /// Subscribe to new transactions
    async fn new_transactions(
        &self,
        _ctx: &Context<'_>,
    ) -> impl Stream<Item = NewTransactionEvent> {
        async_stream::stream! {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            let mut counter = 0u64;

            loop {
                interval.tick().await;
                counter += 1;

                yield NewTransactionEvent {
                    vm_type: if counter % 3 == 0 { "multivm".to_string() }
                             else if counter % 2 == 0 { "svm".to_string() }
                             else { "evm".to_string() },
                    transaction_id: format!("tx_{}", counter),
                    status: "pending".to_string(),
                    timestamp: chrono::Utc::now(),
                };
            }
        }
    }

    /// Subscribe to account updates
    async fn account_updates(
        &self,
        _ctx: &Context<'_>,
        address: String,
    ) -> impl Stream<Item = AccountUpdateEvent> {
        async_stream::stream! {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                yield AccountUpdateEvent {
                    address: address.clone(),
                    vm_type: if address.starts_with("0x") { "evm".to_string() } else { "svm".to_string() },
                    balance_change: "0".to_string(),
                    timestamp: chrono::Utc::now(),
                };
            }
        }
    }

    /// Subscribe to system status updates
    async fn system_status(&self, _ctx: &Context<'_>) -> impl Stream<Item = SystemStatusEvent> {
        async_stream::stream! {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                yield SystemStatusEvent {
                    status: "healthy".to_string(),
                    node_count: 2,
                    active_connections: 150,
                    timestamp: chrono::Utc::now(),
                };
            }
        }
    }
}

/// New block event
#[derive(async_graphql::SimpleObject)]
pub struct NewBlockEvent {
    /// VM type that produced the block
    pub vm_type: String,

    /// Block number
    pub block_number: u64,

    /// Block hash
    pub block_hash: String,

    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// New transaction event
#[derive(async_graphql::SimpleObject)]
pub struct NewTransactionEvent {
    /// VM type
    pub vm_type: String,

    /// Transaction ID
    pub transaction_id: String,

    /// Transaction status
    pub status: String,

    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Account update event
#[derive(async_graphql::SimpleObject)]
pub struct AccountUpdateEvent {
    /// Account address
    pub address: String,

    /// VM type
    pub vm_type: String,

    /// Balance change
    pub balance_change: String,

    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// System status event
#[derive(async_graphql::SimpleObject)]
pub struct SystemStatusEvent {
    /// System status
    pub status: String,

    /// Number of connected nodes
    pub node_count: i32,

    /// Active connections
    pub active_connections: i32,

    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
