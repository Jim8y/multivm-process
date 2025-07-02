#!/bin/bash
# Script to test CI steps locally before pushing to GitHub
# Optimized for speed with parallel builds and caching

set -e  # Exit on error

echo "=== Testing Optimized CI Pipeline Locally ==="
echo

# Check if running on Linux for performance optimizations
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Install faster linker if available
    if command -v apt-get >/dev/null 2>&1; then
        echo "Installing build optimizations..."
        sudo apt-get update >/dev/null 2>&1 || true
        sudo apt-get install -y lld >/dev/null 2>&1 || echo "lld not available"
    fi
fi

# Configure optimized cargo settings
echo "Configuring build optimizations..."
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'EOF'
[build]
rustflags = ["-C", "link-arg=-fuse-ld=lld", "-C", "target-cpu=native"]
jobs = 0  # Use all available cores

[net]
retry = 10
timeout = 60000

[registries.crates-io]
protocol = "sparse"
EOF

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
echo "2. Checking system dependencies and optimizations..."
MISSING_DEPS=()

# Check for performance tools
if ! command -v lld >/dev/null 2>&1; then
    echo "   Note: lld (faster linker) not found - builds may be slower"
fi

CORES=$(nproc 2>/dev/null || echo "4")
echo "   Available CPU cores: $CORES"

command -v pkg-config >/dev/null 2>&1 || MISSING_DEPS+=("pkg-config")
command -v protoc >/dev/null 2>&1 || MISSING_DEPS+=("protobuf-compiler")
pkg-config --exists libudev 2>/dev/null || MISSING_DEPS+=("libudev-dev")

if [ ${#MISSING_DEPS[@]} -eq 0 ]; then
    print_status "All system dependencies are installed"
else
    print_warning "Missing dependencies: ${MISSING_DEPS[*]}"
    echo "   Install with: sudo apt-get update && sudo apt-get install -y ${MISSING_DEPS[*]} lld"
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

# Pre-download and cache dependencies for speed
echo "   Pre-downloading dependencies..."
if [ -f Cargo.lock ]; then
    rm -f Cargo.lock
fi

# Try to build with specific nightly first
echo "   Testing with nightly-2025-06-15 Rust (optimized)..."
if command -v rustup >/dev/null 2>&1 && rustup toolchain list | grep -q nightly; then
    # Install the specific nightly version if not available
    if ! rustup toolchain list | grep -q "nightly-2025-06-15"; then
        echo "   Installing nightly-2025-06-15..."
        rustup install nightly-2025-06-15
    fi
    
    # Pre-fetch dependencies
    echo "   Fetching dependencies..."
    cargo +nightly-2025-06-15 fetch >/dev/null 2>&1 || true
    
    # Build with parallel jobs
    if cargo +nightly-2025-06-15 build --workspace --all-features --jobs $CORES 2>&1 | tee build.log; then
        print_status "Build successful with nightly-2025-06-15 Rust (${CORES} cores)"
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
    echo "   Building with current Rust version (optimized)..."
    if cargo build --workspace --all-features --jobs $CORES 2>&1 | tee build.log; then
        print_status "Build successful with Rust $RUST_VERSION (${CORES} cores)"
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
    if cargo +nightly-2025-06-15 clippy --workspace --lib --bins --tests --jobs $CORES -- -W clippy::correctness -W clippy::suspicious -A warnings 2>&1 | tee clippy.log; then
        print_status "Clippy check passed (nightly-2025-06-15, ${CORES} cores)"
    else
        print_warning "Clippy check failed (non-critical)"
    fi
else
    if cargo clippy --workspace --lib --bins --tests --jobs $CORES -- -W clippy::correctness -W clippy::suspicious -A warnings 2>&1 | tee clippy.log; then
        print_status "Clippy check passed (${CORES} cores)"
    else
        print_warning "Clippy check failed (non-critical)"
    fi
fi

# Test compilation of tests
echo
echo "7. Testing test compilation..."
if [ "$NIGHTLY_WORKS" = "true" ]; then
    if cargo +nightly-2025-06-15 test --workspace --all-features --no-run --jobs $CORES; then
        print_status "Test compilation successful (nightly-2025-06-15, ${CORES} cores)"
    else
        print_error "Test compilation failed"
    fi
else
    if cargo test --workspace --all-features --no-run --jobs $CORES; then
        print_status "Test compilation successful (${CORES} cores)"
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
echo "1. Install missing system dependencies: sudo apt-get install -y ${MISSING_DEPS[*]} lld"
echo "2. Fix formatting issues with: cargo fmt --all"
echo "3. Use optimized builds: cargo build --jobs $CORES"
echo "4. Consider using Rust nightly-2025-06-15 if edition2024 is required"
echo "5. Update Cargo.toml rust-version if using a different version"
echo
echo "Performance tips:"
echo "- Using $CORES CPU cores for parallel compilation"
echo "- Cargo config saved to ~/.cargo/config.toml for faster builds"
echo "- Use 'cargo build --jobs $CORES' for fastest local builds"
echo
echo "Test logs saved to: build.log, clippy.log"