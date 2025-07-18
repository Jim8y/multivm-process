#!/bin/bash
set -e

echo "🚀 MultiVM Integration Test Suite (Custom Forks)"
echo "==============================================="
echo "Using custom forks:"
echo "  - Reth: git@github.com:vm-multiverse/reth.git (dev branch)"
echo "  - Solana: git@github.com:vm-multiverse/multivm-agave.git (master branch)"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if custom binaries are available
CUSTOM_BINARIES_CONFIG="/tmp/multivm-test-binaries/test-binaries.env"
if [ -f "$CUSTOM_BINARIES_CONFIG" ]; then
    echo "Loading custom binary configuration..."
    source "$CUSTOM_BINARIES_CONFIG"
fi

# Override binary paths with custom ones if available
if [ -n "$RETH_BINARY" ] && [ -f "$RETH_BINARY" ]; then
    export PATH="$(dirname $RETH_BINARY):$PATH"
    echo -e "${GREEN}✅ Using custom Reth binary: $RETH_BINARY${NC}"
else
    echo -e "${YELLOW}⚠ Custom Reth binary not found, checking system PATH${NC}"
fi

if [ -n "$SOLANA_TEST_VALIDATOR_BINARY" ] && [ -f "$SOLANA_TEST_VALIDATOR_BINARY" ]; then
    export PATH="$(dirname $SOLANA_TEST_VALIDATOR_BINARY):$PATH"
    # Create a symlink so tests can find it as solana-test-validator
    mkdir -p /tmp/multivm-test-bin
    ln -sf "$SOLANA_TEST_VALIDATOR_BINARY" /tmp/multivm-test-bin/solana-test-validator
    export PATH="/tmp/multivm-test-bin:$PATH"
    echo -e "${GREEN}✅ Using custom Solana binary: $SOLANA_TEST_VALIDATOR_BINARY${NC}"
else
    echo -e "${YELLOW}⚠ Custom Solana binary not found, checking system PATH${NC}"
fi

# Test configuration
SKIP_LONG_RUNNING=${SKIP_LONG_RUNNING:-false}  # Default to running all tests
VERBOSE=${VERBOSE:-true}
TEST_TIMEOUT=${TEST_TIMEOUT:-600}  # 10 minutes for integration tests
RUN_QUICK_TESTS=${RUN_QUICK_TESTS:-true}

echo ""
echo "Configuration:"
echo "  Skip long running tests: $SKIP_LONG_RUNNING"
echo "  Verbose output: $VERBOSE"
echo "  Test timeout: ${TEST_TIMEOUT}s"
echo "  Run quick tests: $RUN_QUICK_TESTS"
echo ""

# Function to check if a binary is available
check_binary() {
    local binary="$1"
    if command -v "$binary" >/dev/null 2>&1; then
        local version=$(command -v "$binary")
        echo -e "${GREEN}✅ $binary found at: $version${NC}"
        
        # Show version info if available
        if [[ "$binary" == "reth" ]]; then
            $binary --version 2>/dev/null || true
        elif [[ "$binary" == "solana-test-validator" ]]; then
            $binary --version 2>/dev/null || true
        fi
        
        return 0
    else
        echo -e "${RED}❌ $binary not found${NC}"
        return 1
    fi
}

# Check prerequisites
echo "Checking prerequisites..."
echo "========================"
check_binary "cargo"
RETH_AVAILABLE=$(check_binary "reth" && echo "true" || echo "false")
SOLANA_AVAILABLE=$(check_binary "solana-test-validator" && echo "true" || echo "false")
echo ""

# If binaries are not available, offer to set them up
if [ "$RETH_AVAILABLE" = "false" ] || [ "$SOLANA_AVAILABLE" = "false" ]; then
    echo -e "${YELLOW}⚠ Some binaries are missing. Would you like to set them up from custom forks?${NC}"
    echo "This will clone and build the custom repositories (may take 10-30 minutes)"
    read -p "Setup custom binaries? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Running setup script..."
        bash "$(dirname "$0")/setup-test-binaries.sh"
        
        # Reload configuration
        if [ -f "$CUSTOM_BINARIES_CONFIG" ]; then
            source "$CUSTOM_BINARIES_CONFIG"
        fi
        
        # Re-check binaries
        RETH_AVAILABLE=$(check_binary "reth" && echo "true" || echo "false")
        SOLANA_AVAILABLE=$(check_binary "solana-test-validator" && echo "true" || echo "false")
    fi
