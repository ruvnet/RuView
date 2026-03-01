//! Filtrations for Persistent Homology
//!
//! A filtration is a sequence of nested simplicial complexes.

use super::{PointCloud, Simplex, SimplicialComplex};

/// A filtered simplex (simplex with birth time)
#[derive(Debug, Clone)]
pub struct FilteredSimplex {
    /// The simplex
    pub simplex: Simplex,
    /// Birth time (filtration value)
    pub birth: f64,
}

impl FilteredSimplex {
    pub fn new(simplex: Simplex, birth: f64) -> Self {
        Self { simplex, birth }
    }
}

/// Filtration: sequence of simplicial complexes
#[derive(Debug, Clone)]
pub struct Filtration {
    /// Filtered simplices sorted by birth time
    pub simplices: Vec<FilteredSimplex>,
    /// Maximum dimension
    pub max_dim: usize,
}

impl Filtration {
    /// Create empty filtration
    pub fn new() -> Self {
        Self {
            simplices: Vec::new(),
            max_dim: 0,
        }
    }

    /// Add filtered simplex
    pub fn add(&mut self, simplex: Simplex, birth: f64) {
        self.max_dim = self.max_dim.max(simplex.dim());
        self.simplices.push(FilteredSimplex::new(simplex, birth));
    }

    /// Sort by birth time (required before computing persistence)
    pub fn sort(&mut self) {
        // Sort by birth time, then by dimension (lower dimension first)
        self.simplices.sort_by(|a, b| {
            a.birth
                .partial_cmp(&b.birth)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.simplex.dim().cmp(&b.simplex.dim()))
        });
    }

    /// Get complex at filtration value t
    pub fn complex_at(&self, t: f64) -> SimplicialComplex {
        let simplices: Vec<Simplex> = self
            .simplices
            .iter()
            .filter(|fs| fs.birth <= t)
            .map(|fs| fs.simplex.clone())
            .collect();
        SimplicialComplex::from_simplices(simplices)
    }

    /// Number of simplices
    pub fn len(&self) -> usize {
        self.simplices.len()
    }

    /// Is empty?
    pub fn is_empty(&self) -> bool {
        self.simplices.is_empty()
    }

    /// Get filtration values
    pub fn filtration_values(&self) -> Vec<f64> {
        let mut values: Vec<f64> = self.simplices.iter().map(|fs| fs.birth).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values.dedup();
        values
    }
}

impl Default for Filtration {
    fn default() -> Self {
        Self::new()
    }
}

/// Vietoris-Rips filtration
///
/// At scale ε, includes all simplices whose vertices are pairwise within distance ε.
#[derive(Debug, Clone)]
pub struct VietorisRips {
    /// Maximum dimension to compute
    pub max_dim: usize,
    /// Maximum filtration value
    pub max_scale: f64,
}

impl VietorisRips {
    /// Create with parameters
    pub fn new(max_dim: usize, max_scale: f64) -> Self {
        Self { max_dim, max_scale }
    }

    /// Build filtration from point cloud
    pub fn build(&self, cloud: &PointCloud) -> Filtration {
        let n = cloud.len();
        let dist = cloud.distance_matrix();

        let mut filtration = Filtration::new();

        // Add vertices at time 0
        for i in 0..n {
            filtration.add(Simplex::vertex(i), 0.0);
        }

        // Add edges at their diameter
        for i in 0..n {
            for j in i + 1..n {
                let d = dist[i * n + j];
                if d <= self.max_scale {
                    filtration.add(Simplex::edge(i, j), d);
                }
            }
        }

        // Add higher simplices (up to max_dim)
        if self.max_dim >= 2 {
            // Triangles
            for i in 0..n {
                for j in i + 1..n {
                    for k in j + 1..n {
                        let d_ij = dist[i * n + j];
                        let d_ik = dist[i * n + k];
                        let d_jk = dist[j * n + k];
                        let diameter = d_ij.max(d_ik).max(d_jk);

                        if diameter <= self.max_scale {
                            filtration.add(Simplex::triangle(i, j, k), diameter);
                        }
                    }
                }
            }
        }

        if self.max_dim >= 3 {
            // Tetrahedra
            for i in 0..n {
                for j in i + 1..n {
                    for k in j + 1..n {
                        for l in k + 1..n {
                            let d_ij = dist[i * n + j];
                            let d_ik = dist[i * n + k];
                            let d_il = dist[i * n + l];
                            let d_jk = dist[j * n + k];
                            let d_jl = dist[j * n + l];
                            let d_kl = dist[k * n + l];
                            let diameter = d_ij.max(d_ik).max(d_il).max(d_jk).max(d_jl).max(d_kl);

                            if diameter <= self.max_scale {
                                filtration.add(Simplex::new(vec![i, j, k, l]), diameter);
                            }
                        }
                    }
                }
            }
        }

        filtration.sort();
        filtration
    }
}

