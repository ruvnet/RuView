//! CRV Session Manager
//!
//! Manages CRV sessions as directed acyclic graphs (DAGs), where each session
//! progresses through stages I-VI. Provides cross-session convergence analysis
//! to find agreement between multiple viewers targeting the same coordinate.
//!
//! # Architecture
//!
//! Each session is a DAG of stage entries. Cross-session convergence is computed
//! by finding entries with high embedding similarity across different sessions
//! targeting the same coordinate.

use crate::error::{CrvError, CrvResult};
use crate::stage_i::StageIEncoder;
use crate::stage_ii::StageIIEncoder;
use crate::stage_iii::StageIIIEncoder;
use crate::stage_iv::StageIVEncoder;
use crate::stage_v::StageVEngine;
use crate::stage_vi::StageVIModeler;
use crate::types::*;
use ruvector_gnn::search::cosine_similarity;
use std::collections::HashMap;

/// A session entry stored in the session graph.
#[derive(Debug, Clone)]
struct SessionEntry {
    /// The stage data embedding.
    embedding: Vec<f32>,
    /// Stage number (1-6).
    stage: u8,
    /// Entry index within the stage.
    entry_index: usize,
    /// Metadata.
    metadata: HashMap<String, serde_json::Value>,
    /// Timestamp.
    timestamp_ms: u64,
}

/// A complete CRV session with all stage data.
#[derive(Debug)]
struct Session {
    /// Session identifier.
    id: SessionId,
    /// Target coordinate.
    coordinate: TargetCoordinate,
    /// Entries organized by stage.
    entries: Vec<SessionEntry>,
}

/// CRV Session Manager: coordinates all stage encoders and manages sessions.
#[derive(Debug)]
pub struct CrvSessionManager {
    /// Configuration.
    config: CrvConfig,
    /// Stage I encoder.
    stage_i: StageIEncoder,
    /// Stage II encoder.
    stage_ii: StageIIEncoder,
    /// Stage III encoder.
    stage_iii: StageIIIEncoder,
    /// Stage IV encoder.
    stage_iv: StageIVEncoder,
    /// Stage V engine.
    stage_v: StageVEngine,
    /// Stage VI modeler.
    stage_vi: StageVIModeler,
    /// Active sessions indexed by session ID.
    sessions: HashMap<SessionId, Session>,
}

impl CrvSessionManager {
    /// Create a new session manager with the given configuration.
    pub fn new(config: CrvConfig) -> Self {
        let stage_i = StageIEncoder::new(&config);
        let stage_ii = StageIIEncoder::new(&config);
        let stage_iii = StageIIIEncoder::new(&config);
        let stage_iv = StageIVEncoder::new(&config);
        let stage_v = StageVEngine::new(&config);
        let stage_vi = StageVIModeler::new(&config);

        Self {
            config,
            stage_i,
            stage_ii,
            stage_iii,
            stage_iv,
            stage_v,
            stage_vi,
            sessions: HashMap::new(),
        }
    }

    /// Create a new session for a given target coordinate.
    pub fn create_session(
        &mut self,
        session_id: SessionId,
        coordinate: TargetCoordinate,
    ) -> CrvResult<()> {
        if self.sessions.contains_key(&session_id) {
            return Err(CrvError::EncodingError(format!(
                "Session {} already exists",
                session_id
            )));
        }

        self.sessions.insert(
            session_id.clone(),
            Session {
                id: session_id,
                coordinate,
                entries: Vec::new(),
            },
        );

        Ok(())
    }

    /// Add Stage I data to a session.
    pub fn add_stage_i(&mut self, session_id: &str, data: &StageIData) -> CrvResult<Vec<f32>> {
        let embedding = self.stage_i.encode(data)?;
        self.add_entry(session_id, 1, embedding.clone(), HashMap::new())?;
        Ok(embedding)
    }

    /// Add Stage II data to a session.
    pub fn add_stage_ii(&mut self, session_id: &str, data: &StageIIData) -> CrvResult<Vec<f32>> {
        let embedding = self.stage_ii.encode(data)?;
        self.add_entry(session_id, 2, embedding.clone(), HashMap::new())?;
        Ok(embedding)
    }

