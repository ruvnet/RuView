//! Triangulation for 2D/3D position estimation from multiple sensors.

use crate::domain::{Coordinates3D, LocationUncertainty, SensorPosition};

/// Configuration for triangulation
#[derive(Debug, Clone)]
pub struct TriangulationConfig {
    /// Minimum number of sensors required
    pub min_sensors: usize,
    /// Maximum position uncertainty to accept (meters)
    pub max_uncertainty: f64,
    /// Path loss exponent for distance estimation
    pub path_loss_exponent: f64,
    /// Reference distance for path loss model (meters)
    pub reference_distance: f64,
    /// Reference RSSI at reference distance (dBm)
    pub reference_rssi: f64,
    /// Use weighted least squares
    pub weighted: bool,
}

impl Default for TriangulationConfig {
    fn default() -> Self {
        Self {
            min_sensors: 3,
            max_uncertainty: 5.0,
            path_loss_exponent: 3.0,  // Indoor with obstacles
            reference_distance: 1.0,
            reference_rssi: -30.0,
            weighted: true,
        }
    }
}

/// Result of a distance estimation
#[derive(Debug, Clone)]
pub struct DistanceEstimate {
    /// Sensor ID
    pub sensor_id: String,
    /// Estimated distance in meters
    pub distance: f64,
    /// Estimation confidence
    pub confidence: f64,
}

/// Triangulator for position estimation
pub struct Triangulator {
    config: TriangulationConfig,
}

impl Triangulator {
    /// Create a new triangulator
    pub fn new(config: TriangulationConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(TriangulationConfig::default())
    }

    /// Estimate position from RSSI measurements
    pub fn estimate_position(
        &self,
        sensors: &[SensorPosition],
        rssi_values: &[(String, f64)],  // (sensor_id, rssi)
    ) -> Option<Coordinates3D> {
        // Get distance estimates from RSSI
        let distances: Vec<(SensorPosition, f64)> = rssi_values
            .iter()
            .filter_map(|(id, rssi)| {
                let sensor = sensors.iter().find(|s| &s.id == id)?;
                if !sensor.is_operational {
                    return None;
                }
                let distance = self.rssi_to_distance(*rssi);
                Some((sensor.clone(), distance))
            })
            .collect();

        if distances.len() < self.config.min_sensors {
            return None;
        }

        // Perform trilateration
        self.trilaterate(&distances)
    }

    /// Estimate position from Time of Arrival measurements
    pub fn estimate_from_toa(
        &self,
        sensors: &[SensorPosition],
        toa_values: &[(String, f64)],  // (sensor_id, time_of_arrival_ns)
    ) -> Option<Coordinates3D> {
        const SPEED_OF_LIGHT: f64 = 299_792_458.0; // m/s

        let distances: Vec<(SensorPosition, f64)> = toa_values
            .iter()
            .filter_map(|(id, toa)| {
                let sensor = sensors.iter().find(|s| &s.id == id)?;
                if !sensor.is_operational {
                    return None;
                }
                // Convert nanoseconds to distance
                let distance = (*toa * 1e-9) * SPEED_OF_LIGHT / 2.0; // Round trip
                Some((sensor.clone(), distance))
            })
            .collect();

        if distances.len() < self.config.min_sensors {
            return None;
        }

        self.trilaterate(&distances)
    }

    /// Convert RSSI to distance using path loss model
    fn rssi_to_distance(&self, rssi: f64) -> f64 {
        // Log-distance path loss model:
        // RSSI = RSSI_0 - 10 * n * log10(d / d_0)
        // Solving for d:
        // d = d_0 * 10^((RSSI_0 - RSSI) / (10 * n))

        let exponent = (self.config.reference_rssi - rssi)
            / (10.0 * self.config.path_loss_exponent);

        self.config.reference_distance * 10.0_f64.powf(exponent)
    }

    /// Perform trilateration using least squares
    fn trilaterate(&self, distances: &[(SensorPosition, f64)]) -> Option<Coordinates3D> {
        if distances.len() < 3 {
            return None;
        }

        // Use linearized least squares approach
        // Reference: https://en.wikipedia.org/wiki/Trilateration

        // Use first sensor as reference
        let (ref_sensor, ref_dist) = &distances[0];
        let x1 = ref_sensor.x;
        let y1 = ref_sensor.y;
        let r1 = *ref_dist;

        // Build system of linear equations: A * [x, y]^T = b
        let n = distances.len() - 1;
        let mut a_matrix = vec![vec![0.0; 2]; n];
        let mut b_vector = vec![0.0; n];

        for (i, (sensor, dist)) in distances.iter().skip(1).enumerate() {
            let xi = sensor.x;
            let yi = sensor.y;
            let ri = *dist;

            // Linearized equation from difference of squared distances
            a_matrix[i][0] = 2.0 * (xi - x1);
            a_matrix[i][1] = 2.0 * (yi - y1);
            b_vector[i] = r1 * r1 - ri * ri - x1 * x1 + xi * xi - y1 * y1 + yi * yi;
        }

        // Solve using least squares: (A^T * A)^-1 * A^T * b
        let solution = self.solve_least_squares(&a_matrix, &b_vector)?;

        // Calculate uncertainty from residuals
        let uncertainty = self.calculate_uncertainty(&solution, distances);

        if uncertainty.horizontal_error > self.config.max_uncertainty {
            return None;
        }

        Some(Coordinates3D::new(
            solution[0],
            solution[1],
            0.0, // Z estimated separately
            uncertainty,
        ))
    }

