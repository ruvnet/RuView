//! Position fusion combining multiple localization techniques.

use crate::domain::{
    Coordinates3D, LocationUncertainty, ScanZone, VitalSignsReading,
    DepthEstimate, DebrisProfile,
};
use super::{Triangulator, TriangulationConfig, DepthEstimator, DepthEstimatorConfig};

/// Service for survivor localization
pub struct LocalizationService {
    triangulator: Triangulator,
    depth_estimator: DepthEstimator,
    position_fuser: PositionFuser,
}

impl LocalizationService {
    /// Create a new localization service
    pub fn new() -> Self {
        Self {
            triangulator: Triangulator::with_defaults(),
            depth_estimator: DepthEstimator::with_defaults(),
            position_fuser: PositionFuser::new(),
        }
    }

    /// Create with custom configurations
    pub fn with_config(
        triangulation_config: TriangulationConfig,
        depth_config: DepthEstimatorConfig,
    ) -> Self {
        Self {
            triangulator: Triangulator::new(triangulation_config),
            depth_estimator: DepthEstimator::new(depth_config),
            position_fuser: PositionFuser::new(),
        }
    }

    /// Estimate survivor position
    pub fn estimate_position(
        &self,
        vitals: &VitalSignsReading,
        zone: &ScanZone,
    ) -> Option<Coordinates3D> {
        // Get sensor positions
        let sensors = zone.sensor_positions();

        if sensors.len() < 3 {
            return None;
        }

        // Estimate 2D position from triangulation
        // In real implementation, RSSI values would come from actual measurements
        let rssi_values = self.simulate_rssi_measurements(sensors, vitals);
        let position_2d = self.triangulator.estimate_position(sensors, &rssi_values)?;

        // Estimate depth
        let debris_profile = self.estimate_debris_profile(zone);
        let signal_attenuation = self.calculate_signal_attenuation(&rssi_values);
        let depth_estimate = self.depth_estimator.estimate_depth(
            signal_attenuation,
            0.0,
            &debris_profile,
        )?;

        // Combine into 3D position
        let position_3d = Coordinates3D::new(
            position_2d.x,
            position_2d.y,
            -depth_estimate.depth, // Negative = below surface
            self.combine_uncertainties(&position_2d.uncertainty, &depth_estimate),
        );

        Some(position_3d)
    }

    /// Simulate RSSI measurements (placeholder for real sensor data)
    fn simulate_rssi_measurements(
        &self,
        sensors: &[crate::domain::SensorPosition],
        _vitals: &VitalSignsReading,
    ) -> Vec<(String, f64)> {
        // In production, this would read actual sensor values
        // For now, return placeholder values
        sensors.iter()
            .map(|s| (s.id.clone(), -50.0 + rand_range(-10.0, 10.0)))
            .collect()
    }

    /// Estimate debris profile for the zone
    fn estimate_debris_profile(&self, _zone: &ScanZone) -> DebrisProfile {
        // Would use zone metadata and signal analysis
        DebrisProfile::default()
    }

    /// Calculate average signal attenuation
    fn calculate_signal_attenuation(&self, rssi_values: &[(String, f64)]) -> f64 {
        if rssi_values.is_empty() {
            return 0.0;
        }

        // Reference RSSI at surface (typical open-air value)
        const REFERENCE_RSSI: f64 = -30.0;

        let avg_rssi: f64 = rssi_values.iter().map(|(_, r)| r).sum::<f64>()
            / rssi_values.len() as f64;

        (REFERENCE_RSSI - avg_rssi).max(0.0)
    }

    /// Combine horizontal and depth uncertainties
    fn combine_uncertainties(
        &self,
        horizontal: &LocationUncertainty,
        depth: &DepthEstimate,
    ) -> LocationUncertainty {
        LocationUncertainty {
            horizontal_error: horizontal.horizontal_error,
            vertical_error: depth.uncertainty,
            confidence: (horizontal.confidence * depth.confidence).sqrt(),
        }
    }
}

impl Default for LocalizationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Fuses multiple position estimates
pub struct PositionFuser {
    /// History of position estimates for smoothing
    history: parking_lot::RwLock<Vec<PositionEstimate>>,
    /// Maximum history size
    max_history: usize,
}

/// A position estimate with metadata
#[derive(Debug, Clone)]
pub struct PositionEstimate {
    /// The position
    pub position: Coordinates3D,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Source of estimate
    pub source: EstimateSource,
    /// Weight for fusion
    pub weight: f64,
}

/// Source of a position estimate
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstimateSource {
    /// From RSSI-based triangulation
    RssiTriangulation,
    /// From time-of-arrival
    TimeOfArrival,
    /// From CSI fingerprinting
    CsiFingerprint,
    /// From angle of arrival
    AngleOfArrival,
    /// From depth estimation
    DepthEstimation,
    /// Fused from multiple sources
    Fused,
}

impl PositionFuser {
    /// Create a new position fuser
    pub fn new() -> Self {
        Self {
            history: parking_lot::RwLock::new(Vec::new()),
            max_history: 20,
        }
    }

    /// Add a position estimate
    pub fn add_estimate(&self, estimate: PositionEstimate) {
        let mut history = self.history.write();
        history.push(estimate);

        // Keep only recent history
        if history.len() > self.max_history {
            history.remove(0);
        }
    }

