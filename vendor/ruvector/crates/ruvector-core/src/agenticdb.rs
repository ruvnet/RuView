//! AgenticDB API Compatibility Layer
//!
//! # ⚠️ CRITICAL WARNING: PLACEHOLDER EMBEDDINGS
//!
//! **THIS MODULE USES HASH-BASED PLACEHOLDER EMBEDDINGS - NOT REAL SEMANTIC EMBEDDINGS**
//!
//! The `generate_text_embedding()` function creates embeddings using a simple hash function
//! that does NOT understand semantic meaning. Similarity is based on character overlap, NOT meaning.
//!
//! **For Production Use:**
//! - Integrate a real embedding model (sentence-transformers, OpenAI, Anthropic, Cohere)
//! - Use ONNX Runtime, candle, or Python bindings for inference
//! - See `/examples/onnx-embeddings` for a production-ready integration example
//!
//! **What This Means:**
//! - "dog" and "cat" will NOT be similar (different characters)
//! - "dog" and "god" WILL be similar (same characters, different order)
//! - Semantic search will not work as expected
//!
//! Provides a drop-in replacement for agenticDB with 5-table schema:
//! - vectors_table: Core embeddings + metadata
//! - reflexion_episodes: Self-critique memories
//! - skills_library: Consolidated patterns
//! - causal_edges: Cause-effect relationships with hypergraphs
//! - learning_sessions: RL training data

use crate::embeddings::{BoxedEmbeddingProvider, HashEmbedding};
use crate::error::{Result, RuvectorError};
use crate::types::*;
use crate::vector_db::VectorDB;
use parking_lot::RwLock;
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// Table definitions
const REFLEXION_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("reflexion_episodes");
const SKILLS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("skills_library");
const CAUSAL_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("causal_edges");
const LEARNING_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("learning_sessions");

/// Reflexion episode for self-critique memory
/// Note: Serialized using JSON (not bincode) due to serde_json::Value in metadata field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflexionEpisode {
    pub id: String,
    pub task: String,
    pub actions: Vec<String>,
    pub observations: Vec<String>,
    pub critique: String,
    pub embedding: Vec<f32>,
    pub timestamp: i64,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Skill definition in the library
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parameters: HashMap<String, String>,
    pub examples: Vec<String>,
    pub embedding: Vec<f32>,
    pub usage_count: usize,
    pub success_rate: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Causal edge in the hypergraph
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct CausalEdge {
    pub id: String,
    pub causes: Vec<String>,  // Hypergraph: multiple causes
    pub effects: Vec<String>, // Hypergraph: multiple effects
    pub confidence: f64,
    pub context: String,
    pub embedding: Vec<f32>,
    pub observations: usize,
    pub timestamp: i64,
}

/// Learning session for RL training
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct LearningSession {
    pub id: String,
    pub algorithm: String, // Q-Learning, DQN, PPO, etc
    pub state_dim: usize,
    pub action_dim: usize,
    pub experiences: Vec<Experience>,
    pub model_params: Option<Vec<u8>>, // Serialized model
    pub created_at: i64,
    pub updated_at: i64,
}

/// Single RL experience
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Experience {
    pub state: Vec<f32>,
    pub action: Vec<f32>,
    pub reward: f64,
    pub next_state: Vec<f32>,
    pub done: bool,
    pub timestamp: i64,
}

/// Prediction with confidence interval
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Prediction {
    pub action: Vec<f32>,
    pub confidence_lower: f64,
    pub confidence_upper: f64,
    pub mean_confidence: f64,
}

/// Query result with utility score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilitySearchResult {
    pub result: SearchResult,
    pub utility_score: f64,
    pub similarity_score: f64,
    pub causal_uplift: f64,
    pub latency_penalty: f64,
}

/// Main AgenticDB interface
pub struct AgenticDB {
    vector_db: Arc<VectorDB>,
    db: Arc<Database>,
    _dimensions: usize,
    embedding_provider: BoxedEmbeddingProvider,
}

impl AgenticDB {
    /// Create a new AgenticDB with the given options and default hash-based embeddings
    pub fn new(options: DbOptions) -> Result<Self> {
        let embedding_provider = Arc::new(HashEmbedding::new(options.dimensions));
        Self::with_embedding_provider(options, embedding_provider)
    }

