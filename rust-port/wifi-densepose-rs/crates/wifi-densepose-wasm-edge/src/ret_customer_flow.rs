//! Customer flow counting — ADR-041 Category 4: Retail & Hospitality.
//!
//! Directional foot traffic counting using asymmetric phase gradient analysis.
//! Maintains running ingress/egress counts and computes net occupancy (in - out).
//! Handles simultaneous bidirectional traffic via per-subcarrier-group gradient
//! decomposition.
//!
//! Events (420-series):
//! - `INGRESS(420)`:           Person entered (cumulative count)
//! - `EGRESS(421)`:            Person exited (cumulative count)
//! - `NET_OCCUPANCY(422)`:     Net occupancy (ingress - egress)
//! - `HOURLY_TRAFFIC(423)`:    Hourly traffic summary
//!
//! Host API used: phase, amplitude, variance, motion energy.

use crate::vendor_common::{CircularBuffer, Ema};

#[cfg(not(feature = "std"))]
use libm::{fabsf, sqrtf};
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

// ── Event IDs ─────────────────────────────────────────────────────────────────

pub const EVENT_INGRESS: i32 = 420;
pub const EVENT_EGRESS: i32 = 421;
pub const EVENT_NET_OCCUPANCY: i32 = 422;
pub const EVENT_HOURLY_TRAFFIC: i32 = 423;

// ── Configuration constants ──────────────────────────────────────────────────

/// Maximum subcarriers.
const MAX_SC: usize = 32;

/// Frame rate assumption (Hz).
const FRAME_RATE: f32 = 20.0;

/// Frames per hour (at 20 Hz).
const FRAMES_PER_HOUR: u32 = 72000;

/// Number of subcarrier groups for directional analysis.
/// We split subcarriers into LOW (near side) and HIGH (far side).
const NUM_GROUPS: usize = 2;

/// Minimum phase gradient magnitude to detect directional movement.
const PHASE_GRADIENT_THRESH: f32 = 0.15;

/// Motion energy threshold for a valid crossing event.
const MOTION_THRESH: f32 = 0.03;

/// Amplitude spike threshold for crossing detection.
const AMPLITUDE_SPIKE_THRESH: f32 = 1.5;

/// Debounce frames between crossing events (prevents double-counting).
const CROSSING_DEBOUNCE: u8 = 10;

/// EMA alpha for gradient smoothing.
const GRADIENT_EMA_ALPHA: f32 = 0.2;

/// Phase gradient history depth (1 second at 20 Hz).
const GRADIENT_HISTORY: usize = 20;

/// Report interval for net occupancy (every ~5 seconds).
const OCCUPANCY_REPORT_INTERVAL: u32 = 100;

/// Maximum events per frame.
const MAX_EVENTS: usize = 4;

// ── Customer Flow Tracker ───────────────────────────────────────────────────

/// Tracks directional foot traffic using phase gradient analysis.
pub struct CustomerFlowTracker {
    /// Previous phase values per subcarrier.
    prev_phases: [f32; MAX_SC],
    /// Previous amplitude values per subcarrier.
    prev_amplitudes: [f32; MAX_SC],
    /// Phase gradient EMA (positive = ingress direction, negative = egress).
    gradient_ema: Ema,
    /// Gradient history for peak detection.
    gradient_history: CircularBuffer<GRADIENT_HISTORY>,
    /// Cumulative ingress count.
    ingress_count: u32,
    /// Cumulative egress count.
    egress_count: u32,
    /// Hourly ingress accumulator.
    hourly_ingress: u32,
    /// Hourly egress accumulator.
    hourly_egress: u32,
    /// Debounce counter (frames since last crossing event).
    debounce_counter: u8,
    /// Whether previous phases have been initialized.
    phase_init: bool,
    /// Frame counter.
    frame_count: u32,
    /// Number of subcarriers seen last frame.
    n_sc: usize,
}

impl CustomerFlowTracker {
    pub const fn new() -> Self {
        Self {
            prev_phases: [0.0; MAX_SC],
            prev_amplitudes: [0.0; MAX_SC],
            gradient_ema: Ema::new(GRADIENT_EMA_ALPHA),
            gradient_history: CircularBuffer::new(),
            ingress_count: 0,
            egress_count: 0,
            hourly_ingress: 0,
            hourly_egress: 0,
            debounce_counter: 0,
            phase_init: false,
            frame_count: 0,
            n_sc: 0,
        }
    }

    /// Process one CSI frame with per-subcarrier phase and amplitude data.
    ///
    /// - `phases`: per-subcarrier unwrapped phase values
    /// - `amplitudes`: per-subcarrier amplitude values
    /// - `variance`: mean subcarrier variance
    /// - `motion_energy`: aggregate motion energy from Tier 2
    ///
    /// Returns event slice `&[(event_type, value)]`.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        _variance: f32,
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        let n_sc = phases.len().min(amplitudes.len()).min(MAX_SC);
        if n_sc < 4 {
            // Need at least 4 subcarriers for directional analysis.
            if !self.phase_init {
                for i in 0..n_sc {
                    self.prev_phases[i] = phases[i];
                    self.prev_amplitudes[i] = amplitudes[i];
                }
                self.phase_init = true;
                self.n_sc = n_sc;
            }
            return &[];
        }
        self.n_sc = n_sc;

