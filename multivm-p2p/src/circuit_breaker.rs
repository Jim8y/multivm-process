//! Circuit Breaker Pattern Implementation for P2P Networking
//!
//! Provides fault tolerance by preventing calls to failing services and allowing
//! them time to recover. Implements the circuit breaker pattern with multiple states
//! and adaptive thresholds.

use crate::error::{P2PError, P2PResult};
use libp2p::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed, allowing all requests
    Closed,
    /// Circuit is open, blocking all requests
    Open,
    /// Circuit is half-open, allowing limited test requests
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open the circuit
    pub failure_threshold: u32,
    /// Success threshold to close the circuit from half-open
    pub success_threshold: u32,
    /// Timeout before transitioning from open to half-open
    pub timeout: Duration,
    /// Window size for failure rate calculation
    pub window_size: Duration,
    /// Maximum number of requests allowed in half-open state
    pub half_open_max_calls: u32,
    /// Minimum request threshold before considering failure rate
    pub min_request_threshold: u32,
    /// Failure rate threshold (0.0 to 1.0)
    pub failure_rate_threshold: f64,
    /// Reset timeout multiplier for exponential backoff
    pub timeout_multiplier: f64,
    /// Maximum timeout duration
    pub max_timeout: Duration,
    /// Enable adaptive thresholds based on peer performance
    pub enable_adaptive_thresholds: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            window_size: Duration::from_secs(300), // 5 minutes
            half_open_max_calls: 3,
            min_request_threshold: 10,
            failure_rate_threshold: 0.5, // 50% failure rate
            timeout_multiplier: 2.0,
            max_timeout: Duration::from_secs(600), // 10 minutes
            enable_adaptive_thresholds: true,
        }
    }
}

/// Request outcome for tracking
#[derive(Debug, Clone)]
pub enum RequestOutcome {
    Success,
    Failure(String),
    Timeout,
}

impl RequestOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, RequestOutcome::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, RequestOutcome::Failure(_) | RequestOutcome::Timeout)
    }
}

/// Request record for window-based tracking
#[derive(Debug, Clone)]
struct RequestRecord {
    timestamp: Instant,
    outcome: RequestOutcome,
    response_time: Duration,
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failure_rate: f64,
    pub last_failure_time: Option<Instant>,
    pub state_changed_at: Instant,
    pub open_count: u32,
    pub half_open_count: u32,
    pub avg_response_time: Duration,
}

/// Circuit breaker for a single peer
#[derive(Debug)]
struct PeerCircuitBreaker {
    peer_id: PeerId,
    config: CircuitBreakerConfig,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    half_open_calls: u32,
    last_failure_time: Option<Instant>,
    state_changed_at: Instant,
    timeout: Duration, // Current timeout (may be exponentially increased)
    request_history: Vec<RequestRecord>,
    total_requests: u64,
    successful_requests: u64,
    open_count: u32,
    half_open_count: u32,
}

impl PeerCircuitBreaker {
    fn new(peer_id: PeerId, config: CircuitBreakerConfig) -> Self {
        Self {
            peer_id,
            timeout: config.timeout,
            config,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            half_open_calls: 0,
            last_failure_time: None,
            state_changed_at: Instant::now(),
            request_history: Vec::new(),
            total_requests: 0,
            successful_requests: 0,
            open_count: 0,
            half_open_count: 0,
        }
    }

