//! Phase phasor coherence monitor — no_std port.
//!
//! Ported from `ruvector/viewpoint/coherence.rs` for WASM execution.
//! Computes mean phasor coherence across subcarriers to detect signal quality
//! and environmental stability.  Low coherence indicates multipath interference
//! or environmental changes that degrade sensing accuracy.

use libm::{cosf, sinf, sqrtf, atan2f};

/// Number of subcarriers to track for coherence.
const MAX_SC: usize = 32;

/// EMA smoothing factor for coherence score.
const ALPHA: f32 = 0.1;

/// Hysteresis thresholds for coherence gate decisions.
const HIGH_THRESHOLD: f32 = 0.7;
const LOW_THRESHOLD: f32 = 0.4;

/// Coherence gate state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GateState {
    /// Signal is coherent — full sensing accuracy.
    Accept,
    /// Marginal coherence — predictions may be degraded.
    Warn,
    /// Incoherent — sensing unreliable, need recalibration.
    Reject,
}

/// Phase phasor coherence monitor.
pub struct CoherenceMonitor {
    /// Previous phase per subcarrier (for delta computation).
    prev_phases: [f32; MAX_SC],
    /// Running phasor sum (real component).
    phasor_re: f32,
    /// Running phasor sum (imaginary component).
    phasor_im: f32,
    /// EMA-smoothed coherence score [0, 1].
    smoothed_coherence: f32,
    /// Number of frames processed.
    frame_count: u32,
    /// Current gate state (with hysteresis).
    gate: GateState,
    /// Whether the monitor has been initialized.
    initialized: bool,
}

impl CoherenceMonitor {
    pub const fn new() -> Self {
        Self {
            prev_phases: [0.0; MAX_SC],
            phasor_re: 0.0,
            phasor_im: 0.0,
            smoothed_coherence: 1.0,
            frame_count: 0,
            gate: GateState::Accept,
            initialized: false,
        }
    }

    /// Process one frame of phase data and return the coherence score [0, 1].
    ///
    /// Coherence is computed as the magnitude of the mean phasor of inter-frame
    /// phase differences across subcarriers.  A score of 1.0 means all
    /// subcarriers exhibit the same phase shift (perfectly coherent signal);
    /// 0.0 means random phase changes (incoherent).
    pub fn process_frame(&mut self, phases: &[f32]) -> f32 {
        let n_sc = if phases.len() > MAX_SC { MAX_SC } else { phases.len() };

        // H-01 fix: guard against zero subcarriers to prevent division by zero.
        if n_sc == 0 {
            return self.smoothed_coherence;
        }

        if !self.initialized {
            for i in 0..n_sc {
                self.prev_phases[i] = phases[i];
            }
            self.initialized = true;
            return 1.0;
        }

        self.frame_count += 1;

        // Compute mean phasor of phase deltas.
        let mut sum_re = 0.0f32;
        let mut sum_im = 0.0f32;

        for i in 0..n_sc {
            let delta = phases[i] - self.prev_phases[i];
            // Phasor: e^{j*delta} = cos(delta) + j*sin(delta)
            sum_re += cosf(delta);
            sum_im += sinf(delta);
            self.prev_phases[i] = phases[i];
        }

        // Mean phasor.
        let n = n_sc as f32;
        let mean_re = sum_re / n;
        let mean_im = sum_im / n;

        // M-02 fix: store per-frame mean phasor so mean_phasor_angle() is accurate.
        self.phasor_re = mean_re;
        self.phasor_im = mean_im;

        // Coherence = magnitude of mean phasor [0, 1].
        let coherence = sqrtf(mean_re * mean_re + mean_im * mean_im);

        // EMA smoothing.
        self.smoothed_coherence = ALPHA * coherence + (1.0 - ALPHA) * self.smoothed_coherence;

        // Hysteresis gate update.
        self.gate = match self.gate {
            GateState::Accept => {
                if self.smoothed_coherence < LOW_THRESHOLD {
                    GateState::Reject
                } else if self.smoothed_coherence < HIGH_THRESHOLD {
                    GateState::Warn
                } else {
                    GateState::Accept
                }
            }
            GateState::Warn => {
                if self.smoothed_coherence >= HIGH_THRESHOLD {
                    GateState::Accept
                } else if self.smoothed_coherence < LOW_THRESHOLD {
                    GateState::Reject
                } else {
                    GateState::Warn
                }
            }
            GateState::Reject => {
                if self.smoothed_coherence >= HIGH_THRESHOLD {
                    GateState::Accept
                } else {
                    GateState::Reject
                }
            }
        };

        self.smoothed_coherence
    }

