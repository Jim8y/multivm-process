#!/bin/bash
# Verify production environment is correctly configured

echo "🔍 MultiVM Production Environment Verification"
echo "============================================="

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ERRORS=0
WARNINGS=0

# Function to check status
check() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $2"
    else
        echo -e "${RED}✗${NC} $2"
        ((ERRORS++))
    fi
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
    ((WARNINGS++))
}

# Check Docker
echo -e "\n📦 Checking Docker..."
docker --version >/dev/null 2>&1
check $? "Docker installed"

docker compose version >/dev/null 2>&1
check $? "Docker Compose installed"

# Check Python
echo -e "\n🐍 Checking Python..."
python3 --version >/dev/null 2>&1
check $? "Python 3 installed"

# Check production scripts exist
echo -e "\n📜 Checking production scripts..."
SCRIPTS=(
    "generate-validator-accounts.py"
    "auto-fund-validators.py"
    "send-real-transactions-final.py"
    "fund-accounts.py"
    "complete-funding-setup.sh"
    "fund-validators-complete.sh"
    "setup-production-testnet.sh"
    "run-complete-setup.sh"
)

for script in "${SCRIPTS[@]}"; do
    if [ -f "$script" ]; then
        check 0 "$script exists"
    else
        check 1 "$script missing"
    fi
done

# Check testnet directory
echo -e "\n📁 Checking testnet configuration..."
if [ -d "testnet-production-complete" ]; then
    check 0 "testnet-production-complete directory exists"
    
    # Check critical files
    [ -f "testnet-production-complete/docker-compose.yml" ]
    check $? "docker-compose.yml exists"
    
    [ -f "testnet-production-complete/validators_complete.json" ]
    check $? "validators_complete.json exists"
    
    [ -f "testnet-production-complete/jwt.hex" ]
    check $? "jwt.hex exists"
else
    check 1 "testnet-production-complete directory missing"
fi

# Check if testnet is running
echo -e "\n🚀 Checking testnet status..."
if cd testnet-production-complete 2>/dev/null && docker compose ps --quiet 2>/dev/null | grep -q .; then
    RUNNING_CONTAINERS=$(docker compose ps --quiet | wc -l)
    EXPECTED_CONTAINERS=15  # 7 multivm + 7 reth + 1 explorer
    
    if [ "$RUNNING_CONTAINERS" -eq "$EXPECTED_CONTAINERS" ]; then
        check 0 "All $EXPECTED_CONTAINERS containers running"
    else
        warn "Only $RUNNING_CONTAINERS/$EXPECTED_CONTAINERS containers running"
    fi
    
    cd - >/dev/null
else
    warn "Testnet not running"
fi

# Check RPC connectivity
echo -e "\n🌐 Checking RPC endpoints..."
if curl -s -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
    http://localhost:8545 >/dev/null 2>&1; then
    check 0 "RPC endpoint responsive (port 8545)"
else
    warn "RPC endpoint not accessible"
fi

# Check explorer
echo -e "\n🔍 Checking blockchain explorer..."
if curl -s -f http://localhost:3000 >/dev/null 2>&1; then
    check 0 "Blockchain explorer accessible"
else
    warn "Blockchain explorer not accessible"
fi

# Check Python environment
echo -e "\n🔧 Checking Python environment..."
if [ -d "venv" ]; then
    check 0 "Python virtual environment exists"
    
    # Check if web3 is installed
    if venv/bin/python3 -c "import web3" 2>/dev/null; then
        check 0 "web3 library installed"
    else
        warn "web3 library not installed (needed for auto-funding)"
    fi
else
    warn "Python virtual environment not created"
fi

# Check documentation
echo -e "\n📚 Checking documentation..."
DOCS=(
    "README.md"
    "FUNDING_INSTRUCTIONS.md"
    "PRODUCTION_DEPLOYMENT_GUIDE.md"
    "API_REFERENCE.md"
    "CONTRIBUTING.md"
    "SECURITY.md"
)

for doc in "${DOCS[@]}"; do
    [ -f "$doc" ]
    check $? "$doc exists"
done

# Summary
echo -e "\n📊 Verification Summary"
echo "======================"
echo -e "✅ Passed checks: $((${#SCRIPTS[@]} + ${#DOCS[@]} + 10 - ERRORS - WARNINGS))"
echo -e "⚠️  Warnings: $WARNINGS"
echo -e "❌ Errors: $ERRORS"

if [ $ERRORS -eq 0 ]; then
    echo -e "\n${GREEN}✨ Production environment is ready!${NC}"
    
    if [ $WARNINGS -gt 0 ]; then
        echo -e "\n💡 To start the testnet:"
        echo "   cd testnet-production-complete && docker compose up -d"
        echo -e "\n💡 To setup funding:"
        echo "   python3 -m venv venv"
        echo "   source venv/bin/activate"
        echo "   pip install web3 eth-account"
        echo "   python3 auto-fund-validators.py"
    fi
else
    echo -e "\n${RED}❌ Production environment has errors!${NC}"
    echo "Please fix the errors above before proceeding."
fi

exit $ERRORS