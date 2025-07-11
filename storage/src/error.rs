//! Storage error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Corruption detected: {0}")]
    Corruption(String),
    
    #[error("Entry not found at index {0}")]
    EntryNotFound(u64),
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    #[error("WAL error: {0}")]
    Wal(String),
    
    #[error("Snapshot error: {0}")]
    Snapshot(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Encryption error: {0}")]
    Encryption(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

impl From<StorageError> for consensus::ConsensusError {
    fn from(err: StorageError) -> Self {
        consensus::ConsensusError::Storage(err.to_string())
    }
}