use crate::{BlockchainType, MessageId};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Block request from main process to engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRequest {
    pub request_id: MessageId,
    pub blockchain_type: BlockchainType,
    pub expected_block_id: u64,
    pub timeout: Duration,
}

/// Generic block response - let each engine define its own block type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse<T> {
    pub request_id: MessageId,
    pub block_data: Option<T>,
    pub error: Option<String>,
}

/// For backward compatibility, keep a simple byte version (for IPC communication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericBlockResponse {
    pub request_id: MessageId,
    pub block_data_bytes: Option<Vec<u8>>, // Serialized block data
    pub blockchain_type: BlockchainType,
    pub error: Option<String>,
}
