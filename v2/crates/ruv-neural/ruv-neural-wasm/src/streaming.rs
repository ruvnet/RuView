//! WebSocket streaming support for real-time neural data processing.
//!
//! Provides a `StreamProcessor` that accumulates incoming neural samples,
//! applies a sliding window, and emits updated topology metrics whenever
//! a complete window is available.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Streaming neural data processor with a sliding window.
///
/// Accumulates incoming samples and produces topology metric updates
/// whenever enough data fills a window. Designed for use with WebSocket
/// connections in the browser.
#[wasm_bindgen]
pub struct StreamProcessor {
    /// Internal sample buffer.
    buffer: Vec<f64>,
    /// Number of samples in a complete analysis window.
    window_size: usize,
    /// Number of samples to advance between windows (hop size).
    step_size: usize,
    /// Number of windows emitted so far.
    windows_emitted: u64,
}

/// Summary statistics for a single window of streaming data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowStats {
    /// Mean value of samples in the window.
    pub mean: f64,
    /// Variance of samples in the window.
    pub variance: f64,
    /// Minimum sample value.
    pub min: f64,
    /// Maximum sample value.
    pub max: f64,
    /// Number of samples in the window.
    pub window_size: usize,
    /// Sequential window index.
    pub window_index: u64,
}

#[wasm_bindgen]
impl StreamProcessor {
    /// Create a new `StreamProcessor`.
    ///
    /// # Arguments
    /// * `window_size` - Number of samples in each analysis window.
    /// * `step_size` - Number of samples to advance between windows (hop size).
    #[wasm_bindgen(constructor)]
    pub fn new(window_size: usize, step_size: usize) -> Self {
        let step_size = if step_size == 0 { 1 } else { step_size };
        Self {
            buffer: Vec::with_capacity(window_size),
            window_size,
            step_size,
            windows_emitted: 0,
        }
    }

    /// Push new samples into the buffer and return window statistics
    /// if a complete window is available.
    ///
    /// Returns `null` if not enough samples have accumulated yet.
    /// When a window is complete, computes statistics and advances
    /// the buffer by `step_size` samples.
    pub fn push_samples(&mut self, samples: &[f64]) -> Option<JsValue> {
        let stats = self.push_samples_native(samples)?;
        serde_wasm_bindgen::to_value(&stats).ok()
    }

    /// Reset the internal buffer and window counter.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.windows_emitted = 0;
    }

    /// Get the current number of buffered samples.
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Get the number of windows emitted so far.
    pub fn windows_emitted(&self) -> u64 {
        self.windows_emitted
    }

    /// Get the configured window size.
    pub fn window_size(&self) -> usize {
        self.window_size
    }

    /// Get the configured step size.
    pub fn step_size(&self) -> usize {
        self.step_size
    }
}

impl StreamProcessor {
    /// Push samples and return native `WindowStats` (usable without WASM runtime).
    pub fn push_samples_native(&mut self, samples: &[f64]) -> Option<WindowStats> {
        self.buffer.extend_from_slice(samples);

        if self.buffer.len() >= self.window_size {
            let window = &self.buffer[..self.window_size];
            let stats = compute_window_stats(window, self.windows_emitted);
            self.windows_emitted += 1;

            // Advance buffer by step_size.
            let drain_count = self.step_size.min(self.buffer.len());
            self.buffer.drain(..drain_count);

            Some(stats)
        } else {
            None
        }
    }
}

/// Compute basic statistics over a sample window.
fn compute_window_stats(window: &[f64], window_index: u64) -> WindowStats {
    let n = window.len() as f64;
    let sum: f64 = window.iter().sum();
    let mean = sum / n;

    let variance = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;

    let min = window
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let max = window
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    WindowStats {
        mean,
        variance,
        min,
        max,
        window_size: window.len(),
        window_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_processor_accumulates() {
        let mut proc = StreamProcessor::new(10, 5);
        assert_eq!(proc.buffered_count(), 0);

        // Push 5 samples (not enough for a window).
        let result = proc.push_samples_native(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(result.is_none());
        assert_eq!(proc.buffered_count(), 5);
    }

    #[test]
    fn test_stream_processor_emits_on_full_window() {
        let mut proc = StreamProcessor::new(4, 2);

        // Push exactly 4 samples.
        let result = proc.push_samples_native(&[1.0, 2.0, 3.0, 4.0]);
        assert!(result.is_some());
        let stats = result.unwrap();
        assert!((stats.mean - 2.5).abs() < 1e-10);
        assert_eq!(proc.windows_emitted(), 1);
        // After step of 2, buffer should have 2 remaining.
        assert_eq!(proc.buffered_count(), 2);
    }

    #[test]
    fn test_stream_processor_reset() {
        let mut proc = StreamProcessor::new(4, 2);
        proc.push_samples_native(&[1.0, 2.0, 3.0, 4.0]);
        proc.reset();
        assert_eq!(proc.buffered_count(), 0);
        assert_eq!(proc.windows_emitted(), 0);
    }

    #[test]
    fn test_window_stats_computation() {
        let window = [2.0, 4.0, 6.0, 8.0];
        let stats = compute_window_stats(&window, 0);
        assert!((stats.mean - 5.0).abs() < 1e-10);
        assert!((stats.variance - 5.0).abs() < 1e-10);
        assert!((stats.min - 2.0).abs() < 1e-10);
        assert!((stats.max - 8.0).abs() < 1e-10);
        assert_eq!(stats.window_size, 4);
    }

    #[test]
    fn test_stream_processor_zero_step_defaults_to_one() {
        let proc = StreamProcessor::new(4, 0);
        assert_eq!(proc.step_size(), 1);
    }

    #[test]
    fn test_multiple_windows() {
        let mut proc = StreamProcessor::new(3, 1);

        // Push 5 samples: should emit window at sample 3.
        let result = proc.push_samples_native(&[1.0, 2.0, 3.0]);
        assert!(result.is_some());
        assert_eq!(proc.windows_emitted(), 1);

        // Push 1 more: buffer should be [2,3,X], then with new sample [2,3,4].
        let result = proc.push_samples_native(&[4.0]);
        assert!(result.is_some());
        assert_eq!(proc.windows_emitted(), 2);
    }
}
