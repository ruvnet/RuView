//! Error Pattern Learning and Recovery
//!
//! Implements intelligent error pattern learning that clusters similar errors
//! and learns recovery strategies from successful recoveries. When a new error
//! occurs, the system can suggest recovery strategies based on past successes.
//!
//! ## Architecture
//!
//! ```text
//! +----------------------+     +-------------------+
//! | ErrorPatternLearner  |---->| ErrorCluster      |
//! | - patterns           |     | - centroid        |
//! | - clusters           |     | - error_patterns  |
//! | - strategies         |     | - recovery_strats |
//! +----------------------+     +-------------------+
//!           |
//!           v
//! +----------------------+     +-------------------+
//! | learn_from_recovery  |---->| RecoveryStrategy  |
//! | - Extract pattern    |     | - description     |
//! | - Update cluster     |     | - success_rate    |
//! | - Store strategy     |     | - context         |
//! +----------------------+     +-------------------+
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::reflection::{ErrorPatternLearner, ErrorPatternLearnerConfig};
//!
//! let mut learner = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());
//!
//! // When an error is encountered and recovered from
//! learner.learn_from_recovery(
//!     "type mismatch: expected i32, found String",
//!     "Added explicit type conversion with .parse()",
//!     None,
//! );
//!
//! // Later, when a similar error occurs
//! let suggestions = learner.suggest_recovery("type mismatch: expected u64, found &str");
//! for suggestion in suggestions {
//!     println!("Try: {} (confidence: {:.2})", suggestion.strategy, suggestion.confidence);
//! }
//! ```

use super::reflective_agent::Reflection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for error pattern learner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPatternLearnerConfig {
    /// Maximum number of error patterns to store
    pub max_patterns: usize,
    /// Maximum number of clusters
    pub max_clusters: usize,
    /// Similarity threshold for clustering (0.0-1.0)
    pub similarity_threshold: f32,
    /// Minimum occurrences before a pattern is considered reliable
    pub min_occurrences: u32,
    /// Decay factor for old patterns
    pub decay_factor: f32,
    /// Maximum age for patterns (seconds)
    pub max_pattern_age_secs: u64,
    /// Minimum success rate for suggesting a strategy
    pub min_success_rate: f32,
}

impl Default for ErrorPatternLearnerConfig {
    fn default() -> Self {
        Self {
            max_patterns: 1000,
            max_clusters: 50,
            similarity_threshold: 0.7,
            min_occurrences: 3,
            decay_factor: 0.95,
            max_pattern_age_secs: 604800, // 1 week
            min_success_rate: 0.5,
        }
    }
}

/// An error pattern extracted from error messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    /// Pattern identifier
    pub id: u64,
    /// Normalized error message template
    pub template: String,
    /// Keywords extracted from the error
    pub keywords: Vec<String>,
    /// Error category
    pub category: ErrorCategory,
    /// Number of times this pattern has been seen
    pub occurrences: u32,
    /// Successful recovery count
    pub recovery_count: u32,
    /// Associated recovery strategies
    pub strategies: Vec<RecoveryStrategy>,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Created timestamp
    pub created_at: u64,
}

