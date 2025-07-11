//! Message types for Raft consensus communication

use crate::{NodeId, storage::LogEntry};
use serde::{Deserialize, Serialize};

/// Type of message in the Raft protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Request for votes during leader election
    RequestVote,
    /// Response to vote request
    RequestVoteResponse,
    /// Append entries for log replication
    AppendEntries,
    /// Response to append entries
    AppendEntriesResponse,
    /// Heartbeat message from leader
    Heartbeat,
    /// Install snapshot for lagging followers
    InstallSnapshot,
    /// Response to install snapshot
    InstallSnapshotResponse,
    /// Handshake for connection establishment
    Handshake,
}

/// Core message structure for Raft protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Type of the message
    pub msg_type: MessageType,
    /// Term number
    pub term: u64,
    /// Sender node ID
    pub from: NodeId,
    /// Recipient node ID
    pub to: NodeId,
    /// Message payload
    pub payload: MessagePayload,
}

/// Payload for different message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Vote request payload
    RequestVote {
        last_log_index: u64,
        last_log_term: u64,
    },
    /// Vote response payload
    RequestVoteResponse {
        vote_granted: bool,
    },
    /// Append entries payload
    AppendEntries {
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    /// Append entries response payload
    AppendEntriesResponse {
        success: bool,
        match_index: u64,
        conflict_index: Option<u64>,
        conflict_term: Option<u64>,
    },
    /// Heartbeat payload (empty)
    Heartbeat,
    /// Install snapshot payload
    InstallSnapshot {
        last_included_index: u64,
        last_included_term: u64,
        offset: u64,
        data: Vec<u8>,
        done: bool,
    },
    /// Install snapshot response payload
    InstallSnapshotResponse {
        term: u64,
    },
    /// Handshake payload
    Handshake,
}

impl Message {
    /// Create a new vote request message
    pub fn request_vote(
        term: u64,
        from: NodeId,
        to: NodeId,
        last_log_index: u64,
        last_log_term: u64,
    ) -> Self {
        Self {
            msg_type: MessageType::RequestVote,
            term,
            from,
            to,
            payload: MessagePayload::RequestVote {
                last_log_index,
                last_log_term,
            },
        }
    }
    
    /// Create a new vote response message
    pub fn request_vote_response(
        term: u64,
        from: NodeId,
        to: NodeId,
        vote_granted: bool,
    ) -> Self {
        Self {
            msg_type: MessageType::RequestVoteResponse,
            term,
            from,
            to,
            payload: MessagePayload::RequestVoteResponse { vote_granted },
        }
    }
    
    /// Create a new append entries message
    pub fn append_entries(
        term: u64,
        from: NodeId,
        to: NodeId,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    ) -> Self {
        Self {
            msg_type: MessageType::AppendEntries,
            term,
            from,
            to,
            payload: MessagePayload::AppendEntries {
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            },
        }
    }
    
    /// Create a new append entries response message
    pub fn append_entries_response(
        term: u64,
        from: NodeId,
        to: NodeId,
        success: bool,
        match_index: u64,
        conflict_index: Option<u64>,
        conflict_term: Option<u64>,
    ) -> Self {
        Self {
            msg_type: MessageType::AppendEntriesResponse,
            term,
            from,
            to,
            payload: MessagePayload::AppendEntriesResponse {
                success,
                match_index,
                conflict_index,
                conflict_term,
            },
        }
    }
    
    /// Create a new heartbeat message
    pub fn heartbeat(term: u64, from: NodeId, to: NodeId) -> Self {
        Self {
            msg_type: MessageType::Heartbeat,
            term,
            from,
            to,
            payload: MessagePayload::Heartbeat,
        }
    }
}