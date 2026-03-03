//! Queue length estimation — ADR-041 Category 4: Retail & Hospitality.
//!
//! Estimates queue length from sequential presence detection using CSI data.
//! Tracks join rate (lambda) and service rate (mu), then applies Little's Law
//! (L = lambda * W) to estimate average wait time.
//!
//! Events (400-series):
//! - `QUEUE_LENGTH(400)`:      Current estimated queue length
//! - `WAIT_TIME_ESTIMATE(401)`: Estimated wait time in seconds
//! - `SERVICE_RATE(402)`:       Service rate (persons/minute)
//! - `QUEUE_ALERT(403)`:        Queue threshold exceeded
//!
//! Host API used: presence, n_persons, variance, motion energy.

use crate::vendor_common::Ema;

#[cfg(not(feature = "std"))]
use libm::fabsf;
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

// ── Event IDs ─────────────────────────────────────────────────────────────────

pub const EVENT_QUEUE_LENGTH: i32 = 400;
pub const EVENT_WAIT_TIME_ESTIMATE: i32 = 401;
pub const EVENT_SERVICE_RATE: i32 = 402;
pub const EVENT_QUEUE_ALERT: i32 = 403;

// ── Configuration constants ──────────────────────────────────────────────────

/// Frame rate assumption (Hz).
const FRAME_RATE: f32 = 20.0;

/// Number of frames per reporting interval (~1 s at 20 Hz).
const REPORT_INTERVAL: u32 = 20;

/// Number of frames per service-rate computation window (~30 s).
const SERVICE_WINDOW_FRAMES: u32 = 600;

/// EMA smoothing for queue length.
const QUEUE_EMA_ALPHA: f32 = 0.1;

/// EMA smoothing for join/service rates.
const RATE_EMA_ALPHA: f32 = 0.05;

/// Variance threshold to detect a new person joining the queue.
const JOIN_VARIANCE_THRESH: f32 = 0.05;

/// Motion energy threshold below which a person is considered "served" (left).
const DEPART_MOTION_THRESH: f32 = 0.02;

/// Queue length alert threshold (persons).
const QUEUE_ALERT_THRESH: f32 = 5.0;

/// Maximum queue length tracked.
const MAX_QUEUE: usize = 20;

/// History window for arrival/departure events (60 seconds at 20 Hz).
const RATE_HISTORY: usize = 1200;

// ── Queue Length Estimator ───────────────────────────────────────────────────

/// Estimates queue length from CSI presence and person-count data.
pub struct QueueLengthEstimator {
    /// Smoothed queue length estimate.
    queue_ema: Ema,
    /// Smoothed arrival rate (persons/minute).
    arrival_rate_ema: Ema,
    /// Smoothed service rate (persons/minute).
    service_rate_ema: Ema,
    /// Previous n_persons value for detecting joins/departures.
    prev_n_persons: i32,
    /// Previous presence state.
    prev_presence: bool,
    /// Running count of arrivals in current window.
    arrivals_in_window: u16,
    /// Running count of departures in current window.
    departures_in_window: u16,
    /// Frame counter.
    frame_count: u32,
    /// Window frame counter (resets every SERVICE_WINDOW_FRAMES).
    window_frame_count: u32,
    /// Previous variance value for detecting transient spikes.
    prev_variance: f32,
    /// Current best estimate of queue length (integer).
    current_queue: u8,
    /// Alert already fired flag (prevents re-alerting same spike).
    alert_active: bool,
}

impl QueueLengthEstimator {
    pub const fn new() -> Self {
        Self {
            queue_ema: Ema::new(QUEUE_EMA_ALPHA),
            arrival_rate_ema: Ema::new(RATE_EMA_ALPHA),
            service_rate_ema: Ema::new(RATE_EMA_ALPHA),
            prev_n_persons: 0,
            prev_presence: false,
            arrivals_in_window: 0,
            departures_in_window: 0,
            frame_count: 0,
            window_frame_count: 0,
            prev_variance: 0.0,
            current_queue: 0,
            alert_active: false,
        }
    }

