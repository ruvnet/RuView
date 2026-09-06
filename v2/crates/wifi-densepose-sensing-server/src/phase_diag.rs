//! Phase-channel diagnostics (2026-08-28).
//!
//! Purpose: decide, from real hardware, whether ESP32 single-antenna CSI phase
//! can carry Doppler at all — the open question blocking the bistatic position
//! tier.
//!
//! Background. `sanitize_phase_linear_detrend` fits `a*k + b` across
//! subcarrier index and subtracts both. Removing the slope `a` is the intended
//! STO/packet-timing fix; removing the intercept `b` also deletes the
//! *common-mode* phase, which is where a moving target's Doppler lives (across
//! HT20 at 2.4 GHz the subcarrier frequencies span only +/-0.36%, so Doppler
//! phase is common-mode to within a third of a percent). This is proven
//! deterministically in `phase_sanitization_annihilates_doppler_tests`.
//!
//! The three effects separate by which axis they vary on:
//!
//! | effect  | across subcarrier `k` | across time `t`     |
//! |---------|-----------------------|---------------------|
//! | STO     | linear ramp           | ~constant           |
//! | CFO     | constant              | slow drift (sub-Hz) |
//! | Doppler | constant              | 1-20 Hz             |
//!
//! So the correct decomposition removes the slope along `k` and the slow trend
//! along `t`. Whether that is *achievable* depends on one measurable fact: does
//! the per-frame common phase advance by less than pi between consecutive
//! frames (unwrappable, so CFO is separable from Doppler), or does it wrap
//! randomly (aliased, so single-antenna phase Doppler is impossible on this
//! silicon)? This module measures exactly that, and nothing else.
//!
//! Everything here is opt-in behind `--phase-diagnostics <dir>` and does no
//! work at all when disabled. It never alters the signal path.
//!
//! Output is CSI-derived and therefore MUST NOT be committed (`CLAUDE.md`).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// Wrap a phase difference into `(-pi, pi]`.
///
/// Frame-to-frame common-phase deltas are only meaningful modulo `2*pi`; the
/// whole question is whether the *wrapped* delta clusters (recoverable drift)
/// or spreads uniformly (aliased noise).
pub fn wrap_to_pi(x: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    let mut v = (x + std::f64::consts::PI).rem_euclid(two_pi) - std::f64::consts::PI;
    // rem_euclid maps exactly -pi to -pi; normalize the boundary to +pi so the
    // interval is half-open the way the doc says.
    if v <= -std::f64::consts::PI {
        v += two_pi;
    }
    v
}

/// Circular mean of a set of wrapped phases, in `(-pi, pi]`.
///
/// A plain arithmetic mean of `atan2` output is wrong across the +/-pi branch
/// cut (mean of `[3.14, -3.14]` is 0, not pi). This is the common-mode phase —
/// the intercept term the sanitizer currently destroys.
pub fn circular_mean_phase(phases: &[f64]) -> Option<f64> {
    if phases.is_empty() {
        return None;
    }
    let (s, c) = phases.iter().fold((0.0_f64, 0.0_f64), |(s, c), &p| {
        (s + p.sin(), c + p.cos())
    });
    if s.abs() < 1e-12 && c.abs() < 1e-12 {
        return None;
    }
    Some(s.atan2(c))
}

/// Circular *resultant length* in `[0, 1]` — how concentrated a set of phases
/// is. 1.0 = all identical, 0.0 = uniformly spread.
///
/// Applied across subcarriers within one frame this measures how common-mode
/// that frame's phase really is (the premise of the whole fix). Applied across
/// a window of frame-to-frame deltas it measures whether CFO is a coherent
/// drift or aliased noise — the decisive number.
pub fn circular_resultant_length(phases: &[f64]) -> f64 {
    if phases.is_empty() {
        return 0.0;
    }
    let (s, c) = phases.iter().fold((0.0_f64, 0.0_f64), |(s, c), &p| {
        (s + p.sin(), c + p.cos())
    });
    let n = phases.len() as f64;
    ((s / n).powi(2) + (c / n).powi(2)).sqrt().clamp(0.0, 1.0)
}

