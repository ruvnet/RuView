// Rolling frame buffer for the temporal head input window (ADR-095 §3.2).
//
// The hot path (`ruv_temporal_push`) writes one frame per call. The
// buffer is sized at `init` time; pushes wrap. `classify` reads the
// most-recent `window_len` frames in chronological order, oldest-first.
//
// Allocation policy: one `Vec<f32>` of size `window_len * input_dim`,
// owned by the context. No per-push allocation — we just memcpy into
// the next slot.

use alloc::vec;
use alloc::vec::Vec;

pub struct FrameRing {
    buf: Vec<f32>,
    window_len: usize,
    input_dim: usize,
    next_write: usize,
    filled: usize,
}

impl FrameRing {
    pub fn new(window_len: usize, input_dim: usize) -> Option<Self> {
        if window_len == 0 || input_dim == 0 {
            return None;
        }
        let total = window_len.checked_mul(input_dim)?;
        Some(Self {
            buf: vec![0.0; total],
            window_len,
            input_dim,
            next_write: 0,
            filled: 0,
        })
    }

    pub fn push(&mut self, frame: &[f32]) {
        let n = core::cmp::min(frame.len(), self.input_dim);
        let off = self.next_write * self.input_dim;
        self.buf[off..off + n].copy_from_slice(&frame[..n]);
        // Zero-pad tail when the caller's frame is shorter than input_dim.
        for s in &mut self.buf[off + n..off + self.input_dim] {
            *s = 0.0;
        }
        self.next_write = (self.next_write + 1) % self.window_len;
        if self.filled < self.window_len {
            self.filled += 1;
        }
    }

    /// Iterate over the buffer in chronological order, oldest-first.
    /// Yields one slice of `input_dim` floats per call. Used by
    /// `ruv_temporal_classify` to flatten into the kernel input.
    pub fn iter_chronological(&self) -> impl Iterator<Item = &[f32]> + '_ {
        let start = if self.filled < self.window_len {
            0
        } else {
            self.next_write
        };
        (0..self.filled).map(move |i| {
            let row = (start + i) % self.window_len;
            let off = row * self.input_dim;
            &self.buf[off..off + self.input_dim]
        })
    }

    pub fn len(&self) -> usize {
        self.filled
    }

    pub fn capacity(&self) -> usize {
        self.window_len
    }
}
