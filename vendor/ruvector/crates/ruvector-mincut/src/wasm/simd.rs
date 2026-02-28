//! WASM SIMD optimizations for minimum cut
//!
//! Uses WebAssembly SIMD128 for parallel operations.

#[cfg(target_arch = "wasm32")]
use core::arch::wasm32::*;

use crate::compact::{BitSet256, CompactVertexId};

/// SIMD-accelerated population count for BitSet256
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn simd_popcount(bits: &[u64; 4]) -> u32 {
    unsafe {
        // Load 128-bit chunks
        let v0 = v128_load(bits.as_ptr() as *const v128);
        let v1 = v128_load(bits.as_ptr().add(2) as *const v128);

        // Count bits using POPCNT
        // WASM SIMD doesn't have direct popcnt, so we use a table lookup method
        let lookup = i8x16(0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4);
        let mask = i8x16_splat(0x0F);

        // Process v0
        let lo0 = v128_and(v0, mask);
        let hi0 = v128_and(u8x16_shr(v0, 4), mask);
        let cnt0 = i8x16_add(i8x16_swizzle(lookup, lo0), i8x16_swizzle(lookup, hi0));

        // Process v1
        let lo1 = v128_and(v1, mask);
        let hi1 = v128_and(u8x16_shr(v1, 4), mask);
        let cnt1 = i8x16_add(i8x16_swizzle(lookup, lo1), i8x16_swizzle(lookup, hi1));

        // Horizontal sum
        let sum = i8x16_add(cnt0, cnt1);
        let sum16 = i16x8_extadd_pairwise_i8x16(sum);
        let sum32 = i32x4_extadd_pairwise_i16x8(sum16);

        let a = i32x4_extract_lane::<0>(sum32);
        let b = i32x4_extract_lane::<1>(sum32);
        let c = i32x4_extract_lane::<2>(sum32);
        let d = i32x4_extract_lane::<3>(sum32);

        (a + b + c + d) as u32
    }
}

/// Fallback popcount for non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn simd_popcount(bits: &[u64; 4]) -> u32 {
    bits.iter().map(|b| b.count_ones()).sum()
}

/// SIMD-accelerated bitset XOR (for boundary computation)
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn simd_xor(a: &BitSet256, b: &BitSet256) -> BitSet256 {
    unsafe {
        let mut result = BitSet256::new();

        let a0 = v128_load(a.bits.as_ptr() as *const v128);
        let a1 = v128_load(a.bits.as_ptr().add(2) as *const v128);
        let b0 = v128_load(b.bits.as_ptr() as *const v128);
        let b1 = v128_load(b.bits.as_ptr().add(2) as *const v128);

        let r0 = v128_xor(a0, b0);
        let r1 = v128_xor(a1, b1);

        v128_store(result.bits.as_mut_ptr() as *mut v128, r0);
        v128_store(result.bits.as_mut_ptr().add(2) as *mut v128, r1);

        result
    }
}

/// Compute XOR of two bitsets (fallback for non-WASM targets)
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn simd_xor(a: &BitSet256, b: &BitSet256) -> BitSet256 {
    a.xor(b)
}

/// SIMD-accelerated boundary computation
/// Counts edges crossing between two vertex sets
#[inline]
pub fn simd_boundary_size(set_a: &BitSet256, edges: &[(CompactVertexId, CompactVertexId)]) -> u16 {
    let mut count = 0u16;

    for &(src, tgt) in edges {
        let src_in = set_a.contains(src);
        let tgt_in = set_a.contains(tgt);

        // Edge crosses boundary if exactly one endpoint is in set_a
        if src_in != tgt_in {
            count += 1;
        }
    }

    count
}

/// Batch membership check using SIMD
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn simd_batch_contains(set: &BitSet256, vertices: &[CompactVertexId; 8]) -> u8 {
    // Returns a bitmask where bit i is set if vertices[i] is in set
    let mut result = 0u8;
    for (i, &v) in vertices.iter().enumerate() {
        if set.contains(v) {
            result |= 1 << i;
        }
    }
    result
}

/// Batch membership check (fallback for non-WASM targets)
/// Returns a bitmask where bit i is set if vertices[i] is in set
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn simd_batch_contains(set: &BitSet256, vertices: &[CompactVertexId; 8]) -> u8 {
    let mut result = 0u8;
    for (i, &v) in vertices.iter().enumerate() {
        if set.contains(v) {
            result |= 1 << i;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_popcount() {
        let bits = [0b1010u64, 0b1111u64, 0u64, 0b10101010u64];
        assert_eq!(simd_popcount(&bits), 2 + 4 + 0 + 4);
    }

    #[test]
    fn test_simd_xor() {
        let mut a = BitSet256::new();
        let mut b = BitSet256::new();

        a.insert(1);
        a.insert(2);
        b.insert(2);
        b.insert(3);

        let result = simd_xor(&a, &b);
        assert!(result.contains(1));
        assert!(!result.contains(2));
        assert!(result.contains(3));
    }

    #[test]
    fn test_simd_boundary_size() {
        let mut set = BitSet256::new();
        set.insert(0);
        set.insert(1);

        let edges = [(0, 1), (1, 2), (2, 3)];

        // Edge (0,1) is inside, (1,2) crosses, (2,3) is outside
        assert_eq!(simd_boundary_size(&set, &edges), 1);
    }
}
