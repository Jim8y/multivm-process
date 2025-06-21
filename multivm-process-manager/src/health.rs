use crate::ProcessHandle;
use multivm_common::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::info;

/// Health monitor for tracking the health of engine processes
#[derive(Clone)]
pub struct HealthMonitor {
    check_interval: Duration,
    last_check_times: Arc<Mutex<HashMap<ProcessId, Instant>>>,
}

impl HealthMonitor {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            check_interval,
            last_check_times: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check the health of a specific process
    pub async fn check_process_health(
        &self,
        handle: &ProcessHandle,
    ) -> MultivmResult<HealthStatus> {
        let process_id = handle.process_id;

        // Check if process is still running
        let is_running = handle.is_running().await;

        if !is_running {
            return Ok(HealthStatus {
                process_id,
                is_healthy: false,
                last_block_processed: None,
                blocks_processed_total: 0,
                uptime: Duration::ZERO,
                memory_usage: 0,
                cpu_usage_percent: 0.0,
                rpc_active: false,
                errors_count: 1,
                last_error: Some("Process not running".to_string()),
                timestamp: std::time::SystemTime::now(),
            });
        }

        // Try to get health status from the process via IPC
        match handle.send_command(IpcCommand::GetHealth).await {
            Ok(IpcResponse::Health { status }) => Ok(status),
            Ok(_) => Err(MultivmError::Ipc(
                "Unexpected response to health check".to_string(),
            )),
            Err(e) => {
                // If IPC fails, process might be unhealthy
                Ok(HealthStatus {
                    process_id,
                    is_healthy: false,
                    last_block_processed: None,
                    blocks_processed_total: 0,
                    uptime: Duration::ZERO,
                    memory_usage: 0,
                    cpu_usage_percent: 0.0,
                    rpc_active: false,
                    errors_count: 1,
                    last_error: Some(format!("IPC health check failed: {}", e)),
                    timestamp: std::time::SystemTime::now(),
                })
            }
        }
    }

    /// Update the last check time for a process
    pub fn update_last_check(&self, process_id: ProcessId) {
        self.last_check_times.lock().unwrap().insert(process_id, Instant::now());
    }

    /// Check if a process needs a health check
    pub fn needs_check(&self, process_id: ProcessId) -> bool {
        match self.last_check_times.lock().unwrap().get(&process_id) {
            Some(last_check) => last_check.elapsed() >= self.check_interval,
            None => true, // Never checked before
        }
    }

    /// Get the check interval
    pub fn check_interval(&self) -> Duration {
        self.check_interval
    }

}
