//! OPM (Optically Pumped Magnetometer) interface.
//!
//! OPMs operating in SERF (Spin-Exchange Relaxation Free) mode provide
//! ~7 fT/sqrt(Hz) sensitivity in a compact, cryogen-free package suitable
//! for wearable MEG systems. This module implements the acquisition interface,
//! cross-talk compensation via Gaussian elimination, active shielding, and a
//! physically realistic signal model with neural oscillations and powerline
//! interference.

use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::sensor::{SensorArray, SensorChannel, SensorType};
use ruv_neural_core::signal::MultiChannelTimeSeries;
use ruv_neural_core::traits::SensorSource;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Configuration for an OPM sensor array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpmConfig {
    /// Number of OPM sensors.
    pub num_channels: usize,
    /// Sample rate in Hz.
    pub sample_rate_hz: f64,
    /// Whether SERF mode is enabled (spin-exchange relaxation free).
    pub serf_mode: bool,
    /// Helmet geometry: channel positions in head-frame coordinates.
    pub channel_positions: Vec<[f64; 3]>,
    /// Per-channel sensitivity in fT/sqrt(Hz).
    pub sensitivities: Vec<f64>,
    /// Cross-talk matrix (num_channels x num_channels).
    /// `cross_talk[i][j]` is the coupling from channel j into channel i.
    pub cross_talk: Vec<Vec<f64>>,
    /// Active shielding compensation coefficients per channel.
    pub active_shielding_coeffs: Vec<f64>,
}

impl Default for OpmConfig {
    fn default() -> Self {
        let num_channels = 32;
        let positions: Vec<[f64; 3]> = (0..num_channels)
            .map(|i| {
                let phi = 2.0 * PI * i as f64 / num_channels as f64;
                let theta = PI / 4.0 + (i as f64 / num_channels as f64) * PI / 2.0;
                let r = 0.1;
                [
                    r * theta.sin() * phi.cos(),
                    r * theta.sin() * phi.sin(),
                    r * theta.cos(),
                ]
            })
            .collect();
        let sensitivities = vec![7.0; num_channels];
        // Identity cross-talk (no coupling).
        let cross_talk = (0..num_channels)
            .map(|i| {
                (0..num_channels)
                    .map(|j| if i == j { 1.0 } else { 0.0 })
                    .collect()
            })
            .collect();
        let active_shielding_coeffs = vec![1.0; num_channels];

        Self {
            num_channels,
            sample_rate_hz: 1000.0,
            serf_mode: true,
            channel_positions: positions,
            sensitivities,
            cross_talk,
            active_shielding_coeffs,
        }
    }
}

/// OPM sensor array.
///
/// Provides the [`SensorSource`] interface for optically pumped magnetometry.
/// Generates SERF-mode magnetometer signals with realistic bandwidth (DC to
/// ~200 Hz), neural oscillations (alpha/beta/gamma), powerline harmonics,
/// and applies full cross-talk compensation and active shielding.
#[derive(Debug)]
pub struct OpmArray {
    config: OpmConfig,
    array: SensorArray,
    sample_counter: u64,
}

impl OpmArray {
    /// Create a new OPM array from configuration.
    pub fn new(config: OpmConfig) -> Self {
        let channels = (0..config.num_channels)
            .map(|i| {
                let pos = config
                    .channel_positions
                    .get(i)
                    .copied()
                    .unwrap_or([0.0, 0.0, 0.0]);
                let sens = config.sensitivities.get(i).copied().unwrap_or(7.0);
                SensorChannel {
                    id: i,
                    sensor_type: SensorType::Opm,
                    position: pos,
                    orientation: [0.0, 0.0, 1.0],
                    sensitivity_ft_sqrt_hz: sens,
                    sample_rate_hz: config.sample_rate_hz,
                    label: format!("OPM-{:03}", i),
                }
            })
            .collect();

        let array = SensorArray {
            channels,
            sensor_type: SensorType::Opm,
            name: "OpmArray".to_string(),
        };

        Self {
            config,
            array,
            sample_counter: 0,
        }
    }

    /// Returns the sensor array metadata.
    pub fn sensor_array(&self) -> &SensorArray {
        &self.array
    }

