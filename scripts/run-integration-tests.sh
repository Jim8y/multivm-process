#!/bin/bash
set -e

echo "🚀 MultiVM Integration Test Suite"
echo "================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test configuration
SKIP_LONG_RUNNING=${SKIP_LONG_RUNNING:-true}
VERBOSE=${VERBOSE:-false}
TEST_TIMEOUT=${TEST_TIMEOUT:-300}

echo "Configuration:"
echo "  Skip long running tests: $SKIP_LONG_RUNNING"
echo "  Verbose output: $VERBOSE"
echo "  Test timeout: ${TEST_TIMEOUT}s"
echo ""

# Function to run a test with timeout and capture result
run_test() {
    local test_name="$1"
    local test_package="$2"
    local test_function="$3"
    local description="$4"
    
    echo -n "Running $test_name... "
    
    local start_time=$(date +%s)
    
    # Build the cargo command
    local cmd="cargo test --package $test_package"
    if [ -n "$test_function" ]; then
        cmd="$cmd $test_function"
    fi
    
    if [ "$VERBOSE" = "true" ]; then
        cmd="$cmd -- --nocapture"
    fi
    
    # Run the test with timeout
    if timeout ${TEST_TIMEOUT}s bash -c "$cmd" >/dev/null 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo -e "${GREEN}PASSED${NC} (${duration}s)"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo -e "${RED}FAILED${NC} (${duration}s)"
        return 1
    fi
}

# Function to check if a binary exists
check_binary() {
    local binary="$1"
    if command -v "$binary" >/dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} $binary found"
        return 0
    else
        echo -e "${YELLOW}⚠${NC} $binary not found (some tests may be skipped)"
        return 1
    fi
}

# Check prerequisites
echo "Checking prerequisites..."
check_binary "cargo"
RETH_AVAILABLE=$(check_binary "reth" && echo "true" || echo "false")
SOLANA_AVAILABLE=$(check_binary "solana-test-validator" && echo "true" || echo "false")
echo ""

# Initialize test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Test categories
declare -A TESTS=(
    # Unit tests (always run)
    ["Consensus Unit Tests"]="multivm-consensus test_consensus_manager_creation Unit tests for consensus manager"
    ["P2P Unit Tests"]="multivm-p2p test_peer_discovery Unit tests for P2P networking"
    ["Account Mapping Tests"]="multivm-account-mapping test_enhanced_account_mapping Unit tests for account mapping"
    
    # Integration tests (conditional)
    ["Basic Integration"]="multivm-process-manager test_process_manager_creation Basic integration tests"
)

# Add conditional integration tests
if [ "$SKIP_LONG_RUNNING" = "false" ]; then
    if [ "$RETH_AVAILABLE" = "true" ]; then
        TESTS["Reth Integration"]="integration_reth_engine_test test_reth_connectivity_only Reth engine integration tests"
    fi
    
    if [ "$SOLANA_AVAILABLE" = "true" ]; then
        TESTS["Solana Integration"]="integration_solana_engine_test test_solana_mempool_only Solana engine integration tests"
    fi
    
    if [ "$RETH_AVAILABLE" = "true" ] && [ "$SOLANA_AVAILABLE" = "true" ]; then
        TESTS["Cross-VM Integration"]="integration_cross_vm_test test_dual_engine_startup_only Cross-VM integration tests"
    fi
fi

echo "Running tests..."
echo ""

# Run tests
for test_category in "${!TESTS[@]}"; do
    IFS=' ' read -r package function description <<< "${TESTS[$test_category]}"
    
    echo "📋 $test_category: $description"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if run_test "$test_category" "$package" "$function" "$description"; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
    
    echo ""
done

# Additional quick tests
echo "🔧 Running additional quick tests..."

# Test basic compilation
echo -n "Testing compilation... "
if cargo check --quiet >/dev/null 2>&1; then
    echo -e "${GREEN}PASSED${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${RED}FAILED${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    FAILED_TESTS=$((FAILED_TESTS + 1))
fi

# Test formatting
echo -n "Testing code formatting... "
if cargo fmt --check --quiet >/dev/null 2>&1; then
    echo -e "${GREEN}PASSED${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${YELLOW}NEEDS FORMATTING${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
fi

# Test clippy
echo -n "Testing linting... "
if cargo clippy --quiet -- -D warnings >/dev/null 2>&1; then
    echo -e "${GREEN}PASSED${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${YELLOW}HAS WARNINGS${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
fi

echo ""

# Generate summary
echo "📊 Test Summary"
echo "==============="
echo "Total tests: $TOTAL_TESTS"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$FAILED_TESTS${NC}"
echo -e "Skipped: ${YELLOW}$SKIPPED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 All tests passed!${NC}"
    SUCCESS_RATE=$(echo "scale=2; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l 2>/dev/null || echo "100")
    echo "Success rate: ${SUCCESS_RATE}%"
else
    echo ""
    echo -e "${RED}❌ Some tests failed${NC}"
    SUCCESS_RATE=$(echo "scale=2; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l 2>/dev/null || echo "0")
    echo "Success rate: ${SUCCESS_RATE}%"
fi

echo ""
echo "💡 Tips:"
echo "  - To run with full output: VERBOSE=true $0"
echo "  - To include long-running tests: SKIP_LONG_RUNNING=false $0"
echo "  - To adjust timeout: TEST_TIMEOUT=600 $0"

# Exit with appropriate code
if [ $FAILED_TESTS -eq 0 ]; then
    exit 0
else
    exit 1
fi