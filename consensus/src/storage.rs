//! Storage traits and implementations for the consensus log

use crate::{NodeId, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Entry in the consensus log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Term when entry was received by leader
    pub term: u64,
    /// Position in the log
    pub index: u64,
    /// Command data
    pub data: Vec<u8>,
}

/// Persistent state that must be saved before responding to RPCs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    /// Latest term server has seen
    pub current_term: u64,
    /// Candidate that received vote in current term
    pub voted_for: Option<NodeId>,
    /// Log entries
    pub log: Vec<LogEntry>,
}

/// Storage trait for consensus state persistence
#[async_trait]
pub trait Storage: Send + Sync {
    /// Save the current term
    async fn save_term(&self, term: u64) -> Result<()>;
    
    /// Load the current term
    async fn load_term(&self) -> Result<u64>;
    
    /// Save the vote
    async fn save_vote(&self, candidate: Option<NodeId>) -> Result<()>;
    
    /// Load the vote
    async fn load_vote(&self) -> Result<Option<NodeId>>;
    
    /// Append entries to the log
    async fn append_entries(&self, entries: &[LogEntry]) -> Result<()>;
    
    /// Get a log entry at the given index
    async fn get_entry(&self, index: u64) -> Result<Option<LogEntry>>;
    
    /// Get entries in the range [start, end)
    async fn get_entries(&self, start: u64, end: u64) -> Result<Vec<LogEntry>>;
    
    /// Get the last log index
    async fn last_index(&self) -> Result<u64>;
    
    /// Get the last log term
    async fn last_term(&self) -> Result<u64>;
    
    /// Delete entries from index onwards
    async fn delete_entries_from(&self, index: u64) -> Result<()>;
    
    /// Save a snapshot
    async fn save_snapshot(&self, index: u64, term: u64, data: Vec<u8>) -> Result<()>;
    
    /// Load the latest snapshot
    async fn load_snapshot(&self) -> Result<Option<(u64, u64, Vec<u8>)>>;
}

/// In-memory storage implementation (for testing)
#[derive(Debug)]
pub struct MemoryStorage {
    state: Arc<parking_lot::RwLock<PersistentState>>,
    snapshot: Arc<parking_lot::RwLock<Option<(u64, u64, Vec<u8>)>>>,
}

impl MemoryStorage {
    /// Create a new in-memory storage
    pub fn new() -> Self {
        Self {
            state: Arc::new(parking_lot::RwLock::new(PersistentState {
                current_term: 0,
                voted_for: None,
                log: Vec::new(),
            })),
            snapshot: Arc::new(parking_lot::RwLock::new(None)),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: Storage + ?Sized> Storage for Arc<T> {
    async fn save_term(&self, term: u64) -> Result<()> {
        (**self).save_term(term).await
    }
    
    async fn load_term(&self) -> Result<u64> {
        (**self).load_term().await
    }
    
    async fn save_vote(&self, candidate: Option<NodeId>) -> Result<()> {
        (**self).save_vote(candidate).await
    }
    
    async fn load_vote(&self) -> Result<Option<NodeId>> {
        (**self).load_vote().await
    }
    
    async fn append_entries(&self, entries: &[LogEntry]) -> Result<()> {
        (**self).append_entries(entries).await
    }
    
    async fn get_entry(&self, index: u64) -> Result<Option<LogEntry>> {
        (**self).get_entry(index).await
    }
    
    async fn get_entries(&self, start: u64, end: u64) -> Result<Vec<LogEntry>> {
        (**self).get_entries(start, end).await
    }
    
    async fn last_index(&self) -> Result<u64> {
        (**self).last_index().await
    }
    
    async fn last_term(&self) -> Result<u64> {
        (**self).last_term().await
    }
    
    async fn delete_entries_from(&self, index: u64) -> Result<()> {
        (**self).delete_entries_from(index).await
    }
    
    async fn save_snapshot(&self, index: u64, term: u64, data: Vec<u8>) -> Result<()> {
        (**self).save_snapshot(index, term, data).await
    }
    
    async fn load_snapshot(&self) -> Result<Option<(u64, u64, Vec<u8>)>> {
        (**self).load_snapshot().await
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn save_term(&self, term: u64) -> Result<()> {
        self.state.write().current_term = term;
        Ok(())
    }
    
    async fn load_term(&self) -> Result<u64> {
        Ok(self.state.read().current_term)
    }
    
    async fn save_vote(&self, candidate: Option<NodeId>) -> Result<()> {
        self.state.write().voted_for = candidate;
        Ok(())
    }
    
    async fn load_vote(&self) -> Result<Option<NodeId>> {
        Ok(self.state.read().voted_for)
    }
    
    async fn append_entries(&self, entries: &[LogEntry]) -> Result<()> {
        self.state.write().log.extend_from_slice(entries);
        Ok(())
    }
    
    async fn get_entry(&self, index: u64) -> Result<Option<LogEntry>> {
        Ok(self.state.read().log.get(index as usize).cloned())
    }
    
    async fn get_entries(&self, start: u64, end: u64) -> Result<Vec<LogEntry>> {
        let state = self.state.read();
        let start = start as usize;
        let end = (end as usize).min(state.log.len());
        Ok(state.log[start..end].to_vec())
    }
    
    async fn last_index(&self) -> Result<u64> {
        Ok(self.state.read().log.len() as u64)
    }
    
    async fn last_term(&self) -> Result<u64> {
        Ok(self.state.read().log.last().map(|e| e.term).unwrap_or(0))
    }
    
    async fn delete_entries_from(&self, index: u64) -> Result<()> {
        if index > 0 {
            let mut state = self.state.write();
            // Convert 1-based index to 0-based for Vec truncation
            let truncate_idx = (index - 1) as usize;
            if truncate_idx < state.log.len() {
                state.log.truncate(truncate_idx);
            }
        }
        Ok(())
    }
    
    async fn save_snapshot(&self, index: u64, term: u64, data: Vec<u8>) -> Result<()> {
        *self.snapshot.write() = Some((index, term, data));
        Ok(())
    }
    
    async fn load_snapshot(&self) -> Result<Option<(u64, u64, Vec<u8>)>> {
        Ok(self.snapshot.read().clone())
    }
}