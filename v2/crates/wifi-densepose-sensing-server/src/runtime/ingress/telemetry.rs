//! UDP ingress telemetry and pipeline processing health tracking.
//!
//! Complies with ELMAS.md: <= 350 lines, atomic counters, pure functions.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use wifi_densepose_sensing_server::domain::node::constants::ESP32_OFFLINE_TIMEOUT;

/// Receive-side counters for the UDP data plane (issue #1894).
pub struct UdpIngress {
    pub running: AtomicBool,
    pub datagrams: AtomicU64,
    pub last_datagram_unix_ms: AtomicU64,
    pub dropped_by_allowlist: AtomicU64,
}

pub static UDP_INGRESS: UdpIngress = UdpIngress {
    running: AtomicBool::new(false),
    datagrams: AtomicU64::new(0),
    last_datagram_unix_ms: AtomicU64::new(0),
    dropped_by_allowlist: AtomicU64::new(0),
};

/// Liveness of the sensing pipeline as reported on `/health`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingState {
    /// Nothing has been received or published yet.
    Idle,
    /// A sensing update was published within the stale window.
    Live,
    /// Datagrams are arriving but no update has been published within the stale window.
    Stalled,
    /// Input has stopped: neither datagrams nor updates within the window.
    NoInput,
}

impl ProcessingState {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcessingState::Idle => "idle",
            ProcessingState::Live => "live",
            ProcessingState::Stalled => "stalled",
            ProcessingState::NoInput => "no_input",
        }
    }
}

pub fn processing_state(
    update_age_ms: Option<u64>,
    datagram_age_ms: Option<u64>,
    stale_after_ms: u64,
) -> ProcessingState {
    let fresh = |age: Option<u64>| age.is_some_and(|a| a <= stale_after_ms);
    if fresh(update_age_ms) {
        ProcessingState::Live
    } else if fresh(datagram_age_ms) {
        ProcessingState::Stalled
    } else if update_age_ms.is_some() || datagram_age_ms.is_some() {
        ProcessingState::NoInput
    } else {
        ProcessingState::Idle
    }
}

/// The `processing` block of `/health`.
pub fn processing_health(
    update_timestamp_s: Option<f64>,
    now_unix_ms: u64,
) -> serde_json::Value {
    let update_age_ms = update_timestamp_s
        .map(|ts| now_unix_ms.saturating_sub((ts * 1000.0).max(0.0) as u64));
    let udp_running = UDP_INGRESS.running.load(Ordering::Relaxed);
    let last_datagram = UDP_INGRESS.last_datagram_unix_ms.load(Ordering::Relaxed);
    let datagram_age_ms = (udp_running && last_datagram > 0)
        .then(|| now_unix_ms.saturating_sub(last_datagram));
    let state = processing_state(
        update_age_ms,
        datagram_age_ms,
        ESP32_OFFLINE_TIMEOUT.as_millis() as u64,
    );
    let mut out = serde_json::json!({
        "state": state.as_str(),
        "last_update_age_ms": update_age_ms,
    });
    if udp_running {
        out["udp"] = serde_json::json!({
            "datagrams": UDP_INGRESS.datagrams.load(Ordering::Relaxed),
            "last_datagram_age_ms": datagram_age_ms,
            "dropped_by_allowlist": UDP_INGRESS.dropped_by_allowlist.load(Ordering::Relaxed),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_update_is_live_regardless_of_udp() {
        assert_eq!(processing_state(Some(100), None, 5_000), ProcessingState::Live);
        assert_eq!(processing_state(Some(100), Some(90_000), 5_000), ProcessingState::Live);
    }

    #[test]
    fn datagrams_without_updates_is_stalled() {
        assert_eq!(processing_state(Some(60_000), Some(50), 5_000), ProcessingState::Stalled);
        assert_eq!(processing_state(None, Some(50), 5_000), ProcessingState::Stalled);
    }

    #[test]
    fn nothing_recent_is_no_input_and_nothing_ever_is_idle() {
        assert_eq!(processing_state(Some(60_000), Some(60_000), 5_000), ProcessingState::NoInput);
        assert_eq!(processing_state(Some(60_000), None, 5_000), ProcessingState::NoInput);
        assert_eq!(processing_state(None, None, 5_000), ProcessingState::Idle);
    }

    #[test]
    fn health_block_reports_update_age() {
        assert_eq!(processing_health(None, 1_000_000)["state"], "idle");
        let out = processing_health(Some(990.0), 1_000_000);
        assert_eq!(out["last_update_age_ms"], 10_000);
        assert_eq!(out["state"], "no_input");
        assert_eq!(processing_health(Some(999.5), 1_000_000)["state"], "live");
    }
}

