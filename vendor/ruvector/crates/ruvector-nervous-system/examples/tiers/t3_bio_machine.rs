//! # Tier 3: Hybrid Biological-Machine Interfaces
//!
//! Assistive tech, rehabilitation, augmentation.
//!
//! ## What Changes
//! - Machine learning adapts to biological timing
//! - Reflex loops integrate with human reflexes
//! - Learning happens through use, not retraining
//!
//! ## Why This Matters
//! - Machines stop fighting biology
//! - Interfaces become intuitive
//! - Ethical and technical alignment improves
//!
//! This is cutting-edge but real.

use std::collections::{HashMap, VecDeque};

/// Biological signal from user
#[derive(Clone, Debug)]
pub struct BioSignal {
    pub timestamp_ms: u64,
    pub signal_type: BioSignalType,
    pub channel: u8,
    pub amplitude: f32,
    pub frequency: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BioSignalType {
    /// Electromyography (muscle)
    EMG,
    /// Electroencephalography (brain)
    EEG,
    /// Electrooculography (eye)
    EOG,
    /// Force sensor
    Force,
    /// Position sensor
    Position,
    /// User intent estimate
    Intent,
}

/// Machine action output
#[derive(Clone, Debug)]
pub struct MachineAction {
    pub timestamp_ms: u64,
    pub action_type: ActionType,
    pub magnitude: f32,
    pub velocity: f32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub enum ActionType {
    /// Motor movement
    Motor { joint: String, target: f32 },
    /// Haptic feedback
    Haptic { pattern: String, intensity: f32 },
    /// Visual feedback
    Visual { indicator: String },
    /// Force assist
    ForceAssist {
        direction: (f32, f32, f32),
        magnitude: f32,
    },
}

/// Biological timing adapter - matches machine timing to neural rhythms
pub struct BiologicalTimingAdapter {
    /// User's natural reaction time (learned)
    pub reaction_time_ms: f32,
    /// User's movement duration preference
    pub movement_duration_ms: f32,
    /// Natural rhythm frequency (Hz)
    pub natural_rhythm_hz: f32,
    /// Adaptation rate
    pub learning_rate: f32,
    /// Timing history for learning
    pub timing_history: VecDeque<(u64, u64)>, // (stimulus, response)
}

impl BiologicalTimingAdapter {
    pub fn new() -> Self {
        Self {
            reaction_time_ms: 200.0, // Default human reaction time
            movement_duration_ms: 500.0,
            natural_rhythm_hz: 1.0, // 1 Hz natural movement
            learning_rate: 0.1,
            timing_history: VecDeque::new(),
        }
    }

    /// Learn from observed stimulus-response timing
    pub fn observe_timing(&mut self, stimulus_time: u64, response_time: u64) {
        let observed_rt = (response_time - stimulus_time) as f32;

        // Update reaction time estimate
        self.reaction_time_ms =
            self.reaction_time_ms * (1.0 - self.learning_rate) + observed_rt * self.learning_rate;

        self.timing_history
            .push_back((stimulus_time, response_time));
        if self.timing_history.len() > 100 {
            self.timing_history.pop_front();
        }

        // Learn natural rhythm from inter-response intervals
        if self.timing_history.len() > 2 {
            let history: Vec<_> = self.timing_history.iter().cloned().collect();
            let intervals: Vec<_> = history
                .windows(2)
                .map(|w| (w[1].1 - w[0].1) as f32)
                .collect();

            if !intervals.is_empty() {
                let avg_interval: f32 = intervals.iter().sum::<f32>() / intervals.len() as f32;
                self.natural_rhythm_hz = 1000.0 / avg_interval;
            }
        }
    }

    /// Get optimal timing for machine response
    pub fn optimal_response_delay(&self, urgency: f32) -> u64 {
        // Higher urgency = faster response, but respect biological limits
        let min_delay = 20.0; // 20ms minimum
        let delay = self.reaction_time_ms * (1.0 - urgency * 0.5);
        delay.max(min_delay) as u64
    }

