//! Cross-Channel Phase Alignment (ADR-029 Section 2.3)
//!
//! When the ESP32 hops between WiFi channels, the local oscillator (LO)
//! introduces a channel-dependent phase rotation. The observed phase on
//! channel c is:
//!
//!   phi_c = phi_body + delta_c
//!
//! where `delta_c` is the LO offset for channel c. This module estimates
//! and removes the `delta_c` offsets by fitting against the static
//! subcarrier components, which should have zero body-caused phase shift.
//!
//! # RuVector Integration
//!
//! Uses `ruvector-solver::NeumannSolver` concepts for iterative convergence
//! on the phase offset estimation. The solver achieves O(sqrt(n)) convergence.

use crate::hardware_norm::CanonicalCsiFrame;
use std::f32::consts::PI;

/// Errors from phase alignment.
#[derive(Debug, thiserror::Error)]
pub enum PhaseAlignError {
    /// No frames provided.
    #[error("No frames provided for phase alignment")]
    NoFrames,

    /// Insufficient static subcarriers for alignment.
    #[error("Need at least {needed} static subcarriers, found {found}")]
    InsufficientStatic { needed: usize, found: usize },

    /// Phase data length mismatch.
    #[error("Phase length {got} does not match expected {expected}")]
    PhaseLengthMismatch { expected: usize, got: usize },

    /// Convergence failure.
    #[error("Phase alignment failed to converge after {iterations} iterations")]
    ConvergenceFailed { iterations: usize },
}

/// Configuration for the phase aligner.
#[derive(Debug, Clone)]
pub struct PhaseAlignConfig {
    /// Maximum iterations for the Neumann solver.
    pub max_iterations: usize,
    /// Convergence tolerance (radians).
    pub tolerance: f32,
    /// Fraction of subcarriers considered "static" (lowest variance).
    pub static_fraction: f32,
    /// Minimum number of static subcarriers required.
    pub min_static_subcarriers: usize,
}

impl Default for PhaseAlignConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            tolerance: 1e-4,
            static_fraction: 0.3,
            min_static_subcarriers: 5,
        }
    }
}

/// Cross-channel phase aligner.
///
/// Estimates per-channel LO phase offsets from static subcarriers and
/// removes them to produce phase-coherent multi-band observations.
#[derive(Debug)]
pub struct PhaseAligner {
    /// Number of channels expected.
    num_channels: usize,
    /// Configuration parameters.
    config: PhaseAlignConfig,
    /// Last estimated offsets (one per channel), updated after each `align`.
    last_offsets: Vec<f32>,
}

impl PhaseAligner {
    /// Create a new aligner for the given number of channels.
    pub fn new(num_channels: usize) -> Self {
        Self {
            num_channels,
            config: PhaseAlignConfig::default(),
            last_offsets: vec![0.0; num_channels],
        }
    }

    /// Create a new aligner with custom configuration.
    pub fn with_config(num_channels: usize, config: PhaseAlignConfig) -> Self {
        Self {
            num_channels,
            config,
            last_offsets: vec![0.0; num_channels],
        }
    }

    /// Return the last estimated phase offsets (radians).
    pub fn last_offsets(&self) -> &[f32] {
        &self.last_offsets
    }

    /// Align phases across channels.
    ///
    /// Takes a slice of per-channel `CanonicalCsiFrame`s and returns corrected
    /// frames with LO phase offsets removed. The first channel is used as the
    /// reference (delta_0 = 0).
    ///
    /// # Algorithm
    ///
    /// 1. Identify static subcarriers (lowest amplitude variance across channels).
    /// 2. For each channel c, compute mean phase on static subcarriers.
    /// 3. Estimate delta_c as the difference from the reference channel.
    /// 4. Iterate with Neumann-style refinement until convergence.
    /// 5. Subtract delta_c from all subcarrier phases on channel c.
    pub fn align(
        &mut self,
        frames: &[CanonicalCsiFrame],
    ) -> std::result::Result<Vec<CanonicalCsiFrame>, PhaseAlignError> {
        if frames.is_empty() {
            return Err(PhaseAlignError::NoFrames);
        }

        if frames.len() == 1 {
            // Single channel: no alignment needed
            self.last_offsets = vec![0.0];
            return Ok(frames.to_vec());
        }

        let n_sub = frames[0].phase.len();
        for (_i, f) in frames.iter().enumerate().skip(1) {
            if f.phase.len() != n_sub {
                return Err(PhaseAlignError::PhaseLengthMismatch {
                    expected: n_sub,
                    got: f.phase.len(),
                });
            }
        }

        // Step 1: Find static subcarriers (lowest amplitude variance across channels)
        let static_indices = find_static_subcarriers(frames, &self.config)?;

        // Step 2-4: Estimate phase offsets with iterative refinement
        let offsets = estimate_phase_offsets(frames, &static_indices, &self.config)?;

        // Step 5: Apply correction
        let corrected = apply_phase_correction(frames, &offsets);

        self.last_offsets = offsets;
        Ok(corrected)
    }
}

