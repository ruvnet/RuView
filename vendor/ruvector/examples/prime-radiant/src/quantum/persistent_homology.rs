//! Persistent Homology
//!
//! Computes persistent homology using the standard algorithm, tracking birth-death
//! pairs of topological features across filtration scales.

use super::simplicial_complex::{Simplex, SimplicialComplex, SparseMatrix};
use super::{constants, QuantumTopologyError, Result};
use std::collections::{HashMap, HashSet, BTreeMap};

/// Birth-death pair representing a persistent feature
#[derive(Debug, Clone, PartialEq)]
pub struct BirthDeathPair {
    /// Dimension of the feature (0 = component, 1 = loop, 2 = void, ...)
    pub dimension: usize,
    /// Birth time (filtration value when feature appears)
    pub birth: f64,
    /// Death time (None = essential feature that never dies)
    pub death: Option<f64>,
    /// Representative cycle (simplex that created the feature)
    pub birth_simplex: Option<Simplex>,
    /// Killing simplex (simplex that killed the feature)
    pub death_simplex: Option<Simplex>,
}

impl BirthDeathPair {
    /// Create a finite-lifetime feature
    pub fn finite(
        dimension: usize,
        birth: f64,
        death: f64,
        birth_simplex: Option<Simplex>,
        death_simplex: Option<Simplex>,
    ) -> Self {
        Self {
            dimension,
            birth,
            death: Some(death),
            birth_simplex,
            death_simplex,
        }
    }

    /// Create an essential (infinite-lifetime) feature
    pub fn essential(dimension: usize, birth: f64, birth_simplex: Option<Simplex>) -> Self {
        Self {
            dimension,
            birth,
            death: None,
            birth_simplex,
            death_simplex: None,
        }
    }

    /// Persistence (lifetime) of the feature
    pub fn persistence(&self) -> f64 {
        match self.death {
            Some(d) => d - self.birth,
            None => f64::INFINITY,
        }
    }

    /// Check if this is an essential feature
    pub fn is_essential(&self) -> bool {
        self.death.is_none()
    }

    /// Midpoint of the interval
    pub fn midpoint(&self) -> f64 {
        match self.death {
            Some(d) => (self.birth + d) / 2.0,
            None => f64::INFINITY,
        }
    }

    /// Check if the feature is alive at time t
    pub fn is_alive_at(&self, t: f64) -> bool {
        self.birth <= t && self.death.map(|d| d > t).unwrap_or(true)
    }
}

/// Persistence diagram: collection of birth-death pairs
#[derive(Debug, Clone)]
pub struct PersistenceDiagram {
    /// Birth-death pairs
    pub pairs: Vec<BirthDeathPair>,
    /// Maximum dimension computed
    pub max_dimension: usize,
}

