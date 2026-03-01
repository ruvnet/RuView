//! Homology group implementation

use nalgebra::DVector;

/// A homology group H_n
///
/// Homology groups measure "holes" in topological spaces:
/// - H_0: connected components
/// - H_1: loops/tunnels
/// - H_2: voids/cavities
#[derive(Debug, Clone)]
pub struct Homology {
    /// Degree n of the homology group
    degree: usize,
    /// Dimension of the homology group (Betti number)
    dimension: usize,
    /// Generators of the homology group (representative cycles)
    generators: Vec<DVector<f64>>,
}

impl Homology {
    /// Create a new homology group
    pub fn new(degree: usize, dimension: usize) -> Self {
        Self {
            degree,
            dimension,
            generators: Vec::new(),
        }
    }

    /// Create a homology group with generators
    pub fn with_generators(degree: usize, generators: Vec<DVector<f64>>) -> Self {
        let dimension = generators.len();
        Self {
            degree,
            dimension,
            generators,
        }
    }

    /// Get the degree of the homology group
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Get the dimension (Betti number)
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get the generators
    pub fn generators(&self) -> &[DVector<f64>] {
        &self.generators
    }

    /// Check if the homology group is trivial
    pub fn is_trivial(&self) -> bool {
        self.dimension == 0
    }

    /// Set the generators
    pub fn set_generators(&mut self, generators: Vec<DVector<f64>>) {
        self.dimension = generators.len();
        self.generators = generators;
    }

    /// Add a generator
    pub fn add_generator(&mut self, generator: DVector<f64>) {
        self.generators.push(generator);
        self.dimension += 1;
    }

    /// Check if a cycle is a boundary (homologous to zero)
    pub fn is_boundary(&self, cycle: &DVector<f64>, epsilon: f64) -> bool {
        // A cycle is a boundary if it's in the span of boundaries
        // For now, check if it's close to zero
        cycle.norm() < epsilon
    }

    /// Compute the homology class of a cycle
    pub fn classify(&self, cycle: &DVector<f64>) -> HomologyClass {
        if self.generators.is_empty() {
            return HomologyClass::Zero;
        }

        // Project onto generator space
        let mut coefficients = Vec::new();
        for gen in &self.generators {
            let coeff = cycle.dot(gen) / gen.dot(gen);
            coefficients.push(coeff);
        }

        HomologyClass::NonTrivial(coefficients)
    }
}

/// A homology class [α] in H_n
#[derive(Debug, Clone)]
pub enum HomologyClass {
    /// The zero class
    Zero,
    /// Non-trivial class with coefficients in terms of generators
    NonTrivial(Vec<f64>),
}

impl HomologyClass {
    /// Check if this is the zero class
    pub fn is_zero(&self) -> bool {
        matches!(self, HomologyClass::Zero)
    }

    /// Get the coefficients if non-trivial
    pub fn coefficients(&self) -> Option<&[f64]> {
        match self {
            HomologyClass::Zero => None,
            HomologyClass::NonTrivial(c) => Some(c),
        }
    }
}

/// Relative homology H_n(X, A)
#[derive(Debug, Clone)]
pub struct RelativeHomology {
    /// Degree
    degree: usize,
    /// Space X
    space: Homology,
    /// Subspace A
    subspace: Homology,
    /// Relative homology dimension
    dimension: usize,
}

impl RelativeHomology {
    /// Create new relative homology
    pub fn new(degree: usize, space: Homology, subspace: Homology) -> Self {
        // Long exact sequence: ... -> H_n(A) -> H_n(X) -> H_n(X,A) -> H_{n-1}(A) -> ...
        let dimension = space.dimension().saturating_sub(subspace.dimension());
        Self {
            degree,
            space,
            subspace,
            dimension,
        }
    }

    /// Get the dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_homology_creation() {
        let h1 = Homology::new(1, 2);
        assert_eq!(h1.degree(), 1);
        assert_eq!(h1.dimension(), 2);
        assert!(!h1.is_trivial());
    }

    #[test]
    fn test_trivial_homology() {
        let h0 = Homology::new(0, 0);
        assert!(h0.is_trivial());
    }

    #[test]
    fn test_homology_class() {
        let class = HomologyClass::NonTrivial(vec![1.0, 2.0]);
        assert!(!class.is_zero());
        assert_eq!(class.coefficients().unwrap(), &[1.0, 2.0]);
    }
}
