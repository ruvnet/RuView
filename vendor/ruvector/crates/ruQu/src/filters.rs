//! Three-Filter Decision Pipeline for ruQu Coherence Gate
//!
//! This module implements the core decision logic for the ruQu coherence gate,
//! consisting of three stacked filters that must all agree for system permit.
//!
//! ## Filter 1: Structural (Min-Cut Based)
//!
//! Detects partition formation in the operational graph using dynamic min-cut.
//! A low cut value indicates that the system is splitting into incoherent partitions.
//!
//! ## Filter 2: Shift (Distribution Drift)
//!
//! Aggregates nonconformity scores to detect when the system's behavior is
//! drifting from expected distributions.
//!
//! ## Filter 3: Evidence (E-Value Accumulation)
//!
//! Uses anytime-valid e-value testing to make statistically rigorous decisions
//! that can be made at any stopping time.
//!
//! ## Decision Logic
//!
//! ```text
//! PERMIT: All three filters pass
//! DENY:   Any filter definitively fails
//! DEFER:  Evidence still accumulating
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

// Integration with ruvector-mincut when available
#[cfg(feature = "structural")]
use ruvector_mincut::{SubpolyConfig, SubpolynomialMinCut};

/// Error types for filter operations
#[derive(Error, Debug)]
pub enum FilterError {
    /// Invalid threshold configuration
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),

    /// System state is malformed
    #[error("Invalid system state: {0}")]
    InvalidState(String),

    /// Structural filter error
    #[error("Structural filter error: {0}")]
    StructuralError(String),

    /// Shift filter error
    #[error("Shift filter error: {0}")]
    ShiftError(String),

    /// Evidence filter error
    #[error("Evidence filter error: {0}")]
    EvidenceError(String),
}

/// Result type for filter operations
pub type Result<T> = std::result::Result<T, FilterError>;

// ============================================================================
// Core Types
// ============================================================================

/// Unique identifier for an edge in the operational graph
pub type EdgeId = u64;

/// Unique identifier for a vertex (qubit, coupler, etc.)
pub type VertexId = u64;

/// Weight on an edge (coupling strength, correlation, etc.)
pub type Weight = f64;

/// A bitmask representing regions of the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RegionMask(pub u64);

impl RegionMask {
    /// Create an empty region mask
    pub fn empty() -> Self {
        Self(0)
    }

    /// Create a mask with all regions set
    pub fn all() -> Self {
        Self(u64::MAX)
    }

    /// Set a region bit
    pub fn set(&mut self, region: u8) {
        self.0 |= 1u64 << region;
    }

    /// Clear a region bit
    pub fn clear(&mut self, region: u8) {
        self.0 &= !(1u64 << region);
    }

    /// Check if a region is set
    pub fn is_set(&self, region: u8) -> bool {
        (self.0 & (1u64 << region)) != 0
    }

    /// Count the number of set regions
    pub fn count(&self) -> u32 {
        self.0.count_ones()
    }

    /// Check if any region is set
    pub fn any(&self) -> bool {
        self.0 != 0
    }

    /// Union with another mask
    pub fn union(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection with another mask
    pub fn intersection(&self, other: &Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// The verdict from the filter pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// System is coherent, operations may proceed
    Permit,
    /// System is incoherent, operations should be halted
    Deny,
    /// Waiting for more evidence (intermediate state)
    Defer,
}

/// Represents the current state of the quantum system
#[derive(Debug, Clone)]
pub struct SystemState {
    /// Number of qubits/vertices
    pub num_vertices: usize,
    /// Adjacency representation: vertex -> [(neighbor, weight)]
    pub adjacency: HashMap<VertexId, Vec<(VertexId, Weight)>>,
    /// Current syndrome observations
    pub syndromes: Vec<f64>,
    /// Historical syndrome window for shift detection
    pub syndrome_history: Vec<Vec<f64>>,
    /// Nonconformity scores per region
    pub nonconformity_scores: Vec<f64>,
    /// Region assignments for vertices
    pub vertex_regions: HashMap<VertexId, u8>,
    /// Current cycle number
    pub cycle: u64,
}

impl SystemState {
    /// Create a new empty system state
    pub fn new(num_vertices: usize) -> Self {
        Self {
            num_vertices,
            adjacency: HashMap::new(),
            syndromes: Vec::new(),
            syndrome_history: Vec::new(),
            nonconformity_scores: Vec::new(),
            vertex_regions: HashMap::new(),
            cycle: 0,
        }
    }

    /// Add an edge to the operational graph
    pub fn add_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) {
        self.adjacency.entry(u).or_default().push((v, weight));
        self.adjacency.entry(v).or_default().push((u, weight));
    }

    /// Add syndrome observation
    pub fn add_syndrome(&mut self, syndrome: f64) {
        self.syndromes.push(syndrome);
    }

    /// Push current syndromes to history and clear
    pub fn advance_cycle(&mut self) {
        if !self.syndromes.is_empty() {
            self.syndrome_history.push(self.syndromes.clone());
            self.syndromes.clear();
        }
        self.cycle += 1;
    }

    /// Set nonconformity score for a region
    pub fn set_nonconformity(&mut self, region: usize, score: f64) {
        if self.nonconformity_scores.len() <= region {
            self.nonconformity_scores.resize(region + 1, 0.0);
        }
        self.nonconformity_scores[region] = score;
    }

    /// Assign a vertex to a region
    pub fn assign_region(&mut self, vertex: VertexId, region: u8) {
        self.vertex_regions.insert(vertex, region);
    }
}

// ============================================================================
// Filter 1: Structural Filter (Min-Cut Based)
// ============================================================================

/// Configuration for the structural filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralConfig {
    /// Minimum cut threshold for coherence
    pub threshold: f64,
    /// Maximum cut size to consider (lambda_max)
    pub max_cut_size: u64,
    /// Enable subpolynomial algorithm (vs simple approximation)
    pub use_subpolynomial: bool,
    /// Expansion parameter phi for expander decomposition
    pub phi: f64,
}

