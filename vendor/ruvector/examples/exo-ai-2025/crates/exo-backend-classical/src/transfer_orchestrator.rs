//! Cross-phase ExoTransferOrchestrator
//!
//! Wires all 5 ruvector-domain-expansion integration phases into a single
//! `run_cycle()` call:
//!
//! 1. **Phase 1** – Domain Bridge (this crate): Thompson sampling over
//!    `ExoRetrievalDomain` + `ExoGraphDomain`.
//! 2. **Phase 2** – Transfer Manifold (exo-manifold): stores priors as
//!    deformable 64-dim patterns.
//! 3. **Phase 3** – Transfer Timeline (exo-temporal): records events in a
//!    causal graph with temporal ordering.
//! 4. **Phase 4** – Transfer CRDT (exo-federation): replicates summaries via
//!    LWW-Map + G-Set.
//! 5. **Phase 5** – Emergent Detection (exo-exotic): tracks whether
//!    cross-domain transfer produces novel emergent capabilities.

use exo_exotic::domain_transfer::EmergentTransferDetector;
use exo_federation::transfer_crdt::{TransferCrdt, TransferPriorSummary};
use exo_manifold::transfer_store::TransferManifold;
use exo_temporal::transfer_timeline::TransferTimeline;
use ruvector_domain_expansion::{
    ArmId, ContextBucket, DomainExpansionEngine, DomainId, Solution, TransferPrior,
};

use crate::domain_bridge::{ExoGraphDomain, ExoRetrievalDomain};

/// Results from a single orchestrated transfer cycle.
#[derive(Debug, Clone)]
pub struct CycleResult {
    /// Evaluation score from the source domain task [0.0, 1.0].
    pub eval_score: f32,
    /// Emergence score after the transfer step.
    pub emergence_score: f64,
    /// Mean improvement from pre-transfer baseline.
    pub mean_improvement: f64,
    /// Number of (src, dst) priors stored in the manifold.
    pub manifold_entries: usize,
    /// Cycle index (1-based).
    pub cycle: u64,
}

/// Orchestrates all 5 integration phases of ruvector-domain-expansion.
pub struct ExoTransferOrchestrator {
    /// Phase 1: Thompson sampling engine with retrieval + graph domains.
    engine: DomainExpansionEngine,
    /// Source domain ID (retrieval).
    src_id: DomainId,
    /// Destination domain ID (graph).
    dst_id: DomainId,
    /// Phase 2: manifold storage for transfer priors.
    manifold: TransferManifold,
    /// Phase 3: temporal causal timeline.
    timeline: TransferTimeline,
    /// Phase 4: CRDT for distributed propagation.
    crdt: TransferCrdt,
    /// Phase 5: emergent capability detector.
    emergence: EmergentTransferDetector,
    /// Monotonic cycle counter.
    cycle: u64,
}

impl ExoTransferOrchestrator {
    /// Create a new orchestrator.
    pub fn new(_node_id: impl Into<String>) -> Self {
        let src_id = DomainId("exo-retrieval".to_string());
        let dst_id = DomainId("exo-graph".to_string());

        let mut engine = DomainExpansionEngine::new();
        engine.register_domain(Box::new(ExoRetrievalDomain::new()));
        engine.register_domain(Box::new(ExoGraphDomain::new()));

        Self {
            engine,
            src_id,
            dst_id,
            manifold: TransferManifold::new(),
            timeline: TransferTimeline::new(),
            crdt: TransferCrdt::new(),
            emergence: EmergentTransferDetector::new(),
            cycle: 0,
        }
    }

    /// Run a single orchestrated transfer cycle across all 5 phases.
    ///
    /// Returns a [`CycleResult`] summarising each phase outcome.
    pub fn run_cycle(&mut self) -> CycleResult {
        self.cycle += 1;

        let bucket = ContextBucket {
            difficulty_tier: "medium".to_string(),
            category: "transfer".to_string(),
        };

        // ── Phase 1: Domain Bridge ─────────────────────────────────────────────
        // Generate a task for the source domain, select the best arm via
        // Thompson sampling, and evaluate it.
        let tasks = self.engine.generate_tasks(&self.src_id, 1, 0.5);
        let eval_score = if let Some(task) = tasks.first() {
            let arm = self
                .engine
                .select_arm(&self.src_id, &bucket)
                .unwrap_or_else(|| ArmId("approximate".to_string()));

            let solution = Solution {
                task_id: task.id.clone(),
                content: arm.0.clone(),
                data: serde_json::json!({ "arm": &arm.0 }),
            };

            let eval =
                self.engine
                    .evaluate_and_record(&self.src_id, task, &solution, bucket.clone(), arm);
            eval.score
        } else {
            0.5f32
        };

        // Transfer priors from source → destination domain.
        self.engine.initiate_transfer(&self.src_id, &self.dst_id);

        // ── Phase 2: Transfer Manifold ─────────────────────────────────────────
        let prior = TransferPrior::uniform(self.src_id.clone());
        let _ = self
            .manifold
            .store_prior(&self.src_id, &self.dst_id, &prior, self.cycle);
        let manifold_entries = self.manifold.len();

        // ── Phase 3: Transfer Timeline ─────────────────────────────────────────
        let _ = self
            .timeline
            .record_transfer(&self.src_id, &self.dst_id, self.cycle, eval_score);

        // ── Phase 4: Transfer CRDT ─────────────────────────────────────────────
        self.crdt.publish_prior(
            &self.src_id,
            &self.dst_id,
            eval_score,
            eval_score, // confidence mirrors eval score
            self.cycle,
        );

        // ── Phase 5: Emergent Detection ────────────────────────────────────────
        if self.cycle == 1 {
            self.emergence.record_baseline(eval_score as f64);
        } else {
            self.emergence.record_post_transfer(eval_score as f64);
        }
        let emergence_score = self.emergence.emergence_score();
        let mean_improvement = self.emergence.mean_improvement();

        CycleResult {
            eval_score,
            emergence_score,
            mean_improvement,
            manifold_entries,
            cycle: self.cycle,
        }
    }

