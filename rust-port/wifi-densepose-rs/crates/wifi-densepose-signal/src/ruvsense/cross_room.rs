//! Cross-room identity continuity.
//!
//! Maintains identity persistence across rooms without optics by
//! fingerprinting each room's electromagnetic profile, tracking
//! exit/entry events, and matching person embeddings across transition
//! boundaries.
//!
//! # Algorithm
//! 1. Each room is fingerprinted as a 128-dim AETHER embedding of its
//!    static CSI profile
//! 2. When a track is lost near a room boundary, record an exit event
//!    with the person's current embedding
//! 3. When a new track appears in an adjacent room within 60s, compare
//!    its embedding against recent exits
//! 4. If cosine similarity > 0.80, link the identities
//!
//! # Invariants
//! - Cross-room match requires > 0.80 cosine similarity AND < 60s temporal gap
//! - Transition graph is append-only (immutable audit trail)
//! - No image data stored — only 128-dim embeddings and structural events
//! - Maximum 100 rooms per deployment
//!
//! # References
//! - ADR-030 Tier 5: Cross-Room Identity Continuity

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from cross-room operations.
#[derive(Debug, thiserror::Error)]
pub enum CrossRoomError {
    /// Room capacity exceeded.
    #[error("Maximum rooms exceeded: limit is {max}")]
    MaxRoomsExceeded { max: usize },

    /// Room not found.
    #[error("Unknown room ID: {0}")]
    UnknownRoom(u64),

    /// Embedding dimension mismatch.
    #[error("Embedding dimension mismatch: expected {expected}, got {got}")]
    EmbeddingDimensionMismatch { expected: usize, got: usize },

    /// Invalid temporal gap for matching.
    #[error("Temporal gap {gap_s:.1}s exceeds maximum {max_s:.1}s")]
    TemporalGapExceeded { gap_s: f64, max_s: f64 },
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for cross-room identity tracking.
#[derive(Debug, Clone)]
pub struct CrossRoomConfig {
    /// Embedding dimension (typically 128).
    pub embedding_dim: usize,
    /// Minimum cosine similarity for cross-room match.
    pub min_similarity: f32,
    /// Maximum temporal gap (seconds) for cross-room match.
    pub max_gap_s: f64,
    /// Maximum rooms in the deployment.
    pub max_rooms: usize,
    /// Maximum pending exit events to retain.
    pub max_pending_exits: usize,
}

impl Default for CrossRoomConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 128,
            min_similarity: 0.80,
            max_gap_s: 60.0,
            max_rooms: 100,
            max_pending_exits: 200,
        }
    }
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A room's electromagnetic fingerprint.
#[derive(Debug, Clone)]
pub struct RoomFingerprint {
    /// Room identifier.
    pub room_id: u64,
    /// Fingerprint embedding vector.
    pub embedding: Vec<f32>,
    /// Timestamp when fingerprint was last computed (microseconds).
    pub computed_at_us: u64,
    /// Number of nodes contributing to this fingerprint.
    pub node_count: usize,
}

/// An exit event: a person leaving a room.
#[derive(Debug, Clone)]
pub struct ExitEvent {
    /// Person embedding at exit time.
    pub embedding: Vec<f32>,
    /// Room exited.
    pub room_id: u64,
    /// Person track ID (local to the room).
    pub track_id: u64,
    /// Timestamp of exit (microseconds).
    pub timestamp_us: u64,
    /// Whether this exit has been matched to an entry.
    pub matched: bool,
}

/// An entry event: a person appearing in a room.
#[derive(Debug, Clone)]
pub struct EntryEvent {
    /// Person embedding at entry time.
    pub embedding: Vec<f32>,
    /// Room entered.
    pub room_id: u64,
    /// Person track ID (local to the room).
    pub track_id: u64,
    /// Timestamp of entry (microseconds).
    pub timestamp_us: u64,
}

/// A cross-room transition record (immutable).
#[derive(Debug, Clone)]
pub struct TransitionEvent {
    /// Person who transitioned.
    pub person_id: u64,
    /// Room exited.
    pub from_room: u64,
    /// Room entered.
    pub to_room: u64,
    /// Exit track ID.
    pub exit_track_id: u64,
    /// Entry track ID.
    pub entry_track_id: u64,
    /// Cosine similarity between exit and entry embeddings.
    pub similarity: f32,
    /// Temporal gap between exit and entry (seconds).
    pub gap_s: f64,
    /// Timestamp of the transition (entry timestamp).
    pub timestamp_us: u64,
}

/// Result of attempting to match an entry against pending exits.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Whether a match was found.
    pub matched: bool,
    /// The transition event, if matched.
    pub transition: Option<TransitionEvent>,
    /// Number of candidates checked.
    pub candidates_checked: usize,
    /// Best similarity found (even if below threshold).
    pub best_similarity: f32,
}

