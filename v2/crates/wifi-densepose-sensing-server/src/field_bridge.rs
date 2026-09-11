//! Bridge between sensing-server frame data and signal crate FieldModel
//! for eigenvalue-based person counting.
//!
//! The FieldModel decomposes CSI observations into environmental drift and
//! body perturbation via SVD eigenmodes. When calibrated, perturbation energy
//! provides a physics-grounded occupancy estimate that supplements the
//! score-based heuristic in `score_to_person_count`.

use std::collections::VecDeque;
use std::sync::LazyLock;
use wifi_densepose_signal::hardware_norm::HardwareNormalizer;
use wifi_densepose_signal::ruvsense::field_model::{
    CalibrationStatus, FieldModel, FieldModelConfig,
};

use super::score_to_person_count;

/// Versioned schema for the immutable receipt emitted after field-model calibration.
pub const CALIBRATION_MODEL_RECEIPT_SCHEMA: &str =
    "ruview.calibration.field-model-receipt.v1";

/// Length-only canonicalizer for calibration frames (issue #1170 pattern,
/// shared with `multistatic_bridge`). Raw ESP32 amplitudes arrive at the
/// hardware's native width (HT20 ≈ 64, HT40 ≈ 128/192); the FieldModel is
/// configured for the canonical 56-tone grid, and `feed_calibration` rejects
/// any other width with `DimensionMismatch`. Resampling here (default 56)
/// lets real HT40 nodes actually calibrate instead of silently feeding nothing.
static CALIB_NORMALIZER: LazyLock<HardwareNormalizer> = LazyLock::new(HardwareNormalizer::new);

/// Number of recent frames to feed into perturbation extraction.
const OCCUPANCY_WINDOW: usize = 50;

/// Perturbation energy threshold for detecting a second person.
const ENERGY_THRESH_2: f64 = 12.0;
/// Perturbation energy threshold for detecting a third person.
const ENERGY_THRESH_3: f64 = 25.0;

/// Maximum occupancy a single ESP32 link can plausibly resolve (#894).
/// The score heuristic (`score_to_person_count`) and the perturbation-energy
/// fallback below both cap here; the eigenvalue path is bounded to match,
/// rather than leaking its internal `min(10)` ceiling on noisy / under-
/// calibrated CSI (the "10 persons reported when 1 present" symptom).
/// Resolving more than this from one link's subcarrier covariance is not
/// reliable — genuine higher counts come from the multistatic fusion path.
const MAX_SINGLE_LINK_OCCUPANCY: usize = 3;

/// Low-authority comparison of the current source window with a restored
/// empty-room bootstrap image. This is background conformance only and cannot
/// authorize presence, person count, pose, identity, or vital signs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BootstrapBackgroundMatch {
    pub matches_empty: bool,
    pub score: Option<f64>,
    pub normalized_residual_z: Option<f64>,
    pub maturity: f64,
    pub reliable: bool,
    pub residual_energy: f64,
    pub residual_energy_threshold: f64,
    pub window_size: usize,
    pub reference_window_count: usize,
}

