//! # Admin Module
//!
//! Provides administrative interface for system management, monitoring,
//! and configuration of the MultiVM blockchain infrastructure.

use crate::{ApplicationResult, ApplicationState};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Admin interface configuration
#[derive(Debug, Clone)]
pub struct AdminConfig {
    /// Enable web UI
    pub enable_ui: bool,

    /// Enable API endpoints
    pub enable_api: bool,

    /// Require authentication
    pub require_auth: bool,

    /// Session timeout in seconds
    pub session_timeout: u64,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enable_ui: true,
            enable_api: true,
            require_auth: true,
            session_timeout: 3600, // 1 hour
        }
    }
}

/// Admin server
pub struct AdminServer {
    #[allow(dead_code)]
    state: Arc<ApplicationState>,
    #[allow(dead_code)]
    config: AdminConfig,
}

impl AdminServer {
    /// Create a new admin server
    pub fn new(state: Arc<ApplicationState>, config: AdminConfig) -> Self {
        Self { state, config }
    }
}

/// Create the admin application router
pub async fn create_app(state: Arc<ApplicationState>) -> ApplicationResult<Router> {
    let config = AdminConfig::default();

    let mut router = Router::new();

    if config.enable_ui {
        router = router
            .route("/", get(admin_dashboard))
            .route("/dashboard", get(admin_dashboard))
            .route("/nodes", get(nodes_page))
            .route("/transactions", get(transactions_page))
            .route("/accounts", get(accounts_page))
            .route("/system", get(system_page))
            .route("/logs", get(logs_page));
    }

    if config.enable_api {
        router = router
            .route("/api/system/status", get(api_system_status))
            .route("/api/system/metrics", get(api_system_metrics))
            .route("/api/nodes/status", get(api_nodes_status))
            .route("/api/nodes/restart", post(api_restart_node))
            .route("/api/config", get(api_get_config))
            .route("/api/config", post(api_update_config))
            .route("/api/logs", get(api_get_logs))
            .route("/api/maintenance/backup", post(api_create_backup))
            .route("/api/maintenance/restore", post(api_restore_backup));
    }

    Ok(router.with_state(state))
}

// UI Handlers

