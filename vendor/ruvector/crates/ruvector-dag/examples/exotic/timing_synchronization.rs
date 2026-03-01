//! # Timing Synchronization
//!
//! Machines that feel timing, not data.
//!
//! Most systems measure values. This measures when things stop lining up.
//!
//! Applications:
//! - Prosthetics that adapt reflex timing to the user's nervous system
//! - Brain-computer interfaces that align with biological rhythms
//! - Control systems that synchronize with humans instead of commanding them
//!
//! You stop predicting intent. You synchronize with it.
//! This is how machines stop feeling external.

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::time::{Duration, Instant};

/// A rhythm detected in a signal stream
#[derive(Clone, Debug)]
pub struct Rhythm {
    /// Detected period in milliseconds
    period_ms: f64,
    /// Phase offset (0-1)
    phase: f64,
    /// Confidence in this rhythm (0-1)
    confidence: f64,
    /// Last peak timestamp
    last_peak: Instant,
}

/// Synchronization state between two rhythmic systems
#[derive(Clone, Debug)]
pub struct SyncState {
    /// Phase difference (-0.5 to 0.5, 0 = perfectly aligned)
    phase_diff: f64,
    /// Whether systems are drifting apart or converging
    drift_rate: f64,
    /// Coupling strength (how much they influence each other)
    coupling: f64,
    /// Time since last alignment event
    since_alignment: Duration,
}

/// A timing-aware interface that synchronizes with external rhythms
pub struct TimingSynchronizer {
    /// Our internal rhythm
    internal_rhythm: Rhythm,

    /// Detected external rhythm (e.g., human nervous system)
    external_rhythm: Option<Rhythm>,

    /// History of phase differences
    phase_history: VecDeque<(Instant, f64)>,

    /// Current synchronization state
    sync_state: SyncState,

    /// Adaptation rate (how quickly we adjust to external rhythm)
    adaptation_rate: f64,

    /// Minimum coupling threshold to attempt sync
    coupling_threshold: f64,

    /// Coherence signal from MinCut (when timing breaks down)
    coherence: f64,
}

impl TimingSynchronizer {
    pub fn new(internal_period_ms: f64) -> Self {
        Self {
            internal_rhythm: Rhythm {
                period_ms: internal_period_ms,
                phase: 0.0,
                confidence: 1.0,
                last_peak: Instant::now(),
            },
            external_rhythm: None,
            phase_history: VecDeque::with_capacity(1000),
            sync_state: SyncState {
                phase_diff: 0.0,
                drift_rate: 0.0,
                coupling: 0.0,
                since_alignment: Duration::ZERO,
            },
            adaptation_rate: 0.1,
            coupling_threshold: 0.3,
            coherence: 1.0,
        }
    }

    /// Observe an external timing signal (e.g., neural spike, heartbeat, movement)
    pub fn observe_external(&mut self, signal_value: f64, timestamp: Instant) {
        // Detect peaks in external signal to find rhythm
        self.detect_external_rhythm(signal_value, timestamp);

        // If we have both rhythms, compute phase relationship
        if let Some(ref external) = self.external_rhythm {
            let phase_diff =
                self.compute_phase_difference(&self.internal_rhythm, external, timestamp);

            // Track phase history
            self.phase_history.push_back((timestamp, phase_diff));
            while self.phase_history.len() > 100 {
                self.phase_history.pop_front();
            }

            // Update sync state
            self.update_sync_state(phase_diff, timestamp);

            // Update coherence based on phase stability
            self.update_coherence();
        }
    }

    /// Advance our internal rhythm and potentially adapt to external
    pub fn tick(&mut self) -> TimingAction {
        let now = Instant::now();

        // Advance internal phase
        let elapsed = now.duration_since(self.internal_rhythm.last_peak);
        let cycle_progress = elapsed.as_secs_f64() * 1000.0 / self.internal_rhythm.period_ms;
        self.internal_rhythm.phase = cycle_progress.fract();

        // Check if we should adapt to external rhythm
        if self.should_adapt() {
            return self.adapt_to_external();
        }

        // Check if we're at a natural action point
        if self.is_action_point() {
            TimingAction::Fire {
                phase: self.internal_rhythm.phase,
                confidence: self.sync_state.coupling,
            }
        } else {
            TimingAction::Wait {
                until_next_ms: self.ms_until_next_action(),
            }
        }
    }

