//! Memory Distillation for ReasoningBank
//!
//! Implements techniques for compressing old trajectories while
//! preserving key lessons and insights for long-term learning.

use crate::error::{Result, RuvLLMError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{PatternCategory, Trajectory, Verdict};

/// Configuration for memory distillation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationConfig {
    /// Minimum age (seconds) before trajectory can be distilled
    pub min_age_for_distillation_secs: u64,
    /// Compression ratio target (e.g., 0.1 = keep 10%)
    pub compression_ratio: f32,
    /// Minimum quality to preserve in summary
    pub min_quality_threshold: f32,
    /// Maximum lessons per distillation
    pub max_lessons: usize,
    /// Minimum trajectories to trigger distillation
    pub min_trajectories_for_distillation: usize,
    /// Enable semantic deduplication
    pub deduplicate_lessons: bool,
    /// Similarity threshold for deduplication
    pub dedup_similarity_threshold: f32,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            min_age_for_distillation_secs: 86400, // 24 hours
            compression_ratio: 0.1,
            min_quality_threshold: 0.4,
            max_lessons: 100,
            min_trajectories_for_distillation: 100,
            deduplicate_lessons: true,
            dedup_similarity_threshold: 0.85,
        }
    }
}

/// A compressed representation of a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedTrajectory {
    /// Original trajectory ID
    pub original_id: u64,
    /// Key embedding (compressed representation)
    pub key_embedding: Vec<f32>,
    /// Verdict
    pub verdict: Verdict,
    /// Quality score
    pub quality: f32,
    /// Preserved lessons
    pub preserved_lessons: Vec<String>,
    /// Summary of key actions
    pub action_summary: Vec<String>,
    /// Original timestamp
    pub original_timestamp: DateTime<Utc>,
    /// Compression timestamp
    pub compressed_at: DateTime<Utc>,
    /// Number of original steps
    pub original_step_count: usize,
    /// Category
    pub category: PatternCategory,
}

impl CompressedTrajectory {
    /// Create from a trajectory
    pub fn from_trajectory(trajectory: &Trajectory) -> Self {
        let action_summary: Vec<String> = trajectory
            .steps
            .iter()
            .filter(|s| s.outcome.is_success())
            .take(5)
            .map(|s| s.action.clone())
            .collect();

        Self {
            original_id: trajectory.id.as_u64(),
            key_embedding: trajectory.query_embedding.clone(),
            verdict: trajectory.verdict.clone(),
            quality: trajectory.quality,
            preserved_lessons: trajectory.lessons.clone(),
            action_summary,
            original_timestamp: trajectory.started_at,
            compressed_at: Utc::now(),
            original_step_count: trajectory.steps.len(),
            category: infer_category(trajectory),
        }
    }

    /// Get memory size estimate (bytes)
    pub fn estimated_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.key_embedding.len() * std::mem::size_of::<f32>()
            + self
                .preserved_lessons
                .iter()
                .map(|s| s.len())
                .sum::<usize>()
            + self.action_summary.iter().map(|s| s.len()).sum::<usize>()
    }
}

/// A key lesson extracted from trajectories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyLesson {
    /// Lesson content
    pub content: String,
    /// Embedding for semantic search
    pub embedding: Vec<f32>,
    /// Source trajectory IDs
    pub source_trajectory_ids: Vec<u64>,
    /// Observation count (how many times seen)
    pub observation_count: u32,
    /// Category
    pub category: PatternCategory,
    /// Importance score
    pub importance: f32,
    /// Success rate when lesson was applied
    pub success_rate: f32,
    /// Average quality of source trajectories
    pub avg_quality: f32,
    /// Example actions demonstrating this lesson
    pub example_actions: Vec<String>,
    /// Tags
    pub tags: Vec<String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last observed timestamp
    pub last_observed: DateTime<Utc>,
}

impl KeyLesson {
    /// Create a new key lesson
    pub fn new(content: String, embedding: Vec<f32>, category: PatternCategory) -> Self {
        let now = Utc::now();
        Self {
            content,
            embedding,
            source_trajectory_ids: Vec::new(),
            observation_count: 1,
            category,
            importance: 0.5,
            success_rate: 0.0,
            avg_quality: 0.0,
            example_actions: Vec::new(),
            tags: Vec::new(),
            created_at: now,
            last_observed: now,
        }
    }

