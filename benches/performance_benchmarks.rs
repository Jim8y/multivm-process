//! Performance benchmarks for critical MultiVM operations
//!
//! These benchmarks measure the performance of key system components
//! to ensure they meet performance requirements and detect regressions.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use multivm_common::{
    MultivmConfig, ProcessId, VmType, IpcCommand, types::core::MessageId,
};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark message ID generation and operations
fn bench_message_id_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_id");
    
    group.bench_function("generate", |b| {
        b.iter(|| {
            let msg_id = MessageId::new();
            black_box(msg_id)
        })
    });
    
    group.bench_function("to_string", |b| {
        let msg_id = MessageId::new();
        b.iter(|| {
            let string_repr = msg_id.to_string();
            black_box(string_repr)
        })
    });
    
    group.bench_function("to_bytes", |b| {
        let msg_id = MessageId::new();
        b.iter(|| {
            let bytes = msg_id.to_bytes();
            black_box(bytes)
        })
    });
    
    group.bench_function("from_bytes", |b| {
        let bytes = [1u8; 16];
        b.iter(|| {
            let msg_id = MessageId::from_bytes(black_box(bytes));
            black_box(msg_id)
        })
    });
    
    group.finish();
}

/// Benchmark IPC message creation and serialization
fn bench_ipc_message_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_message");
    
    group.bench_function("create", |b| {
        b.iter(|| {
            let message = multivm_common::IpcMessage::new(
                black_box(ProcessId::Main),
                black_box(ProcessId::Solana),
                black_box(IpcCommand::Ping)
            );
            black_box(message)
        })
    });
    
    group.bench_function("serialize_json", |b| {
        let message = multivm_common::IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::GetHealth
        );
        b.iter(|| {
            let json = serde_json::to_string(&black_box(&message)).unwrap();
            black_box(json)
        })
    });
    
    group.bench_function("deserialize_json", |b| {
        let message = multivm_common::IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::GetHealth
        );
        let json = serde_json::to_string(&message).unwrap();
        
        b.iter(|| {
            let deserialized: multivm_common::IpcMessage = 
                serde_json::from_str(&black_box(&json)).unwrap();
            black_box(deserialized)
        })
    });
    
    group.bench_function("serialize_bincode", |b| {
        let message = multivm_common::IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::GetHealth
        );
        b.iter(|| {
            let bytes = bincode::serialize(&black_box(&message)).unwrap();
            black_box(bytes)
        })
    });
    
    group.finish();
}

/// Benchmark account address operations
fn bench_account_address_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("account_address");
    
    // Benchmark Solana address operations
    group.bench_function("solana_address_create", |b| {
        let bytes = [1u8; 32];
        b.iter(|| {
            let addr = multivm_account_mapping::addresses::SolanaAddress(black_box(bytes));
            black_box(addr)
        })
    });
    
    group.bench_function("solana_address_validate", |b| {
        let addr = multivm_account_mapping::addresses::SolanaAddress([1u8; 32]);
        b.iter(|| {
            let is_valid = black_box(&addr).is_valid();
            black_box(is_valid)
        })
    });
    
    group.bench_function("solana_address_serialize", |b| {
        let addr = multivm_account_mapping::addresses::SolanaAddress([1u8; 32]);
        b.iter(|| {
            let json = serde_json::to_string(&black_box(&addr)).unwrap();
            black_box(json)
        })
    });
    
    // Benchmark Ethereum address operations
    group.bench_function("ethereum_address_create", |b| {
        let bytes = [1u8; 20];
        b.iter(|| {
            let addr = multivm_account_mapping::addresses::EthereumAddress(black_box(bytes));
            black_box(addr)
        })
    });
    
    group.bench_function("ethereum_address_validate", |b| {
        let addr = multivm_account_mapping::addresses::EthereumAddress([1u8; 20]);
        b.iter(|| {
            let is_valid = black_box(&addr).is_valid();
            black_box(is_valid)
        })
    });
    
    group.finish();
}

/// Benchmark account mapping operations
fn bench_account_mapping_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("account_mapping");
    
    let svm_addr = multivm_account_mapping::addresses::SolanaAddress([1u8; 32]);
    let evm_addr = multivm_account_mapping::addresses::EthereumAddress([1u8; 20]);
    
    group.bench_function("create_mapping", |b| {
        b.iter(|| {
            let mapping = multivm_account_mapping::AccountMapping::new(
                black_box(svm_addr),
                black_box(evm_addr)
            );
            black_box(mapping)
        })
    });
    
    group.bench_function("mapping_lookup_svm", |b| {
        let mapping = multivm_account_mapping::AccountMapping::new(svm_addr, evm_addr);
        b.iter(|| {
            let contains = black_box(&mapping).contains_svm_address(&black_box(svm_addr));
            black_box(contains)
        })
    });
    
    group.bench_function("mapping_lookup_evm", |b| {
        let mapping = multivm_account_mapping::AccountMapping::new(svm_addr, evm_addr);
        b.iter(|| {
            let contains = black_box(&mapping).contains_evm_address(&black_box(evm_addr));
            black_box(contains)
        })
    });
    
    group.finish();
}

