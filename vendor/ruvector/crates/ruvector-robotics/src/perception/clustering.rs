//! Shared spatial-hash clustering with union-find.
//!
//! Used by the obstacle detector, scene graph builder, and perception pipeline
//! to avoid duplicating the same algorithm.
//!
//! ## Optimizations
//!
//! - Union-by-rank prevents tree degeneration, keeping `find` near O(α(n)).
//! - Path halving in `find` for efficient path compression.
//! - `#[inline]` on hot helpers to ensure inlining in tight loops.

use crate::bridge::{Point3D, PointCloud};
use std::collections::HashMap;

/// Cluster a point cloud using spatial hashing and union-find over adjacent cells.
///
/// Points are binned into a 3-D grid with the given `cell_size`. Cells that
/// share a face, edge, or corner (26-neighbourhood) are merged via union-find.
/// Returns the resulting groups as separate point vectors.
pub fn cluster_point_cloud(cloud: &PointCloud, cell_size: f64) -> Vec<Vec<Point3D>> {
    if cloud.points.is_empty() || cell_size <= 0.0 {
        return Vec::new();
    }

    // 1. Map each point to a grid cell.
    let mut cell_map: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (idx, p) in cloud.points.iter().enumerate() {
        let key = cell_key(p, cell_size);
        cell_map.entry(key).or_default().push(idx);
    }

    // 2. Build union-find over cells (with rank for balanced merges).
    let cells: Vec<(i64, i64, i64)> = cell_map.keys().copied().collect();
    let cell_count = cells.len();
    let cell_idx: HashMap<(i64, i64, i64), usize> = cells
        .iter()
        .enumerate()
        .map(|(i, &k)| (k, i))
        .collect();

    let mut parent: Vec<usize> = (0..cell_count).collect();
    let mut rank: Vec<u8> = vec![0; cell_count];

    for &(cx, cy, cz) in &cells {
        let a = cell_idx[&(cx, cy, cz)];
        for dx in -1..=1_i64 {
            for dy in -1..=1_i64 {
                for dz in -1..=1_i64 {
                    let neighbor = (cx + dx, cy + dy, cz + dz);
                    if let Some(&b) = cell_idx.get(&neighbor) {
                        uf_union(&mut parent, &mut rank, a, b);
                    }
                }
            }
        }
    }

    // 3. Group points by their root representative.
    let mut groups: HashMap<usize, Vec<Point3D>> = HashMap::new();
    for (key, point_indices) in &cell_map {
        let ci = cell_idx[key];
        let root = uf_find(&mut parent, ci);
        let entry = groups.entry(root).or_default();
        for &pi in point_indices {
            entry.push(cloud.points[pi]);
        }
    }

    groups.into_values().collect()
}

/// Compute the grid cell key for a point.
#[inline]
fn cell_key(p: &Point3D, cell_size: f64) -> (i64, i64, i64) {
    (
        (p.x as f64 / cell_size).floor() as i64,
        (p.y as f64 / cell_size).floor() as i64,
        (p.z as f64 / cell_size).floor() as i64,
    )
}

/// Path-compressing find (path halving).
#[inline]
fn uf_find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Union by rank: attaches the shorter tree under the taller root.
#[inline]
fn uf_union(parent: &mut [usize], rank: &mut [u8], a: usize, b: usize) {
    let ra = uf_find(parent, a);
    let rb = uf_find(parent, b);
    if ra != rb {
        match rank[ra].cmp(&rank[rb]) {
            std::cmp::Ordering::Less => parent[ra] = rb,
            std::cmp::Ordering::Greater => parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                parent[rb] = ra;
                rank[ra] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud(pts: &[[f32; 3]]) -> PointCloud {
        let points: Vec<Point3D> = pts.iter().map(|a| Point3D::new(a[0], a[1], a[2])).collect();
        PointCloud::new(points, 0)
    }

    #[test]
    fn test_empty_cloud() {
        let cloud = PointCloud::default();
        let clusters = cluster_point_cloud(&cloud, 1.0);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_single_cluster() {
        let cloud = make_cloud(&[
            [1.0, 1.0, 0.0],
            [1.1, 1.0, 0.0],
            [1.0, 1.1, 0.0],
        ]);
        let clusters = cluster_point_cloud(&cloud, 0.5);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].len(), 3);
    }

    #[test]
    fn test_two_clusters() {
        let cloud = make_cloud(&[
            [0.0, 0.0, 0.0],
            [0.1, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [10.1, 10.0, 0.0],
        ]);
        let clusters = cluster_point_cloud(&cloud, 0.5);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn test_negative_coordinates() {
        let cloud = make_cloud(&[
            [-1.0, -1.0, 0.0],
            [-0.9, -1.0, 0.0],
            [1.0, 1.0, 0.0],
        ]);
        let clusters = cluster_point_cloud(&cloud, 0.5);
        assert_eq!(clusters.len(), 2);
    }
}
