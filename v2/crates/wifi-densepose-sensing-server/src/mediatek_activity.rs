//! "How much is happening right now, relative to how much usually happens
//! here" — a fast/slow relative-activity index per MediaTek device and for
//! the room as a whole. The use case is an at-a-glance activity dial: it
//! should drop when nobody is moving and rise when a room full of people all
//! stand up at once.
//!
//! This is deliberately NOT an absolute presence/motion classifier — see
//! `mediatek_heuristic` for that (presence/absent/unknown against fixed
//! thresholds). This module answers a different question: is the channel
//! fluctuating more, right now, than it *typically* does at this location?
//!
//! # Relative to its own baseline, on purpose
//!
//! The index compares a fast signal (`fast`, the last [`FAST_WINDOW`]) to a
//! slow self-baseline (`slow`, an EMA with [`SLOW_TIME_CONSTANT`]). A room
//! that is ALWAYS busy trends back toward the middle of the scale — its own
//! "busy" becomes the new normal — and that is intended, not a bug. A step
//! change (everyone standing up at once) shows as a spike against the
//! baseline; sustained elevated activity decays back toward index 0 as the
//! baseline catches up. Label: [`CLASSIFIER_ID`] = `activity-index-v0`.
//!
//! # Floors, and why `fast` and `slow` share one
//!
//! `fast` is floored to [`SLOW_FLOOR`] before it ever reaches the EMA or the
//! ratio — the SAME floor `slow` itself is clamped to. A perfectly flat
//! channel (fast == 0 forever) then converges `slow` to exactly that same
//! floor too, giving `index = log2(floor / floor) = 0` — "flat means index
//! 0", not a large negative number from an arbitrary epsilon mismatch
//! between two differently-floored quantities.

use crate::SharedState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::warn;

pub(crate) const CLASSIFIER_ID: &str = "activity-index-v0";
/// "Fast" signal window: how much the channel is fluctuating right now.
pub(crate) const FAST_WINDOW: Duration = Duration::from_secs(2);
/// "Slow" baseline time constant. Applied as a genuine time-constant EMA
/// (`alpha = 1 - exp(-dt/tau)`, not a fixed per-sample alpha), so irregular
/// (bursty) arrival timing doesn't change how quickly the baseline actually
/// adapts in wall-clock time.
pub(crate) const SLOW_TIME_CONSTANT: Duration = Duration::from_secs(300);
/// Shared floor for both `fast` and `slow` — see the module doc's "Floors"
/// section for why this must be the SAME constant for both.
pub(crate) const SLOW_FLOOR: f64 = 1e-4;
/// Peak-hold window for both per-device and room `peak_30s`.
pub(crate) const PEAK_HOLD_WINDOW: Duration = Duration::from_secs(30);
pub(crate) const INDEX_MIN: f64 = -2.0;
pub(crate) const INDEX_MAX: f64 = 3.0;
/// How much history `/api/v1/csi/mediatek/activity` can serve.
pub(crate) const HISTORY_RETENTION: Duration = Duration::from_secs(300);
/// Retained-history decimation — a strip chart doesn't need every ingest
/// tick, just ~1 Hz.
pub(crate) const HISTORY_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
/// A device counts toward the room aggregate — and appears in
/// `activity.devices` at all — only while its last update is within this
/// of `now`. Deliberately the SAME constant `/api/v1/csi/mediatek/devices`
/// uses (`mediatek_devices::MEDIATEK_DEVICE_STALE`), not an independent
/// value: a downstream consumer observed `/devices` and `activity.devices`
/// naming two different receiver sets after one unit restarted, because this
/// used to be its own 5s cutoff. The two listings must never disagree about
/// which receivers currently exist.
pub(crate) const ROOM_STALE_AFTER: Duration = crate::mediatek_devices::MEDIATEK_DEVICE_STALE;

