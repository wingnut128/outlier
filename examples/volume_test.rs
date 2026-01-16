//! Volume test script for outlier API
//!
//! Tests the percentile calculation with 1 million values at both
//! 95th and 90th percentile thresholds.
//!
//! Run with:
//!   cargo run --example volume_test                    # Library tests only (1M values)
//!   cargo run --example volume_test -- --count 100000  # Custom value count
//!   cargo run --example volume_test -- --with-api      # Include API tests (start server first)
//!   cargo run --example volume_test -- --api-url http://localhost:8080  # Custom API URL
//!
//! To start the server:
//!   cargo run --features server -- --serve

use outlier::{CalculateRequest, CalculateResponse, calculate_percentile};
use std::time::Instant;

const DEFAULT_NUM_VALUES: usize = 1_000_000;
const DEFAULT_API_URL: &str = "http://localhost:3000";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let with_api = args.iter().any(|a| a == "--with-api");
    let api_url = args
        .iter()
        .position(|a| a == "--api-url")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_API_URL);
    let num_values = args
        .iter()
        .position(|a| a == "--count")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_NUM_VALUES);

    println!("=================================================");
    println!("  Outlier Volume Test - {} Values", num_values);
    println!("=================================================");
    println!();

    // Generate random values using a simple LCG
    println!("Generating {} values...", num_values);
    let gen_start = Instant::now();
    let values = generate_values(num_values);
    let gen_duration = gen_start.elapsed();
    println!("Generated {} values in {:?}", values.len(), gen_duration);
    println!();

    // Calculate statistics about the generated data
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let sum: f64 = values.iter().sum();
    let mean = sum / values.len() as f64;

    println!("Dataset Statistics:");
    println!("  Count: {}", values.len());
    println!("  Min:   {:.4}", min);
    println!("  Max:   {:.4}", max);
    println!("  Mean:  {:.4}", mean);
    println!();

    // ===========================================
    // Direct Library Tests
    // ===========================================
    println!("=================================================");
    println!("  Direct Library Tests");
    println!("=================================================");
    println!();

    // Test 95th percentile
    println!("-------------------------------------------------");
    println!("Testing 95th Percentile (Library)");
    println!("-------------------------------------------------");
    let p95_result = run_percentile_test(&values, 95.0);

    // Test 90th percentile
    println!("-------------------------------------------------");
    println!("Testing 90th Percentile (Library)");
    println!("-------------------------------------------------");
    let p90_result = run_percentile_test(&values, 90.0);

    // Additional percentile tests for comparison
    println!("-------------------------------------------------");
    println!("Additional Percentile Tests (Library)");
    println!("-------------------------------------------------");
    run_percentile_test(&values, 99.0);
    run_percentile_test(&values, 75.0);
    run_percentile_test(&values, 50.0);

    // ===========================================
    // API Endpoint Tests
    // ===========================================
    if with_api {
        println!();
        println!("=================================================");
        println!("  API Endpoint Tests");
        println!("=================================================");
        println!("  Target: {}", api_url);
        println!();

        // Check if server is running
        let health_url = format!("{}/health", api_url);
        println!("Checking server health at {}...", health_url);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let server_available = rt.block_on(async { check_server_health(&health_url).await });

        if server_available {
            println!("Server is healthy!\n");

            // Test 95th percentile via API
            println!("-------------------------------------------------");
            println!("Testing 95th Percentile (API)");
            println!("-------------------------------------------------");
            let api_p95 =
                rt.block_on(async { run_api_percentile_test(api_url, &values, 95.0).await });

            // Verify API result matches library result
            if let (Some(lib_result), Some(api_result)) = (p95_result, api_p95) {
                verify_results("P95", lib_result, api_result);
            }

            // Test 90th percentile via API
            println!("-------------------------------------------------");
            println!("Testing 90th Percentile (API)");
            println!("-------------------------------------------------");
            let api_p90 =
                rt.block_on(async { run_api_percentile_test(api_url, &values, 90.0).await });

            // Verify API result matches library result
            if let (Some(lib_result), Some(api_result)) = (p90_result, api_p90) {
                verify_results("P90", lib_result, api_result);
            }

            // Additional API tests
            println!("-------------------------------------------------");
            println!("Additional Percentile Tests (API)");
            println!("-------------------------------------------------");
            rt.block_on(async {
                run_api_percentile_test(api_url, &values, 99.0).await;
                run_api_percentile_test(api_url, &values, 75.0).await;
                run_api_percentile_test(api_url, &values, 50.0).await;
            });
        } else {
            println!("Server is not available!");
            println!("Start the server with: cargo run --features server -- --serve");
            println!("Then run this test with: cargo run --example volume_test -- --with-api");
        }
    } else {
        println!();
        println!("-------------------------------------------------");
        println!("API tests skipped. Run with --with-api flag to include.");
        println!("Start server first: cargo run --features server -- --serve");
        println!("-------------------------------------------------");
    }

    println!();
    println!("=================================================");
    println!("  Volume Test Complete");
    println!("=================================================");
}

