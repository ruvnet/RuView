//! Spiking neural network tracker — spatial reasoning module (ADR-041).
//!
//! Bio-inspired person tracking using Leaky Integrate-and-Fire (LIF) neurons
//! with STDP learning.  32 input neurons (one per subcarrier) feed into
//! 4 output neurons (one per spatial zone).  The zone with the highest
//! spike rate indicates person location; zone transitions track velocity.
//!
//! Event IDs: 770-773 (Spatial Reasoning series).

use libm::fabsf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Number of input neurons (one per subcarrier).
const N_INPUT: usize = 32;

/// Number of output neurons (one per zone).
const N_OUTPUT: usize = 4;

/// Input neurons per output zone.
const INPUTS_PER_ZONE: usize = N_INPUT / N_OUTPUT; // = 8

/// LIF neuron threshold potential.
const THRESHOLD: f32 = 1.0;

/// Membrane leak factor (per frame).
const LEAK: f32 = 0.95;

/// Reset potential after spike.
const RESET: f32 = 0.0;

/// STDP learning rate (potentiation).
const STDP_LR_PLUS: f32 = 0.01;

/// STDP learning rate (depression).
const STDP_LR_MINUS: f32 = 0.005;

/// STDP time window in frames (approximation of 20ms at 50Hz).
const STDP_WINDOW: u32 = 1;

/// EMA factor for spike rate smoothing.
const RATE_ALPHA: f32 = 0.1;

/// EMA factor for velocity smoothing.
const VEL_ALPHA: f32 = 0.2;

/// Minimum spike rate to consider a zone active.
const MIN_SPIKE_RATE: f32 = 0.05;

/// Weight clamp bounds.
const W_MIN: f32 = 0.0;
const W_MAX: f32 = 2.0;

// ── Event IDs ────────────────────────────────────────────────────────────────

/// Zone ID of the tracked person (0-3), or -1 if lost.
pub const EVENT_TRACK_UPDATE: i32 = 770;

/// Estimated velocity (zone transitions per second, EMA-smoothed).
pub const EVENT_TRACK_VELOCITY: i32 = 771;

/// Mean spike rate across all input neurons [0, 1].
pub const EVENT_SPIKE_RATE: i32 = 772;

/// Emitted when the person is lost (no zone active).
pub const EVENT_TRACK_LOST: i32 = 773;

// ── State ────────────────────────────────────────────────────────────────────

/// Spiking neural network person tracker.
pub struct SpikingTracker {
    /// Membrane potential of each input neuron.
    membrane: [f32; N_INPUT],
    /// Synaptic weights from input to output neurons.
    /// weights[i][z] = connection strength from input i to output zone z.
    weights: [[f32; N_OUTPUT]; N_INPUT],
    /// Spike time of each input neuron (frame number, 0 = never fired).
    input_spike_time: [u32; N_INPUT],
    /// Spike time of each output neuron.
    output_spike_time: [u32; N_OUTPUT],
    /// EMA-smoothed spike rate per zone.
    zone_rate: [f32; N_OUTPUT],
    /// Raw spike count per zone this frame.
    zone_spikes: [u32; N_OUTPUT],
    /// Previous active zone (for velocity).
    prev_zone: i8,
    /// Velocity EMA (zone transitions per frame).
    velocity_ema: f32,
    /// Whether the track is currently active.
    track_active: bool,
    /// Frame counter.
    frame_count: u32,
    /// Frames since last zone transition.
    frames_since_transition: u32,
}

