//! Production monitoring and alerting for P2P network

use crate::error::P2PError;
use crate::network::NetworkHealthStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "metrics")]
mod metrics_impl {
    use super::*;
    use prometheus::{Counter, Gauge, Histogram, Registry};
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::RwLock;
    use tracing::{error, info, warn};

    /// Production monitoring system for P2P network
    #[derive(Debug)]
    pub struct P2PMonitor {
        /// Prometheus metrics registry
        registry: Arc<Registry>,
        /// Metrics collectors
        metrics: P2PMetrics,
        /// Alert configuration
        alert_config: AlertConfig,
        /// Alert state tracking
        alert_state: Arc<RwLock<AlertState>>,
        /// Performance baselines
        baselines: Arc<RwLock<PerformanceBaselines>>,
    }

    /// Prometheus metrics for P2P network
    #[derive(Debug)]
    pub struct P2PMetrics {
        // Connection metrics
        pub active_connections: Gauge,
        pub total_connections: Counter,
        pub failed_connections: Counter,
        pub connection_duration: Histogram,
        
        // Message metrics
        pub messages_sent: Counter,
        pub messages_received: Counter,
        pub message_processing_time: Histogram,
        pub message_size: Histogram,
        
        // Network health metrics
        pub network_health_score: Gauge,
        pub peer_reputation_avg: Gauge,
        pub bandwidth_utilization: Gauge,
        
        // Security metrics
        pub auth_failures: Counter,
        pub banned_peers: Gauge,
        pub rate_limit_violations: Counter,
        pub dos_attacks_detected: Counter,
        
        // Performance metrics
        pub cpu_usage: Gauge,
        pub memory_usage: Gauge,
        pub network_latency: Histogram,
        pub throughput: Gauge,
    }

    impl P2PMonitor {
        /// Create a new monitor with the given configuration
        pub fn new(config: AlertConfig) -> Result<Self, P2PError> {
            let registry = Arc::new(Registry::new());
            let metrics = Self::setup_metrics(&registry)?;
            
            Ok(Self {
                registry,
                metrics,
                alert_config: config,
                alert_state: Arc::new(RwLock::new(AlertState::default())),
                baselines: Arc::new(RwLock::new(PerformanceBaselines::default())),
            })
        }

