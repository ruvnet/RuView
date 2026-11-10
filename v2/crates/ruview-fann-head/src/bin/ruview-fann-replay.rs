//! Replay comparison CLI (ADR-425 D5).
//!
//! ```text
//! ruview-fann-replay --rows replay.jsonl --feature-schema aether-skip/1 \
//!     --sources node-a,node-b [--out report.json] [--learning-curve curve.json]
//! ```

use std::process::ExitCode;

use ruview_fann_head::curve::learning_curve;
use ruview_fann_head::replay::{evaluate, parse_rows};
use ruview_fann_head::train::FitConfig;
use ruview_fann_head::HeadContext;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(rows_path), Some(schema), Some(sources)) = (
        arg(&args, "--rows"),
        arg(&args, "--feature-schema"),
        arg(&args, "--sources"),
    ) else {
        eprintln!("usage: ruview-fann-replay --rows <jsonl> --feature-schema <id> --sources <a,b> [--out <json>] [--learning-curve <json>]");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&rows_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {rows_path}: {e}");
            return ExitCode::from(2);
        }
    };
    let context = HeadContext {
        feature_schema: schema,
        sources: sources.split(',').map(str::to_string).collect(),
        room_revision: "replay".to_string(),
    };
    let rows = match parse_rows(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(path) = arg(&args, "--learning-curve") {
        let curve = match learning_curve(&rows, &context, FitConfig::candidate()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        };
        let json = serde_json::to_string_pretty(&curve).expect("curve serialises");
        if let Err(e) = std::fs::write(&path, json + "\n") {
            eprintln!("error: cannot write {path}: {e}");
            return ExitCode::from(2);
        }
        for room in &curve.rooms {
            eprintln!(
                "[{}] room {}: candidate passes from {:?} rows, linear control from {:?} rows ({} points by {})",
                curve.evidence,
                room.room,
                room.candidate_passes_from_rows,
                room.linear_control_passes_from_rows,
                room.points.len(),
                room.prefix_unit,
            );
        }
    }
    let report = match evaluate(&rows, &context, FitConfig::candidate()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let json = serde_json::to_string_pretty(&report).expect("report serialises");
    match arg(&args, "--out") {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, json + "\n") {
                eprintln!("error: cannot write {path}: {e}");
                return ExitCode::from(2);
            }
        }
        None => println!("{json}"),
    }
    if !report.labels_are_reference {
        eprintln!(
            "note: labels are {:?}, not an independent reference: this is not an accuracy result",
            report.label_sources
        );
    }
    for room in &report.rooms {
        eprintln!(
            "[{}] room {}: candidate gates {:?}, linear control {:?}, incumbent {:?}, pilot target {:?}",
            report.evidence,
            room.room,
            room.candidate.passes_local_gates,
            room.linear_control.passes_local_gates,
            room.incumbent.as_ref().map(|m| m.passes_local_gates),
            room.adjacent.as_ref().map(|a| a.meets_pilot_target),
        );
    }
    ExitCode::SUCCESS
}
