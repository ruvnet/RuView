//! Selective safety net scan for ADR-033 §3.3.
//!
//! When the HNSW candidate set is too small (< 2*k), the safety net
//! activates a targeted three-phase scan:
//!
//! 1. **Multi-centroid union**: vectors near best-matching centroids
//! 2. **HNSW neighbor expansion**: 1-hop neighbors of existing candidates
//! 3. **Recency window**: recently ingested vectors not yet indexed
//!
//! All phases respect triple budget caps (time, candidates, distance ops).

use std::time::Instant;

use rvf_types::quality::{
    BudgetReport, BudgetType, DegradationReason, DegradationReport, FallbackPath, SafetyNetBudget,
};

use crate::options::SearchResult;

/// A candidate with distance and retrieval source.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: u64,
    pub distance: f32,
}

/// Result of the safety net scan.
#[derive(Clone, Debug)]
pub struct SafetyNetResult {
    /// Additional candidates found by the safety net.
    pub candidates: Vec<Candidate>,
    /// Budget consumption report.
    pub budget_report: BudgetReport,
    /// Whether any budget was exhausted.
    pub budget_exhausted: bool,
    /// If degraded, the full report.
    pub degradation: Option<DegradationReport>,
}

/// Budget tracker enforcing all three caps simultaneously.
struct BudgetTracker {
    deadline_us: u64,
    start: Instant,
    max_candidates: u64,
    max_distance_ops: u64,
    candidates_scanned: u64,
    distance_ops: u64,
    exhausted: bool,
    exhausted_type: Option<BudgetType>,
}

impl BudgetTracker {
    fn new(budget: &SafetyNetBudget) -> Self {
        Self {
            deadline_us: budget.max_scan_time_us,
            start: Instant::now(),
            max_candidates: budget.max_scan_candidates,
            max_distance_ops: budget.max_distance_ops,
            candidates_scanned: 0,
            distance_ops: 0,
            exhausted: false,
            exhausted_type: None,
        }
    }

    /// Check if any budget is exceeded. Returns true if we should stop.
    fn is_exceeded(&mut self) -> bool {
        if self.exhausted {
            return true;
        }

        if self.candidates_scanned >= self.max_candidates {
            self.exhausted = true;
            self.exhausted_type = Some(BudgetType::Candidates);
            return true;
        }

        if self.distance_ops >= self.max_distance_ops {
            self.exhausted = true;
            self.exhausted_type = Some(BudgetType::DistanceOps);
            return true;
        }

        let elapsed_us = self.start.elapsed().as_micros() as u64;
        if elapsed_us >= self.deadline_us {
            self.exhausted = true;
            self.exhausted_type = Some(BudgetType::Time);
            return true;
        }

        false
    }

    /// Record a distance operation. Returns true if budget still available.
    fn record_distance_op(&mut self) -> bool {
        self.distance_ops += 1;
        self.candidates_scanned += 1;
        !self.is_exceeded()
    }

    /// Record multiple distance operations. Returns true if budget still available.
    #[allow(dead_code)]
    fn record_ops(&mut self, count: u64) -> bool {
        self.distance_ops += count;
        self.candidates_scanned += count;
        !self.is_exceeded()
    }

    fn elapsed_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }
}