    /// Get current synchronization quality
    pub fn sync_quality(&self) -> f64 {
        // Perfect sync = phase_diff near 0, high coupling, stable drift
        let phase_quality = 1.0 - self.sync_state.phase_diff.abs() * 2.0;
        let stability = 1.0 - self.sync_state.drift_rate.abs().min(1.0);
        let coupling = self.sync_state.coupling;

        (phase_quality * stability * coupling).max(0.0)
    }

    /// Are we currently synchronized with external rhythm?
    pub fn is_synchronized(&self) -> bool {
        self.sync_quality() > 0.7 && self.coherence > 0.8
    }

    /// Get the optimal moment for action (synchronizing with external)
    pub fn optimal_action_phase(&self) -> f64 {
        if let Some(ref external) = self.external_rhythm {
            // Aim for the external rhythm's peak
            let target_phase = 0.0; // Peak of external rhythm
            let adjustment = self.sync_state.phase_diff;
            (target_phase - adjustment).rem_euclid(1.0)
        } else {
            0.0
        }
    }

    fn detect_external_rhythm(&mut self, signal: f64, timestamp: Instant) {
        // Simple peak detection (in real system, use proper rhythm extraction)
        if signal > 0.8 {
            // Peak threshold
            if let Some(ref mut rhythm) = self.external_rhythm {
                let since_last = timestamp.duration_since(rhythm.last_peak);
                let new_period = since_last.as_secs_f64() * 1000.0;

                // Smooth period estimate
                rhythm.period_ms = rhythm.period_ms * 0.8 + new_period * 0.2;
                rhythm.last_peak = timestamp;
                rhythm.confidence = (rhythm.confidence * 0.9 + 0.1).min(1.0);
            } else {
                self.external_rhythm = Some(Rhythm {
                    period_ms: 1000.0, // Initial guess
                    phase: 0.0,
                    confidence: 0.5,
                    last_peak: timestamp,
                });
            }
        }
    }

    fn compute_phase_difference(&self, internal: &Rhythm, external: &Rhythm, now: Instant) -> f64 {
        let internal_phase =
            now.duration_since(internal.last_peak).as_secs_f64() * 1000.0 / internal.period_ms;
        let external_phase =
            now.duration_since(external.last_peak).as_secs_f64() * 1000.0 / external.period_ms;

        let diff = (internal_phase - external_phase).rem_euclid(1.0);
        if diff > 0.5 {
            diff - 1.0
        } else {
            diff
        }
    }

    fn update_sync_state(&mut self, phase_diff: f64, timestamp: Instant) {
        // Compute drift rate from phase history
        if self.phase_history.len() >= 10 {
            let recent: Vec<f64> = self
                .phase_history
                .iter()
                .rev()
                .take(5)
                .map(|(_, p)| *p)
                .collect();
            let older: Vec<f64> = self
                .phase_history
                .iter()
                .rev()
                .skip(5)
                .take(5)
                .map(|(_, p)| *p)
                .collect();

            let recent_avg: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
            let older_avg: f64 = older.iter().sum::<f64>() / older.len() as f64;

            self.sync_state.drift_rate = (recent_avg - older_avg) * 10.0;
        }

        // Update coupling based on rhythm stability
        if let Some(ref external) = self.external_rhythm {
            self.sync_state.coupling = external.confidence * self.internal_rhythm.confidence;
        }

        self.sync_state.phase_diff = phase_diff;

        // Track alignment events
        if phase_diff.abs() < 0.05 {
            self.sync_state.since_alignment = Duration::ZERO;
        } else {
            self.sync_state.since_alignment = timestamp.duration_since(
                self.phase_history
                    .front()
                    .map(|(t, _)| *t)
                    .unwrap_or(timestamp),
            );
        }
    }