impl Default for StructuralConfig {
    fn default() -> Self {
        Self {
            threshold: 2.0,
            max_cut_size: 1000,
            use_subpolynomial: true,
            phi: 0.01,
        }
    }
}

/// Result from structural filter evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralResult {
    /// The computed minimum cut value
    pub cut_value: f64,
    /// Edges in the boundary (if cut is below threshold)
    pub boundary_edges: Vec<EdgeId>,
    /// Whether the system is structurally coherent
    pub is_coherent: bool,
    /// Vertices on the "healthy" side of the cut
    pub healthy_vertices: Option<Vec<VertexId>>,
    /// Vertices on the "unhealthy" side of the cut
    pub unhealthy_vertices: Option<Vec<VertexId>>,
    /// Time taken for computation (microseconds)
    pub compute_time_us: u64,
}

/// Structural filter using dynamic min-cut
#[derive(Debug)]
pub struct StructuralFilter {
    /// Configuration
    config: StructuralConfig,
    /// The min-cut data structure (when using subpolynomial algorithm)
    #[cfg(feature = "structural")]
    mincut: Option<SubpolynomialMinCut>,
    /// Simple adjacency for non-subpolynomial mode
    adjacency: HashMap<VertexId, HashMap<VertexId, Weight>>,
    /// Edge ID counter
    next_edge_id: u64,
    /// Edge ID mapping
    edge_ids: HashMap<(VertexId, VertexId), EdgeId>,
}

impl StructuralFilter {
    /// Create a new structural filter with the given threshold
    pub fn new(threshold: f64) -> Self {
        Self::with_config(StructuralConfig {
            threshold,
            ..Default::default()
        })
    }

    /// Create with full configuration
    pub fn with_config(config: StructuralConfig) -> Self {
        #[cfg(feature = "structural")]
        let mincut = if config.use_subpolynomial {
            let subpoly_config = SubpolyConfig {
                phi: config.phi,
                lambda_max: config.max_cut_size,
                ..Default::default()
            };
            Some(SubpolynomialMinCut::new(subpoly_config))
        } else {
            None
        };

        Self {
            config,
            #[cfg(feature = "structural")]
            mincut,
            adjacency: HashMap::new(),
            next_edge_id: 1,
            edge_ids: HashMap::new(),
        }
    }

    /// Insert an edge into the graph
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<EdgeId> {
        let key = Self::edge_key(u, v);

        if self.edge_ids.contains_key(&key) {
            return Err(FilterError::StructuralError(format!(
                "Edge ({}, {}) already exists",
                u, v
            )));
        }

        let edge_id = self.next_edge_id;
        self.next_edge_id += 1;
        self.edge_ids.insert(key, edge_id);

        // Update local adjacency
        self.adjacency.entry(u).or_default().insert(v, weight);
        self.adjacency.entry(v).or_default().insert(u, weight);

        // Update subpolynomial mincut if enabled
        #[cfg(feature = "structural")]
        if let Some(ref mut mc) = self.mincut {
            let _ = mc.insert_edge(u, v, weight);
        }

        Ok(edge_id)
    }

