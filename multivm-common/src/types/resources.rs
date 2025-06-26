use serde::{Deserialize, Serialize};

/// System resource limits for processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f64,
    pub max_disk_usage_gb: u64,
    pub max_open_files: u32,
    pub max_rpc_connections: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 8192,    // 8GB
            max_cpu_percent: 80.0,  // 80% CPU
            max_disk_usage_gb: 100, // 100GB
            max_open_files: 1024,
            max_rpc_connections: 1000,
        }
    }
}
