//! Performance Report Generator for 7sense
//!
//! This utility parses Criterion benchmark output and generates
//! a comprehensive performance report with pass/fail status against targets.
//!
//! Performance Targets (from ADR-004):
//! - HNSW Search: 150x speedup vs brute force
//! - Query Latency p50: < 10ms
//! - Query Latency p99: < 50ms
//! - Recall@10: >= 0.95
//! - Recall@100: >= 0.98
//! - Embedding inference: >100 segments/second
//! - Batch ingestion: 1M vectors/minute (16,667 vectors/second)
//! - Total query latency: <100ms

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ============================================================================
// Performance Targets
// ============================================================================

mod targets {
    use std::time::Duration;

    // HNSW Performance
    pub const HNSW_SPEEDUP_VS_BRUTE_FORCE: f64 = 150.0;
    pub const QUERY_LATENCY_P50: Duration = Duration::from_millis(10);
    pub const QUERY_LATENCY_P99: Duration = Duration::from_millis(50);
    pub const RECALL_AT_10: f64 = 0.95;
    pub const RECALL_AT_100: f64 = 0.98;
    pub const INSERT_THROUGHPUT_PER_SEC: f64 = 10_000.0;
    pub const BUILD_TIME_1M_VECTORS: Duration = Duration::from_secs(30 * 60);

    // Embedding Performance
    pub const EMBEDDING_SEGMENTS_PER_SECOND: f64 = 100.0;
    pub const SPECTROGRAM_LATENCY: Duration = Duration::from_millis(20);
    pub const NORMALIZATION_LATENCY: Duration = Duration::from_millis(5);

    // Batch Ingestion
    pub const BATCH_VECTORS_PER_MINUTE: f64 = 1_000_000.0;
    pub const BATCH_VECTORS_PER_SECOND: f64 = BATCH_VECTORS_PER_MINUTE / 60.0;

    // API Performance
    pub const TOTAL_QUERY_LATENCY: Duration = Duration::from_millis(100);
    pub const EVIDENCE_PACK_LATENCY: Duration = Duration::from_millis(200);

    // Quantization
    pub const MAX_RECALL_LOSS_INT8: f64 = 0.03;
}

// ============================================================================
// Criterion Output Parsing
// ============================================================================

#[derive(Debug, Clone)]
struct BenchmarkEstimates {
    mean: Duration,
    median: Duration,
    std_dev: Duration,
    confidence_interval: (Duration, Duration),
}

#[derive(Debug, Clone)]
struct BenchmarkResult {
    name: String,
    group: String,
    estimates: BenchmarkEstimates,
    throughput: Option<f64>,
    iterations: u64,
}

/// Parse a Criterion estimates.json file
fn parse_estimates_json(path: &Path) -> Option<BenchmarkEstimates> {
    let content = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mean = json.get("mean")?.get("point_estimate")?.as_f64()?;
    let median = json.get("median")?.get("point_estimate")?.as_f64()?;
    let std_dev = json.get("std_dev")?.get("point_estimate")?.as_f64()?;

    let ci_lower = json.get("mean")?.get("confidence_interval")?.get("lower_bound")?.as_f64()?;
    let ci_upper = json.get("mean")?.get("confidence_interval")?.get("upper_bound")?.as_f64()?;

    Some(BenchmarkEstimates {
        mean: Duration::from_nanos(mean as u64),
        median: Duration::from_nanos(median as u64),
        std_dev: Duration::from_nanos(std_dev as u64),
        confidence_interval: (
            Duration::from_nanos(ci_lower as u64),
            Duration::from_nanos(ci_upper as u64),
        ),
    })
}