    /// Delete an edge from the graph
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()> {
        let key = Self::edge_key(u, v);

        if self.edge_ids.remove(&key).is_none() {
            return Err(FilterError::StructuralError(format!(
                "Edge ({}, {}) not found",
                u, v
            )));
        }

        // Update local adjacency
        if let Some(neighbors) = self.adjacency.get_mut(&u) {
            neighbors.remove(&v);
        }
        if let Some(neighbors) = self.adjacency.get_mut(&v) {
            neighbors.remove(&u);
        }

        // Update subpolynomial mincut if enabled
        #[cfg(feature = "structural")]
        if let Some(ref mut mc) = self.mincut {
            let _ = mc.delete_edge(u, v);
        }

        Ok(())
    }

    /// Build the hierarchy (required for subpolynomial queries)
    pub fn build(&mut self) {
        #[cfg(feature = "structural")]
        if let Some(ref mut mc) = self.mincut {
            mc.build();
        }
    }

    /// Evaluate structural coherence of the system
    pub fn evaluate(&self, _state: &SystemState) -> StructuralResult {
        let start = std::time::Instant::now();

        // Get the minimum cut value
        #[cfg(feature = "structural")]
        let cut_value = if let Some(ref mc) = self.mincut {
            mc.min_cut_value()
        } else {
            self.compute_simple_cut()
        };

        #[cfg(not(feature = "structural"))]
        let cut_value = self.compute_simple_cut();

        let is_coherent = cut_value >= self.config.threshold;

        // Get boundary edges if cut is below threshold
        let boundary_edges = if !is_coherent {
            self.find_boundary_edges(cut_value)
        } else {
            Vec::new()
        };

        StructuralResult {
            cut_value,
            boundary_edges,
            is_coherent,
            healthy_vertices: None, // Would require more complex partition tracking
            unhealthy_vertices: None,
            compute_time_us: start.elapsed().as_micros() as u64,
        }
    }

    /// Compute a simple approximation of the minimum cut
    fn compute_simple_cut(&self) -> f64 {
        if self.adjacency.is_empty() {
            return f64::INFINITY;
        }

        // Simple approximation: minimum vertex cut (sum of edge weights to any vertex)
        let mut min_cut = f64::INFINITY;

        for (_, neighbors) in &self.adjacency {
            let vertex_cut: f64 = neighbors.values().sum();
            min_cut = min_cut.min(vertex_cut);
        }

        min_cut
    }

    /// Find the edges in the boundary (crossing the min-cut)
    fn find_boundary_edges(&self, _cut_value: f64) -> Vec<EdgeId> {
        // Simplified: return edges with lowest weight contribution
        let mut edges: Vec<_> = self.edge_ids.iter().collect();
        edges.sort_by(|a, b| {
            let weight_a = self
                .adjacency
                .get(&a.0 .0)
                .and_then(|n| n.get(&a.0 .1))
                .unwrap_or(&1.0);
            let weight_b = self
                .adjacency
                .get(&b.0 .0)
                .and_then(|n| n.get(&b.0 .1))
                .unwrap_or(&1.0);
            weight_a.partial_cmp(weight_b).unwrap()
        });

        edges.into_iter().take(10).map(|(_, &id)| id).collect()
    }

    fn edge_key(u: VertexId, v: VertexId) -> (VertexId, VertexId) {
        if u < v {
            (u, v)
        } else {
            (v, u)
        }
    }

    /// Get the threshold
    pub fn threshold(&self) -> f64 {
        self.config.threshold
    }
}

// ============================================================================
// Filter 2: Shift Filter (Distribution Drift)
// ============================================================================

/// Configuration for the shift filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftConfig {
    /// Threshold for aggregate shift pressure
    pub threshold: f64,
    /// Window size for history comparison
    pub window_size: usize,
    /// Decay factor for older observations
    pub decay_factor: f64,
    /// Number of regions to track
    pub num_regions: usize,
}

impl Default for ShiftConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            window_size: 100,
            decay_factor: 0.95,
            num_regions: 64,
        }
    }
}

/// Result from shift filter evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftResult {
    /// Aggregate shift pressure (0.0 = stable, 1.0 = severe drift)
    pub pressure: f64,
    /// Regions exhibiting high shift
    pub affected_regions: RegionMask,
    /// Per-region shift values
    pub region_shifts: Vec<f64>,
    /// Whether the distribution is stable
    pub is_stable: bool,
    /// Estimated cycles until critical drift (if drifting)
    pub lead_time: Option<u64>,
}

/// Shift filter for distribution drift detection
#[derive(Debug, Clone)]
pub struct ShiftFilter {
    /// Configuration
    config: ShiftConfig,
    /// Running statistics per region
    region_stats: Vec<RegionStats>,
    /// Global running mean
    global_mean: f64,
    /// Global running variance
    global_variance: f64,
    /// Number of observations
    num_observations: u64,
}