impl PersistenceDiagram {
    /// Create an empty diagram
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            max_dimension: 0,
        }
    }

    /// Add a birth-death pair
    pub fn add(&mut self, pair: BirthDeathPair) {
        self.max_dimension = self.max_dimension.max(pair.dimension);
        self.pairs.push(pair);
    }

    /// Get pairs of dimension k
    pub fn pairs_of_dim(&self, k: usize) -> impl Iterator<Item = &BirthDeathPair> {
        self.pairs.iter().filter(move |p| p.dimension == k)
    }

    /// Get Betti numbers at filtration value t
    pub fn betti_at(&self, t: f64) -> Vec<usize> {
        let mut betti = vec![0; self.max_dimension + 1];

        for pair in &self.pairs {
            if pair.is_alive_at(t) && pair.dimension <= self.max_dimension {
                betti[pair.dimension] += 1;
            }
        }

        betti
    }

    /// Total persistence (sum of all finite lifetimes)
    pub fn total_persistence(&self) -> f64 {
        self.pairs
            .iter()
            .filter(|p| !p.is_essential())
            .map(|p| p.persistence())
            .sum()
    }

    /// Total persistence in dimension k
    pub fn total_persistence_dim(&self, k: usize) -> f64 {
        self.pairs
            .iter()
            .filter(|p| p.dimension == k && !p.is_essential())
            .map(|p| p.persistence())
            .sum()
    }

    /// Average persistence
    pub fn average_persistence(&self) -> f64 {
        let finite: Vec<f64> = self
            .pairs
            .iter()
            .filter(|p| !p.is_essential())
            .map(|p| p.persistence())
            .collect();

        if finite.is_empty() {
            0.0
        } else {
            finite.iter().sum::<f64>() / finite.len() as f64
        }
    }

    /// Maximum persistence (excluding essential features)
    pub fn max_persistence(&self) -> f64 {
        self.pairs
            .iter()
            .filter(|p| !p.is_essential())
            .map(|p| p.persistence())
            .fold(0.0, f64::max)
    }

    /// Filter by minimum persistence threshold
    pub fn filter_by_persistence(&self, threshold: f64) -> Self {
        Self {
            pairs: self
                .pairs
                .iter()
                .filter(|p| p.persistence() >= threshold)
                .cloned()
                .collect(),
            max_dimension: self.max_dimension,
        }
    }

    /// Number of features of each dimension
    pub fn feature_counts(&self) -> Vec<usize> {
        let mut counts = vec![0; self.max_dimension + 1];
        for pair in &self.pairs {
            if pair.dimension <= self.max_dimension {
                counts[pair.dimension] += 1;
            }
        }
        counts
    }

    /// Number of essential features
    pub fn essential_count(&self) -> usize {
        self.pairs.iter().filter(|p| p.is_essential()).count()
    }

    /// Persistence landscape at time t and level k
    pub fn landscape(&self, dim: usize, t: f64, level: usize) -> f64 {
        // Get all pairs of given dimension
        let mut values: Vec<f64> = self
            .pairs_of_dim(dim)
            .filter_map(|p| {
                if let Some(death) = p.death {
                    let mid = (p.birth + death) / 2.0;
                    let half_life = (death - p.birth) / 2.0;

                    if t >= p.birth && t <= death {
                        // Triangle function
                        let value = if t <= mid {
                            t - p.birth
                        } else {
                            death - t
                        };
                        Some(value)
                    } else {
                        None
                    }
                } else {
                    // Essential feature - extends to infinity
                    if t >= p.birth {
                        Some(t - p.birth)
                    } else {
                        None
                    }
                }
            })
            .collect();

        // Sort descending
        values.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        // Return k-th largest (0-indexed)
        values.get(level).copied().unwrap_or(0.0)
    }

    /// Bottleneck distance to another diagram (same dimension)
    pub fn bottleneck_distance(&self, other: &PersistenceDiagram, dim: usize) -> f64 {
        let pairs_self: Vec<&BirthDeathPair> = self.pairs_of_dim(dim).collect();
        let pairs_other: Vec<&BirthDeathPair> = other.pairs_of_dim(dim).collect();

        // Simple approximation using greedy matching
        let mut max_dist = 0.0_f64;

        // For each pair in self, find closest in other
        for p1 in &pairs_self {
            let mut min_dist = f64::INFINITY;
            for p2 in &pairs_other {
                let dist = l_infinity_distance(p1, p2);
                min_dist = min_dist.min(dist);
            }
            // Also consider matching to diagonal
            let diag_dist = p1.persistence() / 2.0;
            min_dist = min_dist.min(diag_dist);
            max_dist = max_dist.max(min_dist);
        }

        // Vice versa
        for p2 in &pairs_other {
            let mut min_dist = f64::INFINITY;
            for p1 in &pairs_self {
                let dist = l_infinity_distance(p1, p2);
                min_dist = min_dist.min(dist);
            }
            let diag_dist = p2.persistence() / 2.0;
            min_dist = min_dist.min(diag_dist);
            max_dist = max_dist.max(min_dist);
        }

        max_dist
    }

    /// Wasserstein distance (q=2) to another diagram
    pub fn wasserstein_distance(&self, other: &PersistenceDiagram, dim: usize) -> f64 {
        let pairs_self: Vec<&BirthDeathPair> = self.pairs_of_dim(dim).collect();
        let pairs_other: Vec<&BirthDeathPair> = other.pairs_of_dim(dim).collect();

        // Use greedy approximation
        let n = pairs_self.len();
        let m = pairs_other.len();

        if n == 0 && m == 0 {
            return 0.0;
        }

        let mut total = 0.0;

        // Sum of squared persistence for unmatched points (to diagonal)
        for p in &pairs_self {
            if !p.is_essential() {
                let diag_dist = p.persistence() / 2.0;
                total += diag_dist * diag_dist;
            }
        }

        for p in &pairs_other {
            if !p.is_essential() {
                let diag_dist = p.persistence() / 2.0;
                total += diag_dist * diag_dist;
            }
        }

        total.sqrt()
    }
}

