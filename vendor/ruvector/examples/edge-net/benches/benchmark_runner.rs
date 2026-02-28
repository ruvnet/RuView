//! Benchmark Runner and Statistical Analysis
//!
//! Provides comprehensive benchmark execution and statistical analysis
//! for edge-net performance metrics.

use std::time::{Duration, Instant};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub total_time_ns: u128,
    pub mean_ns: f64,
    pub median_ns: f64,
    pub std_dev_ns: f64,
    pub min_ns: u128,
    pub max_ns: u128,
    pub samples: Vec<u128>,
}

impl BenchmarkResult {
    pub fn new(name: String, samples: Vec<u128>) -> Self {
        let iterations = samples.len();
        let total_time_ns: u128 = samples.iter().sum();
        let mean_ns = total_time_ns as f64 / iterations as f64;

        let mut sorted_samples = samples.clone();
        sorted_samples.sort_unstable();
        let median_ns = sorted_samples[iterations / 2] as f64;

        let variance = samples.iter()
            .map(|&x| {
                let diff = x as f64 - mean_ns;
                diff * diff
            })
            .sum::<f64>() / iterations as f64;
        let std_dev_ns = variance.sqrt();

        let min_ns = *sorted_samples.first().unwrap();
        let max_ns = *sorted_samples.last().unwrap();

        Self {
            name,
            iterations,
            total_time_ns,
            mean_ns,
            median_ns,
            std_dev_ns,
            min_ns,
            max_ns,
            samples: sorted_samples,
        }
    }

    pub fn throughput_per_sec(&self) -> f64 {
        1_000_000_000.0 / self.mean_ns
    }

    pub fn percentile(&self, p: f64) -> u128 {
        let index = ((p / 100.0) * self.iterations as f64) as usize;
        self.samples[index.min(self.iterations - 1)]
    }
}

#[derive(Debug)]
pub struct BenchmarkSuite {
    pub results: HashMap<String, BenchmarkResult>,
}

