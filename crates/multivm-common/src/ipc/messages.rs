use crate::{
    BlockchainType, EngineState, HealthStatus, MessageId, ProcessId, RpcCall, RpcResponse,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// IPC message envelope for communication between processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    pub id: MessageId,
    pub source: ProcessId,
    pub destination: ProcessId,
    pub command: IpcCommand,
    pub timestamp: SystemTime,
    pub timeout: Option<Duration>,
}

impl IpcMessage {
    pub fn new(source: ProcessId, destination: ProcessId, command: IpcCommand) -> Self {
        Self {
            id: MessageId::new(),
            source,
            destination,
            command,
            timestamp: SystemTime::now(),
            timeout: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(timeout) = self.timeout {
            if let Ok(elapsed) = self.timestamp.elapsed() {
                return elapsed > timeout;
            }
        }
        false
    }
}

/// Commands that can be sent between processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcCommand {
    /// Process a block (using byte array for serialized transmission)
    ProcessBlock {
        block_data_bytes: Vec<u8>, // Serialized block data
        blockchain_type: BlockchainType,
        expect_response: bool,
    },

    /// Get current health status
    GetHealth,

    /// Get current engine state
    GetState,

    /// Request shutdown
    Shutdown {
        graceful: bool,
        timeout: Option<Duration>,
    },

    /// Heartbeat/ping
    Ping,

    /// Execute RPC call
    RpcCall { call: RpcCall },

    /// Request next block from main process
    RequestNextBlock {
        current_block: Option<u64>,
        blockchain_type: BlockchainType,
    },

    /// Start/stop RPC server
    ConfigureRpc { enable: bool, port: Option<u16> },

    /// Update configuration
    UpdateConfig {
        config_data: Vec<u8>, // Serialized configuration
    },

    /// Health check command
    HealthCheck,
}

/// Responses that can be sent between processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    /// Acknowledgment without data
    Ack,

    /// Block processing result (using byte array for serialized result)
    BlockProcessed {
        result_bytes: Vec<u8>, // Serialized processing result
        blockchain_type: BlockchainType,
        success: bool,
    },

    /// Health status response
    Health { status: HealthStatus },

    /// Engine state response
    State { state: EngineState },

    /// Pong response to ping
    Pong,

    /// RPC call response
    RpcResponse { response: RpcResponse },

    /// Next block response (using byte array for serialization)
    NextBlock {
        block_data_bytes: Option<Vec<u8>>, // Serialized block data
        blockchain_type: Option<BlockchainType>,
    },

    /// Error response
    Error {
        code: i32,
        message: String,
        details: Option<String>,
    },

    /// Health check response
    HealthCheck,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_creation() {
        let msg = IpcMessage::new(ProcessId::Main, ProcessId::Solana, IpcCommand::Ping);

        assert_eq!(msg.source, ProcessId::Main);
        assert_eq!(msg.destination, ProcessId::Solana);
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_ipc_message_timeout() {
        let msg = IpcMessage::new(ProcessId::Main, ProcessId::Ethereum, IpcCommand::GetHealth)
            .with_timeout(Duration::from_millis(1));

        // Message should not be expired immediately
        assert!(!msg.is_expired());

        // Wait and check if expired (this test might be flaky in very slow environments)
        std::thread::sleep(Duration::from_millis(2));
        assert!(msg.is_expired());
    }
}
