//! Node management for the cluster

use multivm_core::{Error, Result, NodeId, ClusterMember, ResourceUsage, VmId};
use consensus::raft::RaftNode;
use multivm_network::tcp::TcpTransport;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};
use chrono::{DateTime, Utc};

/// Node status information
#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub node_id: NodeId,
    pub address: std::net::SocketAddr,
    pub state: NodeState,
    pub resource_usage: ResourceUsage,
    pub vm_ids: Vec<VmId>,
    pub last_heartbeat: DateTime<Utc>,
}

/// Node state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Active,
    Draining,
    Maintenance,
    Failed,
}

/// Node information
#[derive(Debug, Clone)]
struct NodeInfo {
    member: ClusterMember,
    state: NodeState,
    resource_usage: ResourceUsage,
    vm_ids: Vec<VmId>,
    last_heartbeat: DateTime<Utc>,
}

/// Node manager
pub struct NodeManager {
    node_id: NodeId,
    consensus: Arc<crate::orchestrator::ConsensusNode>,
    network: Arc<TcpTransport>,
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
}

impl NodeManager {
    /// Create a new node manager
    pub fn new(
        node_id: NodeId,
        consensus: Arc<crate::orchestrator::ConsensusNode>,
        network: Arc<TcpTransport>,
    ) -> Self {
        Self {
            node_id,
            consensus,
            network,
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the node manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting node manager");

        // Start health check task
        let nodes = self.nodes.clone();
        let check_interval = std::time::Duration::from_secs(30);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            loop {
                interval.tick().await;
                Self::check_node_health(&nodes).await;
            }
        });

        Ok(())
    }

    /// Stop the node manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping node manager");
        Ok(())
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, member: ClusterMember) -> Result<()> {
        info!("Adding node {} to cluster", member.id);

        let node_info = NodeInfo {
            member: member.clone(),
            state: NodeState::Active,
            resource_usage: ResourceUsage::default(),
            vm_ids: Vec::new(),
            last_heartbeat: Utc::now(),
        };

        self.nodes.write().await.insert(member.id, node_info);

        // Notify consensus layer
        self.consensus.add_node(member).await.map_err(|e| Error::Consensus(e.to_string()))?;

        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: &NodeId) -> Result<()> {
        info!("Removing node {} from cluster", node_id);

        let removed = self.nodes.write().await.remove(node_id);
        if removed.is_none() {
            return Err(Error::NotFound(format!("Node {} not found", node_id)));
        }

        // Notify consensus layer
        let consensus_node_id = consensus::NodeId::from_bytes(*node_id.as_bytes());
        self.consensus.remove_node(consensus_node_id).await.map_err(|e| Error::Consensus(e.to_string()))?;

        Ok(())
    }

    /// Get all nodes
    pub async fn get_nodes(&self) -> Vec<ClusterMember> {
        self.nodes.read().await
            .values()
            .map(|info| info.member.clone())
            .collect()
    }

    /// Get node status
    pub async fn get_node_status(&self, node_id: &NodeId) -> Result<NodeStatus> {
        let nodes = self.nodes.read().await;
        let info = nodes.get(node_id)
            .ok_or_else(|| Error::NotFound(format!("Node {} not found", node_id)))?;

        Ok(NodeStatus {
            node_id: *node_id,
            address: info.member.address,
            state: info.state,
            resource_usage: info.resource_usage.clone(),
            vm_ids: info.vm_ids.clone(),
            last_heartbeat: info.last_heartbeat,
        })
    }

    /// Update node heartbeat
    pub async fn update_heartbeat(&self, node_id: NodeId, usage: ResourceUsage) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let info = nodes.get_mut(&node_id)
            .ok_or_else(|| Error::NotFound(format!("Node {} not found", node_id)))?;

        info.resource_usage = usage;
        info.last_heartbeat = Utc::now();

        debug!("Updated heartbeat for node {}", node_id);
        Ok(())
    }

    /// Set node state
    pub async fn set_node_state(&self, node_id: &NodeId, state: NodeState) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let info = nodes.get_mut(node_id)
            .ok_or_else(|| Error::NotFound(format!("Node {} not found", node_id)))?;

        info.state = state;
        info!("Node {} state changed to {:?}", node_id, state);

        Ok(())
    }

    /// Add VM to node
    pub async fn add_vm_to_node(&self, node_id: &NodeId, vm_id: VmId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let info = nodes.get_mut(node_id)
            .ok_or_else(|| Error::NotFound(format!("Node {} not found", node_id)))?;

        if !info.vm_ids.contains(&vm_id) {
            info.vm_ids.push(vm_id);
            debug!("Added VM {} to node {}", vm_id, node_id);
        }

        Ok(())
    }

    /// Remove VM from node
    pub async fn remove_vm_from_node(&self, node_id: &NodeId, vm_id: &VmId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let info = nodes.get_mut(node_id)
            .ok_or_else(|| Error::NotFound(format!("Node {} not found", node_id)))?;

        info.vm_ids.retain(|id| id != vm_id);
        debug!("Removed VM {} from node {}", vm_id, node_id);

        Ok(())
    }

    /// Get healthy nodes
    pub async fn get_healthy_nodes(&self) -> Vec<NodeStatus> {
        let nodes = self.nodes.read().await;
        let timeout = chrono::Duration::seconds(60);
        let now = Utc::now();

        nodes.values()
            .filter(|info| {
                info.state == NodeState::Active &&
                now.signed_duration_since(info.last_heartbeat) < timeout
            })
            .map(|info| NodeStatus {
                node_id: info.member.id,
                address: info.member.address,
                state: info.state,
                resource_usage: info.resource_usage.clone(),
                vm_ids: info.vm_ids.clone(),
                last_heartbeat: info.last_heartbeat,
            })
            .collect()
    }

    /// Get leader node ID
    pub async fn get_leader_id(&self) -> Option<NodeId> {
        if self.consensus.is_leader().await {
            Some(self.node_id)
        } else {
            // In a real implementation, we would query the consensus layer
            // For now, return None if we're not the leader
            None
        }
    }
    
    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        self.consensus.is_leader().await
    }

    /// Check node health
    async fn check_node_health(nodes: &Arc<RwLock<HashMap<NodeId, NodeInfo>>>) {
        let mut nodes = nodes.write().await;
        let timeout = chrono::Duration::seconds(60);
        let now = Utc::now();

        for (node_id, info) in nodes.iter_mut() {
            if now.signed_duration_since(info.last_heartbeat) > timeout {
                if info.state != NodeState::Failed {
                    warn!("Node {} appears to be down", node_id);
                    info.state = NodeState::Failed;
                }
            }
        }
    }
}

impl std::fmt::Display for NodeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeState::Active => write!(f, "active"),
            NodeState::Draining => write!(f, "draining"),
            NodeState::Maintenance => write!(f, "maintenance"),
            NodeState::Failed => write!(f, "failed"),
        }
    }
}