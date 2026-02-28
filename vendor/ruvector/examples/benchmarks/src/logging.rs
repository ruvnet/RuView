//! Logging Schema for Benchmark Results
//!
//! Comprehensive logging for:
//! - Temporal reasoning benchmarks
//! - Vector operations
//! - Swarm controller metrics
//! - Tool usage tracking

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Log entry types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEntry {
    /// Temporal benchmark run
    TemporalBenchmark(TemporalBenchmarkLog),
    /// Vector operation
    VectorOperation(VectorOperationLog),
    /// Swarm episode
    SwarmEpisode(SwarmEpisodeLog),
    /// Tool call
    ToolCall(ToolCallLog),
    /// System event
    System(SystemLog),
}

/// Temporal benchmark log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporalBenchmarkLog {
    pub timestamp: DateTime<Utc>,
    pub benchmark_id: String,
    pub puzzle_id: String,
    pub difficulty: u8,
    pub solved: bool,
    pub correct: bool,
    pub steps: usize,
    pub tool_calls: usize,
    pub latency_ms: u64,
    pub constraint_count: usize,
    pub calendar_tool_enabled: bool,
    pub web_search_enabled: bool,
}

/// Vector operation log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VectorOperationLog {
    pub timestamp: DateTime<Utc>,
    pub operation: String,
    pub index_dim: usize,
    pub index_size: usize,
    pub query_count: usize,
    pub top_k: usize,
    pub ivf_enabled: bool,
    pub coherence_score: f32,
    pub latency_us: u64,
    pub results_count: usize,
}

/// Swarm episode log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmEpisodeLog {
    pub timestamp: DateTime<Utc>,
    pub episode: usize,
    pub num_tasks: usize,
    pub solved: usize,
    pub correct: usize,
    pub reward: f64,
    pub oracle_reward: f64,
    pub regret: f64,
    pub cumulative_regret: f64,
    pub average_regret: f64,
    pub is_sublinear: bool,
}

/// Tool call log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallLog {
    pub timestamp: DateTime<Utc>,
    pub tool_name: String,
    pub tool_type: String,
    pub input_summary: String,
    pub success: bool,
    pub latency_ms: u64,
    pub context: String,
}

/// System log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemLog {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub component: String,
}

/// Benchmark logger
pub struct BenchmarkLogger {
    /// Log file path
    path: String,
    /// Writer
    writer: Option<BufWriter<File>>,
    /// In-memory buffer for batch writes
    buffer: Vec<LogEntry>,
    /// Buffer size before flush
    flush_threshold: usize,
}

impl BenchmarkLogger {
    /// Create a new logger
    pub fn new(path: impl Into<String>) -> Result<Self> {
        let path = path.into();

        // Create parent directories
        if let Some(parent) = Path::new(&path).parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(Self {
            path,
            writer: Some(BufWriter::new(file)),
            buffer: Vec::new(),
            flush_threshold: 100,
        })
    }

    /// Log an entry
    pub fn log(&mut self, entry: LogEntry) -> Result<()> {
        self.buffer.push(entry);
        if self.buffer.len() >= self.flush_threshold {
            self.flush()?;
        }
        Ok(())
    }

    /// Log a temporal benchmark result
    pub fn log_temporal(
        &mut self,
        benchmark_id: impl Into<String>,
        puzzle_id: impl Into<String>,
        difficulty: u8,
        solved: bool,
        correct: bool,
        steps: usize,
        tool_calls: usize,
        latency_ms: u64,
        constraint_count: usize,
        calendar_tool: bool,
        web_search: bool,
    ) -> Result<()> {
        self.log(LogEntry::TemporalBenchmark(TemporalBenchmarkLog {
            timestamp: Utc::now(),
            benchmark_id: benchmark_id.into(),
            puzzle_id: puzzle_id.into(),
            difficulty,
            solved,
            correct,
            steps,
            tool_calls,
            latency_ms,
            constraint_count,
            calendar_tool_enabled: calendar_tool,
            web_search_enabled: web_search,
        }))
    }

    /// Log a vector operation
    pub fn log_vector(
        &mut self,
        operation: impl Into<String>,
        index_dim: usize,
        index_size: usize,
        query_count: usize,
        top_k: usize,
        ivf_enabled: bool,
        coherence_score: f32,
        latency_us: u64,
        results_count: usize,
    ) -> Result<()> {
        self.log(LogEntry::VectorOperation(VectorOperationLog {
            timestamp: Utc::now(),
            operation: operation.into(),
            index_dim,
            index_size,
            query_count,
            top_k,
            ivf_enabled,
            coherence_score,
            latency_us,
            results_count,
        }))
    }

    /// Log a swarm episode
    pub fn log_swarm(
        &mut self,
        episode: usize,
        num_tasks: usize,
        solved: usize,
        correct: usize,
        reward: f64,
        oracle_reward: f64,
        cumulative_regret: f64,
        average_regret: f64,
        is_sublinear: bool,
    ) -> Result<()> {
        self.log(LogEntry::SwarmEpisode(SwarmEpisodeLog {
            timestamp: Utc::now(),
            episode,
            num_tasks,
            solved,
            correct,
            reward,
            oracle_reward,
            regret: oracle_reward - reward,
            cumulative_regret,
            average_regret,
            is_sublinear,
        }))
    }

