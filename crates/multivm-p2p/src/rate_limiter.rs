//! Rate limiting implementation for P2P networking
//!
//! Provides protection against DoS attacks by limiting request rates per peer.

use crate::error::{P2PError, P2PResult};
use crate::messages::Priority;
use governor::clock::{QuantaClock, QuantaInstant};
use governor::state::{InMemoryState, NotKeyed};
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter as Governor};
use libp2p::PeerId;
use nonzero_ext::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limiter for P2P operations
pub struct RateLimiter {
    /// Per-peer rate limiter
    peer_limiter: DefaultKeyedRateLimiter<PeerId>,
    /// Global rate limiter
    global_limiter: Arc<Governor<NotKeyed, InMemoryState, QuantaClock>>,
    /// Configuration
    config: RateLimiterConfig,
    /// Simple rate limiter for tests (requests per window)
    simple_limits: HashMap<String, (u32, u32, Instant)>, // (count, limit, window_start)
    /// Window duration for simple limiter
    window_duration: Duration,
    /// Priority multiplier
    priority_multiplier: f64,
}

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Maximum requests per second per peer
    pub per_peer_rate: u32,
    /// Burst size per peer
    pub per_peer_burst: u32,
    /// Global maximum requests per second
    pub global_rate: u32,
    /// Global burst size
    pub global_burst: u32,
    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            per_peer_rate: 100,
            per_peer_burst: 10,
            global_rate: 1000,
            global_burst: 100,
            enabled: true,
        }
    }
}

impl RateLimiter {
    /// Create a new rate limiter from config
    pub fn from_config(config: RateLimiterConfig) -> Self {
        // Create per-peer rate limiter
        let peer_quota = Quota::per_second(
            std::num::NonZeroU32::new(config.per_peer_rate).unwrap_or(nonzero!(1u32)),
        )
        .allow_burst(std::num::NonZeroU32::new(config.per_peer_burst).unwrap_or(nonzero!(1u32)));
        let peer_limiter = DefaultKeyedRateLimiter::keyed(peer_quota);

        // Create global rate limiter
        let global_quota = Quota::per_second(
            std::num::NonZeroU32::new(config.global_rate).unwrap_or(nonzero!(1u32)),
        )
        .allow_burst(std::num::NonZeroU32::new(config.global_burst).unwrap_or(nonzero!(1u32)));
        let global_limiter = Arc::new(Governor::new(
            global_quota,
            InMemoryState::default(),
            &QuantaClock::default(),
        ));

        Self {
            peer_limiter,
            global_limiter,
            config,
            simple_limits: HashMap::new(),
            window_duration: Duration::from_secs(1),
            priority_multiplier: 2.0,
        }
    }

    /// Create a simple rate limiter for testing
    pub fn new(limit: u32, window: Duration) -> Self {
        let config = RateLimiterConfig {
            per_peer_rate: limit,
            per_peer_burst: limit,
            global_rate: limit * 10,
            global_burst: limit * 10,
            enabled: true,
        };

        let mut limiter = Self::new_with_config(config);
        limiter.window_duration = window;
        limiter
    }

    /// Create with specific config
    pub fn new_with_config(config: RateLimiterConfig) -> Self {
        // Create per-peer rate limiter
        let peer_quota = Quota::per_second(
            std::num::NonZeroU32::new(config.per_peer_rate).unwrap_or(nonzero!(1u32)),
        )
        .allow_burst(std::num::NonZeroU32::new(config.per_peer_burst).unwrap_or(nonzero!(1u32)));
        let peer_limiter = DefaultKeyedRateLimiter::keyed(peer_quota);

        // Create global rate limiter
        let global_quota = Quota::per_second(
            std::num::NonZeroU32::new(config.global_rate).unwrap_or(nonzero!(1u32)),
        )
        .allow_burst(std::num::NonZeroU32::new(config.global_burst).unwrap_or(nonzero!(1u32)));
        let global_limiter = Arc::new(Governor::new(
            global_quota,
            InMemoryState::default(),
            &QuantaClock::default(),
        ));

        Self {
            peer_limiter,
            global_limiter,
            config,
            simple_limits: HashMap::new(),
            window_duration: Duration::from_secs(1),
            priority_multiplier: 2.0,
        }
    }

