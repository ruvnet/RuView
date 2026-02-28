//! CoherenceEnergy Value Object
//!
//! Represents the coherence energy computed from sheaf Laplacian residuals.
//! The energy formula is: E(S) = sum(w_e * |r_e|^2) where r_e = rho_u(x_u) - rho_v(x_v)
//!
//! This module provides immutable value objects for:
//! - Total system energy (lower = more coherent)
//! - Per-edge energies for localization
//! - Per-scope energies for hierarchical analysis
//! - Hotspot identification (highest energy edges)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an edge in the sheaf graph
pub type EdgeId = String;

/// Unique identifier for a scope/namespace
pub type ScopeId = String;

/// Energy associated with a single edge
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeEnergy {
    /// Edge identifier
    pub edge_id: EdgeId,
    /// Source node identifier
    pub source: String,
    /// Target node identifier
    pub target: String,
    /// Weighted residual energy: w_e * |r_e|^2
    pub energy: f32,
    /// Raw residual vector (for debugging/analysis)
    pub residual: Vec<f32>,
    /// Residual norm squared: |r_e|^2
    pub residual_norm_sq: f32,
    /// Edge weight
    pub weight: f32,
}

impl EdgeEnergy {
    /// Create a new edge energy
    #[inline]
    pub fn new(
        edge_id: impl Into<EdgeId>,
        source: impl Into<String>,
        target: impl Into<String>,
        residual: Vec<f32>,
        weight: f32,
    ) -> Self {
        let residual_norm_sq = compute_norm_sq(&residual);
        let energy = weight * residual_norm_sq;

        Self {
            edge_id: edge_id.into(),
            source: source.into(),
            target: target.into(),
            energy,
            residual,
            residual_norm_sq,
            weight,
        }
    }

    /// Create edge energy without storing residual (lightweight version)
    /// Use this when the residual vector is not needed for debugging/analysis
    #[inline]
    pub fn new_lightweight(
        edge_id: impl Into<EdgeId>,
        source: impl Into<String>,
        target: impl Into<String>,
        residual_norm_sq: f32,
        weight: f32,
    ) -> Self {
        Self {
            edge_id: edge_id.into(),
            source: source.into(),
            target: target.into(),
            energy: weight * residual_norm_sq,
            residual: Vec::new(),
            residual_norm_sq,
            weight,
        }
    }

    /// Check if this edge has significant energy (above threshold)
    #[inline]
    pub fn is_significant(&self, threshold: f32) -> bool {
        self.energy > threshold
    }

    /// Get the contribution ratio to total energy
    #[inline]
    pub fn contribution_ratio(&self, total_energy: f32) -> f32 {
        if total_energy > 0.0 {
            self.energy / total_energy
        } else {
            0.0
        }
    }
}

/// Energy aggregated by scope/namespace
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeEnergy {
    /// Scope identifier
    pub scope_id: ScopeId,
    /// Total energy within this scope
    pub energy: f32,
    /// Number of edges in this scope
    pub edge_count: usize,
    /// Average energy per edge
    pub average_energy: f32,
    /// Maximum single edge energy
    pub max_edge_energy: f32,
    /// Edge ID with maximum energy (hotspot)
    pub hotspot_edge: Option<EdgeId>,
}

impl ScopeEnergy {
    /// Create a new scope energy from edge energies
    pub fn from_edges(scope_id: impl Into<ScopeId>, edge_energies: &[&EdgeEnergy]) -> Self {
        let scope_id = scope_id.into();
        let edge_count = edge_energies.len();
        let energy: f32 = edge_energies.iter().map(|e| e.energy).sum();
        let average_energy = if edge_count > 0 {
            energy / edge_count as f32
        } else {
            0.0
        };

        let (max_edge_energy, hotspot_edge) = edge_energies
            .iter()
            .max_by(|a, b| {
                a.energy
                    .partial_cmp(&b.energy)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|e| (e.energy, Some(e.edge_id.clone())))
            .unwrap_or((0.0, None));

        Self {
            scope_id,
            energy,
            edge_count,
            average_energy,
            max_edge_energy,
            hotspot_edge,
        }
    }

    /// Check if this scope has coherence issues
    #[inline]
    pub fn is_incoherent(&self, threshold: f32) -> bool {
        self.energy > threshold
    }
}

/// Information about a coherence hotspot (high-energy region)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotspotInfo {
    /// Edge identifier
    pub edge_id: EdgeId,
    /// Energy value
    pub energy: f32,
    /// Source node
    pub source: String,
    /// Target node
    pub target: String,
    /// Rank (1 = highest energy)
    pub rank: usize,
    /// Percentage of total energy
    pub percentage: f32,
}

