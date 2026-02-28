//! SIMD-accelerated operations for WASM
//!
//! Provides SIMD-optimized vector operations when wasm32-simd128 is available.

/// Check if SIMD is available at runtime
pub fn simd_available() -> bool {
    #[cfg(target_feature = "simd128")]
    {
        true
    }
    #[cfg(not(target_feature = "simd128"))]
    {
        false
    }
}

/// SIMD-accelerated vector addition (a += b)
#[cfg(target_feature = "simd128")]
pub fn simd_add_assign(a: &mut [f32], b: &[f32]) {
    use core::arch::wasm32::*;

    assert_eq!(a.len(), b.len());

    let chunks = a.len() / 4;
    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_ptr = a.as_mut_ptr().add(offset);
            let b_ptr = b.as_ptr().add(offset);

            let a_vec = v128_load(a_ptr as *const v128);
            let b_vec = v128_load(b_ptr as *const v128);
            let result = f32x4_add(a_vec, b_vec);

            v128_store(a_ptr as *mut v128, result);
        }
    }

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        a[i] += b[i];
    }
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_add_assign(a: &mut [f32], b: &[f32]) {
    for (av, bv) in a.iter_mut().zip(b.iter()) {
        *av += *bv;
    }
}

/// SIMD-accelerated vector subtraction (a -= b)
#[cfg(target_feature = "simd128")]
pub fn simd_sub_assign(a: &mut [f32], b: &[f32]) {
    use core::arch::wasm32::*;

    assert_eq!(a.len(), b.len());

    let chunks = a.len() / 4;
    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_ptr = a.as_mut_ptr().add(offset);
            let b_ptr = b.as_ptr().add(offset);

            let a_vec = v128_load(a_ptr as *const v128);
            let b_vec = v128_load(b_ptr as *const v128);
            let result = f32x4_sub(a_vec, b_vec);

            v128_store(a_ptr as *mut v128, result);
        }
    }

    for i in (chunks * 4)..a.len() {
        a[i] -= b[i];
    }
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_sub_assign(a: &mut [f32], b: &[f32]) {
    for (av, bv) in a.iter_mut().zip(b.iter()) {
        *av -= *bv;
    }
}

/// SIMD-accelerated vector scaling (a *= scalar)
#[cfg(target_feature = "simd128")]
pub fn simd_scale(a: &mut [f32], scalar: f32) {
    use core::arch::wasm32::*;

    let scalar_vec = f32x4_splat(scalar);
    let chunks = a.len() / 4;

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_ptr = a.as_mut_ptr().add(offset);
            let a_vec = v128_load(a_ptr as *const v128);
            let result = f32x4_mul(a_vec, scalar_vec);
            v128_store(a_ptr as *mut v128, result);
        }
    }

    for i in (chunks * 4)..a.len() {
        a[i] *= scalar;
    }
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_scale(a: &mut [f32], scalar: f32) {
    for v in a.iter_mut() {
        *v *= scalar;
    }
}

/// SIMD-accelerated dot product
#[cfg(target_feature = "simd128")]
pub fn simd_dot(a: &[f32], b: &[f32]) -> f32 {
    use core::arch::wasm32::*;

    assert_eq!(a.len(), b.len());

    let chunks = a.len() / 4;
    let mut sum_vec = f32x4_splat(0.0);

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_vec = v128_load(a.as_ptr().add(offset) as *const v128);
            let b_vec = v128_load(b.as_ptr().add(offset) as *const v128);
            let prod = f32x4_mul(a_vec, b_vec);
            sum_vec = f32x4_add(sum_vec, prod);
        }
    }

    // Horizontal sum
    let sum_array: [f32; 4] = unsafe { core::mem::transmute(sum_vec) };
    let mut sum = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        sum += a[i] * b[i];
    }

    sum
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// SIMD-accelerated L2 norm squared
#[cfg(target_feature = "simd128")]
pub fn simd_l2_norm_squared(a: &[f32]) -> f32 {
    use core::arch::wasm32::*;

    let chunks = a.len() / 4;
    let mut sum_vec = f32x4_splat(0.0);

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_vec = v128_load(a.as_ptr().add(offset) as *const v128);
            let sq = f32x4_mul(a_vec, a_vec);
            sum_vec = f32x4_add(sum_vec, sq);
        }
    }

    let sum_array: [f32; 4] = unsafe { core::mem::transmute(sum_vec) };
    let mut sum = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

    for i in (chunks * 4)..a.len() {
        sum += a[i] * a[i];
    }

    sum
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_l2_norm_squared(a: &[f32]) -> f32 {
    a.iter().map(|x| x * x).sum()
}

