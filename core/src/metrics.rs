//! Metrics collection and monitoring for MultiVM

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::RwLock;
use tracing::{debug, warn};
use sysinfo::{System, Disks, Networks};

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Timer(Duration),
}

/// Metric metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMetadata {
    pub name: String,
    pub description: String,
    pub unit: String,
    pub labels: HashMap<String, String>,
    pub timestamp: u64,
}

/// A complete metric with metadata and value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub metadata: MetricMetadata,
    pub value: MetricValue,
}

/// Metrics registry for collecting and storing metrics
pub struct MetricsRegistry {
    metrics: Arc<RwLock<HashMap<String, Metric>>>,
    _aggregators: Arc<RwLock<HashMap<String, Box<dyn MetricAggregator + Send + Sync>>>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            _aggregators: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a counter metric
    pub async fn counter(&self, name: &str, value: u64, labels: HashMap<String, String>) {
        let key = format!("{}_{}", name, labels_to_string(&labels));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metric = Metric {
            metadata: MetricMetadata {
                name: name.to_string(),
                description: format!("Counter metric: {}", name),
                unit: "count".to_string(),
                labels,
                timestamp,
            },
            value: MetricValue::Counter(value),
        };

        let mut metrics = self.metrics.write().await;
        if let Some(existing) = metrics.get(&key) {
            if let MetricValue::Counter(existing_value) = &existing.value {
                let mut new_metric = metric;
                new_metric.value = MetricValue::Counter(existing_value + value);
                metrics.insert(key, new_metric);
            }
        } else {
            metrics.insert(key, metric);
        }
    }

    /// Record a gauge metric
    pub async fn gauge(&self, name: &str, value: f64, labels: HashMap<String, String>) {
        let key = format!("{}_{}", name, labels_to_string(&labels));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metric = Metric {
            metadata: MetricMetadata {
                name: name.to_string(),
                description: format!("Gauge metric: {}", name),
                unit: "value".to_string(),
                labels,
                timestamp,
            },
            value: MetricValue::Gauge(value),
        };

        self.metrics.write().await.insert(key, metric);
    }

    /// Record a histogram metric
    pub async fn histogram(&self, name: &str, value: f64, labels: HashMap<String, String>) {
        let key = format!("{}_{}", name, labels_to_string(&labels));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut metrics = self.metrics.write().await;
        if let Some(existing) = metrics.get_mut(&key) {
            if let MetricValue::Histogram(ref mut values) = &mut existing.value {
                values.push(value);
                existing.metadata.timestamp = timestamp;
            }
        } else {
            let metric = Metric {
                metadata: MetricMetadata {
                    name: name.to_string(),
                    description: format!("Histogram metric: {}", name),
                    unit: "value".to_string(),
                    labels,
                    timestamp,
                },
                value: MetricValue::Histogram(vec![value]),
            };
            metrics.insert(key, metric);
        }
    }

    /// Record a timer metric
    pub async fn timer(&self, name: &str, duration: Duration, labels: HashMap<String, String>) {
        let key = format!("{}_{}", name, labels_to_string(&labels));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metric = Metric {
            metadata: MetricMetadata {
                name: name.to_string(),
                description: format!("Timer metric: {}", name),
                unit: "duration".to_string(),
                labels,
                timestamp,
            },
            value: MetricValue::Timer(duration),
        };

        self.metrics.write().await.insert(key, metric);
    }

    /// Get all metrics
    pub async fn get_all_metrics(&self) -> Vec<Metric> {
        self.metrics.read().await.values().cloned().collect()
    }

