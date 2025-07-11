//! CLI tests for the orchestrator binary

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use std::net::TcpListener;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("MultiVM Orchestrator"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--node-id"))
        .stdout(predicate::str::contains("--data-dir"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_invalid_args() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--invalid-arg")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
}

#[test]
fn test_cli_config_file_not_found() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--config")
        .arg("/nonexistent/config.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to load config"));
}

#[test]
fn test_cli_with_temp_data_dir() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    
    // Find available ports
    let consensus_port = find_available_port();
    let rpc_port = find_available_port();
    let metrics_port = find_available_port();
    let admin_port = find_available_port();
    
    cmd.arg("--data-dir")
        .arg(temp_dir.path())
        .arg("--consensus-port")
        .arg(consensus_port.to_string())
        .arg("--rpc-port")
        .arg(rpc_port.to_string())
        .arg("--metrics-port")
        .arg(metrics_port.to_string())
        .arg("--admin-port")
        .arg(admin_port.to_string())
        .arg("--no-tls")  // Disable TLS for testing
        .timeout(std::time::Duration::from_secs(2))
        .assert()
        .failure(); // Will timeout and be killed, which is expected
}

#[test]
fn test_cli_custom_node_id() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--node-id")
        .arg("test-node-123")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_cli_config_validation() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    
    // Write invalid config
    std::fs::write(&config_path, "invalid = toml content [").unwrap();
    
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--config")
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse config"));
}

#[test]
fn test_cli_log_level() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--log-level")
        .arg("debug")
        .arg("--help")
        .assert()
        .success();
    
    // Test invalid log level
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--log-level")
        .arg("invalid")
        .arg("--help")
        .assert()
        .failure();
}

#[test]
fn test_cli_cluster_members() {
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    cmd.arg("--cluster-member")
        .arg("node1:127.0.0.1:8080")
        .arg("--cluster-member")
        .arg("node2:127.0.0.1:8081")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_cli_all_ports_configurable() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("orchestrator").unwrap();
    
    cmd.arg("--data-dir")
        .arg(temp_dir.path())
        .arg("--consensus-port").arg("9001")
        .arg("--rpc-port").arg("9002")
        .arg("--metrics-port").arg("9003")
        .arg("--admin-port").arg("9004")
        .arg("--help")
        .assert()
        .success();
}

fn find_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}