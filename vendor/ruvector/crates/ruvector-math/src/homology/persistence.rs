//! Persistent Homology Computation
//!
//! Compute birth-death pairs from a filtration using the standard algorithm.

use super::{BettiNumbers, Filtration, Simplex};
use std::collections::{HashMap, HashSet};

/// Birth-death pair in persistence diagram
#[derive(Debug, Clone, PartialEq)]
pub struct BirthDeathPair {
    /// Dimension of the feature (0 = component, 1 = loop, ...)
    pub dimension: usize,
    /// Birth time
    pub birth: f64,
    /// Death time (None = essential, lives forever)
    pub death: Option<f64>,
}

impl BirthDeathPair {
    /// Create finite interval
    pub fn finite(dimension: usize, birth: f64, death: f64) -> Self {
        Self {
            dimension,
            birth,
            death: Some(death),
        }
    }

    /// Create essential (infinite) interval
    pub fn essential(dimension: usize, birth: f64) -> Self {
        Self {
            dimension,
            birth,
            death: None,
        }
    }

    /// Persistence (lifetime) of feature
    pub fn persistence(&self) -> f64 {
        match self.death {
            Some(d) => d - self.birth,
            None => f64::INFINITY,
        }
    }

    /// Is this an essential feature (never dies)?
    pub fn is_essential(&self) -> bool {
        self.death.is_none()
    }

    /// Midpoint of interval
    pub fn midpoint(&self) -> f64 {
        match self.death {
            Some(d) => (self.birth + d) / 2.0,
            None => f64::INFINITY,
        }
    }
}

/// Persistence diagram: collection of birth-death pairs
#[derive(Debug, Clone)]
pub struct PersistenceDiagram {
    /// Birth-death pairs
    pub pairs: Vec<BirthDeathPair>,
    /// Maximum dimension
    pub max_dim: usize,
}

impl PersistenceDiagram {
    /// Create empty diagram
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            max_dim: 0,
        }
    }

    /// Add a pair
    pub fn add(&mut self, pair: BirthDeathPair) {
        self.max_dim = self.max_dim.max(pair.dimension);
        self.pairs.push(pair);
    }

    /// Get pairs of dimension d
    pub fn pairs_of_dim(&self, d: usize) -> impl Iterator<Item = &BirthDeathPair> {
        self.pairs.iter().filter(move |p| p.dimension == d)
    }

    /// Get Betti numbers at scale t
    pub fn betti_at(&self, t: f64) -> BettiNumbers {
        let mut b0 = 0;
        let mut b1 = 0;
        let mut b2 = 0;

        for pair in &self.pairs {
            let alive = pair.birth <= t && pair.death.map(|d| d > t).unwrap_or(true);
            if alive {
                match pair.dimension {
                    0 => b0 += 1,
                    1 => b1 += 1,
                    2 => b2 += 1,
                    _ => {}
                }
            }
        }

        BettiNumbers::new(b0, b1, b2)
    }

    /// Get total persistence (sum of lifetimes)
    pub fn total_persistence(&self) -> f64 {
        self.pairs
            .iter()
            .filter(|p| !p.is_essential())
            .map(|p| p.persistence())
            .sum()
    }

    /// Get average persistence
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

    /// Filter by minimum persistence
    pub fn filter_by_persistence(&self, min_persistence: f64) -> Self {
        Self {
            pairs: self
                .pairs
                .iter()
                .filter(|p| p.persistence() >= min_persistence)
                .cloned()
                .collect(),
            max_dim: self.max_dim,
        }
    }

    /// Number of features of each dimension
    pub fn feature_counts(&self) -> Vec<usize> {
        let mut counts = vec![0; self.max_dim + 1];
        for pair in &self.pairs {
            if pair.dimension <= self.max_dim {
                counts[pair.dimension] += 1;
            }
        }
        counts
    }
}

impl Default for PersistenceDiagram {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent homology computation
pub struct PersistentHomology {
    /// Working column representation (reduced boundary matrix)
    columns: Vec<Option<HashSet<usize>>>,
    /// Pivot to column mapping
    pivot_to_col: HashMap<usize, usize>,
    /// Birth times
    birth_times: Vec<f64>,
    /// Simplex dimensions
    dimensions: Vec<usize>,
}

impl PersistentHomology {
    /// Compute persistence from filtration
    pub fn compute(filtration: &Filtration) -> PersistenceDiagram {
        let mut ph = Self {
            columns: Vec::new(),
            pivot_to_col: HashMap::new(),
            birth_times: Vec::new(),
            dimensions: Vec::new(),
        };

        ph.run(filtration)
    }

    fn run(&mut self, filtration: &Filtration) -> PersistenceDiagram {
        let n = filtration.simplices.len();
        if n == 0 {
            return PersistenceDiagram::new();
        }

        // Build simplex index mapping
        let simplex_to_idx: HashMap<&Simplex, usize> = filtration
            .simplices
            .iter()
            .enumerate()
            .map(|(i, fs)| (&fs.simplex, i))
            .collect();

        // Initialize
        self.columns = Vec::with_capacity(n);
        self.birth_times = filtration.simplices.iter().map(|fs| fs.birth).collect();
        self.dimensions = filtration
            .simplices
            .iter()
            .map(|fs| fs.simplex.dim())
            .collect();

        // Build boundary matrix columns
        for fs in &filtration.simplices {
            let boundary = self.boundary(&fs.simplex, &simplex_to_idx);
            self.columns.push(if boundary.is_empty() {
                None
            } else {
                Some(boundary)
            });
        }

        // Reduce matrix
        self.reduce();

        // Extract persistence pairs
        self.extract_pairs()
    }

