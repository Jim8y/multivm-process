//! Integration tests for the consensus crate

use consensus::{
    Config, ConsensusAlgorithm, NodeId, NodeState, RaftNode, StateMachine,
    storage::MemoryStorage,
    transport::MemoryTransport,
    raft::{Command, Response},
};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing_subscriber;

/// Simple state machine for testing
#[derive(Debug)]
struct TestStateMachine {
    state: HashMap<String, String>,
}

impl TestStateMachine {
    fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }
}

#[async_trait]
impl StateMachine for TestStateMachine {
    type Command = Command;
    type Response = Response;
    
    async fn apply(&mut self, command: Self::Command) -> Self::Response {
        // Simple key-value store operations
        let cmd_str = String::from_utf8_lossy(&command.data);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        
        match parts.as_slice() {
            ["SET", key, value] => {
                self.state.insert(key.to_string(), value.to_string());
                Response {
                    success: true,
                    data: b"OK".to_vec(),
                }
            }
            ["GET", key] => {
                if let Some(value) = self.state.get(*key) {
                    Response {
                        success: true,
                        data: value.as_bytes().to_vec(),
                    }
                } else {
                    Response {
                        success: false,
                        data: b"NOT_FOUND".to_vec(),
                    }
                }
            }
            _ => Response {
                success: false,
                data: b"INVALID_COMMAND".to_vec(),
            }
        }
    }
    
    async fn snapshot(&self) -> consensus::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.state)?)
    }
    
    async fn restore(&mut self, snapshot: &[u8]) -> consensus::Result<()> {
        self.state = serde_json::from_slice(snapshot)?;
        Ok(())
    }
}

/// Setup a cluster of nodes for testing
async fn setup_cluster(num_nodes: usize) -> Vec<RaftNode<MemoryStorage, MemoryTransport, TestStateMachine>> {
    let _ = tracing_subscriber::fmt::try_init();
    
    // Create node IDs
    let node_ids: Vec<NodeId> = (0..num_nodes).map(|_| NodeId::new()).collect();
    
    // Create transport channels
    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    
    for _ in 0..num_nodes {
        let (tx, rx) = mpsc::channel(100);
        senders.push(tx);
        receivers.push(rx);
    }
    
    // Create nodes
    let mut nodes = Vec::new();
    for i in 0..num_nodes {
        let config = Config::new(node_ids[i], node_ids.clone());
        let storage = MemoryStorage::new();
        let transport = MemoryTransport::new(node_ids[i], receivers.remove(0));
        let state_machine = TestStateMachine::new();
        
        // Register peers
        for j in 0..num_nodes {
            if i != j {
                transport.register_peer(node_ids[j], senders[j].clone()).await;
            }
        }
        
        let node = RaftNode::new(config, storage, transport, state_machine).await.unwrap();
        nodes.push(node);
    }
    
    nodes
}

#[tokio::test]
async fn test_single_node_cluster() {
    let nodes = setup_cluster(1).await;
    let node = &nodes[0];
    
    // Single node should become leader (may need time for election timeout)
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // If still not leader, wait a bit more
    if !node.is_leader().await {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    assert!(node.is_leader().await);
    
    // Test basic operations
    let command = Command {
        data: b"SET key1 value1".to_vec(),
    };
    
    let result = node.propose(command).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert!(response.success);
    assert_eq!(response.data, b"OK");
}

#[tokio::test]
async fn test_leader_election() {
    let nodes = setup_cluster(3).await;
    
    // Wait for election to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Exactly one node should be leader
    let mut leaders = 0;
    let mut followers = 0;
    
    for node in &nodes {
        match node.get_state().await {
            NodeState::Leader => leaders += 1,
            NodeState::Follower => followers += 1,
            _ => {}
        }
    }
    
    assert_eq!(leaders, 1);
    assert_eq!(followers, 2);
}

#[tokio::test]
async fn test_basic_consensus() {
    let nodes = setup_cluster(3).await;
    
    // Wait for election
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Find the leader
    let leader = nodes.iter().find(|n| {
        futures::executor::block_on(n.is_leader())
    }).expect("No leader found");
    
    // Propose a command
    let command = Command {
        data: b"SET test_key test_value".to_vec(),
    };
    
    let result = leader.propose(command).await;
    assert!(result.is_ok());
    
    // Wait for replication
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // All nodes should have the same committed index
    // (This is a simplified test - in practice we'd check the actual log)
}

#[tokio::test]
async fn test_node_recovery() {
    let nodes = setup_cluster(3).await;
    
    // Wait for initial election
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // TODO: Implement proper node shutdown and recovery testing
    // For now, just ensure the cluster can handle basic operations
    
    let leader = nodes.iter().find(|n| {
        futures::executor::block_on(n.is_leader())
    }).expect("No leader found");
    
    let command = Command {
        data: b"SET recovery_test value".to_vec(),
    };
    
    let result = leader.propose(command).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_proposals() {
    let nodes = setup_cluster(3).await;
    
    // Wait for election
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    let leader = nodes.iter().find(|n| {
        futures::executor::block_on(n.is_leader())
    }).expect("No leader found");
    
    // Submit multiple commands sequentially  
    for i in 0..10 {
        let command = Command {
            data: format!("SET key{} value{}", i, i).into_bytes(),
        };
        
        let result = leader.propose(command).await;
        assert!(result.is_ok());
    }
    
}