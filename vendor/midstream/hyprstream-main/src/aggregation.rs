//! Core aggregation framework for time-series data.
//!
//! This module provides the foundational types and functionality for aggregating
//! time-series data. It defines:
//! - Generic aggregation functions (Sum, Avg, Min, Max, Count)
//! - Time window specifications (None, Fixed, Sliding)
//! - Grouping operations
//! - SQL query generation
//!
//! This framework is used by more specific aggregation implementations, such as
//! the metric-specific aggregation in `crate::metrics::aggregation`.

use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::fmt::{Display, Formatter};

/// Time window for aggregation
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimeWindow {
    /// No time window, aggregate all data
    None,
    /// Fixed time window (e.g., 5 minutes, 1 hour)
    Fixed(Duration),
    /// Sliding time window with window size and slide interval
    Sliding {
        window: Duration,
        slide: Duration,
    },
}

/// Generic aggregation functions that can be applied to time-series data
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AggregateFunction {
    /// Count the number of values
    Count,
    /// Sum all values
    Sum,
    /// Calculate the average
    Avg,
    /// Find the minimum value
    Min,
    /// Find the maximum value
    Max,
}

impl Display for AggregateFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregateFunction::Count => write!(f, "COUNT"),
            AggregateFunction::Sum => write!(f, "SUM"),
            AggregateFunction::Avg => write!(f, "AVG"),
            AggregateFunction::Min => write!(f, "MIN"),
            AggregateFunction::Max => write!(f, "MAX"),
        }
    }
}

/// Grouping specification for aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBy {
    pub columns: Vec<String>,
    pub time_column: Option<String>,
}

/// Result of an aggregation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateResult {
    pub value: f64,
    pub timestamp: i64,
}

impl TimeWindow {
    /// Calculates the window boundaries for a given timestamp
    pub fn window_bounds(&self, timestamp: i64) -> (i64, i64) {
        match *self {
            TimeWindow::None => (i64::MIN, i64::MAX),
            TimeWindow::Fixed(duration) => {
                let window_size = duration.as_secs() as i64;
                let window_start = (timestamp / window_size) * window_size;
                (window_start, window_start + window_size)
            },
            TimeWindow::Sliding { window, slide } => {
                let window_size = window.as_secs() as i64;
                let slide_size = slide.as_secs() as i64;
                let current_slide = (timestamp / slide_size) * slide_size;
                (current_slide, current_slide + window_size)
            }
        }
    }

    /// Generates SQL expressions for window boundaries
    pub fn to_sql(&self) -> Option<String> {
        match *self {
            TimeWindow::None => None,
            TimeWindow::Fixed(duration) => {
                let window_size = duration.as_secs();
                Some(format!(
                    "(timestamp / {}) * {} as window_start, 
                    ((timestamp / {}) + 1) * {} as window_end",
                    window_size, window_size, window_size, window_size
                ))
            },
            TimeWindow::Sliding { window, slide } => {
                let window_size = window.as_secs();
                let slide_size = slide.as_secs();
                Some(format!(
                    "(timestamp / {}) * {} as window_start,
                    ((timestamp / {}) * {} + {}) as window_end",
                    slide_size, slide_size, slide_size, slide_size, window_size
                ))
            }
        }
    }
}

impl AggregateFunction {
    /// Generates SQL for the aggregation function
    pub fn to_sql(&self, column: &str) -> String {
        match self {
            AggregateFunction::Sum => format!("SUM({})", column),
            AggregateFunction::Avg => format!("AVG({})", column),
            AggregateFunction::Min => format!("MIN({})", column),
            AggregateFunction::Max => format!("MAX({})", column),
            AggregateFunction::Count => format!("COUNT({})", column),
        }
    }
}

/// Builds a SQL query for aggregation.
///
/// This is the core query builder used by specific aggregation implementations.
/// It provides a flexible way to build SQL queries for different types of
/// time-series data aggregation.
///
/// # Arguments
///
/// * `table_name` - The source table name
/// * `function` - The aggregation function to apply
/// * `group_by` - The grouping specification
/// * `columns` - The columns to aggregate
/// * `from_timestamp` - Optional start of the time range
/// * `to_timestamp` - Optional end of the time range
///
/// # Returns
///
/// A SQL query string for the specified aggregation
pub fn build_aggregate_query(
    table_name: &str,
    function: AggregateFunction,
    group_by: &GroupBy,
    columns: &[&str],
    from_timestamp: Option<i64>,
    to_timestamp: Option<i64>,
) -> String {
    let mut query = String::new();
    
    // Build SELECT clause
    query.push_str("SELECT ");
    
    // Add group by columns
    if !group_by.columns.is_empty() {
        let cols: Vec<&str> = group_by.columns.iter().map(|s| s.as_str()).collect();
        query.push_str(&cols.join(", "));
        query.push_str(", ");
    }
    
    // Add time column if present
    if let Some(time_col) = &group_by.time_column {
        query.push_str(&format!("{}, ", time_col));
    }
    
    // Add aggregation function
    match function {
        AggregateFunction::Sum => query.push_str("SUM(value)"),
        AggregateFunction::Avg => query.push_str("AVG(value)"),
        AggregateFunction::Count => query.push_str("COUNT(*)"),
        AggregateFunction::Min => query.push_str("MIN(value)"),
        AggregateFunction::Max => query.push_str("MAX(value)"),
    }
    
    // Add FROM clause
    query.push_str(&format!(" FROM {}", table_name));
    
    // Add WHERE clause for timestamp range
    if let Some(from_ts) = from_timestamp {
        query.push_str(&format!(" WHERE timestamp >= {}", from_ts));
        if let Some(to_ts) = to_timestamp {
            query.push_str(&format!(" AND timestamp <= {}", to_ts));
        }
    }
    
    // Add GROUP BY clause
    if !group_by.columns.is_empty() || group_by.time_column.is_some() {
        query.push_str(" GROUP BY ");
        let mut group_cols = Vec::new();
        
        if !group_by.columns.is_empty() {
            let cols: Vec<&str> = group_by.columns.iter().map(|s| s.as_str()).collect();
            group_cols.extend(cols);
        }
        
        if let Some(time_col) = &group_by.time_column {
            group_cols.push(time_col.as_str());
        }
        
        query.push_str(&group_cols.join(", "));
    }
    
    query
} 