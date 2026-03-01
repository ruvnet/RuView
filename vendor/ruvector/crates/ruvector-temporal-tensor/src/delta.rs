//! Delta compression, delta chains, and reconstruction policies (ADR-021).
//!
//! Sparse delta encoding for incremental tensor updates, bounded-depth delta
//! chain management with automatic compaction, and SVD-based low-rank factor
//! reconstruction. All structures are WASM-safe (no `f64` in hot paths).

use crate::store::StoreError;

#[allow(unused_imports)]
use crate::store::{BlockKey, ReconstructPolicy};

/// Size of the fixed portion of a serialized delta (header + scale).
const DELTA_HEADER_BYTES: usize = 34;
/// Size of a single serialized sparse entry (index: u16 + value: i16).
const DELTA_ENTRY_BYTES: usize = 4;
/// Maximum power-iteration steps per singular component.
const POWER_ITER_MAX: usize = 30;
/// Convergence threshold for power iteration.
const POWER_ITER_EPS: f32 = 1e-10;

/// Header for a delta record.
#[derive(Clone, Debug)]
pub struct DeltaHeader {
    pub tensor_id: u128,
    pub block_index: u32,
    pub base_epoch: u64,
    pub nnz: u16,
}

/// A single sparse delta entry: index + quantized value.
#[derive(Clone, Copy, Debug)]
pub struct SparseEntry {
    pub index: u16,
    pub value: i16,
}

/// Complete delta record: header + sparse entries + scale.
///
/// Actual diff = `entry.value as f32 * delta_scale`.
#[derive(Clone, Debug)]
pub struct DeltaRecord {
    pub header: DeltaHeader,
    pub delta_scale: f32,
    pub entries: Vec<SparseEntry>,
}

/// Compute a sparse delta between `old` and `new` data.
///
/// Keeps entries whose absolute change exceeds `threshold`. Returns `None`
/// if the changed fraction meets or exceeds `max_change_fraction`.
///
/// # Panics
///
/// Panics if `old.len() != new.len()`.
pub fn compute_delta(
    old: &[f32],
    new: &[f32],
    tensor_id: u128,
    block_index: u32,
    base_epoch: u64,
    threshold: f32,
    max_change_fraction: f32,
) -> Option<DeltaRecord> {
    assert_eq!(old.len(), new.len(), "old and new must have equal length");
    let n = old.len();
    if n == 0 {
        return Some(DeltaRecord {
            header: DeltaHeader {
                tensor_id,
                block_index,
                base_epoch,
                nnz: 0,
            },
            delta_scale: 0.0,
            entries: Vec::new(),
        });
    }

    let mut changed: Vec<(u16, f32)> = Vec::new();
    let mut max_abs = 0.0f32;
    for i in 0..n {
        let diff = new[i] - old[i];
        if diff.abs() >= threshold {
            changed.push((i as u16, diff));
            if diff.abs() > max_abs {
                max_abs = diff.abs();
            }
        }
    }

    if changed.len() as f32 / n as f32 >= max_change_fraction {
        return None;
    }

    let delta_scale = if max_abs == 0.0 {
        1.0
    } else {
        max_abs / i16::MAX as f32
    };
    let inv_scale = 1.0 / delta_scale;
    let entries: Vec<SparseEntry> = changed
        .iter()
        .map(|&(idx, diff)| {
            let q = (diff * inv_scale).round() as i32;
            SparseEntry {
                index: idx,
                value: q.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            }
        })
        .collect();

    Some(DeltaRecord {
        header: DeltaHeader {
            tensor_id,
            block_index,
            base_epoch,
            nnz: entries.len() as u16,
        },
        delta_scale,
        entries,
    })
}

/// Apply a delta to a base data vector in-place.
///
/// Entries whose indices exceed the base length are silently skipped.
pub fn apply_delta(base: &mut [f32], delta: &DeltaRecord) {
    let scale = delta.delta_scale;
    for entry in &delta.entries {
        let idx = entry.index as usize;
        if idx < base.len() {
            base[idx] += entry.value as f32 * scale;
        }
    }
}

/// A chain of deltas applied to a base block.
/// Invariant: `deltas.len() <= max_chain_len`.
#[derive(Clone, Debug)]
pub struct DeltaChain {
    base_data: Vec<f32>,
    deltas: Vec<DeltaRecord>,
    max_chain_len: u8,
}

impl DeltaChain {
    /// Create a new chain with a base block.
    pub fn new(base_data: Vec<f32>, max_chain_len: u8) -> Self {
        Self {
            base_data,
            deltas: Vec::new(),
            max_chain_len,
        }
    }