    /// Fuse multiple position estimates into one
    pub fn fuse(&self, estimates: &[PositionEstimate]) -> Option<Coordinates3D> {
        if estimates.is_empty() {
            return None;
        }

        if estimates.len() == 1 {
            return Some(estimates[0].position.clone());
        }

        // Weighted average based on uncertainty and source confidence
        let mut total_weight = 0.0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;

        for estimate in estimates {
            let weight = self.calculate_weight(estimate);
            total_weight += weight;
            sum_x += estimate.position.x * weight;
            sum_y += estimate.position.y * weight;
            sum_z += estimate.position.z * weight;
        }

        if total_weight == 0.0 {
            return None;
        }

        let fused_x = sum_x / total_weight;
        let fused_y = sum_y / total_weight;
        let fused_z = sum_z / total_weight;

        // Calculate fused uncertainty (reduced due to multiple estimates)
        let fused_uncertainty = self.calculate_fused_uncertainty(estimates);

        Some(Coordinates3D::new(
            fused_x,
            fused_y,
            fused_z,
            fused_uncertainty,
        ))
    }

    /// Fuse with temporal smoothing
    pub fn fuse_with_history(&self, current: &PositionEstimate) -> Option<Coordinates3D> {
        // Add current to history
        self.add_estimate(current.clone());

        let history = self.history.read();

        // Use exponentially weighted moving average
        let alpha: f64 = 0.3; // Smoothing factor
        let mut smoothed = current.position.clone();

        for (i, estimate) in history.iter().rev().enumerate().skip(1) {
            let weight = alpha * (1.0_f64 - alpha).powi(i as i32);
            smoothed.x = smoothed.x * (1.0 - weight) + estimate.position.x * weight;
            smoothed.y = smoothed.y * (1.0 - weight) + estimate.position.y * weight;
            smoothed.z = smoothed.z * (1.0 - weight) + estimate.position.z * weight;
        }

        Some(smoothed)
    }

    /// Calculate weight for an estimate
    fn calculate_weight(&self, estimate: &PositionEstimate) -> f64 {
        // Base weight from source reliability
        let source_weight = match estimate.source {
            EstimateSource::TimeOfArrival => 1.0,
            EstimateSource::AngleOfArrival => 0.9,
            EstimateSource::CsiFingerprint => 0.8,
            EstimateSource::RssiTriangulation => 0.7,
            EstimateSource::DepthEstimation => 0.6,
            EstimateSource::Fused => 1.0,
        };

        // Adjust by uncertainty (lower uncertainty = higher weight)
        let uncertainty_factor = 1.0 / (1.0 + estimate.position.uncertainty.horizontal_error);

        // User-provided weight
        let user_weight = estimate.weight;

        source_weight * uncertainty_factor * user_weight
    }

    /// Calculate uncertainty after fusing multiple estimates
    fn calculate_fused_uncertainty(&self, estimates: &[PositionEstimate]) -> LocationUncertainty {
        if estimates.is_empty() {
            return LocationUncertainty::default();
        }

        // Combined uncertainty is reduced with multiple estimates
        let n = estimates.len() as f64;

        let avg_h_error: f64 = estimates.iter()
            .map(|e| e.position.uncertainty.horizontal_error)
            .sum::<f64>() / n;

        let avg_v_error: f64 = estimates.iter()
            .map(|e| e.position.uncertainty.vertical_error)
            .sum::<f64>() / n;

        // Uncertainty reduction factor (more estimates = more confidence)
        let reduction = (1.0 / n.sqrt()).max(0.5);

        LocationUncertainty {
            horizontal_error: avg_h_error * reduction,
            vertical_error: avg_v_error * reduction,
            confidence: (0.95 * (1.0 + (n - 1.0) * 0.02)).min(0.99),
        }
    }

    /// Clear history
    pub fn clear_history(&self) {
        self.history.write().clear();
    }
}

impl Default for PositionFuser {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple random range (for simulation)
fn rand_range(min: f64, max: f64) -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let pseudo_random = ((seed * 1103515245 + 12345) % (1 << 31)) as f64 / (1u64 << 31) as f64;
    min + pseudo_random * (max - min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_estimate(x: f64, y: f64, z: f64) -> PositionEstimate {
        PositionEstimate {
            position: Coordinates3D::with_default_uncertainty(x, y, z),
            timestamp: Utc::now(),
            source: EstimateSource::RssiTriangulation,
            weight: 1.0,
        }
    }

    #[test]
    fn test_single_estimate_fusion() {
        let fuser = PositionFuser::new();
        let estimate = create_test_estimate(5.0, 10.0, -2.0);

        let result = fuser.fuse(&[estimate]);
        assert!(result.is_some());

        let pos = result.unwrap();
        assert!((pos.x - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_multiple_estimate_fusion() {
        let fuser = PositionFuser::new();

        let estimates = vec![
            create_test_estimate(4.0, 9.0, -1.5),
            create_test_estimate(6.0, 11.0, -2.5),
        ];

        let result = fuser.fuse(&estimates);
        assert!(result.is_some());

        let pos = result.unwrap();
        // Should be roughly in between
        assert!(pos.x > 4.0 && pos.x < 6.0);
        assert!(pos.y > 9.0 && pos.y < 11.0);
    }

    #[test]
    fn test_fused_uncertainty_reduction() {
        let fuser = PositionFuser::new();

        let estimates = vec![
            create_test_estimate(5.0, 10.0, -2.0),
            create_test_estimate(5.1, 10.1, -2.1),
            create_test_estimate(4.9, 9.9, -1.9),
        ];

        let single_uncertainty = estimates[0].position.uncertainty.horizontal_error;
        let fused_uncertainty = fuser.calculate_fused_uncertainty(&estimates);

        // Fused should have lower uncertainty
        assert!(fused_uncertainty.horizontal_error < single_uncertainty);
    }

    #[test]
    fn test_localization_service_creation() {
        let service = LocalizationService::new();
        // Just verify it creates without panic
        assert!(true);
        drop(service);
    }
}
