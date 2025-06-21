//! Centralized metrics collection for MultiVM consensus
//!
//! This module provides a unified metrics system that aggregates metrics
//! from all consensus components for monitoring and observability.

use crate::fork_detection::ForkDetectionMetrics;
use crate::malachite::ConsensusMetrics;
use crate::network_recovery::NetworkRecoveryMetrics;
use crate::state::PersistenceMetrics;
use crate::synchronization::BlockSyncMetrics;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Unified metrics collector for all consensus components
#[derive(Debug)]
pub struct ConsensusMetricsCollector {
    /// Consensus engine metrics
    consensus_metrics: Option<Arc<RwLock<ConsensusMetrics>>>,
    /// Fork detection metrics
    fork_metrics: Option<Arc<RwLock<ForkDetectionMetrics>>>,
    /// Network recovery metrics
    recovery_metrics: Option<Arc<RwLock<NetworkRecoveryMetrics>>>,
    /// Block synchronization metrics
    sync_metrics: Option<Arc<RwLock<BlockSyncMetrics>>>,
    /// State persistence metrics
    persistence_metrics: Option<Arc<RwLock<PersistenceMetrics>>>,
    /// Collection interval
    collection_interval: Duration,
    /// Last collection time
    last_collection: Arc<RwLock<Instant>>,
}

/// Aggregated metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    /// Timestamp of this snapshot
    pub timestamp: SystemTime,
    /// Overall system health
    pub system_health: SystemHealth,
    /// Consensus performance
    pub consensus_performance: ConsensusPerformance,
    /// Network status
    pub network_status: NetworkStatus,
    /// Synchronization status
    pub sync_status: SyncStatus,
    /// Error summary
    pub error_summary: ErrorSummary,
    /// Component-specific metrics
    pub component_metrics: ComponentMetrics,
}

/// System health indicator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SystemHealth {
    /// All components healthy
    Healthy,
    /// Some components degraded
    Degraded,
    /// Critical issues detected
    Critical,
    /// System unavailable
    Unavailable,
}

/// Consensus performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusPerformance {
    /// Current blockchain height
    pub current_height: u64,
    /// Blocks per second
    pub blocks_per_second: f64,
    /// Transactions per second
    pub transactions_per_second: f64,
    /// Average block time in milliseconds
    pub avg_block_time_ms: u64,
    /// Consensus participation rate (percentage)
    pub participation_rate: u8,
}

/// Network status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    /// Network health status
    pub health: String,
    /// Number of connected peers
    pub connected_peers: usize,
    /// Total known peers
    pub total_peers: usize,
    /// Active forks
    pub active_forks: usize,
    /// Network partitions detected
    pub partitions_detected: u64,
}

/// Synchronization status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Current sync status
    pub status: String,
    /// Blocks behind tip
    pub blocks_behind: u64,
    /// Active sync operations
    pub active_syncs: usize,
    /// Sync speed in blocks per second
    pub sync_speed_bps: f64,
}

/// Error summary across all components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    /// Total errors in last period
    pub total_errors: u64,
    /// Consensus errors
    pub consensus_errors: u64,
    /// Network errors
    pub network_errors: u64,
    /// Validation errors
    pub validation_errors: u64,
    /// Recovery failures
    pub recovery_failures: u64,
    /// Critical errors requiring attention
    pub critical_errors: Vec<String>,
}

/// Component-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMetrics {
    /// Malachite consensus metrics
    pub consensus: Option<ConsensusMetrics>,
    /// Fork detection metrics
    pub fork_detection: Option<ForkDetectionMetrics>,
    /// Network recovery metrics
    pub network_recovery: Option<NetworkRecoveryMetrics>,
    /// Block sync metrics
    pub block_sync: Option<BlockSyncMetrics>,
    /// Persistence metrics
    pub persistence: Option<PersistenceMetrics>,
}

impl ConsensusMetricsCollector {
    /// Create a new metrics collector
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            consensus_metrics: None,
            fork_metrics: None,
            recovery_metrics: None,
            sync_metrics: None,
            persistence_metrics: None,
            collection_interval,
            last_collection: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Register consensus metrics source
    pub fn register_consensus_metrics(&mut self, metrics: Arc<RwLock<ConsensusMetrics>>) {
        self.consensus_metrics = Some(metrics);
        debug!("Registered consensus metrics source");
    }

    /// Register fork detection metrics source
    pub fn register_fork_metrics(&mut self, metrics: Arc<RwLock<ForkDetectionMetrics>>) {
        self.fork_metrics = Some(metrics);
        debug!("Registered fork detection metrics source");
    }

    /// Register network recovery metrics source
    pub fn register_recovery_metrics(&mut self, metrics: Arc<RwLock<NetworkRecoveryMetrics>>) {
        self.recovery_metrics = Some(metrics);
        debug!("Registered network recovery metrics source");
    }

