//! # MultiVM Application Layer
//!
//! The Application Layer (Layer 6) provides comprehensive API interfaces for the MultiVM blockchain system.
//! This layer includes REST API, GraphQL, WebSocket, and administrative interfaces that support both
//! Solana Virtual Machine (SVM) and Ethereum Virtual Machine (EVM) operations within the unified
//! MultiVM architecture.
//!
//! ## Features
//!
//! - **REST API**: Compatible endpoints for both SVM and EVM operations
//! - **GraphQL API**: Flexible query interface with real-time subscriptions
//! - **WebSocket Server**: Real-time data streaming and event notifications
//! - **Admin Interface**: System monitoring and management tools
//! - **Cross-VM Operations**: Native support for cross-VM transactions and account binding
//! - **Authentication & Authorization**: JWT and API key based security
//! - **Rate Limiting**: Configurable rate limiting with multiple storage backends
//! - **Caching**: Multi-level caching with Redis and in-memory support
//! - **Monitoring**: Comprehensive metrics, health checks, and distributed tracing
//!
//! ## Architecture
//!
//! The Application Layer is structured as follows:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Application Layer                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │ REST API │ GraphQL │ WebSocket │ Admin Interface            │
//! ├─────────────────────────────────────────────────────────────┤
//! │ SVM Gateway │ EVM Gateway │ MultiVM Gateway │ Monitor       │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Auth Manager │ Rate Limiter │ Cache Layer │ Validation     │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use multivm_application::{ApplicationConfig, ApplicationServer};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load configuration
//!     let config = ApplicationConfig::from_file("config.toml")?;
//!     
//!     // Create and start the application server
//!     let server = ApplicationServer::new(config).await?;
//!     server.start().await?;
//!     
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

pub mod admin;
pub mod api;
pub mod auth;
pub mod cache;
pub mod config;
pub mod error;
pub mod gateway;
pub mod monitoring;

// Re-export main types
pub use config::{
    AdminServerConfig, ApiKeyValidation, ApplicationConfig, AuthConfig, CacheConfig, CacheStrategy,
    DatabaseConfig, FeatureConfig, GraphQLServerConfig, HealthCheckConfig, MemoryCacheConfig,
    MetricsConfig, MetricsFormat, MonitoringConfig, MultivmClientConfig, PerformanceConfig,
    RateLimitStorage, RateLimitingConfig, RedisConfig, RestServerConfig, RethClientConfig,
    RetryConfig, ServerConfig, SolanaClientConfig, TracingConfig, VmClientConfig,
    WebSocketServerConfig,
};
pub use error::{
    ApiResult, ApplicationError, ApplicationResult, AuthResult, CacheResult, GraphQLResult,
    WebSocketResult,
};

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// The main application server that coordinates all API interfaces
pub struct ApplicationServer {
    config: ApplicationConfig,
    state: Arc<ApplicationState>,
}

/// Shared application state
#[derive(Debug)]
pub struct ApplicationState {
    /// Configuration
    pub config: ApplicationConfig,

    /// Authentication manager
    pub auth_manager: Arc<RwLock<auth::AuthManager>>,

    /// Cache layer
    pub cache: Arc<cache::CacheLayer>,

    /// SVM API gateway
    pub svm_gateway: Arc<gateway::SvmApiGateway>,

    /// EVM API gateway  
    pub evm_gateway: Arc<gateway::EvmApiGateway>,

    /// MultiVM API gateway
    pub multivm_gateway: Arc<gateway::MultivmApiGateway>,

    /// Monitoring services
    pub monitoring: Arc<monitoring::MonitoringService>,

    /// Server status
    pub is_running: Arc<RwLock<bool>>,

    /// Shutdown signal
    pub shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

impl ApplicationServer {
    /// Create a new application server with the given configuration
    pub async fn new(config: ApplicationConfig) -> ApplicationResult<Self> {
        info!("Initializing MultiVM Application Server");

        // Validate configuration
        config.validate()?;
        info!("Configuration validated successfully");

        // Initialize shared state
        let state = Arc::new(ApplicationState::new(config.clone()).await?);

        Ok(Self { config, state })
    }

