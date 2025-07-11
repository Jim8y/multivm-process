//! Generated protobuf code

pub mod multivm {
    pub use prost_types;
    tonic::include_proto!("multivm");
}

use multivm_core;

impl From<multivm_core::ResourceRequirements> for multivm::ResourceRequirements {
    fn from(req: multivm_core::ResourceRequirements) -> Self {
        Self {
            cpu_cores: req.cpu_cores,
            memory_mb: req.memory_mb,
            disk_gb: req.disk_gb,
            network_mbps: req.network_mbps,
        }
    }
}

impl TryFrom<multivm::ResourceRequirements> for multivm_core::ResourceRequirements {
    type Error = multivm_core::Error;

    fn try_from(req: multivm::ResourceRequirements) -> Result<Self, Self::Error> {
        multivm_core::ResourceRequirements::new(
            req.cpu_cores,
            req.memory_mb,
            req.disk_gb,
            req.network_mbps,
        )
    }
}

impl From<multivm_core::ResourceUsage> for multivm::ResourceUsage {
    fn from(usage: multivm_core::ResourceUsage) -> Self {
        Self {
            cpu_percent: usage.cpu_percent,
            memory_mb: usage.memory_mb,
            disk_io_mbps: usage.disk_io_mbps,
            network_mbps: usage.network_mbps,
        }
    }
}

impl From<multivm::ResourceUsage> for multivm_core::ResourceUsage {
    fn from(usage: multivm::ResourceUsage) -> Self {
        Self {
            cpu_percent: usage.cpu_percent,
            memory_mb: usage.memory_mb,
            disk_io_mbps: usage.disk_io_mbps,
            network_mbps: usage.network_mbps,
        }
    }
}