        /// Setup Prometheus metrics
        fn setup_metrics(registry: &Arc<Registry>) -> Result<P2PMetrics, P2PError> {
            let metrics = P2PMetrics {
                active_connections: Gauge::new("p2p_active_connections", "Number of active P2P connections")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                total_connections: Counter::new("p2p_total_connections", "Total P2P connections made")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                failed_connections: Counter::new("p2p_failed_connections_total", "Failed connection attempts")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                connection_duration: Histogram::new("p2p_connection_duration_seconds", "Connection duration")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                messages_sent: Counter::new("p2p_messages_sent_total", "Messages sent")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                messages_received: Counter::new("p2p_messages_received_total", "Messages received")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                message_processing_time: Histogram::new("p2p_message_processing_seconds", "Message processing time")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                message_size: Histogram::new("p2p_message_size_bytes", "Message size in bytes")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                network_health_score: Gauge::new("p2p_network_health_score", "Network health score (0-1)")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                peer_reputation_avg: Gauge::new("p2p_peer_reputation_average", "Average peer reputation")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                bandwidth_utilization: Gauge::new("p2p_bandwidth_utilization", "Bandwidth utilization ratio")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                auth_failures: Counter::new("p2p_auth_failures_total", "Authentication failures")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                banned_peers: Gauge::new("p2p_banned_peers", "Number of banned peers")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                rate_limit_violations: Counter::new("p2p_rate_limit_violations_total", "Rate limit violations")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                dos_attacks_detected: Counter::new("p2p_dos_attacks_detected_total", "DoS attacks detected")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                cpu_usage: Gauge::new("p2p_cpu_usage_ratio", "CPU usage ratio")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                memory_usage: Gauge::new("p2p_memory_usage_bytes", "Memory usage in bytes")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                network_latency: Histogram::new("p2p_network_latency_seconds", "Network latency")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
                throughput: Gauge::new("p2p_throughput_messages_per_second", "Message throughput")
                    .map_err(|e| P2PError::ConfigurationError { 
                        message: format!("metrics: {}", e.to_string())
                    })?,
            };

            // Register all metrics
            registry.register(Box::new(metrics.active_connections.clone()))?;
            registry.register(Box::new(metrics.total_connections.clone()))?;
            registry.register(Box::new(metrics.failed_connections.clone()))?;
            registry.register(Box::new(metrics.connection_duration.clone()))?;
            registry.register(Box::new(metrics.messages_sent.clone()))?;
            registry.register(Box::new(metrics.messages_received.clone()))?;
            registry.register(Box::new(metrics.message_processing_time.clone()))?;
            registry.register(Box::new(metrics.message_size.clone()))?;
            registry.register(Box::new(metrics.network_health_score.clone()))?;
            registry.register(Box::new(metrics.peer_reputation_avg.clone()))?;
            registry.register(Box::new(metrics.bandwidth_utilization.clone()))?;
            registry.register(Box::new(metrics.auth_failures.clone()))?;
            registry.register(Box::new(metrics.banned_peers.clone()))?;
            registry.register(Box::new(metrics.rate_limit_violations.clone()))?;
            registry.register(Box::new(metrics.dos_attacks_detected.clone()))?;
            registry.register(Box::new(metrics.cpu_usage.clone()))?;
            registry.register(Box::new(metrics.memory_usage.clone()))?;
            registry.register(Box::new(metrics.network_latency.clone()))?;
            registry.register(Box::new(metrics.throughput.clone()))?;

            Ok(metrics)
        }

        /// Record a connection event
        pub fn record_connection_event(&self, _peer_id: &str, success: bool, duration: Duration) {
            self.metrics.total_connections.inc();
            if !success {
                self.metrics.failed_connections.inc();
            } else {
                self.metrics.active_connections.inc();
                self.metrics.connection_duration.observe(duration.as_secs_f64());
            }
        }

        /// Record a message event
        pub fn record_message_event(&self, message_type: &str, size: usize, processing_time: Duration) {
            self.metrics.messages_received.inc();
            self.metrics.message_size.observe(size as f64);
            self.metrics.message_processing_time.observe(processing_time.as_secs_f64());
        }

        /// Update network health metrics
        pub fn update_network_health(&self, status: &NetworkHealthStatus) {
            let score = match status {
                NetworkHealthStatus::Healthy => 1.0,
                NetworkHealthStatus::Degraded => 0.7,
                NetworkHealthStatus::Unhealthy => 0.3,
                NetworkHealthStatus::Critical => 0.0,
            };
            self.metrics.network_health_score.set(score);
        }

        /// Record a security event
        pub fn record_security_event(&self, event_type: &str, _peer_id: Option<&str>) {
            match event_type {
                "auth_failure" => self.metrics.auth_failures.inc(),
                "rate_limit_violation" => self.metrics.rate_limit_violations.inc(),
                "dos_attack" => self.metrics.dos_attacks_detected.inc(),
                _ => {}
            }
        }

        /// Update system metrics
        pub fn update_system_metrics(&self, cpu_usage: f64, memory_usage: u64) {
            self.metrics.cpu_usage.set(cpu_usage);
            self.metrics.memory_usage.set(memory_usage as f64);
        }

        /// Get metrics as Prometheus formatted string
        pub fn get_metrics_string(&self) -> String {
            use prometheus::Encoder;
            let encoder = prometheus::TextEncoder::new();
            let metric_families = self.registry.gather();
            encoder.encode_to_string(&metric_families).unwrap_or_default()
        }