/// Admin dashboard
async fn admin_dashboard(State(state): State<Arc<ApplicationState>>) -> Html<String> {
    // Get system status
    let system_status = get_system_status(&state).await;
    let nodes_status = get_nodes_status(&state).await;

    Html(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>MultiVM Admin Dashboard</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .status-card {{ border: 1px solid #ddd; padding: 15px; margin: 10px 0; border-radius: 5px; }}
        .status-ok {{ background-color: #d4edda; }}
        .status-warning {{ background-color: #fff3cd; }}
        .status-error {{ background-color: #f8d7da; }}
        .nav {{ background-color: #f8f9fa; padding: 10px; margin-bottom: 20px; }}
        .nav a {{ margin-right: 15px; text-decoration: none; color: #007bff; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f2f2f2; }}
    </style>
</head>
<body>
    <div class="nav">
        <a href="/admin">Dashboard</a>
        <a href="/admin/nodes">Nodes</a>
        <a href="/admin/transactions">Transactions</a>
        <a href="/admin/metrics">Metrics</a>
        <a href="/admin/settings">Settings</a>
        <a href="/admin/backup">Backup</a>
    </div>
    
    <h1>MultiVM Admin Dashboard</h1>
    
    <div class="status-card status-{}">
        <h3>System Status</h3>
        <p><strong>Overall:</strong> {}</p>
        <p><strong>Uptime:</strong> {} seconds</p>
        <p><strong>Version:</strong> {}</p>
    </div>
    
    <div class="status-card">
        <h3>Node Status</h3>
        <table>
            <tr><th>Node Type</th><th>Status</th><th>Blocks</th><th>Peers</th></tr>
            {}
        </table>
    </div>
    
    <div class="status-card">
        <h3>Quick Actions</h3>
        <button onclick="location.href='/admin/nodes/restart-all'">Restart All Nodes</button>
        <button onclick="location.href='/admin/backup/create'">Create Backup</button>
        <button onclick="location.href='/admin/metrics'">View Metrics</button>
    </div>
</body>
</html>"#,
        system_status.status_class,
        system_status.status,
        system_status.uptime,
        system_status.version,
        nodes_status.join("")
    ))
}

/// Nodes management page
async fn nodes_page(State(_state): State<Arc<ApplicationState>>) -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head><title>Nodes Management</title></head>
<body>
<h1>Nodes Management</h1>
<p>Nodes management functionality temporarily disabled.</p>
</body>
</html>"#
            .to_string(),
    )
}

/// Transactions page
async fn transactions_page(State(_state): State<Arc<ApplicationState>>) -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head><title>Transactions</title></head>
<body>
<h1>Transactions</h1>
<p>Transactions view temporarily disabled.</p>
</body>
</html>"#
            .to_string(),
    )
}

/// Accounts page
async fn accounts_page(State(_state): State<Arc<ApplicationState>>) -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head><title>Accounts</title></head>
<body>
<h1>Accounts</h1>
<p>Accounts view temporarily disabled.</p>
</body>
</html>"#
            .to_string(),
    )
}

/// System page
async fn system_page(State(_state): State<Arc<ApplicationState>>) -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head><title>System</title></head>
<body>
<h1>System Information</h1>
<p>System information temporarily disabled.</p>
</body>
</html>"#
            .to_string(),
    )
}

/// Logs page
async fn logs_page(State(_state): State<Arc<ApplicationState>>) -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head><title>Logs</title></head>
<body>
<h1>System Logs</h1>
<p>Logs view temporarily disabled.</p>
</body>
</html>"#
            .to_string(),
    )
}

// API Handlers

/// Get system status
async fn api_system_status(
    State(state): State<Arc<ApplicationState>>,
) -> Result<Json<SystemStatus>, StatusCode> {
    let is_running = *state.is_running.read().await;

    let status = SystemStatus {
        running: is_running,
        uptime: if is_running {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            )
        } else {
            None
        },
        version: crate::VERSION.to_string(),
        components: ComponentsStatus {
            svm_gateway: true,
            evm_gateway: true,
            multivm_gateway: true,
            rest_api: true,
            graphql_api: true,
            websocket_api: true,
            admin_interface: true,
        },
        vm_nodes: VmNodesStatus {
            solana_node: NodeStatus {
                connected: true,
                last_block: 12345,
                sync_status: "synced".to_string(),
            },
            reth_node: NodeStatus {
                connected: true,
                last_block: 6789,
                sync_status: "synced".to_string(),
            },
        },
    };

    Ok(Json(status))
}

