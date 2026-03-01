//! Gaussian splatting types and point-cloud-to-Gaussian conversion.
//!
//! Provides a [`GaussianSplat`] representation that maps each point cloud
//! cluster to a 3D Gaussian with position, colour, opacity, scale, and
//! optional temporal trajectory.  The serialised format is compatible with
//! the `vwm-viewer` Canvas2D renderer.

use crate::bridge::{Point3D, PointCloud};
use crate::perception::clustering;

use serde::{Deserialize, Serialize};

/// A single 3-D Gaussian suitable for splatting-based rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GaussianSplat {
    /// Centre of the Gaussian in world coordinates.
    pub center: [f64; 3],
    /// RGB colour in \[0, 1\].
    pub color: [f32; 3],
    /// Opacity in \[0, 1\].
    pub opacity: f32,
    /// Anisotropic scale along each axis.
    pub scale: [f32; 3],
    /// Number of raw points that contributed to this Gaussian.
    pub point_count: usize,
    /// Semantic label (e.g. `"obstacle"`, `"ground"`).
    pub label: String,
    /// Temporal trajectory: each entry is a position at a successive timestep.
    /// Empty for static Gaussians.
    pub trajectory: Vec<[f64; 3]>,
}

/// A collection of Gaussians derived from one or more point cloud frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaussianSplatCloud {
    pub gaussians: Vec<GaussianSplat>,
    pub timestamp_us: i64,
    pub frame_id: String,
}

impl GaussianSplatCloud {
    /// Number of Gaussians.
    pub fn len(&self) -> usize {
        self.gaussians.len()
    }

    pub fn is_empty(&self) -> bool {
        self.gaussians.is_empty()
    }
}

/// Configuration for point-cloud → Gaussian conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaussianConfig {
    /// Clustering cell size in metres.  Smaller = more Gaussians.
    pub cell_size: f64,
    /// Minimum number of points to form a Gaussian.
    pub min_cluster_size: usize,
    /// Default colour for unlabelled Gaussians `[R, G, B]`.
    pub default_color: [f32; 3],
    /// Base opacity for generated Gaussians.
    pub base_opacity: f32,
}

impl Default for GaussianConfig {
    fn default() -> Self {
        Self {
            cell_size: 0.5,
            min_cluster_size: 2,
            default_color: [0.3, 0.5, 0.8],
            base_opacity: 0.7,
        }
    }
}

/// Convert a [`PointCloud`] into a [`GaussianSplatCloud`] by clustering nearby
/// points and computing per-cluster statistics.
pub fn gaussians_from_cloud(
    cloud: &PointCloud,
    config: &GaussianConfig,
) -> GaussianSplatCloud {
    if cloud.is_empty() || config.cell_size <= 0.0 {
        return GaussianSplatCloud {
            gaussians: Vec::new(),
            timestamp_us: cloud.timestamp_us,
            frame_id: cloud.frame_id.clone(),
        };
    }

    let clusters = clustering::cluster_point_cloud(cloud, config.cell_size);

    let gaussians: Vec<GaussianSplat> = clusters
        .into_iter()
        .filter(|c| c.len() >= config.min_cluster_size)
        .map(|pts| cluster_to_gaussian(&pts, config))
        .collect();

    GaussianSplatCloud {
        gaussians,
        timestamp_us: cloud.timestamp_us,
        frame_id: cloud.frame_id.clone(),
    }
}

fn cluster_to_gaussian(points: &[Point3D], config: &GaussianConfig) -> GaussianSplat {
    let n = points.len() as f64;
    let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
    for p in points {
        sx += p.x as f64;
        sy += p.y as f64;
        sz += p.z as f64;
    }
    let center = [sx / n, sy / n, sz / n];

    // Compute per-axis standard deviation as the scale.
    let (mut vx, mut vy, mut vz) = (0.0_f64, 0.0_f64, 0.0_f64);
    for p in points {
        let dx = p.x as f64 - center[0];
        let dy = p.y as f64 - center[1];
        let dz = p.z as f64 - center[2];
        vx += dx * dx;
        vy += dy * dy;
        vz += dz * dz;
    }
    let scale = [
        (vx / n).sqrt().max(0.01) as f32,
        (vy / n).sqrt().max(0.01) as f32,
        (vz / n).sqrt().max(0.01) as f32,
    ];

    // Opacity proportional to cluster density.
    let opacity = (config.base_opacity * (points.len() as f32 / 50.0).min(1.0)).max(0.1);

    GaussianSplat {
        center,
        color: config.default_color,
        opacity,
        scale,
        point_count: points.len(),
        label: String::new(),
        trajectory: Vec::new(),
    }
}

