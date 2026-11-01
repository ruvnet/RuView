//! Label RuView recordings with occupancy from radar logs.
//!
//! ```text
//! cargo run -p ruview-groundtruth --example radar_autolabel -- \
//!     --session mon data/recordings/mon.csi.jsonl radar/mon.jsonl \
//!     --session tue data/recordings/tue.csi.jsonl radar/tue.jsonl \
//!     --test-session tue --offset-ms 0 --out labels.jsonl
//! ```
//!
//! Writes one labelled window per line (to `--out`, or stdout) and a JSON
//! summary to stderr: per-session kept/skipped counts and, when
//! `--test-session` is given, the session-disjoint majority/median baseline.
//! `--offset-ms` is the recording clock minus the radar clock and applies to
//! every session. `--allow-unverified` accepts radar windows whose source is
//! not flagged as verified hardware.

use std::fs;
use std::io::Write;
use std::process::ExitCode;

use ruview_groundtruth::{
    auto_label, majority_baseline, parse_radar_jsonl, parse_recording_jsonl,
    session_disjoint_split, AutoLabelConfig, LabelledWindow, RadarAdapterConfig, RadarLabels,
};
use serde_json::json;

struct Args {
    sessions: Vec<(String, String, String)>,
    test_sessions: Vec<String>,
    out: Option<String>,
    label: AutoLabelConfig,
    radar: RadarAdapterConfig,
}

fn next(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        sessions: Vec::new(),
        test_sessions: Vec::new(),
        out: None,
        label: AutoLabelConfig::default(),
        radar: RadarAdapterConfig::default(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--session" => {
                let name = next(&mut it, "--session")?;
                let rec = next(&mut it, "--session")?;
                let radar = next(&mut it, "--session")?;
                a.sessions.push((name, rec, radar));
            }
            "--test-session" => a.test_sessions.push(next(&mut it, &flag)?),
            "--out" => a.out = Some(next(&mut it, &flag)?),
            "--offset-ms" => {
                a.label.radar_offset_ms = next(&mut it, &flag)?
                    .parse()
                    .map_err(|e| format!("--offset-ms: {e}"))?;
            }
            "--min-frames" => {
                a.label.min_csi_frames = next(&mut it, &flag)?
                    .parse()
                    .map_err(|e| format!("--min-frames: {e}"))?;
            }
            "--empty-hold-ms" => {
                a.label.empty_hold_ms = next(&mut it, &flag)?
                    .parse()
                    .map_err(|e| format!("--empty-hold-ms: {e}"))?;
            }
            "--allow-unverified" => a.radar.require_verified = false,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if a.sessions.is_empty() {
        return Err("at least one --session NAME RECORDING RADAR is required".into());
    }
    Ok(a)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let mut all: Vec<LabelledWindow> = Vec::new();
    let mut per_session = Vec::new();

    for (name, rec_path, radar_path) in &args.sessions {
        let rec = fs::read_to_string(rec_path).map_err(|e| format!("{rec_path}: {e}"))?;
        let rad = fs::read_to_string(radar_path).map_err(|e| format!("{radar_path}: {e}"))?;
        let frames = parse_recording_jsonl(&rec).map_err(|e| format!("{rec_path}: {e}"))?;
        let windows = parse_radar_jsonl(&rad).map_err(|e| format!("{radar_path}: {e}"))?;
        let labels = RadarLabels::from_windows(name, "radar", &windows, args.radar)
            .map_err(|e| e.to_string())?;
        let out = auto_label(name, &frames, &labels, args.label).map_err(|e| e.to_string())?;
        per_session.push(json!({
            "session": name,
            "csi_frames": frames.len(),
            "radar_windows": windows.len(),
            "kept": out.windows.len(),
            "skipped": out.skipped,
            "radar_unknown": out.radar_unknown,
        }));
        all.extend(out.windows);
    }

    let mut sink: Box<dyn Write> = match &args.out {
        Some(p) => Box::new(fs::File::create(p).map_err(|e| format!("{p}: {e}"))?),
        None => Box::new(std::io::stdout().lock()),
    };
    for w in &all {
        let line = serde_json::to_string(w).map_err(|e| e.to_string())?;
        writeln!(sink, "{line}").map_err(|e| e.to_string())?;
    }

    let baseline = if args.test_sessions.is_empty() {
        None
    } else {
        let names: Vec<&str> = args.test_sessions.iter().map(String::as_str).collect();
        let split = session_disjoint_split(&all, &names).map_err(|e| e.to_string())?;
        let b = majority_baseline(&split.train, &split.test).map_err(|e| e.to_string())?;
        Some(json!({
            "train_sessions": split.train_sessions,
            "test_sessions": split.test_sessions,
            "baseline": b,
        }))
    };
    let summary = json!({
        "offset_ms": args.label.radar_offset_ms,
        "label_config": args.label,
        "radar_config": args.radar,
        "sessions": per_session,
        "split": baseline,
    });
    eprintln!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("radar_autolabel: {e}");
            ExitCode::FAILURE
        }
    }
}
