//! End-to-end NV-diamond simulator pipeline — Pass 5b of the implementation plan.
//!
//! `Pipeline` wires every module: scene → source synthesis → propagation →
//! NV ensemble → digitiser → MagFrame stream. One `Pipeline::run(n)` call
//! produces an n-sample deterministic frame stream from a scene + config.
//!
//! Determinism: same `(scene, config, seed)` ⇒ byte-identical frame stream
//! across runs and machines. Underwrites the proof-bundle commitment in
//! plan §5 — Pass 6 wraps this in a SHA-256 witness.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::digitiser::{adc_quantise, DigitiserConfig};
use crate::frame::{flag, MagFrame};
use crate::scene::Scene;
use crate::sensor::{NvSensor, NvSensorConfig};
use crate::source::scene_field_at;

/// Pipeline configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Sensor / digitiser sampling parameters.
    pub digitiser: DigitiserConfig,
    /// NV-ensemble physics parameters.
    pub sensor: NvSensorConfig,
    /// Per-sample integration time (s). Default 1/f_s.
    pub dt_s: Option<f64>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            digitiser: DigitiserConfig::default(),
            sensor: NvSensorConfig::default(),
            dt_s: None,
        }
    }
}

/// Forward-only NV-diamond pipeline.
#[derive(Debug, Clone)]
pub struct Pipeline {
    scene: Scene,
    config: PipelineConfig,
    seed: u64,
}

impl Pipeline {
    /// Construct a pipeline. `seed` makes shot-noise reproducible — same
    /// `(scene, config, seed)` produces byte-identical output.
    pub fn new(scene: Scene, config: PipelineConfig, seed: u64) -> Self {
        Self { scene, config, seed }
    }

    /// Run `n_samples` of the pipeline. Returns one [`MagFrame`] per
    /// (sensor × sample) — i.e. `n_samples · scene.sensors.len()` frames
    /// in scene-major / sample-minor order.
    pub fn run(&self, n_samples: usize) -> Vec<MagFrame> {
        let dt = self.config.dt_s.unwrap_or(1.0 / self.config.digitiser.f_s_hz);
        let dt_us = (dt * 1.0e6) as u64;
        let nv = NvSensor::new(self.config.sensor);

        let mut out: Vec<MagFrame> =
            Vec::with_capacity(n_samples.saturating_mul(self.scene.sensors.len()));

        for (sensor_idx, &sensor_pos) in self.scene.sensors.iter().enumerate() {
            for sample in 0..n_samples {
                let (b_synth, near_field) = scene_field_at(&self.scene, sensor_pos);
                // Per-sample seed mixes the global seed with sample/sensor
                // indices so different (sensor, sample) pairs draw from
                // independent shot-noise streams while the whole run stays
                // reproducible from the global seed.
                let per_sample_seed = self
                    .seed
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .wrapping_add((sensor_idx as u64) << 32)
                    .wrapping_add(sample as u64);
                let reading = nv.sample(b_synth, dt, per_sample_seed);

                // ADC quantise each axis independently, raising the
                // saturation flag if any axis clips.
                let mut adc_sat = false;
                let mut b_pt = [0.0_f32; 3];
                for k in 0..3 {
                    let (code, sat) = adc_quantise(reading.b_recovered[k]);
                    adc_sat |= sat;
                    let recovered_t = code as f64 * crate::digitiser::ADC_LSB_T;
                    b_pt[k] = (recovered_t * 1.0e12) as f32; // T → pT
                }
                let sigma_pt = [
                    (reading.sigma_per_axis[0] * 1.0e12) as f32,
                    (reading.sigma_per_axis[1] * 1.0e12) as f32,
                    (reading.sigma_per_axis[2] * 1.0e12) as f32,
                ];

                let mut frame = MagFrame::empty(sensor_idx as u16);
                frame.t_us = (sample as u64) * dt_us;
                frame.b_pt = b_pt;
                frame.sigma_pt = sigma_pt;
                frame.noise_floor_pt_sqrt_hz =
                    (reading.noise_floor_t_sqrt_hz * 1.0e12) as f32;
                frame.temperature_k = 295.0;
                if near_field {
                    frame.set_flag(flag::SATURATION_NEAR_FIELD);
                }
                if adc_sat {
                    frame.set_flag(flag::ADC_SATURATED);
                }
                if self.config.sensor.shot_noise_disabled {
                    frame.set_flag(flag::SHOT_NOISE_DISABLED);
                }
                out.push(frame);
            }
        }
        out
    }

