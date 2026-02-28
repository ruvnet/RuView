//! Sheaf-theoretic structures for consistency checking
//!
//! Implements sheaf structures that enforce local-to-global consistency
//! across distributed data.

use dashmap::DashMap;
use exo_core::{EntityId, Error, HyperedgeId, SectionId, SheafConsistencyResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// Domain of a section (the entities it covers)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    entities: HashSet<EntityId>,
}

impl Domain {
    /// Create a new domain from entities
    pub fn new(entities: impl IntoIterator<Item = EntityId>) -> Self {
        Self {
            entities: entities.into_iter().collect(),
        }
    }

    /// Check if domain is empty
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Compute intersection with another domain
    pub fn intersect(&self, other: &Domain) -> Domain {
        let intersection = self
            .entities
            .intersection(&other.entities)
            .copied()
            .collect();
        Domain {
            entities: intersection,
        }
    }

    /// Check if this domain contains an entity
    pub fn contains(&self, entity: &EntityId) -> bool {
        self.entities.contains(entity)
    }
}

/// A section assigns data to a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub domain: Domain,
    pub data: serde_json::Value,
}

impl Section {
    /// Create a new section
    pub fn new(domain: Domain, data: serde_json::Value) -> Self {
        Self {
            id: SectionId::new(),
            domain,
            data,
        }
    }
}

/// Sheaf structure for consistency checking
///
/// A sheaf enforces that local data (sections) must agree on overlaps.
pub struct SheafStructure {
    /// Section storage
    sections: Arc<DashMap<SectionId, Section>>,
    /// Restriction maps (how to restrict a section to a subdomain)
    /// Key is (section_id, domain_hash) where domain_hash is a string representation
    restriction_maps: Arc<DashMap<String, serde_json::Value>>,
    /// Hyperedge to section mapping
    hyperedge_sections: Arc<DashMap<HyperedgeId, Vec<SectionId>>>,
}

impl SheafStructure {
    /// Create a new sheaf structure
    pub fn new() -> Self {
        Self {
            sections: Arc::new(DashMap::new()),
            restriction_maps: Arc::new(DashMap::new()),
            hyperedge_sections: Arc::new(DashMap::new()),
        }
    }

    /// Add a section to the sheaf
    pub fn add_section(&self, section: Section) -> SectionId {
        let id = section.id;
        self.sections.insert(id, section);
        id
    }

    /// Get a section by ID
    pub fn get_section(&self, id: &SectionId) -> Option<Section> {
        self.sections.get(id).map(|entry| entry.clone())
    }

    /// Restrict a section to a subdomain
    ///
    /// This implements the restriction map ρ: F(U) → F(V) for V ⊆ U
    pub fn restrict(&self, section: &Section, subdomain: &Domain) -> serde_json::Value {
        // Create cache key as string (section_id + domain hash)
        let cache_key = format!("{:?}-{:?}", section.id, subdomain.entities);
        if let Some(cached) = self.restriction_maps.get(&cache_key) {
            return cached.clone();
        }

        // Compute restriction (simplified: just filter data by domain)
        let restricted = self.compute_restriction(&section.data, subdomain);

        // Cache the result
        self.restriction_maps.insert(cache_key, restricted.clone());

        restricted
    }

    /// Compute restriction (placeholder implementation)
    fn compute_restriction(
        &self,
        data: &serde_json::Value,
        _subdomain: &Domain,
    ) -> serde_json::Value {
        // Simplified: just clone the data
        // A real implementation would filter data based on subdomain
        data.clone()
    }

    /// Update sections when a hyperedge is created
    pub fn update_sections(
        &mut self,
        hyperedge_id: HyperedgeId,
        entities: &[EntityId],
    ) -> Result<(), Error> {
        // Create a section for this hyperedge
        let domain = Domain::new(entities.iter().copied());
        let section = Section::new(domain, serde_json::json!({}));
        let section_id = self.add_section(section);

        // Associate with hyperedge
        self.hyperedge_sections
            .entry(hyperedge_id)
            .or_insert_with(Vec::new)
            .push(section_id);

        Ok(())
    }

