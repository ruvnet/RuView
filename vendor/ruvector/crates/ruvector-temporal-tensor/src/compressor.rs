//! TemporalTensorCompressor: the main entry point.
//!
//! Manages temporal segments, drift detection, and tier transitions.
//! Caches f32-converted scales to avoid repeated f16 conversion in hot paths.

use crate::quantizer;
use crate::segment;
use crate::tier_policy::TierPolicy;

pub struct TemporalTensorCompressor {
    policy: TierPolicy,
    len: u32,

    access_count: u32,
    last_access_ts: u32,

    active_bits: u8,
    active_group_len: usize,
    active_scales_f16: Vec<u16>,
    active_scales_f32: Vec<f32>, // Cached f32 conversion of scales
    active_frames: u32,
    active_data: Vec<u8>,
}

impl TemporalTensorCompressor {
    /// Create a new compressor for tensors of the given length.
    pub fn new(policy: TierPolicy, len: u32, now_ts: u32) -> Self {
        let bits = policy.select_bits(0, now_ts, now_ts);
        Self {
            policy,
            len,
            access_count: 0,
            last_access_ts: now_ts,
            active_bits: bits,
            active_group_len: policy.group_len.max(1) as usize,
            active_scales_f16: Vec::new(),
            active_scales_f32: Vec::new(),
            active_frames: 0,
            active_data: Vec::new(),
        }
    }

    /// Record an access (increments count, updates timestamp).
    pub fn touch(&mut self, now_ts: u32) {
        self.access_count = self.access_count.wrapping_add(1);
        self.last_access_ts = now_ts;
    }

    /// Set access stats directly (for restoring state).
    pub fn set_access(&mut self, access_count: u32, last_access_ts: u32) {
        self.access_count = access_count;
        self.last_access_ts = last_access_ts;
    }

    /// Current tier bits.
    pub fn active_bits(&self) -> u8 {
        self.active_bits
    }

    /// Number of frames in the current segment.
    pub fn active_frame_count(&self) -> u32 {
        self.active_frames
    }

    /// Current policy.
    pub fn policy(&self) -> &TierPolicy {
        &self.policy
    }

    /// Tensor length.
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Returns `true` if the tensor length is zero.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes currently buffered in the active segment data.
    pub fn active_data_bytes(&self) -> usize {
        self.active_data.len()
    }

    /// Push a frame. If a segment boundary is crossed, the completed segment
    /// bytes are written to `out_segment`. Otherwise `out_segment` is cleared.
    pub fn push_frame(&mut self, frame: &[f32], now_ts: u32, out_segment: &mut Vec<u8>) {
        out_segment.clear();

        if frame.len() != self.len as usize {
            return;
        }

        let desired_bits = self
            .policy
            .select_bits(self.access_count, self.last_access_ts, now_ts);
        let drift_factor = self.policy.drift_factor();

        // Use cached f32 scales for drift check (avoids f16 conversion per group)
        let need_new_segment = self.active_frames == 0
            || desired_bits != self.active_bits
            || !quantizer::frame_fits_scales_f32(
                frame,
                &self.active_scales_f32,
                self.active_group_len,
                self.active_bits,
                drift_factor,
            );

        if need_new_segment {
            self.flush(out_segment);
            self.active_bits = desired_bits;
            self.active_group_len = self.policy.group_len.max(1) as usize;
            self.active_scales_f16 =
                quantizer::compute_scales(frame, self.active_group_len, self.active_bits);
            self.active_scales_f32 = quantizer::scales_to_f32(&self.active_scales_f16);
        }

        // Use cached f32 scales for quantization (avoids f16 conversion per group)
        quantizer::quantize_and_pack_f32(
            frame,
            &self.active_scales_f32,
            self.active_group_len,
            self.active_bits,
            &mut self.active_data,
        );
        self.active_frames = self.active_frames.wrapping_add(1);
    }

    /// Flush the current segment. Writes segment bytes to `out_segment`.
    /// Resets internal state for the next segment.
    pub fn flush(&mut self, out_segment: &mut Vec<u8>) {
        if self.active_frames == 0 {
            return;
        }

        segment::encode(
            self.active_bits,
            self.active_group_len as u32,
            self.len,
            self.active_frames,
            &self.active_scales_f16,
            &self.active_data,
            out_segment,
        );

        self.active_frames = 0;
        self.active_scales_f16.clear();
        self.active_scales_f32.clear();
        self.active_data.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> TierPolicy {
        TierPolicy::default()
    }

    #[test]
    fn test_create_and_push() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 64, 0);
        let frame = vec![1.0f32; 64];
        let mut seg = Vec::new();

