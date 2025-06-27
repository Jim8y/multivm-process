//! Production-ready metrics collection for MultiVM system
//! Comprehensive metrics for monitoring performance, reliability, and business KPIs

use crate::error::{ApplicationError, ApplicationResult};
use prometheus::{
    Counter, Gauge, GaugeVec, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec, Registry,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Production metrics collector with comprehensive monitoring
#[derive(Debug)]
pub struct ProductionMetrics {
    registry: Arc<Registry>,

    // HTTP metrics
    http_requests_total: IntCounterVec,
    http_request_duration: HistogramVec,
    http_requests_in_flight: IntGaugeVec,

    // VM execution metrics
    vm_operations_total: IntCounterVec,
    vm_operation_duration: HistogramVec,
    vm_operation_errors: IntCounterVec,
    vm_engine_status: IntGaugeVec,

    // Cross-VM metrics
    cross_vm_transactions_total: IntCounterVec,
    cross_vm_transaction_duration: HistogramVec,
    cross_vm_transaction_errors: IntCounterVec,
    cross_vm_account_bindings: IntGauge,

    // Consensus metrics
    consensus_rounds_total: IntCounter,
    consensus_round_duration: Histogram,
    consensus_proposals_total: IntCounterVec,
    consensus_votes_total: IntCounterVec,
    consensus_finalized_blocks: IntCounter,
    consensus_latency: Histogram,

    // Database metrics
    db_connections_active: IntGauge,
    db_connections_idle: IntGauge,
    db_query_duration: HistogramVec,
    db_query_errors: IntCounterVec,
    db_transactions_total: IntCounterVec,

    // Cache metrics
    cache_operations_total: IntCounterVec,
    cache_hit_ratio: Gauge,
    cache_memory_usage: IntGauge,
    cache_evictions_total: IntCounter,

    // System metrics
    memory_usage_bytes: IntGauge,
    cpu_usage_percent: Gauge,
    disk_usage_bytes: IntGaugeVec,
    network_bytes_total: IntCounterVec,

    // Business metrics
    active_users: IntGauge,
    transaction_volume_total: IntCounterVec,
    revenue_total: Counter,
    api_key_usage: IntCounterVec,

    // Performance metrics
    throughput_tps: GaugeVec,
    latency_percentiles: HistogramVec,
    error_rate: GaugeVec,
    availability_ratio: Gauge,

    // Security metrics
    auth_attempts_total: IntCounterVec,
    rate_limit_hits: IntCounterVec,
    security_events: IntCounterVec,

    // Internal state
    start_time: Instant,
    last_metrics_update: Arc<RwLock<Instant>>,
}

impl ProductionMetrics {
    /// Create new production metrics collector
    pub fn new() -> ApplicationResult<Self> {
        let registry = Arc::new(Registry::new());

        // HTTP metrics
        let http_requests_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_http_requests_total",
                "Total number of HTTP requests",
            ),
            &["method", "endpoint", "status_code", "version"],
        )?;

        let http_request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "multivm_http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["method", "endpoint", "status_code"],
        )?;

        let http_requests_in_flight = IntGaugeVec::new(
            prometheus::Opts::new(
                "multivm_http_requests_in_flight",
                "Number of HTTP requests currently being processed",
            ),
            &["method", "endpoint"],
        )?;

        // VM execution metrics
        let vm_operations_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_vm_operations_total",
                "Total number of VM operations",
            ),
            &["vm_type", "operation", "status"],
        )?;

        let vm_operation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "multivm_vm_operation_duration_seconds",
                "VM operation duration in seconds",
            )
            .buckets(vec![
                0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0,
            ]),
            &["vm_type", "operation"],
        )?;

        let vm_operation_errors = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_vm_operation_errors_total",
                "Total number of VM operation errors",
            ),
            &["vm_type", "operation", "error_type"],
        )?;

        let vm_engine_status = IntGaugeVec::new(
            prometheus::Opts::new(
                "multivm_vm_engine_status",
                "VM engine status (1=healthy, 0=unhealthy)",
            ),
            &["vm_type", "instance"],
        )?;

        // Cross-VM metrics
        let cross_vm_transactions_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_cross_vm_transactions_total",
                "Total number of cross-VM transactions",
            ),
            &["source_vm", "target_vm", "operation", "status"],
        )?;

        let cross_vm_transaction_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "multivm_cross_vm_transaction_duration_seconds",
                "Cross-VM transaction duration in seconds",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]),
            &["source_vm", "target_vm", "operation"],
        )?;

        let cross_vm_transaction_errors = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_cross_vm_transaction_errors_total",
                "Total number of cross-VM transaction errors",
            ),
            &["source_vm", "target_vm", "operation", "error_type"],
        )?;

        let cross_vm_account_bindings = IntGauge::with_opts(prometheus::Opts::new(
            "multivm_cross_vm_account_bindings",
            "Number of active cross-VM account bindings",
        ))?;

        // Consensus metrics
        let consensus_rounds_total = IntCounter::with_opts(prometheus::Opts::new(
            "multivm_consensus_rounds_total",
            "Total number of consensus rounds",
        ))?;

        let consensus_round_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "multivm_consensus_round_duration_seconds",
                "Consensus round duration in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
        )?;

        let consensus_proposals_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_consensus_proposals_total",
                "Total number of consensus proposals",
            ),
            &["proposer", "status"],
        )?;

        let consensus_votes_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_consensus_votes_total",
                "Total number of consensus votes",
            ),
            &["voter", "vote_type"],
        )?;

        let consensus_finalized_blocks = IntCounter::with_opts(prometheus::Opts::new(
            "multivm_consensus_finalized_blocks_total",
            "Total number of finalized blocks",
        ))?;

        let consensus_latency = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "multivm_consensus_latency_seconds",
                "Consensus latency from proposal to finalization",
            )
            .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 60.0]),
        )?;

        // Database metrics
        let db_connections_active = IntGauge::with_opts(prometheus::Opts::new(
            "multivm_db_connections_active",
            "Number of active database connections",
        ))?;

        let db_connections_idle = IntGauge::with_opts(prometheus::Opts::new(
            "multivm_db_connections_idle",
            "Number of idle database connections",
        ))?;

        let db_query_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "multivm_db_query_duration_seconds",
                "Database query duration in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["query_type", "table"],
        )?;

        let db_query_errors = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_db_query_errors_total",
                "Total number of database query errors",
            ),
            &["query_type", "error_type"],
        )?;

        let db_transactions_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_db_transactions_total",
                "Total number of database transactions",
            ),
            &["operation", "status"],
        )?;

        // Cache metrics
        let cache_operations_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_cache_operations_total",
                "Total number of cache operations",
            ),
            &["operation", "cache_type", "status"],
        )?;

        let cache_hit_ratio = Gauge::with_opts(prometheus::Opts::new(
            "multivm_cache_hit_ratio",
            "Cache hit ratio (0.0 to 1.0)",
        ))?;

        let cache_memory_usage = IntGauge::with_opts(prometheus::Opts::new(
            "multivm_cache_memory_usage_bytes",
            "Cache memory usage in bytes",
        ))?;

        let cache_evictions_total = IntCounter::with_opts(prometheus::Opts::new(
            "multivm_cache_evictions_total",
            "Total number of cache evictions",
        ))?;

        // System metrics
        let memory_usage_bytes = IntGauge::with_opts(prometheus::Opts::new(
            "multivm_memory_usage_bytes",
            "Memory usage in bytes",
        ))?;

        let cpu_usage_percent = Gauge::with_opts(prometheus::Opts::new(
            "multivm_cpu_usage_percent",
            "CPU usage percentage",
        ))?;

        let disk_usage_bytes = IntGaugeVec::new(
            prometheus::Opts::new("multivm_disk_usage_bytes", "Disk usage in bytes"),
            &["mount_point", "device"],
        )?;

        let network_bytes_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_network_bytes_total",
                "Total network bytes transferred",
            ),
            &["direction", "interface"],
        )?;

        // Business metrics
        let active_users = IntGauge::with_opts(prometheus::Opts::new(
            "multivm_active_users",
            "Number of active users",
        ))?;

        let transaction_volume_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_transaction_volume_total",
                "Total transaction volume",
            ),
            &["vm_type", "transaction_type"],
        )?;

        let revenue_total = Counter::with_opts(prometheus::Opts::new(
            "multivm_revenue_total",
            "Total revenue generated",
        ))?;

        let api_key_usage = IntCounterVec::new(
            prometheus::Opts::new("multivm_api_key_usage_total", "API key usage count"),
            &["api_key_id", "endpoint"],
        )?;

        // Performance metrics
        let throughput_tps = GaugeVec::new(
            prometheus::Opts::new(
                "multivm_throughput_tps",
                "Current throughput in transactions per second",
            ),
            &["vm_type", "operation"],
        )?;

        let latency_percentiles = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "multivm_latency_percentiles_seconds",
                "Latency percentiles in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ]),
            &["operation", "percentile"],
        )?;

        let error_rate = GaugeVec::new(
            prometheus::Opts::new("multivm_error_rate", "Current error rate (0.0 to 1.0)"),
            &["component", "error_type"],
        )?;

        let availability_ratio = Gauge::with_opts(prometheus::Opts::new(
            "multivm_availability_ratio",
            "System availability ratio (0.0 to 1.0)",
        ))?;

        // Security metrics
        let auth_attempts_total = IntCounterVec::new(
            prometheus::Opts::new(
                "multivm_auth_attempts_total",
                "Total authentication attempts",
            ),
            &["method", "status", "source"],
        )?;

        let rate_limit_hits = IntCounterVec::new(
            prometheus::Opts::new("multivm_rate_limit_hits_total", "Total rate limit hits"),
            &["endpoint", "limit_type"],
        )?;

        let security_events = IntCounterVec::new(
            prometheus::Opts::new("multivm_security_events_total", "Total security events"),
            &["event_type", "severity"],
        )?;

        // Register all metrics
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration.clone()))?;
        registry.register(Box::new(http_requests_in_flight.clone()))?;
        registry.register(Box::new(vm_operations_total.clone()))?;
        registry.register(Box::new(vm_operation_duration.clone()))?;
        registry.register(Box::new(vm_operation_errors.clone()))?;
        registry.register(Box::new(vm_engine_status.clone()))?;
        registry.register(Box::new(cross_vm_transactions_total.clone()))?;
        registry.register(Box::new(cross_vm_transaction_duration.clone()))?;
        registry.register(Box::new(cross_vm_transaction_errors.clone()))?;
        registry.register(Box::new(cross_vm_account_bindings.clone()))?;
        registry.register(Box::new(consensus_rounds_total.clone()))?;
        registry.register(Box::new(consensus_round_duration.clone()))?;
        registry.register(Box::new(consensus_proposals_total.clone()))?;
        registry.register(Box::new(consensus_votes_total.clone()))?;
        registry.register(Box::new(consensus_finalized_blocks.clone()))?;
        registry.register(Box::new(consensus_latency.clone()))?;
        registry.register(Box::new(db_connections_active.clone()))?;
        registry.register(Box::new(db_connections_idle.clone()))?;
        registry.register(Box::new(db_query_duration.clone()))?;
        registry.register(Box::new(db_query_errors.clone()))?;
        registry.register(Box::new(db_transactions_total.clone()))?;
        registry.register(Box::new(cache_operations_total.clone()))?;
        registry.register(Box::new(cache_hit_ratio.clone()))?;
        registry.register(Box::new(cache_memory_usage.clone()))?;
        registry.register(Box::new(cache_evictions_total.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(cpu_usage_percent.clone()))?;
        registry.register(Box::new(disk_usage_bytes.clone()))?;
        registry.register(Box::new(network_bytes_total.clone()))?;
        registry.register(Box::new(active_users.clone()))?;
        registry.register(Box::new(transaction_volume_total.clone()))?;
        registry.register(Box::new(revenue_total.clone()))?;
        registry.register(Box::new(api_key_usage.clone()))?;
        registry.register(Box::new(throughput_tps.clone()))?;
        registry.register(Box::new(latency_percentiles.clone()))?;
        registry.register(Box::new(error_rate.clone()))?;
        registry.register(Box::new(availability_ratio.clone()))?;
        registry.register(Box::new(auth_attempts_total.clone()))?;
        registry.register(Box::new(rate_limit_hits.clone()))?;
        registry.register(Box::new(security_events.clone()))?;

        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration,
            http_requests_in_flight,
            vm_operations_total,
            vm_operation_duration,
            vm_operation_errors,
            vm_engine_status,
            cross_vm_transactions_total,
            cross_vm_transaction_duration,
            cross_vm_transaction_errors,
            cross_vm_account_bindings,
            consensus_rounds_total,
            consensus_round_duration,
            consensus_proposals_total,
            consensus_votes_total,
            consensus_finalized_blocks,
            consensus_latency,
            db_connections_active,
            db_connections_idle,
            db_query_duration,
            db_query_errors,
            db_transactions_total,
            cache_operations_total,
            cache_hit_ratio,
            cache_memory_usage,
            cache_evictions_total,
            memory_usage_bytes,
            cpu_usage_percent,
            disk_usage_bytes,
            network_bytes_total,
            active_users,
            transaction_volume_total,
            revenue_total,
            api_key_usage,
            throughput_tps,
            latency_percentiles,
            error_rate,
            availability_ratio,
            auth_attempts_total,
            rate_limit_hits,
            security_events,
            start_time: Instant::now(),
            last_metrics_update: Arc::new(RwLock::new(Instant::now())),
        })
    }

    /// Get Prometheus registry for metrics export
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    /// Record HTTP request metrics
    pub fn record_http_request(
        &self,
        method: &str,
        endpoint: &str,
        status_code: u16,
        duration: Duration,
        version: &str,
    ) {
        let status_str = status_code.to_string();

        self.http_requests_total
            .with_label_values(&[method, endpoint, &status_str, version])
            .inc();

        self.http_request_duration
            .with_label_values(&[method, endpoint, &status_str])
            .observe(duration.as_secs_f64());
    }

    /// Track HTTP requests in flight
    pub fn http_request_start(&self, method: &str, endpoint: &str) {
        self.http_requests_in_flight
            .with_label_values(&[method, endpoint])
            .inc();
    }

    pub fn http_request_end(&self, method: &str, endpoint: &str) {
        self.http_requests_in_flight
            .with_label_values(&[method, endpoint])
            .dec();
    }

    /// Record VM operation metrics
    pub fn record_vm_operation(
        &self,
        vm_type: &str,
        operation: &str,
        duration: Duration,
        success: bool,
    ) {
        let status = if success { "success" } else { "error" };

        self.vm_operations_total
            .with_label_values(&[vm_type, operation, status])
            .inc();

        self.vm_operation_duration
            .with_label_values(&[vm_type, operation])
            .observe(duration.as_secs_f64());
    }

    /// Record VM operation error
    pub fn record_vm_error(&self, vm_type: &str, operation: &str, error_type: &str) {
        self.vm_operation_errors
            .with_label_values(&[vm_type, operation, error_type])
            .inc();
    }

    /// Update VM engine status
    pub fn set_vm_engine_status(&self, vm_type: &str, instance: &str, healthy: bool) {
        let status = if healthy { 1 } else { 0 };
        self.vm_engine_status
            .with_label_values(&[vm_type, instance])
            .set(status);
    }

    /// Record cross-VM transaction
    pub fn record_cross_vm_transaction(
        &self,
        source_vm: &str,
        target_vm: &str,
        operation: &str,
        duration: Duration,
        success: bool,
    ) {
        let status = if success { "success" } else { "error" };

        self.cross_vm_transactions_total
            .with_label_values(&[source_vm, target_vm, operation, status])
            .inc();

        self.cross_vm_transaction_duration
            .with_label_values(&[source_vm, target_vm, operation])
            .observe(duration.as_secs_f64());
    }

    /// Record cross-VM transaction error
    pub fn record_cross_vm_error(
        &self,
        source_vm: &str,
        target_vm: &str,
        operation: &str,
        error_type: &str,
    ) {
        self.cross_vm_transaction_errors
            .with_label_values(&[source_vm, target_vm, operation, error_type])
            .inc();
    }

    /// Update cross-VM account bindings count
    pub fn set_cross_vm_account_bindings(&self, count: i64) {
        self.cross_vm_account_bindings.set(count);
    }

    /// Record consensus metrics
    pub fn record_consensus_round(&self, duration: Duration) {
        self.consensus_rounds_total.inc();
        self.consensus_round_duration
            .observe(duration.as_secs_f64());
    }

    pub fn record_consensus_proposal(&self, proposer: &str, accepted: bool) {
        let status = if accepted { "accepted" } else { "rejected" };
        self.consensus_proposals_total
            .with_label_values(&[proposer, status])
            .inc();
    }

    pub fn record_consensus_vote(&self, voter: &str, vote_type: &str) {
        self.consensus_votes_total
            .with_label_values(&[voter, vote_type])
            .inc();
    }

    pub fn record_finalized_block(&self, consensus_latency: Duration) {
        self.consensus_finalized_blocks.inc();
        self.consensus_latency
            .observe(consensus_latency.as_secs_f64());
    }

    /// Record database metrics
    pub fn set_db_connections(&self, active: i64, idle: i64) {
        self.db_connections_active.set(active);
        self.db_connections_idle.set(idle);
    }

    pub fn record_db_query(
        &self,
        query_type: &str,
        table: &str,
        duration: Duration,
        success: bool,
    ) {
        self.db_query_duration
            .with_label_values(&[query_type, table])
            .observe(duration.as_secs_f64());

        if !success {
            self.db_query_errors
                .with_label_values(&[query_type, "execution_error"])
                .inc();
        }
    }

    pub fn record_db_transaction(&self, operation: &str, success: bool) {
        let status = if success { "success" } else { "error" };
        self.db_transactions_total
            .with_label_values(&[operation, status])
            .inc();
    }

    /// Record cache metrics
    pub fn record_cache_operation(&self, operation: &str, cache_type: &str, hit: bool) {
        let status = if hit { "hit" } else { "miss" };
        self.cache_operations_total
            .with_label_values(&[operation, cache_type, status])
            .inc();
    }

    pub fn set_cache_hit_ratio(&self, ratio: f64) {
        self.cache_hit_ratio.set(ratio);
    }

    pub fn set_cache_memory_usage(&self, bytes: i64) {
        self.cache_memory_usage.set(bytes);
    }

    pub fn record_cache_eviction(&self) {
        self.cache_evictions_total.inc();
    }

    /// Update system metrics
    pub fn set_memory_usage(&self, bytes: i64) {
        self.memory_usage_bytes.set(bytes);
    }

    pub fn set_cpu_usage(&self, percent: f64) {
        self.cpu_usage_percent.set(percent);
    }

    pub fn set_disk_usage(&self, mount_point: &str, device: &str, bytes: i64) {
        self.disk_usage_bytes
            .with_label_values(&[mount_point, device])
            .set(bytes);
    }

    pub fn record_network_bytes(&self, direction: &str, interface: &str, bytes: u64) {
        self.network_bytes_total
            .with_label_values(&[direction, interface])
            .inc_by(bytes);
    }

    /// Update business metrics
    pub fn set_active_users(&self, count: i64) {
        self.active_users.set(count);
    }

    pub fn record_transaction_volume(&self, vm_type: &str, transaction_type: &str, count: u64) {
        self.transaction_volume_total
            .with_label_values(&[vm_type, transaction_type])
            .inc_by(count);
    }

    pub fn record_revenue(&self, amount: f64) {
        self.revenue_total.inc_by(amount);
    }

    pub fn record_api_key_usage(&self, api_key_id: &str, endpoint: &str) {
        self.api_key_usage
            .with_label_values(&[api_key_id, endpoint])
            .inc();
    }

    /// Update performance metrics
    pub fn set_throughput(&self, vm_type: &str, operation: &str, tps: f64) {
        self.throughput_tps
            .with_label_values(&[vm_type, operation])
            .set(tps);
    }

    pub fn record_latency_percentile(&self, operation: &str, percentile: &str, latency: Duration) {
        self.latency_percentiles
            .with_label_values(&[operation, percentile])
            .observe(latency.as_secs_f64());
    }

    pub fn set_error_rate(&self, component: &str, error_type: &str, rate: f64) {
        self.error_rate
            .with_label_values(&[component, error_type])
            .set(rate);
    }

    pub fn set_availability(&self, ratio: f64) {
        self.availability_ratio.set(ratio);
    }

    /// Record security metrics
    pub fn record_auth_attempt(&self, method: &str, success: bool, source: &str) {
        let status = if success { "success" } else { "failure" };
        self.auth_attempts_total
            .with_label_values(&[method, status, source])
            .inc();
    }

    pub fn record_rate_limit_hit(&self, endpoint: &str, limit_type: &str) {
        self.rate_limit_hits
            .with_label_values(&[endpoint, limit_type])
            .inc();
    }

    pub fn record_security_event(&self, event_type: &str, severity: &str) {
        self.security_events
            .with_label_values(&[event_type, severity])
            .inc();
    }

    /// Get system uptime
    pub fn uptime_seconds(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Update metrics timestamp
    pub async fn update_timestamp(&self) {
        let mut last_update = self.last_metrics_update.write().await;
        *last_update = Instant::now();
    }

    /// Get metrics in Prometheus format
    pub fn export_metrics(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode_to_string(&metric_families)
            .unwrap_or_default()
    }
}

impl From<prometheus::Error> for ApplicationError {
    fn from(err: prometheus::Error) -> Self {
        ApplicationError::MonitoringError {
            component: "prometheus".to_string(),
            message: err.to_string(),
        }
    }
}