        if self.debounce_counter > 0 {
            self.debounce_counter -= 1;
        }

        // Initialize previous phases on first frame.
        if !self.phase_init {
            for i in 0..n_sc {
                self.prev_phases[i] = phases[i];
                self.prev_amplitudes[i] = amplitudes[i];
            }
            self.phase_init = true;
            return &[];
        }

        // Compute directional phase gradient.
        // Split subcarriers into two groups: low (near entrance) and high (far side).
        let mid = n_sc / 2;

        let mut low_gradient = 0.0f32;
        let mut high_gradient = 0.0f32;

        // Phase velocity per group.
        for i in 0..mid {
            low_gradient += phases[i] - self.prev_phases[i];
        }
        for i in mid..n_sc {
            high_gradient += phases[i] - self.prev_phases[i];
        }

        low_gradient /= mid as f32;
        high_gradient /= (n_sc - mid) as f32;

        // Directional gradient: asymmetric difference between groups.
        // Positive = movement from low to high (ingress).
        // Negative = movement from high to low (egress).
        let directional_gradient = low_gradient - high_gradient;
        let smoothed = self.gradient_ema.update(directional_gradient);
        self.gradient_history.push(smoothed);

        // Amplitude change detection (crossing produces a characteristic pulse).
        let mut amp_change = 0.0f32;
        for i in 0..n_sc {
            amp_change += fabsf(amplitudes[i] - self.prev_amplitudes[i]);
        }
        amp_change /= n_sc as f32;

        // Update previous values.
        for i in 0..n_sc {
            self.prev_phases[i] = phases[i];
            self.prev_amplitudes[i] = amplitudes[i];
        }

        // Build events.
        static mut EVENTS: [(i32, f32); MAX_EVENTS] = [(0, 0.0); MAX_EVENTS];
        let mut ne = 0usize;

        // Crossing detection: look for gradient peak + motion + amplitude spike.
        let gradient_mag = fabsf(smoothed);
        let is_crossing = gradient_mag > PHASE_GRADIENT_THRESH
            && motion_energy > MOTION_THRESH
            && amp_change > AMPLITUDE_SPIKE_THRESH * 0.1
            && self.debounce_counter == 0;

        if is_crossing {
            self.debounce_counter = CROSSING_DEBOUNCE;

            if smoothed > 0.0 {
                // Ingress detected.
                self.ingress_count += 1;
                self.hourly_ingress += 1;
                if ne < MAX_EVENTS {
                    unsafe {
                        EVENTS[ne] = (EVENT_INGRESS, self.ingress_count as f32);
                    }
                    ne += 1;
                }
            } else {
                // Egress detected.
                self.egress_count += 1;
                self.hourly_egress += 1;
                if ne < MAX_EVENTS {
                    unsafe {
                        EVENTS[ne] = (EVENT_EGRESS, self.egress_count as f32);
                    }
                    ne += 1;
                }
            }

            // Emit net occupancy on each crossing.
            let net = self.net_occupancy();
            if ne < MAX_EVENTS {
                unsafe {
                    EVENTS[ne] = (EVENT_NET_OCCUPANCY, net as f32);
                }
                ne += 1;
            }
        }

        // Periodic net occupancy report.
        if self.frame_count % OCCUPANCY_REPORT_INTERVAL == 0 && ne < MAX_EVENTS {
            let net = self.net_occupancy();
            unsafe {
                EVENTS[ne] = (EVENT_NET_OCCUPANCY, net as f32);
            }
            ne += 1;
        }

        // Hourly traffic summary.
        if self.frame_count % FRAMES_PER_HOUR == 0 && self.frame_count > 0 {
            // Encode: ingress * 1000 + egress.
            let summary = self.hourly_ingress as f32 * 1000.0 + self.hourly_egress as f32;
            if ne < MAX_EVENTS {
                unsafe {
                    EVENTS[ne] = (EVENT_HOURLY_TRAFFIC, summary);
                }
                ne += 1;
            }
            self.hourly_ingress = 0;
            self.hourly_egress = 0;
        }

