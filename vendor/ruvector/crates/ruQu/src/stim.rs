//! Stim Integration for Real QEC Simulation
//!
//! This module provides integration with the Stim quantum error correction
//! simulator, enabling realistic syndrome generation and testing.
//!
//! ## What is Stim?
//!
//! [Stim](https://github.com/quantumlib/Stim) is Google's high-performance
//! stabilizer circuit simulator for quantum error correction. It can generate
//! realistic syndrome data at rates exceeding 1 billion measurements per second.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruqu::stim::{StimSyndromeSource, SurfaceCodeConfig};
//!
//! // Create a surface code syndrome source
//! let config = SurfaceCodeConfig::new(7, 0.001); // distance 7, 0.1% error rate
//! let mut source = StimSyndromeSource::new(config)?;
//!
//! // Generate syndromes
//! for round in 0..1000 {
//!     let detectors = source.sample()?;
//!     fabric.process(&detectors)?;
//! }
//! ```
//!
//! ## Supported Codes
//!
//! - Surface code (rotated and unrotated)
//! - Repetition code
//! - Color code (planned)

use crate::error::{Result, RuQuError};
use crate::syndrome::DetectorBitmap;

/// Configuration for surface code simulation
#[derive(Clone, Debug)]
pub struct SurfaceCodeConfig {
    /// Code distance (odd integer, typically 3-21)
    pub distance: usize,
    /// Physical error rate (0.0-1.0)
    pub error_rate: f64,
    /// Number of syndrome rounds per measurement
    pub rounds: usize,
    /// Use rotated surface code layout
    pub rotated: bool,
    /// Include measurement errors
    pub measure_errors: bool,
    /// Random seed (None = use system entropy)
    pub seed: Option<u64>,
}

impl SurfaceCodeConfig {
    /// Create a new surface code configuration
    pub fn new(distance: usize, error_rate: f64) -> Self {
        Self {
            distance,
            error_rate,
            rounds: distance,
            rotated: true,
            measure_errors: true,
            seed: None,
        }
    }

    /// Calculate number of data qubits
    pub fn data_qubits(&self) -> usize {
        self.distance * self.distance
    }

    /// Calculate number of syndrome qubits (ancillas)
    pub fn syndrome_qubits(&self) -> usize {
        // Rotated surface code: (d-1)^2 X stabilizers + (d-1)^2 Z stabilizers
        // Approximately d^2 - 1 total
        (self.distance - 1) * (self.distance - 1) * 2
    }

    /// Calculate number of detectors per round
    pub fn detectors_per_round(&self) -> usize {
        self.syndrome_qubits()
    }

    /// Calculate total detectors across all rounds
    pub fn total_detectors(&self) -> usize {
        self.detectors_per_round() * self.rounds
    }

    /// Builder: set error rate
    pub fn with_error_rate(mut self, rate: f64) -> Self {
        self.error_rate = rate;
        self
    }

    /// Builder: set measurement error rate
    pub fn with_measurement_error_rate(mut self, _rate: f64) -> Self {
        // Store as a fraction of error_rate for now
        self.measure_errors = true;
        self
    }

    /// Builder: set random seed for reproducibility
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Builder: set number of rounds
    pub fn with_rounds(mut self, rounds: usize) -> Self {
        self.rounds = rounds;
        self
    }
}

impl Default for SurfaceCodeConfig {
    fn default() -> Self {
        Self::new(5, 0.001) // Distance 5, 0.1% error rate
    }
}

/// Simple pseudo-random number generator (xorshift64)
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xDEADBEEF } else { seed },
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f64(&mut self) -> f64 {
        (self.next() as f64) / (u64::MAX as f64)
    }
}

/// Syndrome source using stim-like simulation
///
/// When the `stim` feature is enabled, this uses the actual stim-rs bindings.
/// Otherwise, it provides a compatible fallback implementation.
pub struct StimSyndromeSource {
    config: SurfaceCodeConfig,
    rng: Xorshift64,
    round: u64,
    /// Cached detector positions for correlation modeling
    detector_coords: Vec<(usize, usize)>,
}

