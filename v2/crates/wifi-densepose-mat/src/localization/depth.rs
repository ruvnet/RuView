//! Depth estimation through debris layers.

use crate::domain::{DebrisProfile, DepthEstimate, DebrisMaterial, MoistureLevel};

/// Configuration for depth estimation
#[derive(Debug, Clone)]
pub struct DepthEstimatorConfig {
    /// Maximum depth to estimate (meters)
    pub max_depth: f64,
    /// Minimum signal attenuation to consider (dB)
    pub min_attenuation: f64,
    /// WiFi frequency in GHz
    pub frequency_ghz: f64,
    /// Free space path loss at 1 meter (dB)
    pub free_space_loss_1m: f64,
}

impl Default for DepthEstimatorConfig {
    fn default() -> Self {
        Self {
            max_depth: 10.0,
            min_attenuation: 3.0,
            frequency_ghz: 5.8, // 5.8 GHz WiFi
            free_space_loss_1m: 47.0, // FSPL at 1m for 5.8 GHz
        }
    }
}

/// Estimator for survivor depth through debris
pub struct DepthEstimator {
    config: DepthEstimatorConfig,
}

impl DepthEstimator {
    /// Create a new depth estimator
    pub fn new(config: DepthEstimatorConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DepthEstimatorConfig::default())
    }

    /// Estimate depth from signal attenuation
    pub fn estimate_depth(
        &self,
        signal_attenuation: f64,  // Total attenuation in dB
        distance_2d: f64,         // Horizontal distance in meters
        debris_profile: &DebrisProfile,
    ) -> Option<DepthEstimate> {
        if signal_attenuation < self.config.min_attenuation {
            // Very little attenuation - probably not buried
            return Some(DepthEstimate {
                depth: 0.0,
                uncertainty: 0.5,
                debris_profile: debris_profile.clone(),
                confidence: 0.9,
            });
        }

        // Calculate free space path loss for horizontal distance
        let fspl = self.free_space_path_loss(distance_2d);

        // Debris attenuation = total - free space loss
        let debris_attenuation = (signal_attenuation - fspl).max(0.0);

        // Get attenuation coefficient for debris type
        let attenuation_per_meter = debris_profile.attenuation_factor();

        if attenuation_per_meter < 0.1 {
            return None;
        }

        // Estimate depth
        let depth = debris_attenuation / attenuation_per_meter;

        // Clamp to maximum
        if depth > self.config.max_depth {
            return None;
        }

        // Calculate uncertainty (increases with depth and material variability)
        let base_uncertainty = 0.3;
        let depth_uncertainty = depth * 0.15;
        let material_uncertainty = self.material_uncertainty(debris_profile);
        let uncertainty = base_uncertainty + depth_uncertainty + material_uncertainty;

        // Calculate confidence (decreases with depth)
        let confidence = (1.0 - depth / self.config.max_depth).max(0.3);

        Some(DepthEstimate {
            depth,
            uncertainty,
            debris_profile: debris_profile.clone(),
            confidence,
        })
    }

    /// Estimate debris profile from signal characteristics
    pub fn estimate_debris_profile(
        &self,
        signal_variance: f64,
        signal_multipath: f64,
        moisture_indicator: f64,
    ) -> DebrisProfile {
        // Estimate material based on signal characteristics
        let primary_material = if signal_variance > 0.5 {
            // High variance suggests heterogeneous material
            DebrisMaterial::Mixed
        } else if signal_multipath > 0.7 {
            // High multipath suggests reflective surfaces
            DebrisMaterial::HeavyConcrete
        } else if signal_multipath < 0.3 {
            // Low multipath suggests absorptive material
            DebrisMaterial::Soil
        } else {
            DebrisMaterial::LightConcrete
        };

        // Estimate void fraction from multipath
        let void_fraction = signal_multipath.clamp(0.1, 0.5);

        // Estimate moisture from signal characteristics
        let moisture_content = if moisture_indicator > 0.7 {
            MoistureLevel::Wet
        } else if moisture_indicator > 0.4 {
            MoistureLevel::Damp
        } else {
            MoistureLevel::Dry
        };

        DebrisProfile {
            primary_material,
            void_fraction,
            moisture_content,
            metal_content: crate::domain::MetalContent::Low,
        }
    }

    /// Calculate free space path loss
    fn free_space_path_loss(&self, distance: f64) -> f64 {
        // FSPL = 20*log10(d) + 20*log10(f) + 20*log10(4*pi/c)
        // Simplified: FSPL(d) = FSPL(1m) + 20*log10(d)

        if distance <= 0.0 {
            return 0.0;
        }

        self.config.free_space_loss_1m + 20.0 * distance.log10()
    }

    /// Calculate uncertainty based on material properties
    fn material_uncertainty(&self, profile: &DebrisProfile) -> f64 {
        // Mixed materials have higher uncertainty
        let material_factor = match profile.primary_material {
            DebrisMaterial::Mixed => 0.4,
            DebrisMaterial::HeavyConcrete => 0.2,
            DebrisMaterial::LightConcrete => 0.2,
            DebrisMaterial::Soil => 0.3,
            DebrisMaterial::Wood => 0.15,
            DebrisMaterial::Snow => 0.1,
            DebrisMaterial::Metal => 0.5, // Very unpredictable
        };

        // Moisture adds uncertainty
        let moisture_factor = match profile.moisture_content {
            MoistureLevel::Dry => 0.0,
            MoistureLevel::Damp => 0.1,
            MoistureLevel::Wet => 0.2,
            MoistureLevel::Saturated => 0.3,
        };

        material_factor + moisture_factor
    }

    /// Estimate depth from multiple signal paths
    pub fn estimate_from_multipath(
        &self,
        direct_path_attenuation: f64,
        reflected_paths: &[(f64, f64)],  // (attenuation, delay)
        debris_profile: &DebrisProfile,
    ) -> Option<DepthEstimate> {
        // Use path differences to estimate depth
        if reflected_paths.is_empty() {
            return self.estimate_depth(direct_path_attenuation, 0.0, debris_profile);
        }

        // Average extra path length from reflections
        const SPEED_OF_LIGHT: f64 = 299_792_458.0;
        let avg_extra_path: f64 = reflected_paths
            .iter()
            .map(|(_, delay)| delay * SPEED_OF_LIGHT / 2.0) // Round trip
            .sum::<f64>() / reflected_paths.len() as f64;

        // Extra path length is approximately related to depth
        // (reflections bounce off debris layers)
        let estimated_depth = avg_extra_path / 4.0; // Empirical factor

        let attenuation_per_meter = debris_profile.attenuation_factor();
        let attenuation_based_depth = direct_path_attenuation / attenuation_per_meter;

        // Combine estimates
        let depth = (estimated_depth + attenuation_based_depth) / 2.0;

        if depth > self.config.max_depth {
            return None;
        }

        let uncertainty = 0.5 + depth * 0.2;
        let confidence = (1.0 - depth / self.config.max_depth).max(0.3);

        Some(DepthEstimate {
            depth,
            uncertainty,
            debris_profile: debris_profile.clone(),
            confidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_debris() -> DebrisProfile {
        DebrisProfile {
            primary_material: DebrisMaterial::Mixed,
            void_fraction: 0.25,
            moisture_content: MoistureLevel::Dry,
            metal_content: crate::domain::MetalContent::Low,
        }
    }

    #[test]
    fn test_low_attenuation_surface() {
        let estimator = DepthEstimator::with_defaults();

        let result = estimator.estimate_depth(1.0, 5.0, &default_debris());
        assert!(result.is_some());

        let estimate = result.unwrap();
        assert!(estimate.depth < 0.1);
        assert!(estimate.confidence > 0.8);
    }

    #[test]
    fn test_depth_increases_with_attenuation() {
        let estimator = DepthEstimator::with_defaults();
        let debris = default_debris();

        let low = estimator.estimate_depth(10.0, 0.0, &debris);
        let high = estimator.estimate_depth(30.0, 0.0, &debris);

        assert!(low.is_some() && high.is_some());
        assert!(high.unwrap().depth > low.unwrap().depth);
    }

    #[test]
    fn test_confidence_decreases_with_depth() {
        let estimator = DepthEstimator::with_defaults();
        let debris = default_debris();

        let shallow = estimator.estimate_depth(5.0, 0.0, &debris);
        let deep = estimator.estimate_depth(40.0, 0.0, &debris);

        if let (Some(s), Some(d)) = (shallow, deep) {
            assert!(s.confidence > d.confidence);
        }
    }

    #[test]
    fn test_debris_profile_estimation() {
        let estimator = DepthEstimator::with_defaults();

        // High variance = mixed materials
        let profile = estimator.estimate_debris_profile(0.7, 0.5, 0.3);
        assert!(matches!(profile.primary_material, DebrisMaterial::Mixed));

        // High multipath = concrete
        let profile2 = estimator.estimate_debris_profile(0.2, 0.8, 0.3);
        assert!(matches!(profile2.primary_material, DebrisMaterial::HeavyConcrete));
    }

    #[test]
    fn test_free_space_path_loss() {
        let estimator = DepthEstimator::with_defaults();

        // FSPL increases with distance
        let fspl_1m = estimator.free_space_path_loss(1.0);
        let fspl_10m = estimator.free_space_path_loss(10.0);

        assert!(fspl_10m > fspl_1m);
        // Should be about 20 dB difference (20*log10(10))
        assert!((fspl_10m - fspl_1m - 20.0).abs() < 1.0);
    }
}
