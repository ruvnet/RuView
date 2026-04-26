//! TDM (Time-Division Multiplexed) sensing protocol for multistatic WiFi sensing.
//!
//! Implements the TDMA sensing schedule described in ADR-029 (RuvSense) and
//! ADR-031 (RuView). Each ESP32 node transmits NDP frames in its assigned slot
//! while all other nodes receive, producing N*(N-1) bistatic CSI links per cycle.
//!
//! # 4-Node Example (ADR-029 Table)
//!
//! ```text
//! Slot 0: Node A TX, B/C/D RX (4 ms)
//! Slot 1: Node B TX, A/C/D RX (4 ms)
//! Slot 2: Node C TX, A/B/D RX (4 ms)
//! Slot 3: Node D TX, A/B/C RX (4 ms)
//! Slot 4: Processing + fusion  (30 ms)
//! Total: 50 ms = 20 Hz
//! ```
//!
//! # Clock Drift Compensation
//!
//! ESP32 crystal drift is +/-10 ppm. Over a 50 ms cycle:
//!   drift = 10e-6 * 50e-3 = 0.5 us
//!
//! This is well within the 1 ms guard interval between slots, so no
//! cross-node phase alignment is needed at the TDM scheduling layer.
//! The coordinator tracks cumulative drift and issues correction offsets
//! in sync beacons when drift exceeds a configurable threshold.

use std::fmt;
use std::time::{Duration, Instant};

/// Maximum supported nodes in a single TDM schedule.
const MAX_NODES: usize = 16;

/// Default guard interval between TX slots (microseconds).
const DEFAULT_GUARD_US: u64 = 1_000;

/// Default processing time after all TX slots complete (milliseconds).
const DEFAULT_PROCESSING_MS: u64 = 30;

/// Default TX slot duration (milliseconds).
const DEFAULT_SLOT_MS: u64 = 4;

/// Crystal drift specification for ESP32 (parts per million).
const CRYSTAL_DRIFT_PPM: f64 = 10.0;

/// Errors that can occur during TDM schedule operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TdmError {
    /// Node count is zero or exceeds the maximum.
    InvalidNodeCount { count: usize, max: usize },
    /// A slot index is out of bounds for the current schedule.
    SlotIndexOutOfBounds { index: usize, num_slots: usize },
    /// A node ID is not present in the schedule.
    UnknownNode { node_id: u8 },
    /// The guard interval is too large relative to the slot duration.
    GuardIntervalTooLarge { guard_us: u64, slot_us: u64 },
    /// Cycle period is too short to fit all slots plus processing.
    CycleTooShort { needed_us: u64, available_us: u64 },
    /// Drift correction offset exceeds the guard interval.
    DriftExceedsGuard { drift_us: f64, guard_us: u64 },
}

impl fmt::Display for TdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TdmError::InvalidNodeCount { count, max } => {
                write!(f, "Invalid node count: {} (max {})", count, max)
            }
            TdmError::SlotIndexOutOfBounds { index, num_slots } => {
                write!(f, "Slot index {} out of bounds (schedule has {} slots)", index, num_slots)
            }
            TdmError::UnknownNode { node_id } => {
                write!(f, "Unknown node ID: {}", node_id)
            }
            TdmError::GuardIntervalTooLarge { guard_us, slot_us } => {
                write!(f, "Guard interval {} us exceeds slot duration {} us", guard_us, slot_us)
            }
            TdmError::CycleTooShort { needed_us, available_us } => {
                write!(f, "Cycle too short: need {} us, have {} us", needed_us, available_us)
            }
            TdmError::DriftExceedsGuard { drift_us, guard_us } => {
                write!(f, "Drift {:.1} us exceeds guard interval {} us", drift_us, guard_us)
            }
        }
    }
}

impl std::error::Error for TdmError {}

