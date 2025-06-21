//! # GraphQL API Module
//!
//! Provides a GraphQL interface for flexible querying of blockchain data
//! across both SVM and EVM execution environments.

pub mod resolvers;

use crate::{ApplicationResult, ApplicationState};
use async_graphql::{Context, EmptySubscription, Object, Result as GraphQLResult, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::Html,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

/// GraphQL server configuration
#[derive(Debug, Clone)]
pub struct GraphQLConfig {
    /// Enable GraphQL Playground
    pub enable_playground: bool,

    /// Maximum query depth
    pub max_query_depth: usize,

    /// Maximum query complexity
    pub max_query_complexity: usize,

    /// Enable introspection
    pub enable_introspection: bool,
}

impl Default for GraphQLConfig {
    fn default() -> Self {
        Self {
            enable_playground: true,
            max_query_depth: 10,
            max_query_complexity: 1000,
            enable_introspection: true,
        }
    }
}

/// GraphQL server
pub struct GraphQLServer {
    state: Arc<ApplicationState>,
    config: GraphQLConfig,
    schema: Schema<QueryRoot, MutationRoot, EmptySubscription>,
}

/// GraphQL Query root
pub struct QueryRoot;

/// GraphQL Mutation root
pub struct MutationRoot;

/// GraphQL schema type
pub type MultivmSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

impl GraphQLServer {
    /// Create a new GraphQL server
    pub fn new(state: Arc<ApplicationState>, config: GraphQLConfig) -> Self {
        let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription)
            .data(state.clone())
            .limit_depth(config.max_query_depth)
            .limit_complexity(config.max_query_complexity)
            .enable_federation()
            .finish();

        Self {
            state,
            config,
            schema,
        }
    }
}

