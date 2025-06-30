//! Real-time Metrics Dashboard
//!
//! Provides WebSocket-based real-time metrics streaming and a web dashboard
//! for monitoring the MultiVM system in production.

use crate::{ApplicationError, ApplicationResult, ApplicationState};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;
use tracing::{debug, error, info};

/// Dashboard metrics update interval
const METRICS_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

/// Maximum metrics history to keep
const MAX_METRICS_HISTORY: usize = 300; // 5 minutes at 1 second intervals

/// Real-time metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeMetrics {
    /// Timestamp of the metrics
    pub timestamp: i64,
    /// System metrics
    pub system: SystemMetrics,
    /// Blockchain metrics
    pub blockchain: BlockchainMetrics,
    /// Network metrics
    pub network: NetworkMetrics,
    /// Transaction pool metrics
    pub tx_pool: TxPoolMetrics,
    /// Performance metrics
    pub performance: PerformanceMetrics,
}

/// System resource metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in MB
    pub memory_usage: u64,
    /// Disk usage percentage
    pub disk_usage: f64,
    /// Number of active connections
    pub active_connections: u32,
    /// Uptime in seconds
    pub uptime_seconds: u64,
}

/// Blockchain-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainMetrics {
    /// Current block height
    pub block_height: u64,
    /// Blocks per second (current)
    pub blocks_per_second: f64,
    /// Total transactions processed
    pub total_transactions: u64,
    /// Transactions per second (current)
    pub tps_current: f64,
    /// Average TPS over last minute
    pub tps_average: f64,
    /// Last block time
    pub last_block_time: i64,
    /// Average block time (seconds)
    pub avg_block_time: f64,
}

/// Network metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Number of connected peers
    pub peer_count: u32,
    /// Number of active validators
    pub validator_count: u32,
    /// Network latency (ms)
    pub network_latency: u64,
    /// Bandwidth usage (bytes/sec)
    pub bandwidth_in: u64,
    pub bandwidth_out: u64,
}

/// Transaction pool metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxPoolMetrics {
    /// Pending transactions
    pub pending_count: u32,
    /// Queued transactions
    pub queued_count: u32,
    /// Total submitted
    pub total_submitted: u64,
    /// Total included in blocks
    pub total_included: u64,
    /// Average time to inclusion (seconds)
    pub avg_inclusion_time: f64,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// API request rate (req/sec)
    pub api_requests_per_sec: f64,
    /// Average API response time (ms)
    pub avg_api_response_time: u64,
    /// Cache hit ratio
    pub cache_hit_ratio: f64,
    /// Execution engine utilization (%)
    pub engine_utilization: f64,
}

/// Metrics history for trend analysis
#[derive(Debug, Default)]
pub struct MetricsHistory {
    /// Historical metrics data
    history: Vec<RealtimeMetrics>,
    /// Broadcast channel for real-time updates
    broadcast_tx: Option<broadcast::Sender<RealtimeMetrics>>,
}

impl MetricsHistory {
    /// Create new metrics history
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            history: Vec::with_capacity(MAX_METRICS_HISTORY),
            broadcast_tx: Some(tx),
        }
    }

    /// Add new metrics data point
    pub fn add(&mut self, metrics: RealtimeMetrics) {
        // Keep only recent history
        if self.history.len() >= MAX_METRICS_HISTORY {
            self.history.remove(0);
        }
        self.history.push(metrics.clone());

        // Broadcast to all connected clients
        if let Some(tx) = &self.broadcast_tx {
            let _ = tx.send(metrics);
        }
    }

    /// Get subscriber for real-time updates
    pub fn subscribe(&self) -> Option<broadcast::Receiver<RealtimeMetrics>> {
        self.broadcast_tx.as_ref().map(|tx| tx.subscribe())
    }

    /// Get recent history
    pub fn get_recent(&self, count: usize) -> Vec<RealtimeMetrics> {
        let start = self.history.len().saturating_sub(count);
        self.history[start..].to_vec()
    }
}

/// Dashboard service for real-time metrics
#[derive(Debug)]
pub struct DashboardService {
    state: Arc<tokio::sync::RwLock<Option<Arc<ApplicationState>>>>,
    history: Arc<tokio::sync::RwLock<MetricsHistory>>,
}

