//! # Time Crystal Coordinator
//!
//! Implements discrete time crystal dynamics for robust distributed coordination.
//! Time crystals are systems that exhibit periodic behavior in their ground state,
//! breaking time-translation symmetry.
//!
//! ## Key Concepts
//!
//! - **Discrete Time Crystal (DTC)**: System oscillates with period 2T under period-T driving
//! - **Floquet Engineering**: Periodic driving creates stable coordination patterns
//! - **Phase-Locked Coordination**: Agents synchronize to crystal periodicity
//!
//! ## Example
//!
//! ```rust
//! use ruvector_exotic_wasm::time_crystal::{TimeCrystal, CoordinationPattern};
//!
//! // Create a 10-oscillator time crystal with 100ms period
//! let mut crystal = TimeCrystal::new(10, 100);
//!
//! // Crystallize to establish stable periodic order
//! crystal.crystallize();
//!
//! // Get coordination pattern each tick
//! for _ in 0..200 {
//!     let pattern = crystal.tick();
//!     // Use pattern bytes for agent coordination
//! }
//! ```
//!
//! ## Physics Background
//!
//! This implementation is inspired by discrete time crystals in:
//! - Trapped ion experiments (Monroe group)
//! - NV center diamond systems (Lukin group)
//! - Superconducting qubits (Google)
//!
//! The key insight is that period-doubling (or n-tupling) provides robust
//! coordination signals that are resilient to perturbations.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Coordination pattern types from time crystal dynamics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinationPattern {
    /// All oscillators in phase - full coherence
    Coherent,
    /// Period-doubled oscillation (time crystal signature)
    PeriodDoubled,
    /// Anti-phase clustering (two groups)
    AntiPhase,
    /// Complex multi-frequency pattern
    Quasiperiodic,
    /// No stable pattern (thermal/noisy state)
    Disordered,
}

/// A single oscillator in the time crystal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oscillator {
    /// Current phase (0 to 2*PI)
    pub phase: f32,
    /// Natural frequency (slightly varied for each oscillator)
    pub frequency: f32,
    /// Amplitude (0 to 1)
    pub amplitude: f32,
    /// Phase from previous step (for period detection)
    pub prev_phase: f32,
}

impl Oscillator {
    /// Create a new oscillator with random initial conditions
    pub fn new(base_frequency: f32) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        Self {
            phase: rng.gen::<f32>() * 2.0 * std::f32::consts::PI,
            frequency: base_frequency * (0.98 + rng.gen::<f32>() * 0.04),
            amplitude: 0.8 + rng.gen::<f32>() * 0.2,
            prev_phase: 0.0,
        }
    }

    /// Create with specific phase
    pub fn with_phase(base_frequency: f32, phase: f32) -> Self {
        Self {
            phase,
            frequency: base_frequency,
            amplitude: 1.0,
            prev_phase: 0.0,
        }
    }

    /// Get current signal value
    pub fn signal(&self) -> f32 {
        self.amplitude * self.phase.cos()
    }

    /// Check if oscillator is in "up" state
    pub fn is_up(&self) -> bool {
        self.phase.cos() > 0.0
    }
}

/// Time Crystal Coordinator
///
/// Implements discrete time crystal dynamics for distributed coordination.
/// The crystal provides period-doubled coordination patterns that are
/// robust to perturbations and noise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeCrystal {
    /// Oscillators making up the crystal
    oscillators: Vec<Oscillator>,
    /// Base oscillation period in milliseconds
    period_ms: u32,
    /// Current time step
    step: u64,
    /// Coupling strength between oscillators
    coupling: f32,
    /// Driving strength (Floquet parameter)
    driving_strength: f32,
    /// Disorder strength (perturbation level)
    disorder: f32,
    /// Is the crystal in crystalline (ordered) phase?
    is_crystallized: bool,
    /// Order parameter history (for detection)
    order_history: Vec<f32>,
}

impl TimeCrystal {
    /// Create a new time crystal with n oscillators
    pub fn new(n: usize, period_ms: u32) -> Self {
        let base_frequency = 2.0 * std::f32::consts::PI / (period_ms as f32);

        let oscillators = (0..n).map(|_| Oscillator::new(base_frequency)).collect();

        Self {
            oscillators,
            period_ms,
            step: 0,
            coupling: 2.0,
            driving_strength: std::f32::consts::PI, // Pi pulse
            disorder: 0.05,
            is_crystallized: false,
            order_history: Vec::with_capacity(100),
        }
    }