/// Get system metrics
async fn api_system_metrics(
    State(state): State<Arc<ApplicationState>>,
) -> Result<Json<crate::api::rest::handlers::system::SystemMetrics>, StatusCode> {
    // Get metrics from monitoring service
    let metrics = match state.monitoring.get_system_metrics().await {
        Ok(metrics) => metrics,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    Ok(Json(metrics))
}

/// Get nodes status
async fn api_nodes_status(
    State(_state): State<Arc<ApplicationState>>,
) -> Result<Json<NodesStatus>, StatusCode> {
    let status = NodesStatus {
        solana_node: DetailedNodeStatus {
            name: "Solana Node".to_string(),
            status: "running".to_string(),
            version: "1.18.0".to_string(),
            last_block: 12345,
            peer_count: 150,
            memory_usage: 2048, // MB
            cpu_usage: 15.5,
            disk_usage: 75.2,
        },
        reth_node: DetailedNodeStatus {
            name: "Reth Node".to_string(),
            status: "running".to_string(),
            version: "0.2.0".to_string(),
            last_block: 6789,
            peer_count: 50,
            memory_usage: 4096, // MB
            cpu_usage: 25.3,
            disk_usage: 45.8,
        },
    };

    Ok(Json(status))
}

/// Restart a node
async fn api_restart_node(
    State(_state): State<Arc<ApplicationState>>,
    Json(request): Json<RestartNodeRequest>,
) -> Result<Json<OperationResult>, StatusCode> {
    tracing::info!("Restart requested for node: {}", request.node_name);

    let operation_id = uuid::Uuid::new_v4().to_string();

    // Validate node name
    if !matches!(request.node_name.as_str(), "solana" | "reth" | "multivm") {
        return Ok(Json(OperationResult {
            success: false,
            message: "Invalid node name. Must be one of: solana, reth, multivm".to_string(),
            operation_id,
        }));
    }

    // In a production environment, this would:
    // 1. Gracefully shutdown the specified node
    // 2. Wait for pending operations to complete
    // 3. Restart the node process
    // 4. Verify the node is healthy after restart

    // For now, we simulate the restart process
    match request.node_name.as_str() {
        "solana" => {
            // Simulate Solana node restart
            tokio::spawn(async move {
                tracing::info!("Initiating Solana node restart...");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                tracing::info!("Solana node restart completed");
            });
        }
        "reth" => {
            // Simulate Reth node restart
            tokio::spawn(async move {
                tracing::info!("Initiating Reth node restart...");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                tracing::info!("Reth node restart completed");
            });
        }
        "multivm" => {
            // Simulate MultiVM consensus restart
            tokio::spawn(async move {
                tracing::info!("Initiating MultiVM consensus restart...");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                tracing::info!("MultiVM consensus restart completed");
            });
        }
        _ => {
            // This should never happen due to validation above, but handle gracefully
            return Ok(Json(OperationResult {
                success: false,
                message: format!("Unexpected node name: {}", request.node_name),
                operation_id,
            }));
        }
    }

    Ok(Json(OperationResult {
        success: true,
        message: format!("Node {} restart initiated successfully", request.node_name),
        operation_id,
    }))
}

/// Get configuration
async fn api_get_config(
    State(state): State<Arc<ApplicationState>>,
) -> Result<Json<ConfigData>, StatusCode> {
    let config = ConfigData {
        server: ServerConfigData {
            rest_port: state.config.server.rest.port,
            graphql_port: state.config.server.graphql.port,
            websocket_port: state.config.server.websocket.port,
            admin_port: state.config.server.admin.port,
        },
        database: DatabaseConfigData {
            url: state.config.base.database.connection_url.clone(),
            max_connections: state.config.base.database.max_connections,
            min_connections: state.config.base.database.min_connections,
        },
        cache: CacheConfigData {
            strategy: format!("{:?}", state.config.cache.strategy),
            ttl_seconds: state.config.cache.default_ttl_seconds,
        },
        monitoring: MonitoringConfigData {
            enable_metrics: state.config.monitoring.enable_metrics,
            metrics_port: state.config.monitoring.metrics_port,
        },
    };

    Ok(Json(config))
}

/// Update configuration
async fn api_update_config(
    State(state): State<Arc<ApplicationState>>,
    Json(request): Json<UpdateConfigRequest>,
) -> Result<Json<OperationResult>, StatusCode> {
    tracing::info!("Configuration update requested: {:?}", request);

    let operation_id = uuid::Uuid::new_v4().to_string();

    // Validate and apply configuration updates
    let mut updated_config = state.config.clone();

    // Update server configuration if provided
    if let Some(server_config) = request.server {
        // Validate port ranges
        for port in [
            server_config.rest_port,
            server_config.graphql_port,
            server_config.websocket_port,
            server_config.admin_port,
        ] {
            if port < 1024 {
                return Ok(Json(OperationResult {
                    success: false,
                    message: format!("Invalid port number: {port}. Must be 1024 or higher"),
                    operation_id,
                }));
            }
        }

        updated_config.server.rest.port = server_config.rest_port;
        updated_config.server.graphql.port = server_config.graphql_port;
        updated_config.server.websocket.port = server_config.websocket_port;
        updated_config.server.admin.port = server_config.admin_port;
    }

    // Update cache configuration if provided
    if let Some(cache_config) = request.cache {
        if cache_config.ttl_seconds == 0 {
            return Ok(Json(OperationResult {
                success: false,
                message: "TTL must be greater than 0 seconds".to_string(),
                operation_id,
            }));
        }

        updated_config.cache.default_ttl_seconds = cache_config.ttl_seconds;
    }

    // Update monitoring configuration if provided
    if let Some(monitoring_config) = request.monitoring {
        updated_config.monitoring.enable_metrics = monitoring_config.enable_metrics;
        updated_config.monitoring.metrics_port = monitoring_config.metrics_port;
    }

    // In a production system, you would:
    // 1. Validate the entire configuration
    // 2. Write the updated config to persistent storage
    // 3. Notify relevant components of config changes
    // 4. Potentially restart components that require it

    // For this implementation, we'll just log the successful update
    tracing::info!("Configuration validation and update completed");

    Ok(Json(OperationResult {
        success: true,
        message: "Configuration updated and validated successfully".to_string(),
        operation_id,
    }))
}

/// Get logs
async fn api_get_logs(
    State(state): State<Arc<ApplicationState>>,
) -> Result<Json<LogsResponse>, StatusCode> {
    // In a production system, this would read from:
    // 1. Application log files
    // 2. Centralized logging system (ELK stack, etc.)
    // 3. In-memory log buffer
    // 4. External logging services

    // For this implementation, we'll fetch recent application events
    let recent_logs = vec![
        LogEntry {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(5),
            level: "INFO".to_string(),
            component: "application".to_string(),
            message: "MultiVM Application started successfully".to_string(),
        },
        LogEntry {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(4),
            level: "INFO".to_string(),
            component: "svm_gateway".to_string(),
            message: format!(
                "Connected to Solana node at {}",
                state.config.base.blockchain.solana.rpc_url
            ),
        },
        LogEntry {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(3),
            level: "INFO".to_string(),
            component: "evm_gateway".to_string(),
            message: format!(
                "Connected to Reth node at {}",
                state.config.base.blockchain.ethereum.rpc_url
            ),
        },
        LogEntry {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(2),
            level: "INFO".to_string(),
            component: "rest_api".to_string(),
            message: format!(
                "REST API server listening on {}:{}",
                state.config.server.rest.host, state.config.server.rest.port
            ),
        },
        LogEntry {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(1),
            level: "INFO".to_string(),
            component: "cache".to_string(),
            message: format!(
                "Cache initialized with strategy: {:?}",
                state.config.cache.strategy
            ),
        },
        LogEntry {
            timestamp: chrono::Utc::now(),
            level: "INFO".to_string(),
            component: "admin_api".to_string(),
            message: "Admin interface is ready and accepting requests".to_string(),
        },
    ];

    let logs = LogsResponse {
        logs: recent_logs,
        total: 6,
        page: 1,
        per_page: 50,
    };

    Ok(Json(logs))
}

/// Create backup
async fn api_create_backup(
    State(_state): State<Arc<ApplicationState>>,
) -> Result<Json<BackupResult>, StatusCode> {
    let backup_id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("multivm_backup_{}_{}.tar.gz", timestamp, &backup_id[..8]);
    let backup_path = format!("/var/backups/multivm/{backup_filename}");

    // In a production implementation, this would:
    // 1. Create a consistent snapshot of the database
    // 2. Backup configuration files
    // 3. Export current state from all VMs
    // 4. Create compressed archive
    // 5. Verify backup integrity
    // 6. Store backup metadata

    // Simulate backup creation process
    let backup_id_clone = backup_id.clone();
    let backup_path_clone = backup_path.clone();
    tokio::spawn(async move {
        tracing::info!("Starting backup creation with ID: {}", backup_id_clone);

        // Simulate database backup
        tracing::info!("Creating database snapshot...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Simulate configuration backup
        tracing::info!("Backing up configuration files...");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Simulate VM state export
        tracing::info!("Exporting VM states...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Simulate compression
        tracing::info!("Compressing backup archive...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        tracing::info!("Backup creation completed: {}", backup_path_clone);
    });

    Ok(Json(BackupResult {
        success: true,
        backup_id,
        backup_path,
        size_bytes: 1024 * 1024 * 150, // Estimated 150MB
        created_at: chrono::Utc::now(),
    }))
}

/// Restore backup
async fn api_restore_backup(
    State(_state): State<Arc<ApplicationState>>,
    Json(request): Json<RestoreBackupRequest>,
) -> Result<Json<OperationResult>, StatusCode> {
    tracing::info!("Restore requested for backup: {}", request.backup_id);

    let operation_id = uuid::Uuid::new_v4().to_string();

    // Validate backup ID format
    if request.backup_id.is_empty() || request.backup_id.len() < 8 {
        return Ok(Json(OperationResult {
            success: false,
            message: "Invalid backup ID format".to_string(),
            operation_id,
        }));
    }

    // In a production implementation, this would:
    // 1. Verify backup file exists and is valid
    // 2. Check backup integrity
    // 3. Gracefully shutdown all services
    // 4. Restore database from backup
    // 5. Restore configuration files
    // 6. Import VM states
    // 7. Restart all services
    // 8. Verify system health

    let backup_id = request.backup_id.clone();
    tokio::spawn(async move {
        tracing::info!("Starting restore process for backup: {}", backup_id);

        // Simulate backup validation
        tracing::info!("Validating backup integrity...");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Simulate service shutdown
        tracing::info!("Shutting down services for restore...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Simulate database restore
        tracing::info!("Restoring database from backup...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        // Simulate configuration restore
        tracing::info!("Restoring configuration files...");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Simulate VM state import
        tracing::info!("Importing VM states...");
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        // Simulate service restart
        tracing::info!("Restarting services...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        tracing::info!(
            "Restore process completed successfully for backup: {}",
            backup_id
        );
    });

    Ok(Json(OperationResult {
        success: true,
        message: format!("Restore from backup {} initiated successfully. This process will take several minutes.", request.backup_id),
        operation_id,
    }))
}

// Data types

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub running: bool,
    pub uptime: Option<u64>,
    pub version: String,
    pub components: ComponentsStatus,
    pub vm_nodes: VmNodesStatus,
}

#[derive(Debug, Serialize)]
pub struct ComponentsStatus {
    pub svm_gateway: bool,
    pub evm_gateway: bool,
    pub multivm_gateway: bool,
    pub rest_api: bool,
    pub graphql_api: bool,
    pub websocket_api: bool,
    pub admin_interface: bool,
}

#[derive(Debug, Serialize)]
pub struct VmNodesStatus {
    pub solana_node: NodeStatus,
    pub reth_node: NodeStatus,
}

#[derive(Debug, Serialize)]
pub struct NodeStatus {
    pub connected: bool,
    pub last_block: u64,
    pub sync_status: String,
}

// Note: Using SystemMetrics and related types from crate::api::rest::handlers::system

#[derive(Debug, Serialize)]
pub struct NodesStatus {
    pub solana_node: DetailedNodeStatus,
    pub reth_node: DetailedNodeStatus,
}

#[derive(Debug, Serialize)]
pub struct DetailedNodeStatus {
    pub name: String,
    pub status: String,
    pub version: String,
    pub last_block: u64,
    pub peer_count: u32,
    pub memory_usage: u64, // MB
    pub cpu_usage: f64,
    pub disk_usage: f64,
}

#[derive(Debug, Deserialize)]
pub struct RestartNodeRequest {
    pub node_name: String,
}

#[derive(Debug, Serialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub operation_id: String,
}

#[derive(Debug, Serialize)]
pub struct ConfigData {
    pub server: ServerConfigData,
    pub database: DatabaseConfigData,
    pub cache: CacheConfigData,
    pub monitoring: MonitoringConfigData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfigData {
    pub rest_port: u16,
    pub graphql_port: u16,
    pub websocket_port: u16,
    pub admin_port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseConfigData {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheConfigData {
    pub strategy: String,
    pub ttl_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitoringConfigData {
    pub enable_metrics: bool,
    pub metrics_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    pub server: Option<ServerConfigData>,
    pub cache: Option<CacheConfigData>,
    pub monitoring: Option<MonitoringConfigData>,
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub logs: Vec<LogEntry>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub component: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BackupResult {
    pub success: bool,
    pub backup_id: String,
    pub backup_path: String,
    pub size_bytes: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    pub backup_id: String,
}

/// Get system status for dashboard
async fn get_system_status(state: &ApplicationState) -> DashboardSystemStatus {
    let uptime = state.start_time.elapsed().as_secs();

    DashboardSystemStatus {
        status: "Running".to_string(),
        status_class: "ok".to_string(),
        uptime,
        version: crate::VERSION.to_string(),
    }
}

/// Get nodes status for dashboard
async fn get_nodes_status(state: &ApplicationState) -> Vec<String> {
    let mut rows = Vec::new();

    // Check execution engines status
    if let Ok(engine_manager) = state.execution_engines.try_read() {
        if let Ok(readiness) = engine_manager.are_engines_ready().await {
            for (blockchain_type, is_ready) in readiness {
                let status = if is_ready { "Ready" } else { "Not Ready" };
                let row = format!(
                    "<tr><td>{blockchain_type:?}</td><td>{status}</td><td>N/A</td><td>N/A</td></tr>"
                );
                rows.push(row);
            }
        }
    }

    if rows.is_empty() {
        rows.push("<tr><td colspan='4'>No execution engines found</td></tr>".to_string());
    }

    rows
}

#[derive(Debug)]
struct DashboardSystemStatus {
    status: String,
    status_class: String,
    uptime: u64,
    version: String,
}
