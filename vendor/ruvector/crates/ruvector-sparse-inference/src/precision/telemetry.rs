//! Telemetry and statistics for precision lanes
//!
//! Tracks lane usage, transitions, and performance metrics.

use super::lanes::PrecisionLane;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Statistics for a single precision lane
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaneStats {
    /// Total operations in this lane
    pub operations: u64,

    /// Total time spent in this lane (nanoseconds)
    pub total_time_ns: u64,

    /// Average operation time (nanoseconds)
    pub avg_time_ns: u64,

    /// Peak operation time (nanoseconds)
    pub peak_time_ns: u64,

    /// Total bytes processed
    pub bytes_processed: u64,

    /// Average active set size
    pub avg_active_set_size: f32,

    /// Error count
    pub errors: u64,

    /// Escalations from this lane
    pub escalations: u64,

    /// Demotions to this lane
    pub demotions: u64,
}

impl LaneStats {
    /// Record a new operation
    pub fn record_operation(&mut self, duration_ns: u64, bytes: u64, active_set_size: usize) {
        self.operations += 1;
        self.total_time_ns += duration_ns;
        self.bytes_processed += bytes;

        // Update average
        let ops = self.operations as f32;
        self.avg_time_ns = (self.total_time_ns / self.operations) as u64;
        self.avg_active_set_size =
            (self.avg_active_set_size * (ops - 1.0) + active_set_size as f32) / ops;

        // Update peak
        if duration_ns > self.peak_time_ns {
            self.peak_time_ns = duration_ns;
        }
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    /// Record an escalation from this lane
    pub fn record_escalation(&mut self) {
        self.escalations += 1;
    }

    /// Record a demotion to this lane
    pub fn record_demotion(&mut self) {
        self.demotions += 1;
    }

    /// Get throughput in bytes per second
    pub fn throughput_bps(&self) -> f64 {
        if self.total_time_ns == 0 {
            return 0.0;
        }
        (self.bytes_processed as f64 * 1_000_000_000.0) / self.total_time_ns as f64
    }
}

/// Comprehensive telemetry for all precision lanes
#[derive(Debug, Clone)]
pub struct LaneTelemetry {
    /// Per-lane statistics
    pub lane_stats: HashMap<PrecisionLane, LaneStats>,

    /// Current lane
    pub current_lane: PrecisionLane,

    /// Total lane transitions
    pub transitions: u64,

    /// Transition history (recent 100)
    transition_history: Vec<LaneTransition>,

    /// Start time
    start_time: Option<Instant>,

    /// Session duration (seconds)
    pub session_duration_secs: f64,
}

/// Record of a lane transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneTransition {
    /// Source lane
    pub from: PrecisionLane,

    /// Destination lane
    pub to: PrecisionLane,

    /// Reason for transition
    pub reason: TransitionReason,

    /// Timestamp (seconds since session start)
    pub timestamp_secs: f64,
}

/// Reason for lane transition
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TransitionReason {
    /// Novelty threshold exceeded
    Novelty,
    /// Drift persisted
    DriftPersistence,
    /// Stability returned
    StabilityReturned,
    /// Velocity stalled
    VelocityStalled,
    /// Active set shrunk
    ActiveSetShrunk,
    /// Manual override
    Manual,
    /// Initialization
    Init,
}

impl LaneTelemetry {
    /// Create new telemetry tracker
    pub fn new(initial_lane: PrecisionLane) -> Self {
        let mut lane_stats = HashMap::new();
        lane_stats.insert(PrecisionLane::Bit3, LaneStats::default());
        lane_stats.insert(PrecisionLane::Bit5, LaneStats::default());
        lane_stats.insert(PrecisionLane::Bit7, LaneStats::default());
        lane_stats.insert(PrecisionLane::Float32, LaneStats::default());

        Self {
            lane_stats,
            current_lane: initial_lane,
            transitions: 0,
            transition_history: Vec::with_capacity(100),
            start_time: Some(Instant::now()),
            session_duration_secs: 0.0,
        }
    }

