//! Internal world representation with object tracking, occupancy grid,
//! and linear state prediction.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An object being tracked in the world model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedObject {
    pub id: u64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub last_seen: i64,
    pub confidence: f64,
    pub label: String,
}

/// Predicted future state of a tracked object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictedState {
    pub position: [f64; 3],
    pub confidence: f64,
    pub time_horizon: f64,
}

// ---------------------------------------------------------------------------
// World model
// ---------------------------------------------------------------------------

/// Maintains a spatial model of the environment: tracked objects and a 2-D
/// occupancy grid.
#[derive(Debug, Clone)]
pub struct WorldModel {
    tracked_objects: HashMap<u64, TrackedObject>,
    occupancy: Vec<Vec<f32>>,
    grid_size: usize,
    grid_resolution: f64,
}

impl WorldModel {
    /// Create a new world model with a square occupancy grid.
    ///
    /// * `grid_size` -- number of cells along each axis.
    /// * `resolution` -- real-world size of each cell.
    pub fn new(grid_size: usize, resolution: f64) -> Self {
        Self {
            tracked_objects: HashMap::new(),
            occupancy: vec![vec![0.0_f32; grid_size]; grid_size],
            grid_size,
            grid_resolution: resolution,
        }
    }

    /// Insert or update a tracked object.
    pub fn update_object(&mut self, obj: TrackedObject) {
        self.tracked_objects.insert(obj.id, obj);
    }

    /// Remove objects that have not been observed for longer than `max_age`
    /// (microseconds). Returns the number of removed objects.
    pub fn remove_stale_objects(&mut self, current_time: i64, max_age: i64) -> usize {
        let before = self.tracked_objects.len();
        self.tracked_objects
            .retain(|_, obj| (current_time - obj.last_seen) <= max_age);
        before - self.tracked_objects.len()
    }

    /// Predict the future state of the object with the given ID using
    /// constant-velocity extrapolation over `dt` seconds.
    ///
    /// Confidence decays linearly with `dt`.
    pub fn predict_state(&self, object_id: u64, dt: f64) -> Option<PredictedState> {
        let obj = self.tracked_objects.get(&object_id)?;
        let predicted_pos = [
            obj.position[0] + obj.velocity[0] * dt,
            obj.position[1] + obj.velocity[1] * dt,
            obj.position[2] + obj.velocity[2] * dt,
        ];
        // Confidence decays with time horizon (halves every 5 s).
        let decay = (1.0 + dt / 5.0).recip();
        Some(PredictedState {
            position: predicted_pos,
            confidence: obj.confidence * decay,
            time_horizon: dt,
        })
    }

    /// Set the occupancy value at grid cell `(x, y)`.
    ///
    /// Values are typically in `[0.0, 1.0]` where 0 is free and 1 is
    /// occupied. Out-of-bounds writes are silently ignored.
    pub fn update_occupancy(&mut self, x: usize, y: usize, value: f32) {
        if x < self.grid_size && y < self.grid_size {
            self.occupancy[y][x] = value;
        }
    }

    /// Read the occupancy value at grid cell `(x, y)`.
    pub fn get_occupancy(&self, x: usize, y: usize) -> Option<f32> {
        if x < self.grid_size && y < self.grid_size {
            Some(self.occupancy[y][x])
        } else {
            None
        }
    }