/// Serialise a [`GaussianSplatCloud`] to the JSON format expected by the
/// `vwm-viewer` Canvas2D renderer.
pub fn to_viewer_json(cloud: &GaussianSplatCloud) -> serde_json::Value {
    let gs: Vec<serde_json::Value> = cloud
        .gaussians
        .iter()
        .map(|g| {
            let positions: Vec<Vec<f64>> = if g.trajectory.is_empty() {
                vec![g.center.to_vec()]
            } else {
                g.trajectory.iter().map(|p| p.to_vec()).collect()
            };
            serde_json::json!({
                "positions": positions,
                "color": g.color,
                "opacity": g.opacity,
                "scale": g.scale,
                "label": g.label,
                "point_count": g.point_count,
            })
        })
        .collect();

    serde_json::json!({
        "gaussians": gs,
        "timestamp_us": cloud.timestamp_us,
        "frame_id": cloud.frame_id,
        "count": cloud.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud(pts: &[[f32; 3]], ts: i64) -> PointCloud {
        let points: Vec<Point3D> = pts.iter().map(|a| Point3D::new(a[0], a[1], a[2])).collect();
        PointCloud::new(points, ts)
    }

    #[test]
    fn test_empty_cloud() {
        let cloud = PointCloud::default();
        let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
        assert!(gs.is_empty());
    }

    #[test]
    fn test_single_cluster() {
        let cloud = make_cloud(
            &[[1.0, 0.0, 0.0], [1.1, 0.0, 0.0], [1.0, 0.1, 0.0]],
            1000,
        );
        let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
        assert_eq!(gs.len(), 1);
        let g = &gs.gaussians[0];
        assert_eq!(g.point_count, 3);
        assert!(g.center[0] > 0.9 && g.center[0] < 1.2);
    }

    #[test]
    fn test_two_clusters() {
        let cloud = make_cloud(
            &[
                [0.0, 0.0, 0.0], [0.1, 0.0, 0.0],
                [10.0, 10.0, 0.0], [10.1, 10.0, 0.0],
            ],
            2000,
        );
        let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
        assert_eq!(gs.len(), 2);
    }

    #[test]
    fn test_min_cluster_size_filtering() {
        let cloud = make_cloud(
            &[[0.0, 0.0, 0.0], [10.0, 10.0, 0.0]],
            0,
        );
        let config = GaussianConfig { min_cluster_size: 3, ..Default::default() };
        let gs = gaussians_from_cloud(&cloud, &config);
        assert!(gs.is_empty());
    }

    #[test]
    fn test_scale_reflects_spread() {
        // Use a larger cell size so all three points end up in one cluster.
        let cloud = make_cloud(
            &[[0.0, 0.0, 0.0], [0.3, 0.0, 0.0], [0.15, 0.0, 0.0]],
            0,
        );
        let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
        assert_eq!(gs.len(), 1);
        let g = &gs.gaussians[0];
        // X-axis spread > Y/Z spread (Y/Z should be clamped minimum 0.01).
        assert!(g.scale[0] > g.scale[1]);
    }

    #[test]
    fn test_viewer_json_format() {
        let cloud = make_cloud(&[[1.0, 2.0, 3.0], [1.1, 2.0, 3.0]], 5000);
        let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
        let json = to_viewer_json(&gs);
        assert_eq!(json["count"], 1);
        assert_eq!(json["timestamp_us"], 5000);
        let arr = json["gaussians"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0]["positions"].is_array());
        assert!(arr[0]["color"].is_array());
    }

    #[test]
    fn test_serde_roundtrip() {
        let cloud = make_cloud(&[[0.0, 0.0, 0.0], [0.1, 0.1, 0.0]], 0);
        let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
        let json = serde_json::to_string(&gs).unwrap();
        let restored: GaussianSplatCloud = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.len(), gs.len());
    }

    #[test]
    fn test_zero_cell_size() {
        let cloud = make_cloud(&[[1.0, 0.0, 0.0]], 0);
        let config = GaussianConfig { cell_size: 0.0, ..Default::default() };
        let gs = gaussians_from_cloud(&cloud, &config);
        assert!(gs.is_empty());
    }
}