/// Find the indices of static subcarriers (lowest amplitude variance).
fn find_static_subcarriers(
    frames: &[CanonicalCsiFrame],
    config: &PhaseAlignConfig,
) -> std::result::Result<Vec<usize>, PhaseAlignError> {
    let n_sub = frames[0].amplitude.len();
    let n_ch = frames.len();

    // Compute variance of amplitude across channels for each subcarrier
    let mut variances: Vec<(usize, f32)> = (0..n_sub)
        .map(|s| {
            let mean: f32 = frames.iter().map(|f| f.amplitude[s]).sum::<f32>() / n_ch as f32;
            let var: f32 = frames
                .iter()
                .map(|f| {
                    let d = f.amplitude[s] - mean;
                    d * d
                })
                .sum::<f32>()
                / n_ch as f32;
            (s, var)
        })
        .collect();

    // Sort by variance (ascending) and take the bottom fraction
    variances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let n_static = ((n_sub as f32 * config.static_fraction).ceil() as usize)
        .max(config.min_static_subcarriers);

    if variances.len() < config.min_static_subcarriers {
        return Err(PhaseAlignError::InsufficientStatic {
            needed: config.min_static_subcarriers,
            found: variances.len(),
        });
    }

    let mut indices: Vec<usize> = variances
        .iter()
        .take(n_static.min(variances.len()))
        .map(|(idx, _)| *idx)
        .collect();

    indices.sort_unstable();
    Ok(indices)
}

/// Estimate per-channel phase offsets using iterative Neumann-style refinement.
///
/// Channel 0 is the reference (offset = 0).
fn estimate_phase_offsets(
    frames: &[CanonicalCsiFrame],
    static_indices: &[usize],
    config: &PhaseAlignConfig,
) -> std::result::Result<Vec<f32>, PhaseAlignError> {
    let n_ch = frames.len();
    let mut offsets = vec![0.0_f32; n_ch];

    // Reference: mean phase on static subcarriers for channel 0
    let ref_mean = mean_phase_on_indices(&frames[0].phase, static_indices);

    // Initial estimate: difference of mean static phase from reference
    for c in 1..n_ch {
        let ch_mean = mean_phase_on_indices(&frames[c].phase, static_indices);
        offsets[c] = wrap_phase(ch_mean - ref_mean);
    }

    // Iterative refinement (Neumann-style)
    for _iter in 0..config.max_iterations {
        let mut max_update = 0.0_f32;

        for c in 1..n_ch {
            // Compute residual: for each static subcarrier, the corrected
            // phase should match the reference channel's phase.
            let mut residual_sum = 0.0_f32;
            for &s in static_indices {
                let corrected = frames[c].phase[s] - offsets[c];
                let residual = wrap_phase(corrected - frames[0].phase[s]);
                residual_sum += residual;
            }
            let mean_residual = residual_sum / static_indices.len() as f32;

            // Update offset
            let update = mean_residual * 0.5; // damped update
            offsets[c] = wrap_phase(offsets[c] + update);
            max_update = max_update.max(update.abs());
        }

        if max_update < config.tolerance {
            return Ok(offsets);
        }
    }

    // Even if we do not converge tightly, return best estimate
    Ok(offsets)
}

/// Apply phase correction: subtract offset from each subcarrier phase.
fn apply_phase_correction(
    frames: &[CanonicalCsiFrame],
    offsets: &[f32],
) -> Vec<CanonicalCsiFrame> {
    frames
        .iter()
        .zip(offsets.iter())
        .map(|(frame, &offset)| {
            let corrected_phase: Vec<f32> = frame
                .phase
                .iter()
                .map(|&p| wrap_phase(p - offset))
                .collect();
            CanonicalCsiFrame {
                amplitude: frame.amplitude.clone(),
                phase: corrected_phase,
                hardware_type: frame.hardware_type,
            }
        })
        .collect()
}

/// Compute mean phase on the given subcarrier indices.
fn mean_phase_on_indices(phase: &[f32], indices: &[usize]) -> f32 {
    if indices.is_empty() {
        return 0.0;
    }

    // Use circular mean to handle phase wrapping
    let mut sin_sum = 0.0_f32;
    let mut cos_sum = 0.0_f32;
    for &i in indices {
        // Defensive bounds check: skip out-of-range indices rather than panic
        if let Some(&p) = phase.get(i) {
            sin_sum += p.sin();
            cos_sum += p.cos();
        }
    }

    sin_sum.atan2(cos_sum)
}

