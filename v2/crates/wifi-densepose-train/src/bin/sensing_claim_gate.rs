//! CLI for the ADR-328 sensing evidence and claim gate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use wifi_densepose_train::sensing_claim_gate::{
    evaluate_claim_json_for_class, ClaimClass, MAX_INPUT_BYTES,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RequiredClass {
    Research,
    Production,
    SafetyCritical,
}

impl From<RequiredClass> for ClaimClass {
    fn from(value: RequiredClass) -> Self {
        match value {
            RequiredClass::Research => ClaimClass::Research,
            RequiredClass::Production => ClaimClass::Production,
            RequiredClass::SafetyCritical => ClaimClass::SafetyCritical,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "sensing-claim-gate",
    about = "Validate a RuView sensing claim and emit a content-addressed JSON receipt"
)]
struct Args {
    /// Strict JSON claim manifest.
    #[arg(long)]
    manifest: PathBuf,

    /// Repository-owned claim policy.
    #[arg(long)]
    policy: PathBuf,

    /// Release class selected by the protected caller. The manifest must match.
    #[arg(long, value_enum)]
    required_class: RequiredClass,

    /// Optional path for the same receipt printed to stdout. The parent must
    /// already exist, which avoids creating directories from untrusted input.
    #[arg(long)]
    receipt: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("sensing claim gate error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<u8> {
    let manifest = read_metadata_json(&args.manifest, "manifest")?;
    let policy = read_metadata_json(&args.policy, "policy")?;
    let envelope = evaluate_claim_json_for_class(&manifest, &policy, args.required_class.into())?;
    let output = serde_json::to_string_pretty(&envelope).context("serialize receipt envelope")?;

    println!("{output}");
    if let Some(path) = args.receipt {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.is_dir() {
            bail!("receipt parent does not exist: {}", parent.display());
        }
        fs::write(&path, format!("{output}\n"))
            .with_context(|| format!("write receipt {}", path.display()))?;
    }

    Ok(if envelope.receipt.claim_releasable {
        0
    } else {
        2
    })
}

fn read_metadata_json(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata =
        fs::metadata(path).with_context(|| format!("stat {label} {}", path.display()))?;
    if !metadata.is_file() {
        bail!("{label} is not a regular file: {}", path.display());
    }
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        bail!(
            "{label} is {} bytes; maximum is {MAX_INPUT_BYTES}",
            metadata.len()
        );
    }
    fs::read(path).with_context(|| format!("read {label} {}", path.display()))
}
