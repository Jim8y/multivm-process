# Detailed Concurrency Issues with Code Examples

## 1. Deadlock Risk in Coordinator

**File**: `/crates/multivm-process-manager/src/coordinator.rs`

### Issue: Multiple Lock Acquisition Without Consistent Ordering

```rust
// Lines 435-640 in process_block_internal()
async fn process_block_internal(
    // ... parameters ...
    state: &Arc<RwLock<CoordinatorState>>,
    account_mappings: &Arc<RwLock<HashMap<...>>>,
    cross_vm_transfers: &Arc<RwLock<HashMap<...>>>,
) -> MultivmResult<()> {
    // First lock acquisition
    {
        let mut state_guard = state.write().await;  // Lock 1
        state_guard.system_metrics.error_count += 1;
    }
    
    // Later in the same function...
    account_mappings.write().await  // Lock 2
        .insert(source_account.clone(), target_account.clone());
    
    // Even later...
    cross_vm_transfers.write().await  // Lock 3
        .insert(transfer_result.transfer_id.clone(), transfer_result.clone());
    
    // Finally...
    {
        let mut state_guard = state.write().await;  // Lock 1 again
        state_guard.blocks_processed += 1;
    }
}
```

**Risk**: If another thread acquires `cross_vm_transfers` lock first, then tries to acquire `state` lock, while this thread has `state` and wants `cross_vm_transfers`, a deadlock occurs.

## 2. Race Condition in Process Manager

**File**: `/crates/multivm-process-manager/src/manager.rs`

### Issue: Read-Modify-Write Race

```rust
// Lines 229-291 in restart_process()
pub async fn restart_process(&self, process_id: ProcessId) -> MultivmResult<()> {
    // Read lock acquired
    let processes = self.inner.processes.read().await;
    let handle = processes.get(&process_id).cloned()
        .ok_or_else(|| MultivmError::Process(...))?;
    drop(processes);  // Lock released
    
    // TIME GAP - Another thread could modify processes here!
    
    // Stop the process...
    self.stop_process_internal(&handle, true, Some(Duration::from_secs(10))).await?;
    
    // Write lock acquired much later
    self.inner.processes.write().await.remove(&process_id);
    
    // Another thread could have already removed or modified this process!
}
```

## 3. Connection Pool Race Condition

**File**: `/crates/multivm-process-manager/src/ipc/connection_manager.rs`

### Issue: State Check Outside Lock

```rust
// Lines 367-374
for (index, connection) in pool.iter_mut().enumerate() {
    if connection.is_healthy() && connection.state == ConnectionState::Connected {
        connection.state = ConnectionState::Busy;  // State change under iteration!
        return Ok(index);
    }
}
```

**Risk**: While iterating and checking health, another thread could modify the connection state, leading to returning an unhealthy connection.

## 4. Transaction Batcher Ordering Issue

**File**: `/crates/multivm-process-manager/src/transaction_batcher.rs`

### Issue: Multiple Queue Access Without Global Lock

```rust
pub struct TransactionBatcher {
    svm_priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
    evm_priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
    multivm_priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction>>>,
}

// When building a batch, locks are acquired separately:
async fn build_batch(&self) -> BatchResult {
    let mut svm_txs = self.svm_priority_queue.lock().await;
    // Process SVM transactions
    drop(svm_txs);
    
    let mut evm_txs = self.evm_priority_queue.lock().await;
    // Process EVM transactions
    drop(evm_txs);
    
    // Risk: Transactions could be reordered between queue accesses
}
```

## 5. Health Check Race Condition

**File**: `/crates/multivm-process-manager/src/manager.rs`

### Issue: TOCTOU (Time-of-Check-Time-of-Use)

```rust
// Lines 415-432 in health monitoring
let processes = processes_handle.read().await;
for (process_id, handle) in processes.iter() {
    let status = handle.health_check().await;  // This could take time
    if !status.is_healthy {
        // Process might have recovered or died completely by now
        health_monitor.update_last_check(*process_id);
    }
}
drop(processes);
```

## Recommended Fixes

### 1. Implement Lock Ordering

```rust
// Define global lock order
enum LockOrder {
    State = 1,
    AccountMappings = 2,
    CrossVmTransfers = 3,
}

// Always acquire in order
async fn safe_multi_lock_operation() {
    let state = state.write().await;
    let mappings = account_mappings.write().await;
    let transfers = cross_vm_transfers.write().await;
    // Do work...
}
```

### 2. Use Transactional Updates

```rust
// Instead of read-drop-write pattern
async fn restart_process_safe(&self, process_id: ProcessId) -> MultivmResult<()> {
    let mut processes = self.inner.processes.write().await;
    let handle = processes.get(&process_id).cloned()
        .ok_or_else(|| MultivmError::Process(...))?;
    
    // Do all work while holding the lock
    processes.remove(&process_id);
    drop(processes);
    
    // Now safely stop the process
    self.stop_process_internal(&handle, true, Some(Duration::from_secs(10))).await?;
}
```

### 3. Use Versioned Updates

```rust
struct VersionedConnection {
    version: AtomicU64,
    connection: ManagedConnection,
}

// Check version hasn't changed
let version = connection.version.load(Ordering::SeqCst);
if connection.is_healthy() {
    // Try to update with CAS
    if connection.version.compare_exchange(
        version, 
        version + 1, 
        Ordering::SeqCst,
        Ordering::SeqCst
    ).is_ok() {
        // Successfully claimed connection
    }
}
```

### 4. Single Queue with Type Discrimination

```rust
enum AnyTransaction {
    Svm(SvmTransaction),
    Evm(EvmTransaction),
    MultiVm(SpecialTransaction),
}

pub struct TransactionBatcher {
    // Single queue maintains global ordering
    priority_queue: Arc<Mutex<BinaryHeap<PrioritizedTransaction<AnyTransaction>>>>,
}
```

These detailed examples show the specific patterns that could lead to race conditions and deadlocks in the MultiVM codebase. Each issue has a concrete fix that maintains correctness while improving concurrency safety.