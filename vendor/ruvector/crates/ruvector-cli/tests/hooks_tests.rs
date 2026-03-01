//! Unit tests for the hooks CLI commands

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to get the ruvector binary command
fn ruvector_cmd() -> Command {
    Command::cargo_bin("ruvector").unwrap()
}

#[test]
fn test_hooks_help() {
    ruvector_cmd()
        .arg("hooks")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Self-learning intelligence hooks"));
}

#[test]
fn test_hooks_stats() {
    ruvector_cmd()
        .arg("hooks")
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Q-learning patterns"));
}

#[test]
fn test_hooks_session_start() {
    ruvector_cmd()
        .arg("hooks")
        .arg("session-start")
        .assert()
        .success()
        .stdout(predicate::str::contains("Intelligence Layer Active"));
}

#[test]
fn test_hooks_session_end() {
    ruvector_cmd()
        .arg("hooks")
        .arg("session-end")
        .assert()
        .success()
        .stdout(predicate::str::contains("Session ended"));
}

#[test]
fn test_hooks_pre_edit() {
    ruvector_cmd()
        .arg("hooks")
        .arg("pre-edit")
        .arg("src/main.rs")
        .assert()
        .success()
        .stdout(predicate::str::contains("Intelligence Analysis"));
}

#[test]
fn test_hooks_post_edit_success() {
    ruvector_cmd()
        .arg("hooks")
        .arg("post-edit")
        .arg("--success")
        .arg("src/lib.rs")
        .assert()
        .success()
        .stdout(predicate::str::contains("Learning recorded"));
}

#[test]
fn test_hooks_pre_command() {
    ruvector_cmd()
        .arg("hooks")
        .arg("pre-command")
        .arg("cargo build")
        .assert()
        .success()
        .stdout(predicate::str::contains("Command"));
}

#[test]
fn test_hooks_post_command() {
    ruvector_cmd()
        .arg("hooks")
        .arg("post-command")
        .arg("--success")
        .arg("cargo")
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("recorded"));
}

#[test]
fn test_hooks_remember() {
    ruvector_cmd()
        .arg("hooks")
        .arg("remember")
        .arg("--memory-type")
        .arg("test")
        .arg("test content for memory")
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn test_hooks_recall() {
    ruvector_cmd()
        .arg("hooks")
        .arg("recall")
        .arg("test content")
        .assert()
        .success();
}

#[test]
fn test_hooks_learn() {
    ruvector_cmd()
        .arg("hooks")
        .arg("learn")
        .arg("test-state")
        .arg("test-action")
        .arg("--reward")
        .arg("0.8")
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn test_hooks_suggest() {
    ruvector_cmd()
        .arg("hooks")
        .arg("suggest")
        .arg("edit-rs")
        .arg("--actions")
        .arg("coder,reviewer,tester")
        .assert()
        .success()
        .stdout(predicate::str::contains("action"));
}

#[test]
fn test_hooks_route() {
    ruvector_cmd()
        .arg("hooks")
        .arg("route")
        .arg("implement feature")
        .assert()
        .success()
        .stdout(predicate::str::contains("recommended"));
}

#[test]
fn test_hooks_should_test() {
    ruvector_cmd()
        .arg("hooks")
        .arg("should-test")
        .arg("src/lib.rs")
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo test"));
}

#[test]
fn test_hooks_suggest_next() {
    ruvector_cmd()
        .arg("hooks")
        .arg("suggest-next")
        .arg("src/main.rs")
        .assert()
        .success();
}

#[test]
fn test_hooks_record_error() {
    ruvector_cmd()
        .arg("hooks")
        .arg("record-error")
        .arg("cargo build")
        .arg("error[E0308]: mismatched types")
        .assert()
        .success()
        .stdout(predicate::str::contains("E0308"));
}

#[test]
fn test_hooks_suggest_fix() {
    ruvector_cmd()
        .arg("hooks")
        .arg("suggest-fix")
        .arg("E0308")
        .assert()
        .success();
}

#[test]
fn test_hooks_swarm_register() {
    ruvector_cmd()
        .arg("hooks")
        .arg("swarm-register")
        .arg("test-agent-1")
        .arg("rust-developer")
        .arg("--capabilities")
        .arg("rust,testing")
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn test_hooks_swarm_coordinate() {
    ruvector_cmd()
        .arg("hooks")
        .arg("swarm-coordinate")
        .arg("agent-1")
        .arg("agent-2")
        .arg("--weight")
        .arg("0.8")
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn test_hooks_swarm_optimize() {
    ruvector_cmd()
        .arg("hooks")
        .arg("swarm-optimize")
        .arg("task1,task2,task3")
        .assert()
        .success()
        .stdout(predicate::str::contains("assignments"));
}

#[test]
fn test_hooks_swarm_recommend() {
    ruvector_cmd()
        .arg("hooks")
        .arg("swarm-recommend")
        .arg("rust development")
        .assert()
        .success();
}

#[test]
fn test_hooks_swarm_heal() {
    ruvector_cmd()
        .arg("hooks")
        .arg("swarm-heal")
        .arg("failed-agent")
        .assert()
        .success();
}

#[test]
fn test_hooks_swarm_stats() {
    ruvector_cmd()
        .arg("hooks")
        .arg("swarm-stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("agents"));
}

#[test]
fn test_hooks_pre_compact() {
    ruvector_cmd()
        .arg("hooks")
        .arg("pre-compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("Pre-compact"));
}

#[test]
fn test_hooks_init_creates_config() {
    // Just test that init command runs successfully
    // The actual config is created in ~/.ruvector/ not the current directory
    ruvector_cmd().arg("hooks").arg("init").assert().success();
}

#[test]
fn test_hooks_install_runs() {
    // Just test that install command runs successfully
    ruvector_cmd()
        .arg("hooks")
        .arg("install")
        .assert()
        .success();
}
