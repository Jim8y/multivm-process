# Solana Engine Test Port Conflict Fix

## Issues Fixed

### 1. Port Conflicts (Address already in use)
- Multiple tests were running in parallel using the same default ports
- Port 8888 (RPC server) was conflicting between tests
- Port 8899 (Solana RPC) was conflicting between validator instances

### 2. Connection Refused Errors
- Solana validator was crashing or not starting properly
- No visibility into validator errors due to output being directed to /dev/null

## Solutions Applied

### 1. Unique Port Assignment
Added unique port generation in `test_utils.rs`:
```rust
// Atomic counter for generating unique ports
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Get unique ports for testing
pub fn get_unique_ports() -> (u16, u16, u16) {
    let base = 10000 + (PORT_COUNTER.fetch_add(10, Ordering::SeqCst) * 10);
    let gossip_port = base;
    let rpc_port = base + 1;
    let rpc_server_port = base + 2;
    (gossip_port, rpc_port, rpc_server_port)
}
```

Modified `create_engine()` to use unique ports:
```rust
pub async fn create_engine() -> Result<SolanaEngine, SolanaEngineError> {
    let (gossip_port, rpc_port, rpc_server_port) = get_unique_ports();
    
    let config = SolanaConfigBuilder::new()
        .gossip_port(gossip_port)
        .rpc_port(rpc_port)
        .build();
    
    let engine = SolanaEngine::new(config, rpc_server_port).await?;
    Ok(engine)
}
```

### 2. Validator Output Logging
Modified validator startup to log output for debugging:
```rust
// For debugging, let's capture the output to files
let log_file = std::fs::File::create(&self.solana_config.log_path)?;
let err_file = log_file.try_clone()?;

cmd.stdout(log_file)
    .stderr(err_file)
    .kill_on_drop(true);
```

Added `log_path` field to `SolanaConfig`:
```rust
pub struct SolanaConfig {
    // ... other fields ...
    /// Path to the validator log file
    pub log_path: String,
}
```

### 3. Tick Method for Deterministic Mode
Made `tick()` method public for test access (previous fix):
```rust
pub async fn tick(&self) -> Result<(), SolanaEngineError>
```

## Benefits

1. **No More Port Conflicts**: Each test gets unique ports (starting at 10000 and incrementing)
2. **Better Debugging**: Validator output is captured in log files
3. **Parallel Test Execution**: Tests can run concurrently without conflicts
4. **Visibility**: Can inspect validator logs when tests fail

## Testing

With these fixes:
1. Tests can run in parallel without port conflicts
2. Each validator instance gets its own ports
3. Validator logs are available at `/tmp/solana-private-ledger_*/validator.log`
4. Connection refused errors should be eliminated

## Additional Recommendations

1. Consider running tests with `--test-threads=1` if issues persist
2. Check validator logs for specific errors
3. Ensure sufficient system resources for multiple validator instances