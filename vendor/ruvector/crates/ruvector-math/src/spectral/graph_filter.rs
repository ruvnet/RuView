//! Graph Filtering via Chebyshev Polynomials
//!
//! Efficient O(Km) graph filtering where K is polynomial degree
//! and m is the number of edges. No eigendecomposition required.

use super::{ChebyshevExpansion, ScaledLaplacian};

/// Type of spectral filter
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterType {
    /// Low-pass: attenuate high frequencies
    LowPass { cutoff: f64 },
    /// High-pass: attenuate low frequencies
    HighPass { cutoff: f64 },
    /// Band-pass: keep frequencies in range
    BandPass { low: f64, high: f64 },
    /// Heat diffusion: exp(-t*L)
    Heat { time: f64 },
    /// Custom polynomial
    Custom,
}

/// Spectral graph filter using Chebyshev approximation
#[derive(Debug, Clone)]
pub struct SpectralFilter {
    /// Chebyshev expansion of filter function
    pub expansion: ChebyshevExpansion,
    /// Filter type
    pub filter_type: FilterType,
    /// Polynomial degree
    pub degree: usize,
}

impl SpectralFilter {
    /// Create heat diffusion filter: exp(-t*L)
    pub fn heat(time: f64, degree: usize) -> Self {
        Self {
            expansion: ChebyshevExpansion::heat_kernel(time, degree),
            filter_type: FilterType::Heat { time },
            degree,
        }
    }

    /// Create low-pass filter
    pub fn low_pass(cutoff: f64, degree: usize) -> Self {
        let steepness = 5.0 / cutoff.max(0.1);
        let expansion = ChebyshevExpansion::from_function(
            |x| {
                let lambda = (x + 1.0); // Map [-1,1] to [0,2]
                1.0 / (1.0 + (steepness * (lambda - cutoff)).exp())
            },
            degree,
        );

        Self {
            expansion,
            filter_type: FilterType::LowPass { cutoff },
            degree,
        }
    }

    /// Create high-pass filter
    pub fn high_pass(cutoff: f64, degree: usize) -> Self {
        let steepness = 5.0 / cutoff.max(0.1);
        let expansion = ChebyshevExpansion::from_function(
            |x| {
                let lambda = (x + 1.0);
                1.0 / (1.0 + (steepness * (cutoff - lambda)).exp())
            },
            degree,
        );

        Self {
            expansion,
            filter_type: FilterType::HighPass { cutoff },
            degree,
        }
    }

    /// Create band-pass filter
    pub fn band_pass(low: f64, high: f64, degree: usize) -> Self {
        let steepness = 5.0;
        let expansion = ChebyshevExpansion::from_function(
            |x| {
                let lambda = (x + 1.0);
                let low_gate = 1.0 / (1.0 + (steepness * (low - lambda)).exp());
                let high_gate = 1.0 / (1.0 + (steepness * (lambda - high)).exp());
                low_gate * high_gate
            },
            degree,
        );

        Self {
            expansion,
            filter_type: FilterType::BandPass { low, high },
            degree,
        }
    }

    /// Create from custom Chebyshev expansion
    pub fn custom(expansion: ChebyshevExpansion) -> Self {
        let degree = expansion.degree();
        Self {
            expansion,
            filter_type: FilterType::Custom,
            degree,
        }
    }
}

/// Graph filter that applies spectral operations
#[derive(Debug, Clone)]
pub struct GraphFilter {
    /// Scaled Laplacian
    laplacian: ScaledLaplacian,
    /// Spectral filter to apply
    filter: SpectralFilter,
}

impl GraphFilter {
    /// Create graph filter from adjacency and filter specification
    pub fn new(laplacian: ScaledLaplacian, filter: SpectralFilter) -> Self {
        Self { laplacian, filter }
    }

    /// Create from dense adjacency matrix
    pub fn from_adjacency(adj: &[f64], n: usize, filter: SpectralFilter) -> Self {
        let laplacian = ScaledLaplacian::from_adjacency(adj, n);
        Self::new(laplacian, filter)
    }

    /// Create from sparse edges
    pub fn from_sparse(edges: &[(usize, usize, f64)], n: usize, filter: SpectralFilter) -> Self {
        let laplacian = ScaledLaplacian::from_sparse_adjacency(edges, n);
        Self::new(laplacian, filter)
    }

