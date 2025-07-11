//! Main orchestrator implementation

use multivm_core::{Error, Result, NodeId, ClusterMember};
use consensus::{StateMachine, raft::{RaftNode, Config as RaftConfig, Command, Response}};
use multivm_network::{TcpTransport, TcpTransportConfig};
use multivm_storage::{rocksdb::RocksDbStorage, StorageConfig as RocksDbConfig};
use multivm_runtime::{VmRuntime, VmRuntimeConfig};
use multivm_rpc::{RpcServer, RpcServerConfig};
use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use async_trait::async_trait;
use serde_json;
use multivm_rpc::prost_types;

use crate::node_manager::NodeManager;
use crate::vm_manager::VmManager;
use crate::scheduler::Scheduler;

/// Type alias for the consensus node type used in the orchestrator
pub type ConsensusNode = RaftNode<
    multivm_storage::RocksDbStorage,
    multivm_network::TcpTransport,
    OrchestratorStateMachine,
>;

/// Orchestrator state machine for consensus
#[derive(Debug)]
pub struct OrchestratorStateMachine {
    /// VM states and metadata
    vm_state: HashMap<String, String>,
    /// Cluster configuration
    cluster_config: HashMap<String, String>,
}

impl OrchestratorStateMachine {
    fn new() -> Self {
        Self {
            vm_state: HashMap::new(),
            cluster_config: HashMap::new(),
        }
    }
}

#[async_trait]
impl StateMachine for OrchestratorStateMachine {
    type Command = Command;
    type Response = Response;
    
    async fn apply(&mut self, command: Self::Command) -> Self::Response {
        let cmd_str = String::from_utf8_lossy(&command.data);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        
        match parts.as_slice() {
            ["SET_VM_STATE", vm_id, state] => {
                self.vm_state.insert(vm_id.to_string(), state.to_string());
                Response {
                    success: true,
                    data: b"OK".to_vec(),
                }
            }
            ["GET_VM_STATE", vm_id] => {
                if let Some(state) = self.vm_state.get(*vm_id) {
                    Response {
                        success: true,
                        data: state.as_bytes().to_vec(),
                    }
                } else {
                    Response {
                        success: false,
                        data: b"VM not found".to_vec(),
                    }
                }
            }
            ["SET_CONFIG", key, value] => {
                self.cluster_config.insert(key.to_string(), value.to_string());
                Response {
                    success: true,
                    data: b"OK".to_vec(),
                }
            }
            _ => Response {
                success: false,
                data: b"Unknown command".to_vec(),
            }
        }
    }
    
    async fn snapshot(&self) -> consensus::Result<Vec<u8>> {
        let state = serde_json::json!({
            "vm_state": self.vm_state,
            "cluster_config": self.cluster_config
        });
        serde_json::to_vec(&state)
            .map_err(|e| consensus::ConsensusError::Other(format!("Failed to serialize snapshot: {}", e)))
    }
    
    async fn restore(&mut self, snapshot: &[u8]) -> consensus::Result<()> {
        let state: serde_json::Value = serde_json::from_slice(snapshot)
            .map_err(|e| consensus::ConsensusError::Other(format!("Failed to deserialize snapshot: {}", e)))?;
        if let Some(vm_state) = state.get("vm_state") {
            self.vm_state = serde_json::from_value(vm_state.clone())
                .map_err(|e| consensus::ConsensusError::Other(format!("Failed to deserialize vm_state: {}", e)))?;
        }
        if let Some(cluster_config) = state.get("cluster_config") {
            self.cluster_config = serde_json::from_value(cluster_config.clone())
                .map_err(|e| consensus::ConsensusError::Other(format!("Failed to deserialize cluster_config: {}", e)))?;
        }
        Ok(())
    }
}

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Node ID
    pub node_id: NodeId,
    /// Listen address for cluster communication
    pub cluster_addr: SocketAddr,
    /// Listen address for RPC
    pub rpc_addr: SocketAddr,
    /// Data directory
    pub data_dir: std::path::PathBuf,
    /// Cluster members
    pub cluster_members: Vec<ClusterMember>,
    /// VM runtime configuration
    pub runtime_config: VmRuntimeConfig,
    /// Enable leader election
    pub enable_leader_election: bool,
}