fn mean_frame(frames: &[Vec<f64>]) -> Option<Vec<f64>> {
    let width = frames.first()?.len();
    if width == 0 {
        return None;
    }
    let mut mean = vec![0.0_f64; width];
    let mut count = 0_usize;
    for frame in frames {
        if frame.len() != width {
            continue;
        }
        for (sum, value) in mean.iter_mut().zip(frame.iter()) {
            *sum += value;
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    for value in &mut mean {
        *value /= count as f64;
    }
    Some(mean)
}

fn perturbation_occupancy(field: &FieldModel, frames: &[Vec<f64>]) -> Option<usize> {
    let adaptive_threshold = field.empty_room_residual_energy_threshold();
    let frame = match adaptive_threshold {
        Some(_) => mean_frame(frames)?,
        None => frames.first()?.clone(),
    };
    let perturbation = field.extract_perturbation(&[frame]).ok()?;
    let count = match adaptive_threshold {
        Some(threshold) => {
            let empty_threshold = threshold.max(1.0);
            if perturbation.total_energy > empty_threshold * 25.0 {
                3
            } else if perturbation.total_energy > empty_threshold * 12.0 {
                2
            } else if perturbation.total_energy > empty_threshold {
                1
            } else {
                0
            }
        }
        None => {
            if perturbation.total_energy > ENERGY_THRESH_3 {
                3
            } else if perturbation.total_energy > ENERGY_THRESH_2 {
                2
            } else if perturbation.total_energy > 1.0 {
                1
            } else {
                0
            }
        }
    };
    Some(count)
}

/// Provenance for a count produced by the calibrated field model. Callers
/// that attach calibration evidence must never substitute the score heuristic
/// when the calibrated path cannot evaluate the observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibratedOccupancyMethod {
    Eigenvalue,
    PerturbationEnergy,
}

impl CalibratedOccupancyMethod {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Eigenvalue => "field_model_eigenvalue_v1",
            Self::PerturbationEnergy => "field_model_perturbation_energy_v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibratedOccupancy {
    pub person_count: usize,
    pub method: CalibratedOccupancyMethod,
}

/// Create a FieldModelConfig for single-link mode (one ESP32 node = one link).
/// This avoids the DimensionMismatch error when feeding single-frame observations.
pub fn single_link_config() -> FieldModelConfig {
    FieldModelConfig {
        n_links: 1,
        ..FieldModelConfig::default()
    }
}

/// Resolve the model status at the observation wall clock. `FieldModel::status`
/// records the state at collection or restore time and does not advance as a
/// long-running server crosses the stale and expiry boundaries.
pub fn calibration_status_at(field: &FieldModel, observed_at_us: u64) -> CalibrationStatus {
    match field.status() {
        CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting => field.status(),
        CalibrationStatus::Fresh | CalibrationStatus::Stale | CalibrationStatus::Expired => {
            field.check_freshness(observed_at_us)
        }
    }
}

/// Estimate occupancy using the FieldModel when calibrated, falling back
/// to the score-based heuristic otherwise.
///
/// Prefers `estimate_occupancy()` (eigenvalue-based) when the model is
/// calibrated and enough frames are available. Falls back to perturbation
/// energy thresholds, then to the score heuristic.
pub fn occupancy_or_fallback(
    field: &FieldModel,
    frame_history: &VecDeque<Vec<f64>>,
    observed_at_us: u64,
    smoothed_score: f64,
    prev_count: usize,
) -> usize {
    match calibration_status_at(field, observed_at_us) {
        CalibrationStatus::Fresh | CalibrationStatus::Stale => {
            let frames: Vec<Vec<f64>> = frame_history
                .iter()
                .rev()
                .take(OCCUPANCY_WINDOW)
                .map(|frame| CALIB_NORMALIZER.resample_to_canonical(frame))
                .collect();

            if frames.is_empty() {
                return score_to_person_count(smoothed_score, prev_count);
            }

            // Try eigenvalue-based occupancy first (best accuracy). Bound it to
            // the same single-link maximum the sibling estimators use — the
            // perturbation fallback below and score_to_person_count both cap at
            // MAX_SINGLE_LINK_OCCUPANCY. Without this, estimate_occupancy's
            // internal min(10) ceiling leaks up to 10 persons on noisy / under-
            // calibrated CSI (#894), while every other path on the same data
            // would report ≤3.
            if let Ok(count) = field.estimate_occupancy(&frames) {
                return count.min(MAX_SINGLE_LINK_OCCUPANCY);
            } // else fall through to perturbation energy

            // Fallback: perturbation energy thresholds.
            // FieldModel expects [n_links][n_subcarriers] — we use n_links=1.
            perturbation_occupancy(field, &frames)
                .unwrap_or_else(|| score_to_person_count(smoothed_score, prev_count))
        }
        _ => score_to_person_count(smoothed_score, prev_count),
    }
}

/// Return true only when a restored low-authority bootstrap model has a full
/// runtime window and that model resolves the window as empty. This helper is
/// deliberately one-way: a bootstrap prior may suppress an obvious background
/// false positive, but cannot authorize presence or vital signs.
pub fn bootstrap_empty_prior_applies(
    field: &FieldModel,
    frame_history: &VecDeque<Vec<f64>>,
    observed_at_us: u64,
) -> bool {
    bootstrap_background_match(field, frame_history, observed_at_us)
        .is_some_and(|result| result.matches_empty)
}

/// Score the current canonical runtime window against the learned empty-room
/// residual references. Legacy models without scalar references preserve the
/// binary match but intentionally return no score.
pub fn bootstrap_background_match(
    field: &FieldModel,
    frame_history: &VecDeque<Vec<f64>>,
    observed_at_us: u64,
) -> Option<BootstrapBackgroundMatch> {
    if frame_history.len() < OCCUPANCY_WINDOW
        || field.check_freshness(observed_at_us) != CalibrationStatus::Fresh
    {
        return None;
    }
    let frames: Vec<Vec<f64>> = frame_history
        .iter()
        .rev()
        .take(OCCUPANCY_WINDOW)
        .map(|frame| CALIB_NORMALIZER.resample_to_canonical(frame))
        .collect();
    let result = field.empty_room_match(&frames)?;
    Some(BootstrapBackgroundMatch {
        matches_empty: result.matches_empty,
        score: result.score,
        normalized_residual_z: result.normalized_residual_z,
        maturity: result.maturity,
        reliable: result.reliable,
        residual_energy: result.residual_energy,
        residual_energy_threshold: result.residual_energy_threshold,
        window_size: result.window_size,
        reference_window_count: result.reference_window_count,
    })
}

/// Produce a fail-closed occupancy result from a fresh calibrated model.
pub fn calibrated_occupancy(
    field: &FieldModel,
    frame_history: &VecDeque<Vec<f64>>,
    observed_at_us: u64,
) -> Option<CalibratedOccupancy> {
    if field.check_freshness(observed_at_us) != CalibrationStatus::Fresh {
        return None;
    }
    let frames: Vec<Vec<f64>> = frame_history
        .iter()
        .rev()
        .take(OCCUPANCY_WINDOW)
        .map(|frame| CALIB_NORMALIZER.resample_to_canonical(frame))
        .collect();
    if frames.is_empty() {
        return None;
    }
    if let Ok(person_count) = field.estimate_occupancy(&frames) {
        return Some(CalibratedOccupancy {
            person_count: person_count.min(MAX_SINGLE_LINK_OCCUPANCY),
            method: CalibratedOccupancyMethod::Eigenvalue,
        });
    }
    Some(CalibratedOccupancy {
        person_count: perturbation_occupancy(field, &frames)?,
        method: CalibratedOccupancyMethod::PerturbationEnergy,
    })
}

/// Feed the current CSI frame to the FieldModel during calibration.
///
/// Acts while the model is `Uncalibrated` or `Collecting`. The first fed frame
/// flips a freshly-started (`Uncalibrated`) model to `Collecting` inside
/// `feed_calibration`; without accepting the `Uncalibrated` state here the two
/// gates deadlock and the frame count never leaves 0 (calibration/start yields
/// an `Uncalibrated` model that nothing would ever advance). Edge-vitals
/// packets have no CSI observation and must never call this helper. The caller
/// owns the stateful sequence admission gate. Wraps the current frame as a
/// single-link observation (n_links=1) and feeds it.
pub fn maybe_feed_calibration(field: &mut FieldModel, amplitudes: &[f64]) -> bool {
    if !matches!(
        field.status(),
        CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting
    ) || amplitudes.is_empty()
    {
        return false;
    }
    // Resample the raw amplitude vector onto the FieldModel's canonical
    // 56-tone grid before feeding. Real HT40 nodes stream 128-wide frames;
    // feeding those raw made every `feed_calibration` fail DimensionMismatch
    // (swallowed at debug level), pinning frame_count at 0 even after the
    // status-gate deadlock was fixed. Single-link observation: [1][56].
    let canonical = CALIB_NORMALIZER.resample_to_canonical(amplitudes);
    let observations = vec![canonical];
    match field.feed_calibration(&observations) {
        Ok(()) => true,
        Err(e) => {
            tracing::debug!("FieldModel calibration feed: {e}");
            false
        }
    }
}

/// One parsed `--node-positions` entry: a position, and the node id if the
/// operator named one explicitly.
///
/// `node_id: None` means the entry was given positionally, so its identity is
/// its index in the list — which is only the same thing as a node id on a fleet
/// numbered `0, 1, 2, ...`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodePositionEntry {
    pub node_id: Option<u8>,
    pub position: [f32; 3],
}