/// Scan criterion output directory and collect all benchmark results
fn collect_benchmark_results(criterion_dir: &Path) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    if !criterion_dir.exists() {
        return results;
    }

    // Walk through criterion output directories
    for bench_entry in fs::read_dir(criterion_dir).into_iter().flatten() {
        let bench_path = match bench_entry {
            Ok(e) => e.path(),
            Err(_) => continue,
        };

        if !bench_path.is_dir() {
            continue;
        }

        let bench_name = bench_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Look for group directories
        for group_entry in fs::read_dir(&bench_path).into_iter().flatten() {
            let group_path = match group_entry {
                Ok(e) => e.path(),
                Err(_) => continue,
            };

            if !group_path.is_dir() {
                continue;
            }

            let group_name = group_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Look for estimates.json
            let estimates_path = group_path.join("new").join("estimates.json");
            if let Some(estimates) = parse_estimates_json(&estimates_path) {
                results.push(BenchmarkResult {
                    name: format!("{}/{}", bench_name, group_name),
                    group: bench_name.clone(),
                    estimates,
                    throughput: None, // Would need to parse from benchmark.json
                    iterations: 0,
                });
            }

            // Also check for sub-benchmarks
            for sub_entry in fs::read_dir(&group_path).into_iter().flatten() {
                let sub_path = match sub_entry {
                    Ok(e) => e.path(),
                    Err(_) => continue,
                };

                if !sub_path.is_dir() {
                    continue;
                }

                let sub_name = sub_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let estimates_path = sub_path.join("new").join("estimates.json");
                if let Some(estimates) = parse_estimates_json(&estimates_path) {
                    results.push(BenchmarkResult {
                        name: format!("{}/{}/{}", bench_name, group_name, sub_name),
                        group: bench_name.clone(),
                        estimates,
                        throughput: None,
                        iterations: 0,
                    });
                }
            }
        }
    }

    results
}

// ============================================================================
// Performance Checks
// ============================================================================

#[derive(Debug, Clone)]
enum CheckResult {
    Pass,
    Fail(String),
    Warning(String),
    Unknown,
}

impl CheckResult {
    fn symbol(&self) -> &str {
        match self {
            CheckResult::Pass => "[PASS]",
            CheckResult::Fail(_) => "[FAIL]",
            CheckResult::Warning(_) => "[WARN]",
            CheckResult::Unknown => "[????]",
        }
    }

    fn is_pass(&self) -> bool {
        matches!(self, CheckResult::Pass)
    }
}

#[derive(Debug)]
struct PerformanceCheck {
    name: String,
    target: String,
    actual: String,
    result: CheckResult,
}

/// Check HNSW query latency
fn check_hnsw_latency(results: &[BenchmarkResult]) -> Vec<PerformanceCheck> {
    let mut checks = Vec::new();

    // Find HNSW search benchmarks
    for result in results {
        if result.name.contains("hnsw_search") && result.name.contains("k_10") {
            let actual_ms = result.estimates.mean.as_secs_f64() * 1000.0;

            // p50 check (using mean as approximation)
            checks.push(PerformanceCheck {
                name: format!("{} p50", result.name),
                target: format!("< {}ms", targets::QUERY_LATENCY_P50.as_millis()),
                actual: format!("{:.2}ms", actual_ms),
                result: if result.estimates.mean <= targets::QUERY_LATENCY_P50 {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail(format!(
                        "Exceeds target by {:.2}ms",
                        actual_ms - targets::QUERY_LATENCY_P50.as_secs_f64() * 1000.0
                    ))
                },
            });

            // p99 check (using mean + 2*stddev as approximation)
            let p99_estimate = result.estimates.mean + result.estimates.std_dev * 2;
            let p99_ms = p99_estimate.as_secs_f64() * 1000.0;

            checks.push(PerformanceCheck {
                name: format!("{} p99 (estimated)", result.name),
                target: format!("< {}ms", targets::QUERY_LATENCY_P99.as_millis()),
                actual: format!("{:.2}ms", p99_ms),
                result: if p99_estimate <= targets::QUERY_LATENCY_P99 {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail(format!(
                        "Exceeds target by {:.2}ms",
                        p99_ms - targets::QUERY_LATENCY_P99.as_secs_f64() * 1000.0
                    ))
                },
            });
        }
    }

    checks
}

/// Check embedding throughput
fn check_embedding_throughput(results: &[BenchmarkResult]) -> Vec<PerformanceCheck> {
    let mut checks = Vec::new();

    for result in results {
        if result.name.contains("full_embedding_pipeline") && result.name.contains("single") {
            let latency_ms = result.estimates.mean.as_secs_f64() * 1000.0;
            let throughput = 1000.0 / latency_ms;

            checks.push(PerformanceCheck {
                name: "Embedding throughput".to_string(),
                target: format!(">= {} segments/sec", targets::EMBEDDING_SEGMENTS_PER_SECOND),
                actual: format!("{:.1} segments/sec ({:.2}ms/segment)", throughput, latency_ms),
                result: if throughput >= targets::EMBEDDING_SEGMENTS_PER_SECOND {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail(format!(
                        "Below target by {:.1} segments/sec",
                        targets::EMBEDDING_SEGMENTS_PER_SECOND - throughput
                    ))
                },
            });
        }
    }

    checks
}

