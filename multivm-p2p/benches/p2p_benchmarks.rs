//! Comprehensive P2P performance benchmarks
//!
//! This module contains benchmarks for various P2P operations including:
//! - Message creation and serialization
//! - Encryption/decryption performance
//! - Network message routing
//! - Authentication operations
//! - DoS protection mechanisms
//! - Rate limiting performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use libp2p::identity::Keypair;
use multivm_p2p::{
    config::P2PConfig,
    core::manager::{ManagerHandle, P2PManager},
    protocol::messages::*,
    security::{
        auth::{AuthConfig, AuthManager},
        dos_protection::{DosProtectionConfig, DosProtectionManager},
        encryption::EncryptionManager,
    },
};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark message creation and basic operations
fn bench_message_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_creation");

    // Benchmark different message types
    let message_types = vec![
        (
            "ping",
            NetworkMessage::Ping(PingMessage {
                sequence: 1,
                timestamp: chrono::Utc::now(),
            }),
        ),
        (
            "svm_transaction",
            NetworkMessage::SVM(SVMMessage::Transaction {
                data: vec![1; 1024], // 1KB transaction
                signature: vec![0; 64],
            }),
        ),
        (
            "evm_transaction",
            NetworkMessage::EVM(EVMMessage::Transaction {
                data: vec![2; 2048], // 2KB transaction
                hash: [0; 32],
            }),
        ),
        (
            "multivm_command",
            NetworkMessage::MultiVMCommand(MultiVMCommand::StartVM {
                vm_type: VMType::SVM,
                config: serde_json::json!({}),
            }),
        ),
        (
            "multivm_query",
            NetworkMessage::MultiVMQuery(MultiVMQuery::GetVMStatus {
                vm_type: VMType::EVM,
            }),
        ),
    ];

    for (name, message) in message_types {
        group.bench_with_input(BenchmarkId::new("create", name), &message, |b, message| {
            b.iter(|| black_box(message.clone()))
        });
    }

    // Benchmark message serialization
    let test_message = NetworkMessage::SVM(SVMMessage::Transaction {
        data: vec![1; 1024],
        signature: vec![0; 64],
    });

    group.bench_function("serialize", |b| {
        b.iter(|| black_box(bincode::serialize(&test_message).unwrap()))
    });

    group.bench_function("deserialize", |b| {
        let serialized = bincode::serialize(&test_message).unwrap();
        b.iter(|| black_box(bincode::deserialize::<NetworkMessage>(&serialized).unwrap()))
    });

    group.finish();
}

/// Benchmark message throughput with different sizes
fn bench_message_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_throughput");

    let sizes = vec![64, 256, 1024, 4096, 16384, 65536]; // Bytes

    for size in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        let message = NetworkMessage::SVM(SVMMessage::Transaction {
            data: vec![0; size],
            signature: vec![0; 64],
        });

        group.bench_with_input(BenchmarkId::new("serialize", size), &message, |b, msg| {
            b.iter(|| black_box(bincode::serialize(msg).unwrap()))
        });

        group.bench_with_input(
            BenchmarkId::new("create_and_serialize", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let msg = NetworkMessage::SVM(SVMMessage::Transaction {
                        data: vec![0; size],
                        signature: vec![0; 64],
                    });
                    black_box(bincode::serialize(&msg).unwrap())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark encryption/decryption operations
fn bench_encryption(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("encryption");

    let encryption_manager = EncryptionManager::new();
    let data_sizes = vec![64, 256, 1024, 4096, 16384];

    for size in data_sizes {
        let data = vec![0u8; size];
        let peer_public_key = x25519_dalek::PublicKey::from([1u8; 32]);

        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("encrypt", size), &data, |b, data| {
            b.iter(|| {
                black_box(
                    encryption_manager
                        .encrypt_message(&peer_public_key, data)
                        .unwrap(),
                )
            })
        });

        // Benchmark decryption
        let encrypted_data = encryption_manager
            .encrypt_message(&peer_public_key, &data)
            .unwrap();

        group.bench_with_input(
            BenchmarkId::new("decrypt", size),
            &encrypted_data,
            |b, encrypted| {
                b.iter(|| {
                    black_box(
                        encryption_manager
                            .decrypt_message(&peer_public_key, encrypted)
                            .unwrap(),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark authentication operations
fn bench_authentication(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("authentication");

    let auth_config = AuthConfig::default();
    let auth_manager = AuthManager::new(auth_config).unwrap();

    // Benchmark JWT token generation
    group.bench_function("jwt_generate", |b| {
        b.iter(|| {
            black_box(
                auth_manager
                    .generate_jwt_token(
                        "test_user",
                        vec!["read".to_string(), "write".to_string()],
                        "user",
                    )
                    .unwrap(),
            )
        })
    });

    // Benchmark JWT token verification
    let test_token = auth_manager
        .generate_jwt_token("test_user", vec!["read".to_string()], "user")
        .unwrap();

    group.bench_function("jwt_verify", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(
                auth_manager
                    .authenticate_jwt(&test_token, "127.0.0.1")
                    .await,
            )
        })
    });

    // Benchmark API key generation
    group.bench_function("api_key_generate", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(
                auth_manager
                    .generate_api_key("benchmark_key", vec!["admin".to_string()])
                    .await
                    .unwrap(),
            )
        })
    });

    // Benchmark API key authentication
    let (_, api_key) = rt.block_on(async {
        auth_manager
            .generate_api_key("test_key", vec!["read".to_string()])
            .await
            .unwrap()
    });

    group.bench_function("api_key_auth", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(
                auth_manager
                    .authenticate_api_key(&api_key, "127.0.0.1")
                    .await,
            )
        })
    });

    group.finish();
}