#[Object]
impl QueryRoot {
    /// Get system information
    async fn system_info(&self, ctx: &Context<'_>) -> GraphQLResult<SystemInfo> {
        let _state = ctx.data::<Arc<ApplicationState>>()?;

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
        let state = ctx.data::<Arc<ApplicationState>>()?;

        match state.svm_gateway.get_account_info(&address).await {
            Ok(gateway_response) => {
                if let Some(account_info) = gateway_response.data {
                    Ok(Some(SvmAccount {
                        address: address.clone(),
                        lamports: account_info.lamports,
                        owner: account_info.owner,
                        executable: account_info.executable,
                        rent_epoch: account_info.rent_epoch,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Get EVM account information
    async fn evm_account(
        &self,
        ctx: &Context<'_>,
        address: String,
    ) -> GraphQLResult<Option<EvmAccount>> {
        let state = ctx.data::<Arc<ApplicationState>>()?;

        match state.evm_gateway.get_balance(&address).await {
            Ok(balance) => Ok(Some(EvmAccount {
                address: address.clone(),
                balance: balance,
                nonce: 0, // Mock value
                code_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            })),
            Err(_) => Ok(None),
        }
    }

    /// Get MultiVM account bindings
    async fn multivm_bindings(
        &self,
        ctx: &Context<'_>,
        address: String,
    ) -> GraphQLResult<Option<AccountBindings>> {
        let state = ctx.data::<Arc<ApplicationState>>()?;

        // Parse the address and look up actual bindings from the MultiVM gateway
        match multivm_account_mapping::AccountAddress::from_string(&address) {
            Ok(account_address) => {
                match state.multivm_gateway.get_account_binding(&account_address).await {
                    Ok(Some(binding_info)) => Ok(Some(AccountBindings {
                        multivm_account: binding_info.multivm_id,
                        svm_account: binding_info.svm_address,
                        evm_account: binding_info.evm_address,
                        binding_count: 1,
                    })),
                    Ok(None) => Ok(None),
                    Err(_) => Ok(None),
                }
            }
            Err(_) => {
                // Invalid address format
                tracing::warn!("Invalid address format for multivm_bindings query: {}", address);
                Ok(None)
            }
        }
    }

    /// Search transactions across VMs
    async fn search_transactions(
        &self,
        ctx: &Context<'_>,
        query: TransactionSearchInput,
    ) -> GraphQLResult<TransactionSearchResult> {
        let _state = ctx.data::<Arc<ApplicationState>>()?;

        // Placeholder implementation
        Ok(TransactionSearchResult {
            transactions: vec![],
            total_count: 0,
            has_next_page: false,
        })
    }
}

#[Object]
impl MutationRoot {
    /// Send SVM transaction
    async fn send_svm_transaction(
        &self,
        ctx: &Context<'_>,
        transaction_data: String,
    ) -> GraphQLResult<TransactionResult> {
        let state = ctx.data::<Arc<ApplicationState>>()?;

        match state.svm_gateway.send_transaction(&transaction_data).await {
            Ok(response) => Ok(TransactionResult {
                success: true,
                transaction_id: response.data,
                error: None,
            }),
            Err(e) => Ok(TransactionResult {
                success: false,
                transaction_id: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    /// Send EVM transaction
    async fn send_evm_transaction(
        &self,
        ctx: &Context<'_>,
        transaction_data: String,
    ) -> GraphQLResult<TransactionResult> {
        let state = ctx.data::<Arc<ApplicationState>>()?;

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
            Err(e) => Ok(TransactionResult {
                success: false,
                transaction_id: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    /// Bind accounts across VMs
    async fn bind_accounts(
        &self,
        ctx: &Context<'_>,
        input: AccountBindingInput,
    ) -> GraphQLResult<AccountBindingResult> {
        let state = ctx.data::<Arc<ApplicationState>>()?;

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
            Err(e) => Ok(AccountBindingResult {
                success: false,
                multivm_account: String::new(),
                binding_id: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

/// Create the GraphQL application router
pub async fn create_app(state: Arc<ApplicationState>) -> ApplicationResult<Router> {
    let config = GraphQLConfig::default();
    let server = GraphQLServer::new(state.clone(), config.clone());

    let mut router = Router::new()
        .route("/", post(graphql_handler))
        .with_state((state, server.schema));

    if config.enable_playground {
        router = router.route("/", get(graphql_playground));
    }

    Ok(router)
}

/// GraphQL request handler
async fn graphql_handler(
    State((state, schema)): State<(Arc<ApplicationState>, MultivmSchema)>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

/// GraphQL Playground handler
async fn graphql_playground() -> Html<String> {
    Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/"),
    ))
}

// GraphQL types

/// System information
#[derive(async_graphql::SimpleObject)]
pub struct SystemInfo {
    /// Application version
    pub version: String,

    /// Application name
    pub name: String,

    /// System uptime in seconds
    pub uptime: u64,

    /// Number of connected VM nodes
    pub node_count: i32,
}

/// SVM account
#[derive(async_graphql::SimpleObject)]
pub struct SvmAccount {
    /// Account address
    pub address: String,

    /// Account balance in lamports
    pub lamports: u64,

    /// Account owner
    pub owner: String,

    /// Whether account is executable
    pub executable: bool,

    /// Rent epoch
    pub rent_epoch: u64,
}

/// EVM account
#[derive(async_graphql::SimpleObject)]
pub struct EvmAccount {
    /// Account address
    pub address: String,

    /// Account balance
    pub balance: String,

    /// Account nonce
    pub nonce: u64,

    /// Code hash
    pub code_hash: String,
}

/// Account bindings
#[derive(async_graphql::SimpleObject)]
pub struct AccountBindings {
    /// MultiVM account ID
    pub multivm_account: String,

    /// Associated SVM account
    pub svm_account: Option<String>,

    /// Associated EVM account
    pub evm_account: Option<String>,

    /// Number of bindings
    pub binding_count: i32,
}

/// Transaction search input
#[derive(async_graphql::InputObject)]
pub struct TransactionSearchInput {
    /// VM type filter
    pub vm_type: Option<String>,

    /// Account address filter
    pub account: Option<String>,

    /// Maximum number of results
    pub limit: Option<i32>,

    /// Result offset
    pub offset: Option<i32>,
}

/// Transaction search result
#[derive(async_graphql::SimpleObject)]
pub struct TransactionSearchResult {
    /// Found transactions
    pub transactions: Vec<Transaction>,

    /// Total count
    pub total_count: i32,

    /// Whether there are more pages
    pub has_next_page: bool,
}

/// Transaction
#[derive(async_graphql::SimpleObject)]
pub struct Transaction {
    /// Transaction ID
    pub id: String,

    /// VM type
    pub vm_type: String,

    /// Transaction status
    pub status: String,

    /// Block number/slot
    pub block: Option<String>,

    /// Timestamp
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Transaction result
#[derive(async_graphql::SimpleObject)]
pub struct TransactionResult {
    /// Whether transaction was successful
    pub success: bool,

    /// Transaction ID
    pub transaction_id: String,

    /// Error message if failed
    pub error: Option<String>,
}

/// Account binding input
#[derive(async_graphql::InputObject)]
pub struct AccountBindingInput {
    /// SVM account to bind
    pub svm_account: Option<String>,

    /// EVM account to bind
    pub evm_account: Option<String>,
}

/// Account binding result
#[derive(async_graphql::SimpleObject)]
pub struct AccountBindingResult {
    /// Whether binding was successful
    pub success: bool,

    /// Generated MultiVM account ID
    pub multivm_account: String,

    /// Binding ID
    pub binding_id: String,

    /// Error message if failed
    pub error: Option<String>,
}
