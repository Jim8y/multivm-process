#!/bin/bash

# MultiVM Test Suite
# Comprehensive testing utilities for the MultiVM blockchain

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# API endpoints
REST_API="http://localhost:8080"
GRAPHQL_API="http://localhost:8081"
WS_API="ws://localhost:8082"

function show_help() {
    echo -e "${BLUE}MultiVM Test Suite${NC}"
    echo ""
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  health          Check health of all services"
    echo "  api             Test REST API endpoints"
    echo "  graphql         Test GraphQL API"
    echo "  submit-tx       Submit test transactions"
    echo "  monitor         Monitor transaction pool and blocks"
    echo "  stress          Run stress test with high transaction volume"
    echo "  all             Run all tests"
    echo ""
    echo "Options:"
    echo "  -h, --help      Show this help message"
    echo "  -v, --verbose   Enable verbose output"
}

function check_health() {
    echo -e "${BLUE}Checking service health...${NC}"
    
    # Check metrics endpoint
    echo -n "Metrics service: "
    if curl -s http://localhost:9090/metrics > /dev/null; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
    
    # Check health endpoint
    echo -n "Health service: "
    if curl -s http://localhost:8090/health > /dev/null; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
    
    # Check REST API
    echo -n "REST API: "
    if curl -s $REST_API/api/v1/health > /dev/null; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
}

function test_api() {
    echo -e "${BLUE}Testing REST API endpoints...${NC}"
    
    # Test node info
    echo -n "GET /api/v1/node/info: "
    RESPONSE=$(curl -s $REST_API/api/v1/node/info)
    if [[ $RESPONSE == *"version"* ]]; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
    
    # Test stats
    echo -n "GET /api/v1/explorer/stats: "
    RESPONSE=$(curl -s $REST_API/api/v1/explorer/stats)
    if [[ $RESPONSE == *"total_blocks"* ]]; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
}

function test_graphql() {
    echo -e "${BLUE}Testing GraphQL API...${NC}"
    
    # Test GraphQL query
    echo -n "GraphQL nodeInfo query: "
    QUERY='{"query":"{ nodeInfo { version nodeId networkId chainId consensusAlgorithm } }"}'
    RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d "$QUERY" $GRAPHQL_API/graphql)
    if [[ $RESPONSE == *"nodeInfo"* ]]; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
}

function submit_transactions() {
    echo -e "${BLUE}Submitting test transactions...${NC}"
    
    # Submit EVM transaction
    echo -n "Submitting EVM transaction: "
    TX_DATA='{
        "vm_type": "evm",
        "from": "0x1234567890123456789012345678901234567890",
        "to": "0x0987654321098765432109876543210987654321",
        "value": 1000000000000000000,
        "data": "0x",
        "gas_limit": 21000,
        "gas_price": 20000000000
    }'
    RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d "$TX_DATA" $REST_API/api/v1/transactions)
    if [[ $RESPONSE == *"success"* ]]; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
        echo "Response: $RESPONSE"
    fi
    
    # Submit SVM transaction
    echo -n "Submitting SVM transaction: "
    TX_DATA='{
        "vm_type": "svm",
        "signatures": ["test_signature"],
        "accounts": ["account1", "account2"],
        "data": [1, 2, 3, 4],
        "recent_blockhash": "test_blockhash",
        "fee": 5000
    }'
    RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d "$TX_DATA" $REST_API/api/v1/transactions)
    if [[ $RESPONSE == *"success"* ]]; then
        echo -e "${GREEN}✓ OK${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
    fi
}

function monitor_activity() {
    echo -e "${BLUE}Monitoring blockchain activity...${NC}"
    echo "Press Ctrl+C to stop monitoring"
    echo ""
    
    while true; do
        # Get stats
        STATS=$(curl -s $REST_API/api/v1/explorer/stats 2>/dev/null)
        
        if [ ! -z "$STATS" ]; then
            BLOCKS=$(echo $STATS | jq -r '.data.total_blocks // 0')
            PENDING=$(echo $STATS | jq -r '.data.transaction_pool.pending // 0')
            TOTAL_TX=$(echo $STATS | jq -r '.data.transaction_pool.total_submitted // 0')
            
            # Clear and display
            clear
            echo -e "${BLUE}MultiVM Activity Monitor${NC}"
            echo "========================"
            echo -e "Total Blocks: ${GREEN}$BLOCKS${NC}"
            echo -e "Pending Transactions: ${YELLOW}$PENDING${NC}"
            echo -e "Total Submitted: ${BLUE}$TOTAL_TX${NC}"
            echo ""
            echo "Last update: $(date)"
        fi
        
        sleep 2
    done
}

function stress_test() {
    echo -e "${BLUE}Running stress test...${NC}"
    echo "Submitting 100 transactions..."
    
    SUCCESS=0
    FAILED=0
    
    for i in {1..100}; do
        # Alternate between EVM and SVM transactions
        if [ $((i % 2)) -eq 0 ]; then
            TX_DATA="{
                \"vm_type\": \"evm\",
                \"from\": \"0x$(openssl rand -hex 20)\",
                \"to\": \"0x$(openssl rand -hex 20)\",
                \"value\": $((RANDOM * 1000000)),
                \"data\": \"0x\",
                \"gas_limit\": 21000,
                \"gas_price\": 20000000000
            }"
        else
            TX_DATA="{
                \"vm_type\": \"svm\",
                \"signatures\": [\"sig_$i\"],
                \"accounts\": [\"acc_${i}_from\", \"acc_${i}_to\"],
                \"data\": [1, 2, 3, 4],
                \"recent_blockhash\": \"hash_$i\",
                \"fee\": 5000
            }"
        fi
        
        RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d "$TX_DATA" $REST_API/api/v1/transactions)
        if [[ $RESPONSE == *"success"* ]]; then
            ((SUCCESS++))
        else
            ((FAILED++))
        fi
        
        # Progress indicator
        if [ $((i % 10)) -eq 0 ]; then
            echo -n "."
        fi
    done
    
    echo ""
    echo -e "${GREEN}Success: $SUCCESS${NC}, ${RED}Failed: $FAILED${NC}"
}

# Main execution
case "$1" in
    health)
        check_health
        ;;
    api)
        test_api
        ;;
    graphql)
        test_graphql
        ;;
    submit-tx)
        submit_transactions
        ;;
    monitor)
        monitor_activity
        ;;
    stress)
        stress_test
        ;;
    all)
        check_health
        echo ""
        test_api
        echo ""
        test_graphql
        echo ""
        submit_transactions
        ;;
    -h|--help|"")
        show_help
        ;;
    *)
        echo -e "${RED}Unknown command: $1${NC}"
        show_help
        exit 1
        ;;
esac