# MultiVM Development Tools

This directory contains various tools for development, analysis, and maintenance of the MultiVM project.

## Directory Structure

```
tools/
├── analysis/              # Code and system analysis tools
│   ├── analyze_abstraction.sh
│   └── consensus_verification.rs
├── benchmarks/           # Performance benchmarking tools
│   ├── benchmark_system.sh
│   └── results/         # Benchmark results
└── README.md            # This file
```

## Analysis Tools

### analyze_abstraction.sh
Analyzes code abstractions and patterns across the codebase.

Usage:
```bash
./tools/analysis/analyze_abstraction.sh
```

### consensus_verification.rs
Tool for verifying consensus algorithm correctness and performance.

Usage:
```bash
cargo run --bin consensus_verification
```

## Benchmarking Tools

### benchmark_system.sh
Comprehensive system benchmarking script that measures:
- Build performance
- Test execution time
- Runtime performance
- Memory usage
- Consensus throughput

Usage:
```bash
./tools/benchmarks/benchmark_system.sh
```

Results are stored in `tools/benchmarks/results/` with timestamps.

### Benchmark Results
The results directory contains historical benchmark data:
- **compile_times.csv**: Compilation performance over time
- **runtime_metrics.json**: Runtime performance measurements
- **consensus_throughput.csv**: Consensus algorithm performance

## Using the Tools

### Development Workflow
```bash
# 1. Run analysis tools before making changes
./tools/analysis/analyze_abstraction.sh

# 2. Make your changes
# ... code changes ...

# 3. Run benchmarks to check performance impact
./tools/benchmarks/benchmark_system.sh

# 4. Compare results with previous benchmarks
```

### Performance Monitoring
```bash
# Regular performance monitoring
./tools/benchmarks/benchmark_system.sh --profile
```

### Code Quality Analysis
```bash
# Analyze code patterns and abstractions
./tools/analysis/analyze_abstraction.sh --detailed
```

## Adding New Tools

To add a new tool:

1. **Create the tool** in the appropriate subdirectory:
   - `analysis/` for code analysis tools
   - `benchmarks/` for performance tools
   - Create new subdirectories as needed

2. **Make it executable**:
   ```bash
   chmod +x tools/category/your_tool.sh
   ```

3. **Document it** in this README

4. **Follow naming conventions**:
   - Use snake_case for script names
   - Include file extensions (.sh, .rs, .py)
   - Use descriptive names

## Tool Categories

### Analysis Tools
- Static code analysis
- Dependency analysis
- Architecture verification
- Pattern detection

### Benchmarking Tools
- Performance measurement
- Load testing
- Throughput analysis
- Resource usage monitoring

### Maintenance Tools
- Automated cleanup
- Dependency updates
- Security scanning
- Report generation

## Integration with Build System

Tools can be integrated with the build system via Makefile targets:

```makefile
# Add to Makefile
analyze: ## Run code analysis tools
	@./tools/analysis/analyze_abstraction.sh

benchmark: ## Run performance benchmarks
	@./tools/benchmarks/benchmark_system.sh

tools-help: ## Show available tools
	@echo "Available tools:"
	@find tools/ -name "*.sh" -o -name "*.rs" | sort
```

## Best Practices

### Tool Development
- **Self-contained**: Tools should not depend on external services
- **Configurable**: Use command-line arguments for options
- **Documented**: Include usage information and examples
- **Robust**: Handle errors gracefully and provide meaningful output

### Output Format
- **Structured data**: Use JSON or CSV for machine-readable output
- **Human-readable**: Provide clear, formatted output for humans
- **Consistent**: Follow consistent formatting across tools

### Error Handling
- **Exit codes**: Use standard exit codes (0 for success, non-zero for errors)
- **Error messages**: Provide clear, actionable error messages
- **Logging**: Use appropriate log levels and structured logging

## Tool Dependencies

Some tools may require additional dependencies:

### System Dependencies
- **jq**: JSON processing (for analysis tools)
- **csvkit**: CSV processing (for benchmark tools)
- **gnuplot**: Graph generation (for visualization)

### Rust Dependencies
Tools written in Rust should:
- Be part of the workspace if they use MultiVM crates
- Have their own Cargo.toml if standalone
- Follow the same code style as the main project

### Installation
```bash
# Install system dependencies (Ubuntu/Debian)
sudo apt-get install jq csvkit gnuplot

# Install Rust tools
cargo install --path tools/analysis/consensus_verification/
```

## Continuous Integration

Tools can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run analysis tools
  run: |
    ./tools/analysis/analyze_abstraction.sh --ci
    
- name: Run benchmarks
  run: |
    ./tools/benchmarks/benchmark_system.sh --ci
    
- name: Archive results
  uses: actions/upload-artifact@v3
  with:
    name: tool-results
    path: tools/*/results/
```

## Contributing Tools

When contributing new tools:

1. **Discuss first**: Open an issue to discuss the tool's purpose
2. **Follow standards**: Use existing tools as examples
3. **Test thoroughly**: Ensure tools work in different environments
4. **Document completely**: Update this README and add inline documentation
5. **Consider maintenance**: Tools should be maintainable long-term