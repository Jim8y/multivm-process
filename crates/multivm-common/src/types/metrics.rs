use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Detailed metrics for block processing performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetrics {
    pub cpu_time: Duration,
    pub memory_usage_bytes: u64,
    pub disk_reads: u64,
    pub disk_writes: u64,
    pub network_bytes: u64,
    pub compute_units_used: u64, // Solana CUs or Ethereum gas
    pub transaction_count: u64,
    pub account_updates: u64, // Solana accounts or Ethereum state changes
}

impl Default for ProcessingMetrics {
    fn default() -> Self {
        Self {
            cpu_time: Duration::ZERO,
            memory_usage_bytes: 0,
            disk_reads: 0,
            disk_writes: 0,
            network_bytes: 0,
            compute_units_used: 0,
            transaction_count: 0,
            account_updates: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_metrics_default() {
        let metrics = ProcessingMetrics::default();
        assert_eq!(metrics.cpu_time, Duration::ZERO);
        assert_eq!(metrics.memory_usage_bytes, 0);
        assert_eq!(metrics.transaction_count, 0);
    }
}
