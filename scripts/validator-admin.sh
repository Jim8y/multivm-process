#!/bin/bash

# MultiVM Validator Administration Script
# Advanced tools for managing and monitoring validator networks

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEFAULT_VALIDATORS_DIR="/tmp/multivm-validators"
DEFAULT_BASE_PORT=8080

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Usage function
usage() {
    echo "MultiVM Validator Administration Tool"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  status                    Show validator network status"
    echo "  monitor                   Real-time monitoring dashboard"
    echo "  health                    Health check all validators"
    echo "  leader                    Show current leader information"
    echo "  rotate                    Trigger view change (leader rotation)"
    echo "  add-validator ID PORT     Add a new validator to the network"
    echo "  remove-validator ID       Remove validator from the network"
    echo "  submit-tx                 Submit test transaction"
    echo "  stress-test [COUNT]       Run stress test with COUNT transactions"
    echo "  logs ID                   Show logs for specific validator"
    echo "  config ID                 Show configuration for specific validator"
    echo "  benchmark                 Run performance benchmarks"
    echo "  cleanup                   Stop all validators and cleanup"
    echo ""
    echo "Options:"
    echo "  -d, --dir DIR            Validators directory (default: $DEFAULT_VALIDATORS_DIR)"
    echo "  -p, --port PORT          Base port (default: $DEFAULT_BASE_PORT)"
    echo "  -v, --verbose            Verbose output"
    echo "  -h, --help              Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 status                # Show network status"
    echo "  $0 monitor               # Start monitoring dashboard"
    echo "  $0 submit-tx             # Submit a test transaction"
    echo "  $0 stress-test 100       # Submit 100 test transactions"
    echo "  $0 logs validator_01     # Show logs for validator_01"
}

# Parse command line arguments
COMMAND=""
VALIDATORS_DIR="$DEFAULT_VALIDATORS_DIR"
BASE_PORT="$DEFAULT_BASE_PORT"
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--dir)
            VALIDATORS_DIR="$2"
            shift 2
            ;;
        -p|--port)
            BASE_PORT="$2"
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
            if [ -z "$COMMAND" ]; then
                COMMAND="$1"
            else
                # Additional arguments for specific commands
                case $COMMAND in
                    add-validator)
                        VALIDATOR_ID="$1"
                        VALIDATOR_PORT="$2"
                        shift 2
                        ;;
                    remove-validator|logs|config)
                        VALIDATOR_ID="$1"
                        shift
                        ;;
                    stress-test)
                        STRESS_COUNT="$1"
                        shift
                        ;;
                    *)
                        shift
                        ;;
                esac
            fi
            shift
            ;;
    esac
done

if [ -z "$COMMAND" ]; then
    usage
    exit 1
fi

# Helper functions
get_validator_count() {
    if [ ! -d "$VALIDATORS_DIR" ]; then
        echo "0"
        return
    fi
    ls -d "$VALIDATORS_DIR"/validator_* 2>/dev/null | wc -l
}

get_validator_port() {
    local validator_id="$1"
    local validator_num=$(echo "$validator_id" | sed 's/validator_//' | sed 's/^0*//')
    echo $((BASE_PORT + validator_num))
}

check_validator_health() {
    local validator_id="$1"
    local port=$(get_validator_port "$validator_id")
    curl -s -m 2 "http://localhost:$port/health" >/dev/null 2>&1
}

