//! Quantum Decoherence Tracking for Temporal Consciousness
//!
//! This module implements realistic quantum decoherence modeling to validate
//! the feasibility of quantum consciousness operations in real environments.
//! Decoherence sets practical limits on quantum coherence time and thus
//! temporal consciousness precision.
//!
//! ## Decoherence Mechanisms
//!
//! 1. **Thermal Decoherence**: Interaction with thermal environment
//! 2. **Dephasing**: Loss of phase coherence due to noise
//! 3. **Relaxation**: Energy dissipation to environment
//! 4. **Measurement**: Observer-induced decoherence
//!
//! ## Decoherence Time Scales
//!
//! Typical decoherence times in various environments:
//! - Room temperature: 10^-12 to 10^-9 seconds
//! - Cryogenic (mK): 10^-6 to 10^-3 seconds
//! - Ultra-high vacuum: 10^-9 to 10^-6 seconds

use crate::temporal_nexus::quantum::{constants, QuantumError, QuantumResult};
use std::collections::HashMap;

/// Quantum decoherence tracker for consciousness operations
#[derive(Debug, Clone)]
pub struct DecoherenceTracker {
    /// Environment temperature (K)
    temperature: f64,
    /// Thermal decoherence rate (1/s)
    thermal_rate: f64,
    /// Dephasing rate due to noise (1/s)
    dephasing_rate: f64,
    /// System-environment coupling strength
    coupling_strength: f64,
    /// Environmental noise spectrum parameters
    noise_spectrum: NoiseSpectrum,
}

impl DecoherenceTracker {
    /// Create new decoherence tracker with room temperature defaults
    pub fn new() -> Self {
        Self::with_temperature(constants::ROOM_TEMPERATURE_K)
    }

    /// Create tracker with specific temperature
    pub fn with_temperature(temperature_k: f64) -> Self {
        let thermal_rate = Self::calculate_thermal_decoherence_rate(temperature_k);
        
        Self {
            temperature: temperature_k,
            thermal_rate,
            dephasing_rate: 1e9, // 1 GHz typical dephasing
            coupling_strength: 1e-3, // Weak coupling
            noise_spectrum: NoiseSpectrum::new(temperature_k),
        }
    }

    /// Create tracker for cryogenic environment
    pub fn cryogenic() -> Self {
        Self::with_temperature(0.01) // 10 mK
    }

    /// Calculate thermal decoherence rate using Fermi's golden rule
    fn calculate_thermal_decoherence_rate(temperature_k: f64) -> f64 {
        if temperature_k <= 0.0 {
            return 0.0;
        }
        
        let thermal_energy = constants::BOLTZMANN_K * temperature_k;
        let typical_coupling = 1e-6; // Weak coupling in eV
        let coupling_j = typical_coupling * constants::EV_TO_JOULES;
        
        // Decoherence rate ∝ coupling² × thermal energy / ℏ
        let rate = (coupling_j.powi(2) * thermal_energy) / constants::PLANCK_HBAR;
        rate.max(1e3) // Minimum 1 kHz even at very low temperature
    }

    /// Calculate coherence time from decoherence rates
    pub fn coherence_time(&self) -> f64 {
        let total_rate = self.thermal_rate + self.dephasing_rate;
        1.0 / total_rate
    }

    /// Calculate T1 (relaxation) time
    pub fn relaxation_time_t1(&self) -> f64 {
        // T1 typically longer than T2
        2.0 / self.thermal_rate
    }

    /// Calculate T2 (dephasing) time
    pub fn dephasing_time_t2(&self) -> f64 {
        1.0 / self.dephasing_rate
    }

