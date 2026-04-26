//! ADR-084 Pass 5 — privacy-preserving event log.
//!
//! Stores `(timestamp, sketch, novelty, witness_sha256)` tuples instead
//! of raw float embeddings. Two privacy properties matter:
//!
//! 1. **Non-invertibility.** The 1-bit sketch is lossy — there is no
//!    general mathematical inverse from a stored event back to a
//!    `[f32]` source embedding. Even an attacker with side-channel
//!    information about the embedding model's output distribution
//!    cannot reconstruct the underlying CSI.
//!
//! 2. **Content addressing.** Each event carries a SHA-256 of the
//!    serialized [`crate::WireSketch`] payload (header + packed bits).
//!    Two events with the same `witness` are byte-equal — the cluster-Pi
//!    can deduplicate, the gateway can checkpoint without re-storing,
//!    and downstream verifiers can prove "this event came from that
//!    sketch" without ever holding the original embedding.
//!
//! See ADR-084 §"Privacy-preserving event log" and the post-merge
//! security review on PR #435 (finding L7) for context.
//!
//! # Bounded by design
//!
//! [`PrivacyEventLog`] is a fixed-capacity ring buffer; once full,
//! oldest events are FIFO-evicted. A misbehaving sender cannot exhaust
//! receiver memory by flooding the bank — peak footprint is
//! `capacity × (sketch_bytes + 50)` bytes.

use sha2::{Digest, Sha256};
use std::collections::VecDeque;

use crate::sketch::{Sketch, WireSketch};

/// One entry in the privacy-preserving event log.
///
/// All fields are public so callers can serialize / inspect / forward
/// events through their own pipelines without going through getters.
/// The struct is intentionally self-contained — no references to
/// external state, so an event can be moved across thread / process /
/// host boundaries without dangling.
#[derive(Debug, Clone, PartialEq)]
pub struct NoveltyEvent {
    /// Microseconds since UNIX epoch when the underlying frame was
    /// observed. Caller-supplied; the event log doesn't fetch the
    /// clock so test fixtures are deterministic.
    pub timestamp_us: u64,
    /// 1-bit packed sketch bytes (`(embedding_dim + 7) / 8` bytes long).
    pub sketch_bytes: Vec<u8>,
    /// Embedding-model schema version so `(version, witness)` is a
    /// fully qualified content address.
    pub sketch_version: u16,
    /// Source-embedding dimension, fixing the bit count of `sketch_bytes`.
    pub embedding_dim: u16,
    /// Novelty score in `[0.0, 1.0]` at the time the event was logged.
    /// Saturated and stored as f32 for direct downstream use; the q15
    /// quantization happens on the wire format
    /// ([`crate::WireSketch`]) — the in-memory log keeps full f32
    /// precision.
    pub novelty: f32,
    /// SHA-256 of the serialized [`crate::WireSketch`] payload
    /// (header + packed bits + the q15 novelty quantum). Two events
    /// with the same witness are byte-identical on the wire.
    pub witness_sha256: [u8; 32],
}

/// Fixed-capacity, FIFO-evicting log of [`NoveltyEvent`]s.
///
/// Used as the cluster-Pi's per-node anomaly trail. The log is **not**
/// the source of truth for novelty (that's [`crate::SketchBank`] and
/// `EmbeddingHistory::novelty`); it's the *audit* of what happened.
///
/// # Memory bound
///
/// `capacity * (sketch_bytes_per_event + ~50 fixed bytes)` is the worst
/// case. For 64 events × 16-byte sketches that's ~4 KiB — fits in any
/// per-node state struct without concern.
#[derive(Debug, Clone)]
pub struct PrivacyEventLog {
    capacity: usize,
    events: VecDeque<NoveltyEvent>,
}

