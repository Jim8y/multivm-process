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
    core::manager::P2PManager,
    protocol::messages::*,
    security::{
        auth::{AuthConfig, AuthManager},
        dos_protection::{DosProtectionConfig, DosProtectionManager},
        encryption::EncryptionManager,
    },
};
use rand::Rng;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark message creation and basic operations
fn bench_message_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_creation");

    // Create different message payloads
    let message_payloads = vec![
        (
            "heartbeat",
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            }),
        ),
        (
            "svm_transaction",
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: Box::new(vec![1; 1024]), // 1KB transaction
                signature: "test_signature".to_string(),
            }),
        ),
        (
            "evm_transaction",
            MessagePayload::Evm(EvmMessage::Transaction {
                transaction_data: Box::new(vec![2; 2048]), // 2KB transaction
                tx_hash: "0x1234567890abcdef".to_string(),
            }),
        ),
        (
            "multivm_state_sync",
            MessagePayload::MultiVm(MultiVmMessage::StateSync {
                state_root: "state_root_hash".to_string(),
                vm_type: VmType::Svm,
                height: 12345,
            }),
        ),
        (
            "discovery_announce",
            MessagePayload::Discovery(DiscoveryMessage::Announce {
                capabilities: NodeCapabilities {
                    supported_vms: vec![VmType::Svm, VmType::Evm],
                    protocol_versions: vec![1],
                    features: vec!["discovery".to_string()],
                    limits: ResourceLimits {
                        max_connections: 100,
                        max_message_size: 1048576,
                        rate_limit: 1000.0,
                    },
                },
                addresses: vec!["/ip4/127.0.0.1/tcp/9000".to_string()],
            }),
        ),
    ];

    for (name, payload) in message_payloads {
        let message = NetworkMessage::new(
            payload.clone(),
            MessageSource::MultiVmLayer,
            MessageTarget::Broadcast,
        );

        group.bench_with_input(BenchmarkId::new("create", name), &message, |b, message| {
            b.iter(|| black_box(message.clone()))
        });
    }

    // Benchmark message serialization
    let test_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: Box::new(vec![1; 1024]),
            signature: "test_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

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

        let message = NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: Box::new(vec![0; size]),
                signature: "test_signature".to_string(),
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        );

        group.bench_with_input(
            BenchmarkId::new("serialize", size),
            &message,
            |b, message| b.iter(|| black_box(bincode::serialize(message).unwrap())),
        );

        let serialized = bincode::serialize(&message).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", size),
            &serialized,
            |b, data| b.iter(|| black_box(bincode::deserialize::<NetworkMessage>(data).unwrap())),
        );
    }

    group.finish();
}

