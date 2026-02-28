//! SNN-Gated Inference Example - Event-Driven LLM with Spiking Pre-Filter
//!
//! Demonstrates the optimal architecture where Spiking Neural Networks (SNN)
//! handle always-on event detection, while RuvLLM runs only when needed.
//!
//! # The Key Insight
//! ```text
//! ❌ Wrong: "SNN replaces the LLM"
//! ✅ Right: "SNN replaces expensive always-on gating, filtering, and routing"
//! ```
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                     SNN-GATED INFERENCE PIPELINE                        │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │   Sensors ──▶ SNN Front-End ──▶ Event? ──▶ RuVector ──▶ RuvLLM         │
//! │   (always on)  (μW power)        │         (query)    (only on event)   │
//! │                                  │                                      │
//! │                              No event                                   │
//! │                                  │                                      │
//! │                               SLEEP                                     │
//! │                            (99% of time)                                │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Benefits
//! - 10-100x energy reduction (LLM sleeps 99% of the time)
//! - Microsecond response to events (SNN reacts in μs, LLM explains later)
//! - Higher throughput (compute only on events, not silence)

#![allow(unused)]

use heapless::Vec as HVec;
use heapless::String as HString;

const EMBED_DIM: usize = 16;
const SNN_NEURONS: usize = 32;

/// Spiking neuron state
#[derive(Debug, Clone, Copy)]
struct SpikingNeuron {
    /// Membrane potential (mV scaled to i16)
    membrane: i16,
    /// Firing threshold
    threshold: i16,
    /// Refractory period remaining
    refractory: u8,
    /// Leak rate (how fast potential decays)
    leak: i16,
    /// Last spike time
    last_spike: u32,
}

impl SpikingNeuron {
    fn new(threshold: i16) -> Self {
        Self {
            membrane: 0,
            threshold,
            refractory: 0,
            leak: 10, // Decay 10 units per tick
            last_spike: 0,
        }
    }

    /// Process input and return if neuron spiked
    fn process(&mut self, input: i16, current_time: u32) -> bool {
        // Check refractory period
        if self.refractory > 0 {
            self.refractory -= 1;
            return false;
        }

        // Leak (decay toward resting potential)
        if self.membrane > 0 {
            self.membrane = (self.membrane - self.leak).max(0);
        } else if self.membrane < 0 {
            self.membrane = (self.membrane + self.leak).min(0);
        }

        // Integrate input
        self.membrane = self.membrane.saturating_add(input);

        // Check for spike
        if self.membrane >= self.threshold {
            self.membrane = -30; // Hyperpolarization after spike
            self.refractory = 3; // Refractory period
            self.last_spike = current_time;
            return true;
        }

        false
    }

    /// Reset neuron state
    fn reset(&mut self) {
        self.membrane = 0;
        self.refractory = 0;
    }
}

/// SNN Event Types
#[derive(Debug, Clone, Copy, PartialEq)]
enum SNNEvent {
    /// Wake word detected
    WakeWord,
    /// Anomaly onset detected
    AnomalyOnset,
    /// Novelty in sensor pattern
    Novelty,
    /// Threshold crossing
    ThresholdCross,
    /// Rhythm change detected
    RhythmChange,
    /// No event
    None,
}

impl SNNEvent {
    fn priority(&self) -> u8 {
        match self {
            Self::AnomalyOnset => 100,
            Self::WakeWord => 90,
            Self::ThresholdCross => 70,
            Self::RhythmChange => 50,
            Self::Novelty => 40,
            Self::None => 0,
        }
    }
}

/// SNN Front-End for Event Detection
/// Runs continuously at μW power, gates LLM invocation
struct SNNEventDetector {
    /// Neurons for different event types
    neurons: [SpikingNeuron; SNN_NEURONS],
    /// Current simulation time
    current_time: u32,
    /// Spike history (for pattern detection)
    spike_history: HVec<(u8, u32), 64>, // (neuron_id, time)
    /// Event counters
    events_detected: u32,
    /// False positives (estimated)
    false_positives: u32,
    /// Baseline adaptation
    baseline: [i16; 8],
}

