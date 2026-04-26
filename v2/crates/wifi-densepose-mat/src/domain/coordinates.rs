//! 3D coordinate system and location types for survivor localization.

/// 3D coordinates representing survivor position
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Coordinates3D {
    /// East-West offset from reference point (meters)
    pub x: f64,
    /// North-South offset from reference point (meters)
    pub y: f64,
    /// Vertical offset - negative is below surface (meters)
    pub z: f64,
    /// Uncertainty bounds for this position
    pub uncertainty: LocationUncertainty,
}

impl Coordinates3D {
    /// Create new coordinates with uncertainty
    pub fn new(x: f64, y: f64, z: f64, uncertainty: LocationUncertainty) -> Self {
        Self { x, y, z, uncertainty }
    }

    /// Create coordinates with default uncertainty
    pub fn with_default_uncertainty(x: f64, y: f64, z: f64) -> Self {
        Self {
            x,
            y,
            z,
            uncertainty: LocationUncertainty::default(),
        }
    }

    /// Calculate 3D distance to another point
    pub fn distance_to(&self, other: &Coordinates3D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Calculate horizontal (2D) distance only
    pub fn horizontal_distance_to(&self, other: &Coordinates3D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get depth below surface (positive value)
    pub fn depth(&self) -> f64 {
        -self.z.min(0.0)
    }

    /// Check if position is below surface
    pub fn is_buried(&self) -> bool {
        self.z < 0.0
    }

    /// Get the 95% confidence radius (horizontal)
    pub fn confidence_radius(&self) -> f64 {
        self.uncertainty.horizontal_error
    }
}

/// Uncertainty bounds for a position estimate
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LocationUncertainty {
    /// Horizontal error radius at 95% confidence (meters)
    pub horizontal_error: f64,
    /// Vertical error at 95% confidence (meters)
    pub vertical_error: f64,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

impl Default for LocationUncertainty {
    fn default() -> Self {
        Self {
            horizontal_error: 2.0,  // 2 meter default uncertainty
            vertical_error: 1.0,    // 1 meter vertical uncertainty
            confidence: 0.95,       // 95% confidence
        }
    }
}

impl LocationUncertainty {
    /// Create uncertainty with specific error bounds
    pub fn new(horizontal_error: f64, vertical_error: f64) -> Self {
        Self {
            horizontal_error,
            vertical_error,
            confidence: 0.95,
        }
    }

    /// Create high-confidence uncertainty
    pub fn high_confidence(horizontal_error: f64, vertical_error: f64) -> Self {
        Self {
            horizontal_error,
            vertical_error,
            confidence: 0.99,
        }
    }

    /// Check if uncertainty is acceptable for rescue operations
    pub fn is_actionable(&self) -> bool {
        // Within 3 meters horizontal is generally actionable
        self.horizontal_error <= 3.0 && self.confidence >= 0.8
    }

    /// Combine two uncertainties (for sensor fusion)
    pub fn combine(&self, other: &LocationUncertainty) -> LocationUncertainty {
        // Weighted combination based on confidence
        let total_conf = self.confidence + other.confidence;
        let w1 = self.confidence / total_conf;
        let w2 = other.confidence / total_conf;

        // Combined uncertainty is reduced when multiple estimates agree
        let h_var1 = self.horizontal_error * self.horizontal_error;
        let h_var2 = other.horizontal_error * other.horizontal_error;
        let combined_h_var = 1.0 / (1.0/h_var1 + 1.0/h_var2);

        let v_var1 = self.vertical_error * self.vertical_error;
        let v_var2 = other.vertical_error * other.vertical_error;
        let combined_v_var = 1.0 / (1.0/v_var1 + 1.0/v_var2);

        LocationUncertainty {
            horizontal_error: combined_h_var.sqrt(),
            vertical_error: combined_v_var.sqrt(),
            confidence: (w1 * self.confidence + w2 * other.confidence).min(0.99),
        }
    }
}

/// Depth estimate with debris profile
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DepthEstimate {
    /// Estimated depth in meters
    pub depth: f64,
    /// Uncertainty range (plus/minus)
    pub uncertainty: f64,
    /// Estimated debris composition
    pub debris_profile: DebrisProfile,
    /// Confidence in the estimate
    pub confidence: f64,
}

impl DepthEstimate {
    /// Create a new depth estimate
    pub fn new(
        depth: f64,
        uncertainty: f64,
        debris_profile: DebrisProfile,
        confidence: f64,
    ) -> Self {
        Self {
            depth,
            uncertainty,
            debris_profile,
            confidence,
        }
    }

    /// Get minimum possible depth
    pub fn min_depth(&self) -> f64 {
        (self.depth - self.uncertainty).max(0.0)
    }

    /// Get maximum possible depth
    pub fn max_depth(&self) -> f64 {
        self.depth + self.uncertainty
    }

    /// Check if depth is shallow (easier rescue)
    pub fn is_shallow(&self) -> bool {
        self.depth < 1.5
    }

    /// Check if depth is moderate
    pub fn is_moderate(&self) -> bool {
        self.depth >= 1.5 && self.depth < 3.0
    }

    /// Check if depth is deep (difficult rescue)
    pub fn is_deep(&self) -> bool {
        self.depth >= 3.0
    }
}

/// Profile of debris material between sensor and survivor
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DebrisProfile {
    /// Primary material type
    pub primary_material: DebrisMaterial,
    /// Estimated void fraction (0.0-1.0, higher = more air gaps)
    pub void_fraction: f64,
    /// Estimated moisture content (affects signal propagation)
    pub moisture_content: MoistureLevel,
    /// Whether metal content is detected (blocks signals)
    pub metal_content: MetalContent,
}

impl Default for DebrisProfile {
    fn default() -> Self {
        Self {
            primary_material: DebrisMaterial::Mixed,
            void_fraction: 0.3,
            moisture_content: MoistureLevel::Dry,
            metal_content: MetalContent::None,
        }
    }
}

impl DebrisProfile {
    /// Calculate signal attenuation factor
    pub fn attenuation_factor(&self) -> f64 {
        let base = self.primary_material.attenuation_coefficient();
        let moisture_factor = self.moisture_content.attenuation_multiplier();
        let void_factor = 1.0 - (self.void_fraction * 0.3); // Voids reduce attenuation

        base * moisture_factor * void_factor
    }

    /// Check if debris allows good signal penetration
    pub fn is_penetrable(&self) -> bool {
        !matches!(self.metal_content, MetalContent::High | MetalContent::Blocking)
            && self.primary_material.attenuation_coefficient() < 5.0
    }
}

/// Types of debris materials
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DebrisMaterial {
    /// Lightweight concrete, drywall
    LightConcrete,
    /// Heavy concrete, brick
    HeavyConcrete,
    /// Wooden structures
    Wood,
    /// Soil, earth
    Soil,
    /// Mixed rubble (typical collapse)
    Mixed,
    /// Snow/ice (avalanche)
    Snow,
    /// Metal (poor penetration)
    Metal,
}

impl DebrisMaterial {
    /// Get RF attenuation coefficient (dB/meter)
    pub fn attenuation_coefficient(&self) -> f64 {
        match self {
            DebrisMaterial::Snow => 0.5,
            DebrisMaterial::Wood => 1.5,
            DebrisMaterial::LightConcrete => 3.0,
            DebrisMaterial::Soil => 4.0,
            DebrisMaterial::Mixed => 4.5,
            DebrisMaterial::HeavyConcrete => 6.0,
            DebrisMaterial::Metal => 20.0,
        }
    }
}

/// Moisture level in debris
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MoistureLevel {
    /// Dry conditions
    Dry,
    /// Slightly damp
    Damp,
    /// Wet (rain, flooding)
    Wet,
    /// Saturated (submerged)
    Saturated,
}

impl MoistureLevel {
    /// Get attenuation multiplier
    pub fn attenuation_multiplier(&self) -> f64 {
        match self {
            MoistureLevel::Dry => 1.0,
            MoistureLevel::Damp => 1.3,
            MoistureLevel::Wet => 1.8,
            MoistureLevel::Saturated => 2.5,
        }
    }
}

/// Metal content in debris
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MetalContent {
    /// No significant metal
    None,
    /// Low metal content (rebar, pipes)
    Low,
    /// Moderate metal (structural steel)
    Moderate,
    /// High metal content
    High,
    /// Metal is blocking signal
    Blocking,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_calculation() {
        let p1 = Coordinates3D::with_default_uncertainty(0.0, 0.0, 0.0);
        let p2 = Coordinates3D::with_default_uncertainty(3.0, 4.0, 0.0);

        assert!((p1.distance_to(&p2) - 5.0).abs() < 0.001);
        assert!((p1.horizontal_distance_to(&p2) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_depth_calculation() {
        let surface = Coordinates3D::with_default_uncertainty(0.0, 0.0, 0.0);
        assert!(!surface.is_buried());
        assert!(surface.depth().abs() < 0.001);

        let buried = Coordinates3D::with_default_uncertainty(0.0, 0.0, -2.5);
        assert!(buried.is_buried());
        assert!((buried.depth() - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_uncertainty_combination() {
        let u1 = LocationUncertainty::new(2.0, 1.0);
        let u2 = LocationUncertainty::new(2.0, 1.0);

        let combined = u1.combine(&u2);

        // Combined uncertainty should be lower than individual
        assert!(combined.horizontal_error < u1.horizontal_error);
    }

    #[test]
    fn test_depth_estimate_categories() {
        let shallow = DepthEstimate::new(1.0, 0.2, DebrisProfile::default(), 0.8);
        assert!(shallow.is_shallow());

        let moderate = DepthEstimate::new(2.0, 0.3, DebrisProfile::default(), 0.7);
        assert!(moderate.is_moderate());

        let deep = DepthEstimate::new(4.0, 0.5, DebrisProfile::default(), 0.6);
        assert!(deep.is_deep());
    }

    #[test]
    fn test_debris_attenuation() {
        let snow = DebrisProfile {
            primary_material: DebrisMaterial::Snow,
            ..Default::default()
        };
        let concrete = DebrisProfile {
            primary_material: DebrisMaterial::HeavyConcrete,
            ..Default::default()
        };

        assert!(snow.attenuation_factor() < concrete.attenuation_factor());
        assert!(snow.is_penetrable());
    }
}
