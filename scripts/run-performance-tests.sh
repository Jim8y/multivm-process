#!/bin/bash
# Performance testing script for MultiVM system

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RESULTS_DIR="performance_results"
LOG_DIR="performance_logs"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create directories
mkdir -p "$RESULTS_DIR"
mkdir -p "$LOG_DIR"

echo -e "${BLUE}=== MultiVM Performance Testing Suite ===${NC}"
echo -e "Timestamp: $TIMESTAMP\n"

# Function to run a test
run_test() {
    local test_name=$1
    local test_command=$2
    
    echo -e "${YELLOW}Running $test_name...${NC}"
    
    # Start monitoring
    start_monitoring "$test_name"
    
    # Run the test
    if $test_command > "$LOG_DIR/${test_name}_${TIMESTAMP}.log" 2>&1; then
        echo -e "${GREEN}✓ $test_name completed successfully${NC}"
    else
        echo -e "${RED}✗ $test_name failed${NC}"
        cat "$LOG_DIR/${test_name}_${TIMESTAMP}.log"
    fi
    
    # Stop monitoring
    stop_monitoring "$test_name"
}

# Function to start system monitoring
start_monitoring() {
    local test_name=$1
    
    # Start resource monitoring in background
    (
        while true; do
            # CPU and Memory usage
            ps aux | grep -E "multivm|reth|solana" | grep -v grep >> "$LOG_DIR/${test_name}_resources_${TIMESTAMP}.log"
            
            # Network stats
            netstat -i >> "$LOG_DIR/${test_name}_network_${TIMESTAMP}.log"
            
            sleep 5
        done
    ) &
    MONITOR_PID=$!
}

# Function to stop monitoring
stop_monitoring() {
    if [ ! -z "$MONITOR_PID" ]; then
        kill $MONITOR_PID 2>/dev/null || true
    fi
}

# Check if running in test mode
if [ "$1" == "test" ]; then
    echo -e "${YELLOW}Running in test mode (reduced duration)${NC}\n"
    export MULTIVM_TEST_MODE=true
fi

# 1. Unit Performance Tests
echo -e "${BLUE}1. Running unit performance tests...${NC}"
run_test "unit_perf" "cargo test --release -p multivm-consensus test_performance -- --nocapture"
run_test "mapping_perf" "cargo test --release -p multivm-account-mapping test_performance -- --nocapture"

# 2. Integration Performance Tests
echo -e "\n${BLUE}2. Running integration performance tests...${NC}"
run_test "integration_perf" "cargo test --release --test performance_tests -- --nocapture"

# 3. Throughput Benchmark
echo -e "\n${BLUE}3. Running throughput benchmark...${NC}"
run_test "throughput" "cargo run --release --bin multivm-bench -- throughput --duration 300 --clients 50"

# 4. Latency Benchmark
echo -e "\n${BLUE}4. Running latency benchmark...${NC}"
run_test "latency" "cargo run --release --bin multivm-bench -- latency --iterations 1000"

# 5. Scalability Test
echo -e "\n${BLUE}5. Running scalability test...${NC}"
run_test "scalability" "cargo run --release --bin multivm-bench -- scalability --max-clients 200"

# 6. Stress Test
echo -e "\n${BLUE}6. Running stress test...${NC}"
if [ "$1" != "test" ]; then
    run_test "stress" "cargo run --release --bin multivm-bench -- stress --duration 600 --load-factor 0.9"
else
    echo -e "${YELLOW}Skipping stress test in test mode${NC}"
fi

# 7. Generate Performance Report
echo -e "\n${BLUE}7. Generating performance report...${NC}"

# Collect all results
REPORT_FILE="$RESULTS_DIR/performance_report_${TIMESTAMP}.md"

cat > "$REPORT_FILE" << EOF
# MultiVM Performance Test Report

**Date**: $(date)
**System**: $(uname -a)
**CPU**: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs)
**Memory**: $(free -h | grep Mem | awk '{print $2}')

## Test Results Summary

### 1. Throughput Performance
EOF

# Parse throughput results
if [ -f "$LOG_DIR/throughput_${TIMESTAMP}.log" ]; then
    grep -E "Average TPS:|Peak TPS:" "$LOG_DIR/throughput_${TIMESTAMP}.log" >> "$REPORT_FILE" || true
fi

cat >> "$REPORT_FILE" << EOF

### 2. Latency Performance
EOF

# Parse latency results
if [ -f "$LOG_DIR/latency_${TIMESTAMP}.log" ]; then
    grep -E "Average Latency:|P95 Latency:|P99 Latency:" "$LOG_DIR/latency_${TIMESTAMP}.log" >> "$REPORT_FILE" || true
fi

cat >> "$REPORT_FILE" << EOF

### 3. Scalability Results
EOF

# Parse scalability results
if [ -f "$LOG_DIR/scalability_${TIMESTAMP}.log" ]; then
    grep -E "Clients:|TPS:" "$LOG_DIR/scalability_${TIMESTAMP}.log" >> "$REPORT_FILE" || true
fi

cat >> "$REPORT_FILE" << EOF

### 4. Resource Usage

Peak resource usage during tests:
EOF

# Analyze resource logs
for log in "$LOG_DIR"/*_resources_${TIMESTAMP}.log; do
    if [ -f "$log" ]; then
        echo -e "\n#### $(basename $log .log)" >> "$REPORT_FILE"
        # Get peak CPU and memory
        awk '{cpu+=$3; mem+=$4; count++} END {print "- Average CPU: " cpu/count "%\n- Average Memory: " mem/count "%"}' "$log" >> "$REPORT_FILE" 2>/dev/null || true
    fi
done

echo -e "\n${GREEN}Performance report generated: $REPORT_FILE${NC}"

# 8. Compare with baseline (if exists)
BASELINE_FILE="$RESULTS_DIR/baseline.json"
if [ -f "$BASELINE_FILE" ]; then
    echo -e "\n${BLUE}8. Comparing with baseline...${NC}"
    # Would implement comparison logic here
    echo -e "${YELLOW}Baseline comparison not yet implemented${NC}"
fi

# Cleanup
echo -e "\n${BLUE}Cleaning up...${NC}"
stop_monitoring "cleanup"

# Archive logs
tar -czf "$LOG_DIR/performance_logs_${TIMESTAMP}.tar.gz" "$LOG_DIR"/*_${TIMESTAMP}.log
rm -f "$LOG_DIR"/*_${TIMESTAMP}.log

echo -e "\n${GREEN}=== Performance testing completed ===${NC}"
echo -e "Results saved in: $RESULTS_DIR"
echo -e "Logs archived in: $LOG_DIR/performance_logs_${TIMESTAMP}.tar.gz"

# Exit with appropriate code
if grep -q "failed" "$REPORT_FILE"; then
    exit 1
else
    exit 0
fi