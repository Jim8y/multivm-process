#!/bin/bash
# Comprehensive development checks for the MultiVM project

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

print_info() {
    echo -e "${BLUE}[i]${NC} $1"
}

# Function to print section headers
print_header() {
    echo
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# Track overall status
OVERALL_SUCCESS=true

# Check if running in project root
if [ ! -f "Cargo.toml" ]; then
    print_error "This script must be run from the project root directory"
    exit 1
fi

# Detect number of CPU cores
CORES=$(nproc 2>/dev/null || echo "4")
print_info "Using $CORES CPU cores for parallel builds"

# Set environment variables for better performance
export CARGO_INCREMENTAL=1
export RUST_BACKTRACE=1
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"
export RUST_MIN_STACK=16777216  # For solana-execution-engine

# Check for required tools
print_header "Checking Prerequisites"

# Check Rust toolchain
if ! command -v rustc >/dev/null 2>&1; then
    print_error "Rust is not installed"
    exit 1
fi

RUST_VERSION=$(rustc --version | cut -d' ' -f2)
print_status "Rust version: $RUST_VERSION"

# Check for nightly toolchain
if rustup toolchain list | grep -q "nightly-2025-06-15"; then
    print_status "Nightly toolchain nightly-2025-06-15 is installed"
    RUST_TOOLCHAIN="+nightly-2025-06-15"
else
    print_warning "Nightly toolchain nightly-2025-06-15 not found, using default"
    RUST_TOOLCHAIN=""
fi

# Check for lld linker
if ! command -v lld >/dev/null 2>&1; then
    print_warning "lld linker not found - builds may be slower"
    print_info "Install with: sudo apt-get install lld"
    export RUSTFLAGS=""
fi

# Start timing
START_TIME=$(date +%s)

# 1. Format Check
print_header "Running Cargo Format Check"
if cargo $RUST_TOOLCHAIN fmt --all -- --check; then
    print_status "Code formatting check passed"
else
    print_error "Code formatting check failed"
    print_info "Run 'cargo fmt --all' to fix formatting issues"
    OVERALL_SUCCESS=false
fi

# 2. Clippy Lints
print_header "Running Cargo Clippy"
if cargo $RUST_TOOLCHAIN clippy --workspace --all-features -- -W clippy::all -W clippy::pedantic -A clippy::module_name_repetitions -A clippy::must_use_candidate; then
    print_status "Clippy lints passed"
else
    print_error "Clippy lints failed"
    OVERALL_SUCCESS=false
fi

# 3. Build Debug
print_header "Building Debug Mode"
if cargo $RUST_TOOLCHAIN build --workspace --all-features --jobs $CORES; then
    print_status "Debug build successful"
else
    print_error "Debug build failed"
    OVERALL_SUCCESS=false
    exit 1  # Can't continue without successful build
fi

# 4. Build Release
print_header "Building Release Mode"
if cargo $RUST_TOOLCHAIN build --workspace --release --all-features --jobs $CORES; then
    print_status "Release build successful"
else
    print_error "Release build failed"
    OVERALL_SUCCESS=false
fi

# 5. Run Tests
print_header "Running Tests"

# Unit tests
echo
print_info "Running unit tests..."
if cargo $RUST_TOOLCHAIN test --workspace --lib --bins --all-features --jobs $CORES -- --nocapture; then
    print_status "Unit tests passed"
else
    print_error "Unit tests failed"
    OVERALL_SUCCESS=false
fi

# Integration tests
echo
print_info "Running integration tests..."
if cargo $RUST_TOOLCHAIN test --workspace --test '*' --all-features --jobs $CORES -- --nocapture; then
    print_status "Integration tests passed"
else
    print_error "Integration tests failed"
    OVERALL_SUCCESS=false
fi

# Doc tests
echo
print_info "Running documentation tests..."
if cargo $RUST_TOOLCHAIN test --workspace --doc --all-features --jobs $CORES; then
    print_status "Documentation tests passed"
else
    print_error "Documentation tests failed"
    OVERALL_SUCCESS=false
fi

# 6. Check for Security Vulnerabilities
print_header "Running Security Audit"
if command -v cargo-audit >/dev/null 2>&1; then
    if cargo audit --ignore RUSTSEC-0000-0000; then
        print_status "Security audit passed"
    else
        print_warning "Security vulnerabilities found"
    fi
else
    print_warning "cargo-audit not installed"
    print_info "Install with: cargo install cargo-audit"
fi

# 7. Check Documentation
print_header "Checking Documentation"
if cargo $RUST_TOOLCHAIN doc --workspace --no-deps --all-features; then
    print_status "Documentation builds successfully"
else
    print_error "Documentation build failed"
    OVERALL_SUCCESS=false
fi

# Calculate elapsed time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
MINUTES=$((ELAPSED / 60))
SECONDS=$((ELAPSED % 60))

# Summary
print_header "Summary"
echo
if [ "$OVERALL_SUCCESS" = true ]; then
    print_status "All checks passed! ✨"
else
    print_error "Some checks failed. Please fix the issues above."
fi
echo
print_info "Total time: ${MINUTES}m ${SECONDS}s"

# Exit with appropriate code
if [ "$OVERALL_SUCCESS" = true ]; then
    exit 0
else
    exit 1
fi