//! Mesh time synchronization methods for `NodeState`.
//!
//! Implements ADR-110 §A0.12 timestamp recovery and snapshot queries.
//! Complies with ELMAS.md: <= 350 lines, guard clauses, deterministic flow.

use std::time::{Duration, Instant};

use crate::domain::types::NodeSyncSnapshot;
use super::state::NodeState;

impl NodeState {
    /// Return the mesh-aligned epoch computed from the most recent sync packet,
    /// or `None` if absent or stale (> 9s).
    pub fn mesh_aligned_us(&self, local_at_frame_us: u64) -> Option<u64> {
        let sync = self.latest_sync.as_ref()?;
        if !sync.flags.is_valid {
            return None;
        }
        let seen_at = self.latest_sync_at?;
        if seen_at.elapsed() > Duration::from_secs(9) {
            return None;
        }
        Some(sync.apply_to_local(local_at_frame_us))
    }

    /// Sequence-based mesh-time recovery for an in-flight ADR-018 CSI frame.
    pub fn mesh_aligned_us_for_csi_frame(&self, frame_sequence: u32) -> Option<u64> {
        let sync = self.latest_sync.as_ref()?;
        if !sync.flags.is_valid {
            return None;
        }
        let seen_at = self.latest_sync_at?;
        if seen_at.elapsed() > Duration::from_secs(9) {
            return None;
        }
        let delta_frames = frame_sequence.wrapping_sub(sync.sequence);
        if delta_frames > i32::MAX as u32 {
            return None;
        }
        let fps = self.effective_sample_rate_hz();
        Some(sync.mesh_aligned_us_for_sequence(frame_sequence, fps))
    }

    /// Mesh timestamp for the newest CSI frame admitted to `frame_history`.
    pub fn mesh_aligned_us_for_latest_csi_frame(&self) -> Option<u64> {
        if !self.latest_csi_sync_valid {
            return None;
        }
        self.mesh_aligned_us_for_csi_frame(self.latest_csi_sequence?)
    }

    /// Apply a freshly-decoded sync packet to this node.
    pub fn apply_sync_packet(
        &mut self,
        pkt: wifi_densepose_hardware::SyncPacket,
        now: Instant,
    ) {
        self.latest_sync = Some(pkt);
        self.latest_sync_at = Some(now);
    }

    /// Pure snapshot of this node's mesh-sync state.
    pub fn sync_snapshot(&self) -> Option<NodeSyncSnapshot> {
        let sync = self.latest_sync.as_ref()?;
        Some(NodeSyncSnapshot {
            offset_us: sync.local_minus_epoch_us(),
            is_leader: sync.flags.is_leader,
            is_valid: sync.flags.is_valid,
            smoothed: sync.flags.smoothed_used,
            sequence: sync.sequence,
            csi_fps_ema: self.effective_sample_rate_hz(),
            csi_fps_samples: self.csi_fps_samples,
            staleness_ms: self.latest_sync_at.map(|t| t.elapsed().as_millis() as u64),
        })
    }
}