    fn update_coherence(&mut self) {
        // Coherence drops when phase relationship becomes unstable
        let phase_variance: f64 = if self.phase_history.len() > 5 {
            let phases: Vec<f64> = self.phase_history.iter().map(|(_, p)| *p).collect();
            let mean = phases.iter().sum::<f64>() / phases.len() as f64;
            phases.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / phases.len() as f64
        } else {
            0.0
        };

        self.coherence = (1.0 - phase_variance * 10.0).max(0.0).min(1.0);
    }

    fn should_adapt(&self) -> bool {
        self.sync_state.coupling > self.coupling_threshold
            && self.sync_state.phase_diff.abs() > 0.1
            && self.coherence > 0.5
    }

    fn adapt_to_external(&mut self) -> TimingAction {
        // Adjust our period to converge with external
        if let Some(ref external) = self.external_rhythm {
            let period_diff = external.period_ms - self.internal_rhythm.period_ms;
            self.internal_rhythm.period_ms += period_diff * self.adaptation_rate;

            // Also nudge phase
            let phase_adjustment = self.sync_state.phase_diff * self.adaptation_rate;

            TimingAction::Adapt {
                period_delta_ms: period_diff * self.adaptation_rate,
                phase_nudge: phase_adjustment,
            }
        } else {
            TimingAction::Wait {
                until_next_ms: 10.0,
            }
        }
    }

    fn is_action_point(&self) -> bool {
        // Fire at the optimal phase for synchronization
        let optimal = self.optimal_action_phase();
        let current = self.internal_rhythm.phase;
        (current - optimal).abs() < 0.05
    }

    fn ms_until_next_action(&self) -> f64 {
        let optimal = self.optimal_action_phase();
        let current = self.internal_rhythm.phase;
        let phase_delta = (optimal - current).rem_euclid(1.0);
        phase_delta * self.internal_rhythm.period_ms
    }
}

#[derive(Debug)]
pub enum TimingAction {
    /// Fire an action at this moment
    Fire { phase: f64, confidence: f64 },
    /// Wait before next action
    Wait { until_next_ms: f64 },
    /// Adapting rhythm to external source
    Adapt {
        period_delta_ms: f64,
        phase_nudge: f64,
    },
}

fn main() {
    println!("=== Timing Synchronization ===\n");
    println!("Machines that feel timing, not data.\n");

    let mut sync = TimingSynchronizer::new(100.0); // 100ms internal period

    // Simulate external biological rhythm (e.g., 90ms period with noise)
    let external_period = 90.0;
    let start = Instant::now();

    println!("Internal period: 100ms");
    println!("External period: 90ms (simulated biological rhythm)\n");
    println!("Time  | Phase Diff | Sync Quality | Coherence | Action");
    println!("------|------------|--------------|-----------|--------");

    for i in 0..50 {
        let elapsed = Duration::from_millis(i * 20);
        let now = start + elapsed;

        // Generate external rhythm signal (sinusoidal with peaks)
        let external_phase = (elapsed.as_secs_f64() * 1000.0 / external_period) * 2.0 * PI;
        let signal = ((external_phase.sin() + 1.0) / 2.0).powf(4.0); // Sharper peaks

        sync.observe_external(signal, now);
        let action = sync.tick();

        println!(
            "{:5} | {:+.3}      | {:.2}         | {:.2}      | {:?}",
            i * 20,
            sync.sync_state.phase_diff,
            sync.sync_quality(),
            sync.coherence,
            action
        );

        std::thread::sleep(Duration::from_millis(20));
    }

    println!("\n=== Results ===");
    println!(
        "Final internal period: {:.1}ms",
        sync.internal_rhythm.period_ms
    );
    println!("Synchronized: {}", sync.is_synchronized());
    println!("Sync quality: {:.2}", sync.sync_quality());

    println!("\n\"You stop predicting intent. You synchronize with it.\"");
}