/// A single TDM time slot assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdmSlot {
    /// Index of this slot within the cycle (0-based).
    pub index: usize,
    /// Node ID assigned to transmit during this slot.
    pub tx_node_id: u8,
    /// Duration of the TX window (excluding guard interval).
    pub duration: Duration,
    /// Guard interval after this slot before the next begins.
    pub guard_interval: Duration,
}

impl TdmSlot {
    /// Total duration of this slot including guard interval.
    pub fn total_duration(&self) -> Duration {
        self.duration + self.guard_interval
    }

    /// Start offset of this slot within the cycle.
    ///
    /// Requires the full slot list to compute cumulative offset.
    pub fn start_offset(slots: &[TdmSlot], index: usize) -> Option<Duration> {
        if index >= slots.len() {
            return None;
        }
        let mut offset = Duration::ZERO;
        for slot in &slots[..index] {
            offset += slot.total_duration();
        }
        Some(offset)
    }
}

/// TDM sensing schedule defining slot assignments and cycle timing.
///
/// A schedule assigns each node exactly one TX slot per cycle. During a
/// node's TX slot, it transmits NDP frames while all other nodes receive
/// and extract CSI. After all TX slots, a processing window allows the
/// aggregator to fuse the collected CSI data.
///
/// # Example: 4-node schedule at 20 Hz
///
/// ```
/// use wifi_densepose_hardware::esp32::TdmSchedule;
/// use std::time::Duration;
///
/// let schedule = TdmSchedule::uniform(
///     &[0, 1, 2, 3],                  // 4 node IDs
///     Duration::from_millis(4),        // 4 ms per TX slot
///     Duration::from_micros(1_000),    // 1 ms guard interval
///     Duration::from_millis(30),       // 30 ms processing window
/// ).unwrap();
///
/// assert_eq!(schedule.node_count(), 4);
/// assert_eq!(schedule.cycle_period().as_millis(), 50); // 4*(4+1) + 30 = 50
/// assert_eq!(schedule.update_rate_hz(), 20.0);
/// ```
#[derive(Debug, Clone)]
pub struct TdmSchedule {
    /// Ordered slot assignments (one per node).
    slots: Vec<TdmSlot>,
    /// Processing window after all TX slots.
    processing_window: Duration,
    /// Total cycle period (sum of all slots + processing).
    cycle_period: Duration,
}

impl TdmSchedule {
    /// Create a uniform TDM schedule where all nodes have equal slot duration.
    ///
    /// # Arguments
    ///
    /// * `node_ids` - Ordered list of node IDs (determines TX order)
    /// * `slot_duration` - TX window duration per slot
    /// * `guard_interval` - Guard interval between consecutive slots
    /// * `processing_window` - Time after all TX slots for fusion processing
    ///
    /// # Errors
    ///
    /// Returns `TdmError::InvalidNodeCount` if `node_ids` is empty or exceeds
    /// `MAX_NODES`. Returns `TdmError::GuardIntervalTooLarge` if the guard
    /// interval is larger than the slot duration.
    pub fn uniform(
        node_ids: &[u8],
        slot_duration: Duration,
        guard_interval: Duration,
        processing_window: Duration,
    ) -> Result<Self, TdmError> {
        if node_ids.is_empty() || node_ids.len() > MAX_NODES {
            return Err(TdmError::InvalidNodeCount {
                count: node_ids.len(),
                max: MAX_NODES,
            });
        }

        let slot_us = slot_duration.as_micros() as u64;
        let guard_us = guard_interval.as_micros() as u64;
        if guard_us >= slot_us {
            return Err(TdmError::GuardIntervalTooLarge { guard_us, slot_us });
        }

        let slots: Vec<TdmSlot> = node_ids
            .iter()
            .enumerate()
            .map(|(i, &node_id)| TdmSlot {
                index: i,
                tx_node_id: node_id,
                duration: slot_duration,
                guard_interval,
            })
            .collect();

        let tx_total: Duration = slots.iter().map(|s| s.total_duration()).sum();
        let cycle_period = tx_total + processing_window;

        Ok(Self {
            slots,
            processing_window,
            cycle_period,
        })
    }