    /// Append a delta. Returns `Err(StoreError::DeltaChainTooLong)` at max length.
    pub fn append(&mut self, delta: DeltaRecord) -> Result<(), StoreError> {
        if self.deltas.len() >= self.max_chain_len as usize {
            return Err(StoreError::DeltaChainTooLong);
        }
        self.deltas.push(delta);
        Ok(())
    }

    /// Reconstruct the current state by applying all deltas to the base.
    pub fn reconstruct(&self) -> Vec<f32> {
        let mut result = self.base_data.clone();
        for delta in &self.deltas {
            apply_delta(&mut result, delta);
        }
        result
    }

    /// Compact the chain: apply all deltas to base, clear delta list.
    pub fn compact(&mut self) {
        if self.deltas.is_empty() {
            return;
        }
        for delta in &self.deltas {
            apply_delta(&mut self.base_data, delta);
        }
        self.deltas.clear();
    }

    /// Number of deltas in the chain.
    #[inline]
    pub fn chain_len(&self) -> usize {
        self.deltas.len()
    }

    /// Whether the chain needs compaction (at max length).
    #[inline]
    pub fn needs_compaction(&self) -> bool {
        self.deltas.len() >= self.max_chain_len as usize
    }

    /// Total storage bytes: base + serialized size of all deltas.
    pub fn total_bytes(&self) -> usize {
        let base_bytes = self.base_data.len() * 4;
        let delta_bytes: usize = self
            .deltas
            .iter()
            .map(|d| DELTA_HEADER_BYTES + d.entries.len() * DELTA_ENTRY_BYTES)
            .sum();
        base_bytes + delta_bytes
    }
}

/// Low-rank factor representation for reconstruction.
///
/// Stores U (m x k), S (k), V (k x n) such that data ~ U * diag(S) * V.
/// All matrices are row-major.
#[derive(Clone, Debug)]
pub struct FactorSet {
    pub m: usize,
    pub n: usize,
    pub k: usize,
    pub u_data: Vec<f32>, // m * k elements
    pub s_data: Vec<f32>, // k elements
    pub v_data: Vec<f32>, // k * n elements
}

