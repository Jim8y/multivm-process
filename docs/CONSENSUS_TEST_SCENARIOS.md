# Consensus Test Scenarios

This document outlines comprehensive test scenarios for the MultiVM consensus system, covering all aspects of the Byzantine Fault Tolerant (BFT) consensus implementation.

## Table of Contents

- [Test Categories](#test-categories)
- [Leader Selection Scenarios](#leader-selection-scenarios)
- [BFT Consensus Scenarios](#bft-consensus-scenarios)
- [View Change Scenarios](#view-change-scenarios)
- [Fault Tolerance Scenarios](#fault-tolerance-scenarios)
- [Performance Scenarios](#performance-scenarios)
- [Edge Case Scenarios](#edge-case-scenarios)
- [Security Scenarios](#security-scenarios)
- [Integration Scenarios](#integration-scenarios)

## Test Categories

### 1. Functional Tests
- Verify correct behavior under normal operations
- Test all consensus features work as designed
- Validate state transitions and message flows

### 2. Fault Tolerance Tests
- Byzantine validator behavior
- Network partitions and delays
- Leader failures and recovery

### 3. Performance Tests
- Throughput under various loads
- Latency measurements
- Scalability with validator count

### 4. Security Tests
- Attack resistance
- Message authentication
- State corruption prevention

## Leader Selection Scenarios

### Scenario 1: Basic Round-Robin Leader Selection
**Objective**: Verify deterministic round-robin leader selection across all validators

**Setup**:
- 4 validators with equal voting power (100 each)
- Clean network start

**Test Steps**:
1. Start all validators
2. Query current leader from each validator
3. Verify all validators agree on the same leader
4. Record leader for height 0, round 0
5. Trigger view changes to advance rounds
6. Verify leader rotation follows expected pattern

**Expected Results**:
- All validators report same leader at each round
- Leader rotation: validator_00 → validator_01 → validator_02 → validator_03 → validator_00
- Pattern is deterministic and repeatable

**Test Command**:
```bash
./test-consensus-features.sh -t leader -c 4
```

### Scenario 2: Leader Selection with Unequal Voting Power
**Objective**: Verify leader selection works correctly with different voting powers

**Setup**:
- 4 validators with different voting powers: 100, 200, 150, 50

**Test Steps**:
1. Initialize validators with different voting powers
2. Verify leader selection still follows round-robin
3. Confirm voting power doesn't affect leader rotation order

**Expected Results**:
- Leader selection remains deterministic
- Voting power only affects consensus threshold, not leader order

### Scenario 3: Dynamic Validator Set Changes
**Objective**: Test leader selection when validators join/leave

**Setup**:
- Start with 3 validators
- Add 4th validator mid-test
- Remove one validator

**Test Steps**:
1. Start with validators A, B, C
2. Verify leader rotation among 3
3. Add validator D
4. Verify leader rotation includes D
5. Remove validator B
6. Verify leader rotation adjusts correctly

**Expected Results**:
- Leader selection adapts to validator set changes
- No consensus interruption during changes

## BFT Consensus Scenarios

### Scenario 4: Minimum BFT Threshold
**Objective**: Verify 2/3+1 voting threshold enforcement

**Setup**:
- 4 validators (minimum for BFT)
- Total voting power: 400
- Required threshold: 267

**Test Steps**:
1. Submit proposal with only 2 validators voting (200 power)
2. Verify proposal is not committed
3. Add 3rd validator vote (300 power)
4. Verify proposal is committed

**Expected Results**:
- Proposals with <267 voting power are rejected
- Proposals with ≥267 voting power are accepted

**Test Command**:
```bash
./test-consensus-features.sh -t bft -c 4
```

### Scenario 5: Byzantine Validator Behavior
**Objective**: Test system tolerance to Byzantine validators

**Setup**:
- 7 validators total
- 2 Byzantine validators (less than 1/3)

**Test Steps**:
1. Configure 2 validators to send conflicting votes
2. Submit valid proposal from honest validator
3. Verify honest validators reach consensus
4. Verify Byzantine votes are ignored/logged

**Expected Results**:
- Consensus achieved with 5/7 honest validators
- Byzantine behavior logged but doesn't halt consensus

### Scenario 6: Large Validator Set BFT
**Objective**: Test BFT with many validators

**Setup**:
- 20 validators with equal voting power

**Test Steps**:
1. Initialize large validator set
2. Submit multiple proposals concurrently
3. Measure consensus achievement time
4. Verify all honest validators agree

**Expected Results**:
- Consensus scales to 20+ validators
- Performance remains acceptable

## View Change Scenarios

### Scenario 7: Leader Timeout View Change
**Objective**: Test automatic view change on leader failure

**Setup**:
- 4 validators with 30-second timeout
- Current leader: validator_00

**Test Steps**:
1. Stop current leader (validator_00)
2. Wait for timeout period
3. Verify other validators detect timeout
4. Verify view change messages are sent
5. Verify new leader (validator_01) is selected
6. Verify consensus continues

**Expected Results**:
- View change triggered after timeout
- New leader selected correctly
- Consensus resumes without manual intervention

**Test Script**:
```bash
#!/bin/bash
# Test view change on leader failure
./setup-validators.sh 4
cd /tmp/multivm-validators
./start-all.sh
sleep 10
./validator_00/stop.sh  # Stop leader
sleep 35  # Wait for timeout
./test-consensus.sh
```

### Scenario 8: Concurrent View Changes
**Objective**: Test handling of multiple simultaneous view changes

**Setup**:
- 5 validators
- Multiple validators timeout simultaneously

**Test Steps**:
1. Create network partition affecting 2 validators
2. Both partitions trigger view changes
3. Restore network connectivity
4. Verify consensus on single new view

**Expected Results**:
- Conflicting view changes resolved
- Network converges to single view
- No consensus fork

### Scenario 9: Rapid View Changes
**Objective**: Test system stability under frequent view changes

**Setup**:
- 4 validators with short timeout (5 seconds)

**Test Steps**:
1. Repeatedly stop/start leaders
2. Force rapid view changes
3. Submit transactions during changes
4. Verify transaction processing continues

**Expected Results**:
- System remains stable
- Transactions eventually processed
- No state corruption

## Fault Tolerance Scenarios

### Scenario 10: Network Partition (Even Split)
**Objective**: Test behavior under 50-50 network partition

**Setup**:
- 6 validators split into two groups of 3

**Test Steps**:
1. Create network partition
2. Verify neither partition can achieve consensus
3. Submit transactions to both partitions
4. Restore connectivity
5. Verify network converges

**Expected Results**:
- No consensus during even split
- Transactions queued but not processed
- Clean convergence after partition heals

### Scenario 11: Network Partition (Majority-Minority)
**Objective**: Test majority partition continues operation

**Setup**:
- 7 validators: 5 in majority, 2 in minority

**Test Steps**:
1. Create network partition
2. Verify majority partition continues consensus
3. Verify minority partition halts
4. Restore connectivity
5. Verify minority syncs with majority

**Expected Results**:
- Majority partition operates normally
- Minority partition cannot process blocks
- Successful reconciliation

### Scenario 12: Cascading Failures
**Objective**: Test gradual validator failures

**Setup**:
- 7 validators
- Fail validators one by one

**Test Steps**:
1. Start with 7 healthy validators
2. Fail validator 1 - verify consensus continues
3. Fail validator 2 - verify consensus continues
4. Fail validator 3 - verify consensus halts (4/7 remaining)
5. Restore one validator - verify consensus resumes

**Expected Results**:
- Consensus maintained with 5/7 validators
- Consensus halts at 4/7 validators
- Consensus resumes when threshold restored

## Performance Scenarios

### Scenario 13: High Transaction Throughput
**Objective**: Test maximum transaction processing rate

**Setup**:
- 7 validators
- 1000 transactions submitted rapidly

**Test Steps**:
1. Start validator network
2. Submit 1000 transactions from multiple clients
3. Measure time to process all transactions
4. Calculate transactions per second
5. Monitor resource usage

**Expected Results**:
- All transactions processed
- Throughput > 100 TPS
- No consensus stalls

**Test Command**:
```bash
./validator-admin.sh stress-test 1000
```

### Scenario 14: Large Block Consensus
**Objective**: Test consensus with maximum block size

**Setup**:
- 4 validators
- Blocks with 1000 transactions each

**Test Steps**:
1. Fill transaction pool with 1000 txs
2. Trigger block proposal
3. Measure consensus time for large block
4. Verify all validators commit same block

**Expected Results**:
- Large blocks achieve consensus
- Reasonable consensus time (<5 seconds)
- All validators synchronized

### Scenario 15: Validator Scalability
**Objective**: Test performance with increasing validator count

**Setup**:
- Test with 4, 7, 10, 15, 20 validators

**Test Steps**:
1. For each validator count:
   - Setup network
   - Submit 100 transactions
   - Measure consensus metrics
   - Record performance data
2. Plot performance vs validator count

**Expected Results**:
- Linear or sub-linear performance degradation
- Consensus viable up to 20+ validators

## Edge Case Scenarios

### Scenario 16: Clock Skew
**Objective**: Test consensus with validator clock differences

**Setup**:
- 4 validators
- Introduce ±5 second clock skew

**Test Steps**:
1. Adjust system clocks on validators
2. Verify consensus still achieved
3. Test view change timing
4. Verify no consensus forks

**Expected Results**:
- Consensus tolerates reasonable clock skew
- View changes handle timing differences

### Scenario 17: Resource Exhaustion
**Objective**: Test behavior under resource constraints

**Setup**:
- 4 validators
- Limit CPU/memory on one validator

**Test Steps**:
1. Constrain resources on validator_03
2. Submit normal transaction load
3. Verify constrained validator keeps up
4. Increase load until validator fails
5. Verify consensus continues without failed validator

**Expected Results**:
- Graceful degradation under load
- Consensus continues with healthy validators

### Scenario 18: Message Reordering
**Objective**: Test consensus with network message reordering

**Setup**:
- 4 validators
- Introduce artificial message delays

**Test Steps**:
1. Add random 0-2 second delays to messages
2. Verify consensus still achieved
3. Check for any consensus violations
4. Verify eventual consistency

**Expected Results**:
- Consensus robust to message reordering
- No safety violations
- Liveness maintained

## Security Scenarios

### Scenario 19: Double-Spend Attack
**Objective**: Verify double-spend prevention

**Setup**:
- 4 validators
- Malicious client attempting double-spend

**Test Steps**:
1. Submit transaction A spending coins
2. Wait for transaction inclusion
3. Submit transaction B double-spending same coins
4. Verify transaction B rejected
5. Check all validators agree on rejection

**Expected Results**:
- Double-spend detected and rejected
- Consistent state across validators

### Scenario 20: Proposal Flooding
**Objective**: Test resistance to proposal spam

**Setup**:
- 4 validators
- Malicious validator sending many proposals

**Test Steps**:
1. Configure validator_03 to send 100 proposals/second
2. Verify other validators handle load
3. Check consensus continues normally
4. Verify spam proposals rejected

**Expected Results**:
- Spam proposals don't halt consensus
- Rate limiting prevents resource exhaustion

### Scenario 21: State Manipulation
**Objective**: Test detection of state tampering

**Setup**:
- 4 validators
- Corrupt state on one validator

**Test Steps**:
1. Manually modify state database on validator_02
2. Trigger state verification
3. Verify corruption detected
4. Check validator excluded or resyncs
5. Verify consensus continues

**Expected Results**:
- State corruption detected
- Corrupted validator handled gracefully
- Network integrity maintained

## Integration Scenarios

### Scenario 22: Cross-VM Transaction Consensus
**Objective**: Test consensus for cross-VM transactions

**Setup**:
- 4 validators
- Transactions spanning EVM and SVM

**Test Steps**:
1. Submit EVM → SVM transfer transaction
2. Verify consensus on cross-VM state change
3. Submit concurrent cross-VM transactions
4. Verify atomic execution

**Expected Results**:
- Cross-VM transactions achieve consensus
- Atomic state updates across VMs
- No partial execution

### Scenario 23: Validator Hot-Swap
**Objective**: Test replacing validator without downtime

**Setup**:
- 4 validators running
- Prepare replacement for validator_02

**Test Steps**:
1. Start new validator_02_new with same ID
2. Sync state to new validator
3. Switch traffic to new validator
4. Stop old validator
5. Verify consensus uninterrupted

**Expected Results**:
- Seamless validator replacement
- No consensus interruption
- State consistency maintained

### Scenario 24: Multi-Region Deployment
**Objective**: Test consensus across geographic regions

**Setup**:
- 7 validators in 3 regions
- Introduce 50-200ms latency between regions

**Test Steps**:
1. Deploy validators across regions
2. Measure cross-region message latency
3. Submit transactions from each region
4. Verify global consensus achieved
5. Test region failure scenarios

**Expected Results**:
- Consensus works with geographic distribution
- Acceptable performance with latency
- Region failures handled correctly

## Test Execution Guide

### Automated Test Suite
Run all scenarios:
```bash
./test-consensus-features.sh -t all -c 7
```

### Manual Test Execution
For specific scenarios:
```bash
# Setup validators
./setup-validators.sh [count] [port] [voting_power]

# Run specific scenario
cd /tmp/multivm-validators
./start-all.sh
# Execute scenario steps
./test-consensus.sh
```

### Performance Monitoring
During tests, monitor:
- Block production rate
- Transaction throughput
- Consensus latency
- Resource usage
- Network traffic

### Results Documentation
For each scenario, document:
- Setup configuration
- Execution steps
- Observed behavior
- Performance metrics
- Any deviations from expected results

## Continuous Testing

### Daily Regression Tests
- Scenarios 1-6: Basic functionality
- Scenarios 7-9: View changes
- Scenario 13: Performance baseline

### Weekly Comprehensive Tests
- All scenarios
- Extended duration tests
- Stress testing

### Release Testing
- Full test suite
- Performance benchmarks
- Security audit scenarios
- Multi-region testing

## Test Infrastructure

### Required Resources
- Minimum 4 test machines/VMs
- Network control for partition tests
- Clock synchronization tools
- Resource monitoring tools

### Test Automation
- CI/CD integration
- Automated result collection
- Performance tracking
- Regression detection

## Conclusion

These comprehensive test scenarios ensure the MultiVM consensus system is:
- **Functionally correct** under normal operation
- **Byzantine fault tolerant** with up to 1/3 malicious validators  
- **Performance optimized** for production use
- **Secure** against common attacks
- **Robust** under adverse conditions

Regular execution of these scenarios provides confidence in the consensus system's reliability and readiness for production deployment.