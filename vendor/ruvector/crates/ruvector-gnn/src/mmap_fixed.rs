//! Memory-mapped embedding management for large-scale GNN training.
//!
//! This module provides efficient memory-mapped access to embeddings and gradients
//! that don't fit in RAM. It includes:
//! - `MmapManager`: Memory-mapped embedding storage with dirty tracking
//! - `MmapGradientAccumulator`: Lock-free gradient accumulation
//! - `AtomicBitmap`: Thread-safe bitmap for access/dirty tracking
//!
//! Only available on non-WASM targets.

#![cfg(all(not(target_arch = "wasm32"), feature = "mmap"))]

use crate::error::{GnnError, Result};
use std::cell::UnsafeCell;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use parking_lot::RwLock;
use memmap2::{MmapMut, MmapOptions};

/// Thread-safe bitmap using atomic operations.
#[derive(Debug)]
pub struct AtomicBitmap {
    bits: Vec<AtomicU64>,
    size: usize,
}

impl AtomicBitmap {
    pub fn new(size: usize) -> Self {
        let num_words = (size + 63) / 64;
        let bits = (0..num_words).map(|_| AtomicU64::new(0)).collect();
        Self { bits, size }
    }

    pub fn set(&self, index: usize) {
        if index >= self.size {
            return;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        self.bits[word_idx].fetch_or(1u64 << bit_idx, Ordering::Release);
    }

    pub fn clear(&self, index: usize) {
        if index >= self.size {
            return;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        self.bits[word_idx].fetch_and(!(1u64 << bit_idx), Ordering::Release);
    }

    pub fn get(&self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        let word = self.bits[word_idx].load(Ordering::Acquire);
        (word & (1u64 << bit_idx)) != 0
    }

    pub fn clear_all(&self) {
        for word in &self.bits {
            word.store(0, Ordering::Release);
        }
    }

    pub fn get_set_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        for (word_idx, word) in self.bits.iter().enumerate() {
            let mut w = word.load(Ordering::Acquire);
            while w != 0 {
                let bit_idx = w.trailing_zeros() as usize;
                indices.push(word_idx * 64 + bit_idx);
                w &= w - 1;
            }
        }
        indices
    }
}