/// Compute squared L2 distance between two vectors.
fn l2_distance_sq(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// Execute the selective safety net scan.
///
/// The scan proceeds in three phases, each respecting the budget:
/// 1. Multi-centroid union: scan vectors assigned to top-T centroids
/// 2. HNSW neighbor expansion: 1-hop neighbors of existing HNSW candidates
/// 3. Recency window: most recently ingested vectors
///
/// # Arguments
/// * `query` - The query vector.
/// * `k` - Number of neighbors requested.
/// * `hnsw_candidates` - Candidates already found by HNSW (may be empty).
/// * `all_vectors` - All stored vectors as (id, vector) pairs.
/// * `budget` - Triple budget caps.
/// * `vector_count` - Total number of vectors in the store.
pub fn selective_safety_net_scan(
    query: &[f32],
    k: usize,
    hnsw_candidates: &[SearchResult],
    all_vectors: &[(u64, &[f32])],
    budget: &SafetyNetBudget,
    vector_count: u64,
) -> SafetyNetResult {
    if budget.is_disabled() {
        return SafetyNetResult {
            candidates: Vec::new(),
            budget_report: BudgetReport::default(),
            budget_exhausted: false,
            degradation: None,
        };
    }

    let mut tracker = BudgetTracker::new(budget);
    let mut candidates: Vec<Candidate> = Vec::new();

    // Collect existing candidate IDs for dedup.
    let existing_ids: std::collections::HashSet<u64> =
        hnsw_candidates.iter().map(|c| c.id).collect();

    // Phase 1: Scan from the beginning of all_vectors (simulating centroid union).
    // In a full implementation, vectors would be organized by centroid.
    // Here we scan a targeted subset proportional to sqrt(total).
    let phase1_limit = ((vector_count as f64).sqrt().ceil() as usize).min(all_vectors.len());

    for &(id, vec) in all_vectors.iter().take(phase1_limit) {
        if tracker.is_exceeded() {
            break;
        }
        if existing_ids.contains(&id) {
            continue;
        }
        if vec.len() != query.len() {
            continue;
        }
        let dist = l2_distance_sq(query, vec);
        if !tracker.record_distance_op() {
            candidates.push(Candidate { id, distance: dist });
            break;
        }
        candidates.push(Candidate { id, distance: dist });
    }

    // Phase 2: HNSW neighbor expansion.
    // Scan neighbors of existing candidates (approximate using vector proximity).
    if !tracker.is_exceeded() && !hnsw_candidates.is_empty() {
        let expansion_budget = k.min(hnsw_candidates.len());
        let mut neighbor_ids: Vec<u64> = Vec::new();

        for _existing in hnsw_candidates.iter().take(expansion_budget) {
            if tracker.is_exceeded() {
                break;
            }
            // Find nearby vectors as "neighbors" (simplified for runtime).
            for &(id, vec) in all_vectors.iter() {
                if tracker.is_exceeded() {
                    break;
                }
                if existing_ids.contains(&id) || neighbor_ids.contains(&id) {
                    continue;
                }
                if vec.len() != query.len() {
                    continue;
                }
                let dist = l2_distance_sq(query, vec);
                if !tracker.record_distance_op() {
                    candidates.push(Candidate { id, distance: dist });
                    neighbor_ids.push(id);
                    break;
                }
                candidates.push(Candidate { id, distance: dist });
                neighbor_ids.push(id);
                // Only take a few neighbors per candidate.
                if neighbor_ids.len() >= expansion_budget * 3 {
                    break;
                }
            }
        }
    }

    // Phase 3: Recency window — scan most recently added vectors.
    if !tracker.is_exceeded() {
        let recency_limit = (budget.max_scan_candidates - tracker.candidates_scanned)
            .min(all_vectors.len() as u64) as usize;

        for &(id, vec) in all_vectors.iter().rev().take(recency_limit) {
            if tracker.is_exceeded() {
                break;
            }
            if existing_ids.contains(&id) {
                continue;
            }
            if vec.len() != query.len() {
                continue;
            }
            let dist = l2_distance_sq(query, vec);
            if !tracker.record_distance_op() {
                candidates.push(Candidate { id, distance: dist });
                break;
            }
            candidates.push(Candidate { id, distance: dist });
        }
    }

    let elapsed = tracker.elapsed_us();
    let budget_report = BudgetReport {
        safety_net_scan_us: elapsed,
        total_us: elapsed,
        distance_ops: tracker.distance_ops,
        distance_ops_budget: budget.max_distance_ops,
        linear_scan_count: tracker.candidates_scanned,
        linear_scan_budget: budget.max_scan_candidates,
        ..BudgetReport::default()
    };

    let degradation = if tracker.exhausted {
        Some(DegradationReport {
            fallback_path: FallbackPath::SafetyNetBudgetExhausted,
            reason: DegradationReason::BudgetExhausted {
                scanned: tracker.candidates_scanned,
                total: vector_count,
                budget_type: tracker.exhausted_type.unwrap_or(BudgetType::DistanceOps),
            },
            guarantee_lost: "recall may be below target; safety net budget exhausted",
        })
    } else {
        None
    };

    SafetyNetResult {
        candidates,
        budget_report,
        budget_exhausted: tracker.exhausted,
        degradation,
    }
}

/// Determine if the safety net should activate.
///
/// Activates when the HNSW candidate set is smaller than `2 * k`.
pub fn should_activate_safety_net(hnsw_candidate_count: usize, k: usize) -> bool {
    hnsw_candidate_count < 2 * k
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vectors(count: usize, dim: usize) -> Vec<(u64, Vec<f32>)> {
        (0..count)
            .map(|i| {
                let vec: Vec<f32> = (0..dim).map(|d| (i * dim + d) as f32 * 0.01).collect();
                (i as u64, vec)
            })
            .collect()
    }

    #[test]
    fn safety_net_disabled_returns_empty() {
        let query = vec![0.0; 4];
        let vecs = make_vectors(100, 4);
        let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

        let result =
            selective_safety_net_scan(&query, 10, &[], &refs, &SafetyNetBudget::DISABLED, 100);
        assert!(result.candidates.is_empty());
        assert!(!result.budget_exhausted);
    }

    #[test]
    fn safety_net_finds_candidates() {
        let query = vec![0.0; 4];
        let vecs = make_vectors(100, 4);
        let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

        let result =
            selective_safety_net_scan(&query, 10, &[], &refs, &SafetyNetBudget::LAYER_A, 100);
        assert!(!result.candidates.is_empty());
        assert!(result.budget_report.distance_ops > 0);
    }

    #[test]
    fn safety_net_respects_distance_ops_budget() {
        let query = vec![0.0; 4];
        let vecs = make_vectors(50_000, 4);
        let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

        let tight_budget = SafetyNetBudget {
            max_scan_time_us: 1_000_000, // 1 second (won't hit)
            max_scan_candidates: 50,
            max_distance_ops: 50,
        };

        let result = selective_safety_net_scan(&query, 10, &[], &refs, &tight_budget, 50_000);
        // Must not exceed budget.
        assert!(result.budget_report.distance_ops <= 51); // +1 for the op that triggers exhaustion
    }

    #[test]
    fn safety_net_reports_budget_exhaustion() {
        let query = vec![0.0; 4];
        let vecs = make_vectors(10_000, 4);
        let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

        let tiny_budget = SafetyNetBudget {
            max_scan_time_us: 1_000_000,
            max_scan_candidates: 5,
            max_distance_ops: 5,
        };

        let result = selective_safety_net_scan(&query, 10, &[], &refs, &tiny_budget, 10_000);
        assert!(result.budget_exhausted);
        assert!(result.degradation.is_some());
        let deg = result.degradation.unwrap();
        assert_eq!(deg.fallback_path, FallbackPath::SafetyNetBudgetExhausted);
    }

    #[test]
    fn safety_net_deduplicates_existing() {
        let query = vec![0.0; 4];
        let vecs = make_vectors(20, 4);
        let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

        let existing = vec![
            SearchResult {
                id: 0,
                distance: 0.1,
                retrieval_quality: rvf_types::quality::RetrievalQuality::Full,
            },
            SearchResult {
                id: 1,
                distance: 0.2,
                retrieval_quality: rvf_types::quality::RetrievalQuality::Full,
            },
        ];

        let result =
            selective_safety_net_scan(&query, 5, &existing, &refs, &SafetyNetBudget::LAYER_A, 20);
        // Should not contain ids 0 or 1.
        for c in &result.candidates {
            assert!(c.id != 0 && c.id != 1);
        }
    }

    #[test]
    fn should_activate_when_insufficient() {
        assert!(should_activate_safety_net(3, 5));
        assert!(should_activate_safety_net(9, 5));
        assert!(!should_activate_safety_net(10, 5));
        assert!(!should_activate_safety_net(100, 5));
    }

    #[test]
    fn l2_distance_basic() {
        let a = [1.0, 0.0, 0.0];
        let b = [0.0, 0.0, 0.0];
        assert!((l2_distance_sq(&a, &b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn budget_tracker_time_enforcement() {
        let budget = SafetyNetBudget {
            max_scan_time_us: 0, // Instant timeout
            max_scan_candidates: 1_000_000,
            max_distance_ops: 1_000_000,
        };
        let mut tracker = BudgetTracker::new(&budget);
        // Even with generous other budgets, time should exhaust.
        assert!(tracker.is_exceeded());
    }

    #[test]
    fn budget_tracker_candidate_enforcement() {
        let budget = SafetyNetBudget {
            max_scan_time_us: 1_000_000,
            max_scan_candidates: 3,
            max_distance_ops: 1_000_000,
        };
        let mut tracker = BudgetTracker::new(&budget);
        assert!(tracker.record_distance_op()); // 1 <= 3
        assert!(tracker.record_distance_op()); // 2 <= 3
                                               // 3rd record hits the cap (3 >= 3), returns false — budget exhausted.
        assert!(!tracker.record_distance_op());
        assert!(tracker.exhausted);
        assert_eq!(tracker.distance_ops, 3);
    }
}