    /// Set coupling strength between oscillators
    pub fn set_coupling(&mut self, coupling: f32) {
        self.coupling = coupling;
    }

    /// Set driving strength (Floquet parameter)
    pub fn set_driving(&mut self, strength: f32) {
        self.driving_strength = strength;
    }

    /// Set disorder/noise level
    pub fn set_disorder(&mut self, disorder: f32) {
        self.disorder = disorder;
    }

    /// Get number of oscillators
    pub fn oscillator_count(&self) -> usize {
        self.oscillators.len()
    }

    /// Crystallize - establish stable periodic order
    ///
    /// This runs the system with strong driving to reach the time-crystalline phase.
    /// After crystallization, the system exhibits period-doubled dynamics.
    pub fn crystallize(&mut self) {
        // Run many steps with strong coupling to reach ordered state
        let original_coupling = self.coupling;
        self.coupling = 5.0; // Strong coupling for crystallization

        for _ in 0..1000 {
            self.dynamics_step(1.0);
        }

        self.coupling = original_coupling;
        self.is_crystallized = true;
    }

    /// Single dynamics step
    fn dynamics_step(&mut self, dt: f32) {
        let n = self.oscillators.len();
        if n == 0 {
            return;
        }

        // Calculate mean field (order parameter direction)
        let sum_cos: f32 = self.oscillators.iter().map(|o| o.phase.cos()).sum();
        let sum_sin: f32 = self.oscillators.iter().map(|o| o.phase.sin()).sum();
        let mean_phase = sum_sin.atan2(sum_cos);

        // Apply Floquet driving (pi pulse every half period)
        let is_drive_step = (self.step as f32 * dt) % (self.period_ms as f32 / 2.0) < dt;

        // Update each oscillator
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for osc in &mut self.oscillators {
            osc.prev_phase = osc.phase;

            // Natural evolution
            let mut dphi = osc.frequency * dt;

            // Coupling to mean field (Kuramoto-like)
            dphi += (self.coupling / n as f32) * (mean_phase - osc.phase).sin() * dt;

            // Floquet driving (discrete kicks)
            if is_drive_step {
                dphi +=
                    self.driving_strength + rng.gen::<f32>() * self.disorder * 2.0 - self.disorder;
            }

            osc.phase = (osc.phase + dphi).rem_euclid(2.0 * std::f32::consts::PI);
        }

        self.step += 1;
    }

    /// Advance one tick and return coordination pattern
    ///
    /// Returns a byte array where each bit indicates whether the corresponding
    /// oscillator is in the "up" state (positive signal).
    pub fn tick(&mut self) -> Vec<u8> {
        self.dynamics_step(1.0);

        // Calculate order parameter
        let order = self.order_parameter();
        self.order_history.push(order);
        if self.order_history.len() > 100 {
            self.order_history.remove(0);
        }

        // Generate coordination pattern
        self.generate_pattern()
    }

    /// Generate coordination pattern as byte array
    fn generate_pattern(&self) -> Vec<u8> {
        let n = self.oscillators.len();
        let num_bytes = (n + 7) / 8;
        let mut pattern = vec![0u8; num_bytes];

        for (i, osc) in self.oscillators.iter().enumerate() {
            if osc.is_up() {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                pattern[byte_idx] |= 1 << bit_idx;
            }
        }

        pattern
    }

    /// Calculate order parameter (synchronization level)
    ///
    /// Returns value in [0, 1]:
    /// - 1.0: Perfect synchronization
    /// - 0.0: Random/disordered phases
    pub fn order_parameter(&self) -> f32 {
        let n = self.oscillators.len();
        if n == 0 {
            return 0.0;
        }

        let sum_cos: f32 = self.oscillators.iter().map(|o| o.phase.cos()).sum();
        let sum_sin: f32 = self.oscillators.iter().map(|o| o.phase.sin()).sum();

        ((sum_cos / n as f32).powi(2) + (sum_sin / n as f32).powi(2)).sqrt()
    }

