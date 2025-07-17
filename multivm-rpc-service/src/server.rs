//! Main RPC server implementation

use crate::{
    cache::RpcCache,
    config::RpcServiceConfig,
    error::{RpcError, RpcResult},
    health::{HealthChecker, endpoints},
    logging::{LoggingConfig, request_tracing},
    middleware::{auth_middleware, cors_middleware, request_metadata_middleware, RateLimitMiddleware, MetricsMiddleware},
    multivm_api::MultiVmApi,
    observability::{MetricsCollector, TracingCollector, AlertManager},
    performance::{ConnectionPool, AdaptiveTimeout, PerformanceMonitor, RequestBatcher},
    relay::RelayManager,
    rpc_router::RpcRouter,
    types::{JsonRpcRequest, JsonRpcResponse, RequestMetadata, VmType},
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc, time::SystemTime};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::{debug, error, info, warn};

/// Main RPC server
pub struct RpcServer {
    config: RpcServiceConfig,
    relay_manager: Arc<RelayManager>,
    cache: Arc<RpcCache>,
    router: Arc<RpcRouter>,
    multivm_api: Arc<MultiVmApi>,
    rate_limiter: Arc<RateLimitMiddleware>,
    metrics: Arc<MetricsMiddleware>,
    health_checker: Arc<HealthChecker>,
    connection_pool: Arc<ConnectionPool>,
    adaptive_timeout: Arc<AdaptiveTimeout>,
    performance_monitor: Arc<PerformanceMonitor>,
    request_batcher: Arc<RequestBatcher>,
    metrics_collector: Arc<MetricsCollector>,
    tracing_collector: Arc<TracingCollector>,
    alert_manager: Arc<AlertManager>,
}

impl RpcServer {
    /// Create new RPC server
    pub async fn new(config: RpcServiceConfig) -> RpcResult<Self> {
        // Validate configuration
        config.validate().map_err(|e| RpcError::ProxyError {
            message: format!("Invalid configuration: {}", e),
        })?;

        // Create relay manager
        let relay_manager = Arc::new(
            crate::relay::utils::create_relay_manager(&config.backends).await?
        );

        // Create cache
        let cache = Arc::new(RpcCache::with_method_ttls(
            config.cache.max_size,
            std::time::Duration::from_secs(300),
            config.cache.method_ttl.clone(),
        ));

        // Create router
        let router = Arc::new(RpcRouter::new());

        // Create MultiVM API
        let multivm_api = Arc::new(MultiVmApi::new());

        // Create rate limiter
        let rate_limiter = Arc::new(RateLimitMiddleware::new(config.rate_limit.clone()));

        // Create metrics
        let metrics = Arc::new(MetricsMiddleware::new());

        // Initialize logging
        config.logging.clone().init().map_err(|e| RpcError::ProxyError {
            message: format!("Failed to initialize logging: {}", e),
        })?;

        // Create health checker
        let health_checker = Arc::new(HealthChecker::new(config.health.clone()));

        // Create performance components
        let connection_pool = Arc::new(ConnectionPool::new(config.performance.clone()));
        let adaptive_timeout = Arc::new(AdaptiveTimeout::new(config.performance.clone()));
        let performance_monitor = Arc::new(PerformanceMonitor::new());
        let request_batcher = Arc::new(RequestBatcher::new(config.performance.clone()));

        // Create observability components
        let metrics_collector = Arc::new(MetricsCollector::new(config.observability.clone())?);
        let tracing_collector = Arc::new(TracingCollector::new(config.observability.clone()));
        let alert_manager = Arc::new(AlertManager::new(config.observability.clone()));

        Ok(Self {
            config,
            relay_manager,
            cache,
            router,
            multivm_api,
            rate_limiter,
            metrics,
            health_checker,
            connection_pool,
            adaptive_timeout,
            performance_monitor,
            request_batcher,
            metrics_collector,
            tracing_collector,
            alert_manager,
        })
    }

    /// Start the RPC server
    pub async fn start(self) -> RpcResult<()> {
        let bind_address = self.config.server.bind_address;
        
        info!("Starting MultiVM RPC server on {}", bind_address);

        // Start periodic health checks
        let _health_check_handle = self.health_checker.start_periodic_checks();

        // Create the router
        let app = self.create_router().await?;

        // Create listener
        let listener = TcpListener::bind(bind_address)
            .await
            .map_err(|e| RpcError::Internal {
                message: format!("Failed to bind to {}: {}", bind_address, e),
            })?;

        info!("MultiVM RPC server listening on {}", bind_address);

        // Start server
        axum::serve(listener, app)
            .await
            .map_err(|e| RpcError::Internal {
                message: format!("Server error: {}", e),
            })?;

        Ok(())
    }

