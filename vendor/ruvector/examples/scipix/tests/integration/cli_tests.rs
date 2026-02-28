// CLI integration tests
//
// Tests command-line interface functionality

use super::*;
use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Stdio;

#[test]
fn test_cli_ocr_command_with_file() {
    // Create test image
    let image = images::generate_simple_equation("x + 1");
    image.save("/tmp/cli_test.png").unwrap();

    // Run CLI command
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/tmp/cli_test.png")
        .assert()
        .success()
        .stdout(predicate::str::contains("x"))
        .stdout(predicate::str::contains("LaTeX:"));
}

#[test]
fn test_cli_ocr_with_output_format() {
    let image = images::generate_fraction(3, 4);
    image.save("/tmp/cli_fraction.png").unwrap();

    // Test LaTeX output
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/tmp/cli_fraction.png")
        .arg("--format")
        .arg("latex")
        .assert()
        .success()
        .stdout(predicate::str::contains(r"\frac"));

    // Test MathML output
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/tmp/cli_fraction.png")
        .arg("--format")
        .arg("mathml")
        .assert()
        .success()
        .stdout(predicate::str::contains("<mfrac>"));
}

#[test]
fn test_cli_batch_command() {
    // Create test directory with images
    std::fs::create_dir_all("/tmp/cli_batch").unwrap();

    let equations = vec!["a + b", "x - y", "2 * 3"];
    for (i, eq) in equations.iter().enumerate() {
        let image = images::generate_simple_equation(eq);
        image.save(&format!("/tmp/cli_batch/eq_{}.png", i)).unwrap();
    }

    // Run batch command
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("batch")
        .arg("/tmp/cli_batch")
        .arg("--output")
        .arg("/tmp/cli_batch_results.json")
        .assert()
        .success();

    // Verify output file
    let results = std::fs::read_to_string("/tmp/cli_batch_results.json").unwrap();
    assert!(results.contains("a"), "Should contain results");
    assert!(results.len() > 100, "Should have substantial output");
}

#[test]
#[ignore] // Requires built binary and available port
fn test_cli_serve_command_startup() {
    // This test requires the binary to be built first
    // Use std::process::Command for spawn functionality
    use std::process::Command as StdCommand;

    // Get the binary path from environment, or fall back to cargo build path
    let bin_path = std::env::var("CARGO_BIN_EXE_scipix-ocr")
        .unwrap_or_else(|_| "target/debug/scipix-ocr".to_string());

    let mut child = StdCommand::new(&bin_path)
        .arg("serve")
        .arg("--port")
        .arg("18080")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    // Wait for server startup
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Check if server is running
    let client = reqwest::blocking::Client::new();
    let response = client
        .get("http://localhost:18080/health")
        .timeout(std::time::Duration::from_secs(5))
        .send();

    // Kill server
    let _ = child.kill();

    assert!(response.is_ok(), "Server should respond to health check");
}

#[test]
fn test_cli_config_command() {
    // Test config show
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("config").arg("show").assert().success().stdout(
        predicate::str::contains("model_path").or(predicate::str::contains("Configuration")),
    );

    // Test config set
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("config")
        .arg("set")
        .arg("preprocessing.enable_deskew")
        .arg("true")
        .assert()
        .success();
}

#[test]
fn test_cli_invalid_file() {
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/nonexistent/file.png")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}

#[test]
fn test_cli_exit_codes() {
    // Success case
    let image = images::generate_simple_equation("ok");
    image.save("/tmp/exit_code_test.png").unwrap();

    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/tmp/exit_code_test.png")
        .assert()
        .code(0);

    // Failure case
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/nonexistent.png")
        .assert()
        .code(predicate::ne(0));
}

#[test]
fn test_cli_verbose_output() {
    let image = images::generate_simple_equation("verbose");
    image.save("/tmp/verbose_test.png").unwrap();

    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("/tmp/verbose_test.png")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("Processing").or(predicate::str::contains("Confidence")));
}

#[test]
fn test_cli_json_output() {
    let image = images::generate_simple_equation("json");
    image.save("/tmp/json_test.png").unwrap();

    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    let output = cmd
        .arg("ocr")
        .arg("/tmp/json_test.png")
        .arg("--output-format")
        .arg("json")
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify JSON structure
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert!(json.get("latex").is_some(), "Should have latex field");
    assert!(
        json.get("confidence").is_some(),
        "Should have confidence field"
    );
}

#[test]
fn test_cli_help_command() {
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("USAGE:"))
        .stdout(predicate::str::contains("COMMANDS:"));

    // Test subcommand help
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("ocr")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("OPTIONS:"));
}

#[test]
fn test_cli_version_command() {
    let mut cmd = Command::cargo_bin("scipix-ocr").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