    /// Solve linear system using least squares
    fn solve_least_squares(&self, a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
        let n = a.len();
        if n < 2 || a[0].len() != 2 {
            return None;
        }

        // Calculate A^T * A
        let mut ata = vec![vec![0.0; 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..n {
                    ata[i][j] += a[k][i] * a[k][j];
                }
            }
        }

        // Calculate A^T * b
        let mut atb = vec![0.0; 2];
        for i in 0..2 {
            for k in 0..n {
                atb[i] += a[k][i] * b[k];
            }
        }

        // Solve 2x2 system using Cramer's rule
        let det = ata[0][0] * ata[1][1] - ata[0][1] * ata[1][0];
        if det.abs() < 1e-10 {
            return None;
        }

        let x = (atb[0] * ata[1][1] - atb[1] * ata[0][1]) / det;
        let y = (ata[0][0] * atb[1] - ata[1][0] * atb[0]) / det;

        Some(vec![x, y])
    }

    /// Calculate position uncertainty from residuals
    fn calculate_uncertainty(
        &self,
        position: &[f64],
        distances: &[(SensorPosition, f64)],
    ) -> LocationUncertainty {
        // Calculate root mean square error
        let mut sum_sq_error = 0.0;

        for (sensor, measured_dist) in distances {
            let dx = position[0] - sensor.x;
            let dy = position[1] - sensor.y;
            let estimated_dist = (dx * dx + dy * dy).sqrt();
            let error = measured_dist - estimated_dist;
            sum_sq_error += error * error;
        }

        let rmse = (sum_sq_error / distances.len() as f64).sqrt();

        // GDOP (Geometric Dilution of Precision) approximation
        let gdop = self.estimate_gdop(position, distances);

        LocationUncertainty {
            horizontal_error: rmse * gdop,
            vertical_error: rmse * gdop * 1.5, // Vertical typically less accurate
            confidence: 0.95,
        }
    }

    /// Estimate Geometric Dilution of Precision
    fn estimate_gdop(&self, position: &[f64], distances: &[(SensorPosition, f64)]) -> f64 {
        // Simplified GDOP based on sensor geometry
        let mut sum_angle = 0.0;
        let n = distances.len();

        for i in 0..n {
            for j in (i + 1)..n {
                let dx1 = distances[i].0.x - position[0];
                let dy1 = distances[i].0.y - position[1];
                let dx2 = distances[j].0.x - position[0];
                let dy2 = distances[j].0.y - position[1];

                let dot = dx1 * dx2 + dy1 * dy2;
                let mag1 = (dx1 * dx1 + dy1 * dy1).sqrt();
                let mag2 = (dx2 * dx2 + dy2 * dy2).sqrt();

                if mag1 > 0.0 && mag2 > 0.0 {
                    let cos_angle = (dot / (mag1 * mag2)).clamp(-1.0, 1.0);
                    let angle = cos_angle.acos();
                    sum_angle += angle;
                }
            }
        }

        // Average angle between sensor pairs
        let num_pairs = (n * (n - 1)) as f64 / 2.0;
        let avg_angle = if num_pairs > 0.0 {
            sum_angle / num_pairs
        } else {
            std::f64::consts::PI / 4.0
        };

        // GDOP is better when sensors are spread out (angle closer to 90 degrees)
        // GDOP gets worse as sensors are collinear
        let optimal_angle = std::f64::consts::PI / 2.0;
        let angle_factor = (avg_angle / optimal_angle - 1.0).abs() + 1.0;

        angle_factor.max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SensorType;

    fn create_test_sensors() -> Vec<SensorPosition> {
        vec![
            SensorPosition {
                id: "s1".to_string(),
                x: 0.0,
                y: 0.0,
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            },
            SensorPosition {
                id: "s2".to_string(),
                x: 10.0,
                y: 0.0,
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            },
            SensorPosition {
                id: "s3".to_string(),
                x: 5.0,
                y: 10.0,
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            },
        ]
    }

    #[test]
    fn test_rssi_to_distance() {
        let triangulator = Triangulator::with_defaults();

        // At reference distance, RSSI should equal reference RSSI
        let distance = triangulator.rssi_to_distance(-30.0);
        assert!((distance - 1.0).abs() < 0.1);

        // Weaker signal = further distance
        let distance2 = triangulator.rssi_to_distance(-60.0);
        assert!(distance2 > distance);
    }

    #[test]
    fn test_trilateration() {
        let triangulator = Triangulator::with_defaults();
        let sensors = create_test_sensors();

        // Target at (5, 4) - calculate distances
        let target: (f64, f64) = (5.0, 4.0);
        let distances: Vec<(&str, f64)> = vec![
            ("s1", ((target.0 - 0.0_f64).powi(2) + (target.1 - 0.0_f64).powi(2)).sqrt()),
            ("s2", ((target.0 - 10.0_f64).powi(2) + (target.1 - 0.0_f64).powi(2)).sqrt()),
            ("s3", ((target.0 - 5.0_f64).powi(2) + (target.1 - 10.0_f64).powi(2)).sqrt()),
        ];

        let dist_vec: Vec<(SensorPosition, f64)> = distances
            .iter()
            .filter_map(|(id, d)| {
                let sensor = sensors.iter().find(|s| s.id == *id)?;
                Some((sensor.clone(), *d))
            })
            .collect();

        let result = triangulator.trilaterate(&dist_vec);
        assert!(result.is_some());

        let pos = result.unwrap();
        assert!((pos.x - target.0).abs() < 0.5);
        assert!((pos.y - target.1).abs() < 0.5);
    }

    #[test]
    fn test_insufficient_sensors() {
        let triangulator = Triangulator::with_defaults();
        let sensors = create_test_sensors();

        // Only 2 distance measurements
        let rssi_values = vec![
            ("s1".to_string(), -40.0),
            ("s2".to_string(), -45.0),
        ];

        let result = triangulator.estimate_position(&sensors, &rssi_values);
        assert!(result.is_none());
    }
}