/// Ordinary-least-squares slope and intercept of `y` against its own index.
///
/// Same fit `sanitize_phase_linear_detrend` performs, exposed separately so a
/// diagnostic can record the two terms *without* subtracting either. The slope
/// is the STO artifact; the intercept is the common-mode phase.
pub fn ols_slope_intercept(y: &[f64]) -> (f64, f64) {
    let n = y.len() as f64;
    if y.len() < 2 {
        return (0.0, y.first().copied().unwrap_or(0.0));
    }
    let sum_k: f64 = (0..y.len()).map(|k| k as f64).sum();
    let sum_y: f64 = y.iter().sum();
    let sum_kk: f64 = (0..y.len()).map(|k| (k * k) as f64).sum();
    let sum_ky: f64 = y.iter().enumerate().map(|(k, &v)| k as f64 * v).sum();
    let denom = n * sum_kk - sum_k * sum_k;
    if denom.abs() <= 1e-12 {
        return (0.0, sum_y / n);
    }
    let a = (n * sum_ky - sum_k * sum_y) / denom;
    let b = (sum_y - a * sum_k) / n;
    (a, b)
}

/// Unwrap a phase sequence along time, so a slow drift becomes a straight line
/// instead of a sawtooth. Only meaningful if consecutive deltas are `< pi` in
/// magnitude — which is precisely what this module exists to check.
pub fn unwrap_sequence(seq: &[f64]) -> Vec<f64> {
    if seq.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(seq.len());
    out.push(seq[0]);
    for i in 1..seq.len() {
        let d = wrap_to_pi(seq[i] - seq[i - 1]);
        out.push(out[i - 1] + d);
    }
    out
}

/// Per-node running state for the diagnostic.
#[derive(Default)]
struct NodeDiagState {
    /// Frames seen per transmitter MAC (wire v2 only). Bounded: a busy
    /// channel can present many transmitters, and this is a diagnostic, not a
    /// registry, so it stops admitting new ones past a small cap.
    source_counts: std::collections::BTreeMap<[u8; 6], u64>,
    prev_common_phase: Option<f64>,
    frames: u64,
    raw_windows_written: u64,
    last_raw_capture: Option<Instant>,
}

/// One frame's phase-channel measurements, kept separate from the writer so it
/// can be unit-tested without touching the filesystem.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramePhaseStats {
    /// Common-mode phase — the circular mean across subcarriers **after**
    /// removing the STO slope, i.e. the intercept term the sanitizer
    /// destroys. This must be measured on deslanted phase: a steep STO slope
    /// wraps the raw phase several times across the band (observed 2.4 wraps
    /// on node 0), so the circular mean of *raw* phase is the mean of a
    /// near-uniform spread and looks like noise no matter what the underlying
    /// common-mode term is doing.
    pub common_phase: f64,
    /// Circular mean of the raw, un-deslanted phase. Retained only so the
    /// slope-contamination effect above stays visible in the data rather than
    /// being something a future reader has to rediscover.
    pub common_phase_raw: f64,
    /// How common-mode this frame actually is, `[0, 1]`, after removing the
    /// STO slope. High values confirm the premise that Doppler is nearly pure
    /// intercept; low values would mean the phase is dominated by
    /// across-subcarrier structure instead.
    pub common_mode_concentration: f64,
    /// OLS slope across subcarrier index (radians/subcarrier) — the STO term.
    pub sto_slope: f64,
    /// Wrapped change in `common_phase` since this node's previous frame.
    /// `None` on the first frame. **This is the decisive series.**
    pub d_common_phase: Option<f64>,
    /// Mean amplitude, for correlating phase behaviour against the
    /// known-working amplitude channel.
    pub mean_amplitude: f64,
}

