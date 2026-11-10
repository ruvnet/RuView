//! SYNTHETIC RF through real fitting, private storage and cold-load boundaries.
use super::*;
use crate::room_learner::{
    observation::CaptureCohort,
    training::{learning_tests, FitConfig},
};
use std::sync::OnceLock;
const AT: u64 = 1_800_000_000_000;
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
fn receipt_path(store: &ArtifactStore, model: &RoomModel, r: &EvaluationReceipt) -> PathBuf {
    store
        .evaluation_directory(model, r.role, false)
        .unwrap()
        .unwrap()
        .join(format!("{}.json", r.sha256().unwrap()))
}
#[test]
fn real_fit_evaluate_private_store_cold_load_preserves_bound_metadata_without_raw_rows() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    let before = model.export().unwrap();
    assert!(store
        .load_latest_evaluation(&model, DataRole::HeldOut)
        .unwrap()
        .is_none());
    let report = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    assert_eq!(model.export().unwrap(), before);
    assert_eq!(report.dataset_sha256, data_sha256(ds.held_out()).unwrap());
    assert_eq!(report.model_sha256, digest(&before));
    assert!(report.train_lineage_checked);
    assert!(
        !report.adoption_authorized
            && !report.physically_validated
            && !report.independence_verified
            && !report.measurement_clock_available
    );
    let cold = open_store(temp.path());
    let restored = cold.load_candidate(model.context()).unwrap().unwrap();
    assert_eq!(
        cold.load_latest_evaluation(&restored, DataRole::HeldOut)
            .unwrap(),
        Some(report.clone())
    );
    let text =
        String::from_utf8(private_read(&receipt_path(&cold, &restored, &report)).unwrap()).unwrap();
    for raw in [
        "frame_ids",
        "sequence_ids",
        "trial_id\"",
        "views\"",
        "samples\":[",
        "amplitudes",
    ] {
        assert!(!text.contains(raw), "raw field {raw}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(receipt_path(&cold, &restored, &report))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn repeated_receipt_identity_is_idempotent_and_new_receipts_retain_history() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    let first = store
        .evaluate_candidate_at(&model, &ds, DataRole::Validation, AT)
        .unwrap();
    let path = receipt_path(&store, &model, &first);
    let bytes = private_read(&path).unwrap();
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&path).unwrap().ino()
    };
    let repeat = store
        .evaluate_candidate_at(&model, &ds, DataRole::Validation, AT)
        .unwrap();
    assert_eq!(first, repeat);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    }
    let unchanged = store
        .evaluate_candidate_at(&model, &ds, DataRole::Validation, AT + 1)
        .unwrap();
    assert_eq!(
        unchanged, first,
        "identical evidence must not renew its timestamp"
    );
    let mut changed_rows = ds
        .train()
        .iter()
        .chain(ds.validation())
        .chain(ds.held_out())
        .cloned()
        .collect::<Vec<_>>();
    let changed = changed_rows
        .iter_mut()
        .find(|r| r.role == DataRole::Validation)
        .unwrap();
    changed.views[0].samples[0][0] += 0.01;
    let changed = ValidatedDataset::new(ds.context().clone(), changed_rows).unwrap();
    let next = store
        .evaluate_candidate_at(&model, &changed, DataRole::Validation, AT + 2)
        .unwrap();
    assert_ne!(first.sha256().unwrap(), next.sha256().unwrap());
    assert_eq!(private_read(&path).unwrap(), bytes);
    assert_eq!(
        store
            .load_latest_evaluation(&model, DataRole::Validation)
            .unwrap(),
        Some(next)
    );
}
#[test]
fn frozen_model_role_and_context_isolation_and_missing_retained_train_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    for role in [DataRole::Train, DataRole::Inference] {
        assert!(store.evaluate_candidate_at(&model, &ds, role, AT).is_err());
    }
    let mut rows = ds.train().to_vec();
    rows[0].views[0].samples[0][0] += 0.01;
    rows.extend_from_slice(ds.held_out());
    let changed = ValidatedDataset::new(ds.context().clone(), rows).unwrap();
    assert!(store
        .evaluate_candidate_at(&model, &changed, DataRole::HeldOut, AT)
        .unwrap_err()
        .contains("Train set"));
    let cold_rows = ValidatedDataset::new(ds.context().clone(), ds.held_out().to_vec()).unwrap();
    let result = store
        .evaluate_candidate_at(&model, &cold_rows, DataRole::HeldOut, AT)
        .unwrap();
    assert!(!result.train_lineage_checked);
    let second = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 3,
            ..FitConfig::default()
        },
    )
    .unwrap()
    .0;
    store.save_candidate(&second).unwrap();
    assert!(store
        .load_latest_evaluation(&second, DataRole::HeldOut)
        .unwrap()
        .is_none());
    assert_eq!(
        store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .unwrap(),
        Some(result)
    );
    let wrong = ArtifactStore::new(
        &temp.path().join("b".repeat(64)).join("SensingServer"),
        &"b".repeat(64),
        "synthetic-install",
    )
    .unwrap();
    assert!(wrong
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .is_err());
    assert!(wrong
        .load_latest_evaluation(&model, DataRole::HeldOut)
        .is_err());
    let unsaved = tempfile::tempdir().unwrap();
    assert!(open_store(unsaved.path())
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .is_err());
}
#[test]
fn role_leakage_and_repeat_cohort_ids_cannot_reach_receipt_generation() {
    let ds = dataset();
    let mut rows = ds.train().to_vec();
    let mut overlap = rows[0].clone();
    overlap.role = DataRole::HeldOut;
    overlap.id = "renamed-eval".into();
    overlap.trial_id = "renamed-trial".into();
    overlap.session_id = "renamed-session".into();
    rows.push(overlap);
    assert!(ValidatedDataset::new(ds.context().clone(), rows)
        .unwrap_err()
        .contains("frame overlap"));
    let duplicate: Vec<CaptureCohort> = vec![ds.held_out()[0].clone(), ds.held_out()[0].clone()];
    assert!(ValidatedDataset::new(ds.context().clone(), duplicate).is_err());
}
#[test]
fn corrupt_receipt_or_pointer_is_retained_without_discarding_the_model_or_overwriting_history() {
    for pointer in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let (store, model) = saved(temp.path());
        let ds = dataset();
        let receipt = store
            .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
            .unwrap();
        let directory = store
            .evaluation_directory(&model, DataRole::HeldOut, false)
            .unwrap()
            .unwrap();
        let path = if pointer {
            directory.join(LATEST)
        } else {
            receipt_path(&store, &model, &receipt)
        };
        atomic_private_write(&path, b"corrupt evaluation").unwrap();
        assert!(store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .is_err());
        assert!(store
            .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT + 1)
            .is_err());
        assert_eq!(private_read(&path).unwrap(), b"corrupt evaluation");
        assert_eq!(
            store
                .load_candidate(model.context())
                .unwrap()
                .unwrap()
                .export()
                .unwrap(),
            model.export().unwrap()
        );
    }
}
#[test]
fn self_consistent_false_authority_or_wrong_model_role_metrics_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    let original = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    let directory = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap();
    type Mutant = fn(&mut EvaluationReceipt);
    let mutants: [Mutant; 8] = [
        |r: &mut EvaluationReceipt| r.adoption_authorized = true,
        |r: &mut EvaluationReceipt| r.physically_validated = true,
        |r: &mut EvaluationReceipt| r.independence_verified = true,
        |r: &mut EvaluationReceipt| r.report.correct = 999,
        |r: &mut EvaluationReceipt| r.model_sha256 = "0".repeat(64),
        |r: &mut EvaluationReceipt| r.role = DataRole::Train,
        |r: &mut EvaluationReceipt| {
            r.provenance =
                vec![crate::room_learner::observation::CaptureProvenance::LiveReceiverObserved]
        },
        |r: &mut EvaluationReceipt| r.training_data_sha256 = "0".repeat(64),
    ];
    for mutate in mutants {
        let mut bad = original.clone();
        mutate(&mut bad);
        let key = bad.sha256().unwrap();
        let envelope = EvaluationEnvelope {
            schema: SCHEMA.into(),
            receipt_sha256: key.clone(),
            receipt: bad,
        };
        write_immutable(
            &directory.join(format!("{key}.json")),
            &encode(&envelope).unwrap(),
        )
        .unwrap();
        let pointer = EvaluationPointer {
            schema: POINTER_SCHEMA_EVAL.into(),
            receipt_sha256: key,
            model_sha256: original.model_sha256.clone(),
            context_digest: original.context_digest.clone(),
            role: DataRole::HeldOut,
        };
        atomic_private_write(&directory.join(LATEST), &encode(&pointer).unwrap()).unwrap();
        assert!(store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .is_err());
    }
}
#[cfg(unix)]
#[test]
fn symlinks_and_insecure_files_never_replace_private_evidence_or_touch_victims() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for target in ["receipt", "pointer", "role", "model", "evaluations"] {
        let temp = tempfile::tempdir().unwrap();
        let (store, model) = saved(temp.path());
        let ds = dataset();
        let receipt = store
            .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
            .unwrap();
        let directory = store
            .evaluation_directory(&model, DataRole::HeldOut, false)
            .unwrap()
            .unwrap();
        let path = match target {
            "receipt" => receipt_path(&store, &model, &receipt),
            "pointer" => directory.join(LATEST),
            "role" => directory.clone(),
            "model" => directory.parent().unwrap().to_path_buf(),
            _ => directory.parent().unwrap().parent().unwrap().to_path_buf(),
        };
        std::fs::rename(&path, path.with_extension("preserved")).unwrap();
        let victim = temp.path().join("victim");
        atomic_private_write(&victim, b"victim bytes").unwrap();
        symlink(&victim, &path).unwrap();
        assert!(store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .is_err());
        assert!(store
            .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT + 1)
            .is_err());
        assert_eq!(private_read(&victim).unwrap(), b"victim bytes");
    }
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    let receipt = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    let path = receipt_path(&store, &model, &receipt);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store
        .load_latest_evaluation(&model, DataRole::HeldOut)
        .is_err());
}
#[test]
fn disk_obstacles_and_oversized_pointers_leave_frozen_model_and_receipts_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    let context = store.context_dir(model.context(), false).unwrap().unwrap();
    let obstacle = context.join("evaluations");
    atomic_private_write(&obstacle, b"existing bytes").unwrap();
    assert!(store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .is_err());
    assert_eq!(private_read(&obstacle).unwrap(), b"existing bytes");
    assert!(store.load_candidate(model.context()).unwrap().is_some());
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let receipt = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    let path = receipt_path(&store, &model, &receipt);
    let before = private_read(&path).unwrap();
    let pointer = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap()
        .join(LATEST);
    let oversized = vec![b' '; LIMIT + 1];
    atomic_private_write(&pointer, &oversized).unwrap();
    assert!(store
        .load_latest_evaluation(&model, DataRole::HeldOut)
        .is_err());
    assert!(store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT + 1)
        .is_err());
    assert_eq!(private_read(&path).unwrap(), before);
    assert_eq!(std::fs::read(pointer).unwrap(), oversized);
}

