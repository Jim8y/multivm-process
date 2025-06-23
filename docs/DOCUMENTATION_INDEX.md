# MultiVM Documentation Index

This index provides a clear overview of all documentation in the MultiVM project, organized by category.

## 📚 Getting Started

- [README.md](/README.md) - Project overview and quick start
- [Installation Guide](guides/INSTALLATION.md) - Detailed installation instructions
- [Configuration Guide](guides/CONFIGURATION.md) - Configuration reference
- [Deployment Guide](guides/DEPLOYMENT.md) - Production deployment instructions

## 🏗️ Architecture

- [Architecture Overview](architecture/ARCHITECTURE_OVERVIEW.md) - System architecture and design
- [Core Architecture](architecture/MULTIVM_CORE_ARCHITECTURE.md) - Detailed core components
- [Consensus Layer Design](architecture/CONSENSUS_LAYER_DESIGN.md) - Malachite integration
- [Application Layer Design](architecture/APPLICATION_LAYER_DESIGN.md) - API and services
- [IPC Protocols](architecture/IPC_PROTOCOLS.md) - Inter-process communication

## 🛡️ Security

- [Security Guide](references/SECURITY.md) - Comprehensive security documentation
- [Security Improvements Report](reports/SECURITY_IMPROVEMENTS_REPORT.md) - Recent security enhancements
- [Environment Auth](ENVIRONMENT_AUTH.md) - Authentication configuration

## 🧪 Development

- [Contributing Guide](/CONTRIBUTING.md) - How to contribute
- [Testing Guide](guides/TESTING_GUIDE.md) - Running and writing tests
- [Learning Guide](guides/LEARNING_GUIDE.md) - Understanding the codebase
- [Troubleshooting](guides/TROUBLESHOOTING.md) - Common issues and solutions

## 📊 Current Status

- [Final Verification Report](reports/FINAL_VERIFICATION_REPORT.md) - Latest comprehensive status
- [Production Status](status/PRODUCTION_STATUS.md) - Production readiness
- [Malachite Integration Status](reports/MALACHITE_INTEGRATION_STATUS.md) - Consensus implementation
- [Comprehensive Review](reports/COMPREHENSIVE_PROJECT_REVIEW.md) - Project analysis

## 🔧 Component Documentation

### Core Components
- [MultiVM Common](/crates/multivm-common/README.md) - Shared types and utilities
- [Process Manager](/crates/multivm-process-manager/README.md) - Process coordination
- [Consensus (Malachite)](/crates/multivm-consensus/README.md) - Consensus engine
- [Account Mapping](/crates/multivm-account-mapping/README.md) - Cross-VM accounts

### Execution Engines
- [Reth Execution Engine](/crates/reth-execution-engine/README.md) - Ethereum execution
- [Solana Execution Engine](/crates/solana-execution-engine/README.md) - Solana execution

### Infrastructure
- [P2P Networking](/crates/multivm-p2p/README.md) - Network layer
- [Application Layer](/crates/multivm-application/README.md) - APIs and services

## 📝 Configuration

- [Unified Configuration Schema](/config/README.md) - Configuration management
- [Example Configurations](/config/examples/) - Sample configuration files

## 🚀 Deployment

- [Docker Deployment](/deploy/docker/README.md) - Container deployment
- [Docker Testing](deployment/DOCKER_TESTING.md) - Testing with Docker
- [Scripts](/scripts/README.md) - Utility scripts

## 📈 Reports and Analysis

- [Security Improvements](reports/SECURITY_IMPROVEMENTS_REPORT.md) - Security enhancements
- [Consistency Fixes](reports/CONSISTENCY_FIXES_SUMMARY.md) - Configuration consistency
- [Mock Improvements](reports/MOCK_CONSISTENCY_IMPROVEMENTS.md) - Mock process updates
- [Demo Results](reports/demo_results.md) - Demonstration outcomes

## 🗂️ Project Organization

- [Project Structure](/PROJECT_STRUCTURE.md) - Directory layout and organization
- [Changelog](/CHANGELOG.md) - Version history
- [Concurrency Analysis](/CONCURRENCY_ANALYSIS.md) - Concurrency design patterns

## ⚠️ Archived Documentation

Older status reports and duplicate documentation have been moved to `docs/archive/` to maintain a clean documentation structure while preserving historical information.

---

**Note**: This index is the authoritative guide to MultiVM documentation. If you find any broken links or missing documentation, please report it in the project issues.