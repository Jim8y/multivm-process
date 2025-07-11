# Consensus Testing Checklist

This checklist ensures comprehensive testing of the MultiVM consensus system before deployment.

## Pre-Deployment Testing

### ✅ Unit Tests
- [ ] Leader selection module tests pass
  ```bash
  cargo test -p multivm-consensus leader_selection::tests
  ```
- [ ] Validator set management tests pass
  ```bash
  cargo test -p multivm-consensus validator_set::tests
  ```
- [ ] View change mechanism tests pass
  ```bash
  cargo test -p multivm-consensus view_change::tests
  ```
- [ ] Consensus manager tests pass
  ```bash
  cargo test -p multivm-consensus manager::tests
  ```

### ✅ Integration Tests
- [ ] Multi-validator integration tests pass
  ```bash
  cargo test consensus_integration_tests
  ```
- [ ] All integration scenarios verified:
  - [ ] Leader selection consistency
  - [ ] BFT voting thresholds
  - [ ] View change coordination
  - [ ] Transaction flow
  - [ ] Fault tolerance

### ✅ Live Network Tests
- [ ] 4-validator network functions correctly
  ```bash
  ./scripts/setup-validators.sh 4
  ./scripts/test-consensus-features.sh -t live -c 4
  ```
- [ ] 7-validator network scales properly
  ```bash
  ./scripts/setup-validators.sh 7
  ./scripts/test-consensus-features.sh -t live -c 7
  ```
- [ ] 10+ validator network performance acceptable
  ```bash
  ./scripts/setup-validators.sh 10
  ./scripts/validator-admin.sh benchmark
  ```

### ✅ Scenario Tests
- [ ] Run automated scenario tests
  ```bash
  ./scripts/run-consensus-scenarios.sh all
  ```
- [ ] Verify all scenarios pass:
  - [ ] Basic leader selection
  - [ ] BFT voting threshold
  - [ ] Leader timeout and view change
  - [ ] High transaction throughput
  - [ ] Network partition recovery
  - [ ] Byzantine fault tolerance
  - [ ] Validator reconfiguration
  - [ ] Scale performance

### ✅ Manual Verification

#### Leader Selection
- [ ] All validators agree on current leader
  ```bash
  ./scripts/validator-admin.sh leader
  ```
- [ ] Leader rotates correctly on view change
  ```bash
  ./scripts/validator-admin.sh rotate
  ```
- [ ] Leader selection is deterministic

#### BFT Properties
- [ ] Consensus requires 2/3+1 voting power
- [ ] System tolerates up to 1/3 Byzantine validators
- [ ] No consensus during 50-50 partition
- [ ] Majority partition continues operation

#### View Changes
- [ ] Automatic view change on leader timeout
- [ ] Manual view change works correctly
- [ ] View changes don't cause consensus forks
- [ ] System recovers from rapid view changes

#### Transaction Processing
- [ ] Transactions processed in order
- [ ] No transaction duplication
- [ ] Cross-VM transactions work correctly
- [ ] Transaction pool handles high load

#### State Management
- [ ] All validators maintain consistent state
- [ ] State recovery after validator restart
- [ ] Checkpoint creation and restoration
- [ ] No state divergence under load

### ✅ Performance Benchmarks
- [ ] Throughput meets requirements (>100 TPS)
  ```bash
  ./scripts/validator-admin.sh stress-test 1000
  ```
- [ ] Consensus latency acceptable (<5s)
- [ ] Resource usage within limits
- [ ] Network bandwidth reasonable

### ✅ Security Verification
- [ ] Double-spend prevention verified
- [ ] Message authentication working
- [ ] State tampering detected
- [ ] Proposal flooding handled

### ✅ Operational Readiness

#### Monitoring
- [ ] Health check endpoints responsive
  ```bash
  ./scripts/validator-admin.sh health
  ```
- [ ] Metrics collection working
- [ ] Log aggregation configured
- [ ] Alerting rules defined

#### Administration
- [ ] Validator addition process tested
- [ ] Validator removal process tested
- [ ] Configuration updates work
- [ ] Backup/restore procedures verified

#### Documentation
- [ ] Consensus documentation complete
- [ ] Deployment guide updated
- [ ] Troubleshooting guide available
- [ ] API documentation current

## Production Deployment Checklist

### ✅ Pre-Production Environment
- [ ] Deploy to staging environment
- [ ] Run full test suite in staging
- [ ] Verify monitoring and alerting
- [ ] Test disaster recovery procedures

### ✅ Network Configuration
- [ ] Firewall rules configured
- [ ] Load balancers set up
- [ ] DNS entries created
- [ ] SSL certificates installed

### ✅ Validator Setup
- [ ] Validator keys generated securely
- [ ] Initial validator set configured
- [ ] Genesis block agreed upon
- [ ] Bootstrap nodes identified

### ✅ Deployment Steps
1. [ ] Deploy validator nodes
2. [ ] Initialize with genesis configuration
3. [ ] Start validators in sequence
4. [ ] Verify consensus establishment
5. [ ] Enable transaction processing
6. [ ] Monitor initial operation

### ✅ Post-Deployment Verification
- [ ] All validators online and healthy
- [ ] Consensus producing blocks
- [ ] Transactions being processed
- [ ] Monitoring showing green status
- [ ] No errors in logs

## Rollback Plan

### ✅ Rollback Preparation
- [ ] Previous version binaries available
- [ ] State backup completed
- [ ] Rollback procedure documented
- [ ] Rollback tested in staging

### ✅ Rollback Triggers
- [ ] Consensus halted for >10 minutes
- [ ] State divergence detected
- [ ] Critical security issue found
- [ ] Performance degradation >50%

### ✅ Rollback Steps
1. [ ] Stop all validators
2. [ ] Restore previous binaries
3. [ ] Restore state from backup
4. [ ] Restart validators with old version
5. [ ] Verify consensus restored
6. [ ] Investigate root cause

## Sign-off

### ✅ Testing Sign-off
- [ ] QA Lead: _________________ Date: _______
- [ ] Dev Lead: _________________ Date: _______
- [ ] Security: _________________ Date: _______

### ✅ Deployment Sign-off
- [ ] Operations: _______________ Date: _______
- [ ] Product: _________________ Date: _______
- [ ] Management: ______________ Date: _______

## Notes

Use this section to document any issues, workarounds, or special considerations discovered during testing:

_____________________________________________________________________________
_____________________________________________________________________________
_____________________________________________________________________________
_____________________________________________________________________________

## Appendix: Quick Commands

### Start Test Network
```bash
# 4 validators (BFT minimum)
./scripts/setup-validators.sh 4 && cd /tmp/multivm-validators && ./start-all.sh

# 7 validators (recommended)
./scripts/setup-validators.sh 7 && cd /tmp/multivm-validators && ./start-all.sh
```

### Monitor Network
```bash
# Real-time monitoring
./scripts/validator-admin.sh monitor

# Health check
./scripts/validator-admin.sh health

# Current leader
./scripts/validator-admin.sh leader
```

### Test Consensus
```bash
# Submit test transaction
./scripts/validator-admin.sh submit-tx

# Run stress test
./scripts/validator-admin.sh stress-test 100

# Full test suite
./scripts/test-consensus-features.sh -t all
```

### Troubleshooting
```bash
# View logs
./scripts/validator-admin.sh logs validator_00

# Check configuration
./scripts/validator-admin.sh config validator_00

# Clean restart
./scripts/validator-admin.sh cleanup
./scripts/setup-validators.sh
```

---

*This checklist should be completed for each major release and retained for audit purposes.*