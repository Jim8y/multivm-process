//! Zombie Process Reaper
//!
//! Provides functionality to detect and clean up zombie processes that may
//! be left behind by failed or improperly terminated child processes.

use multivm_common::{types::ProcessId, MultivmError, MultivmResult};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Configuration for the zombie process reaper
#[derive(Debug, Clone)]
pub struct ZombieReaperConfig {
    /// How often to scan for zombie processes
    pub scan_interval: Duration,
    /// Maximum age before a zombie process is force killed
    pub max_zombie_age: Duration,
    /// Grace period for processes to cleanly exit before being considered zombies
    pub grace_period: Duration,
    /// Maximum number of orphaned processes to track
    pub max_tracked_processes: usize,
    /// Enable automatic zombie cleanup
    pub auto_cleanup: bool,
    /// Process name patterns to monitor (empty means monitor all)
    pub monitored_patterns: Vec<String>,
}

impl Default for ZombieReaperConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(30),
            max_zombie_age: Duration::from_secs(300), // 5 minutes
            grace_period: Duration::from_secs(30),
            max_tracked_processes: 1000,
            auto_cleanup: true,
            monitored_patterns: vec![
                "mock-solana".to_string(),
                "mock-reth".to_string(),
                "multivm".to_string(),
            ],
        }
    }
}

/// Information about a tracked process
#[derive(Debug, Clone)]
pub struct TrackedProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub command: String,
    pub start_time: Instant,
    pub is_zombie: bool,
    pub zombie_since: Option<Instant>,
    pub multivm_process_id: Option<ProcessId>,
}

/// Statistics about zombie reaping activity
#[derive(Debug, Clone)]
pub struct ZombieReaperStats {
    pub total_scans: u64,
    pub zombies_found: u64,
    pub zombies_reaped: u64,
    pub reaping_failures: u64,
    pub last_scan: Option<Instant>,
    pub currently_tracked: usize,
    pub active_zombies: usize,
}

/// Zombie process reaper that monitors and cleans up orphaned processes
pub struct ZombieReaper {
    config: ZombieReaperConfig,
    tracked_processes: Arc<RwLock<HashMap<u32, TrackedProcess>>>,
    known_multivm_pids: Arc<RwLock<HashSet<u32>>>,
    stats: Arc<Mutex<ZombieReaperStats>>,
    is_running: Arc<Mutex<bool>>,
}