impl FactorSet {
    /// Reconstruct the full data from factors: U * diag(S) * V.
    pub fn reconstruct(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.m * self.n];
        for r in 0..self.k {
            let s_r = self.s_data[r];
            for i in 0..self.m {
                let u_s = self.u_data[i * self.k + r] * s_r;
                let row = i * self.n;
                let v_off = r * self.n;
                for j in 0..self.n {
                    out[row + j] += u_s * self.v_data[v_off + j];
                }
            }
        }
        out
    }

    /// Compute storage size in bytes: (m*k + k + k*n) * 4.
    pub fn storage_bytes(&self) -> usize {
        (self.m * self.k + self.k + self.k * self.n) * 4
    }

    /// Create from a flat data vector using truncated SVD via power iteration.
    ///
    /// Simplified implementation suitable for moderate-sized matrices.
    /// Extracts top-`rank` singular triplets with successive deflation.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != rows * cols`.
    pub fn from_data(data: &[f32], rows: usize, cols: usize, rank: usize) -> Self {
        assert_eq!(
            data.len(),
            rows * cols,
            "data length must equal rows * cols"
        );
        let (m, n) = (rows, cols);
        let k = rank.min(m).min(n);
        let mut work = data.to_vec();
        let mut u_data = vec![0.0f32; m * k];
        let mut s_data = vec![0.0f32; k];
        let mut v_data = vec![0.0f32; k * n];

        for r in 0..k {
            // Deterministic initial vector: Fibonacci-hash sign pattern.
            let inv_sqrt_n = 1.0 / (n as f32).sqrt();
            let mut v = vec![0.0f32; n];
            for j in 0..n {
                let seed = (j as u32)
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add((r as u32).wrapping_mul(0x9E37_79B9));
                v[j] = if seed & 1 == 0 {
                    inv_sqrt_n
                } else {
                    -inv_sqrt_n
                };
            }
            let mut u = vec![0.0f32; m];
            let mut sigma = 0.0f32;

            for _ in 0..POWER_ITER_MAX {
                // u = work * v
                for i in 0..m {
                    let mut acc = 0.0f32;
                    let row = i * n;
                    for j in 0..n {
                        acc += work[row + j] * v[j];
                    }
                    u[i] = acc;
                }
                let su: f32 = u.iter().map(|x| x * x).sum::<f32>().sqrt();
                if su < POWER_ITER_EPS {
                    sigma = 0.0;
                    break;
                }
                let inv = 1.0 / su;
                for x in u.iter_mut() {
                    *x *= inv;
                }

                // v = work^T * u
                for j in 0..n {
                    let mut acc = 0.0f32;
                    for i in 0..m {
                        acc += work[i * n + j] * u[i];
                    }
                    v[j] = acc;
                }
                let sv: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if sv < POWER_ITER_EPS {
                    sigma = su;
                    break;
                }
                sigma = sv;
                let inv = 1.0 / sv;
                for x in v.iter_mut() {
                    *x *= inv;
                }
            }

            s_data[r] = sigma;
            for i in 0..m {
                u_data[i * k + r] = u[i];
            }
            for j in 0..n {
                v_data[r * n + j] = v[j];
            }

            // Deflate: work -= sigma * u * v^T
            if sigma > POWER_ITER_EPS {
                for i in 0..m {
                    let us = u[i] * sigma;
                    let row = i * n;
                    for j in 0..n {
                        work[row + j] -= us * v[j];
                    }
                }
            }
        }
        Self {
            m,
            n,
            k,
            u_data,
            s_data,
            v_data,
        }
    }

    /// Compute the relative reconstruction error (Frobenius norm).
    ///
    /// Returns `||original - reconstructed|| / ||original||`.
    /// Returns 0.0 if the original has zero norm.
    pub fn reconstruction_error(&self, original: &[f32]) -> f32 {
        let reconstructed = self.reconstruct();
        let mut diff_sq = 0.0f32;
        let mut orig_sq = 0.0f32;
        for (i, &o) in original.iter().enumerate() {
            let r = if i < reconstructed.len() {
                reconstructed[i]
            } else {
                0.0
            };
            diff_sq += (o - r) * (o - r);
            orig_sq += o * o;
        }
        if orig_sq < 1e-30 {
            return 0.0;
        }
        (diff_sq / orig_sq).sqrt()
    }

    /// Estimate the fraction of total energy (Frobenius norm) captured by factors.
    ///
    /// Uses `sum(s_i^2)` as captured energy. Requires the original data to compute
    /// total energy as `||data||_F^2`. Returns 1.0 if total energy is near zero.
    pub fn energy_captured(&self, original: &[f32]) -> f32 {
        let total_energy: f32 = original.iter().map(|x| x * x).sum();
        if total_energy < 1e-30 {
            return 1.0;
        }
        let captured: f32 = self.s_data.iter().map(|s| s * s).sum();
        (captured / total_energy).min(1.0)
    }

    /// Compression ratio: original_elements * 4 bytes / storage_bytes.
    ///
    /// Returns 0.0 if storage_bytes is zero.
    pub fn compression_ratio(&self, original_elements: usize) -> f32 {
        let raw = original_elements * 4;
        let stored = self.storage_bytes();
        if stored == 0 {
            return 0.0;
        }
        raw as f32 / stored as f32
    }

    /// Create factors with adaptive rank selection.
    ///
    /// Starts with rank 1 and increases until either `max_rank` is reached or
    /// the reconstruction error falls below `target_error`.
    pub fn from_data_adaptive(
        data: &[f32],
        rows: usize,
        cols: usize,
        max_rank: usize,
        target_error: f32,
    ) -> Self {
        let max_k = max_rank.min(rows).min(cols);
        let mut best = Self::from_data(data, rows, cols, 1);
        for rank in 2..=max_k {
            let err = best.reconstruction_error(data);
            if err <= target_error {
                break;
            }
            best = Self::from_data(data, rows, cols, rank);
        }
        best
    }
}

/// Encode a [`DeltaRecord`] to bytes (little-endian, ADR-021 section 4.1).
pub fn encode_delta(delta: &DeltaRecord) -> Vec<u8> {
    let mut buf = Vec::with_capacity(DELTA_HEADER_BYTES + delta.entries.len() * DELTA_ENTRY_BYTES);
    buf.extend_from_slice(&delta.header.tensor_id.to_le_bytes());
    buf.extend_from_slice(&delta.header.block_index.to_le_bytes());
    buf.extend_from_slice(&delta.header.base_epoch.to_le_bytes());
    buf.extend_from_slice(&delta.header.nnz.to_le_bytes());
    buf.extend_from_slice(&delta.delta_scale.to_le_bytes());
    for entry in &delta.entries {
        buf.extend_from_slice(&entry.index.to_le_bytes());
        buf.extend_from_slice(&entry.value.to_le_bytes());
    }
    buf
}

