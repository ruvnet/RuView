//! NMDA-like coincidence detection for temporal pattern matching
//!
//! Detects when multiple synapses fire simultaneously within a coincidence
//! window (typically 10-50ms), triggering plateau potentials for BTSP.

use super::plateau::PlateauPotential;
use std::collections::VecDeque;

/// Synapse activation event
#[derive(Debug, Clone, Copy)]
struct SpikeEvent {
    synapse_id: usize,
    timestamp: u64,
}

/// Dendrite with NMDA-like coincidence detection
#[derive(Debug, Clone)]
pub struct Dendrite {
    /// Current membrane potential
    membrane: f32,

    /// Calcium concentration
    calcium: f32,

    /// Number of synapses required for NMDA activation
    nmda_threshold: u8,

    /// Plateau potential generator
    plateau: PlateauPotential,

    /// Recent spike events (within coincidence window)
    active_synapses: VecDeque<SpikeEvent>,

    /// Coincidence detection window (ms)
    coincidence_window_ms: f32,

    /// Maximum synapses to track
    max_synapses: usize,
}

impl Dendrite {
    /// Create a new dendrite with NMDA coincidence detection
    ///
    /// # Arguments
    /// * `nmda_threshold` - Number of synapses needed for NMDA activation (typically 5-35)
    /// * `coincidence_window_ms` - Temporal window for coincidence detection (typically 10-50ms)
    pub fn new(nmda_threshold: u8, coincidence_window_ms: f32) -> Self {
        Self {
            membrane: 0.0,
            calcium: 0.0,
            nmda_threshold,
            plateau: PlateauPotential::new(200.0), // 200ms default plateau duration
            active_synapses: VecDeque::new(),
            coincidence_window_ms,
            max_synapses: 1000,
        }
    }

    /// Create dendrite with custom plateau duration
    pub fn with_plateau_duration(
        nmda_threshold: u8,
        coincidence_window_ms: f32,
        plateau_duration_ms: f32,
    ) -> Self {
        Self {
            membrane: 0.0,
            calcium: 0.0,
            nmda_threshold,
            plateau: PlateauPotential::new(plateau_duration_ms),
            active_synapses: VecDeque::new(),
            coincidence_window_ms,
            max_synapses: 1000,
        }
    }

    /// Receive a synaptic spike
    ///
    /// Registers the spike and checks for coincidence detection
    pub fn receive_spike(&mut self, synapse_id: usize, timestamp: u64) {
        // Add spike event
        self.active_synapses.push_back(SpikeEvent {
            synapse_id,
            timestamp,
        });

        // Limit queue size
        if self.active_synapses.len() > self.max_synapses {
            self.active_synapses.pop_front();
        }

        // Small membrane depolarization per spike
        self.membrane += 0.01;
        self.membrane = self.membrane.min(1.0);
    }

    /// Update dendrite state and check for plateau trigger
    ///
    /// Returns true if plateau potential was triggered this update
    pub fn update(&mut self, current_time: u64, dt: f32) -> bool {
        // Remove old spikes outside coincidence window
        let window_start = current_time.saturating_sub(self.coincidence_window_ms as u64);
        while let Some(spike) = self.active_synapses.front() {
            if spike.timestamp < window_start {
                self.active_synapses.pop_front();
            } else {
                break;
            }
        }

        // Count unique synapses in window
        let mut unique_synapses = std::collections::HashSet::new();
        for spike in &self.active_synapses {
            unique_synapses.insert(spike.synapse_id);
        }

        // Check NMDA threshold
        let mut plateau_triggered = false;
        if unique_synapses.len() >= self.nmda_threshold as usize {
            // Trigger plateau potential
            if !self.plateau.is_active() {
                self.plateau.trigger();
                plateau_triggered = true;
            }
        }

        // Update plateau potential
        self.plateau.update(dt);

        // Membrane decay
        self.membrane *= 0.95_f32.powf(dt / 10.0);

        // Calcium dynamics based on plateau
        if self.plateau.is_active() {
            self.calcium += 0.01 * dt;
            self.calcium = self.calcium.min(1.0);
        } else {
            self.calcium *= 0.99_f32.powf(dt / 10.0);
        }

        plateau_triggered
    }