/// Hard minimum for `floor` (the quiet-room baseline) — same value as
/// `SLOW_FLOOR`, for the same reason: a channel that's never shown any
/// measurable fluctuation shouldn't produce an undefined or exploding
/// `abs_level` either.
pub(crate) const FLOOR_HARD_MIN: f64 = SLOW_FLOOR;
/// How quickly `floor` is pulled DOWN toward a new, lower `fast` reading.
/// This approximates a running low-percentile of `fast` over the last tens
/// of minutes without the cost of a real quantile sketch: one low sample
/// nudges the floor a little, a SUSTAINED quiet stretch pulls it most of
/// the way down within a few multiples of this constant. Deliberately
/// asymmetric with `SLOW_TIME_CONSTANT` — the floor is meant to track "how
/// quiet does this room get", which should move slower to reach than the
/// change-detector's baseline, but doesn't need the full 30-minute window
/// a literal percentile would use to be a reasonable proxy for it.
pub(crate) const FLOOR_DOWN_TIME_CONSTANT: Duration = Duration::from_secs(600);
/// Upper bound on how fast `floor` may drift UP on its own (compounding,
/// applied every observation regardless of the downward pull above), so a
/// one-off quiet dip doesn't permanently anchor the floor after the room's
/// true quiet baseline has genuinely risen (furniture moved, sensor
/// relocated, persistent new noise source, etc.) — it re-learns rather
/// than getting stuck.
pub(crate) const FLOOR_UPWARD_CREEP_PER_MINUTE: f64 = 0.01;
/// How long a brand-new device (no persisted floor — see
/// `ActivityTracker::seed_floor`) collects `fast` readings before
/// committing an initial `floor`, instead of seeding it from a single
/// reading. 2026-09-20: without this, the very first observation (often
/// near-zero — a ring with only 1-2 frames has no meaningful variance yet)
/// pinned `floor` at `FLOOR_HARD_MIN`, and the ≤1%/min creep meant it would
/// stay saturated at `abs_level` 1.0 for hours regardless of what the room
/// actually did.
pub(crate) const FLOOR_INIT_WINDOW: Duration = Duration::from_secs(30);
/// Percentile (nearest-rank) of the init-window samples used as the
/// committed initial floor — low enough to represent "how quiet this
/// window got", not its typical level, but not the single minimum sample
/// (more robust to one anomalously-low reading).
pub(crate) const FLOOR_INIT_PERCENTILE: f64 = 0.20;
/// How often `--activity-state` is rewritten to disk.
pub(crate) const ACTIVITY_STATE_PERSIST_INTERVAL: Duration = Duration::from_secs(60);

fn index_to_level(index: f64) -> f64 {
    ((index - INDEX_MIN) / (INDEX_MAX - INDEX_MIN)).clamp(0.0, 1.0)
}

/// Nearest-rank percentile of `samples` (sorted in place). `p` is a
/// fraction in `[0, 1]`; `p=0.0` is the min, `p=1.0` is the max. Returns
/// `0.0` for an empty slice — callers `.max(FLOOR_HARD_MIN)` the result, so
/// this never surfaces as a usable floor on its own.
fn percentile(samples: &mut [f64], p: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(f64::total_cmp);
    let idx = ((samples.len() - 1) as f64 * p.clamp(0.0, 1.0)).round() as usize;
    samples[idx.min(samples.len() - 1)]
}

/// One device's activity verdict at the moment it was last observed.
/// Carries TWO independent measures, because a purely-relative index sags
/// back to mid-scale while a crowd simply stays, so a "maxed out" reading
/// would decay within minutes even with the room still full:
///
/// - `index`/`level`: the CHANGE detector (fast vs. the 300s `slow`
///   baseline) — spikes on a step change like a stand-up, decays back
///   toward 0 as the new activity level becomes the norm. Good for "did
///   something just happen", bad for "is the room currently full".
/// - `floor`/`abs_level`: an ABSOLUTE measure against a slow-moving
///   QUIET-ROOM baseline (`floor`) instead of a fast-moving one — a full
///   room stays high for as long as it stays full; an empty room reads
///   near 0; `floor` itself only re-learns "how quiet is quiet" over tens
///   of minutes, so occupancy doesn't have to decay just because it's
///   sustained. See `is_occupancy_estimate` at the API layer: this is
///   still a channel-fluctuation proxy, not a validated occupancy count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeviceVerdict {
    pub(crate) index: f64,
    pub(crate) level: f64,
    pub(crate) fast: f64,
    pub(crate) slow: f64,
    pub(crate) peak_30s: f64,
    pub(crate) frames_2s: usize,
    pub(crate) floor: f64,
    pub(crate) abs_level: f64,
    pub(crate) abs_peak_30s: f64,
}

/// Per-device EMA/peak-hold state, persisted across ingest ticks. Only
/// mutated when a NEW frame arrives for this specific device — other
/// devices' trackers are read-only from the room aggregate's point of view.
#[derive(Debug, Clone, Default)]
pub(crate) struct ActivityTracker {
    slow: Option<f64>,
    /// `None` until either `seed_floor` was called (persisted-state restore)
    /// or the `FLOOR_INIT_WINDOW` bootstrap below committed a value.
    floor: Option<f64>,
    /// `fast` readings collected since `floor_init_start`, while `floor` is
    /// still `None` and no seed was provided. Cleared once the window
    /// commits a floor.
    floor_init_samples: Vec<f64>,
    floor_init_start: Option<Instant>,
    last_update: Option<Instant>,
    level_history: VecDeque<(Instant, f64)>,
    abs_level_history: VecDeque<(Instant, f64)>,
    last_verdict: Option<DeviceVerdict>,
}

impl ActivityTracker {
    /// Restore a previously-learned floor (from `--activity-state`) for a
    /// device seen in an earlier run, so a restart doesn't re-saturate
    /// `abs_level` while the bootstrap window re-collects — an empty-room
    /// floor learned in one run should survive to the next. Skips the
    /// `FLOOR_INIT_WINDOW` bootstrap entirely: a persisted floor is already
    /// better-informed than a fresh 30s sample. Only meaningful before the
    /// first `observe()` call; a no-op with a warning-free overwrite if
    /// called after (harmless, just wasted — callers only do this once, at
    /// tracker creation).
    pub(crate) fn seed_floor(&mut self, floor: f64) {
        self.floor = Some(floor.max(FLOOR_HARD_MIN));
        self.floor_init_samples.clear();
        self.floor_init_start = None;
    }