/// Benchmark network message operations
fn bench_network_message_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_message");
    
    group.bench_function("create_control_message", |b| {
        b.iter(|| {
            let message = multivm_p2p::messages::NetworkMessage::new(
                multivm_p2p::messages::MessagePayload::Control(
                    multivm_p2p::messages::ControlMessage::Ping
                ),
                multivm_p2p::messages::MessageSource::NetworkLayer,
                multivm_p2p::messages::MessageTarget::Broadcast,
            );
            black_box(message)
        })
    });
    
    group.bench_function("create_svm_message", |b| {
        b.iter(|| {
            let message = multivm_p2p::messages::NetworkMessage::new(
                multivm_p2p::messages::MessagePayload::Svm(
                    multivm_p2p::messages::SvmMessage::Transaction {
                        transaction_data: Box::new(vec![1, 2, 3, 4]),
                        signature: "test_signature".to_string(),
                    }
                ),
                multivm_p2p::messages::MessageSource::SvmExecution,
                multivm_p2p::messages::MessageTarget::Broadcast,
            );
            black_box(message)
        })
    });
    
    group.bench_function("message_size_estimation", |b| {
        let message = multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Control(
                multivm_p2p::messages::ControlMessage::Ping
            ),
            multivm_p2p::messages::MessageSource::NetworkLayer,
            multivm_p2p::messages::MessageTarget::Broadcast,
        );
        b.iter(|| {
            let size = black_box(&message).estimated_size();
            black_box(size)
        })
    });
    
    group.bench_function("message_serialization", |b| {
        let message = multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Control(
                multivm_p2p::messages::ControlMessage::Ping
            ),
            multivm_p2p::messages::MessageSource::NetworkLayer,
            multivm_p2p::messages::MessageTarget::Broadcast,
        );
        b.iter(|| {
            let json = serde_json::to_string(&black_box(&message)).unwrap();
            black_box(json)
        })
    });
    
    group.finish();
}

/// Benchmark configuration operations
fn bench_configuration_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("configuration");
    
    group.bench_function("config_creation", |b| {
        b.iter(|| {
            let config = MultivmConfig::default();
            black_box(config)
        })
    });
    
    group.bench_function("config_validation", |b| {
        let config = MultivmConfig::default();
        b.iter(|| {
            let is_valid = black_box(&config).validate();
            black_box(is_valid)
        })
    });
    
    group.bench_function("config_serialization", |b| {
        let config = MultivmConfig::default();
        b.iter(|| {
            let json = serde_json::to_string(&black_box(&config)).unwrap();
            black_box(json)
        })
    });
    
    group.bench_function("config_deserialization", |b| {
        let config = MultivmConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        b.iter(|| {
            let deserialized: MultivmConfig = serde_json::from_str(&black_box(&json)).unwrap();
            black_box(deserialized)
        })
    });
    
    group.finish();
}

/// Benchmark error handling operations
fn bench_error_handling_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");
    
    group.bench_function("create_network_error", |b| {
        b.iter(|| {
            let error = multivm_common::MultivmError::Network {
                message: "Connection failed".to_string(),
                endpoint: Some("http://localhost:8899".to_string()),
                retry_after: Some(Duration::from_secs(5)),
            };
            black_box(error)
        })
    });
    
    group.bench_function("error_to_string", |b| {
        let error = multivm_common::MultivmError::Network {
            message: "Connection failed".to_string(),
            endpoint: Some("http://localhost:8899".to_string()),
            retry_after: Some(Duration::from_secs(5)),
        };
        b.iter(|| {
            let error_str = black_box(&error).to_string();
            black_box(error_str)
        })
    });
    
    group.bench_function("error_chain_creation", |b| {
        b.iter(|| {
            let inner_error = multivm_common::MultivmError::Internal {
                message: "Inner error".to_string(),
                source: None,
            };
            let outer_error = multivm_common::MultivmError::Process {
                process_id: "test_process".to_string(),
                message: "Process failed".to_string(),
                exit_code: Some(1),
            };
            black_box((inner_error, outer_error))
        })
    });
    
    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_operations");
    
    group.bench_function("concurrent_message_creation", |b| {
        let rt = Runtime::new().unwrap();
        b.to_async(&rt).iter(|| async {
            let mut handles = Vec::new();
            
            for i in 0..100 {
                let handle = tokio::spawn(async move {
                    multivm_common::IpcMessage::new(
                        ProcessId::Main,
                        ProcessId::Solana,
                        IpcCommand::Ping
                    )
                });
                handles.push(handle);
            }
            
            let mut messages = Vec::new();
            for handle in handles {
                messages.push(handle.await.unwrap());
            }
            
            black_box(messages)
        })
    });
    
    // Benchmark concurrent account address validation
    group.bench_function("concurrent_address_validation", |b| {
        let rt = Runtime::new().unwrap();
        let addresses: Vec<_> = (0..100)
            .map(|i| multivm_account_mapping::addresses::SolanaAddress([i as u8; 32]))
            .collect();
        
        b.to_async(&rt).iter(|| async {
            let mut handles = Vec::new();
            
            for addr in &addresses {
                let addr_copy = *addr;
                let handle = tokio::spawn(async move {
                    addr_copy.is_valid()
                });
                handles.push(handle);
            }
            
            let mut results = Vec::new();
            for handle in handles {
                results.push(handle.await.unwrap());
            }
            
            black_box(results)
        })
    });
    
    group.finish();
}