/// Compute one frame's phase statistics from raw (unsanitized) CSI.
///
/// `phases` must be raw `atan2(Q, I)` output, not sanitizer output — the whole
/// point is to see the terms the sanitizer removes.
pub fn frame_phase_stats(phases: &[f64], amplitudes: &[f64]) -> Option<FramePhaseStats> {
    if phases.is_empty() {
        return None;
    }
    let common_phase_raw = circular_mean_phase(phases)?;

    // Remove only the STO slope, then measure what remains. Unwrap first so
    // the OLS sees a line, not a sawtooth.
    let unwrapped = unwrap_sequence(phases);
    let (sto_slope, _) = ols_slope_intercept(&unwrapped);
    let deslanted: Vec<f64> = unwrapped
        .iter()
        .enumerate()
        .map(|(k, &v)| wrap_to_pi(v - sto_slope * k as f64))
        .collect();
    let common_mode_concentration = circular_resultant_length(&deslanted);
    // The real common-mode term: circular mean of the deslanted phase.
    let common_phase = circular_mean_phase(&deslanted).unwrap_or(common_phase_raw);

    let mean_amplitude = if amplitudes.is_empty() {
        0.0
    } else {
        amplitudes.iter().sum::<f64>() / amplitudes.len() as f64
    };

    Some(FramePhaseStats {
        common_phase,
        common_phase_raw,
        common_mode_concentration,
        sto_slope,
        d_common_phase: None,
        mean_amplitude,
    })
}

/// How often to open a bounded raw-I/Q capture window.
const RAW_CAPTURE_PERIOD_SECS: u64 = 600;
/// How many frames per node each raw capture window retains. At ~48 fps this
/// is ~60 s, bounding overnight raw output to a few hundred MB while still
/// giving enough contiguous samples to test candidate sanitizers offline.
const RAW_CAPTURE_FRAMES: u64 = 3000;

/// Buffered CSV/raw sinks for the phase diagnostic. Cheap to construct,
/// disabled entirely when `--phase-diagnostics` is absent.
pub struct PhaseDiagnostics {
    stats: Mutex<BufWriter<File>>,
    raw: Mutex<BufWriter<File>>,
    /// Operator-supplied ground-truth markers ("now standing at node 0"),
    /// written on the same `t_s` clock as the other two sinks so a walk test
    /// can be aligned against the signal without guessing timings.
    markers: Mutex<BufWriter<File>>,
    nodes: Mutex<std::collections::HashMap<u8, NodeDiagState>>,
    started: Instant,
    /// When true, every frame is written to the raw sink instead of the
    /// duty-cycled burst pattern. Needed for a deliberate-motion test: the
    /// default 1.4-min-in-10 duty cycle samples only ~14% of the time and will
    /// miss most of a short walk.
    continuous_raw: bool,
}

impl PhaseDiagnostics {
    /// Create the output files under `dir`, which is created if missing.
    ///
    /// `continuous_raw` disables raw duty-cycling — use it for a bounded,
    /// attended experiment, not an overnight run (~275 MB/hour at 3 nodes).
    pub fn new_with_mode(dir: &Path, continuous_raw: bool) -> std::io::Result<Self> {
        let mut s = Self::new(dir)?;
        s.continuous_raw = continuous_raw;
        if continuous_raw {
            tracing::info!("phase diagnostics: CONTINUOUS raw capture (~275 MB/hour)");
        }
        Ok(s)
    }

    /// Record an operator ground-truth marker at the current `t_s`.
    pub fn mark(&self, label: &str) -> f64 {
        let t_s = Instant::now().duration_since(self.started).as_secs_f64();
        // Labels arrive from an HTTP path segment; keep the CSV parseable by
        // stripping separators and bounding the length rather than trusting it.
        let safe: String = label
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(64)
            .collect();
        if let Ok(mut w) = self.markers.lock() {
            let _ = writeln!(w, "{t_s:.4},{safe}");
            let _ = w.flush();
        }
        tracing::info!("phase diagnostics marker: t_s={t_s:.2} label={safe}");
        t_s
    }

