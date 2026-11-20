//! SYNTHETIC evidence identity across intervening evaluation datasets.
use super::*;
fn changed_evidence(ds: &ValidatedDataset) -> ValidatedDataset {
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
        .samples[0][0] += 0.03;
    ValidatedDataset::new(ds.context().clone(), rows).unwrap()
}
#[test]
fn alternating_datasets_reuse_original_evidence_identity_and_timestamp() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let a = dataset();
    let b = changed_evidence(&a);
    let first = store
        .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT)
        .unwrap();
    let path = receipt_path(&store, &model, &first);
    let first_bytes = private_read(&path).unwrap();
    let middle = store
        .evaluate_candidate_at(&model, &b, DataRole::HeldOut, AT + 1)
        .unwrap();
    assert_ne!(first.dataset_sha256, middle.dataset_sha256);
    // A direct evidence key must work beyond catalog-sized history and ignore
    // unrelated unreadable entries; it must not scan or adopt another receipt.
    let directory = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap();
    for n in 0..257 {
        std::fs::write(
            directory.join(format!("{n:064x}.json")),
            b"unrelated unreadable history",
        )
        .unwrap();
    }
    let repeat = store
        .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT + 2)
        .unwrap();
    assert_eq!(
        repeat.sha256().unwrap(),
        first.sha256().unwrap(),
        "A→B→A must not renew evidence A"
    );
    assert_eq!(repeat.evaluated_at_unix_ms, AT);
    assert_eq!(repeat.train_lineage_checked, first.train_lineage_checked);
    assert_eq!(private_read(&path).unwrap(), first_bytes);
    assert_eq!(
        store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .unwrap(),
        Some(first.clone())
    );
    let cold_a = ValidatedDataset::new(a.context().clone(), a.held_out().to_vec()).unwrap();
    let cold_repeat = store
        .evaluate_candidate_at(&model, &cold_a, DataRole::HeldOut, AT + 3)
        .unwrap();
    assert_eq!(
        cold_repeat, first,
        "historical lineage proof must not be rewritten on reuse"
    );
}

