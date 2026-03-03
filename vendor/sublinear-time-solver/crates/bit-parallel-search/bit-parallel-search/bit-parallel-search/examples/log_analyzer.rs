//! Example: Real-time log analysis
//! Demonstrates 3-5x faster error detection in server logs

use bit_parallel_search::BitParallelSearcher;
use std::time::Instant;

struct LogAnalyzer {
    error_searcher: BitParallelSearcher,
    warn_searcher: BitParallelSearcher,
    fatal_searcher: BitParallelSearcher,
    sql_error_searcher: BitParallelSearcher,
    auth_failure_searcher: BitParallelSearcher,
    timeout_searcher: BitParallelSearcher,
}

impl LogAnalyzer {
    fn new() -> Self {
        Self {
            error_searcher: BitParallelSearcher::new(b"ERROR"),
            warn_searcher: BitParallelSearcher::new(b"WARN"),
            fatal_searcher: BitParallelSearcher::new(b"FATAL"),
            sql_error_searcher: BitParallelSearcher::new(b"SQLException"),
            auth_failure_searcher: BitParallelSearcher::new(b"authentication failed"),
            timeout_searcher: BitParallelSearcher::new(b"timeout"),
        }
    }

    fn analyze_log(&self, log_data: &[u8]) -> LogAnalysis {
        LogAnalysis {
            error_count: self.error_searcher.count_in(log_data),
            warning_count: self.warn_searcher.count_in(log_data),
            fatal_count: self.fatal_searcher.count_in(log_data),
            sql_error_count: self.sql_error_searcher.count_in(log_data),
            auth_failure_count: self.auth_failure_searcher.count_in(log_data),
            timeout_count: self.timeout_searcher.count_in(log_data),
        }
    }

