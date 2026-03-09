//! Time-Division Multiplexing (TDM) scheduler for coordinating multiple ESP32
//! sensor nodes.
//!
//! Each node is assigned a time slot within a repeating frame. During its slot
//! a node may transmit sensor data; outside its slot the node listens or
//! sleeps.

use serde::{Deserialize, Serialize};

/// Synchronization method used to align TDM frames across nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMethod {
    /// GPS pulse-per-second signal.
    GpsPps,
    /// NTP-based time synchronization.
    NtpSync,
    /// WiFi beacon timestamp alignment.
    WifiBeacon,
    /// Leader node broadcasts sync pulses; followers align to it.
    LeaderFollower,
}

/// A single node in the TDM schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdmNode {
    /// Unique node identifier.
    pub node_id: u8,
    /// Assigned slot index within the TDM frame.
    pub slot_index: u8,
    /// ADC channels this node is responsible for.
    pub channels: Vec<u8>,
}

/// TDM scheduler for coordinating multiple ESP32 sensor nodes.
///
/// A TDM frame is divided into equally-sized time slots. Each node transmits
/// only during its assigned slot, preventing collisions and ensuring
/// deterministic latency.
pub struct TdmScheduler {
    /// Registered nodes and their slot assignments.
    pub nodes: Vec<TdmNode>,
    /// Duration of a single slot in microseconds.
    pub slot_duration_us: u32,
    /// Total frame duration in microseconds.
    pub frame_duration_us: u32,
    /// Synchronization method.
    pub sync_method: SyncMethod,
}

impl TdmScheduler {
    /// Create a new scheduler for `num_nodes` nodes with the given slot
    /// duration.
    ///
    /// Nodes are assigned sequential slot indices and the frame duration is
    /// computed as `num_nodes * slot_duration_us`.
    pub fn new(num_nodes: usize, slot_duration_us: u32) -> Self {
        let nodes: Vec<TdmNode> = (0..num_nodes)
            .map(|i| TdmNode {
                node_id: i as u8,
                slot_index: i as u8,
                channels: vec![i as u8],
            })
            .collect();

        let frame_duration_us = slot_duration_us * num_nodes as u32;

        Self {
            nodes,
            slot_duration_us,
            frame_duration_us,
            sync_method: SyncMethod::LeaderFollower,
        }
    }

    /// Returns the slot index that is active at `current_time_us` for the
    /// given node, or `None` if the node is not registered.
    pub fn get_slot(&self, node_id: u8, current_time_us: u64) -> Option<u32> {
        let node = self.nodes.iter().find(|n| n.node_id == node_id)?;
        let position_in_frame = (current_time_us % self.frame_duration_us as u64) as u32;
        let current_slot = position_in_frame / self.slot_duration_us;
        if current_slot == node.slot_index as u32 {
            Some(current_slot)
        } else {
            None
        }
    }

    /// Returns `true` if the current time falls within the node's assigned
    /// slot.
    pub fn is_my_slot(&self, node_id: u8, current_time_us: u64) -> bool {
        self.get_slot(node_id, current_time_us).is_some()
    }

    /// Add a node with a specific slot assignment.
    pub fn add_node(&mut self, node: TdmNode) {
        self.nodes.push(node);
        self.frame_duration_us = self.slot_duration_us * self.nodes.len() as u32;
    }

    /// Returns the number of registered nodes.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the time in microseconds until the given node's next slot
    /// begins.
    pub fn time_until_slot(&self, node_id: u8, current_time_us: u64) -> Option<u64> {
        let node = self.nodes.iter().find(|n| n.node_id == node_id)?;
        let position_in_frame = (current_time_us % self.frame_duration_us as u64) as u32;
        let slot_start = node.slot_index as u32 * self.slot_duration_us;

        if position_in_frame < slot_start {
            Some((slot_start - position_in_frame) as u64)
        } else if position_in_frame < slot_start + self.slot_duration_us {
            Some(0) // Already in slot
        } else {
            // Next frame
            Some((self.frame_duration_us - position_in_frame + slot_start) as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tdm_scheduler_slot_assignment() {
        let sched = TdmScheduler::new(4, 1000);
        assert_eq!(sched.frame_duration_us, 4000);

        // Node 0 should be active at t=0..999
        assert!(sched.is_my_slot(0, 0));
        assert!(sched.is_my_slot(0, 500));
        assert!(!sched.is_my_slot(0, 1000));

        // Node 1 should be active at t=1000..1999
        assert!(sched.is_my_slot(1, 1000));
        assert!(sched.is_my_slot(1, 1500));
        assert!(!sched.is_my_slot(1, 2000));

        // Node 3 active at t=3000..3999
        assert!(sched.is_my_slot(3, 3000));
        assert!(!sched.is_my_slot(3, 0));
    }

    #[test]
    fn test_tdm_frame_wraps() {
        let sched = TdmScheduler::new(2, 500);
        // Frame = 1000 us, so t=1000 wraps to position 0
        assert!(sched.is_my_slot(0, 1000));
        assert!(sched.is_my_slot(1, 1500));
        assert!(sched.is_my_slot(0, 2000));
    }

    #[test]
    fn test_get_slot_returns_none_for_unknown_node() {
        let sched = TdmScheduler::new(2, 1000);
        assert!(sched.get_slot(99, 0).is_none());
    }

    #[test]
    fn test_time_until_slot() {
        let sched = TdmScheduler::new(4, 1000);
        // Node 2's slot starts at 2000. At t=500 that's 1500 us away.
        assert_eq!(sched.time_until_slot(2, 500), Some(1500));
        // At t=2500 we're in the slot
        assert_eq!(sched.time_until_slot(2, 2500), Some(0));
        // At t=3500 the slot ended — next one is at 2000 in the next frame (t=6000)
        // position_in_frame = 3500, slot_start = 2000, frame = 4000
        // next = 4000 - 3500 + 2000 = 2500
        assert_eq!(sched.time_until_slot(2, 3500), Some(2500));
    }

    #[test]
    fn test_add_node_updates_frame() {
        let mut sched = TdmScheduler::new(2, 1000);
        assert_eq!(sched.frame_duration_us, 2000);
        sched.add_node(TdmNode {
            node_id: 5,
            slot_index: 2,
            channels: vec![0, 1],
        });
        assert_eq!(sched.frame_duration_us, 3000);
        assert_eq!(sched.num_nodes(), 3);
    }
}