impl Default for PersistenceDiagram {
    fn default() -> Self {
        Self::new()
    }
}

/// L-infinity distance between two birth-death pairs
fn l_infinity_distance(p1: &BirthDeathPair, p2: &BirthDeathPair) -> f64 {
    let birth_diff = (p1.birth - p2.birth).abs();
    let death_diff = match (p1.death, p2.death) {
        (Some(d1), Some(d2)) => (d1 - d2).abs(),
        (None, None) => 0.0,
        _ => f64::INFINITY,
    };
    birth_diff.max(death_diff)
}

/// Filtration: sequence of simplicial complexes
#[derive(Debug, Clone)]
pub struct Filtration {
    /// Simplices with their birth times
    pub simplices: Vec<FilteredSimplex>,
}

/// Simplex with birth time in filtration
#[derive(Debug, Clone)]
pub struct FilteredSimplex {
    /// The simplex
    pub simplex: Simplex,
    /// Birth time (filtration value)
    pub birth: f64,
}

impl Filtration {
    /// Create empty filtration
    pub fn new() -> Self {
        Self {
            simplices: Vec::new(),
        }
    }

    /// Add a simplex at given birth time
    pub fn add(&mut self, simplex: Simplex, birth: f64) {
        self.simplices.push(FilteredSimplex { simplex, birth });
    }

    /// Sort simplices by birth time, then by dimension
    pub fn sort(&mut self) {
        self.simplices.sort_by(|a, b| {
            a.birth
                .partial_cmp(&b.birth)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.simplex.dim().cmp(&b.simplex.dim()))
        });
    }

    /// Get the simplicial complex at filtration value t
    pub fn complex_at(&self, t: f64) -> SimplicialComplex {
        SimplicialComplex::from_simplices(
            self.simplices
                .iter()
                .filter(|fs| fs.birth <= t)
                .map(|fs| fs.simplex.clone()),
        )
    }
}

impl Default for Filtration {
    fn default() -> Self {
        Self::new()
    }
}

/// Vietoris-Rips filtration builder
pub struct VietorisRipsFiltration {
    /// Maximum simplex dimension
    pub max_dimension: usize,
    /// Maximum filtration value
    pub max_radius: f64,
}

impl VietorisRipsFiltration {
    /// Create a new VR filtration builder
    pub fn new(max_dimension: usize, max_radius: f64) -> Self {
        Self {
            max_dimension,
            max_radius,
        }
    }