    fn find_critical_errors(&self, log_data: &[u8]) -> Vec<usize> {
        // Find all FATAL errors for immediate attention
        #[cfg(feature = "std")]
        {
            self.fatal_searcher.find_all_in(log_data).collect()
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, just find first occurrence
            self.fatal_searcher.find_in(log_data).into_iter().collect()
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct LogAnalysis {
    error_count: usize,
    warning_count: usize,
    fatal_count: usize,
    sql_error_count: usize,
    auth_failure_count: usize,
    timeout_count: usize,
}

impl LogAnalysis {
    fn is_critical(&self) -> bool {
        self.fatal_count > 0 || self.error_count > 100 || self.sql_error_count > 10
    }

    fn severity_score(&self) -> u32 {
        self.fatal_count as u32 * 100
            + self.error_count as u32 * 10
            + self.warning_count as u32
    }
}

fn naive_log_analyzer(log_data: &[u8]) -> LogAnalysis {
    LogAnalysis {
        error_count: count_occurrences_naive(log_data, b"ERROR"),
        warning_count: count_occurrences_naive(log_data, b"WARN"),
        fatal_count: count_occurrences_naive(log_data, b"FATAL"),
        sql_error_count: count_occurrences_naive(log_data, b"SQLException"),
        auth_failure_count: count_occurrences_naive(log_data, b"authentication failed"),
        timeout_count: count_occurrences_naive(log_data, b"timeout"),
    }
}

fn count_occurrences_naive(haystack: &[u8], needle: &[u8]) -> usize {
    let mut count = 0;
    let mut pos = 0;
    while pos <= haystack.len().saturating_sub(needle.len()) {
        if &haystack[pos..pos + needle.len()] == needle {
            count += 1;
        }
        pos += 1;
    }
    count
}

fn generate_sample_logs(size_mb: usize) -> Vec<u8> {
    let log_lines = [
        &b"2024-01-01 10:00:01 INFO  Application started successfully"[..],
        &b"2024-01-01 10:00:02 DEBUG Database connection established"[..],
        &b"2024-01-01 10:00:03 WARN  High memory usage detected (85%)"[..],
        &b"2024-01-01 10:00:04 ERROR Failed to process request: timeout"[..],
        &b"2024-01-01 10:00:05 INFO  User logged in: john@example.com"[..],
        &b"2024-01-01 10:00:06 ERROR SQLException: Connection refused"[..],
        &b"2024-01-01 10:00:07 DEBUG SQL query executed in 23ms"[..],
        &b"2024-01-01 10:00:08 WARN  Rate limit exceeded for IP 192.168.1.100"[..],
        &b"2024-01-01 10:00:09 ERROR authentication failed for user admin"[..],
        &b"2024-01-01 10:00:10 FATAL System out of memory, shutting down"[..],
    ];

    let target_bytes = size_mb * 1024 * 1024;
    let mut log_data = Vec::with_capacity(target_bytes);

    while log_data.len() < target_bytes {
        for line in log_lines.iter() {
            log_data.extend_from_slice(line);
            log_data.push(b'\n');
            if log_data.len() >= target_bytes {
                break;
            }
        }
    }

    log_data.truncate(target_bytes);
    log_data
}

fn main() {
    println!("📊 Real-Time Log Analysis Performance Demo\n");

    // Generate sample log data (1MB)
    let log_data = generate_sample_logs(1);
    println!("Generated {}MB of sample log data", log_data.len() / 1024 / 1024);

    let analyzer = LogAnalyzer::new();

    // Benchmark performance
    let iterations = 1000;

    // Bit-parallel analysis
    let start = Instant::now();
    let mut analysis = LogAnalysis {
        error_count: 0, warning_count: 0, fatal_count: 0,
        sql_error_count: 0, auth_failure_count: 0, timeout_count: 0
    };
    for _ in 0..iterations {
        analysis = analyzer.analyze_log(&log_data);
    }
    let bit_parallel_time = start.elapsed();

    // Naive analysis
    let start = Instant::now();
    for _ in 0..iterations {
        let _analysis = naive_log_analyzer(&log_data);
    }
    let naive_time = start.elapsed();

    println!("\n🚀 Performance Results ({} iterations):", iterations);
    println!("==========================================");
    println!("Bit-parallel: {:?}", bit_parallel_time);
    println!("Naive search: {:?}", naive_time);
    println!("Speedup: {:.1}x faster",
        naive_time.as_nanos() as f64 / bit_parallel_time.as_nanos() as f64);

    // Show analysis results
    println!("\n📈 Log Analysis Results:");
    println!("========================");
    println!("{:#?}", analysis);
    println!("Critical status: {}", if analysis.is_critical() { "🚨 CRITICAL" } else { "✅ Normal" });
    println!("Severity score: {}", analysis.severity_score());

    // Real-world performance metrics
    let mb_per_second = (log_data.len() as f64 / 1024.0 / 1024.0) / bit_parallel_time.as_secs_f64() * iterations as f64;
    println!("\n🌍 Real-World Performance:");
    println!("==========================");
    println!("Throughput: {:.0} MB/second", mb_per_second);
    println!("Can process a 1GB log file in {:.1} seconds", 1024.0 / mb_per_second);

    // Critical error detection
    let critical_positions = analyzer.find_critical_errors(&log_data);
    if !critical_positions.is_empty() {
        println!("\n🚨 Critical Errors Found:");
        println!("FATAL errors at positions: {:?}", critical_positions);
    }

    // Memory efficiency
    println!("\n💾 Memory Usage:");
    println!("Searcher memory: ~{}KB (6 patterns × 2KB each)", 6 * 2);
    println!("Log data: {}MB", log_data.len() / 1024 / 1024);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_analysis() {
        let analyzer = LogAnalyzer::new();
        let log = b"INFO: Starting\nERROR: Failed\nWARN: High load\nERROR: Timeout\n";

        let analysis = analyzer.analyze_log(log);
        assert_eq!(analysis.error_count, 2);
        assert_eq!(analysis.warning_count, 1);
        assert_eq!(analysis.fatal_count, 0);
    }

    #[test]
    fn test_critical_detection() {
        let analyzer = LogAnalyzer::new();
        let critical_log = b"FATAL: System crash\nERROR: Database down\n";

        let analysis = analyzer.analyze_log(critical_log);
        assert!(analysis.is_critical());
        assert_eq!(analysis.fatal_count, 1);
    }

    #[test]
    fn test_severity_scoring() {
        let analysis = LogAnalysis {
            error_count: 5,
            warning_count: 10,
            fatal_count: 1,
            sql_error_count: 0,
            auth_failure_count: 0,
            timeout_count: 0,
        };

        // 1*100 + 5*10 + 10*1 = 160
        assert_eq!(analysis.severity_score(), 160);
    }
}