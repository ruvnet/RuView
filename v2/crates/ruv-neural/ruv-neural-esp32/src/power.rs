//! Power management for battery-operated ESP32 sensor nodes.
//!
//! Provides duty-cycle estimation, sleep scheduling, and automatic duty-cycle
//! optimization to hit a target runtime.

use serde::{Deserialize, Serialize};

/// Operating power mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerMode {
    /// Full speed — all peripherals active.
    Active,
    /// Reduced clock, WiFi power save.
    LowPower,
    /// Minimal peripherals, deep sleep between samples.
    UltraLowPower,
    /// Full deep sleep — wakes only on timer or external interrupt.
    Sleep,
}

impl PowerMode {
    /// Estimated current draw in milliamps for this mode on an ESP32-S3.
    pub fn estimated_current_ma(&self) -> f64 {
        match self {
            PowerMode::Active => 240.0,
            PowerMode::LowPower => 80.0,
            PowerMode::UltraLowPower => 20.0,
            PowerMode::Sleep => 0.01,
        }
    }
}

/// Power management configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    /// Base operating mode.
    pub mode: PowerMode,
    /// Whether to enter light sleep between sample bursts.
    pub sleep_between_samples: bool,
    /// Fraction of time spent actively sampling (0.0-1.0).
    pub sample_duty_cycle: f64,
    /// Fraction of time WiFi is enabled (0.0-1.0).
    pub wifi_duty_cycle: f64,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            mode: PowerMode::Active,
            sleep_between_samples: false,
            sample_duty_cycle: 1.0,
            wifi_duty_cycle: 1.0,
        }
    }
}

/// Power manager that tracks battery state and optimizes duty cycles.
pub struct PowerManager {
    config: PowerConfig,
    battery_mv: u32,
    estimated_runtime_hours: f64,
}

impl PowerManager {
    /// Create a new power manager with the given configuration.
    pub fn new(config: PowerConfig) -> Self {
        Self {
            config,
            battery_mv: 4200, // Fully charged LiPo
            estimated_runtime_hours: 0.0,
        }
    }

    /// Estimate runtime in hours given a battery capacity in mAh.
    ///
    /// The effective current draw is a weighted average of active and sleep
    /// currents based on the configured duty cycles.
    pub fn estimate_runtime(&self, battery_capacity_mah: u32) -> f64 {
        let active_current = self.config.mode.estimated_current_ma();
        let sleep_current = PowerMode::Sleep.estimated_current_ma();

        let sample_active = self.config.sample_duty_cycle.clamp(0.0, 1.0);
        let wifi_active = self.config.wifi_duty_cycle.clamp(0.0, 1.0);

        // WiFi adds roughly 80 mA when active
        let wifi_overhead = 80.0 * wifi_active;

        let effective_current =
            active_current * sample_active + sleep_current * (1.0 - sample_active) + wifi_overhead;

        if effective_current <= 0.0 {
            return f64::INFINITY;
        }

        battery_capacity_mah as f64 / effective_current
    }

    /// Returns `true` if the node should sleep at the given time based on
    /// the configured duty cycle.
    ///
    /// Uses a simple periodic pattern: active for `duty * period`, then sleep
    /// for the remainder. The period is fixed at 1 second (1_000_000 us).
    pub fn should_sleep(&self, current_time_us: u64) -> bool {
        if !self.config.sleep_between_samples {
            return false;
        }
        let period_us: u64 = 1_000_000;
        let active_us = (self.config.sample_duty_cycle * period_us as f64) as u64;
        let position = current_time_us % period_us;
        position >= active_us
    }

