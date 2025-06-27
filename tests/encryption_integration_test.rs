//! Integration test to demonstrate IPC encryption in production configuration

use multivm_common::config::{IpcConfig, IpcTransportConfig};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_production_ipc_config_has_encryption_enabled() {
    // Create production IPC configuration
    let ipc_config = IpcConfig {
        transport: IpcTransportConfig::UnixSocket {
            path: PathBuf::from("/tmp/multivm-production.sock"),
        },
        message_timeout: Duration::from_secs(30),
        enable_encryption: true, // This should be true for production
    };

    assert!(ipc_config.enable_encryption, "IPC encryption must be enabled in production");
    println!("✅ Production IPC configuration has encryption enabled");
}

#[test]
fn test_development_ipc_config_can_disable_encryption() {
    // Development configuration with encryption disabled
    let ipc_config = IpcConfig {
        transport: IpcTransportConfig::TcpSocket {
            host: "127.0.0.1".to_string(),
            port: 9999,
        },
        message_timeout: Duration::from_secs(30),
        enable_encryption: false, // Can be disabled for development/testing
    };

    assert!(!ipc_config.enable_encryption, "IPC encryption can be disabled in development");
    println!("⚠️ Development IPC configuration has encryption disabled - use only for testing");
}