    /// Validate if operation time is within coherence window
    pub fn validate_operation_time(&self, operation_time_s: f64) -> QuantumResult<DecoherenceResult> {
        let coherence_time_s = self.coherence_time();
        let t1_time_s = self.relaxation_time_t1();
        let t2_time_s = self.dephasing_time_t2();
        
        let is_valid = operation_time_s < coherence_time_s;
        let coherence_preserved = (-operation_time_s / coherence_time_s).exp();
        
        if !is_valid {
            return Err(QuantumError::DecoherenceExceeded {
                decoherence_time_s: coherence_time_s,
                operation_time_s,
            });
        }
        
        Ok(DecoherenceResult {
            is_valid,
            operation_time_s,
            coherence_time_s,
            t1_relaxation_s: t1_time_s,
            t2_dephasing_s: t2_time_s,
            coherence_preserved,
            temperature_k: self.temperature,
            thermal_rate_hz: self.thermal_rate,
            dephasing_rate_hz: self.dephasing_rate,
            environment_type: self.classify_environment(),
            noise_analysis: self.noise_spectrum.analyze_at_time(operation_time_s),
        })
    }

    /// Classify environment based on temperature
    pub fn classify_environment(&self) -> EnvironmentType {
        if self.temperature < 1.0 {
            EnvironmentType::UltraCryogenic
        } else if self.temperature < 4.2 {
            EnvironmentType::Cryogenic
        } else if self.temperature < 77.0 {
            EnvironmentType::LiquidNitrogen
        } else if self.temperature < 273.0 {
            EnvironmentType::Cold
        } else {
            EnvironmentType::RoomTemperature
        }
    }

    /// Predict coherence evolution over time
    pub fn predict_coherence_evolution(&self, max_time_s: f64, steps: usize) -> CoherenceEvolution {
        let dt = max_time_s / steps as f64;
        let mut times = Vec::new();
        let mut coherences = Vec::new();
        let coherence_time = self.coherence_time();
        
        for i in 0..=steps {
            let t = i as f64 * dt;
            let coherence = (-t / coherence_time).exp();
            times.push(t);
            coherences.push(coherence);
        }
        
        CoherenceEvolution {
            times,
            coherences,
            coherence_time_s: coherence_time,
            environment: self.classify_environment(),
        }
    }

    /// Analyze decoherence across different time scales
    pub fn analyze_time_scales(&self) -> DecoherenceAnalysis {
        let scales = vec![
            ("attosecond", 1e-18),
            ("femtosecond", 1e-15),
            ("picosecond", 1e-12),
            ("nanosecond", 1e-9),
            ("microsecond", 1e-6),
            ("millisecond", 1e-3),
        ];
        
        let coherence_time = self.coherence_time();
        let mut assessments = Vec::new();
        
        for (name, time_s) in scales {
            let coherence_preserved = (-time_s / coherence_time).exp();
            let is_feasible = coherence_preserved > 0.5; // 50% coherence threshold
            
            assessments.push(TimeScaleAssessment {
                scale_name: name.to_string(),
                time_s,
                coherence_preserved,
                is_feasible,
                coherence_quality: if coherence_preserved > 0.9 {
                    CoherenceQuality::Excellent
                } else if coherence_preserved > 0.7 {
                    CoherenceQuality::Good
                } else if coherence_preserved > 0.5 {
                    CoherenceQuality::Marginal
                } else {
                    CoherenceQuality::Poor
                },
            });
        }
        
        DecoherenceAnalysis {
            environment: self.classify_environment(),
            temperature_k: self.temperature,
            coherence_time_s: coherence_time,
            t1_time_s: self.relaxation_time_t1(),
            t2_time_s: self.dephasing_time_t2(),
            assessments,
            recommended_scale: self.recommend_time_scale(),
        }
    }

    /// Recommend optimal time scale for consciousness operations
    fn recommend_time_scale(&self) -> String {
        let coherence_time = self.coherence_time();
        
        if coherence_time > 1e-6 {
            "microsecond".to_string()
        } else if coherence_time > 1e-9 {
            "nanosecond".to_string()
        } else if coherence_time > 1e-12 {
            "picosecond".to_string()
        } else {
            "femtosecond".to_string()
        }
    }

    /// Set custom decoherence parameters
    pub fn set_decoherence_rates(&mut self, thermal_rate_hz: f64, dephasing_rate_hz: f64) {
        self.thermal_rate = thermal_rate_hz;
        self.dephasing_rate = dephasing_rate_hz;
    }
}