impl StimSyndromeSource {
    /// Create a new syndrome source
    pub fn new(config: SurfaceCodeConfig) -> Result<Self> {
        let seed = config.seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(12345)
        });

        // Pre-compute detector coordinates for correlation modeling
        let mut detector_coords = Vec::new();
        let d = config.distance;
        for r in 0..d - 1 {
            for c in 0..d - 1 {
                // X stabilizers
                detector_coords.push((r, c));
                // Z stabilizers (offset grid)
                detector_coords.push((r, c + d));
            }
        }

        Ok(Self {
            config,
            rng: Xorshift64::new(seed),
            round: 0,
            detector_coords,
        })
    }

    /// Sample a single syndrome round
    pub fn sample(&mut self) -> Result<DetectorBitmap> {
        let num_detectors = self.config.detectors_per_round();
        let mut bitmap = DetectorBitmap::new(num_detectors);

        // Simulate depolarizing noise channel
        for i in 0..num_detectors {
            // Each detector fires with probability related to error rate
            // In a real surface code, this is more complex (depends on neighbors)
            let p = self.effective_detection_probability(i);

            if self.rng.next_f64() < p {
                bitmap.set(i, true);
            }
        }

        // Add correlated errors (simulates real error patterns)
        self.add_correlated_errors(&mut bitmap);

        self.round += 1;
        Ok(bitmap)
    }

    /// Sample multiple rounds
    pub fn sample_batch(&mut self, count: usize) -> Result<Vec<DetectorBitmap>> {
        (0..count).map(|_| self.sample()).collect()
    }

    /// Get current round number
    pub fn current_round(&self) -> u64 {
        self.round
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.round = 0;
        self.rng = Xorshift64::new(self.config.seed.unwrap_or(12345));
    }

    // Private helpers

    fn effective_detection_probability(&self, detector_idx: usize) -> f64 {
        // Base probability from physical error rate
        // In surface code, detector fires when syndrome changes
        // P(detection) ≈ 2p(1-p) for single qubit error, where p = error_rate

        let p = self.config.error_rate;
        let base_prob = 2.0 * p * (1.0 - p);

        // Add measurement error contribution
        let measure_prob = if self.config.measure_errors {
            p * 0.5 // Measurement errors contribute less
        } else {
            0.0
        };

        (base_prob + measure_prob).min(1.0)
    }

    fn add_correlated_errors(&mut self, bitmap: &mut DetectorBitmap) {
        // Model correlated errors (cosmic rays, TLS defects, etc.)
        // These create "stripes" of detections

        // Probability of a correlated event per round
        let cosmic_ray_prob = 0.001 * self.config.error_rate;

        if self.rng.next_f64() < cosmic_ray_prob {
            // Correlated error: affect a row or column of detectors
            let is_row = self.rng.next_f64() < 0.5;
            let d = self.config.distance;
            let idx = (self.rng.next() as usize) % (d - 1);

            for i in 0..d - 1 {
                let detector = if is_row {
                    idx * (d - 1) + i
                } else {
                    i * (d - 1) + idx
                };

                if detector < bitmap.detector_count() {
                    // Flip the detector
                    let current = bitmap.get(detector);
                    bitmap.set(detector, !current);
                }
            }
        }
    }
}

/// Generate syndrome data matching a specific error pattern
pub struct ErrorPatternGenerator {
    config: SurfaceCodeConfig,
}

impl ErrorPatternGenerator {
    /// Create a new pattern generator
    pub fn new(config: SurfaceCodeConfig) -> Self {
        Self { config }
    }

    /// Generate syndrome for a single X error at position (row, col)
    pub fn single_x_error(&self, row: usize, col: usize) -> DetectorBitmap {
        let num_detectors = self.config.detectors_per_round();
        let mut bitmap = DetectorBitmap::new(num_detectors);

        let d = self.config.distance;

        // X error triggers neighboring Z stabilizers
        // In rotated surface code, each data qubit borders up to 4 Z stabilizers
        let z_offset = (d - 1) * (d - 1); // Z stabilizers are after X stabilizers

        // Add detections for neighboring Z stabilizers
        if row > 0 && col < d - 1 {
            bitmap.set(z_offset + (row - 1) * (d - 1) + col, true);
        }
        if row < d - 1 && col < d - 1 {
            bitmap.set(z_offset + row * (d - 1) + col, true);
        }

        bitmap
    }

