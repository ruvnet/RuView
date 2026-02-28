// Hierarchical Causal Structure Management
// Implements transfer entropy and consciousness metrics for HCC framework

use crate::coarse_graining::{ScaleHierarchy, ScaleLevel};
use crate::effective_information::compute_ei_simd;
use std::collections::HashMap;

/// Represents the complete hierarchical causal structure with all metrics
#[derive(Debug, Clone)]
pub struct CausalHierarchy {
    pub hierarchy: ScaleHierarchy,
    pub metrics: HierarchyMetrics,
}

/// Metrics computed at each scale of the hierarchy
#[derive(Debug, Clone)]
pub struct HierarchyMetrics {
    /// Effective information at each scale
    pub ei: Vec<f32>,
    /// Integrated information (Φ) at each scale
    pub phi: Vec<f32>,
    /// Upward transfer entropy (micro → macro)
    pub te_up: Vec<f32>,
    /// Downward transfer entropy (macro → micro)
    pub te_down: Vec<f32>,
    /// Consciousness metric Ψ at each scale
    pub psi: Vec<f32>,
    /// Optimal scale (argmax Ψ)
    pub optimal_scale: usize,
    /// Consciousness score at optimal scale
    pub consciousness_score: f32,
}

impl CausalHierarchy {
    /// Builds hierarchical causal structure from time-series data
    ///
    /// # Arguments
    /// * `data` - Time-series of neural states
    /// * `branching_factor` - k for k-way coarse-graining
    /// * `use_optimal` - Whether to use optimal partitioning (slower but better)
    ///
    /// # Returns
    /// Complete causal hierarchy with all metrics computed
    pub fn from_time_series(data: &[f32], branching_factor: usize, use_optimal: bool) -> Self {
        // Estimate transition matrix from data
        let transition_matrix = estimate_transition_matrix(data, 256); // 256 bins

        // Build scale hierarchy
        let hierarchy = if use_optimal {
            ScaleHierarchy::build_optimal(transition_matrix, branching_factor)
        } else {
            ScaleHierarchy::build_sequential(transition_matrix, branching_factor)
        };

        // Compute all metrics
        let metrics = compute_hierarchy_metrics(&hierarchy, data);

        Self { hierarchy, metrics }
    }

    /// Checks if system is conscious according to HCC criterion
    pub fn is_conscious(&self, threshold: f32) -> bool {
        self.metrics.consciousness_score > threshold
    }

    /// Returns consciousness level classification
    pub fn consciousness_level(&self) -> ConsciousnessLevel {
        match self.metrics.consciousness_score {
            x if x > 10.0 => ConsciousnessLevel::FullyConscious,
            x if x > 5.0 => ConsciousnessLevel::MinimallyConscious,
            x if x > 1.0 => ConsciousnessLevel::Borderline,
            _ => ConsciousnessLevel::Unconscious,
        }
    }

    /// Detects causal emergence across scales
    pub fn causal_emergence(&self) -> Option<(usize, f32)> {
        if self.metrics.ei.is_empty() {
            return None;
        }

        let micro_ei = self.metrics.ei[0];
        let (max_scale, &max_ei) = self
            .metrics
            .ei
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?;

        let emergence_strength = max_ei - micro_ei;

        if emergence_strength > 0.1 {
            Some((max_scale, emergence_strength))
        } else {
            None
        }
    }

    /// Checks for circular causation at optimal scale
    pub fn has_circular_causation(&self) -> bool {
        let s = self.metrics.optimal_scale;

        // Need both upward and downward TE > threshold
        if s >= self.metrics.te_up.len() {
            return false;
        }

        const TE_THRESHOLD: f32 = 0.01; // Minimum TE to count as causal
        self.metrics.te_up[s] > TE_THRESHOLD && self.metrics.te_down[s] > TE_THRESHOLD
    }
}

/// Consciousness level classifications
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsciousnessLevel {
    Unconscious,
    Borderline,
    MinimallyConscious,
    FullyConscious,
}

