//! Sheaf implementation

use super::{BettiNumbers, ChainComplex, Homology, Presheaf, Section};
use crate::{Error, Result};
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

/// A sheaf over a topological space
///
/// A sheaf is a presheaf that satisfies the gluing axioms:
/// 1. Locality: Sections that agree on overlaps are equal
/// 2. Gluing: Compatible sections can be glued to a global section
#[derive(Debug, Clone)]
pub struct Sheaf {
    /// Underlying presheaf
    presheaf: Presheaf,
    /// Cached cohomology groups
    cohomology_cache: HashMap<usize, Homology>,
}

impl Sheaf {
    /// Create a new sheaf from a presheaf
    pub fn from_presheaf(presheaf: Presheaf) -> Self {
        Self {
            presheaf,
            cohomology_cache: HashMap::new(),
        }
    }

    /// Create a sheaf from neural network activations
    ///
    /// Treats each layer as an open set with the activation vectors as sections
    pub fn from_activations(layers: &[DVector<f64>]) -> Result<Self> {
        if layers.is_empty() {
            return Err(Error::InvalidTopology("Empty layer list".to_string()));
        }

        let mut presheaf = Presheaf::new();

        // Add each layer as a section
        for (i, activations) in layers.iter().enumerate() {
            presheaf = presheaf.section(format!("layer_{}", i), activations.clone());
        }

        // Add identity restrictions (simplified topology)
        // In practice, you'd derive these from weight matrices

        Ok(Self::from_presheaf(presheaf))
    }

    /// Create a sheaf builder
    pub fn builder() -> SheafBuilder {
        SheafBuilder::new()
    }

    /// Get the underlying presheaf
    pub fn presheaf(&self) -> &Presheaf {
        &self.presheaf
    }

    /// Compute the n-th cohomology group H^n(X, F)
    ///
    /// Cohomology measures obstructions to extending local sections globally.
    pub fn cohomology(&self, degree: usize) -> Result<Homology> {
        // Check cache first
        if let Some(cached) = self.cohomology_cache.get(&degree) {
            return Ok(cached.clone());
        }

        // Build the Cech complex and compute cohomology
        let complex = self.cech_complex()?;
        let homology = complex.homology(degree)?;

        Ok(homology)
    }

    /// Compute all Betti numbers up to a given degree
    pub fn betti_numbers(&self, max_degree: usize) -> Result<BettiNumbers> {
        let mut betti = BettiNumbers::default();

        for degree in 0..=max_degree {
            let h = self.cohomology(degree)?;
            match degree {
                0 => betti.b0 = h.dimension(),
                1 => betti.b1 = h.dimension(),
                2 => betti.b2 = h.dimension(),
                _ => betti.higher.push(h.dimension()),
            }
        }

        Ok(betti)
    }

    /// Compute persistent homology for multi-scale analysis
    pub fn persistent_homology(&self) -> Result<PersistenceDiagram> {
        // Compute homology at multiple filtration levels
        let mut persistence = PersistenceDiagram::new();

        // Simplified: compute at single scale
        let h0 = self.cohomology(0)?;
        let h1 = self.cohomology(1)?;

        persistence.add_bar(0, 0.0, f64::INFINITY, h0.dimension());
        persistence.add_bar(1, 0.0, f64::INFINITY, h1.dimension());

        Ok(persistence)
    }

    /// Build the Cech complex for cohomology computation
    fn cech_complex(&self) -> Result<ChainComplex> {
        // The Cech complex is built from intersections of open sets
        // C^0: Direct product of all F(U_i)
        // C^1: Direct product of all F(U_i ∩ U_j)
        // etc.

        let open_sets = self.presheaf.open_sets();
        let n = open_sets.len();

        if n == 0 {
            return Err(Error::InvalidTopology("No open sets".to_string()));
        }

        // Build boundary maps
        // For simplicity, use identity matrices as placeholder
        let dim = self
            .presheaf
            .get_section(open_sets[0])
            .map(|s| s.dimension())
            .unwrap_or(1);

        let d0 = DMatrix::zeros(dim, dim);
        let d1 = DMatrix::zeros(dim, dim);

        Ok(ChainComplex::new(vec![d0, d1]))
    }

    /// Compute the Euler characteristic
    pub fn euler_characteristic(&self) -> Result<i64> {
        let betti = self.betti_numbers(2)?;
        Ok(betti.euler_characteristic())
    }

    /// Check if the sheaf is locally constant
    pub fn is_locally_constant(&self, epsilon: f64) -> Result<bool> {
        self.presheaf.check_functoriality(epsilon)
    }
}

/// Builder for constructing sheaves
#[derive(Debug, Default)]
pub struct SheafBuilder {
    presheaf: Presheaf,
}

impl SheafBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            presheaf: Presheaf::new(),
        }
    }

    /// Add a section
    pub fn section(mut self, domain: impl Into<String>, values: DVector<f64>) -> Self {
        self.presheaf = self.presheaf.section(domain, values);
        self
    }

    /// Add a restriction map
    pub fn restriction(
        mut self,
        source: impl Into<String>,
        target: impl Into<String>,
        matrix: DMatrix<f64>,
    ) -> Self {
        self.presheaf = self.presheaf.restriction(source, target, matrix);
        self
    }

    /// Build the sheaf
    pub fn build(self) -> Result<Sheaf> {
        self.presheaf.to_sheaf()
    }
}

/// Persistence diagram for topological data analysis
#[derive(Debug, Clone, Default)]
pub struct PersistenceDiagram {
    /// Bars (birth, death, multiplicity) by dimension
    bars: HashMap<usize, Vec<(f64, f64, usize)>>,
}

impl PersistenceDiagram {
    /// Create a new persistence diagram
    pub fn new() -> Self {
        Self {
            bars: HashMap::new(),
        }
    }

    /// Add a persistence bar
    pub fn add_bar(&mut self, dimension: usize, birth: f64, death: f64, multiplicity: usize) {
        self.bars
            .entry(dimension)
            .or_default()
            .push((birth, death, multiplicity));
    }

    /// Get bars for a given dimension
    pub fn bars(&self, dimension: usize) -> &[(f64, f64, usize)] {
        self.bars.get(&dimension).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Compute bottleneck distance to another diagram
    pub fn bottleneck_distance(&self, other: &PersistenceDiagram) -> f64 {
        // Simplified implementation
        let mut max_dist = 0.0f64;

        for dim in 0..=2 {
            let self_bars = self.bars(dim);
            let other_bars = other.bars(dim);

            // Compare number of bars
            let diff = (self_bars.len() as f64 - other_bars.len() as f64).abs();
            max_dist = max_dist.max(diff);
        }

        max_dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sheaf_from_activations() {
        let layers = vec![
            DVector::from_vec(vec![1.0, 2.0, 3.0]),
            DVector::from_vec(vec![0.5, 1.5]),
        ];

        let sheaf = Sheaf::from_activations(&layers).unwrap();
        assert!(sheaf.presheaf().get_section("layer_0").is_some());
        assert!(sheaf.presheaf().get_section("layer_1").is_some());
    }

    #[test]
    fn test_sheaf_builder() {
        let sheaf = Sheaf::builder()
            .section("U", DVector::from_vec(vec![1.0, 2.0]))
            .section("V", DVector::from_vec(vec![1.0]))
            .build()
            .unwrap();

        assert!(sheaf.presheaf().get_section("U").is_some());
    }
}
