use crate::{BlockchainType, MultivmError, ProcessId};
use async_trait::async_trait;

/// Trait for event handling and notifications
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle a system event
    async fn handle_event(&self, event: SystemEvent) -> Result<(), MultivmError>;

    /// Subscribe to specific event types
    async fn subscribe(&self, event_types: Vec<EventType>) -> Result<(), MultivmError>;

    /// Unsubscribe from event types
    async fn unsubscribe(&self, event_types: Vec<EventType>) -> Result<(), MultivmError>;
}

/// System events that can occur
#[derive(Debug, Clone)]
pub enum SystemEvent {
    ProcessStarted {
        process_id: ProcessId,
    },
    ProcessStopped {
        process_id: ProcessId,
        exit_code: Option<i32>,
    },
    ProcessCrashed {
        process_id: ProcessId,
        error: String,
    },
    BlockProcessed {
        blockchain_type: BlockchainType,
        block_id: u64,
        success: bool,
    },
    RpcCallReceived {
        method: String,
        blockchain_type: BlockchainType,
    },
    HealthCheckFailed {
        process_id: ProcessId,
        error: String,
    },
    ResourceLimitExceeded {
        process_id: ProcessId,
        resource: String,
    },
    ConfigurationChanged {
        component: String,
    },
}

/// Event type categories for subscription
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    Process,
    Block,
    Rpc,
    Health,
    Resource,
    Configuration,
    All,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_types() {
        let event = SystemEvent::ProcessStarted {
            process_id: ProcessId::Solana,
        };

        match event {
            SystemEvent::ProcessStarted { process_id } => {
                assert_eq!(process_id, ProcessId::Solana);
            }
            _ => panic!("Expected ProcessStarted event"),
        }
    }
}
