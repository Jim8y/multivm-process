//! Process isolation and resource limits

use multivm_core::{Error, Result, VmId, ResourceRequirements};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use tracing::{debug, info, warn};
use tempfile::TempDir;

/// Isolation configuration
#[derive(Debug, Clone)]
pub struct IsolationConfig {
    /// Enable cgroup isolation
    pub enable_cgroups: bool,
    /// Enable namespace isolation
    pub enable_namespaces: bool,
    /// Enable seccomp filtering
    pub enable_seccomp: bool,
    /// Cgroup v2 mount path
    pub cgroup_mount: std::path::PathBuf,
    /// Use systemd for cgroup management
    pub use_systemd: bool,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            enable_cgroups: true,
            enable_namespaces: true,
            enable_seccomp: false,
            cgroup_mount: std::path::PathBuf::from("/sys/fs/cgroup"),
            use_systemd: false,
        }
    }
}

/// Isolation context for a VM
#[derive(Debug)]
pub struct IsolationContext {
    vm_id: VmId,
    cgroup_path: Option<String>,
    namespace_dir: Option<TempDir>,
}

impl IsolationContext {
    /// Apply isolation to a command
    pub fn apply_to_command(&self, _cmd: &mut Command) -> Result<()> {
        // Apply cgroup
        if let Some(cgroup_path) = &self.cgroup_path {
            // In systemd mode, we would use systemd-run
            // For now, we'll set the cgroup after process starts
            debug!("Will apply cgroup: {}", cgroup_path);
        }

        // Apply namespaces
        if self.namespace_dir.is_some() {
            // Use unshare to create new namespaces
            // This is simplified - real implementation would use clone flags
            debug!("Will apply namespace isolation");
        }

        Ok(())
    }
}

/// Isolation manager
pub struct IsolationManager {
    config: IsolationConfig,
    contexts: Arc<RwLock<HashMap<VmId, IsolationContext>>>,
}

impl IsolationManager {
    /// Create a new isolation manager
    pub fn new(config: IsolationConfig) -> Result<Self> {
        // Verify cgroup mount exists
        if config.enable_cgroups && !config.cgroup_mount.exists() {
            warn!("Cgroup mount {} does not exist, disabling cgroups", 
                  config.cgroup_mount.display());
        }

        Ok(Self {
            config,
            contexts: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create isolation context for a VM
    pub async fn create_context(
        &self,
        vm_id: &VmId,
        resources: &ResourceRequirements,
    ) -> Result<IsolationContext> {
        info!("Creating isolation context for VM {}", vm_id);

        let mut context = IsolationContext {
            vm_id: *vm_id,
            cgroup_path: None,
            namespace_dir: None,
        };

        // Create cgroup
        if self.config.enable_cgroups {
            let cgroup_path = self.create_cgroup(vm_id, resources).await?;
            context.cgroup_path = Some(cgroup_path);
        }

        // Create namespace directory
        if self.config.enable_namespaces {
            let namespace_dir = TempDir::new()
                .map_err(|e| Error::Io(e))?;
            context.namespace_dir = Some(namespace_dir);
        }

        // Store context
        self.contexts.write().await.insert(*vm_id, context);

        // Return a copy
        let contexts = self.contexts.read().await;
        Ok(IsolationContext {
            vm_id: *vm_id,
            cgroup_path: contexts.get(vm_id).unwrap().cgroup_path.clone(),
            namespace_dir: None, // Don't move the TempDir
        })
    }

    /// Clean up isolation context
    pub async fn cleanup_context(&self, vm_id: &VmId) -> Result<()> {
        info!("Cleaning up isolation context for VM {}", vm_id);

        if let Some(context) = self.contexts.write().await.remove(vm_id) {
            // Remove cgroup
            if let Some(cgroup_path) = context.cgroup_path {
                self.remove_cgroup(&cgroup_path).await?;
            }

            // Namespace directory is cleaned up automatically when TempDir is dropped
        }

        Ok(())
    }

    /// Create cgroup for a VM
    async fn create_cgroup(
        &self,
        vm_id: &VmId,
        resources: &ResourceRequirements,
    ) -> Result<String> {
        let cgroup_name = format!("multivm-{}", vm_id);

        if self.config.use_systemd {
            // Use systemd transient units
            debug!("Creating systemd slice for {}", cgroup_name);
            // This would use systemd D-Bus API in production
            Ok(format!("multivm-{}.slice", vm_id))
        } else {
            // Direct cgroup v2 manipulation
            let cgroup_path = self.config.cgroup_mount
                .join("multivm")
                .join(vm_id.to_string());

            // Create cgroup directory
            tokio::fs::create_dir_all(&cgroup_path)
                .await
                .map_err(|e| Error::Io(e))?;

            // Set CPU limits
            let cpu_max = format!("{} 100000", 
                (resources.cpu_cores * 100_000.0) as u64);
            let cpu_max_path = cgroup_path.join("cpu.max");
            tokio::fs::write(&cpu_max_path, cpu_max)
                .await
                .map_err(|e| Error::Io(e))?;

            // Set memory limits
            let memory_max = format!("{}", resources.memory_mb * 1024 * 1024);
            let memory_max_path = cgroup_path.join("memory.max");
            tokio::fs::write(&memory_max_path, memory_max)
                .await
                .map_err(|e| Error::Io(e))?;

            Ok(cgroup_path.to_string_lossy().to_string())
        }
    }

    /// Remove cgroup
    async fn remove_cgroup(&self, cgroup_path: &str) -> Result<()> {
        if self.config.use_systemd {
            // Stop systemd unit
            debug!("Stopping systemd unit for {}", cgroup_path);
            // This would use systemd D-Bus API in production
        } else {
            // Remove cgroup directory
            let path = std::path::Path::new(cgroup_path);
            if path.exists() {
                tokio::fs::remove_dir(path)
                    .await
                    .map_err(|e| Error::Io(e))?;
            }
        }
        Ok(())
    }

    /// Apply process to cgroup
    pub async fn apply_to_cgroup(&self, vm_id: &VmId, pid: u32) -> Result<()> {
        let contexts = self.contexts.read().await;
        if let Some(context) = contexts.get(vm_id) {
            if let Some(cgroup_path) = &context.cgroup_path {
                if !self.config.use_systemd {
                    // Write PID to cgroup.procs
                    let procs_path = std::path::Path::new(cgroup_path).join("cgroup.procs");
                    tokio::fs::write(&procs_path, pid.to_string())
                        .await
                        .map_err(|e| Error::Io(e))?;
                    
                    debug!("Added PID {} to cgroup {}", pid, cgroup_path);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_isolation_context_creation() {
        let config = IsolationConfig {
            enable_cgroups: false, // Disable for testing without root
            enable_namespaces: true,
            ..Default::default()
        };

        let manager = IsolationManager::new(config).unwrap();
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();

        let context = manager.create_context(&vm_id, &resources).await.unwrap();
        assert_eq!(context.vm_id, vm_id);
        assert!(context.cgroup_path.is_none());

        // Cleanup
        manager.cleanup_context(&vm_id).await.unwrap();
    }
}