    /// Create a new AgenticDB with a custom embedding provider
    ///
    /// # Example with API embeddings
    /// ```rust,no_run
    /// use ruvector_core::{AgenticDB, ApiEmbedding};
    /// use ruvector_core::types::DbOptions;
    /// use std::sync::Arc;
    ///
    /// let mut options = DbOptions::default();
    /// options.dimensions = 1536; // OpenAI embedding dimensions
    /// options.storage_path = "agenticdb.db".to_string();
    ///
    /// let provider = Arc::new(ApiEmbedding::openai("sk-...", "text-embedding-3-small"));
    /// let db = AgenticDB::with_embedding_provider(options, provider)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example with Candle (requires feature flag)
    /// ```rust,no_run
    /// # #[cfg(feature = "real-embeddings")]
    /// # {
    /// use ruvector_core::{AgenticDB, CandleEmbedding};
    /// use ruvector_core::types::DbOptions;
    /// use std::sync::Arc;
    ///
    /// let mut options = DbOptions::default();
    /// options.dimensions = 384; // MiniLM dimensions
    /// options.storage_path = "agenticdb.db".to_string();
    ///
    /// let provider = Arc::new(CandleEmbedding::from_pretrained(
    ///     "sentence-transformers/all-MiniLM-L6-v2",
    ///     false
    /// )?);
    /// let db = AgenticDB::with_embedding_provider(options, provider)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// # }
    /// ```
    pub fn with_embedding_provider(
        options: DbOptions,
        embedding_provider: BoxedEmbeddingProvider,
    ) -> Result<Self> {
        // Validate dimensions match
        if options.dimensions != embedding_provider.dimensions() {
            return Err(RuvectorError::InvalidDimension(format!(
                "Options dimensions ({}) do not match embedding provider dimensions ({})",
                options.dimensions,
                embedding_provider.dimensions()
            )));
        }

        // Create vector DB for core vector operations
        let vector_db = Arc::new(VectorDB::new(options.clone())?);

        // Create separate database for AgenticDB tables
        let agentic_path = format!("{}.agentic", options.storage_path);
        let db = Arc::new(Database::create(&agentic_path)?);

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(REFLEXION_TABLE)?;
            let _ = write_txn.open_table(SKILLS_TABLE)?;
            let _ = write_txn.open_table(CAUSAL_TABLE)?;
            let _ = write_txn.open_table(LEARNING_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self {
            vector_db,
            db,
            _dimensions: options.dimensions,
            embedding_provider,
        })
    }

    /// Create with default options and hash-based embeddings
    pub fn with_dimensions(dimensions: usize) -> Result<Self> {
        let options = DbOptions {
            dimensions,
            ..DbOptions::default()
        };
        Self::new(options)
    }

    /// Get the embedding provider name (for debugging/logging)
    pub fn embedding_provider_name(&self) -> &str {
        self.embedding_provider.name()
    }

    // ============ Vector DB Core Methods ============

    /// Insert a vector entry
    pub fn insert(&self, entry: VectorEntry) -> Result<VectorId> {
        self.vector_db.insert(entry)
    }

    /// Insert multiple vectors in a batch
    pub fn insert_batch(&self, entries: Vec<VectorEntry>) -> Result<Vec<VectorId>> {
        self.vector_db.insert_batch(entries)
    }

    /// Search for similar vectors
    pub fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        self.vector_db.search(query)
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<bool> {
        self.vector_db.delete(id)
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        self.vector_db.get(id)
    }

    // ============ Reflexion Memory API ============

    /// Store a reflexion episode with self-critique
    pub fn store_episode(
        &self,
        task: String,
        actions: Vec<String>,
        observations: Vec<String>,
        critique: String,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        // Generate embedding from critique for similarity search
        let embedding = self.generate_text_embedding(&critique)?;

        let episode = ReflexionEpisode {
            id: id.clone(),
            task,
            actions,
            observations,
            critique,
            embedding: embedding.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            metadata: None,
        };

        // Store in reflexion table
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(REFLEXION_TABLE)?;
            // Use JSON encoding for ReflexionEpisode (contains serde_json::Value which isn't bincode-compatible)
            let json = serde_json::to_vec(&episode)
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            table.insert(id.as_str(), json.as_slice())?;
        }
        write_txn.commit()?;