    /// Start a new session
    pub fn start_session(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// Record an operation in the current lane
    pub fn record_operation(&mut self, duration: Duration, bytes: u64, active_set_size: usize) {
        let duration_ns = duration.as_nanos() as u64;

        if let Some(stats) = self.lane_stats.get_mut(&self.current_lane) {
            stats.record_operation(duration_ns, bytes, active_set_size);
        }

        // Update session duration
        if let Some(start) = self.start_time {
            self.session_duration_secs = start.elapsed().as_secs_f64();
        }
    }

    /// Record a lane transition
    pub fn record_transition(
        &mut self,
        from: PrecisionLane,
        to: PrecisionLane,
        reason: TransitionReason,
    ) {
        self.transitions += 1;
        self.current_lane = to;

        // Record escalation/demotion in stats
        if to.bits() > from.bits() {
            if let Some(stats) = self.lane_stats.get_mut(&from) {
                stats.record_escalation();
            }
        } else {
            if let Some(stats) = self.lane_stats.get_mut(&to) {
                stats.record_demotion();
            }
        }

        // Add to history
        let timestamp_secs = self
            .start_time
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        let transition = LaneTransition {
            from,
            to,
            reason,
            timestamp_secs,
        };

        if self.transition_history.len() >= 100 {
            self.transition_history.remove(0);
        }
        self.transition_history.push(transition);
    }

    /// Record an error in the current lane
    pub fn record_error(&mut self) {
        if let Some(stats) = self.lane_stats.get_mut(&self.current_lane) {
            stats.record_error();
        }
    }

    /// Get statistics for a specific lane
    pub fn get_lane_stats(&self, lane: PrecisionLane) -> Option<&LaneStats> {
        self.lane_stats.get(&lane)
    }

    /// Get total operations across all lanes
    pub fn total_operations(&self) -> u64 {
        self.lane_stats.values().map(|s| s.operations).sum()
    }

    /// Get total errors across all lanes
    pub fn total_errors(&self) -> u64 {
        self.lane_stats.values().map(|s| s.errors).sum()
    }

    /// Get lane usage distribution (percentage)
    pub fn lane_distribution(&self) -> HashMap<PrecisionLane, f32> {
        let total = self.total_operations() as f32;
        if total == 0.0 {
            return HashMap::new();
        }

        self.lane_stats
            .iter()
            .map(|(lane, stats)| (*lane, (stats.operations as f32 / total) * 100.0))
            .collect()
    }

    /// Get transition history
    pub fn transition_history(&self) -> &[LaneTransition] {
        &self.transition_history
    }

    /// Generate summary report
    pub fn summary_report(&self) -> TelemetrySummary {
        TelemetrySummary {
            session_duration_secs: self.session_duration_secs,
            total_operations: self.total_operations(),
            total_transitions: self.transitions,
            total_errors: self.total_errors(),
            lane_distribution: self.lane_distribution(),
            avg_operations_per_sec: if self.session_duration_secs > 0.0 {
                self.total_operations() as f64 / self.session_duration_secs
            } else {
                0.0
            },
            current_lane: self.current_lane,
        }
    }
}

/// Summary of telemetry data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySummary {
    pub session_duration_secs: f64,
    pub total_operations: u64,
    pub total_transitions: u64,
    pub total_errors: u64,
    pub lane_distribution: HashMap<PrecisionLane, f32>,
    pub avg_operations_per_sec: f64,
    pub current_lane: PrecisionLane,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_stats_recording() {
        let mut stats = LaneStats::default();

        stats.record_operation(1000, 64, 100);
        stats.record_operation(2000, 64, 100);

        assert_eq!(stats.operations, 2);
        assert_eq!(stats.total_time_ns, 3000);
        assert_eq!(stats.avg_time_ns, 1500);
        assert_eq!(stats.bytes_processed, 128);
    }

    #[test]
    fn test_telemetry_transitions() {
        let mut telemetry = LaneTelemetry::new(PrecisionLane::Bit5);

        telemetry.record_transition(
            PrecisionLane::Bit5,
            PrecisionLane::Bit7,
            TransitionReason::Novelty,
        );

        assert_eq!(telemetry.transitions, 1);
        assert_eq!(telemetry.current_lane, PrecisionLane::Bit7);
        assert_eq!(telemetry.transition_history.len(), 1);
    }

    #[test]
    fn test_lane_distribution() {
        let mut telemetry = LaneTelemetry::new(PrecisionLane::Bit5);

        // Simulate operations in different lanes
        for _ in 0..30 {
            telemetry.current_lane = PrecisionLane::Bit3;
            telemetry.record_operation(Duration::from_nanos(100), 8, 10);
        }
        for _ in 0..50 {
            telemetry.current_lane = PrecisionLane::Bit5;
            telemetry.record_operation(Duration::from_nanos(200), 16, 50);
        }
        for _ in 0..20 {
            telemetry.current_lane = PrecisionLane::Bit7;
            telemetry.record_operation(Duration::from_nanos(500), 32, 100);
        }

        let distribution = telemetry.lane_distribution();

        assert!((distribution[&PrecisionLane::Bit3] - 30.0).abs() < 0.1);
        assert!((distribution[&PrecisionLane::Bit5] - 50.0).abs() < 0.1);
        assert!((distribution[&PrecisionLane::Bit7] - 20.0).abs() < 0.1);
    }
}
