//! Main VM runtime implementation

use multivm_core::{Error, Result, VmId, VmMetadata, VmState, ResourceRequirements};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::process::{ProcessManager, ProcessHandle};
use crate::monitor::{ResourceMonitor, MonitorConfig};
use crate::isolation::{IsolationConfig, IsolationManager};

/// VM runtime configuration
#[derive(Debug, Clone)]
pub struct VmRuntimeConfig {
    /// Runtime data directory
    pub runtime_dir: std::path::PathBuf,
    /// VM executable command (e.g., "qemu-system-x86_64", "firecracker")
    pub vm_command: String,
    /// Default VM configuration template
    pub vm_config_template: serde_json::Value,
    /// Resource monitoring configuration
    pub monitor_config: MonitorConfig,
    /// Process isolation configuration
    pub isolation_config: IsolationConfig,
    /// Maximum number of concurrent VMs
    pub max_vms: usize,
}

impl Default for VmRuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_dir: std::path::PathBuf::from("/var/lib/multivm/runtime"),
            vm_command: "qemu-system-x86_64".to_string(),
            vm_config_template: serde_json::json!({}),
            monitor_config: MonitorConfig::default(),
            isolation_config: IsolationConfig::default(),
            max_vms: 100,
        }
    }
}

/// VM instance state
struct VmInstance {
    metadata: VmMetadata,
    process: ProcessHandle,
    monitor: Arc<ResourceMonitor>,
}

/// Main VM runtime
pub struct VmRuntime {
    config: VmRuntimeConfig,
    process_manager: Arc<ProcessManager>,
    isolation_manager: Arc<IsolationManager>,
    instances: Arc<RwLock<HashMap<VmId, VmInstance>>>,
}