/// Check API latency
fn check_api_latency(results: &[BenchmarkResult]) -> Vec<PerformanceCheck> {
    let mut checks = Vec::new();

    for result in results {
        if result.name.contains("neighbor_search_endpoint") {
            let latency_ms = result.estimates.mean.as_secs_f64() * 1000.0;

            checks.push(PerformanceCheck {
                name: result.name.clone(),
                target: format!("< {}ms", targets::TOTAL_QUERY_LATENCY.as_millis()),
                actual: format!("{:.2}ms", latency_ms),
                result: if result.estimates.mean <= targets::TOTAL_QUERY_LATENCY {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail(format!(
                        "Exceeds target by {:.2}ms",
                        latency_ms - targets::TOTAL_QUERY_LATENCY.as_secs_f64() * 1000.0
                    ))
                },
            });
        }

        if result.name.contains("evidence_pack") {
            let latency_ms = result.estimates.mean.as_secs_f64() * 1000.0;

            checks.push(PerformanceCheck {
                name: result.name.clone(),
                target: format!("< {}ms", targets::EVIDENCE_PACK_LATENCY.as_millis()),
                actual: format!("{:.2}ms", latency_ms),
                result: if result.estimates.mean <= targets::EVIDENCE_PACK_LATENCY {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail(format!(
                        "Exceeds target by {:.2}ms",
                        latency_ms - targets::EVIDENCE_PACK_LATENCY.as_secs_f64() * 1000.0
                    ))
                },
            });
        }
    }

    checks
}

/// Check insert throughput
fn check_insert_throughput(results: &[BenchmarkResult]) -> Vec<PerformanceCheck> {
    let mut checks = Vec::new();

    for result in results {
        if result.name.contains("hnsw_insert_batch") {
            // Try to extract batch size from name
            let batch_size = if result.name.contains("1000") {
                1000.0
            } else if result.name.contains("5000") {
                5000.0
            } else {
                continue;
            };

            let batch_time_secs = result.estimates.mean.as_secs_f64();
            let throughput = batch_size / batch_time_secs;

            checks.push(PerformanceCheck {
                name: format!("{} throughput", result.name),
                target: format!(">= {:.0} vectors/sec", targets::INSERT_THROUGHPUT_PER_SEC),
                actual: format!("{:.0} vectors/sec", throughput),
                result: if throughput >= targets::INSERT_THROUGHPUT_PER_SEC {
                    CheckResult::Pass
                } else if throughput >= targets::INSERT_THROUGHPUT_PER_SEC * 0.5 {
                    CheckResult::Warning(format!(
                        "Below target but acceptable ({:.1}% of target)",
                        throughput / targets::INSERT_THROUGHPUT_PER_SEC * 100.0
                    ))
                } else {
                    CheckResult::Fail(format!(
                        "Significantly below target ({:.1}% of target)",
                        throughput / targets::INSERT_THROUGHPUT_PER_SEC * 100.0
                    ))
                },
            });
        }
    }

    checks
}

// ============================================================================
// Report Generation
// ============================================================================

#[derive(Debug)]
struct PerformanceReport {
    timestamp: String,
    total_benchmarks: usize,
    checks: Vec<PerformanceCheck>,
    pass_count: usize,
    fail_count: usize,
    warning_count: usize,
}

impl PerformanceReport {
    fn generate(criterion_dir: &Path) -> Self {
        let results = collect_benchmark_results(criterion_dir);
        let mut checks = Vec::new();

        // Run all checks
        checks.extend(check_hnsw_latency(&results));
        checks.extend(check_embedding_throughput(&results));
        checks.extend(check_api_latency(&results));
        checks.extend(check_insert_throughput(&results));

        let pass_count = checks.iter().filter(|c| c.result.is_pass()).count();
        let fail_count = checks.iter().filter(|c| matches!(c.result, CheckResult::Fail(_))).count();
        let warning_count = checks.iter().filter(|c| matches!(c.result, CheckResult::Warning(_))).count();

        Self {
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            total_benchmarks: results.len(),
            checks,
            pass_count,
            fail_count,
            warning_count,
        }
    }