/// Snapshot of coherence energy at a specific timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergySnapshot {
    /// Total system energy
    pub total_energy: f32,
    /// Timestamp of computation
    pub timestamp: DateTime<Utc>,
    /// Content fingerprint for staleness detection
    pub fingerprint: String,
}

impl EnergySnapshot {
    /// Create a new energy snapshot
    pub fn new(total_energy: f32, fingerprint: impl Into<String>) -> Self {
        Self {
            total_energy,
            timestamp: Utc::now(),
            fingerprint: fingerprint.into(),
        }
    }

    /// Check if this snapshot is stale compared to a fingerprint
    #[inline]
    pub fn is_stale(&self, current_fingerprint: &str) -> bool {
        self.fingerprint != current_fingerprint
    }

    /// Get the age of this snapshot in milliseconds
    pub fn age_ms(&self) -> i64 {
        let now = Utc::now();
        (now - self.timestamp).num_milliseconds()
    }
}

/// Global coherence energy: E(S) = sum(w_e * |r_e|^2)
///
/// This is the main value object representing the coherence state of the entire system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceEnergy {
    /// Total system energy (lower = more coherent)
    pub total_energy: f32,
    /// Per-edge energies for localization
    pub edge_energies: HashMap<EdgeId, EdgeEnergy>,
    /// Energy by scope/namespace
    pub scope_energies: HashMap<ScopeId, ScopeEnergy>,
    /// Computation timestamp
    pub computed_at: DateTime<Utc>,
    /// Fingerprint for change detection (Blake3 hash)
    pub fingerprint: String,
    /// Number of edges computed
    pub edge_count: usize,
    /// Number of nodes in the graph
    pub node_count: usize,
}

impl CoherenceEnergy {
    /// Create a new coherence energy result
    pub fn new(
        edge_energies: HashMap<EdgeId, EdgeEnergy>,
        scope_mapping: &HashMap<EdgeId, ScopeId>,
        node_count: usize,
        fingerprint: impl Into<String>,
    ) -> Self {
        let total_energy: f32 = edge_energies.values().map(|e| e.energy).sum();
        let edge_count = edge_energies.len();

        // Aggregate by scope
        let scope_energies = Self::aggregate_by_scope(&edge_energies, scope_mapping);

        Self {
            total_energy,
            edge_energies,
            scope_energies,
            computed_at: Utc::now(),
            fingerprint: fingerprint.into(),
            edge_count,
            node_count,
        }
    }

    /// Create an empty coherence energy
    pub fn empty() -> Self {
        Self {
            total_energy: 0.0,
            edge_energies: HashMap::new(),
            scope_energies: HashMap::new(),
            computed_at: Utc::now(),
            fingerprint: String::new(),
            edge_count: 0,
            node_count: 0,
        }
    }

    /// Check if the system is coherent (energy below threshold)
    #[inline]
    pub fn is_coherent(&self, threshold: f32) -> bool {
        self.total_energy < threshold
    }

    /// Get the average energy per edge
    #[inline]
    pub fn average_edge_energy(&self) -> f32 {
        if self.edge_count > 0 {
            self.total_energy / self.edge_count as f32
        } else {
            0.0
        }
    }

    /// Get energy for a specific scope
    pub fn scope_energy_for(&self, scope_id: &str) -> f32 {
        self.scope_energies
            .get(scope_id)
            .map(|s| s.energy)
            .unwrap_or(0.0)
    }