    /// The tracker's current floor, for periodic persistence
    /// (`--activity-state`). `None` while still in the init-window
    /// bootstrap (nothing durable to save yet).
    pub(crate) fn floor(&self) -> Option<f64> {
        self.floor
    }

    /// Fold in a new `fast` reading observed at `now` (already computed
    /// from the device's CSI ring via `DeviceRing::fast_activity`), update
    /// both the relative (slow EMA) and absolute (floor) baselines plus
    /// their 30s peak-holds, and return (and cache) the verdict.
    pub(crate) fn observe(&mut self, now: Instant, fast: f64, frames_2s: usize) -> DeviceVerdict {
        let fast = fast.max(SLOW_FLOOR);
        let dt = self
            .last_update
            .map(|t| now.saturating_duration_since(t).as_secs_f64())
            .unwrap_or(0.0);

        // Relative: fast vs. a 300s EMA baseline. A change detector.
        let tau = SLOW_TIME_CONSTANT.as_secs_f64();
        self.slow = Some(match self.slow {
            None => fast,
            Some(s) => {
                let alpha = if tau > 0.0 {
                    1.0 - (-dt / tau).exp()
                } else {
                    1.0
                };
                (s + alpha * (fast - s)).max(SLOW_FLOOR)
            }
        });
        let slow = self.slow.unwrap();
        let index = (fast / slow).log2().clamp(INDEX_MIN, INDEX_MAX);
        let level = index_to_level(index);

        // Absolute: fast vs. a slow-to-rise, quick-to-fall quiet-room
        // floor. An occupancy proxy, not a change detector. Bootstrapped
        // from a FLOOR_INIT_WINDOW of samples (a percentile, not the first
        // single reading) unless `seed_floor` already restored one.
        let floor = match self.floor {
            Some(f) => {
                let floor_down_tau = FLOOR_DOWN_TIME_CONSTANT.as_secs_f64();
                let pulled_down = if fast < f {
                    let alpha = if floor_down_tau > 0.0 {
                        1.0 - (-dt / floor_down_tau).exp()
                    } else {
                        1.0
                    };
                    f + alpha * (fast - f)
                } else {
                    f
                };
                let dt_minutes = dt / 60.0;
                let creep = (1.0 + FLOOR_UPWARD_CREEP_PER_MINUTE).powf(dt_minutes);
                let updated = (pulled_down * creep).max(FLOOR_HARD_MIN);
                self.floor = Some(updated);
                updated
            }
            None => {
                let start = *self.floor_init_start.get_or_insert(now);
                self.floor_init_samples.push(fast);
                if now.saturating_duration_since(start) >= FLOOR_INIT_WINDOW {
                    let committed = percentile(&mut self.floor_init_samples, FLOOR_INIT_PERCENTILE)
                        .max(FLOOR_HARD_MIN);
                    self.floor = Some(committed);
                    self.floor_init_samples.clear();
                    committed
                } else {
                    // Provisional: the lowest reading seen so far in the
                    // still-open window, so abs_level is defined from the
                    // first sample rather than reading as a fixed 0 or 1
                    // for up to 30s. Not stored in `self.floor` — the
                    // window isn't committed yet, so a later, genuinely
                    // lower sample can still pull this down before commit.
                    self.floor_init_samples
                        .iter()
                        .copied()
                        .fold(f64::INFINITY, f64::min)
                        .max(FLOOR_HARD_MIN)
                }
            }
        };
        let abs_level = ((fast / floor).log2() / 5.0).clamp(0.0, 1.0);

        self.last_update = Some(now);

        self.level_history.push_back((now, level));
        while self
            .level_history
            .front()
            .is_some_and(|(t, _)| now.saturating_duration_since(*t) > PEAK_HOLD_WINDOW)
        {
            self.level_history.pop_front();
        }
        let peak_30s = self
            .level_history
            .iter()
            .map(|(_, l)| *l)
            .fold(0.0, f64::max);

        self.abs_level_history.push_back((now, abs_level));
        while self
            .abs_level_history
            .front()
            .is_some_and(|(t, _)| now.saturating_duration_since(*t) > PEAK_HOLD_WINDOW)
        {
            self.abs_level_history.pop_front();
        }
        let abs_peak_30s = self
            .abs_level_history
            .iter()
            .map(|(_, l)| *l)
            .fold(0.0, f64::max);

        let verdict = DeviceVerdict {
            index,
            level,
            fast,
            slow,
            peak_30s,
            frames_2s,
            floor,
            abs_level,
            abs_peak_30s,
        };
        self.last_verdict = Some(verdict);
        verdict
    }

    pub(crate) fn last_verdict(&self) -> Option<DeviceVerdict> {
        self.last_verdict
    }

    pub(crate) fn last_update(&self) -> Option<Instant> {
        self.last_update
    }
}

