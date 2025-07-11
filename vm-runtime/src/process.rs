//! Process management for VM instances

use multivm_core::{Error, Result, VmId};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};

use crate::isolation::IsolationContext;

/// Process handle for a running VM
#[derive(Debug)]
pub struct ProcessHandle {
    pub vm_id: VmId,
    pub pid: u32,
    child: Arc<RwLock<Option<Child>>>,
}

impl ProcessHandle {
    fn new(vm_id: VmId, pid: u32, child: Child) -> Self {
        Self {
            vm_id,
            pid,
            child: Arc::new(RwLock::new(Some(child))),
        }
    }

    /// Get process ID
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

/// Process manager for VM instances
pub struct ProcessManager {
    processes: Arc<RwLock<HashMap<VmId, ProcessHandle>>>,
}

impl ProcessManager {
    /// Create a new process manager
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a new process
    pub async fn start_process(
        &self,
        vm_id: VmId,
        command: Vec<String>,
        isolation: Option<IsolationContext>,
    ) -> Result<ProcessHandle> {
        if command.is_empty() {
            return Err(Error::InvalidInput("Empty command".to_string()));
        }

        info!("Starting process for VM {}: {:?}", vm_id, command);

        let program = &command[0];
        let args = &command[1..];

        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply isolation context
        if let Some(ctx) = isolation {
            ctx.apply_to_command(&mut cmd)?;
        }

        // Set environment variables
        cmd.env("MULTIVM_ID", vm_id.to_string());

        // Start the process
        let child = cmd.spawn()
            .map_err(|e| Error::Io(e))?;

        let pid = child.id()
            .ok_or_else(|| Error::Other("Failed to get process ID".to_string()))?;

        info!("Started process {} for VM {}", pid, vm_id);

        // Create process handle
        let handle = ProcessHandle::new(vm_id, pid, child);

        // Store process
        self.processes.write().await.insert(vm_id, handle);

        // Return a clone of the handle
        let processes = self.processes.read().await;
        Ok(ProcessHandle {
            vm_id,
            pid,
            child: processes.get(&vm_id).unwrap().child.clone(),
        })
    }

    /// Stop a process gracefully
    pub async fn stop_process(&self, handle: &ProcessHandle) -> Result<()> {
        info!("Stopping process {} for VM {}", handle.pid, handle.vm_id);

        // Send SIGTERM
        let pid = Pid::from_raw(handle.pid as i32);
        signal::kill(pid, Signal::SIGTERM)
            .map_err(|e| Error::Other(format!("Failed to send SIGTERM: {}", e)))?;

        // Wait for process to exit
        let mut child_opt = handle.child.write().await;
        if let Some(mut child) = child_opt.take() {
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                child.wait()
            ).await {
                Ok(Ok(status)) => {
                    info!("Process {} exited with status: {:?}", handle.pid, status);
                }
                Ok(Err(e)) => {
                    warn!("Failed to wait for process {}: {}", handle.pid, e);
                }
                Err(_) => {
                    warn!("Process {} did not exit within timeout, killing", handle.pid);
                    // Force kill
                    signal::kill(pid, Signal::SIGKILL)
                        .map_err(|e| Error::Other(format!("Failed to send SIGKILL: {}", e)))?;
                }
            }
        }

        // Remove from tracking
        self.processes.write().await.remove(&handle.vm_id);
        Ok(())
    }

    /// Kill a process forcefully
    pub async fn kill_process(&self, handle: &ProcessHandle) -> Result<()> {
        info!("Killing process {} for VM {}", handle.pid, handle.vm_id);

        let pid = Pid::from_raw(handle.pid as i32);
        signal::kill(pid, Signal::SIGKILL)
            .map_err(|e| Error::Other(format!("Failed to send SIGKILL: {}", e)))?;

        // Clean up
        let mut child_opt = handle.child.write().await;
        if let Some(mut child) = child_opt.take() {
            let _ = child.wait().await;
        }

        self.processes.write().await.remove(&handle.vm_id);
        Ok(())
    }

    /// Pause a process (SIGSTOP)
    pub async fn pause_process(&self, handle: &ProcessHandle) -> Result<()> {
        debug!("Pausing process {} for VM {}", handle.pid, handle.vm_id);

        let pid = Pid::from_raw(handle.pid as i32);
        signal::kill(pid, Signal::SIGSTOP)
            .map_err(|e| Error::Other(format!("Failed to send SIGSTOP: {}", e)))?;

        Ok(())
    }

    /// Resume a paused process (SIGCONT)
    pub async fn resume_process(&self, handle: &ProcessHandle) -> Result<()> {
        debug!("Resuming process {} for VM {}", handle.pid, handle.vm_id);

        let pid = Pid::from_raw(handle.pid as i32);
        signal::kill(pid, Signal::SIGCONT)
            .map_err(|e| Error::Other(format!("Failed to send SIGCONT: {}", e)))?;

        Ok(())
    }

    /// Check if a process is running
    pub async fn is_running(&self, handle: &ProcessHandle) -> bool {
        // Check using kill with signal 0
        let pid = Pid::from_raw(handle.pid as i32);
        signal::kill(pid, None).is_ok()
    }

    /// Get all running processes
    pub async fn list_processes(&self) -> Vec<VmId> {
        self.processes.read().await.keys().cloned().collect()
    }

    /// Clean up zombie processes
    pub async fn cleanup_zombies(&self) -> Result<()> {
        let mut to_remove = Vec::new();

        {
            let processes = self.processes.read().await;
            for (vm_id, handle) in processes.iter() {
                if !self.is_running(handle).await {
                    to_remove.push(*vm_id);
                }
            }
        }

        if !to_remove.is_empty() {
            let mut processes = self.processes.write().await;
            for vm_id in to_remove {
                warn!("Removing zombie process for VM {}", vm_id);
                processes.remove(&vm_id);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_lifecycle() {
        let manager = ProcessManager::new();
        let vm_id = VmId::new();

        // Start a simple process
        let command = vec!["sleep".to_string(), "60".to_string()];
        let handle = manager.start_process(vm_id, command, None).await.unwrap();

        // Check it's running
        assert!(manager.is_running(&handle).await);

        // Stop it
        manager.stop_process(&handle).await.unwrap();

        // Check it's no longer running
        assert!(!manager.is_running(&handle).await);
    }
}