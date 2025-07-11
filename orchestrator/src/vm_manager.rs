//! VM management for the orchestrator

use multivm_core::{Error, Result, VmId, VmMetadata, VmState, NodeId, ResourceRequirements};
use multivm_runtime::VmRuntime;
use consensus::raft::RaftNode;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};

use crate::scheduler::Scheduler;

/// VM manager
pub struct VmManager {
    runtime: Arc<VmRuntime>,
    scheduler: Arc<Scheduler>,
    consensus: Arc<crate::orchestrator::ConsensusNode>,
    vms: Arc<RwLock<HashMap<VmId, VmMetadata>>>,
}

impl VmManager {
    /// Create a new VM manager
    pub fn new(
        runtime: Arc<VmRuntime>,
        scheduler: Arc<Scheduler>,
        consensus: Arc<crate::orchestrator::ConsensusNode>,
    ) -> Self {
        Self {
            runtime,
            scheduler,
            consensus,
            vms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the VM manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting VM manager");

        // Start VM health check task
        let runtime = self.runtime.clone();
        let vms = self.vms.clone();
        let check_interval = std::time::Duration::from_secs(10);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            loop {
                interval.tick().await;
                if let Err(e) = runtime.check_health().await {
                    error!("VM health check failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Stop the VM manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping VM manager");

        // Stop all VMs
        let vm_ids: Vec<_> = self.vms.read().await.keys().cloned().collect();
        for vm_id in vm_ids {
            if let Err(e) = self.stop_vm(&vm_id, true).await {
                error!("Failed to stop VM {}: {}", vm_id, e);
            }
        }

        Ok(())
    }

    /// Create a new VM
    pub async fn create_vm(
        &self,
        name: String,
        resources: ResourceRequirements,
        labels: HashMap<String, String>,
    ) -> Result<VmId> {
        info!("Creating VM: {}", name);

        // Generate VM ID
        let vm_id = VmId::new();

        // Create metadata
        let mut metadata = VmMetadata {
            id: vm_id,
            name,
            state: VmState::Creating,
            node_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            resources,
            labels,
        };

        // Schedule VM to a node
        let node_id = self.scheduler.schedule_vm(&metadata).await?;
        metadata.node_id = Some(node_id);

        // Store metadata
        self.vms.write().await.insert(vm_id, metadata.clone());

        // Propose to consensus
        let proposal = serde_json::json!({
            "type": "create_vm",
            "vm_id": vm_id,
            "metadata": metadata,
        });
        
        let proposal_bytes = serde_json::to_vec(&proposal)
            .map_err(|e| Error::Serialization(format!("Failed to serialize proposal: {}", e)))?;
        self.consensus.propose(proposal_bytes).await.map_err(|e| Error::Consensus(e.to_string()))?;

        info!("VM {} created and scheduled to node {}", vm_id, node_id);
        Ok(vm_id)
    }

    /// Start a VM
    pub async fn start_vm(&self, vm_id: &VmId) -> Result<()> {
        info!("Starting VM {}", vm_id);

        let metadata = {
            let vms = self.vms.read().await;
            vms.get(vm_id)
                .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?
                .clone()
        };

        // Check state transition
        if !metadata.state.can_transition_to(&VmState::Running) {
            return Err(Error::InvalidState(format!(
                "VM {} cannot transition from {:?} to Running",
                vm_id, metadata.state
            )));
        }

        // Start on runtime
        self.runtime.start_vm(metadata).await?;

        // Update state
        self.update_vm_state(vm_id, VmState::Running).await?;

        info!("VM {} started", vm_id);
        Ok(())
    }

    /// Stop a VM
    pub async fn stop_vm(&self, vm_id: &VmId, force: bool) -> Result<()> {
        info!("Stopping VM {} (force: {})", vm_id, force);

        let metadata = {
            let vms = self.vms.read().await;
            vms.get(vm_id)
                .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?
                .clone()
        };

        // Check state transition
        if !metadata.state.can_transition_to(&VmState::Stopped) {
            if !force {
                return Err(Error::InvalidState(format!(
                    "VM {} cannot transition from {:?} to Stopped",
                    vm_id, metadata.state
                )));
            }
        }

        // Stop on runtime
        self.runtime.stop_vm(vm_id, force).await?;

        // Update state
        self.update_vm_state(vm_id, VmState::Stopped).await?;

        info!("VM {} stopped", vm_id);
        Ok(())
    }

    /// Delete a VM
    pub async fn delete_vm(&self, vm_id: &VmId) -> Result<()> {
        info!("Deleting VM {}", vm_id);

        // Stop if running
        let metadata = self.get_vm(vm_id).await?;
        if metadata.state == VmState::Running {
            self.stop_vm(vm_id, true).await?;
        }

        // Remove from storage
        self.vms.write().await.remove(vm_id);

        // Propose deletion to consensus
        let proposal = serde_json::json!({
            "type": "delete_vm",
            "vm_id": vm_id,
        });
        
        let proposal_bytes = serde_json::to_vec(&proposal)
            .map_err(|e| Error::Serialization(format!("Failed to serialize proposal: {}", e)))?;
        self.consensus.propose(proposal_bytes).await.map_err(|e| Error::Consensus(e.to_string()))?;

        info!("VM {} deleted", vm_id);
        Ok(())
    }

    /// Get VM metadata
    pub async fn get_vm(&self, vm_id: &VmId) -> Result<VmMetadata> {
        let vms = self.vms.read().await;
        vms.get(vm_id)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))
    }

    /// List VMs
    pub async fn list_vms(&self, label_selector: HashMap<String, String>) -> Result<Vec<VmMetadata>> {
        let vms = self.vms.read().await;
        
        if label_selector.is_empty() {
            Ok(vms.values().cloned().collect())
        } else {
            Ok(vms.values()
                .filter(|vm| {
                    label_selector.iter().all(|(k, v)| {
                        vm.labels.get(k).map_or(false, |val| val == v)
                    })
                })
                .cloned()
                .collect())
        }
    }

    /// Migrate VM to another node
    pub async fn migrate_vm(
        &self,
        vm_id: &VmId,
        target_node: &NodeId,
        live: bool,
    ) -> Result<()> {
        info!("Migrating VM {} to node {} (live: {})", vm_id, target_node, live);

        let mut metadata = self.get_vm(vm_id).await?;

        // Check if already on target node
        if metadata.node_id.as_ref() == Some(target_node) {
            return Ok(());
        }

        // Update state
        self.update_vm_state(vm_id, VmState::Migrating).await?;

        // Implement VM migration
        if live {
            // Live migration: transfer VM state while running
            info!("Performing live migration of VM {} to node {}", vm_id, target_node);
            
            // Step 1: Notify source and target nodes
            let migration_request = serde_json::json!({
                "type": "migrate_vm",
                "vm_id": vm_id,
                "source_node": metadata.node_id,
                "target_node": target_node,
                "live": true,
            });
            
            // Step 2: Initiate state transfer
            let proposal_bytes = serde_json::to_vec(&migration_request)
                .map_err(|e| Error::Serialization(format!("Failed to serialize migration request: {}", e)))?;
            self.consensus.propose(proposal_bytes).await
                .map_err(|e| Error::Consensus(format!("Failed to propose migration: {}", e)))?;
            
            // Step 3: Wait for migration to complete (simulated with timeout)
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        } else {
            // Cold migration: stop VM, transfer, restart
            info!("Performing cold migration of VM {} to node {}", vm_id, target_node);
            
            // Stop VM if running
            if metadata.state == VmState::Running {
                self.stop_vm(vm_id, false).await?;
            }
            
            // Transfer VM configuration
            let migration_request = serde_json::json!({
                "type": "migrate_vm",
                "vm_id": vm_id,
                "source_node": metadata.node_id,
                "target_node": target_node,
                "live": false,
            });
            
            let proposal_bytes = serde_json::to_vec(&migration_request)
                .map_err(|e| Error::Serialization(format!("Failed to serialize migration request: {}", e)))?;
            self.consensus.propose(proposal_bytes).await
                .map_err(|e| Error::Consensus(format!("Failed to propose migration: {}", e)))?;
        }
        
        // Update node assignment
        metadata.node_id = Some(*target_node);
        metadata.updated_at = chrono::Utc::now();
        self.vms.write().await.insert(*vm_id, metadata.clone());

        // For cold migration, restart VM on new node
        if !live && metadata.state == VmState::Running {
            self.start_vm(vm_id).await?;
        } else {
            // For live migration, just update state
            self.update_vm_state(vm_id, VmState::Running).await?;
        }

        info!("VM {} migrated to node {}", vm_id, target_node);
        Ok(())
    }

    /// Update VM state
    async fn update_vm_state(&self, vm_id: &VmId, state: VmState) -> Result<()> {
        let mut vms = self.vms.write().await;
        let metadata = vms.get_mut(vm_id)
            .ok_or_else(|| Error::NotFound(format!("VM {} not found", vm_id)))?;

        metadata.state = state;
        metadata.updated_at = chrono::Utc::now();

        debug!("VM {} state updated to {:?}", vm_id, state);
        Ok(())
    }

    /// Apply a consensus proposal
    pub async fn apply_proposal(&self, data: &[u8]) -> Result<()> {
        let proposal: serde_json::Value = serde_json::from_slice(data)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        match proposal["type"].as_str() {
            Some("create_vm") => {
                let metadata: VmMetadata = serde_json::from_value(proposal["metadata"].clone())
                    .map_err(|e| Error::Serialization(e.to_string()))?;
                self.vms.write().await.insert(metadata.id, metadata);
            }
            Some("delete_vm") => {
                let vm_id: VmId = serde_json::from_value(proposal["vm_id"].clone())
                    .map_err(|e| Error::Serialization(e.to_string()))?;
                self.vms.write().await.remove(&vm_id);
            }
            _ => {
                warn!("Unknown proposal type: {:?}", proposal["type"]);
            }
        }

        Ok(())
    }
}