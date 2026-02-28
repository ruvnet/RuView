//! Intelligent Context Manager - Prepares optimal context for LLM requests
//!
//! Handles context window management, priority scoring, and summarization
//! for different model token limits.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Result, RuvLLMError};

use super::agentic_memory::{AgenticMemory, AgenticMemoryConfig, MemoryType, RetrievedMemory};
use super::semantic_cache::{SemanticCacheConfig, SemanticToolCache};

/// Model token limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelTokenLimit {
    /// Claude Haiku - 200K context
    Haiku,
    /// Claude Sonnet - 200K context
    Sonnet,
    /// Claude Opus - 200K context
    Opus,
    /// Custom limit
    Custom(usize),
}

impl ModelTokenLimit {
    /// Get max tokens for this model
    pub fn max_tokens(&self) -> usize {
        match self {
            ModelTokenLimit::Haiku => 200_000,
            ModelTokenLimit::Sonnet => 200_000,
            ModelTokenLimit::Opus => 200_000,
            ModelTokenLimit::Custom(n) => *n,
        }
    }

    /// Get recommended context budget (80% of max for safety)
    pub fn context_budget(&self) -> usize {
        (self.max_tokens() as f32 * 0.8) as usize
    }
}

/// Configuration for context manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerConfig {
    /// Memory configuration
    pub memory: AgenticMemoryConfig,
    /// Semantic cache configuration
    pub cache: SemanticCacheConfig,
    /// Default model token limit
    pub default_model: ModelTokenLimit,
    /// Characters per token estimate
    pub chars_per_token: f32,
    /// Maximum context elements to consider
    pub max_elements: usize,
    /// Minimum relevance score for inclusion
    pub min_relevance: f32,
    /// Enable summarization for overflow
    pub enable_summarization: bool,
    /// Summarization target ratio (compress to this fraction)
    pub summarization_ratio: f32,
    /// Priority weights for different element types
    pub priority_weights: PriorityWeights,
}

/// Weights for priority scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityWeights {
    /// Weight for recency
    pub recency: f32,
    /// Weight for relevance (similarity)
    pub relevance: f32,
    /// Weight for importance (user marked)
    pub importance: f32,
    /// Weight for access frequency
    pub frequency: f32,
}

impl Default for PriorityWeights {
    fn default() -> Self {
        Self {
            recency: 0.3,
            relevance: 0.4,
            importance: 0.2,
            frequency: 0.1,
        }
    }
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            memory: AgenticMemoryConfig::default(),
            cache: SemanticCacheConfig::default(),
            default_model: ModelTokenLimit::Sonnet,
            chars_per_token: 4.0,
            max_elements: 100,
            min_relevance: 0.1,
            enable_summarization: true,
            summarization_ratio: 0.5,
            priority_weights: PriorityWeights::default(),
        }
    }
}

/// A context element to be included in the prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextElement {
    /// Element ID
    pub id: String,
    /// Element type
    pub element_type: ContextElementType,
    /// Content
    pub content: String,
    /// Estimated tokens
    pub estimated_tokens: usize,
    /// Priority score (0.0 - 1.0)
    pub priority: f32,
    /// Relevance score from similarity search
    pub relevance: f32,
    /// Recency (seconds since creation)
    pub recency_seconds: i64,
    /// Importance flag
    pub is_important: bool,
    /// Access count
    pub access_count: u64,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Type of context element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextElementType {
    /// System instruction
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool result
    Tool,
    /// Memory retrieval
    Memory,
    /// File content
    File,
    /// Cached result
    Cached,
}

/// Priority assigned to an element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementPriority {
    /// Critical - must include
    Critical,
    /// High - include if possible
    High,
    /// Medium - include if space
    Medium,
    /// Low - include if remaining space
    Low,
    /// Optional - only if abundant space
    Optional,
}

impl ElementPriority {
    /// Get numeric priority value
    pub fn value(&self) -> f32 {
        match self {
            ElementPriority::Critical => 1.0,
            ElementPriority::High => 0.8,
            ElementPriority::Medium => 0.6,
            ElementPriority::Low => 0.4,
            ElementPriority::Optional => 0.2,
        }
    }
}

