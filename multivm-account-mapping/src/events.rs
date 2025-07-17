//! Event emission system for account binding operations

use crate::{
    address::{AccountAddress, MultivmAccountId},
    binding_message::BindingAction,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Account binding related events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountBindingEvent {
    /// Automatic binding created when account first appears
    AutoBindingCreated {
        account: AccountAddress,
        multivm_id: MultivmAccountId,
        timestamp: SystemTime,
        transaction_hash: Option<Vec<u8>>,
    },
    /// Cross-VM binding added
    CrossBindingAdded {
        source: AccountAddress,
        target: AccountAddress,
        multivm_id: MultivmAccountId,
        proof_type: String,
        timestamp: SystemTime,
    },
    /// Binding removed
    BindingRemoved {
        account: AccountAddress,
        multivm_id: MultivmAccountId,
        timestamp: SystemTime,
        reason: String,
    },
    /// Binding update
    BindingUpdated {
        multivm_id: MultivmAccountId,
        changes: Vec<String>,
        timestamp: SystemTime,
    },
    /// Recovery initiated
    RecoveryInitiated {
        multivm_id: MultivmAccountId,
        new_account: AccountAddress,
        guardian_count: usize,
        timestamp: SystemTime,
        executable_at: SystemTime,
    },
    /// Recovery completed
    RecoveryCompleted {
        multivm_id: MultivmAccountId,
        old_account: AccountAddress,
        new_account: AccountAddress,
        timestamp: SystemTime,
    },
    /// Guardian added
    GuardianAdded {
        multivm_id: MultivmAccountId,
        guardian: AccountAddress,
        timestamp: SystemTime,
    },
    /// Guardian removed
    GuardianRemoved {
        multivm_id: MultivmAccountId,
        guardian: AccountAddress,
        timestamp: SystemTime,
    },
    /// Suspicious activity detected
    SuspiciousActivity {
        multivm_id: MultivmAccountId,
        account: Option<AccountAddress>,
        activity_type: String,
        risk_score: u8,
        timestamp: SystemTime,
    },
    /// Rate limit exceeded
    RateLimitExceeded {
        account: AccountAddress,
        action: BindingAction,
        limit_type: String,
        timestamp: SystemTime,
    },
}

impl AccountBindingEvent {
    /// Get event timestamp
    pub fn timestamp(&self) -> SystemTime {
        match self {
            Self::AutoBindingCreated { timestamp, .. } => *timestamp,
            Self::CrossBindingAdded { timestamp, .. } => *timestamp,
            Self::BindingRemoved { timestamp, .. } => *timestamp,
            Self::BindingUpdated { timestamp, .. } => *timestamp,
            Self::RecoveryInitiated { timestamp, .. } => *timestamp,
            Self::RecoveryCompleted { timestamp, .. } => *timestamp,
            Self::GuardianAdded { timestamp, .. } => *timestamp,
            Self::GuardianRemoved { timestamp, .. } => *timestamp,
            Self::SuspiciousActivity { timestamp, .. } => *timestamp,
            Self::RateLimitExceeded { timestamp, .. } => *timestamp,
        }
    }

    /// Get event type as string
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AutoBindingCreated { .. } => "AutoBindingCreated",
            Self::CrossBindingAdded { .. } => "CrossBindingAdded",
            Self::BindingRemoved { .. } => "BindingRemoved",
            Self::BindingUpdated { .. } => "BindingUpdated",
            Self::RecoveryInitiated { .. } => "RecoveryInitiated",
            Self::RecoveryCompleted { .. } => "RecoveryCompleted",
            Self::GuardianAdded { .. } => "GuardianAdded",
            Self::GuardianRemoved { .. } => "GuardianRemoved",
            Self::SuspiciousActivity { .. } => "SuspiciousActivity",
            Self::RateLimitExceeded { .. } => "RateLimitExceeded",
        }
    }

    /// Get severity level (for alerting)
    pub fn severity(&self) -> EventSeverity {
        match self {
            Self::AutoBindingCreated { .. } => EventSeverity::Info,
            Self::CrossBindingAdded { .. } => EventSeverity::Info,
            Self::BindingRemoved { .. } => EventSeverity::Warning,
            Self::BindingUpdated { .. } => EventSeverity::Info,
            Self::RecoveryInitiated { .. } => EventSeverity::Warning,
            Self::RecoveryCompleted { .. } => EventSeverity::Warning,
            Self::GuardianAdded { .. } => EventSeverity::Info,
            Self::GuardianRemoved { .. } => EventSeverity::Warning,
            Self::SuspiciousActivity { risk_score, .. } => {
                if *risk_score > 80 {
                    EventSeverity::Critical
                } else if *risk_score > 50 {
                    EventSeverity::Warning
                } else {
                    EventSeverity::Info
                }
            }
            Self::RateLimitExceeded { .. } => EventSeverity::Warning,
        }
    }
}

