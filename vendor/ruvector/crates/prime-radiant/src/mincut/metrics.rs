//! Isolation Metrics
//!
//! Tracking and analysis of isolation operations.

use super::IsolationResult;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Metrics for tracking isolation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationMetrics {
    /// Total number of isolation queries
    pub total_queries: u64,
    /// Number of queries that found isolation
    pub queries_with_isolation: u64,
    /// Total vertices isolated across all queries
    pub total_vertices_isolated: u64,
    /// Total cut edges across all queries
    pub total_cut_edges: u64,
    /// Total cut value across all queries
    pub total_cut_value: f64,
    /// Average vertices isolated per query (that had isolation)
    pub avg_vertices_isolated: f64,
    /// Average cut value per query (that had isolation)
    pub avg_cut_value: f64,
    /// Number of build operations
    pub num_builds: u64,
    /// Number of incremental updates
    pub num_updates: u64,
    /// Maximum single isolation size
    pub max_isolation_size: usize,
    /// Minimum non-zero cut value
    pub min_cut_value: f64,
    /// Start time for tracking
    #[serde(skip)]
    start_time: Option<Instant>,
    /// Total time spent in isolation queries (microseconds)
    pub total_query_time_us: u64,
}

impl IsolationMetrics {
    /// Create new metrics tracker
    pub fn new() -> Self {
        Self {
            total_queries: 0,
            queries_with_isolation: 0,
            total_vertices_isolated: 0,
            total_cut_edges: 0,
            total_cut_value: 0.0,
            avg_vertices_isolated: 0.0,
            avg_cut_value: 0.0,
            num_builds: 0,
            num_updates: 0,
            max_isolation_size: 0,
            min_cut_value: f64::INFINITY,
            start_time: None,
            total_query_time_us: 0,
        }
    }

    /// Record an isolation query result
    pub fn record_isolation(&mut self, result: &IsolationResult) {
        self.total_queries += 1;

        if result.has_isolation() {
            self.queries_with_isolation += 1;
            self.total_vertices_isolated += result.num_isolated() as u64;
            self.total_cut_edges += result.num_cut_edges() as u64;
            self.total_cut_value += result.cut_value;

            self.max_isolation_size = self.max_isolation_size.max(result.num_isolated());

            if result.cut_value > 0.0 {
                self.min_cut_value = self.min_cut_value.min(result.cut_value);
            }

            // Update averages
            self.avg_vertices_isolated =
                self.total_vertices_isolated as f64 / self.queries_with_isolation as f64;
            self.avg_cut_value = self.total_cut_value / self.queries_with_isolation as f64;
        }
    }

    /// Record a build operation
    pub fn record_build(&mut self) {
        self.num_builds += 1;
    }

    /// Record an incremental update
    pub fn record_update(&mut self) {
        self.num_updates += 1;
    }