impl VmRuntime {
    /// Create a new VM runtime
    pub async fn new(config: VmRuntimeConfig) -> Result<Self> {
        // Create runtime directory
        tokio::fs::create_dir_all(&config.runtime_dir)
            .await
            .map_err(|e| Error::Io(e))?;

        let process_manager = Arc::new(ProcessManager::new());
        let isolation_manager = Arc::new(IsolationManager::new(config.isolation_config.clone())?);

        Ok(Self {
            config,
            process_manager,
            isolation_manager,
            instances: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start a VM instance
    pub async fn start_vm(&self, mut metadata: VmMetadata) -> Result<()> {
        let vm_id = metadata.id;
        
        // Check if VM already exists
        {
            let instances = self.instances.read().await;
            if instances.contains_key(&vm_id) {
                return Err(Error::InvalidState(format!("VM {} already exists", vm_id)));
            }
        }

        // Check resource limits
        {
            let instances = self.instances.read().await;
            if instances.len() >= self.config.max_vms {
                return Err(Error::ResourceExhausted(format!(
                    "Maximum number of VMs ({}) reached", self.config.max_vms
                )));
            }
        }

        info!("Starting VM {}", vm_id);

        // Create VM working directory
        let vm_dir = self.config.runtime_dir.join(vm_id.to_string());
        tokio::fs::create_dir_all(&vm_dir)
            .await
            .map_err(|e| Error::Io(e))?;

        // Generate VM configuration
        let vm_config = self.generate_vm_config(&metadata, &vm_dir)?;

        // Write configuration file
        let config_path = vm_dir.join("config.json");
        let config_content = serde_json::to_string_pretty(&vm_config)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        tokio::fs::write(&config_path, config_content)
            .await
            .map_err(|e| Error::Io(e))?;

        // Build command
        let command = self.build_vm_command(&metadata, &config_path)?;

        // Apply isolation
        let isolation_context = self.isolation_manager
            .create_context(&vm_id, &metadata.resources)
            .await?;

        // Start process
        let process = self.process_manager
            .start_process(vm_id, command, Some(isolation_context))
            .await?;

        // Start monitoring
        let monitor = Arc::new(ResourceMonitor::new(
            vm_id,
            process.pid(),
            self.config.monitor_config.clone(),
        ));
        monitor.start().await?;

        // Update state
        metadata.state = VmState::Running;
        metadata.updated_at = chrono::Utc::now();

        // Store instance
        let instance = VmInstance {
            metadata,
            process,
            monitor,
        };

        self.instances.write().await.insert(vm_id, instance);

        info!("VM {} started successfully", vm_id);
        Ok(())
    }

    /// Stop a VM instance
    pub async fn stop_vm(&self, vm_id: &VmId, force: bool) -> Result<()> {
        info!("Stopping VM {} (force: {})", vm_id, force);

        let instance = self.instances.write().await.remove(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;

        // Stop monitoring
        instance.monitor.stop().await?;

        // Stop process
        if force {
            self.process_manager.kill_process(&instance.process).await?;
        } else {
            self.process_manager.stop_process(&instance.process).await?;
        }

        // Clean up VM directory
        let vm_dir = self.config.runtime_dir.join(vm_id.to_string());
        if vm_dir.exists() {
            tokio::fs::remove_dir_all(&vm_dir)
                .await
                .map_err(|e| Error::Io(e))?;
        }

        // Clean up isolation
        self.isolation_manager.cleanup_context(vm_id).await?;

        info!("VM {} stopped successfully", vm_id);
        Ok(())
    }

    /// Pause a VM instance
    pub async fn pause_vm(&self, vm_id: &VmId) -> Result<()> {
        let mut instances = self.instances.write().await;
        let instance = instances.get_mut(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;

        if !instance.metadata.state.can_transition_to(&VmState::Paused) {
            return Err(Error::InvalidState(format!(
                "VM {} cannot transition from {:?} to Paused",
                vm_id, instance.metadata.state
            )));
        }

        self.process_manager.pause_process(&instance.process).await?;
        
        instance.metadata.state = VmState::Paused;
        instance.metadata.updated_at = chrono::Utc::now();

        info!("VM {} paused", vm_id);
        Ok(())
    }

    /// Resume a paused VM instance
    pub async fn resume_vm(&self, vm_id: &VmId) -> Result<()> {
        let mut instances = self.instances.write().await;
        let instance = instances.get_mut(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;

        if !instance.metadata.state.can_transition_to(&VmState::Running) {
            return Err(Error::InvalidState(format!(
                "VM {} cannot transition from {:?} to Running",
                vm_id, instance.metadata.state
            )));
        }

        self.process_manager.resume_process(&instance.process).await?;
        
        instance.metadata.state = VmState::Running;
        instance.metadata.updated_at = chrono::Utc::now();

        info!("VM {} resumed", vm_id);
        Ok(())
    }

    /// Get VM metadata
    pub async fn get_vm(&self, vm_id: &VmId) -> Result<VmMetadata> {
        let instances = self.instances.read().await;
        let instance = instances.get(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;
        Ok(instance.metadata.clone())
    }

    /// List all VMs
    pub async fn list_vms(&self) -> Result<Vec<VmMetadata>> {
        let instances = self.instances.read().await;
        Ok(instances.values().map(|i| i.metadata.clone()).collect())
    }

    /// Get resource usage for a VM
    pub async fn get_vm_resources(&self, vm_id: &VmId) -> Result<multivm_core::ResourceUsage> {
        let instances = self.instances.read().await;
        let instance = instances.get(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;
        instance.monitor.get_current_usage().await
    }

    /// Update VM resources (requires restart in most cases)
    pub async fn update_vm_resources(
        &self,
        vm_id: &VmId,
        resources: ResourceRequirements,
    ) -> Result<()> {
        let mut instances = self.instances.write().await;
        let instance = instances.get_mut(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;

        // For now, just update metadata
        // In production, this would require VM restart or hot-plug support
        instance.metadata.resources = resources;
        instance.metadata.updated_at = chrono::Utc::now();

        warn!("VM {} resources updated in metadata only; restart required for changes to take effect", vm_id);
        Ok(())
    }

    /// Check health of all VMs
    pub async fn check_health(&self) -> Result<()> {
        let instances = self.instances.read().await;
        for (vm_id, instance) in instances.iter() {
            if !self.process_manager.is_running(&instance.process).await {
                error!("VM {} process is not running", vm_id);
                // In production, we would handle recovery here
            }
        }
        Ok(())
    }

    /// Generate VM configuration from template
    fn generate_vm_config(
        &self,
        metadata: &VmMetadata,
        vm_dir: &std::path::Path,
    ) -> Result<serde_json::Value> {
        let mut config = self.config.vm_config_template.clone();
        
        // Merge VM-specific configuration
        if let Some(obj) = config.as_object_mut() {
            obj.insert("id".to_string(), serde_json::json!(metadata.id.to_string()));
            obj.insert("name".to_string(), serde_json::json!(metadata.name));
            obj.insert("vcpus".to_string(), serde_json::json!(metadata.resources.cpu_cores));
            obj.insert("memory_mb".to_string(), serde_json::json!(metadata.resources.memory_mb));
            obj.insert("disk_gb".to_string(), serde_json::json!(metadata.resources.disk_gb));
            obj.insert("runtime_dir".to_string(), serde_json::json!(vm_dir.to_string_lossy()));
        }

        Ok(config)
    }

    /// Build VM command line
    fn build_vm_command(
        &self,
        metadata: &VmMetadata,
        config_path: &std::path::Path,
    ) -> Result<Vec<String>> {
        let mut command = vec![self.config.vm_command.clone()];

        // Add basic QEMU options (simplified for example)
        if self.config.vm_command.contains("qemu") {
            command.extend(vec![
                "-name".to_string(), metadata.name.clone(),
                "-m".to_string(), metadata.resources.memory_mb.to_string(),
                "-smp".to_string(), format!("cpus={}", metadata.resources.cpu_cores),
                "-config".to_string(), config_path.to_string_lossy().to_string(),
                "-nographic".to_string(),
            ]);
        } else if self.config.vm_command.contains("firecracker") {
            command.extend(vec![
                "--config-file".to_string(), config_path.to_string_lossy().to_string(),
            ]);
        }

        Ok(command)
    }
}

/// Shutdown all VMs gracefully
impl Drop for VmRuntime {
    fn drop(&mut self) {
        // In production, implement graceful shutdown
        let runtime = tokio::runtime::Handle::try_current();
        if let Ok(handle) = runtime {
            let instances = self.instances.clone();
            handle.spawn(async move {
                let vms = instances.read().await;
                for vm_id in vms.keys() {
                    warn!("VM {} still running during runtime shutdown", vm_id);
                }
            });
        }
    }
}