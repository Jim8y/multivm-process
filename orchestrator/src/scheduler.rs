//! VM scheduling logic

use multivm_core::{Error, Result, NodeId, VmMetadata};
use std::sync::Arc;
use tracing::{info, debug};

use crate::node_manager::{NodeManager, NodeState};

/// Scheduling strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingStrategy {
    /// Least loaded node
    LeastLoaded,
    /// Round robin
    RoundRobin,
    /// Bin packing
    BinPacking,
    /// Spread across nodes
    Spread,
}

/// Scheduler for VM placement
pub struct Scheduler {
    node_manager: Arc<NodeManager>,
    strategy: SchedulingStrategy,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(node_manager: Arc<NodeManager>) -> Self {
        Self {
            node_manager,
            strategy: SchedulingStrategy::LeastLoaded,
        }
    }

    /// Schedule a VM to a node
    pub async fn schedule_vm(&self, vm: &VmMetadata) -> Result<NodeId> {
        info!("Scheduling VM {} with strategy {:?}", vm.id, self.strategy);

        let nodes = self.node_manager.get_healthy_nodes().await;
        if nodes.is_empty() {
            return Err(Error::ResourceExhausted("No healthy nodes available".to_string()));
        }

        let selected_node = match self.strategy {
            SchedulingStrategy::LeastLoaded => self.select_least_loaded(&nodes, vm).await?,
            SchedulingStrategy::RoundRobin => self.select_round_robin(&nodes).await?,
            SchedulingStrategy::BinPacking => self.select_bin_packing(&nodes, vm).await?,
            SchedulingStrategy::Spread => self.select_spread(&nodes).await?,
        };

        info!("VM {} scheduled to node {}", vm.id, selected_node);
        Ok(selected_node)
    }

    /// Select least loaded node
    async fn select_least_loaded(
        &self,
        nodes: &[crate::node_manager::NodeStatus],
        vm: &VmMetadata,
    ) -> Result<NodeId> {
        // Find node with lowest CPU usage that has enough resources
        let selected = nodes.iter()
            .filter(|node| self.has_sufficient_resources(node, vm))
            .min_by_key(|node| (node.resource_usage.cpu_percent * 100.0) as u32)
            .ok_or_else(|| Error::ResourceExhausted(
                "No node has sufficient resources".to_string()
            ))?;

        debug!("Selected node {} with {}% CPU usage", 
               selected.node_id, selected.resource_usage.cpu_percent);
        
        Ok(selected.node_id)
    }

    /// Select node using round-robin
    async fn select_round_robin(
        &self,
        nodes: &[crate::node_manager::NodeStatus],
    ) -> Result<NodeId> {
        // Simple round-robin: select node with fewest VMs
        let selected = nodes.iter()
            .min_by_key(|node| node.vm_ids.len())
            .ok_or_else(|| Error::ResourceExhausted("No nodes available".to_string()))?;

        Ok(selected.node_id)
    }

    /// Select node using bin packing
    async fn select_bin_packing(
        &self,
        nodes: &[crate::node_manager::NodeStatus],
        vm: &VmMetadata,
    ) -> Result<NodeId> {
        // Bin packing: fill nodes to capacity before using new ones
        let selected = nodes.iter()
            .filter(|node| self.has_sufficient_resources(node, vm))
            .max_by_key(|node| node.vm_ids.len())
            .ok_or_else(|| Error::ResourceExhausted(
                "No node has sufficient resources".to_string()
            ))?;

        Ok(selected.node_id)
    }

    /// Select node using spread strategy
    async fn select_spread(
        &self,
        nodes: &[crate::node_manager::NodeStatus],
    ) -> Result<NodeId> {
        // Spread: distribute VMs evenly across nodes
        let selected = nodes.iter()
            .min_by_key(|node| node.vm_ids.len())
            .ok_or_else(|| Error::ResourceExhausted("No nodes available".to_string()))?;

        Ok(selected.node_id)
    }

    /// Check if node has sufficient resources for VM
    fn has_sufficient_resources(
        &self,
        node: &crate::node_manager::NodeStatus,
        vm: &VmMetadata,
    ) -> bool {
        // Simple check - in production would need more sophisticated logic
        // accounting for existing VM resource usage
        
        // Assume node has 100 CPU cores and 256GB RAM for now
        let available_cpu = 100.0 * (1.0 - node.resource_usage.cpu_percent / 100.0);
        let available_memory = 256 * 1024 - node.resource_usage.memory_mb;

        available_cpu >= vm.resources.cpu_cores &&
        available_memory >= vm.resources.memory_mb
    }

    /// Set scheduling strategy
    pub fn set_strategy(&mut self, strategy: SchedulingStrategy) {
        self.strategy = strategy;
        info!("Scheduling strategy changed to {:?}", strategy);
    }

    /// Find best node for VM migration
    pub async fn find_migration_target(
        &self,
        vm: &VmMetadata,
        exclude_node: &NodeId,
    ) -> Result<NodeId> {
        let nodes = self.node_manager.get_healthy_nodes().await;
        
        let candidates: Vec<_> = nodes.into_iter()
            .filter(|node| node.node_id != *exclude_node)
            .filter(|node| self.has_sufficient_resources(&node, vm))
            .collect();

        if candidates.is_empty() {
            return Err(Error::ResourceExhausted("No suitable migration target found".to_string()));
        }

        // Use current strategy to select target
        self.schedule_vm(vm).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduling_strategy() {
        assert_eq!(SchedulingStrategy::LeastLoaded, SchedulingStrategy::LeastLoaded);
        assert_ne!(SchedulingStrategy::LeastLoaded, SchedulingStrategy::RoundRobin);
    }
}