//! Tiered compressed heartbeat spectrogram (ruvector-temporal-tensor).
//!
//! [`CompressedHeartbeatSpectrogram`] stores a rolling spectrogram with one
//! [`TemporalTensorCompressor`] per frequency bin, enabling independent
//! tiering per bin. Hot tier (recent frames) at 8-bit, cold at 3-bit.
//!
//! [`band_power`] extracts mean squared power in any frequency band.

use ruvector_temporal_tensor::segment as tt_segment;
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

/// Tiered compressed heartbeat spectrogram.
///
/// One compressor per frequency bin. Hot tier (recent) at 8-bit, cold at 3-bit.
pub struct CompressedHeartbeatSpectrogram {
    bin_buffers: Vec<TemporalTensorCompressor>,
    encoded: Vec<Vec<u8>>,
    /// Number of frequency bins (e.g. 128).
    pub n_freq_bins: usize,
    frame_count: u32,
}

impl CompressedHeartbeatSpectrogram {
    /// Create with `n_freq_bins` frequency bins (e.g. 128).
    ///
    /// Each frequency bin gets its own [`TemporalTensorCompressor`] instance
    /// so the tiering policy operates independently per bin.
    pub fn new(n_freq_bins: usize) -> Self {
        let bin_buffers = (0..n_freq_bins)
            .map(|i| TemporalTensorCompressor::new(TierPolicy::default(), 1, i as u32))
            .collect();
        Self {
            bin_buffers,
            encoded: vec![Vec::new(); n_freq_bins],
            n_freq_bins,
            frame_count: 0,
        }
    }

    /// Push one spectrogram column (one time step, all frequency bins).
    ///
    /// `column` must have length equal to `n_freq_bins`.
    pub fn push_column(&mut self, column: &[f32]) {
        let ts = self.frame_count;
        for (i, (&val, buf)) in column.iter().zip(self.bin_buffers.iter_mut()).enumerate() {
            buf.set_access(ts, ts);
            buf.push_frame(&[val], ts, &mut self.encoded[i]);
        }
        self.frame_count += 1;
    }

    /// Total number of columns pushed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Extract mean squared power in a frequency band (indices `low_bin..=high_bin`).
    ///
    /// Decodes only the bins in the requested range and returns the mean of
    /// the squared decoded values over the last up to 100 frames.
    /// Returns `0.0` for an empty range.
    pub fn band_power(&self, low_bin: usize, high_bin: usize) -> f32 {
        let n = (high_bin.min(self.n_freq_bins - 1) + 1).saturating_sub(low_bin);
        if n == 0 {
            return 0.0;
        }
        (low_bin..=high_bin.min(self.n_freq_bins - 1))
            .map(|b| {
                let mut out = Vec::new();
                tt_segment::decode(&self.encoded[b], &mut out);
                out.iter().rev().take(100).map(|x| x * x).sum::<f32>()
            })
            .sum::<f32>()
            / n as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_spectrogram_frame_count() {
        let n_freq_bins = 16;
        let mut spec = CompressedHeartbeatSpectrogram::new(n_freq_bins);

        for i in 0..10 {
            let column: Vec<f32> = (0..n_freq_bins).map(|b| (i * n_freq_bins + b) as f32 * 0.01).collect();
            spec.push_column(&column);
        }

        assert_eq!(spec.frame_count(), 10, "frame_count must equal the number of pushed columns");
    }

    #[test]
    fn heartbeat_band_power_runs() {
        let n_freq_bins = 16;
        let mut spec = CompressedHeartbeatSpectrogram::new(n_freq_bins);

        for i in 0..10 {
            let column: Vec<f32> = (0..n_freq_bins).map(|b| (i + b) as f32 * 0.1).collect();
            spec.push_column(&column);
        }

        // band_power must not panic and must return a non-negative value.
        let power = spec.band_power(2, 6);
        assert!(power >= 0.0, "band_power must be non-negative");
    }
}