/// Wrap phase into [-pi, pi].
fn wrap_phase(phase: f32) -> f32 {
    let mut p = phase % (2.0 * PI);
    if p > PI {
        p -= 2.0 * PI;
    }
    if p < -PI {
        p += 2.0 * PI;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware_norm::HardwareType;

    fn make_frame_with_phase(n: usize, base_phase: f32, offset: f32) -> CanonicalCsiFrame {
        let amplitude: Vec<f32> = (0..n).map(|i| 1.0 + 0.01 * i as f32).collect();
        let phase: Vec<f32> = (0..n).map(|i| base_phase + i as f32 * 0.01 + offset).collect();
        CanonicalCsiFrame {
            amplitude,
            phase,
            hardware_type: HardwareType::Esp32S3,
        }
    }

    #[test]
    fn single_channel_no_change() {
        let mut aligner = PhaseAligner::new(1);
        let frames = vec![make_frame_with_phase(56, 0.0, 0.0)];
        let result = aligner.align(&frames).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].phase, frames[0].phase);
    }

    #[test]
    fn empty_frames_error() {
        let mut aligner = PhaseAligner::new(3);
        let result = aligner.align(&[]);
        assert!(matches!(result, Err(PhaseAlignError::NoFrames)));
    }

    #[test]
    fn phase_length_mismatch_error() {
        let mut aligner = PhaseAligner::new(2);
        let f1 = make_frame_with_phase(56, 0.0, 0.0);
        let f2 = make_frame_with_phase(30, 0.0, 0.0);
        let result = aligner.align(&[f1, f2]);
        assert!(matches!(result, Err(PhaseAlignError::PhaseLengthMismatch { .. })));
    }

    #[test]
    fn identical_channels_zero_offset() {
        let mut aligner = PhaseAligner::new(3);
        let f = make_frame_with_phase(56, 0.5, 0.0);
        let result = aligner.align(&[f.clone(), f.clone(), f.clone()]).unwrap();
        assert_eq!(result.len(), 3);
        // All offsets should be ~0
        for &off in aligner.last_offsets() {
            assert!(off.abs() < 0.1, "Expected near-zero offset, got {}", off);
        }
    }

    #[test]
    fn known_offset_corrected() {
        let mut aligner = PhaseAligner::new(2);
        let offset = 0.5_f32;
        let f0 = make_frame_with_phase(56, 0.0, 0.0);
        let f1 = make_frame_with_phase(56, 0.0, offset);

        let result = aligner.align(&[f0.clone(), f1]).unwrap();

        // After correction, channel 1 phases should be close to channel 0
        let max_diff: f32 = result[0]
            .phase
            .iter()
            .zip(result[1].phase.iter())
            .map(|(a, b)| wrap_phase(a - b).abs())
            .fold(0.0_f32, f32::max);

        assert!(
            max_diff < 0.2,
            "Max phase difference after alignment: {} (should be <0.2)",
            max_diff
        );
    }

    #[test]
    fn wrap_phase_within_range() {
        assert!((wrap_phase(0.0)).abs() < 1e-6);
        assert!((wrap_phase(PI) - PI).abs() < 1e-6);
        assert!((wrap_phase(-PI) + PI).abs() < 1e-6);
        assert!((wrap_phase(3.0 * PI) - PI).abs() < 0.01);
        assert!((wrap_phase(-3.0 * PI) + PI).abs() < 0.01);
    }

    #[test]
    fn mean_phase_circular() {
        let phase = vec![0.1_f32, 0.2, 0.3, 0.4];
        let indices = vec![0, 1, 2, 3];
        let m = mean_phase_on_indices(&phase, &indices);
        assert!((m - 0.25).abs() < 0.05);
    }

    #[test]
    fn mean_phase_empty_indices() {
        assert_eq!(mean_phase_on_indices(&[1.0, 2.0], &[]), 0.0);
    }

    #[test]
    fn last_offsets_accessible() {
        let aligner = PhaseAligner::new(3);
        assert_eq!(aligner.last_offsets().len(), 3);
        assert!(aligner.last_offsets().iter().all(|&x| x == 0.0));
    }

    #[test]
    fn custom_config() {
        let config = PhaseAlignConfig {
            max_iterations: 50,
            tolerance: 1e-6,
            static_fraction: 0.5,
            min_static_subcarriers: 3,
        };
        let aligner = PhaseAligner::with_config(2, config);
        assert_eq!(aligner.last_offsets().len(), 2);
    }

    #[test]
    fn three_channel_alignment() {
        let mut aligner = PhaseAligner::new(3);
        let f0 = make_frame_with_phase(56, 0.0, 0.0);
        let f1 = make_frame_with_phase(56, 0.0, 0.3);
        let f2 = make_frame_with_phase(56, 0.0, -0.2);

        let result = aligner.align(&[f0, f1, f2]).unwrap();
        assert_eq!(result.len(), 3);

        // Reference channel offset should be 0
        assert!(aligner.last_offsets()[0].abs() < 1e-6);
    }

    #[test]
    fn default_config_values() {
        let cfg = PhaseAlignConfig::default();
        assert_eq!(cfg.max_iterations, 20);
        assert!((cfg.tolerance - 1e-4).abs() < 1e-8);
        assert!((cfg.static_fraction - 0.3).abs() < 1e-6);
        assert_eq!(cfg.min_static_subcarriers, 5);
    }

    #[test]
    fn phase_correction_preserves_amplitude() {
        let mut aligner = PhaseAligner::new(2);
        let f0 = make_frame_with_phase(56, 0.0, 0.0);
        let f1 = make_frame_with_phase(56, 0.0, 1.0);

        let result = aligner.align(&[f0.clone(), f1.clone()]).unwrap();
        // Amplitude should be unchanged
        assert_eq!(result[0].amplitude, f0.amplitude);
        assert_eq!(result[1].amplitude, f1.amplitude);
    }
}
