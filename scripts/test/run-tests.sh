#!/bin/bash
# Comprehensive test runner for MultiVM

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
TEST_TYPE="${TEST_TYPE:-all}"
VERBOSE="${VERBOSE:-false}"
COVERAGE="${COVERAGE:-false}"
TIMEOUT="${TIMEOUT:-300}"

log() {
    echo -e "${GREEN}[TEST]${NC} $1"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

run_unit_tests() {
    log "Running unit tests..."
    
    local args=(--workspace --lib)
    
    if [[ "$VERBOSE" == "true" ]]; then
        args+=(--verbose)
    fi
    
    if [[ "$COVERAGE" == "true" ]]; then
        if command -v cargo-tarpaulin &> /dev/null; then
            cargo tarpaulin --out Html --output-dir target/coverage
            info "Coverage report generated in target/coverage/"
        else
            warn "cargo-tarpaulin not found. Install with: cargo install cargo-tarpaulin"
        fi
    fi
    
    timeout "$TIMEOUT" cargo test "${args[@]}"
    log "Unit tests completed"
}

run_integration_tests() {
    log "Running integration tests..."
    
    local args=(--test integration_tests)
    
    if [[ "$VERBOSE" == "true" ]]; then
        args+=(--verbose)
    fi
    
    timeout "$TIMEOUT" cargo test "${args[@]}"
    log "Integration tests completed"
}

run_consensus_tests() {
    log "Running consensus tests..."
    
    # Build required binaries first
    cargo build --release --bin multivm-node --bin mock-solana --bin mock-reth
    
    local args=(--package multivm-consensus --test integration_tests)
    
    if [[ "$VERBOSE" == "true" ]]; then
        args+=(--verbose)
    fi
    
    timeout "$TIMEOUT" cargo test "${args[@]}"
    log "Consensus tests completed"
}

run_network_tests() {
    log "Running network tests..."
    
    local args=(--package multivm-p2p --test integration_tests)
    
    if [[ "$VERBOSE" == "true" ]]; then
        args+=(--verbose)
    fi
    
    timeout "$TIMEOUT" cargo test "${args[@]}"
    log "Network tests completed"
}

run_benchmark_tests() {
    log "Running benchmark tests..."
    
    if [[ ! -d "benchmarks" ]]; then
        warn "No benchmarks directory found, skipping benchmark tests"
        return
    fi
    
    cargo bench --workspace
    log "Benchmark tests completed"
}

run_docker_tests() {
    if ! command -v docker &> /dev/null; then
        warn "Docker not available, skipping Docker tests"
        return
    fi
    
    log "Running Docker tests..."
    
    # Build test image
    docker build -f Dockerfile -t multivm:test .
    
    # Run basic functionality test
    docker run --rm multivm:test --version
    
    log "Docker tests completed"
}

show_test_summary() {
    log "Test Summary:"
    echo "===================="
    
    if [[ -f "target/coverage/tarpaulin-report.html" ]]; then
        info "Coverage report: target/coverage/tarpaulin-report.html"
    fi
    
    # Show any failed tests from cargo output
    if [[ -f "/tmp/test_output.log" ]]; then
        local failed_tests=$(grep -E "test result: FAILED|failures:" /tmp/test_output.log || true)
        if [[ -n "$failed_tests" ]]; then
            warn "Some tests failed. Check the output above for details."
        fi
    fi
    
    log "All requested tests completed successfully!"
}

show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Comprehensive test runner for MultiVM project

OPTIONS:
    -h, --help          Show this help message
    -t, --type TYPE     Test type: unit|integration|consensus|network|bench|docker|all (default: all)
    -v, --verbose       Enable verbose output
    -c, --coverage      Generate coverage report (requires cargo-tarpaulin)
    --timeout SECONDS   Test timeout in seconds (default: 300)

TEST TYPES:
    unit                Run unit tests only
    integration         Run integration tests only
    consensus           Run consensus-specific tests
    network             Run network/P2P tests
    bench               Run benchmark tests
    docker              Run Docker-based tests
    all                 Run all test types

EXAMPLES:
    $0                          # Run all tests
    $0 --type unit              # Run only unit tests
    $0 --type consensus -v      # Run consensus tests with verbose output
    $0 --coverage               # Run tests with coverage report

EOF
}

main() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_usage
                exit 0
                ;;
            -t|--type)
                TEST_TYPE="$2"
                shift 2
                ;;
            -v|--verbose)
                VERBOSE="true"
                shift
                ;;
            -c|--coverage)
                COVERAGE="true"
                shift
                ;;
            --timeout)
                TIMEOUT="$2"
                shift 2
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done
    
    log "Starting MultiVM test suite..."
    info "Test type: $TEST_TYPE"
    info "Verbose: $VERBOSE"
    info "Coverage: $COVERAGE"
    info "Timeout: ${TIMEOUT}s"
    
    # Redirect output to log file for summary
    exec > >(tee /tmp/test_output.log)
    exec 2>&1
    
    case "$TEST_TYPE" in
        unit)
            run_unit_tests
            ;;
        integration)
            run_integration_tests
            ;;
        consensus)
            run_consensus_tests
            ;;
        network)
            run_network_tests
            ;;
        bench)
            run_benchmark_tests
            ;;
        docker)
            run_docker_tests
            ;;
        all)
            run_unit_tests
            run_integration_tests
            run_consensus_tests
            run_network_tests
            run_benchmark_tests
            run_docker_tests
            ;;
        *)
            error "Unknown test type: $TEST_TYPE"
            ;;
    esac
    
    show_test_summary
}

main "$@"