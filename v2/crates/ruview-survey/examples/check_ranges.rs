//! Run check mode on a JSONL file of range observations.
//!
//! ```text
//! cargo run -p ruview-survey --example check_ranges -- \
//!     --node-positions "1:0,0,1.5;2:4.2,0,1.5;3:4.2,3.6,1.5" \
//!     --observations ranges.jsonl [--profile ftm|uwb]
//! ```
//!
//! Each line of `ranges.jsonl` is one `RangeObservation`, for example
//! `{"initiator":1,"responder":2,"range_m":4.6,"sigma_m":0.3,"method":"ftm"}`.
//! Prints the report as JSON. Exit status 0 when consistent, 1 on warnings
//! or insufficient data, 2 on bad input. Never writes anything.

use std::process::ExitCode;

use ruview_survey::{check, parse_node_positions, CheckParams, RangeObservation, Verdict};

/// Largest observations file read, in bytes.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

fn run() -> Result<Verdict, String> {
    let mut positions = None;
    let mut path = None;
    let mut profile = "ftm".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--node-positions" => positions = Some(value()?),
            "--observations" => path = Some(value()?),
            "--profile" => profile = value()?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let positions = parse_node_positions(&positions.ok_or("--node-positions is required")?)
        .map_err(|e| e.to_string())?;
    let path = path.ok_or("--observations is required")?;
    let params = match profile.as_str() {
        "ftm" => CheckParams::ftm_uncalibrated(),
        "uwb" => CheckParams::uwb(),
        other => return Err(format!("unknown profile {other} (ftm|uwb)")),
    };

    let meta = std::fs::metadata(&path).map_err(|e| format!("{path}: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{path}: not a regular file under {MAX_FILE_BYTES} bytes"
        ));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let observations = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            serde_json::from_str::<RangeObservation>(l)
                .map_err(|e| format!("{path}:{}: {e}", i + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let report = check(&positions, &observations, &params).map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(report.verdict)
}

fn main() -> ExitCode {
    match run() {
        Ok(Verdict::Consistent) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(e) => {
            eprintln!("check_ranges: {e}");
            ExitCode::from(2)
        }
    }
}