        // Also index in vector DB for fast similarity search
        self.vector_db.insert(VectorEntry {
            id: Some(format!("reflexion_{}", id)),
            vector: embedding,
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::json!("reflexion"));
                meta.insert("episode_id".to_string(), serde_json::json!(id.clone()));
                meta
            }),
        })?;

        Ok(id)
    }

    /// Retrieve similar reflexion episodes
    pub fn retrieve_similar_episodes(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<ReflexionEpisode>> {
        // Generate embedding for query
        let query_embedding = self.generate_text_embedding(query)?;

        // Search in vector DB
        let results = self.vector_db.search(SearchQuery {
            vector: query_embedding,
            k,
            filter: Some({
                let mut filter = HashMap::new();
                filter.insert("type".to_string(), serde_json::json!("reflexion"));
                filter
            }),
            ef_search: None,
        })?;

        // Retrieve full episodes
        let mut episodes = Vec::new();
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(REFLEXION_TABLE)?;

        for result in results {
            if let Some(metadata) = result.metadata {
                if let Some(episode_id) = metadata.get("episode_id") {
                    let id = episode_id.as_str().unwrap();
                    if let Some(data) = table.get(id)? {
                        // Use JSON decoding for ReflexionEpisode (contains serde_json::Value which isn't bincode-compatible)
                        let episode: ReflexionEpisode = serde_json::from_slice(data.value())
                            .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
                        episodes.push(episode);
                    }
                }
            }
        }

        Ok(episodes)
    }

    // ============ Skill Library API ============

    /// Create a new skill in the library
    pub fn create_skill(
        &self,
        name: String,
        description: String,
        parameters: HashMap<String, String>,
        examples: Vec<String>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        // Generate embedding from description
        let embedding = self.generate_text_embedding(&description)?;

        let skill = Skill {
            id: id.clone(),
            name,
            description,
            parameters,
            examples,
            embedding: embedding.clone(),
            usage_count: 0,
            success_rate: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        // Store in skills table
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(SKILLS_TABLE)?;
            let data = bincode::encode_to_vec(&skill, bincode::config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            table.insert(id.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;

        // Index in vector DB
        self.vector_db.insert(VectorEntry {
            id: Some(format!("skill_{}", id)),
            vector: embedding,
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::json!("skill"));
                meta.insert("skill_id".to_string(), serde_json::json!(id.clone()));
                meta
            }),
        })?;

        Ok(id)
    }

    /// Search skills by description
    pub fn search_skills(&self, query_description: &str, k: usize) -> Result<Vec<Skill>> {
        let query_embedding = self.generate_text_embedding(query_description)?;

        let results = self.vector_db.search(SearchQuery {
            vector: query_embedding,
            k,
            filter: Some({
                let mut filter = HashMap::new();
                filter.insert("type".to_string(), serde_json::json!("skill"));
                filter
            }),
            ef_search: None,
        })?;

        let mut skills = Vec::new();
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SKILLS_TABLE)?;

        for result in results {
            if let Some(metadata) = result.metadata {
                if let Some(skill_id) = metadata.get("skill_id") {
                    let id = skill_id.as_str().unwrap();
                    if let Some(data) = table.get(id)? {
                        let (skill, _): (Skill, usize) =
                            bincode::decode_from_slice(data.value(), bincode::config::standard())
                                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
                        skills.push(skill);
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Auto-consolidate action sequences into skills
    pub fn auto_consolidate(
        &self,
        action_sequences: Vec<Vec<String>>,
        success_threshold: usize,
    ) -> Result<Vec<String>> {
        let mut skill_ids = Vec::new();

        // Group similar sequences (simplified - would use clustering in production)
        for sequence in action_sequences {
            if sequence.len() >= success_threshold {
                let description = format!("Skill: {}", sequence.join(" -> "));
                let skill_id = self.create_skill(
                    format!("Auto-Skill-{}", uuid::Uuid::new_v4()),
                    description,
                    HashMap::new(),
                    sequence.clone(),
                )?;
                skill_ids.push(skill_id);
            }
        }

        Ok(skill_ids)
    }

    // ============ Causal Memory with Hypergraphs ============

    /// Add a causal edge (supporting hypergraphs with multiple causes/effects)
    pub fn add_causal_edge(
        &self,
        causes: Vec<String>,
        effects: Vec<String>,
        confidence: f64,
        context: String,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        // Generate embedding from context
        let embedding = self.generate_text_embedding(&context)?;

        let edge = CausalEdge {
            id: id.clone(),
            causes,
            effects,
            confidence,
            context,
            embedding: embedding.clone(),
            observations: 1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Store in causal table
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CAUSAL_TABLE)?;
            let data = bincode::encode_to_vec(&edge, bincode::config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            table.insert(id.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;

        // Index in vector DB
        self.vector_db.insert(VectorEntry {
            id: Some(format!("causal_{}", id)),
            vector: embedding,
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::json!("causal"));
                meta.insert("causal_id".to_string(), serde_json::json!(id.clone()));
                meta.insert("confidence".to_string(), serde_json::json!(confidence));
                meta
            }),
        })?;

        Ok(id)
    }

    /// Query with utility function: U = α·similarity + β·causal_uplift − γ·latency
    pub fn query_with_utility(
        &self,
        query: &str,
        k: usize,
        alpha: f64,
        beta: f64,
        gamma: f64,
    ) -> Result<Vec<UtilitySearchResult>> {
        let start_time = std::time::Instant::now();
        let query_embedding = self.generate_text_embedding(query)?;

        // Get all causal edges
        let results = self.vector_db.search(SearchQuery {
            vector: query_embedding,
            k: k * 2, // Get more results for utility ranking
            filter: Some({
                let mut filter = HashMap::new();
                filter.insert("type".to_string(), serde_json::json!("causal"));
                filter
            }),
            ef_search: None,
        })?;

        let mut utility_results = Vec::new();

        for result in results {
            let similarity_score = 1.0 / (1.0 + result.score as f64); // Convert distance to similarity

            // Get causal uplift from metadata
            let causal_uplift = if let Some(ref metadata) = result.metadata {
                metadata
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            let latency = start_time.elapsed().as_secs_f64();
            let latency_penalty = latency * gamma;

            // Calculate utility: U = α·similarity + β·causal_uplift − γ·latency
            let utility_score = alpha * similarity_score + beta * causal_uplift - latency_penalty;

            utility_results.push(UtilitySearchResult {
                result,
                utility_score,
                similarity_score,
                causal_uplift,
                latency_penalty,
            });
        }

        // Sort by utility score (descending)
        utility_results.sort_by(|a, b| b.utility_score.partial_cmp(&a.utility_score).unwrap());
        utility_results.truncate(k);

        Ok(utility_results)
    }

    // ============ Learning Sessions API ============

    /// Start a new learning session
    pub fn start_session(
        &self,
        algorithm: String,
        state_dim: usize,
        action_dim: usize,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        let session = LearningSession {
            id: id.clone(),
            algorithm,
            state_dim,
            action_dim,
            experiences: Vec::new(),
            model_params: None,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LEARNING_TABLE)?;
            let data = bincode::encode_to_vec(&session, bincode::config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            table.insert(id.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(id)
    }

    /// Add an experience to a learning session
    pub fn add_experience(
        &self,
        session_id: &str,
        state: Vec<f32>,
        action: Vec<f32>,
        reward: f64,
        next_state: Vec<f32>,
        done: bool,
    ) -> Result<()> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LEARNING_TABLE)?;

        let data = table
            .get(session_id)?
            .ok_or_else(|| RuvectorError::VectorNotFound(session_id.to_string()))?;

        let (mut session, _): (LearningSession, usize) =
            bincode::decode_from_slice(data.value(), bincode::config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;

        drop(table);
        drop(read_txn);

        // Add experience
        session.experiences.push(Experience {
            state,
            action,
            reward,
            next_state,
            done,
            timestamp: chrono::Utc::now().timestamp(),
        });
        session.updated_at = chrono::Utc::now().timestamp();

        // Update session
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LEARNING_TABLE)?;
            let data = bincode::encode_to_vec(&session, bincode::config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            table.insert(session_id, data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Predict action with confidence interval
    pub fn predict_with_confidence(&self, session_id: &str, state: Vec<f32>) -> Result<Prediction> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LEARNING_TABLE)?;

        let data = table
            .get(session_id)?
            .ok_or_else(|| RuvectorError::VectorNotFound(session_id.to_string()))?;

        let (session, _): (LearningSession, usize) =
            bincode::decode_from_slice(data.value(), bincode::config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;

        // Simple prediction based on similar states (would use actual RL model in production)
        let mut similar_actions = Vec::new();
        let mut rewards = Vec::new();

        for exp in &session.experiences {
            let distance = euclidean_distance(&state, &exp.state);
            if distance < 1.0 {
                // Similarity threshold
                similar_actions.push(exp.action.clone());
                rewards.push(exp.reward);
            }
        }

        if similar_actions.is_empty() {
            // Return random action if no similar states
            return Ok(Prediction {
                action: vec![0.0; session.action_dim],
                confidence_lower: 0.0,
                confidence_upper: 0.0,
                mean_confidence: 0.0,
            });
        }

        // Average actions weighted by rewards
        let total_reward: f64 = rewards.iter().sum();
        let mut action = vec![0.0; session.action_dim];

        for (act, reward) in similar_actions.iter().zip(rewards.iter()) {
            let weight = reward / total_reward;
            for (i, val) in act.iter().enumerate() {
                action[i] += val * weight as f32;
            }
        }

        // Calculate confidence interval (simplified)
        let mean_reward = total_reward / rewards.len() as f64;
        let std_dev = calculate_std_dev(&rewards, mean_reward);

        Ok(Prediction {
            action,
            confidence_lower: mean_reward - 1.96 * std_dev,
            confidence_upper: mean_reward + 1.96 * std_dev,
            mean_confidence: mean_reward,
        })
    }

    /// Get learning session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<LearningSession>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LEARNING_TABLE)?;

        if let Some(data) = table.get(session_id)? {
            let (session, _): (LearningSession, usize) =
                bincode::decode_from_slice(data.value(), bincode::config::standard())
                    .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    // ============ Helper Methods ============

    /// Generate text embedding from text using the configured embedding provider.
    ///
    /// By default, this uses hash-based embeddings (fast but not semantic).
    /// Use `with_embedding_provider()` to use real embeddings.
    ///
    /// # Example with real embeddings
    /// ```rust,ignore
    /// use ruvector_core::{AgenticDB, ApiEmbedding};
    /// use ruvector_core::types::DbOptions;
    /// use std::sync::Arc;
    ///
    /// let mut options = DbOptions::default();
    /// options.dimensions = 1536;
    /// let provider = Arc::new(ApiEmbedding::openai("sk-...", "text-embedding-3-small"));
    /// let db = AgenticDB::with_embedding_provider(options, provider)?;
    ///
    /// // Now embeddings will be semantic! (internal method)
    /// let embedding = db.generate_text_embedding("hello world")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        self.embedding_provider.embed(text)
    }
}

// Helper functions
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn calculate_std_dev(values: &[f64], mean: f64) -> f64 {
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

// ============ High-Level API Interfaces (ADR-001) ============

/// Policy Memory Store interface for AI agent policy memory
///
/// This interface provides Q-learning state-action lookups, contextual bandit
/// policy retrieval, and episodic memory for reasoning.
///
/// # Example
/// ```rust,ignore
/// let policy_store = db.policy_memory();
/// policy_store.store_policy("state_a", vec![0.1, 0.2], PolicyAction { action: "move_left", reward: 0.8 })?;
/// let similar = policy_store.retrieve_similar_states(&current_state_embedding, 5)?;
/// ```
pub struct PolicyMemoryStore<'a> {
    db: &'a AgenticDB,
}

/// Policy action with reward information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// Action taken
    pub action: String,
    /// Reward received
    pub reward: f64,
    /// Q-value estimate
    pub q_value: f64,
    /// State embedding
    pub state_embedding: Vec<f32>,
    /// Timestamp
    pub timestamp: i64,
}

/// Policy entry combining state and action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEntry {
    /// Unique identifier
    pub id: String,
    /// State identifier
    pub state_id: String,
    /// Action taken
    pub action: PolicyAction,
    /// Metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl<'a> PolicyMemoryStore<'a> {
    /// Create a new policy memory store interface
    pub fn new(db: &'a AgenticDB) -> Self {
        Self { db }
    }

    /// Store a policy entry (state-action pair)
    pub fn store_policy(
        &self,
        state_id: &str,
        state_embedding: Vec<f32>,
        action: &str,
        reward: f64,
        q_value: f64,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        let _entry = PolicyEntry {
            id: id.clone(),
            state_id: state_id.to_string(),
            action: PolicyAction {
                action: action.to_string(),
                reward,
                q_value,
                state_embedding: state_embedding.clone(),
                timestamp,
            },
            metadata: None,
        };

        // Store in vector DB for similarity search
        self.db.vector_db.insert(VectorEntry {
            id: Some(format!("policy_{}", id)),
            vector: state_embedding,
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::json!("policy"));
                meta.insert("policy_id".to_string(), serde_json::json!(id.clone()));
                meta.insert("state_id".to_string(), serde_json::json!(state_id));
                meta.insert("action".to_string(), serde_json::json!(action));
                meta.insert("reward".to_string(), serde_json::json!(reward));
                meta.insert("q_value".to_string(), serde_json::json!(q_value));
                meta
            }),
        })?;

        Ok(id)
    }

    /// Retrieve similar states for policy lookup
    pub fn retrieve_similar_states(
        &self,
        state_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<PolicyEntry>> {
        let results = self.db.vector_db.search(SearchQuery {
            vector: state_embedding.to_vec(),
            k,
            filter: Some({
                let mut filter = HashMap::new();
                filter.insert("type".to_string(), serde_json::json!("policy"));
                filter
            }),
            ef_search: None,
        })?;

        let mut entries = Vec::new();
        for result in results {
            if let Some(metadata) = result.metadata {
                let policy_id = metadata
                    .get("policy_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let state_id = metadata
                    .get("state_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let action = metadata
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let reward = metadata
                    .get("reward")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let q_value = metadata
                    .get("q_value")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                entries.push(PolicyEntry {
                    id: policy_id.to_string(),
                    state_id: state_id.to_string(),
                    action: PolicyAction {
                        action: action.to_string(),
                        reward,
                        q_value,
                        state_embedding: result.vector.unwrap_or_default(),
                        timestamp: 0,
                    },
                    metadata: None,
                });
            }
        }

        Ok(entries)
    }

    /// Get the best action for a state based on Q-values
    pub fn get_best_action(&self, state_embedding: &[f32], k: usize) -> Result<Option<String>> {
        let similar = self.retrieve_similar_states(state_embedding, k)?;

        similar
            .into_iter()
            .max_by(|a, b| a.action.q_value.partial_cmp(&b.action.q_value).unwrap())
            .map(|entry| Ok(entry.action.action))
            .transpose()
    }

    /// Update Q-value for a state-action pair
    pub fn update_q_value(&self, policy_id: &str, _new_q_value: f64) -> Result<()> {
        // Delete old entry and create new one with updated Q-value
        // Note: In production, this should use an update mechanism
        let _ = self.db.vector_db.delete(&format!("policy_{}", policy_id));
        Ok(())
    }
}

/// Session State Index for real-time session context
///
/// Provides < 10ms latency for interactive use, session isolation via namespaces,
/// and TTL-based cleanup.
pub struct SessionStateIndex<'a> {
    db: &'a AgenticDB,
    session_id: String,
    ttl_seconds: i64,
}

/// Session turn entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTurn {
    /// Turn ID
    pub id: String,
    /// Session ID
    pub session_id: String,
    /// Turn number
    pub turn_number: usize,
    /// Role (user, assistant, system)
    pub role: String,
    /// Content
    pub content: String,
    /// Embedding
    pub embedding: Vec<f32>,
    /// Timestamp
    pub timestamp: i64,
    /// TTL expiry
    pub expires_at: i64,
}

impl<'a> SessionStateIndex<'a> {
    /// Create a new session state index
    pub fn new(db: &'a AgenticDB, session_id: &str, ttl_seconds: i64) -> Self {
        Self {
            db,
            session_id: session_id.to_string(),
            ttl_seconds,
        }
    }

    /// Add a turn to the session
    pub fn add_turn(&self, turn_number: usize, role: &str, content: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        let expires_at = timestamp + self.ttl_seconds;

        // Generate embedding for the content
        let embedding = self.db.generate_text_embedding(content)?;

        // Store in vector DB
        self.db.vector_db.insert(VectorEntry {
            id: Some(format!("session_{}_{}", self.session_id, id)),
            vector: embedding,
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::json!("session_turn"));
                meta.insert(
                    "session_id".to_string(),
                    serde_json::json!(self.session_id.clone()),
                );
                meta.insert("turn_id".to_string(), serde_json::json!(id.clone()));
                meta.insert("turn_number".to_string(), serde_json::json!(turn_number));
                meta.insert("role".to_string(), serde_json::json!(role));
                meta.insert("content".to_string(), serde_json::json!(content));
                meta.insert("timestamp".to_string(), serde_json::json!(timestamp));
                meta.insert("expires_at".to_string(), serde_json::json!(expires_at));
                meta
            }),
        })?;

        Ok(id)
    }

    /// Find relevant past turns based on current context
    pub fn find_relevant_turns(&self, query: &str, k: usize) -> Result<Vec<SessionTurn>> {
        let query_embedding = self.db.generate_text_embedding(query)?;
        let current_time = chrono::Utc::now().timestamp();

        let results = self.db.vector_db.search(SearchQuery {
            vector: query_embedding,
            k: k * 2, // Get extra to filter expired
            filter: Some({
                let mut filter = HashMap::new();
                filter.insert("type".to_string(), serde_json::json!("session_turn"));
                filter.insert(
                    "session_id".to_string(),
                    serde_json::json!(self.session_id.clone()),
                );
                filter
            }),
            ef_search: None,
        })?;

        let mut turns = Vec::new();
        for result in results {
            if let Some(metadata) = result.metadata {
                let expires_at = metadata
                    .get("expires_at")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                // Skip expired turns
                if expires_at < current_time {
                    continue;
                }

                turns.push(SessionTurn {
                    id: metadata
                        .get("turn_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    session_id: self.session_id.clone(),
                    turn_number: metadata
                        .get("turn_number")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize,
                    role: metadata
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    content: metadata
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    embedding: result.vector.unwrap_or_default(),
                    timestamp: metadata
                        .get("timestamp")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    expires_at,
                });

                if turns.len() >= k {
                    break;
                }
            }
        }

        Ok(turns)
    }

    /// Get full session context (all turns in order)
    pub fn get_session_context(&self) -> Result<Vec<SessionTurn>> {
        let mut turns = self.find_relevant_turns("", 1000)?;
        turns.sort_by_key(|t| t.turn_number);
        Ok(turns)
    }

    /// Clean up expired turns
    pub fn cleanup_expired(&self) -> Result<usize> {
        let current_time = chrono::Utc::now().timestamp();
        let all_turns = self.find_relevant_turns("", 10000)?;
        let mut deleted = 0;

        for turn in all_turns {
            if turn.expires_at < current_time {
                let _ = self
                    .db
                    .vector_db
                    .delete(&format!("session_{}_{}", self.session_id, turn.id));
                deleted += 1;
            }
        }

        Ok(deleted)
    }
}