/// Benchmark encryption operations
fn bench_encryption(c: &mut Criterion) {
    let _rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("encryption");

    // Create encryption manager
    let mut encryption_manager = EncryptionManager::new();

    // Generate test key pair
    let keypair = Keypair::generate_ed25519();
    let _peer_id = libp2p::PeerId::from(keypair.public());

    // Generate keys for encryption
    let public_key = encryption_manager.get_public_key();

    let data_sizes = vec![64, 256, 1024, 4096];

    for size in data_sizes {
        let data = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("encrypt", size), &data, |b, data| {
            b.iter(|| {
                black_box(
                    encryption_manager
                        .encrypt_message(&public_key, data)
                        .unwrap(),
                )
            })
        });

        let encrypted = encryption_manager
            .encrypt_message(&public_key, &data)
            .unwrap();
        group.bench_with_input(
            BenchmarkId::new("decrypt", size),
            &encrypted,
            |b, encrypted| {
                b.iter(|| {
                    black_box(
                        encryption_manager
                            .decrypt_message(&public_key, encrypted)
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
    let _rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("authentication");

    // Create auth manager
    let auth_config = AuthConfig::default();
    let auth_manager = AuthManager::new(auth_config).unwrap();

    // Generate test credentials
    let peer_id = libp2p::PeerId::random();

    group.bench_function("generate_jwt_token", |b| {
        b.iter(|| {
            black_box(
                auth_manager
                    .generate_jwt_token(&peer_id.to_string(), vec!["read".to_string()], "user")
                    .unwrap(),
            )
        })
    });

    // Benchmark JWT token generation with different permissions
    let permission_sets = vec![
        vec!["read".to_string()],
        vec!["read".to_string(), "write".to_string()],
        vec!["read".to_string(), "write".to_string(), "admin".to_string()],
    ];

    for permissions in permission_sets {
        let perm_count = permissions.len();
        group.bench_with_input(
            BenchmarkId::new("generate_jwt_with_permissions", perm_count),
            &permissions,
            |b, perms| {
                b.iter(|| {
                    black_box(
                        auth_manager
                            .generate_jwt_token(&peer_id.to_string(), perms.clone(), "user")
                            .unwrap(),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark message routing
fn bench_message_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_routing");

    // Create test message
    let test_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::Heartbeat {
            status: NodeStatus::Active,
            uptime: Duration::from_secs(3600),
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    // Create P2P manager and get handle
    let config = P2PConfig::default();
    let keypair = Keypair::generate_ed25519();
    let (manager, _rx) = P2PManager::new(config, keypair);
    let handle = manager.get_handle();

    group.bench_function("route_message", |b| {
        b.iter(|| {
            // In a real benchmark, we would measure actual routing
            // For now, just measure the overhead of message cloning and routing logic
            let _routed = black_box(test_message.clone());
        })
    });

    // Benchmark routing decision based on message type
    let message_types = vec![
        ("svm", MessageSource::SvmExecution),
        ("evm", MessageSource::EvmExecution),
        ("multivm", MessageSource::MultiVmLayer),
        ("network", MessageSource::NetworkLayer),
    ];

    for (name, source) in message_types {
        let message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            source,
            MessageTarget::Broadcast,
        );

        group.bench_with_input(
            BenchmarkId::new("route_by_source", name),
            &message,
            |b, message| {
                b.iter(|| {
                    // Simulate routing decision
                    match &message.source {
                        MessageSource::SvmExecution => black_box("svm_route"),
                        MessageSource::EvmExecution => black_box("evm_route"),
                        MessageSource::MultiVmLayer => black_box("multivm_route"),
                        MessageSource::NetworkLayer => black_box("network_route"),
                        _ => black_box("default_route"),
                    };
                })
            },
        );
    }

    drop(handle);
    group.finish();
}

/// Benchmark DoS protection mechanisms
fn bench_dos_protection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dos_protection");

    // Create DoS protection manager with specific config
    let dos_config = DosProtectionConfig {
        enable_connection_protection: true,
        max_connections_per_ip: 10,
        connection_rate_limit: 10,
        enable_bandwidth_limiting: true,
        max_bandwidth_per_peer: 10_000_000, // 10MB/s
        enable_message_size_validation: true,
        max_message_size: 1048576, // 1MB
        enable_reputation_system: true,
        min_reputation_score: 0.0,
        enable_adaptive_protection: true,
        protection_strictness: 5,
        memory_limit_mb: 100,
        cpu_threshold: 0.8,
        emergency_mode_timeout: Duration::from_secs(300),
    };

    let dos_manager = DosProtectionManager::new(dos_config);

    // Benchmark connection checking
    let test_ips: Vec<std::net::IpAddr> = (0..100)
        .map(|i| {
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(
                192,
                168,
                (i / 256) as u8,
                (i % 256) as u8,
            ))
        })
        .collect();

    group.bench_function("check_connection", |b| {
        b.to_async(&rt).iter(|| async {
            let ip = test_ips[rand::thread_rng().gen_range(0..test_ips.len())];
            black_box(dos_manager.check_connection(ip).await)
        })
    });

    // Benchmark message rate limiting
    let test_peers: Vec<libp2p::PeerId> = (0..100).map(|_| libp2p::PeerId::random()).collect();

    group.bench_function("check_message", |b| {
        b.to_async(&rt).iter(|| async {
            let peer_id = test_peers[rand::thread_rng().gen_range(0..test_peers.len())];
            black_box(dos_manager.check_message(peer_id, 1024).await)
        })
    });

    // Benchmark concurrent connection checks
    for num_concurrent in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_connection_checks", num_concurrent),
            &num_concurrent,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let futures: Vec<_> = (0..count)
                        .map(|i| {
                            let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(
                                127,
                                0,
                                0,
                                (i % 256) as u8,
                            ));
                            dos_manager.check_connection(ip)
                        })
                        .collect();

                    let results = futures::future::join_all(futures).await;
                    black_box(results)
                })
            },
        );
    }

    // Benchmark concurrent message checks
    for num_concurrent in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_message_checks", num_concurrent),
            &num_concurrent,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let futures: Vec<_> = (0..count)
                        .map(|_| {
                            let peer_id = libp2p::PeerId::random();
                            dos_manager.check_message(peer_id, 1024)
                        })
                        .collect();

                    let results = futures::future::join_all(futures).await;
                    black_box(results)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark network statistics collection
fn bench_network_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_stats");

    // Create test messages of various sizes
    let message_sizes = vec![64, 256, 1024, 4096];

    for size in message_sizes {
        let message = NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: Box::new(vec![0; size]),
                signature: "test_signature".to_string(),
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        );

        group.bench_with_input(
            BenchmarkId::new("stats_collection", size),
            &message,
            |b, message| {
                b.iter(|| {
                    // Simulate stats collection
                    let _size = black_box(bincode::serialize(message).unwrap().len());
                    let _timestamp = black_box(chrono::Utc::now());
                    let _message_type = black_box(&message.payload);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_message_creation,
    bench_message_throughput,
    bench_encryption,
    bench_authentication,
    bench_message_routing,
    bench_dos_protection,
    bench_network_stats
);

criterion_main!(benches);
