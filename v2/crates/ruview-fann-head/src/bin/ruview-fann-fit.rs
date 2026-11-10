//! Fit and export a head from replay rows (ADR-426 D3/D5).
//!
//! ```text
//! ruview-fann-fit --rows rows.jsonl --room office --feature-schema ruview.node-features/1 \
//!     --sources csi-node-1 --out office.head.json [--linear]
//! ```
//!
//! Fits on the room's Train rows only, so the exported head has never seen a
//! Validation or HeldOut row. `--linear` fits the linear control instead of
//! the candidate.

use std::process::ExitCode;

use ruview_fann_head::replay::{parse_rows, Split};
use ruview_fann_head::train::{fit, FitConfig};
use ruview_fann_head::HeadContext;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run(args: &[String]) -> Result<(), String> {
    let (Some(rows_path), Some(room), Some(schema), Some(sources), Some(out)) = (
        arg(args, "--rows"),
        arg(args, "--room"),
        arg(args, "--feature-schema"),
        arg(args, "--sources"),
        arg(args, "--out"),
    ) else {
        return Err(
            "usage: ruview-fann-fit --rows <jsonl> --room <name> --feature-schema <id> \
             --sources <a,b> --out <head.json> [--linear]"
                .into(),
        );
    };
    let text =
        std::fs::read_to_string(&rows_path).map_err(|e| format!("cannot read {rows_path}: {e}"))?;
    let rows = parse_rows(&text).map_err(|e| e.to_string())?;
    let train: Vec<_> = rows
        .iter()
        .filter(|r| r.room == room && r.split == Split::Train)
        .collect();
    let Some(width) = train.first().map(|r| r.features.len()) else {
        return Err(format!("no Train rows for room {room}"));
    };
    if train.iter().any(|r| r.features.len() != width) {
        return Err("Train rows disagree on the feature count".into());
    }
    let features: Vec<f32> = train
        .iter()
        .flat_map(|r| r.features.iter().copied())
        .collect();
    let labels: Vec<bool> = train.iter().map(|r| r.condition.occupied()).collect();
    let config = if args.iter().any(|a| a == "--linear") {
        FitConfig::linear_control()
    } else {
        FitConfig::candidate()
    };
    let context = HeadContext {
        feature_schema: schema,
        sources: sources.split(',').map(str::to_string).collect(),
        room_revision: room.clone(),
    };
    let report = fit(&features, &labels, context, config).map_err(|e| e.to_string())?;
    std::fs::write(&out, report.head.to_json() + "\n")
        .map_err(|e| format!("cannot write {out}: {e}"))?;
    eprintln!(
        "fitted {} parameters on {} Train rows of room {room} (train MSE {:.4}); digest {}",
        report.head.parameter_count(),
        report.rows,
        report.final_train_mse,
        report.head.digest()
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
