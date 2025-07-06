#!/bin/bash

# MultiVM Consensus Features Testing Script
# Comprehensive testing for leader selection, view changes, BFT consensus, and fault tolerance

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_DIR="/tmp/multivm-consensus-test"
DEFAULT_VALIDATOR_COUNT=4

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Functions
log_test() {
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -e "${BLUE}🧪 Test $TOTAL_TESTS: $1${NC}"
}

log_success() {
    PASSED_TESTS=$((PASSED_TESTS + 1))
    echo -e "${GREEN}   ✅ $1${NC}"
}

log_failure() {
    FAILED_TESTS=$((FAILED_TESTS + 1))
    echo -e "${RED}   ❌ $1${NC}"
}

log_info() {
    echo -e "${CYAN}   ℹ️  $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}   ⚠️  $1${NC}"
}

# Usage function
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -c, --count NUM       Number of validators (default: 4)"
    echo "  -t, --test TEST       Run specific test category:"
    echo "                        all, unit, integration, leader, bft, view-change, fault-tolerance"
    echo "  -v, --verbose         Verbose output"
    echo "  -h, --help           Show this help"
    echo ""
    echo "Examples:"
    echo "  $0                    # Run all tests with 4 validators"
    echo "  $0 -c 7              # Run all tests with 7 validators"
    echo "  $0 -t leader         # Run only leader selection tests"
    echo "  $0 -t bft -c 6       # Run BFT tests with 6 validators"
}

# Parse command line arguments
VALIDATOR_COUNT=$DEFAULT_VALIDATOR_COUNT
TEST_CATEGORY="all"
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--count)
            VALIDATOR_COUNT="$2"
            shift 2
            ;;
        -t|--test)
            TEST_CATEGORY="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Validate arguments
if [[ ! "$VALIDATOR_COUNT" =~ ^[0-9]+$ ]] || [ "$VALIDATOR_COUNT" -lt 1 ] || [ "$VALIDATOR_COUNT" -gt 20 ]; then
    echo -e "${RED}❌ Invalid validator count. Must be between 1 and 20.${NC}"
    exit 1
fi

# Header
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${PURPLE}                    MultiVM Consensus Features Test Suite                        ${NC}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "${CYAN}Configuration:${NC}"
echo "   Validators: $VALIDATOR_COUNT"
echo "   Test Category: $TEST_CATEGORY"
echo "   Verbose: $VERBOSE"
echo "   Test Directory: $TEST_DIR"
echo ""

# Create test directory
if [ -d "$TEST_DIR" ]; then
    rm -rf "$TEST_DIR"
fi
mkdir -p "$TEST_DIR"

# Unit Tests
run_unit_tests() {
    echo -e "${YELLOW}🧪 Running Unit Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "Leader Selection Unit Tests"
    cd "$PROJECT_ROOT"
    if cargo test -p multivm-consensus leader_selection::tests --quiet; then
        log_success "Leader selection tests passed"
    else
        log_failure "Leader selection tests failed"
    fi
    
    log_test "Validator Set Management Unit Tests"
    if cargo test -p multivm-consensus validator_set::tests --quiet; then
        log_success "Validator set tests passed"
    else
        log_failure "Validator set tests failed"
    fi
    
    log_test "View Change Mechanism Unit Tests"
    if cargo test -p multivm-consensus view_change::tests --quiet; then
        log_success "View change tests passed"
    else
        log_failure "View change tests failed"
    fi
    
    echo ""
}

# Integration Tests
run_integration_tests() {
    echo -e "${YELLOW}🔗 Running Integration Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "Multi-Validator Integration Tests"
    cd "$PROJECT_ROOT"
    if cargo test consensus_integration_tests --quiet; then
        log_success "Integration tests passed"
    else
        log_failure "Integration tests failed"
    fi
    
    echo ""
}

# Leader Selection Tests
run_leader_selection_tests() {
    echo -e "${YELLOW}👑 Running Leader Selection Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "Round-Robin Leader Selection Determinism"
    cd "$PROJECT_ROOT"
    if cargo test test_multi_validator_leader_selection --quiet; then
        log_success "Leader selection determinism verified"
    else
        log_failure "Leader selection determinism test failed"
    fi
    
    log_test "Leader Rotation Across Multiple Rounds"
    if cargo test test_leader_rotation_across_validators --quiet; then
        log_success "Leader rotation verified"
    else
        log_failure "Leader rotation test failed"
    fi
    
    echo ""
}

