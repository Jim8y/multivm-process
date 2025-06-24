//! Multi-node integration test for P2P networking
//!
//! This test simulates a real P2P network with multiple nodes communicating with each other.

use libp2p::Multiaddr;
use multivm_p2p::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

/// Test node wrapper that includes the P2P network and message handling
struct TestNode {
    id: String,
    network: P2PNetwork,
    received_messages: Arc<RwLock<Vec<NetworkMessage>>>,
    message_receiver: mpsc::Receiver<NetworkMessage>,
}

impl TestNode {
    async fn new(port: u16) -> Self {
        let config = network::NetworkConfig {
            listen_addresses: vec![format!("/ip4/127.0.0.1/tcp/{}", port).parse().unwrap()],
            bootstrap_peers: Vec::new(),
            max_peers: 10,
            enable_mdns: true,
            validation_mode: libp2p::gossipsub::ValidationMode::Permissive,
            connection_timeout: Duration::from_secs(10),
        };

        let mut network = P2PNetwork::new(config).await.unwrap();
        let (message_sender, message_receiver) = mpsc::channel(100);

        // Set up event handler that captures messages
        let received_messages = Arc::new(RwLock::new(Vec::new()));
        let received_messages_clone = received_messages.clone();

        // Create a custom event handler
        struct TestEventHandler {
            message_sender: mpsc::Sender<NetworkMessage>,
            received_messages: Arc<RwLock<Vec<NetworkMessage>>>,
        }

        #[async_trait::async_trait]
        impl NetworkEventHandler for TestEventHandler {
            async fn handle_event(
                &mut self,
                event: NetworkEvent,
            ) -> multivm_common::MultivmResult<()> {
                match event {
                    NetworkEvent::MessageReceived { message, .. } => {
                        self.received_messages.write().await.push(*message.clone());
                        let _ = self.message_sender.send(*message).await;
                    }
                    _ => {}
                }
                Ok(())
            }

            async fn on_peer_connected(
                &self,
                peer_info: &PeerInfo,
            ) -> multivm_common::MultivmResult<()> {
                info!("Peer connected: {}", peer_info.peer_id);
                Ok(())
            }

            async fn on_peer_disconnected(
                &self,
                peer_id: &str,
            ) -> multivm_common::MultivmResult<()> {
                info!("Peer disconnected: {}", peer_id);
                Ok(())
            }
        }

        let handler = Arc::new(TestEventHandler {
            message_sender,
            received_messages: received_messages_clone.clone(),
        });

        network.set_event_handler(handler);

        Self {
            id: network.local_peer_id().to_string(),
            network,
            received_messages: received_messages_clone,
            message_receiver,
        }
    }

    async fn start(&mut self) -> multivm_common::MultivmResult<()> {
        self.network.start().await
    }

    async fn stop(&mut self) -> multivm_common::MultivmResult<()> {
        self.network.stop().await
    }

    async fn connect_to_peer(&self, peer_addr: Multiaddr) -> multivm_common::MultivmResult<()> {
        if let Some(peer_id) = network::extract_peer_id(&peer_addr) {
            self.network.add_peer(peer_id, vec![peer_addr]).await
        } else {
            Err(multivm_common::MultivmError::Network(
                "Invalid peer address".to_string(),
            ))
        }
    }

    async fn subscribe_to_topic(&self, topic: &str) -> multivm_common::MultivmResult<()> {
        self.network.subscribe_topic(topic).await
    }

    async fn broadcast_message(
        &mut self,
        message: NetworkMessage,
    ) -> multivm_common::MultivmResult<()> {
        self.network.broadcast(message).await
    }

    async fn send_to_peer(
        &mut self,
        peer_id: String,
        message: NetworkMessage,
    ) -> multivm_common::MultivmResult<()> {
        self.network.send_to_peer(peer_id, message).await
    }

    async fn get_received_messages(&self) -> Vec<NetworkMessage> {
        self.received_messages.read().await.clone()
    }

