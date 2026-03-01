//! Statistics collection for cost-based query optimization
//!
//! Maintains table and column statistics for query planning

use std::collections::HashMap;
use std::sync::RwLock;

/// Statistics manager for query optimization
pub struct Statistics {
    /// Table-level statistics
    tables: RwLock<HashMap<String, TableStats>>,
    /// Column-level statistics
    columns: RwLock<HashMap<String, ColumnStats>>,
}

impl Statistics {
    /// Create a new statistics manager
    pub fn new() -> Self {
        Self {
            tables: RwLock::new(HashMap::new()),
            columns: RwLock::new(HashMap::new()),
        }
    }

    /// Update table statistics
    pub fn update_table_stats(&self, table_name: String, stats: TableStats) {
        self.tables.write().unwrap().insert(table_name, stats);
    }

    /// Get table statistics
    pub fn get_table_stats(&self, table_name: &str) -> Option<TableStats> {
        self.tables.read().unwrap().get(table_name).cloned()
    }

    /// Update column statistics
    pub fn update_column_stats(&self, column_key: String, stats: ColumnStats) {
        self.columns.write().unwrap().insert(column_key, stats);
    }

    /// Get column statistics
    pub fn get_column_stats(&self, column_key: &str) -> Option<ColumnStats> {
        self.columns.read().unwrap().get(column_key).cloned()
    }

    /// Check if statistics are empty
    pub fn is_empty(&self) -> bool {
        self.tables.read().unwrap().is_empty() && self.columns.read().unwrap().is_empty()
    }

    /// Clear all statistics
    pub fn clear(&self) {
        self.tables.write().unwrap().clear();
        self.columns.write().unwrap().clear();
    }

    /// Estimate join selectivity
    pub fn estimate_join_selectivity(
        &self,
        left_table: &str,
        right_table: &str,
        join_column: &str,
    ) -> f64 {
        let left_stats = self.get_table_stats(left_table);
        let right_stats = self.get_table_stats(right_table);

        if let (Some(left), Some(right)) = (left_stats, right_stats) {
            // Simple selectivity estimate based on cardinalities
            let left_ndv = left.row_count as f64;
            let right_ndv = right.row_count as f64;

            if left_ndv > 0.0 && right_ndv > 0.0 {
                1.0 / left_ndv.max(right_ndv)
            } else {
                0.1 // Default selectivity
            }
        } else {
            0.1 // Default selectivity when stats not available
        }
    }

    /// Estimate filter selectivity
    pub fn estimate_filter_selectivity(&self, column_key: &str, operator: &str) -> f64 {
        if let Some(stats) = self.get_column_stats(column_key) {
            match operator {
                "=" => 1.0 / stats.ndv.max(1) as f64,
                ">" | "<" => 0.33,
                ">=" | "<=" => 0.33,
                "!=" => 1.0 - (1.0 / stats.ndv.max(1) as f64),
                "LIKE" => 0.1,
                "IN" => 0.2,
                _ => 0.1,
            }
        } else {
            0.1 // Default selectivity
        }
    }
}

impl Default for Statistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Table-level statistics
#[derive(Debug, Clone)]
pub struct TableStats {
    /// Total number of rows
    pub row_count: usize,
    /// Average row size in bytes
    pub avg_row_size: usize,
    /// Total table size in bytes
    pub total_size: usize,
    /// Number of distinct values (for single-column tables)
    pub ndv: usize,
    /// Last update timestamp
    pub last_updated: std::time::SystemTime,
}

impl TableStats {
    /// Create new table statistics
    pub fn new(row_count: usize, avg_row_size: usize) -> Self {
        Self {
            row_count,
            avg_row_size,
            total_size: row_count * avg_row_size,
            ndv: row_count, // Conservative estimate
            last_updated: std::time::SystemTime::now(),
        }
    }

    /// Update row count
    pub fn update_row_count(&mut self, row_count: usize) {
        self.row_count = row_count;
        self.total_size = row_count * self.avg_row_size;
        self.last_updated = std::time::SystemTime::now();
    }

    /// Estimate scan cost (relative units)
    pub fn estimate_scan_cost(&self) -> f64 {
        self.row_count as f64 * 0.001 // Simplified cost model
    }
}

/// Column-level statistics
#[derive(Debug, Clone)]
pub struct ColumnStats {
    /// Number of distinct values
    pub ndv: usize,
    /// Number of null values
    pub null_count: usize,
    /// Minimum value (for ordered types)
    pub min_value: Option<ColumnValue>,
    /// Maximum value (for ordered types)
    pub max_value: Option<ColumnValue>,
    /// Histogram for distribution
    pub histogram: Option<Histogram>,
    /// Most common values and their frequencies
    pub mcv: Vec<(ColumnValue, usize)>,
}

impl ColumnStats {
    /// Create new column statistics
    pub fn new(ndv: usize, null_count: usize) -> Self {
        Self {
            ndv,
            null_count,
            min_value: None,
            max_value: None,
            histogram: None,
            mcv: Vec::new(),
        }
    }

    /// Set min/max values
    pub fn with_range(mut self, min: ColumnValue, max: ColumnValue) -> Self {
        self.min_value = Some(min);
        self.max_value = Some(max);
        self
    }

    /// Set histogram
    pub fn with_histogram(mut self, histogram: Histogram) -> Self {
        self.histogram = Some(histogram);
        self
    }

    /// Set most common values
    pub fn with_mcv(mut self, mcv: Vec<(ColumnValue, usize)>) -> Self {
        self.mcv = mcv;
        self
    }