impl ErrorPattern {
    /// Create a new error pattern
    pub fn new(template: impl Into<String>, category: ErrorCategory) -> Self {
        let template = template.into();
        let keywords = Self::extract_keywords(&template);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id: 0,
            template,
            keywords,
            category,
            occurrences: 1,
            recovery_count: 0,
            strategies: Vec::new(),
            last_seen: now,
            created_at: now,
        }
    }

    /// Extract keywords from error message
    fn extract_keywords(message: &str) -> Vec<String> {
        // Common error keywords to look for
        let important_words = [
            "error",
            "failed",
            "invalid",
            "missing",
            "undefined",
            "null",
            "type",
            "mismatch",
            "expected",
            "found",
            "cannot",
            "unable",
            "permission",
            "denied",
            "timeout",
            "connection",
            "overflow",
            "underflow",
            "bounds",
            "index",
            "panic",
            "unwrap",
            "option",
            "result",
            "async",
            "await",
            "lifetime",
            "borrow",
            "move",
        ];

        message
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| word.len() > 2)
            .filter(|word| important_words.iter().any(|iw| word.contains(iw)) || word.len() > 5)
            .map(String::from)
            .take(10)
            .collect()
    }

    /// Compute similarity with another error message
    pub fn similarity(&self, other: &str) -> f32 {
        let other_keywords = Self::extract_keywords(other);

        if self.keywords.is_empty() || other_keywords.is_empty() {
            return 0.0;
        }

        let matching = self
            .keywords
            .iter()
            .filter(|k| {
                other_keywords
                    .iter()
                    .any(|ok| ok.contains(k.as_str()) || k.contains(ok.as_str()))
            })
            .count();

        let max_len = self.keywords.len().max(other_keywords.len());
        matching as f32 / max_len as f32
    }

    /// Get success rate
    pub fn success_rate(&self) -> f32 {
        if self.occurrences == 0 {
            0.0
        } else {
            self.recovery_count as f32 / self.occurrences as f32
        }
    }

    /// Add a recovery strategy
    pub fn add_strategy(&mut self, strategy: RecoveryStrategy) {
        // Check if similar strategy exists
        if let Some(existing) = self
            .strategies
            .iter_mut()
            .find(|s| s.similarity(&strategy) > 0.8)
        {
            existing.merge(&strategy);
        } else {
            self.strategies.push(strategy);
        }
    }
}

/// Category of error
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Type-related errors
    TypeMismatch,
    /// Missing or undefined items
    NotFound,
    /// Permission/access errors
    Permission,
    /// Network/connection errors
    Network,
    /// Timeout errors
    Timeout,
    /// Resource exhaustion
    ResourceExhaustion,
    /// Syntax errors
    Syntax,
    /// Logic/runtime errors
    Logic,
    /// Concurrency errors
    Concurrency,
    /// Memory/lifetime errors (Rust-specific)
    MemoryLifetime,
    /// Unknown category
    Unknown,
}

impl ErrorCategory {
    /// Classify an error message
    pub fn classify(message: &str) -> Self {
        let msg_lower = message.to_lowercase();

        if msg_lower.contains("type mismatch")
            || msg_lower.contains("expected type")
            || msg_lower.contains("mismatched types")
        {
            Self::TypeMismatch
        } else if msg_lower.contains("not found")
            || msg_lower.contains("undefined")
            || msg_lower.contains("does not exist")
            || msg_lower.contains("cannot find")
        {
            Self::NotFound
        } else if msg_lower.contains("permission")
            || msg_lower.contains("denied")
            || msg_lower.contains("unauthorized")
        {
            Self::Permission
        } else if msg_lower.contains("connection")
            || msg_lower.contains("network")
            || msg_lower.contains("socket")
        {
            Self::Network
        } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
            Self::Timeout
        } else if msg_lower.contains("out of memory")
            || msg_lower.contains("resource exhausted")
            || msg_lower.contains("too many")
        {
            Self::ResourceExhaustion
        } else if msg_lower.contains("syntax")
            || msg_lower.contains("parse error")
            || msg_lower.contains("unexpected token")
        {
            Self::Syntax
        } else if msg_lower.contains("borrow")
            || msg_lower.contains("lifetime")
            || msg_lower.contains("moved value")
        {
            Self::MemoryLifetime
        } else if msg_lower.contains("deadlock")
            || msg_lower.contains("race condition")
            || msg_lower.contains("concurrent")
        {
            Self::Concurrency
        } else if msg_lower.contains("panic")
            || msg_lower.contains("assertion")
            || msg_lower.contains("overflow")
        {
            Self::Logic
        } else {
            Self::Unknown
        }
    }
}

/// A learned recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStrategy {
    /// Strategy description
    pub description: String,
    /// Steps to perform
    pub steps: Vec<String>,
    /// Success count
    pub success_count: u32,
    /// Failure count
    pub failure_count: u32,
    /// Average time to recovery (ms)
    pub avg_recovery_time_ms: f32,
    /// Context tags
    pub context_tags: Vec<String>,
    /// Last used timestamp
    pub last_used: u64,
}

