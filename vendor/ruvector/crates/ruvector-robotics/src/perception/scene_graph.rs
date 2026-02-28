//! Scene-graph construction from point clouds and object lists.
//!
//! The [`SceneGraphBuilder`] turns raw sensor data into a structured
//! [`SceneGraph`] of objects and spatial relationships.

use std::collections::HashMap;

use crate::bridge::{Point3D, PointCloud, SceneEdge, SceneGraph, SceneObject};
use crate::perception::clustering;
use crate::perception::config::SceneGraphConfig;

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builds [`SceneGraph`] instances from point clouds or pre-classified
/// object lists using spatial-hash clustering with union-find.
#[derive(Debug, Clone)]
pub struct PointCloudSceneGraphBuilder {
    config: SceneGraphConfig,
}

impl PointCloudSceneGraphBuilder {
    /// Create a new builder with the given configuration.
    pub fn new(config: SceneGraphConfig) -> Self {
        Self { config }
    }

    /// Build a scene graph by clustering a raw point cloud.
    ///
    /// 1. Points are discretised into a spatial hash grid.
    /// 2. Adjacent cells are merged via union-find.
    /// 3. Each cluster above `min_cluster_size` becomes a `SceneObject`.
    /// 4. Edges are created between objects whose centres are within
    ///    `edge_distance_threshold`.
    pub fn build_from_point_cloud(&self, cloud: &PointCloud) -> SceneGraph {
        if cloud.is_empty() {
            return SceneGraph::default();
        }

        let clusters = clustering::cluster_point_cloud(cloud, self.config.cluster_radius);

        // Convert clusters to SceneObjects (cap at max_objects).
        let mut objects: Vec<SceneObject> = clusters
            .into_iter()
            .filter(|pts| pts.len() >= self.config.min_cluster_size)
            .take(self.config.max_objects)
            .enumerate()
            .map(|(id, pts)| Self::cluster_to_object(id, &pts))
            .collect();

        objects.sort_by(|a, b| a.id.cmp(&b.id));
        let edges = self.create_edges(&objects);

        SceneGraph::new(objects, edges, cloud.timestamp_us)
    }

    /// Build a scene graph from a pre-existing list of objects.
    ///
    /// Edges are created between objects whose centres are within
    /// `edge_distance_threshold`.
    pub fn build_from_objects(&self, objects: &[SceneObject]) -> SceneGraph {
        let objects_vec: Vec<SceneObject> = objects
            .iter()
            .take(self.config.max_objects)
            .cloned()
            .collect();

        let edges = self.create_edges(&objects_vec);
        SceneGraph::new(objects_vec, edges, 0)
    }

    /// Merge multiple scene graphs into one, deduplicating objects that share
    /// the same `id`.
    pub fn merge_scenes(&self, scenes: &[SceneGraph]) -> SceneGraph {
        let mut seen_ids: HashMap<usize, SceneObject> = HashMap::new();
        let mut latest_ts: i64 = 0;

        for scene in scenes {
            latest_ts = latest_ts.max(scene.timestamp);
            for obj in &scene.objects {
                // Keep the first occurrence of each id.
                seen_ids.entry(obj.id).or_insert_with(|| obj.clone());
            }
        }

        let mut objects: Vec<SceneObject> = seen_ids.into_values().collect();
        objects.sort_by(|a, b| a.id.cmp(&b.id));

        let truncated: Vec<SceneObject> = objects
            .into_iter()
            .take(self.config.max_objects)
            .collect();

        let edges = self.create_edges(&truncated);
        SceneGraph::new(truncated, edges, latest_ts)
    }

    // -- private helpers ----------------------------------------------------

    fn cluster_to_object(id: usize, points: &[Point3D]) -> SceneObject {
        debug_assert!(!points.is_empty(), "cluster_to_object called with empty slice");
        let (mut min_x, mut min_y, mut min_z) = (f64::MAX, f64::MAX, f64::MAX);
        let (mut max_x, mut max_y, mut max_z) = (f64::MIN, f64::MIN, f64::MIN);
        let (mut sum_x, mut sum_y, mut sum_z) = (0.0_f64, 0.0_f64, 0.0_f64);

        for p in points {
            let (px, py, pz) = (p.x as f64, p.y as f64, p.z as f64);
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            min_z = min_z.min(pz);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
            max_z = max_z.max(pz);
            sum_x += px;
            sum_y += py;
            sum_z += pz;
        }

        let n = points.len() as f64;
        let center = [sum_x / n, sum_y / n, sum_z / n];
        let extent = [
            (max_x - min_x) / 2.0,
            (max_y - min_y) / 2.0,
            (max_z - min_z) / 2.0,
        ];

        SceneObject {
            id,
            center,
            extent,
            confidence: 1.0,
            label: format!("cluster_{}", id),
            velocity: None,
        }
    }

    fn create_edges(&self, objects: &[SceneObject]) -> Vec<SceneEdge> {
        let mut edges = Vec::new();
        let threshold = self.config.edge_distance_threshold;

        for i in 0..objects.len() {
            for j in (i + 1)..objects.len() {
                let d = Self::distance_3d(&objects[i].center, &objects[j].center);
                if d <= threshold {
                    let relation = if d < threshold * 0.33 {
                        "adjacent"
                    } else if d < threshold * 0.66 {
                        "near"
                    } else {
                        "far"
                    };
                    edges.push(SceneEdge {
                        from: objects[i].id,
                        to: objects[j].id,
                        distance: d,
                        relation: relation.to_string(),
                    });
                }
            }
        }

        edges
    }

