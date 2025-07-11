//! Seven-node testnet example demonstrating consensus at scale

use consensus::{
    Config, ConsensusAlgorithm, NodeId, RaftNode, StateMachine,
    storage::MemoryStorage,
    transport::MemoryTransport,
    raft::{Command, Response},
};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber;

/// Simple key-value state machine with node tracking
#[derive(Debug)]
struct TestnetStateMachine {
    node_id: NodeId,
    data: HashMap<String, String>,
    operation_count: u64,
}

impl TestnetStateMachine {
    fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            data: HashMap::new(),
            operation_count: 0,
        }
    }
}

#[async_trait]
impl StateMachine for TestnetStateMachine {
    type Command = Command;
    type Response = Response;
    
    async fn apply(&mut self, command: Self::Command) -> Self::Response {
        self.operation_count += 1;
        
        let cmd_str = String::from_utf8_lossy(&command.data);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        
        match parts.as_slice() {
            ["SET", key, value] => {
                self.data.insert(key.to_string(), value.to_string());
                info!(
                    node_id = %self.node_id,
                    operation = self.operation_count,
                    "SET {} = {}",
                    key,
                    value
                );
                Response {
                    success: true,
                    data: b"OK".to_vec(),
                }
            }
            ["GET", key] => {
                if let Some(value) = self.data.get(*key) {
                    info!(
                        node_id = %self.node_id,
                        operation = self.operation_count,
                        "GET {} = {}",
                        key,
                        value
                    );
                    Response {
                        success: true,
                        data: value.as_bytes().to_vec(),
                    }
                } else {
                    info!(
                        node_id = %self.node_id,
                        operation = self.operation_count,
                        "GET {} = NOT_FOUND",
                        key
                    );
                    Response {
                        success: false,
                        data: b"NOT_FOUND".to_vec(),
                    }
                }
            }
            ["DELETE", key] => {
                if self.data.remove(*key).is_some() {
                    info!(
                        node_id = %self.node_id,
                        operation = self.operation_count,
                        "DELETE {} = OK",
                        key
                    );
                    Response {
                        success: true,
                        data: b"OK".to_vec(),
                    }
                } else {
                    info!(
                        node_id = %self.node_id,
                        operation = self.operation_count,
                        "DELETE {} = NOT_FOUND",
                        key
                    );
                    Response {
                        success: false,
                        data: b"NOT_FOUND".to_vec(),
                    }
                }
            }
            ["STATS"] => {
                let stats = format!("ops:{},keys:{}", self.operation_count, self.data.len());
                info!(
                    node_id = %self.node_id,
                    operation = self.operation_count,
                    "STATS = {}",
                    stats
                );
                Response {
                    success: true,
                    data: stats.as_bytes().to_vec(),
                }
            }
            _ => Response {
                success: false,
                data: b"INVALID_COMMAND".to_vec(),
            }
        }
    }
    
