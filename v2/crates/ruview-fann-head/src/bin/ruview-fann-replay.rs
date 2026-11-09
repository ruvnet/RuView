//! Replay comparison CLI (ADR-425 D5).
//!
//! ```text
//! ruview-fann-replay --rows replay.jsonl --feature-schema aether-skip/1 \
//!     --sources node-a,node-b [--out report.json]
//! ```

use std::process::ExitCode;

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
        eprintln!("usage: ruview-fann-replay --rows <jsonl> --feature-schema <id> --sources <a,b> [--out <json>]");
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
    let report = match parse_rows(&text)
        .and_then(|rows| evaluate(&rows, &context, FitConfig::candidate()))
    {
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