fi

# Initialize test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Function to run a test with proper output
run_test() {
    local test_name="$1"
    local test_command="$2"
    local description="$3"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}📋 Test: $test_name${NC}"
    echo -e "${BLUE}   Description: $description${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    
    local start_time=$(date +%s)
    
    # Create a temporary file for output
    local output_file="/tmp/multivm-test-$$-$RANDOM.log"
    
    # Run the test
    if [ "$VERBOSE" = "true" ]; then
        echo "Running: $test_command"
        if timeout ${TEST_TIMEOUT}s bash -c "$test_command" 2>&1 | tee "$output_file"; then
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            echo -e "\n${GREEN}✅ PASSED${NC} (${duration}s)\n"
            PASSED_TESTS=$((PASSED_TESTS + 1))
            rm -f "$output_file"
            return 0
        else
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            echo -e "\n${RED}❌ FAILED${NC} (${duration}s)\n"
            FAILED_TESTS=$((FAILED_TESTS + 1))
            
            # Show last few lines of output if not verbose
            if [ "$VERBOSE" != "true" ] && [ -f "$output_file" ]; then
                echo "Last 20 lines of output:"
                tail -n 20 "$output_file"
            fi
            
            rm -f "$output_file"
            return 1
        fi
    else
        if timeout ${TEST_TIMEOUT}s bash -c "$test_command" > "$output_file" 2>&1; then
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            echo -e "${GREEN}✅ PASSED${NC} (${duration}s)"
            PASSED_TESTS=$((PASSED_TESTS + 1))
            rm -f "$output_file"
            return 0
        else
            local end_time=$(date +%s)
            local duration=$((end_time - start_time))
            echo -e "${RED}❌ FAILED${NC} (${duration}s)"
            FAILED_TESTS=$((FAILED_TESTS + 1))
            
            # Show last few lines of output on failure
            echo "Last 20 lines of output:"
            tail -n 20 "$output_file"
            
            rm -f "$output_file"
            return 1
        fi
    fi
}

# Run tests based on available binaries
echo ""
echo "Running Integration Tests"
echo "========================"

# Always run unit tests
if [ "$RUN_QUICK_TESTS" = "true" ]; then
    echo -e "\n${YELLOW}Quick Unit Tests${NC}"
    echo "----------------"
    
    run_test "Consensus Unit Tests" \
        "cargo test --package multivm-consensus --lib test_consensus_manager_creation" \
        "Basic consensus manager functionality"
    
    run_test "Account Mapping Tests" \
        "cargo test --package multivm-account-mapping --lib test_basic_operations" \
        "Account mapping operations"
fi

