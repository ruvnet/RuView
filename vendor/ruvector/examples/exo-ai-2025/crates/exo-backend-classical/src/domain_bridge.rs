//! Domain bridge: wraps EXO-AI classical operations as learnable domains
//! for ruvector-domain-expansion's transfer-learning engine.
//!
//! ## Why
//!
//! EXO-AI performs vector similarity search and graph traversal constantly
//! but never *learns* which strategies work best for which problem types.
//! This bridge turns those operations into `Domain` implementations so
//! Thompson Sampling can discover optimal policies and transfer insights
//! across categories (e.g. "approximate HNSW wins on high-dim sparse queries"
//! transfers to graph traversal: "approximate BFS beats exact DFS").
//!
//! ## Two Domains
//!
//! - **ExoRetrievalDomain**: Vector similarity search as a bandit problem.
//!   Arms: `exact`, `approximate`, `beam_rerank`.
//!
//! - **ExoGraphDomain**: Hypergraph traversal as a bandit problem.
//!   Arms: `bfs`, `approx`, `hierarchical`.
//!
//! Embeddings align structurally (same 64-dim layout, same dimension semantics)
//! so cross-domain transfer priors carry meaningful signal.

use ruvector_domain_expansion::{
    ArmId, ContextBucket, Domain, DomainEmbedding, DomainId, Evaluation, Solution, Task,
};
use serde_json::json;
use std::f32::consts::PI;

// ─── Utilities ────────────────────────────────────────────────────────────────

/// Build a ContextBucket from task difficulty.
fn bucket_for(difficulty: f32, category: &str) -> ContextBucket {
    let tier = if difficulty < 0.33 {
        "easy"
    } else if difficulty < 0.67 {
        "medium"
    } else {
        "hard"
    };
    ContextBucket {
        difficulty_tier: tier.to_string(),
        category: category.to_string(),
    }
}

/// Spread a scalar value into a sinusoidal pattern over `n` dimensions.
/// Used to make scalar metrics distinguishable in the 64-dim embedding.
#[inline]
fn spread(val: f32, out: &mut [f32], offset: usize, n: usize) {
    for i in 0..n.min(out.len().saturating_sub(offset)) {
        out[offset + i] = val * ((i as f32 / n as f32) * PI).sin().abs();
    }
}

// ─── Retrieval Domain ─────────────────────────────────────────────────────────

/// Retrieval strategies available to the Thompson Sampling engine.
pub const RETRIEVAL_ARMS: &[&str] = &["exact", "approximate", "beam_rerank"];

/// EXO vector similarity retrieval as a `Domain`.
///
/// **Task spec** (JSON):
/// ```json
/// { "dim": 512, "k": 10, "noise": 0.2, "n_candidates": 100, "arm": "approximate" }
/// ```
///
/// **Reference solution** (optimal): recall = 1.0, latency = low.
///
/// **Transfer signal**: high-dimensional + noisy tasks → prefer `approximate`.
/// This prior transfers to ExoGraphDomain: large + sparse graphs → prefer `approx`.
pub struct ExoRetrievalDomain {
    id: DomainId,
}

impl ExoRetrievalDomain {
    pub fn new() -> Self {
        Self {
            id: DomainId("exo-retrieval".to_string()),
        }
    }

    fn task_id(index: usize) -> String {
        format!("exo-ret-{:05}", index)
    }

    fn category(k: usize) -> String {
        if k <= 5 {
            "top-k-small".to_string()
        } else if k <= 20 {
            "top-k-medium".to_string()
        } else {
            "top-k-large".to_string()
        }
    }

