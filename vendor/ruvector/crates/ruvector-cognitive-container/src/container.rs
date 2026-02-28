use serde::{Deserialize, Serialize};

use crate::epoch::{ContainerEpochBudget, EpochController, Phase};
use crate::error::{ContainerError, Result};
use crate::memory::{MemoryConfig, MemorySlab};
use crate::witness::{
    CoherenceDecision, ContainerWitnessReceipt, VerificationResult, WitnessChain,
};

/// Top-level container configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Memory layout.
    pub memory: MemoryConfig,
    /// Per-epoch tick budgets.
    pub epoch_budget: ContainerEpochBudget,
    /// Unique identifier for this container instance.
    pub instance_id: u64,
    /// Maximum number of witness receipts retained.
    pub max_receipts: usize,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            memory: MemoryConfig::default(),
            epoch_budget: ContainerEpochBudget::default(),
            instance_id: 0,
            max_receipts: 1024,
        }
    }
}

/// A graph-structure delta to apply during the ingest phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Delta {
    EdgeAdd { u: usize, v: usize, weight: f64 },
    EdgeRemove { u: usize, v: usize },
    WeightUpdate { u: usize, v: usize, new_weight: f64 },
    Observation { node: usize, value: f64 },
}

/// Bitmask tracking which pipeline components completed during a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentMask(pub u8);

impl ComponentMask {
    pub const INGEST: Self = Self(0b0000_0001);
    pub const MINCUT: Self = Self(0b0000_0010);
    pub const SPECTRAL: Self = Self(0b0000_0100);
    pub const EVIDENCE: Self = Self(0b0000_1000);
    pub const WITNESS: Self = Self(0b0001_0000);
    pub const ALL: Self = Self(0b0001_1111);

    /// Returns `true` if all bits in `other` are set in `self`.
    pub fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Set all bits present in `other`.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// Output of a single `tick()` invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickResult {
    /// The witness receipt generated for this epoch.
    pub receipt: ContainerWitnessReceipt,
    /// True if any pipeline phase was skipped due to budget exhaustion.
    pub partial: bool,
    /// Bitmask of completed components.
    pub components_completed: u8,
    /// Wall-clock duration in microseconds.
    pub tick_time_us: u64,
}

/// Internal graph representation.
struct GraphState {
    num_vertices: usize,
    num_edges: usize,
    edges: Vec<(usize, usize, f64)>,
    min_cut_value: f64,
    canonical_hash: [u8; 32],
}

impl GraphState {
    fn new() -> Self {
        Self {
            num_vertices: 0,
            num_edges: 0,
            edges: Vec::new(),
            min_cut_value: 0.0,
            canonical_hash: [0u8; 32],
        }
    }
}

/// Internal spectral analysis state.
struct SpectralState {
    scs: f64,
    fiedler: f64,
    gap: f64,
}

impl SpectralState {
    fn new() -> Self {
        Self {
            scs: 0.0,
            fiedler: 0.0,
            gap: 0.0,
        }
    }
}

/// Internal evidence accumulation state.
struct EvidenceState {
    observations: Vec<f64>,
    accumulated_evidence: f64,
    threshold: f64,
}

impl EvidenceState {
    fn new() -> Self {
        Self {
            observations: Vec::new(),
            accumulated_evidence: 0.0,
            threshold: 1.0,
        }
    }
}

/// Serializable snapshot of the container state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub epoch: u64,
    pub config: ContainerConfig,
    pub graph_edges: Vec<(usize, usize, f64)>,
    pub spectral_scs: f64,
    pub evidence_accumulated: f64,
}

/// A sealed cognitive container that orchestrates ingest, min-cut, spectral,
/// evidence, and witness phases within a memory slab and epoch budget.
pub struct CognitiveContainer {
    config: ContainerConfig,
    #[allow(dead_code)]
    slab: MemorySlab,
    epoch: EpochController,
    witness: WitnessChain,
    graph: GraphState,
    spectral: SpectralState,
    evidence: EvidenceState,
    initialized: bool,
}

