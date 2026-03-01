//! Lightweight perception pipeline that ingests [`SensorFrame`]s, maintains a
//! [`SpatialIndex`], detects obstacles, and predicts linear trajectories.

use crate::bridge::indexing::SpatialIndex;
use crate::bridge::{Obstacle, PointCloud, SensorFrame, Trajectory};

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Configuration for the perception pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Points within this radius (metres) of the robot are classified as obstacles.
    pub obstacle_radius: f64,
    /// Whether to predict linear trajectories from consecutive frames.
    pub track_trajectories: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            obstacle_radius: 2.0,
            track_trajectories: true,
        }
    }
}

/// Output of a single pipeline frame.
#[derive(Debug, Clone)]
pub struct PerceptionResult {
    /// Detected obstacles in the current frame.
    pub obstacles: Vec<Obstacle>,
    /// Linear trajectory prediction (if enabled and enough history exists).
    pub trajectory_prediction: Option<Trajectory>,
    /// Wall-clock latency of this frame in microseconds.
    pub frame_latency_us: u64,
}

/// Cumulative pipeline statistics.
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Number of frames processed so far.
    pub frames_processed: u64,
    /// Running average latency in microseconds.
    pub avg_latency_us: f64,
    /// Total number of distinct objects tracked.
    pub objects_tracked: u64,
}

/// A stateful perception pipeline.
///
/// Call [`process_frame`](Self::process_frame) once per sensor tick.
pub struct PerceptionPipeline {
    config: PipelineConfig,
    index: SpatialIndex,
    stats: PipelineStats,
    /// Positions from previous frames for trajectory prediction (capped at 1000).
    position_history: Vec<[f64; 3]>,
    last_timestamp: i64,
    obstacle_counter: u64,
}

