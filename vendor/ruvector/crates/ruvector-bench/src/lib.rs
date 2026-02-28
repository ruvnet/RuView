//! Benchmarking utilities for Ruvector
//!
//! This module provides comprehensive benchmarking tools including:
//! - ANN-Benchmarks compatibility for standardized testing
//! - AgenticDB workload simulation
//! - Latency profiling (p50, p95, p99, p99.9)
//! - Memory usage analysis
//! - Cross-system performance comparison
//! - CPU and memory profiling with flamegraphs

use anyhow::{Context, Result};
use rand::Rng;
use rand_distr::{Distribution, Normal, Uniform};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Benchmark result for a single test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub dataset: String,
    pub dimensions: usize,
    pub num_vectors: usize,
    pub num_queries: usize,
    pub k: usize,
    pub qps: f64,
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub latency_p99: f64,
    pub latency_p999: f64,
    pub recall_at_1: f64,
    pub recall_at_10: f64,
    pub recall_at_100: f64,
    pub memory_mb: f64,
    pub build_time_secs: f64,
    pub metadata: HashMap<String, String>,
}

/// Statistics collector using HDR histogram
pub struct LatencyStats {
    histogram: hdrhistogram::Histogram<u64>,
}

impl LatencyStats {
    pub fn new() -> Result<Self> {
        let histogram = hdrhistogram::Histogram::new_with_bounds(1, 60_000_000, 3)?;
        Ok(Self { histogram })
    }

    pub fn record(&mut self, duration: Duration) -> Result<()> {
        let micros = duration.as_micros() as u64;
        self.histogram.record(micros)?;
        Ok(())
    }

    pub fn percentile(&self, percentile: f64) -> Duration {
        let micros = self.histogram.value_at_percentile(percentile);
        Duration::from_micros(micros)
    }

    pub fn mean(&self) -> Duration {
        Duration::from_micros(self.histogram.mean() as u64)
    }

    pub fn count(&self) -> u64 {
        self.histogram.len()
    }
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

/// Dataset generator for synthetic benchmarks
pub struct DatasetGenerator {
    dimensions: usize,
    distribution: VectorDistribution,
}

#[derive(Debug, Clone, Copy)]
pub enum VectorDistribution {
    Uniform,
    Normal { mean: f32, std_dev: f32 },
    Clustered { num_clusters: usize },
}

impl DatasetGenerator {
    pub fn new(dimensions: usize, distribution: VectorDistribution) -> Self {
        Self {
            dimensions,
            distribution,
        }
    }

    pub fn generate(&self, count: usize) -> Vec<Vec<f32>> {
        let mut rng = rand::thread_rng();
        (0..count).map(|_| self.generate_vector(&mut rng)).collect()
    }

    fn generate_vector<R: Rng>(&self, rng: &mut R) -> Vec<f32> {
        match self.distribution {
            VectorDistribution::Uniform => {
                let uniform = Uniform::new(-1.0, 1.0);
                (0..self.dimensions).map(|_| uniform.sample(rng)).collect()
            }
            VectorDistribution::Normal { mean, std_dev } => {
                let normal = Normal::new(mean, std_dev).unwrap();
                (0..self.dimensions).map(|_| normal.sample(rng)).collect()
            }
            VectorDistribution::Clustered { num_clusters } => {
                let cluster_id = rng.gen_range(0..num_clusters);
                let center_offset = cluster_id as f32 * 10.0;
                let normal = Normal::new(center_offset, 1.0).unwrap();
                (0..self.dimensions).map(|_| normal.sample(rng)).collect()
            }
        }
    }

    pub fn normalize_vector(vec: &mut [f32]) {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in vec.iter_mut() {
                *x /= norm;
            }
        }
    }
}

/// Result writer for benchmark outputs
pub struct ResultWriter {
    output_dir: PathBuf,
}

impl ResultWriter {
    pub fn new<P: AsRef<Path>>(output_dir: P) -> Result<Self> {
        let output_dir = output_dir.as_ref().to_path_buf();
        fs::create_dir_all(&output_dir)?;
        Ok(Self { output_dir })
    }

    pub fn write_json<T: Serialize>(&self, name: &str, data: &T) -> Result<()> {
        let path = self.output_dir.join(format!("{}.json", name));
        let file = File::create(&path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, data)?;
        println!("✓ Written results to: {}", path.display());
        Ok(())
    }

    pub fn write_csv(&self, name: &str, results: &[BenchmarkResult]) -> Result<()> {
        let path = self.output_dir.join(format!("{}.csv", name));
        let mut file = File::create(&path)?;

        // Write header
        writeln!(
            file,
            "name,dataset,dimensions,num_vectors,num_queries,k,qps,p50,p95,p99,p999,recall@1,recall@10,recall@100,memory_mb,build_time"
        )?;

        // Write data
        for result in results {
            writeln!(
                file,
                "{},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{:.2},{:.2}",
                result.name,
                result.dataset,
                result.dimensions,
                result.num_vectors,
                result.num_queries,
                result.k,
                result.qps,
                result.latency_p50,
                result.latency_p95,
                result.latency_p99,
                result.latency_p999,
                result.recall_at_1,
                result.recall_at_10,
                result.recall_at_100,
                result.memory_mb,
                result.build_time_secs,
            )?;
        }

        println!("✓ Written CSV to: {}", path.display());
        Ok(())
    }

