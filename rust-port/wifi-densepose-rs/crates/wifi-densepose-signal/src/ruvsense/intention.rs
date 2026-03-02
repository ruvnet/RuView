//! Pre-movement intention lead signal detector.
//!
//! Detects anticipatory postural adjustments (APAs) 200-500ms before
//! visible movement onset. Works by analyzing the trajectory of AETHER
//! embeddings in embedding space: before a person initiates a step or
//! reach, their weight shifts create subtle CSI changes that appear as
//! velocity and acceleration in embedding space.
//!
//! # Algorithm
//! 1. Maintain a rolling window of recent embeddings (2 seconds at 20 Hz)
//! 2. Compute velocity (first derivative) and acceleration (second derivative)
//!    in embedding space
//! 3. Detect when acceleration exceeds a threshold while velocity is still low
//!    (the body is loading/shifting but hasn't moved yet)
//! 4. Output a lead signal with estimated time-to-movement
//!
//! # References
//! - ADR-030 Tier 3: Intention Lead Signals
//! - Massion (1992), "Movement, posture and equilibrium: Interaction
//!   and coordination" Progress in Neurobiology

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from intention detection operations.
#[derive(Debug, thiserror::Error)]
pub enum IntentionError {
    /// Not enough embedding history to compute derivatives.
    #[error("Insufficient history: need >= {needed} frames, got {got}")]
    InsufficientHistory { needed: usize, got: usize },

    /// Embedding dimension mismatch.
    #[error("Embedding dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the intention detector.
#[derive(Debug, Clone)]
pub struct IntentionConfig {
    /// Embedding dimension (typically 128).
    pub embedding_dim: usize,
    /// Rolling window size in frames (2s at 20Hz = 40 frames).
    pub window_size: usize,
    /// Sampling rate in Hz.
    pub sample_rate_hz: f64,
    /// Acceleration threshold for pre-movement detection (embedding space units/s^2).
    pub acceleration_threshold: f64,
    /// Maximum velocity for a pre-movement signal (below this = still preparing).
    pub max_pre_movement_velocity: f64,
    /// Minimum frames of sustained acceleration to trigger a lead signal.
    pub min_sustained_frames: usize,
    /// Lead time window: max seconds before movement that we flag.
    pub max_lead_time_s: f64,
}

impl Default for IntentionConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 128,
            window_size: 40,
            sample_rate_hz: 20.0,
            acceleration_threshold: 0.5,
            max_pre_movement_velocity: 2.0,
            min_sustained_frames: 4,
            max_lead_time_s: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Lead signal result
// ---------------------------------------------------------------------------

/// Pre-movement lead signal.
#[derive(Debug, Clone)]
pub struct LeadSignal {
    /// Whether a pre-movement signal was detected.
    pub detected: bool,
    /// Confidence in the detection (0.0 to 1.0).
    pub confidence: f64,
    /// Estimated time until movement onset (seconds).
    pub estimated_lead_time_s: f64,
    /// Current velocity magnitude in embedding space.
    pub velocity_magnitude: f64,
    /// Current acceleration magnitude in embedding space.
    pub acceleration_magnitude: f64,
    /// Number of consecutive frames of sustained acceleration.
    pub sustained_frames: usize,
    /// Timestamp (microseconds) of this detection.
    pub timestamp_us: u64,
    /// Dominant direction of acceleration (unit vector in embedding space, first 3 dims).
    pub direction_hint: [f64; 3],
}

/// Trajectory state for one frame.
#[derive(Debug, Clone)]
struct TrajectoryPoint {
    embedding: Vec<f64>,
    timestamp_us: u64,
}

// ---------------------------------------------------------------------------
// Intention detector
// ---------------------------------------------------------------------------

/// Pre-movement intention lead signal detector.
///
/// Maintains a rolling window of embeddings and computes velocity
/// and acceleration in embedding space to detect anticipatory
/// postural adjustments before movement onset.
#[derive(Debug)]
pub struct IntentionDetector {
    config: IntentionConfig,
    /// Rolling window of recent trajectory points.
    history: VecDeque<TrajectoryPoint>,
    /// Count of consecutive frames with pre-movement signature.
    sustained_count: usize,
    /// Total frames processed.
    total_frames: u64,
}

impl IntentionDetector {
    /// Create a new intention detector.
    pub fn new(config: IntentionConfig) -> Result<Self, IntentionError> {
        if config.embedding_dim == 0 {
            return Err(IntentionError::InvalidConfig(
                "embedding_dim must be > 0".into(),
            ));
        }
        if config.window_size < 3 {
            return Err(IntentionError::InvalidConfig(
                "window_size must be >= 3 for second derivative".into(),
            ));
        }
        Ok(Self {
            history: VecDeque::with_capacity(config.window_size),
            config,
            sustained_count: 0,
            total_frames: 0,
        })
    }