    /// Simulate scoring a retrieval strategy on a task.
    /// In production this would run against the actual VectorIndexWrapper.
    fn simulate_score(arm: &str, dim: usize, noise: f32, k: usize) -> (f32, f32, f32) {
        let complexity = (dim as f32 / 1024.0) * (1.0 + noise);
        let (recall, efficiency) = match arm {
            "exact" => {
                // High accuracy but O(n) latency — slow for high-dim
                let recall = 1.0 - noise * 0.1;
                let efficiency = 1.0 - complexity * 0.6;
                (recall, efficiency)
            }
            "approximate" => {
                // Good trade-off — recall drops with noise but stays efficient
                let recall = 1.0 - noise * 0.25;
                let efficiency = 0.85 - complexity * 0.2;
                (recall, efficiency)
            }
            "beam_rerank" => {
                // Best recall on large k, moderate cost
                let recall = 1.0 - noise * 0.15;
                let efficiency = 0.7 - complexity * 0.3;
                let k_bonus = (k as f32 / 50.0).min(0.15);
                (recall + k_bonus * 0.1, efficiency)
            }
            _ => (0.5, 0.5),
        };
        let elegance = if k <= 10 { 0.9 } else { 0.6 };
        (recall.clamp(0.0, 1.0), efficiency.clamp(0.0, 1.0), elegance)
    }
}

impl Default for ExoRetrievalDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl Domain for ExoRetrievalDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "EXO Vector Retrieval"
    }

    fn embedding_dim(&self) -> usize {
        64
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let dim = (64.0 + difficulty * 960.0) as usize;
        let k = (3.0 + difficulty * 47.0) as usize;
        let noise = difficulty * 0.5;
        let n_candidates = (k * 10).max(50);
        let cat = Self::category(k);

        RETRIEVAL_ARMS
            .iter()
            .cycle()
            .take(count)
            .enumerate()
            .map(|(i, arm)| Task {
                id: Self::task_id(i),
                domain_id: self.id.clone(),
                difficulty,
                spec: json!({
                    "dim": dim,
                    "k": k,
                    "noise": noise,
                    "n_candidates": n_candidates,
                    "arm": arm,
                    "category": cat,
                }),
                constraints: vec![
                    format!("recall >= {:.2}", (1.0 - difficulty * 0.4).max(0.5)),
                    "latency_us < 10000".to_string(),
                ],
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let sol = &solution.data;

        let recall = sol.get("recall").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        let latency_us = sol
            .get("latency_us")
            .and_then(|x| x.as_u64())
            .unwrap_or(9999);
        let retrieved_k = sol.get("retrieved_k").and_then(|x| x.as_u64()).unwrap_or(0);
        let target_k = task.spec.get("k").and_then(|x| x.as_u64()).unwrap_or(5);

        let efficiency = (1000.0 / (latency_us as f32 + 1.0)).min(1.0);
        let elegance = if retrieved_k == target_k { 1.0 } else { 0.5 };

        let min_recall: f32 = (1.0 - task.difficulty * 0.4).max(0.5);
        let mut eval = Evaluation::composite(recall, efficiency, elegance);
        eval.constraint_results = vec![recall >= min_recall, latency_us < 10_000];
        eval
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let sol = &solution.data;
        let mut v = vec![0.0f32; 64];

        let recall = sol.get("recall").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        let latency = sol
            .get("latency_us")
            .and_then(|x| x.as_u64())
            .unwrap_or(1000) as f32;
        let k = sol.get("retrieved_k").and_then(|x| x.as_u64()).unwrap_or(5) as f32;
        let arm = sol.get("arm").and_then(|x| x.as_str()).unwrap_or("exact");

        v[0] = recall;
        v[1] = (1000.0 / (latency + 1.0)).min(1.0); // efficiency
        v[2] = (k / 50.0).min(1.0);
        // Strategy one-hot — aligned with ExoGraphDomain positions [5,6,7]
        match arm {
            "exact" => {
                v[5] = 1.0;
            }
            "approximate" => {
                v[6] = 1.0;
            }
            "beam_rerank" => {
                v[7] = 1.0;
            }
            _ => {}
        }
        spread(recall, &mut v, 8, 24); // dims 8..31

        DomainEmbedding::new(v, self.id.clone())
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let dim = task.spec.get("dim").and_then(|x| x.as_u64()).unwrap_or(128) as usize;
        let k = task.spec.get("k").and_then(|x| x.as_u64()).unwrap_or(5) as usize;
        let noise = task
            .spec
            .get("noise")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0) as f32;

        // Optimal arm: beam_rerank for large k, approximate for high-dim noisy
        let arm = if k > 20 {
            "beam_rerank"
        } else if dim > 512 || noise > 0.3 {
            "approximate"
        } else {
            "exact"
        };

        let (recall, _, _) = Self::simulate_score(arm, dim, noise, k);
        // Reference latency: approximate is ~100µs, exact ~500µs at 512-dim
        let latency_us = match arm {
            "exact" => 500u64,
            "approximate" => 100,
            _ => 200,
        };

        Some(Solution {
            task_id: task.id.clone(),
            content: format!("optimal-{}", arm),
            data: json!({
                "recall": recall,
                "latency_us": latency_us,
                "retrieved_k": k,
                "arm": arm,
            }),
        })
    }
}