impl RecoveryStrategy {
    /// Create a new recovery strategy
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            steps: Vec::new(),
            success_count: 1,
            failure_count: 0,
            avg_recovery_time_ms: 0.0,
            context_tags: Vec::new(),
            last_used: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    /// Add a step
    pub fn with_step(mut self, step: impl Into<String>) -> Self {
        self.steps.push(step.into());
        self
    }

    /// Add context tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.context_tags.push(tag.into());
        self
    }

    /// Get success rate
    pub fn success_rate(&self) -> f32 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            self.success_count as f32 / total as f32
        }
    }

    /// Compute similarity with another strategy
    pub fn similarity(&self, other: &RecoveryStrategy) -> f32 {
        let desc_sim = self.description_similarity(&other.description);
        let tag_sim = self.tag_similarity(&other.context_tags);
        desc_sim * 0.7 + tag_sim * 0.3
    }

    /// Simple description similarity
    fn description_similarity(&self, other: &str) -> f32 {
        let desc_lower = self.description.to_lowercase();
        let words1: std::collections::HashSet<&str> = desc_lower.split_whitespace().collect();
        let other_lower = other.to_lowercase();
        let words2: std::collections::HashSet<&str> = other_lower.split_whitespace().collect();

        let intersection = words1.intersection(&words2).count();
        let union = words1.union(&words2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }

    /// Tag similarity
    fn tag_similarity(&self, other_tags: &[String]) -> f32 {
        if self.context_tags.is_empty() && other_tags.is_empty() {
            return 1.0;
        }
        if self.context_tags.is_empty() || other_tags.is_empty() {
            return 0.0;
        }

        let matching = self
            .context_tags
            .iter()
            .filter(|t| other_tags.contains(t))
            .count();

        matching as f32 / self.context_tags.len().max(other_tags.len()) as f32
    }

    /// Merge with another strategy (combine stats)
    pub fn merge(&mut self, other: &RecoveryStrategy) {
        self.success_count += other.success_count;
        self.failure_count += other.failure_count;

        // Running average for recovery time
        let total = self.success_count + other.success_count;
        if total > 0 {
            self.avg_recovery_time_ms = (self.avg_recovery_time_ms
                * (self.success_count - other.success_count) as f32
                + other.avg_recovery_time_ms * other.success_count as f32)
                / total as f32;
        }

        self.last_used = self.last_used.max(other.last_used);
    }

    /// Record a success
    pub fn record_success(&mut self, recovery_time_ms: u64) {
        let n = self.success_count as f32;
        self.avg_recovery_time_ms =
            (self.avg_recovery_time_ms * n + recovery_time_ms as f32) / (n + 1.0);
        self.success_count += 1;
        self.last_used = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }

    /// Record a failure
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_used = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}

/// A cluster of similar errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCluster {
    /// Cluster identifier
    pub id: u64,
    /// Representative pattern (centroid)
    pub centroid: ErrorPattern,
    /// Member patterns
    pub members: Vec<u64>,
    /// Aggregate recovery strategies
    pub strategies: Vec<RecoveryStrategy>,
    /// Total occurrences in cluster
    pub total_occurrences: u32,
    /// Total recoveries in cluster
    pub total_recoveries: u32,
}

impl ErrorCluster {
    /// Create a new cluster from a pattern
    pub fn new(id: u64, pattern: ErrorPattern) -> Self {
        let pattern_id = pattern.id;
        Self {
            id,
            centroid: pattern,
            members: vec![pattern_id],
            strategies: Vec::new(),
            total_occurrences: 1,
            total_recoveries: 0,
        }
    }

    /// Get cluster success rate
    pub fn success_rate(&self) -> f32 {
        if self.total_occurrences == 0 {
            0.0
        } else {
            self.total_recoveries as f32 / self.total_occurrences as f32
        }
    }

    /// Add a member pattern
    pub fn add_member(&mut self, pattern: &ErrorPattern) {
        if !self.members.contains(&pattern.id) {
            self.members.push(pattern.id);
        }
        self.total_occurrences += pattern.occurrences;
        self.total_recoveries += pattern.recovery_count;

        // Merge strategies
        for strategy in &pattern.strategies {
            self.add_strategy(strategy.clone());
        }
    }