/// Room activity index: the `frames_2s`-weighted mean of every device
/// that's updated within `stale_after` of `now`. `INDEX_MIN` (no activity)
/// when no device is fresh. A running `peak_30s` of this value is tracked
/// separately by the caller (`room_peak_30s`) — "weighted mean... plus a
/// max" is the peak-hold on the aggregate, not a per-tick blend with the
/// single highest device (a weighted mean can never exceed its inputs'
/// max, so blending those two per-tick would be a no-op).
pub(crate) fn room_index(
    trackers: &HashMap<String, ActivityTracker>,
    now: Instant,
    stale_after: Duration,
) -> f64 {
    let fresh: Vec<DeviceVerdict> = trackers
        .values()
        .filter(|t| {
            t.last_update()
                .is_some_and(|u| now.saturating_duration_since(u) <= stale_after)
        })
        .filter_map(|t| t.last_verdict())
        .collect();
    if fresh.is_empty() {
        return INDEX_MIN;
    }
    let total_weight: f64 = fresh.iter().map(|v| v.frames_2s as f64).sum();
    let mean = if total_weight > 0.0 {
        fresh
            .iter()
            .map(|v| v.index * v.frames_2s as f64)
            .sum::<f64>()
            / total_weight
    } else {
        // No device has any frames within its own last 2s to weight by
        // (e.g. everyone just barely reconnected) — fall back to a plain
        // mean across the fresh set rather than reporting no activity.
        fresh.iter().map(|v| v.index).sum::<f64>() / fresh.len() as f64
    };
    mean.clamp(INDEX_MIN, INDEX_MAX)
}

pub(crate) fn room_level(index: f64) -> f64 {
    index_to_level(index)
}

/// Room `abs_level`: the MAX over every fresh device's `abs_level` — unlike
/// the relative `room_index`'s weighted mean, this is deliberately a max:
/// a crowd anywhere in any receiver's path counts.
/// `0.0` when no device is fresh.
pub(crate) fn room_abs_level(
    trackers: &HashMap<String, ActivityTracker>,
    now: Instant,
    stale_after: Duration,
) -> f64 {
    trackers
        .values()
        .filter(|t| {
            t.last_update()
                .is_some_and(|u| now.saturating_duration_since(u) <= stale_after)
        })
        .filter_map(|t| t.last_verdict())
        .map(|v| v.abs_level)
        .fold(0.0, f64::max)
}

/// Advances a 30s peak-hold and returns the current peak. Used for both the
/// room's relative `peak_30s` (`room_activity_peak_history`) and its
/// absolute `abs_peak_30s` (`room_abs_activity_peak_history`) on
/// `AppStateInner` — same pattern as `ActivityTracker`'s per-device ones.
pub(crate) fn room_peak_30s(
    peak_history: &mut VecDeque<(Instant, f64)>,
    now: Instant,
    value: f64,
) -> f64 {
    peak_history.push_back((now, value));
    while peak_history
        .front()
        .is_some_and(|(t, _)| now.saturating_duration_since(*t) > PEAK_HOLD_WINDOW)
    {
        peak_history.pop_front();
    }
    peak_history.iter().map(|(_, l)| *l).fold(0.0, f64::max)
}

/// One decimated (~1 Hz) history sample for the `/activity` strip-chart
/// route. Carries both measures per the 2026-09-20 amendment: `level`
/// (relative, spikes-and-decays) and `abs_level` (absolute, stays high
/// while a crowd stays) — see `DeviceVerdict`'s doc comment for what each
/// answers.
#[derive(Debug, Clone)]
pub(crate) struct ActivitySample {
    pub(crate) at: Instant,
    pub(crate) room_level: f64,
    pub(crate) room_abs_level: f64,
    pub(crate) device_levels: BTreeMap<String, f64>,
    pub(crate) device_abs_levels: BTreeMap<String, f64>,
}