    /// Check if plateau potential is currently active
    pub fn has_plateau(&self) -> bool {
        self.plateau.is_active()
    }

    /// Get current membrane potential
    pub fn membrane(&self) -> f32 {
        self.membrane
    }

    /// Get current calcium concentration
    pub fn calcium(&self) -> f32 {
        self.calcium
    }

    /// Get number of active synapses in coincidence window
    pub fn active_synapse_count(&self) -> usize {
        let mut unique = std::collections::HashSet::new();
        for spike in &self.active_synapses {
            unique.insert(spike.synapse_id);
        }
        unique.len()
    }

    /// Get plateau amplitude (0.0-1.0)
    pub fn plateau_amplitude(&self) -> f32 {
        self.plateau.amplitude()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dendrite_creation() {
        let dendrite = Dendrite::new(5, 20.0);
        assert_eq!(dendrite.nmda_threshold, 5);
        assert_eq!(dendrite.coincidence_window_ms, 20.0);
    }

    #[test]
    fn test_single_spike_no_plateau() {
        let mut dendrite = Dendrite::new(5, 20.0);

        dendrite.receive_spike(0, 100);
        let triggered = dendrite.update(100, 1.0);

        assert!(!triggered);
        assert!(!dendrite.has_plateau());
    }

    #[test]
    fn test_coincidence_triggers_plateau() {
        let mut dendrite = Dendrite::new(5, 20.0);

        // Fire 6 different synapses at same time
        for i in 0..6 {
            dendrite.receive_spike(i, 100);
        }

        let triggered = dendrite.update(100, 1.0);

        assert!(triggered);
        assert!(dendrite.has_plateau());
    }

    #[test]
    fn test_coincidence_window() {
        let mut dendrite = Dendrite::new(5, 20.0);

        // Fire synapses spread across time
        dendrite.receive_spike(0, 100);
        dendrite.receive_spike(1, 110);
        dendrite.receive_spike(2, 120);
        dendrite.receive_spike(3, 130); // Still within 20ms window
        dendrite.receive_spike(4, 135);

        // At time 120, all should be in window
        let triggered = dendrite.update(120, 1.0);
        assert!(triggered);
    }

    #[test]
    fn test_spikes_outside_window_ignored() {
        let mut dendrite = Dendrite::new(5, 20.0);

        // Fire synapses too far apart
        dendrite.receive_spike(0, 100);
        dendrite.receive_spike(1, 110);
        dendrite.receive_spike(2, 150); // Outside window
        dendrite.receive_spike(3, 160);
        dendrite.receive_spike(4, 170);

        // At time 170, only 3 recent spikes in window
        let triggered = dendrite.update(170, 1.0);
        assert!(!triggered);
    }

    #[test]
    fn test_active_synapse_count() {
        let mut dendrite = Dendrite::new(5, 20.0);

        dendrite.receive_spike(0, 100);
        dendrite.receive_spike(0, 101); // Same synapse twice
        dendrite.receive_spike(1, 102);
        dendrite.receive_spike(2, 103);

        dendrite.update(103, 1.0);

        // Should count 3 unique synapses, not 4 spikes
        assert_eq!(dendrite.active_synapse_count(), 3);
    }

    #[test]
    fn test_plateau_duration() {
        let mut dendrite = Dendrite::with_plateau_duration(5, 20.0, 100.0);

        // Trigger plateau
        for i in 0..6 {
            dendrite.receive_spike(i, 100);
        }
        dendrite.update(100, 1.0);
        assert!(dendrite.has_plateau());

        // Step forward 50ms - should still be active
        dendrite.update(150, 50.0);
        assert!(dendrite.has_plateau());

        // Step forward another 60ms - should be inactive
        dendrite.update(210, 60.0);
        assert!(!dendrite.has_plateau());
    }

    #[test]
    fn test_calcium_during_plateau() {
        let mut dendrite = Dendrite::new(5, 20.0);

        // Trigger plateau
        for i in 0..6 {
            dendrite.receive_spike(i, 100);
        }
        dendrite.update(100, 1.0);

        let initial_calcium = dendrite.calcium();

        // Calcium should increase during plateau
        dendrite.update(110, 10.0);
        assert!(dendrite.calcium() > initial_calcium);
    }
}