    /// Get metrics by name prefix
    pub async fn get_metrics_by_prefix(&self, prefix: &str) -> Vec<Metric> {
        self.metrics.read().await
            .values()
            .filter(|m| m.metadata.name.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Clear old metrics
    pub async fn clear_old_metrics(&self, max_age: Duration) {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - max_age.as_secs();

        let mut metrics = self.metrics.write().await;
        let before_count = metrics.len();
        metrics.retain(|_, metric| metric.metadata.timestamp > cutoff);
        let after_count = metrics.len();

        if before_count != after_count {
            debug!("Cleared {} old metrics", before_count - after_count);
        }
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(&self, interval: Duration, max_age: Duration) {
        let metrics = self.metrics.clone();
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(interval);
            loop {
                cleanup_interval.tick().await;
                
                let cutoff = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() - max_age.as_secs();

                let mut metrics = metrics.write().await;
                let before_count = metrics.len();
                metrics.retain(|_, metric| metric.metadata.timestamp > cutoff);
                let after_count = metrics.len();

                if before_count != after_count {
                    debug!("Cleaned up {} old metrics", before_count - after_count);
                }
            }
        });
    }
}

/// Trait for metric aggregation
pub trait MetricAggregator {
    fn aggregate(&self, metrics: &[Metric]) -> Option<Metric>;
}

/// Average aggregator
pub struct AverageAggregator;

impl MetricAggregator for AverageAggregator {
    fn aggregate(&self, metrics: &[Metric]) -> Option<Metric> {
        if metrics.is_empty() {
            return None;
        }

        let mut sum = 0.0;
        let mut count = 0;

        for metric in metrics {
            match &metric.value {
                MetricValue::Gauge(value) => {
                    sum += value;
                    count += 1;
                }
                MetricValue::Histogram(values) => {
                    sum += values.iter().sum::<f64>();
                    count += values.len();
                }
                _ => continue,
            }
        }

        if count == 0 {
            return None;
        }

        let avg = sum / count as f64;
        let mut result = metrics[0].clone();
        result.value = MetricValue::Gauge(avg);
        result.metadata.name = format!("{}_avg", result.metadata.name);
        Some(result)
    }
}

/// Sum aggregator
pub struct SumAggregator;

impl MetricAggregator for SumAggregator {
    fn aggregate(&self, metrics: &[Metric]) -> Option<Metric> {
        if metrics.is_empty() {
            return None;
        }

        let mut sum = 0.0;

        for metric in metrics {
            match &metric.value {
                MetricValue::Counter(value) => sum += *value as f64,
                MetricValue::Gauge(value) => sum += value,
                MetricValue::Histogram(values) => sum += values.iter().sum::<f64>(),
                _ => continue,
            }
        }

        let mut result = metrics[0].clone();
        result.value = MetricValue::Gauge(sum);
        result.metadata.name = format!("{}_sum", result.metadata.name);
        Some(result)
    }
}

/// System metrics collector
pub struct SystemMetricsCollector {
    registry: Arc<MetricsRegistry>,
    system: Arc<RwLock<System>>,
}

impl SystemMetricsCollector {
    /// Create new system metrics collector
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self { 
            registry,
            system: Arc::new(RwLock::new(System::new_all())),
        }
    }

    /// Collect system metrics
    pub async fn collect_system_metrics(&self) -> crate::Result<()> {
        let labels = HashMap::new();
        
        // Refresh system information
        {
            let mut system = self.system.write().await;
            system.refresh_all();
        }
        
        let system = self.system.read().await;

        // CPU usage
        if let Ok(cpu_usage) = get_cpu_usage(&system).await {
            self.registry.gauge("system_cpu_usage", cpu_usage, labels.clone()).await;
        }

        // Memory usage
        if let Ok(memory_info) = get_memory_info(&system).await {
            self.registry.gauge("system_memory_total", memory_info.total as f64, labels.clone()).await;
            self.registry.gauge("system_memory_used", memory_info.used as f64, labels.clone()).await;
            self.registry.gauge("system_memory_available", memory_info.available as f64, labels.clone()).await;
        }

        // Disk usage
        if let Ok(disk_info) = get_disk_info(&system).await {
            self.registry.gauge("system_disk_total", disk_info.total as f64, labels.clone()).await;
            self.registry.gauge("system_disk_used", disk_info.used as f64, labels.clone()).await;
            self.registry.gauge("system_disk_available", disk_info.available as f64, labels.clone()).await;
        }

        // Network stats
        if let Ok(network_stats) = get_network_stats(&system).await {
            self.registry.counter("system_network_bytes_sent", network_stats.bytes_sent, labels.clone()).await;
            self.registry.counter("system_network_bytes_received", network_stats.bytes_received, labels.clone()).await;
            self.registry.counter("system_network_packets_sent", network_stats.packets_sent, labels.clone()).await;
            self.registry.counter("system_network_packets_received", network_stats.packets_received, labels).await;
        }

        Ok(())
    }

    /// Start automatic collection
    pub fn start_collection(&self, interval: Duration) {
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let mut collection_interval = tokio::time::interval(interval);
            loop {
                collection_interval.tick().await;
                
                let collector = SystemMetricsCollector::new(registry.clone());
                if let Err(e) = collector.collect_system_metrics().await {
                    warn!("Failed to collect system metrics: {}", e);
                }
            }
        });
    }
}

/// Memory information
#[derive(Debug)]
struct MemoryInfo {
    total: u64,
    used: u64,
    available: u64,
}

