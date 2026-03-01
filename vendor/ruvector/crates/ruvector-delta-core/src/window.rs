//! Delta window for time-bounded aggregation
//!
//! Provides sliding and tumbling windows for aggregating deltas
//! over time or count-based boundaries.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::delta::{Delta, VectorDelta};
use crate::error::{DeltaError, Result};

/// Configuration for delta windows
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window type
    pub window_type: WindowType,
    /// Window size (interpretation depends on type)
    pub size: usize,
    /// Slide amount for sliding windows
    pub slide: usize,
    /// Maximum items to keep
    pub max_items: usize,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_type: WindowType::Tumbling,
            size: 100,
            slide: 1,
            max_items: 10_000,
        }
    }
}

/// Window types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    /// Tumbling window (non-overlapping)
    Tumbling,
    /// Sliding window (overlapping)
    Sliding,
    /// Session window (gap-based)
    Session,
    /// Count-based window
    Count,
}

/// Entry in the window
#[derive(Debug, Clone)]
struct WindowEntry<D> {
    delta: D,
    timestamp_ns: u64,
}

/// Aggregated window result
#[derive(Debug, Clone)]
pub struct WindowResult<D: Clone> {
    /// Composed delta for this window
    pub delta: D,
    /// Start timestamp (ns)
    pub start_ns: u64,
    /// End timestamp (ns)
    pub end_ns: u64,
    /// Number of deltas in window
    pub count: usize,
}

/// A delta window for time-bounded aggregation
#[derive(Debug)]
pub struct DeltaWindow<D: Delta> {
    config: WindowConfig,
    entries: VecDeque<WindowEntry<D>>,
    /// For tumbling/sliding: window boundaries
    window_start_ns: u64,
}

