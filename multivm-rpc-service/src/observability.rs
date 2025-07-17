//! Production-ready observability and metrics

use crate::{
    error::{RpcError, RpcResult},
    types::{VmType, HealthStatus},
};
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, 
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    Registry, TextEncoder, Encoder,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing::{debug, error, info, warn};

/// Metrics configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservabilityConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Metrics namespace
    pub namespace: String,
    /// Subsystem name
    pub subsystem: String,
    /// Enable detailed histograms
    pub detailed_histograms: bool,
    /// Enable trace sampling
    pub trace_sampling: bool,
    /// Trace sample rate (0.0 to 1.0)
    pub trace_sample_rate: f64,
    /// Export format
    pub export_format: ExportFormat,
    /// Push gateway URL (optional)
    pub push_gateway_url: Option<String>,
    /// Push interval
    #[serde(with = "crate::performance::duration_serde")]
    pub push_interval: Duration,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
    Prometheus,
    OpenTelemetry,
    Both,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            namespace: "multivm".to_string(),
            subsystem: "rpc".to_string(),
            detailed_histograms: false,
            trace_sampling: false,
            trace_sample_rate: 0.1,
            export_format: ExportFormat::Prometheus,
            push_gateway_url: None,
            push_interval: Duration::from_secs(60),
        }
    }
}

impl ObservabilityConfig {
    /// Production configuration
    pub fn production() -> Self {
        Self {
            enabled: true,
            namespace: "multivm".to_string(),
            subsystem: "rpc".to_string(),
            detailed_histograms: true,
            trace_sampling: true,
            trace_sample_rate: 0.01, // 1% sampling in production
            export_format: ExportFormat::Both,
            push_gateway_url: None,
            push_interval: Duration::from_secs(30),
        }
    }
}

/// Comprehensive metrics collector
pub struct MetricsCollector {
    config: ObservabilityConfig,
    registry: Registry,
    
    // Request metrics
    request_counter: IntCounterVec,
    request_duration: HistogramVec,
    request_size: HistogramVec,
    response_size: HistogramVec,
    
    // Error metrics
    error_counter: IntCounterVec,
    
    // Cache metrics
    cache_hits: IntCounter,
    cache_misses: IntCounter,
    cache_evictions: IntCounter,
    cache_size: IntGauge,
    
    // Backend metrics
    backend_requests: IntCounterVec,
    backend_errors: IntCounterVec,
    backend_latency: HistogramVec,
    backend_health: IntGaugeVec,
    
    // System metrics
    active_connections: IntGauge,
    queued_requests: IntGauge,
    memory_usage: IntGauge,
    cpu_usage: Gauge,
    
    // Business metrics
    gas_used: CounterVec,
    transaction_count: IntCounterVec,
    block_height: IntGaugeVec,
}