/// Benchmark memory operations
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");
    
    // Benchmark large message creation
    group.bench_function("large_message_creation", |b| {
        b.iter(|| {
            let large_data = vec![0u8; 1024 * 1024]; // 1MB
            let message = multivm_p2p::messages::NetworkMessage::new(
                multivm_p2p::messages::MessagePayload::Svm(
                    multivm_p2p::messages::SvmMessage::Transaction {
                        transaction_data: Box::new(large_data),
                        signature: "large_signature".to_string(),
                    }
                ),
                multivm_p2p::messages::MessageSource::SvmExecution,
                multivm_p2p::messages::MessageTarget::Broadcast,
            );
            black_box(message)
        })
    });
    
    // Benchmark batch operations
    group.bench_function("batch_address_creation", |b| {
        b.iter(|| {
            let mut addresses = Vec::new();
            for i in 0..1000 {
                let addr = multivm_account_mapping::addresses::SolanaAddress([i as u8; 32]);
                addresses.push(addr);
            }
            black_box(addresses)
        })
    });
    
    group.finish();
}

/// Benchmark JSON operations with different payload sizes
fn bench_json_operations_by_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_operations_by_size");
    
    let sizes = vec![100, 1000, 10000, 100000];
    
    for size in sizes {
        group.bench_with_input(BenchmarkId::new("serialize", size), &size, |b, &size| {
            let data = vec![1u8; size];
            let message = multivm_p2p::messages::NetworkMessage::new(
                multivm_p2p::messages::MessagePayload::Svm(
                    multivm_p2p::messages::SvmMessage::Transaction {
                        transaction_data: Box::new(data),
                        signature: "test".to_string(),
                    }
                ),
                multivm_p2p::messages::MessageSource::SvmExecution,
                multivm_p2p::messages::MessageTarget::Broadcast,
            );
            
            b.iter(|| {
                let json = serde_json::to_string(&black_box(&message)).unwrap();
                black_box(json)
            })
        });
        
        group.bench_with_input(BenchmarkId::new("deserialize", size), &size, |b, &size| {
            let data = vec![1u8; size];
            let message = multivm_p2p::messages::NetworkMessage::new(
                multivm_p2p::messages::MessagePayload::Svm(
                    multivm_p2p::messages::SvmMessage::Transaction {
                        transaction_data: Box::new(data),
                        signature: "test".to_string(),
                    }
                ),
                multivm_p2p::messages::MessageSource::SvmExecution,
                multivm_p2p::messages::MessageTarget::Broadcast,
            );
            let json = serde_json::to_string(&message).unwrap();
            
            b.iter(|| {
                let deserialized: multivm_p2p::messages::NetworkMessage = 
                    serde_json::from_str(&black_box(&json)).unwrap();
                black_box(deserialized)
            })
        });
    }
    
    group.finish();
}

/// Benchmark hash operations
fn bench_hash_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_operations");
    
    group.bench_function("address_hash", |b| {
        let addr = multivm_account_mapping::addresses::SolanaAddress([1u8; 32]);
        b.iter(|| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            
            let mut hasher = DefaultHasher::new();
            black_box(&addr).hash(&mut hasher);
            let hash_value = hasher.finish();
            black_box(hash_value)
        })
    });
    
    group.bench_function("message_id_hash", |b| {
        let msg_id = MessageId::new();
        b.iter(|| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            
            let mut hasher = DefaultHasher::new();
            black_box(&msg_id).hash(&mut hasher);
            let hash_value = hasher.finish();
            black_box(hash_value)
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_message_id_operations,
    bench_ipc_message_operations,
    bench_account_address_operations,
    bench_account_mapping_operations,
    bench_network_message_operations,
    bench_configuration_operations,
    bench_error_handling_operations,
    bench_concurrent_operations,
    bench_memory_operations,
    bench_json_operations_by_size,
    bench_hash_operations
);

criterion_main!(benches);