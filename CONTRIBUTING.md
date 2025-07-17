# Contributing to MultiVM

Thank you for your interest in contributing to MultiVM! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct:
- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Accept feedback gracefully

## Getting Started

1. **Fork the Repository**
   ```bash
   # Fork on GitHub, then clone your fork
   git clone https://github.com/YOUR_USERNAME/multivm-process
   cd multivm-process
   ```

2. **Set Up Development Environment**
   ```bash
   # Add upstream remote
   git remote add upstream https://github.com/original/multivm-process
   
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install development tools
   cargo install cargo-watch cargo-edit cargo-audit
   ```

3. **Build and Test**
   ```bash
   # Build the project
   cargo build
   
   # Run tests
   cargo test
   
   # Run with all features
   cargo test --all-features
   ```

## Development Workflow

### 1. Create a Feature Branch

```bash
# Update your fork
git checkout main
git pull upstream main
git push origin main

# Create feature branch
git checkout -b feature/your-feature-name
```

### 2. Make Your Changes

- Write clear, concise commit messages
- Follow the existing code style
- Add tests for new functionality
- Update documentation as needed

### 3. Code Style Guidelines

#### Rust Code
- Follow standard Rust formatting: `cargo fmt`
- Ensure no clippy warnings: `cargo clippy -- -D warnings`
- Use meaningful variable and function names
- Add doc comments for public APIs

Example:
```rust
/// Processes a MultiVM block by routing transactions to appropriate execution engines.
///
/// # Arguments
/// * `block` - The MultiVM block to process
///
/// # Returns
/// * `Ok(BlockResult)` - Processing results
/// * `Err(MultivmError)` - Processing error
pub async fn process_block(block: MultiVMBlock) -> MultivmResult<BlockResult> {
    // Implementation
}
```

#### Documentation
- Use Markdown for documentation
- Include code examples where appropriate
- Keep line length under 100 characters
- Use proper heading hierarchy

### 4. Testing

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test implementation
    }
}
```

#### Integration Tests
Place integration tests in the `tests/` directory:
```rust
// tests/integration_test.rs
use multivm_common::*;

#[tokio::test]
async fn test_full_system() {
    // Test implementation
}
```

### 5. Submit Pull Request

1. **Push to Your Fork**
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create Pull Request**
   - Go to GitHub and create a PR from your fork
   - Use a clear, descriptive title
   - Fill out the PR template
   - Link any related issues

3. **PR Guidelines**
   - Keep PRs focused and reasonably sized
   - Respond to review feedback promptly
   - Update your branch if main has changed
   - Ensure all CI checks pass

## Project Structure

Understanding the project structure helps you contribute effectively:

```
multivm-process/
├── multivm-common/          # Shared types and traits
├── multivm-consensus/       # Consensus implementation
├── multivm-p2p/            # Networking layer
├── multivm-process-manager/ # Process management
├── multivm-account-mapping/ # Account system
├── multivm-application/    # API server
├── multivm-cli/           # CLI tool
├── reth-execution-engine/  # Ethereum integration
└── solana-execution-engine/ # Solana integration
```

## Areas for Contribution

### Good First Issues
- Documentation improvements
- Test coverage increases
- Code cleanup and refactoring
- Bug fixes with clear reproduction steps

### Feature Development
- New API endpoints
- Performance optimizations
- Additional VM integrations
- Monitoring improvements

### Infrastructure
- CI/CD improvements
- Docker optimizations
- Deployment scripts
- Benchmarking tools

## Development Tips

### Running Specific Tests
```bash
# Test single module
cargo test -p multivm-consensus

# Test with output
cargo test -- --nocapture

# Run specific test
cargo test test_consensus_round
```

### Debugging
```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Use println! debugging (remember to remove)
println!("Debug: {:?}", variable);

# Use dbg! macro
dbg!(&variable);
```

### Performance Testing
```bash
# Run benchmarks
cargo bench

# Profile with flamegraph
cargo flamegraph --bin multivm-node
```

## Documentation

### Code Documentation
- Add doc comments to all public items
- Include examples in doc comments
- Run `cargo doc --open` to preview

### Project Documentation
- Update README.md for user-facing changes
- Add technical details to docs/
- Update CHANGELOG.md

## Release Process

1. **Version Bump**
   - Update version in Cargo.toml files
   - Update CHANGELOG.md
   - Create version tag

2. **Testing**
   - Run full test suite
   - Test on different platforms
   - Verify documentation builds

3. **Release**
   - Create GitHub release
   - Publish crates to crates.io
   - Update Docker images

## Getting Help

- **Discord**: Join our [Discord server](https://discord.gg/multivm)
- **Issues**: Check existing [issues](https://github.com/your-org/multivm-process/issues)
- **Discussions**: Start a [discussion](https://github.com/your-org/multivm-process/discussions)

## Recognition

Contributors will be:
- Listed in CONTRIBUTORS.md
- Mentioned in release notes
- Given credit in commit messages

Thank you for contributing to MultiVM! 🚀