    /// Create the default 4-node, 20 Hz schedule from ADR-029.
    ///
    /// ```
    /// use wifi_densepose_hardware::esp32::TdmSchedule;
    ///
    /// let schedule = TdmSchedule::default_4node();
    /// assert_eq!(schedule.node_count(), 4);
    /// assert_eq!(schedule.update_rate_hz(), 20.0);
    /// ```
    pub fn default_4node() -> Self {
        Self::uniform(
            &[0, 1, 2, 3],
            Duration::from_millis(DEFAULT_SLOT_MS),
            Duration::from_micros(DEFAULT_GUARD_US),
            Duration::from_millis(DEFAULT_PROCESSING_MS),
        )
        .expect("default 4-node schedule is always valid")
    }

    /// Number of nodes in this schedule.
    pub fn node_count(&self) -> usize {
        self.slots.len()
    }

    /// Total cycle period (time between consecutive cycle starts).
    pub fn cycle_period(&self) -> Duration {
        self.cycle_period
    }

    /// Effective update rate in Hz.
    pub fn update_rate_hz(&self) -> f64 {
        1.0 / self.cycle_period.as_secs_f64()
    }

    /// Duration of the processing window after all TX slots.
    pub fn processing_window(&self) -> Duration {
        self.processing_window
    }

    /// Get the slot assignment for a given slot index.
    pub fn slot(&self, index: usize) -> Option<&TdmSlot> {
        self.slots.get(index)
    }

    /// Get the slot assigned to a specific node.
    pub fn slot_for_node(&self, node_id: u8) -> Option<&TdmSlot> {
        self.slots.iter().find(|s| s.tx_node_id == node_id)
    }

    /// Immutable slice of all slot assignments.
    pub fn slots(&self) -> &[TdmSlot] {
        &self.slots
    }

    /// Compute the maximum clock drift in microseconds for this cycle.
    ///
    /// Uses the ESP32 crystal specification of +/-10 ppm.
    pub fn max_drift_us(&self) -> f64 {
        CRYSTAL_DRIFT_PPM * 1e-6 * self.cycle_period.as_secs_f64() * 1e6
    }

    /// Check whether clock drift stays within the guard interval.
    pub fn drift_within_guard(&self) -> bool {
        let drift = self.max_drift_us();
        let guard = self.slots.first().map_or(0, |s| s.guard_interval.as_micros() as u64);
        drift < guard as f64
    }
}

/// Event emitted when a TDM slot completes.
///
/// Published by the `TdmCoordinator` after a node finishes its TX window
/// and the guard interval elapses. Listeners (e.g., the aggregator) use
/// this to know when CSI data from this slot is expected to arrive.
#[derive(Debug, Clone)]
pub struct TdmSlotCompleted {
    /// The cycle number (monotonically increasing from coordinator start).
    pub cycle_id: u64,
    /// The slot index within the cycle that completed.
    pub slot_index: usize,
    /// The node that was transmitting.
    pub tx_node_id: u8,
    /// Quality metric: fraction of expected CSI frames actually received (0.0-1.0).
    pub capture_quality: f32,
    /// Timestamp when the slot completed.
    pub completed_at: Instant,
}

/// Sync beacon broadcast by the coordinator at the start of each TDM cycle.
///
/// All nodes use the beacon timestamp to align their local clocks and
/// determine when their TX slot begins. The `drift_correction_us` field
/// allows the coordinator to compensate for cumulative crystal drift.
///
/// # Wire format (planned)
///
/// The beacon is a short UDP broadcast (16 bytes):
/// ```text
/// [0..7]   cycle_id (LE u64)
/// [8..11]  cycle_period_us (LE u32)
/// [12..13] drift_correction_us (LE i16)
/// [14..15] reserved
/// ```
#[derive(Debug, Clone)]
pub struct SyncBeacon {
    /// Monotonically increasing cycle identifier.
    pub cycle_id: u64,
    /// Expected cycle period (from the schedule).
    pub cycle_period: Duration,
    /// Signed drift correction offset in microseconds.
    ///
    /// Positive values mean nodes should start their slot slightly later;
    /// negative means earlier. Derived from observed arrival-time deviations.
    pub drift_correction_us: i16,
    /// Timestamp when the beacon was generated.
    pub generated_at: Instant,
}

