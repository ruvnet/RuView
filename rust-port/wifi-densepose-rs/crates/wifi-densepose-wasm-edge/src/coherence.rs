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
#[derive(Clone, Copy, PartialEq)]
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