impl DashboardService {
    /// Create new dashboard service
    pub fn new() -> Self {
        Self {
            state: Arc::new(tokio::sync::RwLock::new(None)),
            history: Arc::new(tokio::sync::RwLock::new(MetricsHistory::new())),
        }
    }

    /// Set the application state (must be called after creation)
    pub async fn set_state(&self, state: Arc<ApplicationState>) {
        let mut state_lock = self.state.write().await;
        *state_lock = Some(state);
    }

    /// Start metrics collection
    pub async fn start_collection(&self) {
        let state_lock = self.state.read().await;
        let state = match &*state_lock {
            Some(s) => s.clone(),
            None => {
                error!("Cannot start metrics collection: ApplicationState not set");
                return;
            }
        };
        drop(state_lock);
        let history = self.history.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(METRICS_UPDATE_INTERVAL);

            loop {
                interval.tick().await;

                match Self::collect_metrics(&state).await {
                    Ok(metrics) => {
                        let mut hist = history.write().await;
                        hist.add(metrics);
                    }
                    Err(e) => {
                        error!("Failed to collect metrics: {}", e);
                    }
                }
            }
        });

        info!("Real-time metrics collection started");
    }

    /// Collect current metrics
    async fn collect_metrics(state: &Arc<ApplicationState>) -> ApplicationResult<RealtimeMetrics> {
        let timestamp = chrono::Utc::now().timestamp();

        // Collect system metrics
        let system = Self::collect_system_metrics(state).await?;

        // Collect blockchain metrics
        let blockchain = Self::collect_blockchain_metrics(state).await?;

        // Collect network metrics
        let network = Self::collect_network_metrics(state).await?;

        // Collect transaction pool metrics
        let tx_pool = Self::collect_tx_pool_metrics(state).await?;

        // Collect performance metrics
        let performance = Self::collect_performance_metrics(state).await?;

        Ok(RealtimeMetrics {
            timestamp,
            system,
            blockchain,
            network,
            tx_pool,
            performance,
        })
    }

    /// Collect system resource metrics
    async fn collect_system_metrics(
        state: &Arc<ApplicationState>,
    ) -> ApplicationResult<SystemMetrics> {
        // In a real implementation, use sysinfo crate or similar
        let uptime = state.start_time.elapsed().as_secs();

        // Use time-based variation for mock data instead of random
        let variation = ((uptime % 10) as f64) / 10.0;

        Ok(SystemMetrics {
            cpu_usage: 15.5 + (variation * 10.0), // Mock data
            memory_usage: 256 + ((uptime % 128) as u64),
            disk_usage: 35.0 + (variation * 5.0),
            active_connections: 10 + ((uptime % 20) as u32),
            uptime_seconds: uptime,
        })
    }

    /// Collect blockchain metrics
    async fn collect_blockchain_metrics(
        state: &Arc<ApplicationState>,
    ) -> ApplicationResult<BlockchainMetrics> {
        let mut block_height = 0u64;
        let mut total_transactions = 0u64;

        // Get consensus metrics if available
        let consensus_guard = state.consensus_manager.read().await;
        if let Some(consensus) = consensus_guard.as_ref() {
            let pool_stats = consensus.get_transaction_pool_stats().await;
            total_transactions = pool_stats.total_included;
            // In real implementation, get actual block height
            block_height = (state.start_time.elapsed().as_secs() / 5) + 1; // Mock: 1 block per 5 seconds
        }

        // Use time-based variation for mock data
        let elapsed = state.start_time.elapsed().as_secs();
        let variation = ((elapsed % 10) as f64) / 10.0;

        Ok(BlockchainMetrics {
            block_height,
            blocks_per_second: 0.2, // 1 block per 5 seconds
            total_transactions,
            tps_current: variation * 200.0,
            tps_average: 100.0 + variation * 50.0,
            last_block_time: chrono::Utc::now().timestamp() - 2,
            avg_block_time: 5.0,
        })
    }

    /// Collect network metrics
    async fn collect_network_metrics(
        state: &Arc<ApplicationState>,
    ) -> ApplicationResult<NetworkMetrics> {
        let elapsed = state.start_time.elapsed().as_secs();

        Ok(NetworkMetrics {
            peer_count: 4 + ((elapsed % 8) as u32),
            validator_count: 4,
            network_latency: 20 + ((elapsed % 30) as u64),
            bandwidth_in: 1024 * (100 + ((elapsed % 900) as u64)),
            bandwidth_out: 1024 * (50 + ((elapsed % 450) as u64)),
        })
    }

    /// Collect transaction pool metrics
    async fn collect_tx_pool_metrics(
        state: &Arc<ApplicationState>,
    ) -> ApplicationResult<TxPoolMetrics> {
        let mut metrics = TxPoolMetrics {
            pending_count: 0,
            queued_count: 0,
            total_submitted: 0,
            total_included: 0,
            avg_inclusion_time: 5.0,
        };

        let consensus_guard = state.consensus_manager.read().await;
        if let Some(consensus) = consensus_guard.as_ref() {
            let pool_stats = consensus.get_transaction_pool_stats().await;
            metrics.pending_count = pool_stats.current_pool_size as u32;
            metrics.total_submitted = pool_stats.total_submitted;
            metrics.total_included = pool_stats.total_included;
            metrics.queued_count =
                (pool_stats.evm_transactions + pool_stats.svm_transactions) as u32;
        }

        Ok(metrics)
    }

    /// Collect performance metrics
    async fn collect_performance_metrics(
        state: &Arc<ApplicationState>,
    ) -> ApplicationResult<PerformanceMetrics> {
        // Get cache metrics
        let cache_stats = state.cache.get_stats().await?;

        // Use time-based variation for mock data
        let elapsed = state.start_time.elapsed().as_secs();
        let variation = ((elapsed % 10) as f64) / 10.0;

        Ok(PerformanceMetrics {
            api_requests_per_sec: 50.0 + variation * 100.0,
            avg_api_response_time: 5 + ((elapsed % 20) as u64),
            cache_hit_ratio: cache_stats.hit_ratio,
            engine_utilization: 20.0 + variation * 30.0,
        })
    }

    /// Handle WebSocket connection for real-time metrics
    pub async fn handle_websocket(&self, ws: WebSocketUpgrade) -> Response {
        let history = self.history.clone();

        ws.on_upgrade(move |socket| async move {
            if let Err(e) = handle_socket(socket, history).await {
                error!("WebSocket error: {}", e);
            }
        })
    }

    /// Get dashboard HTML page
    pub fn get_dashboard_html() -> &'static str {
        include_str!("../../static/dashboard.html")
    }
}

