//! Node state eviction and periodic sweep helpers.

use std::collections::HashMap;
use std::time::Duration;
use tracing::info;
use super::state::NodeState;

pub const NODE_STATES_EVICT_AFTER: Duration = Duration::from_secs(60);
pub const NODE_STATES_EVICT_INTERVAL: Duration = Duration::from_secs(30);

/// Remove every node silent for `NODE_STATES_EVICT_AFTER` (or that never
/// reported), log it, and return the evicted ids.
pub fn evict_stale_node_states(
    node_states: &mut HashMap<u8, NodeState>,
    now: std::time::Instant,
) -> Vec<u8> {
    let stale_ids: Vec<u8> = node_states
        .iter()
        .filter(|(_, ns)| {
            ns.last_frame_time
                .is_none_or(|t| now.saturating_duration_since(t) >= NODE_STATES_EVICT_AFTER)
        })
        .map(|(&id, _)| id)
        .collect();

    for id in &stale_ids {
        node_states.remove(id);
        info!("node {id} offline (no frames for 60s)");
    }

    if !stale_ids.is_empty() {
        info!(
            "Evicted {} stale node(s), {} active",
            stale_ids.len(),
            node_states.len()
        );
    }
    stale_ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(secs_ago: u64, now: std::time::Instant) -> NodeState {
        let mut ns = NodeState::new();
        ns.last_frame_time = Some(now - Duration::from_secs(secs_ago));
        ns
    }

    #[test]
    fn silent_and_never_reported_nodes_are_evicted_fresh_ones_kept() {
        let now = std::time::Instant::now();
        let mut nodes: HashMap<u8, NodeState> = HashMap::new();
        nodes.insert(1, seen(5, now));
        nodes.insert(2, seen(NODE_STATES_EVICT_AFTER.as_secs() + 1, now));
        let mut never = NodeState::new();
        never.last_frame_time = None;
        nodes.insert(3, never);

        let mut evicted = evict_stale_node_states(&mut nodes, now);
        evicted.sort_unstable();
        assert_eq!(evicted, vec![2, 3]);
        assert_eq!(nodes.keys().copied().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn a_255_node_burst_drains_completely() {
        let now = std::time::Instant::now();
        let mut nodes: HashMap<u8, NodeState> = HashMap::new();
        for id in 1..=255u8 {
            nodes.insert(id, seen(NODE_STATES_EVICT_AFTER.as_secs() + 1, now));
        }
        assert_eq!(evict_stale_node_states(&mut nodes, now).len(), 255);
        assert!(nodes.is_empty());
    }
}

