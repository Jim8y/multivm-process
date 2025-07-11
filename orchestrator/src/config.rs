//! Configuration management for the orchestrator

use multivm_core::{NodeId, ClusterMember};
use multivm_runtime::VmRuntimeConfig;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,
    /// Cluster configuration
    pub cluster: ClusterConfig,
    /// Runtime configuration
    pub runtime: RuntimeConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Network configuration
    pub network: NetworkConfig,
    /// RPC configuration
    pub rpc: RpcConfig,
}

/// Node-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node ID (will be generated if not specified)
    pub id: Option<String>,
    /// Node name
    pub name: String,
    /// Data directory
    pub data_dir: PathBuf,
    /// Log level
    pub log_level: String,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Listen address for cluster communication
    pub listen_addr: SocketAddr,
    /// Initial cluster members (for bootstrapping)
    pub cluster_members: Vec<ClusterMemberConfig>,
    /// Enable leader election
    pub enable_leader_election: bool,
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
    /// Election timeout in milliseconds
    pub election_timeout_ms: u64,
}

/// Cluster member configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMemberConfig {
    /// Node ID
    pub id: String,
    /// Node address
    pub address: SocketAddr,
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// VM executable (e.g., qemu-system-x86_64)
    pub vm_command: String,
    /// Maximum VMs per node
    pub max_vms_per_node: usize,
    /// VM configuration template
    pub vm_config_template: serde_json::Value,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    pub backend: StorageBackend,
    /// RocksDB specific options
    pub rocksdb: RocksDbOptions,
}

/// Storage backend type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    RocksDb,
    Memory,
}

/// RocksDB options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocksDbOptions {
    /// Cache size in MB
    pub cache_size_mb: usize,
    /// Enable compression
    pub compression: bool,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable TLS
    pub enable_tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<PathBuf>,
    /// TLS key path
    pub tls_key_path: Option<PathBuf>,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
}

/// RPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Listen address for RPC
    pub listen_addr: SocketAddr,
    /// Enable authentication
    pub enable_auth: bool,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                id: None,
                name: "multivm-node".to_string(),
                data_dir: PathBuf::from("/var/lib/multivm"),
                log_level: "info".to_string(),
            },
            cluster: ClusterConfig {
                listen_addr: ([127, 0, 0, 1], 7000).into(),
                cluster_members: Vec::new(),
                enable_leader_election: true,
                heartbeat_interval_ms: 100,
                election_timeout_ms: 1000,
            },
            runtime: RuntimeConfig {
                vm_command: "qemu-system-x86_64".to_string(),
                max_vms_per_node: 100,
                vm_config_template: serde_json::json!({
                    "machine": "pc-q35-6.2",
                    "accel": "kvm",
                }),
            },
            storage: StorageConfig {
                backend: StorageBackend::RocksDb,
                rocksdb: RocksDbOptions {
                    cache_size_mb: 256,
                    compression: true,
                },
            },
            network: NetworkConfig {
                enable_tls: true,
                tls_cert_path: Some(PathBuf::from("/etc/multivm/certs/server.crt")),
                tls_key_path: Some(PathBuf::from("/etc/multivm/certs/server.key")),
                connection_timeout_secs: 10,
                request_timeout_secs: 30,
            },
            rpc: RpcConfig {
                listen_addr: ([127, 0, 0, 1], 50051).into(),
                enable_auth: false,
                max_concurrent_requests: 1000,
            },
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate TLS configuration
        if self.network.enable_tls {
            if self.network.tls_cert_path.is_none() || self.network.tls_key_path.is_none() {
                return Err("TLS is enabled but certificate paths are not provided".into());
            }
            
            // Check if certificate files exist
            if let Some(cert_path) = &self.network.tls_cert_path {
                if !cert_path.exists() {
                    return Err(format!("TLS certificate file not found: {}", cert_path.display()).into());
                }
            }
            
            if let Some(key_path) = &self.network.tls_key_path {
                if !key_path.exists() {
                    return Err(format!("TLS key file not found: {}", key_path.display()).into());
                }
            }
        }
        
        // Validate data directory
        if !self.node.data_dir.exists() {
            std::fs::create_dir_all(&self.node.data_dir)?;
        }
        
        Ok(())
    }
    
    /// Convert to orchestrator config
    pub fn to_orchestrator_config(&self) -> crate::orchestrator::OrchestratorConfig {
        let node_id = self.node.id.as_ref()
            .map(|id| NodeId::try_from_slice(id.as_bytes()).ok())
            .flatten()
            .unwrap_or_else(NodeId::new);

        let cluster_members = self.cluster.cluster_members.iter()
            .filter_map(|m| {
                NodeId::try_from_slice(m.id.as_bytes())
                    .ok()
                    .map(|id| ClusterMember::new(id, m.address))
            })
            .collect();

        crate::orchestrator::OrchestratorConfig {
            node_id,
            cluster_addr: self.cluster.listen_addr,
            rpc_addr: self.rpc.listen_addr,
            data_dir: self.node.data_dir.clone(),
            cluster_members,
            runtime_config: VmRuntimeConfig {
                runtime_dir: self.node.data_dir.join("runtime"),
                vm_command: self.runtime.vm_command.clone(),
                vm_config_template: self.runtime.vm_config_template.clone(),
                max_vms: self.runtime.max_vms_per_node,
                ..Default::default()
            },
            enable_leader_election: self.cluster.enable_leader_election,
        }
    }
}

/// Load configuration with defaults and environment overrides
pub fn load_config(config_path: Option<&str>) -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = if let Some(path) = config_path {
        Config::from_file(path)?
    } else {
        Config::default()
    };

    // Apply environment variable overrides
    if let Ok(node_id) = std::env::var("MULTIVM_NODE_ID") {
        config.node.id = Some(node_id);
    }
    
    if let Ok(cluster_addr) = std::env::var("MULTIVM_CLUSTER_ADDR") {
        config.cluster.listen_addr = cluster_addr.parse()?;
    }
    
    if let Ok(rpc_addr) = std::env::var("MULTIVM_RPC_ADDR") {
        config.rpc.listen_addr = rpc_addr.parse()?;
    }

    Ok(config)
}