    /// Check if a request should be allowed
    fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed to transition to half-open
                if self.state_changed_at.elapsed() >= self.timeout {
                    self.transition_to_half_open();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                if self.half_open_calls < self.config.half_open_max_calls {
                    self.half_open_calls += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record the outcome of a request
    fn record_outcome(&mut self, outcome: RequestOutcome, response_time: Duration) {
        let now = Instant::now();
        self.total_requests += 1;

        // Add to request history
        self.request_history.push(RequestRecord {
            timestamp: now,
            outcome: outcome.clone(),
            response_time,
        });

        // Clean old records outside the window
        self.clean_old_records();

        // Update counters based on outcome
        match outcome {
            RequestOutcome::Success => {
                self.successful_requests += 1;
                self.success_count += 1;

                // Reset failure count on success in closed state
                if self.state == CircuitState::Closed {
                    self.failure_count = 0;
                }

                // Check for transition from half-open to closed
                if self.state == CircuitState::HalfOpen
                    && self.success_count >= self.config.success_threshold
                {
                    self.transition_to_closed();
                }
            }
            RequestOutcome::Failure(_) | RequestOutcome::Timeout => {
                self.failure_count += 1;
                self.last_failure_time = Some(now);

                // Reset success count on failure
                self.success_count = 0;

                // Check for transition to open state
                if self.should_open_circuit() {
                    self.transition_to_open();
                }
            }
        }
    }

    /// Check if circuit should be opened
    fn should_open_circuit(&self) -> bool {
        match self.state {
            CircuitState::Open => false, // Already open
            CircuitState::Closed | CircuitState::HalfOpen => {
                // Check failure count threshold
                if self.failure_count >= self.config.failure_threshold {
                    return true;
                }

                // Check failure rate if we have enough requests
                if self.request_history.len() >= self.config.min_request_threshold as usize {
                    let failure_rate = self.calculate_failure_rate();
                    if failure_rate >= self.config.failure_rate_threshold {
                        return true;
                    }
                }

                false
            }
        }
    }

    /// Calculate current failure rate within the window
    fn calculate_failure_rate(&self) -> f64 {
        if self.request_history.is_empty() {
            return 0.0;
        }

        let failures = self
            .request_history
            .iter()
            .filter(|record| record.outcome.is_failure())
            .count();

        failures as f64 / self.request_history.len() as f64
    }

    /// Transition to open state
    fn transition_to_open(&mut self) {
        debug!("Circuit breaker opening for peer {}", self.peer_id);
        self.state = CircuitState::Open;
        self.state_changed_at = Instant::now();
        self.open_count += 1;
        self.half_open_calls = 0;

        // Apply exponential backoff to timeout
        if self.config.timeout_multiplier > 1.0 {
            self.timeout = Duration::from_secs_f64(
                (self.timeout.as_secs_f64() * self.config.timeout_multiplier)
                    .min(self.config.max_timeout.as_secs_f64()),
            );
        }

        info!(
            "Circuit breaker opened for peer {} (timeout: {:?})",
            self.peer_id, self.timeout
        );
    }

    /// Transition to half-open state
    fn transition_to_half_open(&mut self) {
        debug!(
            "Circuit breaker transitioning to half-open for peer {}",
            self.peer_id
        );
        self.state = CircuitState::HalfOpen;
        self.state_changed_at = Instant::now();
        self.half_open_count += 1;
        self.half_open_calls = 0;
        self.success_count = 0;
        self.failure_count = 0;

        info!("Circuit breaker half-open for peer {}", self.peer_id);
    }

    /// Transition to closed state
    fn transition_to_closed(&mut self) {
        debug!("Circuit breaker closing for peer {}", self.peer_id);
        self.state = CircuitState::Closed;
        self.state_changed_at = Instant::now();
        self.half_open_calls = 0;
        self.success_count = 0;
        self.failure_count = 0;

        // Reset timeout to original value
        self.timeout = self.config.timeout;

        info!("Circuit breaker closed for peer {}", self.peer_id);
    }

    /// Clean old records outside the window
    fn clean_old_records(&mut self) {
        let cutoff = Instant::now() - self.config.window_size;
        self.request_history
            .retain(|record| record.timestamp >= cutoff);
    }

    /// Get current statistics
    fn get_stats(&self) -> CircuitBreakerStats {
        let failure_rate = self.calculate_failure_rate();
        let avg_response_time = if !self.request_history.is_empty() {
            let total_time: Duration = self.request_history.iter().map(|r| r.response_time).sum();
            total_time / self.request_history.len() as u32
        } else {
            Duration::from_secs(0)
        };

        CircuitBreakerStats {
            state: self.state.clone(),
            failure_count: self.failure_count,
            success_count: self.success_count,
            total_requests: self.total_requests,
            successful_requests: self.successful_requests,
            failure_rate,
            last_failure_time: self.last_failure_time,
            state_changed_at: self.state_changed_at,
            open_count: self.open_count,
            half_open_count: self.half_open_count,
            avg_response_time,
        }
    }

    /// Update configuration with adaptive thresholds
    fn update_adaptive_config(&mut self, avg_peer_performance: f64) {
        if !self.config.enable_adaptive_thresholds {
            return;
        }

        // Adjust thresholds based on peer performance relative to average
        // Better performing peers get stricter thresholds, worse peers get more lenient ones
        let performance_ratio = if avg_peer_performance > 0.0 {
            (self.successful_requests as f64 / self.total_requests.max(1) as f64)
                / avg_peer_performance
        } else {
            1.0
        };

        if performance_ratio > 1.2 {
            // High-performing peer, stricter thresholds
            self.config.failure_threshold = (self.config.failure_threshold as f64 * 0.8) as u32;
            self.config.failure_rate_threshold *= 0.9;
        } else if performance_ratio < 0.8 {
            // Low-performing peer, more lenient thresholds
            self.config.failure_threshold = (self.config.failure_threshold as f64 * 1.2) as u32;
            self.config.failure_rate_threshold *= 1.1;
        }

        // Ensure thresholds stay within reasonable bounds
        self.config.failure_threshold = self.config.failure_threshold.clamp(3, 20);
        self.config.failure_rate_threshold = self.config.failure_rate_threshold.clamp(0.3, 0.8);
    }
}

/// Circuit breaker manager for multiple peers
pub struct CircuitBreakerManager {
    config: CircuitBreakerConfig,
    circuit_breakers: Arc<RwLock<HashMap<PeerId, PeerCircuitBreaker>>>,
}

impl CircuitBreakerManager {
    /// Create a new circuit breaker manager
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the circuit breaker manager
    pub async fn start(&self) -> P2PResult<()> {
        info!("Starting circuit breaker manager");

        // Start maintenance task
        let circuit_breakers = Arc::clone(&self.circuit_breakers);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                Self::maintenance_task(&circuit_breakers, &config).await;
            }
        });