/// Priority scorer for context elements
pub struct PriorityScorer {
    weights: PriorityWeights,
}

impl PriorityScorer {
    /// Create new scorer with weights
    pub fn new(weights: PriorityWeights) -> Self {
        Self { weights }
    }

    /// Score a context element
    pub fn score(&self, element: &ContextElement) -> f32 {
        // Recency score (exponential decay over 24 hours)
        let recency_score = (-element.recency_seconds as f32 / 86400.0).exp();

        // Relevance score (already normalized 0-1)
        let relevance_score = element.relevance;

        // Importance score
        let importance_score = if element.is_important { 1.0 } else { 0.5 };

        // Frequency score (logarithmic)
        let frequency_score = ((element.access_count as f32 + 1.0).ln() / 10.0).min(1.0);

        // Weighted combination
        let score = self.weights.recency * recency_score
            + self.weights.relevance * relevance_score
            + self.weights.importance * importance_score
            + self.weights.frequency * frequency_score;

        score.min(1.0).max(0.0)
    }

    /// Assign priority tier based on score
    pub fn assign_priority(&self, score: f32) -> ElementPriority {
        if score >= 0.9 {
            ElementPriority::Critical
        } else if score >= 0.7 {
            ElementPriority::High
        } else if score >= 0.5 {
            ElementPriority::Medium
        } else if score >= 0.3 {
            ElementPriority::Low
        } else {
            ElementPriority::Optional
        }
    }
}

/// Memory summarizer for overflow handling
pub struct MemorySummarizer {
    /// Target ratio for compression
    target_ratio: f32,
}

impl MemorySummarizer {
    /// Create new summarizer
    pub fn new(target_ratio: f32) -> Self {
        Self { target_ratio }
    }

    /// Summarize content to fit within token budget
    pub fn summarize(&self, content: &str, max_tokens: usize, chars_per_token: f32) -> String {
        let max_chars = (max_tokens as f32 * chars_per_token) as usize;

        if content.len() <= max_chars {
            return content.to_string();
        }

        // Simple summarization: truncate with indicator
        // In production, use an LLM for better summarization
        let target_len = (max_chars as f32 * self.target_ratio) as usize;

        if target_len < 100 {
            // Too short, just truncate
            format!("{}...", &content[..target_len.min(content.len())])
        } else {
            // Keep beginning and end, truncate middle
            let keep_start = target_len * 2 / 3;
            let keep_end = target_len / 3;

            let start = &content[..keep_start.min(content.len())];
            let end_start = content.len().saturating_sub(keep_end);
            let end = if end_start < content.len() {
                &content[end_start..]
            } else {
                ""
            };

            format!("{}...[truncated]...{}", start, end)
        }
    }

    /// Summarize multiple memories into a single summary
    pub fn summarize_memories(
        &self,
        memories: &[RetrievedMemory],
        max_tokens: usize,
        chars_per_token: f32,
    ) -> String {
        let max_chars = (max_tokens as f32 * chars_per_token) as usize;

        let mut summary = String::with_capacity(max_chars);
        let chars_per_memory = max_chars / memories.len().max(1);

        for (i, mem) in memories.iter().enumerate() {
            let mem_summary = if mem.content.len() > chars_per_memory {
                format!("{}...", &mem.content[..chars_per_memory])
            } else {
                mem.content.clone()
            };

            if i > 0 {
                summary.push_str("\n---\n");
            }
            summary.push_str(&format!("[{}] {}", mem.id, mem_summary));

            if summary.len() >= max_chars {
                break;
            }
        }

        summary
    }
}

/// Prepared context ready for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedContext {
    /// Elements included in context
    pub elements: Vec<ContextElement>,
    /// Total estimated tokens
    pub total_tokens: usize,
    /// Token budget used
    pub budget_used: f32,
    /// Elements that were summarized
    pub summarized_count: usize,
    /// Elements that were excluded
    pub excluded_count: usize,
    /// Preparation time in microseconds
    pub preparation_time_us: u64,
}