    async fn wait_for_messages(&mut self, count: usize, timeout: Duration) -> Vec<NetworkMessage> {
        let mut messages = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;

        while messages.len() < count && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(100), self.message_receiver.recv())
                .await
            {
                Ok(Some(msg)) => messages.push(msg),
                _ => continue,
            }
        }

        messages
    }

    fn listening_address(&self) -> Multiaddr {
        // Return the first listening address for simplicity
        format!("/ip4/127.0.0.1/tcp/0/p2p/{}", self.id)
            .parse()
            .unwrap()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_two_node_communication() {
    // Initialize logging for debugging
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting two-node communication test");

    // Create two nodes
    let mut node1 = TestNode::new(0).await;
    let mut node2 = TestNode::new(0).await;

    // Start both nodes
    node1.start().await.unwrap();
    node2.start().await.unwrap();

    // Subscribe to test topic
    node1.subscribe_to_topic("test-topic").await.unwrap();
    node2.subscribe_to_topic("test-topic").await.unwrap();

    // Connect node2 to node1
    let node1_addr = node1.listening_address();
    node2.connect_to_peer(node1_addr).await.unwrap();

    // Wait for connection to establish
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send message from node1
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(100),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    node1.broadcast_message(test_message.clone()).await.unwrap();

    // Wait for message to be received
    let received = node2.wait_for_messages(1, Duration::from_secs(5)).await;

    assert_eq!(received.len(), 1, "Node2 should receive 1 message");
    assert_eq!(received[0].id, test_message.id, "Message IDs should match");

    // Clean up
    node1.stop().await.unwrap();
    node2.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_multi_node_broadcast() {
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting multi-node broadcast test");

    // Create 4 nodes
    let mut nodes = Vec::new();
    for i in 0..4 {
        let mut node = TestNode::new(0).await;
        node.start().await.unwrap();
        node.subscribe_to_topic("broadcast-test").await.unwrap();
        nodes.push(node);
    }

    // Connect nodes in a chain: 0 -> 1 -> 2 -> 3
    for i in 1..nodes.len() {
        let prev_addr = nodes[i - 1].listening_address();
        nodes[i].connect_to_peer(prev_addr).await.unwrap();
    }

    // Wait for connections
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Broadcast from node 0
    let broadcast_msg = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0xtest_root".to_string(),
            vm_type: VmType::Svm,
            height: 12345,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    nodes[0]
        .broadcast_message(broadcast_msg.clone())
        .await
        .unwrap();

    // Wait for propagation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check all other nodes received the message
    for i in 1..nodes.len() {
        let messages = nodes[i].get_received_messages().await;
        assert!(
            messages.iter().any(|m| m.id == broadcast_msg.id),
            "Node {} should have received the broadcast message",
            i
        );
    }

    // Clean up
    for mut node in nodes {
        node.stop().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_peer_discovery_mdns() {
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting mDNS peer discovery test");

    // Create nodes with mDNS enabled
    let mut node1 = TestNode::new(0).await;
    let mut node2 = TestNode::new(0).await;

    node1.start().await.unwrap();
    node2.start().await.unwrap();

    // Subscribe to discovery topic
    node1.subscribe_to_topic("discovery-test").await.unwrap();
    node2.subscribe_to_topic("discovery-test").await.unwrap();

    // Wait for mDNS discovery (should happen automatically)
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Send test message
    let discovery_msg = NetworkMessage::new(
        MessagePayload::Discovery(DiscoveryMessage::Announce {
            capabilities: NodeCapabilities {
                supported_vms: vec![VmType::Svm, VmType::Evm],
                protocol_versions: vec![1],
                features: vec!["multivm".to_string()],
                limits: ResourceLimits::default(),
            },
            addresses: vec!["/ip4/127.0.0.1/tcp/8000".to_string()],
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    node1.broadcast_message(discovery_msg).await.unwrap();

    // Both nodes should have discovered each other via mDNS
    let node1_peers = node1.network.get_connected_peers().await;
    let node2_peers = node2.network.get_connected_peers().await;

    info!(
        "Node1 peers: {}, Node2 peers: {}",
        node1_peers.len(),
        node2_peers.len()
    );

    // Clean up
    node1.stop().await.unwrap();
    node2.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_direct_peer_messaging() {
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting direct peer messaging test");

    let mut node1 = TestNode::new(0).await;
    let mut node2 = TestNode::new(0).await;

    node1.start().await.unwrap();
    node2.start().await.unwrap();

    // Connect nodes
    let node1_addr = node1.listening_address();
    node2.connect_to_peer(node1_addr).await.unwrap();

    // Subscribe to direct message topics
    let node1_topic = format!("peer-{}", node1.id);
    let node2_topic = format!("peer-{}", node2.id);

    node1.subscribe_to_topic(&node1_topic).await.unwrap();
    node2.subscribe_to_topic(&node2_topic).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send direct message from node2 to node1
    let direct_msg = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::Peer(node2.id.clone()),
        MessageTarget::Peer(node1.id.clone()),
    );

    node2
        .send_to_peer(node1.id.clone(), direct_msg.clone())
        .await
        .unwrap();

    // Wait for message
    let received = node1.wait_for_messages(1, Duration::from_secs(5)).await;

    assert!(!received.is_empty(), "Node1 should receive direct message");
    if !received.is_empty() {
        assert_eq!(received[0].id, direct_msg.id, "Message IDs should match");
    }

    // Clean up
    node1.stop().await.unwrap();
    node2.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_network_resilience() {
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting network resilience test");

    // Create 3 nodes
    let mut nodes = Vec::new();
    for _ in 0..3 {
        let mut node = TestNode::new(0).await;
        node.start().await.unwrap();
        node.subscribe_to_topic("resilience-test").await.unwrap();
        nodes.push(node);
    }

    // Connect in a triangle: 0 <-> 1 <-> 2 <-> 0
    let addrs: Vec<_> = nodes.iter().map(|n| n.listening_address()).collect();

    nodes[1].connect_to_peer(addrs[0].clone()).await.unwrap();
    nodes[2].connect_to_peer(addrs[1].clone()).await.unwrap();
    nodes[0].connect_to_peer(addrs[2].clone()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Send message from node 0
    let test_msg = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(200),
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    nodes[0].broadcast_message(test_msg.clone()).await.unwrap();

    // Wait for propagation
    tokio::time::sleep(Duration::from_secs(1)).await;

    // All nodes should receive the message
    for i in 1..3 {
        let messages = nodes[i].get_received_messages().await;
        assert!(
            messages.iter().any(|m| m.id == test_msg.id),
            "Node {} should have received the message",
            i
        );
    }

    // Simulate node 1 failure by stopping it
    nodes[1].stop().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send another message from node 0
    let test_msg2 = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    nodes[0].broadcast_message(test_msg2.clone()).await.unwrap();

    // Node 2 should still receive it via the direct connection
    tokio::time::sleep(Duration::from_secs(1)).await;

    let node2_messages = nodes[2].get_received_messages().await;
    assert!(
        node2_messages.iter().any(|m| m.id == test_msg2.id),
        "Node 2 should still receive messages after node 1 failure"
    );

    // Clean up
    nodes[0].stop().await.unwrap();
    nodes[2].stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_cross_vm_message_routing() {
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting cross-VM message routing test");

    use multivm_account_mapping::{AccountAddress, EthereumAddress, SolanaAddress};

    // Create nodes representing different VMs
    let mut svm_node = TestNode::new(0).await;
    let mut evm_node = TestNode::new(0).await;
    let mut coordinator_node = TestNode::new(0).await;

    // Start all nodes
    svm_node.start().await.unwrap();
    evm_node.start().await.unwrap();
    coordinator_node.start().await.unwrap();

    // Subscribe to VM-specific topics
    svm_node.subscribe_to_topic("svm-messages").await.unwrap();
    evm_node.subscribe_to_topic("evm-messages").await.unwrap();
    coordinator_node
        .subscribe_to_topic("multivm-messages")
        .await
        .unwrap();

    // Connect nodes
    let coord_addr = coordinator_node.listening_address();
    svm_node.connect_to_peer(coord_addr.clone()).await.unwrap();
    evm_node.connect_to_peer(coord_addr).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Send cross-VM account binding message
    let binding_msg = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::AccountBinding {
            source: AccountAddress::Solana(SolanaAddress([1u8; 32])),
            target: AccountAddress::Ethereum(EthereumAddress([2u8; 20])),
            proof_hash: "0xproof123".to_string(),
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    coordinator_node
        .broadcast_message(binding_msg.clone())
        .await
        .unwrap();

    // Wait for message propagation
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Both VM nodes should receive the binding message
    let svm_messages = svm_node.get_received_messages().await;
    let evm_messages = evm_node.get_received_messages().await;

    assert!(
        svm_messages.iter().any(|m| m.id == binding_msg.id),
        "SVM node should receive account binding message"
    );
    assert!(
        evm_messages.iter().any(|m| m.id == binding_msg.id),
        "EVM node should receive account binding message"
    );

    // Clean up
    svm_node.stop().await.unwrap();
    evm_node.stop().await.unwrap();
    coordinator_node.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_network_performance() {
    let _ = tracing_subscriber::fmt().try_init();

    info!("Starting network performance test");

    let mut node1 = TestNode::new(0).await;
    let mut node2 = TestNode::new(0).await;

    node1.start().await.unwrap();
    node2.start().await.unwrap();

    node1.subscribe_to_topic("perf-test").await.unwrap();
    node2.subscribe_to_topic("perf-test").await.unwrap();

    let node1_addr = node1.listening_address();
    node2.connect_to_peer(node1_addr).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send multiple messages rapidly
    let start = tokio::time::Instant::now();
    let message_count = 100u64;

    for i in 0..message_count {
        let msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(i),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        node1.broadcast_message(msg).await.unwrap();
    }

    // Wait for all messages
    let received = node2
        .wait_for_messages(message_count as usize, Duration::from_secs(10))
        .await;
    let elapsed = start.elapsed();

    info!(
        "Sent {} messages in {:?}, received {}",
        message_count,
        elapsed,
        received.len()
    );

    // Should receive most messages (allow for some loss in test environment)
    assert!(
        received.len() >= (message_count * 90 / 100) as usize,
        "Should receive at least 90% of messages"
    );

    // Check throughput
    let messages_per_second = message_count as f64 / elapsed.as_secs_f64();
    info!("Throughput: {:.2} messages/second", messages_per_second);

    assert!(
        messages_per_second > 10.0,
        "Should achieve at least 10 messages/second"
    );

    // Clean up
    node1.stop().await.unwrap();
    node2.stop().await.unwrap();
}

// Helper function to extract peer ID from multiaddr
fn extract_peer_id(addr: &Multiaddr) -> Option<libp2p::PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}