# Run integration tests based on available binaries
if [ "$SKIP_LONG_RUNNING" = "false" ]; then
    if [ "$RETH_AVAILABLE" = "true" ]; then
        echo -e "\n${YELLOW}Reth Integration Tests${NC}"
        echo "----------------------"
        
        run_test "Reth Connectivity" \
            "cargo test --test integration_reth_engine_test test_reth_connectivity_only -- --nocapture" \
            "Test basic connectivity to Reth process"
        
        run_test "Reth RPC Communication" \
            "cargo test --test integration_reth_engine_test test_reth_rpc_communication -- --nocapture" \
            "Test RPC request relaying to Reth"
        
        if [ "$SKIP_LONG_RUNNING" = "false" ]; then
            run_test "Reth Full Integration" \
                "cargo test --test integration_reth_engine_test test_reth_integration_full_workflow -- --nocapture" \
                "Complete Reth integration workflow including block processing"
        fi
    else
        echo -e "\n${YELLOW}⚠ Skipping Reth integration tests (binary not available)${NC}"
        SKIPPED_TESTS=$((SKIPPED_TESTS + 3))
    fi
    
    if [ "$SOLANA_AVAILABLE" = "true" ]; then
        echo -e "\n${YELLOW}Solana Integration Tests${NC}"
        echo "------------------------"
        
        run_test "Solana Mempool" \
            "cargo test --test integration_solana_engine_test test_solana_mempool_only -- --nocapture" \
            "Test Solana mempool operations"
        
        run_test "Solana Connectivity" \
            "cargo test --test integration_solana_engine_test test_solana_connectivity_only -- --nocapture" \
            "Test basic connectivity to Solana process"
        
        run_test "Solana RPC Communication" \
            "cargo test --test integration_solana_engine_test test_solana_rpc_communication -- --nocapture" \
            "Test RPC request relaying to Solana"
        
        if [ "$SKIP_LONG_RUNNING" = "false" ]; then
            run_test "Solana Full Integration" \
                "cargo test --test integration_solana_engine_test test_solana_integration_full_workflow -- --nocapture" \
                "Complete Solana integration workflow including block processing"
        fi
    else
        echo -e "\n${YELLOW}⚠ Skipping Solana integration tests (binary not available)${NC}"
        SKIPPED_TESTS=$((SKIPPED_TESTS + 4))
    fi
    
    if [ "$RETH_AVAILABLE" = "true" ] && [ "$SOLANA_AVAILABLE" = "true" ]; then
        echo -e "\n${YELLOW}Cross-VM Integration Tests${NC}"
        echo "--------------------------"
        
        run_test "Dual Engine Startup" \
            "cargo test --test integration_cross_vm_test test_dual_engine_startup_only -- --nocapture" \
            "Test starting both Reth and Solana engines"
        
        if [ "$SKIP_LONG_RUNNING" = "false" ]; then
            run_test "Cross-VM Full Integration" \
                "cargo test --test integration_cross_vm_test test_cross_vm_full_integration -- --nocapture" \
                "Complete cross-VM integration with transaction processing"
            
            run_test "RPC Relay Integration" \
                "cargo test --test integration_rpc_relay_test test_rpc_relay_full_integration -- --nocapture" \
                "Test RPC request relaying across VMs"
            
            run_test "Block Processing Integration" \
                "cargo test --test integration_block_processing_test test_block_processing_full_workflow -- --nocapture" \
                "End-to-end block processing workflow"
        fi
    else
        echo -e "\n${YELLOW}⚠ Skipping cross-VM integration tests (both binaries required)${NC}"
        SKIPPED_TESTS=$((SKIPPED_TESTS + 4))
    fi
fi

# Generate summary
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📊 Test Summary${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo "Total tests run: $TOTAL_TESTS"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$FAILED_TESTS${NC}"
echo -e "Skipped: ${YELLOW}$SKIPPED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ] && [ $TOTAL_TESTS -gt 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 All tests passed!${NC}"
    SUCCESS_RATE=100
else
    if [ $TOTAL_TESTS -gt 0 ]; then
        SUCCESS_RATE=$(echo "scale=2; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l 2>/dev/null || echo "0")
    else
        SUCCESS_RATE=0
    fi
fi

echo "Success rate: ${SUCCESS_RATE}%"

# Show custom fork information
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}🔧 Binary Information${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if [ "$RETH_AVAILABLE" = "true" ]; then
    echo "Reth:"
    echo "  Binary: $(command -v reth)"
    if [ -n "$RETH_FORK_REPO" ]; then
        echo "  Fork: $RETH_FORK_REPO ($RETH_FORK_BRANCH)"
    fi
fi

if [ "$SOLANA_AVAILABLE" = "true" ]; then
    echo "Solana:"
    echo "  Binary: $(command -v solana-test-validator)"
    if [ -n "$SOLANA_FORK_REPO" ]; then
        echo "  Fork: $SOLANA_FORK_REPO ($SOLANA_FORK_BRANCH)"
    fi
fi

echo ""
echo "💡 Tips:"
echo "  - To setup custom binaries: ./scripts/setup-test-binaries.sh"
echo "  - To run all tests: SKIP_LONG_RUNNING=false $0"
echo "  - To run quietly: VERBOSE=false $0"
echo "  - To adjust timeout: TEST_TIMEOUT=1200 $0"

# Exit with appropriate code
if [ $FAILED_TESTS -eq 0 ]; then
    exit 0
else
    exit 1
fi