    /// Estimate selectivity for equality predicate
    pub fn estimate_equality_selectivity(&self, value: &ColumnValue) -> f64 {
        // Check if value is in MCV
        for (mcv_val, freq) in &self.mcv {
            if mcv_val == value {
                return *freq as f64 / self.ndv as f64;
            }
        }

        // Default: uniform distribution assumption
        if self.ndv > 0 {
            1.0 / self.ndv as f64
        } else {
            0.0
        }
    }

    /// Estimate selectivity for range predicate
    pub fn estimate_range_selectivity(&self, start: &ColumnValue, end: &ColumnValue) -> f64 {
        if let Some(histogram) = &self.histogram {
            histogram.estimate_range_selectivity(start, end)
        } else {
            0.33 // Default for range queries
        }
    }
}

/// Column value for statistics
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ColumnValue {
    Int64(i64),
    Float64(f64),
    String(String),
    Boolean(bool),
}

/// Histogram for data distribution
#[derive(Debug, Clone)]
pub struct Histogram {
    /// Histogram buckets
    pub buckets: Vec<HistogramBucket>,
    /// Total number of values
    pub total_count: usize,
}

impl Histogram {
    /// Create new histogram
    pub fn new(buckets: Vec<HistogramBucket>, total_count: usize) -> Self {
        Self {
            buckets,
            total_count,
        }
    }

    /// Create equi-width histogram
    pub fn equi_width(min: f64, max: f64, num_buckets: usize, values: &[f64]) -> Self {
        let width = (max - min) / num_buckets as f64;
        let mut buckets = Vec::with_capacity(num_buckets);

        for i in 0..num_buckets {
            let lower = min + i as f64 * width;
            let upper = if i == num_buckets - 1 {
                max
            } else {
                min + (i + 1) as f64 * width
            };

            let count = values.iter().filter(|&&v| v >= lower && v < upper).count();
            // Estimate NDV by counting unique values (using BTreeSet to avoid Hash requirement)
            let ndv = values
                .iter()
                .filter(|&&v| v >= lower && v < upper)
                .map(|&v| ordered_float::OrderedFloat(v))
                .collect::<std::collections::BTreeSet<_>>()
                .len();

            buckets.push(HistogramBucket {
                lower_bound: ColumnValue::Float64(lower),
                upper_bound: ColumnValue::Float64(upper),
                count,
                ndv,
            });
        }

        Self {
            buckets,
            total_count: values.len(),
        }
    }

    /// Estimate selectivity for range query
    pub fn estimate_range_selectivity(&self, start: &ColumnValue, end: &ColumnValue) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }

        let mut matching_count = 0;
        for bucket in &self.buckets {
            if bucket.overlaps(start, end) {
                matching_count += bucket.count;
            }
        }

        matching_count as f64 / self.total_count as f64
    }

    /// Get number of buckets
    pub fn num_buckets(&self) -> usize {
        self.buckets.len()
    }
}

/// Histogram bucket
#[derive(Debug, Clone)]
pub struct HistogramBucket {
    /// Lower bound (inclusive)
    pub lower_bound: ColumnValue,
    /// Upper bound (exclusive, except for last bucket)
    pub upper_bound: ColumnValue,
    /// Number of values in bucket
    pub count: usize,
    /// Number of distinct values in bucket
    pub ndv: usize,
}

impl HistogramBucket {
    /// Check if bucket overlaps with range
    pub fn overlaps(&self, start: &ColumnValue, end: &ColumnValue) -> bool {
        // Simplified overlap check
        self.lower_bound <= *end && self.upper_bound >= *start
    }

    /// Get bucket width (for numeric types)
    pub fn width(&self) -> Option<f64> {
        match (&self.lower_bound, &self.upper_bound) {
            (ColumnValue::Float64(lower), ColumnValue::Float64(upper)) => Some(upper - lower),
            (ColumnValue::Int64(lower), ColumnValue::Int64(upper)) => Some((upper - lower) as f64),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_creation() {
        let stats = Statistics::new();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_table_stats() {
        let stats = Statistics::new();
        let table_stats = TableStats::new(1000, 128);

        stats.update_table_stats("nodes".to_string(), table_stats.clone());

        let retrieved = stats.get_table_stats("nodes");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().row_count, 1000);
    }

    #[test]
    fn test_column_stats() {
        let stats = Statistics::new();
        let col_stats = ColumnStats::new(500, 10);

        stats.update_column_stats("nodes.id".to_string(), col_stats);

        let retrieved = stats.get_column_stats("nodes.id");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().ndv, 500);
    }

    #[test]
    fn test_histogram_creation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let histogram = Histogram::equi_width(1.0, 10.0, 5, &values);

        assert_eq!(histogram.num_buckets(), 5);
        assert_eq!(histogram.total_count, 10);
    }

    #[test]
    fn test_selectivity_estimation() {
        let stats = Statistics::new();
        let table_stats = TableStats::new(1000, 128);

        stats.update_table_stats("nodes".to_string(), table_stats);

        let selectivity = stats.estimate_join_selectivity("nodes", "edges", "id");
        assert!(selectivity > 0.0 && selectivity <= 1.0);
    }

    #[test]
    fn test_filter_selectivity() {
        let stats = Statistics::new();
        let col_stats = ColumnStats::new(100, 5);

        stats.update_column_stats("nodes.age".to_string(), col_stats);

        let selectivity = stats.estimate_filter_selectivity("nodes.age", "=");
        assert_eq!(selectivity, 0.01); // 1/100
    }
}
