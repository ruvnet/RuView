//! Tailgating detection — ADR-041 Category 2 Security module.
//!
//! Detects tailgating at doorways — two or more people passing through in rapid
//! succession — by looking for double-peaked (or multi-peaked) motion energy
//! envelopes. A single authorised passage produces one smooth energy peak; a
//! tailgater following closely produces a second peak within a configurable
//! inter-peak interval.
//!
//! The detector uses temporal clustering of motion energy peaks. When a peak
//! is detected (energy crosses above threshold then falls), a window opens.
//! If another peak occurs within the window, tailgating is flagged.
//!
//! Events: TAILGATE_DETECTED(230), SINGLE_PASSAGE(231), MULTI_PASSAGE(232).
//! Budget: L (<2 ms).

#[cfg(not(feature = "std"))]
use libm::fabsf;
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

/// Motion energy threshold to consider a peak start.
const ENERGY_PEAK_THRESH: f32 = 2.0;
/// Energy must drop below this fraction of peak to end a peak.
const ENERGY_VALLEY_FRAC: f32 = 0.3;
/// Maximum inter-peak interval for tailgating (frames). Default: 3 seconds at 20 Hz.
const TAILGATE_WINDOW: u32 = 60;
/// Minimum peak energy to be considered a valid passage.
const MIN_PEAK_ENERGY: f32 = 1.5;
/// Cooldown after tailgate event (frames).
const COOLDOWN: u16 = 100;
/// Minimum frames a peak must last to be valid (debounce noise spikes).
const MIN_PEAK_FRAMES: u8 = 3;
/// Maximum peaks tracked in one passage window.
const MAX_PEAKS: usize = 8;

pub const EVENT_TAILGATE_DETECTED: i32 = 230;
pub const EVENT_SINGLE_PASSAGE: i32 = 231;
pub const EVENT_MULTI_PASSAGE: i32 = 232;

/// Peak detection state.
#[derive(Clone, Copy, PartialEq)]
enum PeakState {
    /// Waiting for energy to rise above threshold.
    Idle,
    /// Energy is above threshold — tracking a peak.
    InPeak,
    /// Peak ended, watching for another peak within window.
    Watching,
}

/// Tailgating detector.
pub struct TailgateDetector {
    state: PeakState,
    /// Current peak's maximum energy.
    peak_max: f32,
    /// Frames spent in current peak.
    peak_frames: u8,
    /// Peaks detected in current passage window.
    peaks_in_window: u8,
    /// Peak energies recorded.
    peak_energies: [f32; MAX_PEAKS],
    /// Frames since last peak ended (for window timeout).
    frames_since_peak: u32,
    /// Total passages detected.
    single_passages: u32,
    /// Total tailgating events.
    tailgate_count: u32,
    /// Cooldowns.
    cd_tailgate: u16,
    cd_passage: u16,
    frame_count: u32,
    /// Previous motion energy (for slope detection).
    prev_energy: f32,
    /// Variance history for noise floor estimation.
    var_accum: f32,
    var_count: u32,
    noise_floor: f32,
}

impl TailgateDetector {
    pub const fn new() -> Self {
        Self {
            state: PeakState::Idle,
            peak_max: 0.0,
            peak_frames: 0,
            peaks_in_window: 0,
            peak_energies: [0.0; MAX_PEAKS],
            frames_since_peak: 0,
            single_passages: 0,
            tailgate_count: 0,
            cd_tailgate: 0,
            cd_passage: 0,
            frame_count: 0,
            prev_energy: 0.0,
            var_accum: 0.0,
            var_count: 0,
            noise_floor: 0.5,
        }
    }

    /// Process one frame. Returns `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        motion_energy: f32,
        _presence: i32,
        _n_persons: i32,
        variance: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.cd_tailgate = self.cd_tailgate.saturating_sub(1);
        self.cd_passage = self.cd_passage.saturating_sub(1);

        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut ne = 0usize;

        // Update noise floor estimate (exponential moving average of variance).
        self.var_accum += variance;
        self.var_count += 1;
        if self.var_count >= 20 {
            self.noise_floor = (self.var_accum / self.var_count as f32).max(0.1);
            self.var_accum = 0.0;
            self.var_count = 0;
        }

        let threshold = ENERGY_PEAK_THRESH.max(self.noise_floor * 3.0);

