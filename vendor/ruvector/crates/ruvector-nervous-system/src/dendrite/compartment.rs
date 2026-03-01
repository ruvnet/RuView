//! Single compartment model with membrane and calcium dynamics
//!
//! Implements a reduced compartment with:
//! - Membrane potential with exponential decay
//! - Calcium concentration with slower decay
//! - Threshold-based activation detection

/// Single compartment with membrane and calcium dynamics
#[derive(Debug, Clone)]
pub struct Compartment {
    /// Membrane potential (normalized 0.0-1.0)
    membrane: f32,

    /// Calcium concentration (normalized 0.0-1.0)
    calcium: f32,

    /// Membrane time constant (ms)
    tau_membrane: f32,

    /// Calcium time constant (ms)
    tau_calcium: f32,

    /// Resting potential
    resting: f32,
}

impl Compartment {
    /// Create a new compartment with default parameters
    ///
    /// Default values:
    /// - tau_membrane: 20ms (fast membrane dynamics)
    /// - tau_calcium: 100ms (slower calcium decay)
    /// - resting: 0.0 (normalized)
    pub fn new() -> Self {
        Self {
            membrane: 0.0,
            calcium: 0.0,
            tau_membrane: 20.0,
            tau_calcium: 100.0,
            resting: 0.0,
        }
    }

    /// Create a compartment with custom time constants
    pub fn with_time_constants(tau_membrane: f32, tau_calcium: f32) -> Self {
        Self {
            membrane: 0.0,
            calcium: 0.0,
            tau_membrane,
            tau_calcium,
            resting: 0.0,
        }
    }

    /// Update compartment state with input current
    ///
    /// Implements exponential decay for both membrane potential and calcium:
    /// - dV/dt = (I - V) / tau_membrane
    /// - dCa/dt = -Ca / tau_calcium
    ///
    /// # Arguments
    /// * `input_current` - Input current (normalized, positive depolarizes)
    /// * `dt` - Time step in milliseconds
    pub fn step(&mut self, input_current: f32, dt: f32) {
        // Membrane dynamics: exponential decay towards resting + input
        let membrane_decay = (self.resting - self.membrane) / self.tau_membrane;
        self.membrane += (membrane_decay + input_current) * dt;

        // Clamp membrane potential to [0.0, 1.0]
        self.membrane = self.membrane.clamp(0.0, 1.0);

        // Calcium dynamics: exponential decay
        let calcium_decay = -self.calcium / self.tau_calcium;
        self.calcium += calcium_decay * dt;

        // Calcium increases with strong depolarization
        if self.membrane > 0.5 {
            self.calcium += (self.membrane - 0.5) * 0.01 * dt;
        }

        // Clamp calcium to [0.0, 1.0]
        self.calcium = self.calcium.clamp(0.0, 1.0);
    }

    /// Check if compartment is active above threshold
    pub fn is_active(&self, threshold: f32) -> bool {
        self.membrane > threshold
    }

    /// Get current membrane potential
    pub fn membrane(&self) -> f32 {
        self.membrane
    }

    /// Get current calcium concentration
    pub fn calcium(&self) -> f32 {
        self.calcium
    }

    /// Reset compartment to resting state
    pub fn reset(&mut self) {
        self.membrane = self.resting;
        self.calcium = 0.0;
    }

    /// Inject a spike into the compartment
    pub fn inject_spike(&mut self, amplitude: f32) {
        self.membrane += amplitude;
        self.membrane = self.membrane.clamp(0.0, 1.0);
    }
}

impl Default for Compartment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compartment_creation() {
        let comp = Compartment::new();
        assert_eq!(comp.membrane(), 0.0);
        assert_eq!(comp.calcium(), 0.0);
    }

    #[test]
    fn test_compartment_step() {
        let mut comp = Compartment::new();

        // Apply positive current
        comp.step(0.1, 1.0);
        assert!(comp.membrane() > 0.0);
    }

    #[test]
    fn test_membrane_decay() {
        let mut comp = Compartment::new();

        // Inject spike
        comp.inject_spike(0.8);
        let initial = comp.membrane();

        // Let it decay
        for _ in 0..100 {
            comp.step(0.0, 1.0);
        }

        // Should decay towards resting
        assert!(comp.membrane() < initial);
    }

    #[test]
    fn test_calcium_accumulation() {
        let mut comp = Compartment::new();

        // Strong depolarization should increase calcium
        comp.inject_spike(0.9);

        for _ in 0..10 {
            comp.step(0.0, 1.0);
        }

        assert!(comp.calcium() > 0.0);
    }

    #[test]
    fn test_threshold_detection() {
        let mut comp = Compartment::new();
        assert!(!comp.is_active(0.5));

        comp.inject_spike(0.6);
        assert!(comp.is_active(0.5));
    }

    #[test]
    fn test_reset() {
        let mut comp = Compartment::new();
        comp.inject_spike(0.8);
        comp.step(0.0, 1.0);

        comp.reset();
        assert_eq!(comp.membrane(), 0.0);
        assert_eq!(comp.calcium(), 0.0);
    }
}
