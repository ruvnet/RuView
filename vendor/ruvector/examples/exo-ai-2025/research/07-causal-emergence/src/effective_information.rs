// Effective Information (EI) Calculation with SIMD Acceleration
// Implements Hoel's causal emergence framework for O(log n) hierarchical analysis

use std::simd::prelude::*;
use std::simd::StdFloat;

/// Computes effective information using SIMD acceleration
///
/// # Arguments
/// * `transition_matrix` - Flattened n×n matrix where T[i*n + j] = P(j|i)
/// * `n` - Number of states (matrix is n×n)
///
/// # Returns
/// Effective information in bits
///
/// # Complexity
/// O(n²) with SIMD vectorization (8-16× faster than scalar)
pub fn compute_ei_simd(transition_matrix: &[f32], n: usize) -> f32 {
    assert_eq!(transition_matrix.len(), n * n, "Matrix must be n×n");

    // Step 1: Compute marginal output distribution under uniform input
    // p(j) = (1/n) Σᵢ T[i,j]
    let p_out = compute_column_means_simd(transition_matrix, n);

    // Step 2: Compute output entropy H(out)
    let h_out = entropy_simd(&p_out);

    // Step 3: Compute conditional entropy H(out|in)
    // H(out|in) = (1/n) Σᵢ Σⱼ T[i,j] log₂ T[i,j]
    let h_cond = conditional_entropy_simd(transition_matrix, n);

    // Step 4: Effective information = H(out) - H(out|in)
    let ei = h_out - h_cond;

    // Ensure non-negative (numerical errors can cause tiny negatives)
    ei.max(0.0)
}

/// Computes column means of transition matrix (SIMD accelerated)
/// Returns p(j) = mean of column j
fn compute_column_means_simd(matrix: &[f32], n: usize) -> Vec<f32> {
    let mut means = vec![0.0f32; n];

    for j in 0..n {
        let mut sum = f32x16::splat(0.0);

        // Process 16 rows at a time
        let full_chunks = (n / 16) * 16;
        for i in (0..full_chunks).step_by(16) {
            // Load 16 elements from column j
            let mut chunk = [0.0f32; 16];
            for k in 0..16 {
                chunk[k] = matrix[(i + k) * n + j];
            }
            sum += f32x16::from_array(chunk);
        }

        // Handle remaining rows (scalar)
        let mut scalar_sum = sum.reduce_sum();
        for i in full_chunks..n {
            scalar_sum += matrix[i * n + j];
        }

        means[j] = scalar_sum / (n as f32);
    }

    means
}

/// Computes Shannon entropy with SIMD acceleration
/// H(X) = -Σ p(x) log₂ p(x)
pub fn entropy_simd(probs: &[f32]) -> f32 {
    let n = probs.len();
    let mut entropy = f32x16::splat(0.0);

    const EPSILON: f32 = 1e-10;
    let eps_vec = f32x16::splat(EPSILON);
    let log2_e = f32x16::splat(std::f32::consts::LOG2_E);

    // Process 16 elements at a time
    let full_chunks = (n / 16) * 16;
    for i in (0..full_chunks).step_by(16) {
        let p = f32x16::from_slice(&probs[i..i + 16]);

        // Clip to avoid log(0)
        let p_safe = p.simd_max(eps_vec);

        // -p * log₂(p) = -p * ln(p) / ln(2)
        let log_p = p_safe.ln() * log2_e;
        entropy -= p * log_p;
    }

    // Handle remaining elements (scalar)
    let mut scalar_entropy = entropy.reduce_sum();
    for i in full_chunks..n {
        let p = probs[i];
        if p > EPSILON {
            scalar_entropy -= p * p.log2();
        }
    }

    scalar_entropy
}

/// Computes conditional entropy H(out|in) for transition matrix
/// H(out|in) = (1/n) Σᵢ Σⱼ T[i,j] log₂ T[i,j]
fn conditional_entropy_simd(matrix: &[f32], n: usize) -> f32 {
    let mut h_cond = f32x16::splat(0.0);

    const EPSILON: f32 = 1e-10;
    let eps_vec = f32x16::splat(EPSILON);
    let log2_e = f32x16::splat(std::f32::consts::LOG2_E);

    // Process matrix in 16-element chunks
    let total_elements = n * n;
    let full_chunks = (total_elements / 16) * 16;

    for i in (0..full_chunks).step_by(16) {
        let t = f32x16::from_slice(&matrix[i..i + 16]);

        // Clip to avoid log(0)
        let t_safe = t.simd_max(eps_vec);

        // -T[i,j] * log₂(T[i,j])
        let log_t = t_safe.ln() * log2_e;
        h_cond -= t * log_t;
    }

    // Handle remaining elements
    let mut scalar_sum = h_cond.reduce_sum();
    for i in full_chunks..total_elements {
        let t = matrix[i];
        if t > EPSILON {
            scalar_sum -= t * t.log2();
        }
    }

    // Divide by n (average over uniform input distribution)
    scalar_sum / (n as f32)
}

/// Computes effective information for multiple scales in parallel
///
/// # Arguments
/// * `transition_matrices` - Vector of transition matrices at different scales
/// * `state_counts` - Number of states at each scale
///
/// # Returns
/// Vector of EI values, one per scale
pub fn compute_ei_multi_scale(
    transition_matrices: &[Vec<f32>],
    state_counts: &[usize],
) -> Vec<f32> {
    assert_eq!(transition_matrices.len(), state_counts.len());

    transition_matrices
        .iter()
        .zip(state_counts.iter())
        .map(|(matrix, &n)| compute_ei_simd(matrix, n))
        .collect()
}

