//! Unit tests for Raft state management

#[cfg(test)]
mod tests {
    use crate::{NodeId, NodeState};
    use crate::raft::{Config, state::RaftState};
    use std::time::Duration;

    fn create_test_config(node_id: NodeId) -> Config {
        Config {
            node_id,
            cluster_members: vec![(node_id, "127.0.0.1:8080".parse().unwrap())],
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            batch_size: 10,
            snapshot_interval: 100,
        }
    }

    #[tokio::test]
    async fn test_initial_state() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        let server = state.server.read().await;
        assert_eq!(server.current_term, 0);
        assert_eq!(server.voted_for, None);
        assert_eq!(server.state, NodeState::Follower);
        assert_eq!(server.commit_index, 0);
        assert_eq!(server.last_applied, 0);
    }

    #[tokio::test]
    async fn test_become_candidate() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        state.become_candidate(node_id).await;
        
        let server = state.server.read().await;
        assert_eq!(server.state, NodeState::Candidate);
        assert_eq!(server.current_term, 1);
        assert_eq!(server.voted_for, Some(node_id));
        assert!(server.votes_received.contains(&node_id));
    }

    #[tokio::test]
    async fn test_become_leader() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        // First become candidate
        state.become_candidate(node_id).await;
        
        // Then become leader
        state.become_leader().await;
        
        let server = state.server.read().await;
        assert_eq!(server.state, NodeState::Leader);
        assert!(server.votes_received.is_empty());
    }

    #[tokio::test]
    async fn test_become_follower() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        // Set some state first
        state.become_candidate(node_id).await;
        
        // Become follower with higher term
        state.become_follower(5).await;
        
        let server = state.server.read().await;
        assert_eq!(server.state, NodeState::Follower);
        assert_eq!(server.current_term, 5);
        assert_eq!(server.voted_for, None);
        assert!(server.votes_received.is_empty());
    }

    #[tokio::test]
    async fn test_update_commit_index() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        state.update_commit_index(10).await;
        
        let server = state.server.read().await;
        assert_eq!(server.commit_index, 10);
    }

    #[tokio::test]
    async fn test_update_last_applied() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        state.update_last_applied(5).await;
        
        let server = state.server.read().await;
        assert_eq!(server.last_applied, 5);
    }

    #[tokio::test]
    async fn test_record_vote() {
        let node_id = NodeId::new();
        let other_node = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        // Become candidate first
        state.become_candidate(node_id).await;
        
        // Record vote from another node
        state.record_vote(other_node).await;
        
        let server = state.server.read().await;
        assert!(server.votes_received.contains(&node_id));
        assert!(server.votes_received.contains(&other_node));
        assert_eq!(server.votes_received.len(), 2);
    }

    #[tokio::test]
    async fn test_reset_election_timeout() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        // This should not panic
        state.reset_election_timeout().await;
    }

    #[tokio::test]
    async fn test_concurrent_state_updates() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        // Spawn multiple concurrent updates
        let state1 = state.clone();
        let state2 = state.clone();
        let state3 = state.clone();
        
        let handle1 = tokio::spawn(async move {
            for i in 0..100 {
                state1.update_commit_index(i).await;
            }
        });
        
        let handle2 = tokio::spawn(async move {
            for i in 0..100 {
                state2.update_last_applied(i).await;
            }
        });
        
        let handle3 = tokio::spawn(async move {
            for _ in 0..50 {
                state3.reset_election_timeout().await;
            }
        });
        
        // All updates should complete without deadlock
        handle1.await.unwrap();
        handle2.await.unwrap();
        handle3.await.unwrap();
        
        // Final state should be consistent
        let server = state.server.read().await;
        assert!(server.commit_index >= server.last_applied);
    }

    #[tokio::test]
    async fn test_state_transitions() {
        let node_id = NodeId::new();
        let config = create_test_config(node_id);
        let state = RaftState::new(config);
        
        // Test all valid state transitions
        
        // Follower -> Candidate
        state.become_candidate(node_id).await;
        {
            let server = state.server.read().await;
            assert_eq!(server.state, NodeState::Candidate);
        }
        
        // Candidate -> Leader
        state.become_leader().await;
        {
            let server = state.server.read().await;
            assert_eq!(server.state, NodeState::Leader);
        }
        
        // Leader -> Follower (higher term discovered)
        state.become_follower(10).await;
        {
            let server = state.server.read().await;
            assert_eq!(server.state, NodeState::Follower);
            assert_eq!(server.current_term, 10);
        }
    }
}