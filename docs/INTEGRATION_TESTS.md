# MultiVM Integration Tests

This document describes the comprehensive integration test suite for the MultiVM blockchain system, focusing on testing real connections to Reth and Solana processes and their communication workflows.

## Overview

The integration tests are designed to validate:
- **Process Communication**: Real connections between MultiVM and execution engines
- **Transaction Processing**: End-to-end transaction workflows across VMs
- **RPC Request Relaying**: Proper routing and handling of RPC requests
- **Block Processing**: Complete block creation, validation, and execution
- **Cross-VM Coordination**: Synchronization between different execution engines

## Test Structure

### 1. Reth Integration Tests (`integration_reth_engine_test.rs`)

Tests the Reth execution engine integration:

**Test Categories:**
- **Process Connectivity**: Starting Reth and establishing communication
- **RPC Request Relaying**: Testing JSON-RPC method calls
- **Transaction Processing**: Submitting and processing EVM transactions
- **Block Processing**: Block creation and validation workflows
- **Engine API**: Testing Engine API for consensus integration
- **MultiVM Integration**: Cross-VM communication readiness

**Key Tests:**
```rust
#[tokio::test]
async fn test_reth_connectivity_only() // Basic connectivity test
#[tokio::test]
async fn test_reth_rpc_communication() // RPC communication test
#[tokio::test]
async fn test_reth_integration_full_workflow() // Complete workflow test
```

### 2. Solana Integration Tests (`integration_solana_engine_test.rs`)

Tests the Solana execution engine integration:

**Test Categories:**
- **Process Connectivity**: Starting Solana validator and establishing communication
- **RPC Request Relaying**: Testing Solana RPC method calls
- **Transaction Processing**: Submitting and processing Solana transactions
- **Block Processing**: Slot progression and block validation
- **Mempool Operations**: Transaction pool management
- **Account Operations**: Account queries and state management

**Key Tests:**
```rust
#[tokio::test]
async fn test_solana_connectivity_only() // Basic connectivity test
#[tokio::test]
async fn test_solana_rpc_communication() // RPC communication test
#[tokio::test]
async fn test_solana_integration_full_workflow() // Complete workflow test
```

### 3. Cross-VM Integration Tests (`integration_cross_vm_test.rs`)

Tests coordination between Reth and Solana:

**Test Categories:**
- **Dual Engine Startup**: Starting both engines simultaneously
- **Cross-VM Transaction Coordination**: Processing mixed transaction types
- **Consensus Coordination**: Coordinating consensus across VMs
- **State Synchronization**: Ensuring consistent state across VMs
- **Error Handling**: Testing recovery from failures

**Key Tests:**
```rust
#[tokio::test]
async fn test_dual_engine_startup_only() // Basic dual startup test
#[tokio::test]
async fn test_cross_vm_full_integration() // Complete cross-VM test
```

### 4. RPC Relay Tests (`integration_rpc_relay_test.rs`)

Tests RPC request routing and proxy functionality:

**Test Categories:**
- **RPC Server Startup**: Starting RPC proxy servers
- **Request Relaying**: Forwarding requests to appropriate engines
- **Concurrent Requests**: Handling multiple simultaneous requests
- **Error Handling**: Managing invalid requests and timeouts
- **Load Balancing**: Distributing requests across engines

**Key Tests:**
```rust
#[tokio::test]
async fn test_rpc_proxy_only() // Basic proxy functionality
#[tokio::test]
async fn test_rpc_relay_full_integration() // Complete relay test
```

### 5. Block Processing Tests (`integration_block_processing_test.rs`)

Tests end-to-end block processing workflows:

**Test Categories:**
- **Components Startup**: Starting all required components
- **Transaction Submission**: Submitting transactions to consensus
- **Block Proposal**: Creating and proposing blocks
- **Block Execution**: Executing blocks on execution engines
- **Block Finalization**: Finalizing and committing blocks
- **Synchronization**: Ensuring all components stay synchronized

**Key Tests:**
```rust
#[tokio::test]
async fn test_basic_block_processing() // Basic processing test
#[tokio::test]
async fn test_block_processing_full_workflow() // Complete workflow test
```

## Running Integration Tests

### Prerequisites

1. **Rust Environment**: Ensure you have Rust and Cargo installed
2. **Custom Fork Binaries** (for full integration tests):
   - `reth` - Custom Reth binary from `git@github.com:vm-multiverse/reth.git` (dev branch)
   - `solana-test-validator` - Custom Solana validator from `git@github.com:vm-multiverse/multivm-agave.git` (master branch)

### Setting Up Custom Binaries

To build and install the custom fork binaries:

```bash
# Automated setup script
./scripts/setup-test-binaries.sh

# This will:
# 1. Clone git@github.com:vm-multiverse/reth.git (dev branch)
# 2. Clone git@github.com:vm-multiverse/multivm-agave.git (master branch)
# 3. Build both binaries
# 4. Create symlinks for easy access
```

### Quick Test Run

Use the provided test runner script:

```bash
# Run basic integration tests (no external binaries required)
./scripts/run-integration-tests.sh

# Run with verbose output
VERBOSE=true ./scripts/run-integration-tests.sh

# Run including long-running tests (requires Reth and Solana binaries)
SKIP_LONG_RUNNING=false ./scripts/run-integration-tests.sh

# Run with custom timeout
TEST_TIMEOUT=600 ./scripts/run-integration-tests.sh
```

### Manual Test Execution

Run individual test suites:

```bash
# Test Reth integration (requires reth binary)
cargo test --test integration_reth_engine_test test_reth_connectivity_only

# Test Solana integration (requires solana-test-validator)
cargo test --test integration_solana_engine_test test_solana_mempool_only

# Test cross-VM integration (requires both binaries)
cargo test --test integration_cross_vm_test test_dual_engine_startup_only

# Test RPC relay functionality
cargo test --test integration_rpc_relay_test test_rpc_proxy_only

# Test block processing workflow
cargo test --test integration_block_processing_test test_basic_block_processing
```