impl PerceptionPipeline {
    /// Create a pipeline with the given configuration.
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            index: SpatialIndex::new(3),
            stats: PipelineStats::default(),
            position_history: Vec::new(),
            last_timestamp: 0,
            obstacle_counter: 0,
        }
    }

    /// Process a single [`SensorFrame`] and return perception results.
    pub fn process_frame(&mut self, frame: &SensorFrame) -> PerceptionResult {
        let start = Instant::now();

        // -- 1. Rebuild spatial index from the point cloud -------------------
        self.index.clear();
        if let Some(ref cloud) = frame.cloud {
            self.index.insert_point_cloud(cloud);
        }

        // -- 2. Determine robot position (prefer state, fallback to pose) ----
        let robot_pos: Option<[f64; 3]> = frame
            .state
            .as_ref()
            .map(|s| s.position)
            .or_else(|| frame.pose.as_ref().map(|p| p.position));

        // -- 3. Detect obstacles ---------------------------------------------
        let obstacles = self.detect_obstacles(robot_pos, frame.cloud.as_ref());

        // -- 4. Trajectory prediction ----------------------------------------
        let trajectory_prediction = if self.config.track_trajectories {
            if let Some(pos) = robot_pos {
                // Cap history to prevent unbounded memory growth.
                if self.position_history.len() >= 1000 {
                    self.position_history.drain(..500);
                }
                self.position_history.push(pos);
                self.predict_trajectory(frame.timestamp_us)
            } else {
                None
            }
        } else {
            None
        };

        // -- 5. Update stats -------------------------------------------------
        let elapsed_us = start.elapsed().as_micros() as u64;
        self.stats.frames_processed += 1;
        self.stats.objects_tracked += obstacles.len() as u64;
        let n = self.stats.frames_processed as f64;
        self.stats.avg_latency_us =
            self.stats.avg_latency_us * ((n - 1.0) / n) + elapsed_us as f64 / n;
        self.last_timestamp = frame.timestamp_us;

        PerceptionResult {
            obstacles,
            trajectory_prediction,
            frame_latency_us: elapsed_us,
        }
    }

    /// Return cumulative statistics.
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Return a reference to the current spatial index.
    pub fn index(&self) -> &SpatialIndex {
        &self.index
    }

    fn detect_obstacles(
        &mut self,
        robot_pos: Option<[f64; 3]>,
        cloud: Option<&PointCloud>,
    ) -> Vec<Obstacle> {
        let robot_pos = match robot_pos {
            Some(p) => p,
            None => return Vec::new(),
        };
        let cloud = match cloud {
            Some(c) if !c.is_empty() => c,
            _ => return Vec::new(),
        };

        let query = [
            robot_pos[0] as f32,
            robot_pos[1] as f32,
            robot_pos[2] as f32,
        ];
        let radius = self.config.obstacle_radius as f32;

        let neighbours = match self.index.search_radius(&query, radius) {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        };

        neighbours
            .into_iter()
            .map(|(idx, dist)| {
                let pt = &cloud.points[idx];
                self.obstacle_counter += 1;
                Obstacle {
                    id: self.obstacle_counter,
                    position: [pt.x as f64, pt.y as f64, pt.z as f64],
                    distance: dist as f64,
                    radius: 0.1, // point-level radius
                    label: String::new(),
                    confidence: 1.0,
                }
            })
            .collect()
    }

    fn predict_trajectory(&self, current_ts: i64) -> Option<Trajectory> {
        if self.position_history.len() < 2 {
            return None;
        }
        let n = self.position_history.len();
        let prev = &self.position_history[n - 2];
        let curr = &self.position_history[n - 1];
        let vel = [
            curr[0] - prev[0],
            curr[1] - prev[1],
            curr[2] - prev[2],
        ];

        // Predict 5 steps into the future with constant velocity.
        let steps = 5;
        let dt_us: i64 = 100_000; // 100 ms per step
        let mut waypoints = Vec::with_capacity(steps);
        let mut timestamps = Vec::with_capacity(steps);
        for i in 1..=steps {
            let t = i as f64;
            waypoints.push([
                curr[0] + vel[0] * t,
                curr[1] + vel[1] * t,
                curr[2] + vel[2] * t,
            ]);
            timestamps.push(current_ts + dt_us * i as i64);
        }

        Some(Trajectory::new(waypoints, timestamps, 0.8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{Point3D, PointCloud, RobotState, SensorFrame};

    fn make_frame(
        points: Vec<Point3D>,
        position: [f64; 3],
        ts: i64,
    ) -> SensorFrame {
        SensorFrame {
            cloud: Some(PointCloud::new(points, ts)),
            state: Some(RobotState {
                position,
                velocity: [0.0; 3],
                acceleration: [0.0; 3],
                timestamp_us: ts,
            }),
            pose: None,
            timestamp_us: ts,
        }
    }

    #[test]
    fn test_empty_frame() {
        let mut pipeline = PerceptionPipeline::new(PipelineConfig::default());
        let frame = SensorFrame::default();
        let result = pipeline.process_frame(&frame);
        assert!(result.obstacles.is_empty());
        assert!(result.trajectory_prediction.is_none());
        assert_eq!(pipeline.stats().frames_processed, 1);
    }

    #[test]
    fn test_obstacle_detection() {
        let config = PipelineConfig {
            obstacle_radius: 5.0,
            ..Default::default()
        };
        let mut pipeline = PerceptionPipeline::new(config);

        // Robot at origin, obstacle at (1, 0, 0).
        let frame = make_frame(
            vec![
                Point3D::new(1.0, 0.0, 0.0),
                Point3D::new(100.0, 0.0, 0.0), // too far
            ],
            [0.0, 0.0, 0.0],
            1000,
        );
        let result = pipeline.process_frame(&frame);
        assert_eq!(result.obstacles.len(), 1);
        assert!((result.obstacles[0].distance - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_no_obstacles_when_far() {
        let config = PipelineConfig {
            obstacle_radius: 0.5,
            ..Default::default()
        };
        let mut pipeline = PerceptionPipeline::new(config);
        let frame = make_frame(
            vec![Point3D::new(10.0, 10.0, 10.0)],
            [0.0, 0.0, 0.0],
            1000,
        );
        let result = pipeline.process_frame(&frame);
        assert!(result.obstacles.is_empty());
    }

    #[test]
    fn test_trajectory_prediction() {
        let config = PipelineConfig {
            track_trajectories: true,
            obstacle_radius: 0.1,
            ..Default::default()
        };
        let mut pipeline = PerceptionPipeline::new(config);

        // Frame 1: robot at (0, 0, 0).
        let f1 = make_frame(vec![], [0.0, 0.0, 0.0], 1000);
        let r1 = pipeline.process_frame(&f1);
        assert!(r1.trajectory_prediction.is_none()); // need 2+ frames

        // Frame 2: robot at (1, 0, 0) => velocity = (1, 0, 0).
        let f2 = make_frame(vec![], [1.0, 0.0, 0.0], 2000);
        let r2 = pipeline.process_frame(&f2);
        let traj = r2.trajectory_prediction.unwrap();
        assert_eq!(traj.len(), 5);
        // First predicted waypoint should be ~(2, 0, 0).
        assert!((traj.waypoints[0][0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_trajectory_disabled() {
        let config = PipelineConfig {
            track_trajectories: false,
            ..Default::default()
        };
        let mut pipeline = PerceptionPipeline::new(config);
        let f1 = make_frame(vec![], [0.0, 0.0, 0.0], 0);
        let f2 = make_frame(vec![], [1.0, 0.0, 0.0], 1000);
        pipeline.process_frame(&f1);
        let r2 = pipeline.process_frame(&f2);
        assert!(r2.trajectory_prediction.is_none());
    }

    #[test]
    fn test_stats_accumulate() {
        let mut pipeline = PerceptionPipeline::new(PipelineConfig::default());
        for _ in 0..5 {
            pipeline.process_frame(&SensorFrame::default());
        }
        assert_eq!(pipeline.stats().frames_processed, 5);
    }

    #[test]
    fn test_obstacle_ids_increment() {
        let config = PipelineConfig {
            obstacle_radius: 100.0,
            ..Default::default()
        };
        let mut pipeline = PerceptionPipeline::new(config);
        let frame = make_frame(
            vec![
                Point3D::new(1.0, 0.0, 0.0),
                Point3D::new(2.0, 0.0, 0.0),
            ],
            [0.0, 0.0, 0.0],
            0,
        );
        let r = pipeline.process_frame(&frame);
        assert_eq!(r.obstacles.len(), 2);
        // IDs should be monotonically increasing.
        assert!(r.obstacles[0].id < r.obstacles[1].id);
    }

    #[test]
    fn test_pipeline_config_serde() {
        let cfg = PipelineConfig {
            obstacle_radius: 3.5,
            track_trajectories: false,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: PipelineConfig = serde_json::from_str(&json).unwrap();
        assert!((restored.obstacle_radius - 3.5).abs() < f64::EPSILON);
        assert!(!restored.track_trajectories);
    }

    #[test]
    fn test_index_is_rebuilt_per_frame() {
        let config = PipelineConfig {
            obstacle_radius: 100.0,
            ..Default::default()
        };
        let mut pipeline = PerceptionPipeline::new(config);

        let f1 = make_frame(
            vec![Point3D::new(1.0, 0.0, 0.0)],
            [0.0, 0.0, 0.0],
            0,
        );
        pipeline.process_frame(&f1);
        assert_eq!(pipeline.index().len(), 1);

        let f2 = make_frame(
            vec![
                Point3D::new(1.0, 0.0, 0.0),
                Point3D::new(2.0, 0.0, 0.0),
                Point3D::new(3.0, 0.0, 0.0),
            ],
            [0.0, 0.0, 0.0],
            1000,
        );
        pipeline.process_frame(&f2);
        // Index should reflect only the latest frame's cloud.
        assert_eq!(pipeline.index().len(), 3);
    }
}
