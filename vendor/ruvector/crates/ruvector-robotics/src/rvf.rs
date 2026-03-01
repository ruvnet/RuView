//! RVF packaging for robotics data.
//!
//! Bridges the robotics crate with the [RuVector Format](crate) so that
//! point clouds, scene graphs, episodic memory, Gaussian splats, and
//! occupancy grids can be persisted, queried, and transferred as `.rvf`
//! files.
//!
//! Requires the `rvf` feature flag.
//!
//! # Quick Start
//!
//! ```ignore
//! use ruvector_robotics::rvf::RoboticsRvf;
//!
//! let mut rvf = RoboticsRvf::create("scene.rvf", 3)?;
//! rvf.pack_point_cloud(&cloud)?;
//! rvf.pack_scene_graph(&graph)?;
//! let similar = rvf.query_nearest(&[1.0, 2.0, 3.0], 5)?;
//! rvf.close()?;
//! ```

use std::path::Path;

use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{
    IngestResult, QueryOptions, RvfOptions, RvfStore, SearchResult,
};

use crate::bridge::{
    GaussianConfig, Obstacle, PointCloud, SceneGraph, SceneObject, Trajectory,
};
use crate::bridge::gaussian::{gaussians_from_cloud, GaussianSplatCloud};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from RVF packaging operations.
#[derive(Debug, thiserror::Error)]
pub enum RvfPackError {
    #[error("rvf store error: {0}")]
    Store(String),
    #[error("empty data: {0}")]
    EmptyData(&'static str),
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<rvf_types::RvfError> for RvfPackError {
    fn from(e: rvf_types::RvfError) -> Self {
        RvfPackError::Store(format!("{e:?}"))
    }
}

pub type Result<T> = std::result::Result<T, RvfPackError>;

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

// ID generation is handled by the `next_id` counter in `RoboticsRvf`.

// ---------------------------------------------------------------------------
// RoboticsRvf
// ---------------------------------------------------------------------------

/// High-level wrapper that packages robotics data into an RVF file.
///
/// Each robotics type is mapped to a vector encoding:
///
/// | Type | Dimension | Encoding |
/// |------|-----------|----------|
/// | Point cloud | 3 per point | `[x, y, z]` |
/// | Scene object | 9 | `[cx, cy, cz, ex, ey, ez, conf, vx, vy]` |
/// | Trajectory waypoint | 3 per step | `[x, y, z]` |
/// | Gaussian splat | 7 | `[cx, cy, cz, r, g, b, opacity]` |
/// | Obstacle | 6 | `[px, py, pz, dist, radius, conf]` |
pub struct RoboticsRvf {
    store: RvfStore,
    dimension: u16,
    next_id: u64,
}

impl RoboticsRvf {
    /// Create a new `.rvf` file at `path` for robotics vector data.
    ///
    /// `dimension` is the per-vector size (e.g. 3 for raw point clouds,
    /// 7 for Gaussian splats, 9 for scene objects).
    pub fn create<P: AsRef<Path>>(path: P, dimension: u16) -> Result<Self> {
        let options = RvfOptions {
            dimension,
            metric: DistanceMetric::L2,
            ..Default::default()
        };
        let store = RvfStore::create(path.as_ref(), options)?;
        Ok(Self { store, dimension, next_id: 1 })
    }

    /// Open an existing `.rvf` file for read-write access.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let store = RvfStore::open(path.as_ref())?;
        let dim = store.dimension();
        Ok(Self { store, dimension: dim, next_id: 1_000_000 })
    }

    /// Open an existing `.rvf` file for read-only queries.
    pub fn open_readonly<P: AsRef<Path>>(path: P) -> Result<Self> {
        let store = RvfStore::open_readonly(path.as_ref())?;
        let dim = store.dimension();
        Ok(Self { store, dimension: dim, next_id: 0 })
    }

    /// Current store status.
    pub fn status(&self) -> rvf_runtime::StoreStatus {
        self.store.status()
    }

    /// The vector dimension this store was created with.
    pub fn dimension(&self) -> u16 {
        self.dimension
    }

    // -- packing ----------------------------------------------------------

    /// Pack a [`PointCloud`] into the RVF store (dimension must be 3).
    pub fn pack_point_cloud(&mut self, cloud: &PointCloud) -> Result<IngestResult> {
        self.check_dim(3)?;
        if cloud.is_empty() {
            return Err(RvfPackError::EmptyData("point cloud is empty"));
        }

        let vectors: Vec<Vec<f32>> = cloud
            .points
            .iter()
            .map(|p| vec![p.x, p.y, p.z])
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..cloud.len())
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
            .collect();

        Ok(self.store.ingest_batch(&refs, &ids, None)?)
    }

