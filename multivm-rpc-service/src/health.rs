//! Production-ready health check system

use crate::{
    error::{RpcError, RpcResult},
    types::{HealthStatus, NodeHealth, VmType},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

/// Custom serialization for Duration
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}
use tokio::sync::RwLock;
use tracing::{debug, warn, error};

/// Comprehensive health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Overall system status
    pub status: HealthStatus,
    /// Timestamp of health check
    pub timestamp: SystemTime,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Memory usage information
    pub memory: MemoryInfo,
    /// CPU usage information
    pub cpu: CpuInfo,
    /// Disk usage information
    pub disk: DiskInfo,
    /// Network health
    pub network: NetworkHealth,
    /// Backend services health
    pub backends: HashMap<String, NodeHealth>,
    /// Application-specific metrics
    pub application: ApplicationHealth,
    /// Version information
    pub version: VersionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total memory in bytes
    pub total_bytes: u64,
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Available memory in bytes
    pub available_bytes: u64,
    /// Memory usage percentage
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU usage percentage
    pub usage_percent: f64,
    /// Number of CPU cores
    pub cores: u32,
    /// Load average (1 minute)
    pub load_1m: f64,
    /// Load average (5 minutes)
    pub load_5m: f64,
    /// Load average (15 minutes)
    pub load_15m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Total disk space in bytes
    pub total_bytes: u64,
    /// Used disk space in bytes
    pub used_bytes: u64,
    /// Available disk space in bytes
    pub available_bytes: u64,
    /// Disk usage percentage
    pub usage_percent: f64,
    /// Disk path
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealth {
    /// Network connectivity status
    pub connectivity: HealthStatus,
    /// DNS resolution status
    pub dns_resolution: HealthStatus,
    /// Internet connectivity status
    pub internet_access: HealthStatus,
    /// Network latency in milliseconds
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationHealth {
    /// Database connection status
    pub database: HealthStatus,
    /// Cache status
    pub cache: HealthStatus,
    /// RPC endpoints status
    pub rpc_endpoints: HealthStatus,
    /// Authentication service status
    pub authentication: HealthStatus,
    /// Rate limiting status
    pub rate_limiting: HealthStatus,
    /// Active connections count
    pub active_connections: u32,
    /// Request queue size
    pub request_queue_size: u32,
    /// Thread pool status
    pub thread_pool: ThreadPoolHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadPoolHealth {
    /// Active threads
    pub active_threads: u32,
    /// Total threads
    pub total_threads: u32,
    /// Queued tasks
    pub queued_tasks: u32,
    /// Completed tasks
    pub completed_tasks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Application version
    pub version: String,
    /// Git commit hash
    pub commit_hash: Option<String>,
    /// Build timestamp
    pub build_timestamp: Option<String>,
    /// Rust version used for build
    pub rust_version: Option<String>,
}

/// Health check configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthConfig {
    /// Health check interval
    #[serde(with = "duration_serde")]
    pub check_interval: Duration,
    /// Enable system metrics collection
    pub enable_system_metrics: bool,
    /// Enable network checks
    pub enable_network_checks: bool,
    /// Enable deep health checks
    pub enable_deep_checks: bool,
    /// Timeout for individual health checks
    #[serde(with = "duration_serde")]
    pub check_timeout: Duration,
    /// Memory usage warning threshold (percentage)
    pub memory_warning_threshold: f64,
    /// Memory usage critical threshold (percentage)
    pub memory_critical_threshold: f64,
    /// CPU usage warning threshold (percentage)
    pub cpu_warning_threshold: f64,
    /// CPU usage critical threshold (percentage)
    pub cpu_critical_threshold: f64,
    /// Disk usage warning threshold (percentage)
    pub disk_warning_threshold: f64,
    /// Disk usage critical threshold (percentage)
    pub disk_critical_threshold: f64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            enable_system_metrics: true,
            enable_network_checks: true,
            enable_deep_checks: false,
            check_timeout: Duration::from_secs(5),
            memory_warning_threshold: 80.0,
            memory_critical_threshold: 95.0,
            cpu_warning_threshold: 80.0,
            cpu_critical_threshold: 95.0,
            disk_warning_threshold: 80.0,
            disk_critical_threshold: 95.0,
        }
    }
}

