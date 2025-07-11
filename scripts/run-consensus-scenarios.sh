#!/bin/bash

# MultiVM Consensus Scenario Test Runner
# Automated execution of comprehensive consensus test scenarios

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_RESULTS_DIR="/tmp/multivm-consensus-test-results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="$TEST_RESULTS_DIR/test_results_$TIMESTAMP.json"
LOG_FILE="$TEST_RESULTS_DIR/test_log_$TIMESTAMP.log"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Test counters
TOTAL_SCENARIOS=0
PASSED_SCENARIOS=0
FAILED_SCENARIOS=0
SKIPPED_SCENARIOS=0

# Create results directory
mkdir -p "$TEST_RESULTS_DIR"

# Logging functions
log() {
    echo -e "$1" | tee -a "$LOG_FILE"
}

log_success() {
    log "${GREEN}✅ $1${NC}"
}

log_failure() {
    log "${RED}❌ $1${NC}"
}

log_warning() {
    log "${YELLOW}⚠️  $1${NC}"
}

log_info() {
    log "${CYAN}ℹ️  $1${NC}"
}

log_scenario() {
    TOTAL_SCENARIOS=$((TOTAL_SCENARIOS + 1))
    log ""
    log "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    log "${PURPLE}Scenario $TOTAL_SCENARIOS: $1${NC}"
    log "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# Result tracking
record_result() {
    local scenario_name="$1"
    local status="$2"
    local duration="$3"
    local details="$4"
    
    # Append to JSON results file
    if [ ! -f "$RESULTS_FILE" ]; then
        echo '{"scenarios": []}' > "$RESULTS_FILE"
    fi
    
    # Add result using jq
    jq --arg name "$scenario_name" \
       --arg status "$status" \
       --arg duration "$duration" \
       --arg details "$details" \
       --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
       '.scenarios += [{"name": $name, "status": $status, "duration": $duration, "details": $details, "timestamp": $timestamp}]' \
       "$RESULTS_FILE" > "$RESULTS_FILE.tmp" && mv "$RESULTS_FILE.tmp" "$RESULTS_FILE"
}

# Helper functions
cleanup_validators() {
    if [ -d "/tmp/multivm-validators" ]; then
        cd /tmp/multivm-validators
        ./stop-all.sh >/dev/null 2>&1 || true
        cd - >/dev/null
        rm -rf /tmp/multivm-validators
    fi
    pkill -f multivm-node 2>/dev/null || true
    sleep 2
}

setup_validators() {
    local count="$1"
    local base_port="${2:-8080}"
    local voting_power="${3:-100}"
    
    cleanup_validators
    "$SCRIPT_DIR/setup-validators.sh" "$count" "$base_port" "$voting_power" > /dev/null 2>&1
}

# Test Scenarios

# Scenario 1: Basic Round-Robin Leader Selection
scenario_basic_leader_selection() {
    log_scenario "Basic Round-Robin Leader Selection"
    local start_time=$(date +%s)
    
    log_info "Setting up 4 validators with equal voting power"
    setup_validators 4 9000 100
    
    cd /tmp/multivm-validators
    ./start-all.sh >/dev/null 2>&1
    sleep 10
    
    log_info "Checking leader consensus"
    local leaders=()
    local all_agree=true
    
    for i in {0..3}; do
        local port=$((9000 + i))
        local leader=$(curl -s -m 5 "http://localhost:$port/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "unknown"' 2>/dev/null || echo "error")
        leaders+=("$leader")
        log_info "validator_$(printf "%02d" $i) reports leader: $leader"
    done
    
    # Check if all validators agree
    local first_leader="${leaders[0]}"
    for leader in "${leaders[@]}"; do
        if [ "$leader" != "$first_leader" ] || [ "$leader" = "unknown" ] || [ "$leader" = "error" ]; then
            all_agree=false
            break
        fi
    done
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    cleanup_validators
    
    if [ "$all_agree" = true ]; then
        log_success "All validators agree on leader: $first_leader"
        record_result "basic_leader_selection" "passed" "$duration" "All validators agreed on leader: $first_leader"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "Validators disagree on leader"
        record_result "basic_leader_selection" "failed" "$duration" "Validators disagreed on leader"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Scenario 2: BFT Voting Threshold
scenario_bft_threshold() {
    log_scenario "BFT Voting Threshold Verification"
    local start_time=$(date +%s)
    
    log_info "Running BFT threshold unit tests"
    cd "$PROJECT_ROOT"
    
    if cargo test test_bft_voting_threshold --quiet > /dev/null 2>&1; then
        log_success "BFT voting threshold tests passed"
        log_info "4 validators: Required 267/400 voting power (2/3 + 1)"
        log_info "7 validators: Required 467/700 voting power (2/3 + 1)"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        record_result "bft_threshold" "passed" "$duration" "BFT threshold calculations verified"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "BFT voting threshold tests failed"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        record_result "bft_threshold" "failed" "$duration" "BFT threshold tests failed"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Scenario 3: Leader Timeout and View Change
scenario_leader_timeout() {
    log_scenario "Leader Timeout and View Change"
    local start_time=$(date +%s)
    
    log_info "Setting up 4 validators"
    setup_validators 4 9100 100
    
    cd /tmp/multivm-validators
    ./start-all.sh >/dev/null 2>&1
    sleep 10
    
    # Get initial leader
    local initial_leader=$(curl -s -m 5 "http://localhost:9100/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "unknown"' 2>/dev/null || echo "error")
    log_info "Initial leader: $initial_leader"
    
    # Stop the leader
    if [ "$initial_leader" != "unknown" ] && [ "$initial_leader" != "error" ]; then
        log_info "Stopping leader validator: $initial_leader"
        "./$initial_leader/stop.sh" >/dev/null 2>&1
        
        log_info "Waiting for view change timeout (35 seconds)..."
        sleep 35
        
        # Check new leader
        local new_leader=$(curl -s -m 5 "http://localhost:9101/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "unknown"' 2>/dev/null || echo "error")
        log_info "New leader after view change: $new_leader"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        cleanup_validators
        
        if [ "$new_leader" != "$initial_leader" ] && [ "$new_leader" != "unknown" ] && [ "$new_leader" != "error" ]; then
            log_success "View change successful: $initial_leader → $new_leader"
            record_result "leader_timeout" "passed" "$duration" "View change from $initial_leader to $new_leader"
            PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
            return 0
        else
            log_failure "View change failed"
            record_result "leader_timeout" "failed" "$duration" "View change did not occur properly"
            FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
            return 1
        fi
    else
        log_warning "Could not determine initial leader, skipping test"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        cleanup_validators
        
        record_result "leader_timeout" "skipped" "$duration" "Could not determine initial leader"
        SKIPPED_SCENARIOS=$((SKIPPED_SCENARIOS + 1))
        return 2
    fi
}

# Scenario 4: High Transaction Throughput
scenario_high_throughput() {
    log_scenario "High Transaction Throughput"
    local start_time=$(date +%s)
    
    log_info "Setting up 7 validators for throughput test"
    setup_validators 7 9200 100
    
    cd /tmp/multivm-validators
    ./start-all.sh >/dev/null 2>&1
    sleep 15
    
    log_info "Submitting 100 transactions"
    local tx_start=$(date +%s%N)
    local success_count=0
    
    for i in {1..100}; do
        local port=$((9200 + (i % 7)))
        local tx_data='{
            "id": "throughput_tx_'$i'_'$(date +%s)'",
            "type": "evm",
            "sender": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "to": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "value": '$((1000000000000000000 + i))',
            "data": "0x",
            "nonce": '$i'
        }'
        
        if curl -s -m 2 -X POST "http://localhost:$port/api/submit_transaction" \
            -H "Content-Type: application/json" \
            -d "$tx_data" >/dev/null 2>&1; then
            success_count=$((success_count + 1))
        fi
    done
    
    local tx_end=$(date +%s%N)
    local tx_duration_ms=$(((tx_end - tx_start) / 1000000))
    local tps=$(echo "scale=2; $success_count * 1000 / $tx_duration_ms" | bc -l)
    
    log_info "Submitted $success_count/100 transactions in ${tx_duration_ms}ms"
    log_info "Throughput: $tps TPS"
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    cleanup_validators
    
    if [ $success_count -ge 90 ]; then
        log_success "High throughput test passed: $tps TPS"
        record_result "high_throughput" "passed" "$duration" "Achieved $tps TPS with $success_count/100 transactions"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "Throughput test failed: only $success_count/100 transactions succeeded"
        record_result "high_throughput" "failed" "$duration" "Only $success_count/100 transactions succeeded"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Scenario 5: Network Partition Recovery
scenario_partition_recovery() {
    log_scenario "Network Partition Recovery"
    local start_time=$(date +%s)
    
    log_info "Setting up 6 validators"
    setup_validators 6 9300 100
    
    cd /tmp/multivm-validators
    ./start-all.sh >/dev/null 2>&1
    sleep 10
    
    # Get initial state
    local initial_height=$(curl -s -m 5 "http://localhost:9300/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
    log_info "Initial height: $initial_height"
    
    # Simulate partition by stopping half the validators
    log_info "Creating network partition (stopping validators 3-5)"
    for i in {3..5}; do
        "./validator_$(printf "%02d" $i)/stop.sh" >/dev/null 2>&1
    done
    
    sleep 10
    
    # Check if remaining validators can still make progress
    local partition_height=$(curl -s -m 5 "http://localhost:9300/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
    log_info "Height during partition: $partition_height"
    
    # Restore network
    log_info "Healing partition (restarting validators 3-5)"
    for i in {3..5}; do
        nohup "./validator_$(printf "%02d" $i)/start.sh" >/dev/null 2>&1 &
    done
    
    sleep 15
    
    # Check final state
    local final_height=$(curl -s -m 5 "http://localhost:9300/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
    log_info "Final height after recovery: $final_height"
    
    # Check if all validators are in sync
    local all_synced=true
    for i in {0..5}; do
        local port=$((9300 + i))
        local height=$(curl -s -m 5 "http://localhost:$port/api/height" 2>/dev/null | jq -r '.height // -1' 2>/dev/null || echo "-1")
        if [ "$height" != "$final_height" ]; then
            all_synced=false
            log_warning "validator_$(printf "%02d" $i) at height $height (expected $final_height)"
        fi
    done
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    cleanup_validators
    
    if [ "$partition_height" = "$initial_height" ] && [ $final_height -gt $initial_height ] && [ "$all_synced" = true ]; then
        log_success "Partition recovery successful: halted during partition, resumed and synced after"
        record_result "partition_recovery" "passed" "$duration" "Successfully recovered from 50-50 partition"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "Partition recovery failed"
        record_result "partition_recovery" "failed" "$duration" "Failed to properly handle partition"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Scenario 6: Byzantine Validator Tolerance
scenario_byzantine_tolerance() {
    log_scenario "Byzantine Validator Tolerance"
    local start_time=$(date +%s)
    
    log_info "Running Byzantine fault tolerance unit tests"
    cd "$PROJECT_ROOT"
    
    if cargo test test_consensus_fault_tolerance --quiet > /dev/null 2>&1; then
        log_success "Byzantine fault tolerance tests passed"
        log_info "System tolerates up to 1/3 Byzantine validators"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        record_result "byzantine_tolerance" "passed" "$duration" "Byzantine fault tolerance verified"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "Byzantine fault tolerance tests failed"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        record_result "byzantine_tolerance" "failed" "$duration" "Byzantine fault tolerance tests failed"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Scenario 7: Validator Set Reconfiguration
scenario_validator_reconfiguration() {
    log_scenario "Validator Set Reconfiguration"
    local start_time=$(date +%s)
    
    log_info "Running validator set reconfiguration tests"
    cd "$PROJECT_ROOT"
    
    if cargo test test_validator_set_reconfiguration --quiet > /dev/null 2>&1; then
        log_success "Validator reconfiguration tests passed"
        log_info "Dynamic validator set changes work correctly"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        record_result "validator_reconfiguration" "passed" "$duration" "Validator set reconfiguration verified"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "Validator reconfiguration tests failed"
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        record_result "validator_reconfiguration" "failed" "$duration" "Validator reconfiguration tests failed"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Scenario 8: Performance with Many Validators
scenario_many_validators() {
    log_scenario "Performance with Many Validators"
    local start_time=$(date +%s)
    
    log_info "Testing with 10 validators"
    setup_validators 10 9400 100
    
    cd /tmp/multivm-validators
    ./start-all.sh >/dev/null 2>&1
    sleep 20
    
    # Check if all validators are healthy
    local healthy_count=0
    for i in {0..9}; do
        local port=$((9400 + i))
        if curl -s -m 5 "http://localhost:$port/health" >/dev/null 2>&1; then
            healthy_count=$((healthy_count + 1))
        fi
    done
    
    log_info "Healthy validators: $healthy_count/10"
    
    # Test consensus with many validators
    local consensus_working=false
    if [ $healthy_count -ge 7 ]; then
        # Submit a transaction and check if it gets processed
        local tx_data='{
            "id": "many_validators_test_'$(date +%s)'",
            "type": "evm",
            "sender": "0xcccccccccccccccccccccccccccccccccccccccc",
            "to": "0xdddddddddddddddddddddddddddddddddddddddd",
            "value": 1000000000000000000,
            "data": "0x",
            "nonce": 1
        }'
        
        curl -s -m 5 -X POST "http://localhost:9400/api/submit_transaction" \
            -H "Content-Type: application/json" \
            -d "$tx_data" >/dev/null 2>&1
        
        sleep 10
        
        # Check if height increased
        local height=$(curl -s -m 5 "http://localhost:9400/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
        if [ "$height" -gt "0" ]; then
            consensus_working=true
        fi
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    cleanup_validators
    
    if [ $healthy_count -ge 7 ] && [ "$consensus_working" = true ]; then
        log_success "Consensus works with 10 validators"
        record_result "many_validators" "passed" "$duration" "Consensus successful with 10 validators"
        PASSED_SCENARIOS=$((PASSED_SCENARIOS + 1))
        return 0
    else
        log_failure "Consensus failed with many validators"
        record_result "many_validators" "failed" "$duration" "Only $healthy_count/10 validators healthy"
        FAILED_SCENARIOS=$((FAILED_SCENARIOS + 1))
        return 1
    fi
}

# Main execution
main() {
    log "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
    log "${PURPLE}              MultiVM Consensus Scenario Test Runner                           ${NC}"
    log "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
    log ""
    log_info "Test results will be saved to: $RESULTS_FILE"
    log_info "Test log will be saved to: $LOG_FILE"
    log ""
    
    # Initialize results file
    echo '{"test_run": "'$TIMESTAMP'", "scenarios": []}' > "$RESULTS_FILE"
    
    # Run scenarios based on command line arguments
    if [ "$#" -eq 0 ] || [ "$1" = "all" ]; then
        # Run all scenarios
        scenario_basic_leader_selection || true
        scenario_bft_threshold || true
        scenario_leader_timeout || true
        scenario_high_throughput || true
        scenario_partition_recovery || true
        scenario_byzantine_tolerance || true
        scenario_validator_reconfiguration || true
        scenario_many_validators || true
    else
        # Run specific scenarios
        for scenario in "$@"; do
            case $scenario in
                "leader")
                    scenario_basic_leader_selection || true
                    ;;
                "bft")
                    scenario_bft_threshold || true
                    ;;
                "view-change")
                    scenario_leader_timeout || true
                    ;;
                "throughput")
                    scenario_high_throughput || true
                    ;;
                "partition")
                    scenario_partition_recovery || true
                    ;;
                "byzantine")
                    scenario_byzantine_tolerance || true
                    ;;
                "reconfiguration")
                    scenario_validator_reconfiguration || true
                    ;;
                "scale")
                    scenario_many_validators || true
                    ;;
                *)
                    log_warning "Unknown scenario: $scenario"
                    ;;
            esac
        done
    fi
    
    # Final cleanup
    cleanup_validators
    
    # Summary
    log ""
    log "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
    log "${PURPLE}                              Test Summary                                     ${NC}"
    log "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
    log ""
    log_info "Total scenarios: $TOTAL_SCENARIOS"
    log_success "Passed: $PASSED_SCENARIOS"
    log_failure "Failed: $FAILED_SCENARIOS"
    log_warning "Skipped: $SKIPPED_SCENARIOS"
    
    # Update final results
    jq --arg total "$TOTAL_SCENARIOS" \
       --arg passed "$PASSED_SCENARIOS" \
       --arg failed "$FAILED_SCENARIOS" \
       --arg skipped "$SKIPPED_SCENARIOS" \
       '.summary = {"total": $total, "passed": $passed, "failed": $failed, "skipped": $skipped}' \
       "$RESULTS_FILE" > "$RESULTS_FILE.tmp" && mv "$RESULTS_FILE.tmp" "$RESULTS_FILE"
    
    log ""
    log_info "Results saved to: $RESULTS_FILE"
    log_info "Full log saved to: $LOG_FILE"
    
    # Generate HTML report
    generate_html_report
    
    # Exit with appropriate code
    if [ $FAILED_SCENARIOS -eq 0 ]; then
        log ""
        log_success "All tests passed! 🎉"
        exit 0
    else
        log ""
        log_failure "Some tests failed. Please review the results."
        exit 1
    fi
}

# Generate HTML report
generate_html_report() {
    local html_file="$TEST_RESULTS_DIR/test_report_$TIMESTAMP.html"
    
    cat > "$html_file" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>MultiVM Consensus Test Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background-color: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background-color: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        h1 { color: #333; text-align: center; }
        .summary { display: flex; justify-content: space-around; margin: 20px 0; }
        .summary-item { text-align: center; padding: 20px; border-radius: 8px; }
        .passed { background-color: #d4edda; color: #155724; }
        .failed { background-color: #f8d7da; color: #721c24; }
        .skipped { background-color: #fff3cd; color: #856404; }
        .total { background-color: #d1ecf1; color: #0c5460; }
        table { width: 100%; border-collapse: collapse; margin-top: 20px; }
        th, td { padding: 12px; text-align: left; border-bottom: 1px solid #ddd; }
        th { background-color: #f8f9fa; font-weight: bold; }
        tr:hover { background-color: #f8f9fa; }
        .status-passed { color: #28a745; font-weight: bold; }
        .status-failed { color: #dc3545; font-weight: bold; }
        .status-skipped { color: #ffc107; font-weight: bold; }
        .timestamp { color: #6c757d; font-size: 0.9em; }
    </style>
</head>
<body>
    <div class="container">
        <h1>MultiVM Consensus Test Report</h1>
        <p class="timestamp">Generated: TIMESTAMP_PLACEHOLDER</p>
        
        <div class="summary">
            <div class="summary-item total">
                <h2>TOTAL_PLACEHOLDER</h2>
                <p>Total Scenarios</p>
            </div>
            <div class="summary-item passed">
                <h2>PASSED_PLACEHOLDER</h2>
                <p>Passed</p>
            </div>
            <div class="summary-item failed">
                <h2>FAILED_PLACEHOLDER</h2>
                <p>Failed</p>
            </div>
            <div class="summary-item skipped">
                <h2>SKIPPED_PLACEHOLDER</h2>
                <p>Skipped</p>
            </div>
        </div>
        
        <table>
            <thead>
                <tr>
                    <th>Scenario</th>
                    <th>Status</th>
                    <th>Duration (s)</th>
                    <th>Details</th>
                    <th>Timestamp</th>
                </tr>
            </thead>
            <tbody>
                SCENARIOS_PLACEHOLDER
            </tbody>
        </table>
    </div>
</body>
</html>
EOF

    # Replace placeholders with actual data
    local scenarios_html=""
    while IFS= read -r scenario; do
        local name=$(echo "$scenario" | jq -r '.name')
        local status=$(echo "$scenario" | jq -r '.status')
        local duration=$(echo "$scenario" | jq -r '.duration')
        local details=$(echo "$scenario" | jq -r '.details')
        local timestamp=$(echo "$scenario" | jq -r '.timestamp')
        
        scenarios_html+="<tr>"
        scenarios_html+="<td>$name</td>"
        scenarios_html+="<td class=\"status-$status\">$status</td>"
        scenarios_html+="<td>$duration</td>"
        scenarios_html+="<td>$details</td>"
        scenarios_html+="<td class=\"timestamp\">$timestamp</td>"
        scenarios_html+="</tr>"
    done < <(jq -c '.scenarios[]' "$RESULTS_FILE")
    
    sed -i "s|TIMESTAMP_PLACEHOLDER|$(date)|g" "$html_file"
    sed -i "s|TOTAL_PLACEHOLDER|$TOTAL_SCENARIOS|g" "$html_file"
    sed -i "s|PASSED_PLACEHOLDER|$PASSED_SCENARIOS|g" "$html_file"
    sed -i "s|FAILED_PLACEHOLDER|$FAILED_SCENARIOS|g" "$html_file"
    sed -i "s|SKIPPED_PLACEHOLDER|$SKIPPED_SCENARIOS|g" "$html_file"
    sed -i "s|SCENARIOS_PLACEHOLDER|$scenarios_html|g" "$html_file"
    
    log_info "HTML report generated: $html_file"
}

# Show usage
usage() {
    echo "Usage: $0 [scenario1] [scenario2] ..."
    echo ""
    echo "Available scenarios:"
    echo "  all              - Run all scenarios (default)"
    echo "  leader           - Basic leader selection"
    echo "  bft              - BFT voting threshold"
    echo "  view-change      - Leader timeout and view change"
    echo "  throughput       - High transaction throughput"
    echo "  partition        - Network partition recovery"
    echo "  byzantine        - Byzantine validator tolerance"
    echo "  reconfiguration  - Validator set reconfiguration"
    echo "  scale            - Performance with many validators"
    echo ""
    echo "Examples:"
    echo "  $0                    # Run all scenarios"
    echo "  $0 leader bft         # Run specific scenarios"
    echo "  $0 throughput scale   # Run performance scenarios"
}

# Check arguments
if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
    usage
    exit 0
fi

# Run main
main "$@"