impl SyncBeacon {
    /// Serialize the beacon to the 16-byte wire format.
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.cycle_id.to_le_bytes());
        let period_us = self.cycle_period.as_micros() as u32;
        buf[8..12].copy_from_slice(&period_us.to_le_bytes());
        buf[12..14].copy_from_slice(&self.drift_correction_us.to_le_bytes());
        // [14..15] reserved = 0
        buf
    }

    /// Deserialize a beacon from the 16-byte wire format.
    ///
    /// Returns `None` if the buffer is too short.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < 16 {
            return None;
        }
        let cycle_id = u64::from_le_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]);
        let period_us = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let drift_correction_us = i16::from_le_bytes([buf[12], buf[13]]);

        Some(Self {
            cycle_id,
            cycle_period: Duration::from_micros(period_us as u64),
            drift_correction_us,
            generated_at: Instant::now(),
        })
    }
}

/// TDM sensing cycle coordinator.
///
/// Manages the state machine for multistatic sensing cycles. The coordinator
/// runs on the aggregator node and tracks:
///
/// - Current cycle ID and active slot
/// - Which nodes have reported CSI data for the current cycle
/// - Cumulative clock drift for compensation
///
/// # Usage
///
/// ```
/// use wifi_densepose_hardware::esp32::{TdmSchedule, TdmCoordinator};
///
/// let schedule = TdmSchedule::default_4node();
/// let mut coordinator = TdmCoordinator::new(schedule);
///
/// // Start a new sensing cycle
/// let beacon = coordinator.begin_cycle();
/// assert_eq!(beacon.cycle_id, 0);
///
/// // Complete each slot in the 4-node schedule
/// for i in 0..4 {
///     let event = coordinator.complete_slot(i, 0.95);
///     assert_eq!(event.slot_index, i);
/// }
///
/// // After all slots, the cycle is complete
/// assert!(coordinator.is_cycle_complete());
/// ```
#[derive(Debug)]
pub struct TdmCoordinator {
    /// The schedule governing slot assignments and timing.
    schedule: TdmSchedule,
    /// Current cycle number (incremented on each `begin_cycle`).
    cycle_id: u64,
    /// Index of the next slot expected to complete (0..node_count).
    next_slot: usize,
    /// Whether a cycle is currently in progress.
    cycle_active: bool,
    /// Per-node received flags for the current cycle.
    received: Vec<bool>,
    /// Cumulative observed drift in microseconds (for drift compensation).
    cumulative_drift_us: f64,
    /// Timestamp of the last cycle start (for drift measurement).
    last_cycle_start: Option<Instant>,
}

impl TdmCoordinator {
    /// Create a new coordinator with the given schedule.
    pub fn new(schedule: TdmSchedule) -> Self {
        let n = schedule.node_count();
        Self {
            schedule,
            cycle_id: 0,
            next_slot: 0,
            cycle_active: false,
            received: vec![false; n],
            cumulative_drift_us: 0.0,
            last_cycle_start: None,
        }
    }

