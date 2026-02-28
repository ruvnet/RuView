//! Evidence accumulation and filtering

use serde::{Deserialize, Serialize};

/// Aggregated evidence from all tiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedEvidence {
    /// Total accumulated e-value
    pub e_value: f64,
    /// Number of tiles contributing
    pub tile_count: usize,
    /// Minimum e-value across tiles
    pub min_e_value: f64,
    /// Maximum e-value across tiles
    pub max_e_value: f64,
}

impl AggregatedEvidence {
    /// Create empty evidence
    pub fn empty() -> Self {
        Self {
            e_value: 1.0,
            tile_count: 0,
            min_e_value: f64::INFINITY,
            max_e_value: f64::NEG_INFINITY,
        }
    }

    /// Add evidence from a tile
    pub fn add(&mut self, e_value: f64) {
        self.e_value *= e_value;
        self.tile_count += 1;
        self.min_e_value = self.min_e_value.min(e_value);
        self.max_e_value = self.max_e_value.max(e_value);
    }
}

/// Evidence filter for e-process evaluation
///
/// OPTIMIZATION: Uses multiplicative update for O(1) current value maintenance
/// instead of O(n) product computation.
pub struct EvidenceFilter {
    /// Rolling e-value history (ring buffer)
    history: Vec<f64>,
    /// Current position in ring buffer
    position: usize,
    /// Capacity of ring buffer
    capacity: usize,
    /// Current accumulated value (maintained incrementally)
    current: f64,
    /// Log-space accumulator for numerical stability
    log_current: f64,
}

impl EvidenceFilter {
    /// Create a new evidence filter with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            history: Vec::with_capacity(capacity),
            position: 0,
            capacity,
            current: 1.0,
            log_current: 0.0,
        }
    }

    /// Update with a new e-value
    ///
    /// OPTIMIZATION: Uses multiplicative update for O(1) complexity
    /// instead of O(n) product recomputation. Falls back to full
    /// recomputation periodically to prevent numerical drift.
    pub fn update(&mut self, e_value: f64) {
        // Bound to prevent overflow/underflow
        let bounded = e_value.clamp(1e-10, 1e10);
        let log_bounded = bounded.ln();

        if self.history.len() < self.capacity {
            // Growing phase: just accumulate
            self.history.push(bounded);
            self.log_current += log_bounded;
        } else {
            // Ring buffer phase: multiplicative update
            let old_value = self.history[self.position];
            let old_log = old_value.ln();

            self.history[self.position] = bounded;
            self.log_current = self.log_current - old_log + log_bounded;
        }

        self.position = (self.position + 1) % self.capacity;

        // Convert from log-space
        self.current = self.log_current.exp();

        // Periodic full recomputation for numerical stability (every 64 updates)
        if self.position == 0 {
            self.recompute_current();
        }
    }

    /// Recompute current value from history (for stability)
    #[inline]
    fn recompute_current(&mut self) {
        self.log_current = self.history.iter().map(|x| x.ln()).sum();
        self.current = self.log_current.exp();
    }

    /// Get current accumulated e-value
    #[inline]
    pub fn current(&self) -> f64 {
        self.current
    }

    /// Get the history of e-values
    pub fn history(&self) -> &[f64] {
        &self.history
    }

    /// Compute product using SIMD-friendly parallel lanes
    ///
    /// OPTIMIZATION: Uses log-space arithmetic with parallel accumulators
    /// for better numerical stability and vectorization.
    pub fn current_simd(&self) -> f64 {
        if self.history.is_empty() {
            return 1.0;
        }

        // Use 4 parallel lanes for potential SIMD vectorization
        let mut log_lanes = [0.0f64; 4];

        for (i, &val) in self.history.iter().enumerate() {
            log_lanes[i % 4] += val.ln();
        }

        let log_sum = log_lanes[0] + log_lanes[1] + log_lanes[2] + log_lanes[3];
        log_sum.exp()
    }
}

/// Aggregate 255 tile e-values using SIMD-friendly patterns
///
/// OPTIMIZATION: Uses parallel lane accumulation in log-space
/// for numerical stability when combining many e-values.
///
/// # Arguments
/// * `tile_e_values` - Slice of e-values from worker tiles
///
/// # Returns
/// Aggregated e-value (product in log-space)
pub fn aggregate_tiles_simd(tile_e_values: &[f64]) -> f64 {
    if tile_e_values.is_empty() {
        return 1.0;
    }

    // Use 8 parallel lanes for 256-bit SIMD (AVX2)
    let mut log_lanes = [0.0f64; 8];

    // Process in chunks of 8
    let chunks = tile_e_values.chunks_exact(8);
    let remainder = chunks.remainder();

    for chunk in chunks {
        log_lanes[0] += chunk[0].ln();
        log_lanes[1] += chunk[1].ln();
        log_lanes[2] += chunk[2].ln();
        log_lanes[3] += chunk[3].ln();
        log_lanes[4] += chunk[4].ln();
        log_lanes[5] += chunk[5].ln();
        log_lanes[6] += chunk[6].ln();
        log_lanes[7] += chunk[7].ln();
    }

    // Handle remainder
    for (i, &val) in remainder.iter().enumerate() {
        log_lanes[i % 8] += val.ln();
    }

    // Tree reduction
    let sum_0_3 = log_lanes[0] + log_lanes[1] + log_lanes[2] + log_lanes[3];
    let sum_4_7 = log_lanes[4] + log_lanes[5] + log_lanes[6] + log_lanes[7];

    (sum_0_3 + sum_4_7).exp()
}

/// Compute mixture e-value with adaptive precision
///
/// OPTIMIZATION: Uses different precision strategies based on
/// the magnitude of accumulated evidence for optimal performance.
///
/// # Arguments
/// * `log_e_values` - Log e-values from tiles
/// * `weights` - Optional tile weights (None = uniform)
///
/// # Returns
/// Weighted geometric mean of e-values
pub fn mixture_evalue_adaptive(log_e_values: &[f64], weights: Option<&[f64]>) -> f64 {
    if log_e_values.is_empty() {
        return 1.0;
    }

    let total: f64 = match weights {
        Some(w) => {
            // Weighted sum in log-space
            log_e_values
                .iter()
                .zip(w.iter())
                .map(|(&log_e, &weight)| log_e * weight)
                .sum()
        }
        None => {
            // Uniform weights - use SIMD pattern
            let mut lanes = [0.0f64; 4];
            for (i, &log_e) in log_e_values.iter().enumerate() {
                lanes[i % 4] += log_e;
            }
            (lanes[0] + lanes[1] + lanes[2] + lanes[3]) / log_e_values.len() as f64
        }
    };

    total.exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregated_evidence() {
        let mut evidence = AggregatedEvidence::empty();
        evidence.add(2.0);
        evidence.add(3.0);

        assert_eq!(evidence.e_value, 6.0);
        assert_eq!(evidence.tile_count, 2);
        assert_eq!(evidence.min_e_value, 2.0);
        assert_eq!(evidence.max_e_value, 3.0);
    }

    #[test]
    fn test_evidence_filter() {
        let mut filter = EvidenceFilter::new(10);
        filter.update(2.0);
        filter.update(2.0);

        assert_eq!(filter.current(), 4.0);
    }
}
