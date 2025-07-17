#!/bin/bash
# Script to run MultiVM integration tests with proper setup

set -e

echo "=== MultiVM Integration Test Suite ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    local status=$1
    local message=$2
    case $status in
        "info") echo -e "${YELLOW}[INFO]${NC} $message" ;;
        "success") echo -e "${GREEN}[SUCCESS]${NC} $message" ;;
        "error") echo -e "${RED}[ERROR]${NC} $message" ;;
    esac
}

# Check prerequisites
print_status "info" "Checking prerequisites..."

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    print_status "error" "Cargo not found. Please install Rust."
    exit 1
fi

# Clean up any previous test artifacts
print_status "info" "Cleaning up test artifacts..."
rm -f /tmp/test_*.sock /tmp/test_*.ipc 2>/dev/null || true

# Build the project first
print_status "info" "Building MultiVM project..."
cargo build --all-features

# Run unit tests first
print_status "info" "Running unit tests..."
cargo test --lib --all-features -- --nocapture

# Run component integration tests
print_status "info" "Running component integration tests..."
cargo test --test component_integration_tests -- --nocapture --test-threads=1

# Run full system integration tests (if execution engines are available)
print_status "info" "Running full system integration tests..."
if command -v reth &> /dev/null && [ -f "target/release/solana-private-validator" ]; then
    cargo test --test full_system_integration_test -- --nocapture --test-threads=1
else
    print_status "info" "Skipping full system tests (execution engines not available)"
fi

# Run existing integration tests
print_status "info" "Running existing integration tests..."
cargo test --test integration_full_system -- --nocapture --test-threads=1 || true
cargo test --test consensus_integration_tests -- --nocapture --test-threads=1 || true

# Generate test report
print_status "info" "Generating test report..."
cargo test --all-features -- --nocapture 2>&1 | tee test_report.log

# Summary
echo
print_status "success" "Integration test suite completed!"
echo
echo "Test results saved to: test_report.log"
echo

# Check if all tests passed
if grep -q "test result: FAILED" test_report.log; then
    print_status "error" "Some tests failed. Please check test_report.log for details."
    exit 1
else
    print_status "success" "All tests passed!"
fi