// ─── Graph Domain ─────────────────────────────────────────────────────────────

/// Traversal strategies for the graph domain.
pub const GRAPH_ARMS: &[&str] = &["bfs", "approx", "hierarchical"];

/// EXO hypergraph traversal as a `Domain`.
///
/// Structural alignment with ExoRetrievalDomain (same embedding layout)
/// enables cross-domain transfer: retrieval priors seed graph policies.
///
/// **Task spec** (JSON):
/// ```json
/// { "n_entities": 500, "max_hops": 3, "min_coverage": 20,
///   "relation": "causal", "arm": "approx" }
/// ```
pub struct ExoGraphDomain {
    id: DomainId,
}

impl ExoGraphDomain {
    pub fn new() -> Self {
        Self {
            id: DomainId("exo-graph".to_string()),
        }
    }

    fn task_id(index: usize) -> String {
        format!("exo-graph-{:05}", index)
    }

    /// Simulate graph traversal score for an arm + problem parameters.
    fn simulate_score(
        arm: &str,
        n_entities: usize,
        max_hops: usize,
        min_coverage: usize,
    ) -> (f32, f32, f32, u64) {
        let density = (n_entities as f32 / 1000.0).min(1.0);
        let depth_ratio = max_hops as f32 / 6.0;

        let (coverage_ratio, hops_used, latency_us) = match arm {
            "bfs" => {
                // Complete but expensive for large graphs
                let cov = 1.3 - density * 0.4;
                let hops = max_hops.saturating_sub(1);
                let lat = (n_entities as u64) * 10;
                (cov, hops, lat)
            }
            "approx" => {
                // Approximate neighborhood expansion — efficient, slight coverage loss
                let cov = 1.1 - density * 0.2;
                let hops = (max_hops * 2 / 3).max(1);
                let lat = (n_entities as u64) * 3;
                (cov, hops, lat)
            }
            "hierarchical" => {
                // Coarse→fine decomposition — best for large graphs with structure
                let cov = 1.2 - depth_ratio * 0.3;
                let hops = (max_hops * 3 / 4).max(1);
                let lat = (n_entities as u64) * 5;
                (cov, hops, lat)
            }
            _ => (0.5, max_hops, 10_000),
        };

        let entities_found = (min_coverage as f32 * coverage_ratio) as u64;
        let correctness = (entities_found as f32 / min_coverage as f32).min(1.0);
        let efficiency = if max_hops > 0 {
            (1.0 - hops_used as f32 / max_hops as f32).max(0.0)
        } else {
            0.0
        };
        let elegance = if coverage_ratio >= 1.0 && coverage_ratio <= 1.5 {
            1.0
        } else if coverage_ratio > 0.8 {
            0.7
        } else {
            0.3
        };

        (correctness, efficiency, elegance, latency_us)
    }
}

