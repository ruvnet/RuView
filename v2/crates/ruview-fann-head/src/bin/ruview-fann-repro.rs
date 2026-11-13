//! Cross-machine reproducibility probe (ADR-426 D1).
//!
//! Fits the candidate and the linear control on a fixed synthetic data set
//! built from integer arithmetic only, then prints the head digests and a
//! digest of the scores on fixed probe vectors. Two machines agree on the fit
//! exactly when they print the same digests.
//!
//! ```text
//! ruview-fann-repro [--dump <dir>]
//! ruview-fann-repro --head <head.json> [--scores <file>]
//! ```
//!
//! The second form loads an exported head and scores the same probes, to
//! separate "is the fit reproducible" from "does one head score the same
//! everywhere". `--scores` writes each score's bits, one per line.
//!
//! SYNTHETIC input: this says whether fitting is reproducible, nothing else.

use std::process::ExitCode;

use ruview_fann_head::train::{fit, FitConfig};
use ruview_fann_head::{FannHead, HeadContext, RUV_FANN_REV};
use sha2::{Digest, Sha256};

const ROWS: usize = 256;
const WIDTH: usize = 10;
const PROBES: usize = 1024;

/// Deterministic value in [-1, 1) from integer state only.
fn unit(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state >> 40) as f32 / (1u64 << 23) as f32) - 1.0
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn score_digest(head: &FannHead, probes: &[f32]) -> String {
    let mut scorer = head.scorer().expect("scorer");
    let mut h = Sha256::new();
    for x in probes.chunks_exact(WIDTH) {
        h.update(head.score(&mut scorer, x).expect("score").to_le_bytes());
    }
    hex(&h.finalize())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dump = args
        .iter()
        .position(|a| a == "--dump")
        .and_then(|i| args.get(i + 1));

    let mut state = 0x00C0_FFEE_u64;
    let features: Vec<f32> = (0..ROWS * WIDTH).map(|_| unit(&mut state)).collect();
    // A rule a line cannot fit: occupied when exactly one of two features is high.
    let labels: Vec<bool> = features
        .chunks_exact(WIDTH)
        .map(|x| (x[0] > 0.0) != (x[1] > 0.0))
        .collect();
    let probes: Vec<f32> = (0..PROBES * WIDTH)
        .map(|_| unit(&mut state) * 2.0)
        .collect();
    let context = HeadContext {
        feature_schema: "repro/1".into(),
        sources: vec!["a".into(), "b".into()],
        room_revision: "repro".into(),
    };

    let mut out = serde_json::Map::new();
    out.insert("target".into(), env!("RUVIEW_FANN_TARGET").into());
    out.insert("engine_rev".into(), RUV_FANN_REV.into());

    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    if let Some(path) = flag("--head") {
        let head = match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| FannHead::from_json(&t).map_err(|e| e.to_string()))
        {
            Ok(h) => h,
            Err(e) => {
                eprintln!("error: cannot load {path}: {e}");
                return ExitCode::from(2);
            }
        };
        if head.features() != WIDTH {
            eprintln!("error: the probe set is {WIDTH} features wide");
            return ExitCode::from(2);
        }
        out.insert("head_digest".into(), head.digest().into());
        out.insert(
            "head_scores_sha256".into(),
            score_digest(&head, &probes).into(),
        );
        if let Some(scores) = flag("--scores") {
            let mut scorer = head.scorer().expect("scorer");
            let text: String = probes
                .chunks_exact(WIDTH)
                .map(|x| {
                    format!(
                        "{:08x}\n",
                        head.score(&mut scorer, x).expect("score").to_bits()
                    )
                })
                .collect();
            if let Err(e) = std::fs::write(scores, text) {
                eprintln!("error: cannot write {scores}: {e}");
                return ExitCode::from(2);
            }
        }
        println!("{}", serde_json::Value::Object(out));
        return ExitCode::SUCCESS;
    }
    for (name, config) in [
        ("candidate", FitConfig::candidate()),
        ("linear_control", FitConfig::linear_control()),
    ] {
        let report = match fit(&features, &labels, context.clone(), config) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {name}: {e}");
                return ExitCode::from(2);
            }
        };
        out.insert(format!("{name}_digest"), report.head.digest().into());
        out.insert(
            format!("{name}_scores_sha256"),
            score_digest(&report.head, &probes).into(),
        );
        out.insert(
            format!("{name}_final_train_mse_bits"),
            format!("{:08x}", report.final_train_mse.to_bits()).into(),
        );
        if let Some(dir) = dump {
            let path = format!("{dir}/{name}.head.json");
            if let Err(e) = std::fs::write(&path, report.head.to_json() + "\n") {
                eprintln!("error: cannot write {path}: {e}");
                return ExitCode::from(2);
            }
        }
    }
    println!("{}", serde_json::Value::Object(out));
    ExitCode::SUCCESS
}