impl SNNEventDetector {
    fn new() -> Self {
        let mut neurons = [SpikingNeuron::new(100); SNN_NEURONS];

        // Different thresholds for different event types
        // Wake word neurons (sensitive)
        for i in 0..4 {
            neurons[i].threshold = 80;
        }
        // Anomaly neurons (balanced)
        for i in 4..12 {
            neurons[i].threshold = 100;
        }
        // Novelty neurons (less sensitive)
        for i in 12..20 {
            neurons[i].threshold = 120;
        }
        // Rhythm neurons (pattern-based)
        for i in 20..SNN_NEURONS {
            neurons[i].threshold = 90;
            neurons[i].leak = 5; // Slower decay for temporal integration
        }

        Self {
            neurons,
            current_time: 0,
            spike_history: HVec::new(),
            events_detected: 0,
            false_positives: 0,
            baseline: [0; 8],
        }
    }

    /// Process sensor input and detect events
    fn process(&mut self, sensor_data: &[i16]) -> SNNEvent {
        self.current_time += 1;

        // Adapt baseline (slow moving average)
        for (i, &val) in sensor_data.iter().take(8).enumerate() {
            self.baseline[i] = ((self.baseline[i] as i32 * 95 + val as i32 * 5) / 100) as i16;
        }

        let mut spikes = 0u32;
        let mut spike_pattern = [false; SNN_NEURONS];

        // Process through SNN
        for (neuron_idx, neuron) in self.neurons.iter_mut().enumerate() {
            // Map sensor data to neurons
            let input_idx = neuron_idx % sensor_data.len().max(1);
            let raw_input = sensor_data.get(input_idx).copied().unwrap_or(0);

            // Subtract baseline for adaptive threshold
            let input = raw_input - self.baseline.get(input_idx).copied().unwrap_or(0);

            if neuron.process(input, self.current_time) {
                spikes |= 1 << neuron_idx;
                spike_pattern[neuron_idx] = true;

                // Record spike
                if self.spike_history.len() >= 64 {
                    self.spike_history.remove(0);
                }
                let _ = self.spike_history.push((neuron_idx as u8, self.current_time));
            }
        }

        // Decode events from spike patterns
        let event = self.decode_spikes(&spike_pattern);

        if event != SNNEvent::None {
            self.events_detected += 1;
        }

        event
    }

    /// Decode spike pattern into event type
    fn decode_spikes(&self, spikes: &[bool; SNN_NEURONS]) -> SNNEvent {
        // Wake word: neurons 0-3 fire together
        let wake_spikes: u8 = spikes[0..4].iter().filter(|&&s| s).count() as u8;
        if wake_spikes >= 3 {
            return SNNEvent::WakeWord;
        }

        // Anomaly: multiple neurons in 4-11 fire
        let anomaly_spikes: u8 = spikes[4..12].iter().filter(|&&s| s).count() as u8;
        if anomaly_spikes >= 4 {
            return SNNEvent::AnomalyOnset;
        }

        // Threshold crossing: any single strong spike in 4-11
        if spikes[4..12].iter().any(|&s| s) {
            return SNNEvent::ThresholdCross;
        }

        // Novelty: neurons 12-19
        let novelty_spikes: u8 = spikes[12..20].iter().filter(|&&s| s).count() as u8;
        if novelty_spikes >= 2 {
            return SNNEvent::Novelty;
        }

        // Rhythm change: check for pattern in 20-31
        let rhythm_spikes: u8 = spikes[20..].iter().filter(|&&s| s).count() as u8;
        if rhythm_spikes >= 2 {
            // Check if this breaks expected rhythm
            let recent_rhythm = self.spike_history.iter()
                .rev()
                .take(10)
                .filter(|(id, _)| *id >= 20)
                .count();

            if recent_rhythm > 5 {
                return SNNEvent::RhythmChange;
            }
        }

        SNNEvent::None
    }