impl CognitiveContainer {
    /// Create and initialize a new container.
    pub fn new(config: ContainerConfig) -> Result<Self> {
        let slab = MemorySlab::new(config.memory.clone())?;
        let epoch = EpochController::new(config.epoch_budget.clone());
        let witness = WitnessChain::new(config.max_receipts);

        Ok(Self {
            config,
            slab,
            epoch,
            witness,
            graph: GraphState::new(),
            spectral: SpectralState::new(),
            evidence: EvidenceState::new(),
            initialized: true,
        })
    }

    /// Execute one full epoch: ingest deltas, recompute min-cut, update spectral
    /// metrics, accumulate evidence, and produce a witness receipt.
    pub fn tick(&mut self, deltas: &[Delta]) -> Result<TickResult> {
        if !self.initialized {
            return Err(ContainerError::NotInitialized);
        }

        let start = std::time::Instant::now();
        self.epoch.reset();
        let mut completed = ComponentMask(0);

        // Phase 1: Ingest
        if self.epoch.try_budget(Phase::Ingest) {
            for delta in deltas {
                self.apply_delta(delta);
            }
            self.epoch.consume(deltas.len().max(1) as u64);
            completed.insert(ComponentMask::INGEST);
        }

        // Phase 2: Min-cut
        if self.epoch.try_budget(Phase::MinCut) {
            self.recompute_mincut();
            self.epoch.consume(self.graph.num_edges.max(1) as u64);
            completed.insert(ComponentMask::MINCUT);
        }

        // Phase 3: Spectral
        if self.epoch.try_budget(Phase::Spectral) {
            self.update_spectral();
            self.epoch.consume(self.graph.num_vertices.max(1) as u64);
            completed.insert(ComponentMask::SPECTRAL);
        }

        // Phase 4: Evidence
        if self.epoch.try_budget(Phase::Evidence) {
            self.accumulate_evidence();
            self.epoch
                .consume(self.evidence.observations.len().max(1) as u64);
            completed.insert(ComponentMask::EVIDENCE);
        }

        // Phase 5: Witness
        let decision = self.make_decision();
        let input_bytes = self.serialize_deltas(deltas);
        let mincut_bytes = self.graph.min_cut_value.to_le_bytes();
        let evidence_bytes = self.evidence.accumulated_evidence.to_le_bytes();

        let receipt = self.witness.generate_receipt(
            &input_bytes,
            &mincut_bytes,
            self.spectral.scs,
            &evidence_bytes,
            decision,
        );
        completed.insert(ComponentMask::WITNESS);

        Ok(TickResult {
            receipt,
            partial: completed.0 != ComponentMask::ALL.0,
            components_completed: completed.0,
            tick_time_us: start.elapsed().as_micros() as u64,
        })
    }

    /// Reference to the container configuration.
    pub fn config(&self) -> &ContainerConfig {
        &self.config
    }

    /// Current epoch counter (next epoch to be generated).
    pub fn current_epoch(&self) -> u64 {
        self.witness.current_epoch()
    }

    /// Slice of all retained witness receipts.
    pub fn receipt_chain(&self) -> &[ContainerWitnessReceipt] {
        self.witness.receipt_chain()
    }

    /// Verify the integrity of the internal witness chain.
    pub fn verify_chain(&self) -> VerificationResult {
        WitnessChain::verify_chain(self.witness.receipt_chain())
    }

    /// Produce a serializable snapshot of the current container state.
    pub fn snapshot(&self) -> ContainerSnapshot {
        ContainerSnapshot {
            epoch: self.witness.current_epoch(),
            config: self.config.clone(),
            graph_edges: self.graph.edges.clone(),
            spectral_scs: self.spectral.scs,
            evidence_accumulated: self.evidence.accumulated_evidence,
        }
    }

    // ---- Private helpers ----

    fn apply_delta(&mut self, delta: &Delta) {
        match delta {
            Delta::EdgeAdd { u, v, weight } => {
                self.graph.edges.push((*u, *v, *weight));
                self.graph.num_edges += 1;
                let max_node = (*u).max(*v) + 1;
                if max_node > self.graph.num_vertices {
                    self.graph.num_vertices = max_node;
                }
            }
            Delta::EdgeRemove { u, v } => {
                self.graph.edges.retain(|(a, b, _)| !(*a == *u && *b == *v));
                self.graph.num_edges = self.graph.edges.len();
            }
            Delta::WeightUpdate { u, v, new_weight } => {
                for edge in &mut self.graph.edges {
                    if edge.0 == *u && edge.1 == *v {
                        edge.2 = *new_weight;
                    }
                }
            }
            Delta::Observation { value, .. } => {
                self.evidence.observations.push(*value);
            }
        }
    }

