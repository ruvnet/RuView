//! Graph Wavelets
//!
//! Multi-scale analysis on graphs using spectral graph wavelets.
//! Based on Hammond et al. "Wavelets on Graphs via Spectral Graph Theory"

use super::{ChebyshevExpansion, ScaledLaplacian};

/// Wavelet scale configuration
#[derive(Debug, Clone)]
pub struct WaveletScale {
    /// Scale parameter (larger = coarser)
    pub scale: f64,
    /// Chebyshev expansion for this scale
    pub filter: ChebyshevExpansion,
}

impl WaveletScale {
    /// Create wavelet at given scale using Mexican hat kernel
    /// g(λ) = λ * exp(-λ * scale)
    pub fn mexican_hat(scale: f64, degree: usize) -> Self {
        let filter = ChebyshevExpansion::from_function(
            |x| {
                let lambda = (x + 1.0); // Map [-1,1] to [0,2]
                lambda * (-lambda * scale).exp()
            },
            degree,
        );

        Self { scale, filter }
    }

    /// Create wavelet using heat kernel derivative
    /// g(λ) = λ * exp(-λ * scale) (same as Mexican hat)
    pub fn heat_derivative(scale: f64, degree: usize) -> Self {
        Self::mexican_hat(scale, degree)
    }

    /// Create scaling function (low-pass for residual)
    /// h(λ) = exp(-λ * scale)
    pub fn scaling_function(scale: f64, degree: usize) -> Self {
        let filter = ChebyshevExpansion::from_function(
            |x| {
                let lambda = (x + 1.0);
                (-lambda * scale).exp()
            },
            degree,
        );

        Self { scale, filter }
    }
}

/// Graph wavelet at specific vertex
#[derive(Debug, Clone)]
pub struct GraphWavelet {
    /// Wavelet scale
    pub scale: WaveletScale,
    /// Center vertex
    pub center: usize,
    /// Wavelet coefficients for all vertices
    pub coefficients: Vec<f64>,
}

impl GraphWavelet {
    /// Compute wavelet centered at vertex
    pub fn at_vertex(laplacian: &ScaledLaplacian, scale: &WaveletScale, center: usize) -> Self {
        let n = laplacian.n;

        // Delta function at center
        let mut delta = vec![0.0; n];
        if center < n {
            delta[center] = 1.0;
        }

        // Apply wavelet filter: ψ_s,v = g(L) δ_v
        let coefficients = apply_filter(laplacian, &scale.filter, &delta);

        Self {
            scale: scale.clone(),
            center,
            coefficients,
        }
    }

    /// Inner product with signal
    pub fn inner_product(&self, signal: &[f64]) -> f64 {
        self.coefficients
            .iter()
            .zip(signal.iter())
            .map(|(&w, &s)| w * s)
            .sum()
    }

    /// L2 norm
    pub fn norm(&self) -> f64 {
        self.coefficients.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}

/// Spectral Wavelet Transform
#[derive(Debug, Clone)]
pub struct SpectralWaveletTransform {
    /// Laplacian
    laplacian: ScaledLaplacian,
    /// Wavelet scales (finest to coarsest)
    scales: Vec<WaveletScale>,
    /// Scaling function (for residual)
    scaling: WaveletScale,
    /// Chebyshev degree
    degree: usize,
}

impl SpectralWaveletTransform {
    /// Create wavelet transform with logarithmically spaced scales
    pub fn new(laplacian: ScaledLaplacian, num_scales: usize, degree: usize) -> Self {
        // Scales from fine (small t) to coarse (large t)
        let min_scale = 0.1;
        let max_scale = 2.0 / laplacian.lambda_max;

        let scales: Vec<WaveletScale> = (0..num_scales)
            .map(|i| {
                let t = if num_scales > 1 {
                    min_scale * (max_scale / min_scale).powf(i as f64 / (num_scales - 1) as f64)
                } else {
                    min_scale
                };
                WaveletScale::mexican_hat(t, degree)
            })
            .collect();

        let scaling = WaveletScale::scaling_function(max_scale, degree);

        Self {
            laplacian,
            scales,
            scaling,
            degree,
        }
    }

    /// Forward transform: compute wavelet coefficients
    /// Returns (scaling_coeffs, [wavelet_coeffs_scale_0, wavelet_coeffs_scale_1, ...])
    pub fn forward(&self, signal: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
        // Scaling coefficients
        let scaling_coeffs = apply_filter(&self.laplacian, &self.scaling.filter, signal);

        // Wavelet coefficients at each scale
        let wavelet_coeffs: Vec<Vec<f64>> = self
            .scales
            .iter()
            .map(|s| apply_filter(&self.laplacian, &s.filter, signal))
            .collect();

        (scaling_coeffs, wavelet_coeffs)
    }