    /// Begin a new sensing cycle. Returns the sync beacon to broadcast.
    ///
    /// This resets per-slot tracking and increments the cycle ID (except
    /// for the very first cycle, which starts at 0).
    pub fn begin_cycle(&mut self) -> SyncBeacon {
        if self.cycle_active {
            // Auto-finalize the previous cycle
            self.cycle_active = false;
        }

        if self.last_cycle_start.is_some() {
            self.cycle_id += 1;
        }

        self.next_slot = 0;
        self.cycle_active = true;
        for flag in &mut self.received {
            *flag = false;
        }

        // Measure drift from the previous cycle
        let now = Instant::now();
        if let Some(prev) = self.last_cycle_start {
            let actual_us = now.duration_since(prev).as_micros() as f64;
            let expected_us = self.schedule.cycle_period().as_micros() as f64;
            let drift = actual_us - expected_us;
            self.cumulative_drift_us += drift;
        }
        self.last_cycle_start = Some(now);

        // Compute drift correction: negative of cumulative drift, clamped to i16
        let correction = (-self.cumulative_drift_us)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;

        SyncBeacon {
            cycle_id: self.cycle_id,
            cycle_period: self.schedule.cycle_period(),
            drift_correction_us: correction,
            generated_at: now,
        }
    }

    /// Mark a slot as completed and return the completion event.
    ///
    /// # Arguments
    ///
    /// * `slot_index` - The slot that completed (must match `next_slot`)
    /// * `capture_quality` - Fraction of expected CSI frames received (0.0-1.0)
    ///
    /// # Panics
    ///
    /// Does not panic. Returns a `TdmSlotCompleted` event even if the slot
    /// index is unexpected (the coordinator is lenient to allow out-of-order
    /// completions in degraded conditions).
    pub fn complete_slot(&mut self, slot_index: usize, capture_quality: f32) -> TdmSlotCompleted {
        let quality = capture_quality.clamp(0.0, 1.0);
        let tx_node_id = self
            .schedule
            .slot(slot_index)
            .map(|s| s.tx_node_id)
            .unwrap_or(0);

        if slot_index < self.received.len() {
            self.received[slot_index] = true;
        }

        if slot_index == self.next_slot {
            self.next_slot += 1;
        }

        TdmSlotCompleted {
            cycle_id: self.cycle_id,
            slot_index,
            tx_node_id,
            capture_quality: quality,
            completed_at: Instant::now(),
        }
    }

    /// Check whether all slots in the current cycle have completed.
    pub fn is_cycle_complete(&self) -> bool {
        self.received.iter().all(|&r| r)
    }

    /// Number of slots that have completed in the current cycle.
    pub fn completed_slot_count(&self) -> usize {
        self.received.iter().filter(|&&r| r).count()
    }

    /// Current cycle ID.
    pub fn cycle_id(&self) -> u64 {
        self.cycle_id
    }

    /// Whether a cycle is currently active.
    pub fn is_active(&self) -> bool {
        self.cycle_active
    }

    /// Reference to the underlying schedule.
    pub fn schedule(&self) -> &TdmSchedule {
        &self.schedule
    }

    /// Current cumulative drift estimate in microseconds.
    pub fn cumulative_drift_us(&self) -> f64 {
        self.cumulative_drift_us
    }

    /// Compute the maximum single-cycle drift for this schedule.
    ///
    /// Based on ESP32 crystal spec of +/-10 ppm.
    pub fn max_single_cycle_drift_us(&self) -> f64 {
        self.schedule.max_drift_us()
    }