# BFT Consensus Tests
run_bft_tests() {
    echo -e "${YELLOW}🛡️  Running BFT Consensus Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "BFT Voting Thresholds (2/3 + 1)"
    cd "$PROJECT_ROOT"
    if cargo test test_bft_voting_threshold --quiet; then
        log_success "BFT voting thresholds verified"
    else
        log_failure "BFT voting threshold test failed"
    fi
    
    log_test "Byzantine Fault Tolerance"
    if cargo test test_consensus_fault_tolerance --quiet; then
        log_success "Byzantine fault tolerance verified"
    else
        log_failure "Byzantine fault tolerance test failed"
    fi
    
    # Calculate expected thresholds for current validator count
    TOTAL_POWER=$((VALIDATOR_COUNT * 100))
    REQUIRED_POWER=$(((TOTAL_POWER * 2) / 3 + 1))
    MAX_BYZANTINE=$(((VALIDATOR_COUNT - 1) / 3))
    
    log_info "Validator count: $VALIDATOR_COUNT"
    log_info "Total voting power: $TOTAL_POWER"
    log_info "Required threshold: $REQUIRED_POWER ($(echo "scale=1; $REQUIRED_POWER * 100 / $TOTAL_POWER" | bc -l)%)"
    log_info "Max Byzantine validators: $MAX_BYZANTINE"
    
    echo ""
}

# View Change Tests
run_view_change_tests() {
    echo -e "${YELLOW}🔄 Running View Change Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "View Change Coordination"
    cd "$PROJECT_ROOT"
    if cargo test test_view_change_integration --quiet; then
        log_success "View change coordination verified"
    else
        log_failure "View change coordination test failed"
    fi
    
    log_test "View Change Timeout Handling"
    if cargo test test_view_change_timeout --quiet; then
        log_success "View change timeout handling verified"
    else
        log_failure "View change timeout test failed"
    fi
    
    echo ""
}

# Fault Tolerance Tests
run_fault_tolerance_tests() {
    echo -e "${YELLOW}🚨 Running Fault Tolerance Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "Network Partition Simulation"
    cd "$PROJECT_ROOT"
    if cargo test test_network_partition_simulation --quiet; then
        log_success "Network partition handling verified"
    else
        log_failure "Network partition test failed"
    fi
    
    log_test "Validator Set Reconfiguration"
    if cargo test test_validator_set_reconfiguration --quiet; then
        log_success "Validator set reconfiguration verified"
    else
        log_failure "Validator set reconfiguration test failed"
    fi
    
    log_test "Performance with Many Validators"
    if cargo test test_performance_with_many_validators --quiet; then
        log_success "Performance with many validators verified"
    else
        log_failure "Performance test failed"
    fi
    
    echo ""
}