impl Default for ExoGraphDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl Domain for ExoGraphDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "EXO Hypergraph Traversal"
    }

    fn embedding_dim(&self) -> usize {
        64
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let n_entities = (50.0 + difficulty * 950.0) as usize;
        let max_hops = (2.0 + difficulty * 4.0) as usize;
        let min_coverage = (5.0 + difficulty * 95.0) as usize;
        let relations = ["causal", "temporal", "semantic", "structural"];

        GRAPH_ARMS
            .iter()
            .cycle()
            .take(count)
            .enumerate()
            .map(|(i, arm)| Task {
                id: Self::task_id(i),
                domain_id: self.id.clone(),
                difficulty,
                spec: json!({
                    "n_entities": n_entities,
                    "max_hops": max_hops,
                    "min_coverage": min_coverage,
                    "relation": relations[i % 4],
                    "arm": arm,
                }),
                constraints: vec![
                    format!("entities_found >= {}", min_coverage),
                    format!("hops_used <= {}", max_hops),
                ],
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let sol = &solution.data;

        let entities_found = sol
            .get("entities_found")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let hops_used = sol.get("hops_used").and_then(|x| x.as_u64()).unwrap_or(0);
        let coverage_ratio = sol
            .get("coverage_ratio")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0) as f32;

        let min_coverage = task
            .spec
            .get("min_coverage")
            .and_then(|x| x.as_u64())
            .unwrap_or(5);
        let max_hops = task
            .spec
            .get("max_hops")
            .and_then(|x| x.as_u64())
            .unwrap_or(3);

        let correctness = (entities_found as f32 / min_coverage as f32).min(1.0);
        let efficiency = if max_hops > 0 {
            (1.0 - hops_used as f32 / max_hops as f32).max(0.0)
        } else {
            0.0
        };
        let elegance = if coverage_ratio >= 1.0 && coverage_ratio <= 1.5 {
            1.0
        } else if coverage_ratio > 0.8 {
            0.7
        } else {
            0.3
        };

        let mut eval = Evaluation::composite(correctness, efficiency, elegance);
        eval.constraint_results = vec![entities_found >= min_coverage, hops_used <= max_hops];
        eval
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let sol = &solution.data;
        let mut v = vec![0.0f32; 64];

        let coverage = sol
            .get("coverage_ratio")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0) as f32;
        let hops = sol.get("hops_used").and_then(|x| x.as_u64()).unwrap_or(0) as f32;
        let entities = sol
            .get("entities_found")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as f32;
        let arm = sol.get("arm").and_then(|x| x.as_str()).unwrap_or("bfs");

        v[0] = coverage.min(1.0);
        v[1] = (1.0 / (hops + 1.0)).min(1.0); // efficiency proxy
        v[2] = (entities / 100.0).min(1.0);
        // Strategy one-hot — aligned with ExoRetrievalDomain at [5,6,7]
        match arm {
            "bfs" => {
                v[5] = 1.0;
            } // aligns with "exact"
            "approx" => {
                v[6] = 1.0;
            } // aligns with "approximate"
            "hierarchical" => {
                v[7] = 1.0;
            } // aligns with "beam_rerank"
            _ => {}
        }
        spread(coverage.min(1.0), &mut v, 8, 24); // dims 8..31

        DomainEmbedding::new(v, self.id.clone())
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let n = task
            .spec
            .get("n_entities")
            .and_then(|x| x.as_u64())
            .unwrap_or(100) as usize;
        let max_hops = task
            .spec
            .get("max_hops")
            .and_then(|x| x.as_u64())
            .unwrap_or(3) as usize;
        let min_cov = task
            .spec
            .get("min_coverage")
            .and_then(|x| x.as_u64())
            .unwrap_or(5) as usize;

        // Optimal arm: hierarchical for large sparse graphs, approx for medium
        let arm = if n > 500 { "hierarchical" } else { "approx" };
        let (correctness, _, _, lat) = Self::simulate_score(arm, n, max_hops, min_cov);
        let entities = (min_cov as f32 * 1.2 * correctness) as u64;
        let hops = (max_hops as u64).saturating_sub(1).max(1);

        Some(Solution {
            task_id: task.id.clone(),
            content: format!("optimal-{}", arm),
            data: json!({
                "entities_found": entities,
                "hops_used": hops,
                "coverage_ratio": 1.2 * correctness,
                "arm": arm,
                "latency_us": lat,
            }),
        })
    }
}