/// Parse node positions, keeping any explicit node ids.
///
/// Two accepted forms, per entry:
///
/// * `x,y,z`           — positional; identity is the entry's index.
/// * `node_id:x,y,z`   — explicit; identity is the stated node id.
///
/// The explicit form exists because the positional one is only unambiguous on a
/// fleet whose ids happen to run `0, 1, 2, ...`. Anywhere else, "the third
/// entry" and "node 3" are different nodes, and nothing in the string says
/// which was meant. Naming the id removes the guess rather than relocating it.
///
/// Both forms may be mixed; a positional entry keeps its index as its identity.
/// Malformed entries are skipped with a warning rather than failing the boot —
/// a typo in one triplet should not take the server down.
pub fn parse_node_position_entries(input: &str) -> Vec<NodePositionEntry> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .split(';')
        .enumerate()
        .filter_map(|(idx, raw)| {
            let entry = raw.trim();
            if entry.is_empty() {
                return None;
            }
            // An explicit id is everything before the first ':'. Coordinates
            // never contain one, so this cannot be ambiguous.
            let (node_id, triplet) = match entry.split_once(':') {
                Some((id_str, rest)) => match id_str.trim().parse::<u8>() {
                    Ok(id) => (Some(id), rest),
                    Err(_) => {
                        tracing::warn!(
                            "Skipping node position entry {idx}: '{entry}' has \
                             an unparseable node id before ':' (expected \
                             node_id:x,y,z with node_id in 0..=255)"
                        );
                        return None;
                    }
                },
                None => (None, entry),
            };

            let parts: Vec<&str> = triplet.split(',').collect();
            if parts.len() != 3 {
                tracing::warn!(
                    "Skipping malformed node position entry {idx}: '{entry}' (expected x,y,z)"
                );
                return None;
            }
            match (
                parts[0].trim().parse::<f32>(),
                parts[1].trim().parse::<f32>(),
                parts[2].trim().parse::<f32>(),
            ) {
                (Ok(x), Ok(y), Ok(z)) => Some(NodePositionEntry {
                    node_id,
                    position: [x, y, z],
                }),
                _ => {
                    tracing::warn!("Skipping unparseable node position entry {idx}: '{entry}'");
                    None
                }
            }
        })
        .collect()
}

