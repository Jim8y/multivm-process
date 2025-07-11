//! Simple cluster example showing basic usage of the consensus crate

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

/// Simple key-value state machine
#[derive(Debug)]
struct KeyValueStore {
    data: HashMap<String, String>,
}

impl KeyValueStore {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

#[async_trait]
impl StateMachine for KeyValueStore {
    type Command = Command;
    type Response = Response;
    
    async fn apply(&mut self, command: Self::Command) -> Self::Response {
        let cmd_str = String::from_utf8_lossy(&command.data);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        
        match parts.as_slice() {
            ["PUT", key, value] => {
                self.data.insert(key.to_string(), value.to_string());
                info!("PUT {} = {}", key, value);
                Response {
                    success: true,
                    data: b"OK".to_vec(),
                }
            }
            ["GET", key] => {
                if let Some(value) = self.data.get(*key) {
                    info!("GET {} = {}", key, value);
                    Response {
                        success: true,
                        data: value.as_bytes().to_vec(),
                    }
                } else {
                    info!("GET {} = NOT_FOUND", key);
                    Response {
                        success: false,
                        data: b"NOT_FOUND".to_vec(),
                    }
                }
            }
            ["DELETE", key] => {
                if self.data.remove(*key).is_some() {
                    info!("DELETE {} = OK", key);
                    Response {
                        success: true,
                        data: b"OK".to_vec(),
                    }
                } else {
                    info!("DELETE {} = NOT_FOUND", key);
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
    
    info!("Starting simple cluster example");
    
    // Create a 3-node cluster
    let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
    
    // Create transport channels
    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    
    for i in 0..3 {
        let (tx, rx) = mpsc::channel(100);
        senders.push(tx);
        receivers.push(rx);
    }
    
    // Create nodes
    let mut nodes = Vec::new();
    for i in 0..3 {
        let config = Config::new(node_ids[i], node_ids.clone());
        let storage = MemoryStorage::new();
        let transport = MemoryTransport::new(node_ids[i], receivers.remove(0));
        let state_machine = KeyValueStore::new();
        
        // Register peers
        for j in 0..3 {
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
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    // Find the leader
    let leader = nodes.iter().find(|n| {
        futures::executor::block_on(n.is_leader())
    }).expect("No leader found");
    
    info!("Leader elected: {}", leader.get_leader().await.unwrap());
    
    // Demonstrate basic operations
    info!("Demonstrating basic operations...");
    
    // PUT operations
    let commands = vec![
        "PUT name Alice",
        "PUT age 30",
        "PUT city NewYork",
        "PUT occupation Engineer",
    ];
    
    for cmd in commands {
        let command = Command {
            data: cmd.as_bytes().to_vec(),
        };
        
        match leader.propose(command).await {
            Ok(response) => {
                info!("Command '{}' -> {:?}", cmd, String::from_utf8_lossy(&response.data));
            }
            Err(e) => {
                info!("Command '{}' failed: {}", cmd, e);
            }
        }
        
        // Small delay between commands
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // GET operations
    let queries = vec![
        "GET name",
        "GET age",
        "GET city",
        "GET occupation",
        "GET nonexistent",
    ];
    
    for query in queries {
        let command = Command {
            data: query.as_bytes().to_vec(),
        };
        
        match leader.propose(command).await {
            Ok(response) => {
                info!("Query '{}' -> {:?}", query, String::from_utf8_lossy(&response.data));
            }
            Err(e) => {
                info!("Query '{}' failed: {}", query, e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // DELETE operations
    let deletes = vec![
        "DELETE age",
        "DELETE nonexistent",
    ];
    
    for delete in deletes {
        let command = Command {
            data: delete.as_bytes().to_vec(),
        };
        
        match leader.propose(command).await {
            Ok(response) => {
                info!("Delete '{}' -> {:?}", delete, String::from_utf8_lossy(&response.data));
            }
            Err(e) => {
                info!("Delete '{}' failed: {}", delete, e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Verify deletion
    let command = Command {
        data: b"GET age".to_vec(),
    };
    
    match leader.propose(command).await {
        Ok(response) => {
            info!("Verification 'GET age' -> {:?}", String::from_utf8_lossy(&response.data));
        }
        Err(e) => {
            info!("Verification failed: {}", e);
        }
    }
    
    info!("Example completed successfully!");
    
    // Keep the cluster running for a bit
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    Ok(())
}