    /// Get movement duration matched to user
    pub fn matched_duration(&self, distance: f32) -> u64 {
        // Fitts' law inspired: longer movements take longer
        let base = self.movement_duration_ms;
        (base * (1.0 + distance.ln().max(0.0))) as u64
    }
}

/// Reflex integrator - coordinates machine reflexes with user reflexes
pub struct ReflexIntegrator {
    /// User reflex patterns (learned)
    pub user_reflexes: HashMap<String, UserReflexPattern>,
    /// Machine reflex responses
    pub machine_reflexes: Vec<MachineReflex>,
    /// Integration mode
    pub mode: IntegrationMode,
}

#[derive(Clone, Debug)]
pub struct UserReflexPattern {
    pub trigger_signal: BioSignalType,
    pub trigger_threshold: f32,
    pub typical_response_time_ms: f32,
    pub typical_response_magnitude: f32,
    pub observations: u64,
}

pub struct MachineReflex {
    pub name: String,
    pub trigger_threshold: f32,
    pub response: ActionType,
    pub latency_ms: u64,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IntegrationMode {
    /// Machine assists user reflexes
    Assist,
    /// Machine complements (fills gaps)
    Complement,
    /// Machine amplifies user response
    Amplify { gain: f32 },
    /// Machine takes over (user exhausted)
    Takeover,
}

impl ReflexIntegrator {
    pub fn new() -> Self {
        Self {
            user_reflexes: HashMap::new(),
            machine_reflexes: Vec::new(),
            mode: IntegrationMode::Assist,
        }
    }

    /// Learn user reflex pattern
    pub fn learn_user_reflex(&mut self, signal: &BioSignal, response_time: f32, response_mag: f32) {
        let pattern = self
            .user_reflexes
            .entry(format!("{:?}_{}", signal.signal_type, signal.channel))
            .or_insert_with(|| UserReflexPattern {
                trigger_signal: signal.signal_type.clone(),
                trigger_threshold: signal.amplitude,
                typical_response_time_ms: response_time,
                typical_response_magnitude: response_mag,
                observations: 0,
            });

        // Online learning
        let lr = 0.1;
        pattern.typical_response_time_ms =
            pattern.typical_response_time_ms * (1.0 - lr) + response_time * lr;
        pattern.typical_response_magnitude =
            pattern.typical_response_magnitude * (1.0 - lr) + response_mag * lr;
        pattern.observations += 1;
    }

    /// Determine machine response based on user reflex state
    pub fn integrate(&self, signal: &BioSignal, user_responding: bool) -> Option<MachineAction> {
        let pattern_key = format!("{:?}_{}", signal.signal_type, signal.channel);

        match &self.mode {
            IntegrationMode::Assist => {
                // Only help if user is slow
                if !user_responding {
                    if let Some(pattern) = self.user_reflexes.get(&pattern_key) {
                        return Some(MachineAction {
                            timestamp_ms: signal.timestamp_ms,
                            action_type: ActionType::ForceAssist {
                                direction: (0.0, 0.0, 1.0),
                                magnitude: pattern.typical_response_magnitude * 0.5,
                            },
                            magnitude: pattern.typical_response_magnitude * 0.5,
                            velocity: 1.0,
                            duration_ms: 100,
                        });
                    }
                }
            }
            IntegrationMode::Amplify { gain } => {
                // Always amplify user response
                if user_responding {
                    if let Some(pattern) = self.user_reflexes.get(&pattern_key) {
                        return Some(MachineAction {
                            timestamp_ms: signal.timestamp_ms,
                            action_type: ActionType::ForceAssist {
                                direction: (0.0, 0.0, 1.0),
                                magnitude: pattern.typical_response_magnitude * gain,
                            },
                            magnitude: pattern.typical_response_magnitude * gain,
                            velocity: 1.0,
                            duration_ms: 50,
                        });
                    }
                }
            }
            IntegrationMode::Takeover => {
                // Machine handles everything
                return Some(MachineAction {
                    timestamp_ms: signal.timestamp_ms,
                    action_type: ActionType::Motor {
                        joint: "default".to_string(),
                        target: 0.0,
                    },
                    magnitude: 1.0,
                    velocity: 0.5,
                    duration_ms: 200,
                });
            }
            _ => {}
        }

        None
    }
}

/// Intent decoder - learns user intention from patterns
pub struct IntentDecoder {
    /// Signal patterns associated with each intent
    pub intent_patterns: HashMap<String, IntentPattern>,
    /// Recent signals for pattern matching
    pub signal_buffer: VecDeque<BioSignal>,
    /// Confidence threshold for action
    pub confidence_threshold: f32,
}

#[derive(Clone, Debug)]
pub struct IntentPattern {
    pub name: String,
    pub template: Vec<(BioSignalType, f32, f32)>, // (type, amplitude_mean, amplitude_std)
    pub occurrences: u64,
    pub success_rate: f32,
}

impl IntentDecoder {
    pub fn new() -> Self {
        Self {
            intent_patterns: HashMap::new(),
            signal_buffer: VecDeque::new(),
            confidence_threshold: 0.7,
        }
    }