    /// Run the pipeline and return a SHA-256 of the concatenated raw frame
    /// bytes. The witness is content-addressable: same `(scene, config, seed)`
    /// produces byte-identical witnesses across runs and machines. Backbone
    /// of Pass 6's proof bundle.
    pub fn run_with_witness(&self, n_samples: usize) -> (Vec<MagFrame>, [u8; 32]) {
        let frames = self.run(n_samples);
        let mut hasher = Sha256::new();
        for f in &frames {
            hasher.update(f.to_bytes());
        }
        let digest: [u8; 32] = hasher.finalize().into();
        (frames, digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::DipoleSource;

    fn fixture_scene() -> Scene {
        let mut s = Scene::new();
        // Strong-ish dipole 50 cm above the sensor.
        s.add_dipole(DipoleSource::new([0.0, 0.0, 0.5], [0.0, 0.0, 1.0e-3]));
        s.add_sensor([0.0, 0.0, 0.0]);
        s
    }

    #[test]
    fn determinism_same_seed_byte_identical_witness() {
        // Plan §5 acceptance: (scene, seed) → byte-identical proof bundle.
        let scene = fixture_scene();
        let cfg = PipelineConfig::default();
        let p1 = Pipeline::new(scene.clone(), cfg, 42);
        let p2 = Pipeline::new(scene, cfg, 42);
        let (_, w1) = p1.run_with_witness(64);
        let (_, w2) = p2.run_with_witness(64);
        assert_eq!(w1, w2, "same seed must produce identical witnesses");
    }

    #[test]
    fn different_seeds_produce_different_witnesses() {
        // Sanity: the seed actually does something. Two different seeds
        // must produce different witnesses (overwhelmingly likely).
        let scene = fixture_scene();
        let cfg = PipelineConfig::default();
        let (_, w1) = Pipeline::new(scene.clone(), cfg, 1).run_with_witness(64);
        let (_, w2) = Pipeline::new(scene, cfg, 2).run_with_witness(64);
        assert_ne!(w1, w2);
    }

    #[test]
    fn frame_count_matches_sensor_x_sample_product() {
        let scene = fixture_scene();
        let cfg = PipelineConfig::default();
        let p = Pipeline::new(scene, cfg, 7);
        let frames = p.run(32);
        assert_eq!(frames.len(), 32);
        for (i, f) in frames.iter().enumerate() {
            assert_eq!(f.sensor_id, 0);
            assert_eq!(f.t_us, (i as u64) * (1.0e6 / 10_000.0) as u64);
        }
    }

    #[test]
    fn shot_noise_disabled_propagates_flag_and_yields_clean_signal() {
        // With shot noise off, every frame must carry SHOT_NOISE_DISABLED
        // and the recovered field must reproduce the analytical value
        // within ADC ½-LSB. Plan §5 noise-floor commitment.
        let scene = fixture_scene();
        let cfg = PipelineConfig {
            sensor: NvSensorConfig {
                shot_noise_disabled: true,
                ..NvSensorConfig::default()
            },
            ..PipelineConfig::default()
        };
        let p = Pipeline::new(scene.clone(), cfg, 0);
        let frames = p.run(8);
        let (b_analytic, _) = scene_field_at(&scene, scene.sensors[0]);
        for f in &frames {
            assert!(f.has_flag(flag::SHOT_NOISE_DISABLED));
            for k in 0..3 {
                let recovered_t = f.b_pt[k] as f64 * 1.0e-12;
                let lsb_t = crate::digitiser::ADC_LSB_T;
                assert!(
                    (recovered_t - b_analytic[k]).abs() <= lsb_t,
                    "noise-off recovery error > 1 LSB for axis {k}"
                );
            }
        }
    }

    #[test]
    fn adc_saturation_flag_fires_above_full_scale() {
        // Place a dipole close enough to drive the field above ±10 µT FS.
        let mut scene = Scene::new();
        scene.add_dipole(DipoleSource::new([0.0, 0.0, 0.005], [0.0, 0.0, 1.0])); // 1 A·m² at 5 mm
        scene.add_sensor([0.0, 0.0, 0.0]);
        let cfg = PipelineConfig {
            sensor: NvSensorConfig {
                shot_noise_disabled: true,
                ..NvSensorConfig::default()
            },
            ..PipelineConfig::default()
        };
        let frames = Pipeline::new(scene, cfg, 0).run(4);
        let any_sat = frames.iter().any(|f| f.has_flag(flag::ADC_SATURATED));
        assert!(
            any_sat,
            "ADC_SATURATED flag did not fire on a near-field dipole that should drive FS"
        );
    }
}
