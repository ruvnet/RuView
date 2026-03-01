//! Query trajectory tracking for learning query patterns

use std::sync::RwLock;
use std::time::{Duration, SystemTime};

/// A single query trajectory record
#[derive(Debug, Clone)]
pub struct QueryTrajectory {
    /// Query vector
    pub query_vector: Vec<f32>,
    /// Result IDs
    pub result_ids: Vec<u64>,
    /// Query latency in microseconds
    pub latency_us: u64,
    /// Search parameters used
    pub ef_search: usize,
    pub probes: usize,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Relevance feedback (if provided)
    pub relevant_ids: Vec<u64>,
    pub irrelevant_ids: Vec<u64>,
}

impl QueryTrajectory {
    /// Create a new query trajectory
    pub fn new(
        query_vector: Vec<f32>,
        result_ids: Vec<u64>,
        latency_us: u64,
        ef_search: usize,
        probes: usize,
    ) -> Self {
        Self {
            query_vector,
            result_ids,
            latency_us,
            ef_search,
            probes,
            timestamp: SystemTime::now(),
            relevant_ids: Vec::new(),
            irrelevant_ids: Vec::new(),
        }
    }

    /// Add relevance feedback
    pub fn add_feedback(&mut self, relevant_ids: Vec<u64>, irrelevant_ids: Vec<u64>) {
        self.relevant_ids = relevant_ids;
        self.irrelevant_ids = irrelevant_ids;
    }

    /// Calculate precision if feedback is available
    pub fn precision(&self) -> Option<f64> {
        if self.relevant_ids.is_empty() {
            return None;
        }

        let relevant_retrieved = self
            .result_ids
            .iter()
            .filter(|id| self.relevant_ids.contains(id))
            .count();

        Some(relevant_retrieved as f64 / self.result_ids.len() as f64)
    }

    /// Calculate recall if feedback is available
    pub fn recall(&self) -> Option<f64> {
        if self.relevant_ids.is_empty() {
            return None;
        }

        let relevant_retrieved = self
            .result_ids
            .iter()
            .filter(|id| self.relevant_ids.contains(id))
            .count();

        Some(relevant_retrieved as f64 / self.relevant_ids.len() as f64)
    }
}

/// Trajectory tracker with ring buffer
pub struct TrajectoryTracker {
    /// Ring buffer of trajectories
    trajectories: RwLock<Vec<QueryTrajectory>>,
    /// Maximum number of trajectories to keep
    max_size: usize,
    /// Current write position
    write_pos: RwLock<usize>,
}

impl TrajectoryTracker {
    /// Create a new trajectory tracker
    pub fn new(max_size: usize) -> Self {
        Self {
            trajectories: RwLock::new(Vec::with_capacity(max_size)),
            max_size,
            write_pos: RwLock::new(0),
        }
    }

    /// Record a new trajectory
    pub fn record(&self, trajectory: QueryTrajectory) {
        let mut trajectories = self.trajectories.write().unwrap();
        let mut pos = self.write_pos.write().unwrap();

        if trajectories.len() < self.max_size {
            trajectories.push(trajectory);
        } else {
            trajectories[*pos] = trajectory;
        }

        *pos = (*pos + 1) % self.max_size;
    }

    /// Get the most recent n trajectories
    pub fn get_recent(&self, n: usize) -> Vec<QueryTrajectory> {
        let trajectories = self.trajectories.read().unwrap();
        let count = trajectories.len().min(n);

        if count == 0 {
            return Vec::new();
        }

        let pos = *self.write_pos.read().unwrap();
        let mut result = Vec::with_capacity(count);

        if trajectories.len() < self.max_size {
            // Not full yet, just take last n
            let start = trajectories.len().saturating_sub(count);
            result.extend_from_slice(&trajectories[start..]);
        } else {
            // Ring buffer is full, need to handle wrap-around
            for i in 0..count {
                let idx = (pos + self.max_size - count + i) % self.max_size;
                result.push(trajectories[idx].clone());
            }
        }

        result
    }

    /// Get all trajectories
    pub fn get_all(&self) -> Vec<QueryTrajectory> {
        self.trajectories.read().unwrap().clone()
    }