impl ZombieReaper {
    /// Create a new zombie reaper with the given configuration
    pub fn new(config: ZombieReaperConfig) -> Self {
        Self {
            config,
            tracked_processes: Arc::new(RwLock::new(HashMap::new())),
            known_multivm_pids: Arc::new(RwLock::new(HashSet::new())),
            stats: Arc::new(Mutex::new(ZombieReaperStats {
                total_scans: 0,
                zombies_found: 0,
                zombies_reaped: 0,
                reaping_failures: 0,
                last_scan: None,
                currently_tracked: 0,
                active_zombies: 0,
            })),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Start the zombie reaper background task
    pub async fn start(&self) -> MultivmResult<()> {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return Err(MultivmError::Process {
                process_id: "zombie_reaper".to_string(),
                message: "Zombie reaper is already running".to_string(),
                exit_code: None,
            });
        }
        *is_running = true;

        info!(
            "Starting zombie process reaper with scan interval: {:?}",
            self.config.scan_interval
        );

        let config = self.config.clone();
        let tracked_processes = Arc::clone(&self.tracked_processes);
        let known_multivm_pids = Arc::clone(&self.known_multivm_pids);
        let stats = Arc::clone(&self.stats);
        let is_running_flag = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.scan_interval);

            loop {
                // Check if we should continue running
                if !*is_running_flag.lock().await {
                    break;
                }

                interval.tick().await;

                if let Err(e) = Self::perform_zombie_scan(
                    &config,
                    &tracked_processes,
                    &known_multivm_pids,
                    &stats,
                )
                .await
                {
                    error!("Error during zombie scan: {}", e);
                }
            }

            info!("Zombie reaper background task stopped");
        });

        Ok(())
    }

    /// Stop the zombie reaper
    pub async fn stop(&self) -> MultivmResult<()> {
        info!("Stopping zombie process reaper");
        *self.is_running.lock().await = false;
        Ok(())
    }

    /// Register a MultiVM process PID to prevent it from being reaped
    pub async fn register_multivm_process(&self, pid: u32, process_id: ProcessId) {
        debug!("Registering MultiVM process: PID {} ({})", pid, process_id);
        self.known_multivm_pids.write().await.insert(pid);

        // Update tracked process info if it exists
        let mut tracked = self.tracked_processes.write().await;
        if let Some(process) = tracked.get_mut(&pid) {
            process.multivm_process_id = Some(process_id);
        }
    }

    /// Unregister a MultiVM process PID when it's no longer needed
    pub async fn unregister_multivm_process(&self, pid: u32) {
        debug!("Unregistering MultiVM process: PID {}", pid);
        self.known_multivm_pids.write().await.remove(&pid);

        // Mark process as potentially zombieable after grace period
        let mut tracked = self.tracked_processes.write().await;
        if let Some(process) = tracked.get_mut(&pid) {
            process.multivm_process_id = None;
        }
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> ZombieReaperStats {
        let mut stats = self.stats.lock().await;
        let tracked = self.tracked_processes.read().await;

        stats.currently_tracked = tracked.len();
        stats.active_zombies = tracked.values().filter(|p| p.is_zombie).count();

        stats.clone()
    }

    /// Force a manual zombie scan
    pub async fn manual_scan(&self) -> MultivmResult<()> {
        info!("Performing manual zombie scan");
        Self::perform_zombie_scan(
            &self.config,
            &self.tracked_processes,
            &self.known_multivm_pids,
            &self.stats,
        )
        .await
    }

    /// Internal method to perform the actual zombie scanning and reaping
    async fn perform_zombie_scan(
        config: &ZombieReaperConfig,
        tracked_processes: &Arc<RwLock<HashMap<u32, TrackedProcess>>>,
        known_multivm_pids: &Arc<RwLock<HashSet<u32>>>,
        stats: &Arc<Mutex<ZombieReaperStats>>,
    ) -> MultivmResult<()> {
        let scan_start = Instant::now();
        debug!("Starting zombie process scan");

        // Update stats
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.total_scans += 1;
            stats_guard.last_scan = Some(scan_start);
        }

        // Get current system processes
        let system_processes = Self::get_system_processes().await?;
        let known_pids = known_multivm_pids.read().await;

        let mut tracked = tracked_processes.write().await;
        let mut zombies_found = 0;
        let mut zombies_reaped = 0;
        let mut reaping_failures = 0;

        // Update tracked processes with current system state
        for (pid, process_info) in &system_processes {
            // Skip if this is a known active MultiVM process
            if known_pids.contains(pid) {
                continue;
            }

            // Check if this process matches our monitoring patterns
            if !config.monitored_patterns.is_empty()
                && !config
                    .monitored_patterns
                    .iter()
                    .any(|pattern| process_info.command.contains(pattern))
            {
                continue;
            }

            // Update or create tracked process
            let process_entry = tracked.entry(*pid).or_insert_with(|| TrackedProcess {
                pid: *pid,
                parent_pid: process_info.parent_pid,
                command: process_info.command.clone(),
                start_time: scan_start,
                is_zombie: false,
                zombie_since: None,
                multivm_process_id: None,
            });

            // Check if process is a zombie
            if process_info.is_zombie {
                if !process_entry.is_zombie {
                    // Newly detected zombie
                    process_entry.is_zombie = true;
                    process_entry.zombie_since = Some(scan_start);
                    zombies_found += 1;
                    warn!(
                        "Detected zombie process: PID {} ({})",
                        *pid, process_info.command
                    );
                }

                // Check if zombie should be reaped
                if config.auto_cleanup {
                    if let Some(zombie_since) = process_entry.zombie_since {
                        if scan_start.duration_since(zombie_since) > config.max_zombie_age {
                            match Self::reap_zombie_process(*pid).await {
                                Ok(()) => {
                                    info!("Successfully reaped zombie process: PID {}", *pid);
                                    zombies_reaped += 1;
                                    // Remove from tracking since it's been reaped
                                    tracked.remove(pid);
                                }
                                Err(e) => {
                                    error!("Failed to reap zombie process PID {}: {}", *pid, e);
                                    reaping_failures += 1;
                                }
                            }
                        }
                    }
                }
            } else if process_entry.is_zombie {
                // Process is no longer a zombie (probably was reaped by parent)
                debug!("Process PID {} is no longer a zombie", *pid);
                process_entry.is_zombie = false;
                process_entry.zombie_since = None;
            }
        }

        // Clean up tracking for processes that no longer exist
        let mut to_remove = Vec::new();
        for (&pid, _) in tracked.iter() {
            if !system_processes.contains_key(&pid) && !known_pids.contains(&pid) {
                to_remove.push(pid);
            }
        }

        for pid in to_remove {
            debug!("Removing non-existent process from tracking: PID {}", pid);
            tracked.remove(&pid);
        }

        // Limit the number of tracked processes
        if tracked.len() > config.max_tracked_processes {
            let mut sorted_pids: Vec<_> = tracked.keys().cloned().collect();
            sorted_pids.sort();
            let excess = tracked.len() - config.max_tracked_processes;
            for pid in sorted_pids.into_iter().take(excess) {
                tracked.remove(&pid);
            }
        }

        drop(tracked);
        drop(known_pids);

        // Update final stats
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.zombies_found += zombies_found;
            stats_guard.zombies_reaped += zombies_reaped;
            stats_guard.reaping_failures += reaping_failures;
        }

        debug!(
            "Zombie scan completed in {:?}, found {} zombies, reaped {} zombies",
            scan_start.elapsed(),
            zombies_found,
            zombies_reaped
        );

        Ok(())
    }

    /// Get information about current system processes
    async fn get_system_processes() -> MultivmResult<HashMap<u32, SystemProcessInfo>> {
        #[cfg(unix)]
        {
            Self::get_unix_processes().await
        }
        #[cfg(windows)]
        {
            Self::get_windows_processes().await
        }
    }

    #[cfg(unix)]
    async fn get_unix_processes() -> MultivmResult<HashMap<u32, SystemProcessInfo>> {
        use std::fs;
        use std::str::FromStr;

        let mut processes = HashMap::new();

        // Read /proc directory to get process information
        let proc_dir = match fs::read_dir("/proc") {
            Ok(dir) => dir,
            Err(_) => return Ok(processes), // If we can't read /proc, return empty
        };

        for entry in proc_dir.flatten() {
            if let Ok(filename) = entry.file_name().into_string() {
                if let Ok(pid) = u32::from_str(&filename) {
                    if let Ok(process_info) = Self::read_proc_info(pid).await {
                        processes.insert(pid, process_info);
                    }
                }
            }
        }

        Ok(processes)
    }

    #[cfg(unix)]
    async fn read_proc_info(pid: u32) -> MultivmResult<SystemProcessInfo> {
        use std::fs;

        // Read /proc/[pid]/stat for basic process information
        let stat_path = format!("/proc/{pid}/stat");
        let stat_content = fs::read_to_string(stat_path).map_err(|e| MultivmError::Process {
            process_id: pid.to_string(),
            message: format!("Failed to read process stat: {e}"),
            exit_code: None,
        })?;

        let fields: Vec<&str> = stat_content.split_whitespace().collect();
        if fields.len() < 4 {
            return Err(MultivmError::Process {
                process_id: pid.to_string(),
                message: "Invalid stat file format".to_string(),
                exit_code: None,
            });
        }

        // Parse relevant fields
        let state = fields[2].chars().next().unwrap_or('?');
        let parent_pid = fields[3].parse::<u32>().ok();

        // Try to read command line
        let cmdline_path = format!("/proc/{pid}/cmdline");
        let command = fs::read_to_string(cmdline_path)
            .unwrap_or_else(|_| fields[1].trim_matches(['(', ')']).to_string())
            .replace('\0', " ")
            .trim()
            .to_string();

        Ok(SystemProcessInfo {
            pid,
            parent_pid,
            command,
            is_zombie: state == 'Z',
        })
    }

    #[cfg(windows)]
    async fn get_windows_processes() -> MultivmResult<HashMap<u32, SystemProcessInfo>> {
        use std::process::Command;

        let mut processes = HashMap::new();

        // Use tasklist command to get process information
        let output = Command::new("tasklist")
            .args(&["/fo", "csv", "/nh"])
            .output()
            .map_err(|e| MultivmError::Process {
                process_id: "system".to_string(),
                message: format!("Failed to run tasklist: {}", e),
                exit_code: None,
            })?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if let Some(process_info) = Self::parse_tasklist_line(line) {
                processes.insert(process_info.pid, process_info);
            }
        }

        Ok(processes)
    }

    #[cfg(windows)]
    fn parse_tasklist_line(line: &str) -> Option<SystemProcessInfo> {
        let fields: Vec<&str> = line.split(',').map(|s| s.trim_matches('"')).collect();
        if fields.len() >= 2 {
            if let Ok(pid) = fields[1].parse::<u32>() {
                return Some(SystemProcessInfo {
                    pid,
                    parent_pid: None, // tasklist doesn't provide parent PID easily
                    command: fields[0].to_string(),
                    is_zombie: false, // Windows doesn't have zombie processes
                });
            }
        }
        None
    }

    /// Attempt to reap a zombie process
    async fn reap_zombie_process(pid: u32) -> MultivmResult<()> {
        #[cfg(unix)]
        {
            Self::reap_unix_zombie(pid).await
        }
        #[cfg(windows)]
        {
            // Windows doesn't have zombie processes, so this is a no-op
            Ok(())
        }
    }

    #[cfg(unix)]
    async fn reap_unix_zombie(pid: u32) -> MultivmResult<()> {
        use std::process::Command;

        // First try to send SIGTERM
        let term_result = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();

        match term_result {
            Ok(output) if output.status.success() => {
                // Give process time to exit gracefully
                tokio::time::sleep(Duration::from_secs(5)).await;

                // Check if it's still there
                if Self::process_exists(pid).await {
                    // Force kill with SIGKILL
                    let kill_result = Command::new("kill")
                        .args(["-KILL", &pid.to_string()])
                        .output();

                    match kill_result {
                        Ok(output) if output.status.success() => Ok(()),
                        Ok(_) => Err(MultivmError::Process {
                            process_id: pid.to_string(),
                            message: "Failed to force kill zombie process".to_string(),
                            exit_code: None,
                        }),
                        Err(e) => Err(MultivmError::Process {
                            process_id: pid.to_string(),
                            message: format!("Failed to execute kill command: {e}"),
                            exit_code: None,
                        }),
                    }
                } else {
                    Ok(())
                }
            }
            Ok(_) => Err(MultivmError::Process {
                process_id: pid.to_string(),
                message: "Kill command failed".to_string(),
                exit_code: None,
            }),
            Err(e) => Err(MultivmError::Process {
                process_id: pid.to_string(),
                message: format!("Failed to execute kill command: {e}"),
                exit_code: None,
            }),
        }
    }

    #[cfg(unix)]
    async fn process_exists(pid: u32) -> bool {
        std::fs::metadata(format!("/proc/{pid}")).is_ok()
    }
}

/// Information about a system process
#[derive(Debug, Clone)]
struct SystemProcessInfo {
    pid: u32,
    parent_pid: Option<u32>,
    command: String,
    is_zombie: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_zombie_reaper_creation() {
        let config = ZombieReaperConfig::default();
        let reaper = ZombieReaper::new(config);

        let stats = reaper.get_stats().await;
        assert_eq!(stats.total_scans, 0);
        assert_eq!(stats.zombies_found, 0);
    }

    #[tokio::test]
    async fn test_process_registration() {
        let config = ZombieReaperConfig::default();
        let reaper = ZombieReaper::new(config);

        reaper
            .register_multivm_process(1234, ProcessId::Solana)
            .await;

        let known_pids = reaper.known_multivm_pids.read().await;
        assert!(known_pids.contains(&1234));
    }

    #[tokio::test]
    async fn test_process_unregistration() {
        let config = ZombieReaperConfig::default();
        let reaper = ZombieReaper::new(config);

        reaper
            .register_multivm_process(1234, ProcessId::Solana)
            .await;
        reaper.unregister_multivm_process(1234).await;

        let known_pids = reaper.known_multivm_pids.read().await;
        assert!(!known_pids.contains(&1234));
    }
}