get_validator_info() {
    local validator_id="$1"
    local port=$(get_validator_port "$validator_id")
    
    local status="🔴 DOWN"
    local height="N/A"
    local proposer="N/A"
    local tx_count="N/A"
    
    if check_validator_health "$validator_id"; then
        status="🟢 UP"
        height=$(curl -s -m 2 "http://localhost:$port/api/height" 2>/dev/null | jq -r '.height // "N/A"' 2>/dev/null || echo "N/A")
        proposer=$(curl -s -m 2 "http://localhost:$port/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "N/A"' 2>/dev/null || echo "N/A")
        tx_count=$(curl -s -m 2 "http://localhost:$port/api/transaction_pool/stats" 2>/dev/null | jq -r '.current_pool_size // "N/A"' 2>/dev/null || echo "N/A")
    fi
    
    printf "%-15s %-8s %-10s %-15s %-8s %d\n" "$validator_id" "$status" "$height" "$proposer" "$tx_count" "$port"
}

# Command implementations
cmd_status() {
    echo -e "${CYAN}📊 Validator Network Status${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    if [ ! -d "$VALIDATORS_DIR" ]; then
        echo -e "${RED}❌ Validators directory not found: $VALIDATORS_DIR${NC}"
        echo "   Run './setup-validators.sh' to create a validator network first."
        return 1
    fi
    
    printf "%-15s %-8s %-10s %-15s %-8s %s\n" "VALIDATOR" "STATUS" "HEIGHT" "PROPOSER" "TX_POOL" "PORT"
    echo "─────────────────────────────────────────────────────────────────────────────"
    
    local healthy_count=0
    local total_count=0
    
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            get_validator_info "$validator_id"
            
            total_count=$((total_count + 1))
            if check_validator_health "$validator_id"; then
                healthy_count=$((healthy_count + 1))
            fi
        fi
    done
    
    echo "─────────────────────────────────────────────────────────────────────────────"
    echo "Network: $healthy_count/$total_count validators healthy"
    
    if [ $healthy_count -eq $total_count ] && [ $total_count -gt 0 ]; then
        echo -e "${GREEN}✅ Network is fully operational${NC}"
    elif [ $healthy_count -gt 0 ]; then
        echo -e "${YELLOW}⚠️  Partial network operation${NC}"
    else
        echo -e "${RED}❌ Network is down${NC}"
    fi
}

cmd_monitor() {
    echo -e "${CYAN}📊 Starting real-time monitoring dashboard...${NC}"
    echo "Press Ctrl+C to stop monitoring"
    echo ""
    
    while true; do
        clear
        echo -e "${PURPLE}MultiVM Validator Network - Live Dashboard - $(date)${NC}"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        
        cmd_status
        
        echo ""
        echo -e "${BLUE}🔄 Network Activity:${NC}"
        
        # Show recent activity if available
        local first_validator=$(ls "$VALIDATORS_DIR"/validator_* 2>/dev/null | head -1)
        if [ -n "$first_validator" ]; then
            local validator_id=$(basename "$first_validator")
            local port=$(get_validator_port "$validator_id")
            
            echo "   Last update: $(date)"
            echo "   Monitoring port: $port"
            
            # Try to get some network stats
            local stats=$(curl -s -m 2 "http://localhost:$port/api/stats" 2>/dev/null || echo "{}")
            if [ "$stats" != "{}" ]; then
                echo "   Network stats: $stats"
            fi
        fi
        
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        sleep 3
    done
}

cmd_health() {
    echo -e "${CYAN}🏥 Health Check Report${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    local healthy_count=0
    local total_count=0
    local issues=()
    
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            local port=$(get_validator_port "$validator_id")
            
            total_count=$((total_count + 1))
            
            echo -n "Checking $validator_id (port $port)... "
            
            if check_validator_health "$validator_id"; then
                echo -e "${GREEN}✅ Healthy${NC}"
                healthy_count=$((healthy_count + 1))
                
                # Additional health checks
                local height=$(curl -s -m 2 "http://localhost:$port/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
                if [ "$height" -eq "0" ]; then
                    issues+=("$validator_id: No blocks generated")
                fi
            else
                echo -e "${RED}❌ Unhealthy${NC}"
                issues+=("$validator_id: Not responding on port $port")
            fi
        fi
    done
    
    echo ""
    echo "Summary: $healthy_count/$total_count validators healthy"
    
    if [ ${#issues[@]} -gt 0 ]; then
        echo ""
        echo -e "${YELLOW}⚠️  Issues detected:${NC}"
        for issue in "${issues[@]}"; do
            echo "   - $issue"
        done
    fi
    
    # BFT threshold check
    if [ $total_count -gt 0 ]; then
        local required=$((total_count * 2 / 3 + 1))
        echo ""
        echo -e "${BLUE}🛡️  BFT Analysis:${NC}"
        echo "   Total validators: $total_count"
        echo "   Healthy validators: $healthy_count"
        echo "   Required for consensus: $required"
        
        if [ $healthy_count -ge $required ]; then
            echo -e "   ${GREEN}✅ Sufficient for Byzantine Fault Tolerance${NC}"
        else
            echo -e "   ${RED}❌ Insufficient for consensus (need $required, have $healthy_count)${NC}"
        fi
    fi
}

cmd_leader() {
    echo -e "${CYAN}👑 Leader Information${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    local validators=()
    local proposers=()
    local consistent=true
    local first_proposer=""
    
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            local port=$(get_validator_port "$validator_id")
            
            if check_validator_health "$validator_id"; then
                local proposer=$(curl -s -m 2 "http://localhost:$port/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "unknown"' 2>/dev/null || echo "unknown")
                validators+=("$validator_id")
                proposers+=("$proposer")
                
                echo "$validator_id reports leader: $proposer"
                
                if [ -z "$first_proposer" ]; then
                    first_proposer="$proposer"
                elif [ "$proposer" != "$first_proposer" ]; then
                    consistent=false
                fi
            fi
        fi
    done
    
    echo ""
    if [ "$consistent" = true ] && [ "$first_proposer" != "unknown" ] && [ -n "$first_proposer" ]; then
        echo -e "${GREEN}✅ Consensus: All validators agree on leader: $first_proposer${NC}"
    elif [ "$first_proposer" = "unknown" ]; then
        echo -e "${YELLOW}⚠️  Leader determination in progress or not available${NC}"
    else
        echo -e "${RED}❌ Inconsistency: Validators disagree on leader${NC}"
    fi
}

cmd_rotate() {
    echo -e "${CYAN}🔄 Triggering View Change (Leader Rotation)${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    local success_count=0
    local total_count=0
    
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            local port=$(get_validator_port "$validator_id")
            
            if check_validator_health "$validator_id"; then
                total_count=$((total_count + 1))
                echo -n "Triggering view change on $validator_id... "
                
                if curl -s -m 5 -X POST "http://localhost:$port/api/consensus/view_change" >/dev/null 2>&1; then
                    echo -e "${GREEN}✅ Success${NC}"
                    success_count=$((success_count + 1))
                else
                    echo -e "${RED}❌ Failed${NC}"
                fi
            fi
        fi
    done
    
    echo ""
    echo "View change triggered on $success_count/$total_count validators"
    
    if [ $success_count -gt 0 ]; then
        echo "Waiting for new leader selection..."
        sleep 5
        cmd_leader
    fi
}

cmd_submit_tx() {
    echo -e "${CYAN}📝 Submitting Test Transaction${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Find first healthy validator
    local target_port=""
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            local port=$(get_validator_port "$validator_id")
            
            if check_validator_health "$validator_id"; then
                target_port="$port"
                echo "Using $validator_id (port $port) for transaction submission"
                break
            fi
        fi
    done
    
    if [ -z "$target_port" ]; then
        echo -e "${RED}❌ No healthy validators found${NC}"
        return 1
    fi
    
    # Generate unique transaction
    local tx_id="test_tx_$(date +%s)_$$"
    local nonce=$(date +%s)
    
    local tx_data='{
        "id": "'$tx_id'",
        "type": "evm",
        "sender": "0x1234567890123456789012345678901234567890",
        "to": "0x0987654321098765432109876543210987654321",
        "value": 1000000000000000000,
        "data": "0x",
        "nonce": '$nonce'
    }'
    
    echo "Transaction data:"
    echo "$tx_data" | jq .
    echo ""
    
    echo -n "Submitting transaction... "
    local response=$(curl -s -m 5 -X POST "http://localhost:$target_port/api/submit_transaction" \
        -H "Content-Type: application/json" \
        -d "$tx_data" 2>/dev/null)
    
    if echo "$response" | grep -q "success\|accepted\|submitted"; then
        echo -e "${GREEN}✅ Success${NC}"
        echo "Response: $response"
    else
        echo -e "${RED}❌ Failed${NC}"
        echo "Response: $response"
    fi
}

cmd_stress_test() {
    local count=${STRESS_COUNT:-10}
    echo -e "${CYAN}⚡ Running Stress Test ($count transactions)${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Find first healthy validator
    local target_port=""
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            local port=$(get_validator_port "$validator_id")
            
            if check_validator_health "$validator_id"; then
                target_port="$port"
                echo "Using $validator_id (port $port) for stress test"
                break
            fi
        fi
    done
    
    if [ -z "$target_port" ]; then
        echo -e "${RED}❌ No healthy validators found${NC}"
        return 1
    fi
    
    local success_count=0
    local start_time=$(date +%s)
    
    echo "Submitting $count transactions..."
    for i in $(seq 1 $count); do
        local tx_id="stress_tx_${i}_$(date +%s)_$$"
        local nonce=$(($(date +%s) + i))
        
        local tx_data='{
            "id": "'$tx_id'",
            "type": "evm",
            "sender": "0x1111111111111111111111111111111111111111",
            "to": "0x2222222222222222222222222222222222222222",
            "value": '$((1000000000000000000 + i))',
            "data": "0x",
            "nonce": '$nonce'
        }'
        
        if curl -s -m 5 -X POST "http://localhost:$target_port/api/submit_transaction" \
            -H "Content-Type: application/json" \
            -d "$tx_data" >/dev/null 2>&1; then
            success_count=$((success_count + 1))
        fi
        
        # Progress indicator
        if [ $((i % 10)) -eq 0 ]; then
            echo -n "."
        fi
    done
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    echo ""
    echo "Results:"
    echo "  Submitted: $success_count/$count transactions"
    echo "  Duration: ${duration}s"
    echo "  Rate: $(echo "scale=2; $success_count / $duration" | bc -l) tx/s"
    
    if [ $success_count -eq $count ]; then
        echo -e "${GREEN}✅ All transactions submitted successfully${NC}"
    else
        echo -e "${YELLOW}⚠️  Some transactions failed${NC}"
    fi
}

cmd_logs() {
    local validator_id="$VALIDATOR_ID"
    if [ -z "$validator_id" ]; then
        echo -e "${RED}❌ Validator ID required${NC}"
        echo "Usage: $0 logs VALIDATOR_ID"
        return 1
    fi
    
    local log_file="$VALIDATORS_DIR/$validator_id/logs/validator.log"
    
    if [ ! -f "$log_file" ]; then
        echo -e "${RED}❌ Log file not found: $log_file${NC}"
        return 1
    fi
    
    echo -e "${CYAN}📋 Logs for $validator_id${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Log file: $log_file"
    echo ""
    
    # Show last 50 lines with color highlighting
    tail -n 50 "$log_file" | sed \
        -e "s/.*ERROR.*/$(printf "${RED}&${NC}")/" \
        -e "s/.*WARN.*/$(printf "${YELLOW}&${NC}")/" \
        -e "s/.*INFO.*/$(printf "${GREEN}&${NC}")/" \
        -e "s/.*DEBUG.*/$(printf "${BLUE}&${NC}")/"
}

cmd_config() {
    local validator_id="$VALIDATOR_ID"
    if [ -z "$validator_id" ]; then
        echo -e "${RED}❌ Validator ID required${NC}"
        echo "Usage: $0 config VALIDATOR_ID"
        return 1
    fi
    
    local config_file="$VALIDATORS_DIR/$validator_id/config/validator.toml"
    
    if [ ! -f "$config_file" ]; then
        echo -e "${RED}❌ Config file not found: $config_file${NC}"
        return 1
    fi
    
    echo -e "${CYAN}⚙️  Configuration for $validator_id${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Config file: $config_file"
    echo ""
    cat "$config_file"
}

cmd_benchmark() {
    echo -e "${CYAN}⚡ Performance Benchmarks${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Test 1: Leader selection performance
    echo "1. Leader Selection Response Time"
    local total_time=0
    local requests=10
    
    for validator_dir in "$VALIDATORS_DIR"/validator_*; do
        if [ -d "$validator_dir" ]; then
            local validator_id=$(basename "$validator_dir")
            local port=$(get_validator_port "$validator_id")
            
            if check_validator_health "$validator_id"; then
                echo "   Testing $validator_id..."
                
                for i in $(seq 1 $requests); do
                    local start=$(date +%s%N)
                    curl -s -m 2 "http://localhost:$port/api/consensus/proposer" >/dev/null 2>&1
                    local end=$(date +%s%N)
                    local duration=$(((end - start) / 1000000)) # Convert to milliseconds
                    total_time=$((total_time + duration))
                done
                
                local avg_time=$((total_time / requests))
                echo "     Average response time: ${avg_time}ms"
                break
            fi
        fi
    done
    
    # Test 2: Transaction submission rate
    echo ""
    echo "2. Transaction Submission Rate"
    cmd_stress_test 20 | grep -E "Rate:|Duration:"
    
    echo ""
    echo "3. Network Health Check Performance"
    local start_time=$(date +%s%N)
    cmd_health >/dev/null 2>&1
    local end_time=$(date +%s%N)
    local health_check_time=$(((end_time - start_time) / 1000000))
    echo "   Health check duration: ${health_check_time}ms"
}

cmd_cleanup() {
    echo -e "${CYAN}🧹 Cleanup: Stopping all validators${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    if [ -d "$VALIDATORS_DIR" ] && [ -f "$VALIDATORS_DIR/stop-all.sh" ]; then
        echo "Stopping validators..."
        cd "$VALIDATORS_DIR" && ./stop-all.sh
        
        echo -n "Removing validator directory... "
        rm -rf "$VALIDATORS_DIR"
        echo -e "${GREEN}✅ Done${NC}"
    else
        echo "No validators directory found or stop script missing"
        
        # Kill any multivm processes
        echo "Killing any remaining multivm processes..."
        pkill -f multivm-node 2>/dev/null || true
    fi
    
    echo "Cleanup completed"
}

# Main command dispatcher
case $COMMAND in
    status)
        cmd_status
        ;;
    monitor)
        cmd_monitor
        ;;
    health)
        cmd_health
        ;;
    leader)
        cmd_leader
        ;;
    rotate)
        cmd_rotate
        ;;
    submit-tx)
        cmd_submit_tx
        ;;
    stress-test)
        cmd_stress_test
        ;;
    logs)
        cmd_logs
        ;;
    config)
        cmd_config
        ;;
    benchmark)
        cmd_benchmark
        ;;
    cleanup)
        cmd_cleanup
        ;;
    *)
        echo -e "${RED}❌ Unknown command: $COMMAND${NC}"
        usage
        exit 1
        ;;
esac