//! High-precision latency tracking using HDR histogram

use hdrhistogram::Histogram;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Latency tracker with HDR histogram
pub struct LatencyTracker {
    histogram: Arc<Mutex<Histogram<u64>>>,
    name: String,
}

impl LatencyTracker {
    /// Create a new latency tracker
    pub fn new(name: impl Into<String>) -> Self {
        // 3 significant digits, max value 1 hour in microseconds
        let histogram = Histogram::<u64>::new(3)
            .expect("Failed to create histogram");

        Self {
            histogram: Arc::new(Mutex::new(histogram)),
            name: name.into(),
        }
    }

    /// Record a latency measurement
    pub fn record(&self, duration: Duration) {
        let micros = duration.as_micros() as u64;
        if let Some(mut hist) = self.histogram.try_lock() {
            let _ = hist.record(micros);
        }
    }

    /// Get latency statistics
    pub fn stats(&self) -> LatencyStats {
        let hist = self.histogram.lock();

        LatencyStats {
            name: self.name.clone(),
            count: hist.len(),
            min: hist.min(),
            max: hist.max(),
            mean: hist.mean(),
            p50: hist.value_at_quantile(0.50),
            p90: hist.value_at_quantile(0.90),
            p99: hist.value_at_quantile(0.99),
            p999: hist.value_at_quantile(0.999),
        }
    }

    /// Reset the histogram
    pub fn reset(&self) {
        self.histogram.lock().reset();
    }

    /// Create a measurement guard
    pub fn measure(&self) -> LatencyMeasurement {
        LatencyMeasurement {
            tracker: self.clone(),
            start: Instant::now(),
        }
    }
}

impl Clone for LatencyTracker {
    fn clone(&self) -> Self {
        Self {
            histogram: self.histogram.clone(),
            name: self.name.clone(),
        }
    }
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub name: String,
    pub count: u64,
    pub min: u64,
    pub max: u64,
    pub mean: f64,
    pub p50: u64,
    pub p90: u64,
    pub p99: u64,
    pub p999: u64,
}

impl std::fmt::Display for LatencyStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: count={}, min={}µs, max={}µs, mean={:.2}µs, p50={}µs, p90={}µs, p99={}µs, p99.9={}µs",
            self.name, self.count, self.min, self.max, self.mean, self.p50, self.p90, self.p99, self.p999
        )
    }
}

/// RAII guard for automatic latency measurement
pub struct LatencyMeasurement {
    tracker: LatencyTracker,
    start: Instant,
}

impl Drop for LatencyMeasurement {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.tracker.record(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_tracker() {
        let tracker = LatencyTracker::new("test");

        // Record some measurements
        tracker.record(Duration::from_micros(100));
        tracker.record(Duration::from_micros(200));
        tracker.record(Duration::from_micros(300));

        let stats = tracker.stats();
        assert_eq!(stats.count, 3);
        assert!(stats.min >= 100);
        assert!(stats.max <= 300);
        assert!(stats.mean > 0.0);
    }

    #[test]
    fn test_latency_measurement() {
        let tracker = LatencyTracker::new("measurement");

        {
            let _measurement = tracker.measure();
            std::thread::sleep(Duration::from_micros(100));
        }

        let stats = tracker.stats();
        assert_eq!(stats.count, 1);
        assert!(stats.min >= 100);
    }
}