    /// Log a tool call
    pub fn log_tool(
        &mut self,
        tool_name: impl Into<String>,
        tool_type: impl Into<String>,
        input_summary: impl Into<String>,
        success: bool,
        latency_ms: u64,
        context: impl Into<String>,
    ) -> Result<()> {
        self.log(LogEntry::ToolCall(ToolCallLog {
            timestamp: Utc::now(),
            tool_name: tool_name.into(),
            tool_type: tool_type.into(),
            input_summary: input_summary.into(),
            success,
            latency_ms,
            context: context.into(),
        }))
    }

    /// Log a system message
    pub fn log_system(
        &mut self,
        level: impl Into<String>,
        message: impl Into<String>,
        component: impl Into<String>,
    ) -> Result<()> {
        self.log(LogEntry::System(SystemLog {
            timestamp: Utc::now(),
            level: level.into(),
            message: message.into(),
            component: component.into(),
        }))
    }

    /// Flush buffer to file
    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            for entry in self.buffer.drain(..) {
                let json = serde_json::to_string(&entry)?;
                writeln!(writer, "{}", json)?;
            }
            writer.flush()?;
        }
        Ok(())
    }

    /// Close the logger
    pub fn close(&mut self) -> Result<()> {
        self.flush()?;
        self.writer = None;
        Ok(())
    }

    /// Get log file path
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Drop for BenchmarkLogger {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// Log reader for analysis
pub struct LogReader {
    path: String,
}

impl LogReader {
    /// Create a new reader
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Read all entries
    pub fn read_all(&self) -> Result<Vec<LogEntry>> {
        let content = fs::read_to_string(&self.path)?;
        let mut entries = Vec::new();
        for line in content.lines() {
            if !line.is_empty() {
                let entry: LogEntry = serde_json::from_str(line)?;
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Read temporal benchmark entries only
    pub fn read_temporal(&self) -> Result<Vec<TemporalBenchmarkLog>> {
        let entries = self.read_all()?;
        Ok(entries
            .into_iter()
            .filter_map(|e| match e {
                LogEntry::TemporalBenchmark(t) => Some(t),
                _ => None,
            })
            .collect())
    }

    /// Read swarm episode entries only
    pub fn read_swarm(&self) -> Result<Vec<SwarmEpisodeLog>> {
        let entries = self.read_all()?;
        Ok(entries
            .into_iter()
            .filter_map(|e| match e {
                LogEntry::SwarmEpisode(s) => Some(s),
                _ => None,
            })
            .collect())
    }

    /// Compute aggregate statistics
    pub fn aggregate_temporal(&self) -> Result<TemporalAggregates> {
        let logs = self.read_temporal()?;
        if logs.is_empty() {
            return Ok(TemporalAggregates::default());
        }

        let total = logs.len();
        let solved = logs.iter().filter(|l| l.solved).count();
        let correct = logs.iter().filter(|l| l.correct).count();
        let avg_steps = logs.iter().map(|l| l.steps).sum::<usize>() as f64 / total as f64;
        let avg_latency = logs.iter().map(|l| l.latency_ms).sum::<u64>() as f64 / total as f64;
        let avg_tools = logs.iter().map(|l| l.tool_calls).sum::<usize>() as f64 / total as f64;

        // By difficulty
        let mut by_difficulty: std::collections::HashMap<u8, (usize, usize)> =
            std::collections::HashMap::new();
        for log in &logs {
            let entry = by_difficulty.entry(log.difficulty).or_insert((0, 0));
            entry.0 += 1;
            if log.correct {
                entry.1 += 1;
            }
        }

        Ok(TemporalAggregates {
            total_puzzles: total,
            solved_count: solved,
            correct_count: correct,
            accuracy: correct as f64 / total as f64,
            avg_steps,
            avg_latency_ms: avg_latency,
            avg_tool_calls: avg_tools,
            accuracy_by_difficulty: by_difficulty
                .into_iter()
                .map(|(d, (t, c))| (d, c as f64 / t as f64))
                .collect(),
        })
    }
}

/// Aggregate statistics for temporal benchmarks
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TemporalAggregates {
    pub total_puzzles: usize,
    pub solved_count: usize,
    pub correct_count: usize,
    pub accuracy: f64,
    pub avg_steps: f64,
    pub avg_latency_ms: f64,
    pub avg_tool_calls: f64,
    pub accuracy_by_difficulty: std::collections::HashMap<u8, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_logger() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let mut logger = BenchmarkLogger::new(path.to_str().unwrap()).unwrap();

        logger
            .log_temporal(
                "bench-1", "puzzle-1", 5, true, true, 10, 2, 100, 3, true, false,
            )
            .unwrap();

        logger.flush().unwrap();

        let reader = LogReader::new(path.to_str().unwrap());
        let entries = reader.read_all().unwrap();
        assert_eq!(entries.len(), 1);
    }
}
