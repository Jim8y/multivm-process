# 🧪 MultiVM Testing Guide

This guide provides comprehensive information about testing the MultiVM system.

## 📋 Table of Contents

1. [Overview](#overview)
2. [Test Categories](#test-categories)
3. [Running Tests](#running-tests)
4. [Writing Tests](#writing-tests)
5. [Performance Benchmarks](#performance-benchmarks)
6. [Continuous Integration](#continuous-integration)

## Overview

The MultiVM project includes a comprehensive test suite to ensure reliability, correctness, and performance:

- **Unit Tests**: Test individual components in isolation
- **Integration Tests**: Test end-to-end system functionality
- **Performance Benchmarks**: Measure system performance metrics
- **Documentation Tests**: Ensure code examples in docs are correct

## Test Categories

### 1. Unit Tests

Unit tests are located within each crate's source directory:

#### Consensus Tests (`multivm-consensus/src/malachite_tests.rs`)
- Consensus initialization and lifecycle
- Block proposal and voting mechanisms
- Byzantine fault tolerance
- Timeout handling
- State recovery

#### Block Routing Tests (`multivm-process-manager/src/block_router_tests.rs`)
- Transaction decomposition
- Routing accuracy
- Dependency tracking
- Error handling
- Parallel processing

#### Account Mapping Tests (`multivm-account-mapping/src/validation_tests.rs`)
- Signature validation (Ed25519, ECDSA)
- Account binding verification
- Double-binding prevention
- Concurrent validation

#### Secure IPC Tests (`multivm-common/src/ipc/secure_transport_tests.rs`)
- Authentication mechanisms
- Rate limiting
- Message encryption/decryption
- Connection health monitoring
- Error recovery

### 2. Integration Tests

Located in `tests/` directory:

#### Basic Integration Tests (`tests/integration_tests.rs`)
- System startup and shutdown
- Health monitoring
- Block processing pipeline
- Account mapping integration
- Error handling

#### Enhanced Integration Tests (`tests/enhanced_integration_tests.rs`)
- Full system lifecycle
- End-to-end block processing
- Consensus fault tolerance
- Transaction routing accuracy
- Concurrent operations
- System recovery after crashes
- Performance under load

### 3. Performance Benchmarks

Located in `benches/multivm_benchmarks.rs`:

- Consensus block processing throughput
- Block routing performance
- Account mapping validation speed
- Transaction processing rates
- Concurrent operation scaling
- Serialization overhead
- Message processing efficiency

## Running Tests

### Quick Test Run

Run all tests with the provided script:

```bash
./scripts/run_all_tests.sh
```

### Run Tests with Benchmarks

```bash
./scripts/run_all_tests.sh --with-benchmarks
```

### Individual Test Commands

#### Run all unit tests
```bash
cargo test
```

#### Run tests for specific crate
```bash
cargo test -p multivm-consensus
cargo test -p multivm-process-manager
cargo test -p multivm-account-mapping
```

#### Run integration tests only
```bash
cargo test --test integration_tests
cargo test --test enhanced_integration_tests
```

#### Run benchmarks
```bash
cargo bench
```

#### Run specific benchmark
```bash
cargo bench --bench multivm_benchmarks consensus
```

#### Run with verbose output
```bash
cargo test -- --nocapture
```

#### Run single test
```bash
cargo test test_full_system_lifecycle
```

## Writing Tests

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_behavior() {
        // Arrange
        let component = MyComponent::new();
        
        // Act
        let result = component.process(input);
        
        // Assert
        assert_eq!(result, expected_output);
    }

    #[tokio::test]
    async fn test_async_behavior() {
        // Async test implementation
    }
}
```

### Integration Test Example

```rust
#[tokio::test]
async fn test_system_integration() {
    // Setup test environment
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());
    
    // Initialize system
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();
    
    // Test system behavior
    let block = create_test_block();
    coordinator.submit_block(block).await.unwrap();
    
    // Verify results
    let stats = coordinator.get_stats().await;
    assert_eq!(stats.blocks_processed, 1);
    
    // Cleanup
    coordinator.shutdown().await.unwrap();
}
```

### Benchmark Example

```rust
fn benchmark_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_operation");
    
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("size", size),
            size,
            |b, &size| {
                b.iter(|| {
                    perform_operation(black_box(size))
                });
            },
        );
    }
    
    group.finish();
}
```

## Test Best Practices

### 1. Test Organization
- Keep unit tests close to the code they test
- Use descriptive test names that explain what is being tested
- Group related tests in modules

### 2. Test Independence
- Each test should be independent and not rely on others
- Use setup and teardown functions when needed
- Clean up resources (files, network connections) after tests

### 3. Test Coverage
- Aim for high test coverage (>80%)
- Test both happy paths and error cases
- Include edge cases and boundary conditions

### 4. Performance Tests
- Use realistic workloads in benchmarks
- Run benchmarks multiple times for consistency
- Compare against baseline metrics

### 5. Async Testing
- Use `tokio::test` for async tests
- Handle timeouts appropriately
- Test concurrent scenarios

## Performance Benchmarks

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run with HTML report
cargo bench -- --output-format html

# Compare against baseline
cargo bench -- --baseline main
```

### Benchmark Categories

1. **Consensus Performance**
   - Block processing throughput
   - Voting round completion time
   - Message handling latency

2. **Block Routing**
   - Transaction decomposition speed
   - Routing decision time
   - Parallel processing efficiency

3. **Account Mapping**
   - Signature validation speed
   - Mapping lookup performance
   - Concurrent validation throughput

4. **System Performance**
   - End-to-end block processing time
   - Transaction throughput (TPS)
   - Memory usage under load

### Performance Targets

- Block processing: >100 blocks/second
- Transaction throughput: >5000 TPS
- Consensus latency: <500ms per round
- Memory usage: <2GB under normal load

## Continuous Integration

### GitHub Actions Configuration

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: ./scripts/run_all_tests.sh
      - name: Run benchmarks
        run: cargo bench -- --quick
```

### Pre-commit Hooks

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Run tests before commit
cargo test --quiet
if [ $? -ne 0 ]; then
    echo "Tests failed. Commit aborted."
    exit 1
fi

# Check formatting
cargo fmt -- --check
if [ $? -ne 0 ]; then
    echo "Code needs formatting. Run 'cargo fmt'"
    exit 1
fi
```

## Debugging Tests

### Enable Debug Logging

```bash
RUST_LOG=debug cargo test -- --nocapture
```

### Use Test Tracing

```rust
use tracing_test::traced_test;

#[traced_test]
#[tokio::test]
async fn test_with_tracing() {
    // Test logs will be captured and displayed
}
```

### Debugging Tips

1. Use `dbg!()` macro for quick debugging
2. Add temporary `println!()` statements
3. Use debugger with VS Code or IntelliJ Rust
4. Check test output with `--nocapture`

## Test Maintenance

### Regular Tasks

1. **Weekly**: Run full test suite including benchmarks
2. **Before Release**: Full test coverage report
3. **After Major Changes**: Update performance baselines
4. **Quarterly**: Review and update test documentation

### Test Coverage Report

Generate coverage report:

```bash
cargo tarpaulin --out Html
```

View coverage:
```bash
open tarpaulin-report.html
```

## Troubleshooting

### Common Issues

1. **Tests hanging**: Check for deadlocks or infinite loops
2. **Flaky tests**: Add proper synchronization and timeouts
3. **Resource exhaustion**: Ensure proper cleanup in tests
4. **Platform differences**: Use platform-agnostic paths and APIs

### Getting Help

- Check test output carefully for error messages
- Review test logs with `--nocapture`
- Use debugger for complex issues
- Ask for help in project discussions

## Summary

The MultiVM test suite ensures the system is:
- ✅ Functionally correct
- ✅ Performance optimized
- ✅ Fault tolerant
- ✅ Production ready

Regular testing is essential for maintaining system quality and reliability.