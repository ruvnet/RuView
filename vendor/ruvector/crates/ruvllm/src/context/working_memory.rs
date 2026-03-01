//! Working Memory - Short-term memory for current task context
//!
//! Provides fast access to current task state, tool results, and reasoning steps
//! with time-decaying attention weights.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Configuration for working memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryConfig {
    /// Maximum entries in scratchpad
    pub max_scratchpad_entries: usize,
    /// Maximum cached tool results
    pub max_tool_cache_entries: usize,
    /// Time decay factor for attention (per minute)
    pub attention_decay_rate: f32,
    /// Minimum attention weight before eviction
    pub min_attention_threshold: f32,
    /// Default attention weight for new entries
    pub default_attention: f32,
}

impl Default for WorkingMemoryConfig {
    fn default() -> Self {
        Self {
            max_scratchpad_entries: 100,
            max_tool_cache_entries: 50,
            attention_decay_rate: 0.1,
            min_attention_threshold: 0.05,
            default_attention: 1.0,
        }
    }
}

/// Task context representing current task state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    /// Task identifier
    pub task_id: String,
    /// Task description
    pub description: String,
    /// Task type (e.g., "coding", "research", "review")
    pub task_type: String,
    /// Current status
    pub status: TaskStatus,
    /// Task embedding (for similarity search)
    pub embedding: Option<Vec<f32>>,
    /// Files being worked on
    pub active_files: Vec<String>,
    /// Current step index in multi-step tasks
    pub current_step: usize,
    /// Total steps (if known)
    pub total_steps: Option<usize>,
    /// Task-specific metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending
    Pending,
    /// Task is in progress
    InProgress,
    /// Task is blocked (waiting for input)
    Blocked,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
}

impl Default for TaskContext {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            task_id: uuid::Uuid::new_v4().to_string(),
            description: String::new(),
            task_type: "general".to_string(),
            status: TaskStatus::Pending,
            embedding: None,
            active_files: Vec::new(),
            current_step: 0,
            total_steps: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Entry in the reasoning scratchpad
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadEntry {
    /// Entry content
    pub content: String,
    /// Entry type (thought, observation, action, result)
    pub entry_type: ScratchpadEntryType,
    /// Associated attention weight
    pub attention: f32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Optional embedding for semantic search
    pub embedding: Option<Vec<f32>>,
    /// Reference to related entries
    pub related_entries: Vec<usize>,
}

/// Type of scratchpad entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScratchpadEntryType {
    /// Internal thought/reasoning
    Thought,
    /// External observation
    Observation,
    /// Action taken
    Action,
    /// Result of action
    Result,
    /// Error encountered
    Error,
    /// Note/annotation
    Note,
}

/// Attention weights with time decay
#[derive(Debug, Clone)]
pub struct AttentionWeights {
    /// Weight values by key
    weights: HashMap<String, AttentionEntry>,
    /// Decay rate per minute
    decay_rate: f32,
    /// Minimum threshold
    min_threshold: f32,
}

#[derive(Debug, Clone)]
struct AttentionEntry {
    weight: f32,
    last_accessed: DateTime<Utc>,
}

impl AttentionWeights {
    /// Create new attention weights manager
    pub fn new(decay_rate: f32, min_threshold: f32) -> Self {
        Self {
            weights: HashMap::new(),
            decay_rate,
            min_threshold,
        }
    }

    /// Set attention weight for a key
    pub fn set(&mut self, key: &str, weight: f32) {
        self.weights.insert(
            key.to_string(),
            AttentionEntry {
                weight,
                last_accessed: Utc::now(),
            },
        );
    }

    /// Get attention weight for a key with decay applied
    pub fn get(&self, key: &str) -> Option<f32> {
        self.weights.get(key).map(|entry| {
            let elapsed_minutes = (Utc::now() - entry.last_accessed).num_seconds() as f32 / 60.0;
            let decayed = entry.weight * (-self.decay_rate * elapsed_minutes).exp();
            decayed.max(0.0)
        })
    }

    /// Get weight and update last accessed time
    pub fn get_and_touch(&mut self, key: &str) -> Option<f32> {
        if let Some(entry) = self.weights.get_mut(key) {
            let elapsed_minutes = (Utc::now() - entry.last_accessed).num_seconds() as f32 / 60.0;
            entry.weight = entry.weight * (-self.decay_rate * elapsed_minutes).exp();
            entry.last_accessed = Utc::now();
            Some(entry.weight.max(0.0))
        } else {
            None
        }
    }