    /// Create Axum router with all routes and middleware
    async fn create_router(self) -> RpcResult<Router> {
        let server = Arc::new(self);

        let app = Router::new()
            // JSON-RPC endpoint
            .route("/", post(json_rpc_handler))
            .route("/rpc", post(json_rpc_handler))
            .route("/v1/rpc", post(json_rpc_handler))
            // Health check endpoints
            .route("/health", get(health_handler))
            .route("/health/live", get(endpoints::liveness))
            .route("/health/ready", get(|State(server): State<Arc<RpcServer>>| async move {
                endpoints::readiness(State(server.health_checker.clone())).await
            }))
            .route("/health/detailed", get(|State(server): State<Arc<RpcServer>>| async move {
                endpoints::health_detailed(State(server.health_checker.clone())).await
            }))
            // Metrics endpoint
            .route("/metrics", get(metrics_handler))
            // Performance endpoint
            .route("/performance", get(performance_handler))
            // Add server state
            .with_state(server.clone())
            // Add middleware layers
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CompressionLayer::new())
                    .layer(TimeoutLayer::new(server.config.server.request_timeout))
                    .layer(middleware::from_fn_with_state(
                        Arc::new(server.config.auth.clone()),
                        auth_middleware,
                    ))
                    .layer(middleware::from_fn(request_metadata_middleware))
                    .layer(middleware::from_fn(cors_middleware))
                    .layer(
                        CorsLayer::new()
                            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                            .allow_headers(Any)
                            .allow_origin(Any),
                    ),
            );

        Ok(app)
    }
}

/// JSON-RPC request handler
async fn json_rpc_handler(
    State(server): State<Arc<RpcServer>>,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    let start_time = SystemTime::now();
    let metadata = RequestMetadata {
        request_id: uuid::Uuid::new_v4().to_string(),
        client_id: "unknown".to_string(),
        vm_type: VmType::MultiVm,
        timestamp: start_time,
        auth_info: None,
        headers: std::collections::HashMap::new(),
    };

    // Process the request
    let response = match process_rpc_request(&server, request.clone(), &metadata).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("RPC request failed: {}", e);
            JsonRpcResponse::error(request.id, e.to_jsonrpc_error())
        }
    };

    // Record metrics
    let duration = start_time.elapsed().unwrap_or_default();
    let status = if response.error.is_some() { "error" } else { "success" };
    
    server.metrics.middleware(
        &request.method,
        &metadata.client_id,
        start_time,
        status,
        response.error.as_ref().map(|e| e.message.as_str()),
    ).await;

    Json(response).into_response()
}

/// Process individual RPC request
async fn process_rpc_request(
    server: &RpcServer,
    request: JsonRpcRequest,
    _metadata: &RequestMetadata,
) -> RpcResult<JsonRpcResponse> {
    let request_start = std::time::Instant::now();
    debug!("Processing RPC request: {}", request.method);

    // Check cache first
    if let Some(cached_response) = server.cache.get(&request).await {
        debug!("Cache hit for method: {}", request.method);
        server.performance_monitor.record_cache_hit().await;
        server.metrics_collector.record_cache_hit();
        return Ok(cached_response);
    }
    
    server.performance_monitor.record_cache_miss().await;
    server.metrics_collector.record_cache_miss();

    // Route the request
    let routing_decision = server.router.route_request(&request)?;
    
    debug!("Routed {} to {:?}", request.method, routing_decision.target_vm);

    // Process based on target VM
    let response = match routing_decision.target_vm {
        VmType::MultiVm => {
            server.multivm_api.handle_request(routing_decision.transformed_request).await?
        }
        VmType::Ethereum | VmType::Solana => {
            server.relay_manager.forward_request(
                routing_decision.transformed_request,
                routing_decision.target_vm,
            ).await?
        }
    };

    // Transform response if needed
    let final_response = server.router.transform_response(&response, &routing_decision.routing_rule)?;

    // Cache the response if applicable
    if routing_decision.routing_rule.cacheable {
        if let Err(e) = server.cache.put(&request, &final_response).await {
            warn!("Failed to cache response: {}", e);
        }
    }

    // Record performance metrics
    let duration = request_start.elapsed();
    server.performance_monitor.record_request(duration).await;
    server.adaptive_timeout.record_response_time(&request.method, duration).await;
    
    // Record observability metrics
    let request_size = serde_json::to_string(&request).unwrap_or_default().len();
    let response_size = serde_json::to_string(&final_response).unwrap_or_default().len();
    server.metrics_collector.record_request(
        &request.method,
        routing_decision.target_vm,
        duration,
        if final_response.error.is_some() { "error" } else { "success" },
        request_size,
        response_size,
    );

    Ok(final_response)
}