    /// Feed a new embedding and check for pre-movement signals.
    ///
    /// `embedding` is the AETHER embedding for the current frame.
    /// Returns a lead signal result.
    pub fn update(
        &mut self,
        embedding: &[f32],
        timestamp_us: u64,
    ) -> Result<LeadSignal, IntentionError> {
        if embedding.len() != self.config.embedding_dim {
            return Err(IntentionError::DimensionMismatch {
                expected: self.config.embedding_dim,
                got: embedding.len(),
            });
        }

        self.total_frames += 1;

        // Convert to f64 for trajectory analysis
        let emb_f64: Vec<f64> = embedding.iter().map(|&x| x as f64).collect();

        // Add to history
        if self.history.len() >= self.config.window_size {
            self.history.pop_front();
        }
        self.history.push_back(TrajectoryPoint {
            embedding: emb_f64,
            timestamp_us,
        });

        // Need at least 3 points for second derivative
        if self.history.len() < 3 {
            return Ok(LeadSignal {
                detected: false,
                confidence: 0.0,
                estimated_lead_time_s: 0.0,
                velocity_magnitude: 0.0,
                acceleration_magnitude: 0.0,
                sustained_frames: 0,
                timestamp_us,
                direction_hint: [0.0; 3],
            });
        }

        // Compute velocity and acceleration
        let n = self.history.len();
        let dt = 1.0 / self.config.sample_rate_hz;

        // Velocity: (embedding[n-1] - embedding[n-2]) / dt
        let velocity = embedding_diff(
            &self.history[n - 1].embedding,
            &self.history[n - 2].embedding,
            dt,
        );
        let velocity_mag = l2_norm_f64(&velocity);

        // Acceleration: (velocity[n-1] - velocity[n-2]) / dt
        // Approximate: (emb[n-1] - 2*emb[n-2] + emb[n-3]) / dt^2
        let acceleration = embedding_second_diff(
            &self.history[n - 1].embedding,
            &self.history[n - 2].embedding,
            &self.history[n - 3].embedding,
            dt,
        );
        let accel_mag = l2_norm_f64(&acceleration);

        // Pre-movement detection:
        // High acceleration + low velocity = body is loading/shifting but hasn't moved
        let is_pre_movement = accel_mag > self.config.acceleration_threshold
            && velocity_mag < self.config.max_pre_movement_velocity;

        if is_pre_movement {
            self.sustained_count += 1;
        } else {
            self.sustained_count = 0;
        }

        let detected = self.sustained_count >= self.config.min_sustained_frames;

        // Estimate lead time based on current acceleration and velocity
        let estimated_lead = if detected && accel_mag > 1e-10 {
            // Time until velocity reaches threshold: t = (v_thresh - v) / a
            let remaining = (self.config.max_pre_movement_velocity - velocity_mag) / accel_mag;
            remaining.clamp(0.0, self.config.max_lead_time_s)
        } else {
            0.0
        };

        // Confidence based on how clearly the acceleration exceeds threshold
        let confidence = if detected {
            let ratio = accel_mag / self.config.acceleration_threshold;
            (ratio - 1.0).clamp(0.0, 1.0)
                * (self.sustained_count as f64 / self.config.min_sustained_frames as f64).min(1.0)
        } else {
            0.0
        };

        // Direction hint from first 3 dimensions of acceleration
        let direction_hint = [
            acceleration.first().copied().unwrap_or(0.0),
            acceleration.get(1).copied().unwrap_or(0.0),
            acceleration.get(2).copied().unwrap_or(0.0),
        ];

        Ok(LeadSignal {
            detected,
            confidence,
            estimated_lead_time_s: estimated_lead,
            velocity_magnitude: velocity_mag,
            acceleration_magnitude: accel_mag,
            sustained_frames: self.sustained_count,
            timestamp_us,
            direction_hint,
        })
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.history.clear();
        self.sustained_count = 0;
    }