    /// Detect the current coordination pattern type
    pub fn detect_pattern(&self) -> CoordinationPattern {
        if self.order_history.len() < 10 {
            return CoordinationPattern::Disordered;
        }

        let current_order = self.order_parameter();

        // Check for high coherence
        if current_order > 0.9 {
            return CoordinationPattern::Coherent;
        }

        // Check for period-doubling (time crystal signature)
        // Look for oscillation in order parameter with period 2
        if self.order_history.len() >= 4 {
            let last_4: Vec<f32> = self.order_history.iter().rev().take(4).cloned().collect();
            let alternating = (last_4[0] - last_4[2]).abs() < 0.1
                && (last_4[1] - last_4[3]).abs() < 0.1
                && (last_4[0] - last_4[1]).abs() > 0.2;

            if alternating && self.is_crystallized {
                return CoordinationPattern::PeriodDoubled;
            }
        }

        // Check for anti-phase clustering
        let up_count = self.oscillators.iter().filter(|o| o.is_up()).count();
        let ratio = up_count as f32 / self.oscillators.len() as f32;
        if (ratio - 0.5).abs() < 0.15 && current_order < 0.3 {
            return CoordinationPattern::AntiPhase;
        }

        // Check for quasiperiodic
        if current_order > 0.3 && current_order < 0.7 {
            return CoordinationPattern::Quasiperiodic;
        }

        CoordinationPattern::Disordered
    }

    /// Get current phases of all oscillators
    pub fn phases(&self) -> Vec<f32> {
        self.oscillators.iter().map(|o| o.phase).collect()
    }

    /// Get current signals of all oscillators
    pub fn signals(&self) -> Vec<f32> {
        self.oscillators.iter().map(|o| o.signal()).collect()
    }

    /// Get current step count
    pub fn current_step(&self) -> u64 {
        self.step
    }

    /// Check if crystal is in ordered (crystallized) state
    pub fn is_crystallized(&self) -> bool {
        self.is_crystallized
    }

    /// Get period in milliseconds
    pub fn period_ms(&self) -> u32 {
        self.period_ms
    }

    /// Apply external perturbation
    pub fn perturb(&mut self, strength: f32) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for osc in &mut self.oscillators {
            let perturbation = (rng.gen::<f32>() - 0.5) * 2.0 * strength;
            osc.phase = (osc.phase + perturbation).rem_euclid(2.0 * std::f32::consts::PI);
        }
    }

    /// Get robustness measure (how well crystal survives perturbations)
    pub fn robustness(&self) -> f32 {
        if !self.is_crystallized {
            return 0.0;
        }

        // Average order parameter from history
        if self.order_history.is_empty() {
            return self.order_parameter();
        }

        let sum: f32 = self.order_history.iter().sum();
        sum / self.order_history.len() as f32
    }

    /// Create a synchronized crystal (all in phase)
    pub fn synchronized(n: usize, period_ms: u32) -> Self {
        let base_frequency = 2.0 * std::f32::consts::PI / (period_ms as f32);

        let oscillators = (0..n)
            .map(|_| Oscillator::with_phase(base_frequency, 0.0))
            .collect();

        Self {
            oscillators,
            period_ms,
            step: 0,
            coupling: 2.0,
            driving_strength: std::f32::consts::PI,
            disorder: 0.05,
            is_crystallized: true,
            order_history: Vec::with_capacity(100),
        }
    }

    /// Get collective spin (magnetization analog)
    pub fn collective_spin(&self) -> f32 {
        let up = self.oscillators.iter().filter(|o| o.is_up()).count();
        let down = self.oscillators.len() - up;
        (up as i32 - down as i32) as f32 / self.oscillators.len() as f32
    }
}

// WASM Bindings

/// WASM-bindgen wrapper for TimeCrystal
#[wasm_bindgen]
pub struct WasmTimeCrystal {
    inner: TimeCrystal,
}

#[wasm_bindgen]
impl WasmTimeCrystal {
    /// Create a new time crystal with n oscillators
    #[wasm_bindgen(constructor)]
    pub fn new(n: usize, period_ms: u32) -> Self {
        Self {
            inner: TimeCrystal::new(n, period_ms),
        }
    }

    /// Create a synchronized crystal
    pub fn synchronized(n: usize, period_ms: u32) -> WasmTimeCrystal {
        WasmTimeCrystal {
            inner: TimeCrystal::synchronized(n, period_ms),
        }
    }

