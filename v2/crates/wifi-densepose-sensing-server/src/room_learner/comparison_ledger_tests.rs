//! Explicitly SYNTHETIC RF through the real evaluator/store; no live sensor data.
use super::*;
use crate::room_learner::training::{learning_tests, FitConfig};
use std::sync::OnceLock;
const AT: u64 = 1_800_000_000_000;
const NS: u64 = 1_800_000_000_000_000_001;
fn dataset() -> ValidatedDataset {
    let base = learning_tests::dataset(8);
    let mut context = base.context().clone();
    context.owner_scope = "a".repeat(64);
    let mut rows = base
        .train()
        .iter()
        .chain(base.validation())
        .chain(base.held_out())
        .cloned()
        .collect::<Vec<_>>();
    for row in &mut rows {
        row.context_digest = context.digest();
        for view in &mut row.views {
            view.first_time_ns += NS;
            view.last_time_ns += NS;
        }
    }
    ValidatedDataset::new(context, rows).unwrap()
}
fn model() -> RoomModel {
    static MODEL: OnceLock<RoomModel> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            RoomModel::fit(
                &dataset(),
                FitConfig {
                    epochs: 2,
                    ..FitConfig::default()
                },
            )
            .unwrap()
            .0
        })
        .clone()
}
fn open_store(root: &Path) -> ArtifactStore {
    ArtifactStore::new(
        &root.join("a".repeat(64)).join("SensingServer"),
        &"a".repeat(64),
        "synthetic-install",
    )
    .unwrap()
}
fn saved(root: &Path) -> (ArtifactStore, RoomModel) {
    let store = open_store(root);
    let model = model();
    store.save_candidate(&model).unwrap();
    (store, model)
}
fn ledger_path(store: &ArtifactStore, model: &RoomModel, receipt: &EvaluationReceipt) -> PathBuf {
    store
        .comparison_directory(model, receipt.role, false)
        .unwrap()
        .unwrap()
        .join(format!("{}.json", receipt.sha256().unwrap()))
}
fn recorded(
    root: &Path,
) -> (
    ArtifactStore,
    RoomModel,
    EvaluationReceipt,
    ComparisonLedgerSummary,
) {
    let (store, model) = saved(root);
    let ds = dataset();
    let (receipt, predictions) = store
        .evaluate_candidate_recorded_at(&model, &ds, DataRole::HeldOut, AT, true)
        .unwrap();
    let summary = store
        .save_comparison_ledger(&model, &ds, &receipt, predictions.unwrap().unwrap())
        .unwrap();
    (store, model, receipt, summary)
}
#[test]
fn default_evaluation_does_not_create_comparison_files_and_optin_keeps_original_receipt_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model) = saved(dir.path());
    let ds = dataset();
    let receipt = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    assert!(store
        .comparison_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .is_none());
    let eval_dir = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap();
    let before =
        private_read(&eval_dir.join(format!("{}.json", receipt.sha256().unwrap()))).unwrap();
    let pointer_before = private_read(&eval_dir.join(LATEST)).unwrap();
    let (same, ledger) = store
        .evaluate_candidate_with_comparison(&model, &ds, DataRole::HeldOut)
        .unwrap();
    assert!(ledger.is_ok());
    assert_eq!(same, receipt);
    assert_eq!(same.evaluated_at_unix_ms, AT);
    assert_eq!(
        before,
        private_read(&eval_dir.join(format!("{}.json", same.sha256().unwrap()))).unwrap()
    );
    assert_eq!(
        pointer_before,
        private_read(&eval_dir.join(LATEST)).unwrap()
    );
}
#[test]
fn recorded_single_pass_has_exactly_the_normal_report_and_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model) = saved(dir.path());
    let ds = dataset();
    let normal =
        EvaluationReceipt::new(&model, ds.held_out(), DataRole::HeldOut, AT, true).unwrap();
    let (recorded, outcomes) = store
        .evaluate_candidate_recorded_at(&model, &ds, DataRole::HeldOut, AT, true)
        .unwrap();
    assert_eq!(recorded, normal);
    assert_eq!(outcomes.unwrap().unwrap().len(), recorded.report.samples);
}
#[test]
fn cold_load_retains_ns_precision_counts_private_permissions_and_no_raw_samples() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model, receipt, summary) = recorded(dir.path());
    let content = read(&ledger_path(&store, &model, &receipt)).unwrap();
    let text = std::str::from_utf8(&content).unwrap();
    for forbidden in [
        "\"samples\":",
        "\"frame_ids\":",
        "\"sequence_ids\":",
        "native_iq",
        "amplitudes",
        "\"trial_id\":",
        "\"session_id\":",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    let envelope: Envelope = serde_json::from_slice(&content).unwrap();
    let first = &envelope.ledger.outcomes[0].sources[0].first_time_ns;
    assert!(decimal(first).unwrap() > 9_007_199_254_740_992);
    assert!(text.contains(&format!("\"{first}\"")));
    assert!(
        !summary.physically_validated && !summary.adoption_authorized && summary.comparison_only
    );
    assert_eq!(
        open_store(dir.path())
            .load_comparison_ledger_summary(&model, DataRole::HeldOut, &receipt.sha256().unwrap())
            .unwrap(),
        Some(summary)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(ledger_path(&store, &model, &receipt))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(ledger_path(&store, &model, &receipt).parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
#[test]
fn malformed_or_rehashed_wrong_counts_cannot_replace_a_valid_rf_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model, receipt, _) = recorded(dir.path());
    let path = ledger_path(&store, &model, &receipt);
    let mut envelope: Envelope = serde_json::from_slice(&read(&path).unwrap()).unwrap();
    envelope.ledger.outcomes[0].condition = if is_occupied(envelope.ledger.outcomes[0].condition) {
        RoomLabel::Empty
    } else {
        RoomLabel::OccupiedStatic
    };
    envelope.ledger_sha256 = digest(&bytes(&envelope.ledger).unwrap());
    fs::write(&path, bytes(&envelope).unwrap()).unwrap();
    assert!(store
        .load_comparison_ledger_summary(&model, receipt.role, &receipt.sha256().unwrap())
        .is_err());
    let (same, comparison) = store
        .evaluate_candidate_with_comparison(&model, &dataset(), receipt.role)
        .unwrap();
    assert!(comparison.is_err());
    assert_eq!(same, receipt);
    assert_eq!(
        store.load_latest_evaluation(&model, receipt.role).unwrap(),
        Some(receipt)
    );
}
#[test]
fn retained_rows_detect_rehashed_time_changes_even_when_aggregate_counts_match() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model, receipt, _) = recorded(dir.path());
    let path = ledger_path(&store, &model, &receipt);
    let mut envelope: Envelope = serde_json::from_slice(&read(&path).unwrap()).unwrap();
    let source = &mut envelope.ledger.outcomes[0].sources[0];
    source.first_time_ns = (decimal(&source.first_time_ns).unwrap() + 1).to_string();
    envelope.ledger_sha256 = digest(&bytes(&envelope.ledger).unwrap());
    fs::write(&path, bytes(&envelope).unwrap()).unwrap();
    let (same, comparison) = store
        .evaluate_candidate_with_comparison(&model, &dataset(), receipt.role)
        .unwrap();
    assert!(comparison.is_err());
    assert_eq!(same, receipt);
}
#[test]
fn comparison_directory_failure_does_not_revoke_evaluation_or_create_a_fake_summary() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model) = saved(dir.path());
    let ds = dataset();
    let receipt = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    let parent = store
        .evaluation_directory(&model, receipt.role, false)
        .unwrap()
        .unwrap();
    atomic_private_write(&parent.join(DIRECTORY), b"not a directory").unwrap();
    let (same, comparison) = store
        .evaluate_candidate_with_comparison(&model, &ds, receipt.role)
        .unwrap();
    assert!(comparison.is_err());
    assert_eq!(same, receipt);
    assert_eq!(
        store.load_latest_evaluation(&model, receipt.role).unwrap(),
        Some(receipt)
    );
}
#[cfg(unix)]
#[test]
fn symlink_and_oversize_sidecars_are_refused_without_touching_targets() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model, receipt, _) = recorded(dir.path());
    let path = ledger_path(&store, &model, &receipt);
    fs::remove_file(&path).unwrap();
    let outside = dir.path().join("untouched");
    fs::write(&outside, b"sentinel").unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(store
        .load_comparison_ledger_summary(&model, receipt.role, &receipt.sha256().unwrap())
        .is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"sentinel");
    fs::remove_file(&path).unwrap();
    atomic_private_write(&path, &vec![0; MAX_BYTES + 1]).unwrap();
    assert!(store
        .load_comparison_ledger_summary(&model, receipt.role, &receipt.sha256().unwrap())
        .is_err());
}
#[test]
fn wrong_role_model_owner_and_noncanonical_ns_are_not_admitted() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model, receipt, _) = recorded(dir.path());
    assert!(store
        .load_comparison_ledger_summary(&model, DataRole::Train, &receipt.sha256().unwrap())
        .is_err());
    let mut other_model = model.clone();
    other_model.context.owner_scope = "b".repeat(64);
    assert!(store
        .load_comparison_ledger_summary(&other_model, receipt.role, &receipt.sha256().unwrap())
        .is_err());
    let path = ledger_path(&store, &model, &receipt);
    let mut envelope: Envelope = serde_json::from_slice(&read(&path).unwrap()).unwrap();
    envelope.ledger.outcomes[0].sources[0]
        .first_time_ns
        .insert(0, '0');
    envelope.ledger_sha256 = digest(&bytes(&envelope.ledger).unwrap());
    fs::write(path, bytes(&envelope).unwrap()).unwrap();
    assert!(store
        .load_comparison_ledger_summary(&model, receipt.role, &receipt.sha256().unwrap())
        .is_err());
}
#[test]
fn comparison_recomputes_actual_abstention_and_missing_probability_counts() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model) = saved(dir.path());
    let original = dataset();
    let mut rows = original
        .train()
        .iter()
        .chain(original.validation())
        .chain(original.held_out())
        .cloned()
        .collect::<Vec<_>>();
    let held_out = rows
        .iter_mut()
        .find(|row| row.role == DataRole::HeldOut)
        .unwrap();
    held_out.views[0].samples[0][0] = 1_000_000.0;
    let ds = ValidatedDataset::new(original.context().clone(), rows).unwrap();
    let (receipt, comparison) = store
        .evaluate_candidate_with_comparison(&model, &ds, DataRole::HeldOut)
        .unwrap();
    assert!(receipt.report.abstained > 0 && receipt.report.missing > 0);
    let summary = comparison.unwrap();
    assert_eq!(summary.cohort_count, receipt.report.samples);
    let envelope: Envelope =
        serde_json::from_slice(&read(&ledger_path(&store, &model, &receipt)).unwrap()).unwrap();
    assert!(envelope
        .ledger
        .outcomes
        .iter()
        .any(|outcome| outcome.decision == PresenceDecision::Abstain
            && outcome.occupied_probability.is_none()));
}
#[test]
fn managed_lineage_is_genuinely_resolved_and_not_invented_for_diagnostic_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model, receipt, _) = recorded(dir.path());
    let path = ledger_path(&store, &model, &receipt);
    let mut envelope: Envelope = serde_json::from_slice(&read(&path).unwrap()).unwrap();
    assert!(envelope.ledger.managed_training_proof_sha256.is_none());
    assert!(envelope.ledger.sealed_phase_fence_sha256.is_none());
    envelope.ledger.managed_training_proof_sha256 = Some("1".repeat(64));
    envelope.ledger.sealed_phase_fence_sha256 = Some("2".repeat(64));
    envelope.ledger_sha256 = digest(&bytes(&envelope.ledger).unwrap());
    fs::write(path, bytes(&envelope).unwrap()).unwrap();
    assert!(store
        .load_comparison_ledger_summary(&model, receipt.role, &receipt.sha256().unwrap())
        .is_err());
    assert_eq!(
        store.load_latest_evaluation(&model, receipt.role).unwrap(),
        Some(receipt)
    );
}

