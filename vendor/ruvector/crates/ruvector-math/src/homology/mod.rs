//! Persistent Homology and Topological Data Analysis
//!
//! Topological methods for analyzing shape and structure in data.
//!
//! ## Key Capabilities
//!
//! - **Persistent Homology**: Track topological features (components, loops, voids)
//! - **Betti Numbers**: Count topological features at each scale
//! - **Persistence Diagrams**: Visualize feature lifetimes
//! - **Bottleneck/Wasserstein Distance**: Compare topological signatures
//!
//! ## Integration with Mincut
//!
//! TDA complements mincut by providing:
//! - Long-term drift detection (shape changes over time)
//! - Coherence monitoring (are attention patterns stable?)
//! - Anomaly detection (topological outliers)
//!
//! ## Mathematical Background
//!
//! Given a filtration of simplicial complexes K_0 ⊆ K_1 ⊆ ... ⊆ K_n,
//! persistent homology tracks when features are born and die.
//!
//! Birth-death pairs form the persistence diagram.

mod distance;
mod filtration;
mod persistence;
mod simplex;

pub use distance::{BottleneckDistance, WassersteinDistance};
pub use filtration::{AlphaComplex, Filtration, VietorisRips};
pub use persistence::{BirthDeathPair, PersistenceDiagram, PersistentHomology};
pub use simplex::{Simplex, SimplicialComplex};

/// Betti numbers at a given scale
#[derive(Debug, Clone, PartialEq)]
pub struct BettiNumbers {
    /// β_0: number of connected components
    pub b0: usize,
    /// β_1: number of 1-cycles (loops)
    pub b1: usize,
    /// β_2: number of 2-cycles (voids)
    pub b2: usize,
}

impl BettiNumbers {
    /// Create from values
    pub fn new(b0: usize, b1: usize, b2: usize) -> Self {
        Self { b0, b1, b2 }
    }

    /// Total number of features
    pub fn total(&self) -> usize {
        self.b0 + self.b1 + self.b2
    }

    /// Euler characteristic χ = β_0 - β_1 + β_2
    pub fn euler_characteristic(&self) -> i64 {
        self.b0 as i64 - self.b1 as i64 + self.b2 as i64
    }
}

/// Point in Euclidean space
#[derive(Debug, Clone)]
pub struct Point {
    pub coords: Vec<f64>,
}

impl Point {
    /// Create point from coordinates
    pub fn new(coords: Vec<f64>) -> Self {
        Self { coords }
    }

    /// Dimension
    pub fn dim(&self) -> usize {
        self.coords.len()
    }

    /// Euclidean distance to another point
    pub fn distance(&self, other: &Point) -> f64 {
        self.coords
            .iter()
            .zip(other.coords.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Squared distance (faster)
    pub fn distance_sq(&self, other: &Point) -> f64 {
        self.coords
            .iter()
            .zip(other.coords.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum()
    }
}

/// Point cloud for TDA
#[derive(Debug, Clone)]
pub struct PointCloud {
    /// Points
    pub points: Vec<Point>,
    /// Dimension of ambient space
    pub ambient_dim: usize,
}

impl PointCloud {
    /// Create from points
    pub fn new(points: Vec<Point>) -> Self {
        let ambient_dim = points.first().map(|p| p.dim()).unwrap_or(0);
        Self {
            points,
            ambient_dim,
        }
    }

    /// Create from flat array (row-major)
    pub fn from_flat(data: &[f64], dim: usize) -> Self {
        let points: Vec<Point> = data
            .chunks(dim)
            .map(|chunk| Point::new(chunk.to_vec()))
            .collect();
        Self {
            points,
            ambient_dim: dim,
        }
    }

    /// Number of points
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Is empty?
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Compute all pairwise distances
    pub fn distance_matrix(&self) -> Vec<f64> {
        let n = self.points.len();
        let mut dist = vec![0.0; n * n];

        for i in 0..n {
            for j in i + 1..n {
                let d = self.points[i].distance(&self.points[j]);
                dist[i * n + j] = d;
                dist[j * n + i] = d;
            }
        }

        dist
    }

    /// Get bounding box
    pub fn bounding_box(&self) -> Option<(Point, Point)> {
        if self.points.is_empty() {
            return None;
        }

        let dim = self.ambient_dim;
        let mut min_coords = vec![f64::INFINITY; dim];
        let mut max_coords = vec![f64::NEG_INFINITY; dim];

        for p in &self.points {
            for (i, &c) in p.coords.iter().enumerate() {
                min_coords[i] = min_coords[i].min(c);
                max_coords[i] = max_coords[i].max(c);
            }
        }

        Some((Point::new(min_coords), Point::new(max_coords)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_betti_numbers() {
        let betti = BettiNumbers::new(1, 2, 0);

        assert_eq!(betti.total(), 3);
        assert_eq!(betti.euler_characteristic(), -1);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(vec![0.0, 0.0]);
        let p2 = Point::new(vec![3.0, 4.0]);

        assert!((p1.distance(&p2) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_cloud() {
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0], 2);

        assert_eq!(cloud.len(), 3);
        assert_eq!(cloud.ambient_dim, 2);
    }

    #[test]
    fn test_distance_matrix() {
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0], 2);
        let dist = cloud.distance_matrix();

        assert_eq!(dist.len(), 9);
        assert!((dist[0 * 3 + 1] - 1.0).abs() < 1e-10); // (0,0) to (1,0)
        assert!((dist[0 * 3 + 2] - 1.0).abs() < 1e-10); // (0,0) to (0,1)
    }
}