// ---------------------------------------------------------------------------
// Cross-room identity tracker
// ---------------------------------------------------------------------------

/// Cross-room identity continuity tracker.
///
/// Maintains room fingerprints, pending exit events, and an immutable
/// transition graph. Matches person embeddings across rooms using
/// cosine similarity with temporal constraints.
#[derive(Debug)]
pub struct CrossRoomTracker {
    config: CrossRoomConfig,
    /// Room fingerprints indexed by room_id.
    rooms: Vec<RoomFingerprint>,
    /// Pending (unmatched) exit events.
    pending_exits: Vec<ExitEvent>,
    /// Immutable transition log (append-only).
    transitions: Vec<TransitionEvent>,
    /// Next person ID for cross-room identity assignment.
    next_person_id: u64,
}

impl CrossRoomTracker {
    /// Create a new cross-room tracker.
    pub fn new(config: CrossRoomConfig) -> Self {
        Self {
            config,
            rooms: Vec::new(),
            pending_exits: Vec::new(),
            transitions: Vec::new(),
            next_person_id: 1,
        }
    }

    /// Register a room fingerprint.
    pub fn register_room(&mut self, fingerprint: RoomFingerprint) -> Result<(), CrossRoomError> {
        if self.rooms.len() >= self.config.max_rooms {
            return Err(CrossRoomError::MaxRoomsExceeded {
                max: self.config.max_rooms,
            });
        }
        if fingerprint.embedding.len() != self.config.embedding_dim {
            return Err(CrossRoomError::EmbeddingDimensionMismatch {
                expected: self.config.embedding_dim,
                got: fingerprint.embedding.len(),
            });
        }
        // Replace existing fingerprint if room already registered
        if let Some(existing) = self
            .rooms
            .iter_mut()
            .find(|r| r.room_id == fingerprint.room_id)
        {
            *existing = fingerprint;
        } else {
            self.rooms.push(fingerprint);
        }
        Ok(())
    }

    /// Record a person exiting a room.
    pub fn record_exit(&mut self, event: ExitEvent) -> Result<(), CrossRoomError> {
        if event.embedding.len() != self.config.embedding_dim {
            return Err(CrossRoomError::EmbeddingDimensionMismatch {
                expected: self.config.embedding_dim,
                got: event.embedding.len(),
            });
        }
        // Evict oldest if at capacity
        if self.pending_exits.len() >= self.config.max_pending_exits {
            self.pending_exits.remove(0);
        }
        self.pending_exits.push(event);
        Ok(())
    }