    /// Start the application server
    pub async fn start(&self) -> ApplicationResult<()> {
        info!("Starting MultiVM Application Server");

        // Mark as running
        {
            let mut is_running = self.state.is_running.write().await;
            *is_running = true;
        }

        // Start monitoring services first
        self.start_monitoring_services().await?;

        // Start API servers concurrently
        let rest_handle = self.start_rest_server();
        let graphql_handle = self.start_graphql_server();
        let websocket_handle = self.start_websocket_server();
        let admin_handle = self.start_admin_server();

        // Wait for any server to complete (which shouldn't happen in normal operation)
        tokio::select! {
            result = rest_handle => {
                error!("REST server stopped: {:?}", result);
                result?
            }
            result = graphql_handle => {
                error!("GraphQL server stopped: {:?}", result);
                result?
            }
            result = websocket_handle => {
                error!("WebSocket server stopped: {:?}", result);
                result?
            }
            result = admin_handle => {
                error!("Admin server stopped: {:?}", result);
                result?
            }
        }

        Ok(())
    }

    /// Stop the application server gracefully
    pub async fn stop(&self) -> ApplicationResult<()> {
        info!("Stopping MultiVM Application Server");

        // Set running flag to false
        {
            let mut is_running = self.state.is_running.write().await;
            *is_running = false;
        }

        // Send shutdown signal
        if let Some(tx) = &self.state.shutdown_tx {
            let _ = tx.send(());
        }

        info!("MultiVM Application Server stopped successfully");
        Ok(())
    }

    /// Check if the server is running
    pub async fn is_running(&self) -> bool {
        *self.state.is_running.read().await
    }

    /// Get server status information
    pub async fn get_status(&self) -> ApplicationResult<ServerStatus> {
        let is_running = self.is_running().await;
        let uptime = if is_running {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default(),
            )
        } else {
            None
        };

        Ok(ServerStatus {
            is_running,
            uptime,
            version: env!("CARGO_PKG_VERSION").to_string(),
            api_servers: ApiServerStatus {
                rest: self.check_server_health("rest").await,
                graphql: self.check_server_health("graphql").await,
                websocket: self.check_server_health("websocket").await,
                admin: self.check_server_health("admin").await,
            },
        })
    }

    // Private helper methods

    async fn start_monitoring_services(&self) -> ApplicationResult<()> {
        info!("Starting monitoring services");

        // Start metrics collection
        if self.config.monitoring.enable_metrics {
            self.state.monitoring.start_metrics_server().await?;
            info!(
                "Metrics server started on port {}",
                self.config.monitoring.metrics.port
            );
        }

        // Start health check endpoint
        self.state.monitoring.start_health_check_server().await?;
        info!(
            "Health check server started on port {}",
            self.config.monitoring.health_check.port
        );

        Ok(())
    }

