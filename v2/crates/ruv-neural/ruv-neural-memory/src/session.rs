//! Session-based memory management for grouping embeddings by recording session.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::topology::CognitiveState;

use crate::store::NeuralMemoryStore;

/// Metadata for a recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier.
    pub session_id: String,
    /// Subject being recorded.
    pub subject_id: String,
    /// Session start time (Unix timestamp).
    pub start_time: f64,
    /// Session end time (None if still active).
    pub end_time: Option<f64>,
    /// Number of embeddings stored during this session.
    pub num_embeddings: usize,
    /// Cognitive states observed during the session.
    pub cognitive_states_observed: Vec<CognitiveState>,
}

/// Manages neural memory across recording sessions.
pub struct SessionMemory {
    /// Underlying embedding store.
    store: NeuralMemoryStore,
    /// Currently active session ID.
    current_session: Option<String>,
    /// Metadata for all sessions.
    session_metadata: HashMap<String, SessionMetadata>,
    /// Maps session_id to embedding indices.
    session_indices: HashMap<String, Vec<usize>>,
    /// Counter for generating session IDs.
    session_counter: u64,
}

impl SessionMemory {
    /// Create a new session memory with the given store capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            store: NeuralMemoryStore::new(capacity),
            current_session: None,
            session_metadata: HashMap::new(),
            session_indices: HashMap::new(),
            session_counter: 0,
        }
    }

    /// Start a new recording session, returning its unique ID.
    ///
    /// If a session is already active, it is automatically ended first.
    pub fn start_session(&mut self, subject_id: &str) -> String {
        if self.current_session.is_some() {
            self.end_session();
        }

        self.session_counter += 1;
        let session_id = format!("session-{:04}", self.session_counter);

        let metadata = SessionMetadata {
            session_id: session_id.clone(),
            subject_id: subject_id.to_string(),
            start_time: 0.0, // Will be updated on first embedding
            end_time: None,
            num_embeddings: 0,
            cognitive_states_observed: Vec::new(),
        };

        self.session_metadata
            .insert(session_id.clone(), metadata);
        self.session_indices
            .insert(session_id.clone(), Vec::new());
        self.current_session = Some(session_id.clone());

        session_id
    }

    /// End the current recording session.
    pub fn end_session(&mut self) {
        if let Some(ref session_id) = self.current_session.clone() {
            if let Some(meta) = self.session_metadata.get_mut(session_id) {
                // Set end time from the last embedding's timestamp
                if let Some(indices) = self.session_indices.get(session_id) {
                    if let Some(&last_idx) = indices.last() {
                        if let Some(emb) = self.store.get(last_idx) {
                            meta.end_time = Some(emb.timestamp);
                        }
                    }
                }
            }
        }
        self.current_session = None;
    }

    /// Store an embedding in the current session.
    ///
    /// Returns an error if no session is active.
    pub fn store(&mut self, embedding: NeuralEmbedding) -> Result<usize> {
        let session_id = self
            .current_session
            .clone()
            .ok_or_else(|| RuvNeuralError::Memory("No active session".into()))?;

        let timestamp = embedding.timestamp;
        let state = embedding.metadata.cognitive_state;
        let idx = self.store.store(embedding)?;

        // Update session metadata
        if let Some(meta) = self.session_metadata.get_mut(&session_id) {
            if meta.num_embeddings == 0 {
                meta.start_time = timestamp;
            }
            meta.num_embeddings += 1;

            if let Some(s) = state {
                if !meta.cognitive_states_observed.contains(&s) {
                    meta.cognitive_states_observed.push(s);
                }
            }
        }

        if let Some(indices) = self.session_indices.get_mut(&session_id) {
            indices.push(idx);
        }

        Ok(idx)
    }

    /// Get all embeddings from a specific session.
    pub fn get_session_history(&self, session_id: &str) -> Vec<&NeuralEmbedding> {
        match self.session_indices.get(session_id) {
            Some(indices) => indices
                .iter()
                .filter_map(|&i| self.store.get(i))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Get all embeddings for a given subject across all sessions.
    pub fn get_subject_history(&self, subject_id: &str) -> Vec<&NeuralEmbedding> {
        self.store.query_by_subject(subject_id)
    }

    /// Get metadata for a session.
    pub fn get_session_metadata(&self, session_id: &str) -> Option<&SessionMetadata> {
        self.session_metadata.get(session_id)
    }

    /// Get the current active session ID.
    pub fn current_session_id(&self) -> Option<&str> {
        self.current_session.as_deref()
    }

    /// Access the underlying store.
    pub fn store_ref(&self) -> &NeuralMemoryStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::embedding::EmbeddingMetadata;

    fn make_embedding(vector: Vec<f64>, subject: &str, timestamp: f64) -> NeuralEmbedding {
        NeuralEmbedding::new(
            vector,
            timestamp,
            EmbeddingMetadata {
                subject_id: Some(subject.to_string()),
                session_id: None,
                cognitive_state: Some(CognitiveState::Rest),
                source_atlas: Atlas::Schaefer100,
                embedding_method: "test".to_string(),
            },
        )
        .unwrap()
    }

    #[test]
    fn session_lifecycle() {
        let mut mem = SessionMemory::new(100);

        // No session active
        assert!(mem.current_session_id().is_none());

        // Start session
        let sid = mem.start_session("subj1");
        assert_eq!(mem.current_session_id(), Some(sid.as_str()));

        // Store embeddings
        mem.store(make_embedding(vec![1.0, 0.0], "subj1", 1.0))
            .unwrap();
        mem.store(make_embedding(vec![0.0, 1.0], "subj1", 2.0))
            .unwrap();

        // Check session history
        let history = mem.get_session_history(&sid);
        assert_eq!(history.len(), 2);

        // Check metadata
        let meta = mem.get_session_metadata(&sid).unwrap();
        assert_eq!(meta.num_embeddings, 2);
        assert_eq!(meta.subject_id, "subj1");

        // End session
        mem.end_session();
        assert!(mem.current_session_id().is_none());

        let meta = mem.get_session_metadata(&sid).unwrap();
        assert_eq!(meta.end_time, Some(2.0));
    }

    #[test]
    fn store_without_session_fails() {
        let mut mem = SessionMemory::new(100);
        let result = mem.store(make_embedding(vec![1.0], "subj1", 0.0));
        assert!(result.is_err());
    }

    #[test]
    fn multiple_sessions() {
        let mut mem = SessionMemory::new(100);

        let s1 = mem.start_session("subj1");
        mem.store(make_embedding(vec![1.0], "subj1", 1.0))
            .unwrap();
        mem.end_session();

        let s2 = mem.start_session("subj1");
        mem.store(make_embedding(vec![2.0], "subj1", 2.0))
            .unwrap();
        mem.store(make_embedding(vec![3.0], "subj1", 3.0))
            .unwrap();
        mem.end_session();

        assert_eq!(mem.get_session_history(&s1).len(), 1);
        assert_eq!(mem.get_session_history(&s2).len(), 2);

        // Subject history spans all sessions
        let subject_history = mem.get_subject_history("subj1");
        assert_eq!(subject_history.len(), 3);
    }

    #[test]
    fn starting_new_session_ends_previous() {
        let mut mem = SessionMemory::new(100);

        let s1 = mem.start_session("subj1");
        mem.store(make_embedding(vec![1.0], "subj1", 1.0))
            .unwrap();

        // Starting a new session auto-ends the previous one
        let _s2 = mem.start_session("subj2");

        let meta = mem.get_session_metadata(&s1).unwrap();
        assert!(meta.end_time.is_some());
    }
}
