//! Streaming P95 adaptive feature normalizer (ADR-044 §5.2).
//!
//! Self-calibrates feature normalization to whatever distribution the deployment
//! produces — no hardcoded scale values that can saturate in large rooms or
//! degrade in high-interference environments.

use std::collections::VecDeque;

/// Streaming P95 estimator over a fixed-size sliding window.
#[derive(Debug, Clone)]
pub struct RollingP95 {
    buf: VecDeque<f64>,
    window: usize,
    min_samples: usize,
}

impl RollingP95 {
    pub fn new(window: usize, min_samples: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(window),
            window,
            min_samples,
        }
    }

    pub fn push(&mut self, v: f64) {
        if self.buf.len() == self.window {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }

    /// Returns `Some(p95)` once enough samples have accumulated, else `None`.
    pub fn current(&self) -> Option<f64> {
        if self.buf.len() < self.min_samples {
            return None;
        }
        let mut sorted: Vec<f64> = self.buf.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
        Some(sorted[idx.saturating_sub(1).min(sorted.len() - 1)])
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_start_returns_none() {
        let p = RollingP95::new(100, 10);
        assert!(p.current().is_none(), "empty buffer must return None");
    }

    #[test]
    fn below_min_samples_returns_none() {
        let mut p = RollingP95::new(100, 10);
        for i in 1..=9 {
            p.push(i as f64);
        }
        assert!(
            p.current().is_none(),
            "fewer than min_samples must return None"
        );
    }

    #[test]
    fn p95_of_ramp_is_near_95() {
        let mut p = RollingP95::new(100, 10);
        for i in 1..=100 {
            p.push(i as f64);
        }
        let p95 = p.current().expect("should have value after 100 samples");
        assert!(
            (94.0..=96.0).contains(&p95),
            "P95 of 1..=100 should be ~95, got {p95}"
        );
    }

    #[test]
    fn window_slides_evicts_oldest() {
        let mut p = RollingP95::new(5, 3);
        for i in 1..=5 {
            p.push(i as f64);
        }
        p.push(100.0);
        let p95 = p.current().expect("6 pushes, window=5 -> 5 samples");
        assert_eq!(
            p95, 100.0,
            "largest value should dominate p95 after eviction"
        );
    }

    #[test]
    fn len_reports_buffer_size() {
        let mut p = RollingP95::new(10, 5);
        assert_eq!(p.len(), 0);
        p.push(1.0);
        assert_eq!(p.len(), 1);
    }
}

