//! Compressed streaming breathing buffer (ruvector-temporal-tensor).
//!
//! [`CompressedBreathingBuffer`] stores per-frame subcarrier amplitude arrays
//! using a tiered quantization scheme:
//!
//! - Hot tier (recent ~10 frames): 8-bit
//! - Warm tier: 5–7-bit
//! - Cold tier: 3-bit
//!
//! For 56 subcarriers × 60 s × 100 Hz: 13.4 MB raw → 3.4–6.7 MB compressed.

use ruvector_temporal_tensor::segment as tt_segment;
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

/// Streaming compressed breathing buffer.
///
/// Hot frames (recent ~10) at 8-bit, warm at 5–7-bit, cold at 3-bit.
/// For 56 subcarriers × 60 s × 100 Hz: 13.4 MB raw → 3.4–6.7 MB compressed.
pub struct CompressedBreathingBuffer {
    compressor: TemporalTensorCompressor,
    segments: Vec<Vec<u8>>,
    frame_count: u32,
    /// Number of subcarriers per frame (typically 56).
    pub n_subcarriers: usize,
}

impl CompressedBreathingBuffer {
    /// Create a new buffer.
    ///
    /// # Arguments
    ///
    /// - `n_subcarriers`: number of subcarriers per frame; typically 56.
    /// - `zone_id`: disaster zone identifier used as the tensor ID.
    pub fn new(n_subcarriers: usize, zone_id: u32) -> Self {
        Self {
            compressor: TemporalTensorCompressor::new(
                TierPolicy::default(),
                n_subcarriers as u32,
                zone_id,
            ),
            segments: Vec::new(),
            frame_count: 0,
            n_subcarriers,
        }
    }

    /// Push one time-frame of amplitude values.
    ///
    /// The frame is compressed and appended to the internal segment store.
    /// Non-empty segments are retained; empty outputs (compressor buffering)
    /// are silently skipped.
    pub fn push_frame(&mut self, amplitudes: &[f32]) {
        let ts = self.frame_count;
        self.compressor.set_access(ts, ts);
        let mut seg = Vec::new();
        self.compressor.push_frame(amplitudes, ts, &mut seg);
        if !seg.is_empty() {
            self.segments.push(seg);
        }
        self.frame_count += 1;
    }

    /// Number of frames pushed so far.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Decode all compressed frames to a flat `f32` vec.
    ///
    /// Concatenates decoded segments in order. The resulting length may be
    /// less than `frame_count * n_subcarriers` if the compressor has not yet
    /// flushed all frames (tiered flushing may batch frames).
    pub fn to_vec(&self) -> Vec<f32> {
        let mut out = Vec::new();
        for seg in &self.segments {
            tt_segment::decode(seg, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breathing_buffer_frame_count() {
        let n_subcarriers = 56;
        let mut buf = CompressedBreathingBuffer::new(n_subcarriers, 1);

        for i in 0..20 {
            let amplitudes: Vec<f32> = (0..n_subcarriers).map(|s| (i * n_subcarriers + s) as f32 * 0.01).collect();
            buf.push_frame(&amplitudes);
        }

        assert_eq!(buf.frame_count(), 20, "frame_count must equal the number of pushed frames");
    }

    #[test]
    fn breathing_buffer_to_vec_runs() {
        let n_subcarriers = 56;
        let mut buf = CompressedBreathingBuffer::new(n_subcarriers, 2);

        for i in 0..10 {
            let amplitudes: Vec<f32> = (0..n_subcarriers).map(|s| (i + s) as f32 * 0.1).collect();
            buf.push_frame(&amplitudes);
        }

        // to_vec() must not panic; output length is determined by compressor flushing.
        let _decoded = buf.to_vec();
    }
}