/// Statistics for a single region
#[derive(Debug, Clone, Default)]
struct RegionStats {
    /// Running mean
    mean: f64,
    /// Running variance
    variance: f64,
    /// Sum of squared deviations from global mean
    shift_accumulator: f64,
    /// Number of observations in this region
    count: u64,
    /// Recent nonconformity scores
    recent_scores: Vec<f64>,
}

impl ShiftFilter {
    /// Create a new shift filter with the given threshold
    pub fn new(threshold: f64, window_size: usize) -> Self {
        Self::with_config(ShiftConfig {
            threshold,
            window_size,
            ..Default::default()
        })
    }

    /// Create with full configuration
    pub fn with_config(config: ShiftConfig) -> Self {
        Self {
            region_stats: vec![RegionStats::default(); config.num_regions],
            global_mean: 0.0,
            global_variance: 1.0,
            num_observations: 0,
            config,
        }
    }

    /// Update with a new nonconformity score for a region
    pub fn update(&mut self, region: usize, score: f64) {
        if region >= self.region_stats.len() {
            return;
        }

        let stats = &mut self.region_stats[region];

        // Update region statistics using Welford's online algorithm
        stats.count += 1;
        let delta = score - stats.mean;
        stats.mean += delta / stats.count as f64;
        let delta2 = score - stats.mean;
        stats.variance += delta * delta2;

        // Track recent scores
        stats.recent_scores.push(score);
        if stats.recent_scores.len() > self.config.window_size {
            stats.recent_scores.remove(0);
        }

        // Update shift accumulator
        let deviation = (score - self.global_mean).abs();
        stats.shift_accumulator = self.config.decay_factor * stats.shift_accumulator + deviation;

        // Update global statistics
        self.num_observations += 1;
        let g_delta = score - self.global_mean;
        self.global_mean += g_delta / self.num_observations as f64;
        let g_delta2 = score - self.global_mean;
        self.global_variance += g_delta * g_delta2;
    }

    /// Evaluate shift in the system state
    pub fn evaluate(&self, state: &SystemState) -> ShiftResult {
        let mut region_shifts = vec![0.0; self.config.num_regions];
        let mut affected_regions = RegionMask::empty();
        let mut total_pressure = 0.0;

        // Use nonconformity scores from state or compute from region stats
        for (region, stats) in self.region_stats.iter().enumerate() {
            let shift = if region < state.nonconformity_scores.len() {
                self.compute_shift(state.nonconformity_scores[region], stats)
            } else {
                self.compute_shift_from_stats(stats)
            };

            region_shifts[region] = shift;
            total_pressure += shift;

            if shift > self.config.threshold {
                affected_regions.set(region as u8);
            }
        }

        // Normalize pressure
        let num_active = self
            .region_stats
            .iter()
            .filter(|s| s.count > 0)
            .count()
            .max(1);
        let pressure = total_pressure / num_active as f64;

        let is_stable = pressure < self.config.threshold;

        // Estimate lead time if drifting
        let lead_time = if !is_stable && pressure > 0.0 {
            // Simple linear extrapolation
            let cycles_until_critical = ((1.0 - pressure) / pressure * 100.0) as u64;
            Some(cycles_until_critical.max(1))
        } else {
            None
        };

        ShiftResult {
            pressure,
            affected_regions,
            region_shifts,
            is_stable,
            lead_time,
        }
    }

    /// Compute shift for a single observation
    fn compute_shift(&self, score: f64, stats: &RegionStats) -> f64 {
        if stats.count < 2 {
            return 0.0;
        }

        // Compute z-score relative to region mean
        let region_std = (stats.variance / stats.count as f64).sqrt().max(1e-10);
        let z_score = (score - stats.mean).abs() / region_std;

        // Convert to probability of shift
        (z_score / 3.0).min(1.0) // Normalize to [0, 1]
    }

    /// Compute shift from accumulated statistics
    fn compute_shift_from_stats(&self, stats: &RegionStats) -> f64 {
        if stats.count < self.config.window_size as u64 / 2 {
            return 0.0;
        }

        // Use the shift accumulator normalized by observation count
        let normalized = stats.shift_accumulator / stats.count as f64;

        // Compare to global variance
        let global_std = (self.global_variance / self.num_observations.max(1) as f64)
            .sqrt()
            .max(1e-10);

        (normalized / global_std / 2.0).min(1.0)
    }

    /// Get the threshold
    pub fn threshold(&self) -> f64 {
        self.config.threshold
    }