#[test]
fn preexisting_immutable_collision_preserves_the_previous_pointer_and_all_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let ds = dataset();
    let original = store
        .evaluate_candidate_at(&model, &ds, DataRole::HeldOut, AT)
        .unwrap();
    let directory = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap();
    let pointer_before = private_read(&directory.join(LATEST)).unwrap();
    let original_before = private_read(&receipt_path(&store, &model, &original)).unwrap();
    let mut rows = ds
        .train()
        .iter()
        .chain(ds.validation())
        .chain(ds.held_out())
        .cloned()
        .collect::<Vec<_>>();
    rows.iter_mut()
        .find(|r| r.role == DataRole::HeldOut)
        .unwrap()
        .views[0]
        .samples[0][0] += 0.02;
    let changed = ValidatedDataset::new(ds.context().clone(), rows).unwrap();
    let next = EvaluationReceipt::new(&model, changed.held_out(), DataRole::HeldOut, AT + 1, true)
        .unwrap();
    let collision = directory.join(format!("{}.json", next.sha256().unwrap()));
    atomic_private_write(&collision, b"existing immutable evidence").unwrap();
    assert!(store
        .evaluate_candidate_at(&model, &changed, DataRole::HeldOut, AT + 1)
        .is_err());
    assert_eq!(
        private_read(&collision).unwrap(),
        b"existing immutable evidence"
    );
    assert_eq!(
        private_read(&directory.join(LATEST)).unwrap(),
        pointer_before
    );
    assert_eq!(
        private_read(&receipt_path(&store, &model, &original)).unwrap(),
        original_before
    );
    assert_eq!(
        store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .unwrap(),
        Some(original)
    );
}

#[path = "evaluation_index_tests.rs"]
mod index_tests;
