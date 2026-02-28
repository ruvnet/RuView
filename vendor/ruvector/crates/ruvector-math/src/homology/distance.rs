//! Distances between Persistence Diagrams
//!
//! Bottleneck and Wasserstein distances for comparing topological signatures.

use super::{BirthDeathPair, PersistenceDiagram};

/// Bottleneck distance between persistence diagrams
///
/// d_∞(D1, D2) = inf_γ sup_p ||p - γ(p)||_∞
///
/// where γ ranges over bijections between diagrams (with diagonal).
#[derive(Debug, Clone)]
pub struct BottleneckDistance;

impl BottleneckDistance {
    /// Compute bottleneck distance for dimension d
    pub fn compute(d1: &PersistenceDiagram, d2: &PersistenceDiagram, dim: usize) -> f64 {
        let pts1: Vec<(f64, f64)> = d1
            .pairs_of_dim(dim)
            .filter(|p| !p.is_essential())
            .map(|p| (p.birth, p.death.unwrap_or(f64::INFINITY)))
            .collect();

        let pts2: Vec<(f64, f64)> = d2
            .pairs_of_dim(dim)
            .filter(|p| !p.is_essential())
            .map(|p| (p.birth, p.death.unwrap_or(f64::INFINITY)))
            .collect();

        Self::bottleneck_finite(&pts1, &pts2)
    }

    /// Bottleneck distance for finite points
    fn bottleneck_finite(pts1: &[(f64, f64)], pts2: &[(f64, f64)]) -> f64 {
        if pts1.is_empty() && pts2.is_empty() {
            return 0.0;
        }

        // Include diagonal projections
        let mut all_distances = Vec::new();

        // Distances between points
        for &(b1, d1) in pts1 {
            for &(b2, d2) in pts2 {
                let dist = Self::l_inf((b1, d1), (b2, d2));
                all_distances.push(dist);
            }
        }

        // Distances to diagonal
        for &(b, d) in pts1 {
            let diag_dist = (d - b) / 2.0;
            all_distances.push(diag_dist);
        }
        for &(b, d) in pts2 {
            let diag_dist = (d - b) / 2.0;
            all_distances.push(diag_dist);
        }

        if all_distances.is_empty() {
            return 0.0;
        }

        // Sort and binary search for bottleneck
        all_distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // For small instances, use greedy matching at each threshold
        for &threshold in &all_distances {
            if Self::can_match(pts1, pts2, threshold) {
                return threshold;
            }
        }

        // Fallback
        *all_distances.last().unwrap_or(&0.0)
    }

    /// Check if perfect matching exists at threshold
    fn can_match(pts1: &[(f64, f64)], pts2: &[(f64, f64)], threshold: f64) -> bool {
        // Simple greedy matching (not optimal but fast)
        let mut used2 = vec![false; pts2.len()];
        let mut matched1 = 0;

        for &p1 in pts1 {
            // Try to match to a point in pts2
            let mut found = false;
            for (j, &p2) in pts2.iter().enumerate() {
                if !used2[j] && Self::l_inf(p1, p2) <= threshold {
                    used2[j] = true;
                    found = true;
                    break;
                }
            }

            if !found {
                // Try to match to diagonal
                if Self::diag_dist(p1) <= threshold {
                    matched1 += 1;
                    continue;
                }
                return false;
            }
            matched1 += 1;
        }

        // Check unmatched pts2 can go to diagonal
        for (j, &p2) in pts2.iter().enumerate() {
            if !used2[j] && Self::diag_dist(p2) > threshold {
                return false;
            }
        }

        true
    }

    /// L-infinity distance between points
    fn l_inf(p1: (f64, f64), p2: (f64, f64)) -> f64 {
        (p1.0 - p2.0).abs().max((p1.1 - p2.1).abs())
    }

    /// Distance to diagonal
    fn diag_dist(p: (f64, f64)) -> f64 {
        (p.1 - p.0) / 2.0
    }
}