impl PreparedContext {
    /// Get concatenated context string
    pub fn to_string(&self) -> String {
        self.elements
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Get elements by type
    pub fn get_by_type(&self, element_type: ContextElementType) -> Vec<&ContextElement> {
        self.elements
            .iter()
            .filter(|e| e.element_type == element_type)
            .collect()
    }
}

/// Statistics for context manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerStats {
    /// Total preparations
    pub total_preparations: u64,
    /// Average tokens per preparation
    pub avg_tokens: u64,
    /// Average preparation time in microseconds
    pub avg_preparation_time_us: u64,
    /// Summarizations performed
    pub summarizations: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Memory retrievals
    pub memory_retrievals: u64,
}

/// Intelligent context manager
pub struct IntelligentContextManager {
    /// Configuration
    config: ContextManagerConfig,
    /// Agentic memory
    memory: AgenticMemory,
    /// Semantic cache
    cache: SemanticToolCache,
    /// Priority scorer
    scorer: PriorityScorer,
    /// Memory summarizer
    summarizer: MemorySummarizer,
    /// Statistics
    stats: ContextManagerStatsInternal,
}

#[derive(Debug, Default)]
struct ContextManagerStatsInternal {
    preparations: AtomicU64,
    total_tokens: AtomicU64,
    total_time_us: AtomicU64,
    summarizations: AtomicU64,
    cache_hits: AtomicU64,
    memory_retrievals: AtomicU64,
}

impl IntelligentContextManager {
    /// Create new context manager with configuration
    pub fn new(config: ContextManagerConfig) -> Result<Self> {
        let memory = AgenticMemory::new(config.memory.clone())?;
        let cache = SemanticToolCache::new(config.cache.clone())?;
        let scorer = PriorityScorer::new(config.priority_weights.clone());
        let summarizer = MemorySummarizer::new(config.summarization_ratio);

        Ok(Self {
            config,
            memory,
            cache,
            scorer,
            summarizer,
            stats: ContextManagerStatsInternal::default(),
        })
    }