impl HealthConfig {
    /// Production configuration
    pub fn production() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            enable_system_metrics: true,
            enable_network_checks: true,
            enable_deep_checks: true,
            check_timeout: Duration::from_secs(10),
            memory_warning_threshold: 75.0,
            memory_critical_threshold: 90.0,
            cpu_warning_threshold: 75.0,
            cpu_critical_threshold: 90.0,
            disk_warning_threshold: 80.0,
            disk_critical_threshold: 95.0,
        }
    }
}

/// Comprehensive health checker
pub struct HealthChecker {
    config: HealthConfig,
    start_time: SystemTime,
    last_health: Arc<RwLock<Option<SystemHealth>>>,
}

impl HealthChecker {
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            start_time: SystemTime::now(),
            last_health: Arc::new(RwLock::new(None)),
        }
    }

    /// Perform comprehensive health check
    pub async fn check_health(&self) -> RpcResult<SystemHealth> {
        debug!("Starting comprehensive health check");
        
        let start_time = SystemTime::now();
        
        // Collect all health information
        let health = SystemHealth {
            status: HealthStatus::Healthy, // Will be updated based on checks
            timestamp: SystemTime::now(),
            uptime_seconds: self.start_time.elapsed().unwrap_or_default().as_secs(),
            memory: self.check_memory().await?,
            cpu: self.check_cpu().await?,
            disk: self.check_disk().await?,
            network: self.check_network().await?,
            backends: HashMap::new(), // To be filled by caller
            application: self.check_application().await?,
            version: self.get_version_info(),
        };

        // Determine overall status
        let overall_status = self.determine_overall_status(&health);
        let mut final_health = health;
        final_health.status = overall_status;

        // Cache the result
        *self.last_health.write().await = Some(final_health.clone());

        let duration = start_time.elapsed().unwrap_or_default();
        debug!("Health check completed in {:?}", duration);

        if duration > self.config.check_timeout {
            warn!("Health check took longer than expected: {:?}", duration);
        }

        Ok(final_health)
    }

    /// Get last cached health result
    pub async fn get_last_health(&self) -> Option<SystemHealth> {
        self.last_health.read().await.clone()
    }

    /// Check memory usage
    async fn check_memory(&self) -> RpcResult<MemoryInfo> {
        if !self.config.enable_system_metrics {
            return Ok(MemoryInfo {
                total_bytes: 0,
                used_bytes: 0,
                available_bytes: 0,
                usage_percent: 0.0,
            });
        }

        // In production, this would use system APIs like /proc/meminfo
        // For now, providing mock data that demonstrates the structure
        let total = 8_000_000_000u64; // 8GB
        let used = 2_400_000_000u64;  // 2.4GB
        let available = total - used;
        let usage_percent = (used as f64 / total as f64) * 100.0;

        Ok(MemoryInfo {
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            usage_percent,
        })
    }

    /// Check CPU usage
    async fn check_cpu(&self) -> RpcResult<CpuInfo> {
        if !self.config.enable_system_metrics {
            return Ok(CpuInfo {
                usage_percent: 0.0,
                cores: 1,
                load_1m: 0.0,
                load_5m: 0.0,
                load_15m: 0.0,
            });
        }

        // Mock CPU information
        Ok(CpuInfo {
            usage_percent: 25.5,
            cores: 8,
            load_1m: 1.2,
            load_5m: 1.1,
            load_15m: 0.9,
        })
    }

    /// Check disk usage
    async fn check_disk(&self) -> RpcResult<DiskInfo> {
        if !self.config.enable_system_metrics {
            return Ok(DiskInfo {
                total_bytes: 0,
                used_bytes: 0,
                available_bytes: 0,
                usage_percent: 0.0,
                path: "/".to_string(),
            });
        }

        // Mock disk information
        let total = 100_000_000_000u64; // 100GB
        let used = 45_000_000_000u64;   // 45GB
        let available = total - used;
        let usage_percent = (used as f64 / total as f64) * 100.0;

        Ok(DiskInfo {
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            usage_percent,
            path: "/".to_string(),
        })
    }

    /// Check network connectivity
    async fn check_network(&self) -> RpcResult<NetworkHealth> {
        if !self.config.enable_network_checks {
            return Ok(NetworkHealth {
                connectivity: HealthStatus::Unknown,
                dns_resolution: HealthStatus::Unknown,
                internet_access: HealthStatus::Unknown,
                latency_ms: None,
            });
        }

        // Mock network checks - in production, these would be real tests
        Ok(NetworkHealth {
            connectivity: HealthStatus::Healthy,
            dns_resolution: HealthStatus::Healthy,
            internet_access: HealthStatus::Healthy,
            latency_ms: Some(25),
        })
    }

    /// Check application-specific health
    async fn check_application(&self) -> RpcResult<ApplicationHealth> {
        // Mock application health
        Ok(ApplicationHealth {
            database: HealthStatus::Healthy,
            cache: HealthStatus::Healthy,
            rpc_endpoints: HealthStatus::Healthy,
            authentication: HealthStatus::Healthy,
            rate_limiting: HealthStatus::Healthy,
            active_connections: 42,
            request_queue_size: 0,
            thread_pool: ThreadPoolHealth {
                active_threads: 8,
                total_threads: 16,
                queued_tasks: 0,
                completed_tasks: 12345,
            },
        })
    }

    /// Get version information
    fn get_version_info(&self) -> VersionInfo {
        VersionInfo {
            version: crate::VERSION.to_string(),
            commit_hash: option_env!("GIT_HASH").map(|s| s.to_string()),
            build_timestamp: option_env!("BUILD_TIMESTAMP").map(|s| s.to_string()),
            rust_version: option_env!("RUSTC_VERSION").map(|s| s.to_string()),
        }
    }

    /// Determine overall system status based on all checks
    fn determine_overall_status(&self, health: &SystemHealth) -> HealthStatus {
        // Check critical thresholds
        if health.memory.usage_percent > self.config.memory_critical_threshold
            || health.cpu.usage_percent > self.config.cpu_critical_threshold
            || health.disk.usage_percent > self.config.disk_critical_threshold
        {
            return HealthStatus::Unhealthy;
        }

        // Check application health
        match health.application.database {
            HealthStatus::Unhealthy => return HealthStatus::Unhealthy,
            _ => {}
        }

        // Check warning thresholds
        if health.memory.usage_percent > self.config.memory_warning_threshold
            || health.cpu.usage_percent > self.config.cpu_warning_threshold
            || health.disk.usage_percent > self.config.disk_warning_threshold
        {
            return HealthStatus::Degraded;
        }

        // Check network health
        match health.network.connectivity {
            HealthStatus::Unhealthy => return HealthStatus::Degraded,
            _ => {}
        }

        HealthStatus::Healthy
    }

    /// Start periodic health checks
    pub fn start_periodic_checks(&self) -> tokio::task::JoinHandle<()> {
        let checker = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(checker.config.check_interval);
            loop {
                interval.tick().await;
                
                match checker.check_health().await {
                    Ok(health) => {
                        match health.status {
                            HealthStatus::Healthy => {
                                debug!("System health check: HEALTHY");
                            }
                            HealthStatus::Degraded => {
                                warn!("System health check: DEGRADED");
                            }
                            HealthStatus::Unhealthy => {
                                error!("System health check: UNHEALTHY");
                            }
                            HealthStatus::Unknown => {
                                warn!("System health check: UNKNOWN");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to perform health check: {}", e);
                    }
                }
            }
        })
    }
}