impl<D: Delta + Clone> DeltaWindow<D>
where
    D::Base: Clone,
{
    /// Create a new delta window
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            entries: VecDeque::new(),
            window_start_ns: 0,
        }
    }

    /// Create a tumbling window of the given size (in nanoseconds)
    pub fn tumbling(size_ns: u64) -> Self {
        Self::new(WindowConfig {
            window_type: WindowType::Tumbling,
            size: size_ns as usize,
            slide: size_ns as usize,
            max_items: 10_000,
        })
    }

    /// Create a sliding window
    pub fn sliding(size_ns: u64, slide_ns: u64) -> Self {
        Self::new(WindowConfig {
            window_type: WindowType::Sliding,
            size: size_ns as usize,
            slide: slide_ns as usize,
            max_items: 10_000,
        })
    }

    /// Create a count-based window
    pub fn count_based(count: usize) -> Self {
        Self::new(WindowConfig {
            window_type: WindowType::Count,
            size: count,
            slide: count,
            max_items: count * 2,
        })
    }

    /// Add a delta to the window
    pub fn add(&mut self, delta: D, timestamp_ns: u64) {
        // Initialize window start if first entry
        if self.entries.is_empty() {
            self.window_start_ns = timestamp_ns;
        }

        self.entries.push_back(WindowEntry {
            delta,
            timestamp_ns,
        });

        // Enforce max items
        while self.entries.len() > self.config.max_items {
            self.entries.pop_front();
        }
    }

    /// Check if the current window is complete
    pub fn is_complete(&self, current_ns: u64) -> bool {
        match self.config.window_type {
            WindowType::Tumbling | WindowType::Sliding => {
                current_ns >= self.window_start_ns + self.config.size as u64
            }
            WindowType::Count => self.entries.len() >= self.config.size,
            WindowType::Session => {
                // Session window closes after a gap
                if let Some(last) = self.entries.back() {
                    current_ns - last.timestamp_ns > self.config.size as u64
                } else {
                    false
                }
            }
        }
    }

    /// Emit the current window and advance
    pub fn emit(&mut self) -> Option<WindowResult<D>>
    where
        D: Default,
    {
        if self.entries.is_empty() {
            return None;
        }

        match self.config.window_type {
            WindowType::Tumbling => self.emit_tumbling(),
            WindowType::Sliding => self.emit_sliding(),
            WindowType::Count => self.emit_count(),
            WindowType::Session => self.emit_session(),
        }
    }

    fn emit_tumbling(&mut self) -> Option<WindowResult<D>>
    where
        D: Default,
    {
        let window_end = self.window_start_ns + self.config.size as u64;

        // Collect entries in window
        let in_window: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.timestamp_ns < window_end)
            .cloned()
            .collect();

        if in_window.is_empty() {
            return None;
        }

        // Compose all deltas
        let result = self.compose_entries(&in_window);

        // Remove processed entries
        self.entries.retain(|e| e.timestamp_ns >= window_end);

        // Advance window
        self.window_start_ns = window_end;

        Some(result)
    }

    fn emit_sliding(&mut self) -> Option<WindowResult<D>>
    where
        D: Default,
    {
        let window_end = self.window_start_ns + self.config.size as u64;

        // Collect entries in window
        let in_window: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.timestamp_ns >= self.window_start_ns && e.timestamp_ns < window_end)
            .cloned()
            .collect();

        if in_window.is_empty() {
            return None;
        }

        let result = self.compose_entries(&in_window);

        // Slide window
        let new_start = self.window_start_ns + self.config.slide as u64;

        // Remove entries before new window start
        self.entries.retain(|e| e.timestamp_ns >= new_start);

        self.window_start_ns = new_start;

        Some(result)
    }

    fn emit_count(&mut self) -> Option<WindowResult<D>>
    where
        D: Default,
    {
        if self.entries.len() < self.config.size {
            return None;
        }

        let window_entries: Vec<_> = self.entries.drain(..self.config.size).collect();

        Some(self.compose_entries(&window_entries))
    }

    fn emit_session(&mut self) -> Option<WindowResult<D>>
    where
        D: Default,
    {
        if self.entries.is_empty() {
            return None;
        }

        let all_entries: Vec<_> = self.entries.drain(..).collect();
        Some(self.compose_entries(&all_entries))
    }

    fn compose_entries(&self, entries: &[WindowEntry<D>]) -> WindowResult<D>
    where
        D: Default,
    {
        let start_ns = entries.first().map(|e| e.timestamp_ns).unwrap_or(0);
        let end_ns = entries.last().map(|e| e.timestamp_ns).unwrap_or(0);
        let count = entries.len();

        let delta = if entries.is_empty() {
            D::default()
        } else {
            let mut composed = entries[0].delta.clone();
            for entry in entries.iter().skip(1) {
                composed = composed.compose(entry.delta.clone());
            }
            composed
        };

        WindowResult {
            delta,
            start_ns,
            end_ns,
            count,
        }
    }

    /// Get the number of entries in the window
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the window is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for VectorDelta {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Trait for aggregating window results
pub trait WindowAggregator<D: Delta>: Send + Sync {
    /// Aggregate multiple window results
    fn aggregate(&self, results: &[WindowResult<D>]) -> WindowResult<D>;

    /// Get aggregation type name
    fn name(&self) -> &'static str;
}

/// Sum aggregator - composes all deltas
pub struct SumAggregator;

impl<D: Delta + Clone + Default> WindowAggregator<D> for SumAggregator {
    fn aggregate(&self, results: &[WindowResult<D>]) -> WindowResult<D> {
        if results.is_empty() {
            return WindowResult {
                delta: D::default(),
                start_ns: 0,
                end_ns: 0,
                count: 0,
            };
        }

        let start_ns = results.first().map(|r| r.start_ns).unwrap_or(0);
        let end_ns = results.last().map(|r| r.end_ns).unwrap_or(0);
        let count: usize = results.iter().map(|r| r.count).sum();

        let delta = if results.is_empty() {
            D::default()
        } else {
            let mut composed = results[0].delta.clone();
            for result in results.iter().skip(1) {
                composed = composed.compose(result.delta.clone());
            }
            composed
        };

        WindowResult {
            delta,
            start_ns,
            end_ns,
            count,
        }
    }

    fn name(&self) -> &'static str {
        "sum"
    }
}

/// Average aggregator - scales composed delta by 1/count
pub struct AverageAggregator;