    /// Try to match an entry event against pending exits.
    ///
    /// If a match is found, creates a TransitionEvent and marks the
    /// exit as matched. Returns the match result.
    pub fn match_entry(&mut self, entry: &EntryEvent) -> Result<MatchResult, CrossRoomError> {
        if entry.embedding.len() != self.config.embedding_dim {
            return Err(CrossRoomError::EmbeddingDimensionMismatch {
                expected: self.config.embedding_dim,
                got: entry.embedding.len(),
            });
        }

        let mut best_idx: Option<usize> = None;
        let mut best_sim: f32 = -1.0;
        let mut candidates_checked = 0;

        for (idx, exit) in self.pending_exits.iter().enumerate() {
            if exit.matched || exit.room_id == entry.room_id {
                continue;
            }

            // Temporal constraint
            let gap_us = entry.timestamp_us.saturating_sub(exit.timestamp_us);
            let gap_s = gap_us as f64 / 1_000_000.0;
            if gap_s > self.config.max_gap_s {
                continue;
            }

            candidates_checked += 1;

            let sim = cosine_similarity_f32(&exit.embedding, &entry.embedding);
            if sim > best_sim {
                best_sim = sim;
                if sim >= self.config.min_similarity {
                    best_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = best_idx {
            let exit = &self.pending_exits[idx];
            let gap_us = entry.timestamp_us.saturating_sub(exit.timestamp_us);
            let gap_s = gap_us as f64 / 1_000_000.0;

            let person_id = self.next_person_id;
            self.next_person_id += 1;

            let transition = TransitionEvent {
                person_id,
                from_room: exit.room_id,
                to_room: entry.room_id,
                exit_track_id: exit.track_id,
                entry_track_id: entry.track_id,
                similarity: best_sim,
                gap_s,
                timestamp_us: entry.timestamp_us,
            };

            // Mark exit as matched
            self.pending_exits[idx].matched = true;

            // Append to immutable transition log
            self.transitions.push(transition.clone());

            Ok(MatchResult {
                matched: true,
                transition: Some(transition),
                candidates_checked,
                best_similarity: best_sim,
            })
        } else {
            Ok(MatchResult {
                matched: false,
                transition: None,
                candidates_checked,
                best_similarity: if best_sim >= 0.0 { best_sim } else { 0.0 },
            })
        }
    }

    /// Expire old pending exits that exceed the maximum gap time.
    pub fn expire_exits(&mut self, current_us: u64) {
        let max_gap_us = (self.config.max_gap_s * 1_000_000.0) as u64;
        self.pending_exits.retain(|exit| {
            !exit.matched && current_us.saturating_sub(exit.timestamp_us) <= max_gap_us
        });
    }

    /// Number of registered rooms.
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Number of pending (unmatched) exit events.
    pub fn pending_exit_count(&self) -> usize {
        self.pending_exits.iter().filter(|e| !e.matched).count()
    }

    /// Number of transitions recorded.
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Get all transitions for a person.
    pub fn transitions_for_person(&self, person_id: u64) -> Vec<&TransitionEvent> {
        self.transitions
            .iter()
            .filter(|t| t.person_id == person_id)
            .collect()
    }

    /// Get all transitions between two rooms.
    pub fn transitions_between(&self, from_room: u64, to_room: u64) -> Vec<&TransitionEvent> {
        self.transitions
            .iter()
            .filter(|t| t.from_room == from_room && t.to_room == to_room)
            .collect()
    }

    /// Get the room fingerprint for a room ID.
    pub fn room_fingerprint(&self, room_id: u64) -> Option<&RoomFingerprint> {
        self.rooms.iter().find(|r| r.room_id == room_id)
    }
}

/// Cosine similarity between two f32 vectors.
fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-9 {
        0.0
    } else {
        dot / denom
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> CrossRoomConfig {
        CrossRoomConfig {
            embedding_dim: 4,
            min_similarity: 0.80,
            max_gap_s: 60.0,
            max_rooms: 10,
            max_pending_exits: 50,
        }
    }

    fn make_fingerprint(room_id: u64, v: [f32; 4]) -> RoomFingerprint {
        RoomFingerprint {
            room_id,
            embedding: v.to_vec(),
            computed_at_us: 0,
            node_count: 4,
        }
    }

    fn make_exit(room_id: u64, track_id: u64, emb: [f32; 4], ts: u64) -> ExitEvent {
        ExitEvent {
            embedding: emb.to_vec(),
            room_id,
            track_id,
            timestamp_us: ts,
            matched: false,
        }
    }

    fn make_entry(room_id: u64, track_id: u64, emb: [f32; 4], ts: u64) -> EntryEvent {
        EntryEvent {
            embedding: emb.to_vec(),
            room_id,
            track_id,
            timestamp_us: ts,
        }
    }

    #[test]
    fn test_tracker_creation() {
        let tracker = CrossRoomTracker::new(small_config());
        assert_eq!(tracker.room_count(), 0);
        assert_eq!(tracker.pending_exit_count(), 0);
        assert_eq!(tracker.transition_count(), 0);
    }

    #[test]
    fn test_register_room() {
        let mut tracker = CrossRoomTracker::new(small_config());
        tracker
            .register_room(make_fingerprint(1, [1.0, 0.0, 0.0, 0.0]))
            .unwrap();
        assert_eq!(tracker.room_count(), 1);
        assert!(tracker.room_fingerprint(1).is_some());
    }

    #[test]
    fn test_max_rooms_exceeded() {
        let config = CrossRoomConfig {
            max_rooms: 2,
            ..small_config()
        };
        let mut tracker = CrossRoomTracker::new(config);
        tracker
            .register_room(make_fingerprint(1, [1.0, 0.0, 0.0, 0.0]))
            .unwrap();
        tracker
            .register_room(make_fingerprint(2, [0.0, 1.0, 0.0, 0.0]))
            .unwrap();
        assert!(matches!(
            tracker.register_room(make_fingerprint(3, [0.0, 0.0, 1.0, 0.0])),
            Err(CrossRoomError::MaxRoomsExceeded { .. })
        ));
    }

    #[test]
    fn test_successful_cross_room_match() {
        let mut tracker = CrossRoomTracker::new(small_config());

        // Person exits room 1
        let exit_emb = [0.9, 0.1, 0.0, 0.0];
        tracker
            .record_exit(make_exit(1, 100, exit_emb, 1_000_000))
            .unwrap();

        // Same person enters room 2 (similar embedding, within 60s)
        let entry_emb = [0.88, 0.12, 0.01, 0.0];
        let entry = make_entry(2, 200, entry_emb, 5_000_000);
        let result = tracker.match_entry(&entry).unwrap();

        assert!(result.matched);
        let t = result.transition.unwrap();
        assert_eq!(t.from_room, 1);
        assert_eq!(t.to_room, 2);
        assert!(t.similarity >= 0.80);
        assert!(t.gap_s < 60.0);
    }

    #[test]
    fn test_no_match_different_person() {
        let mut tracker = CrossRoomTracker::new(small_config());

        tracker
            .record_exit(make_exit(1, 100, [1.0, 0.0, 0.0, 0.0], 1_000_000))
            .unwrap();

        // Very different embedding
        let entry = make_entry(2, 200, [0.0, 0.0, 0.0, 1.0], 5_000_000);
        let result = tracker.match_entry(&entry).unwrap();

        assert!(!result.matched);
        assert!(result.transition.is_none());
    }

    #[test]
    fn test_no_match_temporal_gap_exceeded() {
        let mut tracker = CrossRoomTracker::new(small_config());

        tracker
            .record_exit(make_exit(1, 100, [1.0, 0.0, 0.0, 0.0], 0))
            .unwrap();

        // Same embedding but 120 seconds later
        let entry = make_entry(2, 200, [1.0, 0.0, 0.0, 0.0], 120_000_000);
        let result = tracker.match_entry(&entry).unwrap();

        assert!(!result.matched, "Should not match with > 60s gap");
    }

    #[test]
    fn test_no_match_same_room() {
        let mut tracker = CrossRoomTracker::new(small_config());

        tracker
            .record_exit(make_exit(1, 100, [1.0, 0.0, 0.0, 0.0], 1_000_000))
            .unwrap();

        // Entry in same room should not match
        let entry = make_entry(1, 200, [1.0, 0.0, 0.0, 0.0], 2_000_000);
        let result = tracker.match_entry(&entry).unwrap();

        assert!(!result.matched, "Same-room entry should not match");
    }

    #[test]
    fn test_expire_exits() {
        let mut tracker = CrossRoomTracker::new(small_config());

        tracker
            .record_exit(make_exit(1, 100, [1.0, 0.0, 0.0, 0.0], 0))
            .unwrap();
        tracker
            .record_exit(make_exit(2, 200, [0.0, 1.0, 0.0, 0.0], 50_000_000))
            .unwrap();

        assert_eq!(tracker.pending_exit_count(), 2);

        // Expire at 70s — first exit (at 0) should be expired
        tracker.expire_exits(70_000_000);
        assert_eq!(tracker.pending_exit_count(), 1);
    }

    #[test]
    fn test_transition_log_immutable() {
        let mut tracker = CrossRoomTracker::new(small_config());

        tracker
            .record_exit(make_exit(1, 100, [1.0, 0.0, 0.0, 0.0], 1_000_000))
            .unwrap();

        let entry = make_entry(2, 200, [0.98, 0.02, 0.0, 0.0], 2_000_000);
        tracker.match_entry(&entry).unwrap();

        assert_eq!(tracker.transition_count(), 1);

        // More transitions should append
        tracker
            .record_exit(make_exit(2, 300, [0.0, 1.0, 0.0, 0.0], 3_000_000))
            .unwrap();
        let entry2 = make_entry(3, 400, [0.01, 0.99, 0.0, 0.0], 4_000_000);
        tracker.match_entry(&entry2).unwrap();

        assert_eq!(tracker.transition_count(), 2);
    }

    #[test]
    fn test_transitions_between_rooms() {
        let mut tracker = CrossRoomTracker::new(small_config());

        // Room 1 → Room 2
        tracker
            .record_exit(make_exit(1, 100, [1.0, 0.0, 0.0, 0.0], 1_000_000))
            .unwrap();
        let entry = make_entry(2, 200, [0.98, 0.02, 0.0, 0.0], 2_000_000);
        tracker.match_entry(&entry).unwrap();

        // Room 2 → Room 3
        tracker
            .record_exit(make_exit(2, 300, [0.0, 1.0, 0.0, 0.0], 3_000_000))
            .unwrap();
        let entry2 = make_entry(3, 400, [0.01, 0.99, 0.0, 0.0], 4_000_000);
        tracker.match_entry(&entry2).unwrap();

        let r1_r2 = tracker.transitions_between(1, 2);
        assert_eq!(r1_r2.len(), 1);

        let r2_r3 = tracker.transitions_between(2, 3);
        assert_eq!(r2_r3.len(), 1);

        let r1_r3 = tracker.transitions_between(1, 3);
        assert_eq!(r1_r3.len(), 0);
    }

    #[test]
    fn test_embedding_dimension_mismatch() {
        let mut tracker = CrossRoomTracker::new(small_config());

        let bad_exit = ExitEvent {
            embedding: vec![1.0, 0.0], // wrong dim
            room_id: 1,
            track_id: 1,
            timestamp_us: 0,
            matched: false,
        };
        assert!(matches!(
            tracker.record_exit(bad_exit),
            Err(CrossRoomError::EmbeddingDimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0_f32, 2.0, 3.0, 4.0];
        let sim = cosine_similarity_f32(&a, &a);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0_f32, 0.0, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0, 0.0];
        let sim = cosine_similarity_f32(&a, &b);
        assert!(sim.abs() < 1e-5);
    }
}
