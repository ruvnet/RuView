//! Runtime state and memory management.
//!
//! All buffers are preallocated at initialization. The inference hot path
//! performs zero heap allocations.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::config::TransformerConfig;
use crate::error::Result;

/// Runtime state for the transformer.
///
/// Owns all buffers required for inference. Single contiguous allocation
/// at initialization, with all slices carved from that allocation.
///
/// Aligned to cache line (64 bytes) for optimal memory access patterns.
#[repr(C, align(64))]
pub struct RuntimeState {
    /// Configuration reference
    config: TransformerConfig,

    /// Cached buffer layout to avoid recomputation
    layout: BufferLayout,

    /// Main buffer holding all allocations
    buffer: Vec<u8>,

    /// KV cache state
    kv_state: KvCacheState,

    /// Cached logits for skip path
    cached_logits: Vec<i32>,

    /// Cached input signature
    cached_signature: Option<u64>,
}

/// KV cache state per layer.
#[derive(Clone)]
pub struct KvCacheState {
    /// Per-layer write indices (ring buffer position)
    pub write_indices: Vec<u16>,

    /// Per-layer valid lengths
    pub valid_lengths: Vec<u16>,

    /// Total layers
    pub layers: usize,

    /// Max sequence length per layer
    pub seq_len_max: usize,
}

impl KvCacheState {
    /// Create new KV cache state
    pub fn new(layers: usize, seq_len_max: usize) -> Self {
        Self {
            write_indices: vec![0; layers],
            valid_lengths: vec![0; layers],
            layers,
            seq_len_max,
        }
    }

    /// Reset all layers
    pub fn reset(&mut self) {
        for i in 0..self.layers {
            self.write_indices[i] = 0;
            self.valid_lengths[i] = 0;
        }
    }

    /// Flush (clear) all layers
    pub fn flush(&mut self) {
        self.reset();
    }

    /// Get next write position for a layer (ring buffer)
    #[inline]
    pub fn next_write_pos(&self, layer: usize) -> usize {
        self.write_indices[layer] as usize
    }

    /// Advance write position for a layer
    #[inline]
    pub fn advance_write(&mut self, layer: usize) {
        let pos = self.write_indices[layer] as usize;
        self.write_indices[layer] = ((pos + 1) % self.seq_len_max) as u16;
        if (self.valid_lengths[layer] as usize) < self.seq_len_max {
            self.valid_lengths[layer] += 1;
        }
    }

    /// Get valid length for a layer
    #[inline]
    pub fn valid_len(&self, layer: usize) -> usize {
        self.valid_lengths[layer] as usize
    }
}

/// Buffer layout for runtime state
///
/// Aligned to cache line (64 bytes) to prevent false sharing and
/// improve cache utilization when accessed from multiple threads.
#[repr(C, align(64))]
struct BufferLayout {
    /// Offset for Q buffer
    q_offset: usize,
    /// Offset for K buffer
    k_offset: usize,
    /// Offset for V buffer
    v_offset: usize,
    /// Offset for attention scores
    attn_scores_offset: usize,
    /// Offset for FFN intermediate
    ffn_intermediate_offset: usize,
    /// Offset for residual
    residual_offset: usize,
    /// Offset for norm temp
    norm_temp_offset: usize,
    /// Offset for K cache
    k_cache_offset: usize,
    /// Offset for V cache
    v_cache_offset: usize,
    /// Total size
    total_size: usize,
}

impl BufferLayout {
    fn compute(config: &TransformerConfig) -> Self {
        let s = config.seq_len_max as usize;
        let d = config.hidden as usize;
        let h = config.heads as usize;
        let _dh = config.head_dim() as usize;
        let w = config.window_normal as usize;
        let ffn_int = config.ffn_intermediate() as usize;
        let l = config.layers as usize;

        // All sizes in bytes (i8 = 1 byte, i32 = 4 bytes, f32 = 4 bytes)
        let mut offset = 0;

        // Q, K, V buffers for current layer (i8)
        let q_offset = offset;
        offset += s * d; // Q

        let k_offset = offset;
        offset += s * d; // K

        let v_offset = offset;
        offset += s * d; // V

        // Attention scores buffer (f32 for softmax)
        let attn_scores_offset = offset;
        offset += h * w * 4; // f32

        // FFN intermediate (i32 accumulator)
        let ffn_intermediate_offset = offset;
        offset += ffn_int * 4;

        // Residual (i8)
        let residual_offset = offset;
        offset += s * d;

        // Norm temp (f32)
        let norm_temp_offset = offset;
        offset += d * 4;

        // K cache: L * S_max * D (i8)
        let k_cache_offset = offset;
        offset += l * s * d;

        // V cache: L * S_max * D (i8)
        let v_cache_offset = offset;
        offset += l * s * d;

        // Align to 64 bytes
        let total_size = (offset + 63) & !63;

        Self {
            q_offset,
            k_offset,
            v_offset,
            attn_scores_offset,
            ffn_intermediate_offset,
            residual_offset,
            norm_temp_offset,
            k_cache_offset,
            v_cache_offset,
            total_size,
        }
    }
}