impl WindowAggregator<VectorDelta> for AverageAggregator {
    fn aggregate(&self, results: &[WindowResult<VectorDelta>]) -> WindowResult<VectorDelta> {
        if results.is_empty() {
            return WindowResult {
                delta: VectorDelta::default(),
                start_ns: 0,
                end_ns: 0,
                count: 0,
            };
        }

        let sum_result = SumAggregator.aggregate(results);
        let count = sum_result.count.max(1) as f32;

        WindowResult {
            delta: sum_result.delta.scale(1.0 / count),
            start_ns: sum_result.start_ns,
            end_ns: sum_result.end_ns,
            count: sum_result.count,
        }
    }

    fn name(&self) -> &'static str {
        "average"
    }
}

/// Exponential moving average aggregator
pub struct EmaAggregator {
    /// Smoothing factor (0 < alpha <= 1)
    pub alpha: f32,
}

impl EmaAggregator {
    /// Create with smoothing factor
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
        }
    }
}

impl WindowAggregator<VectorDelta> for EmaAggregator {
    fn aggregate(&self, results: &[WindowResult<VectorDelta>]) -> WindowResult<VectorDelta> {
        if results.is_empty() {
            return WindowResult {
                delta: VectorDelta::default(),
                start_ns: 0,
                end_ns: 0,
                count: 0,
            };
        }

        let start_ns = results.first().map(|r| r.start_ns).unwrap_or(0);
        let end_ns = results.last().map(|r| r.end_ns).unwrap_or(0);
        let count: usize = results.iter().map(|r| r.count).sum();

        // EMA: new_ema = alpha * current + (1 - alpha) * old_ema
        let mut ema = results[0].delta.clone();
        for result in results.iter().skip(1) {
            let scaled_current = result.delta.scale(self.alpha);
            let scaled_ema = ema.scale(1.0 - self.alpha);
            ema = scaled_current.compose(scaled_ema);
        }

        WindowResult {
            delta: ema,
            start_ns,
            end_ns,
            count,
        }
    }

    fn name(&self) -> &'static str {
        "ema"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tumbling_window() {
        let mut window = DeltaWindow::<VectorDelta>::tumbling(1_000_000); // 1ms

        // Add deltas at different times
        window.add(VectorDelta::from_dense(vec![1.0, 0.0, 0.0]), 0);
        window.add(VectorDelta::from_dense(vec![0.0, 1.0, 0.0]), 500_000);

        // Window not complete yet
        assert!(!window.is_complete(900_000));

        // Window complete
        assert!(window.is_complete(1_000_000));

        // Emit
        let result = window.emit().unwrap();
        assert_eq!(result.count, 2);
    }

    #[test]
    fn test_count_window() {
        let mut window = DeltaWindow::<VectorDelta>::count_based(3);

        window.add(VectorDelta::from_dense(vec![1.0]), 0);
        window.add(VectorDelta::from_dense(vec![1.0]), 1);

        assert!(window.emit().is_none()); // Not enough

        window.add(VectorDelta::from_dense(vec![1.0]), 2);

        let result = window.emit().unwrap();
        assert_eq!(result.count, 3);
    }

    #[test]
    fn test_sliding_window() {
        let mut window = DeltaWindow::<VectorDelta>::sliding(1_000_000, 500_000);

        window.add(VectorDelta::from_dense(vec![1.0]), 0);
        window.add(VectorDelta::from_dense(vec![2.0]), 250_000);
        window.add(VectorDelta::from_dense(vec![3.0]), 750_000);

        // Complete after 1ms
        assert!(window.is_complete(1_000_000));
    }

    #[test]
    fn test_sum_aggregator() {
        let results = vec![
            WindowResult {
                delta: VectorDelta::from_dense(vec![1.0, 0.0]),
                start_ns: 0,
                end_ns: 100,
                count: 1,
            },
            WindowResult {
                delta: VectorDelta::from_dense(vec![0.0, 1.0]),
                start_ns: 100,
                end_ns: 200,
                count: 1,
            },
        ];

        let aggregated = SumAggregator.aggregate(&results);
        assert_eq!(aggregated.count, 2);
    }
}
