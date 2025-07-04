//! Metrics collection and export

use crate::error::{ApplicationError, ApplicationResult};
use parking_lot::RwLock;
use std::sync::Arc;

/// Metrics service for collecting and exporting metrics
#[derive(Debug)]
pub struct MetricsService {
    registry: Arc<RwLock<MetricsRegistry>>,
    config: crate::config::MonitoringConfig,
}

impl MetricsService {
    /// Create new metrics service
    pub async fn new(config: &crate::config::MonitoringConfig) -> ApplicationResult<Self> {
        Ok(Self {
            registry: Arc::new(RwLock::new(MetricsRegistry::new())),
            config: config.clone(),
        })
    }

    /// Start metrics HTTP server
    pub async fn start_server(&self) -> ApplicationResult<()> {
        use axum::{routing::get, Router};
        use tower::ServiceBuilder;
        use tower_http::trace::TraceLayer;

        let addr = format!("0.0.0.0:{}", self.config.metrics_port);
        tracing::info!("Starting metrics server on {}", addr);

        // Create axum router with metrics endpoint
        let app = Router::new()
            .route("/metrics", get(|| async {
                // Return Prometheus format metrics
                let metrics_data = format!(
                    "# HELP http_requests_total Total HTTP requests\n# TYPE http_requests_total counter\nhttp_requests_total {{}} {}\n",
                    0 // Would read from actual registry
                );
                (
                    [("content-type", "text/plain; version=0.0.4")],
                    metrics_data
                )
            }))
            .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

        let socket_addr: std::net::SocketAddr =
            addr.parse()
                .map_err(|e| ApplicationError::ConfigurationError {
                    component: "metrics".to_string(),
                    message: format!("Invalid bind address: {e}"),
                })?;

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(socket_addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("Failed to bind metrics server: {}", e);
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Metrics server error: {}", e);
            }
        });

        tracing::info!("Metrics server started on {}", addr);
        Ok(())
    }

    /// Record HTTP request
    pub fn record_http_request(&self, _method: &str, _path: &str, status: u16, duration_ms: f64) {
        let mut registry = self.registry.write();
        registry.http_requests_total += 1;
        registry.http_request_duration_sum += duration_ms;

        // Track status codes
        match status {
            200..=299 => registry.http_requests_success += 1,
            400..=499 => registry.http_requests_client_error += 1,
            500..=599 => registry.http_requests_server_error += 1,
            _ => {}
        }
    }

    /// Record WebSocket connection
    pub fn record_ws_connection(&self, connected: bool) {
        let mut registry = self.registry.write();
        if connected {
            registry.ws_connections_active += 1;
            registry.ws_connections_total += 1;
        } else {
            registry.ws_connections_active = registry.ws_connections_active.saturating_sub(1);
        }
    }

    /// Record GraphQL query
    pub fn record_graphql_query(&self, _operation: &str, duration_ms: f64, success: bool) {
        let mut registry = self.registry.write();
        registry.graphql_queries_total += 1;
        if success {
            registry.graphql_queries_success += 1;
        }
        registry.graphql_query_duration_sum += duration_ms;
    }

    /// Record cache operation
    pub fn record_cache_operation(&self, operation: &str, hit: bool, _duration_ms: f64) {
        let mut registry = self.registry.write();
        match operation {
            "get" => {
                registry.cache_gets_total += 1;
                if hit {
                    registry.cache_hits += 1;
                }
            }
            "set" => registry.cache_sets_total += 1,
            "delete" => registry.cache_deletes_total += 1,
            _ => {}
        }
    }

    /// Record VM operation
    pub fn record_vm_operation(
        &self,
        vm_type: &str,
        _operation: &str,
        success: bool,
        _duration_ms: f64,
    ) {
        let mut registry = self.registry.write();
        match vm_type {
            "svm" => {
                registry.svm_operations_total += 1;
                if success {
                    registry.svm_operations_success += 1;
                }
            }
            "evm" => {
                registry.evm_operations_total += 1;
                if success {
                    registry.evm_operations_success += 1;
                }
            }
            _ => {}
        }
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> MetricsSnapshot {
        let registry = self.registry.read();
        MetricsSnapshot {
            http_requests_total: registry.http_requests_total,
            http_requests_success: registry.http_requests_success,
            http_requests_client_error: registry.http_requests_client_error,
            http_requests_server_error: registry.http_requests_server_error,
            http_request_duration_avg: if registry.http_requests_total > 0 {
                registry.http_request_duration_sum / registry.http_requests_total as f64
            } else {
                0.0
            },
            ws_connections_active: registry.ws_connections_active,
            ws_connections_total: registry.ws_connections_total,
            graphql_queries_total: registry.graphql_queries_total,
            graphql_queries_success: registry.graphql_queries_success,
            cache_hits: registry.cache_hits,
            cache_gets_total: registry.cache_gets_total,
            cache_hit_rate: if registry.cache_gets_total > 0 {
                registry.cache_hits as f64 / registry.cache_gets_total as f64
            } else {
                0.0
            },
            svm_operations_total: registry.svm_operations_total,
            svm_operations_success: registry.svm_operations_success,
            evm_operations_total: registry.evm_operations_total,
            evm_operations_success: registry.evm_operations_success,
        }
    }
}

/// Internal metrics registry
#[derive(Debug)]
struct MetricsRegistry {
    // HTTP metrics
    http_requests_total: u64,
    http_requests_success: u64,
    http_requests_client_error: u64,
    http_requests_server_error: u64,
    http_request_duration_sum: f64,

    // WebSocket metrics
    ws_connections_active: u64,
    ws_connections_total: u64,

    // GraphQL metrics
    graphql_queries_total: u64,
    graphql_queries_success: u64,
    graphql_query_duration_sum: f64,

    // Cache metrics
    cache_hits: u64,
    cache_gets_total: u64,
    cache_sets_total: u64,
    cache_deletes_total: u64,

    // VM operation metrics
    svm_operations_total: u64,
    svm_operations_success: u64,
    evm_operations_total: u64,
    evm_operations_success: u64,
}

impl MetricsRegistry {
    fn new() -> Self {
        Self {
            http_requests_total: 0,
            http_requests_success: 0,
            http_requests_client_error: 0,
            http_requests_server_error: 0,
            http_request_duration_sum: 0.0,
            ws_connections_active: 0,
            ws_connections_total: 0,
            graphql_queries_total: 0,
            graphql_queries_success: 0,
            graphql_query_duration_sum: 0.0,
            cache_hits: 0,
            cache_gets_total: 0,
            cache_sets_total: 0,
            cache_deletes_total: 0,
            svm_operations_total: 0,
            svm_operations_success: 0,
            evm_operations_total: 0,
            evm_operations_success: 0,
        }
    }
}

/// Metrics snapshot for external consumption
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub http_requests_total: u64,
    pub http_requests_success: u64,
    pub http_requests_client_error: u64,
    pub http_requests_server_error: u64,
    pub http_request_duration_avg: f64,
    pub ws_connections_active: u64,
    pub ws_connections_total: u64,
    pub graphql_queries_total: u64,
    pub graphql_queries_success: u64,
    pub cache_hits: u64,
    pub cache_gets_total: u64,
    pub cache_hit_rate: f64,
    pub svm_operations_total: u64,
    pub svm_operations_success: u64,
    pub evm_operations_total: u64,
    pub evm_operations_success: u64,
}