    /// Create the two output files under `dir`, which is created if missing.
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let stats_path: PathBuf = dir.join(format!("phase_stats_{stamp}.csv"));
        let raw_path: PathBuf = dir.join(format!("phase_raw_{stamp}.csv"));

        let mut stats = BufWriter::new(File::create(&stats_path)?);
        writeln!(
            stats,
            "t_s,node,seq,n_sc,src,common_phase,d_common_phase,common_phase_raw,\
             common_mode_concentration,sto_slope,mean_amplitude"
        )?;
        let mut raw = BufWriter::new(File::create(&raw_path)?);
        writeln!(raw, "t_s,node,seq,n_sc,phases_rad,amplitudes")?;
        let markers_path: PathBuf = dir.join(format!("phase_markers_{stamp}.csv"));
        let mut markers = BufWriter::new(File::create(&markers_path)?);
        writeln!(markers, "t_s,label")?;

        tracing::info!(
            "phase diagnostics enabled: stats={} raw={}",
            stats_path.display(),
            raw_path.display()
        );

        Ok(Self {
            stats: Mutex::new(stats),
            raw: Mutex::new(raw),
            markers: Mutex::new(markers),
            nodes: Mutex::new(std::collections::HashMap::new()),
            started: Instant::now(),
            continuous_raw: false,
        })
    }

    /// Record one raw CSI frame. Called from the ingestion site *before*
    /// sanitization. Never panics and never propagates an I/O error into the
    /// signal path — a failed diagnostic write must not take down sensing.
    /// Per-node tally of how many frames arrived from each transmitter.
    ///
    /// Only populated from wire v2 frames, which carry the transmitter MAC.
    /// This is the cheap answer to "what is this node actually listening to" —
    /// a question that previously required a serial console and a firmware
    /// read of `info->mac`.
    pub fn source_census(&self) -> Vec<(u8, [u8; 6], u64)> {
        let nodes = self.nodes.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        for (&id, st) in nodes.iter() {
            for (mac, count) in &st.source_counts {
                out.push((id, *mac, *count));
            }
        }
        out.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)));
        out
    }

    pub fn observe_frame(
        &self,
        node_id: u8,
        sequence: u32,
        source_mac: Option<[u8; 6]>,
        phases: &[f64],
        amplitudes: &[f64],
    ) {
        let Some(mut s) = frame_phase_stats(phases, amplitudes) else {
            return;
        };
        let now = Instant::now();
        let t_s = now.duration_since(self.started).as_secs_f64();

        let (capture_raw, frames) = {
            let mut nodes = self.nodes.lock().unwrap_or_else(|e| e.into_inner());
            let st = nodes.entry(node_id).or_default();
            // Must run in every mode: the frame-to-frame common-phase delta is
            // the primary series, and it depends on per-node carry-over state.
            if let Some(mac) = source_mac {
                const MAX_TRACKED_SOURCES: usize = 16;
                if st.source_counts.len() < MAX_TRACKED_SOURCES
                    || st.source_counts.contains_key(&mac)
                {
                    *st.source_counts.entry(mac).or_insert(0) += 1;
                }
            }
            s.d_common_phase = st.prev_common_phase.map(|p| wrap_to_pi(s.common_phase - p));
            st.prev_common_phase = Some(s.common_phase);
            st.frames += 1;

            if self.continuous_raw {
                (true, st.frames)
            } else {

            let window_open = match st.last_raw_capture {
                None => true,
                Some(t) => {
                    now.duration_since(t).as_secs() >= RAW_CAPTURE_PERIOD_SECS
                        && st.raw_windows_written == 0
                }
            };
            if window_open && st.raw_windows_written == 0 {
                st.last_raw_capture = Some(now);
            }
            // Capture a bounded burst of contiguous frames, then idle until the
            // next period. Contiguity matters: offline sanitizer experiments
            // need consecutive frames, not a scattered sample.
            let within_burst = st
                .last_raw_capture
                .is_some_and(|t| now.duration_since(t).as_secs() < 90)
                && st.raw_windows_written < RAW_CAPTURE_FRAMES;
            if within_burst {
                st.raw_windows_written += 1;
            } else if st
                .last_raw_capture
                .is_some_and(|t| now.duration_since(t).as_secs() >= RAW_CAPTURE_PERIOD_SECS)
            {
                st.last_raw_capture = Some(now);
                st.raw_windows_written = 0;
            }
            (within_burst, st.frames)
            }
        };
        let _ = frames;

        if let Ok(mut w) = self.stats.lock() {
            let d = s
                .d_common_phase
                .map(|v| format!("{v:.6}"))
                .unwrap_or_default();
            // Compact transmitter id: last 3 octets are enough to tell the AP
            // from peer nodes and neighbours, and keep the row narrow.
            let src = source_mac
                .map(|m| format!("{:02x}{:02x}{:02x}", m[3], m[4], m[5]))
                .unwrap_or_else(|| "-".to_string());
            let _ = writeln!(
                w,
                "{t_s:.4},{node_id},{sequence},{},{src},{:.6},{d},{:.6},{:.6},{:.9},{:.4}",
                phases.len(),
                s.common_phase,
                s.common_phase_raw,
                s.common_mode_concentration,
                s.sto_slope,
                s.mean_amplitude
            );
        }

        if capture_raw {
            if let Ok(mut w) = self.raw.lock() {
                let p: Vec<String> = phases.iter().map(|v| format!("{v:.5}")).collect();
                let a: Vec<String> = amplitudes.iter().map(|v| format!("{v:.2}")).collect();
                let _ = writeln!(
                    w,
                    "{t_s:.4},{node_id},{sequence},{},{},{}",
                    phases.len(),
                    p.join(" "),
                    a.join(" ")
                );
            }
        }
    }

    /// Flush both sinks. Called periodically so an overnight run's data is on
    /// disk even if the process is killed rather than shut down cleanly.
    pub fn flush(&self) {
        if let Ok(mut w) = self.stats.lock() {
            let _ = w.flush();
        }
        if let Ok(mut w) = self.raw.lock() {
            let _ = w.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// The CSV header and the data rows are written by two separate format
    /// strings, and they have drifted apart twice during development (a column
    /// added to one and not the other), producing a file whose values silently
    /// land under the wrong column names. Anything analysing that file reads
    /// the wrong series and reaches confident, wrong conclusions.
    #[test]
    fn stats_header_and_rows_have_the_same_column_count() {
        let dir = std::env::temp_dir().join(format!(
            "ruview_phase_diag_hdr_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let d = PhaseDiagnostics::new(&dir).expect("create sinks");
        let phases: Vec<f64> = (0..56).map(|k| 0.01 * k as f64).collect();
        let amps = vec![10.0_f64; 56];
        d.observe_frame(1, 7, Some([1, 2, 3, 4, 5, 6]), &phases, &amps);
        d.observe_frame(1, 8, Some([1, 2, 3, 4, 5, 6]), &phases, &amps);
        d.flush();

        let stats = std::fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("phase_stats_"))
            })
            .expect("stats file exists");
        let text = std::fs::read_to_string(&stats).expect("read stats");
        let mut lines = text.lines();
        let header = lines.next().expect("header line");
        let header_cols = header.split(',').count();
        let mut rows = 0;
        for row in lines.filter(|l| !l.trim().is_empty()) {
            rows += 1;
            assert_eq!(
                row.split(',').count(),
                header_cols,
                "row has a different column count than the header\n header: {header}\n row:    {row}"
            );
        }
        assert!(rows > 0, "expected at least one data row");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrap_to_pi_maps_into_half_open_interval() {
        assert!((wrap_to_pi(0.0) - 0.0).abs() < 1e-12);
        assert!((wrap_to_pi(3.0 * PI) - PI).abs() < 1e-9);
        assert!((wrap_to_pi(-3.0 * PI) - PI).abs() < 1e-9);
        assert!((wrap_to_pi(PI + 0.1) - (-PI + 0.1)).abs() < 1e-9);
        for &x in &[0.5, -0.5, 1.0, -3.0, 12.7, -12.7] {
            let w = wrap_to_pi(x);
            assert!(w > -PI - 1e-12 && w <= PI + 1e-12, "{x} wrapped to {w}");
        }
    }

    #[test]
    fn circular_mean_handles_the_branch_cut() {
        // Arithmetic mean of these is 0.0, which is wrong by pi.
        let m = circular_mean_phase(&[PI - 0.01, -PI + 0.01]).expect("defined");
        assert!(m.abs() > 3.0, "expected ~pi, got {m}");
    }

    #[test]
    fn resultant_length_is_one_for_identical_and_near_zero_for_spread() {
        assert!((circular_resultant_length(&[0.7; 32]) - 1.0).abs() < 1e-12);
        let spread: Vec<f64> = (0..64).map(|i| -PI + 2.0 * PI * i as f64 / 64.0).collect();
        assert!(
            circular_resultant_length(&spread) < 0.05,
            "uniform phases must have near-zero resultant length"
        );
    }

    #[test]
    fn ols_recovers_a_known_line() {
        let y: Vec<f64> = (0..40).map(|k| 0.25 * k as f64 - 1.5).collect();
        let (a, b) = ols_slope_intercept(&y);
        assert!((a - 0.25).abs() < 1e-9, "slope {a}");
        assert!((b + 1.5).abs() < 1e-9, "intercept {b}");
    }

    #[test]
    fn unwrap_recovers_a_ramp_that_crosses_the_branch_cut() {
        let step = 0.4_f64;
        let wrapped: Vec<f64> = (0..50).map(|i| wrap_to_pi(step * i as f64)).collect();
        let un = unwrap_sequence(&wrapped);
        for i in 1..un.len() {
            assert!(
                (un[i] - un[i - 1] - step).abs() < 1e-9,
                "unwrapped step {i} was {}",
                un[i] - un[i - 1]
            );
        }
    }

    /// The premise of the proposed fix: a Doppler shift is common-mode across
    /// subcarriers, so after removing the STO slope the residual phase is
    /// highly concentrated. If real hardware reports low concentration, the
    /// common-mode model itself is wrong and the fix would not apply.
    #[test]
    fn physically_modelled_frame_is_highly_common_mode_after_deslanting() {
        const N: usize = 56;
        const F_C: f64 = 2.412e9;
        const SPACING: f64 = 312.5e3;
        let tau_sto = 50e-9;
        let tau_dyn = 0.02 / 2.998e8;
        let phases: Vec<f64> = (0..N)
            .map(|i| {
                let k = i as f64 - (N as f64 - 1.0) / 2.0;
                let f_k = F_C + k * SPACING;
                wrap_to_pi(-2.0 * PI * f_k * (tau_sto + tau_dyn))
            })
            .collect();
        let amps = vec![10.0; N];
        let s = frame_phase_stats(&phases, &amps).expect("stats defined");
        assert!(
            s.common_mode_concentration > 0.95,
            "after removing the STO slope a real frame should be near-pure common mode, \
             concentration was {}",
            s.common_mode_concentration
        );
    }

    /// Sanity check on the decisive metric: a coherent slow drift produces
    /// tightly clustered frame-to-frame deltas, while uniform random phase
    /// produces spread ones. This is exactly the discrimination the overnight
    /// run performs on real data.
    #[test]
    fn delta_concentration_separates_coherent_drift_from_random_phase() {
        let drift: Vec<f64> = (0..500).map(|_| 0.12_f64).collect();
        assert!(circular_resultant_length(&drift) > 0.99);

        // Deterministic pseudo-random spread, no rand dependency.
        let mut x = 12345u64;
        let random: Vec<f64> = (0..500)
            .map(|_| {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let u = (x >> 11) as f64 / (1u64 << 53) as f64;
                -PI + 2.0 * PI * u
            })
            .collect();
        assert!(
            circular_resultant_length(&random) < 0.2,
            "aliased phase must not look like coherent drift"
        );
    }
}