# Live Network Tests (requires actual validator setup)
run_live_network_tests() {
    echo -e "${YELLOW}🌐 Running Live Network Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Check if we have the setup script
    if [ ! -f "$SCRIPT_DIR/setup-validators.sh" ]; then
        log_failure "Validator setup script not found"
        return
    fi
    
    log_test "Setting up live validator network"
    
    # Setup validators
    if "$SCRIPT_DIR/setup-validators.sh" "$VALIDATOR_COUNT" 9000 100 > "$TEST_DIR/setup.log" 2>&1; then
        log_success "Validators setup completed"
    else
        log_failure "Validator setup failed"
        if [ "$VERBOSE" = true ]; then
            echo "Setup log:"
            cat "$TEST_DIR/setup.log"
        fi
        return
    fi
    
    VALIDATORS_DIR="/tmp/multivm-validators"
    
    # Start validators
    log_test "Starting validator network"
    if cd "$VALIDATORS_DIR" && ./start-all.sh > "$TEST_DIR/start.log" 2>&1; then
        log_success "Validators started"
    else
        log_failure "Failed to start validators"
        return
    fi
    
    # Wait for network to stabilize
    log_info "Waiting for network to stabilize..."
    sleep 15
    
    # Test network health
    log_test "Network health check"
    HEALTHY_COUNT=0
    for i in $(seq 0 $((VALIDATOR_COUNT - 1))); do
        API_PORT=$((9000 + i))
        if curl -s -m 5 "http://localhost:$API_PORT/health" >/dev/null 2>&1; then
            HEALTHY_COUNT=$((HEALTHY_COUNT + 1))
        fi
    done
    
    if [ "$HEALTHY_COUNT" -eq "$VALIDATOR_COUNT" ]; then
        log_success "All $VALIDATOR_COUNT validators are healthy"
    else
        log_failure "Only $HEALTHY_COUNT/$VALIDATOR_COUNT validators are healthy"
    fi
    
    # Test leader consistency
    log_test "Leader selection consistency"
    FIRST_PROPOSER=""
    CONSISTENT=true
    for i in $(seq 0 $((VALIDATOR_COUNT - 1))); do
        API_PORT=$((9000 + i))
        PROPOSER=$(curl -s -m 5 "http://localhost:$API_PORT/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "unknown"' 2>/dev/null || echo "unknown")
        
        if [ -z "$FIRST_PROPOSER" ]; then
            FIRST_PROPOSER="$PROPOSER"
        elif [ "$PROPOSER" != "$FIRST_PROPOSER" ]; then
            CONSISTENT=false
        fi
    done
    
    if [ "$CONSISTENT" = true ] && [ "$FIRST_PROPOSER" != "unknown" ]; then
        log_success "All validators agree on proposer: $FIRST_PROPOSER"
    else
        log_failure "Validators disagree on proposer"
    fi
    
    # Test transaction processing
    log_test "Transaction processing"
    FIRST_VALIDATOR_PORT=9000
    TX_RESPONSE=$(curl -s -m 5 -X POST "http://localhost:$FIRST_VALIDATOR_PORT/api/submit_transaction" \
        -H "Content-Type: application/json" \
        -d '{
            "id": "test_tx_'"$(date +%s)"'",
            "type": "evm",
            "sender": "0x1234567890123456789012345678901234567890",
            "to": "0x0987654321098765432109876543210987654321",
            "value": 1000000000000000000,
            "data": "0x",
            "nonce": 1
        }' 2>/dev/null)
    
    if echo "$TX_RESPONSE" | grep -q "success\|accepted\|submitted"; then
        log_success "Transaction submitted successfully"
    else
        log_failure "Transaction submission failed"
    fi
    
    # Test block generation
    log_test "Block generation"
    INITIAL_HEIGHT=$(curl -s -m 5 "http://localhost:$FIRST_VALIDATOR_PORT/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
    sleep 10
    FINAL_HEIGHT=$(curl -s -m 5 "http://localhost:$FIRST_VALIDATOR_PORT/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
    
    if [ "$FINAL_HEIGHT" -gt "$INITIAL_HEIGHT" ]; then
        log_success "Block generation verified (height: $INITIAL_HEIGHT → $FINAL_HEIGHT)"
    else
        log_failure "No block generation detected"
    fi
    
    # Cleanup
    log_info "Cleaning up validators..."
    cd "$VALIDATORS_DIR" && ./stop-all.sh > /dev/null 2>&1
    rm -rf "$VALIDATORS_DIR"
    
    echo ""
}

# Performance benchmarks
run_performance_tests() {
    echo -e "${YELLOW}⚡ Running Performance Tests${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    log_test "Consensus Initialization Performance"
    START_TIME=$(date +%s%N)
    cd "$PROJECT_ROOT"
    cargo test test_performance_with_many_validators --quiet > /dev/null 2>&1
    END_TIME=$(date +%s%N)
    DURATION=$(echo "scale=3; ($END_TIME - $START_TIME) / 1000000000" | bc -l)
    log_success "Performance test completed in ${DURATION}s"
    
    echo ""
}

# Main test execution
main() {
    case $TEST_CATEGORY in
        "all")
            run_unit_tests
            run_integration_tests
            run_leader_selection_tests
            run_bft_tests
            run_view_change_tests
            run_fault_tolerance_tests
            run_live_network_tests
            run_performance_tests
            ;;
        "unit")
            run_unit_tests
            ;;
        "integration")
            run_integration_tests
            ;;
        "leader")
            run_leader_selection_tests
            ;;
        "bft")
            run_bft_tests
            ;;
        "view-change")
            run_view_change_tests
            ;;
        "fault-tolerance")
            run_fault_tolerance_tests
            ;;
        "live")
            run_live_network_tests
            ;;
        "performance")
            run_performance_tests
            ;;
        *)
            echo -e "${RED}❌ Unknown test category: $TEST_CATEGORY${NC}"
            usage
            exit 1
            ;;
    esac
    
    # Test summary
    echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${PURPLE}                              Test Summary                                     ${NC}"
    echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}Test Results:${NC}"
    echo "   Total tests: $TOTAL_TESTS"
    echo -e "   ${GREEN}Passed: $PASSED_TESTS${NC}"
    echo -e "   ${RED}Failed: $FAILED_TESTS${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "   ${GREEN}Success rate: 100%${NC}"
        echo ""
        echo -e "${GREEN}🎉 All tests passed! Consensus system is working correctly.${NC}"
        exit 0
    else
        SUCCESS_RATE=$(echo "scale=1; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l)
        echo -e "   ${YELLOW}Success rate: ${SUCCESS_RATE}%${NC}"
        echo ""
        echo -e "${RED}⚠️  Some tests failed. Please review the output above.${NC}"
        exit 1
    fi
}

# Check dependencies
if ! command -v jq &> /dev/null; then
    echo -e "${YELLOW}⚠️  jq not found. Installing...${NC}"
    if command -v apt-get &> /dev/null; then
        sudo apt-get update && sudo apt-get install -y jq
    elif command -v yum &> /dev/null; then
        sudo yum install -y jq
    elif command -v brew &> /dev/null; then
        brew install jq
    else
        echo -e "${RED}❌ Could not install jq. Please install it manually.${NC}"
        exit 1
    fi
fi

if ! command -v bc &> /dev/null; then
    echo -e "${YELLOW}⚠️  bc not found. Installing...${NC}"
    if command -v apt-get &> /dev/null; then
        sudo apt-get update && sudo apt-get install -y bc
    elif command -v yum &> /dev/null; then
        sudo yum install -y bc
    elif command -v brew &> /dev/null; then
        brew install bc
    else
        echo -e "${RED}❌ Could not install bc. Please install it manually.${NC}"
        exit 1
    fi
fi

# Run tests
main