        match self.state {
            PeakState::Idle => {
                if motion_energy > threshold {
                    self.state = PeakState::InPeak;
                    self.peak_max = motion_energy;
                    self.peak_frames = 1;
                    self.peaks_in_window = 0;
                }
            }

            PeakState::InPeak => {
                if motion_energy > self.peak_max {
                    self.peak_max = motion_energy;
                }
                self.peak_frames = self.peak_frames.saturating_add(1);

                // Peak ends when energy drops below valley threshold.
                if motion_energy < self.peak_max * ENERGY_VALLEY_FRAC {
                    if self.peak_frames >= MIN_PEAK_FRAMES && self.peak_max >= MIN_PEAK_ENERGY {
                        // Valid peak recorded.
                        let idx = self.peaks_in_window as usize;
                        if idx < MAX_PEAKS {
                            self.peak_energies[idx] = self.peak_max;
                        }
                        self.peaks_in_window += 1;
                        self.state = PeakState::Watching;
                        self.frames_since_peak = 0;
                    } else {
                        // Noise spike, reset.
                        self.state = PeakState::Idle;
                    }
                    self.peak_max = 0.0;
                    self.peak_frames = 0;
                }
            }

            PeakState::Watching => {
                self.frames_since_peak += 1;

                // Check if a new peak is starting within window.
                if motion_energy > threshold {
                    self.state = PeakState::InPeak;
                    self.peak_max = motion_energy;
                    self.peak_frames = 1;
                    return unsafe { &EVENTS[..0] };
                }

                // Window expired — evaluate passage.
                if self.frames_since_peak >= TAILGATE_WINDOW {
                    if self.peaks_in_window >= 2 {
                        // Multiple peaks detected = tailgating.
                        if self.cd_tailgate == 0 && ne < 3 {
                            unsafe {
                                EVENTS[ne] = (EVENT_TAILGATE_DETECTED, self.peaks_in_window as f32);
                            }
                            ne += 1;
                            self.cd_tailgate = COOLDOWN;
                            self.tailgate_count += 1;
                        }

                        // Also emit multi-passage.
                        if self.cd_passage == 0 && ne < 3 {
                            unsafe {
                                EVENTS[ne] = (EVENT_MULTI_PASSAGE, self.peaks_in_window as f32);
                            }
                            ne += 1;
                            self.cd_passage = COOLDOWN;
                        }
                    } else if self.peaks_in_window == 1 {
                        // Single passage.
                        if self.cd_passage == 0 && ne < 3 {
                            unsafe {
                                EVENTS[ne] = (EVENT_SINGLE_PASSAGE, self.peak_energies[0]);
                            }
                            ne += 1;
                            self.cd_passage = COOLDOWN;
                            self.single_passages += 1;
                        }
                    }

                    // Reset for next passage.
                    self.state = PeakState::Idle;
                    self.peaks_in_window = 0;
                }
            }
        }