        comp.push_frame(&frame, 0, &mut seg);
        assert!(seg.is_empty()); // First frame, no completed segment
        assert_eq!(comp.active_frame_count(), 1);
    }

    #[test]
    fn test_flush_produces_segment() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 64, 0);
        let frame = vec![1.0f32; 64];
        let mut seg = Vec::new();

        comp.push_frame(&frame, 0, &mut seg);
        comp.flush(&mut seg);

        assert!(!seg.is_empty());
        let mut decoded = Vec::new();
        segment::decode(&seg, &mut decoded);
        assert_eq!(decoded.len(), 64);
    }

    #[test]
    fn test_tier_transition_flushes() {
        let policy = TierPolicy {
            hot_min_score: 512,
            warm_min_score: 64,
            warm_bits: 7,
            drift_pct_q8: 26,
            group_len: 64,
        };

        let mut comp = TemporalTensorCompressor::new(policy, 64, 0);
        comp.set_access(100, 0); // Hot
        let frame = vec![1.0f32; 64];
        let mut seg = Vec::new();

        comp.push_frame(&frame, 1, &mut seg);
        assert_eq!(comp.active_bits(), 8);

        // Make it cold
        comp.set_access(1, 0);
        comp.push_frame(&frame, 10000, &mut seg);
        assert!(!seg.is_empty());
        assert_eq!(comp.active_bits(), 3);
    }

    #[test]
    fn test_drift_triggers_new_segment() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 64, 0);
        let mut seg = Vec::new();

        let frame1 = vec![1.0f32; 64];
        comp.push_frame(&frame1, 0, &mut seg);

        let frame2 = vec![5.0f32; 64];
        comp.push_frame(&frame2, 0, &mut seg);

        assert!(!seg.is_empty());
    }

    #[test]
    fn test_multi_frame_same_segment() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 64, 0);
        let mut seg = Vec::new();

        let frame = vec![1.0f32; 64];
        comp.push_frame(&frame, 0, &mut seg);
        assert!(seg.is_empty());

        let frame2 = vec![1.05f32; 64];
        comp.push_frame(&frame2, 0, &mut seg);
        assert!(seg.is_empty());
        assert_eq!(comp.active_frame_count(), 2);
    }

    #[test]
    fn test_full_roundtrip_hot() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 128, 0);
        comp.set_access(100, 0);
        let frame: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) * 0.01).collect();
        let mut seg = Vec::new();

        for _ in 0..10 {
            comp.push_frame(&frame, 1, &mut seg);
        }
        comp.flush(&mut seg);

        let mut decoded = Vec::new();
        segment::decode(&seg, &mut decoded);
        assert_eq!(decoded.len(), 128 * 10);

        let max_abs = frame.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for i in 0..128 {
            let err = (decoded[i] - frame[i]).abs();
            assert!(
                err < max_abs * 0.02,
                "i={i} orig={} dec={} err={err}",
                frame[i],
                decoded[i]
            );
        }
    }

    #[test]
    fn test_full_roundtrip_cold() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 64, 0);
        // Default: access_count=0, cold -> 3-bit
        let frame: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.1).collect();
        let mut seg = Vec::new();

        comp.push_frame(&frame, 0, &mut seg);
        comp.flush(&mut seg);

        let header = segment::parse_header(&seg).unwrap();
        assert_eq!(header.bits, 3);

        let mut decoded = Vec::new();
        segment::decode(&seg, &mut decoded);
        assert_eq!(decoded.len(), 64);

        let max_abs = frame.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (i, (&orig, &dec)) in frame.iter().zip(decoded.iter()).enumerate() {
            let err = (orig - dec).abs();
            // 3-bit: qmax=3, max relative error ~33%
            assert!(err < max_abs * 0.4, "i={i} orig={orig} dec={dec} err={err}");
        }
    }

    #[test]
    fn test_wrong_length_frame_rejected() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 64, 0);
        let frame = vec![1.0f32; 32];
        let mut seg = Vec::new();
        comp.push_frame(&frame, 0, &mut seg);
        assert_eq!(comp.active_frame_count(), 0);
    }

    #[test]
    fn test_accessor_methods() {
        let policy = TierPolicy::default();
        let comp = TemporalTensorCompressor::new(policy, 256, 42);
        assert_eq!(comp.len(), 256);
        assert_eq!(comp.active_frame_count(), 0);
        assert_eq!(comp.active_data_bytes(), 0);
        assert_eq!(comp.policy().group_len, 64);
    }

    #[test]
    fn test_large_tensor_multi_group() {
        let mut comp = TemporalTensorCompressor::new(default_policy(), 512, 0);
        comp.set_access(100, 0); // hot -> 8-bit
        let frame: Vec<f32> = (0..512).map(|i| ((i as f32) * 0.731).sin()).collect();
        let mut seg = Vec::new();

        for _ in 0..50 {
            comp.push_frame(&frame, 1, &mut seg);
        }
        comp.flush(&mut seg);

        let header = segment::parse_header(&seg).unwrap();
        assert_eq!(header.bits, 8);
        assert_eq!(header.tensor_len, 512);
        assert_eq!(header.frame_count, 50);
        assert_eq!(header.scale_count, 8); // 512/64 = 8 groups

        let mut decoded = Vec::new();
        segment::decode(&seg, &mut decoded);
        assert_eq!(decoded.len(), 512 * 50);

        // Verify compression ratio
        let raw = 512 * 4 * 50;
        let compressed = seg.len();
        let ratio = raw as f32 / compressed as f32;
        assert!(ratio > 3.5, "ratio={ratio:.2}x, expected >3.5x");
    }
}
