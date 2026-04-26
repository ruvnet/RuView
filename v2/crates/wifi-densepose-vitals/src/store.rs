//! Vital sign time series store.
//!
//! Stores vital sign readings with configurable retention.
//! Designed for upgrade to `TieredStore` when `ruvector-temporal-tensor`
//! becomes available (ADR-021 phase 2).

use crate::types::{VitalReading, VitalStatus};

/// Simple vital sign store with capacity-limited ring buffer semantics.
pub struct VitalSignStore {
    /// Stored readings (oldest first).
    readings: Vec<VitalReading>,
    /// Maximum number of readings to retain.
    max_readings: usize,
}

/// Summary statistics for stored vital sign readings.
#[derive(Debug, Clone)]
pub struct VitalStats {
    /// Number of readings in the store.
    pub count: usize,
    /// Mean respiratory rate (BPM).
    pub rr_mean: f64,
    /// Mean heart rate (BPM).
    pub hr_mean: f64,
    /// Min respiratory rate (BPM).
    pub rr_min: f64,
    /// Max respiratory rate (BPM).
    pub rr_max: f64,
    /// Min heart rate (BPM).
    pub hr_min: f64,
    /// Max heart rate (BPM).
    pub hr_max: f64,
    /// Fraction of readings with Valid status.
    pub valid_fraction: f64,
}

impl VitalSignStore {
    /// Create a new store with a given maximum capacity.
    ///
    /// When the capacity is exceeded, the oldest readings are evicted.
    #[must_use]
    pub fn new(max_readings: usize) -> Self {
        Self {
            readings: Vec::with_capacity(max_readings.min(4096)),
            max_readings: max_readings.max(1),
        }
    }

    /// Create with default capacity (3600 readings ~ 1 hour at 1 Hz).
    #[must_use]
    pub fn default_capacity() -> Self {
        Self::new(3600)
    }

    /// Push a new reading into the store.
    ///
    /// If the store is at capacity, the oldest reading is evicted.
    pub fn push(&mut self, reading: VitalReading) {
        if self.readings.len() >= self.max_readings {
            self.readings.remove(0);
        }
        self.readings.push(reading);
    }