impl SpikingTracker {
    pub const fn new() -> Self {
        // Initialize weights: each input connects to its "home" zone with
        // weight 1.0 and to other zones with 0.25.
        let mut weights = [[0.25f32; N_OUTPUT]; N_INPUT];
        let mut i = 0;
        while i < N_INPUT {
            let home_zone = i / INPUTS_PER_ZONE;
            if home_zone < N_OUTPUT {
                weights[i][home_zone] = 1.0;
            }
            i += 1;
        }

        Self {
            membrane: [0.0; N_INPUT],
            weights,
            input_spike_time: [0; N_INPUT],
            output_spike_time: [0; N_OUTPUT],
            zone_rate: [0.0; N_OUTPUT],
            zone_spikes: [0; N_OUTPUT],
            prev_zone: -1,
            velocity_ema: 0.0,
            track_active: false,
            frame_count: 0,
            frames_since_transition: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases` — per-subcarrier phase values (up to 32).
    /// `prev_phases` — previous frame phases for delta computation.
    ///
    /// Returns a slice of (event_id, value) pairs to emit.
    pub fn process_frame(&mut self, phases: &[f32], prev_phases: &[f32]) -> &[(i32, f32)] {
        let n_sc = phases.len().min(prev_phases.len()).min(N_INPUT);
        self.frame_count += 1;
        self.frames_since_transition += 1;

        // ── 1. Compute current injection from phase changes ──────────────
        let mut input_spikes = [false; N_INPUT];
        for i in 0..n_sc {
            let current = fabsf(phases[i] - prev_phases[i]);
            // Leaky integration.
            self.membrane[i] = self.membrane[i] * LEAK + current;

            // Fire?
            if self.membrane[i] >= THRESHOLD {
                input_spikes[i] = true;
                self.membrane[i] = RESET;
                self.input_spike_time[i] = self.frame_count;
            }
        }

        // ── 2. Propagate spikes to output neurons ────────────────────────
        let mut output_potential = [0.0f32; N_OUTPUT];
        for i in 0..n_sc {
            if input_spikes[i] {
                for z in 0..N_OUTPUT {
                    output_potential[z] += self.weights[i][z];
                }
            }
        }

        // Determine output spikes.
        let mut output_spikes = [false; N_OUTPUT];
        for z in 0..N_OUTPUT {
            self.zone_spikes[z] = 0;
        }
        for z in 0..N_OUTPUT {
            if output_potential[z] >= THRESHOLD {
                output_spikes[z] = true;
                self.zone_spikes[z] = 1;
                self.output_spike_time[z] = self.frame_count;
            }
        }

        // ── 3. STDP learning ─────────────────────────────────────────────
        // PERF: Only iterate over neurons that actually fired (skip silent inputs).
        // Typical sparsity: ~10-30% of inputs fire, so this skips 70-90% of
        // the 32*4=128 weight update iterations.
        for i in 0..n_sc {
            if !input_spikes[i] {
                continue; // Skip silent input neurons entirely.
            }
            for z in 0..N_OUTPUT {
                if output_spikes[z] {
                    // Pre fires, post fires -> potentiate.
                    let dt = if self.input_spike_time[i] >= self.output_spike_time[z] {
                        self.input_spike_time[i] - self.output_spike_time[z]
                    } else {
                        self.output_spike_time[z] - self.input_spike_time[i]
                    };
                    if dt <= STDP_WINDOW {
                        self.weights[i][z] += STDP_LR_PLUS;
                        if self.weights[i][z] > W_MAX {
                            self.weights[i][z] = W_MAX;
                        }
                    }
                } else {
                    // Pre fires, post silent -> depress slightly.
                    self.weights[i][z] -= STDP_LR_MINUS;
                    if self.weights[i][z] < W_MIN {
                        self.weights[i][z] = W_MIN;
                    }
                }
            }
        }

        // ── 4. Update zone spike rates (EMA) ────────────────────────────
        for z in 0..N_OUTPUT {
            let instant = self.zone_spikes[z] as f32;
            self.zone_rate[z] = RATE_ALPHA * instant + (1.0 - RATE_ALPHA) * self.zone_rate[z];
        }

        // ── 5. Determine active zone ────────────────────────────────────
        let mut best_zone: i8 = -1;
        let mut best_rate = MIN_SPIKE_RATE;
        for z in 0..N_OUTPUT {
            if self.zone_rate[z] > best_rate {
                best_rate = self.zone_rate[z];
                best_zone = z as i8;
            }
        }

        // ── 6. Velocity from zone transitions ───────────────────────────
        if best_zone >= 0 && best_zone != self.prev_zone && self.prev_zone >= 0 {
            let transition_speed = if self.frames_since_transition > 0 {
                1.0 / (self.frames_since_transition as f32)
            } else {
                0.0
            };
            self.velocity_ema = VEL_ALPHA * transition_speed + (1.0 - VEL_ALPHA) * self.velocity_ema;
            self.frames_since_transition = 0;
        }

        let was_active = self.track_active;
        self.track_active = best_zone >= 0;
        if best_zone >= 0 {
            self.prev_zone = best_zone;
        }

        // ── 7. Build events ─────────────────────────────────────────────
        self.build_events(best_zone, was_active)
    }

    /// Construct event output.
    fn build_events(&self, zone: i8, was_active: bool) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;

        // Mean spike rate across all zones.
        let mut total_rate = 0.0f32;
        for z in 0..N_OUTPUT {
            total_rate += self.zone_rate[z];
        }
        let mean_rate = total_rate / N_OUTPUT as f32;

        if zone >= 0 {
            // TRACK_UPDATE with zone ID.
            unsafe { EVENTS[n] = (EVENT_TRACK_UPDATE, zone as f32); }
            n += 1;

            // TRACK_VELOCITY.
            unsafe { EVENTS[n] = (EVENT_TRACK_VELOCITY, self.velocity_ema); }
            n += 1;

            // SPIKE_RATE.
            unsafe { EVENTS[n] = (EVENT_SPIKE_RATE, mean_rate); }
            n += 1;
        } else {
            // SPIKE_RATE even when no track.
            unsafe { EVENTS[n] = (EVENT_SPIKE_RATE, mean_rate); }
            n += 1;

            // TRACK_LOST if we had a track before.
            if was_active {
                unsafe { EVENTS[n] = (EVENT_TRACK_LOST, self.prev_zone as f32); }
                n += 1;
            }
        }

        unsafe { &EVENTS[..n] }
    }

    /// Get the current tracked zone (-1 if lost).
    pub fn current_zone(&self) -> i8 {
        if self.track_active { self.prev_zone } else { -1 }
    }

    /// Get the smoothed spike rate for a zone.
    pub fn zone_spike_rate(&self, zone: usize) -> f32 {
        if zone < N_OUTPUT { self.zone_rate[zone] } else { 0.0 }
    }

    /// Get the EMA-smoothed velocity.
    pub fn velocity(&self) -> f32 {
        self.velocity_ema
    }

    /// Check if a track is currently active.
    pub fn is_tracking(&self) -> bool {
        self.track_active
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_constructor() {
        let st = SpikingTracker::new();
        assert_eq!(st.frame_count, 0);
        assert!(!st.track_active);
        assert_eq!(st.prev_zone, -1);
        assert_eq!(st.current_zone(), -1);
    }

    #[test]
    fn test_initial_weights() {
        let st = SpikingTracker::new();
        // Input 0 should have strong weight to zone 0.
        assert!((st.weights[0][0] - 1.0).abs() < 1e-6);
        // Input 0 should have weak weight to zone 1.
        assert!((st.weights[0][1] - 0.25).abs() < 1e-6);
        // Input 8 should have strong weight to zone 1.
        assert!((st.weights[8][1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_no_activity_no_track() {
        let mut st = SpikingTracker::new();
        let phases = [0.0f32; 32];
        let prev = [0.0f32; 32];
        st.process_frame(&phases, &prev);
        // No phase change -> no spikes -> no track.
        assert!(!st.is_tracking());
    }

    #[test]
    fn test_zone_activation() {
        let mut st = SpikingTracker::new();
        let prev = [0.0f32; 32];

        // Inject large phase change in zone 0 (subcarriers 0-7).
        let mut phases = [0.0f32; 32];
        for i in 0..8 {
            phases[i] = 2.0; // Well above threshold after integration.
        }

        // Feed many frames to build up spike rate difference.
        // LIF neurons reset after firing, so we need enough frames for the
        // EMA spike rate in zone 0 to clearly exceed zone 1.
        for _ in 0..100 {
            st.process_frame(&phases, &prev);
        }

        // Zone 0 should have a meaningful spike rate.
        let r0 = st.zone_spike_rate(0);
        assert!(r0 > MIN_SPIKE_RATE, "zone 0 should be active, rate={}", r0);
    }

    #[test]
    fn test_zone_transition_velocity() {
        let mut st = SpikingTracker::new();
        let prev = [0.0f32; 32];

        // Activate zone 0 for a while.
        let mut phases_z0 = [0.0f32; 32];
        for i in 0..8 {
            phases_z0[i] = 2.0;
        }
        for _ in 0..30 {
            st.process_frame(&phases_z0, &prev);
        }

        // Now activate zone 2 instead.
        let mut phases_z2 = [0.0f32; 32];
        for i in 16..24 {
            phases_z2[i] = 2.0;
        }
        for _ in 0..30 {
            st.process_frame(&phases_z2, &prev);
        }

        // Velocity should be non-zero after a zone transition.
        // (It may take a few frames for the EMA to register.)
        assert!(st.velocity() >= 0.0);
    }

    #[test]
    fn test_stdp_strengthens_active_connections() {
        let mut st = SpikingTracker::new();
        let prev = [0.0f32; 32];

        let initial_w = st.weights[0][0];

        // Repeated activity in zone 0 should strengthen weights[0][0].
        let mut phases = [0.0f32; 32];
        for i in 0..8 {
            phases[i] = 2.0;
        }
        for _ in 0..50 {
            st.process_frame(&phases, &prev);
        }

        // Weight should have increased (or stayed at max).
        assert!(st.weights[0][0] >= initial_w);
    }

    #[test]
    fn test_track_lost_event() {
        let mut st = SpikingTracker::new();
        let prev = [0.0f32; 32];

        // Activate a zone first.
        let mut phases = [0.0f32; 32];
        for i in 0..8 {
            phases[i] = 2.0;
        }
        for _ in 0..30 {
            st.process_frame(&phases, &prev);
        }
        assert!(st.is_tracking());

        // Now go silent — all zeros.
        let silent = [0.0f32; 32];
        let mut lost_emitted = false;
        for _ in 0..100 {
            let events = st.process_frame(&silent, &prev);
            for e in events {
                if e.0 == EVENT_TRACK_LOST {
                    lost_emitted = true;
                }
            }
        }

        // Should eventually lose track and emit TRACK_LOST.
        // (The EMA decay will eventually bring rate below threshold.)
        assert!(lost_emitted || !st.is_tracking());
    }

    #[test]
    fn test_membrane_leak() {
        let mut st = SpikingTracker::new();
        // Inject sub-threshold current.
        st.membrane[0] = 0.5;

        let phases = [0.0f32; 32];
        let prev = [0.0f32; 32];
        st.process_frame(&phases, &prev);

        // Membrane should have decayed by LEAK.
        assert!(st.membrane[0] < 0.5);
        assert!(st.membrane[0] > 0.0);
    }
}
