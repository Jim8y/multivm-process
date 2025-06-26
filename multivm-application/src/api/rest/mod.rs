//! # REST API Module
//!
//! Provides RESTful HTTP endpoints for interacting with the MultiVM blockchain.
//! Supports both SVM and EVM operations through a unified interface.

pub mod handlers;
pub mod middleware;

use crate::{ApplicationResult, ApplicationState};
use axum::{extract::DefaultBodyLimit, http::Method, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub use handlers::*;
pub use middleware::*;

/// REST API server configuration
#[derive(Debug, Clone)]
pub struct RestApiConfig {
    /// Maximum request body size in bytes
    pub max_body_size: usize,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Enable request compression
    pub enable_compression: bool,

    /// CORS configuration
    pub cors: CorsConfig,
}

/// CORS configuration
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    pub allowed_methods: Vec<Method>,

    /// Allowed headers
    pub allowed_headers: Vec<String>,
}

impl Default for RestApiConfig {
    fn default() -> Self {
        Self {
            max_body_size: 16 * 1024 * 1024, // 16MB
            timeout_seconds: 30,
            enable_compression: true,
            cors: CorsConfig {
                allowed_origins: vec!["*".to_string()],
                allowed_methods: vec![
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ],
                allowed_headers: vec![
                    "content-type".to_string(),
                    "authorization".to_string(),
                    "x-api-key".to_string(),
                    "x-request-id".to_string(),
                ],
            },
        }
    }
}

/// REST API server
pub struct RestApiServer {
    state: Arc<ApplicationState>,
    config: RestApiConfig,
}

impl RestApiServer {
    /// Create a new REST API server
    pub fn new(state: Arc<ApplicationState>, config: RestApiConfig) -> Self {
        Self { state, config }
    }

    /// Create the Axum application router
    pub fn create_router(&self) -> Router {
        create_app_with_config(self.state.clone(), self.config.clone())
    }
}

/// Create the main Axum application with all routes and middleware
pub async fn create_app(state: Arc<ApplicationState>) -> ApplicationResult<Router> {
    let config = RestApiConfig::default();
    Ok(create_app_with_config(state, config))
}

/// Create the Axum application with custom configuration
pub fn create_app_with_config(state: Arc<ApplicationState>, config: RestApiConfig) -> Router {
    let _cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check endpoint
        .route("/health", axum::routing::get(handlers::health_check))
        // API versioning
        .nest("/api/v1", create_v1_routes(state.clone()))
        // Apply middleware stack - minimal for axum 0.8 compatibility
        .layer(DefaultBodyLimit::max(config.max_body_size))
        // Add application state
        .with_state(state)
}

/// Create v1 API routes
fn create_v1_routes(_state: Arc<ApplicationState>) -> Router<Arc<ApplicationState>> {
    Router::new()
        // SVM (Solana) endpoints
        .nest("/svm", create_svm_routes())
        // EVM (Ethereum) endpoints
        .nest("/evm", create_evm_routes())
        // MultiVM specific endpoints
        .nest("/multivm", create_multivm_routes())
        // Account management
        .nest("/accounts", create_account_routes())
        // Transaction operations
        .nest("/transactions", create_transaction_routes())
        // Block operations
        .nest("/blocks", create_block_routes())
        // System information
        .nest("/system", create_system_routes())
}

/// Create SVM-specific routes
fn create_svm_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        // Account operations
        .route(
            "/accounts/:address",
            axum::routing::get(handlers::svm::get_account),
        )
        .route(
            "/accounts/:address/balance",
            axum::routing::get(handlers::svm::get_balance),
        )
        .route(
            "/accounts/:address/transactions",
            axum::routing::get(handlers::svm::get_account_transactions),
        )
        // Transaction operations
        .route(
            "/transactions",
            axum::routing::post(handlers::svm::send_transaction),
        )
        .route(
            "/transactions/:signature",
            axum::routing::get(handlers::svm::get_transaction),
        )
        .route(
            "/transactions/simulate",
            axum::routing::post(handlers::svm::simulate_transaction),
        )
        // Block operations
        .route(
            "/blocks/latest",
            axum::routing::get(handlers::svm::get_latest_block),
        )
        .route(
            "/blocks/:slot",
            axum::routing::get(handlers::svm::get_block),
        )
        .route(
            "/blocks/:slot/transactions",
            axum::routing::get(handlers::svm::get_block_transactions),
        )
        // Program operations
        .route(
            "/programs/:program_id/accounts",
            axum::routing::get(handlers::svm::get_program_accounts),
        )
        // Token operations
        .route(
            "/tokens/:mint/accounts",
            axum::routing::get(handlers::svm::get_token_accounts),
        )
        .route(
            "/tokens/:mint/supply",
            axum::routing::get(handlers::svm::get_token_supply),
        )
}

