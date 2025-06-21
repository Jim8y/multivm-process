#!/bin/bash

# Script to run all tests and benchmarks for the MultiVM project

set -e

echo "🧪 Running MultiVM Test Suite"
echo "============================"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to run tests for a crate
run_crate_tests() {
    local crate=$1
    echo -e "\n${BLUE}Testing $crate...${NC}"
    
    if cargo test -p $crate --all-features 2>&1 | tee test_output.tmp; then
        echo -e "${GREEN}✅ $crate tests passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}❌ $crate tests failed${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    
    rm -f test_output.tmp
}

# Function to run integration tests
run_integration_tests() {
    echo -e "\n${BLUE}Running integration tests...${NC}"
    
    if cargo test --test integration_tests --features integration 2>&1 | tee test_output.tmp; then
        echo -e "${GREEN}✅ Integration tests passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}❌ Integration tests failed${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    
    if cargo test --test enhanced_integration_tests --features integration 2>&1 | tee test_output.tmp; then
        echo -e "${GREEN}✅ Enhanced integration tests passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}❌ Enhanced integration tests failed${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    
    rm -f test_output.tmp
}

# Function to run doc tests
run_doc_tests() {
    echo -e "\n${BLUE}Running documentation tests...${NC}"
    
    if cargo test --doc 2>&1 | tee test_output.tmp; then
        echo -e "${GREEN}✅ Documentation tests passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}❌ Documentation tests failed${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    
    rm -f test_output.tmp
}

# Function to run benchmarks
run_benchmarks() {
    echo -e "\n${BLUE}Running benchmarks (quick mode)...${NC}"
    
    if cargo bench --bench multivm_benchmarks -- --quick 2>&1 | tee bench_output.tmp; then
        echo -e "${GREEN}✅ Benchmarks completed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}❌ Benchmarks failed${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    
    rm -f bench_output.tmp
}

# Function to check code formatting
check_formatting() {
    echo -e "\n${BLUE}Checking code formatting...${NC}"
    
    if cargo fmt -- --check 2>&1; then
        echo -e "${GREEN}✅ Code formatting correct${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${YELLOW}⚠️  Code needs formatting (run 'cargo fmt')${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
}

# Function to run clippy lints
run_clippy() {
    echo -e "\n${BLUE}Running clippy lints...${NC}"
    
    if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tee clippy_output.tmp; then
        echo -e "${GREEN}✅ Clippy checks passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${YELLOW}⚠️  Clippy warnings found${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    
    rm -f clippy_output.tmp
}

# Main test execution
echo "Starting test suite execution..."

# Run unit tests for each crate
run_crate_tests "multivm-common"
run_crate_tests "multivm-consensus"
run_crate_tests "multivm-account-mapping"
run_crate_tests "multivm-process-manager"
run_crate_tests "multivm-p2p"

# Run integration tests
run_integration_tests

# Run doc tests
run_doc_tests

# Check formatting
check_formatting

# Run clippy
run_clippy

# Run benchmarks (optional, can be slow)
if [ "$1" == "--with-benchmarks" ]; then
    run_benchmarks
fi

# Summary
echo -e "\n${BLUE}================================${NC}"
echo -e "${BLUE}Test Suite Summary${NC}"
echo -e "${BLUE}================================${NC}"
echo "Total test categories: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All tests passed! The MultiVM system is working correctly.${NC}"
    exit 0
else
    echo -e "\n${RED}❌ Some tests failed. Please review the output above.${NC}"
    exit 1
fi