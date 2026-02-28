//! Session State Index
//!
//! Indexes session state in Ruvector for efficient retrieval and
//! semantic search across sessions. Enables features like:
//! - Session recovery by context similarity
//! - Cross-session knowledge transfer
//! - User session history queries

use crate::error::{Result, RuvLLMError};
use crate::kv_cache::CacheQuantization;
use crate::session::Session;
use chrono::{DateTime, Utc};
use ruvector_core::types::DbOptions;
use ruvector_core::{AgenticDB, SearchQuery, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cache tier for reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheTier {
    /// Hot tier (in memory, full precision)
    Hot,
    /// Warm tier (compressed in memory)
    Warm,
    /// Cold tier (on disk or evicted)
    Cold,
}

impl Default for CacheTier {
    fn default() -> Self {
        Self::Hot
    }
}

/// Cache location type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheLocation {
    /// In-memory location
    Memory {
        /// Offset in memory pool
        offset: usize,
    },
    /// Disk-backed location
    Disk {
        /// File path
        path: String,
        /// Offset in file
        offset: usize,
    },
    /// Evicted (not currently stored)
    Evicted,
}

impl Default for CacheLocation {
    fn default() -> Self {
        Self::Memory { offset: 0 }
    }
}

/// KV cache reference with tiered storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvCacheReference {
    /// Cache storage tier
    pub tier: CacheTier,
    /// Location identifier
    pub location: CacheLocation,
    /// Number of cached tokens
    pub cached_tokens: usize,
    /// Quantization level of cached KV pairs
    pub quantization: CacheQuantization,
    /// Cache creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Default for KvCacheReference {
    fn default() -> Self {
        Self {
            tier: CacheTier::Hot,
            location: CacheLocation::Memory { offset: 0 },
            cached_tokens: 0,
            quantization: CacheQuantization::default(),
            created_at: Utc::now(),
        }
    }
}

/// Session state entry for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session identifier
    pub session_id: String,
    /// User/tenant identifier
    pub user_id: Option<String>,
    /// Embedding of conversation context (768-D)
    pub context_embedding: Vec<f32>,
    /// Reference to KV cache location
    pub kv_cache_ref: KvCacheReference,
    /// Currently active LoRA adapter ID
    pub active_adapter: Option<String>,
    /// Conversation turn count
    pub turn_count: u32,
    /// Last activity timestamp
    pub last_active: DateTime<Utc>,
    /// Session metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SessionState {
    /// Create from a Session
    pub fn from_session(session: &Session) -> Self {
        Self {
            session_id: session.id.clone(),
            user_id: session.user_id.clone(),
            context_embedding: session
                .context_embedding
                .clone()
                .unwrap_or_else(|| vec![0.0; 768]),
            kv_cache_ref: KvCacheReference::default(),
            active_adapter: session.active_adapter.map(|id| id.to_string()),
            turn_count: session.turn_count,
            last_active: session.last_active,
            metadata: session.metadata.custom.clone(),
        }
    }
}

/// Session index backed by Ruvector
pub struct SessionIndex {
    /// Ruvector database
    db: AgenticDB,
    /// Embedding dimension
    embedding_dim: usize,
}

impl SessionIndex {
    /// Create a new session index
    pub fn new(storage_path: &str, embedding_dim: usize) -> Result<Self> {
        let mut options = DbOptions::default();
        options.storage_path = storage_path.to_string();
        options.dimensions = embedding_dim;

        let db = AgenticDB::new(options).map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        Ok(Self { db, embedding_dim })
    }

    /// Store a session state
    pub fn store(&self, state: &SessionState) -> Result<()> {
        // Create metadata
        let mut metadata = HashMap::new();
        metadata.insert(
            "session_id".to_string(),
            serde_json::json!(state.session_id),
        );

        if let Some(ref user_id) = state.user_id {
            metadata.insert("user_id".to_string(), serde_json::json!(user_id));
        }

        metadata.insert(
            "turn_count".to_string(),
            serde_json::json!(state.turn_count),
        );
        metadata.insert(
            "last_active".to_string(),
            serde_json::json!(state.last_active.to_rfc3339()),
        );
        metadata.insert(
            "kv_cache_ref".to_string(),
            serde_json::to_value(&state.kv_cache_ref).unwrap_or_default(),
        );

        if let Some(ref adapter) = state.active_adapter {
            metadata.insert("active_adapter".to_string(), serde_json::json!(adapter));
        }

        for (key, value) in &state.metadata {
            metadata.insert(format!("meta_{}", key), value.clone());
        }

        // Create vector entry
        let vector_entry = VectorEntry {
            id: Some(state.session_id.clone()),
            vector: state.context_embedding.clone(),
            metadata: Some(metadata),
        };

        // Store in Ruvector
        self.db
            .insert(vector_entry)
            .map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Search sessions by context similarity
    pub fn search_by_context(
        &self,
        context_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SessionState>> {
        let query = SearchQuery {
            vector: context_embedding.to_vec(),
            k: limit,
            filter: None,
            ef_search: None,
        };

        let results = self
            .db
            .search(query)
            .map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        let mut states = Vec::with_capacity(results.len());
        for result in results {
            if let Some(metadata) = &result.metadata {
                if let Some(state) =
                    self.state_from_metadata(&result.id, context_embedding, metadata)
                {
                    states.push(state);
                }
            }
        }

        Ok(states)
    }

    /// Delete session state
    pub fn delete(&self, session_id: &str) -> Result<()> {
        self.db
            .delete(session_id)
            .map_err(|e| RuvLLMError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Convert metadata to SessionState
    fn state_from_metadata(
        &self,
        _id: &str,
        embedding: &[f32],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Option<SessionState> {
        let session_id = metadata.get("session_id")?.as_str()?.to_string();

        let user_id = metadata
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let turn_count = metadata
            .get("turn_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let last_active = metadata
            .get("last_active")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let kv_cache_ref: KvCacheReference = metadata
            .get("kv_cache_ref")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let active_adapter = metadata
            .get("active_adapter")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Extract custom metadata
        let mut custom_metadata = HashMap::new();
        for (key, value) in metadata {
            if key.starts_with("meta_") {
                custom_metadata.insert(key[5..].to_string(), value.clone());
            }
        }

        Some(SessionState {
            session_id,
            user_id,
            context_embedding: embedding.to_vec(),
            kv_cache_ref,
            active_adapter,
            turn_count,
            last_active,
            metadata: custom_metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_tier() {
        assert_eq!(CacheTier::default(), CacheTier::Hot);
    }

    #[test]
    fn test_kv_cache_reference_default() {
        let kv_ref = KvCacheReference::default();
        assert_eq!(kv_ref.tier, CacheTier::Hot);
        assert_eq!(kv_ref.cached_tokens, 0);
    }

    #[test]
    fn test_session_state_from_session() {
        use crate::session::{Session, SessionConfig};

        let config = SessionConfig::default();
        let session = Session::new(&config, Some("user-123"));

        let state = SessionState::from_session(&session);
        assert_eq!(state.session_id, session.id);
        assert_eq!(state.user_id, Some("user-123".to_string()));
    }
}
