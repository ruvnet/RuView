//! Bridge between sensing-server per-node state and the signal crate's
//! `MultistaticFuser` for attention-weighted CSI fusion across ESP32 nodes.
//!
//! This module converts the server's `NodeState` (f64 amplitude history) into
//! `MultiBandCsiFrame`s that the multistatic fusion pipeline expects, then
//! drives `MultistaticFuser::fuse` with a graceful fallback when fusion fails
//! (e.g. insufficient nodes or timestamp spread).

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use wifi_densepose_signal::hardware_norm::{CanonicalCsiFrame, HardwareNormalizer, HardwareType};
use wifi_densepose_signal::ruvsense::multiband::MultiBandCsiFrame;
use wifi_densepose_signal::ruvsense::multistatic::{
    FusedSensingFrame, MultistaticConfig, MultistaticFuser,
};

use super::NodeState;

/// Maximum age for a node frame to be considered active (10 seconds).
const STALE_THRESHOLD: Duration = Duration::from_secs(10);

/// Default WiFi channel frequency (MHz) used for single-channel frames.
const DEFAULT_FREQ_MHZ: u32 = 2437; // Channel 6

/// Monotonic reference point for timestamp generation. All node timestamps
/// are relative to this instant, avoiding wall-clock/monotonic mixing issues.
/// Backdate the lazy initialization beyond the active-node window so frames
/// recorded just before the first bridge call retain their arrival-time skew.
static EPOCH: LazyLock<Instant> = LazyLock::new(|| {
    Instant::now()
        .checked_sub(STALE_THRESHOLD + STALE_THRESHOLD)
        .unwrap_or_else(Instant::now)
});

/// Shared length-only canonicalizer (issue #1170). The default 56-tone grid
/// matches what `MultistaticFuser` (ADR-154) expects. Stateless and immutable,
/// so a single process-wide instance is safe to share across nodes.
static NORMALIZER: LazyLock<HardwareNormalizer> = LazyLock::new(HardwareNormalizer::new);

/// Convert a single `NodeState` into a `MultiBandCsiFrame` suitable for
/// multistatic fusion.
///
/// Returns `None` when the node has no frame history or no recorded
/// `last_frame_time`.
pub fn node_frame_from_state(node_id: u8, ns: &NodeState) -> Option<MultiBandCsiFrame> {
    let last_time = ns.last_frame_time.as_ref()?;
    let timestamp_us = ns
        .mesh_aligned_us_for_latest_csi_frame()
        .unwrap_or_else(|| host_arrival_timestamp_us(last_time));
    node_frame_from_state_at(node_id, ns, timestamp_us)
}