/// Event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
}

/// Event listener trait
#[async_trait::async_trait]
pub trait EventListener: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: AccountBindingEvent);
}

/// Event emitter for broadcasting events
pub struct EventEmitter {
    listeners: Vec<Arc<dyn EventListener>>,
    event_sender: Option<mpsc::UnboundedSender<AccountBindingEvent>>,
}

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
            event_sender: None,
        }
    }

    /// Add a listener
    pub fn add_listener(&mut self, listener: Arc<dyn EventListener>) {
        self.listeners.push(listener);
    }

    /// Set up event channel
    pub fn setup_channel(&mut self) -> mpsc::UnboundedReceiver<AccountBindingEvent> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.event_sender = Some(sender);
        receiver
    }

    /// Emit an event
    pub async fn emit(&self, event: AccountBindingEvent) {
        info!(
            "Emitting event: {} with severity: {:?}",
            event.event_type(),
            event.severity()
        );

        // Send to channel if configured
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event.clone());
        }

        // Notify all listeners
        for listener in &self.listeners {
            listener.handle_event(event.clone()).await;
        }
    }
}

/// Logging event listener
pub struct LoggingEventListener;

#[async_trait::async_trait]
impl EventListener for LoggingEventListener {
    async fn handle_event(&self, event: AccountBindingEvent) {
        match event.severity() {
            EventSeverity::Info => {
                info!("Account binding event: {:?}", event);
            }
            EventSeverity::Warning => {
                tracing::warn!("Account binding warning: {:?}", event);
            }
            EventSeverity::Critical => {
                tracing::error!("Account binding critical event: {:?}", event);
            }
        }
    }
}

/// Metrics event listener
pub struct MetricsEventListener {
    // In a real implementation, this would hold metrics collectors
}

#[async_trait::async_trait]
impl EventListener for MetricsEventListener {
    async fn handle_event(&self, event: AccountBindingEvent) {
        debug!("Recording metrics for event: {}", event.event_type());

        // In a real implementation, this would update Prometheus metrics
        match &event {
            AccountBindingEvent::AutoBindingCreated { .. } => {
                // Increment auto_bindings_created counter
            }
            AccountBindingEvent::CrossBindingAdded { .. } => {
                // Increment cross_bindings_added counter
            }
            AccountBindingEvent::SuspiciousActivity { risk_score, .. } => {
                // Record risk score histogram
                debug!("Risk score: {}", risk_score);
            }
            _ => {}
        }
    }
}

/// Webhook event listener for external notifications
pub struct WebhookEventListener {
    webhook_url: String,
    client: reqwest::Client,
}

impl WebhookEventListener {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl EventListener for WebhookEventListener {
    async fn handle_event(&self, event: AccountBindingEvent) {
        // Only send webhooks for warning/critical events
        if event.severity() == EventSeverity::Info {
            return;
        }

        let payload = serde_json::json!({
            "event_type": event.event_type(),
            "severity": format!("{:?}", event.severity()),
            "timestamp": event.timestamp().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            "data": event,
        });

        match self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    tracing::error!("Webhook failed with status: {}", response.status());
                }
            }
            Err(e) => {
                tracing::error!("Failed to send webhook: {}", e);
            }
        }
    }
}

/// Event store for persistence
pub struct EventStore {
    events: Arc<tokio::sync::RwLock<Vec<AccountBindingEvent>>>,
    max_events: usize,
}