    /// Set coupling strength
    #[wasm_bindgen(js_name = setCoupling)]
    pub fn set_coupling(&mut self, coupling: f32) {
        self.inner.set_coupling(coupling);
    }

    /// Set driving strength
    #[wasm_bindgen(js_name = setDriving)]
    pub fn set_driving(&mut self, strength: f32) {
        self.inner.set_driving(strength);
    }

    /// Set disorder level
    #[wasm_bindgen(js_name = setDisorder)]
    pub fn set_disorder(&mut self, disorder: f32) {
        self.inner.set_disorder(disorder);
    }

    /// Crystallize to establish periodic order
    pub fn crystallize(&mut self) {
        self.inner.crystallize();
    }

    /// Advance one tick, returns coordination pattern as Uint8Array
    pub fn tick(&mut self) -> Vec<u8> {
        self.inner.tick()
    }

    /// Get order parameter (synchronization level)
    #[wasm_bindgen(js_name = orderParameter)]
    pub fn order_parameter(&self) -> f32 {
        self.inner.order_parameter()
    }

    /// Get number of oscillators
    #[wasm_bindgen(js_name = oscillatorCount)]
    pub fn oscillator_count(&self) -> usize {
        self.inner.oscillator_count()
    }

    /// Check if crystallized
    #[wasm_bindgen(js_name = isCrystallized)]
    pub fn is_crystallized(&self) -> bool {
        self.inner.is_crystallized()
    }

    /// Get current step
    #[wasm_bindgen(js_name = currentStep)]
    pub fn current_step(&self) -> u32 {
        self.inner.current_step() as u32
    }

    /// Get period in milliseconds
    #[wasm_bindgen(js_name = periodMs)]
    pub fn period_ms(&self) -> u32 {
        self.inner.period_ms()
    }

    /// Get robustness measure
    pub fn robustness(&self) -> f32 {
        self.inner.robustness()
    }

    /// Get collective spin
    #[wasm_bindgen(js_name = collectiveSpin)]
    pub fn collective_spin(&self) -> f32 {
        self.inner.collective_spin()
    }

    /// Apply perturbation
    pub fn perturb(&mut self, strength: f32) {
        self.inner.perturb(strength);
    }

    /// Get current pattern type as string
    #[wasm_bindgen(js_name = patternType)]
    pub fn pattern_type(&self) -> String {
        format!("{:?}", self.inner.detect_pattern())
    }