    /// Apply cross-talk compensation to raw channel data.
    ///
    /// Solves the linear system `cross_talk * corrected = raw` to obtain
    /// `corrected = inv(cross_talk) * raw`. Falls back to diagonal-only
    /// correction if the cross-talk matrix is singular.
    pub fn compensate_cross_talk(&self, raw: &mut [f64]) -> Result<()> {
        if raw.len() != self.config.num_channels {
            return Err(RuvNeuralError::DimensionMismatch {
                expected: self.config.num_channels,
                got: raw.len(),
            });
        }
        if let Some(corrected) = solve_linear_system(&self.config.cross_talk, raw) {
            raw.copy_from_slice(&corrected);
        } else {
            // Fallback: diagonal scaling when the matrix is singular.
            for (i, val) in raw.iter_mut().enumerate() {
                let diag = self.config.cross_talk[i][i];
                if diag.abs() > 1e-15 {
                    *val /= diag;
                }
            }
        }
        Ok(())
    }

    /// Apply full cross-talk compensation to an entire time-series matrix.
    ///
    /// `data` is laid out as channels x samples. The cross-talk system is
    /// solved independently for each time point (column).
    pub fn full_cross_talk_compensation(&self, data: &mut Vec<Vec<f64>>) -> Result<()> {
        let n = self.config.num_channels;
        if data.len() != n {
            return Err(RuvNeuralError::DimensionMismatch {
                expected: n,
                got: data.len(),
            });
        }
        if n == 0 {
            return Ok(());
        }
        let num_samples = data[0].len();
        for ch_data in data.iter() {
            if ch_data.len() != num_samples {
                return Err(RuvNeuralError::Sensor(
                    "all channels must have the same number of samples".to_string(),
                ));
            }
        }

        for t in 0..num_samples {
            let mut col: Vec<f64> = data.iter().map(|ch| ch[t]).collect();
            self.compensate_cross_talk(&mut col)?;
            for (ch, val) in col.into_iter().enumerate() {
                data[ch][t] = val;
            }
        }
        Ok(())
    }

    /// Apply active shielding compensation.
    pub fn apply_active_shielding(&self, data: &mut [f64]) -> Result<()> {
        if data.len() != self.config.num_channels {
            return Err(RuvNeuralError::DimensionMismatch {
                expected: self.config.num_channels,
                got: data.len(),
            });
        }
        for (i, val) in data.iter_mut().enumerate() {
            *val *= self.config.active_shielding_coeffs[i];
        }
        Ok(())
    }
}

/// Solve the linear system `matrix * x = rhs` using Gaussian elimination
/// with partial pivoting.
///
/// Returns `None` if the matrix is singular (any pivot magnitude < 1e-12).
fn solve_linear_system(matrix: &[Vec<f64>], rhs: &[f64]) -> Option<Vec<f64>> {
    let n = rhs.len();
    if matrix.len() != n {
        return None;
    }
    for row in matrix.iter() {
        if row.len() != n {
            return None;
        }
    }

    // Build augmented matrix [A | b].
    let mut aug: Vec<Vec<f64>> = matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut r = row.clone();
            r.push(rhs[i]);
            r
        })
        .collect();

    // Forward elimination with partial pivoting.
    for col in 0..n {
        // Find pivot row.
        let mut max_abs = aug[col][col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let a = aug[row][col].abs();
            if a > max_abs {
                max_abs = a;
                max_row = row;
            }
        }
        if max_abs < 1e-12 {
            return None; // Singular.
        }
        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        for row in (col + 1)..n {
            let factor = aug[row][col] / pivot;
            for j in col..=n {
                let above = aug[col][j];
                aug[row][j] -= factor * above;
            }
        }
    }

    // Back-substitution.
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = aug[i][n];
        for j in (i + 1)..n {
            sum -= aug[i][j] * x[j];
        }
        if aug[i][i].abs() < 1e-12 {
            return None;
        }
        x[i] = sum / aug[i][i];
    }
    Some(x)
}

impl SensorSource for OpmArray {
    fn sensor_type(&self) -> SensorType {
        SensorType::Opm
    }

