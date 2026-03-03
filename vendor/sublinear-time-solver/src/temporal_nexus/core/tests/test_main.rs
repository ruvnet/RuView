//! Test execution entry point and report generation
//!
//! This module provides the main test execution interface for the NanosecondScheduler
//! test suite. It orchestrates all test categories and generates comprehensive reports.

use std::time::Instant;
use std::collections::HashMap;
use crate::temporal_nexus::core::tests::{TestReport, TestConfig, TestUtils};
use crate::temporal_nexus::core::tests::test_runner::TestSuiteRunner;

/// Main test execution function
pub async fn run_all_tests() -> Result<TestReport, Box<dyn std::error::Error>> {
    println!("🚀 Starting NanosecondScheduler Comprehensive Test Suite");
    println!("===============================================");

    let start_time = Instant::now();
    let config = TestConfig::default();
    let mut runner = TestSuiteRunner::new(config);

    // Execute all test suites
    let report = runner.run_all_tests().await?;

    let total_duration = start_time.elapsed();

    // Generate and display report
    generate_test_report(&report, total_duration)?;

    Ok(report)
}

/// Generate a comprehensive test report
fn generate_test_report(report: &TestReport, total_duration: std::time::Duration) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 COMPREHENSIVE TEST REPORT");
    println!("============================");

    // Overall Summary
    println!("\n🎯 OVERALL SUMMARY");
    println!("Total Duration: {:.2}ms", total_duration.as_micros() as f64 / 1000.0);
    println!("Total Tests: {}", report.total_tests);
    println!("Passed: {} ✅", report.passed_tests);
    println!("Failed: {} ❌", report.failed_tests);
    println!("Success Rate: {:.1}%", (report.passed_tests as f64 / report.total_tests as f64) * 100.0);

    // Performance Metrics
    if let Some(perf) = &report.performance_metrics {
        println!("\n⚡ PERFORMANCE METRICS");
        println!("Average Tick Time: {:.2}μs", perf.avg_tick_time_ns as f64 / 1000.0);
        println!("Max Tick Time: {:.2}μs", perf.max_tick_time_ns as f64 / 1000.0);
        println!("Throughput: {:.0} ticks/sec", perf.throughput_tps);
        println!("Memory Usage: {:.2} MB", perf.memory_usage_bytes as f64 / 1024.0 / 1024.0);

        // Check performance targets
        let target_met = perf.avg_tick_time_ns < 1000; // <1μs target
        println!("Target (<1μs): {} {}",
            if target_met { "✅ MET" } else { "❌ FAILED" },
            if target_met { "" } else { "- Performance optimization needed" }
        );
    }

    // Test Category Results
    println!("\n📋 TEST CATEGORY RESULTS");
    for (category, result) in &report.test_results {
        let status = if result.success { "✅" } else { "❌" };
        println!("{} {}: {:.2}ms ({} assertions)",
            status,
            category,
            result.duration_ms,
            result.assertions_count
        );

        if !result.success && !result.failure_details.is_empty() {
            println!("   └─ {}", result.failure_details);
        }
    }

    // Critical Validations
    println!("\n🔍 CRITICAL VALIDATIONS");
    validate_timing_precision(report);
    validate_strange_loop_convergence(report);
    validate_quantum_constraints(report);
    validate_performance_targets(report);

    // Hardware Information
    println!("\n🖥️  HARDWARE INFORMATION");
    if let Ok(cpu_info) = get_cpu_info() {
        println!("CPU: {}", cpu_info);
    }
    if let Ok(tsc_freq) = TestUtils::get_tsc_frequency() {
        println!("TSC Frequency: {:.2} GHz", tsc_freq as f64 / 1_000_000_000.0);
    }

    // Recommendations
    generate_recommendations(report);

    // Save detailed report to file
    save_detailed_report(report, total_duration)?;

    println!("\n✨ Test execution completed successfully!");
    Ok(())
}

/// Validate timing precision requirements
fn validate_timing_precision(report: &TestReport) {
    if let Some(timing_result) = report.test_results.get("timing_precision") {
        if timing_result.success {
            println!("✅ Timing Precision: TSC-based nanosecond accuracy validated");
        } else {
            println!("❌ Timing Precision: Hardware timing validation failed");
        }
    }
}

/// Validate strange loop convergence
fn validate_strange_loop_convergence(report: &TestReport) {
    if let Some(loop_result) = report.test_results.get("strange_loop") {
        if loop_result.success {
            println!("✅ Strange Loop: Lipschitz < 1 constraint satisfied");
        } else {
            println!("❌ Strange Loop: Convergence requirements not met");
        }
    }
}

/// Validate quantum physics constraints
fn validate_quantum_constraints(report: &TestReport) {
    if let Some(quantum_result) = report.test_results.get("quantum_validation") {
        if quantum_result.success {
            println!("✅ Quantum Validation: Physics constraints satisfied");
        } else {
            println!("❌ Quantum Validation: Physics constraint violations detected");
        }
    }
}