    /// Generate a sync beacon for the current cycle without starting a new one.
    ///
    /// Useful for re-broadcasting the beacon if a node missed it.
    pub fn current_beacon(&self) -> SyncBeacon {
        let correction = (-self.cumulative_drift_us)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;

        SyncBeacon {
            cycle_id: self.cycle_id,
            cycle_period: self.schedule.cycle_period(),
            drift_correction_us: correction,
            generated_at: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TdmSchedule tests ----

    #[test]
    fn test_default_4node_schedule() {
        let schedule = TdmSchedule::default_4node();
        assert_eq!(schedule.node_count(), 4);
        // 4 slots * (4ms + 1ms guard) + 30ms processing = 50ms
        assert_eq!(schedule.cycle_period().as_millis(), 50);
        assert_eq!(schedule.update_rate_hz(), 20.0);
        assert!(schedule.drift_within_guard());
    }

    #[test]
    fn test_uniform_schedule_timing() {
        let schedule = TdmSchedule::uniform(
            &[10, 20, 30],
            Duration::from_millis(5),
            Duration::from_micros(500),
            Duration::from_millis(20),
        )
        .unwrap();

        assert_eq!(schedule.node_count(), 3);
        // 3 * (5ms + 0.5ms) + 20ms = 16.5 + 20 = 36.5ms
        let expected_us: u64 = 3 * (5_000 + 500) + 20_000;
        assert_eq!(schedule.cycle_period().as_micros() as u64, expected_us);
    }

    #[test]
    fn test_slot_for_node() {
        let schedule = TdmSchedule::uniform(
            &[5, 10, 15],
            Duration::from_millis(4),
            Duration::from_micros(1_000),
            Duration::from_millis(30),
        )
        .unwrap();

        let slot = schedule.slot_for_node(10).unwrap();
        assert_eq!(slot.index, 1);
        assert_eq!(slot.tx_node_id, 10);

        assert!(schedule.slot_for_node(99).is_none());
    }

    #[test]
    fn test_slot_start_offset() {
        let schedule = TdmSchedule::uniform(
            &[0, 1, 2, 3],
            Duration::from_millis(4),
            Duration::from_micros(1_000),
            Duration::from_millis(30),
        )
        .unwrap();

        // Slot 0 starts at 0
        let offset0 = TdmSlot::start_offset(schedule.slots(), 0).unwrap();
        assert_eq!(offset0, Duration::ZERO);

        // Slot 1 starts at 4ms + 1ms = 5ms
        let offset1 = TdmSlot::start_offset(schedule.slots(), 1).unwrap();
        assert_eq!(offset1.as_micros(), 5_000);

        // Slot 2 starts at 2 * 5ms = 10ms
        let offset2 = TdmSlot::start_offset(schedule.slots(), 2).unwrap();
        assert_eq!(offset2.as_micros(), 10_000);

        // Out of bounds returns None
        assert!(TdmSlot::start_offset(schedule.slots(), 10).is_none());
    }

    #[test]
    fn test_empty_node_list_rejected() {
        let result = TdmSchedule::uniform(
            &[],
            Duration::from_millis(4),
            Duration::from_micros(1_000),
            Duration::from_millis(30),
        );
        assert_eq!(
            result.unwrap_err(),
            TdmError::InvalidNodeCount { count: 0, max: MAX_NODES }
        );
    }

    #[test]
    fn test_too_many_nodes_rejected() {
        let ids: Vec<u8> = (0..=MAX_NODES as u8).collect();
        let result = TdmSchedule::uniform(
            &ids,
            Duration::from_millis(4),
            Duration::from_micros(1_000),
            Duration::from_millis(30),
        );
        assert!(matches!(result, Err(TdmError::InvalidNodeCount { .. })));
    }

    #[test]
    fn test_guard_interval_too_large() {
        let result = TdmSchedule::uniform(
            &[0, 1],
            Duration::from_millis(1),       // 1 ms slot
            Duration::from_millis(2),        // 2 ms guard > slot
            Duration::from_millis(30),
        );
        assert!(matches!(result, Err(TdmError::GuardIntervalTooLarge { .. })));
    }

    #[test]
    fn test_max_drift_calculation() {
        let schedule = TdmSchedule::default_4node();
        let drift = schedule.max_drift_us();
        // 10 ppm * 50ms = 0.5 us
        assert!((drift - 0.5).abs() < 0.01);
    }

    // ---- SyncBeacon tests ----

    #[test]
    fn test_sync_beacon_roundtrip() {
        let beacon = SyncBeacon {
            cycle_id: 42,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: -3,
            generated_at: Instant::now(),
        };

        let bytes = beacon.to_bytes();
        assert_eq!(bytes.len(), 16);

        let decoded = SyncBeacon::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.cycle_id, 42);
        assert_eq!(decoded.cycle_period, Duration::from_millis(50));
        assert_eq!(decoded.drift_correction_us, -3);
    }

    #[test]
    fn test_sync_beacon_short_buffer() {
        assert!(SyncBeacon::from_bytes(&[0u8; 10]).is_none());
    }

    #[test]
    fn test_sync_beacon_zero_drift() {
        let beacon = SyncBeacon {
            cycle_id: 0,
            cycle_period: Duration::from_millis(50),
            drift_correction_us: 0,
            generated_at: Instant::now(),
        };
        let bytes = beacon.to_bytes();
        let decoded = SyncBeacon::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.drift_correction_us, 0);
    }

