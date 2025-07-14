#!/bin/bash

# MultiVM Reth Integration Test Script
# This script performs comprehensive testing of the Reth integration with MultiVM

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"
RETH_DATA_DIR="${RETH_DATA_DIR:-${PROJECT_ROOT}/reth-data}"
RETH_HTTP_PORT="${RETH_HTTP_PORT:-8545}"
RETH_ENGINE_PORT="${RETH_ENGINE_PORT:-8551}"
JWT_SECRET_PATH="${JWT_SECRET_PATH:-${RETH_DATA_DIR}/jwt.hex}"
MULTIVM_IPC_PATH="${MULTIVM_IPC_PATH:-/tmp/multivm-reth.sock}"

# Test results
TESTS_PASSED=0
TESTS_FAILED=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
    ((TESTS_PASSED++))
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    ((TESTS_FAILED++))
}

# Generate JWT token
generate_jwt_token() {
    "${SCRIPT_DIR}/generate-jwt-token.sh" "$JWT_SECRET_PATH" --quiet
}

# Test 1: Check Reth Process
test_reth_process() {
    log_info "Test 1: Checking Reth process..."
    
    if pgrep -f "reth node" > /dev/null; then
        log_success "Reth process is running"
        local pid=$(pgrep -f "reth node" | head -n1)
        log_info "  PID: $pid"
        log_info "  Memory: $(ps -o rss= -p $pid | awk '{print int($1/1024) "MB"}')"
    else
        log_error "Reth process is not running"
        return 1
    fi
}

# Test 2: RPC Connectivity
test_rpc_connectivity() {
    log_info "Test 2: Testing RPC connectivity..."
    
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
        "http://127.0.0.1:$RETH_HTTP_PORT" 2>/dev/null || echo "failed")
    
    if echo "$response" | grep -q '"result"'; then
        local chain_id=$(echo "$response" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
        local chain_id_dec=$((16#${chain_id#0x}))
        log_success "RPC connectivity working (Chain ID: $chain_id_dec)"
    else
        log_error "RPC connectivity failed"
        echo "  Response: $response"
    fi
}

# Test 3: Engine API Authentication
test_engine_api_auth() {
    log_info "Test 3: Testing Engine API authentication..."
    
    # Test without auth (should fail)
    local no_auth_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[[]],"id":1}' \
        "http://127.0.0.1:$RETH_ENGINE_PORT" 2>&1 || echo "failed")
    
    if echo "$no_auth_response" | grep -q -E "(Unauthorized|failed|refused)"; then
        log_info "  ✓ Correctly rejects unauthenticated requests"
    else
        log_warning "  Engine API may not be properly secured"
    fi
    
    # Test with auth (should succeed)
    local jwt_token=$(generate_jwt_token)
    local auth_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $jwt_token" \
        -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[["engine_newPayloadV3","engine_forkchoiceUpdatedV3","engine_getPayloadV3"]],"id":1}' \
        "http://127.0.0.1:$RETH_ENGINE_PORT")
    
    if echo "$auth_response" | grep -q '"result"'; then
        log_success "Engine API authentication working"
        local capabilities=$(echo "$auth_response" | jq -r '.result[]' 2>/dev/null | tr '\n' ' ')
        log_info "  Capabilities: $capabilities"
    else
        log_error "Engine API authentication failed"
        echo "  Response: $auth_response"
    fi
}

# Test 4: Block Production Readiness
test_block_production() {
    log_info "Test 4: Testing block production readiness..."
    
    local jwt_token=$(generate_jwt_token)
    
    # Get current block
    local block_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["latest",false],"id":1}' \
        "http://127.0.0.1:$RETH_HTTP_PORT")
    
    if echo "$block_response" | grep -q '"result"'; then
        local block_number=$(echo "$block_response" | jq -r '.result.number' 2>/dev/null || echo "0x0")
        local block_hash=$(echo "$block_response" | jq -r '.result.hash' 2>/dev/null || echo "null")
        log_info "  Current block: $block_number (hash: ${block_hash:0:10}...)"
        
        # Test forkchoice update
        local forkchoice_state=$(cat <<EOF
{
    "headBlockHash": "$block_hash",
    "safeBlockHash": "$block_hash",
    "finalizedBlockHash": "$block_hash"
}
EOF
)
        
        local fc_response=$(curl -s -X POST \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $jwt_token" \
            -d "{\"jsonrpc\":\"2.0\",\"method\":\"engine_forkchoiceUpdatedV3\",\"params\":[$forkchoice_state,null],\"id\":1}" \
            "http://127.0.0.1:$RETH_ENGINE_PORT")
        
        if echo "$fc_response" | grep -q '"payloadStatus"'; then
            local status=$(echo "$fc_response" | jq -r '.result.payloadStatus.status' 2>/dev/null)
            if [ "$status" = "VALID" ]; then
                log_success "Block production ready (forkchoice update: VALID)"
            else
                log_warning "Block production status: $status"
            fi
        else
            log_error "Forkchoice update failed"
            echo "  Response: $fc_response"
        fi
    else
        log_error "Failed to get current block"
    fi
}

# Test 5: Transaction Pool
test_transaction_pool() {
    log_info "Test 5: Testing transaction pool..."
    
    local pool_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"txpool_status","params":[],"id":1}' \
        "http://127.0.0.1:$RETH_HTTP_PORT")
    
    if echo "$pool_response" | grep -q '"result"'; then
        local pending=$(echo "$pool_response" | jq -r '.result.pending // "0x0"' 2>/dev/null)
        local queued=$(echo "$pool_response" | jq -r '.result.queued // "0x0"' 2>/dev/null)
        log_success "Transaction pool accessible"
        log_info "  Pending: $pending, Queued: $queued"
    else
        log_warning "Transaction pool status unavailable"
    fi
}