impl MetricsCollector {
    pub fn new(config: ObservabilityConfig) -> RpcResult<Self> {
        let registry = Registry::new();
        
        // Request metrics
        let request_counter = IntCounterVec::new(
            prometheus::Opts::new("requests_total", "Total number of requests")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["method", "vm_type", "status"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create request counter: {}", e),
        })?;
        
        let request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new("request_duration_seconds", "Request duration in seconds")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
            &["method", "vm_type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create request duration histogram: {}", e),
        })?;
        
        let request_size = HistogramVec::new(
            prometheus::HistogramOpts::new("request_size_bytes", "Request size in bytes")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
                .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100000.0]),
            &["method"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create request size histogram: {}", e),
        })?;
        
        let response_size = HistogramVec::new(
            prometheus::HistogramOpts::new("response_size_bytes", "Response size in bytes")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
                .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100000.0, 500000.0]),
            &["method"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create response size histogram: {}", e),
        })?;
        
        // Error metrics
        let error_counter = IntCounterVec::new(
            prometheus::Opts::new("errors_total", "Total number of errors")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["method", "error_type", "vm_type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create error counter: {}", e),
        })?;
        
        // Cache metrics
        let cache_hits = IntCounter::with_opts(
            prometheus::Opts::new("cache_hits_total", "Total number of cache hits")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create cache hits counter: {}", e),
        })?;
        
        let cache_misses = IntCounter::with_opts(
            prometheus::Opts::new("cache_misses_total", "Total number of cache misses")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create cache misses counter: {}", e),
        })?;
        
        let cache_evictions = IntCounter::with_opts(
            prometheus::Opts::new("cache_evictions_total", "Total number of cache evictions")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create cache evictions counter: {}", e),
        })?;
        
        let cache_size = IntGauge::with_opts(
            prometheus::Opts::new("cache_size", "Current cache size")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create cache size gauge: {}", e),
        })?;
        
        // Backend metrics
        let backend_requests = IntCounterVec::new(
            prometheus::Opts::new("backend_requests_total", "Total backend requests")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["backend", "vm_type", "status"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create backend requests counter: {}", e),
        })?;
        
        let backend_errors = IntCounterVec::new(
            prometheus::Opts::new("backend_errors_total", "Total backend errors")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["backend", "vm_type", "error_type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create backend errors counter: {}", e),
        })?;
        
        let backend_latency = HistogramVec::new(
            prometheus::HistogramOpts::new("backend_latency_seconds", "Backend latency in seconds")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
                .buckets(vec![0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
            &["backend", "vm_type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create backend latency histogram: {}", e),
        })?;
        
        let backend_health = IntGaugeVec::new(
            prometheus::Opts::new("backend_health", "Backend health status (0=unhealthy, 1=degraded, 2=healthy)")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["backend", "vm_type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create backend health gauge: {}", e),
        })?;
        
        // System metrics
        let active_connections = IntGauge::with_opts(
            prometheus::Opts::new("active_connections", "Number of active connections")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create active connections gauge: {}", e),
        })?;
        
        let queued_requests = IntGauge::with_opts(
            prometheus::Opts::new("queued_requests", "Number of queued requests")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create queued requests gauge: {}", e),
        })?;
        
        let memory_usage = IntGauge::with_opts(
            prometheus::Opts::new("memory_usage_bytes", "Memory usage in bytes")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create memory usage gauge: {}", e),
        })?;
        
        let cpu_usage = Gauge::with_opts(
            prometheus::Opts::new("cpu_usage_percent", "CPU usage percentage")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem)
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create CPU usage gauge: {}", e),
        })?;
        
        // Business metrics
        let gas_used = CounterVec::new(
            prometheus::Opts::new("gas_used_total", "Total gas used")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["vm_type", "method"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create gas used counter: {}", e),
        })?;
        
        let transaction_count = IntCounterVec::new(
            prometheus::Opts::new("transactions_total", "Total number of transactions")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["vm_type", "type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create transaction counter: {}", e),
        })?;
        
        let block_height = IntGaugeVec::new(
            prometheus::Opts::new("block_height", "Current block height")
                .namespace(&config.namespace)
                .subsystem(&config.subsystem),
            &["vm_type"]
        ).map_err(|e| RpcError::Internal {
            message: format!("Failed to create block height gauge: {}", e),
        })?;
        
        // Register all metrics
        registry.register(Box::new(request_counter.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register request counter: {}", e),
        })?;
        registry.register(Box::new(request_duration.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register request duration: {}", e),
        })?;
        registry.register(Box::new(request_size.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register request size: {}", e),
        })?;
        registry.register(Box::new(response_size.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register response size: {}", e),
        })?;
        registry.register(Box::new(error_counter.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register error counter: {}", e),
        })?;
        registry.register(Box::new(cache_hits.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register cache hits: {}", e),
        })?;
        registry.register(Box::new(cache_misses.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register cache misses: {}", e),
        })?;
        registry.register(Box::new(cache_evictions.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register cache evictions: {}", e),
        })?;
        registry.register(Box::new(cache_size.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register cache size: {}", e),
        })?;
        registry.register(Box::new(backend_requests.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register backend requests: {}", e),
        })?;
        registry.register(Box::new(backend_errors.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register backend errors: {}", e),
        })?;
        registry.register(Box::new(backend_latency.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register backend latency: {}", e),
        })?;
        registry.register(Box::new(backend_health.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register backend health: {}", e),
        })?;
        registry.register(Box::new(active_connections.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register active connections: {}", e),
        })?;
        registry.register(Box::new(queued_requests.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register queued requests: {}", e),
        })?;
        registry.register(Box::new(memory_usage.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register memory usage: {}", e),
        })?;
        registry.register(Box::new(cpu_usage.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register cpu usage: {}", e),
        })?;
        registry.register(Box::new(gas_used.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register gas used: {}", e),
        })?;
        registry.register(Box::new(transaction_count.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register transaction count: {}", e),
        })?;
        registry.register(Box::new(block_height.clone())).map_err(|e| RpcError::Internal {
            message: format!("Failed to register block height: {}", e),
        })?;
        
        Ok(Self {
            config,
            registry,
            request_counter,
            request_duration,
            request_size,
            response_size,
            error_counter,
            cache_hits,
            cache_misses,
            cache_evictions,
            cache_size,
            backend_requests,
            backend_errors,
            backend_latency,
            backend_health,
            active_connections,
            queued_requests,
            memory_usage,
            cpu_usage,
            gas_used,
            transaction_count,
            block_height,
        })
    }
    
    /// Record a request
    pub fn record_request(
        &self,
        method: &str,
        vm_type: VmType,
        duration: Duration,
        status: &str,
        request_size: usize,
        response_size: usize,
    ) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        
        self.request_counter
            .with_label_values(&[method, &vm_type_str, status])
            .inc();
            
        self.request_duration
            .with_label_values(&[method, &vm_type_str])
            .observe(duration.as_secs_f64());
            
        self.request_size
            .with_label_values(&[method])
            .observe(request_size as f64);
            
        self.response_size
            .with_label_values(&[method])
            .observe(response_size as f64);
    }
    
    /// Record an error
    pub fn record_error(&self, method: &str, vm_type: VmType, error_type: &str) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        
        self.error_counter
            .with_label_values(&[method, error_type, &vm_type_str])
            .inc();
    }
    
    /// Record cache hit
    pub fn record_cache_hit(&self) {
        if !self.config.enabled {
            return;
        }
        self.cache_hits.inc();
    }
    
    /// Record cache miss
    pub fn record_cache_miss(&self) {
        if !self.config.enabled {
            return;
        }
        self.cache_misses.inc();
    }
    
    /// Record cache eviction
    pub fn record_cache_eviction(&self) {
        if !self.config.enabled {
            return;
        }
        self.cache_evictions.inc();
    }
    
    /// Update cache size
    pub fn update_cache_size(&self, size: i64) {
        if !self.config.enabled {
            return;
        }
        self.cache_size.set(size);
    }
    
    /// Record backend request
    pub fn record_backend_request(
        &self,
        backend: &str,
        vm_type: VmType,
        duration: Duration,
        success: bool,
    ) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        let status = if success { "success" } else { "failure" };
        
        self.backend_requests
            .with_label_values(&[backend, &vm_type_str, status])
            .inc();
            
        self.backend_latency
            .with_label_values(&[backend, &vm_type_str])
            .observe(duration.as_secs_f64());
    }
    
    /// Record backend error
    pub fn record_backend_error(&self, backend: &str, vm_type: VmType, error_type: &str) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        
        self.backend_errors
            .with_label_values(&[backend, &vm_type_str, error_type])
            .inc();
    }
    
    /// Update backend health
    pub fn update_backend_health(&self, backend: &str, vm_type: VmType, health: HealthStatus) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        let health_value = match health {
            HealthStatus::Healthy => 2,
            HealthStatus::Degraded => 1,
            HealthStatus::Unhealthy => 0,
            HealthStatus::Unknown => -1,
        };
        
        self.backend_health
            .with_label_values(&[backend, &vm_type_str])
            .set(health_value);
    }
    
    /// Update system metrics
    pub fn update_system_metrics(
        &self,
        active_connections: i64,
        queued_requests: i64,
        memory_bytes: i64,
        cpu_percent: f64,
    ) {
        if !self.config.enabled {
            return;
        }
        
        self.active_connections.set(active_connections);
        self.queued_requests.set(queued_requests);
        self.memory_usage.set(memory_bytes);
        self.cpu_usage.set(cpu_percent);
    }
    
    /// Record gas usage
    pub fn record_gas_used(&self, vm_type: VmType, method: &str, gas: u64) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        
        self.gas_used
            .with_label_values(&[&vm_type_str, method])
            .inc_by(gas as f64);
    }
    
    /// Record transaction
    pub fn record_transaction(&self, vm_type: VmType, tx_type: &str) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        
        self.transaction_count
            .with_label_values(&[&vm_type_str, tx_type])
            .inc();
    }
    
    /// Update block height
    pub fn update_block_height(&self, vm_type: VmType, height: u64) {
        if !self.config.enabled {
            return;
        }
        
        let vm_type_str = format!("{:?}", vm_type).to_lowercase();
        
        self.block_height
            .with_label_values(&[&vm_type_str])
            .set(height as i64);
    }
    
    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> RpcResult<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)
            .map_err(|e| RpcError::Internal {
                message: format!("Failed to encode metrics: {}", e),
            })?;
        
        String::from_utf8(buffer).map_err(|e| RpcError::Internal {
            message: format!("Failed to convert metrics to string: {}", e),
        })
    }
    
    /// Get registry for external use
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