    // ---- TdmCoordinator tests ----

    #[test]
    fn test_coordinator_begin_cycle() {
        let schedule = TdmSchedule::default_4node();
        let mut coord = TdmCoordinator::new(schedule);

        let beacon = coord.begin_cycle();
        assert_eq!(beacon.cycle_id, 0);
        assert!(coord.is_active());
        assert!(!coord.is_cycle_complete());
        assert_eq!(coord.completed_slot_count(), 0);
    }

    #[test]
    fn test_coordinator_complete_all_slots() {
        let schedule = TdmSchedule::default_4node();
        let mut coord = TdmCoordinator::new(schedule);
        coord.begin_cycle();

        for i in 0..4 {
            assert!(!coord.is_cycle_complete());
            let event = coord.complete_slot(i, 0.95);
            assert_eq!(event.cycle_id, 0);
            assert_eq!(event.slot_index, i);
        }

        assert!(coord.is_cycle_complete());
        assert_eq!(coord.completed_slot_count(), 4);
    }

    #[test]
    fn test_coordinator_cycle_id_increments() {
        let schedule = TdmSchedule::default_4node();
        let mut coord = TdmCoordinator::new(schedule);

        let b0 = coord.begin_cycle();
        assert_eq!(b0.cycle_id, 0);

        // Complete all slots
        for i in 0..4 {
            coord.complete_slot(i, 1.0);
        }

        let b1 = coord.begin_cycle();
        assert_eq!(b1.cycle_id, 1);

        for i in 0..4 {
            coord.complete_slot(i, 1.0);
        }

        let b2 = coord.begin_cycle();
        assert_eq!(b2.cycle_id, 2);
    }

    #[test]
    fn test_coordinator_capture_quality_clamped() {
        let schedule = TdmSchedule::default_4node();
        let mut coord = TdmCoordinator::new(schedule);
        coord.begin_cycle();

        let event = coord.complete_slot(0, 1.5);
        assert_eq!(event.capture_quality, 1.0);

        let event = coord.complete_slot(1, -0.5);
        assert_eq!(event.capture_quality, 0.0);
    }

    #[test]
    fn test_coordinator_current_beacon() {
        let schedule = TdmSchedule::default_4node();
        let mut coord = TdmCoordinator::new(schedule);
        coord.begin_cycle();

        let beacon = coord.current_beacon();
        assert_eq!(beacon.cycle_id, 0);
        assert_eq!(beacon.cycle_period.as_millis(), 50);
    }

    #[test]
    fn test_coordinator_drift_starts_at_zero() {
        let schedule = TdmSchedule::default_4node();
        let coord = TdmCoordinator::new(schedule);
        assert_eq!(coord.cumulative_drift_us(), 0.0);
    }

    #[test]
    fn test_coordinator_max_single_cycle_drift() {
        let schedule = TdmSchedule::default_4node();
        let coord = TdmCoordinator::new(schedule);
        // 10 ppm * 50ms = 0.5 us
        let drift = coord.max_single_cycle_drift_us();
        assert!((drift - 0.5).abs() < 0.01);
    }
}