/// Main orchestrator
#[derive(Clone)]
pub struct Orchestrator {
    config: OrchestratorConfig,
    consensus: Arc<ConsensusNode>,
    network: Arc<TcpTransport>,
    storage: Arc<RocksDbStorage>,
    runtime: Arc<VmRuntime>,
    node_manager: Arc<NodeManager>,
    vm_manager: Arc<VmManager>,
    scheduler: Arc<Scheduler>,
    rpc_server: RpcServer,
    shutdown: Arc<RwLock<bool>>,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub async fn new(config: OrchestratorConfig) -> Result<Self> {
        info!("Creating orchestrator for node {}", config.node_id);

        // Create storage
        let storage_config = RocksDbConfig {
            data_dir: config.data_dir.join("storage"),
            ..Default::default()
        };
        let storage: Arc<RocksDbStorage> = Arc::new(RocksDbStorage::new(storage_config.clone()).await.map_err(|e| Error::Storage(e.to_string()))?);

        // Create network transport with TLS if configured
        let tls_config = if let Ok(app_config) = crate::config::load_config(None) {
            if app_config.network.enable_tls {
                if let (Some(cert_path), Some(key_path)) = (
                    app_config.network.tls_cert_path.as_ref(),
                    app_config.network.tls_key_path.as_ref()
                ) {
                    Some(multivm_network::tls::TlsConfig {
                        cert_path: cert_path.to_string_lossy().to_string(),
                        key_path: key_path.to_string_lossy().to_string(),
                        ca_cert_path: None,
                        verify_hostname: false,
                    })
                } else {
                    warn!("TLS enabled but certificate paths not configured, falling back to insecure connection");
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        
        let network_config = TcpTransportConfig {
            listen_addr: config.cluster_addr,
            tls: tls_config,
            ..Default::default()
        };
        // Create peer mapping from cluster members - convert NodeId types
        let mut peers = std::collections::HashMap::new();
        for member in &config.cluster_members {
            let consensus_node_id = consensus::NodeId::from_bytes(*member.id.as_bytes());
            peers.insert(consensus_node_id, member.address);
        }
        
        let consensus_node_id = consensus::NodeId::from_bytes(*config.node_id.as_bytes());
        let network = TcpTransport::new(
            consensus_node_id,
            network_config.clone(),
            peers.clone(),
        ).await.map_err(|e| Error::Network(e.to_string()))?;
        
        // Create a separate network instance for other components
        let network_for_manager = TcpTransport::new(
            consensus_node_id,
            network_config.clone(),
            peers.clone(),
        ).await.map_err(|e| Error::Network(e.to_string()))?;

        // Create consensus with all cluster members
        let mut consensus_peers = vec![consensus_node_id];
        for member in &config.cluster_members {
            let peer_id = consensus::NodeId::from_bytes(*member.id.as_bytes());
            if peer_id != consensus_node_id {
                consensus_peers.push(peer_id);
            }
        }
        let consensus_config = RaftConfig::new(consensus_node_id, consensus_peers);
        // Create state machine
        let state_machine = OrchestratorStateMachine::new();
        
        let consensus: Arc<ConsensusNode> = Arc::new(RaftNode::new(
            consensus_config,
            RocksDbStorage::new(storage_config).await.map_err(|e| Error::Storage(e.to_string()))?,
            network,
            state_machine,
        ).await.map_err(|e| Error::Consensus(e.to_string()))?);

        // Create VM runtime
        let runtime = Arc::new(VmRuntime::new(config.runtime_config.clone()).await?);

        // Create second network instance for managers
        let network_for_managers = TcpTransport::new(
            consensus_node_id,
            network_config.clone(),
            peers.clone(),
        ).await.map_err(|e| Error::Network(e.to_string()))?;
        
        // Create managers
        let node_manager = Arc::new(NodeManager::new(
            config.node_id,
            consensus.clone(),
            Arc::new(network_for_managers),
        ));
        
        let scheduler = Arc::new(Scheduler::new(node_manager.clone()));
        
        let vm_manager = Arc::new(VmManager::new(
            runtime.clone(),
            scheduler.clone(),
            consensus.clone(),
        ));

        // Create RPC server
        let rpc_config = RpcServerConfig {
            listen_addr: config.rpc_addr,
            ..Default::default()
        };
        let rpc_server = RpcServer::new(rpc_config);

        Ok(Self {
            config,
            consensus,
            network: Arc::new(network_for_manager),
            storage,
            runtime,
            node_manager,
            vm_manager,
            scheduler,
            rpc_server,
            shutdown: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the orchestrator
    pub async fn start(&self) -> Result<()> {
        info!("Starting orchestrator");

        // Start consensus
        self.consensus.start().await.map_err(|e| Error::Consensus(e.to_string()))?;

        // Initialize cluster if needed
        if !self.config.cluster_members.is_empty() {
            self.initialize_cluster().await?;
        }

        // Start node manager
        self.node_manager.start().await?;

        // Start VM manager
        self.vm_manager.start().await?;

        // Start RPC server
        let node_state = Arc::new(NodeStateImpl {
            node_manager: self.node_manager.clone(),
        });
        let vm_state = Arc::new(VmStateImpl {
            vm_manager: self.vm_manager.clone(),
        });

        let rpc_server = self.rpc_server.clone();
        let rpc_handle = tokio::spawn(async move {
            if let Err(e) = rpc_server.run(node_state, vm_state).await {
                error!("RPC server error: {}", e);
            }
        });

        info!("Orchestrator started successfully");

        // Wait for shutdown
        self.wait_for_shutdown().await;

        // Stop components
        rpc_handle.abort();
        self.vm_manager.stop().await?;
        self.node_manager.stop().await?;
        self.consensus.stop().await.map_err(|e| Error::Consensus(e.to_string()))?;

        info!("Orchestrator stopped");
        Ok(())
    }

    /// Initialize cluster
    async fn initialize_cluster(&self) -> Result<()> {
        info!("Initializing cluster with {} members", self.config.cluster_members.len());

        // Add initial members
        for member in &self.config.cluster_members {
            self.node_manager.add_node(member.clone()).await?;
        }

        // Join cluster
        if self.config.enable_leader_election {
            self.consensus.join_cluster(self.config.cluster_members.clone()).await.map_err(|e| Error::Consensus(e.to_string()))?;
        }

        Ok(())
    }

    /// Wait for shutdown signal
    async fn wait_for_shutdown(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if *self.shutdown.read().await {
                break;
            }
        }
    }

    /// Shutdown the orchestrator
    pub async fn shutdown(&self) {
        info!("Shutting down orchestrator");
        *self.shutdown.write().await = true;
    }

    /// Get node manager
    pub fn node_manager(&self) -> &Arc<NodeManager> {
        &self.node_manager
    }

    /// Get VM manager
    pub fn vm_manager(&self) -> &Arc<VmManager> {
        &self.vm_manager
    }

    /// Get scheduler
    pub fn scheduler(&self) -> &Arc<Scheduler> {
        &self.scheduler
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        self.consensus.is_leader().await
    }
}

/// Node state implementation for RPC
struct NodeStateImpl {
    node_manager: Arc<NodeManager>,
}

#[async_trait]
impl multivm_rpc::server::NodeState for NodeStateImpl {
    async fn join_cluster(
        &self,
        req: multivm_rpc::JoinClusterRequest,
    ) -> Result<Vec<multivm_rpc::ClusterNode>> {
        let node_id = NodeId::try_from_slice(req.node_id.as_bytes())
            .map_err(|_| Error::InvalidInput("Invalid node ID".to_string()))?;
        
        let address: SocketAddr = req.address.parse()
            .map_err(|_| Error::InvalidInput("Invalid address".to_string()))?;

        let member = ClusterMember::new(node_id, address);
        self.node_manager.add_node(member).await?;

        // Return current cluster members
        let members = self.node_manager.get_nodes().await;
        let leader_id = self.node_manager.get_leader_id().await;
        Ok(members.into_iter().map(|m| multivm_rpc::ClusterNode {
            node_id: m.id.to_string(),
            address: m.address.to_string(),
            is_leader: leader_id.as_ref() == Some(&m.id),
            last_seen: None,
        }).collect())
    }

    async fn leave_cluster(&self, node_id: &str) -> Result<()> {
        let node_id = NodeId::try_from_slice(node_id.as_bytes())
            .map_err(|_| Error::InvalidInput("Invalid node ID".to_string()))?;
        self.node_manager.remove_node(&node_id).await
    }

    async fn get_node_status(&self, node_id: &str) -> Result<multivm_rpc::NodeStatus> {
        let node_id = NodeId::try_from_slice(node_id.as_bytes())
            .map_err(|_| Error::InvalidInput("Invalid node ID".to_string()))?;
        
        let status = self.node_manager.get_node_status(&node_id).await?;
        
        Ok(multivm_rpc::NodeStatus {
            node_id: node_id.to_string(),
            state: status.state.to_string(),
            resource_usage: Some(status.resource_usage.into()),
            vm_ids: status.vm_ids.into_iter().map(|id| id.to_string()).collect(),
        })
    }

    async fn update_heartbeat(&self, req: multivm_rpc::HeartbeatRequest) -> Result<()> {
        let node_id = NodeId::try_from_slice(req.node_id.as_bytes())
            .map_err(|_| Error::InvalidInput("Invalid node ID".to_string()))?;
        
        let usage = req.resource_usage
            .ok_or_else(|| Error::InvalidInput("Missing resource usage".to_string()))?
            .into();
        
        self.node_manager.update_heartbeat(node_id, usage).await
    }
}

/// VM state implementation for RPC
struct VmStateImpl {
    vm_manager: Arc<VmManager>,
}

#[async_trait]
impl multivm_rpc::server::VmState for VmStateImpl {
    async fn create_vm(&self, req: multivm_rpc::CreateVmRequest) -> Result<String> {
        let resources = req.resources
            .ok_or_else(|| Error::InvalidInput("Missing resources".to_string()))?
            .try_into()?;
        
        let vm_id = self.vm_manager.create_vm(req.name, resources, req.labels).await?;
        Ok(vm_id.to_string())
    }

    async fn start_vm(&self, vm_id: &str) -> Result<()> {
        let vm_id = multivm_core::VmId::try_from_str(vm_id)?;
        self.vm_manager.start_vm(&vm_id).await
    }

    async fn stop_vm(&self, vm_id: &str, force: bool) -> Result<()> {
        let vm_id = multivm_core::VmId::try_from_str(vm_id)?;
        self.vm_manager.stop_vm(&vm_id, force).await
    }

    async fn delete_vm(&self, vm_id: &str) -> Result<()> {
        let vm_id = multivm_core::VmId::try_from_str(vm_id)?;
        self.vm_manager.delete_vm(&vm_id).await
    }

    async fn get_vm(&self, vm_id: &str) -> Result<multivm_rpc::VmInfo> {
        let vm_id = multivm_core::VmId::try_from_str(vm_id)?;
        
        let metadata = self.vm_manager.get_vm(&vm_id).await?;
        
        Ok(multivm_rpc::VmInfo {
            vm_id: metadata.id.to_string(),
            name: metadata.name,
            state: format!("{:?}", metadata.state),
            node_id: metadata.node_id.map(|id| id.to_string()).unwrap_or_default(),
            created_at: Some(prost_types::Timestamp {
                seconds: metadata.created_at.timestamp(),
                nanos: metadata.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: metadata.updated_at.timestamp(),
                nanos: metadata.updated_at.timestamp_subsec_nanos() as i32,
            }),
            resources: Some(metadata.resources.into()),
            labels: metadata.labels,
        })
    }

    async fn list_vms(&self, req: multivm_rpc::ListVmsRequest) -> Result<Vec<multivm_rpc::VmInfo>> {
        let vms = self.vm_manager.list_vms(req.label_selector).await?;
        
        Ok(vms.into_iter().map(|metadata| multivm_rpc::VmInfo {
            vm_id: metadata.id.to_string(),
            name: metadata.name,
            state: format!("{:?}", metadata.state),
            node_id: metadata.node_id.map(|id| id.to_string()).unwrap_or_default(),
            created_at: Some(prost_types::Timestamp {
                seconds: metadata.created_at.timestamp(),
                nanos: metadata.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: metadata.updated_at.timestamp(),
                nanos: metadata.updated_at.timestamp_subsec_nanos() as i32,
            }),
            resources: Some(metadata.resources.into()),
            labels: metadata.labels,
        }).collect())
    }

    async fn migrate_vm(&self, req: multivm_rpc::MigrateVmRequest) -> Result<()> {
        let vm_id = multivm_core::VmId::try_from_str(&req.vm_id)?;
        
        let target_node = NodeId::try_from_slice(req.target_node_id.as_bytes())
            .map_err(|_| Error::InvalidInput("Invalid target node ID".to_string()))?;
        
        self.vm_manager.migrate_vm(&vm_id, &target_node, req.live).await
    }
}