    /// Add signal to buffer
    pub fn observe(&mut self, signal: BioSignal) {
        self.signal_buffer.push_back(signal);
        if self.signal_buffer.len() > 50 {
            self.signal_buffer.pop_front();
        }
    }

    /// Learn intent from labeled example
    pub fn learn_intent(&mut self, intent_name: &str, signals: &[BioSignal]) {
        let template: Vec<_> = signals
            .iter()
            .map(|s| (s.signal_type.clone(), s.amplitude, 0.2)) // Initial std = 0.2
            .collect();

        let pattern = self
            .intent_patterns
            .entry(intent_name.to_string())
            .or_insert_with(|| IntentPattern {
                name: intent_name.to_string(),
                template: template.clone(),
                occurrences: 0,
                success_rate: 0.5,
            });

        pattern.occurrences += 1;

        // Update template with online learning
        for (i, sig) in signals.iter().enumerate() {
            if i < pattern.template.len() {
                let (_, ref mut mean, ref mut std) = pattern.template[i];
                let lr = 0.1;
                *mean = *mean * (1.0 - lr) + sig.amplitude * lr;
                *std = *std * (1.0 - lr) + (sig.amplitude - *mean).abs() * lr;
            }
        }
    }

    /// Decode intent from current buffer
    pub fn decode(&self) -> Option<(String, f32)> {
        if self.signal_buffer.len() < 3 {
            return None;
        }

        let mut best_match: Option<(String, f32)> = None;

        for (name, pattern) in &self.intent_patterns {
            let confidence = self.match_pattern(pattern);

            if confidence > self.confidence_threshold {
                if best_match
                    .as_ref()
                    .map(|(_, c)| confidence > *c)
                    .unwrap_or(true)
                {
                    best_match = Some((name.clone(), confidence));
                }
            }
        }

        best_match
    }

    fn match_pattern(&self, pattern: &IntentPattern) -> f32 {
        if self.signal_buffer.len() < pattern.template.len() {
            return 0.0;
        }

        let recent: Vec<_> = self
            .signal_buffer
            .iter()
            .rev()
            .take(pattern.template.len())
            .collect();

        let mut match_score = 0.0;
        let mut count = 0;

        for (i, (sig_type, mean, std)) in pattern.template.iter().enumerate() {
            if i < recent.len() {
                let signal = recent[i];
                if signal.signal_type == *sig_type {
                    let z = (signal.amplitude - mean).abs() / std.max(0.01);
                    let score = (-z * z / 2.0).exp(); // Gaussian match
                    match_score += score;
                    count += 1;
                }
            }
        }

        if count > 0 {
            match_score / count as f32
        } else {
            0.0
        }
    }

    /// Report feedback on decoded intent
    pub fn feedback(&mut self, intent_name: &str, was_correct: bool) {
        if let Some(pattern) = self.intent_patterns.get_mut(intent_name) {
            let lr = 0.1;
            let target = if was_correct { 1.0 } else { 0.0 };
            pattern.success_rate = pattern.success_rate * (1.0 - lr) + target * lr;
        }
    }
}

/// Complete bio-machine interface
pub struct BioMachineInterface {
    pub name: String,
    pub timing: BiologicalTimingAdapter,
    pub reflexes: ReflexIntegrator,
    pub intent: IntentDecoder,
    pub timestamp: u64,
    /// Adaptation history
    pub adaptation_log: Vec<AdaptationEvent>,
}

#[derive(Clone, Debug)]
pub struct AdaptationEvent {
    pub timestamp: u64,
    pub event_type: String,
    pub old_value: f32,
    pub new_value: f32,
}

impl BioMachineInterface {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            timing: BiologicalTimingAdapter::new(),
            reflexes: ReflexIntegrator::new(),
            intent: IntentDecoder::new(),
            timestamp: 0,
            adaptation_log: Vec::new(),
        }
    }