    /// Get the window size
    pub fn window_size(&self) -> usize {
        self.config.window_size
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.region_stats = vec![RegionStats::default(); self.config.num_regions];
        self.global_mean = 0.0;
        self.global_variance = 1.0;
        self.num_observations = 0;
    }
}

// ============================================================================
// Filter 3: Evidence Filter (E-Value Accumulation)
// ============================================================================

/// Configuration for the evidence filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceConfig {
    /// Threshold for permit (accept hypothesis)
    pub tau_permit: f64,
    /// Threshold for deny (reject hypothesis)
    pub tau_deny: f64,
    /// Prior probability of coherence
    pub prior: f64,
}

impl Default for EvidenceConfig {
    fn default() -> Self {
        Self {
            tau_permit: 20.0,     // Strong evidence for permit
            tau_deny: 1.0 / 20.0, // Strong evidence for deny
            prior: 0.95,          // Assume system is usually coherent
        }
    }
}

/// Accumulator for e-values (anytime-valid inference)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceAccumulator {
    /// Log of accumulated e-value (for numerical stability)
    log_e_value: f64,
    /// Number of samples seen
    samples_seen: u64,
    /// Running evidence for coherence
    log_evidence_coherent: f64,
    /// Running evidence for incoherence
    log_evidence_incoherent: f64,
}

impl Default for EvidenceAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceAccumulator {
    /// Create a new evidence accumulator
    pub fn new() -> Self {
        Self {
            log_e_value: 0.0, // E = 1 initially
            samples_seen: 0,
            log_evidence_coherent: 0.0,
            log_evidence_incoherent: 0.0,
        }
    }

    /// Update with a new observation
    ///
    /// The likelihood ratio should be P(observation | coherent) / P(observation | incoherent)
    pub fn update(&mut self, likelihood_ratio: f64) {
        self.samples_seen += 1;

        // Clamp to avoid extreme values
        let lr = likelihood_ratio.clamp(1e-10, 1e10);

        // Update log e-value
        self.log_e_value += lr.ln();

        // Track evidence for both hypotheses
        if lr > 1.0 {
            self.log_evidence_coherent += lr.ln();
        } else {
            self.log_evidence_incoherent += (-lr.ln()).abs();
        }
    }

    /// Get the current e-value
    pub fn e_value(&self) -> f64 {
        self.log_e_value.exp().min(1e100) // Prevent overflow
    }

    /// Get the log e-value
    pub fn log_e_value(&self) -> f64 {
        self.log_e_value
    }

    /// Get number of samples seen
    pub fn samples_seen(&self) -> u64 {
        self.samples_seen
    }

    /// Reset the accumulator
    pub fn reset(&mut self) {
        self.log_e_value = 0.0;
        self.samples_seen = 0;
        self.log_evidence_coherent = 0.0;
        self.log_evidence_incoherent = 0.0;
    }

    /// Get the posterior odds ratio
    pub fn posterior_odds(&self, prior_odds: f64) -> f64 {
        prior_odds * self.e_value()
    }
}

/// Result from evidence filter evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceResult {
    /// Current e-value
    pub e_value: f64,
    /// Log e-value (for numerical stability)
    pub log_e_value: f64,
    /// Number of samples accumulated
    pub samples_seen: u64,
    /// Verdict if thresholds are crossed
    pub verdict: Option<Verdict>,
    /// Confidence in the verdict (0.0 to 1.0)
    pub confidence: f64,
}

/// Evidence filter using e-value accumulation
#[derive(Debug, Clone)]
pub struct EvidenceFilter {
    /// Configuration
    config: EvidenceConfig,
    /// The evidence accumulator
    accumulator: EvidenceAccumulator,
    /// Per-region accumulators
    region_accumulators: Vec<EvidenceAccumulator>,
}

impl EvidenceFilter {
    /// Create a new evidence filter
    pub fn new(tau_permit: f64, tau_deny: f64) -> Self {
        Self::with_config(EvidenceConfig {
            tau_permit,
            tau_deny,
            ..Default::default()
        })
    }

    /// Create with full configuration
    pub fn with_config(config: EvidenceConfig) -> Self {
        Self {
            config,
            accumulator: EvidenceAccumulator::new(),
            region_accumulators: Vec::new(),
        }
    }

    /// Update with new evidence
    pub fn update(&mut self, likelihood_ratio: f64) {
        self.accumulator.update(likelihood_ratio);
    }

    /// Update evidence for a specific region
    pub fn update_region(&mut self, region: usize, likelihood_ratio: f64) {
        while self.region_accumulators.len() <= region {
            self.region_accumulators.push(EvidenceAccumulator::new());
        }
        self.region_accumulators[region].update(likelihood_ratio);
    }