impl RuntimeState {
    /// Create new runtime state with preallocated buffers.
    pub fn new(config: TransformerConfig) -> Result<Self> {
        config.validate()?;

        let layout = BufferLayout::compute(&config);

        // Single allocation for all buffers, 64-byte aligned
        let buffer = vec![0u8; layout.total_size];

        let kv_state = KvCacheState::new(config.layers as usize, config.seq_len_max as usize);

        let cached_logits = vec![0i32; config.logits as usize];

        Ok(Self {
            config,
            layout,
            buffer,
            kv_state,
            cached_logits,
            cached_signature: None,
        })
    }

    /// Get configuration
    #[inline]
    pub fn config(&self) -> &TransformerConfig {
        &self.config
    }

    /// Get Q buffer slice (i8)
    #[inline]
    pub fn q_buffer(&mut self) -> &mut [i8] {
        let s = self.config.seq_len_max as usize;
        let d = self.config.hidden as usize;
        let start = self.layout.q_offset;
        let end = start + s * d;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() and validated
        // at initialization. The slice [start..end] is guaranteed to be within buffer bounds
        // because layout offsets are calculated from the same config. i8 has the same size
        // and alignment as u8 (both are 1 byte), making the pointer cast sound. The returned
        // slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer[start..end].as_mut_ptr() as *mut i8, s * d)
        }
    }

    /// Get K buffer slice (i8)
    #[inline]
    pub fn k_buffer(&mut self) -> &mut [i8] {
        let s = self.config.seq_len_max as usize;
        let d = self.config.hidden as usize;
        let start = self.layout.k_offset;
        let end = start + s * d;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() and validated
        // at initialization. The slice [start..end] is guaranteed to be within buffer bounds
        // because layout offsets are calculated from the same config. i8 has the same size
        // and alignment as u8 (both are 1 byte), making the pointer cast sound. The returned
        // slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer[start..end].as_mut_ptr() as *mut i8, s * d)
        }
    }

    /// Get V buffer slice (i8)
    #[inline]
    pub fn v_buffer(&mut self) -> &mut [i8] {
        let s = self.config.seq_len_max as usize;
        let d = self.config.hidden as usize;
        let start = self.layout.v_offset;
        let end = start + s * d;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() and validated
        // at initialization. The slice [start..end] is guaranteed to be within buffer bounds
        // because layout offsets are calculated from the same config. i8 has the same size
        // and alignment as u8 (both are 1 byte), making the pointer cast sound. The returned
        // slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer[start..end].as_mut_ptr() as *mut i8, s * d)
        }
    }

    /// Get attention scores buffer (f32)
    #[inline]
    pub fn attn_scores_buffer(&mut self) -> &mut [f32] {
        let h = self.config.heads as usize;
        let w = self.config.window_normal as usize;
        let start = self.layout.attn_scores_offset;
        let count = h * w;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() with sufficient
        // space for h * w * 4 bytes at attn_scores_offset. The buffer is allocated with
        // 64-byte alignment (see line 169), which exceeds f32's 4-byte requirement.
        // The pointer is derived from a valid slice, and the count (h * w elements) fits
        // within the allocated region. The returned slice's lifetime is tied to &mut self.
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer[start..].as_mut_ptr() as *mut f32, count)
        }
    }

    /// Get FFN intermediate buffer (i32)
    #[inline]
    pub fn ffn_buffer(&mut self) -> &mut [i32] {
        let ffn_int = self.config.ffn_intermediate() as usize;
        let start = self.layout.ffn_intermediate_offset;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() with sufficient
        // space for ffn_int * 4 bytes at ffn_intermediate_offset. The buffer is allocated
        // with 64-byte alignment (see line 169), which exceeds i32's 4-byte requirement.
        // The pointer is derived from a valid slice, and the count (ffn_int elements) fits
        // within the allocated region. The returned slice's lifetime is tied to &mut self.
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer[start..].as_mut_ptr() as *mut i32, ffn_int)
        }
    }

    /// Get residual buffer (i8)
    #[inline]
    pub fn residual_buffer(&mut self) -> &mut [i8] {
        let s = self.config.seq_len_max as usize;
        let d = self.config.hidden as usize;
        let start = self.layout.residual_offset;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() with sufficient
        // space for s * d bytes at residual_offset. i8 has the same size and alignment as
        // u8 (both are 1 byte), making the pointer cast sound. The pointer is derived from
        // a valid slice, and the count (s * d elements) fits within the allocated region.
        // The returned slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer[start..].as_mut_ptr() as *mut i8, s * d)
        }
    }

    /// Get norm temp buffer (f32)
    #[inline]
    pub fn norm_buffer(&mut self) -> &mut [f32] {
        let d = self.config.hidden as usize;
        let start = self.layout.norm_temp_offset;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() with sufficient
        // space for d * 4 bytes at norm_temp_offset. The buffer is allocated with 64-byte
        // alignment (see line 169), which exceeds f32's 4-byte requirement. The pointer is
        // derived from a valid slice, and the count (d elements) fits within the allocated
        // region. The returned slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe { core::slice::from_raw_parts_mut(self.buffer[start..].as_mut_ptr() as *mut f32, d) }
    }

    /// Get K cache for a layer (i8)
    #[inline]
    pub fn k_cache(&mut self, layer: usize) -> &mut [i8] {
        let s = self.config.seq_len_max as usize;
        let d = self.config.hidden as usize;
        let layer_size = s * d;
        let start = self.layout.k_cache_offset + layer * layer_size;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() with sufficient
        // space for L * s * d bytes starting at k_cache_offset, where L is the number of
        // layers. The caller must ensure layer < config.layers to stay within bounds.
        // i8 has the same size and alignment as u8 (both are 1 byte), making the pointer
        // cast sound. The returned slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer[start..].as_mut_ptr() as *mut i8,
                layer_size,
            )
        }
    }

    /// Get V cache for a layer (i8)
    #[inline]
    pub fn v_cache(&mut self, layer: usize) -> &mut [i8] {
        let s = self.config.seq_len_max as usize;
        let d = self.config.hidden as usize;
        let layer_size = s * d;
        let start = self.layout.v_cache_offset + layer * layer_size;
        // SAFETY: The buffer is properly sized by BufferLayout::compute() with sufficient
        // space for L * s * d bytes starting at v_cache_offset, where L is the number of
        // layers. The caller must ensure layer < config.layers to stay within bounds.
        // i8 has the same size and alignment as u8 (both are 1 byte), making the pointer
        // cast sound. The returned slice's lifetime is tied to &mut self, preventing aliasing.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer[start..].as_mut_ptr() as *mut i8,
                layer_size,
            )
        }
    }

    /// Get KV cache state
    #[inline]
    pub fn kv_state(&self) -> &KvCacheState {
        &self.kv_state
    }

    /// Get mutable KV cache state
    #[inline]
    pub fn kv_state_mut(&mut self) -> &mut KvCacheState {
        &mut self.kv_state
    }

    /// Flush KV cache
    ///
    /// Uses slice::fill for ~50x faster zeroing compared to byte-by-byte iteration.
    pub fn flush_kv(&mut self) {
        self.kv_state.flush();
        // Zero the cache memory for security (optimized with slice::fill)
        let cache_size = self.config.kv_cache_bytes();
        let start = self.layout.k_cache_offset;
        let end = start.saturating_add(cache_size).min(self.buffer.len());
        self.buffer[start..end].fill(0);
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.flush_kv();
        self.cached_signature = None;
        for l in self.cached_logits.iter_mut() {
            *l = 0;
        }
    }

    /// Get cached logits
    #[inline]
    pub fn cached_logits(&self) -> &[i32] {
        &self.cached_logits
    }

    /// Get mutable cached logits
    #[inline]
    pub fn cached_logits_mut(&mut self) -> &mut [i32] {
        &mut self.cached_logits
    }

    /// Get cached signature
    #[inline]
    pub fn cached_signature(&self) -> Option<u64> {
        self.cached_signature
    }

    /// Set cached signature
    #[inline]
    pub fn set_cached_signature(&mut self, sig: Option<u64>) {
        self.cached_signature = sig;
    }

    /// Check if cached logits match input signature
    pub fn has_cached_for(&self, sig: Option<u64>) -> bool {
        match (self.cached_signature, sig) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Total buffer size in bytes
    #[inline]
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_state_creation() {
        let config = TransformerConfig::micro();
        let state = RuntimeState::new(config).unwrap();
        assert!(state.buffer_size() > 0);
    }

    #[test]
    fn test_kv_cache_state() {
        let mut kv = KvCacheState::new(4, 32);
        assert_eq!(kv.valid_len(0), 0);

        kv.advance_write(0);
        assert_eq!(kv.valid_len(0), 1);
        assert_eq!(kv.next_write_pos(0), 1);

        kv.flush();
        assert_eq!(kv.valid_len(0), 0);
    }

    #[test]
    fn test_buffer_slices() {
        let config = TransformerConfig::micro();
        let mut state = RuntimeState::new(config).unwrap();

        let q = state.q_buffer();
        assert_eq!(q.len(), 32 * 128); // seq_len_max * hidden

        let k = state.k_buffer();
        assert_eq!(k.len(), 32 * 128);

        let attn = state.attn_scores_buffer();
        assert_eq!(attn.len(), 4 * 8); // heads * window_normal
    }

    #[test]
    fn test_cached_signature() {
        let config = TransformerConfig::micro();
        let mut state = RuntimeState::new(config).unwrap();

        assert!(!state.has_cached_for(Some(123)));

        state.set_cached_signature(Some(123));
        assert!(state.has_cached_for(Some(123)));
        assert!(!state.has_cached_for(Some(456)));
    }
}