    /// Pack scene objects into the RVF store (dimension must be 9).
    ///
    /// Each object is encoded as `[cx, cy, cz, ex, ey, ez, conf, vx, vy]`.
    pub fn pack_scene_objects(&mut self, objects: &[SceneObject]) -> Result<IngestResult> {
        self.check_dim(9)?;
        if objects.is_empty() {
            return Err(RvfPackError::EmptyData("no scene objects"));
        }

        let vectors: Vec<Vec<f32>> = objects
            .iter()
            .map(|o| {
                let vel = o.velocity.unwrap_or([0.0; 3]);
                vec![
                    o.center[0] as f32,
                    o.center[1] as f32,
                    o.center[2] as f32,
                    o.extent[0] as f32,
                    o.extent[1] as f32,
                    o.extent[2] as f32,
                    o.confidence,
                    vel[0] as f32,
                    vel[1] as f32,
                ]
            })
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..objects.len())
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
            .collect();

        Ok(self.store.ingest_batch(&refs, &ids, None)?)
    }

    /// Pack a scene graph (objects only) into the RVF store (dimension 9).
    pub fn pack_scene_graph(&mut self, graph: &SceneGraph) -> Result<IngestResult> {
        self.pack_scene_objects(&graph.objects)
    }

    /// Pack trajectory waypoints (dimension must be 3).
    pub fn pack_trajectory(&mut self, trajectory: &Trajectory) -> Result<IngestResult> {
        self.check_dim(3)?;
        if trajectory.is_empty() {
            return Err(RvfPackError::EmptyData("trajectory is empty"));
        }

        let vectors: Vec<Vec<f32>> = trajectory
            .waypoints
            .iter()
            .map(|wp| vec![wp[0] as f32, wp[1] as f32, wp[2] as f32])
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..trajectory.len())
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
            .collect();

        Ok(self.store.ingest_batch(&refs, &ids, None)?)
    }

    /// Convert a point cloud to Gaussian splats and pack them (dimension 7).
    ///
    /// Each Gaussian is encoded as `[cx, cy, cz, r, g, b, opacity]`.
    pub fn pack_gaussians(
        &mut self,
        cloud: &PointCloud,
        config: &GaussianConfig,
    ) -> Result<(GaussianSplatCloud, IngestResult)> {
        self.check_dim(7)?;
        let splat_cloud = gaussians_from_cloud(cloud, config);
        if splat_cloud.is_empty() {
            return Err(RvfPackError::EmptyData("no Gaussian splats produced"));
        }

        let vectors: Vec<Vec<f32>> = splat_cloud
            .gaussians
            .iter()
            .map(|g| {
                vec![
                    g.center[0] as f32,
                    g.center[1] as f32,
                    g.center[2] as f32,
                    g.color[0],
                    g.color[1],
                    g.color[2],
                    g.opacity,
                ]
            })
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..splat_cloud.gaussians.len())
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
            .collect();

        let result = self.store.ingest_batch(&refs, &ids, None)?;
        Ok((splat_cloud, result))
    }

    /// Pack obstacles into the RVF store (dimension must be 6).
    ///
    /// Each obstacle is encoded as `[px, py, pz, distance, radius, confidence]`.
    pub fn pack_obstacles(&mut self, obstacles: &[Obstacle]) -> Result<IngestResult> {
        self.check_dim(6)?;
        if obstacles.is_empty() {
            return Err(RvfPackError::EmptyData("no obstacles"));
        }

        let vectors: Vec<Vec<f32>> = obstacles
            .iter()
            .map(|o| {
                vec![
                    o.position[0] as f32,
                    o.position[1] as f32,
                    o.position[2] as f32,
                    o.distance as f32,
                    o.radius as f32,
                    o.confidence,
                ]
            })
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..obstacles.len())
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
            .collect();

        Ok(self.store.ingest_batch(&refs, &ids, None)?)
    }

    // -- querying ---------------------------------------------------------

    /// Query the store for the `k` nearest vectors to `query`.
    pub fn query_nearest(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        if query.len() != self.dimension as usize {
            return Err(RvfPackError::DimensionMismatch {
                expected: self.dimension as usize,
                got: query.len(),
            });
        }
        Ok(self.store.query(query, k, &QueryOptions::default())?)
    }

    // -- lifecycle --------------------------------------------------------

    /// Compact the store to reclaim dead space.
    pub fn compact(&mut self) -> Result<rvf_runtime::CompactionResult> {
        Ok(self.store.compact()?)
    }

    /// Close the store, flushing all data.
    pub fn close(self) -> Result<()> {
        Ok(self.store.close()?)
    }

    // -- internals --------------------------------------------------------

    fn check_dim(&self, required: u16) -> Result<()> {
        if self.dimension != required {
            return Err(RvfPackError::DimensionMismatch {
                expected: required as usize,
                got: self.dimension as usize,
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Point3D;
    use tempfile::NamedTempFile;

    fn tmp_path() -> std::path::PathBuf {
        let f = NamedTempFile::new().unwrap();
        let p = f.path().with_extension("rvf");
        drop(f);
        p
    }

    #[test]
    fn test_pack_point_cloud_and_query() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 3).unwrap();
        assert_eq!(rvf.dimension(), 3);

        let cloud = PointCloud::new(
            vec![
                Point3D::new(1.0, 0.0, 0.0),
                Point3D::new(2.0, 0.0, 0.0),
                Point3D::new(10.0, 0.0, 0.0),
            ],
            1000,
        );
        let result = rvf.pack_point_cloud(&cloud).unwrap();
        assert_eq!(result.accepted, 3);

        let hits = rvf.query_nearest(&[1.5, 0.0, 0.0], 2).unwrap();
        assert_eq!(hits.len(), 2);
        // Nearest should be one of the first two points.
        assert!(hits[0].distance < 1.0);

        rvf.close().unwrap();
        // Verify file was created.
        assert!(path.exists());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_pack_scene_objects() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 9).unwrap();

        let objects = vec![
            SceneObject::new(0, [1.0, 2.0, 0.0], [0.5, 0.5, 1.8]),
            SceneObject::new(1, [5.0, 0.0, 0.0], [1.0, 1.0, 2.0]),
        ];
        let result = rvf.pack_scene_objects(&objects).unwrap();
        assert_eq!(result.accepted, 2);

        let hits = rvf.query_nearest(&[1.0, 2.0, 0.0, 0.5, 0.5, 1.8, 1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(hits.len(), 1);

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_pack_trajectory() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 3).unwrap();

        let traj = Trajectory::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            vec![100, 200, 300],
            0.95,
        );
        let result = rvf.pack_trajectory(&traj).unwrap();
        assert_eq!(result.accepted, 3);

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_pack_gaussians() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 7).unwrap();

        let cloud = PointCloud::new(
            vec![
                Point3D::new(1.0, 0.0, 0.0),
                Point3D::new(1.1, 0.0, 0.0),
                Point3D::new(1.0, 0.1, 0.0),
                Point3D::new(5.0, 5.0, 0.0),
                Point3D::new(5.1, 5.0, 0.0),
                Point3D::new(5.0, 5.1, 0.0),
            ],
            1000,
        );
        let config = GaussianConfig { min_cluster_size: 3, ..Default::default() };
        let (splat_cloud, result) = rvf.pack_gaussians(&cloud, &config).unwrap();
        assert!(!splat_cloud.is_empty());
        assert!(result.accepted > 0);

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_pack_obstacles() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 6).unwrap();

        let obstacles = vec![
            Obstacle {
                id: 0,
                position: [2.0, 0.0, 0.0],
                distance: 2.0,
                radius: 0.5,
                label: "person".into(),
                confidence: 0.9,
            },
        ];
        let result = rvf.pack_obstacles(&obstacles).unwrap();
        assert_eq!(result.accepted, 1);

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_dimension_mismatch() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 3).unwrap();

        // Trying to pack scene objects (dim 9) into a dim-3 store.
        let objects = vec![SceneObject::new(0, [1.0, 0.0, 0.0], [0.5, 0.5, 0.5])];
        let result = rvf.pack_scene_objects(&objects);
        assert!(result.is_err());

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_empty_data_rejected() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 3).unwrap();

        let empty_cloud = PointCloud::default();
        assert!(rvf.pack_point_cloud(&empty_cloud).is_err());

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_open_and_requery() {
        let path = tmp_path();
        {
            let mut rvf = RoboticsRvf::create(&path, 3).unwrap();
            let cloud = PointCloud::new(
                vec![Point3D::new(1.0, 0.0, 0.0), Point3D::new(2.0, 0.0, 0.0)],
                1000,
            );
            rvf.pack_point_cloud(&cloud).unwrap();
            rvf.close().unwrap();
        }

        // Reopen read-only and query.
        let rvf = RoboticsRvf::open_readonly(&path).unwrap();
        let status = rvf.status();
        assert_eq!(status.total_vectors, 2);

        let hits = rvf.query_nearest(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].distance < 0.01);

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_query_dimension_mismatch() {
        let path = tmp_path();
        let mut rvf = RoboticsRvf::create(&path, 3).unwrap();
        let cloud = PointCloud::new(vec![Point3D::new(1.0, 0.0, 0.0)], 0);
        rvf.pack_point_cloud(&cloud).unwrap();

        // Query with wrong dimension.
        let result = rvf.query_nearest(&[1.0, 0.0], 1);
        assert!(result.is_err());

        rvf.close().unwrap();
        std::fs::remove_file(&path).ok();
    }
}
