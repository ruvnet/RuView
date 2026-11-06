//! Model release gate CLI — ADR-298, wired to a real artifact (issue #1521).
//!
//! ADR-298 defines the release gates ([`wifi_densepose_train::model_gates`])
//! and the ADR-298 workflow runs their unit tests, but nothing could point
//! those gates at a *file*: there was no entry point that reads a published
//! artifact and evaluates it. So a degenerate head could still ship — which is
//! exactly what issue #1521 reports for the published presence head (bias
//! 8.19 against ‖w‖ 3.67, so the logit can never cross zero on L2-normalized
//! embeddings).
//!
//! This binary is that entry point. It reads a published artifact and applies
//! the checks that an artifact can be judged by:
//!
//! * **Container format** — the safetensors header must be padded with `0x20`
//!   space bytes. NUL padding is rejected by the reference deserializer
//!   (issue #1522).
//! * **Head gates** (ADR-298) — unreachable decision boundary, constant output,
//!   degenerate class balance, evaluated on the L2-normalized probe sweep.
//!
//! The report-level gates (missing baseline, metric-name provenance) judge a
//! *reported metric*, not an artifact file, so they are out of scope here and
//! the output says so rather than passing silently.
//!
//! ```text
//! cargo run -p wifi-densepose-train --bin model_release_gate --no-default-features -- \
//!     --artifact dist/models/model.safetensors --l2-normalized-input
//! ```
//!
//! # Exit codes
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0 | every applicable check passed |
//! | 1 | the artifact was evaluated and rejected (format violation or a failed gate) |
//! | 2 | the artifact could not be evaluated (unreadable, malformed, or a missing precondition) |
//!
//! Exit code 2 is deliberately distinct from 1: "we could not tell" must not
//! read as "it is fine".

use std::env;
use std::path::Path;
use std::process::ExitCode;

use wifi_densepose_train::model_artifact::{presence_head_from_json, FormatVerdict, SafeTensorsFile};
use wifi_densepose_train::model_gates::evaluate_head_gates;

/// Default weight tensor name — the name the export script writes.
const DEFAULT_WEIGHT: &str = "presence_head.weights";
/// Default bias tensor name — the name the export script writes.
const DEFAULT_BIAS: &str = "presence_head.bias";

/// The head-gate outcome for one artifact.
enum HeadGate {
    /// The artifact carries no head, so the head gates do not apply to it.
    NotApplicable,
    /// `--format-only` was given, so the head gates were not run.
    Skipped,
    /// The head gates ran and rejected the head.
    Failed(Vec<String>),
    /// The head gates ran and the head passed them.
    Passed,
}

fn arg_val<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn usage() -> String {
    "usage: model_release_gate --artifact <PATH> [--weight <NAME>] [--bias <NAME>] \
     [--format-only] [--l2-normalized-input] [--json]"
        .to_string()
}