/// Distributed tracing support
pub struct TracingCollector {
    config: ObservabilityConfig,
    spans: Arc<tokio::sync::RwLock<HashMap<String, SpanInfo>>>,
}

#[derive(Debug, Clone)]
struct SpanInfo {
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    operation: String,
    start_time: SystemTime,
    end_time: Option<SystemTime>,
    tags: HashMap<String, String>,
    status: SpanStatus,
}

#[derive(Debug, Clone)]
enum SpanStatus {
    Running,
    Ok,
    Error(String),
}

impl TracingCollector {
    pub fn new(config: ObservabilityConfig) -> Self {
        Self {
            config,
            spans: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
    
    /// Start a new span
    pub async fn start_span(
        &self,
        operation: &str,
        trace_id: Option<String>,
        parent_span_id: Option<String>,
    ) -> String {
        if !self.config.trace_sampling {
            return String::new();
        }
        
        // Apply sampling
        if rand::random::<f64>() > self.config.trace_sample_rate {
            return String::new();
        }
        
        let span_id = uuid::Uuid::new_v4().to_string();
        let trace_id = trace_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        let span_info = SpanInfo {
            trace_id: trace_id.clone(),
            span_id: span_id.clone(),
            parent_span_id,
            operation: operation.to_string(),
            start_time: SystemTime::now(),
            end_time: None,
            tags: HashMap::new(),
            status: SpanStatus::Running,
        };
        
        self.spans.write().await.insert(span_id.clone(), span_info);
        
        debug!("Started span {} for operation {}", span_id, operation);
        span_id
    }
    
    /// End a span
    pub async fn end_span(&self, span_id: &str, status: Result<(), String>) {
        if span_id.is_empty() {
            return;
        }
        
        let mut spans = self.spans.write().await;
        if let Some(span) = spans.get_mut(span_id) {
            span.end_time = Some(SystemTime::now());
            span.status = match status {
                Ok(()) => SpanStatus::Ok,
                Err(e) => SpanStatus::Error(e),
            };
            
            debug!("Ended span {} with status {:?}", span_id, span.status);
        }
    }
    
    /// Add tag to span
    pub async fn add_tag(&self, span_id: &str, key: &str, value: &str) {
        if span_id.is_empty() {
            return;
        }
        
        let mut spans = self.spans.write().await;
        if let Some(span) = spans.get_mut(span_id) {
            span.tags.insert(key.to_string(), value.to_string());
        }
    }
    
    /// Export spans (simplified - would integrate with OpenTelemetry in production)
    pub async fn export_spans(&self) -> Vec<SpanInfo> {
        let spans = self.spans.read().await;
        let now = SystemTime::now();
        
        // Export completed spans older than 1 minute
        spans.values()
            .filter(|span| {
                span.end_time.is_some() && 
                span.end_time.unwrap().elapsed().unwrap_or_default() > Duration::from_secs(60)
            })
            .cloned()
            .collect()
    }
    
    /// Clean up old spans
    pub async fn cleanup_old_spans(&self) {
        let mut spans = self.spans.write().await;
        let now = SystemTime::now();
        
        spans.retain(|_, span| {
            if let Some(end_time) = span.end_time {
                end_time.elapsed().unwrap_or_default() < Duration::from_secs(300) // Keep for 5 minutes
            } else {
                // Remove running spans older than 10 minutes
                span.start_time.elapsed().unwrap_or_default() < Duration::from_secs(600)
            }
        });
    }
}

/// Alerting support
pub struct AlertManager {
    config: ObservabilityConfig,
    alerts: Arc<tokio::sync::RwLock<Vec<Alert>>>,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub timestamp: SystemTime,
    pub labels: HashMap<String, String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertManager {
    pub fn new(config: ObservabilityConfig) -> Self {
        Self {
            config,
            alerts: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
    
    /// Create a new alert
    pub async fn create_alert(
        &self,
        severity: AlertSeverity,
        title: &str,
        message: &str,
        labels: HashMap<String, String>,
    ) -> String {
        let alert_id = uuid::Uuid::new_v4().to_string();
        
        let alert = Alert {
            id: alert_id.clone(),
            severity,
            title: title.to_string(),
            message: message.to_string(),
            timestamp: SystemTime::now(),
            labels,
            resolved: false,
        };
        
        self.alerts.write().await.push(alert.clone());
        
        match severity {
            AlertSeverity::Info => info!("Alert {}: {}", title, message),
            AlertSeverity::Warning => warn!("Alert {}: {}", title, message),
            AlertSeverity::Critical => error!("Alert {}: {}", title, message),
        }
        
        alert_id
    }
    
    /// Resolve an alert
    pub async fn resolve_alert(&self, alert_id: &str) {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved = true;
            info!("Resolved alert {}: {}", alert.title, alert.message);
        }
    }
    
    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        self.alerts.read().await
            .iter()
            .filter(|a| !a.resolved)
            .cloned()
            .collect()
    }
    
    /// Check metrics and create alerts
    pub async fn check_thresholds(&self, metrics: &MetricsCollector) {
        // This would check various thresholds and create alerts
        // For example:
        // - High error rate
        // - High latency
        // - Backend failures
        // - Resource exhaustion
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_observability_config() {
        let config = ObservabilityConfig::default();
        assert!(config.enabled);
        assert_eq!(config.namespace, "multivm");
        
        let prod = ObservabilityConfig::production();
        assert!(prod.detailed_histograms);
        assert_eq!(prod.trace_sample_rate, 0.01);
    }
    
    #[tokio::test]
    async fn test_metrics_collector() {
        let config = ObservabilityConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        // Record some metrics
        collector.record_request("eth_getBalance", VmType::Ethereum, Duration::from_millis(100), "success", 100, 200);
        collector.record_cache_hit();
        collector.record_cache_miss();
        
        // Export metrics
        let metrics = collector.export_prometheus().unwrap();
        assert!(metrics.contains("multivm_rpc_requests_total"));
        assert!(metrics.contains("multivm_rpc_cache_hits_total"));
    }
    
    #[tokio::test]
    async fn test_tracing_collector() {
        let config = ObservabilityConfig {
            trace_sampling: true,
            trace_sample_rate: 1.0, // Always sample for testing
            ..Default::default()
        };
        let tracer = TracingCollector::new(config);
        
        let span_id = tracer.start_span("test_operation", None, None).await;
        assert!(!span_id.is_empty());
        
        tracer.add_tag(&span_id, "test_key", "test_value").await;
        tracer.end_span(&span_id, Ok(())).await;
        
        let spans = tracer.export_spans().await;
        // Won't export immediately (needs to be older than 1 minute)
        assert_eq!(spans.len(), 0);
    }
    
    #[tokio::test]
    async fn test_alert_manager() {
        let config = ObservabilityConfig::default();
        let alert_manager = AlertManager::new(config);
        
        let alert_id = alert_manager.create_alert(
            AlertSeverity::Warning,
            "High Error Rate",
            "Error rate exceeded 5%",
            HashMap::new(),
        ).await;
        
        let active = alert_manager.get_active_alerts().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, alert_id);
        
        alert_manager.resolve_alert(&alert_id).await;
        
        let active = alert_manager.get_active_alerts().await;
        assert_eq!(active.len(), 0);
    }
}