        unsafe { &EVENTS[..ne] }
    }

    /// Get net occupancy (ingress - egress), clamped to 0.
    pub fn net_occupancy(&self) -> i32 {
        let net = self.ingress_count as i32 - self.egress_count as i32;
        if net < 0 { 0 } else { net }
    }

    /// Get total ingress count.
    pub fn total_ingress(&self) -> u32 {
        self.ingress_count
    }

    /// Get total egress count.
    pub fn total_egress(&self) -> u32 {
        self.egress_count
    }

    /// Get current smoothed directional gradient.
    pub fn current_gradient(&self) -> f32 {
        self.gradient_ema.value
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_init_state() {
        let cf = CustomerFlowTracker::new();
        assert_eq!(cf.total_ingress(), 0);
        assert_eq!(cf.total_egress(), 0);
        assert_eq!(cf.net_occupancy(), 0);
        assert_eq!(cf.frame_count, 0);
    }

    #[test]
    fn test_too_few_subcarriers() {
        let mut cf = CustomerFlowTracker::new();
        let phases = [0.0f32; 2];
        let amps = [1.0f32; 2];
        let events = cf.process_frame(&phases, &amps, 0.0, 0.0);
        // Should return empty (not enough subcarriers).
        assert!(events.is_empty() || cf.total_ingress() == 0);
    }

    #[test]
    fn test_ingress_detection() {
        let mut cf = CustomerFlowTracker::new();
        let amps = [1.0f32; 16];

        // First frame: initialize phases.
        let phases_init = [0.0f32; 16];
        cf.process_frame(&phases_init, &amps, 0.0, 0.0);

        // Simulate ingress: low subcarriers lead in phase (positive gradient).
        let mut ingress_detected = false;
        for frame in 0..30 {
            let mut phases = [0.0f32; 16];
            // Low subcarriers: advancing phase.
            for i in 0..8 {
                phases[i] = 0.5 * (frame as f32 + 1.0);
            }
            // High subcarriers: lagging phase.
            for i in 8..16 {
                phases[i] = 0.1 * (frame as f32 + 1.0);
            }

            let mut amps_frame = [1.0f32; 16];
            // Amplitude spike.
            for i in 0..16 {
                amps_frame[i] = 1.0 + 0.3 * ((frame % 3) as f32);
            }

            let events = cf.process_frame(&phases, &amps_frame, 0.05, 0.1);
            for &(et, _) in events {
                if et == EVENT_INGRESS {
                    ingress_detected = true;
                }
            }
        }

        assert!(ingress_detected, "ingress should be detected from positive phase gradient");
    }

    #[test]
    fn test_egress_detection() {
        let mut cf = CustomerFlowTracker::new();
        let amps = [1.0f32; 16];
        let phases_init = [0.0f32; 16];
        cf.process_frame(&phases_init, &amps, 0.0, 0.0);

        // Simulate egress: high subcarriers lead (negative gradient).
        let mut egress_detected = false;
        for frame in 0..30 {
            let mut phases = [0.0f32; 16];
            // Low subcarriers: lagging.
            for i in 0..8 {
                phases[i] = 0.05 * (frame as f32 + 1.0);
            }
            // High subcarriers: advancing.
            for i in 8..16 {
                phases[i] = 0.5 * (frame as f32 + 1.0);
            }

            let mut amps_frame = [1.0f32; 16];
            for i in 0..16 {
                amps_frame[i] = 1.0 + 0.3 * ((frame % 3) as f32);
            }

            let events = cf.process_frame(&phases, &amps_frame, 0.05, 0.1);
            for &(et, _) in events {
                if et == EVENT_EGRESS {
                    egress_detected = true;
                }
            }
        }

        assert!(egress_detected, "egress should be detected from negative phase gradient");
    }

    #[test]
    fn test_net_occupancy_clamped_to_zero() {
        let mut cf = CustomerFlowTracker::new();
        // Manually set egress > ingress.
        cf.egress_count = 5;
        cf.ingress_count = 2;
        assert_eq!(cf.net_occupancy(), 0, "net occupancy should not go negative");
    }

    #[test]
    fn test_periodic_occupancy_report() {
        let mut cf = CustomerFlowTracker::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        let mut occupancy_reported = false;
        for _ in 0..OCCUPANCY_REPORT_INTERVAL + 1 {
            let events = cf.process_frame(&phases, &amps, 0.0, 0.0);
            for &(et, _) in events {
                if et == EVENT_NET_OCCUPANCY {
                    occupancy_reported = true;
                }
            }
        }
        assert!(occupancy_reported, "periodic occupancy should be reported");
    }

    #[test]
    fn test_debounce_prevents_double_count() {
        let mut cf = CustomerFlowTracker::new();
        // Initialize.
        let phases_init = [0.0f32; 16];
        let amps = [1.0f32; 16];
        cf.process_frame(&phases_init, &amps, 0.0, 0.0);

        // Force a crossing.
        cf.debounce_counter = 0;
        let mut ingress_count = 0u32;

        // Two rapid frames with strong gradient — only one should count due to debounce.
        for frame in 0..2 {
            let mut phases = [0.0f32; 16];
            for i in 0..8 {
                phases[i] = 2.0 * (frame as f32 + 1.0);
            }
            let events = cf.process_frame(&phases, &amps, 0.1, 0.2);
            for &(et, _) in events {
                if et == EVENT_INGRESS {
                    ingress_count += 1;
                }
            }
        }
        // At most 1 ingress should be counted due to debounce.
        assert!(ingress_count <= 1, "debounce should prevent double counting, got {}", ingress_count);
    }
}