    /// Process one CSI frame with host-provided aggregate signals.
    ///
    /// - `presence`: 1 if someone is present, 0 otherwise
    /// - `n_persons`: estimated person count from Tier 2
    /// - `variance`: mean subcarrier variance (indicates motion)
    /// - `motion_energy`: aggregate motion energy
    ///
    /// Returns event slice `&[(event_type, value)]`.
    pub fn process_frame(
        &mut self,
        presence: i32,
        n_persons: i32,
        variance: f32,
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.window_frame_count += 1;

        let is_present = presence > 0;
        let n = if n_persons < 0 { 0 } else { n_persons };

        // Detect arrivals: n_persons increased or new presence with variance spike.
        if n > self.prev_n_persons {
            let delta = (n - self.prev_n_persons) as u16;
            self.arrivals_in_window = self.arrivals_in_window.saturating_add(delta);
        } else if !self.prev_presence && is_present {
            // Presence edge: someone appeared.
            let var_delta = fabsf(variance - self.prev_variance);
            if var_delta > JOIN_VARIANCE_THRESH {
                self.arrivals_in_window = self.arrivals_in_window.saturating_add(1);
            }
        }

        // Detect departures: n_persons decreased.
        if n < self.prev_n_persons {
            let delta = (self.prev_n_persons - n) as u16;
            self.departures_in_window = self.departures_in_window.saturating_add(delta);
        } else if self.prev_presence && !is_present && motion_energy < DEPART_MOTION_THRESH {
            // Presence edge: everyone left.
            self.departures_in_window = self.departures_in_window.saturating_add(1);
        }

        self.prev_n_persons = n;
        self.prev_presence = is_present;
        self.prev_variance = variance;

        // Update queue estimate: max(0, arrivals - departures) smoothed with person count.
        let raw_queue = if n > 0 { n as f32 } else { 0.0 };
        self.queue_ema.update(raw_queue);
        self.current_queue = (self.queue_ema.value + 0.5) as u8;
        if self.current_queue > MAX_QUEUE as u8 {
            self.current_queue = MAX_QUEUE as u8;
        }

        // Build events.
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut ne = 0usize;

        // Periodic queue length report.
        if self.frame_count % REPORT_INTERVAL == 0 {
            unsafe {
                EVENTS[ne] = (EVENT_QUEUE_LENGTH, self.current_queue as f32);
            }
            ne += 1;
        }

        // Service window elapsed: compute and emit rates.
        if self.window_frame_count >= SERVICE_WINDOW_FRAMES {
            let window_minutes = self.window_frame_count as f32 / (FRAME_RATE * 60.0);
            if window_minutes > 0.0 {
                let arr_rate = self.arrivals_in_window as f32 / window_minutes;
                let svc_rate = self.departures_in_window as f32 / window_minutes;

                self.arrival_rate_ema.update(arr_rate);
                self.service_rate_ema.update(svc_rate);

                // Service rate event.
                if ne < 4 {
                    unsafe {
                        EVENTS[ne] = (EVENT_SERVICE_RATE, self.service_rate_ema.value);
                    }
                    ne += 1;
                }

                // Wait time estimate via Little's Law: W = L / lambda.
                // If arrival rate is near zero, report 0 wait.
                let wait_time = if self.arrival_rate_ema.value > 0.1 {
                    (self.current_queue as f32) / (self.arrival_rate_ema.value / 60.0)
                } else {
                    0.0
                };

                if ne < 4 {
                    unsafe {
                        EVENTS[ne] = (EVENT_WAIT_TIME_ESTIMATE, wait_time);
                    }
                    ne += 1;
                }
            }

            // Reset window counters.
            self.window_frame_count = 0;
            self.arrivals_in_window = 0;
            self.departures_in_window = 0;
        }

        // Queue alert.
        if self.current_queue as f32 >= QUEUE_ALERT_THRESH && !self.alert_active {
            self.alert_active = true;
            if ne < 4 {
                unsafe {
                    EVENTS[ne] = (EVENT_QUEUE_ALERT, self.current_queue as f32);
                }
                ne += 1;
            }
        } else if (self.current_queue as f32) < QUEUE_ALERT_THRESH - 1.0 {
            self.alert_active = false;
        }

        unsafe { &EVENTS[..ne] }
    }

