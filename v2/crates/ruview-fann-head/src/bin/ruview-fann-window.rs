//! Re-encode a server feature capture with the windowed schema (ADR-426 D6).
//!
//! ```text
//! ruview-fann-window --in capture.jsonl --out capture.windowed.jsonl
//! ```
//!
//! Reads records written under `ruview.node-features/1`, runs each node's
//! records through the same [`WindowEncoder`] the server uses live, and writes
//! them back with `ruview.node-features/2` features. Records seen while a
//! node's window is warming up are dropped and counted. Every other field is
//! passed through unchanged.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::process::ExitCode;

use ruview_fann_head::features::{
    WindowEncoder, NODE_FEATURE_COUNT, NODE_FEATURE_SCHEMA, WINDOW_FEATURE_SCHEMA,
};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn base_features(record: &serde_json::Value) -> Option<(i64, u64, [f32; NODE_FEATURE_COUNT])> {
    if record["feature_schema"] != NODE_FEATURE_SCHEMA {
        return None;
    }
    let timestamp = record["timestamp_ms"].as_i64()?;
    let node = record["node_id"].as_u64()?;
    let values = record["features"].as_array()?;
    if values.len() != NODE_FEATURE_COUNT {
        return None;
    }
    let mut base = [0.0f32; NODE_FEATURE_COUNT];
    for (slot, value) in base.iter_mut().zip(values) {
        let v = value.as_f64()?;
        if !v.is_finite() {
            return None;
        }
        *slot = v as f32;
    }
    Some((timestamp, node, base))
}

fn run(args: &[String]) -> Result<(), String> {
    let (Some(input), Some(output)) = (arg(args, "--in"), arg(args, "--out")) else {
        return Err("usage: ruview-fann-window --in <capture.jsonl> --out <windowed.jsonl>".into());
    };
    let reader = std::io::BufReader::new(
        std::fs::File::open(&input).map_err(|e| format!("cannot read {input}: {e}"))?,
    );
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(&output).map_err(|e| format!("cannot write {output}: {e}"))?,
    );
    let mut encoders: BTreeMap<u64, WindowEncoder> = BTreeMap::new();
    let (mut read, mut bad, mut warming, mut written) = (0u64, 0u64, 0u64, 0u64);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("cannot read {input}: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        read += 1;
        let Some((mut record, (timestamp, node, base))) =
            serde_json::from_str::<serde_json::Value>(&line)
                .ok()
                .and_then(|r| base_features(&r).map(|b| (r, b)))
        else {
            bad += 1;
            continue;
        };
        let Some(windowed) = encoders.entry(node).or_default().push(timestamp, base) else {
            warming += 1;
            continue;
        };
        record["feature_schema"] = WINDOW_FEATURE_SCHEMA.into();
        record["features"] = serde_json::json!(windowed);
        // A candidate score recorded against the old features no longer applies.
        record["candidate"] = serde_json::Value::Null;
        writeln!(writer, "{record}").map_err(|e| format!("cannot write {output}: {e}"))?;
        written += 1;
    }
    writer
        .flush()
        .map_err(|e| format!("cannot write {output}: {e}"))?;
    eprintln!(
        "{}",
        serde_json::json!({"read": read, "written": written, "warming_or_gap": warming, "bad": bad})
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