/// Create EVM-specific routes
fn create_evm_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        // Account operations
        .route(
            "/accounts/:address",
            axum::routing::get(handlers::evm::get_account),
        )
        .route(
            "/accounts/:address/balance",
            axum::routing::get(handlers::evm::get_balance),
        )
        .route(
            "/accounts/:address/nonce",
            axum::routing::get(handlers::evm::get_nonce),
        )
        .route(
            "/accounts/:address/code",
            axum::routing::get(handlers::evm::get_code),
        )
        .route(
            "/accounts/:address/transactions",
            axum::routing::get(handlers::evm::get_account_transactions),
        )
        // Transaction operations
        .route(
            "/transactions",
            axum::routing::post(handlers::evm::send_transaction),
        )
        .route(
            "/transactions/:hash",
            axum::routing::get(handlers::evm::get_transaction),
        )
        .route(
            "/transactions/:hash/receipt",
            axum::routing::get(handlers::evm::get_transaction_receipt),
        )
        .route(
            "/transactions/estimate-gas",
            axum::routing::post(handlers::evm::estimate_gas),
        )
        // Block operations
        .route(
            "/blocks/latest",
            axum::routing::get(handlers::evm::get_latest_block),
        )
        .route(
            "/blocks/:block_id",
            axum::routing::get(handlers::evm::get_block),
        )
        .route(
            "/blocks/:block_id/transactions",
            axum::routing::get(handlers::evm::get_block_transactions),
        )
        // Contract operations
        .route(
            "/contracts/:address/call",
            axum::routing::post(handlers::evm::call_contract),
        )
        // Log operations
        .route("/logs", axum::routing::post(handlers::evm::get_logs))
}

/// Create MultiVM-specific routes
fn create_multivm_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        // Cross-VM account binding
        .route(
            "/accounts/bind",
            axum::routing::post(handlers::multivm::bind_accounts),
        )
        .route(
            "/accounts/:address/bindings",
            axum::routing::get(handlers::multivm::get_account_bindings),
        )
        .route(
            "/accounts/unbind",
            axum::routing::post(handlers::multivm::unbind_accounts),
        )
        // Cross-VM transactions
        .route(
            "/transactions/cross-vm",
            axum::routing::post(handlers::multivm::send_cross_vm_transaction),
        )
        .route(
            "/transactions/:id/status",
            axum::routing::get(handlers::multivm::get_cross_vm_transaction_status),
        )
        // MultiVM blocks
        .route(
            "/blocks/latest",
            axum::routing::get(handlers::multivm::get_latest_multivm_block),
        )
        .route(
            "/blocks/:block_id",
            axum::routing::get(handlers::multivm::get_multivm_block),
        )
        // System state
        .route(
            "/state/summary",
            axum::routing::get(handlers::multivm::get_system_state),
        )
}

/// Create account management routes
fn create_account_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        .route("/", axum::routing::get(handlers::accounts::list_accounts))
        .route(
            "/:address",
            axum::routing::get(handlers::accounts::get_account_info),
        )
        .route(
            "/:address/history",
            axum::routing::get(handlers::accounts::get_account_history),
        )
        .route(
            "/search",
            axum::routing::post(handlers::accounts::search_accounts),
        )
}

/// Create transaction management routes
fn create_transaction_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        .route(
            "/",
            axum::routing::get(handlers::transactions::list_transactions),
        )
        .route(
            "/:id",
            axum::routing::get(handlers::transactions::get_transaction_details),
        )
        .route(
            "/search",
            axum::routing::post(handlers::transactions::search_transactions),
        )
        .route(
            "/pending",
            axum::routing::get(handlers::transactions::get_pending_transactions),
        )
}

/// Create block management routes
fn create_block_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        .route("/", axum::routing::get(handlers::blocks::list_blocks))
        .route(
            "/:id",
            axum::routing::get(handlers::blocks::get_block_details),
        )
        .route(
            "/search",
            axum::routing::post(handlers::blocks::search_blocks),
        )
        .route(
            "/stats",
            axum::routing::get(handlers::blocks::get_block_stats),
        )
}

/// Create system information routes
fn create_system_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        .route(
            "/status",
            axum::routing::get(handlers::system::get_system_status),
        )
        .route(
            "/info",
            axum::routing::get(handlers::system::get_system_info),
        )
        .route(
            "/metrics",
            axum::routing::get(handlers::system::get_system_metrics),
        )
        .route(
            "/health",
            axum::routing::get(handlers::system::get_health_status),
        )
}