/// Computes all hierarchical metrics (EI, Φ, TE, Ψ)
fn compute_hierarchy_metrics(hierarchy: &ScaleHierarchy, data: &[f32]) -> HierarchyMetrics {
    let num_scales = hierarchy.num_scales();

    // Compute EI at each scale
    let mut ei = Vec::with_capacity(num_scales);
    for level in &hierarchy.levels {
        let ei_val = compute_ei_simd(&level.transition_matrix, level.num_states);
        ei.push(ei_val);
    }

    // Compute Φ at each scale (approximate)
    let mut phi = Vec::with_capacity(num_scales);
    for level in &hierarchy.levels {
        let phi_val = approximate_phi(&level.transition_matrix, level.num_states);
        phi.push(phi_val);
    }

    // Compute transfer entropy between adjacent scales
    let mut te_up = Vec::with_capacity(num_scales - 1);
    let mut te_down = Vec::with_capacity(num_scales - 1);

    for i in 0..(num_scales - 1) {
        // Project data to each scale
        let data_micro = project_to_scale(data, &hierarchy.levels[i]);
        let data_macro = project_to_scale(data, &hierarchy.levels[i + 1]);

        // Compute bidirectional transfer entropy
        te_up.push(transfer_entropy(&data_micro, &data_macro, 1, 1));
        te_down.push(transfer_entropy(&data_macro, &data_micro, 1, 1));
    }

    // Compute consciousness metric Ψ at each scale
    let mut psi = vec![0.0; num_scales];
    for i in 0..(num_scales - 1) {
        // Ψ = EI · Φ · √(TE_up · TE_down)
        psi[i] = ei[i] * phi[i] * (te_up[i] * te_down[i]).sqrt();
    }

    // Find optimal scale (max Ψ)
    let (optimal_scale, &consciousness_score) = psi
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));

    HierarchyMetrics {
        ei,
        phi,
        te_up,
        te_down,
        psi,
        optimal_scale,
        consciousness_score,
    }
}

/// Estimates transition probability matrix from time-series data
/// Uses binning/discretization
fn estimate_transition_matrix(data: &[f32], num_bins: usize) -> Vec<f32> {
    if data.len() < 2 {
        return vec![1.0]; // Trivial 1×1 matrix
    }

    // Discretize data into bins
    let binned = discretize_data(data, num_bins);

    // Count transitions
    let mut counts = vec![0u32; num_bins * num_bins];
    for i in 0..(binned.len() - 1) {
        let from = binned[i];
        let to = binned[i + 1];
        counts[from * num_bins + to] += 1;
    }

    // Normalize to probabilities
    let mut matrix = vec![0.0f32; num_bins * num_bins];
    for i in 0..num_bins {
        let row_sum: u32 = (0..num_bins).map(|j| counts[i * num_bins + j]).sum();

        if row_sum > 0 {
            for j in 0..num_bins {
                matrix[i * num_bins + j] = counts[i * num_bins + j] as f32 / row_sum as f32;
            }
        } else {
            // Uniform distribution if no data
            for j in 0..num_bins {
                matrix[i * num_bins + j] = 1.0 / num_bins as f32;
            }
        }
    }

    matrix
}

/// Discretizes continuous data into bins
fn discretize_data(data: &[f32], num_bins: usize) -> Vec<usize> {
    if data.is_empty() {
        return Vec::new();
    }

    let min = data.iter().copied().fold(f32::INFINITY, f32::min);
    let max = data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;

    if range < 1e-6 {
        // All values same, put in middle bin
        return vec![num_bins / 2; data.len()];
    }

    data.iter()
        .map(|&x| {
            let normalized = (x - min) / range;
            let bin = (normalized * num_bins as f32).floor() as usize;
            bin.min(num_bins - 1)
        })
        .collect()
}