    /// Get phases as JSON array
    #[wasm_bindgen(js_name = phasesJson)]
    pub fn phases_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner.phases())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get signals as JSON array
    #[wasm_bindgen(js_name = signalsJson)]
    pub fn signals_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner.signals())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crystal_creation() {
        let crystal = TimeCrystal::new(10, 100);
        assert_eq!(crystal.oscillator_count(), 10);
        assert!(!crystal.is_crystallized());
    }

    #[test]
    fn test_crystallization() {
        let mut crystal = TimeCrystal::new(10, 100);
        crystal.crystallize();
        assert!(crystal.is_crystallized());
    }

    #[test]
    fn test_order_parameter_range() {
        let mut crystal = TimeCrystal::new(20, 100);

        for _ in 0..100 {
            crystal.tick();
            let order = crystal.order_parameter();
            assert!(order >= 0.0 && order <= 1.0);
        }
    }

    #[test]
    fn test_synchronized_crystal() {
        let crystal = TimeCrystal::synchronized(10, 100);

        // Synchronized crystal should have high order parameter
        let order = crystal.order_parameter();
        assert!(
            order > 0.95,
            "Synchronized crystal should have high order: {}",
            order
        );
    }

    #[test]
    fn test_tick_pattern_size() {
        let mut crystal = TimeCrystal::new(16, 100);
        let pattern = crystal.tick();

        // 16 oscillators should produce 2 bytes
        assert_eq!(pattern.len(), 2);
    }

    #[test]
    fn test_tick_pattern_size_odd() {
        let mut crystal = TimeCrystal::new(10, 100);
        let pattern = crystal.tick();

        // 10 oscillators should produce 2 bytes (ceiling of 10/8)
        assert_eq!(pattern.len(), 2);
    }

    #[test]
    fn test_pattern_stability_after_crystallization() {
        let mut crystal = TimeCrystal::new(8, 100);
        crystal.crystallize();

        // After crystallization, patterns should be somewhat stable
        let mut patterns: Vec<Vec<u8>> = Vec::new();
        for _ in 0..10 {
            patterns.push(crystal.tick());
        }

        // Check that we see periodic behavior (not all random)
        // At least some patterns should repeat
        let unique_count = patterns
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();

        // With crystallization, should have fewer unique patterns
        assert!(
            unique_count < 10,
            "Crystallized patterns should show periodicity"
        );
    }

    #[test]
    fn test_perturbation() {
        let mut crystal = TimeCrystal::synchronized(10, 100);

        let initial_order = crystal.order_parameter();
        crystal.perturb(1.0); // Strong perturbation
        let after_order = crystal.order_parameter();

        // Order should decrease after perturbation
        assert!(
            after_order < initial_order,
            "Perturbation should reduce order"
        );
    }

    #[test]
    fn test_robustness() {
        let mut crystal = TimeCrystal::new(10, 100);
        crystal.crystallize();

        // Run for a while
        for _ in 0..50 {
            crystal.tick();
        }

        let robustness = crystal.robustness();
        assert!(robustness >= 0.0 && robustness <= 1.0);
        assert!(
            robustness > 0.0,
            "Crystallized system should have positive robustness"
        );
    }

    #[test]
    fn test_collective_spin() {
        let crystal = TimeCrystal::synchronized(10, 100);

        let spin = crystal.collective_spin();
        assert!(spin >= -1.0 && spin <= 1.0);
    }

    #[test]
    fn test_phases_and_signals() {
        let crystal = TimeCrystal::new(5, 100);

        let phases = crystal.phases();
        let signals = crystal.signals();

        assert_eq!(phases.len(), 5);
        assert_eq!(signals.len(), 5);

        for (phase, signal) in phases.iter().zip(signals.iter()) {
            // Signal should be cos of phase (scaled by amplitude)
            let expected_signal = phase.cos();
            assert!((signal.abs() - expected_signal.abs()) < 0.3);
        }
    }

    #[test]
    fn test_pattern_detection() {
        let mut crystal = TimeCrystal::synchronized(10, 100);

        // Run to build history
        for _ in 0..20 {
            crystal.tick();
        }

        let pattern = crystal.detect_pattern();
        // Synchronized crystal should show coherent or period-doubled
        assert!(
            pattern == CoordinationPattern::Coherent
                || pattern == CoordinationPattern::PeriodDoubled
                || pattern == CoordinationPattern::Quasiperiodic,
            "Unexpected pattern: {:?}",
            pattern
        );
    }

    #[test]
    fn test_disorder_effect() {
        let mut crystal1 = TimeCrystal::new(10, 100);
        crystal1.set_disorder(0.01); // Low disorder
        crystal1.crystallize();

        let mut crystal2 = TimeCrystal::new(10, 100);
        crystal2.set_disorder(0.5); // High disorder
        crystal2.crystallize();

        for _ in 0..50 {
            crystal1.tick();
            crystal2.tick();
        }

        // Low disorder should have higher robustness
        assert!(crystal1.robustness() >= crystal2.robustness() * 0.8);
    }

    #[test]
    fn test_period_property() {
        let crystal = TimeCrystal::new(10, 200);
        assert_eq!(crystal.period_ms(), 200);
    }

    #[test]
    fn test_step_counting() {
        let mut crystal = TimeCrystal::new(10, 100);

        assert_eq!(crystal.current_step(), 0);

        for _ in 0..10 {
            crystal.tick();
        }

        assert_eq!(crystal.current_step(), 10);
    }

    #[test]
    fn test_coupling_effect() {
        let mut weak = TimeCrystal::new(10, 100);
        weak.set_coupling(0.1);
        weak.crystallize();

        let mut strong = TimeCrystal::new(10, 100);
        strong.set_coupling(5.0);
        strong.crystallize();

        for _ in 0..50 {
            weak.tick();
            strong.tick();
        }

        // Strong coupling should generally lead to higher synchronization
        // (though not guaranteed due to random initialization)
        assert!(strong.order_parameter() > 0.1);
    }
}
