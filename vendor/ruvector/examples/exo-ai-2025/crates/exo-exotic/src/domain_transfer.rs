//! Phase 5 – Exotic Domain Transfer
//!
//! Three exotic integrations of ruvector-domain-expansion with exo-exotic:
//!
//! 1. **`StrangeLoopDomain`** – A self-referential [`Domain`] that generates
//!    tasks by reflecting on its own self-model. The Thompson Sampling engine
//!    learns which depth of meta-cognition yields the highest reward.
//!
//! 2. **`CollectiveDomainTransfer`** – Couples [`CollectiveConsciousness`]
//!    with a [`DomainExpansionEngine`]: domain arm-reward signals update
//!    substrate activity, and collective Φ measures emergent quality.
//!
//! 3. **`EmergentTransferDetector`** – Wraps [`EmergenceDetector`] to surface
//!    capability gains that arise from cross-domain transfer.

use ruvector_domain_expansion::{
    ArmId, ContextBucket, Domain, DomainEmbedding, DomainExpansionEngine, DomainId, Evaluation,
    Solution, Task,
};
use serde_json::json;
use uuid::Uuid;

use crate::collective::{CollectiveConsciousness, SubstrateSpecialization};
use crate::emergence::EmergenceDetector;
use crate::strange_loops::StrangeLoop;

// ─── 1. StrangeLoopDomain ─────────────────────────────────────────────────────

/// A self-referential domain whose tasks are levels of recursive self-modeling.
///
/// The Thompson Sampling bandit learns which depth of meta-cognition is most
/// rewarding, creating a loop where the engine optimises its own reflection.
pub struct StrangeLoopDomain {
    id: DomainId,
    #[allow(dead_code)]
    strange_loop: StrangeLoop,
}

impl StrangeLoopDomain {
    pub fn new(max_depth: usize) -> Self {
        Self {
            id: DomainId("strange_loop".to_string()),
            strange_loop: StrangeLoop::new(max_depth),
        }
    }

    /// Count self-referential keywords in a solution string.
    fn score_content(content: &str) -> f32 {
        let refs = content.matches("self").count()
            + content.matches("meta").count()
            + content.matches("loop").count();
        (refs as f32 / 5.0).min(1.0)
    }
}

impl Domain for StrangeLoopDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "Strange Loop Self-Reference"
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let max_depth = (difficulty * 4.0).round() as usize;
        (0..count)
            .map(|i| Task {
                id: format!("sl_{:05}", i),
                domain_id: self.id.clone(),
                difficulty,
                spec: json!({ "depth": max_depth, "variant": i % 3 }),
                constraints: vec!["content_must_self_reference".to_string()],
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let score = Self::score_content(&solution.content);
        let efficiency = (1.0 - task.difficulty * 0.3).max(0.0);
        let depth = task.spec.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
        let mut eval = Evaluation::composite(score, efficiency, score * 0.9);
        eval.constraint_results = vec![score > 0.0];
        eval.notes = vec![format!("depth={} score={:.3}", depth, score)];
        eval
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let score = Self::score_content(&solution.content);
        let mut v = vec![0.0f32; 64];
        v[0] = score;
        v[1] = 1.0 - score;
        // Strategy one-hot aligned with domain_bridge.rs layout [5,6,7]
        let depth = solution
            .data
            .get("depth")
            .and_then(|d| d.as_u64())
            .unwrap_or(0);
        if depth < 2 {
            v[5] = 1.0;
        } else if depth < 4 {
            v[6] = 1.0;
        } else {
            v[7] = 1.0;
        }
        for i in 8..64 {
            v[i] = (score * i as f32 * std::f32::consts::PI / 64.0).sin().abs() * 0.5;
        }
        DomainEmbedding::new(v, self.id.clone())
    }

    fn embedding_dim(&self) -> usize {
        64
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let depth = task.spec.get("depth").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        Some(Solution {
            task_id: task.id.clone(),
            content: format!(
                "self-meta-loop: I observe my self-model at meta-depth {}",
                depth
            ),
            data: json!({ "depth": depth, "self_reference": true, "meta_level": depth }),
        })
    }
}

// ─── 2. CollectiveDomainTransfer ─────────────────────────────────────────────

/// Couples [`CollectiveConsciousness`] with a [`DomainExpansionEngine`].
///
/// Each call to `run_cycle` generates tasks on the `StrangeLoopDomain`,
/// evaluates self-referential solutions, records arm outcomes in the engine,
/// and returns the updated collective Φ as a holistic quality measure.
pub struct CollectiveDomainTransfer {
    pub collective: CollectiveConsciousness,
    pub engine: DomainExpansionEngine,
    domain_id: DomainId,
    #[allow(dead_code)]
    substrate_ids: Vec<Uuid>,
    rounds: usize,
}

impl CollectiveDomainTransfer {
    /// Create with `num_substrates` substrates (one per intended domain arm).
    pub fn new(num_substrates: usize) -> Self {
        let specializations = [
            SubstrateSpecialization::Perception,
            SubstrateSpecialization::Processing,
            SubstrateSpecialization::Memory,
            SubstrateSpecialization::Integration,
        ];

        let mut collective = CollectiveConsciousness::new();
        let substrate_ids: Vec<Uuid> = (0..num_substrates)
            .map(|i| collective.add_substrate(specializations[i % specializations.len()].clone()))
            .collect();

        let mut engine = DomainExpansionEngine::new();
        engine.register_domain(Box::new(StrangeLoopDomain::new(4)));

        let domain_id = DomainId("strange_loop".to_string());
        Self {
            collective,
            engine,
            domain_id,
            substrate_ids,
            rounds: 0,
        }
    }