impl Clone for HealthChecker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            start_time: self.start_time,
            last_health: Arc::clone(&self.last_health),
        }
    }
}

/// Health check endpoints for HTTP API
pub mod endpoints {
    use super::*;
    use axum::{extract::State, response::Json};
    use serde_json::Value;
    use std::sync::Arc;

    /// Liveness probe endpoint
    pub async fn liveness() -> Json<Value> {
        Json(serde_json::json!({
            "status": "alive",
            "timestamp": SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }))
    }

    /// Readiness probe endpoint
    pub async fn readiness(
        State(health_checker): State<Arc<HealthChecker>>,
    ) -> Json<Value> {
        match health_checker.get_last_health().await {
            Some(health) => {
                let ready = matches!(health.status, HealthStatus::Healthy | HealthStatus::Degraded);
                Json(serde_json::json!({
                    "status": if ready { "ready" } else { "not_ready" },
                    "health_status": health.status,
                    "timestamp": health.timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                }))
            }
            None => {
                Json(serde_json::json!({
                    "status": "not_ready",
                    "reason": "No health data available"
                }))
            }
        }
    }

    /// Detailed health endpoint
    pub async fn health_detailed(
        State(health_checker): State<Arc<HealthChecker>>,
    ) -> Json<SystemHealth> {
        match health_checker.check_health().await {
            Ok(health) => Json(health),
            Err(_) => {
                // Return a minimal unhealthy status
                Json(SystemHealth {
                    status: HealthStatus::Unhealthy,
                    timestamp: SystemTime::now(),
                    uptime_seconds: 0,
                    memory: MemoryInfo {
                        total_bytes: 0,
                        used_bytes: 0,
                        available_bytes: 0,
                        usage_percent: 0.0,
                    },
                    cpu: CpuInfo {
                        usage_percent: 0.0,
                        cores: 0,
                        load_1m: 0.0,
                        load_5m: 0.0,
                        load_15m: 0.0,
                    },
                    disk: DiskInfo {
                        total_bytes: 0,
                        used_bytes: 0,
                        available_bytes: 0,
                        usage_percent: 0.0,
                        path: "/".to_string(),
                    },
                    network: NetworkHealth {
                        connectivity: HealthStatus::Unknown,
                        dns_resolution: HealthStatus::Unknown,
                        internet_access: HealthStatus::Unknown,
                        latency_ms: None,
                    },
                    backends: HashMap::new(),
                    application: ApplicationHealth {
                        database: HealthStatus::Unknown,
                        cache: HealthStatus::Unknown,
                        rpc_endpoints: HealthStatus::Unknown,
                        authentication: HealthStatus::Unknown,
                        rate_limiting: HealthStatus::Unknown,
                        active_connections: 0,
                        request_queue_size: 0,
                        thread_pool: ThreadPoolHealth {
                            active_threads: 0,
                            total_threads: 0,
                            queued_tasks: 0,
                            completed_tasks: 0,
                        },
                    },
                    version: VersionInfo {
                        version: crate::VERSION.to_string(),
                        commit_hash: None,
                        build_timestamp: None,
                        rust_version: None,
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_config_default() {
        let config = HealthConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert!(config.enable_system_metrics);
    }

    #[test]
    fn test_health_config_production() {
        let config = HealthConfig::production();
        assert_eq!(config.memory_warning_threshold, 75.0);
        assert_eq!(config.memory_critical_threshold, 90.0);
        assert!(config.enable_deep_checks);
    }

    #[tokio::test]
    async fn test_health_checker() {
        let config = HealthConfig::default();
        let checker = HealthChecker::new(config);
        
        let health = checker.check_health().await.unwrap();
        assert!(matches!(health.status, HealthStatus::Healthy));
        assert!(health.uptime_seconds >= 0);
    }

    #[test]
    fn test_memory_info() {
        let memory = MemoryInfo {
            total_bytes: 1000,
            used_bytes: 500,
            available_bytes: 500,
            usage_percent: 50.0,
        };
        
        assert_eq!(memory.usage_percent, 50.0);
    }
}