    /// Add a recovery strategy
    pub fn add_strategy(&mut self, strategy: RecoveryStrategy) {
        if let Some(existing) = self
            .strategies
            .iter_mut()
            .find(|s| s.similarity(&strategy) > 0.8)
        {
            existing.merge(&strategy);
        } else {
            self.strategies.push(strategy);
        }
    }

    /// Get best strategies sorted by success rate
    pub fn best_strategies(&self, limit: usize) -> Vec<&RecoveryStrategy> {
        let mut sorted: Vec<_> = self.strategies.iter().collect();
        sorted.sort_by(|a, b| {
            b.success_rate()
                .partial_cmp(&a.success_rate())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(limit);
        sorted
    }
}

/// A suggestion for error recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySuggestion {
    /// Suggested recovery strategy
    pub strategy: String,
    /// Confidence in this suggestion (0.0-1.0)
    pub confidence: f32,
    /// Historical success rate
    pub success_rate: f32,
    /// Steps to perform
    pub steps: Vec<String>,
    /// Similar errors that were recovered using this strategy
    pub similar_errors: Vec<SimilarError>,
    /// Estimated recovery time (ms)
    pub estimated_time_ms: f32,
}

/// Record of a similar error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarError {
    /// Error message
    pub error: String,
    /// Recovery that worked
    pub recovery: String,
    /// Similarity score
    pub similarity: f32,
}

/// Outcome of a recovery attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryOutcome {
    /// Original error
    pub error: String,
    /// Strategy attempted
    pub strategy: String,
    /// Whether recovery was successful
    pub successful: bool,
    /// Time taken (ms)
    pub duration_ms: u64,
    /// Any notes about the recovery
    pub notes: Option<String>,
}

/// Error pattern learner
pub struct ErrorPatternLearner {
    /// Configuration
    config: ErrorPatternLearnerConfig,
    /// Stored error patterns
    patterns: HashMap<u64, ErrorPattern>,
    /// Error clusters
    clusters: HashMap<u64, ErrorCluster>,
    /// Next pattern ID
    next_pattern_id: u64,
    /// Next cluster ID
    next_cluster_id: u64,
    /// Statistics
    stats: ErrorLearnerStats,
}

/// Statistics for error learner
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorLearnerStats {
    /// Total errors processed
    pub total_errors: u64,
    /// Total recoveries learned
    pub total_recoveries: u64,
    /// Number of patterns
    pub pattern_count: usize,
    /// Number of clusters
    pub cluster_count: usize,
    /// Average cluster size
    pub avg_cluster_size: f32,
    /// Overall recovery rate
    pub overall_recovery_rate: f32,
}

impl ErrorPatternLearner {
    /// Create a new error pattern learner
    pub fn new(config: ErrorPatternLearnerConfig) -> Self {
        Self {
            config,
            patterns: HashMap::new(),
            clusters: HashMap::new(),
            next_pattern_id: 0,
            next_cluster_id: 0,
            stats: ErrorLearnerStats::default(),
        }
    }

    /// Learn from a successful recovery
    pub fn learn_from_recovery(
        &mut self,
        error: &str,
        recovery: &str,
        reflection: Option<&Reflection>,
    ) {
        self.stats.total_recoveries += 1;

        // Find or create pattern
        let pattern_id = self.find_or_create_pattern(error);

        // Create recovery strategy
        let mut strategy = RecoveryStrategy::new(recovery);

        // Add insights from reflection if available
        if let Some(ref r) = reflection {
            for insight in &r.insights {
                strategy = strategy.with_step(insight.clone());
            }
            for suggestion in &r.suggestions {
                strategy = strategy.with_tag(suggestion.clone());
            }
        }

        // Add strategy to pattern
        if let Some(pattern) = self.patterns.get_mut(&pattern_id) {
            pattern.recovery_count += 1;
            pattern.add_strategy(strategy.clone());
        }

        // Add strategy to cluster
        if let Some(cluster_id) = self.find_cluster_for_pattern(pattern_id) {
            if let Some(cluster) = self.clusters.get_mut(&cluster_id) {
                cluster.total_recoveries += 1;
                cluster.add_strategy(strategy);
            }
        }

        self.update_stats();
    }

