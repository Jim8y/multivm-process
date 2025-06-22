# MultiVM Professional Makefile

.PHONY: help build test clean install dev deploy docker lint format check all

# Configuration
CARGO = cargo
DOCKER = docker
BUILD_TYPE ?= debug
VERSION ?= latest
ENVIRONMENT ?= development

# Help target
help: ## Show this help message
	@echo "MultiVM Build System"
	@echo "===================="
	@echo ""
	@echo "Available targets:"
	@echo "  build       Build the project"
	@echo "  test        Run all tests"
	@echo "  clean       Clean build artifacts"
	@echo "  format      Format code"
	@echo "  lint        Run linting"
	@echo "  check       Run all checks"
	@echo "  dev         Start development environment"
	@echo "  docker      Build Docker image"
	@echo "  deploy      Deploy to environment"
	@echo "  all         Run all checks and build"

# Build targets
build: ## Build the project
	@echo "Building MultiVM ($(BUILD_TYPE))..."
	@./scripts/build/build.sh $(if $(filter release,$(BUILD_TYPE)),--release)

test: ## Run all tests
	@./scripts/test/run-tests.sh

clean: ## Clean build artifacts
	@echo "Cleaning..."
	@$(CARGO) clean
	@rm -rf logs/*.log temp/* build/*

# Code quality
format: ## Format code
	@$(CARGO) fmt --all

lint: ## Run clippy lints  
	@$(CARGO) clippy --all-targets --all-features -- -D warnings

check: format lint ## Run all checks
	@$(CARGO) check --all-targets --all-features

# Development
dev: ## Start development environment
	@./scripts/dev/run-single-node.sh

# Docker
docker: ## Build Docker image
	@$(DOCKER) build -t multivm:$(VERSION) .

# Deployment  
deploy: ## Deploy to environment
	@./scripts/deploy/deploy.sh --environment $(ENVIRONMENT)

# Meta targets
all: check build test ## Run everything
	@echo "All tasks completed!"

install: ## Install development tools
	@$(CARGO) install cargo-audit cargo-tarpaulin