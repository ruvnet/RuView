//! Integration tests for [`wifi_densepose_train::proof`].
//!
//! The proof module verifies checkpoint directories and (in the full
//! implementation) runs a short deterministic training proof.  All tests here
//! use temporary directories and fixed inputs — no `rand`, no OS entropy.
//!
//! Tests that depend on functions not yet implemented (`run_proof`,
//! `generate_expected_hash`) are marked `#[ignore]` so they compile and
//! document the expected API without failing CI until the implementation lands.
//!
//! This entire module is gated behind `tch-backend` because the `proof`
//! module is only compiled when that feature is enabled.

#[cfg(feature = "tch-backend")]
mod tch_proof_tests {

use tempfile::TempDir;
use wifi_densepose_train::proof;

// ---------------------------------------------------------------------------
// verify_checkpoint_dir
// ---------------------------------------------------------------------------

/// `verify_checkpoint_dir` must return `true` for an existing directory.
#[test]
fn verify_checkpoint_dir_returns_true_for_existing_dir() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let result = proof::verify_checkpoint_dir(tmp.path());
    assert!(
        result,
        "verify_checkpoint_dir must return true for an existing directory: {:?}",
        tmp.path()
    );
}

/// `verify_checkpoint_dir` must return `false` for a non-existent path.
#[test]
fn verify_checkpoint_dir_returns_false_for_nonexistent_path() {
    let nonexistent = std::path::Path::new(
        "/tmp/wifi_densepose_proof_test_no_such_dir_at_all",
    );
    assert!(
        !nonexistent.exists(),
        "test precondition: path must not exist before test"
    );

    let result = proof::verify_checkpoint_dir(nonexistent);
    assert!(
        !result,
        "verify_checkpoint_dir must return false for a non-existent path"
    );
}

/// `verify_checkpoint_dir` must return `false` for a path pointing to a file
/// (not a directory).
#[test]
fn verify_checkpoint_dir_returns_false_for_file() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let file_path = tmp.path().join("not_a_dir.txt");
    std::fs::write(&file_path, b"test file content").expect("file must be writable");

    let result = proof::verify_checkpoint_dir(&file_path);
    assert!(
        !result,
        "verify_checkpoint_dir must return false for a file, got true for {:?}",
        file_path
    );
}

/// `verify_checkpoint_dir` called twice on the same directory must return the
/// same result (deterministic, no side effects).
#[test]
fn verify_checkpoint_dir_is_idempotent() {
    let tmp = TempDir::new().expect("TempDir must be created");

    let first = proof::verify_checkpoint_dir(tmp.path());
    let second = proof::verify_checkpoint_dir(tmp.path());

    assert_eq!(
        first, second,
        "verify_checkpoint_dir must return the same result on repeated calls"
    );
}

/// A newly created sub-directory inside the temp root must also return `true`.
#[test]
fn verify_checkpoint_dir_works_for_nested_directory() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let nested = tmp.path().join("checkpoints").join("epoch_01");
    std::fs::create_dir_all(&nested).expect("nested dir must be created");

    let result = proof::verify_checkpoint_dir(&nested);
    assert!(
        result,
        "verify_checkpoint_dir must return true for a valid nested directory: {:?}",
        nested
    );
}

// ---------------------------------------------------------------------------
// Future API: run_proof
// ---------------------------------------------------------------------------
// The tests below document the intended proof API and will be un-ignored once
// `wifi_densepose_train::proof::run_proof` is implemented.