        Ok(())
    }

    /// Check if a request to a peer should be allowed
    pub async fn can_execute(&self, peer_id: PeerId) -> bool {
        let mut breakers = self.circuit_breakers.write().await;
        let breaker = breakers
            .entry(peer_id)
            .or_insert_with(|| PeerCircuitBreaker::new(peer_id, self.config.clone()));

        breaker.can_execute()
    }

    /// Record the outcome of a request
    pub async fn record_outcome(
        &self,
        peer_id: PeerId,
        outcome: RequestOutcome,
        response_time: Duration,
    ) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&peer_id) {
            breaker.record_outcome(outcome, response_time);
        }
    }

    /// Record a successful request
    pub async fn record_success(&self, peer_id: PeerId, response_time: Duration) {
        self.record_outcome(peer_id, RequestOutcome::Success, response_time)
            .await;
    }

    /// Record a failed request
    pub async fn record_failure(&self, peer_id: PeerId, error: String, response_time: Duration) {
        self.record_outcome(peer_id, RequestOutcome::Failure(error), response_time)
            .await;
    }

    /// Record a timeout
    pub async fn record_timeout(&self, peer_id: PeerId, response_time: Duration) {
        self.record_outcome(peer_id, RequestOutcome::Timeout, response_time)
            .await;
    }

    /// Get circuit breaker statistics for a peer
    pub async fn get_peer_stats(&self, peer_id: PeerId) -> Option<CircuitBreakerStats> {
        let breakers = self.circuit_breakers.read().await;
        breakers.get(&peer_id).map(|breaker| breaker.get_stats())
    }

    /// Get all circuit breaker statistics
    pub async fn get_all_stats(&self) -> HashMap<PeerId, CircuitBreakerStats> {
        let breakers = self.circuit_breakers.read().await;
        breakers
            .iter()
            .map(|(peer_id, breaker)| (*peer_id, breaker.get_stats()))
            .collect()
    }

    /// Get peers that are currently available (circuit closed or half-open with capacity)
    pub async fn get_available_peers(&self, peers: &[PeerId]) -> Vec<PeerId> {
        let mut available = Vec::new();

        for &peer_id in peers {
            if self.can_execute(peer_id).await {
                available.push(peer_id);
            }
        }

        available
    }

    /// Force open a circuit breaker (for manual intervention)
    pub async fn force_open(&self, peer_id: PeerId) -> P2PResult<()> {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&peer_id) {
            breaker.transition_to_open();
            info!("Manually opened circuit breaker for peer {}", peer_id);
        }
        Ok(())
    }

    /// Force close a circuit breaker (for manual intervention)
    pub async fn force_close(&self, peer_id: PeerId) -> P2PResult<()> {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&peer_id) {
            breaker.transition_to_closed();
            info!("Manually closed circuit breaker for peer {}", peer_id);
        }
        Ok(())
    }

    /// Reset a circuit breaker (clear all history)
    pub async fn reset(&self, peer_id: PeerId) -> P2PResult<()> {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&peer_id) {
            *breaker = PeerCircuitBreaker::new(peer_id, self.config.clone());
            info!("Reset circuit breaker for peer {}", peer_id);
        }
        Ok(())
    }

    /// Get summary statistics across all circuit breakers
    pub async fn get_summary_stats(&self) -> CircuitBreakerSummary {
        let breakers = self.circuit_breakers.read().await;

        let mut total_peers = 0;
        let mut closed_count = 0;
        let mut open_count = 0;
        let mut half_open_count = 0;
        let mut total_requests = 0;
        let mut successful_requests = 0;
        let mut total_failures = 0;

        for breaker in breakers.values() {
            total_peers += 1;

            match breaker.state {
                CircuitState::Closed => closed_count += 1,
                CircuitState::Open => open_count += 1,
                CircuitState::HalfOpen => half_open_count += 1,
            }

            total_requests += breaker.total_requests;
            successful_requests += breaker.successful_requests;
            total_failures += breaker.failure_count as u64;
        }

        let success_rate = if total_requests > 0 {
            successful_requests as f64 / total_requests as f64
        } else {
            1.0
        };

        CircuitBreakerSummary {
            total_peers,
            closed_count,
            open_count,
            half_open_count,
            total_requests,
            successful_requests,
            success_rate,
            total_failures,
        }
    }

    /// Maintenance task for cleanup and adaptive adjustments
    async fn maintenance_task(
        circuit_breakers: &Arc<RwLock<HashMap<PeerId, PeerCircuitBreaker>>>,
        config: &CircuitBreakerConfig,
    ) {
        let mut breakers = circuit_breakers.write().await;

        // Calculate average peer performance for adaptive thresholds
        let avg_performance = if !breakers.is_empty() {
            let total_performance: f64 = breakers
                .values()
                .map(|b| b.successful_requests as f64 / b.total_requests.max(1) as f64)
                .sum();
            total_performance / breakers.len() as f64
        } else {
            1.0
        };

        // Update adaptive configurations and clean old records
        for breaker in breakers.values_mut() {
            breaker.clean_old_records();
            breaker.update_adaptive_config(avg_performance);
        }

        debug!(
            "Circuit breaker maintenance completed for {} peers",
            breakers.len()
        );
    }
}

/// Summary statistics for all circuit breakers
#[derive(Debug, Clone)]
pub struct CircuitBreakerSummary {
    pub total_peers: usize,
    pub closed_count: usize,
    pub open_count: usize,
    pub half_open_count: usize,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub success_rate: f64,
    pub total_failures: u64,
}