    /// Simplified Stoer-Wagner-style min-cut: find the minimum total weight
    /// among all vertex partitions. For small graphs this uses the minimum
    /// weighted vertex degree as a fast approximation.
    fn recompute_mincut(&mut self) {
        if self.graph.edges.is_empty() {
            self.graph.min_cut_value = 0.0;
            self.graph.canonical_hash = [0u8; 32];
            return;
        }

        // Approximate min-cut via minimum weighted degree.
        let n = self.graph.num_vertices;
        let mut degree = vec![0.0f64; n];
        for &(u, v, w) in &self.graph.edges {
            if u < n {
                degree[u] += w;
            }
            if v < n {
                degree[v] += w;
            }
        }

        self.graph.min_cut_value = degree
            .iter()
            .copied()
            .filter(|&d| d > 0.0)
            .fold(f64::MAX, f64::min);
        if self.graph.min_cut_value == f64::MAX {
            self.graph.min_cut_value = 0.0;
        }

        // Canonical hash: hash sorted edges.
        let mut sorted = self.graph.edges.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let bytes: Vec<u8> = sorted
            .iter()
            .flat_map(|(u, v, w)| {
                let mut b = Vec::with_capacity(24);
                b.extend_from_slice(&u.to_le_bytes());
                b.extend_from_slice(&v.to_le_bytes());
                b.extend_from_slice(&w.to_le_bytes());
                b
            })
            .collect();
        self.graph.canonical_hash = crate::witness::deterministic_hash_public(&bytes);
    }

    /// Simplified spectral metrics: SCS is the ratio of min-cut to total weight.
    fn update_spectral(&mut self) {
        let total_weight: f64 = self.graph.edges.iter().map(|e| e.2).sum();
        if total_weight > 0.0 {
            self.spectral.scs = self.graph.min_cut_value / total_weight;
            self.spectral.fiedler = self.spectral.scs;
            self.spectral.gap = 1.0 - self.spectral.scs;
        } else {
            self.spectral.scs = 0.0;
            self.spectral.fiedler = 0.0;
            self.spectral.gap = 0.0;
        }
    }

    /// Simple sequential probability ratio test (SPRT) style accumulation.
    fn accumulate_evidence(&mut self) {
        if self.evidence.observations.is_empty() {
            return;
        }
        let mean: f64 = self.evidence.observations.iter().sum::<f64>()
            / self.evidence.observations.len() as f64;
        self.evidence.accumulated_evidence += mean.abs();
    }

    /// Decision logic based on spectral coherence and accumulated evidence.
    fn make_decision(&self) -> CoherenceDecision {
        if self.graph.edges.is_empty() {
            return CoherenceDecision::Inconclusive;
        }
        if self.spectral.scs >= 0.5 && self.evidence.accumulated_evidence < self.evidence.threshold
        {
            return CoherenceDecision::Pass;
        }
        if self.spectral.scs < 0.2 {
            let severity = ((1.0 - self.spectral.scs) * 10.0).min(255.0) as u8;
            return CoherenceDecision::Fail { severity };
        }
        CoherenceDecision::Inconclusive
    }

