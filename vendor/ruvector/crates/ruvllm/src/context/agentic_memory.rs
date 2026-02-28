//! Agentic Memory - Unified memory system combining multiple memory types
//!
//! Combines working memory, episodic memory, semantic memory, and procedural memory
//! into a unified interface for AI agents.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
use ruvector_core::types::{DistanceMetric, HnswConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Result, RuvLLMError};

use super::episodic_memory::{Episode, EpisodicMemory, EpisodicMemoryConfig, Trajectory};
use super::working_memory::{TaskContext, WorkingMemory, WorkingMemoryConfig};

/// Configuration for agentic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticMemoryConfig {
    /// Working memory configuration
    pub working: WorkingMemoryConfig,
    /// Episodic memory configuration
    pub episodic: EpisodicMemoryConfig,
    /// Embedding dimension for semantic memory
    pub semantic_dim: usize,
    /// Maximum semantic facts
    pub max_semantic_facts: usize,
    /// Maximum procedural skills
    pub max_procedural_skills: usize,
    /// HNSW M parameter for semantic index
    pub semantic_hnsw_m: usize,
    /// HNSW ef_construction for semantic index
    pub semantic_hnsw_ef_construction: usize,
    /// HNSW ef_search for semantic index
    pub semantic_hnsw_ef_search: usize,
    /// Enable memory consolidation
    pub enable_consolidation: bool,
    /// Consolidation threshold (minimum episodes before consolidation)
    pub consolidation_threshold: usize,
}

impl Default for AgenticMemoryConfig {
    fn default() -> Self {
        Self {
            working: WorkingMemoryConfig::default(),
            episodic: EpisodicMemoryConfig::default(),
            semantic_dim: 768,
            max_semantic_facts: 10_000,
            max_procedural_skills: 1_000,
            semantic_hnsw_m: 16,
            semantic_hnsw_ef_construction: 100,
            semantic_hnsw_ef_search: 50,
            enable_consolidation: true,
            consolidation_threshold: 100,
        }
    }
}

/// Type of memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    /// Short-term working memory
    Working,
    /// Long-term episodic memory (trajectories)
    Episodic,
    /// Semantic memory (facts and knowledge)
    Semantic,
    /// Procedural memory (skills and action sequences)
    Procedural,
}

/// A semantic fact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFact {
    /// Fact ID
    pub id: String,
    /// Fact content
    pub content: String,
    /// Fact embedding
    pub embedding: Vec<f32>,
    /// Confidence score
    pub confidence: f32,
    /// Source (where this fact came from)
    pub source: String,
    /// Related facts
    pub related: Vec<String>,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Access count
    pub access_count: u64,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,
}

/// A procedural skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralSkill {
    /// Skill ID
    pub id: String,
    /// Skill name
    pub name: String,
    /// Skill description
    pub description: String,
    /// Action sequence
    pub actions: Vec<SkillAction>,
    /// Trigger conditions (when to use this skill)
    pub triggers: Vec<String>,
    /// Skill embedding
    pub embedding: Vec<f32>,
    /// Success rate
    pub success_rate: f32,
    /// Execution count
    pub execution_count: u64,
    /// Average duration in milliseconds
    pub avg_duration_ms: u64,
    /// Tags
    pub tags: Vec<String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last used timestamp
    pub last_used: DateTime<Utc>,
}

/// An action in a procedural skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAction {
    /// Action type
    pub action_type: String,
    /// Action parameters
    pub params: HashMap<String, String>,
    /// Expected result pattern
    pub expected_result: Option<String>,
    /// Alternative actions if this fails
    pub fallback: Option<Box<SkillAction>>,
}

/// Unified agentic memory system
pub struct AgenticMemory {
    /// Configuration
    config: AgenticMemoryConfig,
    /// Working memory
    working: WorkingMemory,
    /// Episodic memory
    episodic: EpisodicMemory,
    /// Semantic memory index
    semantic_index: Arc<RwLock<HnswIndex>>,
    /// Semantic facts storage
    semantic_facts: Arc<RwLock<HashMap<String, SemanticFact>>>,
    /// Procedural memory index
    procedural_index: Arc<RwLock<HnswIndex>>,
    /// Procedural skills storage
    procedural_skills: Arc<RwLock<HashMap<String, ProceduralSkill>>>,
    /// Statistics
    stats: AgenticMemoryStatsInternal,
}

#[derive(Debug, Default)]
struct AgenticMemoryStatsInternal {
    stores: AtomicU64,
    retrievals: AtomicU64,
    pruning_ops: AtomicU64,
    consolidations: AtomicU64,
}

