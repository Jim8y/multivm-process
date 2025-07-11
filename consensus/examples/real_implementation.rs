//! Example of using consensus with real network and storage implementations

use consensus::{Config, RaftNode, StateMachine};
use multivm_network::tcp::{TcpTransport, TcpTransportConfig};
use multivm_storage::rocksdb::RocksDbStorage;
use multivm_storage::StorageConfig;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Example state machine that stores key-value pairs
#[derive(Debug)]
struct KvStateMachine {
    data: HashMap<String, String>,
}

#[async_trait::async_trait]
impl StateMachine for KvStateMachine {
    type Command = (String, String); // (key, value)
    type Response = Option<String>;  // Previous value
    
    async fn apply(&mut self, command: Self::Command) -> Self::Response {
        self.data.insert(command.0, command.1)
    }
    
    async fn snapshot(&self) -> consensus::Result<Vec<u8>> {
        Ok(bincode::serialize(&self.data).unwrap())
    }
    
    async fn restore(&mut self, snapshot: &[u8]) -> consensus::Result<()> {
        self.data = bincode::deserialize(snapshot).unwrap();
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create node configuration
    let node_id = consensus::NodeId::new();
    let listen_addr: SocketAddr = "127.0.0.1:7000".parse()?;
    
    // Configure storage
    let storage_config = StorageConfig {
        data_dir: PathBuf::from(format!("./data/node_{}", node_id)),
        ..Default::default()
    };
    let storage = RocksDbStorage::new(storage_config).await?;
    
    // Configure network transport
    let transport_config = TcpTransportConfig {
        listen_addr,
        ..Default::default()
    };
    
    // Define cluster peers
    let mut peers = HashMap::new();
    peers.insert(
        consensus::NodeId::from_bytes([1; 16]),
        "127.0.0.1:7001".parse()?,
    );
    peers.insert(
        consensus::NodeId::from_bytes([2; 16]),
        "127.0.0.1:7002".parse()?,
    );
    
    let transport = TcpTransport::new(node_id, transport_config, peers).await?;
    
    // Create state machine
    let state_machine = KvStateMachine {
        data: HashMap::new(),
    };
    
    // Create Raft configuration
    let config = Config {
        election_timeout_min: 150,
        election_timeout_max: 300,
        heartbeat_interval: 50,
        max_batch_size: 100,
        snapshot_threshold: 10000,
    };
    
    // Create and start Raft node
    let raft_node = RaftNode::new(
        node_id,
        config,
        storage,
        transport,
        state_machine,
    )?;
    
    raft_node.start().await?;
    
    // Example: Propose a command if we're the leader
    if raft_node.is_leader().await {
        let command = ("key1".to_string(), "value1".to_string());
        match raft_node.propose(command).await {
            Ok(response) => println!("Command applied, previous value: {:?}", response),
            Err(e) => eprintln!("Failed to propose command: {}", e),
        }
    }
    
    // Keep the node running
    tokio::signal::ctrl_c().await?;
    
    // Shutdown gracefully
    raft_node.shutdown().await?;
    
    Ok(())
}