### Running All Tests

```bash
# Run all integration tests (may take a long time)
cargo test --tests
```

## Test Configuration

Tests can be configured through environment variables:

```bash
# Skip long-running tests
export SKIP_LONG_RUNNING=true

# Enable verbose output
export VERBOSE=true

# Set test timeout
export TEST_TIMEOUT=300

# Custom data directories
export RETH_DATA_DIR=/tmp/custom-reth-test
export SOLANA_LEDGER_PATH=/tmp/custom-solana-test
```

## Test Architecture

### Test Data Flow

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Consensus     │    │   Reth Engine   │    │ Solana Engine   │
│   Manager       │    │                 │    │                 │
├─────────────────┤    ├─────────────────┤    ├─────────────────┤
│ • Block Proposal│    │ • EVM Execution │    │ • SVM Execution │
│ • Consensus     │    │ • State Updates │    │ • Account Mgmt  │
│ • Finalization  │    │ • RPC Endpoints │    │ • Slot Progress │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                  ┌─────────────────┐
                  │  Integration    │
                  │  Test Suite     │
                  │                 │
                  │ • Coordinates   │
                  │ • Validates     │
                  │ • Reports       │
                  └─────────────────┘
```

### Test Isolation

Each test suite runs in isolation with:
- **Separate Data Directories**: Each test uses unique temporary directories
- **Unique Port Numbers**: Tests use different ports to avoid conflicts
- **Independent Processes**: Each test spawns its own engine processes
- **Cleanup Procedures**: Automatic cleanup after test completion

## Test Scenarios

### 1. Basic Connectivity
- Start execution engines
- Verify health endpoints
- Test basic RPC calls
- Validate process communication

### 2. Transaction Processing
- Submit various transaction types
- Verify transaction validation
- Check transaction execution
- Validate state changes

### 3. Block Processing
- Create blocks with mixed transactions
- Validate block structure
- Execute blocks on engines
- Verify consensus finalization

### 4. Error Handling
- Test invalid transactions
- Handle network failures
- Validate error recovery
- Check system resilience

### 5. Performance Testing
- Concurrent request handling
- Transaction throughput
- Block processing speed
- Resource utilization

## Troubleshooting

### Common Issues

1. **Binary Not Found**
   ```
   Error: reth binary not found in PATH
   Solution: Install custom Reth binary or skip long-running tests
   ```

2. **Port Conflicts**
   ```
   Error: Address already in use
   Solution: Ensure no other processes are using test ports
   ```

3. **Timeout Errors**
   ```
   Error: Test timed out
   Solution: Increase TEST_TIMEOUT or check system resources
   ```

4. **Permission Errors**
   ```
   Error: Permission denied
   Solution: Ensure write access to /tmp directory
   ```

### Debug Mode

Run tests with debug output:

```bash
# Enable debug logging
RUST_LOG=debug cargo test --test integration_reth_engine_test

# Enable trace logging
RUST_LOG=trace cargo test --test integration_cross_vm_test
```

### Test Data Inspection

Test data is stored in temporary directories:
- Reth data: `/tmp/*reth*`
- Solana data: `/tmp/*solana*`
- Logs: Check stdout/stderr for detailed logs

## Contributing

When adding new integration tests:

1. **Follow Naming Convention**: Use descriptive test names
2. **Add Documentation**: Document test purpose and requirements
3. **Include Cleanup**: Ensure proper resource cleanup
4. **Test Isolation**: Don't rely on other tests' state
5. **Error Handling**: Handle expected failures gracefully
6. **Performance**: Consider test execution time

### Example Test Structure

```rust
#[tokio::test]
#[ignore = "Requires actual binary"]
async fn test_new_integration_feature() {
    // Setup logging
    setup_logging();
    
    // Create test configuration
    let config = TestConfig::default();
    let mut test_suite = TestSuite::new(config);
    
    // Setup test environment
    test_suite.setup().await.expect("Setup failed");
    
    // Run test logic
    test_suite.test_feature().await.expect("Test failed");
    
    // Cleanup
    test_suite.cleanup().await.expect("Cleanup failed");
}
```

## Continuous Integration

The integration tests are designed to work in CI environments:

```yaml
# Example GitHub Actions workflow
- name: Run Integration Tests
  run: |
    # Run basic tests always
    ./scripts/run-integration-tests.sh
    
    # Run full tests if binaries are available
    if [ -f "reth" ] && [ -f "solana-test-validator" ]; then
      SKIP_LONG_RUNNING=false ./scripts/run-integration-tests.sh
    fi
```

## Security Considerations

Integration tests use:
- **Isolated Networks**: Tests don't connect to external networks
- **Temporary Data**: All data is cleaned up after tests
- **Mock Credentials**: Only test credentials are used
- **Local Processes**: All processes run locally

## Performance Metrics

Tests measure:
- **Startup Time**: Time to start engines
- **Transaction Throughput**: Transactions processed per second
- **Block Processing Time**: Time to process blocks
- **RPC Response Time**: Time to respond to RPC requests
- **Memory Usage**: Memory consumption during tests
- **CPU Usage**: CPU utilization during tests

## Future Enhancements

Planned improvements:
- **Property-based Testing**: Generate random test scenarios
- **Chaos Engineering**: Test system resilience
- **Performance Benchmarking**: Automated performance regression testing
- **Visual Test Reports**: Generate HTML test reports
- **Test Parallelization**: Run tests in parallel safely
- **Docker Integration**: Run tests in containerized environments