    fn num_channels(&self) -> usize {
        self.config.num_channels
    }

    fn sample_rate_hz(&self) -> f64 {
        self.config.sample_rate_hz
    }

    fn read_chunk(&mut self, num_samples: usize) -> Result<MultiChannelTimeSeries> {
        let timestamp = self.sample_counter as f64 / self.config.sample_rate_hz;
        let dt = 1.0 / self.config.sample_rate_hz;
        let powerline_freq = 60.0; // Hz (could be made configurable)

        let mut rng = rand::thread_rng();
        let data: Vec<Vec<f64>> = (0..self.config.num_channels)
            .map(|ch| {
                let sens = self.config.sensitivities.get(ch).copied().unwrap_or(7.0);
                // White noise: sensitivity in fT/sqrt(Hz) -> per-sample sigma
                let white_sigma = sens * (self.config.sample_rate_hz / 2.0).sqrt();
                let scale = sens / 7.0; // normalized to default sensitivity
                let shielding = self.config.active_shielding_coeffs
                    .get(ch).copied().unwrap_or(1.0);

                (0..num_samples)
                    .map(|s| {
                        let t = timestamp + s as f64 * dt;

                        // 1. Brain signal: alpha + beta + gamma neural oscillations
                        let alpha = 50.0 * scale * (2.0 * PI * 10.0 * t + 0.3 * ch as f64).sin();
                        let beta = 20.0 * scale * (2.0 * PI * 20.0 * t + 0.7 * ch as f64).sin();
                        let gamma = 5.0 * scale * (2.0 * PI * 40.0 * t + 1.1 * ch as f64).sin();
                        let brain = alpha + beta + gamma;

                        // 2. Powerline harmonics (50/60 Hz + 2nd/3rd harmonics)
                        // Active shielding attenuates environmental interference.
                        // A shielding coeff of 1.0 means "fully compensated" (no residual).
                        // Values < 1.0 leave residual interference.
                        let residual = (1.0 - shielding.clamp(0.0, 1.0)).max(0.0);
                        let powerline = 500.0 * residual
                            * ((2.0 * PI * powerline_freq * t).sin()
                                + 0.3 * (2.0 * PI * 2.0 * powerline_freq * t).sin()
                                + 0.1 * (2.0 * PI * 3.0 * powerline_freq * t).sin());

                        // 3. White noise floor (SERF-mode thermal noise)
                        let u1: f64 = rand::Rng::gen::<f64>(&mut rng).max(1e-15);
                        let u2: f64 = rand::Rng::gen(&mut rng);
                        let white = white_sigma * (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();

                        brain + powerline + white
                    })
                    .collect()
            })
            .collect();

        self.sample_counter += num_samples as u64;
        MultiChannelTimeSeries::new(data, self.config.sample_rate_hz, timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a small OpmArray with a given cross-talk matrix.
    fn make_opm(cross_talk: Vec<Vec<f64>>) -> OpmArray {
        let n = cross_talk.len();
        let config = OpmConfig {
            num_channels: n,
            sample_rate_hz: 1000.0,
            serf_mode: true,
            channel_positions: vec![[0.0, 0.0, 0.0]; n],
            sensitivities: vec![7.0; n],
            cross_talk,
            active_shielding_coeffs: vec![1.0; n],
        };
        OpmArray::new(config)
    }

    #[test]
    fn identity_cross_talk_is_noop() {
        let ct = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let opm = make_opm(ct);
        let mut data = vec![1.0, 2.0, 3.0];
        opm.compensate_cross_talk(&mut data).unwrap();
        assert!((data[0] - 1.0).abs() < 1e-12);
        assert!((data[1] - 2.0).abs() < 1e-12);
        assert!((data[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn known_3x3_cross_talk_solution() {
        // Cross-talk matrix C, raw vector b.
        // We pick a known x, compute b = C * x, then verify compensation recovers x.
        let ct = vec![
            vec![2.0, 1.0, 0.0],
            vec![0.0, 3.0, 1.0],
            vec![1.0, 0.0, 2.0],
        ];
        // Known corrected values.
        let expected = vec![1.0, 2.0, 3.0];
        // raw = C * expected.
        let mut raw = vec![
            2.0 * 1.0 + 1.0 * 2.0 + 0.0 * 3.0, // 4.0
            0.0 * 1.0 + 3.0 * 2.0 + 1.0 * 3.0, // 9.0
            1.0 * 1.0 + 0.0 * 2.0 + 2.0 * 3.0, // 7.0
        ];
        let opm = make_opm(ct);
        opm.compensate_cross_talk(&mut raw).unwrap();
        for (got, want) in raw.iter().zip(expected.iter()) {
            assert!(
                (got - want).abs() < 1e-10,
                "got {got}, want {want}"
            );
        }
    }

    #[test]
    fn singular_matrix_falls_back_to_diagonal() {
        // Singular: row 1 == row 0.
        let ct = vec![
            vec![2.0, 1.0],
            vec![2.0, 1.0],
        ];
        let opm = make_opm(ct);
        let mut data = vec![4.0, 6.0];
        // Should not error -- falls back to diagonal.
        opm.compensate_cross_talk(&mut data).unwrap();
        // Diagonal fallback: data[0] /= 2.0, data[1] /= 1.0.
        assert!((data[0] - 2.0).abs() < 1e-12);
        assert!((data[1] - 6.0).abs() < 1e-12);
    }

    #[test]
    fn solve_linear_system_basic() {
        let mat = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let rhs = vec![5.0, 7.0];
        let x = solve_linear_system(&mat, &rhs).unwrap();
        assert!((x[0] - 5.0).abs() < 1e-12);
        assert!((x[1] - 7.0).abs() < 1e-12);
    }

    #[test]
    fn solve_linear_system_singular_returns_none() {
        let mat = vec![
            vec![1.0, 2.0],
            vec![2.0, 4.0],
        ];
        let rhs = vec![3.0, 6.0];
        assert!(solve_linear_system(&mat, &rhs).is_none());
    }

    #[test]
    fn full_cross_talk_compensation_time_series() {
        let ct = vec![
            vec![2.0, 1.0, 0.0],
            vec![0.0, 3.0, 1.0],
            vec![1.0, 0.0, 2.0],
        ];
        let opm = make_opm(ct.clone());

        // Two time points with known corrected values.
        let expected_t0 = vec![1.0, 2.0, 3.0];
        let expected_t1 = vec![4.0, 5.0, 6.0];

        // Compute raw = C * expected for each time point.
        let raw_t0: Vec<f64> = (0..3)
            .map(|i| ct[i].iter().zip(&expected_t0).map(|(c, x)| c * x).sum())
            .collect();
        let raw_t1: Vec<f64> = (0..3)
            .map(|i| ct[i].iter().zip(&expected_t1).map(|(c, x)| c * x).sum())
            .collect();

        // data layout: channels x samples.
        let mut data = vec![
            vec![raw_t0[0], raw_t1[0]],
            vec![raw_t0[1], raw_t1[1]],
            vec![raw_t0[2], raw_t1[2]],
        ];

        opm.full_cross_talk_compensation(&mut data).unwrap();

        for (ch, (e0, e1)) in [expected_t0, expected_t1]
            .iter()
            .enumerate()
            .flat_map(|(t, exp)| exp.iter().enumerate().map(move |(ch, &v)| (ch, (t, v))))
            .fold(
                vec![(0.0, 0.0); 3],
                |mut acc, (ch, (t, v))| {
                    if t == 0 { acc[ch].0 = v; } else { acc[ch].1 = v; }
                    acc
                },
            )
            .into_iter()
            .enumerate()
        {
            assert!(
                (data[ch][0] - e0).abs() < 1e-10,
                "ch{ch} t0: got {}, want {e0}",
                data[ch][0]
            );
            assert!(
                (data[ch][1] - e1).abs() < 1e-10,
                "ch{ch} t1: got {}, want {e1}",
                data[ch][1]
            );
        }
    }

    #[test]
    fn dimension_mismatch_error() {
        let opm = make_opm(vec![vec![1.0]]);
        let mut data = vec![1.0, 2.0];
        assert!(opm.compensate_cross_talk(&mut data).is_err());
    }
}
