#!/bin/bash
# Script to test CI steps locally before pushing to GitHub

set -e  # Exit on error

echo "=== Testing CI Pipeline Locally ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

# Check Rust version
echo "1. Checking Rust version..."
RUST_VERSION=$(rustc --version | cut -d' ' -f2)
echo "   Current Rust version: $RUST_VERSION"

# Try to determine the best Rust version
if cargo --version >/dev/null 2>&1; then
    print_status "Cargo is installed"
else
    print_error "Cargo is not installed"
    exit 1
fi

# Check for required system dependencies
echo
echo "2. Checking system dependencies..."
MISSING_DEPS=()

command -v pkg-config >/dev/null 2>&1 || MISSING_DEPS+=("pkg-config")
command -v protoc >/dev/null 2>&1 || MISSING_DEPS+=("protobuf-compiler")
pkg-config --exists libudev 2>/dev/null || MISSING_DEPS+=("libudev-dev")

if [ ${#MISSING_DEPS[@]} -eq 0 ]; then
    print_status "All system dependencies are installed"
else
    print_warning "Missing dependencies: ${MISSING_DEPS[*]}"
    echo "   Install with: sudo apt-get update && sudo apt-get install -y ${MISSING_DEPS[*]}"
fi

# Test SSH setup (if SSH_KEY is set)
echo
echo "3. Testing SSH configuration..."
if [ -n "$SSH_KEY" ]; then
    mkdir -p ~/.ssh
    echo "$SSH_KEY" > ~/.ssh/id_rsa_test
    chmod 600 ~/.ssh/id_rsa_test
    
    if ssh-keygen -y -f ~/.ssh/id_rsa_test >/dev/null 2>&1; then
        print_status "SSH key format is valid"
    else
        print_error "SSH key format is invalid"
    fi
    rm -f ~/.ssh/id_rsa_test
else
    print_warning "SSH_KEY environment variable not set"
fi

# Test formatting
echo
echo "4. Testing code formatting..."
if cargo fmt --all -- --check; then
    print_status "Code formatting check passed"
else
    print_error "Code formatting check failed"
    echo "   Run: cargo fmt --all"
fi

# Test with different Rust versions
echo
echo "5. Testing build with current Rust version..."

# Remove Cargo.lock to avoid version conflicts
if [ -f Cargo.lock ]; then
    print_warning "Removing Cargo.lock to avoid version conflicts"
    rm -f Cargo.lock
fi

# Try to build with specific nightly first
echo "   Testing with nightly-2025-06-15 Rust..."
if command -v rustup >/dev/null 2>&1 && rustup toolchain list | grep -q nightly; then
    # Install the specific nightly version if not available
    if ! rustup toolchain list | grep -q "nightly-2025-06-15"; then
        echo "   Installing nightly-2025-06-15..."
        rustup install nightly-2025-06-15
    fi
    
    if cargo +nightly-2025-06-15 build --workspace --all-features 2>&1 | tee build.log; then
        print_status "Build successful with nightly-2025-06-15 Rust"
        NIGHTLY_WORKS=true
    else
        print_error "Build failed with nightly-2025-06-15 Rust"
        NIGHTLY_WORKS=false
    fi
else
    print_warning "Nightly Rust not available, trying with current version"
    NIGHTLY_WORKS=false
fi

# Try with current version if nightly didn't work
if [ "$NIGHTLY_WORKS" != "true" ]; then
    echo "   Building with current Rust version..."
    if cargo build --workspace --all-features 2>&1 | tee build.log; then
        print_status "Build successful with Rust $RUST_VERSION"
    else
        print_error "Build failed with Rust $RUST_VERSION"
    
    # Check for specific errors
    if grep -q "edition2024" build.log; then
        print_warning "Some dependencies require edition2024 (needs nightly Rust)"
    fi
    
    if grep -q "extract_if" build.log; then
        print_warning "extract_if API incompatibility detected"
    fi
    
    if grep -q "lock file version" build.log; then
        print_warning "Cargo.lock version incompatibility"
    fi
fi

    fi
fi

# Test clippy
echo
echo "6. Testing clippy..."
if [ "$NIGHTLY_WORKS" = "true" ]; then
    if cargo +nightly-2025-06-15 clippy --workspace --lib --bins --tests -- -W clippy::correctness -W clippy::suspicious -A warnings 2>&1 | tee clippy.log; then
        print_status "Clippy check passed (nightly-2025-06-15)"
    else
        print_warning "Clippy check failed (non-critical)"
    fi
else
    if cargo clippy --workspace --lib --bins --tests -- -W clippy::correctness -W clippy::suspicious -A warnings 2>&1 | tee clippy.log; then
        print_status "Clippy check passed"
    else
        print_warning "Clippy check failed (non-critical)"
    fi
fi

# Test compilation of tests
echo
echo "7. Testing test compilation..."
if [ "$NIGHTLY_WORKS" = "true" ]; then
    if cargo +nightly-2025-06-15 test --workspace --all-features --no-run; then
        print_status "Test compilation successful (nightly-2025-06-15)"
    else
        print_error "Test compilation failed"
    fi
else
    if cargo test --workspace --all-features --no-run; then
        print_status "Test compilation successful"
    else
        print_error "Test compilation failed"
    fi
fi

# Summary
echo
echo "=== Summary ==="
echo

# Suggest Rust version based on results
if grep -q "edition2024" build.log 2>/dev/null; then
    echo "Recommendation: Use Rust nightly-2025-06-15 for edition2024 support"
    echo "  Install with: rustup install nightly-2025-06-15"
    echo "  Use with: cargo +nightly-2025-06-15 build"
elif grep -q "extract_if" build.log 2>/dev/null; then
    echo "Recommendation: Check Solana/Agave dependency compatibility"
    echo "  The extract_if API has changed between Rust versions"
fi

echo
echo "To fix issues before pushing:"
echo "1. Install missing system dependencies (if any)"
echo "2. Fix formatting issues with: cargo fmt --all"
echo "3. Consider using Rust nightly-2025-06-15 if edition2024 is required"
echo "4. Update Cargo.toml rust-version if using a different version"
echo
echo "Test logs saved to: build.log, clippy.log"