    /// Process biological signal through the interface
    pub fn process(&mut self, signal: BioSignal) -> Option<MachineAction> {
        self.timestamp = signal.timestamp_ms;

        // 1. Intent decoding
        self.intent.observe(signal.clone());

        if let Some((intent, confidence)) = self.intent.decode() {
            // Intent detected - generate appropriate action
            let delay = self.timing.optimal_response_delay(confidence);

            return Some(MachineAction {
                timestamp_ms: self.timestamp + delay,
                action_type: ActionType::Haptic {
                    pattern: format!("intent_{}", intent),
                    intensity: confidence,
                },
                magnitude: confidence,
                velocity: 1.0,
                duration_ms: self.timing.matched_duration(1.0),
            });
        }

        // 2. Reflex integration
        // Check if user is responding (simplified)
        let user_responding = signal.amplitude > 0.3;

        if let Some(action) = self.reflexes.integrate(&signal, user_responding) {
            return Some(action);
        }

        None
    }

    /// Learn from user interaction
    pub fn learn(&mut self, signal: &BioSignal, response_time: f32, was_successful: bool) {
        let old_rt = self.timing.reaction_time_ms;

        self.timing.observe_timing(
            signal.timestamp_ms,
            signal.timestamp_ms + response_time as u64,
        );

        self.reflexes
            .learn_user_reflex(signal, response_time, signal.amplitude);

        // Log adaptation
        if (old_rt - self.timing.reaction_time_ms).abs() > 5.0 {
            self.adaptation_log.push(AdaptationEvent {
                timestamp: self.timestamp,
                event_type: "reaction_time".to_string(),
                old_value: old_rt,
                new_value: self.timing.reaction_time_ms,
            });
        }
    }