/// Wasserstein distance between persistence diagrams
///
/// W_p(D1, D2) = (inf_γ Σ ||p - γ(p)||_∞^p)^{1/p}
#[derive(Debug, Clone)]
pub struct WassersteinDistance {
    /// Power p (usually 1 or 2)
    pub p: f64,
}

impl WassersteinDistance {
    /// Create with power p
    pub fn new(p: f64) -> Self {
        Self { p: p.max(1.0) }
    }

    /// Compute W_p distance for dimension d
    pub fn compute(&self, d1: &PersistenceDiagram, d2: &PersistenceDiagram, dim: usize) -> f64 {
        let pts1: Vec<(f64, f64)> = d1
            .pairs_of_dim(dim)
            .filter(|p| !p.is_essential())
            .map(|p| (p.birth, p.death.unwrap_or(f64::INFINITY)))
            .collect();

        let pts2: Vec<(f64, f64)> = d2
            .pairs_of_dim(dim)
            .filter(|p| !p.is_essential())
            .map(|p| (p.birth, p.death.unwrap_or(f64::INFINITY)))
            .collect();

        self.wasserstein_finite(&pts1, &pts2)
    }

    /// Wasserstein distance for finite points (greedy approximation)
    fn wasserstein_finite(&self, pts1: &[(f64, f64)], pts2: &[(f64, f64)]) -> f64 {
        if pts1.is_empty() && pts2.is_empty() {
            return 0.0;
        }

        // Greedy matching (approximation)
        let mut used2 = vec![false; pts2.len()];
        let mut total_cost = 0.0;

        for &p1 in pts1 {
            let diag_cost = Self::diag_dist(p1).powf(self.p);

            // Find best match
            let mut best_cost = diag_cost;
            let mut best_j = None;

            for (j, &p2) in pts2.iter().enumerate() {
                if !used2[j] {
                    let cost = Self::l_inf(p1, p2).powf(self.p);
                    if cost < best_cost {
                        best_cost = cost;
                        best_j = Some(j);
                    }
                }
            }

            total_cost += best_cost;
            if let Some(j) = best_j {
                used2[j] = true;
            }
        }

        // Unmatched pts2 go to diagonal
        for (j, &p2) in pts2.iter().enumerate() {
            if !used2[j] {
                total_cost += Self::diag_dist(p2).powf(self.p);
            }
        }

        total_cost.powf(1.0 / self.p)
    }

    fn l_inf(p1: (f64, f64), p2: (f64, f64)) -> f64 {
        (p1.0 - p2.0).abs().max((p1.1 - p2.1).abs())
    }

    fn diag_dist(p: (f64, f64)) -> f64 {
        (p.1 - p.0) / 2.0
    }
}

/// Persistence landscape for machine learning
#[derive(Debug, Clone)]
pub struct PersistenceLandscape {
    /// Landscape functions λ_k(t)
    pub landscapes: Vec<Vec<f64>>,
    /// Grid points
    pub grid: Vec<f64>,
    /// Number of landscape functions
    pub num_landscapes: usize,
}

