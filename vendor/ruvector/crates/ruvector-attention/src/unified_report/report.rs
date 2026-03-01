//! Unified Geometry Report Builder

use super::metrics::{MetricType, MetricValue};
use crate::info_bottleneck::KLDivergence;
use crate::pde_attention::GraphLaplacian;
use crate::topology::WindowCoherence;
use serde::{Deserialize, Serialize};

/// Report configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Number of OT projections
    pub ot_projections: usize,
    /// k for k-NN coherence
    pub knn_k: usize,
    /// Sigma for diffusion
    pub diffusion_sigma: f32,
    /// Whether to compute H0 persistence (expensive)
    pub compute_persistence: bool,
    /// Random seed
    pub seed: u64,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            ot_projections: 8,
            knn_k: 8,
            diffusion_sigma: 1.0,
            compute_persistence: false,
            seed: 42,
        }
    }
}

/// Unified geometry report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryReport {
    /// OT sliced Wasserstein mean distance
    pub ot_mean_distance: f32,
    /// Topology coherence score
    pub topology_coherence: f32,
    /// H0 persistence death sum (if computed)
    pub h0_death_sum: Option<f32>,
    /// Information bottleneck KL
    pub ib_kl: f32,
    /// Diffusion energy
    pub diffusion_energy: f32,
    /// Attention entropy
    pub attention_entropy: f32,
    /// All metrics with thresholds
    pub metrics: Vec<MetricValue>,
    /// Overall health score (0-1)
    pub health_score: f32,
    /// Recommended action
    pub recommendation: AttentionRecommendation,
}

/// Recommended action based on report
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttentionRecommendation {
    /// Full attention, normal operation
    Stable,
    /// Reduce attention width
    Cautious,
    /// Retrieval only, no updates
    Freeze,
    /// Increase temperature
    IncreaseTemperature,
    /// Decrease temperature
    DecreaseTemperature,
    /// Add regularization
    AddRegularization,
}

/// Report builder
pub struct ReportBuilder {
    config: ReportConfig,
}

impl ReportBuilder {
    /// Create new report builder
    pub fn new(config: ReportConfig) -> Self {
        Self { config }
    }

    /// Build report from query and keys
    pub fn build(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        attention_weights: Option<&[f32]>,
        ib_mean: Option<&[f32]>,
        ib_log_var: Option<&[f32]>,
    ) -> GeometryReport {
        let n = keys.len();
        if n == 0 {
            return GeometryReport::empty();
        }

        let _dim = keys[0].len();

        // 1. OT distance (simplified sliced Wasserstein)
        let ot_mean = self.compute_ot_distance(query, keys);

        // 2. Topology coherence
        let coherence = self.compute_coherence(keys);

        // 3. H0 persistence (optional)
        let h0_sum = if self.config.compute_persistence {
            Some(self.compute_h0_persistence(keys))
        } else {
            None
        };

        // 4. IB KL
        let ib_kl = match (ib_mean, ib_log_var) {
            (Some(m), Some(v)) => KLDivergence::gaussian_to_unit_arrays(m, v),
            _ => 0.0,
        };

        // 5. Diffusion energy
        let diffusion_energy = self.compute_diffusion_energy(query, keys);

        // 6. Attention entropy
        let entropy = match attention_weights {
            Some(w) => self.compute_entropy(w),
            None => (n as f32).ln(), // Max entropy
        };

        // Build metrics
        let mut metrics = vec![
            MetricValue::new(MetricType::OTDistance, ot_mean, 0.0, 10.0, 5.0, 8.0),
            MetricValue::new(MetricType::TopologyCoherence, coherence, 0.0, 1.0, 0.3, 0.1),
            MetricValue::new(MetricType::IBKL, ib_kl, 0.0, 100.0, 50.0, 80.0),
            MetricValue::new(
                MetricType::DiffusionEnergy,
                diffusion_energy,
                0.0,
                100.0,
                50.0,
                80.0,
            ),
            MetricValue::new(
                MetricType::AttentionEntropy,
                entropy,
                0.0,
                (n as f32).ln().max(1.0),
                0.5,
                0.2,
            ),
        ];

        if let Some(h0) = h0_sum {
            metrics.push(MetricValue::new(
                MetricType::H0Persistence,
                h0,
                0.0,
                100.0,
                50.0,
                80.0,
            ));
        }

        // Compute health score
        let health_score = self.compute_health_score(&metrics);

        // Determine recommendation
        let recommendation = self.determine_recommendation(&metrics, coherence, entropy, n);

        GeometryReport {
            ot_mean_distance: ot_mean,
            topology_coherence: coherence,
            h0_death_sum: h0_sum,
            ib_kl,
            diffusion_energy,
            attention_entropy: entropy,
            metrics,
            health_score,
            recommendation,
        }
    }