/// Benchmark DoS protection mechanisms
fn bench_dos_protection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dos_protection");

    let config = DosProtectionConfig::default();
    let dos_manager = DosProtectionManager::new(config);

    rt.block_on(async {
        dos_manager.start().await.unwrap();
    });

    // Benchmark connection checking
    group.bench_function("check_connection", |b| {
        b.to_async(&rt).iter(|| async {
            let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
            black_box(dos_manager.check_connection(ip).await)
        })
    });

    // Benchmark message validation
    let test_message = NetworkMessage::Ping(PingMessage {
        sequence: 1,
        timestamp: chrono::Utc::now(),
    });

    group.bench_function("check_message", |b| {
        b.to_async(&rt).iter(|| async {
            let peer_id = libp2p::PeerId::random();
            black_box(dos_manager.check_message(peer_id, 1024).await)
        })
    });

    // Benchmark reputation updates
    group.bench_function("record_success", |b| {
        b.to_async(&rt).iter(|| async {
            let peer_id = libp2p::PeerId::random();
            black_box(
                dos_manager
                    .record_success(peer_id, Duration::from_millis(50))
                    .await,
            )
        })
    });

    group.bench_function("record_failure", |b| {
        b.to_async(&rt).iter(|| async {
            let peer_id = libp2p::PeerId::random();
            black_box(
                dos_manager
                    .record_failure(peer_id, "timeout".to_string(), Duration::from_millis(5000))
                    .await,
            )
        })
    });

    group.finish();
}

/// Benchmark P2P Manager operations
fn bench_p2p_manager(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("p2p_manager");

    // Setup P2P manager
    let (manager, command_rx) = {
        let config = P2PConfig::default();
        let keypair = Keypair::generate_ed25519();
        P2PManager::new(config, keypair)
    };

    let handle = manager.get_handle();

    // We can't easily start the full manager in benchmarks, so we'll benchmark
    // the handle operations that don't require the manager to be running

    // Benchmark message creation and queueing (this will fail but measures overhead)
    let test_message = NetworkMessage::Ping(PingMessage {
        sequence: 1,
        timestamp: chrono::Utc::now(),
    });

    group.bench_function("message_send_attempt", |b| {
        b.to_async(&rt).iter(|| async {
            // This will fail since manager isn't running, but measures the overhead
            let _ = handle
                .send_message(test_message.clone(), Priority::Normal)
                .await;
            black_box(())
        })
    });

    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_operations");

    let dos_manager = {
        let config = DosProtectionConfig::default();
        let manager = DosProtectionManager::new(config);
        rt.block_on(async {
            manager.start().await.unwrap();
        });
        manager
    };

    // Benchmark concurrent connection checks
    let connection_counts = vec![1, 10, 50, 100];

    for count in connection_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_connections", count),
            &count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();

                    for i in 0..count {
                        let dos_manager = &dos_manager;
                        let handle = tokio::spawn(async move {
                            let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(
                                127,
                                0,
                                0,
                                (i % 255) as u8 + 1,
                            ));
                            dos_manager.check_connection(ip).await
                        });
                        handles.push(handle);
                    }

                    // Wait for all operations to complete
                    for handle in handles {
                        let _ = handle.await;
                    }

                    black_box(())
                })
            },
        );
    }

    // Benchmark concurrent message validations
    for count in &[1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_message_checks", count),
            count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();

                    for _ in 0..count {
                        let dos_manager = &dos_manager;
                        let handle = tokio::spawn(async move {
                            let peer_id = libp2p::PeerId::random();
                            dos_manager.check_message(peer_id, 1024).await
                        });
                        handles.push(handle);
                    }

                    // Wait for all operations to complete
                    for handle in handles {
                        let _ = handle.await;
                    }

                    black_box(())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage and allocation patterns
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    // Benchmark message pool allocation
    group.bench_function("message_pool_allocation", |b| {
        b.iter(|| {
            let mut messages = Vec::new();
            for i in 0..1000 {
                let message = NetworkMessage::Ping(PingMessage {
                    sequence: i as u64,
                    timestamp: chrono::Utc::now(),
                });
                messages.push(message);
            }
            black_box(messages)
        })
    });

    // Benchmark large message handling
    let large_sizes = vec![1024, 10240, 102400, 1048576]; // 1KB to 1MB

    for size in large_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("large_message_handling", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let message = NetworkMessage::SVM(SVMMessage::Transaction {
                        data: vec![0; size],
                        signature: vec![0; 64],
                    });

                    // Simulate processing
                    let serialized = bincode::serialize(&message).unwrap();
                    let _deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

                    black_box(message)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    p2p_benches,
    bench_message_creation,
    bench_message_throughput,
    bench_encryption,
    bench_authentication,
    bench_dos_protection,
    bench_p2p_manager,
    bench_concurrent_operations,
    bench_memory_usage
);

criterion_main!(p2p_benches);
