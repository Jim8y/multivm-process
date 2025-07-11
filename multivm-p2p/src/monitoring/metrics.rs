//! Metrics for P2P networking layer

#[cfg(feature = "metrics")]
use once_cell::sync::Lazy;
#[cfg(feature = "metrics")]
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec, Registry,
};

/// Total messages sent by type
#[cfg(feature = "metrics")]
pub static MESSAGES_SENT: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "multivm_p2p_messages_sent_total",
        "Total number of messages sent",
        &["message_type", "target_type"]
    )
    .unwrap()
});

/// Total messages received by type
#[cfg(feature = "metrics")]
pub static MESSAGES_RECEIVED: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "multivm_p2p_messages_received_total",
        "Total number of messages received",
        &["message_type", "source_peer"]
    )
    .unwrap()
});

/// Message processing duration
#[cfg(feature = "metrics")]
pub static MESSAGE_PROCESSING_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "multivm_p2p_message_processing_duration_seconds",
        "Message processing duration in seconds",
        &["message_type"],
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
    )
    .unwrap()
});

/// Active peer connections
#[cfg(feature = "metrics")]
pub static ACTIVE_PEERS: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "multivm_p2p_active_peers",
        "Number of active peer connections",
        &["peer_type", "connection_status"]
    )
    .unwrap()
});

/// Network bandwidth
#[cfg(feature = "metrics")]
pub static NETWORK_BANDWIDTH: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "multivm_p2p_bandwidth_bytes_per_second",
        "Network bandwidth in bytes per second",
        &["direction", "protocol"]
    )
    .unwrap()
});

/// Protocol translation metrics
#[cfg(feature = "metrics")]
pub static PROTOCOL_TRANSLATIONS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "multivm_p2p_protocol_translations_total",
        "Total number of protocol translations",
        &["source_vm", "target_vm", "status"]
    )
    .unwrap()
});

/// Routing metrics
#[cfg(feature = "metrics")]
pub static ROUTING_DECISIONS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "multivm_p2p_routing_decisions_total",
        "Total number of routing decisions",
        &["strategy", "message_type", "result"]
    )
    .unwrap()
});

/// Discovery metrics
#[cfg(feature = "metrics")]
pub static PEER_DISCOVERIES: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "multivm_p2p_peer_discoveries_total",
        "Total number of peer discoveries",
        &["discovery_method", "result"]
    )
    .unwrap()
});

/// Network health status
#[cfg(feature = "metrics")]
pub static NETWORK_HEALTH: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "multivm_p2p_network_health",
        "Network health status (0=critical, 1=warning, 2=healthy)",
        &["component"]
    )
    .unwrap()
});

/// Connection pool metrics
#[cfg(feature = "metrics")]
pub static CONNECTION_POOL: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "multivm_p2p_connection_pool",
        "Connection pool statistics",
        &["pool_type", "status"]
    )
    .unwrap()
});

/// Message queue depth
#[cfg(feature = "metrics")]
pub static MESSAGE_QUEUE_DEPTH: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "multivm_p2p_message_queue_depth",
        "Current depth of message queues",
        &["queue_type", "priority"]
    )
    .unwrap()
});

/// Error metrics
#[cfg(feature = "metrics")]
pub static NETWORK_ERRORS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "multivm_p2p_errors_total",
        "Total number of network errors",
        &["error_type", "severity"]
    )
    .unwrap()
});

/// Initialize all metrics
#[cfg(feature = "metrics")]
pub fn init_metrics() -> Registry {
    let registry = Registry::new();

    // Register all metrics
    registry.register(Box::new(MESSAGES_SENT.clone())).unwrap();
    registry
        .register(Box::new(MESSAGES_RECEIVED.clone()))
        .unwrap();
    registry
        .register(Box::new(MESSAGE_PROCESSING_DURATION.clone()))
        .unwrap();
    registry.register(Box::new(ACTIVE_PEERS.clone())).unwrap();
    registry
        .register(Box::new(NETWORK_BANDWIDTH.clone()))
        .unwrap();
    registry
        .register(Box::new(PROTOCOL_TRANSLATIONS.clone()))
        .unwrap();
    registry
        .register(Box::new(ROUTING_DECISIONS.clone()))
        .unwrap();
    registry
        .register(Box::new(PEER_DISCOVERIES.clone()))
        .unwrap();
    registry.register(Box::new(NETWORK_HEALTH.clone())).unwrap();
    registry
        .register(Box::new(CONNECTION_POOL.clone()))
        .unwrap();
    registry
        .register(Box::new(MESSAGE_QUEUE_DEPTH.clone()))
        .unwrap();
    registry.register(Box::new(NETWORK_ERRORS.clone())).unwrap();

    registry
}