        /// Send webhook alert
        async fn send_webhook_alert(&self, webhook_url: &str, alert: &Alert) -> Result<(), P2PError> {
            let client = reqwest::Client::new();
            let payload = serde_json::to_value(alert).map_err(|e| P2PError::InvalidMessage(
                format!("Failed to serialize alert: {}", e)
            ))?;

            let response = client
                .post(webhook_url)
                .json(&payload)
                .send()
                .await
                .map_err(|e| P2PError::ConnectionError {
                    message: format!("Failed to send webhook: {}", e),
                })?;

            if !response.status().is_success() {
                return Err(P2PError::ConnectionError {
                    message: format!("Webhook returned status: {}", response.status()),
                });
            }

            Ok(())
        }
    }

    /// Alert state tracking
    #[derive(Debug, Default)]
    pub(super) struct AlertState {
        last_alerts: HashMap<String, Instant>,
    }

    /// Performance baselines for anomaly detection
    #[derive(Debug, Default)]
    pub(super) struct PerformanceBaselines {
        baseline_cpu: f64,
        baseline_memory: u64,
        baseline_latency: Duration,
        baseline_throughput: f64,
    }

    // Re-export the implementation types
    pub use self::P2PMonitor;
}

#[cfg(not(feature = "metrics"))]
mod stub_impl {
    use super::*;

    /// Stub monitoring system for P2P network
    #[derive(Debug)]
    pub struct P2PMonitor {
        _placeholder: (),
    }

    impl P2PMonitor {
        /// Create a new stub monitor
        pub fn new(_config: AlertConfig) -> Result<Self, P2PError> {
            Ok(Self {
                _placeholder: (),
            })
        }

        /// Stub for recording connection event
        pub fn record_connection_event(&self, _peer_id: &str, _success: bool, _duration: Duration) {}

        /// Stub for recording message event
        pub fn record_message_event(&self, _message_type: &str, _size: usize, _processing_time: Duration) {}

        /// Stub for updating network health
        pub fn update_network_health(&self, _status: &NetworkHealthStatus) {}

        /// Stub for recording security event
        pub fn record_security_event(&self, _event_type: &str, _peer_id: Option<&str>) {}

        /// Stub for updating system metrics
        pub fn update_system_metrics(&self, _cpu_usage: f64, _memory_usage: u64) {}

        /// Stub for getting metrics as string
        pub fn get_metrics_string(&self) -> String {
            "# Metrics disabled\n".to_string()
        }
    }

    /// Stub alert state
    #[derive(Debug, Default)]
    pub(super) struct AlertState {}

    /// Stub performance baselines
    #[derive(Debug, Default)]
    pub(super) struct PerformanceBaselines {}
}

// Re-export the appropriate implementation
#[cfg(feature = "metrics")]
pub use metrics_impl::*;

#[cfg(not(feature = "metrics"))]
pub use stub_impl::*;

/// Alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub alert_cooldown: Duration,
    pub thresholds: AlertThresholds,
}

/// Alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub max_failed_connections: u64,
    pub max_auth_failures_per_minute: u64,
    pub min_health_score: f64,
    pub max_cpu_usage: f64,
    pub max_memory_usage: u64,
    pub max_network_latency_ms: u64,
}

/// Alert data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

/// Alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    NetworkHealth,
    HighCpuUsage,
    HighMemoryUsage,
    SecurityIncident,
    PerformanceDegradation,
    PeerConnectivity,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_url: None,
            alert_cooldown: Duration::from_secs(300),
            thresholds: AlertThresholds::default(),
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_failed_connections: 100,
            max_auth_failures_per_minute: 50,
            min_health_score: 0.5,
            max_cpu_usage: 0.9,
            max_memory_usage: 1024 * 1024 * 1024, // 1GB
            max_network_latency_ms: 1000,
        }
    }
}

/// Security event types for monitoring
#[derive(Debug, Clone)]
pub enum SecurityEventType {
    AuthFailure,
    PeerBanned,
    PeerUnbanned,
    RateLimitViolation,
    DosAttackDetected,
}