    fn distance_3d(a: &[f64; 3], b: &[f64; 3]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud(raw: &[[f32; 3]]) -> PointCloud {
        let points: Vec<Point3D> = raw.iter().map(|p| Point3D::new(p[0], p[1], p[2])).collect();
        PointCloud::new(points, 1000)
    }

    #[test]
    fn test_empty_cloud() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig::default());
        let graph = builder.build_from_point_cloud(&PointCloud::default());
        assert!(graph.objects.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_single_cluster() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            cluster_radius: 1.0,
            min_cluster_size: 3,
            max_objects: 10,
            edge_distance_threshold: 5.0,
        });
        let cloud = make_cloud(&[
            [0.0, 0.0, 0.0],
            [0.1, 0.0, 0.0],
            [0.0, 0.1, 0.0],
            [0.1, 0.1, 0.0],
        ]);
        let graph = builder.build_from_point_cloud(&cloud);
        assert_eq!(graph.objects.len(), 1);
        assert!(graph.edges.is_empty()); // Only one object, no edges.
    }

    #[test]
    fn test_room_point_cloud() {
        // Simulate a room with two walls (clusters far apart).
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            cluster_radius: 0.5,
            min_cluster_size: 3,
            max_objects: 10,
            edge_distance_threshold: 50.0,
        });

        let mut points = Vec::new();
        // Wall 1: cluster around (0, 0, 0)
        for i in 0..5 {
            points.push([i as f32 * 0.1, 0.0, 0.0]);
        }
        // Wall 2: cluster around (10, 0, 0)
        for i in 0..5 {
            points.push([10.0 + i as f32 * 0.1, 0.0, 0.0]);
        }

        let cloud = make_cloud(&points);
        let graph = builder.build_from_point_cloud(&cloud);
        assert_eq!(graph.objects.len(), 2);
        // Both walls should be connected since threshold is 50.
        assert!(!graph.edges.is_empty());
    }

    #[test]
    fn test_separated_clusters_no_edge() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            cluster_radius: 0.5,
            min_cluster_size: 3,
            max_objects: 10,
            edge_distance_threshold: 2.0,
        });

        let cloud = make_cloud(&[
            // Cluster A
            [0.0, 0.0, 0.0],
            [0.1, 0.0, 0.0],
            [0.0, 0.1, 0.0],
            // Cluster B -- far away (100 units)
            [100.0, 0.0, 0.0],
            [100.1, 0.0, 0.0],
            [100.0, 0.1, 0.0],
        ]);

        let graph = builder.build_from_point_cloud(&cloud);
        assert_eq!(graph.objects.len(), 2);
        // Should NOT have edges -- clusters are 100 units apart, threshold is 2.
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_build_from_objects() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            edge_distance_threshold: 5.0,
            ..SceneGraphConfig::default()
        });

        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(1, [3.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(2, [100.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ];

        let graph = builder.build_from_objects(&objects);
        assert_eq!(graph.objects.len(), 3);

        // Objects 0 and 1 are 3.0 apart (within threshold),
        // object 2 is 100.0 away (outside threshold).
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, 0);
        assert_eq!(graph.edges[0].to, 1);
    }

    #[test]
    fn test_merge_deduplication() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            edge_distance_threshold: 10.0,
            ..SceneGraphConfig::default()
        });

        let scene_a = SceneGraph::new(
            vec![
                SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
                SceneObject::new(1, [2.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ],
            vec![],
            100,
        );

        let scene_b = SceneGraph::new(
            vec![
                SceneObject::new(1, [2.0, 0.0, 0.0], [1.0, 1.0, 1.0]), // duplicate id
                SceneObject::new(2, [4.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ],
            vec![],
            200,
        );

        let merged = builder.merge_scenes(&[scene_a, scene_b]);
        // Should have 3 unique objects: ids 0, 1, 2.
        assert_eq!(merged.objects.len(), 3);
        assert_eq!(merged.timestamp, 200);
    }

    #[test]
    fn test_merge_preserves_latest_timestamp() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig::default());
        let s1 = SceneGraph::new(vec![], vec![], 50);
        let s2 = SceneGraph::new(vec![], vec![], 300);
        let s3 = SceneGraph::new(vec![], vec![], 100);
        let merged = builder.merge_scenes(&[s1, s2, s3]);
        assert_eq!(merged.timestamp, 300);
    }

    #[test]
    fn test_edge_relations() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            edge_distance_threshold: 30.0,
            ..SceneGraphConfig::default()
        });

        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(1, [5.0, 0.0, 0.0], [1.0, 1.0, 1.0]),  // ~5 < 9.9 => adjacent
            SceneObject::new(2, [15.0, 0.0, 0.0], [1.0, 1.0, 1.0]), // ~15 < 19.8 => near
            SceneObject::new(3, [25.0, 0.0, 0.0], [1.0, 1.0, 1.0]), // ~25 < 30 => far
        ];

        let graph = builder.build_from_objects(&objects);

        // Check that adjacent relation exists for objects 0 and 1.
        let edge_0_1 = graph
            .edges
            .iter()
            .find(|e| e.from == 0 && e.to == 1);
        assert!(edge_0_1.is_some());
        assert_eq!(edge_0_1.unwrap().relation, "adjacent");
    }

    #[test]
    fn test_max_objects_cap() {
        let builder = PointCloudSceneGraphBuilder::new(SceneGraphConfig {
            cluster_radius: 0.5,
            min_cluster_size: 1,
            max_objects: 2,
            edge_distance_threshold: 100.0,
        });

        let cloud = make_cloud(&[
            [0.0, 0.0, 0.0],
            [50.0, 0.0, 0.0],
            [100.0, 0.0, 0.0],
        ]);

        let graph = builder.build_from_point_cloud(&cloud);
        // min_cluster_size=1, so each point is its own cluster.
        // max_objects=2, so at most 2 objects.
        assert!(graph.objects.len() <= 2);
    }
}
