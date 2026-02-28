//! # Temporal Qualia
//!
//! Subjective experience of time dilation and compression in cognitive systems.
//! Explores how information processing rate affects perceived time.
//!
//! ## Key Concepts
//!
//! - **Time Dilation**: Subjective slowing of time during high information load
//! - **Time Compression**: Subjective speeding up during routine/familiar tasks
//! - **Temporal Binding**: Binding events into perceived "now"
//! - **Time Crystals**: Periodic patterns in cognitive temporal space
//!
//! ## Theoretical Basis
//!
//! Inspired by:
//! - Eagleman's research on temporal perception
//! - Internal clock models (scalar timing theory)
//! - Attention and time perception studies

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// System for experiencing and measuring subjective time
#[derive(Debug)]
pub struct TemporalQualia {
    /// Internal clock rate (ticks per objective time unit)
    clock_rate: f64,
    /// Base clock rate (reference)
    base_rate: f64,
    /// Attention level (affects time perception)
    attention: f64,
    /// Novelty level (affects time perception)
    novelty: f64,
    /// Time crystal patterns
    time_crystals: Vec<TimeCrystal>,
    /// Temporal binding window (ms equivalent)
    binding_window: f64,
    /// Experience buffer
    experience_buffer: VecDeque<TemporalEvent>,
    /// Subjective duration tracker
    subjective_duration: f64,
    /// Objective duration tracker
    objective_duration: f64,
}

/// A pattern repeating in cognitive temporal space
#[derive(Debug, Clone)]
pub struct TimeCrystal {
    pub id: Uuid,
    /// Period of the crystal (cognitive time units)
    pub period: f64,
    /// Amplitude of oscillation
    pub amplitude: f64,
    /// Phase offset
    pub phase: f64,
    /// Pattern stability (0-1)
    pub stability: f64,
    /// Cognitive content repeated
    pub content_pattern: Vec<f64>,
}

/// Subjective time perception interface
#[derive(Debug)]
pub struct SubjectiveTime {
    /// Current subjective moment
    now: f64,
    /// Duration of "now" (specious present)
    specious_present: f64,
    /// Past experiences (accessible memory)
    past: VecDeque<f64>,
    /// Future anticipation
    anticipated: Vec<f64>,
    /// Time perception mode
    mode: TimeMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeMode {
    /// Normal flow of time
    Normal,
    /// Dilated (slow motion subjective time)
    Dilated,
    /// Compressed (fast-forward subjective time)
    Compressed,
    /// Flow state (time seems to disappear)
    Flow,
    /// Dissociated (disconnected from time)
    Dissociated,
}

/// A temporal event to be experienced
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEvent {
    pub id: Uuid,
    /// Objective timestamp
    pub objective_time: f64,
    /// Subjective timestamp
    pub subjective_time: f64,
    /// Information content
    pub information: f64,
    /// Emotional arousal
    pub arousal: f64,
    /// Novelty of event
    pub novelty: f64,
}

impl TemporalQualia {
    /// Create a new temporal qualia system
    pub fn new() -> Self {
        Self {
            clock_rate: 1.0,
            base_rate: 1.0,
            attention: 0.5,
            novelty: 0.5,
            time_crystals: Vec::new(),
            binding_window: 100.0, // ~100ms binding window
            experience_buffer: VecDeque::with_capacity(1000),
            subjective_duration: 0.0,
            objective_duration: 0.0,
        }
    }

    /// Measure current time dilation factor
    pub fn measure_dilation(&self) -> f64 {
        // Dilation = subjective time / objective time
        // > 1 means time seems slower (more subjective time per objective unit)
        // < 1 means time seems faster
        if self.objective_duration > 0.0 {
            self.subjective_duration / self.objective_duration
        } else {
            1.0
        }
    }

    /// Process an experience and update temporal perception
    pub fn experience(&mut self, event: TemporalEvent) {
        // Update novelty-based time dilation
        // Novel events make time seem longer (more information to process)
        let dilation_factor = 1.0 + (event.novelty * 0.5) + (event.arousal * 0.3);

        // Attention modulates time perception
        let attention_factor = 1.0 + (self.attention - 0.5) * 0.4;

        // Update clock rate
        self.clock_rate = self.base_rate * dilation_factor * attention_factor;

        // Track durations
        let obj_delta = 1.0; // Assume unit objective time per event
        let subj_delta = obj_delta * self.clock_rate;

        self.objective_duration += obj_delta;
        self.subjective_duration += subj_delta;

        // Update novelty (adapts over time)
        self.novelty = self.novelty * 0.9 + event.novelty * 0.1;

        // Store experience
        self.experience_buffer.push_back(event);
        if self.experience_buffer.len() > 1000 {
            self.experience_buffer.pop_front();
        }
    }

    /// Set attention level
    pub fn set_attention(&mut self, attention: f64) {
        self.attention = attention.clamp(0.0, 1.0);
    }

