//! Hyperdimensional Computing (HDC) module
//!
//! Implements binary hypervectors with SIMD-optimized operations for
//! ultra-fast pattern matching and associative memory.

mod memory;
mod ops;
mod similarity;
mod vector;

pub use memory::HdcMemory;
pub use ops::{bind, bind_multiple, bundle, invert, permute};
pub use similarity::{
    batch_similarities, cosine_similarity, find_similar, hamming_distance, jaccard_similarity,
    normalized_hamming, pairwise_similarities, top_k_similar,
};
pub use vector::{HdcError, Hypervector};

/// Number of bits in a hypervector (10,000)
pub const HYPERVECTOR_BITS: usize = 10_000;

/// Number of u64 words needed to store HYPERVECTOR_BITS (157 = ceil(10000/64))
pub const HYPERVECTOR_U64_LEN: usize = 157;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(HYPERVECTOR_U64_LEN, 157);
        assert_eq!(HYPERVECTOR_BITS, 10_000);
        assert!(HYPERVECTOR_U64_LEN * 64 >= HYPERVECTOR_BITS);
    }

    #[test]
    fn test_module_exports() {
        // Verify all exports are accessible
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let _bound = bind(&v1, &v2);
        let _bundled = bundle(&[v1.clone(), v2.clone()]);
        let _dist = hamming_distance(&v1, &v2);
        let _sim = cosine_similarity(&v1, &v2);

        let mut memory = HdcMemory::new();
        memory.store("test", v1.clone());
    }
}