    /// Inverse transform: reconstruct signal from coefficients
    /// Note: Perfect reconstruction requires frame bounds analysis
    pub fn inverse(&self, scaling_coeffs: &[f64], wavelet_coeffs: &[Vec<f64>]) -> Vec<f64> {
        let n = self.laplacian.n;
        let mut signal = vec![0.0; n];

        // Add scaling contribution
        let scaled_scaling = apply_filter(&self.laplacian, &self.scaling.filter, scaling_coeffs);
        for i in 0..n {
            signal[i] += scaled_scaling[i];
        }

        // Add wavelet contributions
        for (scale, coeffs) in self.scales.iter().zip(wavelet_coeffs.iter()) {
            let scaled_wavelet = apply_filter(&self.laplacian, &scale.filter, coeffs);
            for i in 0..n {
                signal[i] += scaled_wavelet[i];
            }
        }

        signal
    }

    /// Compute wavelet energy at each scale
    pub fn scale_energies(&self, signal: &[f64]) -> Vec<f64> {
        let (_, wavelet_coeffs) = self.forward(signal);

        wavelet_coeffs
            .iter()
            .map(|coeffs| coeffs.iter().map(|x| x * x).sum::<f64>())
            .collect()
    }

    /// Get all wavelets centered at a vertex
    pub fn wavelets_at(&self, vertex: usize) -> Vec<GraphWavelet> {
        self.scales
            .iter()
            .map(|s| GraphWavelet::at_vertex(&self.laplacian, s, vertex))
            .collect()
    }

    /// Number of scales
    pub fn num_scales(&self) -> usize {
        self.scales.len()
    }

    /// Get scale parameters
    pub fn scale_values(&self) -> Vec<f64> {
        self.scales.iter().map(|s| s.scale).collect()
    }
}

/// Apply Chebyshev filter to signal using recurrence
fn apply_filter(
    laplacian: &ScaledLaplacian,
    filter: &ChebyshevExpansion,
    signal: &[f64],
) -> Vec<f64> {
    let n = laplacian.n;
    let coeffs = &filter.coefficients;

    if coeffs.is_empty() || signal.len() != n {
        return vec![0.0; n];
    }

    let k = coeffs.len() - 1;

    let mut t_prev: Vec<f64> = signal.to_vec();
    let mut t_curr: Vec<f64> = laplacian.apply(signal);

    let mut output = vec![0.0; n];

    // c_0 * T_0 * x
    for i in 0..n {
        output[i] += coeffs[0] * t_prev[i];
    }

    // c_1 * T_1 * x
    if coeffs.len() > 1 {
        for i in 0..n {
            output[i] += coeffs[1] * t_curr[i];
        }
    }

    // Recurrence
    for ki in 2..=k {
        let lt_curr = laplacian.apply(&t_curr);
        let mut t_next = vec![0.0; n];
        for i in 0..n {
            t_next[i] = 2.0 * lt_curr[i] - t_prev[i];
        }

        for i in 0..n {
            output[i] += coeffs[ki] * t_next[i];
        }

        t_prev = t_curr;
        t_curr = t_next;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_graph_laplacian(n: usize) -> ScaledLaplacian {
        let edges: Vec<(usize, usize, f64)> = (0..n - 1).map(|i| (i, i + 1, 1.0)).collect();
        ScaledLaplacian::from_sparse_adjacency(&edges, n)
    }

    #[test]
    fn test_wavelet_scale() {
        let scale = WaveletScale::mexican_hat(0.5, 10);
        assert_eq!(scale.scale, 0.5);
        assert!(!scale.filter.coefficients.is_empty());
    }

    #[test]
    fn test_graph_wavelet() {
        let laplacian = path_graph_laplacian(10);
        let scale = WaveletScale::mexican_hat(0.5, 10);

        let wavelet = GraphWavelet::at_vertex(&laplacian, &scale, 5);

        assert_eq!(wavelet.center, 5);
        assert_eq!(wavelet.coefficients.len(), 10);
        // Wavelet should be localized around center
        assert!(wavelet.coefficients[5].abs() > 0.0);
    }

    #[test]
    fn test_wavelet_transform() {
        let laplacian = path_graph_laplacian(20);
        let transform = SpectralWaveletTransform::new(laplacian, 4, 10);

        assert_eq!(transform.num_scales(), 4);

        // Test forward transform
        let signal: Vec<f64> = (0..20).map(|i| (i as f64 * 0.3).sin()).collect();
        let (scaling, wavelets) = transform.forward(&signal);

        assert_eq!(scaling.len(), 20);
        assert_eq!(wavelets.len(), 4);
        for w in &wavelets {
            assert_eq!(w.len(), 20);
        }
    }

    #[test]
    fn test_scale_energies() {
        let laplacian = path_graph_laplacian(20);
        let transform = SpectralWaveletTransform::new(laplacian, 4, 10);

        let signal: Vec<f64> = (0..20).map(|i| (i as f64 * 0.3).sin()).collect();
        let energies = transform.scale_energies(&signal);

        assert_eq!(energies.len(), 4);
        // All energies should be non-negative
        for e in energies {
            assert!(e >= 0.0);
        }
    }

    #[test]
    fn test_wavelets_at_vertex() {
        let laplacian = path_graph_laplacian(10);
        let transform = SpectralWaveletTransform::new(laplacian, 3, 8);

        let wavelets = transform.wavelets_at(5);

        assert_eq!(wavelets.len(), 3);
        for w in &wavelets {
            assert_eq!(w.center, 5);
        }
    }
}