impl EventStore {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            max_events,
        }
    }

    pub async fn store(&self, event: AccountBindingEvent) {
        let mut events = self.events.write().await;
        events.push(event);

        // Keep only the most recent events
        if events.len() > self.max_events {
            let drain_count = events.len() - self.max_events;
            events.drain(0..drain_count);
        }
    }

    pub async fn get_events(&self, filter: Option<EventFilter>) -> Vec<AccountBindingEvent> {
        let events = self.events.read().await;

        if let Some(filter) = filter {
            events
                .iter()
                .filter(|e| filter.matches(e))
                .cloned()
                .collect()
        } else {
            events.clone()
        }
    }
}

/// Event filter for querying
pub struct EventFilter {
    pub multivm_id: Option<MultivmAccountId>,
    pub account: Option<AccountAddress>,
    pub event_types: Option<Vec<String>>,
    pub min_severity: Option<EventSeverity>,
    pub since: Option<SystemTime>,
}

impl EventFilter {
    pub fn matches(&self, event: &AccountBindingEvent) -> bool {
        // Check MultiVM ID
        if let Some(ref id) = self.multivm_id {
            let event_id = match event {
                AccountBindingEvent::AutoBindingCreated { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::CrossBindingAdded { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::BindingRemoved { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::BindingUpdated { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::RecoveryInitiated { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::RecoveryCompleted { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::GuardianAdded { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::GuardianRemoved { multivm_id, .. } => Some(multivm_id),
                AccountBindingEvent::SuspiciousActivity { multivm_id, .. } => Some(multivm_id),
                _ => None,
            };

            if let Some(event_id) = event_id {
                if event_id != id {
                    return false;
                }
            }
        }

        // Check event type
        if let Some(ref types) = self.event_types {
            if !types.contains(&event.event_type().to_string()) {
                return false;
            }
        }

        // Check severity
        if let Some(min_severity) = self.min_severity {
            let severity_value = |s: EventSeverity| match s {
                EventSeverity::Info => 0,
                EventSeverity::Warning => 1,
                EventSeverity::Critical => 2,
            };

            if severity_value(event.severity()) < severity_value(min_severity) {
                return false;
            }
        }

        // Check timestamp
        if let Some(since) = self.since {
            if event.timestamp() < since {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{EthereumAddress, SolanaAddress};

    #[tokio::test]
    async fn test_event_emission() {
        let mut emitter = EventEmitter::new();
        let logging_listener = Arc::new(LoggingEventListener);
        emitter.add_listener(logging_listener);

        let event = AccountBindingEvent::AutoBindingCreated {
            account: AccountAddress::Ethereum(EthereumAddress([1u8; 20])),
            multivm_id: MultivmAccountId::from_seed(b"test"),
            timestamp: SystemTime::now(),
            transaction_hash: None,
        };

        emitter.emit(event).await;
    }

    #[test]
    fn test_event_severity() {
        let low_risk_event = AccountBindingEvent::SuspiciousActivity {
            multivm_id: MultivmAccountId::from_seed(b"test"),
            account: None,
            activity_type: "test".to_string(),
            risk_score: 30,
            timestamp: SystemTime::now(),
        };
        assert_eq!(low_risk_event.severity(), EventSeverity::Info);

        let high_risk_event = AccountBindingEvent::SuspiciousActivity {
            multivm_id: MultivmAccountId::from_seed(b"test"),
            account: None,
            activity_type: "test".to_string(),
            risk_score: 90,
            timestamp: SystemTime::now(),
        };
        assert_eq!(high_risk_event.severity(), EventSeverity::Critical);
    }

    #[tokio::test]
    async fn test_event_store() {
        let store = EventStore::new(10);

        for i in 0..15 {
            let event = AccountBindingEvent::AutoBindingCreated {
                account: AccountAddress::Solana(SolanaAddress([i as u8; 32])),
                multivm_id: MultivmAccountId::from_seed(&[i as u8]),
                timestamp: SystemTime::now(),
                transaction_hash: None,
            };
            store.store(event).await;
        }

        let events = store.get_events(None).await;
        assert_eq!(events.len(), 10); // Should only keep last 10
    }
}
