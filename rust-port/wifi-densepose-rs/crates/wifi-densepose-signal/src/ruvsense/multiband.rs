//! Multi-Band CSI Frame Fusion (ADR-029 Section 2.3)
//!
//! Aggregates per-channel CSI frames from channel-hopping into a wideband
//! virtual snapshot. An ESP32-S3 cycling through channels 1/6/11 at 50 ms
//! dwell per channel yields 3 canonical-56 CSI rows per sensing cycle.
//! This module fuses them into a single `MultiBandCsiFrame` annotated with
//! center frequencies and cross-channel coherence.
//!
//! # RuVector Integration
//!
//! - `ruvector-attention` for cross-channel feature weighting (future)

use crate::hardware_norm::CanonicalCsiFrame;

/// Errors from multi-band frame fusion.
#[derive(Debug, thiserror::Error)]
pub enum MultiBandError {
    /// No channel frames provided.
    #[error("No channel frames provided for multi-band fusion")]
    NoFrames,

    /// Mismatched subcarrier counts across channels.
    #[error("Subcarrier count mismatch: channel {channel_idx} has {got}, expected {expected}")]
    SubcarrierMismatch {
        channel_idx: usize,
        expected: usize,
        got: usize,
    },

    /// Frequency list length does not match frame count.
    #[error("Frequency count ({freq_count}) does not match frame count ({frame_count})")]
    FrequencyCountMismatch { freq_count: usize, frame_count: usize },

    /// Duplicate frequency in channel list.
    #[error("Duplicate frequency {freq_mhz} MHz at index {idx}")]
    DuplicateFrequency { freq_mhz: u32, idx: usize },
}

/// Fused multi-band CSI from one node at one time slot.
///
/// Holds one canonical-56 row per channel, ordered by center frequency.
/// The `coherence` field quantifies agreement across channels (0.0-1.0).
#[derive(Debug, Clone)]
pub struct MultiBandCsiFrame {
    /// Originating node identifier (0-255).
    pub node_id: u8,
    /// Timestamp of the sensing cycle in microseconds.
    pub timestamp_us: u64,
    /// One canonical-56 CSI frame per channel, ordered by center frequency.
    pub channel_frames: Vec<CanonicalCsiFrame>,
    /// Center frequencies (MHz) for each channel row.
    pub frequencies_mhz: Vec<u32>,
    /// Cross-channel coherence score (0.0-1.0).
    pub coherence: f32,
}

/// Configuration for the multi-band fusion process.
#[derive(Debug, Clone)]
pub struct MultiBandConfig {
    /// Time window in microseconds within which frames are considered
    /// part of the same sensing cycle.
    pub window_us: u64,
    /// Expected number of channels per cycle.
    pub expected_channels: usize,
    /// Minimum coherence to accept the fused frame.
    pub min_coherence: f32,
}

impl Default for MultiBandConfig {
    fn default() -> Self {
        Self {
            window_us: 200_000, // 200 ms default window
            expected_channels: 3,
            min_coherence: 0.3,
        }
    }
}

/// Builder for constructing a `MultiBandCsiFrame` from per-channel observations.
#[derive(Debug)]
pub struct MultiBandBuilder {
    node_id: u8,
    timestamp_us: u64,
    frames: Vec<CanonicalCsiFrame>,
    frequencies: Vec<u32>,
}

impl MultiBandBuilder {
    /// Create a new builder for the given node and timestamp.
    pub fn new(node_id: u8, timestamp_us: u64) -> Self {
        Self {
            node_id,
            timestamp_us,
            frames: Vec::new(),
            frequencies: Vec::new(),
        }
    }

    /// Add a channel observation at the given center frequency.
    pub fn add_channel(
        mut self,
        frame: CanonicalCsiFrame,
        freq_mhz: u32,
    ) -> Self {
        self.frames.push(frame);
        self.frequencies.push(freq_mhz);
        self
    }