/// Proof must run without panicking and report that loss decreased.
///
/// This test is `#[ignore]`d until `run_proof` is implemented.
#[test]
#[ignore = "run_proof not yet implemented — remove #[ignore] when the function lands"]
fn proof_runs_without_panic() {
    // When implemented, proof::run_proof(dir) should return a struct whose
    // `loss_decreased` field is true, demonstrating that the training proof
    // converges on the synthetic dataset.
    //
    // Expected signature:
    //   pub fn run_proof(dir: &Path) -> anyhow::Result<ProofResult>
    //
    // Where ProofResult has:
    //   .loss_decreased: bool
    //   .initial_loss: f32
    //   .final_loss: f32
    //   .steps_completed: usize
    //   .model_hash: String
    //   .hash_matches: Option<bool>
    let _tmp = TempDir::new().expect("TempDir must be created");
    // Uncomment when run_proof is available:
    // let result = proof::run_proof(_tmp.path()).unwrap();
    // assert!(result.loss_decreased,
    //     "proof must show loss decreased: initial={}, final={}",
    //     result.initial_loss, result.final_loss);
}

/// Two proof runs with the same parameters must produce identical results.
///
/// This test is `#[ignore]`d until `run_proof` is implemented.
#[test]
#[ignore = "run_proof not yet implemented — remove #[ignore] when the function lands"]
fn proof_is_deterministic() {
    // When implemented, two independent calls to proof::run_proof must:
    //   - produce the same model_hash
    //   - produce the same final_loss (bit-identical or within 1e-6)
    let _tmp1 = TempDir::new().expect("TempDir 1 must be created");
    let _tmp2 = TempDir::new().expect("TempDir 2 must be created");
    // Uncomment when run_proof is available:
    // let r1 = proof::run_proof(_tmp1.path()).unwrap();
    // let r2 = proof::run_proof(_tmp2.path()).unwrap();
    // assert_eq!(r1.model_hash, r2.model_hash, "model hashes must match");
    // assert_eq!(r1.final_loss, r2.final_loss, "final losses must match");
}

/// Hash generation and verification must roundtrip.
///
/// This test is `#[ignore]`d until `generate_expected_hash` is implemented.
#[test]
#[ignore = "generate_expected_hash not yet implemented — remove #[ignore] when the function lands"]
fn hash_generation_and_verification_roundtrip() {
    // When implemented:
    //   1. generate_expected_hash(dir) stores a reference hash file in dir
    //   2. run_proof(dir) loads the reference file and sets hash_matches = Some(true)
    //      when the model hash matches
    let _tmp = TempDir::new().expect("TempDir must be created");
    // Uncomment when both functions are available:
    // let hash = proof::generate_expected_hash(_tmp.path()).unwrap();
    // let result = proof::run_proof(_tmp.path()).unwrap();
    // assert_eq!(result.hash_matches, Some(true));
    // assert_eq!(result.model_hash, hash);
}

// ---------------------------------------------------------------------------
// Filesystem helpers (deterministic, no randomness)
// ---------------------------------------------------------------------------

/// Creating and verifying a checkpoint directory within a temp tree must
/// succeed without errors.
#[test]
fn checkpoint_dir_creation_and_verification_workflow() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let checkpoint_dir = tmp.path().join("model_checkpoints");

    // Directory does not exist yet.
    assert!(
        !proof::verify_checkpoint_dir(&checkpoint_dir),
        "must return false before the directory is created"
    );

    // Create the directory.
    std::fs::create_dir_all(&checkpoint_dir).expect("checkpoint dir must be created");

    // Now it should be valid.
    assert!(
        proof::verify_checkpoint_dir(&checkpoint_dir),
        "must return true after the directory is created"
    );
}

/// Multiple sibling checkpoint directories must each independently return the
/// correct result.
#[test]
fn multiple_checkpoint_dirs_are_independent() {
    let tmp = TempDir::new().expect("TempDir must be created");

    let dir_a = tmp.path().join("epoch_01");
    let dir_b = tmp.path().join("epoch_02");
    let dir_missing = tmp.path().join("epoch_99");

    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    // dir_missing is intentionally not created.

    assert!(
        proof::verify_checkpoint_dir(&dir_a),
        "dir_a must be valid"
    );
    assert!(
        proof::verify_checkpoint_dir(&dir_b),
        "dir_b must be valid"
    );
    assert!(
        !proof::verify_checkpoint_dir(&dir_missing),
        "dir_missing must be invalid"
    );
}

} // mod tch_proof_tests
