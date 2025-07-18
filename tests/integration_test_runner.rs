//! Integration Test Runner
//! Comprehensive test runner for all MultiVM integration tests

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use tracing::{info, warn, error};

/// Test configuration for the integration test runner
#[derive(Debug, Clone)]
pub struct IntegrationTestRunnerConfig {
    pub test_timeout: Duration,
    pub setup_timeout: Duration,
    pub cleanup_timeout: Duration,
    pub parallel_tests: bool,
    pub verbose: bool,
    pub skip_long_running: bool,
}

impl Default for IntegrationTestRunnerConfig {
    fn default() -> Self {
        Self {
            test_timeout: Duration::from_secs(300), // 5 minutes per test
            setup_timeout: Duration::from_secs(60),
            cleanup_timeout: Duration::from_secs(30),
            parallel_tests: false, // Run tests sequentially by default
            verbose: true,
            skip_long_running: true, // Skip long-running tests by default
        }
    }
}

/// Integration test suite information
#[derive(Debug, Clone)]
pub struct TestSuite {
    pub name: String,
    pub description: String,
    pub test_functions: Vec<String>,
    pub requires_reth: bool,
    pub requires_solana: bool,
    pub long_running: bool,
}

/// Integration test runner
pub struct IntegrationTestRunner {
    config: IntegrationTestRunnerConfig,
    test_suites: Vec<TestSuite>,
}

impl IntegrationTestRunner {
    pub fn new(config: IntegrationTestRunnerConfig) -> Self {
        let test_suites = vec![
            TestSuite {
                name: "reth_engine_integration".to_string(),
                description: "Reth execution engine communication and processing".to_string(),
                test_functions: vec![
                    "test_reth_connectivity_only".to_string(),
                    "test_reth_rpc_communication".to_string(),
                    "test_reth_integration_full_workflow".to_string(),
                ],
                requires_reth: true,
                requires_solana: false,
                long_running: true,
            },
            TestSuite {
                name: "solana_engine_integration".to_string(),
                description: "Solana execution engine communication and processing".to_string(),
                test_functions: vec![
                    "test_solana_mempool_only".to_string(),
                    "test_solana_connectivity_only".to_string(),
                    "test_solana_rpc_communication".to_string(),
                    "test_solana_integration_full_workflow".to_string(),
                ],
                requires_reth: false,
                requires_solana: true,
                long_running: true,
            },
            TestSuite {
                name: "cross_vm_integration".to_string(),
                description: "Cross-VM transaction processing and coordination".to_string(),
                test_functions: vec![
                    "test_dual_engine_startup_only".to_string(),
                    "test_cross_vm_full_integration".to_string(),
                ],
                requires_reth: true,
                requires_solana: true,
                long_running: true,
            },
            TestSuite {
                name: "rpc_relay_integration".to_string(),
                description: "RPC request relaying and proxy functionality".to_string(),
                test_functions: vec![
                    "test_rpc_proxy_only".to_string(),
                    "test_rpc_relay_full_integration".to_string(),
                ],
                requires_reth: true,
                requires_solana: true,
                long_running: true,
            },
            TestSuite {
                name: "block_processing_integration".to_string(),
                description: "End-to-end block processing workflow".to_string(),
                test_functions: vec![
                    "test_basic_block_processing".to_string(),
                    "test_block_processing_full_workflow".to_string(),
                ],
                requires_reth: true,
                requires_solana: true,
                long_running: true,
            },
        ];

        Self {
            config,
            test_suites,
        }
    }

    /// Check if required binaries are available
    pub fn check_prerequisites(&self) -> Result<(), String> {
        info!("Checking prerequisites for integration tests");

        let mut missing_requirements = Vec::new();

        // Check for Reth binary
        if self.test_suites.iter().any(|suite| suite.requires_reth) {
            if !self.check_binary_available("reth") {
                missing_requirements.push("reth binary not found in PATH");
            }
        }

        // Check for Solana binary
        if self.test_suites.iter().any(|suite| suite.requires_solana) {
            if !self.check_binary_available("solana-test-validator") {
                missing_requirements.push("solana-test-validator binary not found in PATH");
            }
        }

        // Check for required directories
        let required_dirs = vec!["/tmp"];
        for dir in required_dirs {
            if !std::path::Path::new(dir).exists() {
                missing_requirements.push(&format!("Required directory {} does not exist", dir));
            }
        }

        if !missing_requirements.is_empty() {
            return Err(format!("Missing requirements: {}", missing_requirements.join(", ")));
        }

        info!("All prerequisites are available");
        Ok(())
    }

