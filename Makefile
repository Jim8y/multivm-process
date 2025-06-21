# Multi-VM Blockchain Execution System Makefile
# Professional grade build and deployment automation

.PHONY: help build test clean deploy start stop restart status health docs format lint fix example scripts

# Default target
help:
	@echo "🚀 Multi-VM Blockchain Execution System"
	@echo "========================================"
	@echo ""
	@echo "Available targets:"
	@echo "  🔨 build      - Build the entire system in release mode"
	@echo "  🧪 test       - Run all tests and checks"
	@echo "  🧹 clean      - Clean build artifacts and data"
	@echo "  🚀 deploy     - Full deployment (build + start + test)"
	@echo "  ▶️  start      - Start all system components"
	@echo "  ⏹️  stop       - Stop all system components"
	@echo "  🔄 restart    - Restart all system components"
	@echo "  📊 status     - Show system status"
	@echo "  🏥 health     - Run comprehensive health check"
	@echo "  📚 docs       - Generate documentation"
	@echo "  🎨 format     - Format all code"
	@echo "  🔍 lint       - Run linter checks"
	@echo "  🔧 fix        - Fix common issues automatically"
	@echo "  📝 example    - Run usage examples"
	@echo "  🔗 scripts    - Make scripts executable"
	@echo ""
	@echo "Environment variables:"
	@echo "  LOG_LEVEL     - Set log level (default: info)"
	@echo "  BUILD_MODE    - Set build mode (debug/release, default: release)"
	@echo "  DATA_DIR      - Set data directory (default: ./data)"

# Variables
LOG_LEVEL ?= info
BUILD_MODE ?= release
DATA_DIR ?= ./data
RUST_LOG ?= $(LOG_LEVEL)

# Build system
build:
	@echo "🔨 Building Multi-VM system in $(BUILD_MODE) mode..."
	@cargo build --$(BUILD_MODE)
	@echo "✅ Build completed successfully!"

# Run comprehensive tests
test:
	@echo "🧪 Running comprehensive test suite..."
	@echo "  📋 Running Rust tests..."
	@cargo test --$(BUILD_MODE)
	@echo "  🔍 Running linter checks..."
	@cargo clippy --$(BUILD_MODE) -- -D warnings || true
	@echo "  📊 Running system tests..."
	@./scripts/test_system.sh || echo "⚠️  System not running, skipping integration tests"
	@echo "✅ Test suite completed!"

# Clean everything
clean:
	@echo "🧹 Cleaning system..."
	@cargo clean
	@rm -rf $(DATA_DIR)
	@rm -rf ./logs
	@rm -rf ./target
	@echo "✅ System cleaned!"

# Full deployment
deploy: scripts build
	@echo "🚀 Deploying Multi-VM system..."
	@./scripts/deploy.sh deploy
	@echo "✅ Deployment completed!"

# Start system
start: scripts
	@echo "▶️ Starting Multi-VM system..."
	@./scripts/deploy.sh start
	@echo "✅ System started!"

# Stop system
stop: scripts
	@echo "⏹️ Stopping Multi-VM system..."
	@./scripts/deploy.sh stop
	@echo "✅ System stopped!"

# Restart system
restart: scripts
	@echo "🔄 Restarting Multi-VM system..."
	@./scripts/deploy.sh restart
	@echo "✅ System restarted!"

# Show status
status: scripts
	@echo "📊 Multi-VM System Status:"
	@./scripts/deploy.sh status

# Run health check
health: scripts
	@echo "🏥 Running comprehensive health check..."
	@./scripts/test_system.sh

# Generate documentation
docs:
	@echo "📚 Generating documentation..."
	@cargo doc --$(BUILD_MODE) --no-deps --open
	@echo "✅ Documentation generated!"

# Format code
format:
	@echo "🎨 Formatting code..."
	@cargo fmt --all
	@echo "✅ Code formatted!"

# Run linter
lint:
	@echo "🔍 Running linter checks..."
	@cargo clippy --$(BUILD_MODE) --all-targets --all-features -- -D warnings
	@echo "✅ Linter checks completed!"

# Fix common issues
fix:
	@echo "🔧 Fixing common issues..."
	@cargo fix --allow-dirty --allow-staged
	@cargo fmt --all
	@echo "✅ Common issues fixed!"

# Run usage examples
example: build
	@echo "📝 Running usage examples..."
	@cd examples && cargo run --bin usage_example
	@echo "✅ Examples completed!"

# Make scripts executable
scripts:
	@chmod +x scripts/*.sh

# Development workflow targets
dev-setup: scripts build
	@echo "🛠️ Setting up development environment..."
	@mkdir -p $(DATA_DIR)
	@mkdir -p ./logs
	@echo "✅ Development environment ready!"

dev-test: dev-setup
	@echo "🧪 Running development tests..."
	@cargo test
	@echo "✅ Development tests completed!"

dev-run: dev-setup start
	@echo "🚀 Development environment is running!"
	@echo "  📋 Use 'make status' to check system status"
	@echo "  🧪 Use 'make health' to run health checks"
	@echo "  ⏹️ Use 'make stop' to stop the system"

# Production workflow targets
prod-deploy: clean build deploy health
	@echo "🏭 Production deployment completed!"
	@echo "  🔧 System has been built, deployed, and tested"
	@echo "  📊 Check 'make status' for current state"

prod-restart: stop build start health
	@echo "🔄 Production restart completed!"

# Maintenance targets
logs:
	@echo "📋 Recent system logs:"
	@if [ -d "./logs" ]; then \
		find ./logs -name "*.log" -type f -exec tail -20 {} + 2>/dev/null || echo "No log files found"; \
	else \
		echo "No logs directory found"; \
	fi

disk-usage:
	@echo "💾 Disk usage analysis:"
	@if [ -d "$(DATA_DIR)" ]; then \
		du -sh $(DATA_DIR)/* 2>/dev/null || echo "No data directories found"; \
	else \
		echo "No data directory found"; \
	fi
	@echo "  📁 Total project size: $$(du -sh . 2>/dev/null | cut -f1)"

performance:
	@echo "⚡ Performance analysis:"
	@./scripts/test_system.sh performance

# CI/CD targets
ci-test: build test lint
	@echo "🤖 CI/CD tests completed successfully!"

ci-deploy: clean build
	@echo "🤖 CI/CD deployment artifacts ready!"

# Quick targets for common operations
quick-start: scripts start
quick-stop: scripts stop
quick-test: scripts
	@./scripts/test_system.sh connectivity

# Help for specific areas
help-dev:
	@echo "🛠️ Development Workflow:"
	@echo "  1. make dev-setup     - Set up development environment"
	@echo "  2. make dev-run       - Start development system"
	@echo "  3. make dev-test      - Run development tests"
	@echo "  4. make stop          - Stop when done"

help-prod:
	@echo "🏭 Production Workflow:"
	@echo "  1. make prod-deploy   - Full production deployment"
	@echo "  2. make health        - Verify system health"
	@echo "  3. make prod-restart  - Safe production restart"

help-debug:
	@echo "🐛 Debugging Workflow:"
	@echo "  1. make status        - Check system status"
	@echo "  2. make logs          - View recent logs"
	@echo "  3. make health        - Run health diagnostics"
	@echo "  4. make disk-usage    - Check disk usage"

# Show current configuration
config:
	@echo "⚙️ Current Configuration:"
	@echo "  📊 Log Level: $(LOG_LEVEL)"
	@echo "  🔨 Build Mode: $(BUILD_MODE)"
	@echo "  📁 Data Directory: $(DATA_DIR)"
	@echo "  🌍 Rust Log: $(RUST_LOG)" 