    /// Record an error (without recovery)
    pub fn record_error(&mut self, error: &str) {
        self.stats.total_errors += 1;
        let pattern_id = self.find_or_create_pattern(error);

        // Update pattern occurrence count
        if let Some(pattern) = self.patterns.get_mut(&pattern_id) {
            pattern.occurrences += 1;
            pattern.last_seen = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
        }

        // Update cluster
        if let Some(cluster_id) = self.find_cluster_for_pattern(pattern_id) {
            if let Some(cluster) = self.clusters.get_mut(&cluster_id) {
                cluster.total_occurrences += 1;
            }
        }

        self.update_stats();
    }

    /// Suggest recovery strategies for an error
    pub fn suggest_recovery(&self, error: &str) -> Vec<RecoverySuggestion> {
        let mut suggestions = Vec::new();

        // Find similar patterns
        let similar_patterns = self.find_similar_patterns(error);

        for (pattern, similarity) in similar_patterns {
            // Skip if not enough data
            if pattern.occurrences < self.config.min_occurrences {
                continue;
            }

            // Get strategies from pattern
            for strategy in &pattern.strategies {
                if strategy.success_rate() < self.config.min_success_rate {
                    continue;
                }

                let confidence = similarity * strategy.success_rate();

                // Check if we already have a similar suggestion
                let is_duplicate = suggestions.iter().any(|s: &RecoverySuggestion| {
                    RecoveryStrategy::new(&s.strategy).similarity(strategy) > 0.8
                });

                if !is_duplicate {
                    suggestions.push(RecoverySuggestion {
                        strategy: strategy.description.clone(),
                        confidence,
                        success_rate: strategy.success_rate(),
                        steps: strategy.steps.clone(),
                        similar_errors: vec![SimilarError {
                            error: pattern.template.clone(),
                            recovery: strategy.description.clone(),
                            similarity,
                        }],
                        estimated_time_ms: strategy.avg_recovery_time_ms,
                    });
                }
            }
        }

        // Also check clusters for aggregate strategies
        for cluster in self.clusters.values() {
            let similarity = cluster.centroid.similarity(error);
            if similarity < self.config.similarity_threshold {
                continue;
            }

            for strategy in cluster.best_strategies(3) {
                let confidence = similarity * cluster.success_rate() * strategy.success_rate();

                let is_duplicate = suggestions.iter().any(|s: &RecoverySuggestion| {
                    RecoveryStrategy::new(&s.strategy).similarity(strategy) > 0.8
                });

                if !is_duplicate && confidence > 0.3 {
                    suggestions.push(RecoverySuggestion {
                        strategy: strategy.description.clone(),
                        confidence,
                        success_rate: strategy.success_rate(),
                        steps: strategy.steps.clone(),
                        similar_errors: Vec::new(),
                        estimated_time_ms: strategy.avg_recovery_time_ms,
                    });
                }
            }
        }

        // Sort by confidence
        suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        suggestions.truncate(5); // Return top 5
        suggestions
    }

    /// Find or create a pattern for an error
    fn find_or_create_pattern(&mut self, error: &str) -> u64 {
        // Check for existing similar pattern
        for (id, pattern) in &self.patterns {
            if pattern.similarity(error) > self.config.similarity_threshold {
                return *id;
            }
        }

        // Create new pattern
        let category = ErrorCategory::classify(error);
        let mut pattern = ErrorPattern::new(error, category);
        pattern.id = self.next_pattern_id;
        self.next_pattern_id += 1;

        let pattern_id = pattern.id;
        self.patterns.insert(pattern_id, pattern.clone());

        // Add to cluster
        self.add_to_cluster(pattern);

        // Prune if over capacity
        if self.patterns.len() > self.config.max_patterns {
            self.prune_old_patterns();
        }

        pattern_id
    }

    /// Find similar patterns
    fn find_similar_patterns(&self, error: &str) -> Vec<(&ErrorPattern, f32)> {
        let mut similar: Vec<_> = self
            .patterns
            .values()
            .map(|p| (p, p.similarity(error)))
            .filter(|(_, sim)| *sim > self.config.similarity_threshold * 0.5) // Lower threshold for suggestions
            .collect();

        similar.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        similar.truncate(10);
        similar
    }