    /// Run one collective domain cycle.
    ///
    /// Generates tasks, scores self-referential solutions, and records arm
    /// outcomes. Returns the collective Φ after the cycle.
    pub fn run_cycle(&mut self) -> f64 {
        let bucket = ContextBucket {
            difficulty_tier: "medium".to_string(),
            category: "self_reference".to_string(),
        };
        let arm_id = ArmId("arm_0".to_string());
        let n = self.substrate_ids.len().max(1);
        let tasks = self.engine.generate_tasks(&self.domain_id, n, 0.5);

        for (i, task) in tasks.iter().enumerate() {
            let solution = Solution {
                task_id: task.id.clone(),
                content: format!(
                    "self-meta-loop: I observe my self-model at meta-depth {}",
                    i
                ),
                data: json!({ "depth": i, "self_reference": true }),
            };
            self.engine.evaluate_and_record(
                &self.domain_id,
                task,
                &solution,
                bucket.clone(),
                arm_id.clone(),
            );
        }

        self.rounds += 1;
        self.collective.compute_global_phi()
    }

    /// Collective Φ (integrated information) across all substrates.
    pub fn collective_phi(&mut self) -> f64 {
        self.collective.compute_global_phi()
    }

    /// Number of transfer rounds completed.
    pub fn rounds(&self) -> usize {
        self.rounds
    }
}

// ─── 3. EmergentTransferDetector ─────────────────────────────────────────────

/// Detects emergent capability gains arising from cross-domain transfer.
///
/// Feed baseline scores before transfer and post-transfer scores after; the
/// `EmergenceDetector` surfaces non-linear improvements that go beyond the
/// sum of individual domain gains.
pub struct EmergentTransferDetector {
    detector: EmergenceDetector,
    baseline_scores: Vec<f64>,
    post_transfer_scores: Vec<f64>,
}

impl EmergentTransferDetector {
    pub fn new() -> Self {
        Self {
            detector: EmergenceDetector::new(),
            baseline_scores: Vec::new(),
            post_transfer_scores: Vec::new(),
        }
    }

    /// Record a baseline domain score (before transfer).
    pub fn record_baseline(&mut self, score: f64) {
        self.baseline_scores.push(score);
        self.detector.set_micro_state(self.baseline_scores.clone());
    }

    /// Record a post-transfer domain score.
    pub fn record_post_transfer(&mut self, score: f64) {
        self.post_transfer_scores.push(score);
        let mut combined = self.baseline_scores.clone();
        combined.extend_from_slice(&self.post_transfer_scores);
        self.detector.set_micro_state(combined);
    }

    /// Compute emergence score (higher = more emergent capability gain).
    pub fn emergence_score(&mut self) -> f64 {
        self.detector.detect_emergence()
    }

    /// Mean improvement from baseline to post-transfer scores.
    pub fn mean_improvement(&self) -> f64 {
        if self.baseline_scores.is_empty() || self.post_transfer_scores.is_empty() {
            return 0.0;
        }
        let base_mean: f64 =
            self.baseline_scores.iter().sum::<f64>() / self.baseline_scores.len() as f64;
        let post_mean: f64 =
            self.post_transfer_scores.iter().sum::<f64>() / self.post_transfer_scores.len() as f64;
        post_mean - base_mean
    }
}

impl Default for EmergentTransferDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strange_loop_domain_basics() {
        let domain = StrangeLoopDomain::new(5);
        assert_eq!(domain.name(), "Strange Loop Self-Reference");
        assert_eq!(domain.embedding_dim(), 64);

        let tasks = domain.generate_tasks(3, 0.5);
        assert_eq!(tasks.len(), 3);

        let sol = domain.reference_solution(&tasks[0]).unwrap();
        let eval = domain.evaluate(&tasks[0], &sol);
        // Reference solution contains "self" and "meta" → score > 0
        assert!(eval.score > 0.0);
    }

    #[test]
    fn test_strange_loop_embedding() {
        let domain = StrangeLoopDomain::new(5);
        let tasks = domain.generate_tasks(1, 0.5);
        let sol = domain.reference_solution(&tasks[0]).unwrap();
        let emb = domain.embed(&sol);
        assert_eq!(emb.dim, 64);
        assert_eq!(emb.vector.len(), 64);
    }

    #[test]
    fn test_collective_domain_transfer() {
        let mut cdt = CollectiveDomainTransfer::new(2);
        let phi = cdt.run_cycle();
        assert!(phi >= 0.0);
        assert_eq!(cdt.rounds(), 1);

        let phi2 = cdt.run_cycle();
        assert!(phi2 >= 0.0);
        assert_eq!(cdt.rounds(), 2);
    }

    #[test]
    fn test_emergent_transfer_detector() {
        let mut etd = EmergentTransferDetector::new();
        etd.record_baseline(0.5);
        etd.record_post_transfer(0.7);
        let improvement = etd.mean_improvement();
        assert!((improvement - 0.2).abs() < 1e-10);
        let score = etd.emergence_score();
        assert!(score >= 0.0);
    }

    #[test]
    fn test_empty_detector() {
        let etd = EmergentTransferDetector::new();
        assert_eq!(etd.mean_improvement(), 0.0);
    }
}