impl BenchmarkSuite {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.results.insert(result.name.clone(), result);
    }

    pub fn run_benchmark<F>(&mut self, name: &str, iterations: usize, mut f: F)
    where
        F: FnMut(),
    {
        let mut samples = Vec::with_capacity(iterations);

        // Warmup
        for _ in 0..10 {
            f();
        }

        // Actual benchmarking
        for _ in 0..iterations {
            let start = Instant::now();
            f();
            let elapsed = start.elapsed().as_nanos();
            samples.push(elapsed);
        }

        let result = BenchmarkResult::new(name.to_string(), samples);
        self.add_result(result);
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        report.push_str("# Edge-Net Comprehensive Benchmark Report\n\n");
        report.push_str("## Summary Statistics\n\n");

        let mut results: Vec<_> = self.results.values().collect();
        results.sort_by(|a, b| a.name.cmp(&b.name));

        for result in &results {
            report.push_str(&format!("\n### {}\n", result.name));
            report.push_str(&format!("- Iterations: {}\n", result.iterations));
            report.push_str(&format!("- Mean: {:.2} ns ({:.2} µs)\n",
                result.mean_ns, result.mean_ns / 1000.0));
            report.push_str(&format!("- Median: {:.2} ns ({:.2} µs)\n",
                result.median_ns, result.median_ns / 1000.0));
            report.push_str(&format!("- Std Dev: {:.2} ns\n", result.std_dev_ns));
            report.push_str(&format!("- Min: {} ns\n", result.min_ns));
            report.push_str(&format!("- Max: {} ns\n", result.max_ns));
            report.push_str(&format!("- P95: {} ns\n", result.percentile(95.0)));
            report.push_str(&format!("- P99: {} ns\n", result.percentile(99.0)));
            report.push_str(&format!("- Throughput: {:.2} ops/sec\n", result.throughput_per_sec()));
        }

        report.push_str("\n## Comparative Analysis\n\n");

        // Spike-driven vs Standard Attention Energy Analysis
        if let Some(spike_result) = self.results.get("spike_attention_seq64_dim128") {
            let theoretical_energy_ratio = 87.0;
            let measured_speedup = 1.0; // Placeholder - would compare with standard attention
            report.push_str("### Spike-Driven Attention Energy Efficiency\n");
            report.push_str(&format!("- Theoretical Energy Ratio: {}x\n", theoretical_energy_ratio));
            report.push_str(&format!("- Measured Performance: {:.2} ops/sec\n",
                spike_result.throughput_per_sec()));
            report.push_str(&format!("- Mean Latency: {:.2} µs\n",
                spike_result.mean_ns / 1000.0));
        }

        // RAC Coherence Performance
        if let Some(rac_result) = self.results.get("rac_event_ingestion") {
            report.push_str("\n### RAC Coherence Engine Performance\n");
            report.push_str(&format!("- Event Ingestion Rate: {:.2} events/sec\n",
                rac_result.throughput_per_sec()));
            report.push_str(&format!("- Mean Latency: {:.2} µs\n",
                rac_result.mean_ns / 1000.0));
        }

        // Learning Module Performance
        if let Some(bank_1k) = self.results.get("reasoning_bank_lookup_1k") {
            if let Some(bank_10k) = self.results.get("reasoning_bank_lookup_10k") {
                let scaling_factor = bank_10k.mean_ns / bank_1k.mean_ns;
                report.push_str("\n### ReasoningBank Scaling Analysis\n");
                report.push_str(&format!("- 1K patterns: {:.2} µs\n", bank_1k.mean_ns / 1000.0));
                report.push_str(&format!("- 10K patterns: {:.2} µs\n", bank_10k.mean_ns / 1000.0));
                report.push_str(&format!("- Scaling factor: {:.2}x (ideal: 10x for linear)\n",
                    scaling_factor));
                report.push_str(&format!("- Lookup efficiency: {:.1}% of linear\n",
                    (10.0 / scaling_factor) * 100.0));
            }
        }

        report.push_str("\n## Performance Targets\n\n");
        report.push_str("| Component | Target | Actual | Status |\n");
        report.push_str("|-----------|--------|--------|--------|\n");

        // Check against targets
        if let Some(result) = self.results.get("spike_attention_seq64_dim128") {
            let target_us = 100.0;
            let actual_us = result.mean_ns / 1000.0;
            let status = if actual_us < target_us { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("| Spike Attention (64x128) | <{} µs | {:.2} µs | {} |\n",
                target_us, actual_us, status));
        }

        if let Some(result) = self.results.get("rac_event_ingestion") {
            let target_us = 50.0;
            let actual_us = result.mean_ns / 1000.0;
            let status = if actual_us < target_us { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("| RAC Event Ingestion | <{} µs | {:.2} µs | {} |\n",
                target_us, actual_us, status));
        }

        if let Some(result) = self.results.get("reasoning_bank_lookup_10k") {
            let target_ms = 10.0;
            let actual_ms = result.mean_ns / 1_000_000.0;
            let status = if actual_ms < target_ms { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("| ReasoningBank Lookup (10K) | <{} ms | {:.2} ms | {} |\n",
                target_ms, actual_ms, status));
        }

        report
    }

    pub fn generate_json(&self) -> String {
        serde_json::to_string_pretty(&self.results).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for BenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result() {
        let samples = vec![100, 105, 95, 110, 90, 105, 100, 95, 100, 105];
        let result = BenchmarkResult::new("test".to_string(), samples);

        assert_eq!(result.iterations, 10);
        assert!(result.mean_ns > 95.0 && result.mean_ns < 110.0);
        assert!(result.median_ns > 95.0 && result.median_ns < 110.0);
    }

    #[test]
    fn test_benchmark_suite() {
        let mut suite = BenchmarkSuite::new();

        suite.run_benchmark("simple_add", 100, || {
            let _ = 1 + 1;
        });

        assert!(suite.results.contains_key("simple_add"));
        assert!(suite.results.get("simple_add").unwrap().iterations == 100);
    }
}