    /// Check whether the straight-line path between two grid cells is free
    /// (all intermediate cells have occupancy < 0.5).
    ///
    /// Uses Bresenham-like sampling along the line.
    pub fn is_path_clear(&self, from: [usize; 2], to: [usize; 2]) -> bool {
        let steps = {
            let dx = (to[0] as isize - from[0] as isize).unsigned_abs();
            let dy = (to[1] as isize - from[1] as isize).unsigned_abs();
            dx.max(dy).max(1)
        };

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = (from[0] as f64 + t * (to[0] as f64 - from[0] as f64)).round() as usize;
            let y = (from[1] as f64 + t * (to[1] as f64 - from[1] as f64)).round() as usize;
            if x >= self.grid_size || y >= self.grid_size {
                return false;
            }
            if self.occupancy[y][x] >= 0.5 {
                return false;
            }
        }
        true
    }

    /// Number of currently tracked objects.
    pub fn object_count(&self) -> usize {
        self.tracked_objects.len()
    }

    /// Retrieve a tracked object by ID.
    pub fn get_object(&self, id: u64) -> Option<&TrackedObject> {
        self.tracked_objects.get(&id)
    }

    /// Grid size (cells per axis).
    pub fn grid_size(&self) -> usize {
        self.grid_size
    }

    /// Grid cell resolution in world units.
    pub fn grid_resolution(&self) -> f64 {
        self.grid_resolution
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_object(id: u64, pos: [f64; 3], vel: [f64; 3], last_seen: i64) -> TrackedObject {
        TrackedObject {
            id,
            position: pos,
            velocity: vel,
            last_seen,
            confidence: 0.9,
            label: format!("obj_{}", id),
        }
    }

    #[test]
    fn test_update_and_get_object() {
        let mut wm = WorldModel::new(10, 0.1);
        wm.update_object(sample_object(1, [1.0, 2.0, 3.0], [0.0; 3], 100));
        assert_eq!(wm.object_count(), 1);
        let obj = wm.get_object(1).unwrap();
        assert_eq!(obj.position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_remove_stale_objects() {
        let mut wm = WorldModel::new(10, 0.1);
        wm.update_object(sample_object(1, [0.0; 3], [0.0; 3], 100));
        wm.update_object(sample_object(2, [0.0; 3], [0.0; 3], 500));
        let removed = wm.remove_stale_objects(600, 200);
        assert_eq!(removed, 1);
        assert!(wm.get_object(1).is_none());
        assert!(wm.get_object(2).is_some());
    }

    #[test]
    fn test_predict_state() {
        let mut wm = WorldModel::new(10, 0.1);
        wm.update_object(sample_object(1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0));
        let pred = wm.predict_state(1, 2.0).unwrap();
        assert!((pred.position[0] - 2.0).abs() < 1e-9);
        assert!((pred.time_horizon - 2.0).abs() < 1e-9);
        assert!(pred.confidence < 0.9); // Decayed
    }

    #[test]
    fn test_predict_missing_object() {
        let wm = WorldModel::new(10, 0.1);
        assert!(wm.predict_state(99, 1.0).is_none());
    }

    #[test]
    fn test_occupancy_update_and_read() {
        let mut wm = WorldModel::new(5, 0.5);
        wm.update_occupancy(2, 3, 0.8);
        assert!((wm.get_occupancy(2, 3).unwrap() - 0.8).abs() < f32::EPSILON);
        assert!((wm.get_occupancy(0, 0).unwrap()).abs() < f32::EPSILON);
    }

    #[test]
    fn test_occupancy_out_of_bounds() {
        let mut wm = WorldModel::new(5, 0.5);
        wm.update_occupancy(10, 10, 1.0); // Should be silently ignored.
        assert!(wm.get_occupancy(10, 10).is_none());
    }

    #[test]
    fn test_path_clear() {
        let mut wm = WorldModel::new(10, 0.1);
        assert!(wm.is_path_clear([0, 0], [9, 0]));
        wm.update_occupancy(5, 0, 1.0);
        assert!(!wm.is_path_clear([0, 0], [9, 0]));
    }

    #[test]
    fn test_path_clear_diagonal() {
        let wm = WorldModel::new(10, 0.1);
        assert!(wm.is_path_clear([0, 0], [9, 9]));
    }

    #[test]
    fn test_grid_properties() {
        let wm = WorldModel::new(20, 0.05);
        assert_eq!(wm.grid_size(), 20);
        assert!((wm.grid_resolution() - 0.05).abs() < 1e-9);
    }
}
