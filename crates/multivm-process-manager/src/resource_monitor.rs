use multivm_common::*;
use std::time::{Duration, Instant};
use sysinfo::{CpuExt, ProcessExt, System, SystemExt};

/// System resource monitor for tracking CPU, memory, and other resources
pub struct SystemResourceMonitor {
    system: System,
    limits: ResourceLimits,
    last_update: Instant,
    update_interval: Duration,
}

impl SystemResourceMonitor {
    pub fn new(limits: ResourceLimits) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system,
            limits,
            last_update: Instant::now(),
            update_interval: Duration::from_secs(5),
        }
    }

    /// Update system information if needed
    fn update_if_needed(&mut self) {
        if self.last_update.elapsed() >= self.update_interval {
            self.system.refresh_all();
            self.last_update = Instant::now();
        }
    }

    /// Check system resources against limits
    pub async fn check_system_resources(&mut self) -> MultivmResult<Vec<String>> {
        self.update_if_needed();

        let mut violations = Vec::new();

        // Check CPU usage
        let cpu_usage = self.get_cpu_usage();
        if cpu_usage > self.limits.max_cpu_percent {
            violations.push(format!(
                "CPU usage ({:.1}%) exceeds limit ({:.1}%)",
                cpu_usage, self.limits.max_cpu_percent
            ));
        }

        // Check memory usage
        let memory_usage_mb = self.get_memory_usage_mb();
        if memory_usage_mb > self.limits.max_memory_mb {
            violations.push(format!(
                "Memory usage ({} MB) exceeds limit ({} MB)",
                memory_usage_mb, self.limits.max_memory_mb
            ));
        }

        // Check disk usage
        let disk_usage_gb = self.get_disk_usage_gb().await?;
        if disk_usage_gb > self.limits.max_disk_usage_gb {
            violations.push(format!(
                "Disk usage ({} GB) exceeds limit ({} GB)",
                disk_usage_gb, self.limits.max_disk_usage_gb
            ));
        }

        if !violations.is_empty() {
            tracing::warn!("Resource limit violations: {:?}", violations);
        }

        Ok(violations)
    }

    /// Get current CPU usage percentage
    pub fn get_cpu_usage(&mut self) -> f64 {
        self.update_if_needed();

        let cpu_count = self.system.cpus().len();
        if cpu_count == 0 {
            return 0.0;
        }

        let total_usage: f32 = self.system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum();

        (total_usage / cpu_count as f32) as f64
    }

    /// Get current memory usage in MB
    pub fn get_memory_usage_mb(&mut self) -> u64 {
        self.update_if_needed();

        let used_memory = self.system.used_memory();
        used_memory / 1024 / 1024 // Convert bytes to MB
    }

    /// Get current disk usage in GB for data directories
    pub async fn get_disk_usage_gb(&self) -> MultivmResult<u64> {
        use std::path::Path;

        let data_dirs = vec![
            "./data",
            "./data/reth",
            "./data/solana",
            "/tmp/multivm", // IPC sockets location
        ];

        let mut total_usage_bytes = 0u64;

        for dir_path in data_dirs {
            if Path::new(dir_path).exists() {
                match self.calculate_directory_size(dir_path).await {
                    Ok(size) => total_usage_bytes += size,
                    Err(e) => {
                        tracing::warn!("Failed to calculate size for {}: {}", dir_path, e);
                    }
                }
            }
        }

        // Convert bytes to GB
        Ok(total_usage_bytes / 1024 / 1024 / 1024)
    }

    /// Calculate the total size of a directory in bytes
    async fn calculate_directory_size(&self, dir_path: &str) -> MultivmResult<u64> {
        use std::fs;
        use std::path::Path;

        let path = Path::new(dir_path);
        if !path.exists() {
            return Ok(0);
        }

        let _total_size = 0u64;

        // Use tokio to avoid blocking on large directories
        let path_clone = path.to_path_buf();
        let size = tokio::task::spawn_blocking(move || -> Result<u64, std::io::Error> {
            fn visit_dir(dir: &Path) -> std::io::Result<u64> {
                let mut size = 0u64;

                if dir.is_dir() {
                    for entry in fs::read_dir(dir)? {
                        let entry = entry?;
                        let path = entry.path();

                        if path.is_dir() {
                            size += visit_dir(&path)?;
                        } else {
                            if let Ok(metadata) = entry.metadata() {
                                size += metadata.len();
                            }
                        }
                    }
                }

                Ok(size)
            }

            visit_dir(&path_clone)
        })
        .await
        .map_err(|e| MultivmError::Process(format!("Task join error: {}", e)))?
        .map_err(|e| MultivmError::Io(format!("IO error calculating directory size: {}", e)))?;

        Ok(size)
    }

    /// Get total system memory in MB
    pub fn get_total_memory_mb(&mut self) -> u64 {
        self.update_if_needed();
        self.system.total_memory() / 1024 / 1024
    }

    /// Get memory usage percentage
    pub fn get_memory_usage_percent(&mut self) -> f64 {
        let used = self.get_memory_usage_mb();
        let total = self.get_total_memory_mb();

        if total == 0 {
            0.0
        } else {
            (used as f64 / total as f64) * 100.0
        }
    }

    /// Get process-specific resource usage
    pub fn get_process_resources(&mut self, pid: u32) -> Option<ProcessResources> {
        self.update_if_needed();

        self.system
            .process(sysinfo::Pid::from(pid as usize))
            .map(|process| ProcessResources {
                pid,
                cpu_usage: process.cpu_usage() as f64,
                memory_usage_kb: process.memory(),
                virtual_memory_kb: process.virtual_memory(),
                start_time: process.start_time(),
            })
    }
}

/// Process-specific resource information
#[derive(Debug, Clone)]
pub struct ProcessResources {
    pub pid: u32,
    pub cpu_usage: f64,
    pub memory_usage_kb: u64,
    pub virtual_memory_kb: u64,
    pub start_time: u64,
}

impl ProcessResources {
    pub fn memory_usage_mb(&self) -> u64 {
        self.memory_usage_kb / 1024
    }

    pub fn virtual_memory_mb(&self) -> u64 {
        self.virtual_memory_kb / 1024
    }
}