    /// Build the fused multi-band frame.
    ///
    /// Validates inputs, sorts by frequency, and computes cross-channel coherence.
    pub fn build(mut self) -> std::result::Result<MultiBandCsiFrame, MultiBandError> {
        if self.frames.is_empty() {
            return Err(MultiBandError::NoFrames);
        }

        if self.frequencies.len() != self.frames.len() {
            return Err(MultiBandError::FrequencyCountMismatch {
                freq_count: self.frequencies.len(),
                frame_count: self.frames.len(),
            });
        }

        // Check for duplicate frequencies
        for i in 0..self.frequencies.len() {
            for j in (i + 1)..self.frequencies.len() {
                if self.frequencies[i] == self.frequencies[j] {
                    return Err(MultiBandError::DuplicateFrequency {
                        freq_mhz: self.frequencies[i],
                        idx: j,
                    });
                }
            }
        }

        // Validate consistent subcarrier counts
        let expected_len = self.frames[0].amplitude.len();
        for (i, frame) in self.frames.iter().enumerate().skip(1) {
            if frame.amplitude.len() != expected_len {
                return Err(MultiBandError::SubcarrierMismatch {
                    channel_idx: i,
                    expected: expected_len,
                    got: frame.amplitude.len(),
                });
            }
        }

        // Sort frames by frequency
        let mut indices: Vec<usize> = (0..self.frames.len()).collect();
        indices.sort_by_key(|&i| self.frequencies[i]);

        let sorted_frames: Vec<CanonicalCsiFrame> =
            indices.iter().map(|&i| self.frames[i].clone()).collect();
        let sorted_freqs: Vec<u32> =
            indices.iter().map(|&i| self.frequencies[i]).collect();

        self.frames = sorted_frames;
        self.frequencies = sorted_freqs;

        // Compute cross-channel coherence
        let coherence = compute_cross_channel_coherence(&self.frames);

        Ok(MultiBandCsiFrame {
            node_id: self.node_id,
            timestamp_us: self.timestamp_us,
            channel_frames: self.frames,
            frequencies_mhz: self.frequencies,
            coherence,
        })
    }
}

/// Compute cross-channel coherence as the mean pairwise Pearson correlation
/// of amplitude vectors across all channel pairs.
///
/// Returns a value in [0.0, 1.0] where 1.0 means perfect correlation.
fn compute_cross_channel_coherence(frames: &[CanonicalCsiFrame]) -> f32 {
    if frames.len() < 2 {
        return 1.0; // single channel is trivially coherent
    }

    let mut total_corr = 0.0_f64;
    let mut pair_count = 0u32;

    for i in 0..frames.len() {
        for j in (i + 1)..frames.len() {
            let corr = pearson_correlation_f32(
                &frames[i].amplitude,
                &frames[j].amplitude,
            );
            total_corr += corr as f64;
            pair_count += 1;
        }
    }

    if pair_count == 0 {
        return 1.0;
    }

    // Map correlation [-1, 1] to coherence [0, 1]
    let mean_corr = total_corr / pair_count as f64;
    ((mean_corr + 1.0) / 2.0).clamp(0.0, 1.0) as f32
}

/// Pearson correlation coefficient between two f32 slices.
fn pearson_correlation_f32(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }

    let n_f = n as f32;
    let mean_a: f32 = a[..n].iter().sum::<f32>() / n_f;
    let mean_b: f32 = b[..n].iter().sum::<f32>() / n_f;

    let mut cov = 0.0_f32;
    let mut var_a = 0.0_f32;
    let mut var_b = 0.0_f32;

    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-12 {
        return 0.0;
    }

    (cov / denom).clamp(-1.0, 1.0)
}

/// Concatenate the amplitude vectors from all channels into a single
/// wideband amplitude vector. Useful for downstream models that expect
/// a flat feature vector.
pub fn concatenate_amplitudes(frame: &MultiBandCsiFrame) -> Vec<f32> {
    let total_len: usize = frame.channel_frames.iter().map(|f| f.amplitude.len()).sum();
    let mut out = Vec::with_capacity(total_len);
    for cf in &frame.channel_frames {
        out.extend_from_slice(&cf.amplitude);
    }
    out
}

