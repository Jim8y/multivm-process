use serde::{Deserialize, Serialize};

/// Unique identifier for messages between processes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(u64);

impl MessageId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Process identifier for different blockchain engines
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcessId {
    Main,
    Solana,
    Ethereum,
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessId::Main => write!(f, "main"),
            ProcessId::Solana => write!(f, "solana"),
            ProcessId::Ethereum => write!(f, "ethereum"),
        }
    }
}

/// Blockchain type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockchainType {
    Solana,
    Ethereum,
}

impl std::fmt::Display for BlockchainType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockchainType::Solana => write!(f, "solana"),
            BlockchainType::Ethereum => write!(f, "ethereum"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_uniqueness() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_process_id_display() {
        assert_eq!(ProcessId::Main.to_string(), "main");
        assert_eq!(ProcessId::Solana.to_string(), "solana");
        assert_eq!(ProcessId::Ethereum.to_string(), "ethereum");
    }

    #[test]
    fn test_blockchain_type_display() {
        assert_eq!(BlockchainType::Solana.to_string(), "solana");
        assert_eq!(BlockchainType::Ethereum.to_string(), "ethereum");
    }
}