    /// Get trajectories within a time window
    pub fn get_since(&self, duration: Duration) -> Vec<QueryTrajectory> {
        let trajectories = self.trajectories.read().unwrap();
        let cutoff = SystemTime::now() - duration;

        trajectories
            .iter()
            .filter(|t| t.timestamp >= cutoff)
            .cloned()
            .collect()
    }

    /// Get trajectories with feedback only
    pub fn get_with_feedback(&self) -> Vec<QueryTrajectory> {
        let trajectories = self.trajectories.read().unwrap();
        trajectories
            .iter()
            .filter(|t| !t.relevant_ids.is_empty())
            .cloned()
            .collect()
    }

    /// Calculate average latency
    pub fn avg_latency(&self) -> Option<f64> {
        let trajectories = self.trajectories.read().unwrap();
        if trajectories.is_empty() {
            return None;
        }

        let sum: u64 = trajectories.iter().map(|t| t.latency_us).sum();
        Some(sum as f64 / trajectories.len() as f64)
    }

    /// Get statistics
    pub fn stats(&self) -> TrajectoryStats {
        let trajectories = self.trajectories.read().unwrap();

        if trajectories.is_empty() {
            return TrajectoryStats::default();
        }

        let total = trajectories.len();
        let with_feedback = trajectories
            .iter()
            .filter(|t| !t.relevant_ids.is_empty())
            .count();

        let avg_latency =
            trajectories.iter().map(|t| t.latency_us).sum::<u64>() as f64 / total as f64;

        let avg_precision = if with_feedback > 0 {
            trajectories
                .iter()
                .filter_map(|t| t.precision())
                .sum::<f64>()
                / with_feedback as f64
        } else {
            0.0
        };

        let avg_recall = if with_feedback > 0 {
            trajectories.iter().filter_map(|t| t.recall()).sum::<f64>() / with_feedback as f64
        } else {
            0.0
        };

        TrajectoryStats {
            total_trajectories: total,
            trajectories_with_feedback: with_feedback,
            avg_latency_us: avg_latency,
            avg_precision,
            avg_recall,
        }
    }
}

/// Trajectory statistics
#[derive(Debug, Clone, Default)]
pub struct TrajectoryStats {
    pub total_trajectories: usize,
    pub trajectories_with_feedback: usize,
    pub avg_latency_us: f64,
    pub avg_precision: f64,
    pub avg_recall: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_creation() {
        let traj = QueryTrajectory::new(vec![1.0, 2.0, 3.0], vec![1, 2, 3], 1000, 50, 10);

        assert_eq!(traj.query_vector, vec![1.0, 2.0, 3.0]);
        assert_eq!(traj.result_ids, vec![1, 2, 3]);
        assert_eq!(traj.latency_us, 1000);
    }

    #[test]
    fn test_trajectory_feedback() {
        let mut traj = QueryTrajectory::new(vec![1.0, 2.0], vec![1, 2, 3, 4], 1000, 50, 10);

        traj.add_feedback(vec![1, 2, 5], vec![3]);

        assert_eq!(traj.precision(), Some(0.5)); // 2 out of 4 relevant
        assert_eq!(traj.recall(), Some(2.0 / 3.0)); // 2 out of 3 total relevant
    }

    #[test]
    fn test_tracker_ring_buffer() {
        let tracker = TrajectoryTracker::new(3);

        // Add 5 trajectories
        for i in 0..5 {
            tracker.record(QueryTrajectory::new(vec![i as f32], vec![i], 1000, 50, 10));
        }

        let all = tracker.get_all();
        assert_eq!(all.len(), 3); // Ring buffer size

        // Should have trajectories 2, 3, 4 (last 3)
        let recent = tracker.get_recent(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_tracker_stats() {
        let tracker = TrajectoryTracker::new(10);

        tracker.record(QueryTrajectory::new(vec![1.0], vec![1, 2], 1000, 50, 10));

        tracker.record(QueryTrajectory::new(vec![2.0], vec![3, 4], 2000, 60, 15));

        let stats = tracker.stats();
        assert_eq!(stats.total_trajectories, 2);
        assert_eq!(stats.avg_latency_us, 1500.0);
    }
}
