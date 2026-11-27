//! SYNTHETIC scenario harness: plan item 0.1 (review item R1).
//!
//! Generates 39 CSI scenarios from a small physical model and replays each
//! one, frame by frame, through the same per-node ESP32 path the server runs
//! for live frames (`extract_features_from_frame`, `smooth_and_classify_node`,
//! the vital-sign detector and `smooth_vitals_node`). It then scores presence,
//! still-person hold and breathing error against ground truth.
//!
//! **Every accuracy number here is SYNTHETIC.** It measures the code against
//! a simple model of a room, not a real one. Per-frame timings are MEASURED
//! on whatever machine runs the test and are reported, never asserted.
//!
//! The model: a static six-path room; optionally a person (chest motion from
//! breathing and heartbeat, or walking); and the impairments ESP32 CSI is
//! known to carry: a random common phase offset and a random phase slope per
//! packet, per-packet gain jitter, additive noise and int8 I/Q quantization.
//! It has no multipath from the person, no furniture motion, no pets and no
//! interference.
//!
//! The run is a ratchet, not a target. It fails when a metric is worse than
//! the committed baseline (`tests/fixtures/scenario_baseline.json`) by more
//! than a small tolerance. After an intentional improvement, refresh the
//! baseline and commit it with the change:
//!
//! ```text
//! RUVIEW_SCENARIO_UPDATE_BASELINE=1 cargo test --release \
//!   -p wifi-densepose-sensing-server --no-default-features \
//!   scenario_harness -- --ignored --nocapture
//! ```
//!
//! It is `#[ignore]`d because a debug build takes minutes. CI runs it in
//! release mode from `.github/workflows/scenario-harness.yml`. Set
//! `RUVIEW_SCENARIO_REPORT=<path>` to also write the Markdown report there.

use super::*;
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::time::Instant;

/// Bump when the generator or the scenario list changes, so a stale baseline
/// is refused instead of compared.
const GENERATOR_VERSION: &str = "scenario-gen-1";

/// A room "reads occupied" when presence is reported for more than this share
/// of the empty, post-warm-up frames.
const OCCUPIED_SHARE_PCT: f64 = 5.0;
/// A still person counts as "held" when presence stays on for at least this
/// share of the 100-240 s window.
const HELD_SHARE_PCT: f64 = 90.0;

/// Ratchet tolerances. The generator and the code are deterministic, so
/// these only absorb floating-point differences between platforms.
const TOL_PCT: f64 = 2.0;
const TOL_BPM: f64 = 0.25;

const C_LIGHT: f64 = 299_792_458.0;
const FC_HZ: f64 = 2.437e9; // channel 6
const DF_HZ: f64 = 312.5e3; // HT20 subcarrier spacing

// ---------------------------------------------------------------------------
// Deterministic random numbers (SplitMix64 + Box-Muller). No external crate,
// so the scenario set cannot drift when a dependency changes its algorithm.
// ---------------------------------------------------------------------------