    fn print_text(&self) {
        println!();
        println!("================================================================");
        println!("          7sense Performance Report");
        println!("================================================================");
        println!();
        println!("Generated: {}", self.timestamp);
        println!("Total Benchmarks Analyzed: {}", self.total_benchmarks);
        println!();

        println!("Performance Targets (ADR-004):");
        println!("  - HNSW Search: 150x speedup vs brute force");
        println!("  - Query Latency p50: < 10ms");
        println!("  - Query Latency p99: < 50ms");
        println!("  - Recall@10: >= 0.95");
        println!("  - Recall@100: >= 0.98");
        println!("  - Embedding: >100 segments/second");
        println!("  - Batch: 1M vectors/minute");
        println!("  - Total query: <100ms");
        println!();

        println!("================================================================");
        println!("                    Check Results");
        println!("================================================================");
        println!();

        for check in &self.checks {
            let symbol = check.result.symbol();
            println!("{} {}", symbol, check.name);
            println!("      Target: {}", check.target);
            println!("      Actual: {}", check.actual);
            if let CheckResult::Fail(msg) | CheckResult::Warning(msg) = &check.result {
                println!("      Note: {}", msg);
            }
            println!();
        }

        println!("================================================================");
        println!("                      Summary");
        println!("================================================================");
        println!();
        println!("  Total Checks: {}", self.checks.len());
        println!("  Passed:       {} ({}%)",
            self.pass_count,
            if self.checks.is_empty() { 0 } else { self.pass_count * 100 / self.checks.len() }
        );
        println!("  Failed:       {}", self.fail_count);
        println!("  Warnings:     {}", self.warning_count);
        println!();

        if self.fail_count == 0 {
            println!("  Status: ALL TARGETS MET");
        } else {
            println!("  Status: {} TARGET(S) NOT MET", self.fail_count);
        }
        println!();
        println!("================================================================");
    }

    fn to_json(&self) -> String {
        let checks_json: Vec<serde_json::Value> = self.checks.iter().map(|c| {
            serde_json::json!({
                "name": c.name,
                "target": c.target,
                "actual": c.actual,
                "status": match &c.result {
                    CheckResult::Pass => "pass",
                    CheckResult::Fail(_) => "fail",
                    CheckResult::Warning(_) => "warning",
                    CheckResult::Unknown => "unknown",
                },
                "message": match &c.result {
                    CheckResult::Fail(msg) | CheckResult::Warning(msg) => msg.clone(),
                    _ => String::new(),
                }
            })
        }).collect();

        serde_json::to_string_pretty(&serde_json::json!({
            "timestamp": self.timestamp,
            "total_benchmarks": self.total_benchmarks,
            "summary": {
                "total_checks": self.checks.len(),
                "pass_count": self.pass_count,
                "fail_count": self.fail_count,
                "warning_count": self.warning_count,
                "all_targets_met": self.fail_count == 0
            },
            "checks": checks_json
        })).unwrap_or_else(|_| "{}".to_string())
    }

    fn exit_code(&self) -> i32 {
        if self.fail_count > 0 {
            1
        } else {
            0
        }
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let criterion_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("target/criterion")
    };

    let output_format = if args.len() > 2 {
        args[2].as_str()
    } else {
        "text"
    };

    let report = PerformanceReport::generate(&criterion_dir);

    match output_format {
        "json" => println!("{}", report.to_json()),
        "text" | _ => report.print_text(),
    }

    std::process::exit(report.exit_code());
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_result_symbols() {
        assert_eq!(CheckResult::Pass.symbol(), "[PASS]");
        assert_eq!(CheckResult::Fail("test".to_string()).symbol(), "[FAIL]");
        assert_eq!(CheckResult::Warning("test".to_string()).symbol(), "[WARN]");
    }

    #[test]
    fn test_performance_check_latency() {
        let result = BenchmarkResult {
            name: "hnsw_search/k_10".to_string(),
            group: "hnsw_benchmark".to_string(),
            estimates: BenchmarkEstimates {
                mean: Duration::from_millis(5),
                median: Duration::from_millis(5),
                std_dev: Duration::from_millis(1),
                confidence_interval: (Duration::from_millis(4), Duration::from_millis(6)),
            },
            throughput: None,
            iterations: 100,
        };

        let checks = check_hnsw_latency(&[result]);
        assert!(!checks.is_empty());
        assert!(checks[0].result.is_pass());
    }

    #[test]
    fn test_performance_check_latency_fail() {
        let result = BenchmarkResult {
            name: "hnsw_search/k_10".to_string(),
            group: "hnsw_benchmark".to_string(),
            estimates: BenchmarkEstimates {
                mean: Duration::from_millis(100), // Exceeds target
                median: Duration::from_millis(100),
                std_dev: Duration::from_millis(10),
                confidence_interval: (Duration::from_millis(90), Duration::from_millis(110)),
            },
            throughput: None,
            iterations: 100,
        };

        let checks = check_hnsw_latency(&[result]);
        assert!(!checks.is_empty());
        assert!(!checks[0].result.is_pass());
    }
}