/// Exit code 2 — the artifact could not be evaluated.
fn cannot_evaluate(reason: &str, json: bool) -> ExitCode {
    if json {
        let payload = serde_json::json!({
            "verdict": "not_evaluated",
            "error": reason,
        });
        println!("{payload}");
    } else {
        eprintln!("error: {reason}");
        eprintln!("{}", usage());
    }
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let json = args.iter().any(|a| a == "--json");
    let format_only = args.iter().any(|a| a == "--format-only");
    let l2_normalized_input = args.iter().any(|a| a == "--l2-normalized-input");
    let weight_name = arg_val(&args, "--weight").unwrap_or(DEFAULT_WEIGHT);
    let bias_name = arg_val(&args, "--bias").unwrap_or(DEFAULT_BIAS);

    let Some(artifact) = arg_val(&args, "--artifact") else {
        return cannot_evaluate("--artifact <PATH> is required", json);
    };
    let path = Path::new(artifact);

    // ── Read the artifact ────────────────────────────────────────────────────
    // A `.json` extension selects the presence-head sidecar; anything else is
    // read as a safetensors container.
    let is_sidecar = path.extension().and_then(|e| e.to_str()) == Some("json");

    let (container, head) = if is_sidecar {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => return cannot_evaluate(&format!("cannot read `{artifact}`: {e}"), json),
        };
        let head = match presence_head_from_json(&bytes) {
            Ok(h) => h,
            Err(e) => return cannot_evaluate(&format!("`{artifact}`: {e}"), json),
        };
        (None, Some(head))
    } else {
        let file = match SafeTensorsFile::open(path) {
            Ok(f) => f,
            Err(e) => return cannot_evaluate(&format!("`{artifact}`: {e}"), json),
        };
        let has_weight = file.tensor(weight_name).is_ok();
        let has_bias = file.tensor(bias_name).is_ok();
        if has_weight != has_bias {
            let missing = if has_weight { bias_name } else { weight_name };
            return cannot_evaluate(
                &format!(
                    "`{artifact}` carries only one half of the head: `{missing}` is missing, so the \
                     head cannot be evaluated"
                ),
                json,
            );
        }
        let head = if has_weight {
            match file.linear_head(weight_name, bias_name) {
                Ok(h) => Some(h),
                Err(e) => return cannot_evaluate(&format!("`{artifact}`: {e}"), json),
            }
        } else {
            None
        };
        (Some(file), head)
    };

    // ── Container format check ───────────────────────────────────────────────
    let format_violation = match container.as_ref().map(|f| f.format_verdict()) {
        Some(FormatVerdict::Violation { detail }) => Some(detail),
        _ => None,
    };

    // ── Head gates (ADR-298) ─────────────────────────────────────────────────
    let head_gate = if format_only {
        HeadGate::Skipped
    } else {
        match &head {
            None => HeadGate::NotApplicable,
            Some(head) => {
                if !l2_normalized_input {
                    return cannot_evaluate(
                        &format!(
                            "`{artifact}` carries a head, and the ADR-298 head gates are only \
                             defined for a head that consumes L2-normalized embeddings. Pass \
                             --l2-normalized-input to assert that the head's input is \
                             L2-normalized; without it the gates would produce a false verdict."
                        ),
                        json,
                    );
                }
                let report = evaluate_head_gates(head);
                if report.passed() {
                    HeadGate::Passed
                } else {
                    HeadGate::Failed(report.failures.iter().map(|f| f.to_string()).collect())
                }
            }
        }
    };

    // ── Verdict ──────────────────────────────────────────────────────────────
    let failed = format_violation.is_some()
        || matches!(head_gate, HeadGate::Failed(_));
    let verdict = if failed { "fail" } else { "pass" };

    if json {
        let mut payload = serde_json::Map::new();
        payload.insert("artifact".into(), artifact.into());
        payload.insert("verdict".into(), verdict.into());
        payload.insert(
            "container_format".into(),
            match &format_violation {
                None => serde_json::json!({ "status": "compliant" }),
                Some(detail) => serde_json::json!({ "status": "violation", "detail": detail }),
            },
        );
        payload.insert(
            "head".into(),
            match &head {
                None => serde_json::json!({
                    "applicable": false,
                    "reason": "the artifact carries no head, so the ADR-298 head gates do not apply",
                }),
                Some(head) => {
                    let mut head_payload = serde_json::Map::new();
                    head_payload.insert("applicable".into(), true.into());
                    head_payload.insert("dim".into(), head.dim().into());
                    head_payload.insert("weight_norm".into(), head.weight_norm().into());
                    head_payload.insert("bias".into(), head.bias().into());
                    head_payload.insert(
                        "min_logit_normalized".into(),
                        head.min_logit_normalized().into(),
                    );
                    head_payload.insert(
                        "max_logit_normalized".into(),
                        head.max_logit_normalized().into(),
                    );
                    head_payload.insert(
                        "l2_normalized_input".into(),
                        l2_normalized_input.into(),
                    );
                    head_payload.insert(
                        "gate".into(),
                        match &head_gate {
                            HeadGate::Passed => serde_json::json!({ "status": "pass" }),
                            HeadGate::Failed(failures) => {
                                serde_json::json!({ "status": "fail", "failures": failures })
                            }
                            HeadGate::NotApplicable => {
                                serde_json::json!({ "status": "not_applicable" })
                            }
                            HeadGate::Skipped => serde_json::json!({ "status": "skipped" }),
                        },
                    );
                    serde_json::Value::Object(head_payload)
                }
            },
        );
        payload.insert(
            "report_level_gates".into(),
            serde_json::json!({
                "status": "not_applicable",
                "reason": "the missing-baseline and metric-name-provenance gates judge a reported \
                           metric, not an artifact file; they are not evaluated here",
            }),
        );
        println!("{}", serde_json::Value::Object(payload));
    } else {
        println!("artifact: {artifact}");
        match &format_violation {
            None => println!("container format: PASS"),
            Some(detail) => println!("container format: FAIL — {detail}"),
        }
        match &head {
            None => println!("head gate: NOT APPLICABLE — the artifact carries no head"),
            Some(head) => {
                println!(
                    "head: dim={} ‖w‖={:.6} bias={:.6} reachable logit=[{:.6}, {:.6}]",
                    head.dim(),
                    head.weight_norm(),
                    head.bias(),
                    head.min_logit_normalized(),
                    head.max_logit_normalized()
                );
                match &head_gate {
                    HeadGate::Passed => println!("head gate: PASS"),
                    HeadGate::Failed(failures) => {
                        println!("head gate: FAIL ({} issue(s))", failures.len());
                        for failure in failures {
                            println!("  - {failure}");
                        }
                    }
                    HeadGate::NotApplicable => {
                        println!("head gate: NOT APPLICABLE — the artifact carries no head")
                    }
                    HeadGate::Skipped => println!("head gate: SKIPPED (--format-only)"),
                }
            }
        }
        if failed {
            eprintln!("VERDICT: FAIL — the artifact is not releasable as-is");
            return ExitCode::FAILURE;
        }
        println!("VERDICT: PASS");
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