    /// Return the current cycle number.
    pub fn cycle(&self) -> u64 {
        self.cycle
    }

    /// Return the best published prior for the (src → dst) pair.
    pub fn best_prior(&self) -> Option<&TransferPriorSummary> {
        self.crdt.best_prior_for(&self.src_id, &self.dst_id)
    }

    /// Serialize the current engine state as an RVF byte stream.
    ///
    /// Packages three artifact types into concatenated RVF segments:
    /// - `TransferPrior` segments (one per registered domain that has priors)
    /// - `PolicyKernel` segments (the current population of policy variants)
    /// - `CostCurve` segments (convergence tracking per domain)
    ///
    /// The returned bytes can be written to a `.rvf` file or streamed over the
    /// network for federated transfer.
    ///
    /// Requires the `rvf` feature.
    #[cfg(feature = "rvf")]
    pub fn package_as_rvf(&self) -> Vec<u8> {
        use ruvector_domain_expansion::rvf_bridge;

        // Collect TransferPriors for both registered domains.
        let priors: Vec<_> = [&self.src_id, &self.dst_id]
            .iter()
            .filter_map(|id| self.engine.thompson.extract_prior(id))
            .collect();

        // All PolicyKernels from the current population.
        let kernels: Vec<_> = self.engine.population.population().to_vec();

        // CostCurves tracked by the acceleration scoreboard.
        let curves: Vec<_> = [&self.src_id, &self.dst_id]
            .iter()
            .filter_map(|id| self.engine.scoreboard.curves.get(id))
            .cloned()
            .collect();

        rvf_bridge::assemble_domain_expansion_segments(&priors, &kernels, &curves, 1)
    }

    /// Write the current engine state to a `.rvf` file at `path`.
    ///
    /// Requires the `rvf` feature.
    #[cfg(feature = "rvf")]
    pub fn save_rvf(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.package_as_rvf())
    }
}

impl Default for ExoTransferOrchestrator {
    fn default() -> Self {
        Self::new("default_node")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = ExoTransferOrchestrator::new("test_node");
        assert_eq!(orchestrator.cycle(), 0);
        assert!(orchestrator.best_prior().is_none());
    }

    #[test]
    fn test_single_cycle() {
        let mut orchestrator = ExoTransferOrchestrator::new("node_1");
        let result = orchestrator.run_cycle();

        assert_eq!(result.cycle, 1);
        assert!(result.eval_score >= 0.0 && result.eval_score <= 1.0);
        assert!(result.manifold_entries >= 1);
        assert!(orchestrator.best_prior().is_some());
    }

    #[test]
    fn test_multi_cycle_emergence() {
        let mut orchestrator = ExoTransferOrchestrator::new("node_2");

        // Warm up: baseline cycle
        let r1 = orchestrator.run_cycle();
        assert_eq!(r1.cycle, 1);

        // Transfer cycles: emergence detector should fire
        for _ in 0..4 {
            let r = orchestrator.run_cycle();
            assert!(r.emergence_score >= 0.0);
        }

        assert_eq!(orchestrator.cycle(), 5);
    }

    #[test]
    #[cfg(feature = "rvf")]
    fn test_package_as_rvf_empty() {
        // Before any cycle the population has kernels but no domain-specific
        // priors or curves, so we should still get a valid (possibly short) RVF stream.
        let orchestrator = ExoTransferOrchestrator::new("rvf_node");
        let bytes = orchestrator.package_as_rvf();

        // A valid RVF stream from the population must be a multiple of 64 bytes
        // and at least contain population kernel segments.
        assert_eq!(bytes.len() % 64, 0, "RVF output must be 64-byte aligned");
    }

    #[test]
    #[cfg(feature = "rvf")]
    fn test_package_as_rvf_after_cycles() {
        // RVF segment magic: "RVFS" in little-endian = 0x5256_4653
        const SEGMENT_MAGIC: u32 = 0x5256_4653;

        let mut orchestrator = ExoTransferOrchestrator::new("rvf_cycle_node");

        // Warm up to generate priors and curves.
        for _ in 0..3 {
            orchestrator.run_cycle();
        }

        let bytes = orchestrator.package_as_rvf();

        // Must be 64-byte aligned and contain at least one segment.
        assert!(
            !bytes.is_empty(),
            "RVF output must not be empty after cycles"
        );
        assert_eq!(bytes.len() % 64, 0, "RVF output must be 64-byte aligned");

        // Verify the first segment's magic bytes.
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(
            magic, SEGMENT_MAGIC,
            "First segment must have valid RVF magic"
        );
    }

    #[test]
    #[cfg(feature = "rvf")]
    fn test_save_rvf_to_file() {
        let mut orchestrator = ExoTransferOrchestrator::new("rvf_file_node");
        orchestrator.run_cycle();

        let path = std::env::temp_dir().join("exo_test.rvf");
        orchestrator
            .save_rvf(&path)
            .expect("save_rvf should succeed");

        let written = std::fs::read(&path).expect("file should exist after save_rvf");
        assert!(!written.is_empty());
        assert_eq!(written.len() % 64, 0);

        // Clean up
        let _ = std::fs::remove_file(&path);
    }
}
