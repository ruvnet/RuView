//! End-to-end tests for the ADR-298 model release gate CLI (issue #1521).
//!
//! The fixtures are generated at run time rather than committed as binary
//! blobs: the degenerate head of issue #1521 is reproduced exactly from the
//! numbers the issue publishes (‖w‖ ≈ 3.67, bias ≈ 8.19), and the healthy head
//! is the same shape with a reachable boundary. Nothing here needs a network,
//! credentials, or a downloaded model.
//!
//! What is exercised here is the *entry point* the ADR-298 workflow asked for
//! ("until a CLI entry point exists to run `evaluate_linear_head` against an
//! arbitrary `.safetensors`/`.rvf` file"): the CLI reads a real file, applies
//! the gates, and reports a verdict through the exit code.

use std::path::Path;
use std::process::{Command, Output};

use wifi_densepose_train::model_artifact::build_safetensors;

/// The exit code the CLI uses for "could not be evaluated".
const EXIT_NOT_EVALUATED: i32 = 2;

/// The degenerate presence head of issue #1521: four equal components give
/// ‖w‖ = 3.67, and the bias of 8.19 keeps the logit positive for every
/// unit-norm embedding.
fn issue_1521_tensors() -> Vec<(&'static str, Vec<f32>)> {
    let component = 3.67f32 / 2.0;
    vec![
        ("presence_head.weights", vec![component; 4]),
        ("presence_head.bias", vec![8.19]),
    ]
}

/// A healthy head: ‖w‖ = 4, bias = 0, so the logit sweeps [-4, 4].
fn healthy_tensors() -> Vec<(&'static str, Vec<f32>)> {
    vec![
        ("presence_head.weights", vec![2.0; 4]),
        ("presence_head.bias", vec![0.0]),
    ]
}

fn write_container(dir: &Path, name: &str, tensors: &[(&str, &[f32])], pad_byte: u8) -> std::path::PathBuf {
    let path = dir.join(name);
    let bytes = build_safetensors(tensors, pad_byte);
    std::fs::write(&path, bytes).expect("fixture is written");
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_model_release_gate"))
        .args(args)
        .output()
        .expect("the gate binary runs")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn rejects_the_degenerate_issue_1521_head() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tensors = issue_1521_tensors();
    let path = write_container(
        dir.path(),
        "model.safetensors",
        &tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect::<Vec<_>>(),
        0x20,
    );

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--l2-normalized-input",
    ]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    // The details go to stdout; the verdict line goes to stderr, following the
    // convention the crate's other gate binaries use.
    assert!(stderr_of(&output).contains("VERDICT: FAIL"), "{stdout}");
    // The failure must name the defect and carry the offending numbers.
    assert!(
        stdout.contains("unreachable decision boundary"),
        "{stdout}"
    );
    assert!(stdout.contains("head gate: FAIL"), "{stdout}");
}

#[test]
fn accepts_a_healthy_head() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tensors = healthy_tensors();
    let path = write_container(
        dir.path(),
        "model.safetensors",
        &tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect::<Vec<_>>(),
        0x20,
    );

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--l2-normalized-input",
    ]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert!(stdout.contains("VERDICT: PASS"), "{stdout}");
    assert!(stdout.contains("head gate: PASS"), "{stdout}");
}

#[test]
fn rejects_a_container_with_nul_padded_headers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tensors = healthy_tensors();
    let path = write_container(
        dir.path(),
        "model.safetensors",
        &tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect::<Vec<_>>(),
        0x00,
    );

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--l2-normalized-input",
    ]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert!(stdout.contains("container format: FAIL"), "{stdout}");
    assert!(stdout.contains("0x00"), "{stdout}");
}

#[test]
fn refuses_to_evaluate_a_head_without_the_normalization_assertion() {
    // The ADR-298 head gates are only defined for a head that consumes
    // L2-normalized embeddings. Without the assertion the CLI must refuse to
    // produce a verdict — it must not quietly pass.
    let dir = tempfile::tempdir().expect("temp dir");
    let tensors = healthy_tensors();
    let path = write_container(
        dir.path(),
        "model.safetensors",
        &tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect::<Vec<_>>(),
        0x20,
    );

    let output = run(&["--artifact", path.to_str().expect("path")]);

    assert_eq!(
        output.status.code(),
        Some(EXIT_NOT_EVALUATED),
        "stderr: {}",
        stderr_of(&output)
    );
    let stderr = stderr_of(&output);
    assert!(stderr.contains("--l2-normalized-input"), "{stderr}");
}

#[test]
fn reports_an_unreadable_artifact_as_not_evaluated() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("does-not-exist.safetensors");

    let output = run(&[
        "--artifact",
        missing.to_str().expect("path"),
        "--l2-normalized-input",
    ]);

    assert_eq!(
        output.status.code(),
        Some(EXIT_NOT_EVALUATED),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stderr_of(&output).contains("cannot read"), "{}", stderr_of(&output));
}

#[test]
fn reads_the_presence_head_sidecar() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("presence-head.json");
    // The published sidecar shape: {"weights": [...], "bias": <number>}.
    std::fs::write(&path, br#"{"weights":[1.835,1.835,1.835,1.835],"bias":8.19}"#)
        .expect("fixture is written");

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--l2-normalized-input",
    ]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert!(stderr_of(&output).contains("VERDICT: FAIL"), "{stdout}");
    assert!(stdout.contains("unreachable decision boundary"), "{stdout}");
}

#[test]
fn a_container_without_a_head_is_reported_as_not_applicable() {
    // The gate must say the head gate does not apply — not silently pass.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_container(
        dir.path(),
        "encoder.safetensors",
        &[("encoder.layer", &[1.0f32, 2.0][..])],
        0x20,
    );

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--l2-normalized-input",
    ]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert!(stdout.contains("NOT APPLICABLE"), "{stdout}");
    assert!(stdout.contains("VERDICT: PASS"), "{stdout}");
}

#[test]
fn emits_a_machine_readable_verdict() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tensors = issue_1521_tensors();
    let path = write_container(
        dir.path(),
        "model.safetensors",
        &tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect::<Vec<_>>(),
        0x20,
    );

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--l2-normalized-input",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let payload: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout is one JSON document");
    assert_eq!(payload["verdict"], "fail");
    assert_eq!(payload["head"]["gate"]["status"], "fail");
    assert_eq!(payload["head"]["dim"], 4);
    let norm = payload["head"]["weight_norm"].as_f64().expect("weight norm");
    let bias = payload["head"]["bias"].as_f64().expect("bias");
    assert!((norm - 3.67).abs() < 1e-3, "weight norm {norm}");
    assert!((bias - 8.19).abs() < 1e-3, "bias {bias}");
    // The report-level gates must be declared as out of scope, not skipped
    // silently.
    assert_eq!(payload["report_level_gates"]["status"], "not_applicable");
}

#[test]
fn format_only_skips_the_head_gate() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tensors = issue_1521_tensors();
    let path = write_container(
        dir.path(),
        "model.safetensors",
        &tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect::<Vec<_>>(),
        0x20,
    );

    let output = run(&[
        "--artifact",
        path.to_str().expect("path"),
        "--format-only",
    ]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert!(stdout.contains("head gate: SKIPPED"), "{stdout}");
    assert!(stdout.contains("container format: PASS"), "{stdout}");
}