    /// Add a pattern to an appropriate cluster
    fn add_to_cluster(&mut self, pattern: ErrorPattern) {
        // Find best matching cluster
        let mut best_cluster: Option<u64> = None;
        let mut best_similarity = 0.0f32;

        for (id, cluster) in &self.clusters {
            let sim = cluster.centroid.similarity(&pattern.template);
            if sim > self.config.similarity_threshold && sim > best_similarity {
                best_similarity = sim;
                best_cluster = Some(*id);
            }
        }

        if let Some(cluster_id) = best_cluster {
            if let Some(cluster) = self.clusters.get_mut(&cluster_id) {
                cluster.add_member(&pattern);
            }
        } else if self.clusters.len() < self.config.max_clusters {
            // Create new cluster
            let cluster = ErrorCluster::new(self.next_cluster_id, pattern);
            self.clusters.insert(self.next_cluster_id, cluster);
            self.next_cluster_id += 1;
        }
    }

    /// Find which cluster contains a pattern
    fn find_cluster_for_pattern(&self, pattern_id: u64) -> Option<u64> {
        for (cluster_id, cluster) in &self.clusters {
            if cluster.members.contains(&pattern_id) {
                return Some(*cluster_id);
            }
        }
        None
    }

    /// Prune old patterns
    fn prune_old_patterns(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let to_remove: Vec<u64> = self
            .patterns
            .iter()
            .filter(|(_, p)| {
                let age = now.saturating_sub(p.last_seen);
                age > self.config.max_pattern_age_secs && p.recovery_count < 2
            })
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            self.patterns.remove(&id);
        }

        // Apply decay to remaining patterns
        for pattern in self.patterns.values_mut() {
            pattern.occurrences =
                (pattern.occurrences as f32 * self.config.decay_factor).ceil() as u32;
        }
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.pattern_count = self.patterns.len();
        self.stats.cluster_count = self.clusters.len();

        if !self.clusters.is_empty() {
            let total_members: usize = self.clusters.values().map(|c| c.members.len()).sum();
            self.stats.avg_cluster_size = total_members as f32 / self.clusters.len() as f32;
        }