    /// Compute boundary of simplex as set of face indices
    fn boundary(&self, simplex: &Simplex, idx_map: &HashMap<&Simplex, usize>) -> HashSet<usize> {
        let mut boundary = HashSet::new();
        for face in simplex.faces() {
            if let Some(&idx) = idx_map.get(&face) {
                boundary.insert(idx);
            }
        }
        boundary
    }

    /// Reduce using standard persistence algorithm
    fn reduce(&mut self) {
        let n = self.columns.len();

        for j in 0..n {
            // Reduce column j
            while let Some(pivot) = self.get_pivot(j) {
                if let Some(&other) = self.pivot_to_col.get(&pivot) {
                    // Add column 'other' to column j (mod 2)
                    self.add_columns(j, other);
                } else {
                    // No collision, record pivot
                    self.pivot_to_col.insert(pivot, j);
                    break;
                }
            }
        }
    }

    /// Get pivot (largest index) of column
    fn get_pivot(&self, col: usize) -> Option<usize> {
        self.columns[col]
            .as_ref()
            .and_then(|c| c.iter().max().copied())
    }

    /// Add column src to column dst (XOR / mod 2)
    fn add_columns(&mut self, dst: usize, src: usize) {
        let src_col = self.columns[src].clone();
        if let (Some(ref mut dst_col), Some(ref src_col)) = (&mut self.columns[dst], &src_col) {
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
                self.columns[dst] = None;
            } else {
                *dst_col = new_col;
            }
        }
    }

    /// Extract birth-death pairs from reduced matrix
    fn extract_pairs(&self) -> PersistenceDiagram {
        let n = self.columns.len();
        let mut diagram = PersistenceDiagram::new();
        let mut paired = HashSet::new();

        // Process pivot pairs (death creates pair with birth)
        for (&pivot, &col) in &self.pivot_to_col {
            let birth = self.birth_times[pivot];
            let death = self.birth_times[col];
            let dim = self.dimensions[pivot];

            if death > birth {
                diagram.add(BirthDeathPair::finite(dim, birth, death));
            }

            paired.insert(pivot);
            paired.insert(col);
        }

        // Remaining columns are essential (infinite persistence)
        for j in 0..n {
            if !paired.contains(&j) && self.columns[j].is_none() {
                let dim = self.dimensions[j];
                let birth = self.birth_times[j];
                diagram.add(BirthDeathPair::essential(dim, birth));
            }
        }

        diagram
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::homology::{PointCloud, VietorisRips};

    #[test]
    fn test_birth_death_pair() {
        let finite = BirthDeathPair::finite(0, 0.0, 1.0);
        assert_eq!(finite.persistence(), 1.0);
        assert!(!finite.is_essential());

        let essential = BirthDeathPair::essential(0, 0.0);
        assert!(essential.is_essential());
        assert_eq!(essential.persistence(), f64::INFINITY);
    }

    #[test]
    fn test_persistence_diagram() {
        let mut diagram = PersistenceDiagram::new();
        diagram.add(BirthDeathPair::essential(0, 0.0));
        diagram.add(BirthDeathPair::finite(0, 0.0, 1.0));
        diagram.add(BirthDeathPair::finite(1, 0.5, 2.0));

        assert_eq!(diagram.pairs.len(), 3);

        let betti = diagram.betti_at(0.75);
        assert_eq!(betti.b0, 2); // Both 0-dim features alive
        assert_eq!(betti.b1, 1); // 1-dim feature alive
    }

    #[test]
    fn test_persistent_homology_simple() {
        // Two points
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0], 2);
        let rips = VietorisRips::new(1, 2.0);
        let filtration = rips.build(&cloud);

        let diagram = PersistentHomology::compute(&filtration);

        // Should have:
        // - One essential H0 (final connected component)
        // - One finite H0 that dies when edge connects the points
        let h0_pairs: Vec<_> = diagram.pairs_of_dim(0).collect();
        assert!(!h0_pairs.is_empty());
    }

    #[test]
    fn test_persistent_homology_triangle() {
        // Three points forming triangle
        let cloud = PointCloud::from_flat(&[0.0, 0.0, 1.0, 0.0, 0.5, 0.866], 2);
        let rips = VietorisRips::new(2, 2.0);
        let filtration = rips.build(&cloud);

        let diagram = PersistentHomology::compute(&filtration);

        // Should have H0 features (components merging)
        let h0_count = diagram.pairs_of_dim(0).count();
        assert!(h0_count > 0);
    }

    #[test]
    fn test_filter_by_persistence() {
        let mut diagram = PersistenceDiagram::new();
        diagram.add(BirthDeathPair::finite(0, 0.0, 0.1));
        diagram.add(BirthDeathPair::finite(0, 0.0, 1.0));
        diagram.add(BirthDeathPair::essential(0, 0.0));

        let filtered = diagram.filter_by_persistence(0.5);
        assert_eq!(filtered.pairs.len(), 2); // Only persistence >= 0.5
    }

    #[test]
    fn test_feature_counts() {
        let mut diagram = PersistenceDiagram::new();
        diagram.add(BirthDeathPair::finite(0, 0.0, 1.0));
        diagram.add(BirthDeathPair::finite(0, 0.0, 1.0));
        diagram.add(BirthDeathPair::finite(1, 0.0, 1.0));

        let counts = diagram.feature_counts();
        assert_eq!(counts[0], 2);
        assert_eq!(counts[1], 1);
    }
}
