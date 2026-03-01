//! HDC operations: binding, bundling, permutation

use super::vector::{HdcError, Hypervector};
use super::HYPERVECTOR_U64_LEN;

/// Binds two hypervectors using XOR
///
/// This is a convenience function equivalent to `v1.bind(&v2)`.
///
/// # Performance
///
/// <50ns on modern CPUs
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, bind};
///
/// let a = Hypervector::random();
/// let b = Hypervector::random();
/// let bound = bind(&a, &b);
/// ```
#[inline]
pub fn bind(v1: &Hypervector, v2: &Hypervector) -> Hypervector {
    v1.bind(v2)
}

/// Bundles multiple hypervectors by majority voting
///
/// This is a convenience function equivalent to `Hypervector::bundle(vectors)`.
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, bundle};
///
/// let v1 = Hypervector::random();
/// let v2 = Hypervector::random();
/// let v3 = Hypervector::random();
///
/// let bundled = bundle(&[v1, v2, v3]).unwrap();
/// ```
pub fn bundle(vectors: &[Hypervector]) -> Result<Hypervector, HdcError> {
    Hypervector::bundle(vectors)
}

/// Permutes a hypervector by rotating bits
///
/// Permutation creates a new representation that is orthogonal to the original,
/// useful for encoding sequences and positions.
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, permute};
///
/// let v = Hypervector::random();
/// let p1 = permute(&v, 1);
/// let p2 = permute(&v, 2);
///
/// // Permuted vectors are orthogonal
/// assert!(v.similarity(&p1) < 0.6);
/// assert!(p1.similarity(&p2) < 0.6);
/// ```
pub fn permute(v: &Hypervector, shift: usize) -> Hypervector {
    if shift == 0 {
        return v.clone();
    }

    let mut result = Hypervector::zero();
    let total_bits = HYPERVECTOR_U64_LEN * 64;
    let shift = shift % total_bits; // Normalize shift

    // Rotate bits left by shift positions
    for i in 0..total_bits {
        let src_idx = i;
        let dst_idx = (i + shift) % total_bits;

        let src_word = src_idx / 64;
        let src_bit = src_idx % 64;
        let dst_word = dst_idx / 64;
        let dst_bit = dst_idx % 64;

        let bit = (v.bits()[src_word] >> src_bit) & 1;
        result.bits[dst_word] |= bit << dst_bit;
    }

    result
}

/// Inverts all bits in a hypervector
///
/// Useful for negation and creating opposite representations.
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, invert};
///
/// let v = Hypervector::random();
/// let inv = invert(&v);
///
/// // Similarity should be near 0 (opposite)
/// assert!(v.similarity(&inv) < 0.1);
/// ```
pub fn invert(v: &Hypervector) -> Hypervector {
    let mut result = Hypervector::zero();

    for i in 0..HYPERVECTOR_U64_LEN {
        result.bits[i] = !v.bits()[i];
    }

    result
}

/// Binds multiple vectors in sequence
///
/// Equivalent to `v1.bind(&v2).bind(&v3)...`
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, bind_multiple};
///
/// let v1 = Hypervector::random();
/// let v2 = Hypervector::random();
/// let v3 = Hypervector::random();
///
/// let bound = bind_multiple(&[v1, v2, v3]).unwrap();
/// ```
pub fn bind_multiple(vectors: &[Hypervector]) -> Result<Hypervector, HdcError> {
    if vectors.is_empty() {
        return Err(HdcError::EmptyVectorSet);
    }

    let mut result = vectors[0].clone();

    for v in &vectors[1..] {
        result = result.bind(v);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bind_function() {
        let a = Hypervector::random();
        let b = Hypervector::random();

        let bound1 = bind(&a, &b);
        let bound2 = a.bind(&b);

        assert_eq!(bound1, bound2);
    }

    #[test]
    fn test_bundle_function() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let bundled1 = bundle(&[v1.clone(), v2.clone()]).unwrap();
        let bundled2 = Hypervector::bundle(&[v1, v2]).unwrap();

        assert_eq!(bundled1, bundled2);
    }

    #[test]
    fn test_permute_zero_is_identity() {
        let v = Hypervector::random();
        let p = permute(&v, 0);

        assert_eq!(v, p);
    }

    #[test]
    fn test_permute_creates_orthogonal() {
        let v = Hypervector::random();
        let p1 = permute(&v, 1);
        let p2 = permute(&v, 2);

        // Permuted vectors should be mostly orthogonal
        assert!(v.similarity(&p1) < 0.6);
        assert!(p1.similarity(&p2) < 0.6);
    }

    #[test]
    fn test_permute_inverse() {
        let v = Hypervector::random();
        let total_bits = HYPERVECTOR_U64_LEN * 64;

        let p = permute(&v, 100);
        let back = permute(&p, total_bits - 100);

        assert_eq!(v, back);
    }

    #[test]
    fn test_invert_creates_opposite() {
        let v = Hypervector::random();
        let inv = invert(&v);

        // Inverted vector should have opposite bits
        let sim = v.similarity(&inv);
        assert!(sim < 0.1, "similarity: {}", sim);
    }

    #[test]
    fn test_invert_double_is_identity() {
        let v = Hypervector::random();
        let inv = invert(&v);
        let back = invert(&inv);

        assert_eq!(v, back);
    }

    #[test]
    fn test_bind_multiple_single() {
        let v = Hypervector::random();
        let result = bind_multiple(&[v.clone()]).unwrap();

        assert_eq!(result, v);
    }

    #[test]
    fn test_bind_multiple_two() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let result1 = bind_multiple(&[v1.clone(), v2.clone()]).unwrap();
        let result2 = v1.bind(&v2);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_bind_multiple_three() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();
        let v3 = Hypervector::random();

        let result1 = bind_multiple(&[v1.clone(), v2.clone(), v3.clone()]).unwrap();
        let result2 = v1.bind(&v2).bind(&v3);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_bind_multiple_empty_error() {
        let result = bind_multiple(&[]);
        assert!(matches!(result, Err(HdcError::EmptyVectorSet)));
    }
}