impl AgenticMemory {
    /// Create new agentic memory with configuration
    pub fn new(config: AgenticMemoryConfig) -> Result<Self> {
        let working = WorkingMemory::new(config.working.clone());
        let episodic = EpisodicMemory::new(config.episodic.clone())?;

        // Create semantic index
        let semantic_hnsw_config = HnswConfig {
            m: config.semantic_hnsw_m,
            ef_construction: config.semantic_hnsw_ef_construction,
            ef_search: config.semantic_hnsw_ef_search,
            max_elements: config.max_semantic_facts,
        };
        let semantic_index = HnswIndex::new(
            config.semantic_dim,
            DistanceMetric::Cosine,
            semantic_hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        // Create procedural index
        let procedural_hnsw_config = HnswConfig {
            m: config.semantic_hnsw_m,
            ef_construction: config.semantic_hnsw_ef_construction,
            ef_search: config.semantic_hnsw_ef_search,
            max_elements: config.max_procedural_skills,
        };
        let procedural_index = HnswIndex::new(
            config.semantic_dim,
            DistanceMetric::Cosine,
            procedural_hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        Ok(Self {
            config,
            working,
            episodic,
            semantic_index: Arc::new(RwLock::new(semantic_index)),
            semantic_facts: Arc::new(RwLock::new(HashMap::new())),
            procedural_index: Arc::new(RwLock::new(procedural_index)),
            procedural_skills: Arc::new(RwLock::new(HashMap::new())),
            stats: AgenticMemoryStatsInternal::default(),
        })
    }

    /// Store content in memory
    pub fn store(
        &self,
        key: &str,
        content: &str,
        embedding: Vec<f32>,
        memory_type: MemoryType,
    ) -> Result<String> {
        self.stats.stores.fetch_add(1, Ordering::SeqCst);

        match memory_type {
            MemoryType::Working => {
                self.working
                    .set_variable(key, serde_json::json!({ "content": content }));
                Ok(key.to_string())
            }
            MemoryType::Episodic => {
                // Create a simple trajectory for storage
                let trajectory = Trajectory {
                    id: key.to_string(),
                    steps: vec![],
                    outcome: 1.0,
                    quality_score: 1.0,
                    task_type: "storage".to_string(),
                    agent_type: None,
                    duration_ms: 0,
                    created_at: Utc::now(),
                };
                self.episodic.store_episode(trajectory, embedding, vec![])?;
                Ok(key.to_string())
            }
            MemoryType::Semantic => {
                self.store_semantic_fact(key, content, embedding, 1.0, "user", vec![])
            }
            MemoryType::Procedural => Err(RuvLLMError::InvalidOperation(
                "Use store_procedural_skill for procedural memory".to_string(),
            )),
        }
    }

    /// Store a semantic fact
    pub fn store_semantic_fact(
        &self,
        id: &str,
        content: &str,
        embedding: Vec<f32>,
        confidence: f32,
        source: &str,
        tags: Vec<String>,
    ) -> Result<String> {
        let fact_id = if id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            id.to_string()
        };

        let now = Utc::now();
        let fact = SemanticFact {
            id: fact_id.clone(),
            content: content.to_string(),
            embedding: embedding.clone(),
            confidence,
            source: source.to_string(),
            related: vec![],
            tags,
            access_count: 0,
            created_at: now,
            last_accessed: now,
        };

        // Add to index
        {
            let mut index = self.semantic_index.write();
            index.add(fact_id.clone(), embedding)?;
        }

        // Store fact
        {
            let mut facts = self.semantic_facts.write();
            facts.insert(fact_id.clone(), fact);
        }

        // Enforce limit
        self.enforce_semantic_limit()?;

        Ok(fact_id)
    }

    /// Store a procedural skill
    pub fn store_procedural_skill(&self, skill: ProceduralSkill) -> Result<String> {
        let skill_id = skill.id.clone();
        let embedding = skill.embedding.clone();

        // Add to index
        {
            let mut index = self.procedural_index.write();
            index.add(skill_id.clone(), embedding)?;
        }

        // Store skill
        {
            let mut skills = self.procedural_skills.write();
            skills.insert(skill_id.clone(), skill);
        }

        // Enforce limit
        self.enforce_procedural_limit()?;

        Ok(skill_id)
    }

    /// Retrieve from memory by query
    pub fn retrieve(
        &self,
        query_embedding: &[f32],
        memory_type: MemoryType,
        k: usize,
    ) -> Result<Vec<RetrievedMemory>> {
        self.stats.retrievals.fetch_add(1, Ordering::SeqCst);

        match memory_type {
            MemoryType::Working => {
                let entries = self.working.search_scratchpad(query_embedding, k);
                Ok(entries
                    .into_iter()
                    .map(|e| RetrievedMemory {
                        id: format!("scratchpad-{}", e.timestamp.timestamp()),
                        content: e.content,
                        memory_type: MemoryType::Working,
                        score: 0.0, // No score for working memory
                        metadata: HashMap::new(),
                    })
                    .collect())
            }
            MemoryType::Episodic => {
                let episodes = self.episodic.search_similar(query_embedding, k)?;
                Ok(episodes
                    .into_iter()
                    .map(|e| RetrievedMemory {
                        id: e.id.clone(),
                        content: e
                            .compressed
                            .as_ref()
                            .map(|c| c.summary.clone())
                            .unwrap_or_else(|| format!("Episode: {} steps", e.metadata.step_count)),
                        memory_type: MemoryType::Episodic,
                        score: e.metadata.quality_score,
                        metadata: {
                            let mut m = HashMap::new();
                            m.insert("task_type".to_string(), e.metadata.task_type);
                            m.insert("outcome".to_string(), e.metadata.outcome.to_string());
                            m
                        },
                    })
                    .collect())
            }
            MemoryType::Semantic => {
                let results = {
                    let index = self.semantic_index.read();
                    index.search(query_embedding, k)?
                };

                let facts = self.semantic_facts.read();
                Ok(results
                    .into_iter()
                    .filter_map(|r| {
                        facts.get(&r.id).map(|fact| RetrievedMemory {
                            id: fact.id.clone(),
                            content: fact.content.clone(),
                            memory_type: MemoryType::Semantic,
                            score: 1.0 - r.score, // Convert distance to similarity
                            metadata: {
                                let mut m = HashMap::new();
                                m.insert("source".to_string(), fact.source.clone());
                                m.insert("confidence".to_string(), fact.confidence.to_string());
                                m
                            },
                        })
                    })
                    .collect())
            }
            MemoryType::Procedural => {
                let results = {
                    let index = self.procedural_index.read();
                    index.search(query_embedding, k)?
                };

                let skills = self.procedural_skills.read();
                Ok(results
                    .into_iter()
                    .filter_map(|r| {
                        skills.get(&r.id).map(|skill| RetrievedMemory {
                            id: skill.id.clone(),
                            content: format!("{}: {}", skill.name, skill.description),
                            memory_type: MemoryType::Procedural,
                            score: skill.success_rate,
                            metadata: {
                                let mut m = HashMap::new();
                                m.insert(
                                    "execution_count".to_string(),
                                    skill.execution_count.to_string(),
                                );
                                m.insert(
                                    "success_rate".to_string(),
                                    skill.success_rate.to_string(),
                                );
                                m
                            },
                        })
                    })
                    .collect())
            }
        }
    }

    /// Get relevant memories across all types
    pub fn get_relevant(&self, query_embedding: &[f32], k: usize) -> Result<Vec<RetrievedMemory>> {
        let mut all_results = Vec::new();

        // Get from each memory type
        for mem_type in [
            MemoryType::Working,
            MemoryType::Episodic,
            MemoryType::Semantic,
            MemoryType::Procedural,
        ] {
            if let Ok(results) = self.retrieve(query_embedding, mem_type, k) {
                all_results.extend(results);
            }
        }

        // Sort by score and take top k
        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_results.truncate(k);

        Ok(all_results)
    }

    /// Prune low-relevance memories
    pub fn prune(&self) -> Result<PruneStats> {
        self.stats.pruning_ops.fetch_add(1, Ordering::SeqCst);

        // Prune working memory
        let working_prune = self.working.prune();

        // Compress old episodic memories
        let episodes_compressed = self.episodic.compress_old_episodes()?;

        Ok(PruneStats {
            working_pruned: working_prune.variables_removed + working_prune.tool_cache_expired,
            episodes_compressed,
            facts_pruned: 0,
            skills_pruned: 0,
        })
    }

    /// Consolidate episodic memories into semantic facts
    pub fn consolidate(&self) -> Result<ConsolidationResult> {
        if !self.config.enable_consolidation {
            return Ok(ConsolidationResult {
                facts_created: 0,
                skills_created: 0,
                patterns_found: 0,
            });
        }

        self.stats.consolidations.fetch_add(1, Ordering::SeqCst);

        let episodic_stats = self.episodic.stats();
        if episodic_stats.total_episodes < self.config.consolidation_threshold as u64 {
            return Ok(ConsolidationResult {
                facts_created: 0,
                skills_created: 0,
                patterns_found: 0,
            });
        }

        // This is a simplified consolidation - in production, use clustering and pattern extraction
        // For now, we just mark that consolidation would happen
        Ok(ConsolidationResult {
            facts_created: 0,
            skills_created: 0,
            patterns_found: 0,
        })
    }

    /// Get working memory reference
    pub fn working(&self) -> &WorkingMemory {
        &self.working
    }

    /// Get episodic memory reference
    pub fn episodic(&self) -> &EpisodicMemory {
        &self.episodic
    }

    /// Get semantic fact by ID
    pub fn get_semantic_fact(&self, id: &str) -> Option<SemanticFact> {
        self.semantic_facts.read().get(id).cloned()
    }

    /// Get procedural skill by ID
    pub fn get_procedural_skill(&self, id: &str) -> Option<ProceduralSkill> {
        self.procedural_skills.read().get(id).cloned()
    }

    /// Set current task in working memory
    pub fn set_task(&self, task: TaskContext) {
        self.working.set_task(task);
    }

    /// Get current task from working memory
    pub fn get_task(&self) -> Option<TaskContext> {
        self.working.get_task()
    }

    /// Get memory statistics
    pub fn stats(&self) -> AgenticMemoryStats {
        let episodic_stats = self.episodic.stats();
        let working_stats = self.working.stats();

        AgenticMemoryStats {
            working: working_stats,
            episodic: episodic_stats,
            semantic_facts: self.semantic_facts.read().len(),
            procedural_skills: self.procedural_skills.read().len(),
            total_stores: self.stats.stores.load(Ordering::SeqCst),
            total_retrievals: self.stats.retrievals.load(Ordering::SeqCst),
            pruning_operations: self.stats.pruning_ops.load(Ordering::SeqCst),
            consolidations: self.stats.consolidations.load(Ordering::SeqCst),
        }
    }

    /// Clear all memories
    pub fn clear(&self) -> Result<()> {
        self.working.clear();
        self.episodic.clear()?;
        self.semantic_facts.write().clear();
        self.procedural_skills.write().clear();

        // Recreate indices
        let semantic_hnsw_config = HnswConfig {
            m: self.config.semantic_hnsw_m,
            ef_construction: self.config.semantic_hnsw_ef_construction,
            ef_search: self.config.semantic_hnsw_ef_search,
            max_elements: self.config.max_semantic_facts,
        };
        *self.semantic_index.write() = HnswIndex::new(
            self.config.semantic_dim,
            DistanceMetric::Cosine,
            semantic_hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        let procedural_hnsw_config = HnswConfig {
            m: self.config.semantic_hnsw_m,
            ef_construction: self.config.semantic_hnsw_ef_construction,
            ef_search: self.config.semantic_hnsw_ef_search,
            max_elements: self.config.max_procedural_skills,
        };
        *self.procedural_index.write() = HnswIndex::new(
            self.config.semantic_dim,
            DistanceMetric::Cosine,
            procedural_hnsw_config,
        )
        .map_err(|e| RuvLLMError::Ruvector(e.to_string()))?;

        Ok(())
    }

    /// Enforce semantic facts limit
    fn enforce_semantic_limit(&self) -> Result<()> {
        let mut facts = self.semantic_facts.write();

        while facts.len() > self.config.max_semantic_facts {
            // Remove least accessed fact
            if let Some(oldest_id) = facts
                .iter()
                .min_by_key(|(_, f)| f.access_count)
                .map(|(id, _)| id.clone())
            {
                facts.remove(&oldest_id);
                let mut index = self.semantic_index.write();
                let _ = index.remove(&oldest_id);
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Enforce procedural skills limit
    fn enforce_procedural_limit(&self) -> Result<()> {
        let mut skills = self.procedural_skills.write();

        while skills.len() > self.config.max_procedural_skills {
            // Remove least successful skill
            if let Some(worst_id) = skills
                .iter()
                .min_by(|(_, a), (_, b)| {
                    a.success_rate
                        .partial_cmp(&b.success_rate)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(id, _)| id.clone())
            {
                skills.remove(&worst_id);
                let mut index = self.procedural_index.write();
                let _ = index.remove(&worst_id);
            } else {
                break;
            }
        }

        Ok(())
    }
}

/// Retrieved memory item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedMemory {
    /// Memory ID
    pub id: String,
    /// Memory content
    pub content: String,
    /// Memory type
    pub memory_type: MemoryType,
    /// Relevance score
    pub score: f32,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Statistics from pruning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneStats {
    /// Working memory items pruned
    pub working_pruned: usize,
    /// Episodic memories compressed
    pub episodes_compressed: usize,
    /// Semantic facts pruned
    pub facts_pruned: usize,
    /// Procedural skills pruned
    pub skills_pruned: usize,
}

/// Result of consolidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    /// New semantic facts created
    pub facts_created: usize,
    /// New procedural skills created
    pub skills_created: usize,
    /// Patterns found in episodes
    pub patterns_found: usize,
}

/// Agentic memory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticMemoryStats {
    /// Working memory stats
    pub working: super::working_memory::WorkingMemoryStats,
    /// Episodic memory stats
    pub episodic: super::episodic_memory::EpisodicMemoryStats,
    /// Number of semantic facts
    pub semantic_facts: usize,
    /// Number of procedural skills
    pub procedural_skills: usize,
    /// Total store operations
    pub total_stores: u64,
    /// Total retrieval operations
    pub total_retrievals: u64,
    /// Pruning operations performed
    pub pruning_operations: u64,
    /// Consolidation operations performed
    pub consolidations: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_embedding(dim: usize) -> Vec<f32> {
        vec![0.1; dim]
    }

    #[test]
    fn test_agentic_memory_creation() {
        let config = AgenticMemoryConfig {
            semantic_dim: 128,
            episodic: EpisodicMemoryConfig {
                embedding_dim: 128,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = AgenticMemory::new(config).unwrap();
        assert_eq!(memory.stats().semantic_facts, 0);
    }

    #[test]
    fn test_store_and_retrieve_semantic() {
        let config = AgenticMemoryConfig {
            semantic_dim: 128,
            episodic: EpisodicMemoryConfig {
                embedding_dim: 128,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = AgenticMemory::new(config).unwrap();

        let embedding = test_embedding(128);
        memory
            .store_semantic_fact(
                "fact-1",
                "Rust is a systems programming language",
                embedding.clone(),
                0.9,
                "user",
                vec!["rust".to_string()],
            )
            .unwrap();

        let results = memory
            .retrieve(&embedding, MemoryType::Semantic, 5)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Rust"));
    }

    #[test]
    fn test_store_and_retrieve_procedural() {
        let config = AgenticMemoryConfig {
            semantic_dim: 128,
            episodic: EpisodicMemoryConfig {
                embedding_dim: 128,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = AgenticMemory::new(config).unwrap();

        let embedding = test_embedding(128);
        let skill = ProceduralSkill {
            id: "skill-1".to_string(),
            name: "Read and Edit File".to_string(),
            description: "Read a file, make changes, write back".to_string(),
            actions: vec![
                SkillAction {
                    action_type: "read_file".to_string(),
                    params: HashMap::new(),
                    expected_result: Some("file contents".to_string()),
                    fallback: None,
                },
                SkillAction {
                    action_type: "edit_file".to_string(),
                    params: HashMap::new(),
                    expected_result: Some("success".to_string()),
                    fallback: None,
                },
            ],
            triggers: vec!["edit".to_string(), "modify".to_string()],
            embedding: embedding.clone(),
            success_rate: 0.95,
            execution_count: 100,
            avg_duration_ms: 500,
            tags: vec!["file".to_string()],
            created_at: Utc::now(),
            last_used: Utc::now(),
        };

        memory.store_procedural_skill(skill).unwrap();

        let results = memory
            .retrieve(&embedding, MemoryType::Procedural, 5)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Read and Edit"));
    }

    #[test]
    fn test_get_relevant() {
        let config = AgenticMemoryConfig {
            semantic_dim: 128,
            episodic: EpisodicMemoryConfig {
                embedding_dim: 128,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = AgenticMemory::new(config).unwrap();

        let embedding = test_embedding(128);
        memory
            .store_semantic_fact(
                "fact-1",
                "Test fact",
                embedding.clone(),
                0.9,
                "user",
                vec![],
            )
            .unwrap();

        let results = memory.get_relevant(&embedding, 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_clear() {
        let config = AgenticMemoryConfig {
            semantic_dim: 128,
            episodic: EpisodicMemoryConfig {
                embedding_dim: 128,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = AgenticMemory::new(config).unwrap();

        let embedding = test_embedding(128);
        memory
            .store_semantic_fact("fact-1", "Test", embedding, 0.9, "user", vec![])
            .unwrap();

        assert_eq!(memory.stats().semantic_facts, 1);
        memory.clear().unwrap();
        assert_eq!(memory.stats().semantic_facts, 0);
    }
}