/// Detects causal emergence by comparing EI across scales
///
/// # Returns
/// (emergent_scale, ei_gain) where:
/// - emergent_scale: Index of scale with maximum EI
/// - ei_gain: EI(emergent) - EI(micro)
pub fn detect_causal_emergence(ei_per_scale: &[f32]) -> Option<(usize, f32)> {
    if ei_per_scale.is_empty() {
        return None;
    }

    let (max_scale, &max_ei) = ei_per_scale
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?;

    let micro_ei = ei_per_scale[0];
    let ei_gain = max_ei - micro_ei;

    Some((max_scale, ei_gain))
}

/// Normalized effective information (0 to 1 scale)
///
/// EI_normalized = EI / log₂(n)
/// where log₂(n) is the maximum possible EI
pub fn normalized_ei(ei: f32, num_states: usize) -> f32 {
    if num_states <= 1 {
        return 0.0;
    }

    let max_ei = (num_states as f32).log2();
    (ei / max_ei).min(1.0).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ei_deterministic_cycle() {
        // Deterministic cyclic transition: i → (i+1) mod n
        let n = 16;
        let mut matrix = vec![0.0; n * n];
        for i in 0..n {
            matrix[i * n + ((i + 1) % n)] = 1.0;
        }

        let ei = compute_ei_simd(&matrix, n);
        let expected = (n as f32).log2(); // Should be maximal

        assert!(
            (ei - expected).abs() < 0.1,
            "Deterministic system should have EI ≈ log₂(n), got {}, expected {}",
            ei,
            expected
        );
    }

    #[test]
    fn test_ei_random() {
        // Uniform random transitions
        let n = 16;
        let matrix = vec![1.0 / (n as f32); n * n];

        let ei = compute_ei_simd(&matrix, n);

        assert!(ei < 0.1, "Random system should have EI ≈ 0, got {}", ei);
    }

    #[test]
    fn test_ei_identity() {
        // Identity transition (state doesn't change)
        let n = 8;
        let mut matrix = vec![0.0; n * n];
        for i in 0..n {
            matrix[i * n + i] = 1.0;
        }

        let ei = compute_ei_simd(&matrix, n);
        let expected = (n as f32).log2();

        assert!(
            (ei - expected).abs() < 0.1,
            "Identity should have maximal EI, got {}, expected {}",
            ei,
            expected
        );
    }

    #[test]
    fn test_entropy_uniform() {
        let probs = vec![0.25, 0.25, 0.25, 0.25];
        let h = entropy_simd(&probs);
        assert!(
            (h - 2.0).abs() < 0.01,
            "Uniform 4-state should have H=2 bits"
        );
    }

    #[test]
    fn test_entropy_deterministic() {
        let probs = vec![1.0, 0.0, 0.0, 0.0];
        let h = entropy_simd(&probs);
        assert!(h < 0.01, "Deterministic should have H≈0, got {}", h);
    }

    #[test]
    fn test_causal_emergence_detection() {
        // Create hierarchy where middle scale has highest EI
        let ei_scales = vec![2.0, 3.5, 4.2, 3.8, 2.5];

        let (emergent_scale, gain) = detect_causal_emergence(&ei_scales).unwrap();

        assert_eq!(emergent_scale, 2, "Should detect scale 2 as emergent");
        assert!(
            (gain - 2.2).abs() < 0.01,
            "EI gain should be 4.2 - 2.0 = 2.2"
        );
    }

    #[test]
    fn test_normalized_ei() {
        let ei = 3.0;
        let n = 8; // log₂(8) = 3.0

        let norm = normalized_ei(ei, n);
        assert!((norm - 1.0).abs() < 0.01, "Should normalize to 1.0");
    }

    #[test]
    fn test_multi_scale_computation() {
        // Test computing EI for multiple scales
        let matrix1 = vec![0.25; 16]; // 4×4 matrix
        let matrix2 = vec![0.5; 4]; // 2×2 matrix (correctly sized)

        let matrices = vec![matrix1, matrix2];
        let counts = vec![4, 2]; // Correct sizes

        // Compute EI for both scales
        let results = compute_ei_multi_scale(&matrices, &counts);
        assert_eq!(results.len(), 2);

        // Results should be non-negative
        for ei in &results {
            assert!(*ei >= 0.0);
        }
    }
}

// Benchmarking utilities
#[cfg(feature = "bench")]
pub mod bench {
    use super::*;
    use std::time::Instant;

    pub fn benchmark_ei(n: usize, iterations: usize) -> f64 {
        // Create random transition matrix
        let mut matrix = vec![0.0; n * n];
        for i in 0..n {
            let mut row_sum = 0.0;
            for j in 0..n {
                let val = (i * n + j) as f32 % 100.0 / 100.0;
                matrix[i * n + j] = val;
                row_sum += val;
            }
            // Normalize row to sum to 1
            for j in 0..n {
                matrix[i * n + j] /= row_sum;
            }
        }

        let start = Instant::now();
        for _ in 0..iterations {
            compute_ei_simd(&matrix, n);
        }
        let elapsed = start.elapsed();

        elapsed.as_secs_f64() / iterations as f64
    }

    pub fn print_benchmark_results() {
        println!("Effective Information SIMD Benchmarks:");
        println!("----------------------------------------");

        for n in [16, 64, 256, 1024] {
            let time = benchmark_ei(n, 100);
            let states_per_sec = (n * n) as f64 / time;
            println!(
                "n={:4}: {:.3}ms ({:.0} states/sec)",
                n,
                time * 1000.0,
                states_per_sec
            );
        }
    }
}