/// Validate performance targets
fn validate_performance_targets(report: &TestReport) {
    if let Some(perf) = &report.performance_metrics {
        let target_met = perf.avg_tick_time_ns < 1000;
        if target_met {
            println!("✅ Performance: <1μs overhead target achieved");
        } else {
            println!("❌ Performance: Overhead exceeds 1μs target");
        }

        let throughput_met = perf.throughput_tps > 1_000_000.0;
        if throughput_met {
            println!("✅ Throughput: >1M ticks/sec target achieved");
        } else {
            println!("❌ Throughput: Below 1M ticks/sec target");
        }
    }
}

/// Generate recommendations based on test results
fn generate_recommendations(report: &TestReport) {
    println!("\n💡 RECOMMENDATIONS");

    let mut recommendations = Vec::new();

    // Performance recommendations
    if let Some(perf) = &report.performance_metrics {
        if perf.avg_tick_time_ns > 1000 {
            recommendations.push("Consider optimizing tick processing for <1μs target");
        }
        if perf.throughput_tps < 1_000_000.0 {
            recommendations.push("Investigate throughput bottlenecks");
        }
        if perf.memory_usage_bytes > 100 * 1024 * 1024 {
            recommendations.push("Review memory usage patterns");
        }
    }

    // Test failure recommendations
    for (category, result) in &report.test_results {
        if !result.success {
            match category.as_str() {
                "timing_precision" => recommendations.push("Verify TSC availability and permissions"),
                "strange_loop" => recommendations.push("Review Lipschitz constant calculations"),
                "quantum_validation" => recommendations.push("Check quantum physics constraint implementations"),
                "edge_cases" => recommendations.push("Strengthen error handling and boundary checks"),
                _ => recommendations.push(&format!("Review {} implementation", category)),
            }
        }
    }

    if recommendations.is_empty() {
        println!("🎉 All tests passed! System meets all requirements.");
    } else {
        for (i, rec) in recommendations.iter().enumerate() {
            println!("{}. {}", i + 1, rec);
        }
    }
}

/// Get CPU information
fn get_cpu_info() -> Result<String, Box<dyn std::error::Error>> {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{__cpuid, _rdtsc};

        // Get CPU brand string
        let cpuid = unsafe { __cpuid(0x80000002) };
        Ok(format!("x86_64 CPU with TSC support"))
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        Ok("Non-x86_64 architecture".to_string())
    }
}

/// Save detailed report to file
fn save_detailed_report(report: &TestReport, total_duration: std::time::Duration) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("/workspaces/sublinear-time-solver/src/temporal_nexus/core/tests/test_report_{}.json", timestamp);

    let detailed_report = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "total_duration_ms": total_duration.as_millis(),
        "summary": {
            "total_tests": report.total_tests,
            "passed_tests": report.passed_tests,
            "failed_tests": report.failed_tests,
            "success_rate": (report.passed_tests as f64 / report.total_tests as f64) * 100.0
        },
        "performance_metrics": report.performance_metrics,
        "test_results": report.test_results,
        "system_info": {
            "cpu_arch": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "tsc_available": cfg!(target_arch = "x86_64")
        }
    });

    let mut file = File::create(&filename)?;
    file.write_all(detailed_report.to_string().as_bytes())?;

    println!("\n📄 Detailed report saved to: {}", filename);
    Ok(())
}

/// Benchmark test runner for CI/CD integration
pub async fn run_benchmark_tests() -> Result<HashMap<String, f64>, Box<dyn std::error::Error>> {
    let config = TestConfig {
        enable_hardware_tests: true,
        stress_test_duration_ms: 1000,
        performance_iterations: 10000,
        tsc_frequency_hz: TestUtils::get_tsc_frequency()?,
    };

    let mut runner = TestSuiteRunner::new(config);
    let report = runner.run_performance_benchmarks().await?;

    let mut benchmarks = HashMap::new();

    if let Some(perf) = &report.performance_metrics {
        benchmarks.insert("avg_tick_time_us".to_string(), perf.avg_tick_time_ns as f64 / 1000.0);
        benchmarks.insert("max_tick_time_us".to_string(), perf.max_tick_time_ns as f64 / 1000.0);
        benchmarks.insert("throughput_tps".to_string(), perf.throughput_tps);
        benchmarks.insert("memory_usage_mb".to_string(), perf.memory_usage_bytes as f64 / 1024.0 / 1024.0);
    }

    Ok(benchmarks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_report_generation() {
        let result = run_all_tests().await;
        assert!(result.is_ok(), "Test suite should complete successfully");

        let report = result.unwrap();
        assert!(report.total_tests > 0, "Should have executed tests");
    }

    #[tokio::test]
    async fn test_benchmark_runner() {
        let result = run_benchmark_tests().await;
        assert!(result.is_ok(), "Benchmark tests should complete");

        let benchmarks = result.unwrap();
        assert!(benchmarks.contains_key("avg_tick_time_us"), "Should include timing benchmarks");
    }
}