/// Appends a new history sample if at least `HISTORY_SAMPLE_INTERVAL` has
/// passed since the last one (decimation), then prunes anything older than
/// `HISTORY_RETENTION`. A no-op call this often is cheap; the ingest path
/// calls it on every MediaTek frame and lets this function decide whether
/// it's actually time to sample.
#[allow(clippy::too_many_arguments)]
pub(crate) fn maybe_sample_history(
    history: &mut VecDeque<ActivitySample>,
    last_sampled: &mut Option<Instant>,
    now: Instant,
    room_level: f64,
    room_abs_level: f64,
    trackers: &HashMap<String, ActivityTracker>,
) {
    if last_sampled.is_some_and(|t| now.saturating_duration_since(t) < HISTORY_SAMPLE_INTERVAL) {
        return;
    }
    *last_sampled = Some(now);
    // Same staleness cutoff as activity.devices / /api/v1/csi/mediatek/devices
    // (see ROOM_STALE_AFTER's doc comment) — a device the live listings have
    // dropped shouldn't linger in freshly-recorded history samples either.
    let fresh = |t: &&ActivityTracker| {
        t.last_update()
            .is_some_and(|u| now.saturating_duration_since(u) <= ROOM_STALE_AFTER)
    };
    let device_levels: BTreeMap<String, f64> = trackers
        .iter()
        .filter(|(_, t)| fresh(t))
        .filter_map(|(id, t)| t.last_verdict().map(|v| (id.clone(), v.level)))
        .collect();
    let device_abs_levels: BTreeMap<String, f64> = trackers
        .iter()
        .filter(|(_, t)| fresh(t))
        .filter_map(|(id, t)| t.last_verdict().map(|v| (id.clone(), v.abs_level)))
        .collect();
    history.push_back(ActivitySample {
        at: now,
        room_level,
        room_abs_level,
        device_levels,
        device_abs_levels,
    });
    while history
        .front()
        .is_some_and(|s| now.saturating_duration_since(s.at) > HISTORY_RETENTION)
    {
        history.pop_front();
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ActivityHistoryQuery {
    seconds: Option<u64>,
}

/// `GET /api/v1/csi/mediatek/activity?seconds=300` — 1 Hz(-ish) samples of
/// room + per-device activity level, oldest first, for a strip chart. Each
/// sample's `age_ms` is relative to the moment of THIS request (there is no
/// meaningful absolute timestamp to report — retained samples are keyed by
/// a monotonic `Instant`, matching this crate's existing `age_ms`
/// convention elsewhere, e.g. `mediatek_devices`). `seconds` is capped at
/// `HISTORY_RETENTION` (300s) — asking for more just returns everything
/// retained. Never 404s: an empty `samples` array is a valid answer before
/// the first MediaTek frame has ever arrived.
pub(crate) async fn mediatek_activity_history(
    State(state): State<SharedState>,
    Query(query): Query<ActivityHistoryQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    let now = Instant::now();
    let window = query
        .seconds
        .map(Duration::from_secs)
        .unwrap_or(HISTORY_RETENTION)
        .min(HISTORY_RETENTION);
    let samples: Vec<serde_json::Value> = s
        .activity_history
        .iter()
        .filter(|sample| now.saturating_duration_since(sample.at) <= window)
        .map(|sample| {
            let devices: serde_json::Map<String, serde_json::Value> = sample
                .device_levels
                .iter()
                .map(|(id, level)| {
                    let abs_level = sample.device_abs_levels.get(id).copied().unwrap_or(0.0);
                    (
                        id.clone(),
                        serde_json::json!({ "level": level, "abs_level": abs_level }),
                    )
                })
                .collect();
            serde_json::json!({
                "age_ms": now.saturating_duration_since(sample.at).as_millis() as u64,
                "room_level": sample.room_level,
                "room_abs_level": sample.room_abs_level,
                "devices": devices,
            })
        })
        .collect();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "seconds": window.as_secs(),
            "sample_count": samples.len(),
            "samples": samples,
        })),
    )
}

/// Loads a previously-persisted `--activity-state` file (JSON
/// `{device_id: floor}`) at startup, for seeding new `ActivityTracker`s via
/// `ActivityTracker::seed_floor` so a restart doesn't reset the quiet-room
/// baseline. Never fails the caller: a missing file returns an empty map
/// (first run), and a missing/corrupt file logs a warning and also returns
/// empty (start fresh rather than refuse to boot over a stale-state file).
pub(crate) fn load_activity_state(path: &Path) -> HashMap<String, f64> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            warn!(
                "activity state at {} is not valid JSON ({e}) — starting with no seeded floors",
                path.display()
            );
            HashMap::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => {
            warn!(
                "could not read activity state at {} ({e}) — starting with no seeded floors",
                path.display()
            );
            HashMap::new()
        }
    }
}

fn write_activity_state(path: &Path, floors: &HashMap<String, f64>) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(floors)?;
    // Write-then-rename so a crash mid-write can never leave a truncated,
    // unparseable state file for the next `load_activity_state` to choke on.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Background task: every `ACTIVITY_STATE_PERSIST_INTERVAL`, snapshots