/// Generate a vector of pseudo-random values using a Linear Congruential Generator
/// Values are in the range [0, 10000)
fn generate_values(count: usize) -> Vec<f64> {
    let mut values = Vec::with_capacity(count);

    // LCG parameters (same as glibc)
    let a: u64 = 1103515245;
    let c: u64 = 12345;
    let m: u64 = 2147483648; // 2^31

    let mut seed: u64 = 42; // Fixed seed for reproducibility

    for _ in 0..count {
        seed = (a.wrapping_mul(seed).wrapping_add(c)) % m;
        let value = (seed as f64 / m as f64) * 10000.0;
        values.push(value);
    }

    values
}

/// Run a percentile test using the library directly and print results
fn run_percentile_test(values: &[f64], percentile: f64) -> Option<f64> {
    let start = Instant::now();

    match calculate_percentile(values, percentile) {
        Ok(result) => {
            let duration = start.elapsed();
            println!("  P{}: {:.4}", percentile, result);
            println!("  Calculation time: {:?}", duration);
            println!(
                "  Throughput: {:.2} values/sec",
                values.len() as f64 / duration.as_secs_f64()
            );
            println!();
            Some(result)
        }
        Err(e) => {
            println!("  Error calculating P{}: {}", percentile, e);
            println!();
            None
        }
    }
}

/// Check if the server is available
async fn check_server_health(url: &str) -> bool {
    let client = reqwest::Client::new();
    match client
        .get(url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// Run a percentile test via the API endpoint
async fn run_api_percentile_test(base_url: &str, values: &[f64], percentile: f64) -> Option<f64> {
    let client = reqwest::Client::new();
    let url = format!("{}/calculate", base_url);

    let request = CalculateRequest {
        values: values.to_vec(),
        percentile,
    };

    let start = Instant::now();

    match client
        .post(&url)
        .json(&request)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<CalculateResponse>().await {
                    Ok(resp) => {
                        let duration = start.elapsed();
                        println!("  P{}: {:.4}", percentile, resp.result);
                        println!("  Calculation time: {:?}", duration);
                        println!(
                            "  Throughput: {:.2} values/sec",
                            values.len() as f64 / duration.as_secs_f64()
                        );
                        println!("  Response count: {}", resp.count);
                        println!();
                        Some(resp.result)
                    }
                    Err(e) => {
                        println!("  Error parsing response for P{}: {}", percentile, e);
                        println!();
                        None
                    }
                }
            } else {
                println!(
                    "  API error for P{}: HTTP {}",
                    percentile,
                    response.status()
                );
                if let Ok(text) = response.text().await {
                    println!("  Response: {}", text);
                }
                println!();
                None
            }
        }
        Err(e) => {
            println!("  Request error for P{}: {}", percentile, e);
            println!();
            None
        }
    }
}

/// Verify that library and API results match
fn verify_results(label: &str, lib_result: f64, api_result: f64) {
    let diff = (lib_result - api_result).abs();
    if diff < 0.0001 {
        println!("  ✓ {} results match (diff: {:.6})", label, diff);
    } else {
        println!(
            "  ✗ {} results differ! Library: {:.4}, API: {:.4}, diff: {:.6}",
            label, lib_result, api_result, diff
        );
    }
    println!();
}