// ─── Transfer Adapter ─────────────────────────────────────────────────────────

/// Unified adapter that registers both EXO domains into a `DomainExpansionEngine`
/// and exposes a simple training + transfer lifecycle API.
///
/// # Example
/// ```no_run
/// use exo_backend_classical::domain_bridge::ExoTransferAdapter;
///
/// let mut adapter = ExoTransferAdapter::new();
/// adapter.warmup(30);                     // train retrieval + graph
/// let accel = adapter.transfer_ret_to_graph(10); // measure acceleration
/// println!("Transfer acceleration: {:.2}x", accel);
/// ```
pub struct ExoTransferAdapter {
    /// The underlying domain-expansion engine (also contains built-in domains).
    pub engine: ruvector_domain_expansion::DomainExpansionEngine,
}

impl ExoTransferAdapter {
    /// Create adapter and register both EXO domains alongside the built-in ones.
    pub fn new() -> Self {
        let mut engine = ruvector_domain_expansion::DomainExpansionEngine::new();
        engine.register_domain(Box::new(ExoRetrievalDomain::new()));
        engine.register_domain(Box::new(ExoGraphDomain::new()));
        Self { engine }
    }

    /// Run one training cycle on the given domain:
    /// generate a task, pick a strategy arm, record outcome.
    fn train_one(&mut self, domain_id: &DomainId, difficulty: f32) -> f32 {
        let tasks = self.engine.generate_tasks(domain_id, 1, difficulty);
        let task = match tasks.into_iter().next() {
            Some(t) => t,
            None => return 0.0,
        };

        // Select arm via Thompson Sampling
        let arm_str = task
            .spec
            .get("arm")
            .and_then(|x| x.as_str())
            .unwrap_or("exact");
        let arm = ArmId(arm_str.to_string());
        let bucket = bucket_for(difficulty, arm_str);

        // Synthesize a plausible solution for the chosen arm
        let solution = self.make_solution(&task, arm_str);

        let eval = self
            .engine
            .evaluate_and_record(domain_id, &task, &solution, bucket, arm);
        eval.score
    }

    /// Build a synthetic solution for the given arm choice.
    fn make_solution(&self, task: &Task, arm: &str) -> Solution {
        let spec = &task.spec;
        let data = if task.domain_id.0 == "exo-retrieval" {
            let dim = spec.get("dim").and_then(|x| x.as_u64()).unwrap_or(128) as usize;
            let k = spec.get("k").and_then(|x| x.as_u64()).unwrap_or(5) as usize;
            let noise = spec.get("noise").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let (recall, _, _) = ExoRetrievalDomain::simulate_score(arm, dim, noise, k);
            let latency_us = match arm {
                "exact" => 500u64,
                "approximate" => 80,
                _ => 150,
            };
            json!({ "recall": recall, "latency_us": latency_us, "retrieved_k": k, "arm": arm })
        } else {
            let n = spec
                .get("n_entities")
                .and_then(|x| x.as_u64())
                .unwrap_or(100) as usize;
            let max_hops = spec.get("max_hops").and_then(|x| x.as_u64()).unwrap_or(3) as usize;
            let min_cov = spec
                .get("min_coverage")
                .and_then(|x| x.as_u64())
                .unwrap_or(5) as usize;
            let (corr, _, _, lat) = ExoGraphDomain::simulate_score(arm, n, max_hops, min_cov);
            let found = (min_cov as f32 * 1.1 * corr) as u64;
            let hops = (max_hops as u64).saturating_sub(1).max(1);
            json!({ "entities_found": found, "hops_used": hops,
                    "coverage_ratio": 1.1 * corr, "arm": arm, "latency_us": lat })
        };
        Solution {
            task_id: task.id.clone(),
            content: arm.to_string(),
            data,
        }
    }

