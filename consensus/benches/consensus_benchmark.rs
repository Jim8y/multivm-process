//! Benchmarks for the consensus crate

use consensus::{
    Config, ConsensusAlgorithm, NodeId, RaftNode, StateMachine,
    storage::MemoryStorage,
    transport::MemoryTransport,
    raft::{Command, Response},
};
use async_trait::async_trait;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::runtime::Runtime;

/// Benchmark state machine
#[derive(Debug)]
struct BenchmarkStateMachine {
    counter: u64,
}

impl BenchmarkStateMachine {
    fn new() -> Self {
        Self { counter: 0 }
    }
}

#[async_trait]
impl StateMachine for BenchmarkStateMachine {
    type Command = Command;
    type Response = Response;
    
    async fn apply(&mut self, _command: Self::Command) -> Self::Response {
        self.counter += 1;
        Response {
            success: true,
            data: self.counter.to_le_bytes().to_vec(),
        }
    }
    
    async fn snapshot(&self) -> consensus::Result<Vec<u8>> {
        Ok(self.counter.to_le_bytes().to_vec())
    }
    
    async fn restore(&mut self, snapshot: &[u8]) -> consensus::Result<()> {
        if snapshot.len() >= 8 {
            let bytes: [u8; 8] = snapshot[0..8].try_into().unwrap();
            self.counter = u64::from_le_bytes(bytes);
        }
        Ok(())
    }
}

/// Setup a single node for benchmarking
async fn setup_single_node() -> RaftNode<MemoryStorage, MemoryTransport, BenchmarkStateMachine> {
    let node_id = NodeId::new();
    let config = Config::new(node_id, vec![node_id]);
    let storage = MemoryStorage::new();
    let (tx, rx) = mpsc::channel(1000);
    let transport = MemoryTransport::new(node_id, rx);
    let state_machine = BenchmarkStateMachine::new();
    
    RaftNode::new(config, storage, transport, state_machine).await.unwrap()
}

fn bench_single_node_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("single_node_proposal", |b| {
        b.to_async(&rt).iter(|| async {
            let node = setup_single_node().await;
            
            // Wait for node to become leader
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            
            let command = Command {
                data: black_box(b"increment".to_vec()),
            };
            
            let result = node.propose(command).await;
            assert!(result.is_ok());
        });
    });
}

fn bench_batch_proposals(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("batch_proposals");
    for size in [1, 10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let node = setup_single_node().await;
                
                // Wait for node to become leader
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                
                for i in 0..size {
                    let command = Command {
                        data: black_box(format!("command_{}", i).into_bytes()),
                    };
                    
                    let result = node.propose(command).await;
                    assert!(result.is_ok());
                }
            });
        });
    }
    group.finish();
}

fn bench_storage_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("storage_append", |b| {
        b.to_async(&rt).iter(|| async {
            let storage = MemoryStorage::new();
            let entry = consensus::storage::LogEntry {
                term: 1,
                index: 1,
                data: black_box(b"test_data".to_vec()),
            };
            
            let result = storage.append_entries(&[entry]).await;
            assert!(result.is_ok());
        });
    });
    
    c.bench_function("storage_get", |b| {
        b.to_async(&rt).iter(|| async {
            let storage = MemoryStorage::new();
            let entry = consensus::storage::LogEntry {
                term: 1,
                index: 1,
                data: b"test_data".to_vec(),
            };
            
            storage.append_entries(&[entry]).await.unwrap();
            let result = storage.get_entry(black_box(1)).await;
            assert!(result.is_ok());
        });
    });
}

fn bench_message_serialization(c: &mut Criterion) {
    use consensus::{Message, MessagePayload};
    
    let message = Message {
        msg_type: consensus::MessageType::AppendEntries,
        term: 1,
        from: NodeId::new(),
        to: NodeId::new(),
        payload: MessagePayload::AppendEntries {
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![
                consensus::storage::LogEntry {
                    term: 1,
                    index: 1,
                    data: b"test_data".to_vec(),
                }
            ],
            leader_commit: 0,
        },
    };
    
    c.bench_function("message_serialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_vec(&black_box(&message)).unwrap();
            black_box(serialized);
        });
    });
    
    c.bench_function("message_deserialize", |b| {
        let serialized = serde_json::to_vec(&message).unwrap();
        b.iter(|| {
            let deserialized: Message = serde_json::from_slice(&black_box(&serialized)).unwrap();
            black_box(deserialized);
        });
    });
}

criterion_group!(
    benches,
    bench_single_node_throughput,
    bench_batch_proposals,
    bench_storage_operations,
    bench_message_serialization
);
criterion_main!(benches);