/// Health check handler  
async fn health_handler(State(server): State<Arc<RpcServer>>) -> Json<serde_json::Value> {
    // Get comprehensive system health
    let system_health = match server.health_checker.check_health().await {
        Ok(health) => health,
        Err(_) => {
            return Json(serde_json::json!({
                "status": "unhealthy",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "error": "Failed to perform health check"
            }));
        }
    };

    // Get backend health statuses
    let backend_health = server.relay_manager.get_all_health().await;
    
    Json(serde_json::json!({
        "status": format!("{:?}", system_health.status).to_lowercase(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": crate::VERSION,
        "uptime_seconds": system_health.uptime_seconds,
        "memory": {
            "usage_percent": system_health.memory.usage_percent,
            "used_bytes": system_health.memory.used_bytes,
            "total_bytes": system_health.memory.total_bytes
        },
        "cpu": {
            "usage_percent": system_health.cpu.usage_percent,
            "cores": system_health.cpu.cores,
            "load_1m": system_health.cpu.load_1m
        },
        "disk": {
            "usage_percent": system_health.disk.usage_percent,
            "path": system_health.disk.path
        },
        "backends": backend_health.iter().map(|h| {
            serde_json::json!({
                "name": h.name,
                "vm_type": format!("{:?}", h.vm_type),
                "status": format!("{:?}", h.status).to_lowercase(),
                "response_time_ms": h.response_time_ms,
                "last_success": h.last_success.duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default().as_secs()
            })
        }).collect::<Vec<_>>(),
        "application": {
            "active_connections": system_health.application.active_connections,
            "request_queue_size": system_health.application.request_queue_size,
            "cache_enabled": server.cache.is_enabled()
        }
    }))
}

/// Metrics handler (Prometheus format)
async fn metrics_handler(State(server): State<Arc<RpcServer>>) -> impl IntoResponse {
    // Export metrics in Prometheus format
    match server.metrics_collector.export_prometheus() {
        Ok(metrics) => {
            axum::response::Response::builder()
                .header("Content-Type", "text/plain; version=0.0.4")
                .body(axum::body::Body::from(metrics))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            error!("Failed to export metrics: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Performance metrics handler
async fn performance_handler(State(server): State<Arc<RpcServer>>) -> Json<serde_json::Value> {
    let summary = server.performance_monitor.get_summary().await;
    
    Json(serde_json::json!({
        "uptime_seconds": summary.uptime.as_secs(),
        "total_requests": summary.total_requests,
        "avg_response_time_ms": summary.avg_response_time.as_millis(),
        "cache_hit_rate": summary.cache_hit_rate,
        "requests_per_second": summary.requests_per_second,
        "batch_efficiency": summary.batch_efficiency,
        "connection_reuse_rate": summary.connection_reuse_rate,
        "performance_config": {
            "batching_enabled": server.config.performance.enable_batching,
            "max_batch_size": server.config.performance.max_batch_size,
            "connection_pool_size": server.config.performance.connection_pool_size,
            "compression_enabled": server.config.performance.enable_compression,
            "adaptive_timeout": server.config.performance.adaptive_timeout,
        }
    }))
}

/// WebSocket handler for subscriptions (future enhancement)
async fn _websocket_handler() -> impl IntoResponse {
    // WebSocket support for Ethereum/Solana subscriptions would go here
    StatusCode::NOT_IMPLEMENTED
}

/// Example CLI to run the server
#[cfg(feature = "cli")]
pub async fn run_server_cli() -> RpcResult<()> {
    use std::path::Path;

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config_path = std::env::var("RPC_CONFIG_PATH")
        .unwrap_or_else(|_| "rpc-config.toml".to_string());

    let config = if Path::new(&config_path).exists() {
        RpcServiceConfig::from_file(Path::new(&config_path)).map_err(|e| RpcError::Internal {
            message: format!("Failed to load config: {}", e),
        })?
    } else {
        info!("No config file found, using default configuration");
        RpcServiceConfig::default()
    };

    // Check if running in production mode
    let is_production = std::env::var("MULTIVM_ENV")
        .unwrap_or_else(|_| "development".to_string())
        .to_lowercase() == "production";

    // Apply production validation if needed
    if is_production {
        config.validate_production().map_err(|e| RpcError::ProxyError {
            message: format!("Production configuration validation failed: {}", e),
        })?;
        info!("Production configuration validation passed");
    }

    // Create and start server
    let server = RpcServer::new(config).await?;
    server.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use serde_json::json;

    async fn create_test_server() -> RpcServer {
        let config = RpcServiceConfig::dev_config();
        RpcServer::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_server_creation() {
        let server = create_test_server().await;
        // Basic smoke test
        assert!(server.config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let server = create_test_server().await;
        let app = server.create_router().await.unwrap();
        let test_server = TestServer::new(app).unwrap();

        let response = test_server.get("/health").await;
        response.assert_status_ok();
        
        let body: serde_json::Value = response.json();
        assert!(body.get("status").is_some());
        assert!(body.get("backends").is_some());
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let server = create_test_server().await;
        let app = server.create_router().await.unwrap();
        let test_server = TestServer::new(app).unwrap();

        let response = test_server.get("/metrics").await;
        response.assert_status_ok();
        
        // Metrics are in Prometheus format, not JSON
        let body = response.text();
        assert!(body.contains("multivm_rpc"));
        assert!(body.contains("# HELP"));
    }

    #[tokio::test]
    async fn test_rpc_endpoint() {
        let server = create_test_server().await;
        let app = server.create_router().await.unwrap();
        let test_server = TestServer::new(app).unwrap();

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "method": "multivm_getVersion",
            "params": null,
            "id": 1
        });

        let response = test_server.post("/rpc").json(&rpc_request).await;
        response.assert_status_ok();
        
        let body: serde_json::Value = response.json();
        assert_eq!(body.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
        assert!(body.get("result").is_some());
        assert_eq!(body.get("id").unwrap().as_i64().unwrap(), 1);
    }
}