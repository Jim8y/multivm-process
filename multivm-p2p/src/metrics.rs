//! Metrics for P2P networking layer

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec, Registry,
};

lazy_static! {
    /// Total messages sent by type
    pub static ref MESSAGES_SENT: CounterVec = register_counter_vec!(
        "multivm_p2p_messages_sent_total",
        "Total number of messages sent",
        &["message_type", "target_type"]
    ).unwrap();

    /// Total messages received by type
    pub static ref MESSAGES_RECEIVED: CounterVec = register_counter_vec!(
        "multivm_p2p_messages_received_total",
        "Total number of messages received",
        &["message_type", "source_peer"]
    ).unwrap();

    /// Message processing duration
    pub static ref MESSAGE_PROCESSING_DURATION: HistogramVec = register_histogram_vec!(
        "multivm_p2p_message_processing_duration_seconds",
        "Message processing duration in seconds",
        &["message_type"],
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
    ).unwrap();

    /// Active peer connections
    pub static ref ACTIVE_PEERS: GaugeVec = register_gauge_vec!(
        "multivm_p2p_active_peers",
        "Number of active peer connections",
        &["peer_type", "connection_status"]
    ).unwrap();

    /// Network bandwidth
    pub static ref NETWORK_BANDWIDTH: GaugeVec = register_gauge_vec!(
        "multivm_p2p_bandwidth_bytes_per_second",
        "Network bandwidth in bytes per second",
        &["direction", "protocol"]
    ).unwrap();

    /// Protocol translation metrics
    pub static ref PROTOCOL_TRANSLATIONS: CounterVec = register_counter_vec!(
        "multivm_p2p_protocol_translations_total",
        "Total number of protocol translations",
        &["source_vm", "target_vm", "status"]
    ).unwrap();

    /// Routing metrics
    pub static ref ROUTING_DECISIONS: CounterVec = register_counter_vec!(
        "multivm_p2p_routing_decisions_total",
        "Total number of routing decisions",
        &["strategy", "message_type", "result"]
    ).unwrap();

    /// Discovery metrics
    pub static ref PEER_DISCOVERIES: CounterVec = register_counter_vec!(
        "multivm_p2p_peer_discoveries_total",
        "Total number of peer discoveries",
        &["discovery_method", "result"]
    ).unwrap();

    /// Network health status
    pub static ref NETWORK_HEALTH: GaugeVec = register_gauge_vec!(
        "multivm_p2p_network_health",
        "Network health status (0=critical, 1=warning, 2=healthy)",
        &["component"]
    ).unwrap();

    /// Connection pool metrics
    pub static ref CONNECTION_POOL: GaugeVec = register_gauge_vec!(
        "multivm_p2p_connection_pool",
        "Connection pool statistics",
        &["pool_type", "status"]
    ).unwrap();

    /// Message queue depth
    pub static ref MESSAGE_QUEUE_DEPTH: GaugeVec = register_gauge_vec!(
        "multivm_p2p_message_queue_depth",
        "Current depth of message queues",
        &["queue_type", "priority"]
    ).unwrap();

    /// Error metrics
    pub static ref NETWORK_ERRORS: CounterVec = register_counter_vec!(
        "multivm_p2p_errors_total",
        "Total number of network errors",
        &["error_type", "severity"]
    ).unwrap();
}

/// Initialize all metrics
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
pub fn record_message_sent(message_type: &str, target_type: &str) {
    MESSAGES_SENT
        .with_label_values(&[message_type, target_type])
        .inc();
}

/// Record message received
pub fn record_message_received(message_type: &str, source_peer: &str) {
    MESSAGES_RECEIVED
        .with_label_values(&[message_type, source_peer])
        .inc();
}

/// Record message processing duration
pub fn record_processing_duration(message_type: &str, duration: f64) {
    MESSAGE_PROCESSING_DURATION
        .with_label_values(&[message_type])
        .observe(duration);
}

/// Update active peer count
pub fn update_active_peers(peer_type: &str, connection_status: &str, count: f64) {
    ACTIVE_PEERS
        .with_label_values(&[peer_type, connection_status])
        .set(count);
}

/// Update network bandwidth
pub fn update_bandwidth(direction: &str, protocol: &str, bytes_per_sec: f64) {
    NETWORK_BANDWIDTH
        .with_label_values(&[direction, protocol])
        .set(bytes_per_sec);
}

/// Record protocol translation
pub fn record_protocol_translation(source_vm: &str, target_vm: &str, status: &str) {
    PROTOCOL_TRANSLATIONS
        .with_label_values(&[source_vm, target_vm, status])
        .inc();
}

/// Record routing decision
pub fn record_routing_decision(strategy: &str, message_type: &str, result: &str) {
    ROUTING_DECISIONS
        .with_label_values(&[strategy, message_type, result])
        .inc();
}

/// Record peer discovery
pub fn record_peer_discovery(method: &str, result: &str) {
    PEER_DISCOVERIES.with_label_values(&[method, result]).inc();
}

/// Update network health status
pub fn update_network_health(component: &str, status: f64) {
    NETWORK_HEALTH.with_label_values(&[component]).set(status);
}

/// Update connection pool metrics
pub fn update_connection_pool(pool_type: &str, status: &str, count: f64) {
    CONNECTION_POOL
        .with_label_values(&[pool_type, status])
        .set(count);
}

/// Update message queue depth
pub fn update_queue_depth(queue_type: &str, priority: &str, depth: f64) {
    MESSAGE_QUEUE_DEPTH
        .with_label_values(&[queue_type, priority])
        .set(depth);
}

/// Record network error
pub fn record_network_error(error_type: &str, severity: &str) {
    NETWORK_ERRORS
        .with_label_values(&[error_type, severity])
        .inc();
}

/// Helper to convert health status to numeric value
pub fn health_status_to_metric(status: &crate::network::NetworkHealthStatus) -> f64 {
    match status {
        crate::network::NetworkHealthStatus::Healthy => 2.0,
        crate::network::NetworkHealthStatus::Warning => 1.0,
        crate::network::NetworkHealthStatus::Critical => 0.0,
    }
}