    /// Prepare context for an LLM request
    pub fn prepare_context(
        &self,
        messages: &[Message],
        query_embedding: Option<&[f32]>,
        model: Option<ModelTokenLimit>,
    ) -> Result<PreparedContext> {
        let start = std::time::Instant::now();
        self.stats.preparations.fetch_add(1, Ordering::SeqCst);

        let model = model.unwrap_or(self.config.default_model);
        let budget = model.context_budget();

        let mut elements: Vec<ContextElement> = Vec::new();
        let now = Utc::now();

        // Step 1: Convert messages to context elements
        for (i, msg) in messages.iter().enumerate() {
            let element_type = match msg.role {
                MessageRole::System => ContextElementType::System,
                MessageRole::User => ContextElementType::User,
                MessageRole::Assistant => ContextElementType::Assistant,
            };

            let estimated_tokens = self.estimate_tokens(&msg.content);
            let recency = (now - msg.timestamp).num_seconds();

            let element = ContextElement {
                id: format!("msg-{}", i),
                element_type,
                content: msg.content.clone(),
                estimated_tokens,
                priority: if element_type == ContextElementType::System {
                    1.0
                } else {
                    0.8
                },
                relevance: 1.0, // Messages are fully relevant
                recency_seconds: recency,
                is_important: element_type == ContextElementType::System,
                access_count: 1,
                metadata: HashMap::new(),
            };

            elements.push(element);
        }

        // Step 2: Retrieve relevant memories if embedding provided
        if let Some(embedding) = query_embedding {
            self.stats.memory_retrievals.fetch_add(1, Ordering::SeqCst);

            let memories = self
                .memory
                .get_relevant(embedding, self.config.max_elements)?;

            for mem in memories {
                if mem.score < self.config.min_relevance {
                    continue;
                }

                let estimated_tokens = self.estimate_tokens(&mem.content);
                let element = ContextElement {
                    id: mem.id.clone(),
                    element_type: ContextElementType::Memory,
                    content: mem.content,
                    estimated_tokens,
                    priority: 0.0, // Will be scored
                    relevance: mem.score,
                    recency_seconds: 3600, // Default 1 hour for memories
                    is_important: false,
                    access_count: 1,
                    metadata: mem.metadata,
                };

                elements.push(element);
            }
        }

        // Step 3: Check semantic cache for tool results
        if let Some(embedding) = query_embedding {
            if let Some(cached) = self.cache.get(embedding)? {
                self.stats.cache_hits.fetch_add(1, Ordering::SeqCst);

                let estimated_tokens = self.estimate_tokens(&cached.result);
                let element = ContextElement {
                    id: format!("cache-{}", cached.tool_name),
                    element_type: ContextElementType::Cached,
                    content: format!("[Cached {}] {}", cached.tool_name, cached.result),
                    estimated_tokens,
                    priority: 0.7,
                    relevance: cached.similarity,
                    recency_seconds: (now - cached.cached_at).num_seconds(),
                    is_important: false,
                    access_count: cached.access_count,
                    metadata: HashMap::new(),
                };

                elements.push(element);
            }
        }

        // Step 4: Score and sort elements
        for element in &mut elements {
            if element.priority == 0.0 {
                element.priority = self.scorer.score(element);
            }
        }

        elements.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 5: Fit elements within budget
        let mut total_tokens = 0usize;
        let mut included = Vec::new();
        let mut summarized_count = 0usize;
        let mut excluded_count = 0usize;

        for element in elements {
            if total_tokens + element.estimated_tokens <= budget {
                total_tokens += element.estimated_tokens;
                included.push(element);
            } else if self.config.enable_summarization && element.priority > 0.5 {
                // Try to summarize and include
                let remaining_budget = budget - total_tokens;
                if remaining_budget > 50 {
                    // At least 50 tokens
                    let summarized_content = self.summarizer.summarize(
                        &element.content,
                        remaining_budget,
                        self.config.chars_per_token,
                    );
                    let summarized_tokens = self.estimate_tokens(&summarized_content);

                    if summarized_tokens <= remaining_budget {
                        let mut summarized_element = element;
                        summarized_element.content = summarized_content;
                        summarized_element.estimated_tokens = summarized_tokens;
                        total_tokens += summarized_tokens;
                        included.push(summarized_element);
                        summarized_count += 1;
                        self.stats.summarizations.fetch_add(1, Ordering::SeqCst);
                    } else {
                        excluded_count += 1;
                    }
                } else {
                    excluded_count += 1;
                }
            } else {
                excluded_count += 1;
            }
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.stats
            .total_tokens
            .fetch_add(total_tokens as u64, Ordering::SeqCst);
        self.stats
            .total_time_us
            .fetch_add(elapsed, Ordering::SeqCst);

        Ok(PreparedContext {
            elements: included,
            total_tokens,
            budget_used: total_tokens as f32 / budget as f32,
            summarized_count,
            excluded_count,
            preparation_time_us: elapsed,
        })
    }

    /// Get memory reference
    pub fn memory(&self) -> &AgenticMemory {
        &self.memory
    }

    /// Get mutable memory reference
    pub fn memory_mut(&mut self) -> &mut AgenticMemory {
        &mut self.memory
    }

    /// Get cache reference
    pub fn cache(&self) -> &SemanticToolCache {
        &self.cache
    }

    /// Store in memory
    pub fn store_memory(
        &self,
        key: &str,
        content: &str,
        embedding: Vec<f32>,
        memory_type: MemoryType,
    ) -> Result<String> {
        self.memory.store(key, content, embedding, memory_type)
    }

    /// Cache tool result
    pub fn cache_tool_result(
        &self,
        tool_name: &str,
        input: &str,
        result: &str,
        embedding: Vec<f32>,
    ) -> Result<()> {
        self.cache.store(tool_name, input, result, embedding)
    }

    /// Get statistics
    pub fn stats(&self) -> ContextManagerStats {
        let preps = self.stats.preparations.load(Ordering::SeqCst);
        let total_tokens = self.stats.total_tokens.load(Ordering::SeqCst);
        let total_time = self.stats.total_time_us.load(Ordering::SeqCst);

        ContextManagerStats {
            total_preparations: preps,
            avg_tokens: if preps > 0 { total_tokens / preps } else { 0 },
            avg_preparation_time_us: if preps > 0 { total_time / preps } else { 0 },
            summarizations: self.stats.summarizations.load(Ordering::SeqCst),
            cache_hits: self.stats.cache_hits.load(Ordering::SeqCst),
            memory_retrievals: self.stats.memory_retrievals.load(Ordering::SeqCst),
        }
    }

    /// Estimate tokens for content
    fn estimate_tokens(&self, content: &str) -> usize {
        (content.len() as f32 / self.config.chars_per_token).ceil() as usize
    }
}

/// A message for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    /// System message
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ContextManagerConfig {
        ContextManagerConfig {
            memory: AgenticMemoryConfig {
                semantic_dim: 128,
                episodic: super::super::episodic_memory::EpisodicMemoryConfig {
                    embedding_dim: 128,
                    ..Default::default()
                },
                ..Default::default()
            },
            cache: SemanticCacheConfig {
                embedding_dim: 128,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_context_manager_creation() {
        let config = test_config();
        let manager = IntelligentContextManager::new(config).unwrap();
        assert_eq!(manager.stats().total_preparations, 0);
    }

    #[test]
    fn test_prepare_context_basic() {
        let config = test_config();
        let manager = IntelligentContextManager::new(config).unwrap();

        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                timestamp: Utc::now(),
            },
            Message {
                role: MessageRole::User,
                content: "Hello!".to_string(),
                timestamp: Utc::now(),
            },
        ];

        let prepared = manager.prepare_context(&messages, None, None).unwrap();

        assert_eq!(prepared.elements.len(), 2);
        assert!(prepared.total_tokens > 0);
        assert!(prepared.budget_used < 1.0);
    }

    #[test]
    fn test_prepare_context_with_memory() {
        let config = test_config();
        let manager = IntelligentContextManager::new(config).unwrap();

        // Store some memory
        let embedding = vec![0.1; 128];
        manager
            .store_memory(
                "fact-1",
                "Test fact",
                embedding.clone(),
                MemoryType::Semantic,
            )
            .unwrap();

        let messages = vec![Message {
            role: MessageRole::User,
            content: "Tell me about the test.".to_string(),
            timestamp: Utc::now(),
        }];

        let prepared = manager
            .prepare_context(&messages, Some(&embedding), None)
            .unwrap();

        // Should include the message and memory
        assert!(prepared.elements.len() >= 1);
    }

    #[test]
    fn test_priority_scorer() {
        let scorer = PriorityScorer::new(PriorityWeights::default());

        let element = ContextElement {
            id: "test".to_string(),
            element_type: ContextElementType::Memory,
            content: "Test content".to_string(),
            estimated_tokens: 10,
            priority: 0.0,
            relevance: 0.9,
            recency_seconds: 60,
            is_important: true,
            access_count: 10,
            metadata: HashMap::new(),
        };

        let score = scorer.score(&element);
        assert!(score > 0.5);
        assert!(score <= 1.0);

        let priority = scorer.assign_priority(score);
        assert!(matches!(
            priority,
            ElementPriority::High | ElementPriority::Critical
        ));
    }

    #[test]
    fn test_memory_summarizer() {
        let summarizer = MemorySummarizer::new(0.5);

        let long_content = "A".repeat(1000);
        let summarized = summarizer.summarize(&long_content, 50, 4.0);

        assert!(summarized.len() < long_content.len());
        assert!(summarized.contains("..."));
    }

    #[test]
    fn test_model_token_limits() {
        assert_eq!(ModelTokenLimit::Haiku.max_tokens(), 200_000);
        assert_eq!(ModelTokenLimit::Sonnet.max_tokens(), 200_000);
        assert_eq!(ModelTokenLimit::Opus.max_tokens(), 200_000);
        assert_eq!(ModelTokenLimit::Custom(100_000).max_tokens(), 100_000);

        assert!(ModelTokenLimit::Sonnet.context_budget() < ModelTokenLimit::Sonnet.max_tokens());
    }
}
