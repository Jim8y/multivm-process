# Contributing to MultiVM

Thank you for your interest in contributing to the MultiVM project! This document provides guidelines and information for contributors.

## 🚀 Getting Started

### Prerequisites

- **Rust**: 1.70 or later with Cargo
- **Git**: For version control
- **Redis**: For caching (development)
- **PostgreSQL**: For persistent storage (development)

### Development Setup

1. **Fork and clone the repository**:
   ```bash
   git clone https://github.com/your-username/multivm-process.git
   cd multivm-process
   ```

2. **Install dependencies**:
   ```bash
   # Install Rust if not already installed
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install required tools
   cargo install cargo-watch cargo-audit
   ```

3. **Build the project**:
   ```bash
   cargo build
   ```

4. **Run tests**:
   ```bash
   cargo test --workspace
   ```

5. **Start development services** (optional):
   ```bash
   # Start Redis
   redis-server
   
   # Start PostgreSQL
   pg_ctl -D /usr/local/var/postgres start
   ```

## 📋 Development Workflow

### Branch Strategy

- **`main`**: Production-ready code
- **`develop`**: Integration branch for features
- **`feature/*`**: Feature development branches
- **`bugfix/*`**: Bug fix branches
- **`hotfix/*`**: Critical production fixes

### Making Changes

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**:
   - Follow the coding standards (see below)
   - Add tests for new functionality
   - Update documentation as needed

3. **Test your changes**:
   ```bash
   # Run all tests
   cargo test --workspace
   
   # Run specific component tests
   cargo test -p multivm-account-mapping
   
   # Run with coverage (if available)
   cargo test --workspace -- --nocapture
   ```

4. **Check code quality**:
   ```bash
   # Format code
   cargo fmt
   
   # Run linter
   cargo clippy -- -D warnings
   
   # Security audit
   cargo audit
   ```

5. **Commit your changes**:
   ```bash
   git add .
   git commit -m "feat: add new cross-VM transaction type"
   ```

6. **Push and create PR**:
   ```bash
   git push origin feature/your-feature-name
   ```

## 📝 Coding Standards

### Rust Style Guide

- Follow the official [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/)
- Use `cargo fmt` for consistent formatting
- Use `cargo clippy` to catch common mistakes

### Code Organization

```
crates/
├── multivm-common/          # Shared types and utilities
├── multivm-application/     # Main application server
├── multivm-account-mapping/ # Cross-VM account management
├── multivm-consensus/       # Consensus implementation
├── multivm-p2p/            # P2P networking
├── multivm-process-manager/ # Process lifecycle
├── multivm-cli/            # Command-line tools
├── solana-execution-engine/ # Solana VM integration
├── reth-execution-engine/   # Ethereum VM integration
└── multivm-mock-processes/  # Testing utilities
```

### Naming Conventions

- **Crates**: `kebab-case` (e.g., `multivm-account-mapping`)
- **Modules**: `snake_case` (e.g., `account_mapping`)
- **Types**: `PascalCase` (e.g., `AccountBinding`)
- **Functions**: `snake_case` (e.g., `bind_accounts`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `MAX_RETRIES`)

### Documentation

- Add doc comments for all public APIs
- Include examples in doc comments where helpful
- Update README files for significant changes

```rust
/// Binds accounts across different virtual machines.
/// 
/// # Arguments
/// 
/// * `solana_address` - The Solana account address
/// * `ethereum_address` - The Ethereum account address
/// * `proof` - Cryptographic proof of ownership
/// 
/// # Example
/// 
/// ```rust
/// let binding = bind_accounts(
///     "11111111111111111111111111111112",
///     "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C",
///     proof_data
/// ).await?;
/// ```
pub async fn bind_accounts(
    solana_address: &str,
    ethereum_address: &str, 
    proof: &[u8]
) -> Result<AccountBinding, MultivmError> {
    // Implementation
}
```

### Error Handling

- Use the unified `MultivmError` type from `multivm-common`
- Provide meaningful error messages
- Include context where helpful

```rust
use multivm_common::{MultivmError, MultivmResult};