    /// Evaluate the current evidence
    pub fn evaluate(&self, _state: &SystemState) -> EvidenceResult {
        let e_value = self.accumulator.e_value();
        let log_e_value = self.accumulator.log_e_value();

        let verdict = if e_value >= self.config.tau_permit {
            Some(Verdict::Permit)
        } else if e_value <= self.config.tau_deny {
            Some(Verdict::Deny)
        } else {
            None
        };

        // Compute confidence based on distance from thresholds
        let confidence = if e_value >= self.config.tau_permit {
            ((e_value.ln() - self.config.tau_permit.ln())
                / (self.config.tau_permit.ln().abs() + 1.0))
                .min(1.0)
        } else if e_value <= self.config.tau_deny {
            ((self.config.tau_deny.ln() - e_value.ln()) / (self.config.tau_deny.ln().abs() + 1.0))
                .min(1.0)
        } else {
            0.0
        };

        EvidenceResult {
            e_value,
            log_e_value,
            samples_seen: self.accumulator.samples_seen(),
            verdict,
            confidence,
        }
    }

    /// Get the permit threshold
    pub fn tau_permit(&self) -> f64 {
        self.config.tau_permit
    }

    /// Get the deny threshold
    pub fn tau_deny(&self) -> f64 {
        self.config.tau_deny
    }

    /// Get the accumulator
    pub fn accumulator(&self) -> &EvidenceAccumulator {
        &self.accumulator
    }

    /// Reset all evidence
    pub fn reset(&mut self) {
        self.accumulator.reset();
        for acc in &mut self.region_accumulators {
            acc.reset();
        }
    }
}

// ============================================================================
// Filter Pipeline
// ============================================================================

/// Configuration for the complete filter pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Structural filter config
    pub structural: StructuralConfig,
    /// Shift filter config
    pub shift: ShiftConfig,
    /// Evidence filter config
    pub evidence: EvidenceConfig,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            structural: StructuralConfig::default(),
            shift: ShiftConfig::default(),
            evidence: EvidenceConfig::default(),
        }
    }
}

/// Combined results from all filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterResults {
    /// Result from structural filter
    pub structural: StructuralResult,
    /// Result from shift filter
    pub shift: ShiftResult,
    /// Result from evidence filter
    pub evidence: EvidenceResult,
    /// Overall verdict
    pub verdict: Option<Verdict>,
    /// Regions requiring attention
    pub affected_regions: RegionMask,
    /// Recommended actions
    pub recommendations: Vec<String>,
    /// Total evaluation time (microseconds)
    pub total_time_us: u64,
}

/// The complete three-filter decision pipeline
#[derive(Debug)]
pub struct FilterPipeline {
    /// Structural filter (min-cut based)
    structural: StructuralFilter,
    /// Shift filter (distribution drift)
    shift: ShiftFilter,
    /// Evidence filter (e-value accumulation)
    evidence: EvidenceFilter,
}