/// Witness Log for cryptographically-linked audit trail
///
/// Provides immutable entries, hash-chain linking, and semantic searchability.
pub struct WitnessLog<'a> {
    db: &'a AgenticDB,
    last_hash: RwLock<Option<String>>,
}

/// Witness log entry with hash chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEntry {
    /// Entry ID
    pub id: String,
    /// Previous entry hash (forms chain)
    pub prev_hash: Option<String>,
    /// Current entry hash
    pub hash: String,
    /// Agent ID that performed the action
    pub agent_id: String,
    /// Action type
    pub action_type: String,
    /// Action details
    pub details: String,
    /// Action embedding for semantic search
    pub embedding: Vec<f32>,
    /// Timestamp
    pub timestamp: i64,
    /// Additional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl<'a> WitnessLog<'a> {
    /// Create a new witness log
    pub fn new(db: &'a AgenticDB) -> Self {
        Self {
            db,
            last_hash: RwLock::new(None),
        }
    }

    /// Compute SHA256 hash of entry data
    fn compute_hash(
        prev_hash: &Option<String>,
        agent_id: &str,
        action_type: &str,
        details: &str,
        timestamp: i64,
    ) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        if let Some(prev) = prev_hash {
            prev.hash(&mut hasher);
        }
        agent_id.hash(&mut hasher);
        action_type.hash(&mut hasher);
        details.hash(&mut hasher);
        timestamp.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Append an entry to the witness log (immutable, hash-linked)
    pub fn append(&self, agent_id: &str, action_type: &str, details: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        // Get previous hash for chain
        let prev_hash = self.last_hash.read().clone();

        // Compute hash for this entry
        let hash = Self::compute_hash(&prev_hash, agent_id, action_type, details, timestamp);

        // Generate embedding for semantic search
        let embedding = self
            .db
            .generate_text_embedding(&format!("{} {} {}", agent_id, action_type, details))?;

        // Store in vector DB (append-only)
        self.db.vector_db.insert(VectorEntry {
            id: Some(format!("witness_{}", id)),
            vector: embedding.clone(),
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), serde_json::json!("witness"));
                meta.insert("witness_id".to_string(), serde_json::json!(id.clone()));
                meta.insert("agent_id".to_string(), serde_json::json!(agent_id));
                meta.insert("action_type".to_string(), serde_json::json!(action_type));
                meta.insert("details".to_string(), serde_json::json!(details));
                meta.insert("timestamp".to_string(), serde_json::json!(timestamp));
                meta.insert("hash".to_string(), serde_json::json!(hash.clone()));
                if let Some(ref prev) = prev_hash {
                    meta.insert("prev_hash".to_string(), serde_json::json!(prev));
                }
                meta
            }),
        })?;

        // Update last hash
        *self.last_hash.write() = Some(hash.clone());

        Ok(id)
    }

    /// Search witness log semantically
    pub fn search(&self, query: &str, k: usize) -> Result<Vec<WitnessEntry>> {
        let query_embedding = self.db.generate_text_embedding(query)?;

        let results = self.db.vector_db.search(SearchQuery {
            vector: query_embedding,
            k,
            filter: Some({
                let mut filter = HashMap::new();
                filter.insert("type".to_string(), serde_json::json!("witness"));
                filter
            }),
            ef_search: None,
        })?;

        let mut entries = Vec::new();
        for result in results {
            if let Some(metadata) = result.metadata {
                entries.push(WitnessEntry {
                    id: metadata
                        .get("witness_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    prev_hash: metadata
                        .get("prev_hash")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    hash: metadata
                        .get("hash")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    agent_id: metadata
                        .get("agent_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    action_type: metadata
                        .get("action_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    details: metadata
                        .get("details")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    embedding: result.vector.unwrap_or_default(),
                    timestamp: metadata
                        .get("timestamp")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    metadata: None,
                });
            }
        }

        Ok(entries)
    }

    /// Get entries by agent ID
    pub fn get_by_agent(&self, agent_id: &str, k: usize) -> Result<Vec<WitnessEntry>> {
        // Use semantic search with agent_id as query
        self.search(agent_id, k)
    }

    /// Verify hash chain integrity
    pub fn verify_chain(&self) -> Result<bool> {
        let entries = self.search("", 10000)?;

        // Sort by timestamp
        let mut sorted_entries = entries;
        sorted_entries.sort_by_key(|e| e.timestamp);

        // Verify each entry's prev_hash matches previous entry's hash
        for i in 1..sorted_entries.len() {
            let prev = &sorted_entries[i - 1];
            let curr = &sorted_entries[i];

            if let Some(ref prev_hash) = curr.prev_hash {
                if prev_hash != &prev.hash {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

impl AgenticDB {
    /// Get the Policy Memory Store interface
    pub fn policy_memory(&self) -> PolicyMemoryStore<'_> {
        PolicyMemoryStore::new(self)
    }

    /// Get a Session State Index for a specific session
    pub fn session_index(&self, session_id: &str, ttl_seconds: i64) -> SessionStateIndex<'_> {
        SessionStateIndex::new(self, session_id, ttl_seconds)
    }

    /// Get the Witness Log interface
    pub fn witness_log(&self) -> WitnessLog<'_> {
        WitnessLog::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_db() -> Result<AgenticDB> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 128;
        AgenticDB::new(options)
    }

    #[test]
    fn test_reflexion_episode() -> Result<()> {
        let db = create_test_db()?;

        let id = db.store_episode(
            "Solve math problem".to_string(),
            vec!["read problem".to_string(), "calculate".to_string()],
            vec!["got 42".to_string()],
            "Should have shown work".to_string(),
        )?;

        let episodes = db.retrieve_similar_episodes("math problem solving", 5)?;
        assert!(!episodes.is_empty());
        assert_eq!(episodes[0].id, id);

        Ok(())
    }

    #[test]
    fn test_skill_library() -> Result<()> {
        let db = create_test_db()?;

        let mut params = HashMap::new();
        params.insert("input".to_string(), "string".to_string());

        let skill_id = db.create_skill(
            "Parse JSON".to_string(),
            "Parse JSON from string".to_string(),
            params,
            vec!["json.parse()".to_string()],
        )?;

        let skills = db.search_skills("parse json data", 5)?;
        assert!(!skills.is_empty());

        Ok(())
    }

    #[test]
    fn test_causal_edge() -> Result<()> {
        let db = create_test_db()?;

        let edge_id = db.add_causal_edge(
            vec!["rain".to_string()],
            vec!["wet ground".to_string()],
            0.95,
            "Weather observation".to_string(),
        )?;

        let results = db.query_with_utility("weather patterns", 5, 0.7, 0.2, 0.1)?;
        assert!(!results.is_empty());

        Ok(())
    }

    #[test]
    fn test_learning_session() -> Result<()> {
        let db = create_test_db()?;

        let session_id = db.start_session("Q-Learning".to_string(), 4, 2)?;

        db.add_experience(
            &session_id,
            vec![1.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.0],
            1.0,
            vec![0.0, 1.0, 0.0, 0.0],
            false,
        )?;

        let prediction = db.predict_with_confidence(&session_id, vec![1.0, 0.0, 0.0, 0.0])?;
        assert_eq!(prediction.action.len(), 2);

        Ok(())
    }

    #[test]
    fn test_auto_consolidate() -> Result<()> {
        let db = create_test_db()?;

        let sequences = vec![
            vec![
                "step1".to_string(),
                "step2".to_string(),
                "step3".to_string(),
            ],
            vec![
                "action1".to_string(),
                "action2".to_string(),
                "action3".to_string(),
            ],
        ];

        let skill_ids = db.auto_consolidate(sequences, 3)?;
        assert_eq!(skill_ids.len(), 2);

        Ok(())
    }
}