    /// Boost attention for a key
    pub fn boost(&mut self, key: &str, amount: f32) {
        if let Some(entry) = self.weights.get_mut(key) {
            entry.weight = (entry.weight + amount).min(1.0);
            entry.last_accessed = Utc::now();
        }
    }

    /// Remove entries below threshold
    pub fn prune(&mut self) -> Vec<String> {
        let mut removed = Vec::new();
        let now = Utc::now();

        self.weights.retain(|key, entry| {
            let elapsed_minutes = (now - entry.last_accessed).num_seconds() as f32 / 60.0;
            let decayed = entry.weight * (-self.decay_rate * elapsed_minutes).exp();

            if decayed < self.min_threshold {
                removed.push(key.clone());
                false
            } else {
                true
            }
        });

        removed
    }

    /// Get all weights above threshold
    pub fn get_all(&self) -> Vec<(String, f32)> {
        let now = Utc::now();
        self.weights
            .iter()
            .filter_map(|(key, entry)| {
                let elapsed_minutes = (now - entry.last_accessed).num_seconds() as f32 / 60.0;
                let decayed = entry.weight * (-self.decay_rate * elapsed_minutes).exp();
                if decayed >= self.min_threshold {
                    Some((key.clone(), decayed))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Short-term working memory for current task
pub struct WorkingMemory {
    /// Configuration
    config: WorkingMemoryConfig,
    /// Current task context
    current_task: Arc<RwLock<Option<TaskContext>>>,
    /// Reasoning scratchpad
    scratchpad: Arc<RwLock<VecDeque<ScratchpadEntry>>>,
    /// Tool result cache
    tool_cache: Arc<RwLock<HashMap<String, CachedToolResult>>>,
    /// Attention weights for context elements
    attention: Arc<RwLock<AttentionWeights>>,
    /// Active variables/state
    variables: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

/// Cached tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolResult {
    /// Tool name
    pub tool_name: String,
    /// Tool input (hashed for comparison)
    pub input_hash: String,
    /// Tool output
    pub output: String,
    /// Success status
    pub success: bool,
    /// Cached at timestamp
    pub cached_at: DateTime<Utc>,
    /// Time-to-live
    pub ttl: Duration,
}

impl WorkingMemory {
    /// Create new working memory with configuration
    pub fn new(config: WorkingMemoryConfig) -> Self {
        let attention =
            AttentionWeights::new(config.attention_decay_rate, config.min_attention_threshold);

        Self {
            config,
            current_task: Arc::new(RwLock::new(None)),
            scratchpad: Arc::new(RwLock::new(VecDeque::new())),
            tool_cache: Arc::new(RwLock::new(HashMap::new())),
            attention: Arc::new(RwLock::new(attention)),
            variables: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set current task
    pub fn set_task(&self, task: TaskContext) {
        let task_id = task.task_id.clone();
        *self.current_task.write() = Some(task);
        self.attention
            .write()
            .set(&task_id, self.config.default_attention);
    }

    /// Get current task
    pub fn get_task(&self) -> Option<TaskContext> {
        self.current_task.read().clone()
    }

    /// Update task status
    pub fn update_task_status(&self, status: TaskStatus) {
        if let Some(task) = self.current_task.write().as_mut() {
            task.status = status;
            task.updated_at = Utc::now();
        }
    }

    /// Add entry to scratchpad
    pub fn add_to_scratchpad(&self, content: String, entry_type: ScratchpadEntryType) {
        self.add_to_scratchpad_with_embedding(content, entry_type, None);
    }

    /// Add entry to scratchpad with embedding
    pub fn add_to_scratchpad_with_embedding(
        &self,
        content: String,
        entry_type: ScratchpadEntryType,
        embedding: Option<Vec<f32>>,
    ) {
        let mut scratchpad = self.scratchpad.write();

        let entry = ScratchpadEntry {
            content,
            entry_type,
            attention: self.config.default_attention,
            timestamp: Utc::now(),
            embedding,
            related_entries: Vec::new(),
        };

        scratchpad.push_back(entry);

        // Enforce max entries
        while scratchpad.len() > self.config.max_scratchpad_entries {
            scratchpad.pop_front();
        }
    }

    /// Get recent scratchpad entries
    pub fn get_recent(&self, count: usize) -> Vec<ScratchpadEntry> {
        let scratchpad = self.scratchpad.read();
        scratchpad.iter().rev().take(count).cloned().collect()
    }

    /// Get scratchpad entries by type
    pub fn get_by_type(&self, entry_type: ScratchpadEntryType) -> Vec<ScratchpadEntry> {
        let scratchpad = self.scratchpad.read();
        scratchpad
            .iter()
            .filter(|e| e.entry_type == entry_type)
            .cloned()
            .collect()
    }

    /// Search scratchpad by similarity (requires embeddings)
    pub fn search_scratchpad(&self, query_embedding: &[f32], k: usize) -> Vec<ScratchpadEntry> {
        let scratchpad = self.scratchpad.read();

        let mut with_scores: Vec<(f32, &ScratchpadEntry)> = scratchpad
            .iter()
            .filter_map(|entry| {
                entry.embedding.as_ref().map(|emb| {
                    let score = cosine_similarity(query_embedding, emb);
                    (score, entry)
                })
            })
            .collect();

        with_scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        with_scores
            .into_iter()
            .take(k)
            .map(|(_, e)| e.clone())
            .collect()
    }

    /// Clear scratchpad
    pub fn clear_scratchpad(&self) {
        self.scratchpad.write().clear();
    }

    /// Cache tool result
    pub fn cache_tool_result(
        &self,
        tool_name: &str,
        input: &str,
        output: String,
        success: bool,
        ttl: Duration,
    ) {
        let input_hash = format!("{:x}", md5::compute(input));
        let key = format!("{}:{}", tool_name, input_hash);

        let result = CachedToolResult {
            tool_name: tool_name.to_string(),
            input_hash,
            output,
            success,
            cached_at: Utc::now(),
            ttl,
        };

        let mut cache = self.tool_cache.write();
        cache.insert(key, result);

        // Enforce max entries (remove oldest)
        while cache.len() > self.config.max_tool_cache_entries {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }
    }

    /// Get cached tool result
    pub fn get_cached_tool_result(&self, tool_name: &str, input: &str) -> Option<CachedToolResult> {
        let input_hash = format!("{:x}", md5::compute(input));
        let key = format!("{}:{}", tool_name, input_hash);

        let cache = self.tool_cache.read();
        cache.get(&key).and_then(|result| {
            let age = Utc::now() - result.cached_at;
            if age < result.ttl {
                Some(result.clone())
            } else {
                None
            }
        })
    }

    /// Clear tool cache
    pub fn clear_tool_cache(&self) {
        self.tool_cache.write().clear();
    }

    /// Set variable
    pub fn set_variable(&self, key: &str, value: serde_json::Value) {
        self.variables.write().insert(key.to_string(), value);
        self.attention
            .write()
            .set(key, self.config.default_attention);
    }

    /// Get variable
    pub fn get_variable(&self, key: &str) -> Option<serde_json::Value> {
        let result = self.variables.read().get(key).cloned();
        if result.is_some() {
            self.attention.write().boost(key, 0.1);
        }
        result
    }

    /// Get all variables
    pub fn get_all_variables(&self) -> HashMap<String, serde_json::Value> {
        self.variables.read().clone()
    }

    /// Get attention weight for a key
    pub fn get_attention(&self, key: &str) -> Option<f32> {
        self.attention.read().get(key)
    }

    /// Boost attention for a key
    pub fn boost_attention(&self, key: &str, amount: f32) {
        self.attention.write().boost(key, amount);
    }

    /// Prune low-attention entries
    pub fn prune(&self) -> PruneResult {
        let removed_keys = self.attention.write().prune();

        // Remove pruned variables
        {
            let mut variables = self.variables.write();
            for key in &removed_keys {
                variables.remove(key);
            }
        }

        // Clean expired tool cache
        let expired_tools: Vec<String> = {
            let cache = self.tool_cache.read();
            let now = Utc::now();
            cache
                .iter()
                .filter(|(_, v)| now - v.cached_at >= v.ttl)
                .map(|(k, _)| k.clone())
                .collect()
        };

        {
            let mut cache = self.tool_cache.write();
            for key in &expired_tools {
                cache.remove(key);
            }
        }

        PruneResult {
            variables_removed: removed_keys.len(),
            tool_cache_expired: expired_tools.len(),
        }
    }

    /// Get memory statistics
    pub fn stats(&self) -> WorkingMemoryStats {
        WorkingMemoryStats {
            scratchpad_entries: self.scratchpad.read().len(),
            tool_cache_entries: self.tool_cache.read().len(),
            variables_count: self.variables.read().len(),
            has_active_task: self.current_task.read().is_some(),
            attention_entries: self.attention.read().get_all().len(),
        }
    }

    /// Clear all working memory
    pub fn clear(&self) {
        *self.current_task.write() = None;
        self.scratchpad.write().clear();
        self.tool_cache.write().clear();
        self.variables.write().clear();
        *self.attention.write() = AttentionWeights::new(
            self.config.attention_decay_rate,
            self.config.min_attention_threshold,
        );
    }
}

/// Result of pruning operation
#[derive(Debug, Clone)]
pub struct PruneResult {
    /// Number of variables removed
    pub variables_removed: usize,
    /// Number of expired tool cache entries
    pub tool_cache_expired: usize,
}

/// Working memory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryStats {
    /// Number of scratchpad entries
    pub scratchpad_entries: usize,
    /// Number of tool cache entries
    pub tool_cache_entries: usize,
    /// Number of variables
    pub variables_count: usize,
    /// Whether there's an active task
    pub has_active_task: bool,
    /// Number of attention entries
    pub attention_entries: usize,
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_memory_creation() {
        let config = WorkingMemoryConfig::default();
        let memory = WorkingMemory::new(config);
        assert!(memory.get_task().is_none());
    }

    #[test]
    fn test_task_context() {
        let config = WorkingMemoryConfig::default();
        let memory = WorkingMemory::new(config);

        let task = TaskContext {
            task_id: "test-1".to_string(),
            description: "Test task".to_string(),
            ..Default::default()
        };

        memory.set_task(task.clone());
        assert!(memory.get_task().is_some());
        assert_eq!(memory.get_task().unwrap().task_id, "test-1");
    }

    #[test]
    fn test_scratchpad() {
        let config = WorkingMemoryConfig::default();
        let memory = WorkingMemory::new(config);

        memory.add_to_scratchpad("Thought 1".to_string(), ScratchpadEntryType::Thought);
        memory.add_to_scratchpad("Action 1".to_string(), ScratchpadEntryType::Action);
        memory.add_to_scratchpad("Result 1".to_string(), ScratchpadEntryType::Result);

        let recent = memory.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].entry_type, ScratchpadEntryType::Result);

        let thoughts = memory.get_by_type(ScratchpadEntryType::Thought);
        assert_eq!(thoughts.len(), 1);
    }

    #[test]
    fn test_tool_cache() {
        let config = WorkingMemoryConfig::default();
        let memory = WorkingMemory::new(config);

        memory.cache_tool_result(
            "read_file",
            "/path/to/file.rs",
            "file contents".to_string(),
            true,
            Duration::minutes(5),
        );

        let cached = memory.get_cached_tool_result("read_file", "/path/to/file.rs");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().output, "file contents");

        // Different input should not match
        let not_cached = memory.get_cached_tool_result("read_file", "/other/file.rs");
        assert!(not_cached.is_none());
    }

    #[test]
    fn test_variables() {
        let config = WorkingMemoryConfig::default();
        let memory = WorkingMemory::new(config);

        memory.set_variable("count", serde_json::json!(42));
        memory.set_variable("name", serde_json::json!("test"));

        assert_eq!(memory.get_variable("count"), Some(serde_json::json!(42)));
        assert_eq!(memory.get_variable("name"), Some(serde_json::json!("test")));
        assert!(memory.get_variable("unknown").is_none());
    }

    #[test]
    fn test_attention_weights() {
        let mut attention = AttentionWeights::new(0.1, 0.05);

        attention.set("key1", 1.0);
        attention.set("key2", 0.5);

        assert!(attention.get("key1").unwrap() > 0.9);
        assert!(attention.get("key2").unwrap() > 0.4);

        attention.boost("key2", 0.3);
        assert!(attention.get("key2").unwrap() > 0.7);
    }

    #[test]
    fn test_clear() {
        let config = WorkingMemoryConfig::default();
        let memory = WorkingMemory::new(config);

        memory.set_task(TaskContext::default());
        memory.add_to_scratchpad("test".to_string(), ScratchpadEntryType::Note);
        memory.set_variable("x", serde_json::json!(1));

        memory.clear();

        assert!(memory.get_task().is_none());
        assert_eq!(memory.get_recent(10).len(), 0);
        assert!(memory.get_variable("x").is_none());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 0.001);
    }
}