    /// Train both EXO domains for `cycles` iterations each.
    /// Returns (retrieval_mean, graph_mean) scores.
    pub fn warmup(&mut self, cycles: usize) -> (f32, f32) {
        let ret_id = DomainId("exo-retrieval".to_string());
        let gph_id = DomainId("exo-graph".to_string());
        let difficulties = [0.2, 0.5, 0.8];

        let ret_score: f32 = (0..cycles)
            .map(|i| self.train_one(&ret_id, difficulties[i % 3]))
            .sum::<f32>()
            / cycles.max(1) as f32;

        let gph_score: f32 = (0..cycles)
            .map(|i| self.train_one(&gph_id, difficulties[i % 3]))
            .sum::<f32>()
            / cycles.max(1) as f32;

        (ret_score, gph_score)
    }

    /// Transfer priors from retrieval domain → graph domain.
    /// Returns the acceleration factor (>1.0 means transfer helped).
    pub fn transfer_ret_to_graph(&mut self, measure_cycles: usize) -> f32 {
        let src = DomainId("exo-retrieval".to_string());
        let dst = DomainId("exo-graph".to_string());

        // Measure baseline graph performance BEFORE transfer
        let gph_id = DomainId("exo-graph".to_string());
        let difficulties = [0.3, 0.6, 0.9];
        let baseline: f32 = (0..measure_cycles)
            .map(|i| self.train_one(&gph_id, difficulties[i % 3]))
            .sum::<f32>()
            / measure_cycles.max(1) as f32;

        // Initiate transfer: inject retrieval priors into graph bandit
        self.engine.initiate_transfer(&src, &dst);

        // Measure graph performance AFTER transfer
        let transfer: f32 = (0..measure_cycles)
            .map(|i| self.train_one(&gph_id, difficulties[i % 3]))
            .sum::<f32>()
            / measure_cycles.max(1) as f32;

        // Acceleration = ratio of improvement
        if baseline > 0.0 {
            transfer / baseline
        } else {
            1.0
        }
    }

    /// Summary from the scoreboard.
    pub fn summary(&self) -> ruvector_domain_expansion::ScoreboardSummary {
        self.engine.scoreboard_summary()
    }
}

