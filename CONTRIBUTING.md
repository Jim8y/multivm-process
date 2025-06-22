# Contributing to MultiVM

Thank you for your interest in contributing to MultiVM! This document provides guidelines and information for contributors.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Issue Reporting](#issue-reporting)

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Docker and Docker Compose
- Git

### Setup Development Environment

1. **Clone the repository:**
   ```bash
   git clone https://github.com/vm-multiverse/multivm.git
   cd multivm
   ```

2. **Install development tools:**
   ```bash
   make install
   ```

3. **Set up the development environment:**
   ```bash
   make setup
   ```

4. **Run tests to verify setup:**
   ```bash
   make test
   ```

## Development Workflow

### Standard Development Process

1. **Create a feature branch:**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes following our code standards**

3. **Run the full test suite:**
   ```bash
   make all  # Runs format, lint, build, and test
   ```

4. **Commit your changes:**
   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

5. **Push and create a pull request:**
   ```bash
   git push origin feature/your-feature-name
   ```

### Available Commands

```bash
# Development
make dev                # Start development environment
make build              # Build the project
make test               # Run all tests
make check              # Run all checks (format, lint, etc.)

# Code Quality
make format             # Format code with rustfmt
make lint               # Run clippy lints
make audit              # Run security audit

# Docker
make docker             # Build Docker image
make deploy             # Deploy to environment
```

## Code Standards

### Rust Style Guidelines

We follow the standard Rust style guidelines with these specific requirements:

1. **Use `rustfmt` for formatting:**
   ```bash
   cargo fmt --all
   ```

2. **Address all Clippy warnings:**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

3. **Write comprehensive documentation:**
   - All public functions must have documentation
   - Include examples in documentation where appropriate
   - Update README.md for significant changes

### Code Organization

- **crates/**: Core library crates
- **scripts/**: Build, test, and deployment scripts
- **docs/**: Comprehensive documentation
- **examples/**: Usage examples and demos
- **tests/**: Integration tests

### Naming Conventions

- Use `snake_case` for functions and variables
- Use `PascalCase` for types and structs
- Use `SCREAMING_SNAKE_CASE` for constants
- Use descriptive names that clearly indicate purpose

### Error Handling

- Use `Result<T, E>` for operations that can fail
- Create custom error types for different error categories
- Provide meaningful error messages
- Log errors appropriately with context

### Documentation Standards

- Use triple-slash comments (`///`) for public API documentation
- Include examples in documentation:
  ```rust
  /// Creates a new MultiVM coordinator
  /// 
  /// # Examples
  /// 
  /// ```rust
  /// use multivm_core::MultivmCoordinator;
  /// 
  /// let coordinator = MultivmCoordinator::new(config)?;
  /// ```
  pub fn new(config: CoordinatorConfig) -> Result<Self, MultivmError> {
      // Implementation
  }
  ```

## Testing

### Test Categories

1. **Unit Tests**: Test individual functions and modules
2. **Integration Tests**: Test component interaction
3. **End-to-End Tests**: Test complete workflows
4. **Performance Tests**: Benchmark critical paths

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = setup_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_function() {
        // Test async functions
    }
}
```

### Running Tests

```bash
# Run all tests
make test

# Run specific test types
make test-unit
make test-integration
make test-consensus

# Run with coverage
make test-coverage
```

## Pull Request Process

### Before Submitting

1. **Ensure all tests pass:**
   ```bash
   make all
   ```

2. **Update documentation** if needed

3. **Add tests** for new functionality

4. **Check for breaking changes** and update accordingly

### PR Guidelines

1. **Use descriptive titles:**
   - `feat: add new consensus mechanism`
   - `fix: resolve memory leak in block processor`
   - `docs: update API documentation`

2. **Include comprehensive description:**
   - What changes were made
   - Why the changes were necessary
   - How to test the changes
   - Any breaking changes

3. **Reference related issues:**
   ```
   Closes #123
   Related to #456
   ```

### Review Process

1. All PRs require at least one review
2. All CI checks must pass
3. Documentation must be updated for API changes
4. Breaking changes require special approval

## Issue Reporting

### Bug Reports

Include the following information:

1. **Environment details:**
   - OS and version
   - Rust version
   - MultiVM version

2. **Reproduction steps:**
   - Minimal code example
   - Expected behavior
   - Actual behavior

3. **Additional context:**
   - Log output
   - Configuration files
   - Error messages

### Feature Requests

1. **Clear description** of the proposed feature
2. **Use cases** and motivation
3. **Proposed API** or interface
4. **Alternatives considered**

### Security Issues

**Do not open public issues for security vulnerabilities.**

Instead, email security issues to: security@multivm.dev

## Code of Conduct

### Our Standards

- **Be respectful** and inclusive
- **Be collaborative** and constructive
- **Focus on what's best** for the community
- **Show empathy** towards other community members

### Unacceptable Behavior

- Harassment or discrimination
- Trolling or inflammatory comments
- Publishing private information
- Any conduct that could be considered inappropriate in a professional setting

## Development Tips

### Useful Tools

```bash
# Install additional development tools
cargo install cargo-watch     # Auto-rebuild on changes
cargo install cargo-expand    # Show macro expansions
cargo install cargo-outdated  # Check for outdated dependencies
```

### IDE Setup

We recommend using:
- **VS Code** with rust-analyzer extension
- **IntelliJ IDEA** with Rust plugin
- **Vim/Neovim** with rust.vim and coc-rust-analyzer

### Performance Profiling

```bash
# Profile with cargo-flamegraph
cargo install flamegraph
cargo flamegraph --bin multivm-node

# Profile with perf (Linux)
perf record --call-graph=dwarf target/release/multivm-node
perf report
```

## Questions?

- **Documentation**: Check the [docs/](docs/) directory
- **Examples**: See [examples/](examples/) directory
- **Issues**: Open a GitHub issue
- **Discussions**: Use GitHub Discussions for questions

Thank you for contributing to MultiVM!