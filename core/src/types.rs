//! Core types used throughout the MultiVM system

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a VM instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmId(pub Uuid);

impl VmId {
    /// Create a new random VM ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    /// Create from string
    pub fn try_from_str(s: &str) -> Result<Self, crate::Error> {
        let uuid = Uuid::parse_str(s)
            .map_err(|_| crate::Error::InvalidInput(format!("Invalid VM ID: {}", s)))?;
        Ok(Self(uuid))
    }
}

impl Default for VmId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for VmId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node identifier in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    /// Create a new random node ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    /// Create from bytes
    /// 
    /// # Arguments
    /// * `bytes` - Must be exactly 16 bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
    
    /// Try to create from a slice
    /// 
    /// # Errors
    /// Returns an error if the slice is not exactly 16 bytes
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, crate::Error> {
        if bytes.len() != 16 {
            return Err(crate::Error::InvalidInput(
                format!("NodeId requires exactly 16 bytes, got {}", bytes.len())
            ));
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(bytes);
        Ok(Self::from_bytes(arr))
    }
    
    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Version number for data structures
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Version(pub u64);

impl Version {
    /// Initial version
    pub const INITIAL: Self = Self(0);
    
    /// Increment version
    pub fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Cluster member information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMember {
    /// Node ID
    pub id: NodeId,
    /// Network address
    pub address: std::net::SocketAddr,
    /// Member metadata
    pub metadata: std::collections::HashMap<String, String>,
    /// Health status
    pub healthy: bool,
    /// Last heartbeat timestamp
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

impl ClusterMember {
    /// Create a new cluster member
    pub fn new(id: NodeId, address: std::net::SocketAddr) -> Self {
        Self {
            id,
            address,
            metadata: std::collections::HashMap::new(),
            healthy: true,
            last_heartbeat: chrono::Utc::now(),
        }
    }
    
    /// Check if member is considered alive based on heartbeat
    pub fn is_alive(&self, timeout: chrono::Duration) -> bool {
        self.healthy && chrono::Utc::now() - self.last_heartbeat < timeout
    }
}

/// VM state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmState {
    /// VM is being created
    Creating,
    /// VM is running
    Running,
    /// VM is paused
    Paused,
    /// VM is stopped
    Stopped,
    /// VM is being migrated
    Migrating,
    /// VM has failed
    Failed,
    /// VM is being deleted
    Deleting,
}

impl VmState {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, VmState::Failed | VmState::Deleting)
    }
    
    /// Check if this is a running state
    pub fn is_running(&self) -> bool {
        matches!(self, VmState::Running)
    }
    
    /// Validate state transition
    pub fn can_transition_to(&self, target: &VmState) -> bool {
        use VmState::*;
        match (self, target) {
            // From Creating
            (Creating, Running) => true,
            (Creating, Failed) => true,
            
            // From Running
            (Running, Paused) => true,
            (Running, Stopped) => true,
            (Running, Migrating) => true,
            (Running, Failed) => true,
            (Running, Deleting) => true,
            
            // From Paused
            (Paused, Running) => true,
            (Paused, Stopped) => true,
            (Paused, Failed) => true,
            (Paused, Deleting) => true,
            
            // From Stopped
            (Stopped, Running) => true,
            (Stopped, Deleting) => true,
            
            // From Migrating
            (Migrating, Running) => true,
            (Migrating, Failed) => true,
            
            // From Failed
            (Failed, Deleting) => true,
            
            // Terminal states cannot transition
            (Deleting, _) => false,
            
            // All other transitions are invalid
            _ => false,
        }
    }
}

/// VM metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmMetadata {
    /// VM ID
    pub id: VmId,
    /// VM name
    pub name: String,
    /// VM state
    pub state: VmState,
    /// Node hosting the VM
    pub node_id: Option<NodeId>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Resource requirements
    pub resources: ResourceRequirements,
    /// User-defined labels
    pub labels: std::collections::HashMap<String, String>,
}

/// Resource requirements for a VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// CPU cores (must be > 0)
    pub cpu_cores: f32,
    /// Memory in MB (must be > 0)
    pub memory_mb: u64,
    /// Disk in GB (must be > 0)
    pub disk_gb: u64,
    /// Network bandwidth in Mbps
    pub network_mbps: Option<u64>,
}

impl ResourceRequirements {
    /// Create new resource requirements with validation
    /// 
    /// # Errors
    /// Returns an error if any required resource is 0 or negative
    pub fn new(cpu_cores: f32, memory_mb: u64, disk_gb: u64, network_mbps: Option<u64>) -> Result<Self, crate::Error> {
        if cpu_cores <= 0.0 {
            return Err(crate::Error::InvalidInput("CPU cores must be > 0".to_string()));
        }
        if memory_mb == 0 {
            return Err(crate::Error::InvalidInput("Memory must be > 0 MB".to_string()));
        }
        if disk_gb == 0 {
            return Err(crate::Error::InvalidInput("Disk space must be > 0 GB".to_string()));
        }
        
        Ok(Self {
            cpu_cores,
            memory_mb,
            disk_gb,
            network_mbps,
        })
    }
    
    /// Validate resource requirements
    pub fn validate(&self) -> Result<(), crate::Error> {
        if self.cpu_cores <= 0.0 || self.memory_mb == 0 || self.disk_gb == 0 {
            return Err(crate::Error::InvalidInput(
                "All resource requirements must be > 0".to_string()
            ));
        }
        Ok(())
    }
}

/// Resource usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU usage percentage
    pub cpu_percent: f32,
    /// Memory usage in MB
    pub memory_mb: u64,
    /// Disk I/O in MB/s
    pub disk_io_mbps: f32,
    /// Network I/O in Mbps
    pub network_mbps: f32,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_mb: 0,
            disk_io_mbps: 0.0,
            network_mbps: 0.0,
        }
    }
}

/// Common configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonConfig {
    /// Node ID
    pub node_id: NodeId,
    /// Data directory
    pub data_dir: std::path::PathBuf,
    /// Log level
    pub log_level: String,
    /// Metrics port
    pub metrics_port: u16,
    /// Admin API port
    pub admin_port: u16,
}

impl Default for CommonConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::new(),
            data_dir: std::path::PathBuf::from("/var/lib/multivm"),
            log_level: "info".to_string(),
            metrics_port: 9090,
            admin_port: 8080,
        }
    }
}