    /// Merge with another observation of the same lesson
    pub fn merge(&mut self, other: &KeyLesson) {
        self.observation_count += other.observation_count;

        // Rolling average for metrics
        let n = self.observation_count as f32;
        let w1 = (n - other.observation_count as f32) / n;
        let w2 = other.observation_count as f32 / n;

        self.importance = self.importance * w1 + other.importance * w2;
        self.success_rate = self.success_rate * w1 + other.success_rate * w2;
        self.avg_quality = self.avg_quality * w1 + other.avg_quality * w2;

        // Merge source trajectories
        for id in &other.source_trajectory_ids {
            if !self.source_trajectory_ids.contains(id) {
                self.source_trajectory_ids.push(*id);
            }
        }

        // Merge example actions (limit to 10)
        for action in &other.example_actions {
            if !self.example_actions.contains(action) && self.example_actions.len() < 10 {
                self.example_actions.push(action.clone());
            }
        }

        // Update timestamp
        self.last_observed = self.last_observed.max(other.last_observed);
    }

    /// Compute similarity with another lesson (by content hash)
    pub fn content_similarity(&self, other: &KeyLesson) -> f32 {
        // Simple Jaccard similarity on words
        let content1_lower = self.content.to_lowercase();
        let content2_lower = other.content.to_lowercase();

        let words1: std::collections::HashSet<&str> = content1_lower.split_whitespace().collect();
        let words2: std::collections::HashSet<&str> = content2_lower.split_whitespace().collect();

        let intersection = words1.intersection(&words2).count();
        let union = words1.union(&words2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }

    /// Compute embedding similarity
    pub fn embedding_similarity(&self, other: &KeyLesson) -> f32 {
        if self.embedding.len() != other.embedding.len() || self.embedding.is_empty() {
            return 0.0;
        }

        let dot: f32 = self
            .embedding
            .iter()
            .zip(&other.embedding)
            .map(|(a, b)| a * b)
            .sum();
        let norm_a: f32 = self.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 1e-8 && norm_b > 1e-8 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

/// Result of distillation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationResult {
    /// Compressed trajectories
    pub compressed_trajectories: Vec<CompressedTrajectory>,
    /// Key lessons extracted
    pub key_lessons: Vec<KeyLesson>,
    /// Number of trajectories processed
    pub trajectories_processed: usize,
    /// Memory saved (estimated bytes)
    pub memory_saved: usize,
    /// Compression ratio achieved
    pub compression_ratio: f32,
    /// Processing time (ms)
    pub processing_time_ms: u64,
    /// Summary by category
    pub category_summary: HashMap<String, usize>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Generates summaries from trajectories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectorySummary {
    /// Category
    pub category: PatternCategory,
    /// Success count
    pub success_count: usize,
    /// Failure count
    pub failure_count: usize,
    /// Total trajectories
    pub total: usize,
    /// Average quality
    pub avg_quality: f32,
    /// Common actions
    pub common_actions: Vec<(String, usize)>,
    /// Common lessons
    pub common_lessons: Vec<(String, usize)>,
}

/// Memory distiller for compressing old trajectories
pub struct MemoryDistiller {
    /// Configuration
    config: DistillationConfig,
    /// Distillation count
    distillation_count: u64,
    /// Total trajectories distilled
    total_distilled: u64,
    /// Total memory saved
    total_memory_saved: u64,
}

impl MemoryDistiller {
    /// Create a new distiller
    pub fn new(config: DistillationConfig) -> Self {
        Self {
            config,
            distillation_count: 0,
            total_distilled: 0,
            total_memory_saved: 0,
        }
    }

    /// Extract key lessons from trajectories
    pub fn extract_key_lessons(&self, trajectories: &[Trajectory]) -> Result<DistillationResult> {
        let start = std::time::Instant::now();

        if trajectories.len() < self.config.min_trajectories_for_distillation {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Need at least {} trajectories, got {}",
                self.config.min_trajectories_for_distillation,
                trajectories.len()
            )));
        }

        // Compress trajectories
        let compressed: Vec<CompressedTrajectory> = trajectories
            .iter()
            .filter(|t| t.quality >= self.config.min_quality_threshold)
            .map(CompressedTrajectory::from_trajectory)
            .collect();

        // Extract lessons
        let mut lessons = self.extract_lessons_from_trajectories(trajectories);

        // Deduplicate if enabled
        if self.config.deduplicate_lessons {
            lessons = self.deduplicate_lessons(lessons);
        }

        // Limit lessons
        lessons.truncate(self.config.max_lessons);

        // Calculate category summary
        let mut category_summary: HashMap<String, usize> = HashMap::new();
        for trajectory in trajectories {
            let cat = infer_category(trajectory).to_string();
            *category_summary.entry(cat).or_insert(0) += 1;
        }

        // Estimate memory savings
        let original_size: usize = trajectories
            .iter()
            .map(|t| estimate_trajectory_size(t))
            .sum();
        let compressed_size: usize = compressed.iter().map(|c| c.estimated_size()).sum();
        let memory_saved = original_size.saturating_sub(compressed_size);

        let compression_ratio = if original_size > 0 {
            compressed_size as f32 / original_size as f32
        } else {
            1.0
        };

        let processing_time_ms = start.elapsed().as_millis() as u64;

        Ok(DistillationResult {
            compressed_trajectories: compressed,
            key_lessons: lessons,
            trajectories_processed: trajectories.len(),
            memory_saved,
            compression_ratio,
            processing_time_ms,
            category_summary,
            timestamp: Utc::now(),
        })
    }