/// Alpha complex filtration (more efficient than Rips for low dimensions)
///
/// Based on Delaunay triangulation with radius filtering.
#[derive(Debug, Clone)]
pub struct AlphaComplex {
    /// Maximum alpha value
    pub max_alpha: f64,
}

impl AlphaComplex {
    /// Create with maximum alpha
    pub fn new(max_alpha: f64) -> Self {
        Self { max_alpha }
    }

    /// Build filtration from point cloud (simplified version)
    ///
    /// Note: Full alpha complex requires Delaunay triangulation.
    /// This is a simplified version that approximates using distance thresholds.
    pub fn build(&self, cloud: &PointCloud) -> Filtration {
        let n = cloud.len();
        let dist = cloud.distance_matrix();

        let mut filtration = Filtration::new();

        // Vertices at time 0
        for i in 0..n {
            filtration.add(Simplex::vertex(i), 0.0);
        }

        // Edges: birth time is half the distance (radius, not diameter)
        for i in 0..n {
            for j in i + 1..n {
                let alpha = dist[i * n + j] / 2.0;
                if alpha <= self.max_alpha {
                    filtration.add(Simplex::edge(i, j), alpha);
                }
            }
        }

        // Triangles: birth time based on circumradius approximation
        for i in 0..n {
            for j in i + 1..n {
                for k in j + 1..n {
                    let d_ij = dist[i * n + j];
                    let d_ik = dist[i * n + k];
                    let d_jk = dist[j * n + k];

                    // Approximate circumradius
                    let s = (d_ij + d_ik + d_jk) / 2.0;
                    let area_sq = s * (s - d_ij) * (s - d_ik) * (s - d_jk);
                    let alpha = if area_sq > 0.0 {
                        (d_ij * d_ik * d_jk) / (4.0 * area_sq.sqrt())
                    } else {
                        d_ij.max(d_ik).max(d_jk) / 2.0
                    };

                    if alpha <= self.max_alpha {
                        filtration.add(Simplex::triangle(i, j, k), alpha);
                    }
                }
            }
        }

        filtration.sort();
        filtration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filtration_creation() {
        let mut filtration = Filtration::new();
        filtration.add(Simplex::vertex(0), 0.0);
        filtration.add(Simplex::vertex(1), 0.0);
        filtration.add(Simplex::edge(0, 1), 1.0);

        assert_eq!(filtration.len(), 3);
    }

    #[test]
    fn test_filtration_sort() {
        let mut filtration = Filtration::new();
        filtration.add(Simplex::edge(0, 1), 1.0);
        filtration.add(Simplex::vertex(0), 0.0);
        filtration.add(Simplex::vertex(1), 0.0);

        filtration.sort();

        // Vertices should come before edge
        assert!(filtration.simplices[0].simplex.is_vertex());
        assert!(filtration.simplices[1].simplex.is_vertex());
        assert!(filtration.simplices[2].simplex.is_edge());
    }

    #[test]
    fn test_vietoris_rips() {
        // Triangle of points
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0, 0.5, 0.866], 2);
        let rips = VietorisRips::new(2, 2.0);

        let filtration = rips.build(&cloud);

        // Should have 3 vertices, 3 edges, 1 triangle
        let values = filtration.filtration_values();
        assert!(!values.is_empty());
    }

    #[test]
    fn test_complex_at() {
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0, 2.0, 0.0], 2);
        let rips = VietorisRips::new(1, 2.0);
        let filtration = rips.build(&cloud);

        // At scale 0.5, only vertices
        let complex_0 = filtration.complex_at(0.5);
        assert_eq!(complex_0.count_dim(0), 3);
        assert_eq!(complex_0.count_dim(1), 0);

        // At scale 1.5, vertices and adjacent edges
        let complex_1 = filtration.complex_at(1.5);
        assert_eq!(complex_1.count_dim(0), 3);
        assert!(complex_1.count_dim(1) >= 2); // At least edges 0-1 and 1-2
    }

    #[test]
    fn test_alpha_complex() {
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0], 2);
        let alpha = AlphaComplex::new(2.0);

        let filtration = alpha.build(&cloud);

        assert!(filtration.len() >= 3); // At least vertices
    }
}
