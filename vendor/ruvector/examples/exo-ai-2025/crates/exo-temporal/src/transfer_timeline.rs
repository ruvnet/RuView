//! Phase 3 – Transfer Timeline
//!
//! Records domain transfer events in the EXO temporal causal graph so the
//! system can review its own transfer history and anticipate the next
//! beneficial `(src, dst)` pair to activate.

use ruvector_domain_expansion::DomainId;

use crate::{
    AnticipationHint, ConsolidationConfig, ConsolidationResult, TemporalConfig, TemporalMemory,
};
use exo_core::{Metadata, Pattern, PatternId, SubstrateTime};

const DIM: usize = 64;

// ─── embedding helpers ────────────────────────────────────────────────────────

/// FNV-1a hash of a string, normalised to [0, 1].
fn domain_hash(id: &str) -> f32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h as f32 / u32::MAX as f32
}

/// Build a 64-dim pattern embedding for a transfer event.
///
/// Layout:
/// * `[0]`      – src domain hash (normalised)
/// * `[1]`      – dst domain hash (normalised)
/// * `[2]`      – cycle (log-normalised to [0, 1] over 1 000 cycles)
/// * `[3]`      – delta_reward (clamped to [0, 1])
/// * `[4..64]`  – sinusoidal harmonics of `(src_hash + dst_hash)`
fn build_embedding(src: &DomainId, dst: &DomainId, cycle: u64, delta_reward: f32) -> Vec<f32> {
    let mut emb = vec![0.0f32; DIM];
    let sh = domain_hash(&src.0);
    let dh = domain_hash(&dst.0);
    emb[0] = sh;
    emb[1] = dh;
    emb[2] = (cycle as f32).ln_1p() / (1_000.0_f32).ln_1p();
    emb[3] = delta_reward.clamp(0.0, 1.0);
    for i in 4..DIM {
        let phase = (sh + dh) * i as f32 * std::f32::consts::PI / DIM as f32;
        emb[i] = phase.sin() * 0.5 + 0.5;
    }
    emb
}

// ─── TransferTimeline ─────────────────────────────────────────────────────────

/// Records transfer events in the temporal causal graph and provides
/// anticipation hints for the next beneficial transfer.
pub struct TransferTimeline {
    memory: TemporalMemory,
    last_transfer_id: Option<PatternId>,
    /// Total transfer events recorded (short-term + consolidated).
    count: usize,
}

impl TransferTimeline {
    /// Create with a low salience threshold so even weak transfers are kept.
    pub fn new() -> Self {
        let config = TemporalConfig {
            consolidation: ConsolidationConfig {
                salience_threshold: 0.1,
                ..Default::default()
            },
            ..Default::default()
        };
        Self {
            memory: TemporalMemory::new(config),
            last_transfer_id: None,
            count: 0,
        }
    }

    /// Record a transfer event.
    ///
    /// `delta_reward` is the improvement in arm reward after transfer
    /// (`> 0` = positive transfer, `< 0` = negative transfer).
    ///
    /// Each event is linked causally to the previous one so the temporal
    /// causal graph can trace the full transfer trajectory.
    pub fn record_transfer(
        &mut self,
        src: &DomainId,
        dst: &DomainId,
        cycle: u64,
        delta_reward: f32,
    ) -> crate::Result<PatternId> {
        let embedding = build_embedding(src, dst, cycle, delta_reward);
        let salience = delta_reward.abs().clamp(0.1, 1.0);

        let antecedents: Vec<PatternId> = self.last_transfer_id.iter().copied().collect();
        let pattern = Pattern {
            id: PatternId::new(),
            embedding,
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: antecedents.clone(),
            salience,
        };
        let id = self.memory.store(pattern, &antecedents)?;
        self.last_transfer_id = Some(id);
        self.count += 1;
        Ok(id)
    }

    /// Consolidate short-term transfer events to long-term memory.
    pub fn consolidate(&self) -> ConsolidationResult {
        self.memory.consolidate()
    }

    /// Return anticipation hints based on recent transfer causality.
    ///
    /// If a previous transfer was recorded the hints suggest continuing
    /// the same causal chain and sequential pattern.
    pub fn anticipate_next(&self) -> Vec<AnticipationHint> {
        match self.last_transfer_id {
            Some(id) => vec![
                AnticipationHint::CausalChain { context: id },
                AnticipationHint::SequentialPattern { recent: vec![id] },
            ],
            None => vec![],
        }
    }

    /// Total number of transfer events recorded.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Causal graph reference for advanced queries.
    pub fn causal_graph(&self) -> &crate::CausalGraph {
        self.memory.causal_graph()
    }
}

impl Default for TransferTimeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_count() {
        let mut tl = TransferTimeline::new();
        let src = DomainId("retrieval".to_string());
        let dst = DomainId("graph".to_string());

        tl.record_transfer(&src, &dst, 1, 0.3).unwrap();
        tl.record_transfer(&src, &dst, 2, 0.5).unwrap();
        assert_eq!(tl.count(), 2);
    }

    #[test]
    fn test_consolidate() {
        let mut tl = TransferTimeline::new();
        let src = DomainId("a".to_string());
        let dst = DomainId("b".to_string());
        for i in 0..5 {
            tl.record_transfer(&src, &dst, i, 0.4).unwrap();
        }
        let result = tl.consolidate();
        assert!(result.num_consolidated >= 1);
    }

    #[test]
    fn test_anticipate_empty() {
        let tl = TransferTimeline::new();
        assert!(tl.anticipate_next().is_empty());
    }

    #[test]
    fn test_anticipate_after_record() {
        let mut tl = TransferTimeline::new();
        let src = DomainId("x".to_string());
        let dst = DomainId("y".to_string());
        tl.record_transfer(&src, &dst, 1, 0.4).unwrap();
        let hints = tl.anticipate_next();
        assert!(!hints.is_empty());
    }

    #[test]
    fn test_embedding_values() {
        let src = DomainId("retrieval".to_string());
        let dst = DomainId("graph".to_string());
        let emb = build_embedding(&src, &dst, 42, 0.7);
        assert_eq!(emb.len(), DIM);
        assert!((emb[3] - 0.7).abs() < 1e-6);
    }
}
