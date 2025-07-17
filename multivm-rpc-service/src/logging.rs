//! Production-ready logging configuration

use tracing::{Level, Subscriber};
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
    EnvFilter,
    Registry,
};
use std::io;

/// Custom serialization for tracing::Level
mod level_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use tracing::Level;

    pub fn serialize<S>(level: &Level, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        level.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Level, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(Level::TRACE),
            "DEBUG" => Ok(Level::DEBUG),
            "INFO" => Ok(Level::INFO),
            "WARN" => Ok(Level::WARN),
            "ERROR" => Ok(Level::ERROR),
            _ => Err(serde::de::Error::custom(format!("Invalid log level: {}", s))),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(with = "level_serde")]
    pub level: Level,
    /// Enable JSON formatting
    pub json_format: bool,
    /// Enable ANSI colors
    pub ansi: bool,
    /// Target for logs (console, file, or both)
    pub target: LogTarget,
    /// File path if logging to file
    pub file_path: Option<String>,
    /// Enable request tracing
    pub enable_request_tracing: bool,
    /// Enable performance metrics
    pub enable_metrics: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LogTarget {
    Console,
    File,
    Both,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            json_format: false,
            ansi: true,
            target: LogTarget::Console,
            file_path: None,
            enable_request_tracing: true,
            enable_metrics: true,
        }
    }
}

impl LoggingConfig {
    /// Production configuration
    pub fn production() -> Self {
        Self {
            level: Level::INFO,
            json_format: true,
            ansi: false,
            target: LogTarget::Both,
            file_path: Some("/var/log/multivm-rpc/service.log".to_string()),
            enable_request_tracing: true,
            enable_metrics: true,
        }
    }

    /// Development configuration
    pub fn development() -> Self {
        Self {
            level: Level::DEBUG,
            json_format: false,
            ansi: true,
            target: LogTarget::Console,
            file_path: None,
            enable_request_tracing: true,
            enable_metrics: false,
        }
    }

    /// Initialize logging with this configuration
    pub fn init(self) -> Result<(), Box<dyn std::error::Error>> {
        // Skip initialization in tests if global subscriber is already set
        #[cfg(test)]
        {
            if tracing::dispatcher::has_been_set() {
                return Ok(());
            }
        }
        let env_filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new(&format!("multivm_rpc_service={}", self.level)))
            .unwrap();

        let registry = Registry::default().with(env_filter);

        match self.target {
            LogTarget::Console => {
                if self.json_format {
                    registry
                        .with(fmt::layer().json().with_writer(io::stdout))
                        .init();
                } else {
                    registry
                        .with(
                            fmt::layer()
                                .with_ansi(self.ansi)
                                .with_writer(io::stdout)
                        )
                        .init();
                }
            }
            LogTarget::File => {
                if let Some(ref path) = self.file_path {
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)?;
                    
                    registry
                        .with(
                            fmt::layer()
                                .json()
                                .with_writer(file)
                        )
                        .init();
                } else {
                    return Err("File path required for file logging".into());
                }
            }
            LogTarget::Both => {
                // Console layer
                let console_layer = if self.json_format {
                    fmt::layer().json().with_writer(io::stdout).boxed()
                } else {
                    fmt::layer()
                        .with_ansi(self.ansi)
                        .with_writer(io::stdout)
                        .boxed()
                };

                // File layer
                if let Some(ref path) = self.file_path {
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)?;
                    
                    let file_layer = fmt::layer()
                        .json()
                        .with_writer(file)
                        .boxed();

                    registry
                        .with(console_layer)
                        .with(file_layer)
                        .init();
                } else {
                    registry.with(console_layer).init();
                }
            }
        }

        Ok(())
    }
}

/// Request tracing middleware
pub mod request_tracing {
    use axum::{
        extract::{Request, State},
        middleware::Next,
        response::Response,
    };
    use std::time::SystemTime;
    use tracing::{info, warn, Span};
    use uuid::Uuid;

    #[derive(Clone)]
    pub struct TracingConfig {
        pub log_requests: bool,
        pub log_responses: bool,
        pub log_slow_requests: bool,
        pub slow_request_threshold_ms: u64,
    }

    impl Default for TracingConfig {
        fn default() -> Self {
            Self {
                log_requests: true,
                log_responses: false,
                log_slow_requests: true,
                slow_request_threshold_ms: 1000,
            }
        }
    }

    /// Request tracing middleware
    pub async fn request_tracing_middleware(
        State(config): State<TracingConfig>,
        mut request: Request,
        next: Next,
    ) -> Response {
        let request_id = Uuid::new_v4().to_string();
        let start_time = SystemTime::now();
        let method = request.method().clone();
        let uri = request.uri().clone();
        let version = request.version();

        // Add request ID to request extensions
        request.extensions_mut().insert(request_id.clone());

        let span = tracing::info_span!(
            "http_request",
            request_id = %request_id,
            method = %method,
            uri = %uri,
            version = ?version,
        );

        let _guard = span.enter();

        if config.log_requests {
            info!(
                request_id = %request_id,
                method = %method,
                uri = %uri,
                "Processing request"
            );
        }

        let response = next.run(request).await;
        
        let duration = start_time.elapsed().unwrap_or_default();
        let status = response.status();

        if config.log_slow_requests && duration.as_millis() > config.slow_request_threshold_ms as u128 {
            warn!(
                request_id = %request_id,
                method = %method,
                uri = %uri,
                status = %status,
                duration_ms = duration.as_millis(),
                "Slow request detected"
            );
        } else if config.log_responses {
            info!(
                request_id = %request_id,
                method = %method,
                uri = %uri,
                status = %status,
                duration_ms = duration.as_millis(),
                "Request completed"
            );
        }

        response
    }
}

