//! Resource monitoring for VM instances

use multivm_core::{Error, Result, VmId, ResourceUsage};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, error};
use procfs::process::Process;

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Monitoring interval
    pub interval: std::time::Duration,
    /// Enable CPU monitoring
    pub monitor_cpu: bool,
    /// Enable memory monitoring
    pub monitor_memory: bool,
    /// Enable disk I/O monitoring
    pub monitor_disk_io: bool,
    /// Enable network monitoring
    pub monitor_network: bool,
    /// History size (number of samples to keep)
    pub history_size: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            interval: std::time::Duration::from_secs(5),
            monitor_cpu: true,
            monitor_memory: true,
            monitor_disk_io: true,
            monitor_network: true,
            history_size: 120, // 10 minutes at 5-second intervals
        }
    }
}

/// Resource usage sample
#[derive(Debug, Clone)]
struct ResourceSample {
    timestamp: chrono::DateTime<chrono::Utc>,
    usage: ResourceUsage,
}

/// Resource monitor for a VM instance
pub struct ResourceMonitor {
    vm_id: VmId,
    pid: u32,
    config: MonitorConfig,
    state: Arc<RwLock<MonitorState>>,
    handle: RwLock<Option<JoinHandle<()>>>,
}

struct MonitorState {
    current: Option<ResourceUsage>,
    history: Vec<ResourceSample>,
    cpu_ticks_prev: Option<u64>,
    cpu_time_prev: Option<std::time::Instant>,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(vm_id: VmId, pid: u32, config: MonitorConfig) -> Self {
        let history_size = config.history_size;
        Self {
            vm_id,
            pid,
            config,
            state: Arc::new(RwLock::new(MonitorState {
                current: None,
                history: Vec::with_capacity(history_size),
                cpu_ticks_prev: None,
                cpu_time_prev: None,
            })),
            handle: RwLock::new(None),
        }
    }

    /// Start monitoring
    pub async fn start(&self) -> Result<()> {
        let mut handle_guard = self.handle.write().await;
        if handle_guard.is_some() {
            return Err(Error::InvalidState("Monitor already running".to_string()));
        }

        info!("Starting resource monitor for VM {} (PID: {})", self.vm_id, self.pid);

        let vm_id = self.vm_id;
        let pid = self.pid;
        let config = self.config.clone();
        let state = self.state.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                if let Err(e) = Self::collect_metrics(pid, &config, &state).await {
                    error!("Failed to collect metrics for VM {}: {}", vm_id, e);
                    // Continue monitoring even on errors
                }
            }
        });

        *handle_guard = Some(handle);
        Ok(())
    }

    /// Stop monitoring
    pub async fn stop(&self) -> Result<()> {
        let mut handle_guard = self.handle.write().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            info!("Stopped resource monitor for VM {}", self.vm_id);
        }
        Ok(())
    }

    /// Get current resource usage
    pub async fn get_current_usage(&self) -> Result<ResourceUsage> {
        let state = self.state.read().await;
        state.current.clone()
            .ok_or_else(|| Error::NotFound("No resource data available yet".to_string()))
    }

    /// Get usage history
    pub async fn get_history(&self) -> Vec<ResourceSample> {
        let state = self.state.read().await;
        state.history.clone()
    }

    /// Collect metrics for a process
    async fn collect_metrics(
        pid: u32,
        config: &MonitorConfig,
        state: &Arc<RwLock<MonitorState>>,
    ) -> Result<()> {
        // Get process info
        let process = Process::new(pid as i32)
            .map_err(|e| Error::Other(format!("Failed to access process: {}", e)))?;

        let mut usage = ResourceUsage {
            cpu_percent: 0.0,
            memory_mb: 0,
            disk_io_mbps: 0.0,
            network_mbps: 0.0,
        };

        // CPU usage
        if config.monitor_cpu {
            let stat = process.stat()
                .map_err(|e| Error::Other(format!("Failed to read process stat: {}", e)))?;
            
            let cpu_ticks = stat.utime + stat.stime;
            let now = std::time::Instant::now();

            let mut state_guard = state.write().await;
            if let (Some(prev_ticks), Some(prev_time)) = 
                (state_guard.cpu_ticks_prev, state_guard.cpu_time_prev) {
                
                let ticks_diff = cpu_ticks.saturating_sub(prev_ticks);
                let time_diff = now.duration_since(prev_time).as_secs_f64();
                
                if time_diff > 0.0 {
                    // Calculate CPU percentage (assuming 100 ticks per second)
                    let ticks_per_sec = procfs::ticks_per_second() as f64;
                    usage.cpu_percent = ((ticks_diff as f64 / time_diff / ticks_per_sec) * 100.0) as f32;
                }
            }
            
            state_guard.cpu_ticks_prev = Some(cpu_ticks);
            state_guard.cpu_time_prev = Some(now);
        }

        // Memory usage
        if config.monitor_memory {
            let status = process.status()
                .map_err(|e| Error::Other(format!("Failed to read process status: {}", e)))?;
            
            if let Some(rss_bytes) = status.vmrss {
                usage.memory_mb = rss_bytes / 1024; // Convert KB to MB
            }
        }

        // Disk I/O (simplified - in production would track deltas)
        if config.monitor_disk_io {
            if let Ok(io) = process.io() {
                // This is total bytes, would need to track deltas for rate
                let total_io = io.read_bytes + io.write_bytes;
                usage.disk_io_mbps = ((total_io as f64) / 1_000_000.0 / 1000.0) as f32; // Very rough estimate
            }
        }

        // Network (would need to parse /proc/net/dev or use netlink)
        if config.monitor_network {
            // Placeholder - real implementation would track network interfaces
            usage.network_mbps = 0.0;
        }

        // Update state
        {
            let mut state_guard = state.write().await;
            state_guard.current = Some(usage.clone());

            // Add to history
            let sample = ResourceSample {
                timestamp: chrono::Utc::now(),
                usage: usage.clone(),
            };
            
            state_guard.history.push(sample);
            
            // Trim history if needed
            if state_guard.history.len() > config.history_size {
                state_guard.history.remove(0);
            }
        }

        debug!("Collected metrics for PID {}: {:?}", pid, usage);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitor_lifecycle() {
        let vm_id = VmId::new();
        let pid = std::process::id(); // Use our own PID for testing
        let config = MonitorConfig {
            interval: std::time::Duration::from_millis(100),
            ..Default::default()
        };

        let monitor = ResourceMonitor::new(vm_id, pid, config);

        // Start monitoring
        monitor.start().await.unwrap();

        // Wait for some samples
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Get current usage
        let usage = monitor.get_current_usage().await.unwrap();
        assert!(usage.memory_mb > 0);

        // Check history
        let history = monitor.get_history().await;
        assert!(!history.is_empty());

        // Stop monitoring
        monitor.stop().await.unwrap();
    }
}