//! Metric-specific aggregation functionality.
//!
//! This module provides metric-specific implementations of aggregation functions
//! while leveraging the core aggregation framework from `crate::aggregation`.
//! It handles the specific requirements of metric aggregation, such as:
//! - Running window calculations (sum, avg, count)
//! - Metric-specific SQL query generation
//! - Direct aggregation of MetricRecord instances
//!
//! The implementation reuses the generic aggregation types and query building
//! functionality from the core aggregation module while adding metric-specific
//! logic and optimizations.

use crate::metrics::MetricRecord;
use crate::aggregation::{AggregateFunction, GroupBy, build_aggregate_query};
use tonic::Status;

/// The standard metric value columns used in aggregation queries
const METRIC_VALUE_COLUMNS: [&str; 3] = [
    "value_running_window_sum",
    "value_running_window_avg",
    "value_running_window_count"
];

/// Applies the aggregation function to a set of metrics.
///
/// This function implements metric-specific aggregation by operating directly
/// on MetricRecord instances. It uses the appropriate running window value
/// based on the aggregation function type.
///
/// # Arguments
///
/// * `function` - The aggregation function to apply
/// * `metrics` - The set of metrics to aggregate
///
/// # Returns
///
/// The aggregated value as a float, or an error if the operation fails
pub fn apply_function(function: AggregateFunction, metrics: &[MetricRecord]) -> Result<f64, Status> {
    if metrics.is_empty() {
        return Ok(0.0);
    }

    match function {
        AggregateFunction::Sum => Ok(metrics.iter().map(|m| m.value_running_window_sum).sum()),
        AggregateFunction::Avg => {
            let sum: f64 = metrics.iter().map(|m| m.value_running_window_avg).sum();
            Ok(sum / metrics.len() as f64)
        },
        AggregateFunction::Min => Ok(metrics
            .iter()
            .map(|m| m.value_running_window_sum)
            .fold(f64::INFINITY, f64::min)),
        AggregateFunction::Max => Ok(metrics
            .iter()
            .map(|m| m.value_running_window_sum)
            .fold(f64::NEG_INFINITY, f64::max)),
        AggregateFunction::Count => Ok(metrics.len() as f64),
    }
}

/// Builds a SQL query for metrics aggregation.
///
/// This function specializes the generic aggregate query builder for metrics
/// by providing the metric-specific value columns and table name. It reuses
/// the core query building logic while adding metric-specific context.
///
/// # Arguments
///
/// * `function` - The aggregation function to apply
/// * `group_by` - The grouping specification
/// * `from_timestamp` - The start of the time range
/// * `to_timestamp` - The optional end of the time range
///
/// # Returns
///
/// A SQL query string optimized for metric aggregation
pub fn build_metrics_query(
    function: AggregateFunction,
    group_by: &GroupBy,
    from_timestamp: i64,
    to_timestamp: Option<i64>,
) -> String {
    let columns = METRIC_VALUE_COLUMNS.iter()
        .map(|&c| c.to_string())
        .collect::<Vec<_>>();

    let column_refs: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
    build_aggregate_query(
        "metrics",
        function,
        group_by,
        &column_refs,
        Some(from_timestamp),
        to_timestamp,
    )
} 