    async fn start_rest_server(&self) -> ApplicationResult<()> {
        info!(
            "Starting REST API server on {}",
            self.config.rest_socket_addr()?
        );

        let app = api::rest::create_app(self.state.clone()).await?;
        let listener = tokio::net::TcpListener::bind(self.config.rest_socket_addr()?)
            .await
            .map_err(|e| ApplicationError::StartupError {
                service: "rest_api".to_string(),
                message: e.to_string(),
            })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| ApplicationError::InternalError {
                component: "rest_server".to_string(),
                message: e.to_string(),
            })
    }

    async fn start_graphql_server(&self) -> ApplicationResult<()> {
        info!(
            "Starting GraphQL server on {}",
            self.config.graphql_socket_addr()?
        );

        let app = api::graphql::create_app(self.state.clone()).await?;
        let listener = tokio::net::TcpListener::bind(self.config.graphql_socket_addr()?)
            .await
            .map_err(|e| ApplicationError::StartupError {
                service: "graphql".to_string(),
                message: e.to_string(),
            })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| ApplicationError::InternalError {
                component: "graphql_server".to_string(),
                message: e.to_string(),
            })
    }

    async fn start_websocket_server(&self) -> ApplicationResult<()> {
        info!(
            "Starting WebSocket server on {}",
            self.config.websocket_socket_addr()?
        );

        let server = api::websocket::WebSocketServer::new(self.state.clone())?;
        server.start().await
    }

    async fn start_admin_server(&self) -> ApplicationResult<()> {
        if !self.config.server.admin.enable_ui {
            warn!("Admin interface disabled, skipping admin server startup");
            // Keep running but do nothing
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        }

        info!(
            "Starting Admin server on {}",
            self.config.admin_socket_addr()?
        );

        let app = admin::create_app(self.state.clone()).await?;
        let listener = tokio::net::TcpListener::bind(self.config.admin_socket_addr()?)
            .await
            .map_err(|e| ApplicationError::StartupError {
                service: "admin".to_string(),
                message: e.to_string(),
            })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| ApplicationError::InternalError {
                component: "admin_server".to_string(),
                message: e.to_string(),
            })
    }

    async fn check_server_health(&self, _server_name: &str) -> bool {
        // Implementation would check if each server is responding
        // For now, just return true if we're running
        self.is_running().await
    }
}

impl ApplicationState {
    /// Create new application state
    pub async fn new(config: ApplicationConfig) -> ApplicationResult<Self> {
        info!("Initializing application state");

        // Initialize authentication manager
        let auth_manager = Arc::new(RwLock::new(auth::AuthManager::new(&config.auth).await?));

        // Initialize cache layer
        let cache = Arc::new(cache::CacheLayer::new(&config.cache).await?);

        // Initialize API gateways
        let svm_gateway =
            Arc::new(gateway::SvmApiGateway::new(&config.vm_clients.solana, cache.clone()).await?);
        let evm_gateway =
            Arc::new(gateway::EvmApiGateway::new(&config.vm_clients.reth, cache.clone()).await?);
        let multivm_gateway = Arc::new(
            gateway::MultivmApiGateway::new(&config.vm_clients.multivm, cache.clone()).await?,
        );

        // Initialize monitoring
        let monitoring = Arc::new(monitoring::MonitoringService::new(&config.monitoring).await?);

        // Create shutdown channel
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);

        Ok(Self {
            config,
            auth_manager,
            cache,
            svm_gateway,
            evm_gateway,
            multivm_gateway,
            monitoring,
            is_running: Arc::new(RwLock::new(false)),
            shutdown_tx: Some(shutdown_tx),
        })
    }
}

/// Server status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStatus {
    /// Whether the server is currently running
    pub is_running: bool,

    /// Server uptime
    pub uptime: Option<std::time::Duration>,

    /// Server version
    pub version: String,

    /// Status of individual API servers
    pub api_servers: ApiServerStatus,
}

/// Status of individual API servers
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiServerStatus {
    /// REST API server status
    pub rest: bool,

    /// GraphQL server status
    pub graphql: bool,

    /// WebSocket server status
    pub websocket: bool,

    /// Admin interface status
    pub admin: bool,
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_application_state_creation() {
        let mut config = ApplicationConfig::default();
        // Disable Redis for tests
        config.cache.redis.url = String::new();
        let state = ApplicationState::new(config).await;
        assert!(state.is_ok());
    }

    #[tokio::test]
    async fn test_server_creation() {
        let mut config = ApplicationConfig::default();
        // Disable Redis for tests
        config.cache.redis.url = String::new();
        let server = ApplicationServer::new(config).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_status() {
        let mut config = ApplicationConfig::default();
        // Disable Redis for tests
        config.cache.redis.url = String::new();
        let server = ApplicationServer::new(config).await.unwrap();
        let status = server.get_status().await.unwrap();

        assert!(!status.is_running); // Should not be running initially
        assert_eq!(status.version, VERSION);
    }
}