    /// Add Stage III data to a session.
    pub fn add_stage_iii(&mut self, session_id: &str, data: &StageIIIData) -> CrvResult<Vec<f32>> {
        let embedding = self.stage_iii.encode(data)?;
        self.add_entry(session_id, 3, embedding.clone(), HashMap::new())?;
        Ok(embedding)
    }

    /// Add Stage IV data to a session.
    pub fn add_stage_iv(&mut self, session_id: &str, data: &StageIVData) -> CrvResult<Vec<f32>> {
        let embedding = self.stage_iv.encode(data)?;
        self.add_entry(session_id, 4, embedding.clone(), HashMap::new())?;
        Ok(embedding)
    }

    /// Run Stage V interrogation on a session.
    ///
    /// Probes the accumulated session data with specified queries.
    pub fn run_stage_v(
        &mut self,
        session_id: &str,
        probe_queries: &[(&str, u8, Vec<f32>)], // (query text, target stage, query embedding)
        k: usize,
    ) -> CrvResult<StageVData> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| CrvError::SessionNotFound(session_id.to_string()))?;

        let all_embeddings: Vec<Vec<f32>> = session
            .entries
            .iter()
            .map(|e| e.embedding.clone())
            .collect();

        let mut probes = Vec::new();
        let mut cross_refs = Vec::new();

        for (query_text, target_stage, query_emb) in probe_queries {
            // Filter candidates to the target stage
            let stage_entries: Vec<Vec<f32>> = session
                .entries
                .iter()
                .filter(|e| e.stage == *target_stage)
                .map(|e| e.embedding.clone())
                .collect();

            if stage_entries.is_empty() {
                continue;
            }

            let mut probe = self.stage_v.probe(query_emb, &stage_entries, k)?;
            probe.query = query_text.to_string();
            probe.target_stage = *target_stage;
            probes.push(probe);
        }

        // Cross-reference between all stage pairs
        for from_stage in 1..=4u8 {
            for to_stage in (from_stage + 1)..=4u8 {
                let from_entries: Vec<Vec<f32>> = session
                    .entries
                    .iter()
                    .filter(|e| e.stage == from_stage)
                    .map(|e| e.embedding.clone())
                    .collect();
                let to_entries: Vec<Vec<f32>> = session
                    .entries
                    .iter()
                    .filter(|e| e.stage == to_stage)
                    .map(|e| e.embedding.clone())
                    .collect();

                if !from_entries.is_empty() && !to_entries.is_empty() {
                    let refs = self.stage_v.cross_reference(
                        from_stage,
                        &from_entries,
                        to_stage,
                        &to_entries,
                        self.config.convergence_threshold,
                    );
                    cross_refs.extend(refs);
                }
            }
        }

        let stage_v_data = StageVData {
            probes,
            cross_references: cross_refs,
        };

        // Encode Stage V result and add to session
        if !stage_v_data.probes.is_empty() {
            let embedding = self.stage_v.encode(&stage_v_data, &all_embeddings)?;
            self.add_entry(session_id, 5, embedding, HashMap::new())?;
        }

        Ok(stage_v_data)
    }

    /// Run Stage VI composite modeling on a session.
    pub fn run_stage_vi(&mut self, session_id: &str) -> CrvResult<StageVIData> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| CrvError::SessionNotFound(session_id.to_string()))?;

        let embeddings: Vec<Vec<f32>> = session
            .entries
            .iter()
            .map(|e| e.embedding.clone())
            .collect();
        let labels: Vec<(u8, usize)> = session
            .entries
            .iter()
            .map(|e| (e.stage, e.entry_index))
            .collect();

        let stage_vi_data = self.stage_vi.partition(&embeddings, &labels)?;

        // Encode Stage VI result and add to session
        let embedding = self.stage_vi.encode(&stage_vi_data)?;
        self.add_entry(session_id, 6, embedding, HashMap::new())?;

        Ok(stage_vi_data)
    }

    /// Find convergence across multiple sessions targeting the same coordinate.
    ///
    /// This is the core multi-viewer matching operation: given sessions from
    /// different viewers targeting the same coordinate, find which aspects
    /// of their signal line data converge (agree).
    pub fn find_convergence(
        &self,
        coordinate: &str,
        min_similarity: f32,
    ) -> CrvResult<ConvergenceResult> {
        // Collect all sessions for this coordinate
        let relevant_sessions: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| s.coordinate == coordinate)
            .collect();

        if relevant_sessions.len() < 2 {
            return Err(CrvError::EmptyInput(
                "Need at least 2 sessions for convergence analysis".to_string(),
            ));
        }

        let mut session_pairs = Vec::new();
        let mut scores = Vec::new();
        let mut convergent_stages = Vec::new();

        // Compare all pairs of sessions
        for i in 0..relevant_sessions.len() {
            for j in (i + 1)..relevant_sessions.len() {
                let sess_a = relevant_sessions[i];
                let sess_b = relevant_sessions[j];

                // Compare stage-by-stage
                for stage in 1..=6u8 {
                    let entries_a: Vec<&[f32]> = sess_a
                        .entries
                        .iter()
                        .filter(|e| e.stage == stage)
                        .map(|e| e.embedding.as_slice())
                        .collect();
                    let entries_b: Vec<&[f32]> = sess_b
                        .entries
                        .iter()
                        .filter(|e| e.stage == stage)
                        .map(|e| e.embedding.as_slice())
                        .collect();

                    if entries_a.is_empty() || entries_b.is_empty() {
                        continue;
                    }

                    // Find best match for each entry in A against entries in B
                    for emb_a in &entries_a {
                        for emb_b in &entries_b {
                            if emb_a.len() == emb_b.len() && !emb_a.is_empty() {
                                let sim = cosine_similarity(emb_a, emb_b);
                                if sim >= min_similarity {
                                    session_pairs.push((sess_a.id.clone(), sess_b.id.clone()));
                                    scores.push(sim);
                                    if !convergent_stages.contains(&stage) {
                                        convergent_stages.push(stage);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Compute consensus embedding (mean of all converging embeddings)
        let consensus_embedding = if !scores.is_empty() {
            let mut consensus = vec![0.0f32; self.config.dimensions];
            let mut count = 0usize;

            for session in &relevant_sessions {
                for entry in &session.entries {
                    if convergent_stages.contains(&entry.stage) {
                        for (i, &v) in entry.embedding.iter().enumerate() {
                            if i < self.config.dimensions {
                                consensus[i] += v;
                            }
                        }
                        count += 1;
                    }
                }
            }

            if count > 0 {
                for v in &mut consensus {
                    *v /= count as f32;
                }
                Some(consensus)
            } else {
                None
            }
        } else {
            None
        };

        // Sort convergent stages
        convergent_stages.sort();

        Ok(ConvergenceResult {
            session_pairs,
            scores,
            convergent_stages,
            consensus_embedding,
        })
    }

    /// Get all embeddings for a session.
    pub fn get_session_embeddings(&self, session_id: &str) -> CrvResult<Vec<CrvSessionEntry>> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| CrvError::SessionNotFound(session_id.to_string()))?;

        Ok(session
            .entries
            .iter()
            .map(|e| CrvSessionEntry {
                session_id: session.id.clone(),
                coordinate: session.coordinate.clone(),
                stage: e.stage,
                embedding: e.embedding.clone(),
                metadata: e.metadata.clone(),
                timestamp_ms: e.timestamp_ms,
            })
            .collect())
    }

    /// Get the number of entries in a session.
    pub fn session_entry_count(&self, session_id: &str) -> usize {
        self.sessions
            .get(session_id)
            .map(|s| s.entries.len())
            .unwrap_or(0)
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Remove a session.
    pub fn remove_session(&mut self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    /// Get access to the Stage I encoder for direct operations.
    pub fn stage_i_encoder(&self) -> &StageIEncoder {
        &self.stage_i
    }

    /// Get access to the Stage II encoder for direct operations.
    pub fn stage_ii_encoder(&self) -> &StageIIEncoder {
        &self.stage_ii
    }

    /// Get access to the Stage IV encoder for direct operations.
    pub fn stage_iv_encoder(&self) -> &StageIVEncoder {
        &self.stage_iv
    }

    /// Get access to the Stage V engine for direct operations.
    pub fn stage_v_engine(&self) -> &StageVEngine {
        &self.stage_v
    }

    /// Get access to the Stage VI modeler for direct operations.
    pub fn stage_vi_modeler(&self) -> &StageVIModeler {
        &self.stage_vi
    }

    /// Internal: add an entry to a session.
    fn add_entry(
        &mut self,
        session_id: &str,
        stage: u8,
        embedding: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> CrvResult<()> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| CrvError::SessionNotFound(session_id.to_string()))?;

        let entry_index = session.entries.iter().filter(|e| e.stage == stage).count();

        session.entries.push(SessionEntry {
            embedding,
            stage,
            entry_index,
            metadata,
            timestamp_ms: 0,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 32,
            convergence_threshold: 0.5,
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_session_creation() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        manager
            .create_session("sess-1".to_string(), "1234-5678".to_string())
            .unwrap();
        assert_eq!(manager.session_count(), 1);
        assert_eq!(manager.session_entry_count("sess-1"), 0);
    }

    #[test]
    fn test_add_stage_i() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        manager
            .create_session("sess-1".to_string(), "1234-5678".to_string())
            .unwrap();

        let data = StageIData {
            stroke: vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)],
            spontaneous_descriptor: "angular".to_string(),
            classification: GestaltType::Manmade,
            confidence: 0.9,
        };

        let emb = manager.add_stage_i("sess-1", &data).unwrap();
        assert_eq!(emb.len(), 32);
        assert_eq!(manager.session_entry_count("sess-1"), 1);
    }

    #[test]
    fn test_add_stage_ii() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        manager
            .create_session("sess-1".to_string(), "coord-1".to_string())
            .unwrap();

        let data = StageIIData {
            impressions: vec![
                (SensoryModality::Texture, "rough".to_string()),
                (SensoryModality::Color, "gray".to_string()),
            ],
            feature_vector: None,
        };

        let emb = manager.add_stage_ii("sess-1", &data).unwrap();
        assert_eq!(emb.len(), 32);
    }

    #[test]
    fn test_full_session_flow() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        manager
            .create_session("sess-1".to_string(), "coord-1".to_string())
            .unwrap();

        // Stage I
        let s1 = StageIData {
            stroke: vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)],
            spontaneous_descriptor: "angular".to_string(),
            classification: GestaltType::Manmade,
            confidence: 0.9,
        };
        manager.add_stage_i("sess-1", &s1).unwrap();

        // Stage II
        let s2 = StageIIData {
            impressions: vec![
                (SensoryModality::Texture, "rough stone".to_string()),
                (SensoryModality::Temperature, "cold".to_string()),
            ],
            feature_vector: None,
        };
        manager.add_stage_ii("sess-1", &s2).unwrap();

        // Stage IV
        let s4 = StageIVData {
            emotional_impact: vec![("solemn".to_string(), 0.6)],
            tangibles: vec!["stone blocks".to_string()],
            intangibles: vec!["ancient".to_string()],
            aol_detections: vec![],
        };
        manager.add_stage_iv("sess-1", &s4).unwrap();

        assert_eq!(manager.session_entry_count("sess-1"), 3);

        // Get all entries
        let entries = manager.get_session_embeddings("sess-1").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].stage, 1);
        assert_eq!(entries[1].stage, 2);
        assert_eq!(entries[2].stage, 4);
    }

    #[test]
    fn test_duplicate_session() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        manager
            .create_session("sess-1".to_string(), "coord-1".to_string())
            .unwrap();

        let result = manager.create_session("sess-1".to_string(), "coord-2".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_session_not_found() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        let s1 = StageIData {
            stroke: vec![(0.0, 0.0), (1.0, 1.0)],
            spontaneous_descriptor: "test".to_string(),
            classification: GestaltType::Natural,
            confidence: 0.5,
        };

        let result = manager.add_stage_i("nonexistent", &s1);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_session() {
        let config = test_config();
        let mut manager = CrvSessionManager::new(config);

        manager
            .create_session("sess-1".to_string(), "coord-1".to_string())
            .unwrap();
        assert_eq!(manager.session_count(), 1);

        assert!(manager.remove_session("sess-1"));
        assert_eq!(manager.session_count(), 0);

        assert!(!manager.remove_session("sess-1"));
    }
}