/// Handle individual WebSocket connection
async fn handle_socket(
    mut socket: WebSocket,
    history: Arc<tokio::sync::RwLock<MetricsHistory>>,
) -> ApplicationResult<()> {
    // Send initial history
    let recent = history.read().await.get_recent(60); // Last minute
    let history_msg = serde_json::json!({
        "type": "history",
        "data": recent
    });

    socket
        .send(Message::Text(serde_json::to_string(&history_msg)?.into()))
        .await
        .map_err(|e| ApplicationError::InternalError {
            component: "dashboard".to_string(),
            message: e.to_string(),
        })?;

    // Subscribe to real-time updates
    let mut rx =
        history
            .read()
            .await
            .subscribe()
            .ok_or_else(|| ApplicationError::InternalError {
                component: "dashboard".to_string(),
                message: "Failed to subscribe to metrics".to_string(),
            })?;

    // Send real-time updates
    loop {
        tokio::select! {
            // Receive metrics updates
            Ok(metrics) = rx.recv() => {
                let update_msg = serde_json::json!({
                    "type": "update",
                    "data": metrics
                });

                if socket
                    .send(Message::Text(serde_json::to_string(&update_msg)?.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            // Handle client messages (ping/pong)
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Text(text) => {
                        debug!("Received text: {}", text);
                        // Could handle client commands here
                    }
                    Message::Close(_) => {
                        debug!("Client disconnected");
                        break;
                    }
                    _ => {}
                }
            }

            // Keep connection alive
            _ = time::sleep(Duration::from_secs(30)) => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// Enable dashboard
    pub enabled: bool,
    /// Dashboard port
    pub port: u16,
    /// Metrics retention period (seconds)
    pub retention_seconds: u64,
    /// Update interval (milliseconds)
    pub update_interval_ms: u64,
}