        if self.stats.total_errors > 0 {
            self.stats.overall_recovery_rate =
                self.stats.total_recoveries as f32 / self.stats.total_errors as f32;
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &ErrorLearnerStats {
        &self.stats
    }

    /// Get all patterns
    pub fn patterns(&self) -> &HashMap<u64, ErrorPattern> {
        &self.patterns
    }

    /// Get all clusters
    pub fn clusters(&self) -> &HashMap<u64, ErrorCluster> {
        &self.clusters
    }

    /// Clear all learned data
    pub fn clear(&mut self) {
        self.patterns.clear();
        self.clusters.clear();
        self.stats = ErrorLearnerStats::default();
        self.next_pattern_id = 0;
        self.next_cluster_id = 0;
    }

    /// Export learned patterns
    pub fn export(&self) -> (Vec<ErrorPattern>, Vec<ErrorCluster>) {
        (
            self.patterns.values().cloned().collect(),
            self.clusters.values().cloned().collect(),
        )
    }

    /// Import learned patterns
    pub fn import(&mut self, patterns: Vec<ErrorPattern>, clusters: Vec<ErrorCluster>) {
        for pattern in patterns {
            let id = pattern.id.max(self.next_pattern_id);
            self.next_pattern_id = id + 1;
            self.patterns.insert(pattern.id, pattern);
        }

        for cluster in clusters {
            let id = cluster.id.max(self.next_cluster_id);
            self.next_cluster_id = id + 1;
            self.clusters.insert(cluster.id, cluster);
        }

        self.update_stats();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_category_classification() {
        assert_eq!(
            ErrorCategory::classify("type mismatch: expected i32"),
            ErrorCategory::TypeMismatch
        );
        assert_eq!(
            ErrorCategory::classify("variable not found"),
            ErrorCategory::NotFound
        );
        assert_eq!(
            ErrorCategory::classify("permission denied"),
            ErrorCategory::Permission
        );
        assert_eq!(
            ErrorCategory::classify("connection refused"),
            ErrorCategory::Network
        );
        assert_eq!(
            ErrorCategory::classify("request timed out"),
            ErrorCategory::Timeout
        );
        assert_eq!(
            ErrorCategory::classify("cannot borrow as mutable"),
            ErrorCategory::MemoryLifetime
        );
    }

    #[test]
    fn test_error_pattern_creation() {
        let pattern = ErrorPattern::new(
            "type mismatch: expected i32, found String",
            ErrorCategory::TypeMismatch,
        );
        assert!(!pattern.keywords.is_empty());
        assert!(pattern
            .keywords
            .iter()
            .any(|k| k.contains("type") || k.contains("mismatch")));
    }

    #[test]
    fn test_error_pattern_similarity() {
        let pattern = ErrorPattern::new("type mismatch: expected i32", ErrorCategory::TypeMismatch);

        let similar = pattern.similarity("type mismatch: expected u64");
        let different = pattern.similarity("file not found");

        assert!(similar > different);
    }

    #[test]
    fn test_recovery_strategy_creation() {
        let strategy = RecoveryStrategy::new("Add type annotation")
            .with_step("Identify the mismatched type")
            .with_step("Add explicit annotation")
            .with_tag("type_error");

        assert!(!strategy.steps.is_empty());
        assert!(!strategy.context_tags.is_empty());
    }

    #[test]
    fn test_recovery_strategy_success_rate() {
        let mut strategy = RecoveryStrategy::new("test");
        strategy.success_count = 7;
        strategy.failure_count = 3;

        assert!((strategy.success_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_error_pattern_learner_creation() {
        let learner = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());
        assert_eq!(learner.stats().pattern_count, 0);
    }

    #[test]
    fn test_learn_from_recovery() {
        let mut learner = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());

        learner.learn_from_recovery(
            "type mismatch: expected i32, found String",
            "Added .parse() to convert string to integer",
            None,
        );

        assert_eq!(learner.stats().total_recoveries, 1);
        assert!(!learner.patterns().is_empty());
    }

    #[test]
    fn test_suggest_recovery() {
        let mut learner = ErrorPatternLearner::new(ErrorPatternLearnerConfig {
            min_occurrences: 1, // Lower for testing
            min_success_rate: 0.0,
            ..Default::default()
        });

        // Learn from several similar errors
        for _ in 0..3 {
            learner.learn_from_recovery(
                "type mismatch: expected i32, found String",
                "Use .parse() for conversion",
                None,
            );
        }

        // Get suggestions for similar error
        let suggestions = learner.suggest_recovery("type mismatch: expected u64, found &str");

        // Should have at least one suggestion
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].confidence > 0.0);
    }

    #[test]
    fn test_record_error() {
        let mut learner = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());

        learner.record_error("test error message");
        assert_eq!(learner.stats().total_errors, 1);

        learner.record_error("test error message");
        assert_eq!(learner.stats().total_errors, 2);

        // Should only have one pattern (duplicates merged)
        assert_eq!(learner.patterns().len(), 1);
    }

    #[test]
    fn test_export_import() {
        let mut learner1 = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());

        learner1.learn_from_recovery("error 1", "recovery 1", None);
        learner1.learn_from_recovery("error 2", "recovery 2", None);

        let (patterns, clusters) = learner1.export();

        let mut learner2 = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());
        learner2.import(patterns, clusters);

        assert_eq!(learner1.patterns().len(), learner2.patterns().len());
    }

    #[test]
    fn test_cluster_creation() {
        let mut learner = ErrorPatternLearner::new(ErrorPatternLearnerConfig::default());

        // Add similar errors - should cluster together
        learner.record_error("type mismatch: expected i32");
        learner.record_error("type mismatch: expected u64");
        learner.record_error("type mismatch: expected f32");

        // Should have fewer clusters than patterns due to grouping
        assert!(learner.clusters().len() <= learner.patterns().len());
    }

    #[test]
    fn test_strategy_merge() {
        let mut s1 = RecoveryStrategy::new("Add type annotation");
        s1.success_count = 5;
        s1.failure_count = 2;

        let s2 = RecoveryStrategy::new("Add type annotation with cast");
        // s2 has default success_count = 1

        s1.merge(&s2);

        assert_eq!(s1.success_count, 6);
    }
}