    /// Identify the top-k hotspots (highest energy edges)
    pub fn hotspots(&self, k: usize) -> Vec<HotspotInfo> {
        let mut sorted: Vec<_> = self.edge_energies.values().collect();
        sorted.sort_by(|a, b| {
            b.energy
                .partial_cmp(&a.energy)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted
            .into_iter()
            .take(k)
            .enumerate()
            .map(|(i, e)| HotspotInfo {
                edge_id: e.edge_id.clone(),
                energy: e.energy,
                source: e.source.clone(),
                target: e.target.clone(),
                rank: i + 1,
                percentage: if self.total_energy > 0.0 {
                    (e.energy / self.total_energy) * 100.0
                } else {
                    0.0
                },
            })
            .collect()
    }

    /// Get all edges with energy above threshold
    pub fn high_energy_edges(&self, threshold: f32) -> Vec<&EdgeEnergy> {
        self.edge_energies
            .values()
            .filter(|e| e.energy > threshold)
            .collect()
    }

    /// Create a snapshot of the current energy state
    pub fn snapshot(&self) -> EnergySnapshot {
        EnergySnapshot {
            total_energy: self.total_energy,
            timestamp: self.computed_at,
            fingerprint: self.fingerprint.clone(),
        }
    }

    /// Get the energy distribution statistics
    pub fn statistics(&self) -> EnergyStatistics {
        if self.edge_energies.is_empty() {
            return EnergyStatistics::default();
        }

        let energies: Vec<f32> = self.edge_energies.values().map(|e| e.energy).collect();
        let min = energies
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        let max = energies
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        let mean = self.average_edge_energy();

        // Compute standard deviation
        let variance: f32 =
            energies.iter().map(|e| (e - mean).powi(2)).sum::<f32>() / energies.len() as f32;
        let std_dev = variance.sqrt();

        // Compute median
        let mut sorted_energies = energies.clone();
        sorted_energies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if sorted_energies.len() % 2 == 0 {
            let mid = sorted_energies.len() / 2;
            (sorted_energies[mid - 1] + sorted_energies[mid]) / 2.0
        } else {
            sorted_energies[sorted_energies.len() / 2]
        };

        EnergyStatistics {
            min,
            max,
            mean,
            median,
            std_dev,
            count: self.edge_count,
        }
    }

    /// Aggregate edge energies by scope
    fn aggregate_by_scope(
        edge_energies: &HashMap<EdgeId, EdgeEnergy>,
        scope_mapping: &HashMap<EdgeId, ScopeId>,
    ) -> HashMap<ScopeId, ScopeEnergy> {
        // Group edges by scope
        let mut scope_groups: HashMap<ScopeId, Vec<&EdgeEnergy>> = HashMap::new();

        for (edge_id, edge_energy) in edge_energies {
            let scope_id = scope_mapping
                .get(edge_id)
                .cloned()
                .unwrap_or_else(|| "default".to_string());

            scope_groups.entry(scope_id).or_default().push(edge_energy);
        }

        // Create scope energies
        scope_groups
            .into_iter()
            .map(|(scope_id, edges)| {
                let scope_energy = ScopeEnergy::from_edges(&scope_id, &edges);
                (scope_id, scope_energy)
            })
            .collect()
    }
}

/// Statistical summary of energy distribution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnergyStatistics {
    /// Minimum edge energy
    pub min: f32,
    /// Maximum edge energy
    pub max: f32,
    /// Mean edge energy
    pub mean: f32,
    /// Median edge energy
    pub median: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// Number of edges
    pub count: usize,
}

/// Compute the squared L2 norm of a vector
///
/// Uses SIMD optimization when available via the `simd` feature.
/// For small vectors (<= 8), uses unrolled scalar loop for better performance.
#[inline]
pub fn compute_norm_sq(v: &[f32]) -> f32 {
    let len = v.len();

    // Fast path for small vectors - avoid SIMD overhead
    if len <= 8 {
        let mut sum = 0.0f32;
        for &x in v {
            sum += x * x;
        }
        return sum;
    }

    #[cfg(feature = "simd")]
    {
        compute_norm_sq_simd(v)
    }
    #[cfg(not(feature = "simd"))]
    {
        compute_norm_sq_unrolled(v)
    }
}

/// Unrolled scalar computation for non-SIMD builds
#[cfg(not(feature = "simd"))]
#[inline]
fn compute_norm_sq_unrolled(v: &[f32]) -> f32 {
    let chunks = v.chunks_exact(4);
    let remainder = chunks.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for chunk in chunks {
        acc0 += chunk[0] * chunk[0];
        acc1 += chunk[1] * chunk[1];
        acc2 += chunk[2] * chunk[2];
        acc3 += chunk[3] * chunk[3];
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for &x in remainder {
        sum += x * x;
    }
    sum
}

/// SIMD-optimized squared norm computation
#[cfg(feature = "simd")]
fn compute_norm_sq_simd(v: &[f32]) -> f32 {
    use wide::f32x8;

    let chunks = v.chunks_exact(8);
    let remainder = chunks.remainder();

    let mut sum = f32x8::ZERO;

    for chunk in chunks {
        let vals = f32x8::from(<[f32; 8]>::try_from(chunk).unwrap());
        sum += vals * vals;
    }

    let mut total: f32 = sum.reduce_add();

    // Handle remainder
    for &val in remainder {
        total += val * val;
    }

    total
}

/// Compute the residual between two projected states
///
/// r_e = rho_u(x_u) - rho_v(x_v)
#[inline]
pub fn compute_residual(projected_source: &[f32], projected_target: &[f32]) -> Vec<f32> {
    debug_assert_eq!(
        projected_source.len(),
        projected_target.len(),
        "Projected vectors must have same dimension"
    );

    let len = projected_source.len();
    let mut result = Vec::with_capacity(len);

    #[cfg(feature = "simd")]
    {
        result = compute_residual_simd(projected_source, projected_target);
    }
    #[cfg(not(feature = "simd"))]
    {
        // Unrolled loop for better vectorization
        let chunks_a = projected_source.chunks_exact(4);
        let chunks_b = projected_target.chunks_exact(4);
        let rem_a = chunks_a.remainder();
        let rem_b = chunks_b.remainder();

        for (ca, cb) in chunks_a.zip(chunks_b) {
            result.push(ca[0] - cb[0]);
            result.push(ca[1] - cb[1]);
            result.push(ca[2] - cb[2]);
            result.push(ca[3] - cb[3]);
        }
        for (&a, &b) in rem_a.iter().zip(rem_b.iter()) {
            result.push(a - b);
        }
    }
    result
}

/// Compute residual into pre-allocated buffer (zero allocation)
#[inline]
pub fn compute_residual_into(
    projected_source: &[f32],
    projected_target: &[f32],
    result: &mut [f32],
) {
    debug_assert_eq!(
        projected_source.len(),
        projected_target.len(),
        "Projected vectors must have same dimension"
    );
    debug_assert_eq!(
        result.len(),
        projected_source.len(),
        "Result buffer size mismatch"
    );

    // Unrolled loop for better vectorization
    let len = projected_source.len();
    let chunks = len / 4;

    for i in 0..chunks {
        let base = i * 4;
        result[base] = projected_source[base] - projected_target[base];
        result[base + 1] = projected_source[base + 1] - projected_target[base + 1];
        result[base + 2] = projected_source[base + 2] - projected_target[base + 2];
        result[base + 3] = projected_source[base + 3] - projected_target[base + 3];
    }
    for i in (chunks * 4)..len {
        result[i] = projected_source[i] - projected_target[i];
    }
}

/// Compute residual norm squared directly without allocating residual vector
/// This is the most efficient path when the residual vector itself is not needed
#[inline]
pub fn compute_residual_norm_sq(projected_source: &[f32], projected_target: &[f32]) -> f32 {
    debug_assert_eq!(
        projected_source.len(),
        projected_target.len(),
        "Projected vectors must have same dimension"
    );

    let len = projected_source.len();

    // Fast path for small vectors
    if len <= 8 {
        let mut sum = 0.0f32;
        for (&a, &b) in projected_source.iter().zip(projected_target.iter()) {
            let d = a - b;
            sum += d * d;
        }
        return sum;
    }

    // Unrolled loop with 4 accumulators for ILP
    let chunks = len / 4;
    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for i in 0..chunks {
        let base = i * 4;
        let d0 = projected_source[base] - projected_target[base];
        let d1 = projected_source[base + 1] - projected_target[base + 1];
        let d2 = projected_source[base + 2] - projected_target[base + 2];
        let d3 = projected_source[base + 3] - projected_target[base + 3];

        acc0 += d0 * d0;
        acc1 += d1 * d1;
        acc2 += d2 * d2;
        acc3 += d3 * d3;
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;

    // Handle remainder
    for i in (chunks * 4)..len {
        let d = projected_source[i] - projected_target[i];
        sum += d * d;
    }

    sum
}

/// SIMD-optimized residual computation
#[cfg(feature = "simd")]
fn compute_residual_simd(a: &[f32], b: &[f32]) -> Vec<f32> {
    use wide::f32x8;

    let mut result = vec![0.0f32; a.len()];

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let chunks_r = result.chunks_exact_mut(8);

    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    for ((chunk_a, chunk_b), chunk_r) in chunks_a.zip(chunks_b).zip(chunks_r) {
        let va = f32x8::from(<[f32; 8]>::try_from(chunk_a).unwrap());
        let vb = f32x8::from(<[f32; 8]>::try_from(chunk_b).unwrap());
        let diff = va - vb;
        let arr: [f32; 8] = diff.into();
        chunk_r.copy_from_slice(&arr);
    }

    // Handle remainder
    let offset = a.len() - remainder_a.len();
    for (i, (&va, &vb)) in remainder_a.iter().zip(remainder_b.iter()).enumerate() {
        result[offset + i] = va - vb;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_energy_creation() {
        let residual = vec![1.0, 0.0, 0.0];
        let edge = EdgeEnergy::new("e1", "n1", "n2", residual, 2.0);

        assert_eq!(edge.edge_id, "e1");
        assert_eq!(edge.residual_norm_sq, 1.0);
        assert_eq!(edge.energy, 2.0); // weight * norm_sq
    }

    #[test]
    fn test_coherence_energy_hotspots() {
        let mut edge_energies = HashMap::new();

        edge_energies.insert(
            "e1".to_string(),
            EdgeEnergy::new("e1", "n1", "n2", vec![1.0], 1.0),
        );
        edge_energies.insert(
            "e2".to_string(),
            EdgeEnergy::new("e2", "n2", "n3", vec![2.0], 1.0),
        );
        edge_energies.insert(
            "e3".to_string(),
            EdgeEnergy::new("e3", "n3", "n4", vec![3.0], 1.0),
        );

        let scope_mapping = HashMap::new();
        let energy = CoherenceEnergy::new(edge_energies, &scope_mapping, 4, "fp1");

        let hotspots = energy.hotspots(2);
        assert_eq!(hotspots.len(), 2);
        assert_eq!(hotspots[0].edge_id, "e3"); // highest energy
        assert_eq!(hotspots[1].edge_id, "e2");
    }

    #[test]
    fn test_compute_norm_sq() {
        let v = vec![3.0, 4.0];
        assert_eq!(compute_norm_sq(&v), 25.0);

        let v = vec![1.0, 2.0, 2.0];
        assert_eq!(compute_norm_sq(&v), 9.0);
    }

    #[test]
    fn test_compute_residual() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![0.5, 1.0, 2.0];
        let r = compute_residual(&a, &b);

        assert_eq!(r.len(), 3);
        assert!((r[0] - 0.5).abs() < 1e-6);
        assert!((r[1] - 1.0).abs() < 1e-6);
        assert!((r[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_energy_statistics() {
        let mut edge_energies = HashMap::new();
        edge_energies.insert(
            "e1".to_string(),
            EdgeEnergy::new("e1", "n1", "n2", vec![1.0], 1.0),
        );
        edge_energies.insert(
            "e2".to_string(),
            EdgeEnergy::new("e2", "n2", "n3", vec![2.0], 1.0),
        );
        edge_energies.insert(
            "e3".to_string(),
            EdgeEnergy::new("e3", "n3", "n4", vec![3.0], 1.0),
        );

        let scope_mapping = HashMap::new();
        let energy = CoherenceEnergy::new(edge_energies, &scope_mapping, 4, "fp1");
        let stats = energy.statistics();

        assert_eq!(stats.count, 3);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 9.0);
    }

    #[test]
    fn test_scope_energy_aggregation() {
        let e1 = EdgeEnergy::new("e1", "n1", "n2", vec![1.0], 1.0);
        let e2 = EdgeEnergy::new("e2", "n2", "n3", vec![2.0], 1.0);

        let scope = ScopeEnergy::from_edges("scope1", &[&e1, &e2]);

        assert_eq!(scope.edge_count, 2);
        assert_eq!(scope.energy, 5.0); // 1 + 4
        assert_eq!(scope.hotspot_edge, Some("e2".to_string()));
    }
}