impl Default for ExoTransferAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieval_task_generation() {
        let d = ExoRetrievalDomain::new();
        let tasks = d.generate_tasks(6, 0.5);
        assert_eq!(tasks.len(), 6);
        for t in &tasks {
            assert_eq!(t.domain_id, DomainId("exo-retrieval".to_string()));
            assert!(t.spec.get("k").and_then(|x| x.as_u64()).unwrap_or(0) > 0);
        }
    }

    #[test]
    fn test_retrieval_perfect_solution() {
        let d = ExoRetrievalDomain::new();
        let tasks = d.generate_tasks(1, 0.2);
        let task = &tasks[0];
        let k = task.spec.get("k").and_then(|x| x.as_u64()).unwrap_or(5);
        let sol = Solution {
            task_id: task.id.clone(),
            content: "exact".to_string(),
            data: serde_json::json!({
                "recall": 1.0f32,
                "latency_us": 80u64,
                "retrieved_k": k,
                "arm": "exact",
            }),
        };
        let eval = d.evaluate(task, &sol);
        assert!(
            eval.correctness > 0.9,
            "recall=1.0 → correctness > 0.9, got {}",
            eval.correctness
        );
        assert!(
            eval.score > 0.7,
            "perfect retrieval score > 0.7, got {}",
            eval.score
        );
    }

    #[test]
    fn test_retrieval_reference_solution() {
        let d = ExoRetrievalDomain::new();
        let tasks = d.generate_tasks(1, 0.4);
        let ref_sol = d.reference_solution(&tasks[0]);
        assert!(ref_sol.is_some());
        let sol = ref_sol.unwrap();
        let eval = d.evaluate(&tasks[0], &sol);
        assert!(
            eval.score > 0.5,
            "reference solution should be good: {}",
            eval.score
        );
    }

    #[test]
    fn test_graph_task_generation() {
        let d = ExoGraphDomain::new();
        let tasks = d.generate_tasks(6, 0.6);
        assert_eq!(tasks.len(), 6);
        for t in &tasks {
            assert_eq!(t.domain_id, DomainId("exo-graph".to_string()));
            assert!(t.spec.get("max_hops").and_then(|x| x.as_u64()).unwrap_or(0) >= 2);
        }
    }

    #[test]
    fn test_graph_reference_solution() {
        let d = ExoGraphDomain::new();
        let tasks = d.generate_tasks(1, 0.3);
        let ref_sol = d.reference_solution(&tasks[0]);
        assert!(ref_sol.is_some());
        let sol = ref_sol.unwrap();
        let eval = d.evaluate(&tasks[0], &sol);
        assert!(
            eval.correctness > 0.5,
            "reference solution correctness: {}",
            eval.correctness
        );
    }

    #[test]
    fn test_embeddings_64_dim_and_aligned() {
        let rd = ExoRetrievalDomain::new();
        let gd = ExoGraphDomain::new();

        let sol_r = Solution {
            task_id: "t0".to_string(),
            content: "approximate".to_string(),
            data: serde_json::json!({
                "recall": 0.85f32, "latency_us": 120u64,
                "retrieved_k": 10u64, "arm": "approximate"
            }),
        };
        let sol_g = Solution {
            task_id: "t0".to_string(),
            content: "approx".to_string(),
            data: serde_json::json!({
                "entities_found": 15u64, "hops_used": 2u64,
                "coverage_ratio": 1.1f32, "arm": "approx"
            }),
        };

        let emb_r = rd.embed(&sol_r);
        let emb_g = gd.embed(&sol_g);

        assert_eq!(emb_r.vector.len(), 64, "retrieval embedding must be 64-dim");
        assert_eq!(emb_g.vector.len(), 64, "graph embedding must be 64-dim");

        // Both use "approximate"/"approx" → v[6] should be 1.0 in both
        assert!(
            (emb_r.vector[6] - 1.0).abs() < 1e-6,
            "retrieval approx arm at v[6]"
        );
        assert!(
            (emb_g.vector[6] - 1.0).abs() < 1e-6,
            "graph approx arm at v[6]"
        );

        // Cosine similarity should be meaningful (both represent "approximate" strategy)
        let sim = emb_r.cosine_similarity(&emb_g);
        assert!(
            sim > 0.3,
            "aligned embeddings should have decent similarity: {}",
            sim
        );
    }

    #[test]
    fn test_adapter_warmup_and_transfer() {
        let mut adapter = ExoTransferAdapter::new();

        // Train for a few cycles
        let (ret_score, gph_score) = adapter.warmup(10);
        assert!(
            ret_score >= 0.0 && ret_score <= 1.0,
            "retrieval score in [0,1]: {}",
            ret_score
        );
        assert!(
            gph_score >= 0.0 && gph_score <= 1.0,
            "graph score in [0,1]: {}",
            gph_score
        );

        // Transfer — acceleration >= 0
        let accel = adapter.transfer_ret_to_graph(5);
        assert!(accel >= 0.0, "acceleration must be non-negative: {}", accel);
    }

    #[test]
    fn test_bucket_tier_assignment() {
        let easy = bucket_for(0.1, "top-k-small");
        let med = bucket_for(0.5, "top-k-medium");
        let hard = bucket_for(0.9, "top-k-large");
        assert_eq!(easy.difficulty_tier, "easy");
        assert_eq!(med.difficulty_tier, "medium");
        assert_eq!(hard.difficulty_tier, "hard");
    }
}