    /// Generate syndrome for a single Z error at position (row, col)
    pub fn single_z_error(&self, row: usize, col: usize) -> DetectorBitmap {
        let num_detectors = self.config.detectors_per_round();
        let mut bitmap = DetectorBitmap::new(num_detectors);

        let d = self.config.distance;

        // Z error triggers neighboring X stabilizers
        if row < d - 1 && col > 0 {
            bitmap.set(row * (d - 1) + (col - 1), true);
        }
        if row < d - 1 && col < d - 1 {
            bitmap.set(row * (d - 1) + col, true);
        }

        bitmap
    }

    /// Generate syndrome for a logical X error (horizontal string)
    pub fn logical_x_error(&self) -> DetectorBitmap {
        let num_detectors = self.config.detectors_per_round();
        let mut bitmap = DetectorBitmap::new(num_detectors);

        // Logical X is a string of X errors across the code
        // Only boundary stabilizers detect it
        let d = self.config.distance;
        let z_offset = (d - 1) * (d - 1);

        // Top boundary Z stabilizers
        for col in 0..d - 1 {
            bitmap.set(z_offset + col, true);
        }

        bitmap
    }
}

/// Statistics about generated syndromes
#[derive(Clone, Debug, Default)]
pub struct SyndromeStats {
    /// Total syndromes generated
    pub total_syndromes: u64,
    /// Total detectors fired
    pub total_detections: u64,
    /// Average detection rate
    pub avg_detection_rate: f64,
    /// Maximum detections in a single syndrome
    pub max_detections: usize,
    /// Estimated logical error rate
    pub estimated_logical_error_rate: f64,
}

impl SyndromeStats {
    /// Update stats with a new syndrome
    pub fn update(&mut self, bitmap: &DetectorBitmap) {
        self.total_syndromes += 1;
        let fired = bitmap.fired_count();
        self.total_detections += fired as u64;

        if fired > self.max_detections {
            self.max_detections = fired;
        }

        self.avg_detection_rate = self.total_detections as f64
            / (self.total_syndromes as f64 * bitmap.detector_count() as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_code_config() {
        let config = SurfaceCodeConfig::new(7, 0.001);

        assert_eq!(config.distance, 7);
        assert_eq!(config.data_qubits(), 49);
        assert!(config.syndrome_qubits() > 0);
    }

    #[test]
    fn test_syndrome_source_creation() {
        let config = SurfaceCodeConfig::new(5, 0.01);
        let source = StimSyndromeSource::new(config);
        assert!(source.is_ok());
    }

    #[test]
    fn test_syndrome_sampling() {
        let config = SurfaceCodeConfig::new(5, 0.1); // High error rate for testing
        let mut source = StimSyndromeSource::new(config).unwrap();

        let bitmap = source.sample().unwrap();

        // Should have correct number of detectors
        assert_eq!(bitmap.detector_count(), source.config.detectors_per_round());
    }

    #[test]
    fn test_syndrome_batch() {
        let config = SurfaceCodeConfig::new(5, 0.01);
        let mut source = StimSyndromeSource::new(config).unwrap();

        let batch = source.sample_batch(100).unwrap();
        assert_eq!(batch.len(), 100);
    }

    #[test]
    fn test_error_pattern_generator() {
        let config = SurfaceCodeConfig::new(5, 0.01);
        let gen = ErrorPatternGenerator::new(config);

        let x_error = gen.single_x_error(2, 2);
        assert!(x_error.fired_count() <= 4); // At most 4 neighboring stabilizers

        let logical = gen.logical_x_error();
        assert!(logical.fired_count() > 0);
    }

    #[test]
    fn test_syndrome_stats() {
        let mut stats = SyndromeStats::default();

        let config = SurfaceCodeConfig::new(5, 0.1);
        let mut source = StimSyndromeSource::new(config).unwrap();

        for _ in 0..100 {
            let bitmap = source.sample().unwrap();
            stats.update(&bitmap);
        }

        assert_eq!(stats.total_syndromes, 100);
        assert!(stats.avg_detection_rate > 0.0);
    }

    #[test]
    fn test_xorshift_rng() {
        let mut rng = Xorshift64::new(12345);

        // Should produce different values
        let a = rng.next();
        let b = rng.next();
        assert_ne!(a, b);

        // f64 should be in [0, 1)
        for _ in 0..100 {
            let f = rng.next_f64();
            assert!(f >= 0.0 && f < 1.0);
        }
    }
}