/// Decode a [`DeltaRecord`] from bytes.
///
/// Returns `Err(StoreError::InvalidBlock)` on truncated or malformed input.
pub fn decode_delta(data: &[u8]) -> Result<DeltaRecord, StoreError> {
    if data.len() < DELTA_HEADER_BYTES {
        return Err(StoreError::InvalidBlock);
    }
    let tensor_id = u128::from_le_bytes(
        data[0..16]
            .try_into()
            .map_err(|_| StoreError::InvalidBlock)?,
    );
    let block_index = u32::from_le_bytes(
        data[16..20]
            .try_into()
            .map_err(|_| StoreError::InvalidBlock)?,
    );
    let base_epoch = u64::from_le_bytes(
        data[20..28]
            .try_into()
            .map_err(|_| StoreError::InvalidBlock)?,
    );
    let nnz = u16::from_le_bytes(
        data[28..30]
            .try_into()
            .map_err(|_| StoreError::InvalidBlock)?,
    );
    let delta_scale = f32::from_le_bytes(
        data[30..34]
            .try_into()
            .map_err(|_| StoreError::InvalidBlock)?,
    );

    if data.len() < DELTA_HEADER_BYTES + (nnz as usize) * DELTA_ENTRY_BYTES {
        return Err(StoreError::InvalidBlock);
    }
    let mut entries = Vec::with_capacity(nnz as usize);
    let mut off = DELTA_HEADER_BYTES;
    for _ in 0..nnz {
        let index = u16::from_le_bytes(
            data[off..off + 2]
                .try_into()
                .map_err(|_| StoreError::InvalidBlock)?,
        );
        let value = i16::from_le_bytes(
            data[off + 2..off + 4]
                .try_into()
                .map_err(|_| StoreError::InvalidBlock)?,
        );
        entries.push(SparseEntry { index, value });
        off += DELTA_ENTRY_BYTES;
    }

    Ok(DeltaRecord {
        header: DeltaHeader {
            tensor_id,
            block_index,
            base_epoch,
            nnz,
        },
        delta_scale,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_delta(entries: Vec<(u16, i16)>, scale: f32) -> DeltaRecord {
        let sparse: Vec<SparseEntry> = entries
            .iter()
            .map(|&(i, v)| SparseEntry { index: i, value: v })
            .collect();
        DeltaRecord {
            header: DeltaHeader {
                tensor_id: 42,
                block_index: 0,
                base_epoch: 1,
                nnz: sparse.len() as u16,
            },
            delta_scale: scale,
            entries: sparse,
        }
    }

    #[test]
    fn test_compute_delta_small_change() {
        let old = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mut new = old.clone();
        new[2] = 3.5;
        let d = compute_delta(&old, &new, 1, 0, 0, 0.01, 0.5).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].index, 2);
        assert!(d.delta_scale > 0.0);
    }

    #[test]
    fn test_compute_delta_large_change_returns_none() {
        let old = vec![1.0; 10];
        let new = vec![5.0; 10];
        assert!(compute_delta(&old, &new, 1, 0, 0, 0.01, 0.5).is_none());
    }

    #[test]
    fn test_apply_delta_modifies_base() {
        let mut base = vec![1.0, 2.0, 3.0, 4.0];
        apply_delta(&mut base, &make_delta(vec![(1, 100), (3, -50)], 0.01));
        assert!((base[0] - 1.0).abs() < 1e-6);
        assert!((base[1] - 3.0).abs() < 1e-6); // 2.0 + 100*0.01
        assert!((base[2] - 3.0).abs() < 1e-6);
        assert!((base[3] - 3.5).abs() < 1e-6); // 4.0 - 50*0.01
    }

    #[test]
    fn test_chain_append_and_reconstruct() {
        let mut chain = DeltaChain::new(vec![1.0, 2.0, 3.0, 4.0], 4);
        chain.append(make_delta(vec![(0, 1000)], 0.001)).unwrap(); // +1.0
        assert_eq!(chain.chain_len(), 1);
        let r = chain.reconstruct();
        assert!((r[0] - 2.0).abs() < 1e-3);
        assert!((r[1] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_chain_compact_preserves_state() {
        let mut chain = DeltaChain::new(vec![0.0; 4], 8);
        chain.append(make_delta(vec![(0, 100)], 0.1)).unwrap(); // +10.0
        chain.append(make_delta(vec![(1, 200)], 0.1)).unwrap(); // +20.0
        let before = chain.reconstruct();
        chain.compact();
        assert_eq!(chain.chain_len(), 0);
        let after = chain.reconstruct();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_chain_max_length_enforcement() {
        let mut chain = DeltaChain::new(vec![1.0; 4], 2);
        assert!(chain.append(make_delta(vec![(0, 1)], 0.1)).is_ok());
        assert!(chain.append(make_delta(vec![(1, 1)], 0.1)).is_ok());
        assert!(chain.append(make_delta(vec![(2, 1)], 0.1)).is_err());
    }

    #[test]
    fn test_chain_needs_compaction() {
        let mut chain = DeltaChain::new(vec![1.0; 4], 2);
        assert!(!chain.needs_compaction());
        chain.append(make_delta(vec![(0, 1)], 0.1)).unwrap();
        assert!(!chain.needs_compaction());
        chain.append(make_delta(vec![(1, 1)], 0.1)).unwrap();
        assert!(chain.needs_compaction());
    }

    #[test]
    fn test_factor_reconstruct() {
        let (u, v, s) = (vec![1.0, 2.0, 3.0], vec![4.0, 5.0], 2.0);
        let f = FactorSet {
            m: 3,
            n: 2,
            k: 1,
            u_data: u.clone(),
            s_data: vec![s],
            v_data: v.clone(),
        };
        let r = f.reconstruct();
        assert_eq!(r.len(), 6);
        for i in 0..3 {
            for j in 0..2 {
                assert!((r[i * 2 + j] - u[i] * s * v[j]).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_factor_from_data_approximation() {
        let (m, n) = (8, 6);
        let data: Vec<f32> = (0..m * n)
            .map(|idx| {
                let (i, j) = (idx / n, idx % n);
                (i as f32 + 1.0) * (j as f32 + 1.0)
            })
            .collect();
        let reconstructed = FactorSet::from_data(&data, m, n, 1).reconstruct();
        let max_err = data
            .iter()
            .zip(reconstructed.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(
            max_err < 0.5,
            "max error {max_err} too large for rank-1 input"
        );
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let orig = DeltaRecord {
            header: DeltaHeader {
                tensor_id: 0xDEADBEEFCAFEBABE,
                block_index: 42,
                base_epoch: 100,
                nnz: 3,
            },
            delta_scale: 0.001,
            entries: vec![
                SparseEntry {
                    index: 10,
                    value: 500,
                },
                SparseEntry {
                    index: 20,
                    value: -300,
                },
                SparseEntry {
                    index: 30,
                    value: 1,
                },
            ],
        };
        let bytes = encode_delta(&orig);
        assert_eq!(bytes.len(), DELTA_HEADER_BYTES + 3 * DELTA_ENTRY_BYTES);
        let dec = decode_delta(&bytes).unwrap();
        assert_eq!(dec.header.tensor_id, orig.header.tensor_id);
        assert_eq!(dec.header.block_index, orig.header.block_index);
        assert_eq!(dec.header.nnz, orig.header.nnz);
        assert!((dec.delta_scale - orig.delta_scale).abs() < 1e-10);
        for (a, b) in dec.entries.iter().zip(orig.entries.iter()) {
            assert_eq!(a.index, b.index);
            assert_eq!(a.value, b.value);
        }
    }

    #[test]
    fn test_decode_truncated_header() {
        assert!(decode_delta(&vec![0u8; 20]).is_err());
    }

    #[test]
    fn test_decode_truncated_entries() {
        let mut bytes = encode_delta(&make_delta(vec![(0, 1), (1, 2)], 1.0));
        bytes[28] = 5;
        bytes[29] = 0; // claim 5 entries, only 2 present
        assert!(decode_delta(&bytes).is_err());
    }

    #[test]
    fn test_empty_delta_roundtrip() {
        let d = DeltaRecord {
            header: DeltaHeader {
                tensor_id: 99,
                block_index: 7,
                base_epoch: 50,
                nnz: 0,
            },
            delta_scale: 0.0,
            entries: Vec::new(),
        };
        let dec = decode_delta(&encode_delta(&d)).unwrap();
        assert_eq!(dec.entries.len(), 0);
    }

    #[test]
    fn test_single_entry_delta() {
        let old = vec![1.0; 100];
        let mut new = old.clone();
        new[50] = 2.0;
        let d = compute_delta(&old, &new, 1, 0, 0, 0.01, 0.5).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].index, 50);
        let mut base = old.clone();
        apply_delta(&mut base, &d);
        assert!((base[50] - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_full_density_delta() {
        let old = vec![0.0; 4];
        let new = vec![0.1, 0.2, 0.3, 0.4];
        let d = compute_delta(&old, &new, 1, 0, 0, 0.001, 1.1).unwrap();
        assert_eq!(d.entries.len(), 4);
        let mut base = old.clone();
        apply_delta(&mut base, &d);
        for i in 0..4 {
            assert!((base[i] - new[i]).abs() < 0.01, "index {i}");
        }
    }

    #[test]
    fn test_compute_apply_roundtrip_64() {
        let old: Vec<f32> = (0..64).map(|i| i as f32 * 0.1).collect();
        let mut new = old.clone();
        new[5] += 0.5;
        new[10] -= 0.3;
        new[60] += 1.0;
        let d = compute_delta(&old, &new, 1, 0, 0, 0.01, 0.5).unwrap();
        let mut recon = old.clone();
        apply_delta(&mut recon, &d);
        for i in 0..64 {
            assert!((recon[i] - new[i]).abs() < 0.01, "index {i}");
        }
    }

    #[test]
    fn test_reconstruction_error_zero_for_exact() {
        // Rank-1 data should be exactly reconstructed with rank-1 factors
        let (m, n) = (4, 3);
        let data: Vec<f32> = (0..m * n)
            .map(|idx| {
                let (i, j) = (idx / n, idx % n);
                (i as f32 + 1.0) * (j as f32 + 1.0)
            })
            .collect();
        let factors = FactorSet::from_data(&data, m, n, 1);
        let err = factors.reconstruction_error(&data);
        assert!(err < 0.01, "err={err} too large for rank-1 data");
    }

    #[test]
    fn test_reconstruction_error_decreases_with_rank() {
        let (m, n) = (8, 6);
        let data: Vec<f32> = (0..m * n).map(|i| (i as f32 * 0.7).sin()).collect();
        let err1 = FactorSet::from_data(&data, m, n, 1).reconstruction_error(&data);
        let err3 = FactorSet::from_data(&data, m, n, 3).reconstruction_error(&data);
        assert!(err3 <= err1 + 1e-6, "err3={err3} > err1={err1}");
    }

    #[test]
    fn test_energy_captured_rank1_data() {
        let (m, n) = (4, 3);
        let data: Vec<f32> = (0..m * n)
            .map(|idx| {
                let (i, j) = (idx / n, idx % n);
                (i as f32 + 1.0) * (j as f32 + 1.0)
            })
            .collect();
        let factors = FactorSet::from_data(&data, m, n, 1);
        let energy = factors.energy_captured(&data);
        assert!(energy > 0.95, "energy={energy} too low for rank-1 data");
    }

    #[test]
    fn test_compression_ratio_meaningful() {
        let (m, n) = (16, 16);
        let data: Vec<f32> = (0..m * n).map(|i| i as f32).collect();
        let factors = FactorSet::from_data(&data, m, n, 2);
        let ratio = factors.compression_ratio(m * n);
        // rank-2 storage: (16*2 + 2 + 2*16) * 4 = 264 bytes vs 16*16*4 = 1024 bytes
        assert!(ratio > 1.0, "ratio={ratio} should be > 1");
    }

    #[test]
    fn test_from_data_adaptive_stops_early() {
        let (m, n) = (4, 3);
        // Rank-1 data: adaptive should stop at rank 1
        let data: Vec<f32> = (0..m * n)
            .map(|idx| {
                let (i, j) = (idx / n, idx % n);
                (i as f32 + 1.0) * (j as f32 + 1.0)
            })
            .collect();
        let factors = FactorSet::from_data_adaptive(&data, m, n, 5, 0.05);
        // Should use rank 1 since data is rank 1
        assert!(
            factors.k <= 2,
            "k={} should be small for rank-1 data",
            factors.k
        );
    }

    #[test]
    fn test_from_data_adaptive_increases_rank() {
        let (m, n) = (8, 6);
        // Multi-rank data
        let data: Vec<f32> = (0..m * n)
            .map(|i| (i as f32 * 0.3).sin() + (i as f32 * 0.7).cos())
            .collect();
        let factors = FactorSet::from_data_adaptive(&data, m, n, 6, 0.01);
        let err = factors.reconstruction_error(&data);
        // Should achieve close to target error or use max rank
        assert!(err < 0.1 || factors.k == 6, "err={err}, k={}", factors.k);
    }
}