    pub fn write_markdown_report(&self, name: &str, results: &[BenchmarkResult]) -> Result<()> {
        let path = self.output_dir.join(format!("{}.md", name));
        let mut file = File::create(&path)?;

        writeln!(file, "# Ruvector Benchmark Results\n")?;
        writeln!(
            file,
            "Generated: {}\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )?;

        for result in results {
            writeln!(file, "## {}\n", result.name)?;
            writeln!(
                file,
                "**Dataset:** {} ({}D, {} vectors)\n",
                result.dataset, result.dimensions, result.num_vectors
            )?;
            writeln!(file, "### Performance")?;
            writeln!(file, "- **QPS:** {:.2}", result.qps)?;
            writeln!(file, "- **Latency (p50):** {:.2}ms", result.latency_p50)?;
            writeln!(file, "- **Latency (p95):** {:.2}ms", result.latency_p95)?;
            writeln!(file, "- **Latency (p99):** {:.2}ms", result.latency_p99)?;
            writeln!(file, "- **Latency (p99.9):** {:.2}ms", result.latency_p999)?;
            writeln!(file, "")?;
            writeln!(file, "### Recall")?;
            writeln!(file, "- **Recall@1:** {:.2}%", result.recall_at_1 * 100.0)?;
            writeln!(file, "- **Recall@10:** {:.2}%", result.recall_at_10 * 100.0)?;
            writeln!(
                file,
                "- **Recall@100:** {:.2}%",
                result.recall_at_100 * 100.0
            )?;
            writeln!(file, "")?;
            writeln!(file, "### Resources")?;
            writeln!(file, "- **Memory:** {:.2} MB", result.memory_mb)?;
            writeln!(file, "- **Build Time:** {:.2}s", result.build_time_secs)?;
            writeln!(file, "")?;
        }

        println!("✓ Written markdown report to: {}", path.display());
        Ok(())
    }
}

/// Memory profiler
pub struct MemoryProfiler {
    #[cfg(feature = "profiling")]
    initial_allocated: usize,
    #[cfg(not(feature = "profiling"))]
    _phantom: (),
}

impl MemoryProfiler {
    pub fn new() -> Self {
        #[cfg(feature = "profiling")]
        {
            use jemalloc_ctl::{epoch, stats};
            epoch::mib().unwrap().advance().unwrap();
            let allocated = stats::allocated::mib().unwrap().read().unwrap();
            Self {
                initial_allocated: allocated,
            }
        }
        #[cfg(not(feature = "profiling"))]
        {
            Self { _phantom: () }
        }
    }

    pub fn current_usage_mb(&self) -> f64 {
        #[cfg(feature = "profiling")]
        {
            use jemalloc_ctl::{epoch, stats};
            epoch::mib().unwrap().advance().unwrap();
            let allocated = stats::allocated::mib().unwrap().read().unwrap();
            (allocated - self.initial_allocated) as f64 / 1_048_576.0
        }
        #[cfg(not(feature = "profiling"))]
        {
            0.0
        }
    }

    pub fn system_memory_info() -> Result<(u64, u64)> {
        use sysinfo::System;
        let mut sys = System::new_all();
        sys.refresh_all();
        let total = sys.total_memory();
        let used = sys.used_memory();
        Ok((total, used))
    }
}

impl Default for MemoryProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate recall between search results and ground truth
pub fn calculate_recall(results: &[Vec<String>], ground_truth: &[Vec<String>], k: usize) -> f64 {
    assert_eq!(results.len(), ground_truth.len());

    let mut total_recall = 0.0;
    for (result, truth) in results.iter().zip(ground_truth.iter()) {
        let result_set: std::collections::HashSet<_> = result.iter().take(k).collect();
        let truth_set: std::collections::HashSet<_> = truth.iter().take(k).collect();
        let intersection = result_set.intersection(&truth_set).count();
        total_recall += intersection as f64 / k.min(truth.len()) as f64;
    }

    total_recall / results.len() as f64
}

/// Progress bar helper
pub fn create_progress_bar(len: u64, msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(len);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(msg.to_string());
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_generator() {
        let gen = DatasetGenerator::new(128, VectorDistribution::Uniform);
        let vectors = gen.generate(100);
        assert_eq!(vectors.len(), 100);
        assert_eq!(vectors[0].len(), 128);
    }

    #[test]
    fn test_latency_stats() {
        let mut stats = LatencyStats::new().unwrap();
        for i in 0..1000 {
            stats.record(Duration::from_micros(i)).unwrap();
        }
        assert!(stats.percentile(0.5).as_micros() > 0);
    }

    #[test]
    fn test_recall_calculation() {
        let results = vec![
            vec!["1".to_string(), "2".to_string(), "3".to_string()],
            vec!["4".to_string(), "5".to_string(), "6".to_string()],
        ];
        let ground_truth = vec![
            vec!["1".to_string(), "2".to_string(), "7".to_string()],
            vec!["4".to_string(), "8".to_string(), "6".to_string()],
        ];
        let recall = calculate_recall(&results, &ground_truth, 3);
        assert!((recall - 0.666).abs() < 0.01);
    }
}