    /// Extract lessons from trajectories
    fn extract_lessons_from_trajectories(&self, trajectories: &[Trajectory]) -> Vec<KeyLesson> {
        let mut lesson_map: HashMap<String, KeyLesson> = HashMap::new();

        for trajectory in trajectories {
            // Extract explicit lessons
            for lesson_content in &trajectory.lessons {
                let lesson = self.create_lesson(lesson_content.clone(), trajectory);
                self.merge_lesson(&mut lesson_map, lesson);
            }

            // Extract implicit lessons from successful patterns
            if trajectory.is_success() {
                let action_pattern: String = trajectory
                    .steps
                    .iter()
                    .filter(|s| s.outcome.is_success())
                    .take(3)
                    .map(|s| s.action.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> ");

                if !action_pattern.is_empty() {
                    let lesson_content = format!("Successful pattern: {}", action_pattern);
                    let lesson = self.create_lesson(lesson_content, trajectory);
                    self.merge_lesson(&mut lesson_map, lesson);
                }
            }

            // Extract lessons from failures
            if let Verdict::Failure(ref cause) = trajectory.verdict {
                let lesson_content = format!("Avoid: {}", cause);
                let mut lesson = self.create_lesson(lesson_content, trajectory);
                lesson.importance = 0.8; // Higher importance for failure lessons
                self.merge_lesson(&mut lesson_map, lesson);
            }

            // Extract lessons from recovered attempts
            if let Verdict::RecoveredViaReflection {
                reflection_attempts,
                ..
            } = trajectory.verdict
            {
                let lesson_content = format!(
                    "Recovery possible after {} attempts via reflection",
                    reflection_attempts
                );
                let mut lesson = self.create_lesson(lesson_content, trajectory);
                lesson.importance = 0.9; // High importance for recovery lessons
                self.merge_lesson(&mut lesson_map, lesson);
            }
        }

        // Sort by importance and observation count
        let mut lessons: Vec<KeyLesson> = lesson_map.into_values().collect();
        lessons.sort_by(|a, b| {
            let score_a = a.importance * (a.observation_count as f32).ln_1p();
            let score_b = b.importance * (b.observation_count as f32).ln_1p();
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        lessons
    }

    /// Create a lesson from trajectory context
    fn create_lesson(&self, content: String, trajectory: &Trajectory) -> KeyLesson {
        let example_actions: Vec<String> = trajectory
            .steps
            .iter()
            .filter(|s| s.outcome.is_success())
            .take(3)
            .map(|s| s.action.clone())
            .collect();

        let mut lesson = KeyLesson::new(
            content,
            trajectory.query_embedding.clone(),
            infer_category(trajectory),
        );

        lesson.source_trajectory_ids = vec![trajectory.id.as_u64()];
        lesson.success_rate = if trajectory.is_success() { 1.0 } else { 0.0 };
        lesson.avg_quality = trajectory.quality;
        lesson.example_actions = example_actions;
        lesson.tags = trajectory.metadata.tags.clone();

        lesson
    }

    /// Merge lesson into map
    fn merge_lesson(&self, map: &mut HashMap<String, KeyLesson>, lesson: KeyLesson) {
        let key = lesson.content.clone();
        if let Some(existing) = map.get_mut(&key) {
            existing.merge(&lesson);
        } else {
            map.insert(key, lesson);
        }
    }

    /// Deduplicate lessons by similarity
    fn deduplicate_lessons(&self, lessons: Vec<KeyLesson>) -> Vec<KeyLesson> {
        let mut deduplicated: Vec<KeyLesson> = Vec::new();

        for lesson in lessons {
            let is_duplicate = deduplicated.iter().any(|existing| {
                let content_sim = lesson.content_similarity(existing);
                let embedding_sim = lesson.embedding_similarity(existing);
                let combined_sim = 0.6 * content_sim + 0.4 * embedding_sim;
                combined_sim > self.config.dedup_similarity_threshold
            });

            if !is_duplicate {
                deduplicated.push(lesson);
            } else {
                // Merge with most similar existing
                if let Some(most_similar) = deduplicated.iter_mut().max_by(|a, b| {
                    let sim_a = lesson.content_similarity(a);
                    let sim_b = lesson.content_similarity(b);
                    sim_a
                        .partial_cmp(&sim_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    most_similar.merge(&lesson);
                }
            }
        }

        deduplicated
    }

    /// Compress old trajectories
    pub fn compress_old_trajectories(
        &self,
        trajectories: &[Trajectory],
    ) -> Vec<CompressedTrajectory> {
        let now = Utc::now();
        let min_age = chrono::Duration::seconds(self.config.min_age_for_distillation_secs as i64);

        trajectories
            .iter()
            .filter(|t| now - t.started_at >= min_age)
            .map(CompressedTrajectory::from_trajectory)
            .collect()
    }

    /// Generate summary for a group of trajectories
    pub fn generate_summary(&self, trajectories: &[Trajectory]) -> TrajectorySummary {
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut total_quality = 0.0f32;
        let mut action_counts: HashMap<String, usize> = HashMap::new();
        let mut lesson_counts: HashMap<String, usize> = HashMap::new();

        for trajectory in trajectories {
            if trajectory.is_success() {
                success_count += 1;
            } else if trajectory.is_failure() {
                failure_count += 1;
            }

            total_quality += trajectory.quality;

            for step in &trajectory.steps {
                *action_counts.entry(step.action.clone()).or_insert(0) += 1;
            }

            for lesson in &trajectory.lessons {
                *lesson_counts.entry(lesson.clone()).or_insert(0) += 1;
            }
        }

        // Sort by frequency
        let mut common_actions: Vec<_> = action_counts.into_iter().collect();
        common_actions.sort_by(|a, b| b.1.cmp(&a.1));
        common_actions.truncate(10);

        let mut common_lessons: Vec<_> = lesson_counts.into_iter().collect();
        common_lessons.sort_by(|a, b| b.1.cmp(&a.1));
        common_lessons.truncate(10);

        // Determine category (most common)
        let category = if !trajectories.is_empty() {
            let mut cat_counts: HashMap<PatternCategory, usize> = HashMap::new();
            for t in trajectories {
                let cat = infer_category(t);
                *cat_counts.entry(cat).or_insert(0) += 1;
            }
            cat_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(cat, _)| cat)
                .unwrap_or(PatternCategory::General)
        } else {
            PatternCategory::General
        };

        TrajectorySummary {
            category,
            success_count,
            failure_count,
            total: trajectories.len(),
            avg_quality: if trajectories.is_empty() {
                0.0
            } else {
                total_quality / trajectories.len() as f32
            },
            common_actions,
            common_lessons,
        }
    }

    /// Get statistics
    pub fn stats(&self) -> DistillerStats {
        DistillerStats {
            distillation_count: self.distillation_count,
            total_distilled: self.total_distilled,
            total_memory_saved: self.total_memory_saved,
        }
    }
}

/// Statistics for the distiller
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DistillerStats {
    /// Number of distillations performed
    pub distillation_count: u64,
    /// Total trajectories distilled
    pub total_distilled: u64,
    /// Total memory saved (bytes)
    pub total_memory_saved: u64,
}

/// Helper function to infer category from trajectory
fn infer_category(trajectory: &Trajectory) -> PatternCategory {
    // Check verdict first
    match &trajectory.verdict {
        Verdict::RecoveredViaReflection { .. } => return PatternCategory::Reflection,
        Verdict::Failure(_) => return PatternCategory::ErrorRecovery,
        _ => {}
    }

    // Check metadata
    if let Some(ref req_type) = trajectory.metadata.request_type {
        let req_lower = req_type.to_lowercase();
        if req_lower.contains("code") {
            return PatternCategory::CodeGeneration;
        }
        if req_lower.contains("research") {
            return PatternCategory::Research;
        }
    }

    // Check tools
    if !trajectory.metadata.tools_invoked.is_empty() {
        return PatternCategory::ToolUse;
    }

    PatternCategory::General
}

/// Estimate trajectory memory size
fn estimate_trajectory_size(trajectory: &Trajectory) -> usize {
    let base_size = std::mem::size_of::<Trajectory>();
    let embedding_size = trajectory.query_embedding.len() * std::mem::size_of::<f32>();
    let response_embedding_size = trajectory
        .response_embedding
        .as_ref()
        .map(|e| e.len() * std::mem::size_of::<f32>())
        .unwrap_or(0);
    let steps_size: usize = trajectory
        .steps
        .iter()
        .map(|s| {
            std::mem::size_of_val(s)
                + s.action.len()
                + s.rationale.len()
                + s.context_embedding
                    .as_ref()
                    .map(|e| e.len() * 4)
                    .unwrap_or(0)
        })
        .sum();
    let lessons_size: usize = trajectory.lessons.iter().map(|l| l.len()).sum();

    base_size + embedding_size + response_embedding_size + steps_size + lessons_size
}

#[cfg(test)]
mod tests {
    use super::super::trajectory::{StepOutcome, TrajectoryRecorder};
    use super::*;

    fn make_trajectory(id: u64, quality: f32) -> Trajectory {
        let mut recorder = TrajectoryRecorder::new(vec![0.1; 64]);
        recorder.add_step(
            "action1".to_string(),
            "rationale1".to_string(),
            StepOutcome::Success,
            0.9,
        );
        recorder.add_step(
            "action2".to_string(),
            "rationale2".to_string(),
            StepOutcome::Success,
            0.8,
        );
        recorder.add_lesson(format!("Lesson from trajectory {}", id));

        let mut trajectory = recorder.complete(if quality > 0.5 {
            Verdict::Success
        } else {
            Verdict::Partial {
                completion_ratio: quality,
            }
        });

        // Override the auto-generated ID
        trajectory.id = super::super::trajectory::TrajectoryId::from_u64(id);
        trajectory
    }

    #[test]
    fn test_distillation_config_default() {
        let config = DistillationConfig::default();
        assert_eq!(config.min_age_for_distillation_secs, 86400);
        assert!(config.deduplicate_lessons);
    }

    #[test]
    fn test_compressed_trajectory() {
        let trajectory = make_trajectory(1, 0.8);
        let compressed = CompressedTrajectory::from_trajectory(&trajectory);

        assert_eq!(compressed.original_id, 1);
        assert!(compressed.estimated_size() > 0);
    }

    #[test]
    fn test_key_lesson_creation() {
        let lesson = KeyLesson::new(
            "Test lesson".to_string(),
            vec![0.1; 64],
            PatternCategory::General,
        );

        assert_eq!(lesson.observation_count, 1);
        assert_eq!(lesson.importance, 0.5);
    }

    #[test]
    fn test_key_lesson_merge() {
        let mut lesson1 = KeyLesson::new(
            "Test lesson".to_string(),
            vec![0.1; 4],
            PatternCategory::General,
        );
        lesson1.importance = 0.5;
        lesson1.success_rate = 0.8;

        let mut lesson2 = KeyLesson::new(
            "Test lesson".to_string(),
            vec![0.2; 4],
            PatternCategory::General,
        );
        lesson2.importance = 0.7;
        lesson2.success_rate = 0.6;

        lesson1.merge(&lesson2);

        assert_eq!(lesson1.observation_count, 2);
        assert!(lesson1.importance > 0.5 && lesson1.importance < 0.7);
    }

    #[test]
    fn test_lesson_similarity() {
        let lesson1 = KeyLesson::new(
            "Test lesson about code generation".to_string(),
            vec![1.0, 0.0, 0.0, 0.0],
            PatternCategory::General,
        );
        let lesson2 = KeyLesson::new(
            "Test lesson about code generation".to_string(),
            vec![1.0, 0.0, 0.0, 0.0],
            PatternCategory::General,
        );
        let lesson3 = KeyLesson::new(
            "Different topic entirely".to_string(),
            vec![0.0, 1.0, 0.0, 0.0],
            PatternCategory::General,
        );

        assert!((lesson1.content_similarity(&lesson2) - 1.0).abs() < 0.01);
        assert!(lesson1.content_similarity(&lesson3) < 0.5);

        assert!((lesson1.embedding_similarity(&lesson2) - 1.0).abs() < 0.01);
        assert!(lesson1.embedding_similarity(&lesson3).abs() < 0.01);
    }

    #[test]
    fn test_memory_distiller_creation() {
        let config = DistillationConfig::default();
        let distiller = MemoryDistiller::new(config);

        let stats = distiller.stats();
        assert_eq!(stats.distillation_count, 0);
    }

    #[test]
    fn test_extract_key_lessons() {
        let config = DistillationConfig {
            min_trajectories_for_distillation: 5,
            ..Default::default()
        };
        let distiller = MemoryDistiller::new(config);

        // Create test trajectories
        let trajectories: Vec<Trajectory> = (0..10).map(|i| make_trajectory(i, 0.7)).collect();

        let result = distiller.extract_key_lessons(&trajectories).unwrap();

        assert_eq!(result.trajectories_processed, 10);
        assert!(!result.key_lessons.is_empty());
        assert!(!result.compressed_trajectories.is_empty());
    }

    #[test]
    fn test_extract_lessons_requires_minimum() {
        let config = DistillationConfig {
            min_trajectories_for_distillation: 100,
            ..Default::default()
        };
        let distiller = MemoryDistiller::new(config);

        let trajectories: Vec<Trajectory> = (0..10).map(|i| make_trajectory(i, 0.7)).collect();

        let result = distiller.extract_key_lessons(&trajectories);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_summary() {
        let config = DistillationConfig::default();
        let distiller = MemoryDistiller::new(config);

        let trajectories: Vec<Trajectory> = (0..5)
            .map(|i| make_trajectory(i, if i % 2 == 0 { 0.8 } else { 0.3 }))
            .collect();

        let summary = distiller.generate_summary(&trajectories);

        assert_eq!(summary.total, 5);
        assert!(summary.success_count > 0);
        assert!(summary.avg_quality > 0.0);
    }

    #[test]
    fn test_deduplication() {
        let config = DistillationConfig {
            deduplicate_lessons: true,
            dedup_similarity_threshold: 0.8,
            ..Default::default()
        };
        let distiller = MemoryDistiller::new(config);

        let lessons = vec![
            KeyLesson::new(
                "Test lesson one".to_string(),
                vec![1.0, 0.0],
                PatternCategory::General,
            ),
            KeyLesson::new(
                "Test lesson one".to_string(),
                vec![1.0, 0.0],
                PatternCategory::General,
            ),
            KeyLesson::new(
                "Different lesson".to_string(),
                vec![0.0, 1.0],
                PatternCategory::General,
            ),
        ];

        let deduped = distiller.deduplicate_lessons(lessons);

        assert!(deduped.len() < 3);
    }

    #[test]
    fn test_infer_category() {
        let mut trajectory = make_trajectory(1, 0.8);
        trajectory.metadata.request_type = Some("code generation".to_string());

        let category = infer_category(&trajectory);
        assert_eq!(category, PatternCategory::CodeGeneration);
    }
}