/// Compute the mean amplitude across all channels, producing a single
/// canonical-length vector that averages multi-band observations.
pub fn mean_amplitude(frame: &MultiBandCsiFrame) -> Vec<f32> {
    if frame.channel_frames.is_empty() {
        return Vec::new();
    }

    let n_sub = frame.channel_frames[0].amplitude.len();
    let n_ch = frame.channel_frames.len() as f32;
    let mut mean = vec![0.0_f32; n_sub];

    for cf in &frame.channel_frames {
        for (i, &val) in cf.amplitude.iter().enumerate() {
            if i < n_sub {
                mean[i] += val;
            }
        }
    }

    for v in &mut mean {
        *v /= n_ch;
    }

    mean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware_norm::HardwareType;

    fn make_canonical(amplitude: Vec<f32>, phase: Vec<f32>) -> CanonicalCsiFrame {
        CanonicalCsiFrame {
            amplitude,
            phase,
            hardware_type: HardwareType::Esp32S3,
        }
    }

    fn make_frame(n_sub: usize, scale: f32) -> CanonicalCsiFrame {
        let amp: Vec<f32> = (0..n_sub).map(|i| scale * (i as f32 * 0.1).sin()).collect();
        let phase: Vec<f32> = (0..n_sub).map(|i| (i as f32 * 0.05).cos()).collect();
        make_canonical(amp, phase)
    }

    #[test]
    fn build_single_channel() {
        let frame = MultiBandBuilder::new(0, 1000)
            .add_channel(make_frame(56, 1.0), 2412)
            .build()
            .unwrap();
        assert_eq!(frame.node_id, 0);
        assert_eq!(frame.timestamp_us, 1000);
        assert_eq!(frame.channel_frames.len(), 1);
        assert_eq!(frame.frequencies_mhz, vec![2412]);
        assert!((frame.coherence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn build_three_channels_sorted_by_freq() {
        let frame = MultiBandBuilder::new(1, 2000)
            .add_channel(make_frame(56, 1.0), 2462) // ch 11
            .add_channel(make_frame(56, 1.0), 2412) // ch 1
            .add_channel(make_frame(56, 1.0), 2437) // ch 6
            .build()
            .unwrap();
        assert_eq!(frame.frequencies_mhz, vec![2412, 2437, 2462]);
        assert_eq!(frame.channel_frames.len(), 3);
    }

    #[test]
    fn empty_frames_error() {
        let result = MultiBandBuilder::new(0, 0).build();
        assert!(matches!(result, Err(MultiBandError::NoFrames)));
    }

    #[test]
    fn subcarrier_mismatch_error() {
        let result = MultiBandBuilder::new(0, 0)
            .add_channel(make_frame(56, 1.0), 2412)
            .add_channel(make_frame(30, 1.0), 2437)
            .build();
        assert!(matches!(result, Err(MultiBandError::SubcarrierMismatch { .. })));
    }

    #[test]
    fn duplicate_frequency_error() {
        let result = MultiBandBuilder::new(0, 0)
            .add_channel(make_frame(56, 1.0), 2412)
            .add_channel(make_frame(56, 1.0), 2412)
            .build();
        assert!(matches!(result, Err(MultiBandError::DuplicateFrequency { .. })));
    }

    #[test]
    fn coherence_identical_channels() {
        let f = make_frame(56, 1.0);
        let frame = MultiBandBuilder::new(0, 0)
            .add_channel(f.clone(), 2412)
            .add_channel(f.clone(), 2437)
            .build()
            .unwrap();
        // Identical channels should have coherence == 1.0
        assert!((frame.coherence - 1.0).abs() < 0.01);
    }

    #[test]
    fn coherence_orthogonal_channels() {
        let n = 56;
        let amp_a: Vec<f32> = (0..n).map(|i| (i as f32 * 0.3).sin()).collect();
        let amp_b: Vec<f32> = (0..n).map(|i| (i as f32 * 0.3).cos()).collect();
        let ph = vec![0.0_f32; n];

        let frame = MultiBandBuilder::new(0, 0)
            .add_channel(make_canonical(amp_a, ph.clone()), 2412)
            .add_channel(make_canonical(amp_b, ph), 2437)
            .build()
            .unwrap();
        // Orthogonal signals should produce lower coherence
        assert!(frame.coherence < 0.9);
    }

    #[test]
    fn concatenate_amplitudes_correct_length() {
        let frame = MultiBandBuilder::new(0, 0)
            .add_channel(make_frame(56, 1.0), 2412)
            .add_channel(make_frame(56, 2.0), 2437)
            .add_channel(make_frame(56, 3.0), 2462)
            .build()
            .unwrap();
        let concat = concatenate_amplitudes(&frame);
        assert_eq!(concat.len(), 56 * 3);
    }

    #[test]
    fn mean_amplitude_correct() {
        let n = 4;
        let f1 = make_canonical(vec![1.0, 2.0, 3.0, 4.0], vec![0.0; n]);
        let f2 = make_canonical(vec![3.0, 4.0, 5.0, 6.0], vec![0.0; n]);
        let frame = MultiBandBuilder::new(0, 0)
            .add_channel(f1, 2412)
            .add_channel(f2, 2437)
            .build()
            .unwrap();
        let m = mean_amplitude(&frame);
        assert_eq!(m.len(), 4);
        assert!((m[0] - 2.0).abs() < 1e-6);
        assert!((m[1] - 3.0).abs() < 1e-6);
        assert!((m[2] - 4.0).abs() < 1e-6);
        assert!((m[3] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn mean_amplitude_empty() {
        let frame = MultiBandCsiFrame {
            node_id: 0,
            timestamp_us: 0,
            channel_frames: vec![],
            frequencies_mhz: vec![],
            coherence: 1.0,
        };
        assert!(mean_amplitude(&frame).is_empty());
    }

    #[test]
    fn pearson_correlation_perfect() {
        let a = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0_f32, 4.0, 6.0, 8.0, 10.0];
        let r = pearson_correlation_f32(&a, &b);
        assert!((r - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pearson_correlation_negative() {
        let a = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0];
        let b = vec![5.0_f32, 4.0, 3.0, 2.0, 1.0];
        let r = pearson_correlation_f32(&a, &b);
        assert!((r + 1.0).abs() < 1e-5);
    }

    #[test]
    fn pearson_correlation_empty() {
        assert_eq!(pearson_correlation_f32(&[], &[]), 0.0);
    }

    #[test]
    fn default_config() {
        let cfg = MultiBandConfig::default();
        assert_eq!(cfg.expected_channels, 3);
        assert_eq!(cfg.window_us, 200_000);
        assert!((cfg.min_coherence - 0.3).abs() < f32::EPSILON);
    }
}
