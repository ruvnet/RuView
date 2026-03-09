use std::sync::Mutex;

use crate::domain::node::DiscoveredNode;

/// Sub-state for discovered nodes.
#[derive(Default)]
pub struct DiscoveryState {
    pub nodes: Vec<DiscoveredNode>,
}

/// Sub-state for the managed sensing server process.
#[derive(Default)]
pub struct ServerState {
    pub running: bool,
    pub pid: Option<u32>,
}

/// Top-level application state managed by Tauri.
#[derive(Default)]
pub struct AppState {
    pub discovery: Mutex<DiscoveryState>,
    pub server: Mutex<ServerState>,
}