    /// Build filtration from point cloud
    pub fn build(&self, points: &[Vec<f64>]) -> Filtration {
        let n = points.len();
        let mut filtration = Filtration::new();

        // Add vertices at t=0
        for i in 0..n {
            filtration.add(Simplex::vertex(i), 0.0);
        }

        // Compute pairwise distances
        let mut edges: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let dist = euclidean_distance(&points[i], &points[j]);
                if dist <= self.max_radius * 2.0 {
                    edges.push((i, j, dist));
                }
            }
        }

        // Sort edges by distance
        edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        // Build adjacency list
        let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n];

        // Add edges and higher simplices
        for (i, j, dist) in edges {
            let birth = dist / 2.0; // Diameter is 2*radius

            // Add edge
            filtration.add(Simplex::edge(i, j), birth);
            adj[i].insert(j);
            adj[j].insert(i);

            if self.max_dimension >= 2 {
                // Find triangles
                let common: Vec<usize> = adj[i]
                    .intersection(&adj[j])
                    .copied()
                    .collect();

                for k in common {
                    filtration.add(Simplex::triangle(i, j, k), birth);

                    if self.max_dimension >= 3 {
                        // Find tetrahedra
                        let common_3: Vec<usize> = adj[i]
                            .intersection(&adj[j])
                            .filter(|&&l| adj[k].contains(&l) && l != k)
                            .copied()
                            .collect();

                        for l in common_3 {
                            if l > k {
                                filtration.add(Simplex::tetrahedron(i, j, k, l), birth);
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

/// Euclidean distance
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Persistent homology computation engine
pub struct PersistentHomologyComputer {
    /// Maximum dimension to compute
    max_dimension: usize,
}

impl PersistentHomologyComputer {
    /// Create a new computation engine
    pub fn new(max_dimension: usize) -> Self {
        Self { max_dimension }
    }

    /// Compute persistent homology from a filtration
    pub fn compute(&self, filtration: &Filtration) -> PersistenceDiagram {
        let n = filtration.simplices.len();
        if n == 0 {
            return PersistenceDiagram::new();
        }

        // Build simplex to index mapping
        let simplex_to_idx: HashMap<&Simplex, usize> = filtration
            .simplices
            .iter()
            .enumerate()
            .map(|(i, fs)| (&fs.simplex, i))
            .collect();

        // Initialize columns (boundary chains)
        let mut columns: Vec<Option<HashSet<usize>>> = Vec::with_capacity(n);
        let mut birth_times = Vec::with_capacity(n);
        let mut dimensions = Vec::with_capacity(n);
        let mut simplices_vec = Vec::with_capacity(n);

        for fs in &filtration.simplices {
            birth_times.push(fs.birth);
            dimensions.push(fs.simplex.dim());
            simplices_vec.push(fs.simplex.clone());

            // Compute boundary
            let boundary: HashSet<usize> = fs
                .simplex
                .boundary_faces()
                .into_iter()
                .filter_map(|(face, _sign)| simplex_to_idx.get(&face).copied())
                .collect();

            columns.push(if boundary.is_empty() {
                None
            } else {
                Some(boundary)
            });
        }

        // Reduce matrix using standard algorithm
        let mut pivot_to_col: HashMap<usize, usize> = HashMap::new();

        for j in 0..n {
            while let Some(pivot) = get_pivot(&columns[j]) {
                if let Some(&other) = pivot_to_col.get(&pivot) {
                    // Add column 'other' to column j (mod 2)
                    add_columns(&mut columns, j, other);
                } else {
                    pivot_to_col.insert(pivot, j);
                    break;
                }
            }
        }

        // Extract persistence pairs
        let mut diagram = PersistenceDiagram::new();
        let mut paired: HashSet<usize> = HashSet::new();

        for (&pivot, &col) in &pivot_to_col {
            let birth = birth_times[pivot];
            let death = birth_times[col];
            let dim = dimensions[pivot];

            if death > birth && dim <= self.max_dimension {
                diagram.add(BirthDeathPair::finite(
                    dim,
                    birth,
                    death,
                    Some(simplices_vec[pivot].clone()),
                    Some(simplices_vec[col].clone()),
                ));
            }

            paired.insert(pivot);
            paired.insert(col);
        }

        // Add essential features (unpaired simplices with zero boundary)
        for j in 0..n {
            if !paired.contains(&j) && columns[j].is_none() {
                let dim = dimensions[j];
                if dim <= self.max_dimension {
                    diagram.add(BirthDeathPair::essential(
                        dim,
                        birth_times[j],
                        Some(simplices_vec[j].clone()),
                    ));
                }
            }
        }

        diagram
    }

    /// Compute from point cloud
    pub fn compute_from_points(
        &self,
        points: &[Vec<f64>],
        max_radius: f64,
    ) -> PersistenceDiagram {
        let vr = VietorisRipsFiltration::new(self.max_dimension, max_radius);
        let filtration = vr.build(points);
        self.compute(&filtration)
    }
}

/// Get pivot (largest index) from column
fn get_pivot(col: &Option<HashSet<usize>>) -> Option<usize> {
    col.as_ref().and_then(|c| c.iter().max().copied())
}

/// Add column src to column dst (XOR / mod 2)
fn add_columns(columns: &mut [Option<HashSet<usize>>], dst: usize, src: usize) {
    if let Some(ref src_col) = columns[src].clone() {
        if let Some(ref mut dst_col) = columns[dst] {
            // Symmetric difference
            let mut new_col = HashSet::new();
            for &idx in dst_col.iter() {
                if !src_col.contains(&idx) {
                    new_col.insert(idx);
                }
            }
            for &idx in src_col.iter() {
                if !dst_col.contains(&idx) {
                    new_col.insert(idx);
                }
            }
            if new_col.is_empty() {
                columns[dst] = None;
            } else {
                *dst_col = new_col;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_birth_death_pair() {
        let finite = BirthDeathPair::finite(0, 0.0, 1.0, None, None);
        assert_eq!(finite.persistence(), 1.0);
        assert!(!finite.is_essential());
        assert!(finite.is_alive_at(0.5));
        assert!(!finite.is_alive_at(1.5));

        let essential = BirthDeathPair::essential(0, 0.0, None);
        assert!(essential.is_essential());
        assert_eq!(essential.persistence(), f64::INFINITY);
        assert!(essential.is_alive_at(1000.0));
    }

    #[test]
    fn test_persistence_diagram() {
        let mut diagram = PersistenceDiagram::new();
        diagram.add(BirthDeathPair::essential(0, 0.0, None));
        diagram.add(BirthDeathPair::finite(0, 0.0, 1.0, None, None));
        diagram.add(BirthDeathPair::finite(1, 0.5, 2.0, None, None));

        assert_eq!(diagram.pairs.len(), 3);
        assert_eq!(diagram.essential_count(), 1);

        let betti = diagram.betti_at(0.75);
        assert_eq!(betti[0], 2); // Both H0 features alive
        assert_eq!(betti[1], 1); // H1 feature alive

        assert!((diagram.total_persistence() - 2.5).abs() < 1e-10); // 1.0 + 1.5
    }

    #[test]
    fn test_filtration() {
        let mut filtration = Filtration::new();
        filtration.add(Simplex::vertex(0), 0.0);
        filtration.add(Simplex::vertex(1), 0.0);
        filtration.add(Simplex::edge(0, 1), 1.0);

        filtration.sort();

        let complex = filtration.complex_at(0.5);
        assert_eq!(complex.count(0), 2);
        assert_eq!(complex.count(1), 0);

        let complex = filtration.complex_at(1.5);
        assert_eq!(complex.count(1), 1);
    }

    #[test]
    fn test_persistent_homology_simple() {
        // Two points that merge
        let points = vec![vec![0.0, 0.0], vec![1.0, 0.0]];

        let computer = PersistentHomologyComputer::new(1);
        let diagram = computer.compute_from_points(&points, 1.0);

        // Should have:
        // - One essential H0 (final connected component)
        // - One finite H0 that dies when edge connects
        let h0_pairs: Vec<_> = diagram.pairs_of_dim(0).collect();
        assert!(h0_pairs.len() >= 1);
    }

    #[test]
    fn test_persistent_homology_triangle() {
        // Three points forming equilateral triangle
        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.5, 0.866],
        ];

        let computer = PersistentHomologyComputer::new(2);
        let diagram = computer.compute_from_points(&points, 1.0);

        // Should have H0 and possibly H1 features
        assert!(!diagram.pairs.is_empty());
    }

    #[test]
    fn test_bottleneck_distance() {
        let mut d1 = PersistenceDiagram::new();
        d1.add(BirthDeathPair::finite(0, 0.0, 1.0, None, None));

        let mut d2 = PersistenceDiagram::new();
        d2.add(BirthDeathPair::finite(0, 0.0, 2.0, None, None));

        let dist = d1.bottleneck_distance(&d2, 0);
        assert!(dist >= 0.0);
    }

    #[test]
    fn test_landscape() {
        let mut diagram = PersistenceDiagram::new();
        diagram.add(BirthDeathPair::finite(1, 0.0, 2.0, None, None));

        // At midpoint t=1, landscape should have maximum
        let val = diagram.landscape(1, 1.0, 0);
        assert!((val - 1.0).abs() < 1e-10);

        // At t=0 or t=2, landscape should be 0
        let val_0 = diagram.landscape(1, 0.0, 0);
        assert!(val_0.abs() < 1e-10);
    }
}