impl PersistenceLandscape {
    /// Compute landscape from persistence diagram
    pub fn from_diagram(
        diagram: &PersistenceDiagram,
        dim: usize,
        num_landscapes: usize,
        resolution: usize,
    ) -> Self {
        let pairs: Vec<(f64, f64)> = diagram
            .pairs_of_dim(dim)
            .filter(|p| !p.is_essential())
            .map(|p| (p.birth, p.death.unwrap_or(f64::INFINITY)))
            .filter(|p| p.1.is_finite())
            .collect();

        if pairs.is_empty() {
            return Self {
                landscapes: vec![vec![0.0; resolution]; num_landscapes],
                grid: (0..resolution)
                    .map(|i| i as f64 / resolution as f64)
                    .collect(),
                num_landscapes,
            };
        }

        // Determine grid
        let min_t = pairs.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let max_t = pairs.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let range = (max_t - min_t).max(1e-10);

        let grid: Vec<f64> = (0..resolution)
            .map(|i| min_t + (i as f64 / (resolution - 1).max(1) as f64) * range)
            .collect();

        // Compute tent functions at each grid point
        let mut landscapes = vec![vec![0.0; resolution]; num_landscapes];

        for (gi, &t) in grid.iter().enumerate() {
            // Evaluate all tent functions at t
            let mut values: Vec<f64> = pairs
                .iter()
                .map(|&(b, d)| {
                    if t < b || t > d {
                        0.0
                    } else if t <= (b + d) / 2.0 {
                        t - b
                    } else {
                        d - t
                    }
                })
                .collect();

            // Sort descending
            values.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

            // Take top k
            for (k, &v) in values.iter().take(num_landscapes).enumerate() {
                landscapes[k][gi] = v;
            }
        }

        Self {
            landscapes,
            grid,
            num_landscapes,
        }
    }

    /// L2 distance between landscapes
    pub fn l2_distance(&self, other: &Self) -> f64 {
        if self.grid.len() != other.grid.len() || self.num_landscapes != other.num_landscapes {
            return f64::INFINITY;
        }

        let n = self.grid.len();
        let dt = if n > 1 {
            (self.grid[n - 1] - self.grid[0]) / (n - 1) as f64
        } else {
            1.0
        };

        let mut total = 0.0;
        for k in 0..self.num_landscapes {
            for i in 0..n {
                let diff = self.landscapes[k][i] - other.landscapes[k][i];
                total += diff * diff * dt;
            }
        }

        total.sqrt()
    }

    /// Get feature vector (flattened landscape)
    pub fn to_vector(&self) -> Vec<f64> {
        self.landscapes
            .iter()
            .flat_map(|l| l.iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_diagram() -> PersistenceDiagram {
        let mut d = PersistenceDiagram::new();
        d.add(BirthDeathPair::finite(0, 0.0, 1.0));
        d.add(BirthDeathPair::finite(0, 0.5, 1.5));
        d.add(BirthDeathPair::finite(1, 0.2, 0.8));
        d
    }

    #[test]
    fn test_bottleneck_same() {
        let d = sample_diagram();
        let dist = BottleneckDistance::compute(&d, &d, 0);
        assert!(dist < 1e-10);
    }

    #[test]
    fn test_bottleneck_different() {
        let d1 = sample_diagram();
        let mut d2 = PersistenceDiagram::new();
        d2.add(BirthDeathPair::finite(0, 0.0, 2.0));

        let dist = BottleneckDistance::compute(&d1, &d2, 0);
        assert!(dist > 0.0);
    }

    #[test]
    fn test_wasserstein() {
        let d1 = sample_diagram();
        let d2 = sample_diagram();

        let w1 = WassersteinDistance::new(1.0);
        let dist = w1.compute(&d1, &d2, 0);
        assert!(dist < 1e-10);
    }

    #[test]
    fn test_persistence_landscape() {
        let d = sample_diagram();
        let landscape = PersistenceLandscape::from_diagram(&d, 0, 3, 20);

        assert_eq!(landscape.landscapes.len(), 3);
        assert_eq!(landscape.grid.len(), 20);
    }

    #[test]
    fn test_landscape_distance() {
        let d1 = sample_diagram();
        let l1 = PersistenceLandscape::from_diagram(&d1, 0, 3, 20);
        let l2 = PersistenceLandscape::from_diagram(&d1, 0, 3, 20);

        let dist = l1.l2_distance(&l2);
        assert!(dist < 1e-10);
    }

    #[test]
    fn test_landscape_vector() {
        let d = sample_diagram();
        let landscape = PersistenceLandscape::from_diagram(&d, 0, 2, 10);

        let vec = landscape.to_vector();
        assert_eq!(vec.len(), 20); // 2 landscapes × 10 points
    }
}