/// Record message sent
#[cfg(feature = "metrics")]
pub fn record_message_sent(message_type: &str, target_type: &str) {
    MESSAGES_SENT
        .with_label_values(&[message_type, target_type])
        .inc();
}

/// Record message received
#[cfg(feature = "metrics")]
pub fn record_message_received(message_type: &str, source_peer: &str) {
    MESSAGES_RECEIVED
        .with_label_values(&[message_type, source_peer])
        .inc();
}

/// Record message processing duration
#[cfg(feature = "metrics")]
pub fn record_processing_duration(message_type: &str, duration: f64) {
    MESSAGE_PROCESSING_DURATION
        .with_label_values(&[message_type])
        .observe(duration);
}

/// Update active peer count
#[cfg(feature = "metrics")]
pub fn update_active_peers(peer_type: &str, connection_status: &str, count: f64) {
    ACTIVE_PEERS
        .with_label_values(&[peer_type, connection_status])
        .set(count);
}

/// Update network bandwidth
#[cfg(feature = "metrics")]
pub fn update_bandwidth(direction: &str, protocol: &str, bytes_per_sec: f64) {
    NETWORK_BANDWIDTH
        .with_label_values(&[direction, protocol])
        .set(bytes_per_sec);
}

/// Record protocol translation
#[cfg(feature = "metrics")]
pub fn record_protocol_translation(source_vm: &str, target_vm: &str, status: &str) {
    PROTOCOL_TRANSLATIONS
        .with_label_values(&[source_vm, target_vm, status])
        .inc();
}

/// Record routing decision
#[cfg(feature = "metrics")]
pub fn record_routing_decision(strategy: &str, message_type: &str, result: &str) {
    ROUTING_DECISIONS
        .with_label_values(&[strategy, message_type, result])
        .inc();
}

/// Record peer discovery
#[cfg(feature = "metrics")]
pub fn record_peer_discovery(method: &str, result: &str) {
    PEER_DISCOVERIES.with_label_values(&[method, result]).inc();
}

/// Update network health status
#[cfg(feature = "metrics")]
pub fn update_network_health(component: &str, status: f64) {
    NETWORK_HEALTH.with_label_values(&[component]).set(status);
}

/// Update connection pool metrics
#[cfg(feature = "metrics")]
pub fn update_connection_pool(pool_type: &str, status: &str, count: f64) {
    CONNECTION_POOL
        .with_label_values(&[pool_type, status])
        .set(count);
}

/// Update message queue depth
#[cfg(feature = "metrics")]
pub fn update_queue_depth(queue_type: &str, priority: &str, depth: f64) {
    MESSAGE_QUEUE_DEPTH
        .with_label_values(&[queue_type, priority])
        .set(depth);
}

/// Record network error
#[cfg(feature = "metrics")]
pub fn record_network_error(error_type: &str, severity: &str) {
    NETWORK_ERRORS
        .with_label_values(&[error_type, severity])
        .inc();
}

// No-op implementations when metrics feature is not enabled
#[cfg(not(feature = "metrics"))]
pub fn record_message_sent(_message_type: &str, _target_type: &str) {
    // Production implementation without prometheus could:
    // - Log metrics to structured logs for external collection
    // - Maintain in-memory counters for basic monitoring
    // - Write to time-series database directly
}

#[cfg(not(feature = "metrics"))]
pub fn record_message_received(_message_type: &str, _source_peer: &str) {
    // Production alternative: use structured logging for metrics collection
    tracing::debug!(
        message_type = _message_type,
        source_peer = _source_peer,
        "message_received"
    );
}