/// every device's COMMITTED floor (`ActivityTracker::floor()` — a device
/// still in its `FLOOR_INIT_WINDOW` bootstrap has nothing durable yet) and
/// writes it to `path`. The write itself runs on a blocking thread so a
/// slow or contended disk never stalls the async runtime; only spawned at
/// all when `--activity-state` was given.
pub(crate) async fn activity_state_persist_task(state: SharedState, path: std::path::PathBuf) {
    let mut interval = tokio::time::interval(ACTIVITY_STATE_PERSIST_INTERVAL);
    loop {
        interval.tick().await;
        let snapshot: HashMap<String, f64> = {
            let s = state.read().await;
            s.mediatek_activity_by_device
                .iter()
                .filter_map(|(id, t)| t.floor().map(|f| (id.clone(), f)))
                .collect()
        };
        let write_path = path.clone();
        match tokio::task::spawn_blocking(move || write_activity_state(&write_path, &snapshot))
            .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!(
                "failed to persist activity state to {}: {e}",
                path.display()
            ),
            Err(e) => warn!("activity state persist task panicked: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-09-20 live regression: floor used to seed from a single first
    /// reading (`None => fast.max(FLOOR_HARD_MIN)`), which was often
    /// near-zero on a fresh ring — pinning `abs_level` at 1.0 for hours
    /// regardless of what the room actually did, since the floor could
    /// only creep up at most 1%/min from there. The floor must instead
    /// bootstrap from a percentile of the first FLOOR_INIT_WINDOW of
    /// readings, so a device reading only its own steady, unremarkable
    /// baseline does NOT read as maximally active.
    #[test]
    fn floor_bootstraps_from_the_init_window_not_a_single_first_reading() {
        let mut tracker = ActivityTracker::default();
        let start = Instant::now();
        let mut t = start;
        let quiet_fast = 0.01; // representative "nothing happening" reading

        let mut verdict = tracker.observe(t, quiet_fast, 10);
        while t.duration_since(start) < FLOOR_INIT_WINDOW {
            t += Duration::from_secs(2);
            verdict = tracker.observe(t, quiet_fast, 10);
        }

        assert!(
            verdict.floor > quiet_fast * 0.5,
            "floor committed far below the actual quiet reading: floor={} quiet_fast={quiet_fast}",
            verdict.floor
        );
        assert!(
            verdict.abs_level < 0.3,
            "a device reading only its own steady baseline must not saturate abs_level, got {}",
            verdict.abs_level
        );
    }

    /// Before the init window has fully elapsed, `abs_level` must still be
    /// well-defined (from the running minimum seen so far), not a
    /// placeholder 0 or 1 — a live dashboard shouldn't show a meaningless
    /// number for the first 30s of a device's life.
    #[test]
    fn provisional_floor_during_the_bootstrap_window_tracks_the_running_minimum() {
        let mut tracker = ActivityTracker::default();
        let start = Instant::now();
        // A dip partway through the window: the provisional floor must
        // follow it down (not stay pinned at the first sample), since a
        // later, genuinely lower reading should still be eligible to win
        // the window's percentile once it commits.
        let first = tracker.observe(start, 0.02, 10);
        assert!(
            first.floor > 0.0,
            "provisional floor must be defined from the very first sample"
        );
        let dip = tracker.observe(start + Duration::from_secs(5), 0.005, 10);
        assert!(
            dip.floor <= 0.005 + 1e-12,
            "provisional floor must track a new lower reading: {}",
            dip.floor
        );
    }

    /// `--activity-state` restore: a device with a persisted floor must
    /// skip the FLOOR_INIT_WINDOW bootstrap entirely — its very first
    /// `observe()` call already reflects the seeded value.
    #[test]
    fn seed_floor_skips_the_bootstrap_window() {
        let mut tracker = ActivityTracker::default();
        tracker.seed_floor(0.02);
        let verdict = tracker.observe(Instant::now(), 0.02, 10);
        assert!(
            (verdict.floor - 0.02).abs() < 1e-9,
            "seeded floor must be used immediately: {}",
            verdict.floor
        );
        assert!(
            verdict.abs_level < 0.1,
            "fast == seeded floor must read a near-zero abs_level right away"
        );
    }

    #[test]
    fn flat_amplitude_signal_converges_to_index_zero() {
        let mut tracker = ActivityTracker::default();
        let start = Instant::now();
        let mut t = start;
        let mut verdict = tracker.observe(t, 0.0, 10);
        // Feed a perfectly flat `fast` reading for a while.
        for _ in 0..20 {
            t += Duration::from_millis(500);
            verdict = tracker.observe(t, 0.0, 10);
        }
        assert!(
            (verdict.index - 0.0).abs() < 1e-9,
            "index was {}",
            verdict.index
        );
        // The [-2, +3] range isn't centered on 0: level = (index - (-2)) / 5,
        // so index 0 sits at 2/5 = 0.4 up the scale, not the midpoint 0.5.
        assert!(
            (verdict.level - 0.4).abs() < 1e-9,
            "index 0 must map to level 0.4, got {}",
            verdict.level
        );
    }

    #[test]
    fn a_10x_burst_spikes_to_the_index_ceiling_then_decays() {
        let mut tracker = ActivityTracker::default();
        let start = Instant::now();
        let mut t = start;
        let baseline_fast = 0.05;
        // Establish a settled baseline (slow ~= baseline_fast) over several
        // slow-time-constant spans, well past what a naive fixed alpha
        // would need — the time-aware EMA converges in wall-clock time,
        // not sample count.
        let mut verdict = tracker.observe(t, baseline_fast, 20);
        for _ in 0..5 {
            t += SLOW_TIME_CONSTANT;
            verdict = tracker.observe(t, baseline_fast, 20);
        }
        assert!(
            (verdict.slow - baseline_fast).abs() < baseline_fast * 0.01,
            "baseline should have settled: slow={}",
            verdict.slow
        );
        assert!(
            verdict.index.abs() < 0.05,
            "settled baseline should read ~index 0, got {}",
            verdict.index
        );

        // A sudden 10x std burst: fast jumps 10x, slow hasn't moved yet.
        t += Duration::from_millis(100);
        let burst = tracker.observe(t, baseline_fast * 10.0, 20);
        assert!(
            (burst.index - INDEX_MAX).abs() < 0.1,
            "log2(10) ~= 3.32 should clamp to +3, got {}",
            burst.index
        );

        // Hold the burst level and let the baseline start catching up —
        // the index must decay back down, not stay pinned at the ceiling.
        let mut sustained = burst;
        for _ in 0..5 {
            t += Duration::from_secs(60);
            sustained = tracker.observe(t, baseline_fast * 10.0, 20);
        }
        assert!(
            sustained.index < burst.index,
            "index must decay as slow catches up to a sustained burst: {} -> {}",
            burst.index,
            sustained.index
        );
    }

    /// While the relative
    /// index decays back toward 0 as `slow` catches up to a SUSTAINED
    /// burst, `abs_level` must stay high — it's judged against `floor`
    /// (the quiet-room baseline), which barely moves while `fast` stays
    /// well above it (floor only pulls DOWN toward a lower reading; a
    /// sustained high reading only lets it creep up by at most 1%/min).
    #[test]
    fn a_sustained_burst_keeps_abs_level_high_while_relative_index_decays() {
        let mut tracker = ActivityTracker::default();
        let start = Instant::now();
        let mut t = start;
        let baseline_fast = 0.05;

        let mut verdict = tracker.observe(t, baseline_fast, 20);
        for _ in 0..5 {
            t += SLOW_TIME_CONSTANT;
            verdict = tracker.observe(t, baseline_fast, 20);
        }
        assert!(
            verdict.index.abs() < 0.05,
            "settled baseline should read ~index 0, got {}",
            verdict.index
        );
        assert!(
            verdict.abs_level < 0.05,
            "a settled quiet baseline should read ~abs_level 0, got {}",
            verdict.abs_level
        );

        // A sustained 10x std burst — held, not transient.
        t += Duration::from_millis(100);
        let burst = tracker.observe(t, baseline_fast * 10.0, 20);
        assert!(
            burst.abs_level > 0.3,
            "a real burst must read a meaningfully high abs_level, got {}",
            burst.abs_level
        );

        let mut sustained = burst;
        for _ in 0..5 {
            t += Duration::from_secs(60);
            sustained = tracker.observe(t, baseline_fast * 10.0, 20);
        }
        assert!(
            sustained.index < burst.index,
            "sanity check (already covered elsewhere): relative index must have decayed, {} -> {}",
            burst.index,
            sustained.index
        );
        assert!(
            sustained.abs_level > burst.abs_level * 0.8,
            "abs_level must stay high while the burst is sustained, not decay like the relative index: {} -> {}",
            burst.abs_level,
            sustained.abs_level
        );
    }

    /// 2026-09-20 amendment: once activity genuinely drops back to (or
    /// below) the quiet-room floor and stays there, `abs_level` must
    /// return to ~0 — the floor's downward pull is what lets it "re-learn"
    /// a quiet room after a crowd leaves, on the order of a few
    /// `FLOOR_DOWN_TIME_CONSTANT`s.
    #[test]
    fn abs_level_returns_to_zero_once_activity_drops_back_to_the_floor() {
        let mut tracker = ActivityTracker::default();
        let mut t = Instant::now();
        let baseline_fast = 0.05;

        tracker.observe(t, baseline_fast, 20);
        for _ in 0..5 {
            t += SLOW_TIME_CONSTANT;
            tracker.observe(t, baseline_fast, 20);
        }

        // Elevated for a while (people in the room).
        t += Duration::from_millis(100);
        for _ in 0..5 {
            t += Duration::from_secs(60);
            tracker.observe(t, baseline_fast * 10.0, 20);
        }

        // Everyone leaves: fast drops straight back to the original quiet
        // level, sustained long enough for the floor to re-learn it.
        let mut after_leave = tracker.observe(t + Duration::from_secs(1), baseline_fast, 20);
        for _ in 0..10 {
            t += FLOOR_DOWN_TIME_CONSTANT;
            after_leave = tracker.observe(t, baseline_fast, 20);
        }
        assert!(
            after_leave.abs_level < 0.05,
            "abs_level must return to ~0 once the floor re-learns the quiet level, got {} (floor={})",
            after_leave.abs_level,
            after_leave.floor
        );
    }

    #[test]
    fn two_devices_room_index_is_the_weighted_mean() {
        let mut trackers = HashMap::new();
        let baseline_t = Instant::now();
        let now = baseline_t + Duration::from_secs(1);

        // A tracker's FIRST observation always seeds slow = fast (no prior
        // baseline to compare against), so index is always exactly 0 on
        // that call regardless of the fast value. Establish a real
        // baseline first, then feed a different reading to get a genuinely
        // non-zero, non-equal index per device.
        let mut a = ActivityTracker::default();
        a.observe(baseline_t, 0.05, 30);
        a.observe(now, 0.5, 30); // well above its own baseline -> positive index

        let mut b = ActivityTracker::default();
        b.observe(baseline_t, 0.05, 10);
        b.observe(now, 0.06, 10); // barely above its own baseline -> small positive index

        trackers.insert("dev-a".to_string(), a);
        trackers.insert("dev-b".to_string(), b);

        let idx_a = trackers["dev-a"].last_verdict().unwrap().index;
        let idx_b = trackers["dev-b"].last_verdict().unwrap().index;
        assert!(
            idx_a > idx_b,
            "test needs two genuinely different indices: a={idx_a} b={idx_b}"
        );
        let expected = (idx_a * 30.0 + idx_b * 10.0) / 40.0;

        let room = room_index(&trackers, now, ROOM_STALE_AFTER);
        assert!(
            (room - expected).abs() < 1e-9,
            "room index {} != weighted mean {}",
            room,
            expected
        );
        // Sanity: the weighted mean must lie strictly between the two
        // inputs (never at either extreme) for genuinely different values.
        assert!(
            room > idx_b && room < idx_a,
            "room={room} must lie strictly between idx_b={idx_b} and idx_a={idx_a}"
        );
    }

    #[test]
    fn a_device_stale_past_room_stale_after_does_not_vote() {
        let mut trackers = HashMap::new();
        let old = Instant::now();
        let now = old + ROOM_STALE_AFTER + Duration::from_secs(1);
        let mut a = ActivityTracker::default();
        a.observe(old, 5.0, 30);
        trackers.insert("dev-a".to_string(), a);

        assert_eq!(
            room_index(&trackers, now, ROOM_STALE_AFTER),
            INDEX_MIN,
            "a stale-only room has no evidence of activity"
        );
    }

    /// Room abs_level is a MAX over devices, not a weighted mean — a crowd
    /// anywhere in any receiver's path counts,
    /// so one busy device must win even against a much heavier-weighted
    /// quiet one.
    #[test]
    fn room_abs_level_is_the_max_over_devices_not_a_weighted_mean() {
        let mut trackers = HashMap::new();
        let baseline_t = Instant::now();
        let now = baseline_t + Duration::from_secs(1);

        // A tracker's FIRST observation always seeds floor = fast (no
        // quiet-room baseline to compare against yet), so abs_level is 0 on
        // that call regardless of the fast value — establish a real floor
        // first, same as the relative-index tests above.
        let mut quiet = ActivityTracker::default();
        quiet.observe(baseline_t, SLOW_FLOOR, 100);
        quiet.observe(now, SLOW_FLOOR, 100); // heavy weight, stays at its own quiet floor

        let mut busy = ActivityTracker::default();
        busy.observe(baseline_t, SLOW_FLOOR, 1);
        busy.observe(now, 0.5, 1); // light weight, but a real spike above its own floor

        trackers.insert("dev-quiet".to_string(), quiet);
        trackers.insert("dev-busy".to_string(), busy);

        let abs_busy = trackers["dev-busy"].last_verdict().unwrap().abs_level;
        let abs_quiet = trackers["dev-quiet"].last_verdict().unwrap().abs_level;
        assert!(
            abs_busy > abs_quiet,
            "test setup needs a genuinely busier device"
        );

        let room_abs = room_abs_level(&trackers, now, ROOM_STALE_AFTER);
        assert_eq!(
            room_abs, abs_busy,
            "room abs_level must be the max, not diluted by the heavier-weighted quiet device"
        );
    }

    #[test]
    fn room_peak_30s_holds_the_highest_recent_room_level() {
        let mut history = VecDeque::new();
        let start = Instant::now();
        assert_eq!(room_peak_30s(&mut history, start, 0.2), 0.2);
        assert_eq!(
            room_peak_30s(&mut history, start + Duration::from_secs(5), 0.9),
            0.9
        );
        // A later, lower reading must not erase the still-recent peak
        // (the t=5 sample, 0.9, is only 5s old here).
        assert_eq!(
            room_peak_30s(&mut history, start + Duration::from_secs(10), 0.3),
            0.9
        );
        // At t=40: t=0 (age 40>30, pruned), t=5 (age 35>30, pruned), t=10
        // (age exactly 30, kept — the peak sample has aged out, not this
        // one), t=40 (new). Remaining max is the t=10 sample, 0.3.
        assert_eq!(
            room_peak_30s(&mut history, start + Duration::from_secs(40), 0.1),
            0.3
        );
    }

    #[test]
    fn history_is_decimated_and_pruned() {
        let mut history = VecDeque::new();
        let mut last_sampled = None;
        let trackers = HashMap::new();
        let start = Instant::now();

        maybe_sample_history(&mut history, &mut last_sampled, start, 0.1, 0.1, &trackers);
        maybe_sample_history(
            &mut history,
            &mut last_sampled,
            start + Duration::from_millis(200),
            0.2,
            0.2,
            &trackers,
        );
        assert_eq!(
            history.len(),
            1,
            "a sample within HISTORY_SAMPLE_INTERVAL must be dropped, not appended"
        );

        maybe_sample_history(
            &mut history,
            &mut last_sampled,
            start + HISTORY_SAMPLE_INTERVAL,
            0.3,
            0.3,
            &trackers,
        );
        assert_eq!(history.len(), 2);

        maybe_sample_history(
            &mut history,
            &mut last_sampled,
            start + HISTORY_RETENTION + Duration::from_secs(10),
            0.4,
            0.4,
            &trackers,
        );
        assert_eq!(
            history.len(),
            1,
            "samples older than HISTORY_RETENTION must be pruned"
        );
    }
}