/// Performance monitoring
pub mod metrics {
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
        time::{Duration, SystemTime},
    };
    use tracing::{info, warn};

    #[derive(Debug, Clone)]
    pub struct PerformanceMetrics {
        pub request_count: u64,
        pub error_count: u64,
        pub total_duration: Duration,
        pub min_duration: Duration,
        pub max_duration: Duration,
        pub last_updated: SystemTime,
    }

    impl Default for PerformanceMetrics {
        fn default() -> Self {
            Self {
                request_count: 0,
                error_count: 0,
                total_duration: Duration::ZERO,
                min_duration: Duration::MAX,
                max_duration: Duration::ZERO,
                last_updated: SystemTime::now(),
            }
        }
    }

    impl PerformanceMetrics {
        pub fn record_request(&mut self, duration: Duration, is_error: bool) {
            self.request_count += 1;
            if is_error {
                self.error_count += 1;
            }
            
            self.total_duration += duration;
            
            if duration < self.min_duration {
                self.min_duration = duration;
            }
            if duration > self.max_duration {
                self.max_duration = duration;
            }
            
            self.last_updated = SystemTime::now();
        }

        pub fn average_duration(&self) -> Duration {
            if self.request_count > 0 {
                self.total_duration / self.request_count as u32
            } else {
                Duration::ZERO
            }
        }

        pub fn error_rate(&self) -> f64 {
            if self.request_count > 0 {
                self.error_count as f64 / self.request_count as f64
            } else {
                0.0
            }
        }
    }

    #[derive(Default)]
    pub struct MetricsCollector {
        method_metrics: Arc<RwLock<HashMap<String, PerformanceMetrics>>>,
        global_metrics: Arc<RwLock<PerformanceMetrics>>,
    }

    impl MetricsCollector {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn record(&self, method: &str, duration: Duration, is_error: bool) {
            // Update method-specific metrics
            {
                let mut method_metrics = self.method_metrics.write().unwrap();
                method_metrics
                    .entry(method.to_string())
                    .or_default()
                    .record_request(duration, is_error);
            }

            // Update global metrics
            {
                let mut global_metrics = self.global_metrics.write().unwrap();
                global_metrics.record_request(duration, is_error);
            }
        }

        pub fn get_method_metrics(&self, method: &str) -> Option<PerformanceMetrics> {
            self.method_metrics
                .read()
                .unwrap()
                .get(method)
                .cloned()
        }

        pub fn get_global_metrics(&self) -> PerformanceMetrics {
            self.global_metrics.read().unwrap().clone()
        }

        pub fn get_all_metrics(&self) -> HashMap<String, PerformanceMetrics> {
            self.method_metrics.read().unwrap().clone()
        }

        /// Log metrics summary
        pub fn log_metrics_summary(&self) {
            let global = self.get_global_metrics();
            let all_methods = self.get_all_metrics();

            info!(
                total_requests = global.request_count,
                total_errors = global.error_count,
                error_rate = global.error_rate(),
                avg_duration_ms = global.average_duration().as_millis(),
                min_duration_ms = global.min_duration.as_millis(),
                max_duration_ms = global.max_duration.as_millis(),
                "Global metrics summary"
            );

            for (method, metrics) in all_methods {
                if metrics.request_count > 0 {
                    info!(
                        method = %method,
                        requests = metrics.request_count,
                        errors = metrics.error_count,
                        error_rate = metrics.error_rate(),
                        avg_duration_ms = metrics.average_duration().as_millis(),
                        "Method metrics"
                    );
                }
            }
        }

        /// Start periodic metrics logging
        pub fn start_periodic_logging(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
            let collector = self.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(interval);
                loop {
                    interval.tick().await;
                    collector.log_metrics_summary();
                }
            })
        }
    }

    impl Clone for MetricsCollector {
        fn clone(&self) -> Self {
            Self {
                method_metrics: Arc::clone(&self.method_metrics),
                global_metrics: Arc::clone(&self.global_metrics),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, Level::INFO);
        assert!(!config.json_format);
        assert!(config.ansi);
    }

    #[test]
    fn test_logging_config_production() {
        let config = LoggingConfig::production();
        assert_eq!(config.level, Level::INFO);
        assert!(config.json_format);
        assert!(!config.ansi);
        assert!(config.file_path.is_some());
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = metrics::MetricsCollector::new();
        
        collector.record("test_method", Duration::from_millis(100), false);
        collector.record("test_method", Duration::from_millis(200), true);
        
        let metrics = collector.get_method_metrics("test_method").unwrap();
        assert_eq!(metrics.request_count, 2);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(metrics.error_rate(), 0.5);
    }
}