#[cfg(not(feature = "metrics"))]
pub fn record_processing_duration(_message_type: &str, _duration: f64) {
    // Production alternative: log performance metrics for analysis
    if _duration > 1.0 {
        tracing::warn!(
            message_type = _message_type,
            duration_seconds = _duration,
            "slow_message_processing"
        );
    }
}

#[cfg(not(feature = "metrics"))]
pub fn update_active_peers(_peer_type: &str, _connection_status: &str, _count: f64) {
    // Production alternative: maintain in-memory counters or log significant changes
    tracing::info!(
        peer_type = _peer_type,
        status = _connection_status,
        count = _count,
        "peer_count_update"
    );
}

#[cfg(not(feature = "metrics"))]
pub fn update_bandwidth(_direction: &str, _protocol: &str, _bytes_per_sec: f64) {
    // Production alternative: log bandwidth usage for monitoring
    if _bytes_per_sec > 1_000_000.0 {
        // Log if > 1MB/s
        tracing::info!(
            direction = _direction,
            protocol = _protocol,
            bandwidth_mbps = _bytes_per_sec / 1_000_000.0,
            "high_bandwidth_usage"
        );
    }
}

#[cfg(not(feature = "metrics"))]
pub fn record_protocol_translation(_source_vm: &str, _target_vm: &str, _status: &str) {
    // Production alternative: log protocol translation events
    tracing::debug!(
        source_vm = _source_vm,
        target_vm = _target_vm,
        status = _status,
        "protocol_translation"
    );
}

#[cfg(not(feature = "metrics"))]
pub fn record_routing_decision(_strategy: &str, _message_type: &str, _result: &str) {
    // Production alternative: log routing decisions for analysis
    if _result == "failed" {
        tracing::warn!(
            strategy = _strategy,
            message_type = _message_type,
            result = _result,
            "routing_failed"
        );
    }
}

#[cfg(not(feature = "metrics"))]
pub fn record_peer_discovery(_method: &str, _result: &str) {
    // Production alternative: log peer discovery events
    tracing::info!(method = _method, result = _result, "peer_discovery");
}

#[cfg(not(feature = "metrics"))]
pub fn update_network_health(_component: &str, _status: f64) {
    // Production alternative: log health status changes
    let health_str = if _status >= 2.0 {
        "healthy"
    } else if _status >= 1.0 {
        "warning"
    } else {
        "critical"
    };
    tracing::info!(
        component = _component,
        status = health_str,
        "network_health"
    );
}

#[cfg(not(feature = "metrics"))]
pub fn update_connection_pool(_pool_type: &str, _status: &str, _count: f64) {
    // Production alternative: log connection pool status
    tracing::debug!(
        pool_type = _pool_type,
        status = _status,
        count = _count,
        "connection_pool"
    );
}

#[cfg(not(feature = "metrics"))]
pub fn update_queue_depth(_queue_type: &str, _priority: &str, _depth: f64) {
    // Production alternative: log queue depth for monitoring backpressure
    if _depth > 100.0 {
        tracing::warn!(
            queue_type = _queue_type,
            priority = _priority,
            depth = _depth,
            "high_queue_depth"
        );
    }
}

#[cfg(not(feature = "metrics"))]
pub fn record_network_error(_error_type: &str, _severity: &str) {
    // Production alternative: log network errors for analysis
    match _severity {
        "critical" => tracing::error!(error_type = _error_type, "critical_network_error"),
        "warning" => tracing::warn!(error_type = _error_type, "network_warning"),
        _ => tracing::debug!(
            error_type = _error_type,
            severity = _severity,
            "network_error"
        ),
    }
}

#[cfg(not(feature = "metrics"))]
pub struct Registry;

#[cfg(not(feature = "metrics"))]
pub fn init_metrics() -> Registry {
    Registry
}

/// Helper to convert health status to numeric value
pub fn health_status_to_metric(status: &crate::core::network::NetworkHealthStatus) -> f64 {
    match status {
        crate::core::network::NetworkHealthStatus::Healthy => 2.0,
        crate::core::network::NetworkHealthStatus::Warning => 1.0,
        crate::core::network::NetworkHealthStatus::Critical => 0.0,
    }
}