    /// Get the current gate state.
    pub fn gate_state(&self) -> GateState {
        self.gate
    }

    /// Get the mean phasor angle (radians) — indicates dominant phase drift direction.
    pub fn mean_phasor_angle(&self) -> f32 {
        atan2f(self.phasor_im, self.phasor_re)
    }

    /// Get the EMA-smoothed coherence score.
    pub fn coherence_score(&self) -> f32 {
        self.smoothed_coherence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_monitor_init() {
        let mon = CoherenceMonitor::new();
        assert!(!mon.initialized);
        assert_eq!(mon.gate_state(), GateState::Accept);
        assert!((mon.coherence_score() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_empty_phases_returns_current_score() {
        let mut mon = CoherenceMonitor::new();
        let score = mon.process_frame(&[]);
        assert!((score - 1.0).abs() < 0.001, "empty input should return current smoothed score");
    }

    #[test]
    fn test_first_frame_returns_one() {
        let mut mon = CoherenceMonitor::new();
        let score = mon.process_frame(&[0.1, 0.2, 0.3]);
        assert!((score - 1.0).abs() < 0.001, "first frame should return 1.0");
        assert!(mon.initialized);
    }

    #[test]
    fn test_constant_phases_high_coherence() {
        let mut mon = CoherenceMonitor::new();
        let phases = [1.0f32; 16];
        // First frame initializes
        mon.process_frame(&phases);
        // Subsequent frames with same phases => zero delta => cos(0)=1 => coherence=1.0
        for _ in 0..50 {
            let score = mon.process_frame(&phases);
            assert!(score > 0.9, "constant phases should yield high coherence, got {}", score);
        }
        assert_eq!(mon.gate_state(), GateState::Accept);
    }

    #[test]
    fn test_incoherent_phases_lower_coherence() {
        let mut mon = CoherenceMonitor::new();
        // Initialize with baseline
        mon.process_frame(&[0.0f32; 16]);

        // Feed phases where each subcarrier has a different, large shift
        // so the phasor directions cancel out, yielding low per-frame coherence.
        // The EMA (alpha=0.1) needs many frames to converge from the initial 1.0.
        for i in 0..2000 {
            let mut phases = [0.0f32; 16];
            for j in 0..16 {
                // Each subcarrier gets a distinct, rapidly changing phase
                // so inter-frame deltas point in different directions.
                phases[j] = (j as f32) * 3.14159 * 0.5 + (i as f32) * (j as f32 + 1.0) * 0.7;
            }
            mon.process_frame(&phases);
        }
        // After many truly incoherent frames, the EMA should have converged
        // below the high threshold.
        assert!(mon.coherence_score() < HIGH_THRESHOLD,
            "incoherent phases should yield coherence below {}, got {}",
            HIGH_THRESHOLD, mon.coherence_score());
    }

    #[test]
    fn test_gate_hysteresis() {
        let mut mon = CoherenceMonitor::new();
        // Force coherence down by setting smoothed_coherence directly
        // then test the gate transitions
        mon.initialized = true;
        mon.smoothed_coherence = 0.8;
        mon.gate = GateState::Accept;

        // Process frame that will lower coherence
        // With constant phases the raw coherence is 1.0 but EMA is 0.1*1.0 + 0.9*0.8 = 0.82
        // Still Accept
        let phases = [1.0f32; 8];
        mon.process_frame(&phases);
        assert_eq!(mon.gate_state(), GateState::Accept);
    }

    #[test]
    fn test_mean_phasor_angle_zero_for_no_drift() {
        let mut mon = CoherenceMonitor::new();
        let phases = [0.0f32; 8];
        mon.process_frame(&phases);
        mon.process_frame(&phases);
        // Zero phase delta => phasor at (1, 0) => angle = 0
        let angle = mon.mean_phasor_angle();
        assert!(angle.abs() < 0.01, "no drift should yield phasor angle ~0, got {}", angle);
    }
}
