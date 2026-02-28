//! Additional product manifold operations

use super::ProductManifold;
use crate::error::{MathError, Result};
use crate::utils::{norm, EPS};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Batch operations on product manifolds
impl ProductManifold {
    /// Compute pairwise distances between all points
    /// Uses parallel computation when 'parallel' feature is enabled
    pub fn pairwise_distances(&self, points: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
        let n = points.len();

        #[cfg(feature = "parallel")]
        {
            self.pairwise_distances_parallel(points, n)
        }

        #[cfg(not(feature = "parallel"))]
        {
            self.pairwise_distances_sequential(points, n)
        }
    }

    /// Sequential pairwise distance computation
    #[inline]
    fn pairwise_distances_sequential(
        &self,
        points: &[Vec<f64>],
        n: usize,
    ) -> Result<Vec<Vec<f64>>> {
        let mut distances = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.distance(&points[i], &points[j])?;
                distances[i][j] = d;
                distances[j][i] = d;
            }
        }

        Ok(distances)
    }

    /// Parallel pairwise distance computation using rayon
    #[cfg(feature = "parallel")]
    fn pairwise_distances_parallel(&self, points: &[Vec<f64>], n: usize) -> Result<Vec<Vec<f64>>> {
        // Compute upper triangle in parallel
        let pairs: Vec<_> = (0..n)
            .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
            .collect();

        let results: Vec<(usize, usize, f64)> = pairs
            .par_iter()
            .filter_map(|&(i, j)| {
                self.distance(&points[i], &points[j])
                    .ok()
                    .map(|d| (i, j, d))
            })
            .collect();

        let mut distances = vec![vec![0.0; n]; n];
        for (i, j, d) in results {
            distances[i][j] = d;
            distances[j][i] = d;
        }

        Ok(distances)
    }

    /// Find k-nearest neighbors
    /// Uses parallel computation when 'parallel' feature is enabled
    pub fn knn(&self, query: &[f64], points: &[Vec<f64>], k: usize) -> Result<Vec<(usize, f64)>> {
        #[cfg(feature = "parallel")]
        {
            self.knn_parallel(query, points, k)
        }

        #[cfg(not(feature = "parallel"))]
        {
            self.knn_sequential(query, points, k)
        }
    }

    /// Sequential k-nearest neighbors
    #[inline]
    fn knn_sequential(
        &self,
        query: &[f64],
        points: &[Vec<f64>],
        k: usize,
    ) -> Result<Vec<(usize, f64)>> {
        let mut distances: Vec<(usize, f64)> = points
            .iter()
            .enumerate()
            .filter_map(|(i, p)| self.distance(query, p).ok().map(|d| (i, d)))
            .collect();

        // Use sort_unstable_by for better performance
        distances
            .sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        distances.truncate(k);

        Ok(distances)
    }

    /// Parallel k-nearest neighbors using rayon
    #[cfg(feature = "parallel")]
    fn knn_parallel(
        &self,
        query: &[f64],
        points: &[Vec<f64>],
        k: usize,
    ) -> Result<Vec<(usize, f64)>> {
        let mut distances: Vec<(usize, f64)> = points
            .par_iter()
            .enumerate()
            .filter_map(|(i, p)| self.distance(query, p).ok().map(|d| (i, d)))
            .collect();

        // Use sort_unstable_by for better performance
        distances
            .sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        distances.truncate(k);

        Ok(distances)
    }

    /// Geodesic interpolation between two points
    ///
    /// Returns point at fraction t along geodesic from x to y
    pub fn geodesic(&self, x: &[f64], y: &[f64], t: f64) -> Result<Vec<f64>> {
        let t = t.clamp(0.0, 1.0);

        // log_x(y) gives direction
        let v = self.log_map(x, y)?;

        // Scale by t
        let tv: Vec<f64> = v.iter().map(|&vi| t * vi).collect();

        // exp_x(t * v)
        self.exp_map(x, &tv)
    }

    /// Sample points along geodesic
    pub fn geodesic_path(&self, x: &[f64], y: &[f64], num_points: usize) -> Result<Vec<Vec<f64>>> {
        let mut path = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let t = i as f64 / (num_points - 1).max(1) as f64;
            path.push(self.geodesic(x, y, t)?);
        }

        Ok(path)
    }

    /// Parallel transport vector v from x to y
    pub fn parallel_transport(&self, x: &[f64], y: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.dim() || y.len() != self.dim() || v.len() != self.dim() {
            return Err(MathError::dimension_mismatch(self.dim(), x.len()));
        }

        let mut result = vec![0.0; self.dim()];
        let (e_range, h_range, s_range) = self.config().component_ranges();

        // Euclidean: parallel transport is identity
        for i in e_range.clone() {
            result[i] = v[i];
        }

        // Hyperbolic parallel transport
        if !h_range.is_empty() {
            let x_h = &x[h_range.clone()];
            let y_h = &y[h_range.clone()];
            let v_h = &v[h_range.clone()];
            let pt_h = self.poincare_parallel_transport(x_h, y_h, v_h)?;
            for (i, val) in h_range.clone().zip(pt_h.iter()) {
                result[i] = *val;
            }
        }

        // Spherical parallel transport
        if !s_range.is_empty() {
            let x_s = &x[s_range.clone()];
            let y_s = &y[s_range.clone()];
            let v_s = &v[s_range.clone()];
            let pt_s = self.spherical_parallel_transport(x_s, y_s, v_s)?;
            for (i, val) in s_range.clone().zip(pt_s.iter()) {
                result[i] = *val;
            }
        }

        Ok(result)
    }

    /// Poincaré ball parallel transport
    fn poincare_parallel_transport(&self, x: &[f64], y: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        let c = -self.config().hyperbolic_curvature;

        let x_norm_sq: f64 = x.iter().map(|&xi| xi * xi).sum();
        let y_norm_sq: f64 = y.iter().map(|&yi| yi * yi).sum();

        let lambda_x = 2.0 / (1.0 - c * x_norm_sq).max(EPS);
        let lambda_y = 2.0 / (1.0 - c * y_norm_sq).max(EPS);

        let scale = lambda_x / lambda_y;

        // Gyration correction
        let xy_dot: f64 = x.iter().zip(y.iter()).map(|(&xi, &yi)| xi * yi).sum();
        let _gyration_factor = 1.0 + c * xy_dot;

        // Simplified parallel transport (good approximation for small distances)
        Ok(v.iter().map(|&vi| scale * vi).collect())
    }

    /// Spherical parallel transport
    fn spherical_parallel_transport(&self, x: &[f64], y: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        use crate::utils::dot;

        let cos_theta = dot(x, y).clamp(-1.0, 1.0);

        if (cos_theta - 1.0).abs() < EPS {
            return Ok(v.to_vec());
        }

        let theta = cos_theta.acos();

        // Direction from x to y
        let u: Vec<f64> = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| yi - cos_theta * xi)
            .collect();
        let u_norm = norm(&u);

        if u_norm < EPS {
            return Ok(v.to_vec());
        }

        let u: Vec<f64> = u.iter().map(|&ui| ui / u_norm).collect();

        // Components of v
        let v_u = dot(v, &u);
        let v_x = dot(v, x);

        // Parallel transport formula
        let result: Vec<f64> = (0..x.len())
            .map(|i| {
                let v_perp = v[i] - v_u * u[i] - v_x * x[i];
                v_perp + v_u * (-theta.sin() * x[i] + theta.cos() * u[i])
                    - v_x * (theta.cos() * x[i] + theta.sin() * u[i])
            })
            .collect();

        Ok(result)
    }

    /// Compute variance of points on manifold
    pub fn variance(&self, points: &[Vec<f64>], mean: Option<&[f64]>) -> Result<f64> {
        if points.is_empty() {
            return Ok(0.0);
        }

        let mean = match mean {
            Some(m) => m.to_vec(),
            None => self.frechet_mean(points, None)?,
        };

        let mut total_sq_dist = 0.0;
        for p in points {
            let d = self.distance(&mean, p)?;
            total_sq_dist += d * d;
        }

        Ok(total_sq_dist / points.len() as f64)
    }

    /// Project gradient to tangent space at point
    ///
    /// For product manifolds, this projects each component appropriately
    pub fn project_gradient(&self, point: &[f64], gradient: &[f64]) -> Result<Vec<f64>> {
        if point.len() != self.dim() || gradient.len() != self.dim() {
            return Err(MathError::dimension_mismatch(self.dim(), point.len()));
        }

        let mut result = gradient.to_vec();
        let (_e_range, h_range, s_range) = self.config().component_ranges();

        // Euclidean: gradient is already in tangent space (no modification needed)

        // Hyperbolic: scale by (1 - ||x||²)² / 4
        if !h_range.is_empty() {
            let x_h = &point[h_range.clone()];
            let x_norm_sq: f64 = x_h.iter().map(|&xi| xi * xi).sum();
            let c = -self.config().hyperbolic_curvature;
            let lambda = 2.0 / (1.0 - c * x_norm_sq).max(EPS);
            let scale = 1.0 / (lambda * lambda);

            for i in h_range.clone() {
                result[i] *= scale;
            }
        }

        // Spherical: project out normal component
        if !s_range.is_empty() {
            let x_s = &point[s_range.clone()];
            let g_s = &gradient[s_range.clone()];

            // Normal component: (g · x) x
            let normal_component: f64 = g_s.iter().zip(x_s.iter()).map(|(&gi, &xi)| gi * xi).sum();

            for (i, &xi) in s_range.clone().zip(x_s.iter()) {
                result[i] -= normal_component * xi;
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairwise_distances() {
        let manifold = ProductManifold::new(2, 0, 0);

        let points = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        let dists = manifold.pairwise_distances(&points).unwrap();

        assert!(dists[0][0].abs() < 1e-10);
        assert!((dists[0][1] - 1.0).abs() < 1e-10);
        assert!((dists[0][2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_knn() {
        let manifold = ProductManifold::new(2, 0, 0);

        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![2.0, 0.0],
            vec![3.0, 0.0],
        ];

        let query = vec![0.5, 0.0];
        let neighbors = manifold.knn(&query, &points, 2).unwrap();

        assert_eq!(neighbors.len(), 2);
        // Closest should be [0,0] or [1,0]
        assert!(neighbors[0].0 == 0 || neighbors[0].0 == 1);
    }

    #[test]
    fn test_geodesic_path() {
        let manifold = ProductManifold::new(2, 0, 0);

        let x = vec![0.0, 0.0];
        let y = vec![2.0, 2.0];

        let path = manifold.geodesic_path(&x, &y, 5).unwrap();

        assert_eq!(path.len(), 5);

        // Midpoint should be (1, 1)
        assert!((path[2][0] - 1.0).abs() < 1e-6);
        assert!((path[2][1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_variance() {
        let manifold = ProductManifold::new(2, 0, 0);

        // Points at unit distance from origin
        let points = vec![
            vec![1.0, 0.0],
            vec![-1.0, 0.0],
            vec![0.0, 1.0],
            vec![0.0, -1.0],
        ];

        let variance = manifold.variance(&points, Some(&vec![0.0, 0.0])).unwrap();
        assert!((variance - 1.0).abs() < 1e-10);
    }
}