    /// Get the most recent reading, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&VitalReading> {
        self.readings.last()
    }

    /// Get the last `n` readings (most recent last).
    ///
    /// Returns fewer than `n` if the store contains fewer readings.
    #[must_use]
    pub fn history(&self, n: usize) -> &[VitalReading] {
        let start = self.readings.len().saturating_sub(n);
        &self.readings[start..]
    }

    /// Compute summary statistics over all stored readings.
    ///
    /// Returns `None` if the store is empty.
    #[must_use]
    pub fn stats(&self) -> Option<VitalStats> {
        if self.readings.is_empty() {
            return None;
        }

        let n = self.readings.len() as f64;
        let mut rr_sum = 0.0;
        let mut hr_sum = 0.0;
        let mut rr_min = f64::MAX;
        let mut rr_max = f64::MIN;
        let mut hr_min = f64::MAX;
        let mut hr_max = f64::MIN;
        let mut valid_count = 0_usize;

        for r in &self.readings {
            let rr = r.respiratory_rate.value_bpm;
            let hr = r.heart_rate.value_bpm;
            rr_sum += rr;
            hr_sum += hr;
            rr_min = rr_min.min(rr);
            rr_max = rr_max.max(rr);
            hr_min = hr_min.min(hr);
            hr_max = hr_max.max(hr);

            if r.respiratory_rate.status == VitalStatus::Valid
                && r.heart_rate.status == VitalStatus::Valid
            {
                valid_count += 1;
            }
        }

        Some(VitalStats {
            count: self.readings.len(),
            rr_mean: rr_sum / n,
            hr_mean: hr_sum / n,
            rr_min,
            rr_max,
            hr_min,
            hr_max,
            valid_fraction: valid_count as f64 / n,
        })
    }

    /// Number of readings currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.readings.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }

    /// Maximum capacity of the store.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max_readings
    }

    /// Clear all stored readings.
    pub fn clear(&mut self) {
        self.readings.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{VitalEstimate, VitalReading, VitalStatus};

    fn make_reading(rr: f64, hr: f64) -> VitalReading {
        VitalReading {
            respiratory_rate: VitalEstimate {
                value_bpm: rr,
                confidence: 0.9,
                status: VitalStatus::Valid,
            },
            heart_rate: VitalEstimate {
                value_bpm: hr,
                confidence: 0.85,
                status: VitalStatus::Valid,
            },
            subcarrier_count: 56,
            signal_quality: 0.9,
            timestamp_secs: 0.0,
        }
    }

    #[test]
    fn empty_store() {
        let store = VitalSignStore::new(10);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.latest().is_none());
        assert!(store.stats().is_none());
    }

    #[test]
    fn push_and_retrieve() {
        let mut store = VitalSignStore::new(10);
        store.push(make_reading(15.0, 72.0));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        let latest = store.latest().unwrap();
        assert!((latest.respiratory_rate.value_bpm - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn eviction_at_capacity() {
        let mut store = VitalSignStore::new(3);
        store.push(make_reading(10.0, 60.0));
        store.push(make_reading(15.0, 72.0));
        store.push(make_reading(20.0, 80.0));
        assert_eq!(store.len(), 3);

        // Push one more; oldest should be evicted
        store.push(make_reading(25.0, 90.0));
        assert_eq!(store.len(), 3);

        // Oldest should now be 15.0, not 10.0
        let oldest = &store.history(10)[0];
        assert!((oldest.respiratory_rate.value_bpm - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn history_returns_last_n() {
        let mut store = VitalSignStore::new(10);
        for i in 0..5 {
            store.push(make_reading(10.0 + i as f64, 60.0 + i as f64));
        }

        let last3 = store.history(3);
        assert_eq!(last3.len(), 3);
        assert!((last3[0].respiratory_rate.value_bpm - 12.0).abs() < f64::EPSILON);
        assert!((last3[2].respiratory_rate.value_bpm - 14.0).abs() < f64::EPSILON);
    }

    #[test]
    fn history_when_fewer_than_n() {
        let mut store = VitalSignStore::new(10);
        store.push(make_reading(15.0, 72.0));
        let all = store.history(100);
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn stats_computation() {
        let mut store = VitalSignStore::new(10);
        store.push(make_reading(10.0, 60.0));
        store.push(make_reading(20.0, 80.0));
        store.push(make_reading(15.0, 70.0));

        let stats = store.stats().unwrap();
        assert_eq!(stats.count, 3);
        assert!((stats.rr_mean - 15.0).abs() < f64::EPSILON);
        assert!((stats.hr_mean - 70.0).abs() < f64::EPSILON);
        assert!((stats.rr_min - 10.0).abs() < f64::EPSILON);
        assert!((stats.rr_max - 20.0).abs() < f64::EPSILON);
        assert!((stats.hr_min - 60.0).abs() < f64::EPSILON);
        assert!((stats.hr_max - 80.0).abs() < f64::EPSILON);
        assert!((stats.valid_fraction - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_valid_fraction() {
        let mut store = VitalSignStore::new(10);
        store.push(make_reading(15.0, 72.0)); // Valid
        store.push(VitalReading {
            respiratory_rate: VitalEstimate {
                value_bpm: 15.0,
                confidence: 0.3,
                status: VitalStatus::Degraded,
            },
            heart_rate: VitalEstimate {
                value_bpm: 72.0,
                confidence: 0.8,
                status: VitalStatus::Valid,
            },
            subcarrier_count: 56,
            signal_quality: 0.5,
            timestamp_secs: 1.0,
        });

        let stats = store.stats().unwrap();
        assert!((stats.valid_fraction - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn clear_empties_store() {
        let mut store = VitalSignStore::new(10);
        store.push(make_reading(15.0, 72.0));
        store.push(make_reading(16.0, 73.0));
        assert_eq!(store.len(), 2);
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn default_capacity_is_3600() {
        let store = VitalSignStore::default_capacity();
        assert_eq!(store.capacity(), 3600);
    }
}
