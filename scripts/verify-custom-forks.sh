#!/bin/bash
set -e

echo "🔍 Verifying MultiVM Custom Fork Configuration"
echo "============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Expected configurations
EXPECTED_RETH_REPO="git@github.com:vm-multiverse/reth.git"
EXPECTED_RETH_BRANCH="dev"
EXPECTED_SOLANA_REPO="git@github.com:vm-multiverse/multivm-agave.git"
EXPECTED_SOLANA_BRANCH="master"

echo -e "\n${BLUE}Expected Configuration:${NC}"
echo "Reth Repository: $EXPECTED_RETH_REPO ($EXPECTED_RETH_BRANCH branch)"
echo "Solana Repository: $EXPECTED_SOLANA_REPO ($EXPECTED_SOLANA_BRANCH branch)"

# Function to check git remote
check_git_remote() {
    local dir="$1"
    local expected_repo="$2"
    local expected_branch="$3"
    
    if [ -d "$dir/.git" ]; then
        cd "$dir"
        local remote_url=$(git remote get-url origin 2>/dev/null || echo "")
        local current_branch=$(git branch --show-current 2>/dev/null || echo "")
        
        if [ "$remote_url" = "$expected_repo" ] || [ "$remote_url" = "${expected_repo%.git}.git" ]; then
            echo -e "${GREEN}✅ Correct repository${NC}"
        else
            echo -e "${RED}❌ Wrong repository: $remote_url${NC}"
            return 1
        fi
        
        if [ "$current_branch" = "$expected_branch" ]; then
            echo -e "${GREEN}✅ Correct branch: $current_branch${NC}"
        else
            echo -e "${YELLOW}⚠ Wrong branch: $current_branch (expected: $expected_branch)${NC}"
            return 1
        fi
        
        cd - > /dev/null
        return 0
    else
        echo -e "${YELLOW}⚠ Directory not found or not a git repository${NC}"
        return 1
    fi
}

# Check test binary directories
echo -e "\n${BLUE}Checking test binary directories...${NC}"

if [ -d "/tmp/multivm-test-binaries/reth" ]; then
    echo -e "\n${YELLOW}Reth test directory:${NC}"
    check_git_remote "/tmp/multivm-test-binaries/reth" "$EXPECTED_RETH_REPO" "$EXPECTED_RETH_BRANCH"
fi

if [ -d "/tmp/multivm-test-binaries/multivm-agave" ]; then
    echo -e "\n${YELLOW}Solana test directory:${NC}"
    check_git_remote "/tmp/multivm-test-binaries/multivm-agave" "$EXPECTED_SOLANA_REPO" "$EXPECTED_SOLANA_BRANCH"
fi

# Check local development directories
echo -e "\n${BLUE}Checking local development directories...${NC}"

if [ -d "./reth" ]; then
    echo -e "\n${YELLOW}Local Reth directory:${NC}"
    check_git_remote "./reth" "$EXPECTED_RETH_REPO" "$EXPECTED_RETH_BRANCH"
fi

if [ -d "./multivm-agave" ]; then
    echo -e "\n${YELLOW}Local Solana directory:${NC}"
    check_git_remote "./multivm-agave" "$EXPECTED_SOLANA_REPO" "$EXPECTED_SOLANA_BRANCH"
fi

# Check binary versions if available
echo -e "\n${BLUE}Checking installed binaries...${NC}"

check_binary_info() {
    local binary="$1"
    local binary_path=$(which "$binary" 2>/dev/null || echo "")
    
    if [ -n "$binary_path" ]; then
        echo -e "\n${YELLOW}$binary:${NC}"
        echo "Path: $binary_path"
        
        # Check if it's a symlink
        if [ -L "$binary_path" ]; then
            local target=$(readlink -f "$binary_path")
            echo "Symlink target: $target"
            
            # Check if it points to our custom build
            if [[ "$target" == *"/multivm-test-binaries/"* ]]; then
                echo -e "${GREEN}✅ Using custom fork binary${NC}"
            else
                echo -e "${YELLOW}⚠ Not using custom fork binary${NC}"
            fi
        fi
        
        # Show version if possible
        $binary --version 2>/dev/null | head -1 || echo "Version info not available"
    else
        echo -e "\n${YELLOW}$binary: Not found${NC}"
    fi
}

check_binary_info "reth"
check_binary_info "reth-multivm"
check_binary_info "solana-test-validator"
check_binary_info "solana-test-validator-multivm"

# Verify configuration files
echo -e "\n${BLUE}Checking configuration files...${NC}"

echo -e "\n${YELLOW}Scripts using custom forks:${NC}"
grep -l "vm-multiverse" scripts/*.sh 2>/dev/null | while read script; do
    echo -e "${GREEN}✅ $(basename $script)${NC}"
done

echo -e "\n${YELLOW}Docker configurations:${NC}"
if [ -f "docker-compose.testnet-reth.yml" ]; then
    echo -e "${GREEN}✅ docker-compose.testnet-reth.yml${NC}"
fi
if [ -f "docker-compose.testnet-solana.yml" ]; then
    echo -e "${GREEN}✅ docker-compose.testnet-solana.yml${NC}"
fi

# Summary
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "\nTo ensure you're using the correct custom forks:"
echo -e "1. Run ${GREEN}./scripts/setup-test-binaries.sh${NC} to build custom binaries"
echo -e "2. Source the configuration: ${GREEN}source /tmp/multivm-test-binaries/test-binaries.env${NC}"
echo -e "3. Run integration tests: ${GREEN}./scripts/run-integration-tests-custom.sh${NC}"
echo -e "\n${YELLOW}Remember:${NC} Never use official Reth or Solana binaries!"
echo -e "Always use:"
echo -e "  - Reth: ${BLUE}$EXPECTED_RETH_REPO${NC} (${BLUE}$EXPECTED_RETH_BRANCH${NC} branch)"
echo -e "  - Solana: ${BLUE}$EXPECTED_SOLANA_REPO${NC} (${BLUE}$EXPECTED_SOLANA_BRANCH${NC} branch)"