# Test 6: IPC Socket
test_ipc_socket() {
    log_info "Test 6: Testing IPC socket readiness..."
    
    if [ -S "$MULTIVM_IPC_PATH" ]; then
        log_success "IPC socket exists: $MULTIVM_IPC_PATH"
        # Note: Actual IPC communication would be tested by the Rust integration tests
    else
        log_warning "IPC socket not found at $MULTIVM_IPC_PATH"
        log_info "  This may be created when MultiVM connects"
    fi
}

# Test 7: Configuration Files
test_configuration() {
    log_info "Test 7: Checking configuration files..."
    
    local all_good=true
    
    # Check JWT secret
    if [ -f "$JWT_SECRET_PATH" ]; then
        local jwt_len=$(cat "$JWT_SECRET_PATH" | tr -d '\n' | wc -c)
        if [ "$jwt_len" -eq 64 ]; then
            log_info "  ✓ JWT secret valid (64 hex chars)"
        else
            log_error "  ✗ JWT secret invalid length: $jwt_len (expected 64)"
            all_good=false
        fi
    else
        log_error "  ✗ JWT secret not found: $JWT_SECRET_PATH"
        all_good=false
    fi
    
    # Check data directory
    if [ -d "$RETH_DATA_DIR/db" ]; then
        log_info "  ✓ Database directory exists"
    else
        log_error "  ✗ Database directory missing: $RETH_DATA_DIR/db"
        all_good=false
    fi
    
    # Check logs
    if [ -f "$RETH_DATA_DIR/logs/reth.log" ]; then
        log_info "  ✓ Log file exists"
    else
        log_warning "  ! Log file not found: $RETH_DATA_DIR/logs/reth.log"
    fi
    
    if [ "$all_good" = true ]; then
        log_success "Configuration files valid"
    fi
}

# Test 8: Performance Check
test_performance() {
    log_info "Test 8: Performance check..."
    
    # Measure RPC response time
    local start_time=$(date +%s%N)
    curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        "http://127.0.0.1:$RETH_HTTP_PORT" > /dev/null
    local end_time=$(date +%s%N)
    
    local response_time=$(( (end_time - start_time) / 1000000 ))
    
    if [ "$response_time" -lt 100 ]; then
        log_success "RPC response time: ${response_time}ms (excellent)"
    elif [ "$response_time" -lt 500 ]; then
        log_success "RPC response time: ${response_time}ms (good)"
    else
        log_warning "RPC response time: ${response_time}ms (slow)"
    fi
}

# Main test execution
main() {
    log_info "=== MultiVM Reth Integration Test Suite ==="
    log_info "Testing Reth at http://127.0.0.1:$RETH_HTTP_PORT"
    echo ""
    
    # Run all tests
    test_reth_process
    echo ""
    
    test_rpc_connectivity
    echo ""
    
    test_engine_api_auth
    echo ""
    
    test_block_production
    echo ""
    
    test_transaction_pool
    echo ""
    
    test_ipc_socket
    echo ""
    
    test_configuration
    echo ""
    
    test_performance
    echo ""
    
    # Summary
    log_info "=== Test Summary ==="
    log_info "Tests Passed: $TESTS_PASSED"
    log_info "Tests Failed: $TESTS_FAILED"
    
    if [ "$TESTS_FAILED" -eq 0 ]; then
        log_success "All tests passed! Reth is ready for MultiVM integration."
        exit 0
    else
        log_error "Some tests failed. Please check the configuration."
        exit 1
    fi
}

# Check if jq is available (optional but helpful)
if ! command -v jq &> /dev/null; then
    log_warning "jq not found - some output formatting may be limited"
fi

# Run main
main "$@"