/// Build the `node_id -> position` map the live `NodeInfo` output reads.
///
/// Identity is the explicit `node_id:` prefix when the operator gave one, and
/// the entry's list index otherwise.
///
/// This exists as a function rather than a loop inside `main()` because the bug
/// it fixes was invisible without a test: positions were inserted keyed by list
/// index and read back by `node_id`, so on any fleet not numbered `0, 1, 2, ...`
/// every lookup missed and silently fell through to the hardcoded
/// `[2.0, 0.0, 1.5]` that `--node-positions` exists to replace. The config
/// looked applied and did nothing.
pub fn node_positions_by_id(
    entries: &[NodePositionEntry],
) -> std::collections::HashMap<u8, [f32; 3]> {
    let mut map = std::collections::HashMap::new();
    for (idx, e) in entries.iter().enumerate() {
        let key = e.node_id.unwrap_or(idx as u8);
        if let Some(prev) = map.insert(key, e.position) {
            tracing::warn!(
                "node position for node {key} given twice ({prev:?} then {:?}); the later entry wins",
                e.position
            );
        }
    }
    map
}

/// Parse node positions from a semicolon-delimited string.
///
/// Format: `"x,y,z;x,y,z;..."` where each coordinate is an `f32`.
/// Malformed entries are skipped with a warning log.
///
/// Positional view, for consumers that index the list rather than key it by
/// node id. Since issue #1866 the fusion path is no longer one of them: it
/// takes the keyed map from [`node_positions_by_id`] via
/// `MultistaticFuser::set_node_positions_by_id()`. Use
/// [`parse_node_position_entries`] where identity is needed.
pub fn parse_node_positions(input: &str) -> Vec<[f32; 3]> {
    parse_node_position_entries(input)
        .into_iter()
        .map(|e| e.position)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_test_model() -> FieldModel {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 10,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 2.0,
        };
        let mut field = FieldModel::new(config).expect("field model");
        for index in 0..20 {
            let phase = index as f64 * 0.1;
            let observation = vec![(0..56)
                .map(|subcarrier| {
                    1.0 + subcarrier as f64 * 0.01 + (phase + subcarrier as f64 * 0.03).sin() * 0.01
                })
                .collect()];
            field
                .feed_calibration(&observation)
                .expect("calibration frame");
        }
        field
            .finalize_calibration(1_000_000, 7)
            .expect("finalize calibration");
        field
    }

    #[test]
    fn test_parse_node_positions() {
        let positions = parse_node_positions("0,0,1.5;3,0,1.5;1.5,3,1.5");
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], [0.0, 0.0, 1.5]);
        assert_eq!(positions[1], [3.0, 0.0, 1.5]);
        assert_eq!(positions[2], [1.5, 3.0, 1.5]);
    }

    #[test]
    fn test_parse_node_positions_empty() {
        let positions = parse_node_positions("");
        assert!(positions.is_empty());
    }

    #[test]
    fn test_parse_node_positions_invalid() {
        let positions = parse_node_positions("abc;1,2,3");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_parse_node_positions_partial_triplet() {
        let positions = parse_node_positions("1,2;3,4,5");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], [3.0, 4.0, 5.0]);
    }

    // ---- Explicit node ids (the indexing-mismatch regression) ----

    /// Positional entries carry no id, so identity falls back to the index.
    /// This is the ONLY case the old behaviour handled correctly.
    #[test]
    fn positional_entries_report_no_explicit_id() {
        let e = parse_node_position_entries("0,0,1.5;3,0,1.5");
        assert_eq!(e.len(), 2);
        assert!(e.iter().all(|x| x.node_id.is_none()));
        assert_eq!(e[1].position, [3.0, 0.0, 1.5]);
    }

    /// THE REGRESSION. Positions were stored keyed by list index and read back
    /// by node_id, so on any fleet not numbered 0,1,2,... every lookup missed
    /// and fell through to the hardcoded default the option exists to replace.
    /// Non-sequential ids are what catch that.
    #[test]
    fn non_sequential_node_ids_are_preserved() {
        let e = parse_node_position_entries("11:1,2,3;12:4,5,6;13:7,8,9");
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].node_id, Some(11));
        assert_eq!(e[1].node_id, Some(12));
        assert_eq!(e[2].node_id, Some(13));
        assert_eq!(e[2].position, [7.0, 8.0, 9.0]);
        // The index would have been 0,1,2 -- none of which is a real node here.
        for (idx, entry) in e.iter().enumerate() {
            assert_ne!(
                entry.node_id,
                Some(idx as u8),
                "index and node id must not be conflated"
            );
        }
    }

    /// Two-digit ids, and a fleet re-homed to even numbers -- the shape of the
    /// planned hardware check (nine boards moved from 0..8 to 0,2,4,...,16).
    #[test]
    fn two_digit_and_even_numbered_fleet_ids_parse() {
        let spec = "0:0,0,1;2:1,0,1;4:2,0,1;6:3,0,1;8:4,0,1;10:5,0,1;12:6,0,1;14:7,0,1;16:8,0,1";
        let e = parse_node_position_entries(spec);
        assert_eq!(e.len(), 9);
        let ids: Vec<u8> = e.iter().map(|x| x.node_id.unwrap()).collect();
        assert_eq!(ids, vec![0, 2, 4, 6, 8, 10, 12, 14, 16]);
        assert_eq!(e[8].position, [8.0, 0.0, 1.0]);
    }

    /// A non-zero starting index must work too -- nothing requires node 0.
    #[test]
    fn a_fleet_that_does_not_start_at_zero_parses() {
        let e = parse_node_position_entries("18:1.5,2.5,3.5");
        assert_eq!(e[0].node_id, Some(18));
        assert_eq!(e[0].position, [1.5, 2.5, 3.5]);
    }

    /// Mixing is allowed: an unkeyed entry keeps its index.
    #[test]
    fn mixed_forms_keep_their_own_identity() {
        let e = parse_node_position_entries("0,0,1;7:9,9,9");
        assert_eq!(e[0].node_id, None);
        assert_eq!(e[1].node_id, Some(7));
    }

    /// An unparseable id is skipped rather than silently treated as positional
    /// -- guessing there would reintroduce the exact ambiguity being removed.
    #[test]
    fn an_unparseable_node_id_is_skipped_not_guessed() {
        let e = parse_node_position_entries("abc:1,2,3;5:4,5,6");
        assert_eq!(e.len(), 1, "only the well-formed entry survives");
        assert_eq!(e[0].node_id, Some(5));
    }

    /// 256 is out of range for a u8 node id and must not wrap to 0.
    #[test]
    fn an_out_of_range_node_id_is_rejected() {
        let e = parse_node_position_entries("256:1,2,3");
        assert!(e.is_empty(), "256 must not wrap to node 0");
    }

    /// The positional view stays byte-compatible for the fuser, which indexes
    /// the list rather than keying it.
    #[test]
    fn the_positional_view_is_unchanged_by_explicit_ids() {
        let p = parse_node_positions("11:1,2,3;12:4,5,6");
        assert_eq!(p, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    /// The regression itself, at the map that `NodeInfo` actually reads.
    ///
    /// The old code inserted keyed by list index and looked up by node_id. On a
    /// fleet numbered 11,12,13 that map holds keys 0,1,2, so every lookup misses
    /// and the node reports the hardcoded default. This asserts both halves:
    /// the real ids resolve, and the indices are absent.
    #[test]
    fn the_map_is_keyed_by_node_id_not_list_index() {
        let entries = parse_node_position_entries("11:1,2,3;12:4,5,6;13:7,8,9");
        let map = node_positions_by_id(&entries);

        assert_eq!(map.get(&11), Some(&[1.0, 2.0, 3.0]));
        assert_eq!(map.get(&12), Some(&[4.0, 5.0, 6.0]));
        assert_eq!(map.get(&13), Some(&[7.0, 8.0, 9.0]));

        for idx in 0u8..3 {
            assert!(
                map.get(&idx).is_none(),
                "list index {idx} must not be a key -- that is the bug: a lookup                  by node_id would miss and fall back to the hardcoded position"
            );
        }
    }

    /// Positional input keeps the old behaviour exactly: index becomes the key.
    /// This is what made the bug invisible on a 0..8 fleet.
    #[test]
    fn positional_input_still_keys_by_index() {
        let entries = parse_node_position_entries("0,0,1;3,0,1;6,0,1");
        let map = node_positions_by_id(&entries);
        assert_eq!(map.get(&0), Some(&[0.0, 0.0, 1.0]));
        assert_eq!(map.get(&1), Some(&[3.0, 0.0, 1.0]));
        assert_eq!(map.get(&2), Some(&[6.0, 0.0, 1.0]));
    }

    /// A re-homed fleet at 0,2,4,...,16 resolves on the real ids, and the odd
    /// indices in between are absent.
    #[test]
    fn an_even_numbered_fleet_resolves_by_id() {
        let spec = "0:0,0,1;2:1,0,1;4:2,0,1;6:3,0,1;8:4,0,1;10:5,0,1;12:6,0,1;14:7,0,1;16:8,0,1";
        let map = node_positions_by_id(&parse_node_position_entries(spec));
        assert_eq!(map.len(), 9);
        assert_eq!(map.get(&16), Some(&[8.0, 0.0, 1.0]));
        assert_eq!(map.get(&10), Some(&[5.0, 0.0, 1.0]));
        // Index-keyed would have produced 0..8; 1,3,5,7 are not real nodes here.
        for odd in [1u8, 3, 5, 7] {
            assert!(map.get(&odd).is_none(), "node {odd} does not exist in this fleet");
        }
    }

    /// A duplicate id keeps the later entry rather than silently holding two.
    #[test]
    fn a_duplicate_node_id_keeps_the_later_entry() {
        let map = node_positions_by_id(&parse_node_position_entries("4:1,1,1;4:2,2,2"));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&4), Some(&[2.0, 2.0, 2.0]));
    }

    /// Whitespace around entries and coordinates is tolerated.
    #[test]
    fn whitespace_is_tolerated() {
        let e = parse_node_position_entries(" 3 : 1.0 , 2.0 , 3.0 ");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].node_id, Some(3));
        assert_eq!(e[0].position, [1.0, 2.0, 3.0]);
    }

    /// Regression: a freshly-started (`Uncalibrated`) field model must begin
    /// collecting once frames arrive. Before the fix, `maybe_feed_calibration`
    /// only fed while already `Collecting`, but only `feed_calibration` sets
    /// `Collecting` — so the first frame was never fed and the count stayed 0.
    #[test]
    fn maybe_feed_calibration_advances_uncalibrated_to_collecting() {
        let mut field = FieldModel::new(single_link_config()).expect("field model");
        assert_eq!(field.status(), CalibrationStatus::Uncalibrated);
        assert_eq!(field.calibration_frame_count(), 0);

        // n_subcarriers defaults to 56; one single-link frame of that width.
        let frame = vec![0.5_f64; 56];
        assert!(maybe_feed_calibration(&mut field, &frame));

        assert_eq!(
            field.status(),
            CalibrationStatus::Collecting,
            "first frame must flip Uncalibrated -> Collecting"
        );
        assert_eq!(
            field.calibration_frame_count(),
            1,
            "frame count must advance past 0"
        );

        // A subsequent unique frame keeps accumulating while Collecting.
        assert!(maybe_feed_calibration(&mut field, &frame));
        assert_eq!(field.calibration_frame_count(), 2);
    }

    /// Regression (#1170 pattern): a real HT40 node streams 128-wide amplitude
    /// frames, but the FieldModel is a 56-tone grid. Before canonicalization,
    /// `feed_calibration` rejected every frame with DimensionMismatch (swallowed
    /// at debug), so frame_count stayed 0 even with the deadlock fixed. The feed
    /// must resample 128 → 56 and actually accumulate.
    #[test]
    fn maybe_feed_calibration_resamples_wide_frames_and_accumulates() {
        let mut field = FieldModel::new(single_link_config()).expect("field model");

        // 128-wide frame (HT40), NOT the model's 56 — would DimensionMismatch raw.
        let wide = vec![0.5_f64; 128];
        assert!(maybe_feed_calibration(&mut field, &wide));

        assert_eq!(
            field.status(),
            CalibrationStatus::Collecting,
            "128-wide frame must resample to 56 and be accepted"
        );
        assert_eq!(
            field.calibration_frame_count(),
            1,
            "wide frame must accumulate, not be silently dropped"
        );
    }

    #[test]
    fn maybe_feed_calibration_rejects_an_empty_observation() {
        let mut field = FieldModel::new(single_link_config()).expect("field model");
        assert!(!maybe_feed_calibration(&mut field, &[]));
        assert_eq!(field.calibration_frame_count(), 0);
    }

    #[test]
    fn calibrated_occupancy_is_fresh_model_only_and_normalizes_runtime_frames() {
        let field = fresh_test_model();
        let mut history = VecDeque::new();
        history.push_back(vec![1.02; 128]);
        let result =
            calibrated_occupancy(&field, &history, 1_500_000).expect("fresh calibrated inference");
        assert!(result.person_count <= MAX_SINGLE_LINK_OCCUPANCY);
        assert!(matches!(
            result.method,
            CalibratedOccupancyMethod::Eigenvalue | CalibratedOccupancyMethod::PerturbationEnergy
        ));
        assert_eq!(calibrated_occupancy(&field, &history, 4_000_000), None);
        assert_eq!(
            calibrated_occupancy(&field, &VecDeque::new(), 1_500_000),
            None
        );
    }

    #[test]
    fn occupancy_falls_back_after_expiry_without_a_process_restart() {
        let field = fresh_test_model();
        let baseline = field.modes().unwrap().baseline[0].clone();
        let history: VecDeque<Vec<f64>> =
            (0..OCCUPANCY_WINDOW).map(|_| baseline.clone()).collect();

        assert_eq!(field.status(), CalibrationStatus::Fresh);
        assert_eq!(
            calibration_status_at(&field, 1_500_000),
            CalibrationStatus::Fresh
        );
        assert_eq!(
            occupancy_or_fallback(&field, &history, 1_500_000, 0.0, 0),
            0,
            "a fresh field model may resolve its learned baseline as empty"
        );

        assert_eq!(field.status(), CalibrationStatus::Fresh);
        assert_eq!(
            calibration_status_at(&field, 4_000_000),
            CalibrationStatus::Expired
        );
        assert_eq!(
            occupancy_or_fallback(&field, &history, 4_000_000, 0.0, 0),
            score_to_person_count(0.0, 0),
            "an expired in-memory model must fall back even though its cached status is fresh"
        );
    }

    #[test]
    fn adaptive_runtime_reference_scores_empty_and_rejects_shift() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 500,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 60.0,
        };
        let mut field = FieldModel::new(config).expect("field model");
        let mut calibration = Vec::new();
        for frame_index in 0..600 {
            let time = frame_index as f64 * 0.071;
            let frame: Vec<f64> = (0..56)
                .map(|subcarrier| {
                    let carrier = subcarrier as f64;
                    18.0 + carrier * 0.08
                        + (time + carrier * 0.13).sin() * 0.8
                        + (time * 0.37 + carrier * 0.031).cos() * 0.35
                })
                .collect();
            field.feed_calibration(&[frame.clone()]).unwrap();
            calibration.push(frame);
        }
        field.finalize_calibration(1_000_000, 7).unwrap();

        let empty_history: VecDeque<Vec<f64>> = calibration[550..600].iter().cloned().collect();
        let empty = calibrated_occupancy(&field, &empty_history, 1_500_000)
            .expect("adaptive perturbation result");
        assert_eq!(empty.person_count, 0);
        let background = bootstrap_background_match(&field, &empty_history, 1_500_000)
            .expect("background score");
        assert!(background.matches_empty);
        assert_eq!(background.reference_window_count, 12);
        assert!(background.reliable);
        assert_eq!(background.maturity, 0.6);
        assert!(background.normalized_residual_z.is_some());
        assert!(background
            .score
            .is_some_and(|score| (0.5..=1.0).contains(&score)));

        let short_history: VecDeque<Vec<f64>> = empty_history.iter().take(49).cloned().collect();
        assert!(!bootstrap_empty_prior_applies(
            &field,
            &short_history,
            1_500_000
        ));

        let shifted_history: VecDeque<Vec<f64>> = calibration[550..600]
            .iter()
            .map(|frame| {
                frame
                    .iter()
                    .enumerate()
                    .map(|(index, value)| value + if index % 3 == 0 { 8.0 } else { -5.0 })
                    .collect()
            })
            .collect();
        let shifted = calibrated_occupancy(&field, &shifted_history, 1_500_000)
            .expect("shifted perturbation result");
        assert!(shifted.person_count >= 1);
        let shifted_background = bootstrap_background_match(&field, &shifted_history, 1_500_000)
            .expect("shifted background result");
        assert!(!shifted_background.matches_empty);
        assert_eq!(shifted_background.score, Some(0.0));
        assert!(shifted_background.reliable);
    }
}
