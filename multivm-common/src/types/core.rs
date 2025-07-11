use serde::{Deserialize, Serialize};

/// Unique identifier for messages between processes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(u64);

impl MessageId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Process identifier for different blockchain engines
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcessId {
    Main,
    Solana,
    Ethereum,
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessId::Main => write!(f, "main"),
            ProcessId::Solana => write!(f, "solana"),
            ProcessId::Ethereum => write!(f, "ethereum"),
        }
    }
}

/// System memory usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percentage: f64,
}

/// System CPU usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUsage {
    pub percentage: f64,
    pub cores: u32,
    pub load_average: [f64; 3], // 1min, 5min, 15min
}

/// System disk usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percentage: f64,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub memory: MemoryUsage,
    pub cpu: CpuUsage,
    pub disk: DiskUsage,
    pub uptime: std::time::Duration,
}
/// Blockchain type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockchainType {
    Solana,
    Ethereum,
}

impl std::fmt::Display for BlockchainType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockchainType::Solana => write!(f, "solana"),
            BlockchainType::Ethereum => write!(f, "ethereum"),
        }
    }
}
