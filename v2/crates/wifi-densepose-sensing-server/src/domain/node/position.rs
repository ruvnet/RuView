//! Node position resolution, active position mapping, and default fallback management.
//!
//! Complies with ELMAS.md: <= 350 lines, guard clauses, pure domain logic.

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tracing::{info, warn};

use crate::domain::classification::NODE_STALE_AFTER_MS;
use super::state::NodeState;

/// Position reported for a node with no `--node-positions` entry.
pub const DEFAULT_NODE_POSITION: [f64; 3] = [2.0, 0.0, 1.5];

/// Verify whether a node reported within the strict freshness window.
pub fn node_is_fresh(n: &NodeState, now: Instant) -> bool {
    n.last_frame_time
        .and_then(|seen| now.checked_duration_since(seen))
        .is_some_and(|age| age < std::time::Duration::from_millis(NODE_STALE_AFTER_MS))
}

/// Positions of the nodes currently reporting, keyed by node id.
///
/// Rank assignment makes a position depend on which other nodes happen to be
/// alive. One node going quiet re-ranks every node above it and silently moves
/// their coordinates. Keying by id states the mapping directly.
pub fn node_positions_by_active_id(
    node_positions_config: &HashMap<u8, [f32; 3]>,
    node_states: &HashMap<u8, NodeState>,
    now: Instant,
) -> HashMap<u8, [f64; 3]> {
    node_states
        .iter()
        .filter(|(_, n)| {
            n.last_frame_time
                .is_some_and(|t| now.duration_since(t).as_secs() < 10)
        })
        .filter_map(|(&id, _)| {
            node_positions_config
                .get(&id)
                .map(|p| (id, [p[0] as f64, p[1] as f64, p[2] as f64]))
        })
        .collect()
}

/// Node ids with no `--node-positions` entry, sorted ascending.
pub fn default_position_node_ids(
    node_positions_config: &HashMap<u8, [f32; 3]>,
    node_ids: impl IntoIterator<Item = u8>,
) -> Vec<u8> {
    let mut ids: Vec<u8> = node_ids
        .into_iter()
        .filter(|id| !node_positions_config.contains_key(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Log once per node id when a fresh node falls back to the default position.
pub fn note_default_position_nodes(
    node_positions_config: &HashMap<u8, [f32; 3]>,
    node_states: &HashMap<u8, NodeState>,
    warned: &mut HashSet<u8>,
    now: Instant,
) -> Vec<u8> {
    let fresh = node_states
        .iter()
        .filter(|(_, n)| node_is_fresh(n, now))
        .map(|(&id, _)| id);
    let newly: Vec<u8> = default_position_node_ids(node_positions_config, fresh)
        .into_iter()
        .filter(|id| warned.insert(*id))
        .collect();
    if newly.is_empty() {
        return newly;
    }
    if node_positions_config.is_empty() {
        info!(
            "node(s) {newly:?} use the default position {DEFAULT_NODE_POSITION:?} \
             (no --node-positions configured)"
        );
    } else {
        let mut configured: Vec<u8> = node_positions_config.keys().copied().collect();
        configured.sort_unstable();
        warn!(
            "node(s) {newly:?} have no --node-positions entry and use the default \
             position {DEFAULT_NODE_POSITION:?}; configured node ids are {configured:?}"
        );
    }
    newly
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_node(now: Instant) -> NodeState {
        let mut ns = NodeState::new();
        ns.last_frame_time = Some(now);
        ns
    }

    fn cfg(entries: &[(u8, [f32; 3])]) -> HashMap<u8, [f32; 3]> {
        entries.iter().copied().collect()
    }

    #[test]
    fn non_sequential_node_ids_get_their_own_position() {
        let now = Instant::now();
        let mut node_states = HashMap::new();
        node_states.insert(13, active_node(now));
        node_states.insert(11, active_node(now));
        node_states.insert(12, active_node(now));

        let configured = cfg(&[
            (11, [1.0, 0.0, 0.0]),
            (12, [2.0, 0.0, 0.0]),
            (13, [3.0, 0.0, 0.0]),
        ]);
        let resolved = node_positions_by_active_id(&configured, &node_states, now);

        assert_eq!(resolved.get(&11), Some(&[1.0, 0.0, 0.0]));
        assert_eq!(resolved.get(&12), Some(&[2.0, 0.0, 0.0]));
        assert_eq!(resolved.get(&13), Some(&[3.0, 0.0, 0.0]));
    }

    #[test]
    fn a_stale_node_does_not_move_the_others() {
        let now = Instant::now();
        let configured = cfg(&[
            (11, [1.0, 0.0, 0.0]),
            (12, [2.0, 0.0, 0.0]),
            (13, [3.0, 0.0, 0.0]),
        ]);

        let mut node_states = HashMap::new();
        node_states.insert(11, active_node(now));
        node_states.insert(12, active_node(now));
        node_states.insert(13, active_node(now));
        let all_up = node_positions_by_active_id(&configured, &node_states, now);

        let mut stale = NodeState::new();
        stale.last_frame_time = Some(now - std::time::Duration::from_secs(30));
        node_states.insert(12, stale);
        let one_down = node_positions_by_active_id(&configured, &node_states, now);

        assert_eq!(one_down.get(&12), None, "a stale node reports no position");
        assert_eq!(one_down.get(&11), all_up.get(&11), "node 11 must not move");
        assert_eq!(
            one_down.get(&13),
            all_up.get(&13),
            "node 13 must not inherit node 12's coordinates"
        );
    }

    #[test]
    fn missing_config_entries_fall_through_to_the_caller_default() {
        let now = Instant::now();
        let mut node_states = HashMap::new();
        node_states.insert(11, active_node(now));

        let resolved = node_positions_by_active_id(&cfg(&[]), &node_states, now);
        assert_eq!(resolved.get(&11), None);
    }

    #[test]
    fn default_position_node_ids_lists_unconfigured_nodes_sorted() {
        let configured = cfg(&[(11, [0.0; 3]), (12, [0.0; 3]), (2, [0.0; 3])]);
        assert_eq!(
            default_position_node_ids(&configured, [3, 1, 2, 3]),
            vec![1, 3]
        );
        assert!(default_position_node_ids(&configured, [2, 11]).is_empty());
    }

    #[test]
    fn default_position_note_fires_once_per_fresh_node() {
        let now = Instant::now();
        let configured = cfg(&[(11, [1.0, 0.0, 0.0])]);
        let mut node_states = HashMap::new();
        node_states.insert(1, active_node(now));
        node_states.insert(11, active_node(now));
        let mut stale = NodeState::new();
        stale.last_frame_time = Some(now - std::time::Duration::from_secs(30));
        node_states.insert(5, stale);
        let mut warned = HashSet::new();

        let first = note_default_position_nodes(&configured, &node_states, &mut warned, now);
        assert_eq!(first, vec![1]);

        let again = note_default_position_nodes(&configured, &node_states, &mut warned, now);
        assert!(again.is_empty());

        node_states.remove(&1);
        node_states.insert(1, active_node(now));
        node_states.insert(2, active_node(now));
        let later = note_default_position_nodes(&configured, &node_states, &mut warned, now);
        assert_eq!(later, vec![2]);
    }
}
