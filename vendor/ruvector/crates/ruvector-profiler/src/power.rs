#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PowerSample {
    pub watts: f64,
    pub timestamp_us: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnergyResult {
    pub total_joules: f64,
    pub mean_watts: f64,
    pub peak_watts: f64,
    pub duration_s: f64,
    pub samples: usize,
}

/// Trait for reading instantaneous power (NVML, RAPL, etc.).
pub trait PowerSource {
    fn read_watts(&self) -> f64;
}

/// Fixed-wattage mock for deterministic tests.
pub struct MockPowerSource {
    pub watts: f64,
}
impl PowerSource for MockPowerSource {
    fn read_watts(&self) -> f64 {
        self.watts
    }
}

/// Trapezoidal integration of power samples (must be sorted by timestamp).
pub fn estimate_energy(samples: &[PowerSample]) -> EnergyResult {
    let n = samples.len();
    if n < 2 {
        return EnergyResult {
            total_joules: 0.0,
            samples: n,
            duration_s: 0.0,
            mean_watts: samples.first().map_or(0.0, |s| s.watts),
            peak_watts: samples.first().map_or(0.0, |s| s.watts),
        };
    }
    let (mut joules, mut peak, mut sum) = (0.0f64, f64::NEG_INFINITY, 0.0f64);
    for i in 0..n {
        let w = samples[i].watts;
        sum += w;
        if w > peak {
            peak = w;
        }
        if i > 0 {
            let dt = samples[i]
                .timestamp_us
                .saturating_sub(samples[i - 1].timestamp_us) as f64
                / 1e6;
            joules += (samples[i - 1].watts + w) / 2.0 * dt;
        }
    }
    let dur = samples
        .last()
        .unwrap()
        .timestamp_us
        .saturating_sub(samples[0].timestamp_us) as f64
        / 1e6;
    EnergyResult {
        total_joules: joules,
        mean_watts: sum / n as f64,
        peak_watts: peak,
        duration_s: dur,
        samples: n,
    }
}

pub struct PowerTracker {
    pub samples: Vec<PowerSample>,
    pub label: String,
}

impl PowerTracker {
    pub fn new(label: &str) -> Self {
        Self {
            samples: Vec::new(),
            label: label.to_string(),
        }
    }

    pub fn sample(&mut self, source: &dyn PowerSource) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        self.samples.push(PowerSample {
            watts: source.read_watts(),
            timestamp_us: ts,
        });
    }

    pub fn energy(&self) -> EnergyResult {
        estimate_energy(&self.samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ps(w: f64, t: u64) -> PowerSample {
        PowerSample {
            watts: w,
            timestamp_us: t,
        }
    }

    #[test]
    fn energy_empty() {
        let r = estimate_energy(&[]);
        assert_eq!(r.samples, 0);
    }

    #[test]
    fn energy_single() {
        let r = estimate_energy(&[ps(42.0, 0)]);
        assert_eq!((r.total_joules, r.mean_watts), (0.0, 42.0));
    }

    #[test]
    fn energy_constant_100w_1s() {
        let r = estimate_energy(&[ps(100.0, 0), ps(100.0, 1_000_000)]);
        assert!((r.total_joules - 100.0).abs() < 1e-9);
    }

    #[test]
    fn energy_ramp() {
        let r = estimate_energy(&[ps(0.0, 0), ps(200.0, 1_000_000)]);
        assert!((r.total_joules - 100.0).abs() < 1e-9);
    }

    #[test]
    fn mock_source() {
        assert_eq!(MockPowerSource { watts: 75.0 }.read_watts(), 75.0);
    }

    #[test]
    fn tracker_collects() {
        let src = MockPowerSource { watts: 50.0 };
        let mut t = PowerTracker::new("gpu");
        t.sample(&src);
        t.sample(&src);
        assert_eq!(t.samples.len(), 2);
    }
}