    /// Simplified sliced Wasserstein distance
    fn compute_ot_distance(&self, query: &[f32], keys: &[&[f32]]) -> f32 {
        let dim = query.len();
        let n = keys.len();
        if n == 0 {
            return 0.0;
        }

        // Generate random projections
        let mut rng_state = self.config.seed;
        let projections: Vec<Vec<f32>> = (0..self.config.ot_projections)
            .map(|_| self.random_unit_vector(dim, &mut rng_state))
            .collect();

        // Project query
        let q_projs: Vec<f32> = projections.iter().map(|p| Self::dot(query, p)).collect();

        // Mean absolute distance over keys
        let mut total = 0.0f32;
        for key in keys {
            let mut dist = 0.0f32;
            for (i, proj) in projections.iter().enumerate() {
                let k_proj = Self::dot(key, proj);
                dist += (q_projs[i] - k_proj).abs();
            }
            total += dist / self.config.ot_projections as f32;
        }

        total / n as f32
    }

    /// Compute coherence using WindowCoherence
    fn compute_coherence(&self, keys: &[&[f32]]) -> f32 {
        use crate::topology::CoherenceMetric;

        let coherence = WindowCoherence::compute(
            keys,
            self.config.knn_k,
            &[
                CoherenceMetric::BoundaryMass,
                CoherenceMetric::SimilarityVariance,
            ],
        );

        coherence.score
    }

    /// Compute H0 persistence (expensive)
    fn compute_h0_persistence(&self, keys: &[&[f32]]) -> f32 {
        let n = keys.len();
        if n <= 1 {
            return 0.0;
        }

        // Build distance matrix
        let mut edges: Vec<(f32, usize, usize)> = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let dist = Self::l2_distance(keys[i], keys[j]);
                edges.push((dist, i, j));
            }
        }

        edges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Union-Find for Kruskal's algorithm
        let mut parent: Vec<usize> = (0..n).collect();
        let mut rank = vec![0u8; n];
        let mut deaths = Vec::new();

        fn find(parent: &mut [usize], x: usize) -> usize {
            if parent[x] != x {
                parent[x] = find(parent, parent[x]);
            }
            parent[x]
        }

        fn union(parent: &mut [usize], rank: &mut [u8], a: usize, b: usize) -> bool {
            let mut ra = find(parent, a);
            let mut rb = find(parent, b);
            if ra == rb {
                return false;
            }
            if rank[ra] < rank[rb] {
                std::mem::swap(&mut ra, &mut rb);
            }
            parent[rb] = ra;
            if rank[ra] == rank[rb] {
                rank[ra] += 1;
            }
            true
        }

        for (w, i, j) in edges {
            if union(&mut parent, &mut rank, i, j) {
                deaths.push(w);
                if deaths.len() == n - 1 {
                    break;
                }
            }
        }

        // Remove last (infinite lifetime component)
        if !deaths.is_empty() {
            deaths.pop();
        }

        deaths.iter().sum()
    }

    /// Compute diffusion energy
    fn compute_diffusion_energy(&self, query: &[f32], keys: &[&[f32]]) -> f32 {
        use crate::pde_attention::LaplacianType;

        let n = keys.len();
        if n == 0 {
            return 0.0;
        }

        // Initial logits
        let x: Vec<f32> = keys.iter().map(|k| Self::dot(query, k)).collect();

        // Build Laplacian
        let lap = GraphLaplacian::from_keys(
            keys,
            self.config.diffusion_sigma,
            LaplacianType::Unnormalized,
        );

        // Energy = x^T L x
        let lx = lap.apply(&x);
        Self::dot(&x, &lx)
    }

    /// Compute entropy
    fn compute_entropy(&self, weights: &[f32]) -> f32 {
        let eps = 1e-10;
        let mut entropy = 0.0f32;

        for &w in weights {
            if w > eps {
                entropy -= w * w.ln();
            }
        }

        entropy.max(0.0)
    }

    /// Compute overall health score
    fn compute_health_score(&self, metrics: &[MetricValue]) -> f32 {
        if metrics.is_empty() {
            return 1.0;
        }

        let healthy_count = metrics.iter().filter(|m| m.is_healthy).count();
        healthy_count as f32 / metrics.len() as f32
    }

    /// Determine recommendation
    fn determine_recommendation(
        &self,
        metrics: &[MetricValue],
        coherence: f32,
        entropy: f32,
        n: usize,
    ) -> AttentionRecommendation {
        let max_entropy = (n as f32).ln().max(1.0);
        let entropy_ratio = entropy / max_entropy;

        // Check for critical conditions
        let has_critical = metrics.iter().any(|m| m.is_critical());
        if has_critical {
            return AttentionRecommendation::Freeze;
        }

        // Low coherence = cautious mode
        if coherence < 0.3 {
            return AttentionRecommendation::Cautious;
        }

        // Very low entropy = temperature too low
        if entropy_ratio < 0.2 {
            return AttentionRecommendation::IncreaseTemperature;
        }

        // Very high entropy = temperature too high
        if entropy_ratio > 0.9 {
            return AttentionRecommendation::DecreaseTemperature;
        }

        // Check for warnings
        let has_warning = metrics.iter().any(|m| m.is_warning());
        if has_warning {
            return AttentionRecommendation::AddRegularization;
        }

        AttentionRecommendation::Stable
    }

    /// Generate random unit vector
    fn random_unit_vector(&self, dim: usize, state: &mut u64) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        for i in 0..dim {
            // XorShift
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            let u = (*state & 0x00FF_FFFF) as f32 / 16_777_216.0;
            v[i] = u * 2.0 - 1.0;
        }

        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }

        v
    }

    /// Dot product
    #[inline]
    fn dot(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(&ai, &bi)| ai * bi).sum()
    }

    /// L2 distance
    #[inline]
    fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(&ai, &bi)| (ai - bi) * (ai - bi))
            .sum::<f32>()
            .sqrt()
    }
}