impl FilterPipeline {
    /// Create a new filter pipeline with the given configuration
    pub fn new(config: FilterConfig) -> Self {
        Self {
            structural: StructuralFilter::with_config(config.structural),
            shift: ShiftFilter::with_config(config.shift),
            evidence: EvidenceFilter::with_config(config.evidence),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(FilterConfig::default())
    }

    /// Evaluate the system state through all three filters
    ///
    /// The pipeline returns PERMIT only if ALL filters pass:
    /// - Structural: cut_value >= threshold (no partition forming)
    /// - Shift: pressure < threshold (distribution stable)
    /// - Evidence: e_value >= tau_permit (sufficient evidence)
    ///
    /// Any filter can trigger DENY or DEFER.
    pub fn evaluate(&self, state: &SystemState) -> FilterResults {
        let start = std::time::Instant::now();

        // Evaluate all three filters
        let structural_result = self.structural.evaluate(state);
        let shift_result = self.shift.evaluate(state);
        let evidence_result = self.evidence.evaluate(state);

        // Determine overall verdict
        let verdict = self.combine_verdicts(&structural_result, &shift_result, &evidence_result);

        // Collect affected regions
        let mut affected_regions = shift_result.affected_regions;

        // Add recommendations based on filter results
        let mut recommendations = Vec::new();

        if !structural_result.is_coherent {
            recommendations.push(format!(
                "Structural: Cut value {:.2} below threshold {:.2} - partition forming",
                structural_result.cut_value,
                self.structural.threshold()
            ));
        }

        if !shift_result.is_stable {
            recommendations.push(format!(
                "Shift: Pressure {:.2} above threshold {:.2} - distribution drift detected",
                shift_result.pressure,
                self.shift.threshold()
            ));
            if let Some(lead_time) = shift_result.lead_time {
                recommendations.push(format!(
                    "Estimated {} cycles until critical drift",
                    lead_time
                ));
            }
        }

        if evidence_result.verdict == Some(Verdict::Deny) {
            recommendations.push(format!(
                "Evidence: E-value {:.2e} below deny threshold - insufficient evidence for coherence",
                evidence_result.e_value
            ));
        } else if evidence_result.verdict.is_none() {
            recommendations.push(format!(
                "Evidence: E-value {:.2e} - gathering more evidence ({} samples)",
                evidence_result.e_value, evidence_result.samples_seen
            ));
        }

        FilterResults {
            structural: structural_result,
            shift: shift_result,
            evidence: evidence_result,
            verdict,
            affected_regions,
            recommendations,
            total_time_us: start.elapsed().as_micros() as u64,
        }
    }

    /// Combine verdicts from all three filters
    fn combine_verdicts(
        &self,
        structural: &StructuralResult,
        shift: &ShiftResult,
        evidence: &EvidenceResult,
    ) -> Option<Verdict> {
        // DENY takes priority - any filter can trigger it
        if !structural.is_coherent {
            return Some(Verdict::Deny);
        }
        if !shift.is_stable {
            // Shift instability leads to Cautious/Defer, not immediate Deny
            // unless evidence also suggests Deny
            if evidence.verdict == Some(Verdict::Deny) {
                return Some(Verdict::Deny);
            }
            return Some(Verdict::Defer);
        }
        if evidence.verdict == Some(Verdict::Deny) {
            return Some(Verdict::Deny);
        }

        // PERMIT requires all filters to pass
        if structural.is_coherent && shift.is_stable {
            if evidence.verdict == Some(Verdict::Permit) {
                return Some(Verdict::Permit);
            }
            // Evidence is still accumulating - defer but optimistic
            if evidence.verdict.is_none() {
                return Some(Verdict::Defer);
            }
        }

        // Default to defer
        Some(Verdict::Defer)
    }

    /// Get mutable reference to structural filter for graph updates
    pub fn structural_mut(&mut self) -> &mut StructuralFilter {
        &mut self.structural
    }

    /// Get mutable reference to shift filter for updates
    pub fn shift_mut(&mut self) -> &mut ShiftFilter {
        &mut self.shift
    }

    /// Get mutable reference to evidence filter for updates
    pub fn evidence_mut(&mut self) -> &mut EvidenceFilter {
        &mut self.evidence
    }

    /// Get reference to structural filter
    pub fn structural(&self) -> &StructuralFilter {
        &self.structural
    }

    /// Get reference to shift filter
    pub fn shift(&self) -> &ShiftFilter {
        &self.shift
    }

    /// Get reference to evidence filter
    pub fn evidence(&self) -> &EvidenceFilter {
        &self.evidence
    }

    /// Reset all filters to initial state
    pub fn reset(&mut self) {
        self.shift.reset();
        self.evidence.reset();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_mask() {
        let mut mask = RegionMask::empty();
        assert!(!mask.any());

        mask.set(5);
        assert!(mask.is_set(5));
        assert!(!mask.is_set(4));
        assert_eq!(mask.count(), 1);

        mask.set(10);
        assert_eq!(mask.count(), 2);

        mask.clear(5);
        assert!(!mask.is_set(5));
        assert!(mask.is_set(10));
    }

    #[test]
    fn test_structural_filter_basic() {
        let mut filter = StructuralFilter::new(2.0);

        // Add a triangle graph
        filter.insert_edge(1, 2, 1.0).unwrap();
        filter.insert_edge(2, 3, 1.0).unwrap();
        filter.insert_edge(3, 1, 1.0).unwrap();

        let state = SystemState::new(3);
        let result = filter.evaluate(&state);

        // Triangle should have cut value of 2
        assert!(result.cut_value >= 2.0);
        assert!(result.is_coherent);
    }

    #[test]
    fn test_structural_filter_low_cut() {
        // Use simple cut calculation for predictable unit test behavior
        let config = StructuralConfig {
            threshold: 3.0,           // High threshold
            use_subpolynomial: false, // Disable subpolynomial for unit tests
            ..Default::default()
        };
        let mut filter = StructuralFilter::with_config(config);

        // Add a weak connection
        filter.insert_edge(1, 2, 1.0).unwrap();

        let state = SystemState::new(2);
        let result = filter.evaluate(&state);

        // Single edge has cut value of 1, below threshold of 3
        assert!(!result.is_coherent);
    }

    #[test]
    fn test_shift_filter_stable() {
        let mut filter = ShiftFilter::new(0.5, 100);

        // Add some stable observations
        for i in 0..50 {
            filter.update(0, 0.5 + (i as f64 * 0.01) % 0.1);
            filter.update(1, 0.5 + (i as f64 * 0.01) % 0.1);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        assert!(result.is_stable);
    }

    #[test]
    fn test_shift_filter_drift() {
        let mut filter = ShiftFilter::new(0.3, 100);

        // Start with stable observations
        for _ in 0..30 {
            filter.update(0, 0.5);
        }

        // Then add drifting observations
        for i in 0..30 {
            filter.update(0, 0.5 + i as f64 * 0.1);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Should detect drift
        assert!(result.pressure > 0.0);
    }

    #[test]
    fn test_evidence_accumulator() {
        let mut acc = EvidenceAccumulator::new();
        assert_eq!(acc.e_value(), 1.0);
        assert_eq!(acc.samples_seen(), 0);

        // Add evidence for coherence
        acc.update(2.0); // Twice as likely to be coherent
        assert!(acc.e_value() > 1.0);
        assert_eq!(acc.samples_seen(), 1);

        // Add more evidence
        acc.update(2.0);
        acc.update(2.0);
        assert_eq!(acc.samples_seen(), 3);
        assert!(acc.e_value() > 4.0);
    }

    #[test]
    fn test_evidence_filter_permit() {
        let mut filter = EvidenceFilter::new(10.0, 0.1);

        // Add strong evidence for coherence
        for _ in 0..10 {
            filter.update(2.0);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        assert!(result.e_value > 10.0);
        assert_eq!(result.verdict, Some(Verdict::Permit));
    }

    #[test]
    fn test_evidence_filter_deny() {
        let mut filter = EvidenceFilter::new(10.0, 0.1);

        // Add strong evidence against coherence
        for _ in 0..10 {
            filter.update(0.5);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        assert!(result.e_value < 0.1);
        assert_eq!(result.verdict, Some(Verdict::Deny));
    }

    #[test]
    fn test_filter_pipeline_permit() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 1.0,
                ..Default::default()
            },
            shift: ShiftConfig {
                threshold: 0.5,
                ..Default::default()
            },
            evidence: EvidenceConfig {
                tau_permit: 5.0,
                tau_deny: 0.2,
                ..Default::default()
            },
        };

        let mut pipeline = FilterPipeline::new(config);

        // Build a strong graph
        pipeline.structural_mut().insert_edge(1, 2, 2.0).unwrap();
        pipeline.structural_mut().insert_edge(2, 3, 2.0).unwrap();
        pipeline.structural_mut().insert_edge(3, 1, 2.0).unwrap();

        // Add stable shift observations
        for i in 0..20 {
            pipeline.shift_mut().update(0, 0.5);
        }

        // Add strong evidence
        for _ in 0..5 {
            pipeline.evidence_mut().update(2.0);
        }

        let state = SystemState::new(3);
        let result = pipeline.evaluate(&state);

        assert_eq!(result.verdict, Some(Verdict::Permit));
    }

    #[test]
    fn test_filter_pipeline_deny_structural() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 5.0,           // High threshold
                use_subpolynomial: false, // Disable for unit test predictability
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pipeline = FilterPipeline::new(config);

        // Build a weak graph (cut value = 1.0, below threshold 5.0)
        pipeline.structural_mut().insert_edge(1, 2, 1.0).unwrap();

        let state = SystemState::new(2);
        let result = pipeline.evaluate(&state);

        // Structural filter should cause deny because cut_value < threshold
        assert_eq!(result.verdict, Some(Verdict::Deny));
        assert!(!result.recommendations.is_empty());
    }

    #[test]
    fn test_system_state() {
        let mut state = SystemState::new(10);

        state.add_edge(1, 2, 1.0);
        assert!(state.adjacency.contains_key(&1));
        assert!(state.adjacency.contains_key(&2));

        state.add_syndrome(0.5);
        state.add_syndrome(0.6);
        assert_eq!(state.syndromes.len(), 2);

        state.advance_cycle();
        assert!(state.syndromes.is_empty());
        assert_eq!(state.syndrome_history.len(), 1);
        assert_eq!(state.cycle, 1);
    }

    #[test]
    fn test_filter_config_serialization() {
        let config = FilterConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: FilterConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.structural.threshold, restored.structural.threshold);
        assert_eq!(config.shift.threshold, restored.shift.threshold);
        assert_eq!(config.evidence.tau_permit, restored.evidence.tau_permit);
    }
}
