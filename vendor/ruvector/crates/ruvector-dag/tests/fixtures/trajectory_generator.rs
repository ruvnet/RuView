//! Trajectory Generator for testing

use ruvector_dag::sona::DagTrajectory;
use rand::Rng;

pub struct TrajectoryGenerator {
    rng: rand::rngs::ThreadRng,
    embedding_dim: usize,
}

impl TrajectoryGenerator {
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            rng: rand::thread_rng(),
            embedding_dim,
        }
    }

    pub fn generate(&mut self, mechanism: &str) -> DagTrajectory {
        let query_hash = self.rng.gen::<u64>();
        let dag_embedding = self.random_embedding();
        let execution_time_ms = 10.0 + self.rng.gen::<f64>() * 990.0;
        let baseline_time_ms = execution_time_ms * (1.0 + self.rng.gen::<f64>() * 0.5);

        DagTrajectory::new(
            query_hash,
            dag_embedding,
            mechanism.to_string(),
            execution_time_ms,
            baseline_time_ms,
        )
    }

    pub fn generate_batch(&mut self, count: usize) -> Vec<DagTrajectory> {
        let mechanisms = ["topological", "causal_cone", "critical_path", "mincut_gated"];

        (0..count)
            .map(|i| {
                let mech = mechanisms[i % mechanisms.len()];
                self.generate(mech)
            })
            .collect()
    }

    pub fn generate_improving_batch(&mut self, count: usize) -> Vec<DagTrajectory> {
        // Generate trajectories with improving quality
        (0..count)
            .map(|i| {
                let improvement = i as f64 / count as f64;
                let execution_time = 100.0 * (1.0 - improvement * 0.5);
                let baseline = 100.0;

                DagTrajectory::new(
                    self.rng.gen(),
                    self.random_embedding(),
                    "auto".to_string(),
                    execution_time,
                    baseline,
                )
            })
            .collect()
    }

    fn random_embedding(&mut self) -> Vec<f32> {
        let mut embedding: Vec<f32> = (0..self.embedding_dim)
            .map(|_| self.rng.gen::<f32>() * 2.0 - 1.0)
            .collect();

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= norm);
        }

        embedding
    }
}

impl Default for TrajectoryGenerator {
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trajectory() {
        let mut gen = TrajectoryGenerator::new(128);
        let traj = gen.generate("topological");

        assert_eq!(traj.dag_embedding.len(), 128);
        assert_eq!(traj.mechanism, "topological");
        assert!(traj.execution_time_ms > 0.0);
        assert!(traj.baseline_time_ms > 0.0);
    }

    #[test]
    fn test_generate_batch() {
        let mut gen = TrajectoryGenerator::new(64);
        let trajectories = gen.generate_batch(20);

        assert_eq!(trajectories.len(), 20);

        // Check mechanism distribution
        let mechanisms: Vec<_> = trajectories.iter().map(|t| &t.mechanism).collect();
        assert!(mechanisms.contains(&&"topological".to_string()));
        assert!(mechanisms.contains(&&"causal_cone".to_string()));
    }

    #[test]
    fn test_improving_batch() {
        let mut gen = TrajectoryGenerator::new(128);
        let trajectories = gen.generate_improving_batch(10);

        assert_eq!(trajectories.len(), 10);

        // Check that execution times are decreasing (improvement)
        for i in 0..trajectories.len() - 1 {
            assert!(trajectories[i].execution_time_ms >= trajectories[i + 1].execution_time_ms);
        }
    }

    #[test]
    fn test_normalized_embeddings() {
        let mut gen = TrajectoryGenerator::new(64);
        let traj = gen.generate("test");

        // Check that embedding is normalized
        let norm: f32 = traj.dag_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }
}