    /// Number of frames in the history.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Total frames processed.
    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// First difference of two embedding vectors, divided by dt.
fn embedding_diff(a: &[f64], b: &[f64], dt: f64) -> Vec<f64> {
    a.iter()
        .zip(b.iter())
        .map(|(&ai, &bi)| (ai - bi) / dt)
        .collect()
}

/// Second difference: (a - 2b + c) / dt^2.
fn embedding_second_diff(a: &[f64], b: &[f64], c: &[f64], dt: f64) -> Vec<f64> {
    let dt2 = dt * dt;
    a.iter()
        .zip(b.iter())
        .zip(c.iter())
        .map(|((&ai, &bi), &ci)| (ai - 2.0 * bi + ci) / dt2)
        .collect()
}

/// L2 norm of an f64 slice.
fn l2_norm_f64(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> IntentionConfig {
        IntentionConfig {
            embedding_dim: 4,
            window_size: 10,
            sample_rate_hz: 20.0,
            acceleration_threshold: 0.5,
            max_pre_movement_velocity: 2.0,
            min_sustained_frames: 3,
            max_lead_time_s: 0.5,
        }
    }

    fn static_embedding() -> Vec<f32> {
        vec![1.0, 0.0, 0.0, 0.0]
    }

    #[test]
    fn test_creation() {
        let config = make_config();
        let detector = IntentionDetector::new(config).unwrap();
        assert_eq!(detector.history_len(), 0);
        assert_eq!(detector.total_frames(), 0);
    }

    #[test]
    fn test_invalid_config_zero_dim() {
        let config = IntentionConfig {
            embedding_dim: 0,
            ..make_config()
        };
        assert!(matches!(
            IntentionDetector::new(config),
            Err(IntentionError::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_invalid_config_small_window() {
        let config = IntentionConfig {
            window_size: 2,
            ..make_config()
        };
        assert!(matches!(
            IntentionDetector::new(config),
            Err(IntentionError::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_dimension_mismatch() {
        let config = make_config();
        let mut detector = IntentionDetector::new(config).unwrap();
        let result = detector.update(&[1.0, 0.0], 0);
        assert!(matches!(
            result,
            Err(IntentionError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_static_scene_no_detection() {
        let config = make_config();
        let mut detector = IntentionDetector::new(config).unwrap();

        for frame in 0..20 {
            let signal = detector
                .update(&static_embedding(), frame * 50_000)
                .unwrap();
            assert!(
                !signal.detected,
                "Static scene should not trigger detection"
            );
        }
    }

    #[test]
    fn test_gradual_acceleration_detected() {
        let mut config = make_config();
        config.acceleration_threshold = 100.0; // low threshold for test
        config.max_pre_movement_velocity = 100000.0;
        config.min_sustained_frames = 2;

        let mut detector = IntentionDetector::new(config).unwrap();

        // Feed gradually accelerating embeddings
        // Position = 0.5 * a * t^2, so embedding shifts quadratically
        let mut any_detected = false;
        for frame in 0..30_u64 {
            let t = frame as f32 * 0.05;
            let pos = 50.0 * t * t; // acceleration = 100 units/s^2
            let emb = vec![1.0 + pos, 0.0, 0.0, 0.0];
            let signal = detector.update(&emb, frame * 50_000).unwrap();
            if signal.detected {
                any_detected = true;
                assert!(signal.confidence > 0.0);
                assert!(signal.acceleration_magnitude > 0.0);
            }
        }
        assert!(any_detected, "Accelerating signal should trigger detection");
    }

    #[test]
    fn test_constant_velocity_no_detection() {
        let config = make_config();
        let mut detector = IntentionDetector::new(config).unwrap();

        // Constant velocity = zero acceleration → no pre-movement
        for frame in 0..20_u64 {
            let pos = frame as f32 * 0.01; // constant velocity
            let emb = vec![1.0 + pos, 0.0, 0.0, 0.0];
            let signal = detector.update(&emb, frame * 50_000).unwrap();
            assert!(
                !signal.detected,
                "Constant velocity should not trigger pre-movement"
            );
        }
    }

    #[test]
    fn test_reset() {
        let config = make_config();
        let mut detector = IntentionDetector::new(config).unwrap();

        for frame in 0..5_u64 {
            detector
                .update(&static_embedding(), frame * 50_000)
                .unwrap();
        }
        assert_eq!(detector.history_len(), 5);

        detector.reset();
        assert_eq!(detector.history_len(), 0);
    }

    #[test]
    fn test_lead_signal_fields() {
        let config = make_config();
        let mut detector = IntentionDetector::new(config).unwrap();

        // Need at least 3 frames for derivatives
        for frame in 0..3_u64 {
            let signal = detector
                .update(&static_embedding(), frame * 50_000)
                .unwrap();
            assert_eq!(signal.sustained_frames, 0);
        }

        let signal = detector.update(&static_embedding(), 150_000).unwrap();
        assert!(signal.velocity_magnitude >= 0.0);
        assert!(signal.acceleration_magnitude >= 0.0);
        assert_eq!(signal.direction_hint.len(), 3);
    }

    #[test]
    fn test_window_size_limit() {
        let config = IntentionConfig {
            window_size: 5,
            ..make_config()
        };
        let mut detector = IntentionDetector::new(config).unwrap();

        for frame in 0..10_u64 {
            detector
                .update(&static_embedding(), frame * 50_000)
                .unwrap();
        }
        assert_eq!(detector.history_len(), 5);
    }

    #[test]
    fn test_embedding_diff() {
        let a = vec![2.0, 4.0];
        let b = vec![1.0, 2.0];
        let diff = embedding_diff(&a, &b, 0.5);
        assert!((diff[0] - 2.0).abs() < 1e-10); // (2-1)/0.5
        assert!((diff[1] - 4.0).abs() < 1e-10); // (4-2)/0.5
    }

    #[test]
    fn test_embedding_second_diff() {
        // Quadratic sequence: 1, 4, 9 → second diff = 2
        let a = vec![9.0];
        let b = vec![4.0];
        let c = vec![1.0];
        let sd = embedding_second_diff(&a, &b, &c, 1.0);
        assert!((sd[0] - 2.0).abs() < 1e-10);
    }
}