/// SIMD-accelerated element-wise difference (result = a - b)
#[cfg(target_feature = "simd128")]
pub fn simd_diff(a: &[f32], b: &[f32], result: &mut [f32]) {
    use core::arch::wasm32::*;

    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    let chunks = a.len() / 4;

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_vec = v128_load(a.as_ptr().add(offset) as *const v128);
            let b_vec = v128_load(b.as_ptr().add(offset) as *const v128);
            let diff = f32x4_sub(a_vec, b_vec);
            v128_store(result.as_mut_ptr().add(offset) as *mut v128, diff);
        }
    }

    for i in (chunks * 4)..a.len() {
        result[i] = a[i] - b[i];
    }
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_diff(a: &[f32], b: &[f32], result: &mut [f32]) {
    for i in 0..a.len() {
        result[i] = a[i] - b[i];
    }
}

/// SIMD-accelerated element-wise absolute value
#[cfg(target_feature = "simd128")]
pub fn simd_abs(a: &mut [f32]) {
    use core::arch::wasm32::*;

    let chunks = a.len() / 4;

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_ptr = a.as_mut_ptr().add(offset);
            let a_vec = v128_load(a_ptr as *const v128);
            let result = f32x4_abs(a_vec);
            v128_store(a_ptr as *mut v128, result);
        }
    }

    for i in (chunks * 4)..a.len() {
        a[i] = a[i].abs();
    }
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_abs(a: &mut [f32]) {
    for v in a.iter_mut() {
        *v = v.abs();
    }
}

/// SIMD-accelerated clamp
#[cfg(target_feature = "simd128")]
pub fn simd_clamp(a: &mut [f32], min: f32, max: f32) {
    use core::arch::wasm32::*;

    let min_vec = f32x4_splat(min);
    let max_vec = f32x4_splat(max);
    let chunks = a.len() / 4;

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_ptr = a.as_mut_ptr().add(offset);
            let a_vec = v128_load(a_ptr as *const v128);
            let clamped = f32x4_max(f32x4_min(a_vec, max_vec), min_vec);
            v128_store(a_ptr as *mut v128, clamped);
        }
    }

    for i in (chunks * 4)..a.len() {
        a[i] = a[i].clamp(min, max);
    }
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_clamp(a: &mut [f32], min: f32, max: f32) {
    for v in a.iter_mut() {
        *v = v.clamp(min, max);
    }
}

/// Count non-zero elements with SIMD acceleration
#[cfg(target_feature = "simd128")]
pub fn simd_count_nonzero(a: &[f32], epsilon: f32) -> usize {
    use core::arch::wasm32::*;

    let eps_vec = f32x4_splat(epsilon);
    let neg_eps_vec = f32x4_splat(-epsilon);
    let chunks = a.len() / 4;
    let mut count = 0usize;

    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_vec = v128_load(a.as_ptr().add(offset) as *const v128);

            // Check if |a| > epsilon
            let gt_eps = f32x4_gt(a_vec, eps_vec);
            let lt_neg_eps = f32x4_lt(a_vec, neg_eps_vec);
            let nonzero = v128_or(gt_eps, lt_neg_eps);

            // Convert to bitmask and count
            let mask = i32x4_bitmask(nonzero) as u8;
            count += mask.count_ones() as usize;
        }
    }

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        if a[i].abs() > epsilon {
            count += 1;
        }
    }

    count
}

#[cfg(not(target_feature = "simd128"))]
pub fn simd_count_nonzero(a: &[f32], epsilon: f32) -> usize {
    a.iter().filter(|v| v.abs() > epsilon).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_assign() {
        let mut a = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![1.0f32, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

        simd_add_assign(&mut a, &b);

        assert!((a[0] - 2.0).abs() < 1e-6);
        assert!((a[7] - 9.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot() {
        let a = vec![1.0f32, 2.0, 3.0, 4.0];
        let b = vec![1.0f32, 1.0, 1.0, 1.0];

        let result = simd_dot(&a, &b);
        assert!((result - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_squared() {
        let a = vec![3.0f32, 4.0];
        let result = simd_l2_norm_squared(&a);
        assert!((result - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_scale() {
        let mut a = vec![1.0f32, 2.0, 3.0, 4.0];
        simd_scale(&mut a, 2.0);

        assert!((a[0] - 2.0).abs() < 1e-6);
        assert!((a[3] - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_count_nonzero() {
        let a = vec![1.0f32, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0];
        let count = simd_count_nonzero(&a, 1e-7);
        assert_eq!(count, 4);
    }
}