    /// Start timing a query
    pub fn start_query(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// End timing a query
    pub fn end_query(&mut self) {
        if let Some(start) = self.start_time.take() {
            self.total_query_time_us += start.elapsed().as_micros() as u64;
        }
    }

    /// Get isolation rate (queries with isolation / total queries)
    pub fn isolation_rate(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.queries_with_isolation as f64 / self.total_queries as f64
    }

    /// Get average query time in microseconds
    pub fn avg_query_time_us(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.total_query_time_us as f64 / self.total_queries as f64
    }

    /// Get updates per build ratio
    pub fn updates_per_build(&self) -> f64 {
        if self.num_builds == 0 {
            return self.num_updates as f64;
        }
        self.num_updates as f64 / self.num_builds as f64
    }

    /// Get efficiency (vertices isolated per cut value)
    pub fn isolation_efficiency(&self) -> f64 {
        if self.total_cut_value < 1e-10 {
            return 0.0;
        }
        self.total_vertices_isolated as f64 / self.total_cut_value
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Create a summary report
    pub fn summary(&self) -> MetricsSummary {
        MetricsSummary {
            total_queries: self.total_queries,
            isolation_rate: self.isolation_rate(),
            avg_vertices_isolated: self.avg_vertices_isolated,
            avg_cut_value: self.avg_cut_value,
            avg_query_time_us: self.avg_query_time_us(),
            max_isolation_size: self.max_isolation_size,
            updates_per_build: self.updates_per_build(),
            isolation_efficiency: self.isolation_efficiency(),
        }
    }
}

impl Default for IsolationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of isolation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// Total isolation queries
    pub total_queries: u64,
    /// Rate of queries that found isolation
    pub isolation_rate: f64,
    /// Average vertices isolated per successful query
    pub avg_vertices_isolated: f64,
    /// Average cut value per successful query
    pub avg_cut_value: f64,
    /// Average query time in microseconds
    pub avg_query_time_us: f64,
    /// Maximum single isolation size
    pub max_isolation_size: usize,
    /// Updates per build operation
    pub updates_per_build: f64,
    /// Vertices isolated per unit cut value
    pub isolation_efficiency: f64,
}

impl std::fmt::Display for MetricsSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Isolation Metrics Summary:")?;
        writeln!(f, "  Total queries: {}", self.total_queries)?;
        writeln!(f, "  Isolation rate: {:.2}%", self.isolation_rate * 100.0)?;
        writeln!(
            f,
            "  Avg vertices isolated: {:.2}",
            self.avg_vertices_isolated
        )?;
        writeln!(f, "  Avg cut value: {:.4}", self.avg_cut_value)?;
        writeln!(f, "  Avg query time: {:.2} us", self.avg_query_time_us)?;
        writeln!(f, "  Max isolation size: {}", self.max_isolation_size)?;
        writeln!(f, "  Updates per build: {:.2}", self.updates_per_build)?;
        writeln!(
            f,
            "  Isolation efficiency: {:.4}",
            self.isolation_efficiency
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_result(num_isolated: usize, cut_value: f64) -> IsolationResult {
        let mut isolated = HashSet::new();
        for i in 0..num_isolated {
            isolated.insert(i as u64);
        }

        IsolationResult {
            isolated_vertices: isolated,
            cut_edges: vec![(0, 100)], // dummy
            cut_value,
            num_high_energy_edges: 1,
            threshold: 1.0,
            is_verified: true,
        }
    }

    #[test]
    fn test_new_metrics() {
        let metrics = IsolationMetrics::new();
        assert_eq!(metrics.total_queries, 0);
        assert_eq!(metrics.isolation_rate(), 0.0);
    }

    #[test]
    fn test_record_isolation() {
        let mut metrics = IsolationMetrics::new();

        let result = make_result(5, 2.5);
        metrics.record_isolation(&result);

        assert_eq!(metrics.total_queries, 1);
        assert_eq!(metrics.queries_with_isolation, 1);
        assert_eq!(metrics.total_vertices_isolated, 5);
        assert!((metrics.avg_cut_value - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_no_isolation() {
        let mut metrics = IsolationMetrics::new();

        let result = IsolationResult::no_isolation();
        metrics.record_isolation(&result);

        assert_eq!(metrics.total_queries, 1);
        assert_eq!(metrics.queries_with_isolation, 0);
        assert_eq!(metrics.isolation_rate(), 0.0);
    }

    #[test]
    fn test_multiple_queries() {
        let mut metrics = IsolationMetrics::new();

        metrics.record_isolation(&make_result(5, 2.0));
        metrics.record_isolation(&make_result(10, 3.0));
        metrics.record_isolation(&IsolationResult::no_isolation());

        assert_eq!(metrics.total_queries, 3);
        assert_eq!(metrics.queries_with_isolation, 2);
        assert!((metrics.isolation_rate() - 2.0 / 3.0).abs() < 0.01);
        assert_eq!(metrics.max_isolation_size, 10);
    }

    #[test]
    fn test_build_and_update() {
        let mut metrics = IsolationMetrics::new();

        metrics.record_build();
        metrics.record_update();
        metrics.record_update();
        metrics.record_update();

        assert_eq!(metrics.num_builds, 1);
        assert_eq!(metrics.num_updates, 3);
        assert!((metrics.updates_per_build() - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_summary() {
        let mut metrics = IsolationMetrics::new();

        metrics.record_isolation(&make_result(5, 2.0));
        metrics.record_isolation(&make_result(10, 3.0));

        let summary = metrics.summary();
        assert_eq!(summary.total_queries, 2);
        assert!((summary.isolation_rate - 1.0).abs() < 0.01);
        assert!((summary.avg_vertices_isolated - 7.5).abs() < 0.01);
    }
}