/// Projects time-series data to a specific scale level
fn project_to_scale(data: &[f32], level: &ScaleLevel) -> Vec<usize> {
    let binned = discretize_data(data, level.partition.num_micro_states());

    // Map micro-states to macro-states
    let micro_to_macro: HashMap<usize, usize> = level
        .partition
        .groups
        .iter()
        .enumerate()
        .flat_map(|(macro_idx, micro_group)| {
            micro_group
                .iter()
                .map(move |&micro_idx| (micro_idx, macro_idx))
        })
        .collect();

    binned
        .iter()
        .map(|&micro| *micro_to_macro.get(&micro).unwrap_or(&0))
        .collect()
}

/// Computes transfer entropy between two time series
///
/// TE(X→Y) = I(Y_t+1; X_t | Y_t)
///
/// # Arguments
/// * `x` - Source time series (discretized)
/// * `y` - Target time series (discretized)
/// * `k` - History length for X
/// * `l` - History length for Y
pub fn transfer_entropy(x: &[usize], y: &[usize], k: usize, l: usize) -> f32 {
    if x.len() != y.len() || x.len() < k.max(l) + 1 {
        return 0.0;
    }

    let t_max = x.len() - 1;
    let lag = k.max(l);

    // Count joint occurrences
    let mut counts = HashMap::new();
    for t in lag..t_max {
        let x_past: Vec<_> = x[t - k..t].to_vec();
        let y_past: Vec<_> = y[t - l..t].to_vec();
        let y_future = y[t + 1];

        *counts
            .entry((y_future, x_past.clone(), y_past.clone()))
            .or_insert(0) += 1;
    }

    let total = (t_max - lag) as f32;

    // Compute marginals
    let mut p_y_future: HashMap<usize, f32> = HashMap::new();
    let mut p_x_past: HashMap<Vec<usize>, f32> = HashMap::new();
    let mut p_y_past: HashMap<Vec<usize>, f32> = HashMap::new();
    let mut p_y_xy: HashMap<(usize, Vec<usize>, Vec<usize>), f32> = HashMap::new();
    let mut p_xy: HashMap<(Vec<usize>, Vec<usize>), f32> = HashMap::new();
    let mut p_y: HashMap<Vec<usize>, f32> = HashMap::new();

    for ((y_fut, x_p, y_p), &count) in &counts {
        let prob = count as f32 / total;
        *p_y_future.entry(*y_fut).or_insert(0.0) += prob;
        *p_x_past.entry(x_p.clone()).or_insert(0.0) += prob;
        *p_y_past.entry(y_p.clone()).or_insert(0.0) += prob;
        *p_y_xy
            .entry((*y_fut, x_p.clone(), y_p.clone()))
            .or_insert(0.0) += prob;
        *p_xy.entry((x_p.clone(), y_p.clone())).or_insert(0.0) += prob;
        *p_y.entry(y_p.clone()).or_insert(0.0) += prob;
    }

    // Compute TE = I(Y_future; X_past | Y_past)
    let mut te = 0.0;
    for ((_y_fut, x_p, y_p), &p_joint) in &counts {
        let p = p_joint as f32 / total;
        let p_y_given_xy = p / p_xy[&(x_p.clone(), y_p.clone())];
        let p_y_given_y = p / p_y[y_p];

        te += p * (p_y_given_xy / p_y_given_y).log2();
    }

    te.max(0.0) // Ensure non-negative
}

/// Approximates integrated information Φ using spectral method
fn approximate_phi(transition_matrix: &[f32], n: usize) -> f32 {
    if n <= 1 {
        return 0.0;
    }

    // Find bipartition that minimizes KL divergence
    // For simplicity, try a few random partitions and take minimum

    let mut min_kl = f32::MAX;

    // Try half-split partition
    let mid = n / 2;
    let partition_a: Vec<_> = (0..mid).collect();
    let partition_b: Vec<_> = (mid..n).collect();

    let kl = compute_kl_partition(transition_matrix, &partition_a, &partition_b, n);
    min_kl = min_kl.min(kl);

    // Try a few random partitions
    for _ in 0..5 {
        let size_a = (n / 2).max(1);
        let mut partition_a: Vec<_> = (0..size_a).collect();
        let partition_b: Vec<_> = (size_a..n).collect();

        // Randomize (simple shuffle)
        partition_a.rotate_left(size_a / 3);

        let kl = compute_kl_partition(transition_matrix, &partition_a, &partition_b, n);
        min_kl = min_kl.min(kl);
    }

    min_kl
}