#[test]
fn valid_reversed_source_views_record_canonical_metadata_without_rewriting_rf_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let (store, model) = saved(dir.path());
    let original = dataset();
    let mut rows = original
        .train()
        .iter()
        .chain(original.validation())
        .chain(original.held_out())
        .cloned()
        .collect::<Vec<_>>();
    for row in rows.iter_mut().filter(|row| row.role == DataRole::HeldOut) {
        row.views.reverse();
    }
    let reversed = ValidatedDataset::new(original.context().clone(), rows).unwrap();
    let expected_dataset_sha = data_sha256(reversed.held_out()).unwrap();
    assert_ne!(
        expected_dataset_sha,
        data_sha256(original.held_out()).unwrap()
    );
    let expected_report = model.evaluate(reversed.held_out()).unwrap();
    let (receipt, comparison) = store
        .evaluate_candidate_with_comparison(&model, &reversed, DataRole::HeldOut)
        .unwrap();
    assert_eq!(receipt.report, expected_report);
    assert_eq!(receipt.dataset_sha256, expected_dataset_sha);
    let summary = comparison.expect("valid source view order must not reject comparison metadata");
    let envelope: Envelope =
        serde_json::from_slice(&read(&ledger_path(&store, &model, &receipt)).unwrap()).unwrap();
    for (recorded, row) in envelope.ledger.outcomes.iter().zip(reversed.held_out()) {
        assert_eq!(
            recorded.cohort_sha256,
            data_sha256(std::slice::from_ref(row)).unwrap()
        );
        assert_eq!(
            recorded
                .sources
                .iter()
                .map(|source| source.source_id)
                .collect::<Vec<_>>(),
            model
                .context()
                .sources
                .iter()
                .map(|source| source.source_id)
                .collect::<Vec<_>>()
        );
        for source in &recorded.sources {
            let original_view = row
                .views
                .iter()
                .find(|view| view.source_id == source.source_id)
                .unwrap();
            assert_eq!(
                source.first_time_ns,
                original_view.first_time_ns.to_string()
            );
            assert_eq!(
                source.frame_commitment_sha256,
                commitment(
                    "ruview.comparison.frames.v1",
                    &(&original_view.frame_ids, &original_view.sequence_ids)
                )
                .unwrap()
            );
        }
    }
    let cold = open_store(dir.path());
    let restored = cold.load_candidate(model.context()).unwrap().unwrap();
    assert_eq!(
        cold.load_comparison_ledger_summary(
            &restored,
            DataRole::HeldOut,
            &receipt.sha256().unwrap()
        )
        .unwrap(),
        Some(summary)
    );
}