/// Disk information
#[derive(Debug)]
struct DiskInfo {
    total: u64,
    used: u64,
    available: u64,
}

/// Network statistics
#[derive(Debug)]
struct NetworkStats {
    bytes_sent: u64,
    bytes_received: u64,
    packets_sent: u64,
    packets_received: u64,
}

/// Get CPU usage percentage
async fn get_cpu_usage(system: &System) -> crate::Result<f64> {
    let cpu_usage = system.global_cpu_info().cpu_usage();
    Ok(cpu_usage as f64)
}

/// Get memory information
async fn get_memory_info(system: &System) -> crate::Result<MemoryInfo> {
    let total_memory = system.total_memory();
    let used_memory = system.used_memory();
    let available_memory = system.available_memory();
    
    Ok(MemoryInfo {
        total: total_memory,
        used: used_memory,
        available: available_memory,
    })
}

/// Get disk information
async fn get_disk_info(_system: &System) -> crate::Result<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut total_space: u64 = 0;
    let mut available_space: u64 = 0;
    
    for disk in &disks {
        total_space += disk.total_space();
        available_space += disk.available_space();
    }
    
    let used_space = total_space.saturating_sub(available_space);
    
    Ok(DiskInfo {
        total: total_space,
        used: used_space,
        available: available_space,
    })
}

/// Get network statistics
async fn get_network_stats(_system: &System) -> crate::Result<NetworkStats> {
    let networks = Networks::new_with_refreshed_list();
    let mut bytes_sent = 0;
    let mut bytes_received = 0;
    let mut packets_sent = 0;
    let mut packets_received = 0;
    
    for (_, network) in &networks {
        bytes_sent += network.total_transmitted();
        bytes_received += network.total_received();
        packets_sent += network.total_packets_transmitted();
        packets_received += network.total_packets_received();
    }
    
    Ok(NetworkStats {
        bytes_sent,
        bytes_received,
        packets_sent,
        packets_received,
    })
}

/// Convert labels to string for key generation
fn labels_to_string(labels: &HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = labels.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);
    pairs.into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(",")
}

/// Metrics exporter trait
pub trait MetricsExporter {
    fn export(&self, metrics: &[Metric]) -> String;
}

/// Prometheus format exporter
pub struct PrometheusExporter;

impl MetricsExporter for PrometheusExporter {
    fn export(&self, metrics: &[Metric]) -> String {
        let mut output = String::new();
        
        for metric in metrics {
            let labels_str = if metric.metadata.labels.is_empty() {
                String::new()
            } else {
                let labels: Vec<String> = metric.metadata.labels.iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", labels.join(","))
            };

            match &metric.value {
                MetricValue::Counter(value) => {
                    output.push_str(&format!("{}{} {}\n", metric.metadata.name, labels_str, value));
                }
                MetricValue::Gauge(value) => {
                    output.push_str(&format!("{}{} {}\n", metric.metadata.name, labels_str, value));
                }
                MetricValue::Histogram(values) => {
                    for value in values {
                        output.push_str(&format!("{}_bucket{} {}\n", metric.metadata.name, labels_str, value));
                    }
                }
                MetricValue::Timer(duration) => {
                    output.push_str(&format!("{}{} {}\n", metric.metadata.name, labels_str, duration.as_secs_f64()));
                }
            }
        }
        
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_registry() {
        let registry = MetricsRegistry::new();
        let labels = HashMap::new();

        registry.counter("test_counter", 5, labels.clone()).await;
        registry.gauge("test_gauge", 42.5, labels.clone()).await;
        registry.histogram("test_histogram", 1.5, labels.clone()).await;
        registry.timer("test_timer", Duration::from_millis(100), labels).await;

        let metrics = registry.get_all_metrics().await;
        assert_eq!(metrics.len(), 4);
    }

    #[tokio::test]
    async fn test_system_metrics_collector() {
        let registry = Arc::new(MetricsRegistry::new());
        let collector = SystemMetricsCollector::new(registry.clone());
        
        collector.collect_system_metrics().await.unwrap();
        
        let metrics = registry.get_all_metrics().await;
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_prometheus_exporter() {
        let metrics = vec![
            Metric {
                metadata: MetricMetadata {
                    name: "test_counter".to_string(),
                    description: "Test counter".to_string(),
                    unit: "count".to_string(),
                    labels: HashMap::new(),
                    timestamp: 0,
                },
                value: MetricValue::Counter(42),
            }
        ];

        let exporter = PrometheusExporter;
        let output = exporter.export(&metrics);
        assert!(output.contains("test_counter 42"));
    }
}