impl Default for DecoherenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Environmental noise spectrum characterization
#[derive(Debug, Clone)]
pub struct NoiseSpectrum {
    /// Temperature for thermal noise
    temperature: f64,
    /// Low-frequency cutoff (Hz)
    low_freq_cutoff: f64,
    /// High-frequency cutoff (Hz)
    high_freq_cutoff: f64,
    /// Noise power spectral density
    spectral_density: HashMap<String, f64>,
}

impl NoiseSpectrum {
    fn new(temperature_k: f64) -> Self {
        let mut spectral_density = HashMap::new();
        
        // Johnson noise (thermal)
        let johnson_noise = 4.0 * constants::BOLTZMANN_K * temperature_k;
        spectral_density.insert("thermal".to_string(), johnson_noise);
        
        // 1/f noise
        spectral_density.insert("flicker".to_string(), 1e-15);
        
        // Shot noise
        spectral_density.insert("shot".to_string(), 1e-18);
        
        Self {
            temperature: temperature_k,
            low_freq_cutoff: 1e3,  // 1 kHz
            high_freq_cutoff: 1e12, // 1 THz
            spectral_density,
        }
    }
    
    fn analyze_at_time(&self, time_s: f64) -> NoiseAnalysis {
        let frequency = 1.0 / time_s;
        
        let thermal_noise = self.spectral_density["thermal"];
        let flicker_noise = self.spectral_density["flicker"] / frequency.max(1.0);
        let shot_noise = self.spectral_density["shot"];
        
        let total_noise = thermal_noise + flicker_noise + shot_noise;
        
        NoiseAnalysis {
            frequency_hz: frequency,
            thermal_noise_density: thermal_noise,
            flicker_noise_density: flicker_noise,
            shot_noise_density: shot_noise,
            total_noise_density: total_noise,
            dominant_source: if thermal_noise > flicker_noise && thermal_noise > shot_noise {
                "thermal".to_string()
            } else if flicker_noise > shot_noise {
                "flicker".to_string()
            } else {
                "shot".to_string()
            },
        }
    }
}

/// Result of decoherence validation
#[derive(Debug, Clone)]
pub struct DecoherenceResult {
    pub is_valid: bool,
    pub operation_time_s: f64,
    pub coherence_time_s: f64,
    pub t1_relaxation_s: f64,
    pub t2_dephasing_s: f64,
    pub coherence_preserved: f64,
    pub temperature_k: f64,
    pub thermal_rate_hz: f64,
    pub dephasing_rate_hz: f64,
    pub environment_type: EnvironmentType,
    pub noise_analysis: NoiseAnalysis,
}

impl DecoherenceResult {
    pub fn summary(&self) -> String {
        format!(
            "Decoherence Check: {} (coherence: {:.1}%, T₂: {:.1e}s, env: {:?})",
            if self.is_valid { "PASS" } else { "FAIL" },
            self.coherence_preserved * 100.0,
            self.coherence_time_s,
            self.environment_type
        )
    }
}

/// Environment classification for decoherence analysis
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentType {
    UltraCryogenic,   // < 1K
    Cryogenic,        // 1-4.2K
    LiquidNitrogen,   // 4.2-77K
    Cold,             // 77-273K
    RoomTemperature,  // > 273K
}

/// Coherence quality classification
#[derive(Debug, Clone, PartialEq)]
pub enum CoherenceQuality {
    Excellent, // > 90%
    Good,      // 70-90%
    Marginal,  // 50-70%
    Poor,      // < 50%
}

/// Noise analysis for specific operation
#[derive(Debug, Clone)]
pub struct NoiseAnalysis {
    pub frequency_hz: f64,
    pub thermal_noise_density: f64,
    pub flicker_noise_density: f64,
    pub shot_noise_density: f64,
    pub total_noise_density: f64,
    pub dominant_source: String,
}

/// Coherence evolution over time
#[derive(Debug, Clone)]
pub struct CoherenceEvolution {
    pub times: Vec<f64>,
    pub coherences: Vec<f64>,
    pub coherence_time_s: f64,
    pub environment: EnvironmentType,
}

