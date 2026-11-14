//! Header-only admission for the legacy ONE-author exact-grid baseline.
//! This is not managed per-source learning or a hardware/accuracy claim.
use super::*;
use std::time::Instant;
const MAX_REPORTED_GRIDS: usize = 8;
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GridReadinessReason {
    Compatible,
    NodeMissing,
    RawGridMissing,
    GridEvidenceWarming,
    GridEvidenceStale,
    NonmatchingQualifiedGrid,
    GridEvidenceUnqualified,
}
#[derive(Debug, Serialize)]
pub(super) struct BoundNodeGridReadiness {
    pub node_id: u8,
    pub ready: bool,
    pub reason: GridReadinessReason,
    pub observed_grids: Vec<CsiGridKey>,
    pub eligible_grids: Vec<CsiGridKey>,
    pub observed_grids_truncated: bool,
    pub eligible_grids_truncated: bool,
}
#[derive(Debug, Serialize)]
pub(super) struct LegacyGridReadiness {
    pub schema: &'static str,
    pub author_source_node_id: u8,
    pub selected_grid: CsiGridKey,
    pub all_bound_nodes_ready: bool,
    pub nodes: Vec<BoundNodeGridReadiness>,
}
/// Shared with the existing author selection: always inspect every candidate,
/// never infer compatibility from a node's preferred grid or board label.
pub(super) fn eligible_grid(grid: CsiGridKey, evidence: CalibrationGridEvidence) -> bool {
    grid.n_subcarriers > 0
        && grid.ppdu_type <= 3
        && evidence.span_s >= CALIBRATION_GRID_MIN_SPAN_S
        && evidence.rate_hz >= CALIBRATION_GRID_MIN_RATE_HZ
        && evidence.max_gap_s < CALIBRATION_GRID_MAX_GAP_S
        && evidence.latest_age_s < CALIBRATION_GRID_MAX_GAP_S
}
fn sorted_grids(mut values: Vec<CsiGridKey>, selected: CsiGridKey) -> Vec<CsiGridKey> {
    values.retain(|g| g.n_subcarriers > 0);
    values.sort_by_key(|g| {
        (
            *g != selected,
            std::cmp::Reverse(g.n_subcarriers),
            g.ppdu_type,
        )
    });
    values.dedup();
    values
}
fn node_readiness(
    id: u8,
    node: Option<&NodeState>,
    selected: CsiGridKey,
    now: Instant,
) -> BoundNodeGridReadiness {
    let Some(node) = node else {
        return BoundNodeGridReadiness {
            node_id: id,
            ready: false,
            reason: GridReadinessReason::NodeMissing,
            observed_grids: vec![],
            eligible_grids: vec![],
            observed_grids_truncated: false,
            eligible_grids_truncated: false,
        };
    };
    let candidates = node.calibration_grid_candidates(now);
    let matching = candidates
        .iter()
        .find(|(g, _)| *g == selected)
        .map(|(_, e)| *e);
    let ready = matching.is_some_and(|e| eligible_grid(selected, e));
    let other_qualified = candidates
        .iter()
        .any(|(g, e)| *g != selected && eligible_grid(*g, *e));
    // A fresh but still warming/weak matching stream is not measured proof of a
    // hardware change, even when a different grid is currently more frequent.
    let reason = if ready {
        GridReadinessReason::Compatible
    } else if let Some(e) = matching.filter(|e| e.latest_age_s < CALIBRATION_GRID_MAX_GAP_S) {
        if e.span_s < CALIBRATION_GRID_MIN_SPAN_S {
            GridReadinessReason::GridEvidenceWarming
        } else {
            GridReadinessReason::GridEvidenceUnqualified
        }
    } else if other_qualified {
        GridReadinessReason::NonmatchingQualifiedGrid
    } else if node.raw_grid_observations.is_empty() {
        GridReadinessReason::RawGridMissing
    } else if candidates.is_empty()
        || candidates
            .iter()
            .all(|(_, e)| e.latest_age_s >= CALIBRATION_GRID_MAX_GAP_S)
    {
        GridReadinessReason::GridEvidenceStale
    } else if candidates.iter().any(|(g, e)| {
        g.ppdu_type <= 3 && g.n_subcarriers > 0 && e.span_s < CALIBRATION_GRID_MIN_SPAN_S
    }) {
        GridReadinessReason::GridEvidenceWarming
    } else {
        GridReadinessReason::GridEvidenceUnqualified
    };
    let all_observed = sorted_grids(candidates.iter().map(|(g, _)| *g).collect(), selected);
    let all_eligible = sorted_grids(
        candidates
            .iter()
            .filter(|(g, e)| eligible_grid(*g, *e))
            .map(|(g, _)| *g)
            .collect(),
        selected,
    );
    // Joint projection preserves subset semantics under the cap. Reserve the
    // selected observed-but-unqualified tuple rather than hiding why it blocks.
    let selected_observed = all_observed.contains(&selected);
    let reserve = usize::from(selected_observed && !all_eligible.contains(&selected));
    let eligible_grids: Vec<_> = all_eligible
        .iter()
        .copied()
        .take(MAX_REPORTED_GRIDS - reserve)
        .collect();
    let mut observed_grids = Vec::new();
    if selected_observed {
        observed_grids.push(selected);
    }
    for g in eligible_grids.iter().chain(&all_observed) {
        if observed_grids.len() == MAX_REPORTED_GRIDS {
            break;
        }
        if !observed_grids.contains(g) {
            observed_grids.push(*g);
        }
    }
    let observed_grids_truncated = all_observed.len() > observed_grids.len();
    let eligible_grids_truncated = all_eligible.len() > eligible_grids.len();
    BoundNodeGridReadiness {
        node_id: id,
        ready,
        reason,
        observed_grids,
        eligible_grids,
        observed_grids_truncated,
        eligible_grids_truncated,
    }
}
impl LegacyGridReadiness {
    pub(super) fn known_incompatibility(&self) -> bool {
        self.nodes
            .iter()
            .any(|n| n.reason == GridReadinessReason::NonmatchingQualifiedGrid)
    }
}
pub(super) fn inspect(
    state: &AppStateInner,
    ids: &[u8],
    binding: CalibrationGridBinding,
    now: Instant,
) -> Option<LegacyGridReadiness> {
    if !valid_source_node_ids(ids) || ids.binary_search(&binding.source_node_id).is_err() {
        return None;
    }
    let nodes: Vec<_> = ids
        .iter()
        .map(|id| node_readiness(*id, state.node_states.get(id), binding.grid, now))
        .collect();
    Some(LegacyGridReadiness {
        schema: "ruview.legacy-calibration-grid-readiness.v1",
        author_source_node_id: binding.source_node_id,
        selected_grid: binding.grid,
        all_bound_nodes_ready: nodes.iter().all(|n| n.ready),
        nodes,
    })
}