impl GeometryReport {
    /// Create empty report
    pub fn empty() -> Self {
        Self {
            ot_mean_distance: 0.0,
            topology_coherence: 1.0,
            h0_death_sum: None,
            ib_kl: 0.0,
            diffusion_energy: 0.0,
            attention_entropy: 0.0,
            metrics: vec![],
            health_score: 1.0,
            recommendation: AttentionRecommendation::Stable,
        }
    }

    /// Check if attention is healthy
    pub fn is_healthy(&self) -> bool {
        self.health_score > 0.7
    }

    /// Get all warning metrics
    pub fn warnings(&self) -> Vec<&MetricValue> {
        self.metrics.iter().filter(|m| m.is_warning()).collect()
    }

    /// Get all critical metrics
    pub fn criticals(&self) -> Vec<&MetricValue> {
        self.metrics.iter().filter(|m| m.is_critical()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_builder() {
        let builder = ReportBuilder::new(ReportConfig::default());

        let query = vec![1.0f32; 16];
        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32 * 0.1; 16]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let report = builder.build(&query, &keys_refs, None, None, None);

        assert!(report.topology_coherence >= 0.0);
        assert!(report.topology_coherence <= 1.0);
        assert!(report.health_score >= 0.0);
        assert!(report.health_score <= 1.0);
    }

    #[test]
    fn test_empty_report() {
        let report = GeometryReport::empty();
        assert!(report.is_healthy());
        assert_eq!(report.recommendation, AttentionRecommendation::Stable);
    }

    #[test]
    fn test_with_attention_weights() {
        let builder = ReportBuilder::new(ReportConfig::default());

        let query = vec![1.0f32; 8];
        let keys: Vec<Vec<f32>> = vec![vec![1.0; 8], vec![0.9; 8], vec![0.1; 8]];
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let weights = vec![0.6, 0.3, 0.1];

        let report = builder.build(&query, &keys_refs, Some(&weights), None, None);

        assert!(report.attention_entropy > 0.0);
    }
}