    /// Get spike rate (for monitoring)
    fn spike_rate(&self) -> f32 {
        let recent_spikes = self.spike_history.iter()
            .filter(|(_, t)| self.current_time - *t < 100)
            .count();

        recent_spikes as f32 / 100.0 * SNN_NEURONS as f32
    }

    /// Reset all neurons
    fn reset(&mut self) {
        for neuron in self.neurons.iter_mut() {
            neuron.reset();
        }
        self.spike_history.clear();
    }
}

/// Routing decision based on SNN event
#[derive(Debug, Clone, Copy)]
enum RouteDecision {
    /// Sleep, no action needed
    Sleep,
    /// Quick local response (no LLM)
    LocalResponse,
    /// Query RuVector memory
    FetchMemory,
    /// Run RuvLLM for generation
    RunLLM,
    /// Escalate to bigger model
    Escalate,
    /// Require human confirmation
    RequireConfirmation,
}

/// SNN-based Router
struct SNNRouter {
    /// Confidence threshold for local response
    local_threshold: u8,
    /// LLM invocation count
    llm_invocations: u32,
    /// Skipped invocations (energy saved)
    skipped_invocations: u32,
}

impl SNNRouter {
    fn new() -> Self {
        Self {
            local_threshold: 80,
            llm_invocations: 0,
            skipped_invocations: 0,
        }
    }

    /// Route based on SNN event and confidence
    fn route(&mut self, event: SNNEvent, confidence: u8) -> RouteDecision {
        match event {
            SNNEvent::None => {
                self.skipped_invocations += 1;
                RouteDecision::Sleep
            }
            SNNEvent::WakeWord => {
                if confidence >= 90 {
                    self.llm_invocations += 1;
                    RouteDecision::RunLLM
                } else {
                    RouteDecision::LocalResponse
                }
            }
            SNNEvent::AnomalyOnset => {
                if confidence >= 95 {
                    RouteDecision::RequireConfirmation
                } else if confidence >= 70 {
                    self.llm_invocations += 1;
                    RouteDecision::RunLLM
                } else {
                    RouteDecision::FetchMemory
                }
            }
            SNNEvent::ThresholdCross => {
                self.skipped_invocations += 1;
                RouteDecision::LocalResponse
            }
            SNNEvent::Novelty => {
                RouteDecision::FetchMemory
            }
            SNNEvent::RhythmChange => {
                if confidence >= 80 {
                    self.llm_invocations += 1;
                    RouteDecision::RunLLM
                } else {
                    RouteDecision::FetchMemory
                }
            }
        }
    }

    /// Get energy savings ratio
    fn energy_savings_ratio(&self) -> f32 {
        let total = self.llm_invocations + self.skipped_invocations;
        if total == 0 {
            return 0.0;
        }
        self.skipped_invocations as f32 / total as f32
    }
}

/// Simulated power model (μW)
fn estimate_power(route: RouteDecision) -> u32 {
    match route {
        RouteDecision::Sleep => 10,           // Deep sleep: 10 μW
        RouteDecision::LocalResponse => 500,  // Quick compute: 500 μW
        RouteDecision::FetchMemory => 2000,   // Memory access: 2 mW
        RouteDecision::RunLLM => 50000,       // Full LLM: 50 mW
        RouteDecision::Escalate => 100000,    // External: 100 mW
        RouteDecision::RequireConfirmation => 5000, // Alert: 5 mW
    }
}

