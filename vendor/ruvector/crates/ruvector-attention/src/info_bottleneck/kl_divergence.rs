//! KL Divergence Computations
//!
//! Efficient KL divergence for various distributions used in attention.

/// Diagonal Gaussian parameters
#[derive(Debug, Clone)]
pub struct DiagonalGaussian {
    /// Mean vector
    pub mean: Vec<f32>,
    /// Log variance vector
    pub log_var: Vec<f32>,
}

impl DiagonalGaussian {
    /// Create from mean and log variance
    pub fn new(mean: Vec<f32>, log_var: Vec<f32>) -> Self {
        Self { mean, log_var }
    }

    /// Create unit Gaussian (mean=0, var=1)
    pub fn unit(dim: usize) -> Self {
        Self {
            mean: vec![0.0; dim],
            log_var: vec![0.0; dim],
        }
    }

    /// Sample using reparameterization trick
    /// z = mean + std * epsilon, where epsilon ~ N(0, 1)
    pub fn sample(&self, epsilon: &[f32]) -> Vec<f32> {
        let n = self.mean.len();
        let mut z = vec![0.0f32; n];

        for i in 0..n {
            let std = (0.5 * self.log_var[i]).exp();
            z[i] = self.mean[i] + std * epsilon[i];
        }

        z
    }

    /// Get variance
    pub fn variance(&self) -> Vec<f32> {
        self.log_var.iter().map(|&lv| lv.exp()).collect()
    }

    /// Get standard deviation
    pub fn std(&self) -> Vec<f32> {
        self.log_var.iter().map(|&lv| (0.5 * lv).exp()).collect()
    }
}

/// KL Divergence computations
#[derive(Debug, Clone)]
pub struct KLDivergence;

impl KLDivergence {
    /// KL(N(mu, sigma^2) || N(0, 1))
    /// = 0.5 * sum(exp(log_var) + mu^2 - 1 - log_var)
    pub fn gaussian_to_unit(gaussian: &DiagonalGaussian) -> f32 {
        let n = gaussian.mean.len();
        let mut kl = 0.0f32;

        for i in 0..n {
            let mu = gaussian.mean[i];
            let lv = gaussian.log_var[i];
            let var = lv.exp();
            kl += var + mu * mu - 1.0 - lv;
        }

        0.5 * kl
    }

    /// KL(N(mu, sigma^2) || N(0, 1)) from separate arrays
    pub fn gaussian_to_unit_arrays(mean: &[f32], log_var: &[f32]) -> f32 {
        let n = mean.len().min(log_var.len());
        let mut kl = 0.0f32;

        for i in 0..n {
            let mu = mean[i];
            let lv = log_var[i];
            let var = lv.exp();
            kl += var + mu * mu - 1.0 - lv;
        }

        0.5 * kl
    }

    /// KL(N(mu1, sigma1^2) || N(mu2, sigma2^2))
    /// = 0.5 * sum(log(var2/var1) + (var1 + (mu1-mu2)^2)/var2 - 1)
    pub fn gaussian_to_gaussian(q: &DiagonalGaussian, p: &DiagonalGaussian) -> f32 {
        let n = q.mean.len().min(p.mean.len());
        let mut kl = 0.0f32;

        for i in 0..n {
            let mu_q = q.mean[i];
            let mu_p = p.mean[i];
            let lv_q = q.log_var[i];
            let lv_p = p.log_var[i];

            let var_q = lv_q.exp();
            let var_p = lv_p.exp().max(1e-8);

            let log_ratio = lv_p - lv_q;
            let diff = mu_q - mu_p;

            kl += log_ratio + (var_q + diff * diff) / var_p - 1.0;
        }

        0.5 * kl
    }

    /// KL divergence between categorical distributions
    /// KL(p || q) = sum(p * log(p/q))
    pub fn categorical(p: &[f32], q: &[f32]) -> f32 {
        let n = p.len().min(q.len());
        let mut kl = 0.0f32;
        let eps = 1e-10;

        for i in 0..n {
            let pi = p[i].max(eps);
            let qi = q[i].max(eps);
            if pi > eps {
                kl += pi * (pi / qi).ln();
            }
        }

        kl.max(0.0)
    }

    /// Symmetric KL (Jensen-Shannon divergence approximation)
    /// JS(p, q) ≈ 0.5 * (KL(p || m) + KL(q || m)) where m = (p+q)/2
    pub fn jensen_shannon(p: &[f32], q: &[f32]) -> f32 {
        let n = p.len().min(q.len());
        let mut m = vec![0.0f32; n];

        for i in 0..n {
            m[i] = 0.5 * (p[i] + q[i]);
        }

        0.5 * (Self::categorical(p, &m) + Self::categorical(q, &m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kl_to_unit() {
        // Unit Gaussian should have KL = 0
        let unit = DiagonalGaussian::unit(4);
        let kl = KLDivergence::gaussian_to_unit(&unit);
        assert!(kl.abs() < 1e-5);
    }

    #[test]
    fn test_kl_nonzero() {
        let g = DiagonalGaussian::new(vec![1.0, 0.5, -0.5], vec![0.5, 0.0, -0.5]);
        let kl = KLDivergence::gaussian_to_unit(&g);
        assert!(kl > 0.0);
    }

    #[test]
    fn test_kl_arrays() {
        let mean = vec![0.0, 0.0];
        let log_var = vec![0.0, 0.0];

        let kl = KLDivergence::gaussian_to_unit_arrays(&mean, &log_var);
        assert!(kl.abs() < 1e-5);
    }

    #[test]
    fn test_categorical_kl() {
        let p = vec![0.5, 0.5];
        let q = vec![0.5, 0.5];

        let kl = KLDivergence::categorical(&p, &q);
        assert!(kl.abs() < 1e-5);

        let q2 = vec![0.9, 0.1];
        let kl2 = KLDivergence::categorical(&p, &q2);
        assert!(kl2 > 0.0);
    }

    #[test]
    fn test_jensen_shannon() {
        let p = vec![0.5, 0.5];
        let q = vec![0.5, 0.5];

        let js = KLDivergence::jensen_shannon(&p, &q);
        assert!(js.abs() < 1e-5);
    }

    #[test]
    fn test_sample() {
        let g = DiagonalGaussian::new(vec![0.0, 1.0], vec![0.0, 0.0]);
        let epsilon = vec![0.0, 0.0];

        let z = g.sample(&epsilon);
        assert!((z[0] - 0.0).abs() < 1e-5);
        assert!((z[1] - 1.0).abs() < 1e-5);
    }
}
