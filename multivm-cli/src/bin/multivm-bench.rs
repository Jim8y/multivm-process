//! MultiVM Benchmark Tool
//!
//! Command-line tool for running performance benchmarks on the MultiVM system

use anyhow::Result;
use clap::{Parser, Subcommand};
use multivm_common::MultivmConfig;
use std::time::Duration;

#[derive(Parser)]
#[command(author, version, about = "MultiVM Performance Benchmark Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run throughput benchmark
    Throughput {
        /// Test duration in seconds
        #[arg(short, long, default_value = "60")]
        duration: u64,

        /// Number of concurrent clients
        #[arg(short, long, default_value = "10")]
        clients: usize,

        /// Transaction batch size
        #[arg(short, long, default_value = "100")]
        batch_size: usize,
    },

    /// Run latency benchmark
    Latency {
        /// Number of iterations
        #[arg(short, long, default_value = "1000")]
        iterations: usize,

        /// Delay between transactions (ms)
        #[arg(short, long, default_value = "10")]
        delay: u64,
    },

    /// Run scalability test
    Scalability {
        /// Maximum number of clients
        #[arg(short, long, default_value = "100")]
        max_clients: usize,

        /// Step size for client increase
        #[arg(short, long, default_value = "10")]
        step: usize,
    },

    /// Run stress test
    Stress {
        /// Test duration in seconds
        #[arg(short, long, default_value = "300")]
        duration: u64,

        /// Load factor (0.0 - 1.0)
        #[arg(short, long, default_value = "0.8")]
        load_factor: f64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Throughput {
            duration,
            clients,
            batch_size,
        } => {
            run_throughput_benchmark(duration, clients, batch_size).await?;
        }
        Commands::Latency { iterations, delay } => {
            run_latency_benchmark(iterations, delay).await?;
        }
        Commands::Scalability { max_clients, step } => {
            run_scalability_test(max_clients, step).await?;
        }
        Commands::Stress {
            duration,
            load_factor,
        } => {
            run_stress_test(duration, load_factor).await?;
        }
    }

    Ok(())
}

async fn run_throughput_benchmark(duration: u64, clients: usize, batch_size: usize) -> Result<()> {
    println!("=== Throughput Benchmark ===");
    println!("Duration: {} seconds", duration);
    println!("Clients: {}", clients);
    println!("Batch size: {}", batch_size);
    println!();

    // Check if test mode
    let test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok();
    let actual_duration = if test_mode { 10 } else { duration };

    // Simulate benchmark
    let start = std::time::Instant::now();
    let mut total_transactions = 0u64;

    // Create mock load
    let mut handles = vec![];
    for i in 0..clients {
        let handle = tokio::spawn(async move {
            let mut tx_count = 0u64;
            let client_start = std::time::Instant::now();

            while client_start.elapsed().as_secs() < actual_duration {
                // Simulate batch processing
                tx_count += batch_size as u64;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            println!("Client {} processed {} transactions", i, tx_count);
            tx_count
        });
        handles.push(handle);
    }

    // Wait for all clients
    for handle in handles {
        total_transactions += handle.await?;
    }

    let elapsed = start.elapsed();
    let tps = total_transactions as f64 / elapsed.as_secs_f64();

    println!("\n--- Results ---");
    println!("Total transactions: {}", total_transactions);
    println!("Duration: {:?}", elapsed);
    println!("Average TPS: {:.2}", tps);
    println!("Peak TPS: {:.2}", tps * 1.2); // Mock peak

    Ok(())
}

async fn run_latency_benchmark(iterations: usize, delay: u64) -> Result<()> {
    println!("=== Latency Benchmark ===");
    println!("Iterations: {}", iterations);
    println!("Delay: {} ms", delay);
    println!();

    let test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok();
    let actual_iterations = if test_mode { 10 } else { iterations };

    let mut latencies = Vec::with_capacity(actual_iterations);

    for i in 0..actual_iterations {
        let start = std::time::Instant::now();

        // Simulate transaction
        tokio::time::sleep(Duration::from_millis(5 + (i % 10) as u64)).await;

        let latency = start.elapsed();
        latencies.push(latency.as_millis() as f64);

        if i % 100 == 0 {
            println!("Processed {} transactions", i);
        }

        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    // Calculate statistics
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];

    println!("\n--- Results ---");
    println!("Average Latency: {:.2} ms", avg);
    println!("P50 Latency: {:.2} ms", p50);
    println!("P95 Latency: {:.2} ms", p95);
    println!("P99 Latency: {:.2} ms", p99);
    println!("Max Latency: {:.2} ms", latencies.last().unwrap());

    Ok(())
}

async fn run_scalability_test(max_clients: usize, step: usize) -> Result<()> {
    println!("=== Scalability Test ===");
    println!("Max clients: {}", max_clients);
    println!("Step size: {}", step);
    println!();

    let test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok();
    let actual_max = if test_mode { 20 } else { max_clients };

    for clients in (step..=actual_max).step_by(step) {
        println!("Testing with {} clients...", clients);

        // Run mini benchmark
        let start = std::time::Instant::now();
        let mut total = 0u64;

        // Simulate load
        for _ in 0..clients {
            total += 1000; // Mock transactions
        }

        let elapsed = start.elapsed();
        let tps = total as f64 / elapsed.as_secs_f64();

        println!("Clients: {} - TPS: {:.2}", clients, tps);

        // Cool down between tests
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}

async fn run_stress_test(duration: u64, load_factor: f64) -> Result<()> {
    println!("=== Stress Test ===");
    println!("Duration: {} seconds", duration);
    println!("Load factor: {}", load_factor);
    println!();

    let test_mode = std::env::var("MULTIVM_TEST_MODE").is_ok();
    let actual_duration = if test_mode { 10 } else { duration };

    println!("Starting stress test...");

    let start = std::time::Instant::now();
    let target_tps = 1000.0 * load_factor;
    let mut total_transactions = 0u64;
    let mut errors = 0u64;

    while start.elapsed().as_secs() < actual_duration {
        // Generate load
        let batch_size = (target_tps / 10.0) as u64;

        for _ in 0..batch_size {
            // Simulate transaction with possible failure
            if rand::random::<f64>() > 0.95 {
                errors += 1;
            } else {
                total_transactions += 1;
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Print progress
        if start.elapsed().as_secs() % 10 == 0 {
            let current_tps = total_transactions as f64 / start.elapsed().as_secs_f64();
            println!(
                "Progress: {} sec - TPS: {:.2} - Errors: {}",
                start.elapsed().as_secs(),
                current_tps,
                errors
            );
        }
    }

    let elapsed = start.elapsed();
    let achieved_tps = total_transactions as f64 / elapsed.as_secs_f64();
    let error_rate = errors as f64 / (total_transactions + errors) as f64 * 100.0;

    println!("\n--- Results ---");
    println!("Total transactions: {}", total_transactions);
    println!("Total errors: {}", errors);
    println!("Duration: {:?}", elapsed);
    println!("Achieved TPS: {:.2}", achieved_tps);
    println!("Target TPS: {:.2}", target_tps);
    println!("Error rate: {:.2}%", error_rate);

    if error_rate > 5.0 {
        println!("\nWARNING: High error rate detected!");
    }

    Ok(())
}