    async fn snapshot(&self) -> consensus::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.data)?)
    }
    
    async fn restore(&mut self, snapshot: &[u8]) -> consensus::Result<()> {
        self.data = serde_json::from_slice(snapshot)?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    info!("Starting 7-node testnet");
    
    // Create a 7-node cluster
    let node_ids: Vec<NodeId> = (0..7).map(|_| NodeId::new()).collect();
    
    info!("Node IDs:");
    for (i, node_id) in node_ids.iter().enumerate() {
        info!("  Node {}: {}", i, node_id);
    }
    
    // Create transport channels
    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    
    for i in 0..7 {
        let (tx, rx) = mpsc::channel(100);
        senders.push(tx);
        receivers.push(rx);
    }
    
    // Create nodes
    let mut nodes = Vec::new();
    for i in 0..7 {
        let config = Config::new(node_ids[i], node_ids.clone());
        let storage = MemoryStorage::new();
        let transport = MemoryTransport::new(node_ids[i], receivers.remove(0));
        let state_machine = TestnetStateMachine::new(node_ids[i]);
        
        // Register peers
        for j in 0..7 {
            if i != j {
                transport.register_peer(node_ids[j], senders[j].clone()).await;
            }
        }
        
        let node = RaftNode::new(config, storage, transport, state_machine).await?;
        nodes.push(node);
        
        info!("Created node {}: {}", i, node_ids[i]);
    }
    
    // Wait for leader election
    info!("Waiting for leader election...");
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    
    // Check cluster state
    let mut leaders = 0;
    let mut followers = 0;
    let mut candidates = 0;
    
    for (i, node) in nodes.iter().enumerate() {
        let state = node.get_state().await;
        match state {
            consensus::NodeState::Leader => {
                leaders += 1;
                info!("Node {} is LEADER", i);
            }
            consensus::NodeState::Follower => {
                followers += 1;
                info!("Node {} is FOLLOWER", i);
            }
            consensus::NodeState::Candidate => {
                candidates += 1;
                info!("Node {} is CANDIDATE", i);
            }
            consensus::NodeState::Shutdown => {
                info!("Node {} is SHUTDOWN", i);
            }
        }
    }
    
    info!(
        "Cluster state: {} leaders, {} followers, {} candidates",
        leaders, followers, candidates
    );
    
    if leaders != 1 {
        info!("WARNING: Expected exactly 1 leader, found {}", leaders);
        // Wait a bit more for election to stabilize
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        
        // Check again
        let leader = nodes.iter().find(|n| {
            futures::executor::block_on(n.is_leader())
        });
        
        if let Some(leader) = leader {
            info!("Found leader: {}", leader.get_leader().await.unwrap());
        } else {
            return Err("No leader elected in 7-node cluster".into());
        }
    }
    
    let leader = nodes.iter().find(|n| {
        futures::executor::block_on(n.is_leader())
    }).expect("No leader found");
    
    info!("Leader elected: {}", leader.get_leader().await.unwrap());
    
    // Demonstrate testnet operations
    info!("Running testnet operations...");
    
    // Phase 1: Basic operations
    info!("=== PHASE 1: Basic Operations ===");
    let basic_commands = vec![
        "SET node_count 7",
        "SET cluster_type testnet",
        "SET consensus_algorithm raft",
        "SET started_at 2024-01-01",
    ];
    
    for cmd in basic_commands {
        let command = Command {
            data: cmd.as_bytes().to_vec(),
        };
        
        match leader.propose(command).await {
            Ok(response) => {
                info!("✓ Command '{}' -> {:?}", cmd, String::from_utf8_lossy(&response.data));
            }
            Err(e) => {
                info!("✗ Command '{}' failed: {}", cmd, e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
    
    // Phase 2: Concurrent operations
    info!("=== PHASE 2: Concurrent Operations ===");
    
    // Do sequential operations instead of concurrent due to lifetime issues
    for i in 0..7 {
        for j in 0..5 {
            let cmd = format!("SET node_{}_op_{} value_{}", i, j, i * 100 + j);
            let command = Command {
                data: cmd.as_bytes().to_vec(),
            };
            
            match leader.propose(command).await {
                Ok(_) => info!("Operation succeeded: {}", cmd),
                Err(e) => info!("Operation failed: {} - {}", cmd, e),
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }
    }
    
    // Phase 3: Read operations
    info!("=== PHASE 3: Read Operations ===");
    let read_commands = vec![
        "GET node_count",
        "GET cluster_type",
        "GET consensus_algorithm",
        "GET node_0_op_0",
        "GET node_3_op_2",
        "GET node_6_op_4",
        "STATS",
    ];
    
    for cmd in read_commands {
        let command = Command {
            data: cmd.as_bytes().to_vec(),
        };
        
        match leader.propose(command).await {
            Ok(response) => {
                info!("📖 Query '{}' -> {:?}", cmd, String::from_utf8_lossy(&response.data));
            }
            Err(e) => {
                info!("✗ Query '{}' failed: {}", cmd, e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Phase 4: Stress test
    info!("=== PHASE 4: Stress Test ===");
    let start_time = std::time::Instant::now();
    let mut successful_ops = 0;
    let mut failed_ops = 0;
    
    for i in 0..50 {
        let cmd = format!("SET stress_test_{} iteration_{}", i % 10, i);
        let command = Command {
            data: cmd.as_bytes().to_vec(),
        };
        
        match leader.propose(command).await {
            Ok(_) => successful_ops += 1,
            Err(_) => failed_ops += 1,
        }
        
        // No delay for stress test
    }
    
    let duration = start_time.elapsed();
    info!(
        "Stress test completed: {} successful, {} failed in {:?} ({:.2} ops/sec)",
        successful_ops,
        failed_ops,
        duration,
        successful_ops as f64 / duration.as_secs_f64()
    );
    
    // Final stats
    info!("=== FINAL CLUSTER STATE ===");
    for (i, node) in nodes.iter().enumerate() {
        let state = node.get_state().await;
        let leader_id = node.get_leader().await;
        info!(
            "Node {}: {:?}, Leader: {:?}",
            i, state, leader_id
        );
    }
    
    // Get final stats from leader
    let stats_command = Command {
        data: b"STATS".to_vec(),
    };
    
    if let Ok(response) = leader.propose(stats_command).await {
        info!("Final stats: {:?}", String::from_utf8_lossy(&response.data));
    }
    
    info!("7-node testnet completed successfully!");
    
    // Keep the cluster running for observation
    info!("Keeping cluster running for 10 seconds for observation...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    
    Ok(())
}