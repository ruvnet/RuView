//! Multi-sensor point cloud fusion.
//!
//! Aligns and merges point clouds from multiple sensors into a single
//! unified cloud, using nearest-timestamp matching and optional confidence
//! weighting.

use crate::bridge::{Point3D, PointCloud};

use serde::{Deserialize, Serialize};

/// Configuration for the sensor fusion module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    /// Maximum timestamp delta (µs) for two frames to be considered
    /// synchronised.  Frames further apart are discarded.
    pub max_time_delta_us: i64,
    /// Whether to apply confidence weighting based on point density.
    pub density_weighting: bool,
    /// Minimum voxel size for down-sampling the fused cloud.  Set to 0.0
    /// to disable.
    pub voxel_size: f64,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            max_time_delta_us: 50_000, // 50 ms
            density_weighting: false,
            voxel_size: 0.0,
        }
    }
}

/// Fuse multiple point clouds into a single unified cloud.
///
/// Clouds whose timestamps are further than `config.max_time_delta_us` from
/// the *reference* (first cloud in the slice) are skipped.  When
/// `voxel_size > 0`, the merged cloud is down-sampled via voxel-grid
/// filtering.
pub fn fuse_clouds(clouds: &[PointCloud], config: &FusionConfig) -> PointCloud {
    if clouds.is_empty() {
        return PointCloud::default();
    }

    let reference_ts = clouds[0].timestamp_us;

    // Pre-allocate merged vectors based on total point count of eligible clouds.
    let total_cap: usize = clouds
        .iter()
        .filter(|c| (c.timestamp_us - reference_ts).abs() <= config.max_time_delta_us)
        .map(|c| c.points.len())
        .sum();
    let mut merged_points: Vec<Point3D> = Vec::with_capacity(total_cap);
    let mut merged_intensities: Vec<f32> = Vec::with_capacity(total_cap);

    for cloud in clouds {
        let dt = (cloud.timestamp_us - reference_ts).abs();
        if dt > config.max_time_delta_us {
            continue;
        }

        merged_points.extend_from_slice(&cloud.points);

        if config.density_weighting && !cloud.is_empty() {
            let weight = 1.0 / (cloud.points.len() as f32).sqrt();
            merged_intensities.extend(cloud.intensities.iter().map(|i| i * weight));
        } else {
            merged_intensities.extend_from_slice(&cloud.intensities);
        }
    }

    if config.voxel_size > 0.0 && !merged_points.is_empty() {
        let (dp, di) = voxel_downsample(&merged_points, &merged_intensities, config.voxel_size);
        merged_points = dp;
        merged_intensities = di;
    }

    let mut result = PointCloud::new(merged_points, reference_ts);
    result.intensities = merged_intensities;
    result
}

/// Simple voxel grid down-sampling: keep one representative point per voxel.
fn voxel_downsample(
    points: &[Point3D],
    intensities: &[f32],
    cell_size: f64,
) -> (Vec<Point3D>, Vec<f32>) {
    use std::collections::HashMap;

    let mut voxels: HashMap<(i64, i64, i64), (Point3D, f32, usize)> = HashMap::new();

    for (i, p) in points.iter().enumerate() {
        let key = (
            (p.x as f64 / cell_size).floor() as i64,
            (p.y as f64 / cell_size).floor() as i64,
            (p.z as f64 / cell_size).floor() as i64,
        );
        let intensity = intensities.get(i).copied().unwrap_or(1.0);
        let entry = voxels.entry(key).or_insert((*p, intensity, 0));
        entry.2 += 1;
        // Running average position.
        let n = entry.2 as f32;
        entry.0.x = entry.0.x + (p.x - entry.0.x) / n;
        entry.0.y = entry.0.y + (p.y - entry.0.y) / n;
        entry.0.z = entry.0.z + (p.z - entry.0.z) / n;
        entry.1 = entry.1 + (intensity - entry.1) / n;
    }

    let mut out_pts = Vec::with_capacity(voxels.len());
    let mut out_int = Vec::with_capacity(voxels.len());
    for (pt, inten, _) in voxels.into_values() {
        out_pts.push(pt);
        out_int.push(inten);
    }
    (out_pts, out_int)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud(pts: &[[f32; 3]], ts: i64) -> PointCloud {
        let points: Vec<Point3D> = pts.iter().map(|a| Point3D::new(a[0], a[1], a[2])).collect();
        PointCloud::new(points, ts)
    }

    #[test]
    fn test_fuse_empty() {
        let result = fuse_clouds(&[], &FusionConfig::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_fuse_single() {
        let c = make_cloud(&[[1.0, 0.0, 0.0]], 1000);
        let result = fuse_clouds(&[c], &FusionConfig::default());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_fuse_two_clouds() {
        let c1 = make_cloud(&[[1.0, 0.0, 0.0]], 1000);
        let c2 = make_cloud(&[[2.0, 0.0, 0.0]], 1010);
        let result = fuse_clouds(&[c1, c2], &FusionConfig::default());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_fuse_skips_stale() {
        let c1 = make_cloud(&[[1.0, 0.0, 0.0]], 0);
        let c2 = make_cloud(&[[2.0, 0.0, 0.0]], 100_000); // 100ms apart
        let config = FusionConfig { max_time_delta_us: 50_000, ..Default::default() };
        let result = fuse_clouds(&[c1, c2], &config);
        assert_eq!(result.len(), 1); // c2 skipped
    }

    #[test]
    fn test_voxel_downsample() {
        let c1 = make_cloud(
            &[
                [0.0, 0.0, 0.0], [0.01, 0.01, 0.01], // same voxel
                [5.0, 5.0, 5.0], // different voxel
            ],
            0,
        );
        let config = FusionConfig { voxel_size: 1.0, ..Default::default() };
        let result = fuse_clouds(&[c1], &config);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_density_weighting() {
        let c1 = make_cloud(&[[1.0, 0.0, 0.0]], 0);
        let config = FusionConfig { density_weighting: true, ..Default::default() };
        let result = fuse_clouds(&[c1], &config);
        assert_eq!(result.len(), 1);
        // With 1 point, weight = 1/sqrt(1) = 1.0, so intensity unchanged.
        assert!((result.intensities[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_fuse_preserves_timestamp() {
        let c1 = make_cloud(&[[1.0, 0.0, 0.0]], 5000);
        let c2 = make_cloud(&[[2.0, 0.0, 0.0]], 5010);
        let result = fuse_clouds(&[c1, c2], &FusionConfig::default());
        assert_eq!(result.timestamp_us, 5000);
    }
}
