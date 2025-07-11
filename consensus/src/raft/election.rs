//! Election logic for Raft consensus

use crate::{
    Message, MessagePayload, NodeId, Result,
    raft::{Config, state::RaftState},
    storage::Storage,
    transport::Transport,
};
use tracing::{debug, info, warn};

/// Handle election timeout - start a new election
pub async fn start_election<S: Storage, T: Transport>(
    config: &Config,
    state: &RaftState,
    storage: &S,
    transport: &T,
) -> Result<()> {
    info!(node_id = %config.node_id, "Starting election");
    
    // Transition to candidate
    state.become_candidate(config.node_id).await;
    
    // Save state
    let (current_term, voted_for) = {
        let server = state.server.read().await;
        (server.current_term, server.voted_for)
    };
    
    storage.save_term(current_term).await?;
    storage.save_vote(voted_for).await?;
    
    // Get last log info
    let last_index = storage.last_index().await?;
    let last_term = storage.last_term().await?;
    
    for &peer in &config.peers {
        if peer != config.node_id {
            let msg = Message::request_vote(
                current_term,
                config.node_id,
                peer,
                last_index,
                last_term,
            );
            
            // Fire and forget - we'll handle responses as they come
            let _ = transport.send(peer, msg).await;
        }
    }
    
    Ok(())
}

/// Handle a RequestVote message
pub async fn handle_request_vote<S: Storage, T: Transport>(
    config: &Config,
    state: &RaftState,
    storage: &S,
    transport: &T,
    from: NodeId,
    term: u64,
    last_log_index: u64,
    last_log_term: u64,
) -> Result<()> {
    let mut vote_granted = false;
    
    {
        let mut server = state.server.write().await;
        
        // Update term if necessary
        if term > server.current_term {
            server.current_term = term;
            server.voted_for = None;
            server.state = crate::NodeState::Follower;
            server.leader_id = None;
        }
    }
    
    // Check if we can grant the vote
    let (can_vote, current_term) = {
        let server = state.server.read().await;
        let can_vote = term == server.current_term && 
            (server.voted_for.is_none() || server.voted_for == Some(from));
        (can_vote, server.current_term)
    };
    
    if can_vote {
        // Check log is up to date
        let our_last_index = storage.last_index().await?;
        let our_last_term = storage.last_term().await?;
        
        let log_ok = last_log_term > our_last_term ||
            (last_log_term == our_last_term && last_log_index >= our_last_index);
            
        if log_ok {
            vote_granted = true;
        }
    }
    
    // Grant vote if appropriate
    if vote_granted {
        {
            let mut server = state.server.write().await;
            server.voted_for = Some(from);
        }
        storage.save_vote(Some(from)).await?;
        
        debug!(
            node_id = %config.node_id,
            candidate = %from,
            term = current_term,
            "Granted vote"
        );
    }
    
    // Send response
    let response = Message::request_vote_response(
        current_term,
        config.node_id,
        from,
        vote_granted,
    );
    
    transport.send(from, response).await?;
    
    Ok(())
}

/// Handle a RequestVoteResponse message
pub async fn handle_request_vote_response<S: Storage>(
    config: &Config,
    state: &RaftState,
    storage: &S,
    from: NodeId,
    term: u64,
    vote_granted: bool,
) -> Result<bool> {
    let (current_term, should_update_term, is_candidate) = {
        let server = state.server.read().await;
        (server.current_term, term > server.current_term, server.state == crate::NodeState::Candidate && term == server.current_term)
    };
    
    // Ignore old responses
    if term < current_term {
        return Ok(false);
    }
    
    // Update term if necessary
    if should_update_term {
        {
            let mut server = state.server.write().await;
            server.current_term = term;
            server.voted_for = None;
            server.state = crate::NodeState::Follower;
            server.leader_id = None;
        }
        storage.save_term(term).await?;
        storage.save_vote(None).await?;
        return Ok(false);
    }
    
    // Only process if we're still a candidate
    if !is_candidate {
        return Ok(false);
    }
    
    // Update candidate state
    let won_election = {
        let mut candidate_guard = state.candidate.write().await;
        if let Some(candidate) = candidate_guard.as_mut() {
            candidate.responses_received.insert(from);
            
            if vote_granted {
                candidate.votes_received.insert(from);
                
                // Check if we won the election
                if candidate.has_majority(config.peers.len()) {
                    info!(
                        node_id = %config.node_id,
                        votes = candidate.votes_received.len(),
                        total = config.peers.len(),
                        "Won election"
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    };
    
    if won_election {
        // Become leader
        let last_index = storage.last_index().await?;
        state.become_leader(config.node_id, &config.peers, last_index).await;
        return Ok(true);
    }
    
    Ok(false)
}