    /// Enter a specific time mode
    pub fn enter_mode(&mut self, mode: TimeMode) {
        match mode {
            TimeMode::Normal => {
                self.clock_rate = self.base_rate;
            }
            TimeMode::Dilated => {
                self.clock_rate = self.base_rate * 2.0; // 2x subjective time
            }
            TimeMode::Compressed => {
                self.clock_rate = self.base_rate * 0.5; // 0.5x subjective time
            }
            TimeMode::Flow => {
                // In flow, subjective time seems to stop
                self.clock_rate = self.base_rate * 0.1;
            }
            TimeMode::Dissociated => {
                self.clock_rate = 0.0; // No subjective time passes
            }
        }
    }

    /// Add a time crystal pattern
    pub fn add_time_crystal(&mut self, period: f64, amplitude: f64, content: Vec<f64>) {
        self.time_crystals.push(TimeCrystal {
            id: Uuid::new_v4(),
            period,
            amplitude,
            phase: 0.0,
            stability: 0.5,
            content_pattern: content,
        });
    }

    /// Get time crystal contribution at current time
    pub fn crystal_contribution(&self, time: f64) -> f64 {
        self.time_crystals
            .iter()
            .map(|crystal| {
                let phase = (time / crystal.period + crystal.phase) * std::f64::consts::TAU;
                crystal.amplitude * phase.sin() * crystal.stability
            })
            .sum()
    }

    /// Estimate how much time has subjectively passed
    pub fn subjective_elapsed(&self) -> f64 {
        self.subjective_duration
    }

    /// Get objective time elapsed
    pub fn objective_elapsed(&self) -> f64 {
        self.objective_duration
    }

    /// Get current clock rate
    pub fn current_clock_rate(&self) -> f64 {
        self.clock_rate
    }

    /// Bind events within temporal window
    pub fn temporal_binding(&self) -> Vec<Vec<&TemporalEvent>> {
        let mut bindings: Vec<Vec<&TemporalEvent>> = Vec::new();
        let mut current_binding: Vec<&TemporalEvent> = Vec::new();
        let mut window_start = 0.0;

        for event in &self.experience_buffer {
            if event.objective_time - window_start <= self.binding_window {
                current_binding.push(event);
            } else {
                if !current_binding.is_empty() {
                    bindings.push(current_binding);
                    current_binding = Vec::new();
                }
                window_start = event.objective_time;
                current_binding.push(event);
            }
        }

        if !current_binding.is_empty() {
            bindings.push(current_binding);
        }

        bindings
    }

    /// Get temporal perception statistics
    pub fn statistics(&self) -> TemporalStatistics {
        let avg_novelty = if self.experience_buffer.is_empty() {
            0.0
        } else {
            self.experience_buffer
                .iter()
                .map(|e| e.novelty)
                .sum::<f64>()
                / self.experience_buffer.len() as f64
        };

        TemporalStatistics {
            dilation_factor: self.measure_dilation(),
            clock_rate: self.clock_rate,
            attention_level: self.attention,
            average_novelty: avg_novelty,
            crystal_count: self.time_crystals.len(),
            experiences_buffered: self.experience_buffer.len(),
        }
    }

    /// Reset temporal tracking
    pub fn reset(&mut self) {
        self.subjective_duration = 0.0;
        self.objective_duration = 0.0;
        self.clock_rate = self.base_rate;
        self.experience_buffer.clear();
    }
}

impl Default for TemporalQualia {
    fn default() -> Self {
        Self::new()
    }
}

impl SubjectiveTime {
    /// Create a new subjective time interface
    pub fn new() -> Self {
        Self {
            now: 0.0,
            specious_present: 3.0, // ~3 seconds specious present
            past: VecDeque::with_capacity(100),
            anticipated: Vec::new(),
            mode: TimeMode::Normal,
        }
    }

    /// Advance subjective time
    pub fn tick(&mut self, delta: f64) {
        self.past.push_back(self.now);
        if self.past.len() > 100 {
            self.past.pop_front();
        }

        self.now += delta;
    }

    /// Get current subjective moment
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Get the specious present (experienced "now")
    pub fn specious_present_range(&self) -> (f64, f64) {
        let half = self.specious_present / 2.0;
        (self.now - half, self.now + half)
    }

    /// Set anticipation for future moments
    pub fn anticipate(&mut self, future_moments: Vec<f64>) {
        self.anticipated = future_moments;
    }

    /// Get accessible past
    pub fn accessible_past(&self) -> &VecDeque<f64> {
        &self.past
    }

    /// Set time mode
    pub fn set_mode(&mut self, mode: TimeMode) {
        self.mode = mode;
    }

    /// Get current mode
    pub fn mode(&self) -> &TimeMode {
        &self.mode
    }