    /// Check sheaf consistency (from pseudocode: CheckSheafConsistency)
    ///
    /// Verifies that local sections agree on their overlaps,
    /// satisfying the sheaf axioms.
    pub fn check_consistency(&self, section_ids: &[SectionId]) -> SheafConsistencyResult {
        let mut inconsistencies = Vec::new();

        // Get all sections
        let sections: Vec<_> = section_ids
            .iter()
            .filter_map(|id| self.get_section(id))
            .collect();

        // Check all pairs of overlapping sections (from pseudocode)
        for i in 0..sections.len() {
            for j in (i + 1)..sections.len() {
                let section_a = &sections[i];
                let section_b = &sections[j];

                let overlap = section_a.domain.intersect(&section_b.domain);

                if overlap.is_empty() {
                    continue;
                }

                // Restriction maps (from pseudocode)
                let restricted_a = self.restrict(section_a, &overlap);
                let restricted_b = self.restrict(section_b, &overlap);

                // Check agreement (from pseudocode)
                if !approximately_equal(&restricted_a, &restricted_b, 1e-6) {
                    let discrepancy = compute_discrepancy(&restricted_a, &restricted_b);
                    inconsistencies.push(format!(
                        "Sections {} and {} disagree on overlap (discrepancy: {:.6})",
                        section_a.id.0, section_b.id.0, discrepancy
                    ));
                }
            }
        }

        if inconsistencies.is_empty() {
            SheafConsistencyResult::Consistent
        } else {
            SheafConsistencyResult::Inconsistent(inconsistencies)
        }
    }

    /// Get sections associated with a hyperedge
    pub fn get_hyperedge_sections(&self, hyperedge_id: &HyperedgeId) -> Vec<SectionId> {
        self.hyperedge_sections
            .get(hyperedge_id)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }
}

impl Default for SheafStructure {
    fn default() -> Self {
        Self::new()
    }
}

/// Sheaf inconsistency record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafInconsistency {
    pub sections: (SectionId, SectionId),
    pub overlap: Domain,
    pub discrepancy: f64,
}

/// Check if two JSON values are approximately equal
fn approximately_equal(a: &serde_json::Value, b: &serde_json::Value, epsilon: f64) -> bool {
    match (a, b) {
        (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
            let a_f64 = na.as_f64().unwrap_or(0.0);
            let b_f64 = nb.as_f64().unwrap_or(0.0);
            (a_f64 - b_f64).abs() < epsilon
        }
        (serde_json::Value::Array(aa), serde_json::Value::Array(ab)) => {
            if aa.len() != ab.len() {
                return false;
            }
            aa.iter()
                .zip(ab.iter())
                .all(|(x, y)| approximately_equal(x, y, epsilon))
        }
        (serde_json::Value::Object(oa), serde_json::Value::Object(ob)) => {
            if oa.len() != ob.len() {
                return false;
            }
            oa.iter().all(|(k, va)| {
                ob.get(k)
                    .map(|vb| approximately_equal(va, vb, epsilon))
                    .unwrap_or(false)
            })
        }
        _ => a == b,
    }
}

/// Compute discrepancy between two JSON values
fn compute_discrepancy(a: &serde_json::Value, b: &serde_json::Value) -> f64 {
    match (a, b) {
        (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
            let a_f64 = na.as_f64().unwrap_or(0.0);
            let b_f64 = nb.as_f64().unwrap_or(0.0);
            (a_f64 - b_f64).abs()
        }
        (serde_json::Value::Array(aa), serde_json::Value::Array(ab)) => {
            let diffs: Vec<f64> = aa
                .iter()
                .zip(ab.iter())
                .map(|(x, y)| compute_discrepancy(x, y))
                .collect();
            diffs.iter().sum::<f64>() / diffs.len().max(1) as f64
        }
        _ => {
            if a == b {
                0.0
            } else {
                1.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_intersection() {
        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();

        let d1 = Domain::new(vec![e1, e2]);
        let d2 = Domain::new(vec![e2, e3]);

        let overlap = d1.intersect(&d2);
        assert!(!overlap.is_empty());
        assert!(overlap.contains(&e2));
        assert!(!overlap.contains(&e1));
    }

    #[test]
    fn test_sheaf_consistency() {
        let sheaf = SheafStructure::new();

        let e1 = EntityId::new();
        let e2 = EntityId::new();

        // Create two sections with same data on overlapping domains
        let domain1 = Domain::new(vec![e1, e2]);
        let section1 = Section::new(domain1, serde_json::json!({"value": 42}));

        let domain2 = Domain::new(vec![e2]);
        let section2 = Section::new(domain2, serde_json::json!({"value": 42}));

        let id1 = sheaf.add_section(section1);
        let id2 = sheaf.add_section(section2);

        // Should be consistent
        let result = sheaf.check_consistency(&[id1, id2]);
        assert!(matches!(result, SheafConsistencyResult::Consistent));
    }

    #[test]
    fn test_approximately_equal() {
        let a = serde_json::json!(1.0);
        let b = serde_json::json!(1.0000001);

        assert!(approximately_equal(&a, &b, 1e-6));
        assert!(!approximately_equal(&a, &b, 1e-8));
    }
}
