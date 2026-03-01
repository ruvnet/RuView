//! Integration tests for Ruvector CLI

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ruvector"));
}

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains(
        "High-performance Rust vector database",
    ));
}

#[test]
fn test_create_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("create")
        .arg("--path")
        .arg(db_path.to_str().unwrap())
        .arg("--dimensions")
        .arg("128");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Database created successfully"));

    // Verify database file exists
    assert!(db_path.exists());
}

#[test]
fn test_info_command() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Create database first
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("create")
        .arg("--path")
        .arg(db_path.to_str().unwrap())
        .arg("--dimensions")
        .arg("64");
    cmd.assert().success();

    // Check info
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("info").arg("--db").arg(db_path.to_str().unwrap());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Database Statistics"))
        .stdout(predicate::str::contains("Dimensions: 64"));
}

#[test]
fn test_insert_from_json() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let json_path = dir.path().join("vectors.json");

    // Create test JSON file
    let test_data = r#"[
        {
            "id": "v1",
            "vector": [1.0, 2.0, 3.0],
            "metadata": {"label": "test1"}
        },
        {
            "id": "v2",
            "vector": [4.0, 5.0, 6.0],
            "metadata": {"label": "test2"}
        }
    ]"#;
    fs::write(&json_path, test_data).unwrap();

    // Create database
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("create")
        .arg("--path")
        .arg(db_path.to_str().unwrap())
        .arg("--dimensions")
        .arg("3");
    cmd.assert().success();

    // Insert vectors
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("insert")
        .arg("--db")
        .arg(db_path.to_str().unwrap())
        .arg("--input")
        .arg(json_path.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .arg("--no-progress");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Inserted 2 vectors"));
}

#[test]
fn test_search_command() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let json_path = dir.path().join("vectors.json");

    // Create test data
    let test_data = r#"[
        {"id": "v1", "vector": [1.0, 0.0, 0.0]},
        {"id": "v2", "vector": [0.0, 1.0, 0.0]},
        {"id": "v3", "vector": [0.0, 0.0, 1.0]}
    ]"#;
    fs::write(&json_path, test_data).unwrap();

    // Create and populate database
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("create")
        .arg("--path")
        .arg(db_path.to_str().unwrap())
        .arg("--dimensions")
        .arg("3");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("insert")
        .arg("--db")
        .arg(db_path.to_str().unwrap())
        .arg("--input")
        .arg(json_path.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .arg("--no-progress");
    cmd.assert().success();

    // Search
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("search")
        .arg("--db")
        .arg(db_path.to_str().unwrap())
        .arg("--query")
        .arg("[1.0, 0.0, 0.0]")
        .arg("--top-k")
        .arg("2");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("v1"));
}

#[test]
fn test_benchmark_command() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Create database
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("create")
        .arg("--path")
        .arg(db_path.to_str().unwrap())
        .arg("--dimensions")
        .arg("128");
    cmd.assert().success();

    // Run benchmark
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("benchmark")
        .arg("--db")
        .arg(db_path.to_str().unwrap())
        .arg("--queries")
        .arg("100");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Benchmark Results"))
        .stdout(predicate::str::contains("Queries per second"));
}

#[test]
fn test_error_handling() {
    // Test with invalid database path - /dev/null is a device file, not a directory,
    // so we cannot create a database file inside it. This guarantees failure
    // regardless of user permissions.
    let mut cmd = Command::cargo_bin("ruvector").unwrap();
    cmd.arg("info").arg("--db").arg("/dev/null/db.db");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}