fn host_arrival_timestamp_us(last_time: &Instant) -> u64 {
    last_time
        .checked_duration_since(*EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

fn node_frame_from_state_at(
    node_id: u8,
    ns: &NodeState,
    timestamp_us: u64,
) -> Option<MultiBandCsiFrame> {
    let latest = ns.frame_history.back()?;
    if latest.is_empty() {
        return None;
    }

    // Issue #1170: resample the raw amplitude onto the canonical 56-tone grid
    // BEFORE fusion. ESP32 nodes in mixed HT20/HT40 capture modes report
    // different subcarrier counts (64 / 128 / 192); feeding those raw into
    // `MultistaticFuser::fuse` tripped `DimensionMismatch` on every cycle and
    // silently disabled real multistatic fusion. Length-only canonicalization
    // (no z-score) keeps the amplitude scale the person-score relies on.
    let canonical_amp = NORMALIZER.resample_to_canonical(latest);
    let amplitude: Vec<f32> = canonical_amp.iter().map(|&v| v as f32).collect();
    let n_sub = amplitude.len();
    let phase = vec![0.0_f32; n_sub];

    let canonical = CanonicalCsiFrame {
        amplitude,
        phase,
        hardware_type: HardwareType::Esp32S3,
    };

    Some(MultiBandCsiFrame {
        node_id,
        timestamp_us,
        channel_frames: vec![canonical],
        frequencies_mhz: vec![DEFAULT_FREQ_MHZ],
        coherence: 1.0, // single-channel, perfect self-coherence
    })
}

/// Collect the default-guard coherent `MultiBandCsiFrame` cohort.
///
/// A node is considered active if its `last_frame_time` is within
/// [`STALE_THRESHOLD`] of `now`.
pub fn node_frames_from_states(node_states: &HashMap<u8, NodeState>) -> Vec<MultiBandCsiFrame> {
    node_frames_from_states_with_guard(
        node_states,
        MultistaticConfig::default().guard_interval_us,
    )
}

/// Collect the freshest temporally coherent cohort of active node frames.
///
/// Nodes can publish at very different rates (for example, a mixed S3/C6
/// fleet). `STALE_THRESHOLD` determines whether a node is alive; it does not
/// mean its latest frame belongs to the current sensing cycle. After choosing
/// one timestamp domain for the whole cycle, retain only frames within the
/// fuser's hard guard of the freshest frame. This prevents a slow-but-live node
/// from turning every governed cycle into `TimestampMismatch`, while always
/// preserving at least the freshest node for the supported single-node path.
pub fn node_frames_from_states_with_guard(
    node_states: &HashMap<u8, NodeState>,
    guard_interval_us: u64,
) -> Vec<MultiBandCsiFrame> {
    let now = Instant::now();
    let mut active: Vec<(u8, &NodeState)> = node_states
        .iter()
        .filter_map(|(&node_id, ns)| {
            let last_time = ns.last_frame_time.as_ref()?;
            (now.duration_since(*last_time) <= STALE_THRESHOLD).then_some((node_id, ns))
        })
        .collect();
    active.sort_unstable_by_key(|(node_id, _)| *node_id);

    if active.is_empty() {
        return Vec::new();
    }

    let guard_interval_us = guard_interval_us.max(1);

    // Timestamp domains must be selected for the cycle as a whole. A CSI
    // frame can legitimately arrive between periodic sync-marked frames. If
    // that one node fell back to process-local host time while a peer retained
    // mesh epoch time, the resulting hundreds-of-seconds spread made every
    // governed fusion cycle fail. Use mesh time only when every active node
    // can provide it; otherwise use host-arrival time consistently for all.
    let mesh_times: Option<Vec<u64>> = active
        .iter()
        .map(|(_, ns)| ns.mesh_aligned_us_for_latest_csi_frame())
        .collect::<Option<Vec<_>>>()
        .filter(|times| {
            let Some(min) = times.iter().min() else {
                return false;
            };
            let Some(max) = times.iter().max() else {
                return false;
            };
            max.saturating_sub(*min) <= guard_interval_us
        });

    let mut timed = Vec::with_capacity(active.len());
    for (index, (node_id, ns)) in active.into_iter().enumerate() {
        let timestamp_us = mesh_times.as_ref().map_or_else(
            || {
                host_arrival_timestamp_us(
                    ns.last_frame_time.as_ref().expect("active node has time"),
                )
            },
            |times| times[index],
        );
        timed.push((node_id, ns, timestamp_us));
    }

    let freshest_timestamp_us = timed
        .iter()
        .map(|(_, _, timestamp_us)| *timestamp_us)
        .max()
        .expect("non-empty active cohort");

    let mut frames = Vec::with_capacity(timed.len());
    for (node_id, ns, timestamp_us) in timed {
        if freshest_timestamp_us.saturating_sub(timestamp_us) > guard_interval_us {
            continue;
        }
        if let Some(frame) = node_frame_from_state_at(node_id, ns, timestamp_us) {
            frames.push(frame);
        }
    }

    frames
}

/// Attempt multistatic fusion; fall back to max per-node person count on failure.
///
/// Returns `(fused_frame, fallback_person_count)`. When fusion succeeds,
/// `fallback_person_count` is `None` — the caller must compute count from
/// the fused amplitudes. On failure, returns the maximum per-node count
/// (not the sum, to avoid double-counting overlapping coverage).
pub fn fuse_or_fallback(
    fuser: &MultistaticFuser,
    node_states: &HashMap<u8, NodeState>,
    dedup_factor: f64,
) -> (Option<FusedSensingFrame>, Option<usize>) {
    let frames =
        node_frames_from_states_with_guard(node_states, fuser.guard_interval_us());
    if frames.is_empty() {
        return (None, Some(0));
    }

    match fuser.fuse(&frames) {
        Ok(fused) => {
            // Caller must compute person count from fused amplitudes.
            (Some(fused), None)
        }
        Err(e) => {
            tracing::debug!("Multistatic fusion failed ({e}), using per-node sum/dedup fallback");
            // Sum per-node counts then divide by dedup_factor (assumed average
            // visibility per body across nodes).  ADR-044 §5.1.
            // dedup_factor is runtime-configurable; default 3.0.
            let total: usize = node_states
                .values()
                .filter(|ns| {
                    ns.last_frame_time
                        .map(|t| t.elapsed() <= STALE_THRESHOLD)
                        .unwrap_or(false)
                })
                .map(|ns| ns.prev_person_count)
                .sum();
            let estimated = ((total as f64) / dedup_factor).ceil() as usize;
            (None, Some(estimated))
        }
    }
}

/// Compute a person-presence score from fused amplitude data.
///
/// Uses the squared coefficient of variation (variance / mean^2) as a
/// lightweight proxy for body-induced CSI perturbation. A flat amplitude
/// vector (no person) yields a score near zero; a vector with high variance
/// relative to its mean (person moving) yields a score approaching 1.0.
pub fn compute_person_score_from_amplitudes(amplitudes: &[f32]) -> f64 {
    if amplitudes.is_empty() {
        return 0.0;
    }

    let n = amplitudes.len() as f64;
    let sum: f64 = amplitudes.iter().map(|&a| a as f64).sum();
    let mean = sum / n;

    let variance: f64 = amplitudes
        .iter()
        .map(|&a| {
            let diff = (a as f64) - mean;
            diff * diff
        })
        .sum::<f64>()
        / n;

    let score = variance / (mean * mean + 1e-10);
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use wifi_densepose_hardware::{SyncPacket, SyncPacketFlags};

    /// Helper: build a minimal NodeState for testing. Uses `NodeState::new()`
    /// then mutates the `pub(crate)` fields the bridge needs.
    fn make_node_state(
        frame_history: VecDeque<Vec<f64>>,
        last_frame_time: Option<Instant>,
        prev_person_count: usize,
    ) -> NodeState {
        let mut ns = NodeState::new();
        ns.frame_history = frame_history;
        ns.last_frame_time = last_frame_time;
        ns.prev_person_count = prev_person_count;
        ns
    }

    #[test]
    fn test_node_frame_from_empty_state() {
        let ns = make_node_state(VecDeque::new(), Some(Instant::now()), 0);
        assert!(node_frame_from_state(1, &ns).is_none());
    }

    #[test]
    fn test_node_frame_from_state_no_time() {
        let mut history = VecDeque::new();
        history.push_back(vec![1.0, 2.0, 3.0]);
        let ns = make_node_state(history, None, 0);
        assert!(node_frame_from_state(1, &ns).is_none());
    }

    #[test]
    fn test_node_frame_conversion() {
        let mut history = VecDeque::new();
        history.push_back(vec![10.0, 20.0, 30.5]);
        let ns = make_node_state(history, Some(Instant::now()), 0);

        let frame = node_frame_from_state(42, &ns).expect("should produce a frame");
        assert_eq!(frame.node_id, 42);
        assert_eq!(frame.channel_frames.len(), 1);

        let ch = &frame.channel_frames[0];
        // Issue #1170: amplitude is now resampled onto the canonical 56-tone
        // grid regardless of the raw count.
        assert_eq!(ch.amplitude.len(), 56);
        // resample_cubic preserves the endpoints (no z-scoring), so the scale
        // the person-score relies on is intact.
        assert!((ch.amplitude[0] - 10.0_f32).abs() < 1e-3);
        assert!((ch.amplitude[55] - 30.5_f32).abs() < 1e-3);
        // Phase should be all zeros
        assert!(ch.phase.iter().all(|&p| p == 0.0));
        assert_eq!(ch.hardware_type, HardwareType::Esp32S3);
    }

    fn mark_mesh_timed_frame(
        ns: &mut NodeState,
        node_id: u8,
        sync_sequence: u32,
        frame_sequence: u32,
        mesh_epoch_us: u64,
        host_arrival: Instant,
    ) {
        ns.apply_sync_packet(
            SyncPacket {
                node_id,
                proto_ver: 1,
                flags: SyncPacketFlags {
                    is_leader: node_id == 1,
                    is_valid: true,
                    smoothed_used: node_id != 1,
                },
                local_us: 10_000_000,
                epoch_us: mesh_epoch_us,
                sequence: sync_sequence,
                // proto v1 fixture: the v2 bytes are reserved-zero.
                node_mac: None,
                health: Default::default(),
            },
            Instant::now(),
        );
        ns.observe_accepted_csi_frame(frame_sequence, true, host_arrival);
    }

    #[test]
    fn mesh_timestamp_replaces_skewed_host_arrival_time() {
        let mut history = VecDeque::new();
        history.push_back(vec![10.0, 20.0, 30.0]);
        let host_arrival = Instant::now();
        let mut ns = make_node_state(history, None, 0);
        mark_mesh_timed_frame(&mut ns, 1, 100, 101, 1_000_000, host_arrival);

        let frame = node_frame_from_state(1, &ns).expect("mesh-timed frame");
        assert_eq!(frame.timestamp_us, 1_050_000);
    }

    #[test]
    fn mesh_time_allows_fusion_despite_udp_arrival_skew() {
        let base = Instant::now() - Duration::from_millis(500);
        let mut states = HashMap::new();

        let mut first_history = VecDeque::new();
        first_history.push_back(vec![1.0; 64]);
        let mut first = make_node_state(first_history, None, 0);
        mark_mesh_timed_frame(&mut first, 1, 100, 101, 1_000_000, base);
        states.insert(1, first);

        let mut second_history = VecDeque::new();
        second_history.push_back(vec![1.1; 64]);
        let mut second = make_node_state(second_history, None, 0);
        mark_mesh_timed_frame(
            &mut second,
            2,
            200,
            201,
            1_005_000,
            base + Duration::from_millis(200),
        );
        states.insert(2, second);

        let frames = node_frames_from_states(&states);
        let spread = frames.iter().map(|f| f.timestamp_us).max().unwrap()
            - frames.iter().map(|f| f.timestamp_us).min().unwrap();
        assert_eq!(spread, 5_000, "mesh capture spread, not 200 ms UDP skew");
        assert!(
            MultistaticFuser::new().fuse(&frames).is_ok(),
            "mesh-aligned frames inside the 60 ms guard must fuse"
        );
    }

    #[test]
    fn partial_sync_uses_one_host_timestamp_domain_for_the_cycle() {
        let base = Instant::now() - Duration::from_millis(500);
        let mut states = HashMap::new();

        let mut synced_history = VecDeque::new();
        synced_history.push_back(vec![1.0; 64]);
        let mut synced = make_node_state(synced_history, None, 0);
        mark_mesh_timed_frame(&mut synced, 1, 100, 101, 500_000_000, base);
        states.insert(1, synced);

        let mut unsynced_history = VecDeque::new();
        unsynced_history.push_back(vec![1.1; 64]);
        states.insert(
            2,
            make_node_state(unsynced_history, Some(base + Duration::from_millis(5)), 0),
        );

        let frames = node_frames_from_states(&states);
        let spread = frames.iter().map(|f| f.timestamp_us).max().unwrap()
            - frames.iter().map(|f| f.timestamp_us).min().unwrap();
        assert_eq!(spread, 5_000, "partial sync must fall back as one cycle");
        assert!(
            MultistaticFuser::new().fuse(&frames).is_ok(),
            "mixed sync validity must not mix mesh and host timestamp domains"
        );
    }

    #[test]
    fn incoherent_mesh_timestamps_fall_back_to_host_arrival_for_the_cycle() {
        let base = Instant::now() - Duration::from_millis(500);
        let mut states = HashMap::new();

        for (node_id, mesh_epoch_us, arrival) in [
            (1, 1_000_000, base),
            (2, 500_000_000, base + Duration::from_millis(5)),
        ] {
            let mut history = VecDeque::new();
            history.push_back(vec![1.0; 64]);
            let mut state = make_node_state(history, None, 0);
            mark_mesh_timed_frame(&mut state, node_id, 100, 101, mesh_epoch_us, arrival);
            states.insert(node_id, state);
        }

        let frames = node_frames_from_states(&states);
        let spread = frames.iter().map(|f| f.timestamp_us).max().unwrap()
            - frames.iter().map(|f| f.timestamp_us).min().unwrap();
        assert_eq!(spread, 5_000, "incoherent mesh time must not reach fusion");
        assert!(MultistaticFuser::new().fuse(&frames).is_ok());
    }

    #[test]
    fn unsynchronized_frames_prune_to_freshest_host_cohort() {
        let base = Instant::now() - Duration::from_millis(500);
        let mut states = HashMap::new();

        for (node_id, arrival) in [(1, base), (2, base + Duration::from_millis(200))] {
            let mut history = VecDeque::new();
            history.push_back(vec![1.0; 64]);
            states.insert(node_id, make_node_state(history, Some(arrival), 0));
        }

        let frames = node_frames_from_states(&states);
        assert_eq!(frames.len(), 1, "only the freshest host frame is coherent");
        assert_eq!(frames[0].node_id, 2);
        assert!(
            MultistaticFuser::new().fuse(&frames).is_ok(),
            "an asynchronous slow node must not fail the live cycle"
        );
    }

    #[test]
    fn slow_live_node_is_excluded_from_fresh_cohort() {
        let now = Instant::now();
        let mut states = HashMap::new();
        for (node_id, age_ms, n_sub) in [
            (1, 0, 64),
            (3, 10, 256),
            (4, 50, 64),
            (7, 1_000, 256),
        ] {
            let mut history = VecDeque::new();
            history.push_back(vec![1.0 + node_id as f64 * 0.01; n_sub]);
            states.insert(
                node_id,
                make_node_state(
                    history,
                    Some(now - Duration::from_millis(age_ms)),
                    1,
                ),
            );
        }

        let frames = node_frames_from_states_with_guard(&states, 60_000);
        let ids: Vec<u8> = frames.iter().map(|frame| frame.node_id).collect();
        assert_eq!(ids, vec![1, 3, 4]);
        assert!(MultistaticFuser::new().fuse(&frames).is_ok());
    }

    #[test]
    fn configured_guard_is_shared_with_cohort_selection() {
        let base = Instant::now() - Duration::from_millis(500);
        let mut states = HashMap::new();
        for (node_id, arrival) in [
            (1, base),
            (2, base + Duration::from_millis(150)),
        ] {
            let mut history = VecDeque::new();
            history.push_back(vec![1.0; 64]);
            states.insert(node_id, make_node_state(history, Some(arrival), 0));
        }

        let cfg = MultistaticConfig {
            guard_interval_us: 200_000,
            ..MultistaticConfig::default()
        };
        let fuser = MultistaticFuser::with_config(cfg);
        let (fused, fallback) = fuse_or_fallback(&fuser, &states, 3.0);
        assert_eq!(fused.as_ref().map(|frame| frame.active_nodes), Some(2));
        assert!(fallback.is_none());
    }

    #[test]
    fn heterogeneous_node_counts_canonicalize_and_fuse() {
        // Issue #1170 regression: a mixed mesh with HT20 (64-bin) and HT40
        // (192-bin) nodes must canonicalize to a uniform 56 tones and fuse,
        // instead of tripping DimensionMismatch on every cycle.
        let mut states: HashMap<u8, NodeState> = HashMap::new();

        let mut h64 = VecDeque::new();
        h64.push_back((0..64).map(|i| 1.0 + 0.1 * i as f64).collect::<Vec<f64>>());
        states.insert(1, make_node_state(h64, Some(Instant::now()), 1));

        let mut h192 = VecDeque::new();
        h192.push_back((0..192).map(|i| 2.0 + 0.05 * i as f64).collect::<Vec<f64>>());
        states.insert(3, make_node_state(h192, Some(Instant::now()), 1));

        let frames = node_frames_from_states(&states);
        assert_eq!(frames.len(), 2, "both nodes should produce frames");
        for f in &frames {
            assert_eq!(
                f.channel_frames[0].amplitude.len(),
                56,
                "every node must present the canonical 56-tone dimension"
            );
        }

        // The fuser must now accept the cycle (no DimensionMismatch).
        let fuser = MultistaticFuser::new();
        let result = fuser.fuse(&frames);
        assert!(
            result.is_ok(),
            "heterogeneous mesh should fuse after canonicalization, got {result:?}"
        );

        // And the higher-level fallback path returns the fused frame, not the
        // sum/dedup fallback.
        let (fused, fallback) = fuse_or_fallback(&fuser, &states, 3.0);
        assert!(fused.is_some(), "fusion should succeed");
        assert!(fallback.is_none(), "no fallback when fusion succeeds");
    }

    #[test]
    fn test_stale_node_excluded() {
        let mut states: HashMap<u8, NodeState> = HashMap::new();

        // Active node: frame just received
        let mut active_history = VecDeque::new();
        active_history.push_back(vec![1.0, 2.0]);
        states.insert(1, make_node_state(active_history, Some(Instant::now()), 1));

        // Stale node: frame 20 seconds ago
        let mut stale_history = VecDeque::new();
        stale_history.push_back(vec![3.0, 4.0]);
        let stale_time = Instant::now() - Duration::from_secs(20);
        states.insert(2, make_node_state(stale_history, Some(stale_time), 1));

        let frames = node_frames_from_states(&states);
        assert_eq!(frames.len(), 1, "stale node should be excluded");
        assert_eq!(frames[0].node_id, 1);
    }

    #[test]
    fn test_compute_person_score_empty() {
        assert!((compute_person_score_from_amplitudes(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_person_score_flat() {
        // Constant amplitude => variance = 0 => score ~ 0
        let flat = vec![5.0_f32; 64];
        let score = compute_person_score_from_amplitudes(&flat);
        assert!(
            score < 0.001,
            "flat signal should have near-zero score, got {score}"
        );
    }

    #[test]
    fn test_compute_person_score_varied() {
        // High variance relative to mean should produce a positive score
        let varied: Vec<f32> = (0..64)
            .map(|i| if i % 2 == 0 { 1.0 } else { 10.0 })
            .collect();
        let score = compute_person_score_from_amplitudes(&varied);
        assert!(
            score > 0.1,
            "varied signal should have positive score, got {score}"
        );
        assert!(score <= 1.0, "score should be clamped to 1.0, got {score}");
    }

    #[test]
    fn test_compute_person_score_clamped() {
        // Near-zero mean with non-zero variance => would blow up without clamp
        let vals = vec![0.0_f32, 0.0, 0.0, 0.001];
        let score = compute_person_score_from_amplitudes(&vals);
        assert!(score <= 1.0, "score must be clamped to 1.0");
    }

    #[test]
    fn test_fuse_or_fallback_empty() {
        let fuser = MultistaticFuser::new();
        let states: HashMap<u8, NodeState> = HashMap::new();
        let (fused, count) = fuse_or_fallback(&fuser, &states, 3.0);
        assert!(fused.is_none());
        assert_eq!(count, Some(0));
    }
}