fn index_path(store: &ArtifactStore, model: &RoomModel, receipt: &EvaluationReceipt) -> PathBuf {
    store
        .evaluation_directory(model, receipt.role, false)
        .unwrap()
        .unwrap()
        .join("by-dataset")
        .join(format!("{}.json", receipt.dataset_sha256))
}
#[test]
fn corrupt_or_mismatched_index_never_overwrites_another_latest_receipt() {
    for field in [
        "corrupt",
        "owner_scope",
        "installation_id",
        "context_digest",
        "model_sha256",
        "role",
        "dataset_sha256",
        "receipt_sha256",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let (store, model) = saved(temp.path());
        let a = dataset();
        let b = changed_evidence(&a);
        let first = store
            .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT)
            .unwrap();
        let middle = store
            .evaluate_candidate_at(&model, &b, DataRole::HeldOut, AT + 1)
            .unwrap();
        let directory = store
            .evaluation_directory(&model, DataRole::HeldOut, false)
            .unwrap()
            .unwrap();
        let pointer = directory.join(LATEST);
        let before = private_read(&pointer).unwrap();
        let path = index_path(&store, &model, &first);
        let mut bad: serde_json::Value =
            serde_json::from_slice(&private_read(&path).unwrap()).unwrap();
        let changed = match field {
            "owner_scope" => serde_json::json!("b".repeat(64)),
            "installation_id" => serde_json::json!("wrong-install"),
            "context_digest" | "model_sha256" => serde_json::json!("0".repeat(64)),
            "role" => serde_json::json!("Validation"),
            "dataset_sha256" => serde_json::json!(middle.dataset_sha256),
            "receipt_sha256" => serde_json::json!(middle.sha256().unwrap()),
            _ => serde_json::Value::Null,
        };
        let bytes = if field == "corrupt" {
            b"corrupt index".to_vec()
        } else {
            bad[field] = changed;
            serde_json::to_vec(&bad).unwrap()
        };
        atomic_private_write(&path, &bytes).unwrap();
        assert!(
            store
                .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT + 2)
                .is_err(),
            "index {field}"
        );
        assert_eq!(private_read(&pointer).unwrap(), before);
        assert_eq!(private_read(&path).unwrap(), bytes);
        assert_eq!(
            store
                .load_latest_evaluation(&model, DataRole::HeldOut)
                .unwrap(),
            Some(middle)
        );
        assert!(store.load_candidate(model.context()).unwrap().is_some());
    }
}
#[cfg(unix)]
#[test]
fn evidence_index_symlink_permission_and_size_errors_preserve_latest_and_victims() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for failure in ["symlink", "permission", "oversized"] {
        let temp = tempfile::tempdir().unwrap();
        let (store, model) = saved(temp.path());
        let a = dataset();
        let b = changed_evidence(&a);
        let first = store
            .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT)
            .unwrap();
        store
            .evaluate_candidate_at(&model, &b, DataRole::HeldOut, AT + 1)
            .unwrap();
        let path = index_path(&store, &model, &first);
        let directory = store
            .evaluation_directory(&model, DataRole::HeldOut, false)
            .unwrap()
            .unwrap();
        let before = private_read(&directory.join(LATEST)).unwrap();
        let victim = temp.path().join("victim");
        atomic_private_write(&victim, b"preserved private victim").unwrap();
        match failure {
            "symlink" => {
                std::fs::rename(&path, path.with_extension("preserved")).unwrap();
                symlink(&victim, &path).unwrap();
            }
            "permission" => {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap()
            }
            _ => atomic_private_write(&path, &vec![b' '; LIMIT + 1]).unwrap(),
        }
        assert!(store
            .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT + 2)
            .is_err());
        assert_eq!(private_read(&directory.join(LATEST)).unwrap(), before);
        assert_eq!(private_read(&victim).unwrap(), b"preserved private victim");
        assert!(store.load_candidate(model.context()).unwrap().is_some());
    }
}
#[test]
fn blocked_new_dataset_index_fails_without_publishing_or_replacing_existing_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let a = dataset();
    let b = changed_evidence(&a);
    let first = store
        .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT)
        .unwrap();
    let directory = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap();
    let pointer = directory.join(LATEST);
    let before = private_read(&pointer).unwrap();
    let old_receipt = private_read(&receipt_path(&store, &model, &first)).unwrap();
    let blocked = directory
        .join("by-dataset")
        .join(format!("{}.json", data_sha256(b.held_out()).unwrap()));
    std::fs::create_dir(&blocked).unwrap();
    assert!(store
        .evaluate_candidate_at(&model, &b, DataRole::HeldOut, AT + 1)
        .is_err());
    assert_eq!(private_read(&pointer).unwrap(), before);
    assert_eq!(
        private_read(&receipt_path(&store, &model, &first)).unwrap(),
        old_receipt
    );
    assert!(blocked.is_dir());
    assert_eq!(
        store
            .load_latest_evaluation(&model, DataRole::HeldOut)
            .unwrap(),
        Some(first)
    );
}

#[test]
fn persisted_evidence_index_recovers_original_receipt_after_missing_latest_pointer() {
    let temp = tempfile::tempdir().unwrap();
    let (store, model) = saved(temp.path());
    let a = dataset();
    let original = store
        .evaluate_candidate_at(&model, &a, DataRole::HeldOut, AT)
        .unwrap();
    let directory = store
        .evaluation_directory(&model, DataRole::HeldOut, false)
        .unwrap()
        .unwrap();
    let previous_pointer = directory.join("latest-before-interruption.json");
    std::fs::rename(directory.join(LATEST), &previous_pointer).unwrap();
    let bytes = private_read(&previous_pointer).unwrap();
    let cold = open_store(temp.path());
    let loaded = cold.load_candidate(model.context()).unwrap().unwrap();
    let retry = cold
        .evaluate_candidate_at(&loaded, &a, DataRole::HeldOut, AT + 50)
        .unwrap();
    assert_eq!(
        retry, original,
        "retry must not renew evidence after a partial publication"
    );
    assert_eq!(private_read(&previous_pointer).unwrap(), bytes);
    assert_eq!(
        cold.load_latest_evaluation(&loaded, DataRole::HeldOut)
            .unwrap(),
        Some(original)
    );
}
