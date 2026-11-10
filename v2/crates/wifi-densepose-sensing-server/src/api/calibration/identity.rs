//! Calibration session identity and binding validators.

use std::collections::BTreeSet;
use rand::{rngs::OsRng, RngCore};
use crate::domain::node::NodeState;
use crate::domain::node::constants::ESP32_OFFLINE_TIMEOUT;

/// Generate an opaque hexadecimal calibration identifier with a prefix.
pub fn opaque_calibration_id(prefix: &str) -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let suffix: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{prefix}-{suffix}")
}

/// Generate an opaque model identifier for a calibration capture.
pub fn opaque_calibration_model_id() -> String {
    opaque_calibration_id("cal-model")
}

/// Validate that a binding digest is exactly 64 lower-case hex characters.
pub fn valid_binding_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Validate that source node IDs are non-empty, at most 16, and strictly sorted ascending.
pub fn valid_source_node_ids(source_node_ids: &[u8]) -> bool {
    !source_node_ids.is_empty()
        && source_node_ids.len() <= 16
        && source_node_ids.windows(2).all(|pair| pair[0] < pair[1])
}

/// Check if a given node ID is in the accepted calibration sources set.
pub fn calibration_source_accepts(
    source_node_ids: &BTreeSet<u8>,
    node_id: u8,
) -> bool {
    source_node_ids.is_empty() || source_node_ids.contains(&node_id)
}

/// Verify if a node has recent accepted CSI frames within the offline timeout.
pub fn calibration_source_has_fresh_csi(node: &NodeState, now: std::time::Instant) -> bool {
    node.latest_accepted_csi_at
        .is_some_and(|seen| now.saturating_duration_since(seen) < ESP32_OFFLINE_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_link_calibration_binds_to_first_source() {
        let mut sources = BTreeSet::new();
        assert!(calibration_source_accepts(&sources, 5));
        sources.insert(5);
        assert!(calibration_source_accepts(&sources, 5));
        assert!(!calibration_source_accepts(&sources, 3));
    }

    #[test]
    fn explicit_single_link_source_rejects_other_live_nodes() {
        let sources = BTreeSet::from([5]);
        assert!(calibration_source_accepts(&sources, 5));
        assert!(!calibration_source_accepts(&sources, 3));
        assert!(!calibration_source_accepts(&sources, 7));
    }

    #[test]
    fn edge_vitals_liveness_cannot_authorize_raw_csi_calibration() {
        let now = std::time::Instant::now();
        let mut node = NodeState::new();
        node.last_frame_time = Some(now);
        assert!(!calibration_source_has_fresh_csi(&node, now));

        node.observe_accepted_csi_frame(9, false, now);
        assert!(calibration_source_has_fresh_csi(&node, now));
        assert!(!calibration_source_has_fresh_csi(
            &node,
            now + ESP32_OFFLINE_TIMEOUT
        ));
    }

    #[test]
    fn binding_digest_validation() {
        assert!(valid_binding_digest(&"a".repeat(64)));
        assert!(valid_binding_digest(&"0123456789abcdef".repeat(4)));
        assert!(!valid_binding_digest(&"a".repeat(63)));
        assert!(!valid_binding_digest(&"a".repeat(65)));
        assert!(!valid_binding_digest(&"G".repeat(64)));
    }

    #[test]
    fn source_node_ids_validation() {
        assert!(valid_source_node_ids(&[1, 2, 3]));
        assert!(valid_source_node_ids(&[5]));
        assert!(!valid_source_node_ids(&[]));
        assert!(!valid_source_node_ids(&[2, 1])); // not sorted
        assert!(!valid_source_node_ids(&[1, 1])); // duplicates
        assert!(!valid_source_node_ids(&(0..17).collect::<Vec<_>>())); // > 16
    }
}