    /// Register block sync metrics source
    pub fn register_sync_metrics(&mut self, metrics: Arc<RwLock<BlockSyncMetrics>>) {
        self.sync_metrics = Some(metrics);
        debug!("Registered block sync metrics source");
    }

    /// Register persistence metrics source
    pub fn register_persistence_metrics(&mut self, metrics: Arc<RwLock<PersistenceMetrics>>) {
        self.persistence_metrics = Some(metrics);
        debug!("Registered persistence metrics source");
    }

    /// Collect metrics from all registered sources
    pub async fn collect_metrics(&self) -> AggregatedMetrics {
        let mut aggregated = AggregatedMetrics {
            timestamp: SystemTime::now(),
            system_health: SystemHealth::Healthy,
            consensus_performance: ConsensusPerformance {
                current_height: 0,
                blocks_per_second: 0.0,
                transactions_per_second: 0.0,
                avg_block_time_ms: 0,
                participation_rate: 100,
            },
            network_status: NetworkStatus {
                health: "Unknown".to_string(),
                connected_peers: 0,
                total_peers: 0,
                active_forks: 0,
                partitions_detected: 0,
            },
            sync_status: SyncStatus {
                status: "Unknown".to_string(),
                blocks_behind: 0,
                active_syncs: 0,
                sync_speed_bps: 0.0,
            },
            error_summary: ErrorSummary {
                total_errors: 0,
                consensus_errors: 0,
                network_errors: 0,
                validation_errors: 0,
                recovery_failures: 0,
                critical_errors: Vec::new(),
            },
            component_metrics: ComponentMetrics {
                consensus: None,
                fork_detection: None,
                network_recovery: None,
                block_sync: None,
                persistence: None,
            },
        };

        // Collect consensus metrics
        if let Some(consensus) = &self.consensus_metrics {
            let metrics = consensus.read().await.clone();
            aggregated.consensus_performance.current_height = metrics.current_height;
            aggregated.consensus_performance.avg_block_time_ms = metrics.avg_block_time_ms;
            aggregated.consensus_performance.transactions_per_second = 
                if metrics.avg_block_time_ms > 0 {
                    (metrics.transactions_processed as f64 * 1000.0) / metrics.avg_block_time_ms as f64
                } else {
                    0.0
                };
            aggregated.error_summary.consensus_errors = metrics.consensus_errors;
            aggregated.error_summary.validation_errors = metrics.validation_errors;
            aggregated.component_metrics.consensus = Some(metrics);
        }

        // Collect fork detection metrics
        if let Some(fork) = &self.fork_metrics {
            let metrics = fork.read().await.clone();
            aggregated.network_status.active_forks = metrics.active_forks;
            aggregated.network_status.partitions_detected = metrics.network_partitions;
            aggregated.component_metrics.fork_detection = Some(metrics);
        }

        // Collect network recovery metrics
        if let Some(recovery) = &self.recovery_metrics {
            let metrics = recovery.read().await.clone();
            aggregated.network_status.health = format!("{:?}", metrics.current_health_status);
            aggregated.network_status.partitions_detected += metrics.partitions_detected;
            aggregated.error_summary.recovery_failures = metrics.failed_recoveries;
            aggregated.component_metrics.network_recovery = Some(metrics);
        }

        // Collect block sync metrics
        if let Some(sync) = &self.sync_metrics {
            let metrics = sync.read().await.clone();
            aggregated.sync_status.status = format!("{:?}", metrics.current_status);
            aggregated.sync_status.active_syncs = metrics.active_sync_operations;
            aggregated.sync_status.sync_speed_bps = metrics.avg_sync_speed_bps;
            aggregated.error_summary.network_errors = metrics.network_errors;
            aggregated.component_metrics.block_sync = Some(metrics);
        }

        // Collect persistence metrics
        if let Some(persistence) = &self.persistence_metrics {
            let metrics = persistence.read().await.clone();
            aggregated.error_summary.total_errors += 
                metrics.verification_failures + metrics.corruption_detections;
            if metrics.corruption_detections > 0 {
                aggregated.error_summary.critical_errors.push(
                    format!("{} corruption detections", metrics.corruption_detections)
                );
            }
            aggregated.component_metrics.persistence = Some(metrics);
        }

        // Determine overall system health
        aggregated.system_health = self.calculate_system_health(&aggregated);

        // Update last collection time
        *self.last_collection.write().await = Instant::now();

        info!(
            "Metrics collected: height={}, health={:?}, errors={}",
            aggregated.consensus_performance.current_height,
            aggregated.system_health,
            aggregated.error_summary.total_errors
        );

        aggregated
    }