    /// Get the current smoothed queue length.
    pub fn queue_length(&self) -> u8 {
        self.current_queue
    }

    /// Get the smoothed arrival rate (persons/minute).
    pub fn arrival_rate(&self) -> f32 {
        self.arrival_rate_ema.value
    }

    /// Get the smoothed service rate (persons/minute).
    pub fn service_rate(&self) -> f32 {
        self.service_rate_ema.value
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let q = QueueLengthEstimator::new();
        assert_eq!(q.queue_length(), 0);
        assert_eq!(q.frame_count, 0);
        assert!(!q.alert_active);
    }

    #[test]
    fn test_empty_queue_no_events_except_periodic() {
        let mut q = QueueLengthEstimator::new();
        // Process frames with no presence.
        for i in 1..=40 {
            let events = q.process_frame(0, 0, 0.0, 0.0);
            if i % REPORT_INTERVAL == 0 {
                assert!(!events.is_empty(), "periodic report expected at frame {}", i);
                assert_eq!(events[0].0, EVENT_QUEUE_LENGTH);
                assert!(events[0].1 < 0.5, "queue should be ~0");
            }
        }
        assert_eq!(q.queue_length(), 0);
    }

    #[test]
    fn test_queue_grows_with_persons() {
        let mut q = QueueLengthEstimator::new();
        // Simulate people arriving: ramp n_persons from 0 to 3.
        for _ in 0..60 {
            q.process_frame(1, 3, 0.1, 0.5);
        }
        // Queue EMA should converge towards 3.
        assert!(q.queue_length() >= 2, "queue should track person count, got {}", q.queue_length());
    }

    #[test]
    fn test_arrival_detection() {
        let mut q = QueueLengthEstimator::new();
        // Start with 0 people.
        q.process_frame(0, 0, 0.0, 0.0);
        // One person arrives.
        q.process_frame(1, 1, 0.1, 0.3);
        // Another person arrives.
        q.process_frame(1, 2, 0.15, 0.4);
        // Check arrivals tracked.
        assert!(q.arrivals_in_window >= 2, "should detect at least 2 arrivals, got {}", q.arrivals_in_window);
    }

    #[test]
    fn test_departure_detection() {
        let mut q = QueueLengthEstimator::new();
        // Start with 3 people.
        q.process_frame(1, 3, 0.1, 0.5);
        // One departs.
        q.process_frame(1, 2, 0.08, 0.3);
        // Another departs.
        q.process_frame(1, 1, 0.05, 0.2);
        assert!(q.departures_in_window >= 2, "should detect departures, got {}", q.departures_in_window);
    }

    #[test]
    fn test_queue_alert() {
        let mut q = QueueLengthEstimator::new();
        let mut alert_fired = false;
        // Push enough frames with high person count to trigger alert.
        for _ in 0..200 {
            let events = q.process_frame(1, 8, 0.2, 0.8);
            for &(et, _) in events {
                if et == EVENT_QUEUE_ALERT {
                    alert_fired = true;
                }
            }
        }
        assert!(alert_fired, "queue alert should fire when queue >= {}", QUEUE_ALERT_THRESH);
    }

    #[test]
    fn test_service_rate_computation() {
        let mut q = QueueLengthEstimator::new();
        let mut service_rate_emitted = false;

        // Simulate arrivals and departures over a full window.
        for i in 0..SERVICE_WINDOW_FRAMES + 1 {
            let n = if i < 300 { 3 } else { 1 };
            let events = q.process_frame(1, n, 0.1, 0.3);
            for &(et, _) in events {
                if et == EVENT_SERVICE_RATE {
                    service_rate_emitted = true;
                }
            }
        }
        assert!(service_rate_emitted, "service rate should be emitted after window elapses");
    }

    #[test]
    fn test_negative_inputs_handled() {
        let mut q = QueueLengthEstimator::new();
        // Negative n_persons should be treated as 0.
        let _events = q.process_frame(-1, -5, -0.1, -0.5);
        // Should not panic.
        assert_eq!(q.queue_length(), 0);
    }
}
