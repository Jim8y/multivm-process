# Solana Engine RPC Test Fix

## Issue
The `engine_rpc_tests::test_get_slot_rpc_until_slot_10` test was failing with a timeout because the Solana validator was running in deterministic mode and wasn't progressing slots automatically.

## Root Cause
1. The Solana private validator is started with the `--deterministic` flag
2. In deterministic mode, the validator requires manual ticks to advance slots
3. The test was waiting for slots to advance but not sending ticks

## Solution Applied

### 1. Made tick() method public
Changed in `solana-execution-engine/src/engine.rs`:
```rust
// Before
pub(crate) async fn tick(&self) -> Result<(), SolanaEngineError> {

// After  
pub async fn tick(&self) -> Result<(), SolanaEngineError> {
```

### 2. Updated test to call tick()
Modified `solana-execution-engine/tests/engine_rpc_tests.rs` to actively tick the validator:
```rust
// Tick the validator to advance slots (since it's in deterministic mode)
// We need to tick multiple times per slot based on ticks_per_slot config
for _ in 0..2 {  // Default ticks_per_slot is 2
    if let Err(e) = engine.tick().await {
        warn!("Failed to tick validator: {}", e);
    }
}
```

## Technical Details

### Deterministic Mode
- The validator runs with `--deterministic` flag for predictable behavior in tests
- Requires manual ticks via IPC socket to advance time/slots
- Default configuration: 2 ticks per slot

### IPC Communication
- Uses Unix domain socket at `/tmp/solana-private-validator.sock`
- The `IpcClient` sends tick commands to advance the validator

## Testing
After these changes, the test should:
1. Start the validator in deterministic mode
2. Send ticks to advance slots
3. Monitor slot progression via RPC
4. Complete when target slot is reached

## Alternative Solutions
1. Run validator without `--deterministic` flag (less predictable for tests)
2. Create a separate public API for test utilities
3. Use a different test approach that doesn't require slot progression