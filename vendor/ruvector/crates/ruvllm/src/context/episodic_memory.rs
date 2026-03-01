//! Episodic Memory - Long-term memory for past trajectories and experiences
//!
//! Stores episodes with HNSW-indexed embeddings for efficient similarity search.
//! Supports memory compression for older episodes to manage storage.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
use ruvector_core::types::{DistanceMetric, HnswConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Result, RuvLLMError};

/// Configuration for episodic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemoryConfig {
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Maximum episodes to store
    pub max_episodes: usize,
    /// HNSW M parameter
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search parameter
    pub hnsw_ef_search: usize,
    /// Age threshold for compression (in days)
    pub compression_age_days: i64,
    /// Compression ratio (0.0 - 1.0, lower = more compression)
    pub compression_ratio: f32,
    /// Enable automatic compression
    pub auto_compress: bool,
}

impl Default for EpisodicMemoryConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 768,
            max_episodes: 10_000,
            hnsw_m: 16,
            hnsw_ef_construction: 100,
            hnsw_ef_search: 50,
            compression_age_days: 7,
            compression_ratio: 0.5,
            auto_compress: true,
        }
    }
}

/// A trajectory representing a sequence of actions and states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Trajectory identifier
    pub id: String,
    /// Sequence of state-action pairs
    pub steps: Vec<TrajectoryStep>,
    /// Final outcome (success: 1.0, failure: 0.0)
    pub outcome: f32,
    /// Quality score
    pub quality_score: f32,
    /// Task type
    pub task_type: String,
    /// Agent that executed this trajectory
    pub agent_type: Option<String>,
    /// Total duration
    pub duration_ms: u64,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// A single step in a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// State description
    pub state: String,
    /// Action taken
    pub action: String,
    /// Result of action
    pub result: Option<String>,
    /// Step embedding
    pub embedding: Option<Vec<f32>>,
    /// Reward signal
    pub reward: f32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Episode metadata for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeMetadata {
    /// Episode ID
    pub episode_id: String,
    /// Task description
    pub task_description: String,
    /// Task type
    pub task_type: String,
    /// Outcome (0.0-1.0)
    pub outcome: f32,
    /// Quality score
    pub quality_score: f32,
    /// Agent used
    pub agent_type: Option<String>,
    /// Number of steps
    pub step_count: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Is compressed
    pub is_compressed: bool,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// An episode in long-term memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// Episode ID
    pub id: String,
    /// Episode embedding (summary)
    pub embedding: Vec<f32>,
    /// Episode metadata
    pub metadata: EpisodeMetadata,
    /// Full trajectory (may be compressed)
    pub trajectory: Option<Trajectory>,
    /// Compressed representation
    pub compressed: Option<CompressedEpisode>,
}

/// Compressed episode representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedEpisode {
    /// Compressed embedding (may be lower dimension)
    pub embedding: Vec<f32>,
    /// Summary text
    pub summary: String,
    /// Key observations
    pub key_observations: Vec<String>,
    /// Key actions
    pub key_actions: Vec<String>,
    /// Learned patterns
    pub patterns: Vec<String>,
    /// Original step count
    pub original_step_count: usize,
    /// Compression timestamp
    pub compressed_at: DateTime<Utc>,
}

/// Memory compressor for old episodes
pub struct MemoryCompressor {
    /// Compression ratio
    ratio: f32,
    /// Target embedding dimension (for dimensionality reduction)
    target_dim: Option<usize>,
}

impl MemoryCompressor {
    /// Create new compressor
    pub fn new(ratio: f32, target_dim: Option<usize>) -> Self {
        Self { ratio, target_dim }
    }

