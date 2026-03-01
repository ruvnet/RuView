//! Depth Computation for Hyperbolic Hierarchy
//!
//! Computes hierarchical depth from Poincare ball coordinates.

/// Epsilon for numerical stability
const EPS: f32 = 1e-5;

/// Computes depth in the Poincare ball model
///
/// Depth is defined as the hyperbolic distance from the origin,
/// which correlates with hierarchy level in embedded trees.
#[derive(Debug, Clone)]
pub struct DepthComputer {
    /// Curvature of the hyperbolic space
    curvature: f32,
    /// Threshold boundaries for hierarchy levels
    level_thresholds: [f32; 4],
}

impl DepthComputer {
    /// Create a new depth computer
    pub fn new(curvature: f32) -> Self {
        // Default thresholds based on typical hierarchy depths
        Self {
            curvature,
            level_thresholds: [0.5, 1.0, 2.0, 3.0],
        }
    }

    /// Create with custom thresholds
    pub fn with_thresholds(curvature: f32, thresholds: [f32; 4]) -> Self {
        Self {
            curvature,
            level_thresholds: thresholds,
        }
    }

    /// Compute depth as hyperbolic distance from origin
    ///
    /// In the Poincare ball, depth = 2 * arctanh(|x|) / sqrt(-c)
    pub fn compute_depth(&self, point: &[f32]) -> f32 {
        let norm_sq: f32 = point.iter().map(|x| x * x).sum();
        let norm = norm_sq.sqrt();

        if norm < EPS {
            return 0.0;
        }

        let c = -self.curvature;

        // arctanh(x) = 0.5 * ln((1+x)/(1-x))
        let clamped_norm = norm.min(1.0 - EPS);
        let arctanh = 0.5 * ((1.0 + clamped_norm) / (1.0 - clamped_norm)).ln();

        2.0 * arctanh / c.sqrt()
    }

    /// Compute normalized depth (0 to 1 range based on typical max)
    pub fn normalized_depth(&self, point: &[f32]) -> f32 {
        let depth = self.compute_depth(point);
        // Typical max depth around 5-6 for deep hierarchies
        (depth / 5.0).min(1.0)
    }

    /// Classify depth into hierarchy level
    pub fn classify_level(&self, depth: f32) -> HierarchyLevel {
        if depth < self.level_thresholds[0] {
            HierarchyLevel::Root
        } else if depth < self.level_thresholds[1] {
            HierarchyLevel::High
        } else if depth < self.level_thresholds[2] {
            HierarchyLevel::Mid
        } else if depth < self.level_thresholds[3] {
            HierarchyLevel::Deep
        } else {
            HierarchyLevel::VeryDeep
        }
    }

    /// Compute radius at which a given depth is achieved
    pub fn radius_for_depth(&self, target_depth: f32) -> f32 {
        let c = -self.curvature;
        // Inverse of depth formula: r = tanh(depth * sqrt(c) / 2)
        (target_depth * c.sqrt() / 2.0).tanh()
    }

    /// Get curvature
    pub fn curvature(&self) -> f32 {
        self.curvature
    }
}

/// Hierarchy level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HierarchyLevel {
    /// Root level (depth < 0.5)
    Root,
    /// High level (0.5 <= depth < 1.0)
    High,
    /// Mid level (1.0 <= depth < 2.0)
    Mid,
    /// Deep level (2.0 <= depth < 3.0)
    Deep,
    /// Very deep level (depth >= 3.0)
    VeryDeep,
}

impl HierarchyLevel {
    /// Get numeric level (0 = Root, 4 = VeryDeep)
    pub fn as_level(&self) -> usize {
        match self {
            Self::Root => 0,
            Self::High => 1,
            Self::Mid => 2,
            Self::Deep => 3,
            Self::VeryDeep => 4,
        }
    }

    /// Get weight multiplier for this level
    pub fn weight_multiplier(&self) -> f32 {
        match self {
            Self::Root => 1.0,
            Self::High => 1.2,
            Self::Mid => 1.5,
            Self::Deep => 2.0,
            Self::VeryDeep => 3.0,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::High => "high",
            Self::Mid => "mid",
            Self::Deep => "deep",
            Self::VeryDeep => "very_deep",
        }
    }
}

impl std::fmt::Display for HierarchyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_at_origin() {
        let computer = DepthComputer::new(-1.0);
        let origin = vec![0.0, 0.0, 0.0, 0.0];
        let depth = computer.compute_depth(&origin);
        assert!(depth < 0.01);
    }

    #[test]
    fn test_depth_increases_with_radius() {
        let computer = DepthComputer::new(-1.0);

        let point1 = vec![0.1, 0.0, 0.0, 0.0];
        let point2 = vec![0.5, 0.0, 0.0, 0.0];
        let point3 = vec![0.9, 0.0, 0.0, 0.0];

        let d1 = computer.compute_depth(&point1);
        let d2 = computer.compute_depth(&point2);
        let d3 = computer.compute_depth(&point3);

        assert!(d1 < d2);
        assert!(d2 < d3);
    }

    #[test]
    fn test_hierarchy_levels() {
        let computer = DepthComputer::new(-1.0);

        assert_eq!(computer.classify_level(0.3), HierarchyLevel::Root);
        assert_eq!(computer.classify_level(0.7), HierarchyLevel::High);
        assert_eq!(computer.classify_level(1.5), HierarchyLevel::Mid);
        assert_eq!(computer.classify_level(2.5), HierarchyLevel::Deep);
        assert_eq!(computer.classify_level(4.0), HierarchyLevel::VeryDeep);
    }

    #[test]
    fn test_radius_for_depth() {
        let computer = DepthComputer::new(-1.0);

        let radius = computer.radius_for_depth(1.0);
        let point = vec![radius, 0.0, 0.0, 0.0];
        let computed_depth = computer.compute_depth(&point);

        assert!((computed_depth - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_normalized_depth() {
        let computer = DepthComputer::new(-1.0);

        let shallow = vec![0.1, 0.0, 0.0, 0.0];
        let deep = vec![0.95, 0.0, 0.0, 0.0];

        let norm_shallow = computer.normalized_depth(&shallow);
        let norm_deep = computer.normalized_depth(&deep);

        assert!(norm_shallow < 0.2);
        assert!(norm_deep > 0.5);
        assert!(norm_shallow <= 1.0);
        assert!(norm_deep <= 1.0);
    }
}
