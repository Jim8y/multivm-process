//! Process manager specific commands

use multivm_common::{types::ProcessId, MultivmResult};
use serde::{Deserialize, Serialize};

/// Commands specific to process management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessManagerCommand {
    /// Start a process
    StartProcess { 
        process_id: ProcessId 
    },
    
    /// Stop a process
    StopProcess { 
        process_id: ProcessId 
    },
    
    /// Get process status
    GetProcessStatus { 
        process_id: ProcessId 
    },
    
    /// Health check
    HealthCheck { 
        process_id: ProcessId 
    },
    
    /// Restart a process
    RestartProcess { 
        process_id: ProcessId 
    },
    
    /// Get process metrics
    GetProcessMetrics { 
        process_id: ProcessId 
    },
    
    /// Route a block
    RouteBlock { 
        block_data: Vec<u8> 
    },
    
    /// Process a transaction
    ProcessTransaction { 
        process_id: ProcessId, 
        transaction_data: Vec<u8> 
    },
    
    /// List all processes
    ListProcesses,
    
    /// Get system health
    GetSystemHealth,
}

/// Response for process manager commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessManagerResponse {
    /// Generic success
    Success,
    
    /// Process status
    ProcessStatus {
        running: bool,
        uptime: Option<u64>,
        memory_usage: Option<u64>,
    },
    
    /// Process list
    ProcessList {
        processes: Vec<(ProcessId, bool)>, // (id, is_running)
    },
    
    /// System health
    SystemHealth {
        healthy: bool,
        running_processes: usize,
        total_processes: usize,
    },
    
    /// Error response
    Error {
        message: String,
    },
}