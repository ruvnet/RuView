//! Periodic node state eviction background task.
//!
//! Complies with ELMAS.md: <= 350 lines, decoupled runtime task.

use crate::SharedState;
use wifi_densepose_sensing_server::domain::node::{
    evict_stale_node_states, NODE_STATES_EVICT_INTERVAL,
};

/// Periodic node eviction task independent of frame arrival.
///
/// Removes any node silent for `NODE_STATES_EVICT_AFTER` (60s) from `node_states`
/// to bound memory and clear abandoned IDs.
pub async fn node_states_eviction_task(state: SharedState) {
    let mut interval = tokio::time::interval(NODE_STATES_EVICT_INTERVAL);
    loop {
        interval.tick().await;
        let mut s = state.write().await;
        evict_stale_node_states(&mut s.node_states, std::time::Instant::now());
    }
}