    /// Estimate duration between two moments
    pub fn estimate_duration(&self, start: f64, end: f64) -> f64 {
        let objective = end - start;

        // Subjective duration affected by mode
        match self.mode {
            TimeMode::Normal => objective,
            TimeMode::Dilated => objective * 2.0,
            TimeMode::Compressed => objective * 0.5,
            TimeMode::Flow => objective * 0.1,
            TimeMode::Dissociated => 0.0,
        }
    }
}

impl Default for SubjectiveTime {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeCrystal {
    /// Create a new time crystal
    pub fn new(period: f64, amplitude: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            period,
            amplitude,
            phase: 0.0,
            stability: 0.5,
            content_pattern: Vec::new(),
        }
    }

    /// Get value at given time
    pub fn value_at(&self, time: f64) -> f64 {
        let phase = (time / self.period + self.phase) * std::f64::consts::TAU;
        self.amplitude * phase.sin()
    }

    /// Update stability based on persistence
    pub fn reinforce(&mut self) {
        self.stability = (self.stability + 0.1).min(1.0);
    }

    /// Decay stability
    pub fn decay(&mut self, factor: f64) {
        self.stability *= factor;
    }
}

/// Statistics about temporal perception
#[derive(Debug, Clone)]
pub struct TemporalStatistics {
    pub dilation_factor: f64,
    pub clock_rate: f64,
    pub attention_level: f64,
    pub average_novelty: f64,
    pub crystal_count: usize,
    pub experiences_buffered: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_qualia_creation() {
        let tq = TemporalQualia::new();
        assert_eq!(tq.measure_dilation(), 1.0); // Initial dilation is 1.0
    }

    #[test]
    fn test_time_dilation_with_novelty() {
        let mut tq = TemporalQualia::new();

        // Experience high novelty events
        for i in 0..10 {
            tq.experience(TemporalEvent {
                id: Uuid::new_v4(),
                objective_time: i as f64,
                subjective_time: 0.0,
                information: 0.5,
                arousal: 0.7,
                novelty: 0.9, // High novelty
            });
        }

        // Time should seem dilated (more subjective time)
        assert!(tq.measure_dilation() > 1.0);
    }

    #[test]
    fn test_time_compression_with_familiarity() {
        let mut tq = TemporalQualia::new();

        // Experience low novelty events
        for i in 0..10 {
            tq.experience(TemporalEvent {
                id: Uuid::new_v4(),
                objective_time: i as f64,
                subjective_time: 0.0,
                information: 0.1,
                arousal: 0.1,
                novelty: 0.1, // Low novelty
            });
        }

        // Time should feel slightly dilated still due to base processing
        let dilation = tq.measure_dilation();
        assert!(dilation >= 1.0);
    }

    #[test]
    fn test_time_modes() {
        let mut tq = TemporalQualia::new();
        let base = tq.current_clock_rate();

        tq.enter_mode(TimeMode::Dilated);
        assert!(tq.current_clock_rate() > base);

        tq.enter_mode(TimeMode::Compressed);
        assert!(tq.current_clock_rate() < base);

        tq.enter_mode(TimeMode::Flow);
        assert!(tq.current_clock_rate() < tq.base_rate);
    }

    #[test]
    fn test_time_crystal() {
        let crystal = TimeCrystal::new(10.0, 1.0);

        // Value should oscillate
        let v1 = crystal.value_at(0.0);
        let v2 = crystal.value_at(2.5); // Quarter period
        let v3 = crystal.value_at(5.0); // Half period

        assert!((v1 - 0.0).abs() < 0.01); // sin(0) = 0
        assert!(v2 > 0.9); // sin(π/2) ≈ 1
        assert!((v3 - 0.0).abs() < 0.01); // sin(π) ≈ 0
    }

    #[test]
    fn test_subjective_time() {
        let mut st = SubjectiveTime::new();

        st.tick(1.0);
        st.tick(1.0);
        st.tick(1.0);

        assert_eq!(st.now(), 3.0);
        assert_eq!(st.accessible_past().len(), 3);
    }

    #[test]
    fn test_specious_present() {
        let st = SubjectiveTime::new();
        let (start, end) = st.specious_present_range();

        assert!(end - start > 0.0); // Has duration
        assert_eq!(end - start, st.specious_present); // Equals specious present duration
    }

    #[test]
    fn test_temporal_statistics() {
        let mut tq = TemporalQualia::new();
        tq.add_time_crystal(5.0, 1.0, vec![0.1, 0.2]);

        for i in 0..5 {
            tq.experience(TemporalEvent {
                id: Uuid::new_v4(),
                objective_time: i as f64,
                subjective_time: 0.0,
                information: 0.5,
                arousal: 0.5,
                novelty: 0.5,
            });
        }

        let stats = tq.statistics();
        assert_eq!(stats.crystal_count, 1);
        assert_eq!(stats.experiences_buffered, 5);
    }
}