    fn serialize_deltas(&self, deltas: &[Delta]) -> Vec<u8> {
        serde_json::to_vec(deltas).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_container() -> CognitiveContainer {
        CognitiveContainer::new(ContainerConfig::default()).unwrap()
    }

    #[test]
    fn test_container_lifecycle() {
        let mut container = default_container();
        assert_eq!(container.current_epoch(), 0);

        let result = container.tick(&[]).unwrap();
        assert_eq!(result.receipt.epoch, 0);
        assert_eq!(container.current_epoch(), 1);

        match container.verify_chain() {
            VerificationResult::Valid { chain_length, .. } => {
                assert_eq!(chain_length, 1);
            }
            other => panic!("Expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn test_container_tick_with_deltas() {
        let mut container = default_container();

        let deltas = vec![
            Delta::EdgeAdd {
                u: 0,
                v: 1,
                weight: 1.0,
            },
            Delta::EdgeAdd {
                u: 1,
                v: 2,
                weight: 2.0,
            },
            Delta::EdgeAdd {
                u: 2,
                v: 0,
                weight: 1.5,
            },
            Delta::Observation {
                node: 0,
                value: 0.8,
            },
        ];

        let result = container.tick(&deltas).unwrap();
        assert!(!result.partial);
        assert_eq!(result.components_completed, ComponentMask::ALL.0);

        // Graph should reflect the edges.
        let snap = container.snapshot();
        assert_eq!(snap.graph_edges.len(), 3);
        assert!(snap.spectral_scs > 0.0);
    }

    #[test]
    fn test_container_snapshot_restore() {
        let mut container = default_container();
        container
            .tick(&[Delta::EdgeAdd {
                u: 0,
                v: 1,
                weight: 3.0,
            }])
            .unwrap();

        let snap = container.snapshot();
        let json = serde_json::to_string(&snap).expect("serialize snapshot");
        let restored: ContainerSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(restored.epoch, snap.epoch);
        assert_eq!(restored.graph_edges.len(), snap.graph_edges.len());
        assert!((restored.spectral_scs - snap.spectral_scs).abs() < f64::EPSILON);
    }

    #[test]
    fn test_container_decision_logic() {
        let mut container = default_container();

        // Empty graph => Inconclusive
        let r = container.tick(&[]).unwrap();
        assert_eq!(r.receipt.decision, CoherenceDecision::Inconclusive);

        // Single edge: min-cut/total = 1.0 (high scs), no evidence => Pass
        let r = container
            .tick(&[Delta::EdgeAdd {
                u: 0,
                v: 1,
                weight: 5.0,
            }])
            .unwrap();
        assert_eq!(r.receipt.decision, CoherenceDecision::Pass);
    }

    #[test]
    fn test_container_multiple_epochs() {
        let mut container = default_container();
        for i in 0..10 {
            container
                .tick(&[Delta::EdgeAdd {
                    u: i,
                    v: i + 1,
                    weight: 1.0,
                }])
                .unwrap();
        }
        assert_eq!(container.current_epoch(), 10);

        match container.verify_chain() {
            VerificationResult::Valid {
                chain_length,
                first_epoch,
                last_epoch,
            } => {
                assert_eq!(chain_length, 10);
                assert_eq!(first_epoch, 0);
                assert_eq!(last_epoch, 9);
            }
            other => panic!("Expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn test_container_edge_remove() {
        let mut container = default_container();
        container
            .tick(&[
                Delta::EdgeAdd {
                    u: 0,
                    v: 1,
                    weight: 1.0,
                },
                Delta::EdgeAdd {
                    u: 1,
                    v: 2,
                    weight: 2.0,
                },
            ])
            .unwrap();

        container.tick(&[Delta::EdgeRemove { u: 0, v: 1 }]).unwrap();

        let snap = container.snapshot();
        assert_eq!(snap.graph_edges.len(), 1);
        assert_eq!(snap.graph_edges[0], (1, 2, 2.0));
    }

    #[test]
    fn test_container_weight_update() {
        let mut container = default_container();
        container
            .tick(&[Delta::EdgeAdd {
                u: 0,
                v: 1,
                weight: 1.0,
            }])
            .unwrap();

        container
            .tick(&[Delta::WeightUpdate {
                u: 0,
                v: 1,
                new_weight: 5.0,
            }])
            .unwrap();

        let snap = container.snapshot();
        assert_eq!(snap.graph_edges[0].2, 5.0);
    }

    #[test]
    fn test_component_mask() {
        let mut mask = ComponentMask(0);
        assert!(!mask.contains(ComponentMask::INGEST));

        mask.insert(ComponentMask::INGEST);
        assert!(mask.contains(ComponentMask::INGEST));
        assert!(!mask.contains(ComponentMask::MINCUT));

        mask.insert(ComponentMask::MINCUT);
        assert!(mask.contains(ComponentMask::INGEST));
        assert!(mask.contains(ComponentMask::MINCUT));

        assert!(!mask.contains(ComponentMask::ALL));
    }
}
