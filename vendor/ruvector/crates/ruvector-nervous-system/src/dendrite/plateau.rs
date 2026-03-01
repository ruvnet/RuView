//! Dendritic plateau potential for behavioral timescale synaptic plasticity
//!
//! Implements a plateau potential that:
//! - Activates when NMDA threshold is reached
//! - Lasts 100-500ms (behavioral timescale)
//! - Provides temporal credit assignment signal for BTSP

/// Dendritic plateau potential
#[derive(Debug, Clone)]
pub struct PlateauPotential {
    /// Duration of plateau in milliseconds
    duration_ms: f32,

    /// Time remaining in current plateau (ms)
    time_remaining: f32,

    /// Amplitude of plateau (0.0-1.0)
    amplitude: f32,

    /// Whether plateau is currently active
    active: bool,
}

impl PlateauPotential {
    /// Create a new plateau potential with specified duration
    ///
    /// # Arguments
    /// * `duration_ms` - Duration of plateau in milliseconds (typically 100-500ms)
    pub fn new(duration_ms: f32) -> Self {
        Self {
            duration_ms,
            time_remaining: 0.0,
            amplitude: 0.0,
            active: false,
        }
    }

    /// Trigger the plateau potential
    ///
    /// Initiates a plateau with full amplitude and resets the timer
    pub fn trigger(&mut self) {
        self.active = true;
        self.time_remaining = self.duration_ms;
        self.amplitude = 1.0;
    }

    /// Update plateau state
    ///
    /// Decrements timer and updates amplitude. Deactivates when time expires.
    ///
    /// # Arguments
    /// * `dt` - Time step in milliseconds
    pub fn update(&mut self, dt: f32) {
        if !self.active {
            return;
        }

        self.time_remaining -= dt;

        if self.time_remaining <= 0.0 {
            // Plateau expired
            self.active = false;
            self.amplitude = 0.0;
            self.time_remaining = 0.0;
        } else {
            // Maintain amplitude during plateau
            // Could implement decay here if needed
            self.amplitude = 1.0;
        }
    }

    /// Check if plateau is currently active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get current amplitude (0.0-1.0)
    pub fn amplitude(&self) -> f32 {
        self.amplitude
    }

    /// Get time remaining in plateau (ms)
    pub fn time_remaining(&self) -> f32 {
        self.time_remaining
    }

    /// Reset plateau to inactive state
    pub fn reset(&mut self) {
        self.active = false;
        self.amplitude = 0.0;
        self.time_remaining = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plateau_creation() {
        let plateau = PlateauPotential::new(200.0);
        assert!(!plateau.is_active());
        assert_eq!(plateau.amplitude(), 0.0);
    }

    #[test]
    fn test_plateau_trigger() {
        let mut plateau = PlateauPotential::new(200.0);

        plateau.trigger();

        assert!(plateau.is_active());
        assert_eq!(plateau.amplitude(), 1.0);
        assert_eq!(plateau.time_remaining(), 200.0);
    }

    #[test]
    fn test_plateau_duration() {
        let mut plateau = PlateauPotential::new(100.0);

        plateau.trigger();

        // Update 50ms
        plateau.update(50.0);
        assert!(plateau.is_active());
        assert_eq!(plateau.time_remaining(), 50.0);

        // Update another 60ms - should expire
        plateau.update(60.0);
        assert!(!plateau.is_active());
        assert_eq!(plateau.amplitude(), 0.0);
    }

    #[test]
    fn test_plateau_maintains_amplitude() {
        let mut plateau = PlateauPotential::new(200.0);

        plateau.trigger();

        // Amplitude should remain at 1.0 during active period
        for _ in 0..10 {
            plateau.update(10.0);
            if plateau.is_active() {
                assert_eq!(plateau.amplitude(), 1.0);
            }
        }
    }

    #[test]
    fn test_plateau_reset() {
        let mut plateau = PlateauPotential::new(200.0);

        plateau.trigger();
        plateau.update(50.0);

        plateau.reset();

        assert!(!plateau.is_active());
        assert_eq!(plateau.amplitude(), 0.0);
        assert_eq!(plateau.time_remaining(), 0.0);
    }

    #[test]
    fn test_update_inactive_plateau() {
        let mut plateau = PlateauPotential::new(200.0);

        // Should do nothing when inactive
        plateau.update(10.0);

        assert!(!plateau.is_active());
        assert_eq!(plateau.amplitude(), 0.0);
    }
}