        self.prev_energy = motion_energy;
        unsafe { &EVENTS[..ne] }
    }

    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn tailgate_count(&self) -> u32 { self.tailgate_count }
    pub fn single_passages(&self) -> u32 { self.single_passages }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulate a passage: ramp energy up then down.
    fn simulate_peak(det: &mut TailgateDetector, peak_energy: f32) -> Vec<(i32, f32)> {
        let mut all_events = Vec::new();
        // Ramp up over 5 frames.
        for i in 1..=5 {
            let e = peak_energy * (i as f32 / 5.0);
            let ev = det.process_frame(e, 1, 1, 0.1);
            all_events.extend_from_slice(ev);
        }
        // Ramp down over 5 frames.
        for i in (0..5).rev() {
            let e = peak_energy * (i as f32 / 5.0);
            let ev = det.process_frame(e, 1, 1, 0.1);
            all_events.extend_from_slice(ev);
        }
        all_events
    }

    #[test]
    fn test_init() {
        let det = TailgateDetector::new();
        assert_eq!(det.frame_count(), 0);
        assert_eq!(det.tailgate_count(), 0);
        assert_eq!(det.single_passages(), 0);
    }

    #[test]
    fn test_single_passage() {
        let mut det = TailgateDetector::new();
        // Stabilize noise floor.
        for _ in 0..30 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // Single peak.
        simulate_peak(&mut det, 5.0);

        // Wait for window to expire.
        let mut found_single = false;
        for _ in 0..(TAILGATE_WINDOW + 10) {
            let ev = det.process_frame(0.1, 0, 0, 0.05);
            for &(et, _) in ev {
                if et == EVENT_SINGLE_PASSAGE {
                    found_single = true;
                }
            }
        }
        assert!(found_single, "single passage should be detected");
    }

    #[test]
    fn test_tailgate_detection() {
        let mut det = TailgateDetector::new();
        // Stabilize noise floor.
        for _ in 0..30 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // First peak (authorized person).
        simulate_peak(&mut det, 5.0);

        // Brief gap (< TAILGATE_WINDOW frames).
        for _ in 0..10 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // Second peak (tailgater).
        simulate_peak(&mut det, 4.0);

        // Wait for window to expire.
        let mut found_tailgate = false;
        for _ in 0..(TAILGATE_WINDOW + 10) {
            let ev = det.process_frame(0.1, 0, 0, 0.05);
            for &(et, _) in ev {
                if et == EVENT_TAILGATE_DETECTED {
                    found_tailgate = true;
                }
            }
        }
        assert!(found_tailgate, "tailgating should be detected with two close peaks");
    }

    #[test]
    fn test_widely_spaced_peaks_no_tailgate() {
        let mut det = TailgateDetector::new();
        // Stabilize.
        for _ in 0..30 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // First peak.
        simulate_peak(&mut det, 5.0);

        // Wait longer than tailgate window.
        for _ in 0..(TAILGATE_WINDOW + 30) {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // Second peak.
        simulate_peak(&mut det, 5.0);

        // Wait for evaluation.
        let mut found_tailgate = false;
        for _ in 0..(TAILGATE_WINDOW + 10) {
            let ev = det.process_frame(0.1, 0, 0, 0.05);
            for &(et, _) in ev {
                if et == EVENT_TAILGATE_DETECTED {
                    found_tailgate = true;
                }
            }
        }
        assert!(!found_tailgate, "widely spaced peaks should not trigger tailgate");
    }

    #[test]
    fn test_noise_spike_ignored() {
        let mut det = TailgateDetector::new();
        // Stabilize.
        for _ in 0..30 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // Very brief spike (1 frame above threshold — below MIN_PEAK_FRAMES).
        det.process_frame(5.0, 1, 1, 0.1);
        det.process_frame(0.1, 0, 0, 0.05); // Immediately drop.

        // Should not produce any passage events.
        let mut any_passage = false;
        for _ in 0..(TAILGATE_WINDOW + 10) {
            let ev = det.process_frame(0.1, 0, 0, 0.05);
            for &(et, _) in ev {
                if et == EVENT_SINGLE_PASSAGE || et == EVENT_TAILGATE_DETECTED {
                    any_passage = true;
                }
            }
        }
        assert!(!any_passage, "noise spike should not trigger passage event");
    }

    #[test]
    fn test_multi_passage_event() {
        let mut det = TailgateDetector::new();
        // Stabilize.
        for _ in 0..30 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // Three peaks in rapid succession.
        simulate_peak(&mut det, 5.0);
        for _ in 0..8 { det.process_frame(0.1, 0, 0, 0.05); }
        simulate_peak(&mut det, 4.5);
        for _ in 0..8 { det.process_frame(0.1, 0, 0, 0.05); }
        simulate_peak(&mut det, 4.0);

        let mut found_multi = false;
        for _ in 0..(TAILGATE_WINDOW + 10) {
            let ev = det.process_frame(0.1, 0, 0, 0.05);
            for &(et, v) in ev {
                if et == EVENT_MULTI_PASSAGE {
                    found_multi = true;
                    assert!(v >= 2.0, "multi passage should report 2+ peaks");
                }
            }
        }
        assert!(found_multi, "multi-passage event should fire with 3 rapid peaks");
    }

    #[test]
    fn test_low_energy_ignored() {
        let mut det = TailgateDetector::new();
        for _ in 0..30 {
            det.process_frame(0.1, 0, 0, 0.05);
        }

        // Below peak threshold.
        for _ in 0..100 {
            let ev = det.process_frame(0.5, 1, 1, 0.1);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_TAILGATE_DETECTED);
                assert_ne!(et, EVENT_SINGLE_PASSAGE);
            }
        }
    }
}
