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

    // Additional metrics for manager aggregation
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub peak_memory_usage_mb: u64,
    pub cpu_usage_percent: f64,
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
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_response_time_ms: 0.0,
            peak_memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
        }
    }
}
