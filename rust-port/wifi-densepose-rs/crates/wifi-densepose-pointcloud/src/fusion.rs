//! Multi-modal fusion: camera depth + WiFi RF tomography → unified point cloud.

use crate::pointcloud::{PointCloud, ColorPoint};
use std::collections::HashMap;

/// Occupancy volume from WiFi RF tomography (mirrors RuView's OccupancyVolume).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OccupancyVolume {
    pub densities: Vec<f64>,   // [nz][ny][nx] voxel densities
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub bounds: [f64; 6],      // [x_min, y_min, z_min, x_max, y_max, z_max]
    pub occupied_count: usize,
}

/// Convert WiFi occupancy volume to a sparse point cloud.
///
/// Each occupied voxel (density > threshold) becomes a point at the voxel center.
pub fn occupancy_to_pointcloud(vol: &OccupancyVolume) -> PointCloud {
    let mut cloud = PointCloud::new("wifi_occupancy");
    let threshold = 0.3;

    let dx = (vol.bounds[3] - vol.bounds[0]) / vol.nx as f64;
    let dy = (vol.bounds[4] - vol.bounds[1]) / vol.ny as f64;
    let dz = (vol.bounds[5] - vol.bounds[2]) / vol.nz as f64;

    for iz in 0..vol.nz {
        for iy in 0..vol.ny {
            for ix in 0..vol.nx {
                let idx = iz * vol.ny * vol.nx + iy * vol.nx + ix;
                let density = vol.densities[idx];
                if density > threshold {
                    let x = vol.bounds[0] + (ix as f64 + 0.5) * dx;
                    let y = vol.bounds[1] + (iy as f64 + 0.5) * dy;
                    let z = vol.bounds[2] + (iz as f64 + 0.5) * dz;

                    // Color by density (green=low, red=high)
                    let t = ((density - threshold) / (1.0 - threshold)).min(1.0);
                    let r = (t * 255.0) as u8;
                    let g = ((1.0 - t) * 200.0) as u8;

                    cloud.points.push(ColorPoint {
                        x: x as f32,
                        y: y as f32,
                        z: z as f32,
                        r, g, b: 50,
                        intensity: density as f32,
                    });
                }
            }
        }
    }
    cloud
}

/// Fuse multiple point clouds with voxel-grid downsampling.
///
/// Points from all clouds are binned into voxels of the given size.
/// Each voxel produces one averaged point (position, color, max intensity).
pub fn fuse_clouds(clouds: &[&PointCloud], voxel_size: f32) -> PointCloud {
    let mut cells: HashMap<(i32, i32, i32), (f32, f32, f32, f32, f32, f32, f32, u32)> = HashMap::new();
    // (sum_x, sum_y, sum_z, sum_r, sum_g, sum_b, max_intensity, count)

    for cloud in clouds {
        for p in &cloud.points {
            let key = (
                (p.x / voxel_size).floor() as i32,
                (p.y / voxel_size).floor() as i32,
                (p.z / voxel_size).floor() as i32,
            );
            let entry = cells.entry(key).or_insert((0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0));
            entry.0 += p.x;
            entry.1 += p.y;
            entry.2 += p.z;
            entry.3 += p.r as f32;
            entry.4 += p.g as f32;
            entry.5 += p.b as f32;
            entry.6 = entry.6.max(p.intensity);
            entry.7 += 1;
        }
    }

    let mut fused = PointCloud::new("fused");
    for (_, (sx, sy, sz, sr, sg, sb, mi, n)) in &cells {
        let n = *n as f32;
        fused.points.push(ColorPoint {
            x: sx / n, y: sy / n, z: sz / n,
            r: (sr / n) as u8, g: (sg / n) as u8, b: (sb / n) as u8,
            intensity: *mi,
        });
    }
    fused
}

/// Generate a demo occupancy volume (room with person).
pub fn demo_occupancy() -> OccupancyVolume {
    let nx = 10;
    let ny = 10;
    let nz = 5;
    let mut densities = vec![0.0f64; nx * ny * nz];

    // Walls (high density at edges)
    for iz in 0..nz {
        for iy in 0..ny {
            for ix in 0..nx {
                let idx = iz * ny * nx + iy * nx + ix;
                // Edges = walls
                if ix == 0 || ix == nx - 1 || iy == 0 || iy == ny - 1 {
                    densities[idx] = 0.8;
                }
                // Floor
                if iz == 0 {
                    densities[idx] = 0.6;
                }
                // Person at center (iz=1-3, ix=4-6, iy=4-6)
                if (4..=6).contains(&ix) && (4..=6).contains(&iy) && (1..=3).contains(&iz) {
                    densities[idx] = 0.9;
                }
            }
        }
    }

    let occupied_count = densities.iter().filter(|&&d| d > 0.3).count();
    OccupancyVolume {
        densities, nx, ny, nz,
        bounds: [0.0, 0.0, 0.0, 5.0, 5.0, 3.0],
        occupied_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloud_with(name: &str, pts: &[(f32, f32, f32)]) -> PointCloud {
        let mut c = PointCloud::new(name);
        for &(x, y, z) in pts {
            c.points.push(ColorPoint { x, y, z, r: 10, g: 20, b: 30, intensity: 0.5 });
        }
        c
    }

    #[test]
    fn fuse_clouds_merges_non_overlapping() {
        let a = cloud_with("a", &[(0.0, 0.0, 0.0)]);
        let b = cloud_with("b", &[(5.0, 5.0, 5.0)]);
        let fused = fuse_clouds(&[&a, &b], 0.1);
        assert_eq!(fused.points.len(), 2, "two far-apart points should yield two voxels");
    }

    #[test]
    fn fuse_clouds_voxel_dedup() {
        // Points all within one voxel must collapse to a single averaged point.
        let a = cloud_with("a", &[
            (0.01, 0.02, 0.03),
            (0.04, 0.01, 0.02),
            (0.03, 0.03, 0.01),
        ]);
        let fused = fuse_clouds(&[&a], 0.5);
        assert_eq!(fused.points.len(), 1, "three close points → one voxel");
    }
}
