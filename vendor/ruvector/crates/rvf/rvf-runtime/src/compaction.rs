//! Background compaction for dead space reclamation.
//!
//! Compaction scheduling policy (from spec 10, section 7):
//! - IO budget: max 30% of IOPS (60% in emergency)
//! - Priority: queries > ingest > compaction
//! - Triggers: dead_space > 20%, segment_count > 32, time > 60s
//! - Emergency: dead_space > 70% -> preempt ingest
//!
//! Segment selection order:
//! 1. Tombstoned segments (reclaim dead space)
//! 2. Small VEC_SEGs (< 1MB, merge into larger)
//! 3. High-overlap INDEX_SEGs
//! 4. Cold OVERLAY_SEGs

/// Compaction trigger thresholds.
#[allow(dead_code)]
pub(crate) struct CompactionThresholds {
    /// Minimum dead space ratio to trigger compaction.
    pub dead_space_ratio: f64,
    /// Maximum segment count before compaction.
    pub max_segment_count: u32,
    /// Minimum seconds since last compaction.
    pub min_interval_secs: u64,
    /// Emergency dead space ratio (preempts ingest).
    pub emergency_ratio: f64,
}

impl Default for CompactionThresholds {
    fn default() -> Self {
        Self {
            dead_space_ratio: 0.20,
            max_segment_count: 32,
            min_interval_secs: 60,
            emergency_ratio: 0.70,
        }
    }
}

/// Compaction decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum CompactionDecision {
    /// No compaction needed.
    None,
    /// Normal compaction should run.
    Normal,
    /// Emergency compaction (high dead space).
    Emergency,
}

/// Evaluate whether compaction should run.
#[allow(dead_code)]
pub(crate) fn evaluate_triggers(
    dead_space_ratio: f64,
    segment_count: u32,
    secs_since_last: u64,
    thresholds: &CompactionThresholds,
) -> CompactionDecision {
    // Emergency check first.
    if dead_space_ratio > thresholds.emergency_ratio {
        return CompactionDecision::Emergency;
    }

    // Check all normal conditions.
    if secs_since_last < thresholds.min_interval_secs {
        return CompactionDecision::None;
    }

    if dead_space_ratio > thresholds.dead_space_ratio {
        return CompactionDecision::Normal;
    }

    if segment_count > thresholds.max_segment_count {
        return CompactionDecision::Normal;
    }

    CompactionDecision::None
}

/// Represents a compaction plan: which segments to compact and how.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct CompactionPlan {
    /// Segment IDs to compact (input).
    pub source_segments: Vec<u64>,
    /// Whether this is emergency compaction.
    pub emergency: bool,
    /// IO budget as a fraction (0.30 normal, 0.60 emergency).
    pub io_budget: f64,
}

impl CompactionPlan {
    /// Create a normal compaction plan.
    #[allow(dead_code)]
    pub(crate) fn normal(segments: Vec<u64>) -> Self {
        Self {
            source_segments: segments,
            emergency: false,
            io_budget: 0.30,
        }
    }

    /// Create an emergency compaction plan.
    #[allow(dead_code)]
    pub(crate) fn emergency(segments: Vec<u64>) -> Self {
        Self {
            source_segments: segments,
            emergency: true,
            io_budget: 0.60,
        }
    }
}

/// Select segments for compaction based on the tiered strategy.
///
/// Priority:
/// 1. Tombstoned segments
/// 2. Small VEC_SEGs (< threshold)
/// 3. Remaining segments by age
#[allow(dead_code)]
pub(crate) fn select_segments(
    segment_dir: &[(u64, u64, u8, bool)], // (seg_id, payload_len, seg_type, is_tombstoned)
    max_segments: usize,
) -> Vec<u64> {
    let mut selected = Vec::new();

    // Phase 1: tombstoned segments.
    for &(seg_id, _, _, tombstoned) in segment_dir {
        if tombstoned && selected.len() < max_segments {
            selected.push(seg_id);
        }
    }

    // Phase 2: small VEC_SEGs (< 1MB).
    let small_threshold = 1024 * 1024;
    for &(seg_id, payload_len, seg_type, _) in segment_dir {
        if seg_type == 0x01
            && payload_len < small_threshold
            && selected.len() < max_segments
            && !selected.contains(&seg_id)
        {
            selected.push(seg_id);
        }
    }

    // Phase 3: fill remaining with oldest segments.
    for &(seg_id, _, _, _) in segment_dir {
        if selected.len() >= max_segments {
            break;
        }
        if !selected.contains(&seg_id) {
            selected.push(seg_id);
        }
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_compaction_when_fresh() {
        let decision = evaluate_triggers(0.10, 10, 30, &CompactionThresholds::default());
        assert_eq!(decision, CompactionDecision::None);
    }

    #[test]
    fn normal_compaction_on_dead_space() {
        let decision = evaluate_triggers(0.25, 10, 120, &CompactionThresholds::default());
        assert_eq!(decision, CompactionDecision::Normal);
    }

    #[test]
    fn normal_compaction_on_segment_count() {
        let decision = evaluate_triggers(0.10, 50, 120, &CompactionThresholds::default());
        assert_eq!(decision, CompactionDecision::Normal);
    }

    #[test]
    fn emergency_compaction_on_high_dead_space() {
        let decision = evaluate_triggers(0.75, 10, 10, &CompactionThresholds::default());
        assert_eq!(decision, CompactionDecision::Emergency);
    }

    #[test]
    fn no_compaction_before_interval() {
        let decision = evaluate_triggers(0.25, 50, 30, &CompactionThresholds::default());
        // Even though dead_space and segment_count exceed thresholds,
        // interval hasn't passed.
        assert_eq!(decision, CompactionDecision::None);
    }

    #[test]
    fn select_tombstoned_first() {
        let segments = vec![
            (1, 500_000, 0x01, false),
            (2, 100_000, 0x01, true), // tombstoned
            (3, 200_000, 0x01, false),
            (4, 50_000, 0x01, true), // tombstoned
        ];
        let selected = select_segments(&segments, 3);
        // Tombstoned segments (2, 4) should come first.
        assert_eq!(selected[0], 2);
        assert_eq!(selected[1], 4);
        assert_eq!(selected.len(), 3);
    }
}