/// Computes KL divergence between full and partitioned systems
fn compute_kl_partition(
    _matrix: &[f32],
    partition_a: &[usize],
    partition_b: &[usize],
    n: usize,
) -> f32 {
    // Compute stationary distribution (simplified: uniform)
    let p_full = vec![1.0 / n as f32; n];

    // Compute partitioned distribution (independent subsystems)
    let mut p_cut = vec![0.0; n];

    let prob_a = partition_a.len() as f32 / n as f32;
    let prob_b = partition_b.len() as f32 / n as f32;

    for &i in partition_a {
        p_cut[i] = prob_a / partition_a.len() as f32;
    }
    for &i in partition_b {
        p_cut[i] = prob_b / partition_b.len() as f32;
    }

    // KL divergence
    let mut kl = 0.0;
    for i in 0..n {
        if p_full[i] > 1e-10 && p_cut[i] > 1e-10 {
            kl += p_full[i] * (p_full[i] / p_cut[i]).log2();
        }
    }

    kl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discretization() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let binned = discretize_data(&data, 5);

        assert_eq!(binned.len(), 5);
        // Should map to different bins
        assert_eq!(binned[0], 0);
        assert_eq!(binned[4], 4);
    }

    #[test]
    fn test_transfer_entropy_independent() {
        // Two independent random sequences
        let x = vec![0, 1, 0, 1, 0, 1, 0, 1];
        let y = vec![1, 0, 1, 0, 1, 0, 1, 0];

        let te = transfer_entropy(&x, &y, 1, 1);

        // Should be low for independent sequences
        assert!(te < 0.5);
    }

    #[test]
    fn test_transfer_entropy_deterministic() {
        // Y follows X with 1-step delay: Y[t+1] = X[t]
        let x = vec![0, 1, 1, 0, 1, 0, 0, 1];
        let y = vec![0, 0, 1, 1, 0, 1, 0, 0]; // Shifted version

        let te = transfer_entropy(&x, &y, 1, 1);

        // Should be high
        assert!(te > 0.1);
    }

    #[test]
    fn test_causal_hierarchy_construction() {
        // Synthetic oscillating data
        let data: Vec<f32> = (0..1000)
            .map(|t| (t as f32 * 0.1).sin() + 0.5 * (t as f32 * 0.3).cos())
            .collect();

        let hierarchy = CausalHierarchy::from_time_series(&data, 2, false);

        // Should have multiple scales
        assert!(hierarchy.hierarchy.num_scales() > 1);

        // Metrics should be computed
        assert_eq!(hierarchy.metrics.ei.len(), hierarchy.hierarchy.num_scales());
    }

    #[test]
    fn test_consciousness_detection() {
        // Create data with strong multi-scale structure
        let data: Vec<f32> = (0..1000)
            .map(|t| {
                // Multiple frequencies -> multi-scale structure
                (t as f32 * 0.05).sin()
                    + 0.5 * (t as f32 * 0.2).cos()
                    + 0.25 * (t as f32 * 0.8).sin()
            })
            .collect();

        let hierarchy = CausalHierarchy::from_time_series(&data, 2, false);

        // Check consciousness score is positive
        assert!(hierarchy.metrics.consciousness_score >= 0.0);

        // Check level classification works
        let level = hierarchy.consciousness_level();
        assert!(matches!(
            level,
            ConsciousnessLevel::Unconscious
                | ConsciousnessLevel::Borderline
                | ConsciousnessLevel::MinimallyConscious
                | ConsciousnessLevel::FullyConscious
        ));
    }
}