    /// Adjust the sample and WiFi duty cycles to reach the target runtime.
    pub fn optimize_duty_cycle(&mut self, target_runtime_hours: f64) {
        // Binary search for the duty cycle that achieves the target runtime
        // with a 2000 mAh reference battery.
        let battery_mah = 2000u32;
        let mut low = 0.01_f64;
        let mut high = 1.0_f64;

        for _ in 0..50 {
            let mid = (low + high) / 2.0;
            self.config.sample_duty_cycle = mid;
            self.config.wifi_duty_cycle = mid;
            let runtime = self.estimate_runtime(battery_mah);
            if runtime < target_runtime_hours {
                high = mid;
            } else {
                low = mid;
            }
        }

        self.config.sample_duty_cycle = low;
        self.config.wifi_duty_cycle = low;
        self.estimated_runtime_hours = self.estimate_runtime(battery_mah);
    }

    /// Update the battery voltage reading.
    pub fn set_battery_mv(&mut self, mv: u32) {
        self.battery_mv = mv;
    }

    /// Current battery voltage in millivolts.
    pub fn battery_mv(&self) -> u32 {
        self.battery_mv
    }

    /// Estimated remaining runtime in hours (after calling
    /// `optimize_duty_cycle`).
    pub fn estimated_runtime_hours(&self) -> f64 {
        self.estimated_runtime_hours
    }

    /// Returns a reference to the current power configuration.
    pub fn config(&self) -> &PowerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_runtime_active() {
        let config = PowerConfig {
            mode: PowerMode::Active,
            sleep_between_samples: false,
            sample_duty_cycle: 1.0,
            wifi_duty_cycle: 1.0,
        };
        let pm = PowerManager::new(config);
        let hours = pm.estimate_runtime(2000);
        // 2000 mAh / (240 + 80) = 6.25 hours
        assert!((hours - 6.25).abs() < 0.1, "got {hours}");
    }

    #[test]
    fn test_estimate_runtime_low_duty() {
        let config = PowerConfig {
            mode: PowerMode::Active,
            sleep_between_samples: true,
            sample_duty_cycle: 0.1,
            wifi_duty_cycle: 0.1,
        };
        let pm = PowerManager::new(config);
        let hours = pm.estimate_runtime(2000);
        // Much longer than 6.25 hours
        assert!(hours > 20.0, "expected >20h, got {hours}");
    }

    #[test]
    fn test_should_sleep() {
        let config = PowerConfig {
            mode: PowerMode::Active,
            sleep_between_samples: true,
            sample_duty_cycle: 0.5,
            wifi_duty_cycle: 1.0,
        };
        let pm = PowerManager::new(config);
        // Active window: 0..500_000 us, sleep: 500_000..1_000_000 us
        assert!(!pm.should_sleep(0));
        assert!(!pm.should_sleep(499_999));
        assert!(pm.should_sleep(500_000));
        assert!(pm.should_sleep(999_999));
    }

    #[test]
    fn test_should_sleep_disabled() {
        let config = PowerConfig {
            mode: PowerMode::Active,
            sleep_between_samples: false,
            sample_duty_cycle: 0.1,
            wifi_duty_cycle: 0.1,
        };
        let pm = PowerManager::new(config);
        assert!(!pm.should_sleep(999_999));
    }

    #[test]
    fn test_optimize_duty_cycle() {
        let config = PowerConfig {
            mode: PowerMode::Active,
            sleep_between_samples: true,
            sample_duty_cycle: 1.0,
            wifi_duty_cycle: 1.0,
        };
        let mut pm = PowerManager::new(config);
        pm.optimize_duty_cycle(48.0); // Target 48 hours

        // Duty cycles should have been reduced
        assert!(pm.config().sample_duty_cycle < 1.0);
        assert!(pm.config().sample_duty_cycle > 0.0);
    }

    #[test]
    fn test_power_mode_current() {
        assert!(PowerMode::Active.estimated_current_ma() > PowerMode::LowPower.estimated_current_ma());
        assert!(PowerMode::LowPower.estimated_current_ma() > PowerMode::UltraLowPower.estimated_current_ma());
        assert!(PowerMode::UltraLowPower.estimated_current_ma() > PowerMode::Sleep.estimated_current_ma());
    }
}
