//! Log replication logic for Raft consensus

use crate::{
    Message, MessagePayload, NodeId, Result, ConsensusError,
    raft::{Config, state::RaftState},
    storage::{LogEntry, Storage},
    transport::Transport,
};
use std::cmp::min;
use tracing::{debug, warn};

/// Send AppendEntries to all followers
pub async fn replicate_to_followers<S: Storage, T: Transport>(
    config: &Config,
    state: &RaftState,
    storage: &S,
    transport: &T,
) -> Result<()> {
    let (current_term, commit_index) = {
        let server = state.server.read().await;
        (server.current_term, server.commit_index)
    };
    
    let leader_guard = state.leader.read().await;
    let leader = leader_guard.as_ref().ok_or(ConsensusError::NotLeader)?;
    
    for &follower in &config.peers {
        if follower == config.node_id {
            continue;
        }
        
        let next_index = leader.next_index.get(&follower).copied().unwrap_or(1);
        let prev_index = next_index.saturating_sub(1);
        let prev_term = if prev_index > 0 {
            storage.get_entry(prev_index).await?
                .map(|e| e.term)
                .unwrap_or(0)
        } else {
            0
        };
        
        // Get entries to send
        let last_index = storage.last_index().await?;
        let max_entries = min(config.max_append_entries, (last_index - prev_index) as usize);
        let entries = if max_entries > 0 {
            storage.get_entries(next_index, next_index + max_entries as u64).await?
        } else {
            Vec::new()
        };
        
        let msg = Message::append_entries(
            current_term,
            config.node_id,
            follower,
            prev_index,
            prev_term,
            entries,
            commit_index,
        );
        
        // Send without waiting for response
        let _ = transport.send(follower, msg).await;
    }
    
    Ok(())
}

/// Handle AppendEntries message
pub async fn handle_append_entries<S: Storage, T: Transport>(
    config: &Config,
    state: &RaftState,
    storage: &S,
    transport: &T,
    from: NodeId,
    term: u64,
    prev_log_index: u64,
    prev_log_term: u64,
    entries: Vec<LogEntry>,
    leader_commit: u64,
) -> Result<()> {
    let mut success = false;
    let mut match_index = 0;
    let mut conflict_index = None;
    let mut conflict_term = None;
    
    let current_term = {
        let mut server = state.server.write().await;
        
        // Update term if necessary
        if term > server.current_term {
            server.current_term = term;
            server.voted_for = None;
            server.state = crate::NodeState::Follower;
            let new_term = server.current_term;
            drop(server);
            storage.save_term(term).await?;
            storage.save_vote(None).await?;
            
            let mut server = state.server.write().await;
            server.leader_id = Some(from);
            server.state = crate::NodeState::Follower;
            new_term
        } else if term < server.current_term {
            // Reject if term is old
            let current_term = server.current_term;
            drop(server);
            let response = Message::append_entries_response(
                current_term,
                config.node_id,
                from,
                false,
                0,
                None,
                None,
            );
            transport.send(from, response).await?;
            return Ok(());
        } else {
            // We're hearing from the leader
            server.leader_id = Some(from);
            server.state = crate::NodeState::Follower;
            server.current_term
        }
    };
    
    // Check log consistency
    if prev_log_index > 0 {
        if let Some(entry) = storage.get_entry(prev_log_index).await? {
            if entry.term != prev_log_term {
                // Log inconsistency
                conflict_term = Some(entry.term);
                
                // Find first index with conflict term
                for i in 1..=prev_log_index {
                    if let Some(e) = storage.get_entry(i).await? {
                        if e.term == entry.term {
                            conflict_index = Some(i);
                            break;
                        }
                    }
                }
            } else {
                success = true;
            }
        } else {
            // We don't have the previous entry
            let last_index = storage.last_index().await?;
            conflict_index = Some(last_index + 1);
        }
    } else {
        success = true;
    }
    
    if success && !entries.is_empty() {
        // Delete conflicting entries and append new ones
        let mut idx = prev_log_index + 1;
        let mut new_entries = Vec::new();
        
        for entry in entries {
            if let Some(existing) = storage.get_entry(idx).await? {
                if existing.term != entry.term {
                    // Delete this and all following entries
                    storage.delete_entries_from(idx).await?;
                    new_entries.push(entry);
                }
            } else {
                new_entries.push(entry);
            }
            idx += 1;
        }
        
        if !new_entries.is_empty() {
            storage.append_entries(&new_entries).await?;
        }
        
        match_index = idx - 1;
    } else if success {
        match_index = prev_log_index;
    }
    
    // Update commit index
    if success && leader_commit > 0 {
        let mut server = state.server.write().await;
        let last_index = storage.last_index().await?;
        server.commit_index = min(leader_commit, last_index);
    }
    
    // Send response
    let response = Message::append_entries_response(
        current_term,
        config.node_id,
        from,
        success,
        match_index,
        conflict_index,
        conflict_term,
    );
    
    transport.send(from, response).await?;
    
    Ok(())
}

/// Handle AppendEntriesResponse message
pub async fn handle_append_entries_response(
    config: &Config,
    state: &RaftState,
    from: NodeId,
    term: u64,
    success: bool,
    match_index: u64,
    conflict_index: Option<u64>,
    _conflict_term: Option<u64>,
) -> Result<()> {
    let (should_ignore, should_step_down, is_leader) = {
        let server = state.server.read().await;
        (
            term < server.current_term,
            term > server.current_term,
            server.state == crate::NodeState::Leader && term == server.current_term
        )
    };
    
    // Ignore old responses
    if should_ignore {
        return Ok(());
    }
    
    // Step down if we get a newer term
    if should_step_down {
        let mut server = state.server.write().await;
        server.current_term = term;
        server.voted_for = None;
        server.state = crate::NodeState::Follower;
        server.leader_id = None;
        return Ok(());
    }
    
    // Only process if we're still leader
    if !is_leader {
        return Ok(());
    }
    
    // Update follower progress
    let new_commit_index = {
        let mut leader_guard = state.leader.write().await;
        if let Some(leader) = leader_guard.as_mut() {
            if success {
                leader.update_progress(from, match_index);
                
                // Check if we can advance commit index
                let mut match_indices: Vec<u64> = leader.match_index.values().copied().collect();
                match_indices.push(match_index); // Include our own progress
                match_indices.sort_unstable();
                
                Some(match_indices[match_indices.len() / 2])
            } else {
                // Handle conflict
                if let Some(conflict_idx) = conflict_index {
                    leader.next_index.insert(from, conflict_idx);
                } else {
                    leader.decrement_next_index(from);
                }
                None
            }
        } else {
            None
        }
    };
    
    if let Some(new_commit_index) = new_commit_index {
        let mut server = state.server.write().await;
        if new_commit_index > server.commit_index {
            server.commit_index = new_commit_index;
            
            debug!(
                node_id = %config.node_id,
                commit_index = new_commit_index,
                "Advanced commit index"
            );
        }
    }
    
    Ok(())
}