impl PrivacyEventLog {
    /// Create a new log with the given fixed capacity.
    ///
    /// `capacity == 0` is allowed; the log accepts pushes but
    /// immediately discards them, which is occasionally useful as a
    /// no-op stub in test fixtures or when the privacy log is meant
    /// to be disabled at deployment time.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            events: VecDeque::with_capacity(capacity.min(1024)),
        }
    }

    /// Append an event built from a `Sketch` + novelty score.
    ///
    /// The event's `witness_sha256` is computed over the [`WireSketch`]
    /// serialization of `(sketch, novelty)` — so two pushes of the same
    /// `(sketch, novelty)` produce byte-identical witnesses, enabling
    /// dedup at the receiver.
    ///
    /// FIFO-evicts the oldest event if the log is at capacity. Returns
    /// the number of events present after the push (0 when capacity is
    /// 0, otherwise `<= capacity`).
    pub fn push(&mut self, sketch: &Sketch, novelty: f32, timestamp_us: u64) -> usize {
        if self.capacity == 0 {
            return 0;
        }
        let wire = WireSketch::serialize(sketch, novelty);
        let mut hasher = Sha256::new();
        hasher.update(&wire);
        let witness: [u8; 32] = hasher.finalize().into();

        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(NoveltyEvent {
            timestamp_us,
            sketch_bytes: sketch.packed_bytes().to_vec(),
            sketch_version: sketch.sketch_version(),
            embedding_dim: sketch.embedding_dim(),
            novelty,
            witness_sha256: witness,
        });
        self.events.len()
    }

    /// Number of events currently stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// True iff the log has no events.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Bank capacity (the max number of events ever held simultaneously).
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Iterate over events oldest-first.
    pub fn iter(&self) -> impl Iterator<Item = &NoveltyEvent> {
        self.events.iter()
    }

    /// Find the most recent event whose `witness_sha256` matches.
    /// Returns `None` if no event matches.
    ///
    /// Used by content-addressable lookups — a downstream receiver
    /// can ask "have you logged this exact `(sketch, novelty)` before?"
    /// without re-transmitting the sketch.
    pub fn find_by_witness(&self, witness: &[u8; 32]) -> Option<&NoveltyEvent> {
        self.events
            .iter()
            .rev()
            .find(|e| &e.witness_sha256 == witness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::Sketch;

    fn make_sketch(seed: u32) -> Sketch {
        let v: Vec<f32> = (0..32)
            .map(|i| ((i as u32).wrapping_mul(seed) as f32).sin())
            .collect();
        Sketch::from_embedding(&v, 1)
    }

    #[test]
    fn push_grows_until_capacity_then_fifo_evicts() {
        let mut log = PrivacyEventLog::new(3);
        for i in 0..5u64 {
            log.push(&make_sketch(i as u32 + 1), 0.5, i * 1000);
        }
        assert_eq!(log.len(), 3, "must cap at capacity");
        // Oldest two evicted; first remaining timestamp is 2_000.
        let first = log.iter().next().unwrap();
        assert_eq!(first.timestamp_us, 2000);
    }

    #[test]
    fn zero_capacity_log_silently_drops_pushes() {
        let mut log = PrivacyEventLog::new(0);
        let n = log.push(&make_sketch(1), 0.5, 0);
        assert_eq!(n, 0);
        assert_eq!(log.len(), 0);
        assert!(log.is_empty());
    }

    #[test]
    fn witness_is_deterministic_for_same_sketch_and_novelty() {
        let mut log_a = PrivacyEventLog::new(2);
        let mut log_b = PrivacyEventLog::new(2);
        let s = make_sketch(7);
        // Same sketch + same novelty + (intentionally different)
        // timestamps — witness must NOT depend on timestamp; the
        // wire format does not include it.
        log_a.push(&s, 0.25, 100);
        log_b.push(&s, 0.25, 999_999);
        let wa = log_a.iter().next().unwrap().witness_sha256;
        let wb = log_b.iter().next().unwrap().witness_sha256;
        assert_eq!(wa, wb, "witness must be content-addressable, not time-addressable");
    }

    #[test]
    fn witness_differs_for_different_novelty_scores() {
        let mut log = PrivacyEventLog::new(2);
        let s = make_sketch(11);
        log.push(&s, 0.10, 0);
        log.push(&s, 0.90, 0);
        let mut iter = log.iter();
        let w0 = iter.next().unwrap().witness_sha256;
        let w1 = iter.next().unwrap().witness_sha256;
        assert_ne!(w0, w1, "different novelty → different witness");
    }

    #[test]
    fn find_by_witness_returns_most_recent_match() {
        let mut log = PrivacyEventLog::new(5);
        let s = make_sketch(42);
        log.push(&s, 0.5, 100);
        log.push(&make_sketch(99), 0.3, 200);
        log.push(&s, 0.5, 300); // duplicate by witness, newer timestamp

        let target_witness = log.iter().nth(2).unwrap().witness_sha256;
        let hit = log.find_by_witness(&target_witness).unwrap();
        assert_eq!(hit.timestamp_us, 300, "find_by_witness returns most recent");
    }

    #[test]
    fn find_by_witness_returns_none_on_miss() {
        let mut log = PrivacyEventLog::new(2);
        log.push(&make_sketch(1), 0.5, 0);
        let bogus = [0xAA_u8; 32];
        assert!(log.find_by_witness(&bogus).is_none());
    }

    #[test]
    fn event_does_not_carry_raw_embedding() {
        // The whole point of the event log: an attacker with read
        // access to the log cannot recover the source CSI / embedding.
        // Verify structurally that no `Vec<f32>` field exists on
        // NoveltyEvent — only the bit-packed sketch.
        let mut log = PrivacyEventLog::new(1);
        let s = make_sketch(5);
        log.push(&s, 0.5, 0);
        let event = log.iter().next().unwrap();
        // The packed sketch is bytes (1-bit-per-source-dim, ceil-divided).
        // Length proves the source dim (32 bits = 4 bytes).
        assert_eq!(event.sketch_bytes.len(), 4);
        assert_eq!(event.embedding_dim, 32);
        // No way to reconstruct the original `[f32; 32]` from these 4 bytes
        // alone; that's the privacy guarantee. (Compile-time witnessed:
        // there's no Vec<f32> field on NoveltyEvent.)
    }
}