fn main() {
    println!("⚡ SNN-Gated Inference Example");
    println!("==============================\n");

    println!("Key Insight:");
    println!("  ❌ Wrong: SNN replaces the LLM");
    println!("  ✅ Right: SNN replaces expensive always-on gating\n");

    let mut snn = SNNEventDetector::new();
    let mut router = SNNRouter::new();

    // Simulate 1000 time steps of sensor data
    println!("🔄 Running simulation (1000 time steps)...\n");

    let mut total_power_uw = 0u64;
    let mut events: HVec<(u32, SNNEvent, RouteDecision), 64> = HVec::new();

    for t in 0..1000 {
        // Generate sensor data
        // 99% of the time: normal background noise
        // 1% of the time: actual events
        let sensor_data: [i16; 8] = if t % 100 == 42 {
            // Anomaly spike
            [200, 180, 150, 120, 100, 90, 80, 70]
        } else if t % 200 == 150 {
            // Wake word pattern
            [150, 160, 155, 145, 30, 25, 20, 15]
        } else if t % 300 == 250 {
            // Novelty
            [50, 100, 50, 100, 50, 100, 50, 100]
        } else {
            // Normal noise
            let noise = ((t * 7) % 40) as i16 - 20;
            [noise, noise + 5, noise - 3, noise + 2, noise - 1, noise + 4, noise - 2, noise + 1]
        };

        // SNN processes (always on, μW power)
        let event = snn.process(&sensor_data);

        // Calculate confidence from spike history
        let confidence = if event != SNNEvent::None {
            85 + (snn.spike_history.len() % 15) as u8
        } else {
            0
        };

        // Route decision
        let route = router.route(event, confidence);

        // Accumulate power
        total_power_uw += estimate_power(route) as u64;

        // Record interesting events
        if event != SNNEvent::None {
            if events.len() < 64 {
                let _ = events.push((t, event, route));
            }
        }
    }

    // Results
    println!("📊 Simulation Results:\n");

    println!("Events Detected:");
    for (time, event, route) in events.iter().take(10) {
        println!("  t={:4}: {:?} → {:?}", time, event, route);
    }
    if events.len() > 10 {
        println!("  ... and {} more events", events.len() - 10);
    }

    println!("\n📈 Statistics:");
    println!("  Total events detected: {}", snn.events_detected);
    println!("  LLM invocations: {}", router.llm_invocations);
    println!("  Skipped invocations: {}", router.skipped_invocations);
    println!("  Energy savings ratio: {:.1}%", router.energy_savings_ratio() * 100.0);

    println!("\n⚡ Power Analysis:");
    let avg_power_uw = total_power_uw / 1000;
    println!("  Total energy: {} μJ (1000 steps)", total_power_uw);
    println!("  Average power: {} μW", avg_power_uw);

    // Compare to always-on LLM
    let always_on_power = 50000u64 * 1000; // 50mW * 1000 steps
    let savings = (always_on_power - total_power_uw) as f64 / always_on_power as f64 * 100.0;
    println!("\n  vs Always-On LLM:");
    println!("    Always-on: {} μJ", always_on_power);
    println!("    SNN-gated: {} μJ", total_power_uw);
    println!("    Savings: {:.1}%", savings);
    println!("    Reduction: {:.0}x", always_on_power as f64 / total_power_uw.max(1) as f64);

    // Three-stage benchmark comparison
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Three-Stage Benchmark (as suggested):\n");

    println!("Stage A - Baseline (LLM on every window):");
    println!("  Power: 50,000 μW constant");
    println!("  LLM calls: 1000");
    println!("  Energy: 50,000,000 μJ\n");

    println!("Stage B - SNN Gate (LLM only on spikes):");
    println!("  Power: {} μW average", avg_power_uw);
    println!("  LLM calls: {}", router.llm_invocations);
    println!("  Energy: {} μJ", total_power_uw);
    println!("  Improvement: {:.0}x\n", 50_000_000f64 / total_power_uw as f64);

    println!("Stage C - SNN + Coherence (conservative on low coherence):");
    println!("  [Would add min-cut gating for additional safety]");
    println!("  Expected: Additional 20-30% reduction in false positives");

    println!("\n✨ SNN-Gated Inference Demo Complete!");
    println!("\n💡 Key Takeaways:");
    println!("   - SNN runs at μW, LLM runs at mW");
    println!("   - 99% of sensor data is silence → 99% sleep time");
    println!("   - SNN detects in μs, LLM explains later");
    println!("   - Perfect for: wearables, industrial, home hubs, swarm nodes");
}