pub fn validate_address(address: &str) -> MultivmResult<()> {
    if address.is_empty() {
        return Err(MultivmError::Validation {
            field: "address".to_string(),
            message: "Address cannot be empty".to_string(),
            value: address.to_string(),
        });
    }
    Ok(())
}
```

### Testing

- Write unit tests for all public functions
- Use integration tests for component interactions
- Mock external dependencies in tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_account_binding() {
        let binding = AccountBinding::new(
            "test_solana_address",
            "test_ethereum_address",
            &[1, 2, 3, 4]
        );
        
        assert_eq!(binding.solana_address, "test_solana_address");
        assert_eq!(binding.ethereum_address, "test_ethereum_address");
    }
}
```

## 🔍 Code Review Process

### Submitting Pull Requests

1. **Ensure your PR**:
   - Has a clear title and description
   - References any related issues
   - Includes tests for new functionality
   - Updates documentation as needed
   - Passes all CI checks

2. **PR Template**:
   ```markdown
   ## Description
   Brief description of changes
   
   ## Type of Change
   - [ ] Bug fix
   - [ ] New feature
   - [ ] Breaking change
   - [ ] Documentation update
   
   ## Testing
   - [ ] Unit tests added/updated
   - [ ] Integration tests added/updated
   - [ ] Manual testing completed
   
   ## Checklist
   - [ ] Code follows style guidelines
   - [ ] Self-review completed
   - [ ] Documentation updated
   - [ ] No new warnings introduced
   ```

### Review Criteria

Reviewers will check for:

- **Correctness**: Does the code work as intended?
- **Performance**: Are there any performance implications?
- **Security**: Are there any security concerns?
- **Maintainability**: Is the code easy to understand and maintain?
- **Testing**: Are there adequate tests?
- **Documentation**: Is the code properly documented?

## 🐛 Reporting Issues

### Bug Reports

When reporting bugs, please include:

- **Environment**: OS, Rust version, component versions
- **Steps to reproduce**: Clear steps to reproduce the issue
- **Expected behavior**: What should happen
- **Actual behavior**: What actually happens
- **Logs**: Relevant log output or error messages

### Feature Requests

For feature requests, please include:

- **Use case**: Why is this feature needed?
- **Proposed solution**: How should it work?
- **Alternatives**: Any alternative approaches considered?
- **Impact**: Who would benefit from this feature?

## 🏗️ Architecture Guidelines

### Component Design

- **Single Responsibility**: Each component should have a clear, single purpose
- **Loose Coupling**: Minimize dependencies between components
- **High Cohesion**: Related functionality should be grouped together
- **Interface Segregation**: Use traits to define clear interfaces

### Performance Considerations

- **Async/Await**: Use async programming for I/O operations
- **Memory Management**: Be mindful of memory allocations
- **Caching**: Implement appropriate caching strategies
- **Monitoring**: Add metrics for performance tracking

### Security Best Practices

- **Input Validation**: Validate all inputs
- **Authentication**: Implement proper authentication
- **Authorization**: Check permissions appropriately
- **Encryption**: Use encryption for sensitive data

## 📚 Resources

### Documentation

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)

### Tools

- [Rust Analyzer](https://rust-analyzer.github.io/): IDE support
- [Cargo Watch](https://github.com/watchexec/cargo-watch): Auto-rebuild on changes
- [Cargo Audit](https://github.com/RustSec/rustsec/tree/main/cargo-audit): Security auditing

## 🤝 Community

### Communication

- **GitHub Issues**: For bug reports and feature requests
- **GitHub Discussions**: For general questions and discussions
- **Pull Requests**: For code contributions

### Code of Conduct

We are committed to providing a welcoming and inclusive environment for all contributors. Please be respectful and professional in all interactions.

## 📄 License

By contributing to MultiVM, you agree that your contributions will be licensed under the MIT License.

## 🙏 Recognition

Contributors will be recognized in:

- The project README
- Release notes for significant contributions
- The project's contributor list

Thank you for contributing to MultiVM! 🚀