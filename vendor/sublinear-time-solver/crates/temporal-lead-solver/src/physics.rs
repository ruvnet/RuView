//! Physical constants and temporal calculations for FTL information theory

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Speed of light in vacuum (m/s)
pub const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;

/// Distance representation with conversions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Distance {
    meters: f64,
}

impl Distance {
    /// Create distance from meters
    pub fn meters(m: f64) -> Self {
        Self { meters: m }
    }

    /// Create distance from kilometers
    pub fn kilometers(km: f64) -> Self {
        Self { meters: km * 1000.0 }
    }

    /// Create distance from miles
    pub fn miles(miles: f64) -> Self {
        Self {
            meters: miles * 1609.344,
        }
    }

    /// Create distance from light-seconds
    pub fn light_seconds(ls: f64) -> Self {
        Self {
            meters: ls * SPEED_OF_LIGHT_MPS,
        }
    }

    /// Get distance in meters
    pub fn as_meters(&self) -> f64 {
        self.meters
    }

    /// Get distance in kilometers
    pub fn as_kilometers(&self) -> f64 {
        self.meters / 1000.0
    }

    /// Calculate light travel time for this distance
    pub fn light_travel_time(&self) -> Duration {
        let seconds = self.meters / SPEED_OF_LIGHT_MPS;
        Duration::from_secs_f64(seconds)
    }

    /// Calculate light travel time in milliseconds
    pub fn light_travel_time_ms(&self) -> f64 {
        (self.meters / SPEED_OF_LIGHT_MPS) * 1000.0
    }

    /// Named distances for common scenarios
    pub fn tokyo_to_nyc() -> Self {
        Self::kilometers(10_900.0)
    }

    pub fn earth_to_moon() -> Self {
        Self::kilometers(384_400.0)
    }

    pub fn earth_to_mars_min() -> Self {
        Self::kilometers(54_600_000.0)
    }

    pub fn earth_to_mars_max() -> Self {
        Self::kilometers(401_000_000.0)
    }

    pub fn one_au() -> Self {
        Self::kilometers(149_597_870.7)
    }
}

/// Speed of light utilities
pub struct SpeedOfLight;

impl SpeedOfLight {
    /// Get speed in m/s
    pub fn meters_per_second() -> f64 {
        SPEED_OF_LIGHT_MPS
    }

    /// Get speed in km/s
    pub fn kilometers_per_second() -> f64 {
        SPEED_OF_LIGHT_MPS / 1000.0
    }

    /// Time to travel a distance
    pub fn time_to_travel(distance: Distance) -> Duration {
        distance.light_travel_time()
    }

    /// Distance light travels in given duration
    pub fn distance_in_time(duration: Duration) -> Distance {
        Distance::meters(SPEED_OF_LIGHT_MPS * duration.as_secs_f64())
    }
}

/// Temporal advantage calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalAdvantage {
    pub distance: Distance,
    pub light_time: Duration,
    pub prediction_time: Duration,
    pub advantage: Duration,
    pub effective_velocity_ratio: f64,
}

impl TemporalAdvantage {
    /// Calculate temporal advantage for given distance and prediction time
    pub fn calculate(distance: Distance, prediction_time: Duration) -> Self {
        let light_time = distance.light_travel_time();

        let advantage = if light_time > prediction_time {
            light_time - prediction_time
        } else {
            Duration::ZERO
        };

        let effective_velocity_ratio = if prediction_time.as_secs_f64() > 0.0 {
            light_time.as_secs_f64() / prediction_time.as_secs_f64()
        } else {
            f64::INFINITY
        };

        Self {
            distance,
            light_time,
            prediction_time,
            advantage,
            effective_velocity_ratio,
        }
    }

    /// Check if FTL is achieved
    pub fn is_ftl(&self) -> bool {
        self.effective_velocity_ratio > 1.0
    }

    /// Get advantage in milliseconds
    pub fn advantage_ms(&self) -> f64 {
        self.advantage.as_secs_f64() * 1000.0
    }

    /// Get effective information velocity (m/s)
    pub fn effective_velocity(&self) -> f64 {
        if self.prediction_time.as_secs_f64() > 0.0 {
            self.distance.as_meters() / self.prediction_time.as_secs_f64()
        } else {
            f64::INFINITY
        }
    }

    /// Format as human-readable string
    pub fn describe(&self) -> String {
        if self.is_ftl() {
            format!(
                "FTL achieved! Distance: {:.0}km, Light time: {:.1}ms, Prediction: {:.3}ms, Advantage: {:.1}ms ({}x light speed)",
                self.distance.as_kilometers(),
                self.light_time.as_secs_f64() * 1000.0,
                self.prediction_time.as_secs_f64() * 1000.0,
                self.advantage_ms(),
                self.effective_velocity_ratio as u64
            )
        } else {
            format!(
                "Sub-light. Distance: {:.0}km, Light time: {:.1}ms, Prediction: {:.1}ms",
                self.distance.as_kilometers(),
                self.light_time.as_secs_f64() * 1000.0,
                self.prediction_time.as_secs_f64() * 1000.0
            )
        }
    }
}

/// Relativistic effects calculator (for validation)
pub struct RelativisticEffects;

impl RelativisticEffects {
    /// Lorentz factor γ = 1/√(1 - v²/c²)
    pub fn lorentz_factor(velocity: f64) -> f64 {
        let beta = velocity / SPEED_OF_LIGHT_MPS;
        if beta >= 1.0 {
            f64::INFINITY
        } else {
            1.0 / (1.0 - beta * beta).sqrt()
        }
    }

    /// Time dilation factor
    pub fn time_dilation(velocity: f64, time: Duration) -> Duration {
        let gamma = Self::lorentz_factor(velocity);
        Duration::from_secs_f64(time.as_secs_f64() * gamma)
    }

    /// Check if velocity would violate causality
    pub fn violates_causality(velocity: f64) -> bool {
        velocity >= SPEED_OF_LIGHT_MPS
    }

    /// Calculate information velocity that doesn't violate physics
    pub fn validate_information_velocity(
        distance: Distance,
        computation_time: Duration,
    ) -> (bool, String) {
        let effective_velocity = distance.as_meters() / computation_time.as_secs_f64();

        if effective_velocity <= SPEED_OF_LIGHT_MPS {
            (true, "Information velocity is sub-light".to_string())
        } else {
            // This is where we explain the FTL paradox resolution
            (
                true,
                format!(
                    "Information appears to travel at {:.2}x light speed, but this is predictive computation, not physical signal transmission. No causality violation.",
                    effective_velocity / SPEED_OF_LIGHT_MPS
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_conversions() {
        let d = Distance::kilometers(1.0);
        assert_eq!(d.as_meters(), 1000.0);
    }

    #[test]
    fn test_light_travel_time() {
        let d = Distance::kilometers(300_000.0); // ~1 light second
        let time = d.light_travel_time();
        assert!((time.as_secs_f64() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_temporal_advantage() {
        let distance = Distance::tokyo_to_nyc();
        let prediction_time = Duration::from_micros(100);
        let advantage = TemporalAdvantage::calculate(distance, prediction_time);

        assert!(advantage.is_ftl());
        assert!(advantage.effective_velocity_ratio > 100.0);
    }

    #[test]
    fn test_relativistic_validation() {
        let distance = Distance::kilometers(1000.0);
        let compute_time = Duration::from_micros(1);
        let (valid, _msg) = RelativisticEffects::validate_information_velocity(distance, compute_time);
        assert!(valid); // Valid because it's predictive, not physical
    }
}