struct Rng {
    state: u64,
    spare: Option<f64>,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x5DEE_CE66_D1CE_4E5B,
            spare: None,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    /// Standard normal.
    fn normal(&mut self) -> f64 {
        if let Some(s) = self.spare.take() {
            return s;
        }
        let u1 = 1.0 - self.uniform(); // (0, 1]
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        self.spare = Some(r * (2.0 * PI * u2).sin());
        r * (2.0 * PI * u2).cos()
    }
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Person {
    None,
    Still,
    Walk,
    EnterThenStill,
}

#[derive(Clone, Copy, PartialEq)]
enum Family {
    Empty,
    Walk,
    EnterStill,
    Vitals,
}

#[derive(Clone)]
struct Scenario {
    name: String,
    family: Family,
    seconds: f64,
    fs: f64,
    seed: u64,
    person: Person,
    /// Person reflection power relative to the static channel, in dB.
    dyn_db: f64,
    breath_bpm: f64,
    heart_bpm: f64,
    /// Peak chest displacement from breathing, in mm.
    breath_mm: f64,
    /// Peak chest displacement from the heartbeat, in mm.
    heart_mm: f64,
    /// Standard deviation of the packet-detection delay, in ns.
    sto_ns: f64,
    /// Standard deviation of the per-packet gain jitter, in dB.
    agc_db: f64,
    snr_db: f64,
}

impl Scenario {
    fn new(name: String, family: Family, seed: u64, person: Person) -> Self {
        Self {
            name,
            family,
            seconds: 120.0,
            fs: 20.0,
            seed,
            person,
            dyn_db: -15.0,
            breath_bpm: 15.0,
            heart_bpm: 72.0,
            breath_mm: 2.5,
            heart_mm: 0.25,
            sto_ns: 25.0,
            agc_db: 0.5,
            snr_db: 25.0,
        }
    }
}

/// The 39 scenarios from the review: 5 empty rooms, 5 walking, 5 that enter,
/// walk and then sit still, and 24 still people across three reflection
/// strengths for vital signs.
fn scenarios() -> Vec<Scenario> {
    let mut out = Vec::new();
    for s in 0..5u64 {
        out.push(Scenario::new(
            format!("empty_s{s}"),
            Family::Empty,
            s,
            Person::None,
        ));
    }
    for s in 0..5u64 {
        let mut sc = Scenario::new(format!("walk_s{s}"), Family::Walk, 100 + s, Person::Walk);
        sc.dyn_db = -12.0;
        out.push(sc);
    }
    for s in 0..5u64 {
        let mut sc = Scenario::new(
            format!("enter_still_s{s}"),
            Family::EnterStill,
            200 + s,
            Person::EnterThenStill,
        );
        sc.seconds = 240.0;
        out.push(sc);
    }
    let mut pick = Rng::new(7);
    let mut i = 0u64;
    for dyn_db in [-10.0, -15.0, -20.0] {
        for _ in 0..8 {
            let mut sc = Scenario::new(
                format!("vitals_{i:02}"),
                Family::Vitals,
                300 + i,
                Person::Still,
            );
            sc.seconds = 90.0;
            sc.dyn_db = dyn_db;
            sc.breath_bpm = (pick.range(8.0, 24.0) * 10.0).round() / 10.0;
            sc.heart_bpm = (pick.range(55.0, 100.0) * 10.0).round() / 10.0;
            out.push(sc);
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

struct Generated {
    amplitudes: Vec<Vec<f64>>,
    phases: Vec<Vec<f64>>,
    present: Vec<bool>,
}

/// Multiplies a complex number by e^{j*theta}.
fn rotate(re: f64, im: f64, theta: f64) -> (f64, f64) {
    let (s, c) = theta.sin_cos();
    (re * c - im * s, re * s + im * c)
}

fn generate(sc: &Scenario) -> Generated {
    let mut rng = Rng::new(sc.seed);
    let k_idx: Vec<f64> = (-26..=26).filter(|k| *k != 0).map(|k| k as f64).collect();
    let fk: Vec<f64> = k_idx.iter().map(|k| FC_HZ + k * DF_HZ).collect();
    let n = (sc.seconds * sc.fs).round() as usize;

    // Static room: six paths with exponentially decaying power.
    let mut delays: Vec<f64> = (0..6).map(|_| rng.range(10e-9, 80e-9)).collect();
    delays.sort_by(f64::total_cmp);
    let gains: Vec<(f64, f64)> = delays
        .iter()
        .map(|d| {
            let s = ((-d / 30e-9).exp() / 2.0).sqrt();
            (s * rng.normal(), s * rng.normal())
        })
        .collect();
    let hs: Vec<(f64, f64)> = fk
        .iter()
        .map(|f| {
            delays.iter().zip(&gains).fold((0.0, 0.0), |acc, (d, g)| {
                let (re, im) = rotate(g.0, g.1, -2.0 * PI * f * d);
                (acc.0 + re, acc.1 + im)
            })
        })
        .collect();
    let p_static = hs.iter().map(|(re, im)| re * re + im * im).sum::<f64>() / hs.len() as f64;
    let a_dyn = (p_static * 10f64.powf(sc.dyn_db / 10.0)).sqrt();

    let phi_b = rng.range(0.0, 2.0 * PI);
    let phi_h = rng.range(0.0, 2.0 * PI);
    // Slow sub-millimetre drift of a still body.
    let mut sway = Vec::with_capacity(n);
    let mut acc = 0.0;
    for _ in 0..n {
        acc += rng.normal();
        sway.push(2.0 * 0.4e-3 * acc / (n as f64).sqrt());
    }

    let d0 = 4.0; // reflection path length, m
    let (t_enter, t_sit) = (30.0, 40.0);
    let sit_index = (t_sit * sc.fs).round() as usize;
    let walk_path = |t: f64| d0 + 1.5 * (0.8 / 1.5 * t).sin();
    let sit_anchor = walk_path(sit_index as f64 / sc.fs - t_enter);

    let noise_std = (p_static * 10f64.powf(-sc.snr_db / 10.0) / 2.0).sqrt();
    let scale = 20.0 / p_static.sqrt();
    let quantize = |x: f64| (x * scale).round_ties_even().clamp(-128.0, 127.0);

    let mut out = Generated {
        amplitudes: Vec::with_capacity(n),
        phases: Vec::with_capacity(n),
        present: Vec::with_capacity(n),
    };
    for i in 0..n {
        let t = i as f64 / sc.fs;
        let chest = 2.0 * sc.breath_mm * 1e-3 * (2.0 * PI * sc.breath_bpm / 60.0 * t + phi_b).sin()
            + 2.0 * sc.heart_mm * 1e-3 * (2.0 * PI * sc.heart_bpm / 60.0 * t + phi_h).sin();
        let (present, moving, path) = match sc.person {
            Person::None => (false, false, 0.0),
            Person::Still => (true, false, d0 + chest + sway[i]),
            Person::Walk => (true, true, walk_path(t) + chest),
            Person::EnterThenStill => {
                if t < t_enter {
                    (false, false, 0.0)
                } else if i < sit_index {
                    (true, true, walk_path(t - t_enter))
                } else {
                    (true, false, sit_anchor + chest + sway[i])
                }
            }
        };
        let flutter = if moving {
            1.0 + 0.3 * (2.0 * PI * 0.7 * t + 1.0).sin()
        } else {
            1.0
        };

        // Per-packet impairments.
        let cpo = rng.range(-PI, PI);
        let sto = rng.normal() * sc.sto_ns * 1e-9;
        let agc = 10f64.powf(rng.normal() * sc.agc_db / 20.0);

        let mut amps = Vec::with_capacity(fk.len());
        let mut phases = Vec::with_capacity(fk.len());
        for (j, f) in fk.iter().enumerate() {
            let (mut re, mut im) = hs[j];
            if present {
                let (pr, pi) = rotate(a_dyn * flutter, 0.0, -2.0 * PI * f * path / C_LIGHT);
                re += pr;
                im += pi;
            }
            let (rr, ri) = rotate(agc * re, agc * im, cpo - 2.0 * PI * k_idx[j] * DF_HZ * sto);
            let qr = quantize(rr + noise_std * rng.normal());
            let qi = quantize(ri + noise_std * rng.normal());
            amps.push(qr.hypot(qi));
            phases.push(qi.atan2(qr));
        }
        out.amplitudes.push(amps);
        out.phases.push(phases);
        out.present.push(present);
    }
    out
}

// ---------------------------------------------------------------------------
// Replay through the server's per-node path
// ---------------------------------------------------------------------------

struct FrameOut {
    presence: bool,
    detector_br: Option<f64>,
    detector_hr: Option<f64>,
}

#[derive(Default)]
struct Timing {
    frames: u64,
    features_us: f64,
    vitals_us: f64,
    total_us: f64,
}

fn frame_from(amplitudes: Vec<f64>, phases: Vec<f64>, sequence: u32) -> Esp32Frame {
    Esp32Frame {
        magic: 0xC511_0001,
        node_id: 1,
        n_antennas: 1,
        n_subcarriers: amplitudes.len() as u16,
        freq_mhz: 2437,
        sequence,
        rssi: -50,
        noise_floor: -90,
        ppdu_type: wifi_densepose_hardware::PpduType::HtLegacy,
        adr018_flags: wifi_densepose_hardware::Adr018Flags::default(),
        amplitudes,
        phases,
    }
}

fn replay(sc: &Scenario, g: &Generated, timing: &mut Timing) -> Vec<FrameOut> {
    let mut ns = NodeState::new();
    ns.csi_fps_ema = sc.fs;
    ns.vital_detector = VitalSignDetector::new(sc.fs);
    let mut out = Vec::with_capacity(g.amplitudes.len());
    for (i, (a, p)) in g.amplitudes.iter().zip(&g.phases).enumerate() {
        let frame = frame_from(a.clone(), p.clone(), i as u32);
        let t_all = Instant::now();
        ns.frame_history.push_back(frame.amplitudes.clone());
        if ns.frame_history.len() > FRAME_HISTORY_CAPACITY {
            ns.frame_history.pop_front();
        }
        let t0 = Instant::now();
        let (_features, mut cls, _br_hz, _sub_var, raw_motion) =
            extract_features_from_frame(&frame, &ns.frame_history, sc.fs);
        smooth_and_classify_node(&mut ns, &mut cls, raw_motion);
        let features_us = t0.elapsed().as_secs_f64() * 1e6;
        let t1 = Instant::now();
        let raw = ns
            .vital_detector
            .process_frame(&frame.amplitudes, &frame.phases);
        let _published = smooth_vitals_node(&mut ns, &raw);
        let vitals_us = t1.elapsed().as_secs_f64() * 1e6;
        let _persons = estimate_persons_from_correlation(&ns.frame_history);
        timing.frames += 1;
        timing.features_us += features_us;
        timing.vitals_us += vitals_us;
        timing.total_us += t_all.elapsed().as_secs_f64() * 1e6;
        out.push(FrameOut {
            presence: cls.presence,
            detector_br: raw.breathing_rate_bpm,
            detector_hr: raw.heart_rate_bpm,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy)]
struct EnterStill {
    /// Presence while the room is still empty (10-30 s). Lower is better.
    empty_10_30_pct: f64,
    walking_30_40_pct: f64,
    still_40_100_pct: f64,
    still_100_240_pct: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct VitalsLevel {
    /// Mean absolute breathing error over scenarios with an estimate (None
    /// when no scenario at this level produced one).
    breath_mae_bpm: Option<f64>,
    /// Scenarios with no breathing estimate in the last 15 s.
    breath_missing: usize,
    hr_mae_bpm: Option<f64>,
    hr_missing: usize,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct VitalsScenario {
    dyn_db: f64,
    breath_true_bpm: f64,
    /// Median detector estimate over the last 15 s.
    breath_est_bpm: Option<f64>,
    heart_true_bpm: f64,
    heart_est_bpm: Option<f64>,
}

#[derive(Serialize, Deserialize)]
struct Summary {
    /// Empty rooms that read as occupied (more than 5% of post-warm-up frames).
    empty_rooms_read_occupied: usize,
    empty_rooms_total: usize,
    /// Enter-then-sit scenarios where the still person is held for at least
    /// 90% of 100-240 s.
    still_person_held: usize,
    /// Of those, the ones that also kept the empty 10-30 s window under 5%.
    still_person_held_without_false_presence: usize,
    still_person_total: usize,
}

#[derive(Serialize, Deserialize)]
struct Metrics {
    evidence: String,
    generator: String,
    summary: Summary,
    empty_false_presence_pct: BTreeMap<String, f64>,
    walk_detect_pct: BTreeMap<String, f64>,
    enter_still: BTreeMap<String, EnterStill>,
    /// Keyed by reflection strength in dB.
    vitals: BTreeMap<String, VitalsLevel>,
    /// Reported for debugging; not ratcheted.
    vitals_by_scenario: BTreeMap<String, VitalsScenario>,
    /// Heart-rate error from always guessing 80 BPM: the chance level.
    hr_chance_mae_bpm: f64,
}

fn share_pct(xs: &[FrameOut], from: usize, to: usize) -> f64 {
    let to = to.min(xs.len());
    if from >= to {
        return 0.0;
    }
    100.0 * xs[from..to].iter().filter(|f| f.presence).count() as f64 / (to - from) as f64
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    Some(if v.len() % 2 == 0 {
        (v[m - 1] + v[m]) / 2.0
    } else {
        v[m]
    })
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn score(runs: &[(Scenario, Vec<FrameOut>)]) -> Metrics {
    let mut m = Metrics {
        evidence: "SYNTHETIC".into(),
        generator: GENERATOR_VERSION.into(),
        summary: Summary {
            empty_rooms_read_occupied: 0,
            empty_rooms_total: 0,
            still_person_held: 0,
            still_person_held_without_false_presence: 0,
            still_person_total: 0,
        },
        empty_false_presence_pct: BTreeMap::new(),
        walk_detect_pct: BTreeMap::new(),
        enter_still: BTreeMap::new(),
        vitals: BTreeMap::new(),
        vitals_by_scenario: BTreeMap::new(),
        hr_chance_mae_bpm: 0.0,
    };
    let mut vitals_errors: BTreeMap<String, (Vec<f64>, usize, Vec<f64>, usize)> = BTreeMap::new();
    let mut chance = Vec::new();

    for (sc, out) in runs {
        let at = |s: f64| (s * sc.fs).round() as usize;
        match sc.family {
            Family::Empty => {
                let pct = round2(share_pct(out, at(10.0), out.len()));
                m.summary.empty_rooms_total += 1;
                if pct > OCCUPIED_SHARE_PCT {
                    m.summary.empty_rooms_read_occupied += 1;
                }
                m.empty_false_presence_pct.insert(sc.name.clone(), pct);
            }
            Family::Walk => {
                m.walk_detect_pct
                    .insert(sc.name.clone(), round2(share_pct(out, at(10.0), out.len())));
            }
            Family::EnterStill => {
                let e = EnterStill {
                    empty_10_30_pct: round2(share_pct(out, at(10.0), at(30.0))),
                    walking_30_40_pct: round2(share_pct(out, at(30.0), at(40.0))),
                    still_40_100_pct: round2(share_pct(out, at(40.0), at(100.0))),
                    still_100_240_pct: round2(share_pct(out, at(100.0), out.len())),
                };
                m.summary.still_person_total += 1;
                if e.still_100_240_pct >= HELD_SHARE_PCT {
                    m.summary.still_person_held += 1;
                    if e.empty_10_30_pct <= OCCUPIED_SHARE_PCT {
                        m.summary.still_person_held_without_false_presence += 1;
                    }
                }
                m.enter_still.insert(sc.name.clone(), e);
            }
            Family::Vitals => {
                let tail = &out[out.len().saturating_sub(at(15.0))..];
                let br = median(tail.iter().filter_map(|f| f.detector_br).collect());
                let hr = median(tail.iter().filter_map(|f| f.detector_hr).collect());
                let entry = vitals_errors
                    .entry(format!("{:.0}dB", sc.dyn_db))
                    .or_insert_with(|| (Vec::new(), 0, Vec::new(), 0));
                match br {
                    Some(b) => entry.0.push((b - sc.breath_bpm).abs()),
                    None => entry.1 += 1,
                }
                match hr {
                    Some(h) => entry.2.push((h - sc.heart_bpm).abs()),
                    None => entry.3 += 1,
                }
                chance.push((sc.heart_bpm - 80.0).abs());
                m.vitals_by_scenario.insert(
                    sc.name.clone(),
                    VitalsScenario {
                        dyn_db: sc.dyn_db,
                        breath_true_bpm: sc.breath_bpm,
                        breath_est_bpm: br.map(round2),
                        heart_true_bpm: sc.heart_bpm,
                        heart_est_bpm: hr.map(round2),
                    },
                );
            }
        }
    }
    let mean = |v: &[f64]| (!v.is_empty()).then(|| round2(v.iter().sum::<f64>() / v.len() as f64));
    for (level, (br, br_missing, hr, hr_missing)) in vitals_errors {
        m.vitals.insert(
            level,
            VitalsLevel {
                breath_mae_bpm: mean(&br),
                breath_missing: br_missing,
                hr_mae_bpm: mean(&hr),
                hr_missing,
            },
        );
    }
    m.hr_chance_mae_bpm = mean(&chance).unwrap_or(0.0);
    m
}

// ---------------------------------------------------------------------------
// Ratchet against the committed baseline
// ---------------------------------------------------------------------------

fn regressions(now: &Metrics, base: &Metrics) -> Vec<String> {
    let mut bad = Vec::new();
    for (name, b) in &base.empty_false_presence_pct {
        let n = now
            .empty_false_presence_pct
            .get(name)
            .copied()
            .unwrap_or(f64::INFINITY);
        if n > b + TOL_PCT {
            bad.push(format!(
                "{name}: empty-room false presence {n:.1}% (baseline {b:.1}%)"
            ));
        }
    }
    for (name, b) in &base.walk_detect_pct {
        let n = now.walk_detect_pct.get(name).copied().unwrap_or(0.0);
        if n < b - TOL_PCT {
            bad.push(format!(
                "{name}: walking detection {n:.1}% (baseline {b:.1}%)"
            ));
        }
    }
    for (name, b) in &base.enter_still {
        let Some(n) = now.enter_still.get(name) else {
            bad.push(format!("{name}: missing"));
            continue;
        };
        if n.empty_10_30_pct > b.empty_10_30_pct + TOL_PCT {
            bad.push(format!(
                "{name}: false presence before entry {:.1}% (baseline {:.1}%)",
                n.empty_10_30_pct, b.empty_10_30_pct
            ));
        }
        for (label, nv, bv) in [
            ("walking 30-40 s", n.walking_30_40_pct, b.walking_30_40_pct),
            ("still 40-100 s", n.still_40_100_pct, b.still_40_100_pct),
            ("still 100-240 s", n.still_100_240_pct, b.still_100_240_pct),
        ] {
            if nv < bv - TOL_PCT {
                bad.push(format!(
                    "{name}: presence {label} {nv:.1}% (baseline {bv:.1}%)"
                ));
            }
        }
    }
    // Heart rate is not ratcheted: it is at chance level and is being hidden
    // (plan item 0.4), so its number is reported only.
    for (level, b) in &base.vitals {
        let Some(n) = now.vitals.get(level) else {
            bad.push(format!("vitals {level}: missing"));
            continue;
        };
        if n.breath_missing > b.breath_missing {
            bad.push(format!(
                "vitals {level}: {} scenarios without a breathing estimate (baseline {})",
                n.breath_missing, b.breath_missing
            ));
        }
        if let Some(bb) = b.breath_mae_bpm {
            match n.breath_mae_bpm {
                Some(nb) if nb <= bb + TOL_BPM => {}
                Some(nb) => bad.push(format!(
                    "vitals {level}: breathing error {nb:.2} BPM (baseline {bb:.2})"
                )),
                None => bad.push(format!(
                    "vitals {level}: no breathing estimates (baseline error {bb:.2} BPM)"
                )),
            }
        }
    }
    bad
}

fn report_markdown(m: &Metrics, t: &Timing) -> String {
    let s = &m.summary;
    let mut r = String::new();
    let _ = writeln!(r, "## Scenario harness (SYNTHETIC, `{}`)\n", m.generator);
    let _ = writeln!(
        r,
        "Accuracy numbers are SYNTHETIC: simulated rooms, not real ones. Timings are MEASURED on this runner.\n"
    );
    let _ = writeln!(r, "| Check | Result |\n|---|---|");
    let _ = writeln!(
        r,
        "| Empty rooms that read occupied (>{OCCUPIED_SHARE_PCT}% of frames) | {} of {} |",
        s.empty_rooms_read_occupied, s.empty_rooms_total
    );
    let _ = writeln!(
        r,
        "| Still person held (>={HELD_SHARE_PCT}% of 100-240 s) | {} of {} |",
        s.still_person_held, s.still_person_total
    );
    let _ = writeln!(
        r,
        "| ...and room not occupied before entry | {} of {} |",
        s.still_person_held_without_false_presence, s.still_person_total
    );
    let bpm = |x: Option<f64>| x.map_or("n/a".to_string(), |v| format!("{v:.2} BPM"));
    for (level, v) in &m.vitals {
        let _ = writeln!(
            r,
            "| Breathing error at {level} reflection | {} ({} of 8 without an estimate) |",
            bpm(v.breath_mae_bpm),
            v.breath_missing
        );
    }
    for (level, v) in &m.vitals {
        let _ = writeln!(
            r,
            "| Heart-rate error at {level} (chance: {:.2} BPM) | {} |",
            m.hr_chance_mae_bpm,
            bpm(v.hr_mae_bpm)
        );
    }
    let per = |x: f64| x / t.frames.max(1) as f64;
    let _ = writeln!(
        r,
        "| Per-frame time, features / vitals / total (MEASURED) | {:.0} / {:.0} / {:.0} µs |",
        per(t.features_us),
        per(t.vitals_us),
        per(t.total_us)
    );
    let _ = writeln!(r, "\n<details><summary>Per-scenario presence</summary>\n");
    let _ = writeln!(r, "| Scenario | Presence |\n|---|---|");
    for (name, v) in &m.empty_false_presence_pct {
        let _ = writeln!(r, "| {name} (empty) | {v:.1}% |");
    }
    for (name, v) in &m.walk_detect_pct {
        let _ = writeln!(r, "| {name} (walking) | {v:.1}% |");
    }
    for (name, e) in &m.enter_still {
        let _ = writeln!(
            r,
            "| {name} (empty 10-30 / walk / still 40-100 / still 100-240) | {:.1}% / {:.1}% / {:.1}% / {:.1}% |",
            e.empty_10_30_pct, e.walking_30_40_pct, e.still_40_100_pct, e.still_100_240_pct
        );
    }
    let _ = writeln!(
        r,
        "\n</details>\n\n<details><summary>Per-scenario vital signs</summary>\n"
    );
    let _ = writeln!(r, "| Scenario | Reflection | Breathing true / estimated | Heart rate true / estimated |\n|---|---|---|---|");
    let est = |x: Option<f64>| x.map_or("none".to_string(), |v| format!("{v:.1}"));
    for (name, v) in &m.vitals_by_scenario {
        let _ = writeln!(
            r,
            "| {name} | {:.0} dB | {:.1} / {} | {:.1} / {} |",
            v.dyn_db,
            v.breath_true_bpm,
            est(v.breath_est_bpm),
            v.heart_true_bpm,
            est(v.heart_est_bpm)
        );
    }
    let _ = writeln!(r, "\n</details>");
    r
}

#[test]
fn generator_is_deterministic_and_scenario_set_is_stable() {
    let all = scenarios();
    assert_eq!(all.len(), 39);
    let sc = &all[0];
    let mut short = sc.clone();
    short.seconds = 2.0;
    let a = generate(&short);
    let b = generate(&short);
    assert_eq!(a.amplitudes, b.amplitudes);
    assert_eq!(a.phases, b.phases);
    assert_eq!(a.amplitudes.len(), 40);
    assert_eq!(a.amplitudes[0].len(), 52);
    // int8 I/Q: amplitudes are bounded by the quantizer.
    assert!(a
        .amplitudes
        .iter()
        .flatten()
        .all(|x| *x <= 128.0 * std::f64::consts::SQRT_2));
}

#[test]
#[ignore = "slow in debug builds; CI runs it with --release (scenario-harness.yml)"]
fn scenario_harness() {
    let mut timing = Timing::default();
    let runs: Vec<(Scenario, Vec<FrameOut>)> = scenarios()
        .into_iter()
        .map(|sc| {
            let g = generate(&sc);
            let out = replay(&sc, &g, &mut timing);
            (sc, out)
        })
        .collect();
    let metrics = score(&runs);
    let report = report_markdown(&metrics, &timing);
    eprintln!("{report}");
    if let Ok(path) = std::env::var("RUVIEW_SCENARIO_REPORT") {
        std::fs::write(&path, &report).expect("write RUVIEW_SCENARIO_REPORT");
    }

    let baseline_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scenario_baseline.json");
    if std::env::var("RUVIEW_SCENARIO_UPDATE_BASELINE").as_deref() == Ok("1") {
        let json = serde_json::to_string_pretty(&metrics).expect("serialize metrics");
        std::fs::write(&baseline_path, json + "\n").expect("write baseline");
        eprintln!("baseline written to {}", baseline_path.display());
        return;
    }
    let base: Metrics = serde_json::from_str(
        &std::fs::read_to_string(&baseline_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", baseline_path.display())),
    )
    .expect("parse baseline");
    assert_eq!(
        base.generator, GENERATOR_VERSION,
        "baseline was produced by a different generator; refresh it with RUVIEW_SCENARIO_UPDATE_BASELINE=1"
    );
    let bad = regressions(&metrics, &base);
    assert!(
        bad.is_empty(),
        "SYNTHETIC scenario metrics got worse than the committed baseline:\n  {}\n\
         If the change is intentional, refresh the baseline with RUVIEW_SCENARIO_UPDATE_BASELINE=1 and explain why in the PR.",
        bad.join("\n  ")
    );
}