    /// Check if a request from a peer should be allowed
    pub fn check_peer_limit(&self, peer_id: &PeerId) -> P2PResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check peer-specific limit
        match self.peer_limiter.check_key(peer_id) {
            Ok(_) => {
                // Also check global limit
                match self.global_limiter.check() {
                    Ok(_) => Ok(()),
                    Err(_) => Err(P2PError::RateLimitExceeded(
                        "Global rate limit exceeded".to_string(),
                    )),
                }
            }
            Err(_) => Err(P2PError::RateLimitExceeded(format!(
                "Rate limit exceeded for peer: {}",
                peer_id
            ))),
        }
    }

    /// Check if a request should be allowed (global limit only)
    pub fn check_global_limit(&self) -> P2PResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        match self.global_limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => Err(P2PError::RateLimitExceeded(
                "Global rate limit exceeded".to_string(),
            )),
        }
    }

    /// Get current statistics
    pub fn get_stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            enabled: self.config.enabled,
            per_peer_rate: self.config.per_peer_rate,
            global_rate: self.config.global_rate,
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: RateLimiterConfig) {
        // Recreate limiters with new config
        *self = Self::new_with_config(config);
    }

    /// Simple rate limit check for testing
    pub fn check_rate_limit(&mut self, peer_id: &str) -> bool {
        let now = Instant::now();
        let limit = self.config.per_peer_rate;

        let (count, _, window_start) = self
            .simple_limits
            .entry(peer_id.to_string())
            .or_insert((0, limit, now));

        // Reset window if expired
        if now.duration_since(*window_start) >= self.window_duration {
            *count = 0;
            *window_start = now;
        }

        if *count < limit {
            *count += 1;
            true
        } else {
            false
        }
    }

    /// Check rate limit with priority
    pub fn check_rate_limit_with_priority(&mut self, peer_id: &str, priority: Priority) -> bool {
        let now = Instant::now();
        let base_limit = self.config.per_peer_rate;
        let limit = match priority {
            Priority::High => (base_limit as f64 * self.priority_multiplier) as u32,
            Priority::Normal => base_limit,
            Priority::Low => base_limit / 2,
            Priority::Critical => (base_limit as f64 * self.priority_multiplier * 2.0) as u32,
        };

        let entry = self
            .simple_limits
            .entry(format!("{}:{:?}", peer_id, priority))
            .or_insert((0, limit, now));

        let (count, entry_limit, window_start) = entry;
        *entry_limit = limit; // Update limit based on priority

        // Reset window if expired
        if now.duration_since(*window_start) >= self.window_duration {
            *count = 0;
            *window_start = now;
        }

        if *count < limit {
            *count += 1;
            true
        } else {
            false
        }
    }

    /// Set priority multiplier
    pub fn set_priority_multiplier(&mut self, multiplier: f64) {
        self.priority_multiplier = multiplier;
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub enabled: bool,
    pub per_peer_rate: u32,
    pub global_rate: u32,
}

/// Message-specific rate limiter
pub struct MessageRateLimiter {
    /// Rate limiters per message type
    limiters:
        std::collections::HashMap<String, Arc<Governor<NotKeyed, InMemoryState, QuantaClock>>>,
    /// Default quota for unknown message types
    default_quota: Quota,
}

impl MessageRateLimiter {
    /// Create a new message rate limiter
    pub fn new() -> Self {
        let default_quota = Quota::per_second(std::num::NonZeroU32::new(50).unwrap())
            .allow_burst(std::num::NonZeroU32::new(5).unwrap());

        Self {
            limiters: std::collections::HashMap::new(),
            default_quota,
        }
    }

    /// Set rate limit for a specific message type
    pub fn set_message_limit(&mut self, msg_type: String, per_second: u32, burst: u32) {
        let quota = Quota::per_second(
            std::num::NonZeroU32::new(per_second).unwrap_or(std::num::NonZeroU32::new(1).unwrap()),
        )
        .allow_burst(
            std::num::NonZeroU32::new(burst).unwrap_or(std::num::NonZeroU32::new(1).unwrap()),
        );
        let limiter = Arc::new(Governor::new(
            quota,
            InMemoryState::default(),
            &QuantaClock::default(),
        ));
        self.limiters.insert(msg_type, limiter);
    }

    /// Check if a message type is allowed
    pub fn check_message(&mut self, msg_type: &str) -> P2PResult<()> {
        let limiter = self
            .limiters
            .entry(msg_type.to_string())
            .or_insert_with(|| {
                Arc::new(Governor::new(
                    self.default_quota,
                    InMemoryState::default(),
                    &QuantaClock::default(),
                ))
            });

        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => Err(P2PError::RateLimitExceeded(format!(
                "Rate limit exceeded for message type: {}",
                msg_type
            ))),
        }
    }
}

impl Default for MessageRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