    /// Apply filter to signal: y = h(L) * x
    /// Uses Chebyshev recurrence: O(K*m) where K is degree, m is edges
    pub fn apply(&self, signal: &[f64]) -> Vec<f64> {
        let n = self.laplacian.n;
        let k = self.filter.degree;
        let coeffs = &self.filter.expansion.coefficients;

        if coeffs.is_empty() || signal.len() != n {
            return vec![0.0; n];
        }

        // Chebyshev recurrence on graph:
        // T_0(L) * x = x
        // T_1(L) * x = L * x
        // T_{k+1}(L) * x = 2*L*T_k(L)*x - T_{k-1}(L)*x

        let mut t_prev: Vec<f64> = signal.to_vec(); // T_0 * x = x
        let mut t_curr: Vec<f64> = self.laplacian.apply(signal); // T_1 * x = L * x

        // Output: y = sum_k c_k * T_k(L) * x
        let mut output = vec![0.0; n];

        // Add c_0 * T_0 * x
        for i in 0..n {
            output[i] += coeffs[0] * t_prev[i];
        }

        // Add c_1 * T_1 * x if exists
        if coeffs.len() > 1 {
            for i in 0..n {
                output[i] += coeffs[1] * t_curr[i];
            }
        }

        // Recurrence for k >= 2
        for ki in 2..=k {
            if ki >= coeffs.len() {
                break;
            }

            // T_{k+1} * x = 2*L*T_k*x - T_{k-1}*x
            let lt_curr = self.laplacian.apply(&t_curr);
            let mut t_next = vec![0.0; n];
            for i in 0..n {
                t_next[i] = 2.0 * lt_curr[i] - t_prev[i];
            }

            // Add c_k * T_k * x
            for i in 0..n {
                output[i] += coeffs[ki] * t_next[i];
            }

            // Shift
            t_prev = t_curr;
            t_curr = t_next;
        }

        output
    }

    /// Apply filter multiple times (for stronger effect)
    pub fn apply_n(&self, signal: &[f64], n_times: usize) -> Vec<f64> {
        let mut result = signal.to_vec();
        for _ in 0..n_times {
            result = self.apply(&result);
        }
        result
    }

    /// Compute filter energy: x^T h(L) x
    pub fn energy(&self, signal: &[f64]) -> f64 {
        let filtered = self.apply(signal);
        signal
            .iter()
            .zip(filtered.iter())
            .map(|(&x, &y)| x * y)
            .sum()
    }

    /// Get estimated spectral range
    pub fn lambda_max(&self) -> f64 {
        self.laplacian.lambda_max
    }
}

/// Multi-scale graph filtering
#[derive(Debug, Clone)]
pub struct MultiscaleFilter {
    /// Filters at different scales
    filters: Vec<GraphFilter>,
    /// Scale parameters
    scales: Vec<f64>,
}

impl MultiscaleFilter {
    /// Create multiscale heat diffusion filters
    pub fn heat_scales(laplacian: ScaledLaplacian, scales: Vec<f64>, degree: usize) -> Self {
        let filters: Vec<GraphFilter> = scales
            .iter()
            .map(|&t| GraphFilter::new(laplacian.clone(), SpectralFilter::heat(t, degree)))
            .collect();

        Self { filters, scales }
    }

    /// Apply all scales and return matrix (n × num_scales)
    pub fn apply_all(&self, signal: &[f64]) -> Vec<Vec<f64>> {
        self.filters.iter().map(|f| f.apply(signal)).collect()
    }

    /// Get scale values
    pub fn scales(&self) -> &[f64] {
        &self.scales
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_graph() -> (Vec<f64>, usize) {
        // Triangle graph: complete K_3
        let adj = vec![0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        (adj, 3)
    }

    #[test]
    fn test_heat_filter() {
        let (adj, n) = simple_graph();
        let filter = GraphFilter::from_adjacency(&adj, n, SpectralFilter::heat(0.5, 10));

        let signal = vec![1.0, 0.0, 0.0]; // Delta at node 0
        let smoothed = filter.apply(&signal);

        assert_eq!(smoothed.len(), 3);
        // Heat diffusion should spread the signal
        // After smoothing, node 0 should have less concentration
    }

    #[test]
    fn test_low_pass_filter() {
        let (adj, n) = simple_graph();
        let filter = GraphFilter::from_adjacency(&adj, n, SpectralFilter::low_pass(0.5, 10));

        let signal = vec![1.0, -1.0, 0.0]; // High frequency component
        let filtered = filter.apply(&signal);

        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_constant_signal() {
        let (adj, n) = simple_graph();
        let filter = GraphFilter::from_adjacency(&adj, n, SpectralFilter::heat(1.0, 10));

        // Constant signal is in null space of Laplacian
        let signal = vec![1.0, 1.0, 1.0];
        let filtered = filter.apply(&signal);

        // Should remain approximately constant
        let mean: f64 = filtered.iter().sum::<f64>() / 3.0;
        for &v in &filtered {
            assert!(
                (v - mean).abs() < 0.5,
                "Constant signal not preserved: {:?}",
                filtered
            );
        }
    }

    #[test]
    fn test_multiscale() {
        let (adj, n) = simple_graph();
        let laplacian = ScaledLaplacian::from_adjacency(&adj, n);
        let scales = vec![0.1, 0.5, 1.0, 2.0];

        let multiscale = MultiscaleFilter::heat_scales(laplacian, scales.clone(), 10);

        let signal = vec![1.0, 0.0, 0.0];
        let all_scales = multiscale.apply_all(&signal);

        assert_eq!(all_scales.len(), 4);
        for scale_result in &all_scales {
            assert_eq!(scale_result.len(), 3);
        }
    }

    #[test]
    fn test_sparse_graph() {
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0)];
        let n = 4;

        let filter = GraphFilter::from_sparse(&edges, n, SpectralFilter::heat(0.5, 10));

        let signal = vec![1.0, 0.0, 0.0, 0.0];
        let smoothed = filter.apply(&signal);

        assert_eq!(smoothed.len(), 4);
    }
}
