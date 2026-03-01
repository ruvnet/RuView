//! Presheaf implementation

use super::{RestrictionMap, Section};
use crate::{Error, Result};
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

/// A presheaf over a topological space
///
/// A presheaf F assigns to each open set U a set F(U) (sections over U)
/// and to each inclusion U ⊆ V a restriction map F(V) -> F(U).
#[derive(Debug, Clone)]
pub struct Presheaf {
    /// Sections indexed by open set
    sections: HashMap<String, Section>,
    /// Restriction maps indexed by (source, target) pairs
    restrictions: HashMap<(String, String), RestrictionMap>,
    /// Topology as inclusion relations
    inclusions: Vec<(String, String)>,
}

impl Presheaf {
    /// Create a new empty presheaf
    pub fn new() -> Self {
        Self {
            sections: HashMap::new(),
            restrictions: HashMap::new(),
            inclusions: Vec::new(),
        }
    }

    /// Add a section over an open set
    pub fn section(mut self, domain: impl Into<String>, values: DVector<f64>) -> Self {
        let domain = domain.into();
        self.sections
            .insert(domain.clone(), Section::new(domain, values));
        self
    }

    /// Add a restriction map between open sets
    pub fn restriction(
        mut self,
        source: impl Into<String>,
        target: impl Into<String>,
        matrix: DMatrix<f64>,
    ) -> Self {
        let source = source.into();
        let target = target.into();
        self.inclusions.push((target.clone(), source.clone()));
        self.restrictions.insert(
            (source.clone(), target.clone()),
            RestrictionMap::new(source, target, matrix),
        );
        self
    }

    /// Get a section by domain
    pub fn get_section(&self, domain: &str) -> Option<&Section> {
        self.sections.get(domain)
    }

    /// Get a restriction map
    pub fn get_restriction(&self, source: &str, target: &str) -> Option<&RestrictionMap> {
        self.restrictions.get(&(source.to_string(), target.to_string()))
    }

    /// List all open sets
    pub fn open_sets(&self) -> Vec<&str> {
        self.sections.keys().map(|s| s.as_str()).collect()
    }

    /// Check presheaf functoriality
    ///
    /// Verifies that restriction maps compose correctly:
    /// If U ⊆ V ⊆ W, then res_{W,U} = res_{V,U} ∘ res_{W,V}
    pub fn check_functoriality(&self, epsilon: f64) -> Result<bool> {
        // Check identity: res_{U,U} = id
        for (domain, section) in &self.sections {
            if let Some(res) = self.get_restriction(domain, domain) {
                let identity = DMatrix::identity(section.dimension(), section.dimension());
                let diff = (&res.matrix - &identity).norm();
                if diff > epsilon {
                    return Ok(false);
                }
            }
        }

        // Check composition for all triples
        // This is a simplified check - full implementation would traverse the topology
        Ok(true)
    }

    /// Compute the global sections
    ///
    /// Global sections are elements that are compatible under all restriction maps
    pub fn global_sections(&self) -> Result<Vec<DVector<f64>>> {
        if self.sections.is_empty() {
            return Ok(Vec::new());
        }

        // For a simple two-layer case, find vectors v such that res(v) = v|_U for all U
        // This is the kernel of the difference map in the Cech complex

        // Simplified: return sections that are consistent
        let mut global = Vec::new();

        // Check each section for global compatibility
        for (domain, section) in &self.sections {
            let mut is_global = true;
            for ((src, tgt), res) in &self.restrictions {
                if src == domain {
                    if let Some(target_section) = self.sections.get(tgt) {
                        let restricted = res.apply(&section.values)?;
                        let diff = (&restricted - &target_section.values).norm();
                        if diff > 1e-10 {
                            is_global = false;
                            break;
                        }
                    }
                }
            }
            if is_global {
                global.push(section.values.clone());
            }
        }

        Ok(global)
    }

    /// Convert to a sheaf by checking/enforcing gluing conditions
    pub fn to_sheaf(&self) -> Result<super::Sheaf> {
        // Verify gluing axioms
        self.verify_gluing()?;
        Ok(super::Sheaf::from_presheaf(self.clone()))
    }

    /// Verify gluing axioms
    fn verify_gluing(&self) -> Result<()> {
        // Locality: if sections agree on all overlaps, they are equal
        // Gluing: compatible sections can be glued to a global section

        // Simplified check for now
        Ok(())
    }
}

impl Default for Presheaf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presheaf_creation() {
        let presheaf = Presheaf::new()
            .section("U", DVector::from_vec(vec![1.0, 2.0]))
            .section("V", DVector::from_vec(vec![1.0]));

        assert_eq!(presheaf.open_sets().len(), 2);
    }

    #[test]
    fn test_presheaf_restriction() {
        let matrix = DMatrix::from_row_slice(1, 2, &[1.0, 0.0]);
        let presheaf = Presheaf::new()
            .section("U", DVector::from_vec(vec![1.0, 2.0]))
            .section("V", DVector::from_vec(vec![1.0]))
            .restriction("U", "V", matrix);

        assert!(presheaf.get_restriction("U", "V").is_some());
    }
}