/// Decoherence analysis across time scales
#[derive(Debug, Clone)]
pub struct DecoherenceAnalysis {
    pub environment: EnvironmentType,
    pub temperature_k: f64,
    pub coherence_time_s: f64,
    pub t1_time_s: f64,
    pub t2_time_s: f64,
    pub assessments: Vec<TimeScaleAssessment>,
    pub recommended_scale: String,
}

/// Assessment of coherence at specific time scale
#[derive(Debug, Clone)]
pub struct TimeScaleAssessment {
    pub scale_name: String,
    pub time_s: f64,
    pub coherence_preserved: f64,
    pub is_feasible: bool,
    pub coherence_quality: CoherenceQuality,
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_decoherence_tracker_creation() {
        let tracker = DecoherenceTracker::new();
        assert_eq!(tracker.temperature, constants::ROOM_TEMPERATURE_K);
        assert!(tracker.coherence_time() > 0.0);
    }

    #[test]
    fn test_cryogenic_environment() {
        let cryo_tracker = DecoherenceTracker::cryogenic();
        let room_tracker = DecoherenceTracker::new();
        
        // Cryogenic should have much longer coherence time
        assert!(cryo_tracker.coherence_time() > room_tracker.coherence_time());
        assert_eq!(cryo_tracker.classify_environment(), EnvironmentType::UltraCryogenic);
    }

    #[test]
    fn test_operation_time_validation() {
        let tracker = DecoherenceTracker::new();
        
        // Very short operation should be valid
        let result = tracker.validate_operation_time(1e-12).unwrap();
        assert!(result.is_valid);
        assert!(result.coherence_preserved > 0.9);
    }

    #[test]
    fn test_coherence_evolution() {
        let tracker = DecoherenceTracker::new();
        let evolution = tracker.predict_coherence_evolution(1e-9, 100);
        
        assert_eq!(evolution.times.len(), 101);
        assert_eq!(evolution.coherences.len(), 101);
        
        // Coherence should decay over time
        assert!(evolution.coherences[0] > evolution.coherences[50]);
        assert!(evolution.coherences[50] > evolution.coherences[100]);
    }

    #[test]
    fn test_time_scale_analysis() {
        let tracker = DecoherenceTracker::new();
        let analysis = tracker.analyze_time_scales();
        
        assert_eq!(analysis.assessments.len(), 6);
        
        // Shorter time scales should generally have better coherence
        let femtosecond = analysis.assessments.iter()
            .find(|a| a.scale_name == "femtosecond").unwrap();
        let microsecond = analysis.assessments.iter()
            .find(|a| a.scale_name == "microsecond").unwrap();
        
        assert!(femtosecond.coherence_preserved > microsecond.coherence_preserved);
    }

    #[test]
    fn test_thermal_decoherence_rate() {
        // Higher temperature should lead to faster decoherence
        let rate_300k = DecoherenceTracker::calculate_thermal_decoherence_rate(300.0);
        let rate_100k = DecoherenceTracker::calculate_thermal_decoherence_rate(100.0);
        let rate_10k = DecoherenceTracker::calculate_thermal_decoherence_rate(10.0);
        
        assert!(rate_300k > rate_100k);
        assert!(rate_100k > rate_10k);
    }

    #[test]
    fn test_noise_spectrum_analysis() {
        let tracker = DecoherenceTracker::new();
        let noise = tracker.noise_spectrum.analyze_at_time(1e-9);
        
        assert!(noise.total_noise_density > 0.0);
        assert!(!noise.dominant_source.is_empty());
        assert_eq!(noise.frequency_hz, 1e9); // 1 GHz for 1 ns
    }

    #[test]
    fn test_environment_classification() {
        assert_eq!(DecoherenceTracker::with_temperature(0.1).classify_environment(), 
                   EnvironmentType::UltraCryogenic);
        assert_eq!(DecoherenceTracker::with_temperature(4.0).classify_environment(), 
                   EnvironmentType::Cryogenic);
        assert_eq!(DecoherenceTracker::with_temperature(300.0).classify_environment(), 
                   EnvironmentType::RoomTemperature);
    }
}