    /// Compress a trajectory into a summary
    pub fn compress(&self, trajectory: &Trajectory) -> CompressedEpisode {
        // Select key steps based on reward
        let total_steps = trajectory.steps.len();
        let keep_count = ((total_steps as f32) * self.ratio).max(1.0) as usize;

        let mut steps_with_reward: Vec<(usize, &TrajectoryStep)> =
            trajectory.steps.iter().enumerate().collect();
        steps_with_reward.sort_by(|a, b| {
            b.1.reward
                .partial_cmp(&a.1.reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let key_steps: Vec<&TrajectoryStep> = steps_with_reward
            .into_iter()
            .take(keep_count)
            .map(|(_, s)| s)
            .collect();

        let key_observations: Vec<String> = key_steps.iter().map(|s| s.state.clone()).collect();
        let key_actions: Vec<String> = key_steps.iter().map(|s| s.action.clone()).collect();

        // Generate summary
        let summary = format!(
            "Task: {} | Outcome: {:.2} | Steps: {} | Key actions: {}",
            trajectory.task_type,
            trajectory.outcome,
            total_steps,
            key_actions.len()
        );

        // Extract patterns (simplified - in production, use clustering)
        let patterns = self.extract_patterns(&key_actions);

        // Compute compressed embedding (average of key step embeddings or reduce dimensions)
        let embedding = self.compress_embedding(&key_steps);

        CompressedEpisode {
            embedding,
            summary,
            key_observations,
            key_actions,
            patterns,
            original_step_count: total_steps,
            compressed_at: Utc::now(),
        }
    }

    /// Extract common patterns from actions
    fn extract_patterns(&self, actions: &[String]) -> Vec<String> {
        let mut patterns = Vec::new();

        // Simple pattern extraction - look for repeated action types
        let mut action_counts: HashMap<String, usize> = HashMap::new();
        for action in actions {
            // Extract action type (first word)
            if let Some(action_type) = action.split_whitespace().next() {
                *action_counts.entry(action_type.to_string()).or_insert(0) += 1;
            }
        }

        // Keep patterns that appear more than once
        for (pattern, count) in action_counts {
            if count > 1 {
                patterns.push(format!("{}:{}", pattern, count));
            }
        }

        patterns
    }

    /// Compress embedding (average or reduce dimensions)
    fn compress_embedding(&self, steps: &[&TrajectoryStep]) -> Vec<f32> {
        let embeddings: Vec<&Vec<f32>> =
            steps.iter().filter_map(|s| s.embedding.as_ref()).collect();

        if embeddings.is_empty() {
            return Vec::new();
        }

        let dim = embeddings[0].len();
        let target_dim = self.target_dim.unwrap_or(dim);

        // Average embeddings
        let mut avg = vec![0.0f32; dim];
        for emb in &embeddings {
            for (i, v) in emb.iter().enumerate() {
                avg[i] += v;
            }
        }
        let n = embeddings.len() as f32;
        for v in &mut avg {
            *v /= n;
        }

        // Simple dimensionality reduction if needed (truncation - in production use PCA)
        if target_dim < dim {
            avg.truncate(target_dim);
        }

        avg
    }
}

/// Statistics for episodic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemoryStats {
    /// Total episodes stored
    pub total_episodes: u64,
    /// Compressed episodes
    pub compressed_episodes: u64,
    /// Uncompressed episodes
    pub uncompressed_episodes: u64,
    /// Total searches
    pub total_searches: u64,
    /// Average search latency in microseconds
    pub avg_search_latency_us: u64,
    /// Successful retrievals
    pub successful_retrievals: u64,
}

/// Long-term episodic memory with HNSW indexing
pub struct EpisodicMemory {
    /// Configuration
    config: EpisodicMemoryConfig,
    /// HNSW index for similarity search
    index: Arc<RwLock<HnswIndex>>,
    /// Episode storage
    episodes: Arc<RwLock<HashMap<String, Episode>>>,
    /// Memory compressor
    compressor: MemoryCompressor,
    /// Statistics
    stats: EpisodicMemoryStatsInternal,
}

#[derive(Debug, Default)]
struct EpisodicMemoryStatsInternal {
    total_searches: AtomicU64,
    successful_retrievals: AtomicU64,
    total_search_latency_us: AtomicU64,
}

impl EpisodicMemory {
    /// Create new episodic memory with configuration
    pub fn new(config: EpisodicMemoryConfig) -> Result<Self> {
        let hnsw_config = HnswConfig {
            m: config.hnsw_m,
            ef_construction: config.hnsw_ef_construction,
            ef_search: config.hnsw_ef_search,
            max_elements: config.max_episodes,
        };

        let index = HnswIndex::new(config.embedding_dim, DistanceMetric::Cosine, hnsw_config)
            .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        let compressor = MemoryCompressor::new(config.compression_ratio, None);

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(index)),
            episodes: Arc::new(RwLock::new(HashMap::new())),
            compressor,
            stats: EpisodicMemoryStatsInternal::default(),
        })
    }

    /// Store an episode from a trajectory
    pub fn store_episode(
        &self,
        trajectory: Trajectory,
        summary_embedding: Vec<f32>,
        tags: Vec<String>,
    ) -> Result<String> {
        let episode_id = trajectory.id.clone();

        let metadata = EpisodeMetadata {
            episode_id: episode_id.clone(),
            task_description: trajectory.task_type.clone(),
            task_type: trajectory.task_type.clone(),
            outcome: trajectory.outcome,
            quality_score: trajectory.quality_score,
            agent_type: trajectory.agent_type.clone(),
            step_count: trajectory.steps.len(),
            duration_ms: trajectory.duration_ms,
            is_compressed: false,
            tags,
            created_at: trajectory.created_at,
        };

        let episode = Episode {
            id: episode_id.clone(),
            embedding: summary_embedding.clone(),
            metadata,
            trajectory: Some(trajectory),
            compressed: None,
        };

        // Add to HNSW index
        {
            let mut index = self.index.write();
            index.add(episode_id.clone(), summary_embedding)?;
        }

        // Store episode
        {
            let mut episodes = self.episodes.write();
            episodes.insert(episode_id.clone(), episode);
        }

        // Trigger compression if needed
        if self.config.auto_compress {
            self.compress_old_episodes()?;
        }

        // Enforce max episodes
        self.enforce_limit()?;

        Ok(episode_id)
    }

    /// Search for similar episodes
    pub fn search_similar(&self, query_embedding: &[f32], k: usize) -> Result<Vec<Episode>> {
        let start = std::time::Instant::now();

        let results = {
            let index = self.index.read();
            index.search(query_embedding, k)?
        };

        let episodes = self.episodes.read();
        let found: Vec<Episode> = results
            .into_iter()
            .filter_map(|r| episodes.get(&r.id).cloned())
            .collect();

        let latency = start.elapsed().as_micros() as u64;
        self.stats.total_searches.fetch_add(1, Ordering::SeqCst);
        self.stats
            .total_search_latency_us
            .fetch_add(latency, Ordering::SeqCst);

        if !found.is_empty() {
            self.stats
                .successful_retrievals
                .fetch_add(1, Ordering::SeqCst);
        }

        Ok(found)
    }

    /// Search with filtering
    pub fn search_with_filter<F>(
        &self,
        query_embedding: &[f32],
        k: usize,
        filter: F,
    ) -> Result<Vec<Episode>>
    where
        F: Fn(&EpisodeMetadata) -> bool,
    {
        // Search more than needed to account for filtering
        let search_k = k * 3;
        let results = self.search_similar(query_embedding, search_k)?;

        let filtered: Vec<Episode> = results
            .into_iter()
            .filter(|e| filter(&e.metadata))
            .take(k)
            .collect();

        Ok(filtered)
    }

    /// Search by task type
    pub fn search_by_task_type(
        &self,
        query_embedding: &[f32],
        task_type: &str,
        k: usize,
    ) -> Result<Vec<Episode>> {
        self.search_with_filter(query_embedding, k, |meta| meta.task_type == task_type)
    }

    /// Search successful episodes only
    pub fn search_successful(
        &self,
        query_embedding: &[f32],
        min_quality: f32,
        k: usize,
    ) -> Result<Vec<Episode>> {
        self.search_with_filter(query_embedding, k, |meta| {
            meta.outcome > 0.5 && meta.quality_score >= min_quality
        })
    }

    /// Compress old episodes
    pub fn compress_old_episodes(&self) -> Result<usize> {
        let threshold = Utc::now() - Duration::days(self.config.compression_age_days);
        let mut compressed_count = 0;

        let episodes_to_compress: Vec<String> = {
            let episodes = self.episodes.read();
            episodes
                .iter()
                .filter(|(_, e)| {
                    e.metadata.created_at < threshold
                        && !e.metadata.is_compressed
                        && e.trajectory.is_some()
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in episodes_to_compress {
            if let Some(episode) = self.episodes.write().get_mut(&id) {
                if let Some(trajectory) = episode.trajectory.take() {
                    let compressed = self.compressor.compress(&trajectory);
                    episode.compressed = Some(compressed);
                    episode.metadata.is_compressed = true;
                    compressed_count += 1;
                }
            }
        }

        Ok(compressed_count)
    }

    /// Get episode by ID
    pub fn get(&self, id: &str) -> Option<Episode> {
        self.episodes.read().get(id).cloned()
    }

    /// Delete episode
    pub fn delete(&self, id: &str) -> Result<bool> {
        let removed = {
            let mut episodes = self.episodes.write();
            episodes.remove(id).is_some()
        };

        if removed {
            let mut index = self.index.write();
            index.remove(&id.to_string())?;
        }

        Ok(removed)
    }

    /// Enforce storage limit
    fn enforce_limit(&self) -> Result<()> {
        let mut episodes = self.episodes.write();

        while episodes.len() > self.config.max_episodes {
            // Find oldest compressed episode to remove
            if let Some(oldest) = episodes
                .iter()
                .filter(|(_, e)| e.metadata.is_compressed)
                .min_by_key(|(_, e)| e.metadata.created_at)
                .map(|(id, _)| id.clone())
            {
                episodes.remove(&oldest);
                let mut index = self.index.write();
                let _ = index.remove(&oldest);
            } else if let Some(oldest) = episodes
                .iter()
                .min_by_key(|(_, e)| e.metadata.created_at)
                .map(|(id, _)| id.clone())
            {
                // Fall back to removing oldest uncompressed
                episodes.remove(&oldest);
                let mut index = self.index.write();
                let _ = index.remove(&oldest);
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> EpisodicMemoryStats {
        let episodes = self.episodes.read();
        let compressed = episodes
            .iter()
            .filter(|(_, e)| e.metadata.is_compressed)
            .count() as u64;
        let total = episodes.len() as u64;

        let searches = self.stats.total_searches.load(Ordering::SeqCst);
        let total_latency = self.stats.total_search_latency_us.load(Ordering::SeqCst);
        let avg_latency = if searches > 0 {
            total_latency / searches
        } else {
            0
        };

        EpisodicMemoryStats {
            total_episodes: total,
            compressed_episodes: compressed,
            uncompressed_episodes: total - compressed,
            total_searches: searches,
            avg_search_latency_us: avg_latency,
            successful_retrievals: self.stats.successful_retrievals.load(Ordering::SeqCst),
        }
    }

    /// Clear all episodes
    pub fn clear(&self) -> Result<()> {
        self.episodes.write().clear();

        // Recreate index
        let hnsw_config = HnswConfig {
            m: self.config.hnsw_m,
            ef_construction: self.config.hnsw_ef_construction,
            ef_search: self.config.hnsw_ef_search,
            max_elements: self.config.max_episodes,
        };

        let new_index = HnswIndex::new(
            self.config.embedding_dim,
            DistanceMetric::Cosine,
            hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        *self.index.write() = new_index;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_embedding(dim: usize) -> Vec<f32> {
        vec![0.1; dim]
    }

    fn test_trajectory() -> Trajectory {
        Trajectory {
            id: "traj-1".to_string(),
            steps: vec![
                TrajectoryStep {
                    state: "Initial state".to_string(),
                    action: "read_file /src/main.rs".to_string(),
                    result: Some("file contents".to_string()),
                    embedding: Some(vec![0.1; 128]),
                    reward: 0.5,
                    timestamp: Utc::now(),
                },
                TrajectoryStep {
                    state: "After reading".to_string(),
                    action: "edit_file /src/main.rs".to_string(),
                    result: Some("edited".to_string()),
                    embedding: Some(vec![0.2; 128]),
                    reward: 0.8,
                    timestamp: Utc::now(),
                },
            ],
            outcome: 1.0,
            quality_score: 0.9,
            task_type: "coding".to_string(),
            agent_type: Some("coder".to_string()),
            duration_ms: 5000,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_episodic_memory_creation() {
        let config = EpisodicMemoryConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let memory = EpisodicMemory::new(config).unwrap();
        assert_eq!(memory.stats().total_episodes, 0);
    }

    #[test]
    fn test_store_and_search() {
        let config = EpisodicMemoryConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let memory = EpisodicMemory::new(config).unwrap();

        let trajectory = test_trajectory();
        let embedding = test_embedding(128);

        let id = memory
            .store_episode(trajectory, embedding.clone(), vec!["test".to_string()])
            .unwrap();

        assert_eq!(id, "traj-1");
        assert_eq!(memory.stats().total_episodes, 1);

        let results = memory.search_similar(&embedding, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "traj-1");
    }

    #[test]
    fn test_search_with_filter() {
        let config = EpisodicMemoryConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let memory = EpisodicMemory::new(config).unwrap();

        let trajectory = test_trajectory();
        let embedding = test_embedding(128);

        memory
            .store_episode(trajectory, embedding.clone(), vec!["test".to_string()])
            .unwrap();

        // Filter by task type
        let results = memory.search_by_task_type(&embedding, "coding", 5).unwrap();
        assert_eq!(results.len(), 1);

        let results = memory
            .search_by_task_type(&embedding, "research", 5)
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_compression() {
        let compressor = MemoryCompressor::new(0.5, None);
        let trajectory = test_trajectory();

        let compressed = compressor.compress(&trajectory);

        assert!(!compressed.summary.is_empty());
        assert!(!compressed.key_actions.is_empty());
        assert_eq!(compressed.original_step_count, 2);
    }

    #[test]
    fn test_delete() {
        let config = EpisodicMemoryConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let memory = EpisodicMemory::new(config).unwrap();

        let trajectory = test_trajectory();
        let embedding = test_embedding(128);

        memory.store_episode(trajectory, embedding, vec![]).unwrap();

        assert!(memory.get("traj-1").is_some());
        assert!(memory.delete("traj-1").unwrap());
        assert!(memory.get("traj-1").is_none());
    }

    #[test]
    fn test_clear() {
        let config = EpisodicMemoryConfig {
            embedding_dim: 128,
            ..Default::default()
        };
        let memory = EpisodicMemory::new(config).unwrap();

        let trajectory = test_trajectory();
        let embedding = test_embedding(128);

        memory.store_episode(trajectory, embedding, vec![]).unwrap();

        assert_eq!(memory.stats().total_episodes, 1);
        memory.clear().unwrap();
        assert_eq!(memory.stats().total_episodes, 0);
    }
}