    /// Get interface status
    pub fn status(&self) -> InterfaceStatus {
        InterfaceStatus {
            adapted_reaction_time_ms: self.timing.reaction_time_ms,
            natural_rhythm_hz: self.timing.natural_rhythm_hz,
            integration_mode: self.reflexes.mode.clone(),
            known_intents: self.intent.intent_patterns.len(),
            known_reflexes: self.reflexes.user_reflexes.len(),
            adaptations_made: self.adaptation_log.len(),
        }
    }
}

#[derive(Debug)]
pub struct InterfaceStatus {
    pub adapted_reaction_time_ms: f32,
    pub natural_rhythm_hz: f32,
    pub integration_mode: IntegrationMode,
    pub known_intents: usize,
    pub known_reflexes: usize,
    pub adaptations_made: usize,
}

fn main() {
    println!("=== Tier 3: Hybrid Biological-Machine Interfaces ===\n");

    let mut interface = BioMachineInterface::new("Prosthetic Arm");

    // Register some intents
    println!("Learning user intents...");
    for i in 0..20 {
        // Simulate grip intent pattern
        let grip_signals = vec![
            BioSignal {
                timestamp_ms: i * 1000,
                signal_type: BioSignalType::EMG,
                channel: 0,
                amplitude: 0.8 + (i as f32 * 0.1).sin() * 0.1,
                frequency: Some(150.0),
            },
            BioSignal {
                timestamp_ms: i * 1000 + 50,
                signal_type: BioSignalType::EMG,
                channel: 1,
                amplitude: 0.6 + (i as f32 * 0.1).sin() * 0.1,
                frequency: Some(120.0),
            },
        ];
        interface.intent.learn_intent("grip", &grip_signals);

        // Simulate release intent
        let release_signals = vec![BioSignal {
            timestamp_ms: i * 1000,
            signal_type: BioSignalType::EMG,
            channel: 0,
            amplitude: 0.2,
            frequency: Some(50.0),
        }];
        interface.intent.learn_intent("release", &release_signals);
    }

    println!(
        "  Intents learned: {}",
        interface.intent.intent_patterns.len()
    );

    // Simulate usage to adapt timing
    println!("\nAdapting to user timing...");
    for i in 0..50 {
        let signal = BioSignal {
            timestamp_ms: i * 500,
            signal_type: BioSignalType::EMG,
            channel: 0,
            amplitude: 0.7,
            frequency: Some(100.0),
        };

        // Simulate user response time varying around 180ms
        let response_time = 180.0 + (i as f32 * 0.2).sin() * 20.0;

        interface.learn(&signal, response_time, true);

        if i % 10 == 0 {
            println!(
                "  Step {}: adapted RT = {:.1}ms",
                i, interface.timing.reaction_time_ms
            );
        }
    }

    // Test intent decoding
    println!("\nTesting intent decoding...");

    // Grip intent
    for _ in 0..3 {
        interface.intent.observe(BioSignal {
            timestamp_ms: interface.timestamp + 10,
            signal_type: BioSignalType::EMG,
            channel: 0,
            amplitude: 0.75,
            frequency: Some(140.0),
        });
    }

    if let Some((intent, confidence)) = interface.intent.decode() {
        println!(
            "  Decoded intent: {} (confidence: {:.2})",
            intent, confidence
        );
    }

    // Test machine action generation
    println!("\nGenerating machine actions...");
    let signal = BioSignal {
        timestamp_ms: interface.timestamp + 100,
        signal_type: BioSignalType::EMG,
        channel: 0,
        amplitude: 0.8,
        frequency: Some(150.0),
    };

    if let Some(action) = interface.process(signal) {
        println!("  Action: {:?}", action.action_type);
        println!(
            "  Timing: delay={}ms, duration={}ms",
            action.timestamp_ms - interface.timestamp,
            action.duration_ms
        );
    }

    // Change integration mode
    println!("\nChanging to amplification mode...");
    interface.reflexes.mode = IntegrationMode::Amplify { gain: 1.5 };

    let status = interface.status();
    println!("\n=== Interface Status ===");
    println!(
        "  Adapted reaction time: {:.1}ms",
        status.adapted_reaction_time_ms
    );
    println!("  Natural rhythm: {:.2}Hz", status.natural_rhythm_hz);
    println!("  Integration mode: {:?}", status.integration_mode);
    println!("  Known intents: {}", status.known_intents);
    println!("  Known reflexes: {}", status.known_reflexes);
    println!("  Adaptations made: {}", status.adaptations_made);

    println!("\n=== Key Benefits ===");
    println!("- Machine timing adapts to biological rhythms");
    println!("- Reflex loops integrate with human reflexes");
    println!("- Learning happens through use, not retraining");
    println!("- Machines stop fighting biology");
    println!("- Interfaces become intuitive over time");
    println!("\nThis is cutting-edge but real.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_adaptation() {
        let mut adapter = BiologicalTimingAdapter::new();

        // Initial reaction time
        let initial = adapter.reaction_time_ms;

        // Learn faster reaction times
        for i in 0..10 {
            adapter.observe_timing(i * 1000, i * 1000 + 150);
        }

        assert!(adapter.reaction_time_ms < initial);
    }

    #[test]
    fn test_intent_learning() {
        let mut decoder = IntentDecoder::new();

        let signals = vec![BioSignal {
            timestamp_ms: 0,
            signal_type: BioSignalType::EMG,
            channel: 0,
            amplitude: 0.8,
            frequency: None,
        }];

        decoder.learn_intent("test", &signals);
        assert!(decoder.intent_patterns.contains_key("test"));
    }

    #[test]
    fn test_reflex_integration() {
        let mut integrator = ReflexIntegrator::new();
        integrator.mode = IntegrationMode::Assist;

        // Learn a user reflex
        let signal = BioSignal {
            timestamp_ms: 0,
            signal_type: BioSignalType::EMG,
            channel: 0,
            amplitude: 0.5,
            frequency: None,
        };

        integrator.learn_user_reflex(&signal, 200.0, 0.8);

        // When user not responding, machine should assist
        let action = integrator.integrate(&signal, false);
        assert!(action.is_some());

        // When user is responding, no assist needed
        let action = integrator.integrate(&signal, true);
        assert!(action.is_none());
    }
}
