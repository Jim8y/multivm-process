use crate::{BlockchainType, ProcessId};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    pub fn is_operational(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }
}

/// Detailed health information for a blockchain engine process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    pub process_id: ProcessId,
    pub status: HealthStatus,
    pub last_block_processed: Option<u64>,
    pub blocks_processed_total: u64,
    pub uptime: Duration,
    pub memory_usage: u64,
    pub cpu_usage_percent: f64,
    pub rpc_active: bool,
    pub errors_count: u64,
    pub last_error: Option<String>,
    pub timestamp: SystemTime,
}

/// Engine state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    pub process_id: ProcessId,
    pub blockchain_type: BlockchainType,
    pub current_block: Option<u64>,
    pub state_root: Vec<u8>,
    pub is_syncing: bool,
    pub peer_count: u32, // Should be 0 for our use case
    pub rpc_endpoints: Vec<String>,
    pub data_directory: String,
    pub chain_id: u64,
}