    /// Calculate overall system health based on metrics
    fn calculate_system_health(&self, metrics: &AggregatedMetrics) -> SystemHealth {
        // Critical if there are critical errors
        if !metrics.error_summary.critical_errors.is_empty() {
            return SystemHealth::Critical;
        }

        // Critical if network is partitioned
        if metrics.network_status.health.contains("Partitioned") || 
           metrics.network_status.health.contains("Critical") {
            return SystemHealth::Critical;
        }

        // Degraded if there are many errors or active forks
        if metrics.error_summary.total_errors > 10 || metrics.network_status.active_forks > 1 {
            return SystemHealth::Degraded;
        }

        // Degraded if sync is far behind
        if metrics.sync_status.blocks_behind > 100 {
            return SystemHealth::Degraded;
        }

        SystemHealth::Healthy
    }

    /// Check if metrics collection is due
    pub async fn should_collect(&self) -> bool {
        let last = *self.last_collection.read().await;
        last.elapsed() >= self.collection_interval
    }

    /// Get a summary string of current metrics
    pub async fn get_summary(&self) -> String {
        let metrics = self.collect_metrics().await;
        
        format!(
            "Consensus Metrics Summary:\n\
             - System Health: {:?}\n\
             - Height: {}\n\
             - TPS: {:.2}\n\
             - Network: {} ({}/{} peers)\n\
             - Sync: {} ({} active)\n\
             - Errors: {} total ({} critical)",
            metrics.system_health,
            metrics.consensus_performance.current_height,
            metrics.consensus_performance.transactions_per_second,
            metrics.network_status.health,
            metrics.network_status.connected_peers,
            metrics.network_status.total_peers,
            metrics.sync_status.status,
            metrics.sync_status.active_syncs,
            metrics.error_summary.total_errors,
            metrics.error_summary.critical_errors.len()
        )
    }
}

/// Metrics exporter trait for different output formats
#[async_trait::async_trait]
pub trait MetricsExporter: Send + Sync {
    /// Export metrics in the implementation's format
    async fn export(&self, metrics: &AggregatedMetrics) -> Result<String, String>;
}

/// JSON metrics exporter
pub struct JsonExporter;

#[async_trait::async_trait]
impl MetricsExporter for JsonExporter {
    async fn export(&self, metrics: &AggregatedMetrics) -> Result<String, String> {
        serde_json::to_string_pretty(metrics)
            .map_err(|e| format!("Failed to serialize metrics: {}", e))
    }
}

/// Prometheus-compatible metrics exporter
pub struct PrometheusExporter;

#[async_trait::async_trait]
impl MetricsExporter for PrometheusExporter {
    async fn export(&self, metrics: &AggregatedMetrics) -> Result<String, String> {
        let mut output = String::new();
        
        // System metrics
        output.push_str(&format!(
            "# HELP multivm_consensus_height Current blockchain height\n\
             # TYPE multivm_consensus_height gauge\n\
             multivm_consensus_height {}\n",
            metrics.consensus_performance.current_height
        ));
        
        output.push_str(&format!(
            "# HELP multivm_consensus_tps Transactions per second\n\
             # TYPE multivm_consensus_tps gauge\n\
             multivm_consensus_tps {:.2}\n",
            metrics.consensus_performance.transactions_per_second
        ));
        
        output.push_str(&format!(
            "# HELP multivm_network_peers Connected peers\n\
             # TYPE multivm_network_peers gauge\n\
             multivm_network_peers {}\n",
            metrics.network_status.connected_peers
        ));
        
        output.push_str(&format!(
            "# HELP multivm_errors_total Total errors\n\
             # TYPE multivm_errors_total counter\n\
             multivm_errors_total {}\n",
            metrics.error_summary.total_errors
        ));
        
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = ConsensusMetricsCollector::new(Duration::from_secs(60));
        let metrics = collector.collect_metrics().await;
        
        assert_eq!(metrics.system_health, SystemHealth::Healthy);
        assert!(metrics.error_summary.critical_errors.is_empty());
    }

    #[tokio::test]
    async fn test_json_exporter() {
        let collector = ConsensusMetricsCollector::new(Duration::from_secs(60));
        let metrics = collector.collect_metrics().await;
        
        let exporter = JsonExporter;
        let json = exporter.export(&metrics).await.unwrap();
        assert!(json.contains("\"system_health\""));
    }

    #[tokio::test]
    async fn test_prometheus_exporter() {
        let collector = ConsensusMetricsCollector::new(Duration::from_secs(60));
        let metrics = collector.collect_metrics().await;
        
        let exporter = PrometheusExporter;
        let prom = exporter.export(&metrics).await.unwrap();
        assert!(prom.contains("multivm_consensus_height"));
    }
}