    /// Check if a binary is available in PATH
    fn check_binary_available(&self, binary_name: &str) -> bool {
        Command::new("which")
            .arg(binary_name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Run all integration tests
    pub async fn run_all_tests(&self) -> Result<TestResults, String> {
        info!("Starting integration test runner");

        // Check prerequisites
        self.check_prerequisites()?;

        let mut results = TestResults::new();

        // Filter test suites based on configuration
        let test_suites = self.filter_test_suites();

        if test_suites.is_empty() {
            warn!("No test suites to run based on current configuration");
            return Ok(results);
        }

        info!("Running {} test suites", test_suites.len());

        if self.config.parallel_tests {
            // Run tests in parallel (with caution for resource conflicts)
            results = self.run_tests_parallel(test_suites).await?;
        } else {
            // Run tests sequentially
            results = self.run_tests_sequential(test_suites).await?;
        }

        info!("Integration test runner completed");
        Ok(results)
    }

    /// Filter test suites based on configuration
    fn filter_test_suites(&self) -> Vec<TestSuite> {
        self.test_suites
            .iter()
            .filter(|suite| {
                if self.config.skip_long_running && suite.long_running {
                    info!("Skipping long-running test suite: {}", suite.name);
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Run tests sequentially
    async fn run_tests_sequential(&self, test_suites: Vec<TestSuite>) -> Result<TestResults, String> {
        let mut results = TestResults::new();

        for suite in test_suites {
            info!("Running test suite: {} - {}", suite.name, suite.description);

            let suite_results = self.run_test_suite(&suite).await?;
            results.merge(suite_results);

            // Add delay between test suites to allow cleanup
            if !self.config.parallel_tests {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        Ok(results)
    }

    /// Run tests in parallel
    async fn run_tests_parallel(&self, test_suites: Vec<TestSuite>) -> Result<TestResults, String> {
        let mut results = TestResults::new();

        // Group tests by resource requirements to avoid conflicts
        let mut reth_only_tests = Vec::new();
        let mut solana_only_tests = Vec::new();
        let mut cross_vm_tests = Vec::new();

        for suite in test_suites {
            if suite.requires_reth && suite.requires_solana {
                cross_vm_tests.push(suite);
            } else if suite.requires_reth {
                reth_only_tests.push(suite);
            } else if suite.requires_solana {
                solana_only_tests.push(suite);
            }
        }

        // Run each group sequentially, but tests within each group can run in parallel
        for group in vec![reth_only_tests, solana_only_tests, cross_vm_tests] {
            if group.is_empty() {
                continue;
            }

            let group_futures: Vec<_> = group
                .into_iter()
                .map(|suite| self.run_test_suite(&suite))
                .collect();

            let group_results = futures::future::join_all(group_futures).await;

            for result in group_results {
                match result {
                    Ok(suite_results) => results.merge(suite_results),
                    Err(e) => error!("Test suite failed: {}", e),
                }
            }
        }

        Ok(results)
    }

    /// Run a single test suite
    async fn run_test_suite(&self, suite: &TestSuite) -> Result<TestResults, String> {
        let mut results = TestResults::new();

        for test_function in &suite.test_functions {
            info!("Running test function: {}", test_function);

            let test_result = self.run_test_function(&suite.name, test_function).await;
            
            match test_result {
                Ok(duration) => {
                    info!("✅ Test {} passed in {:?}", test_function, duration);
                    results.add_success(test_function.clone(), duration);
                }
                Err(e) => {
                    error!("❌ Test {} failed: {}", test_function, e);
                    results.add_failure(test_function.clone(), e);
                }
            }
        }

        Ok(results)
    }

    /// Run a single test function
    async fn run_test_function(&self, suite_name: &str, test_function: &str) -> Result<Duration, String> {
        let start_time = std::time::Instant::now();

        // Build the test command
        let test_binary = format!("integration_{}_test", suite_name);
        let mut cmd = Command::new("cargo");
        cmd.args(&["test", "--test", &test_binary, test_function]);

        if self.config.verbose {
            cmd.arg("--");
            cmd.arg("--nocapture");
        }

        // Set environment variables
        cmd.env("RUST_LOG", "info");
        cmd.env("RUST_BACKTRACE", "1");

        // Execute the test with timeout
        let result = tokio::time::timeout(
            self.config.test_timeout,
            tokio::task::spawn_blocking(move || {
                cmd.output()
            })
        ).await;

        match result {
            Ok(Ok(output)) => {
                let duration = start_time.elapsed();
                
                if output.status.success() {
                    if self.config.verbose {
                        info!("Test output: {}", String::from_utf8_lossy(&output.stdout));
                    }
                    Ok(duration)
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    Err(format!("Test failed with exit code {}: {}", 
                               output.status.code().unwrap_or(-1), error_msg))
                }
            }
            Ok(Err(e)) => {
                Err(format!("Failed to execute test: {}", e))
            }
            Err(_) => {
                Err(format!("Test timed out after {:?}", self.config.test_timeout))
            }
        }
    }

    /// Generate test report
    pub fn generate_report(&self, results: &TestResults) -> String {
        let mut report = String::new();
        
        report.push_str("# MultiVM Integration Test Report\n\n");
        report.push_str(&format!("## Summary\n"));
        report.push_str(&format!("- Total tests: {}\n", results.total_tests()));
        report.push_str(&format!("- Passed: {}\n", results.passed_tests()));
        report.push_str(&format!("- Failed: {}\n", results.failed_tests()));
        report.push_str(&format!("- Success rate: {:.2}%\n\n", results.success_rate()));

        if !results.successes.is_empty() {
            report.push_str("## Passed Tests\n");
            for (test_name, duration) in &results.successes {
                report.push_str(&format!("- ✅ {} ({:?})\n", test_name, duration));
            }
            report.push_str("\n");
        }

        if !results.failures.is_empty() {
            report.push_str("## Failed Tests\n");
            for (test_name, error) in &results.failures {
                report.push_str(&format!("- ❌ {} - {}\n", test_name, error));
            }
            report.push_str("\n");
        }

        report.push_str("## Test Suites\n");
        for suite in &self.test_suites {
            report.push_str(&format!("### {}\n", suite.name));
            report.push_str(&format!("{}\n", suite.description));
            report.push_str(&format!("- Requires Reth: {}\n", suite.requires_reth));
            report.push_str(&format!("- Requires Solana: {}\n", suite.requires_solana));
            report.push_str(&format!("- Long running: {}\n", suite.long_running));
            report.push_str("\n");
        }

        report
    }
}

/// Test results aggregator
#[derive(Debug, Clone)]
pub struct TestResults {
    pub successes: Vec<(String, Duration)>,
    pub failures: Vec<(String, String)>,
}

impl TestResults {
    pub fn new() -> Self {
        Self {
            successes: Vec::new(),
            failures: Vec::new(),
        }
    }

    pub fn add_success(&mut self, test_name: String, duration: Duration) {
        self.successes.push((test_name, duration));
    }

    pub fn add_failure(&mut self, test_name: String, error: String) {
        self.failures.push((test_name, error));
    }

    pub fn merge(&mut self, other: TestResults) {
        self.successes.extend(other.successes);
        self.failures.extend(other.failures);
    }

    pub fn total_tests(&self) -> usize {
        self.successes.len() + self.failures.len()
    }

    pub fn passed_tests(&self) -> usize {
        self.successes.len()
    }

    pub fn failed_tests(&self) -> usize {
        self.failures.len()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tests() == 0 {
            0.0
        } else {
            (self.passed_tests() as f64 / self.total_tests() as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber;

    fn setup_logging() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .without_time()
            .try_init();
    }

    #[tokio::test]
    async fn test_integration_runner_basic() {
        setup_logging();
        
        let config = IntegrationTestRunnerConfig {
            skip_long_running: true,
            test_timeout: Duration::from_secs(10),
            ..Default::default()
        };

        let runner = IntegrationTestRunner::new(config);
        
        // Test prerequisite checking
        let prereq_result = runner.check_prerequisites();
        info!("Prerequisites check: {:?}", prereq_result);
        
        // Test filtering
        let filtered_suites = runner.filter_test_suites();
        info!("Filtered {} test suites", filtered_suites.len());
        
        // Test report generation
        let mut test_results = TestResults::new();
        test_results.add_success("test_example".to_string(), Duration::from_secs(1));
        test_results.add_failure("test_failure".to_string(), "Example failure".to_string());
        
        let report = runner.generate_report(&test_results);
        info!("Generated report:\n{}", report);
        
        assert!(report.contains("Total tests: 2"));
        assert!(report.contains("Passed: 1"));
        assert!(report.contains("Failed: 1"));
    }

    #[tokio::test]
    #[ignore = "Requires actual test execution"]
    async fn test_integration_runner_full() {
        setup_logging();
        
        let config = IntegrationTestRunnerConfig {
            skip_long_running: false,
            parallel_tests: false,
            verbose: true,
            test_timeout: Duration::from_secs(60),
            ..Default::default()
        };

        let runner = IntegrationTestRunner::new(config);
        
        match runner.run_all_tests().await {
            Ok(results) => {
                info!("Test run completed successfully");
                info!("Results: {} passed, {} failed", results.passed_tests(), results.failed_tests());
                
                let report = runner.generate_report(&results);
                info!("Final report:\n{}", report);
            }
            Err(e) => {
                error!("Test run failed: {}", e);
            }
        }
    }
}

/// Main function for running integration tests from command line
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .without_time()
        .init();

    let args: Vec<String> = std::env::args().collect();
    
    let config = IntegrationTestRunnerConfig {
        skip_long_running: !args.contains(&"--long-running".to_string()),
        parallel_tests: args.contains(&"--parallel".to_string()),
        verbose: args.contains(&"--verbose".to_string()),
        test_timeout: Duration::from_secs(
            args.iter()
                .position(|arg| arg == "--timeout")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(300)
        ),
        ..Default::default()
    };

    info!("Starting MultiVM integration test runner");
    info!("Configuration: {:?}", config);

    let runner = IntegrationTestRunner::new(config);
    
    match runner.run_all_tests().await {
        Ok(results) => {
            let report = runner.generate_report(&results);
            println!("{}", report);
            
            if results.failed_tests() > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Integration test runner failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}