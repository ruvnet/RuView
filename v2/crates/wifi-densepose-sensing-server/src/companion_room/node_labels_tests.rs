use super::*;
use std::sync::Arc;
fn private_tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    // Matches Integration's real companion::private_dir, not an unprotected TempDir.
    #[cfg(unix)]
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn room(ids: &[u8]) -> Value {
    let slots = ["north_west", "north_center", "north_east", "west_center"];
    json!({"schema":"ruview.companion.room.v1","id":"room-1","name":"Living room","widthM":5,"lengthM":6,"heightM":2.6,"nodes":ids.iter().enumerate().map(|(i,id)|json!({"nodeId":id,"slot":slots[i],"xM":0.45,"yM":0.45,"role":"receiver","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1})).collect::<Vec<_>>(),"accessPoint":{"slot":"south_east","xM":4.55,"yM":5.55,"provenance":"operator_entered","confirmedAtUnixMs":1}})
}
fn setup(ids: &[u8]) -> (tempfile::TempDir, Arc<SharedRoomStore>) {
    let dir = private_tempdir();
    let store =
        SharedRoomStore::load_for_account(dir.path(), "installation-1", Some(&"a".repeat(64)))
            .unwrap();
    store.publish(0, room(ids)).unwrap();
    (dir, store)
}
fn request(store: &SharedRoomStore, revision: u64, labels: Value) -> Update {
    serde_json::from_value(json!({"schema":UPDATE,"installation_id":"installation-1","expected_publication":store.node_labels_snapshot().unwrap()["publication"],"expected_metadata_revision":revision,"labels":labels})).unwrap()
}
fn set(store: &SharedRoomStore, revision: u64, name: &str) -> Result<Value, StatusCode> {
    store.update_node_labels(
        request(store, revision, json!([{"node_id":1,"display_name":name}])),
        || true,
    )
}
#[test]
fn labels_cas_idempotence_cold_read_and_legacy_bytes_are_separate() {
    let (dir, store) = setup(&[1]);
    let before = fs::read(&store.path).unwrap();
    for count in [1usize, 2, 4] {
        let ids = (0..count).map(|i| (i * 9 + 1) as u8).collect::<Vec<_>>();
        let (_d, s) = setup(&ids);
        let labels = ids
            .iter()
            .map(|id| json!({"node_id":id,"display_name":format!("Receiver {id}")}))
            .collect::<Vec<_>>();
        let ack = s
            .update_node_labels(request(&s, 0, json!(labels)), || true)
            .unwrap();
        assert_eq!(ack["snapshot"]["labels"].as_array().unwrap().len(), count);
    }
    assert_eq!(
        store.node_labels_snapshot().unwrap()["status"],
        "not_reported"
    );
    let ack = set(&store, 0, "West transmitter").unwrap();
    assert_eq!(ack["effect"], "updated");
    assert_eq!(ack["snapshot"]["metadata_revision"], 1);
    let sidecar = fs::read(dir.path().join(FILE)).unwrap();
    let retry = set(&store, 1, "West transmitter").unwrap();
    assert_eq!(retry["effect"], "already_current");
    assert_eq!(sidecar, fs::read(dir.path().join(FILE)).unwrap());
    assert_eq!(
        set(&store, 0, "different").unwrap_err(),
        StatusCode::CONFLICT
    );
    let restored =
        SharedRoomStore::load_for_account(dir.path(), "installation-1", Some(&"a".repeat(64)))
            .unwrap();
    assert_eq!(restored.node_labels_snapshot().unwrap(), ack["snapshot"]);
    assert_eq!(fs::read(&store.path).unwrap(), before);
    // Older strict Envelope is unchanged and can read the same room file.
    let old = SharedRoomStore::load(dir.path(), "installation-1").unwrap();
    assert_eq!(
        serde_json::to_value(old.snapshot().unwrap()).unwrap(),
        serde_json::from_slice::<Value>(&before).unwrap()
    );
    assert_eq!(
        old.node_labels_snapshot().unwrap_err(),
        StatusCode::FORBIDDEN
    );
    let cleared = store
        .update_node_labels(request(&store, 1, json!([])), || true)
        .unwrap();
    assert_eq!(cleared["snapshot"]["status"], "current");
    assert_eq!(cleared["snapshot"]["labels"], json!([]));
}
#[test]
fn malformed_labels_exact_scalars_utf16_and_tuple_are_refused() {
    let (_dir, store) = setup(&[1]);
    for labels in [
        json!([{"node_id":2,"display_name":"Other"}]),
        json!([{"node_id":1,"display_name":" leading"}]),
        json!([{"node_id":1,"display_name":"two  spaces"}]),
        json!([{"node_id":1,"display_name":"bad\u{7f}"}]),
        json!([{"node_id":1,"display_name":"😀".repeat(21)}]),
        json!([{"node_id":1,"display_name":""}]),
        json!([{"node_id":1,"display_name":"A"},{"node_id":1,"display_name":"B"}]),
    ] {
        assert_eq!(
            store
                .update_node_labels(request(&store, 0, labels), || true)
                .unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
    for value in [json!(["Name"]), json!(4), json!(null)] {
        let mut wire = serde_json::to_value(request(&store, 0, json!([]))).unwrap();
        wire["labels"] = json!([{"node_id":1,"display_name":value}]);
        assert!(serde_json::from_value::<Update>(wire).is_err());
    }
    let mut wrong = request(&store, 0, json!([]));
    wrong.expected_publication.publication_revision += 1;
    assert_eq!(
        store.update_node_labels(wrong, || true).unwrap_err(),
        StatusCode::CONFLICT
    );
    assert!(set(&store, 0, &"😀".repeat(20)).is_ok());
}
#[test]
fn corrupted_unknown_foreign_symlink_and_stage_are_withheld_never_zero_overwritten() {
    for mode in [
        "future",
        "oversized",
        "owner",
        "partial",
        "stage",
        "symlink",
    ] {
        let (dir, store) = setup(&[1]);
        set(&store, 0, "A").unwrap();
        let file = dir.path().join(FILE);
        match mode {
            "stage" => fs::write(dir.path().join(STAGE), b"partial").unwrap(),
            "partial" => fs::write(&file, b"{").unwrap(),
            "oversized" => fs::write(&file, vec![b' '; MAX_BYTES + 1]).unwrap(),
            #[cfg(unix)]
            "symlink" => {
                fs::remove_file(&file).unwrap();
                std::os::unix::fs::symlink(&store.path, &file).unwrap();
            }
            _ => {
                let mut v: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
                if mode == "owner" {
                    v["owner_scope"] = json!("b".repeat(64));
                } else {
                    v["schema"] = json!("future.v9");
                }
                fs::write(&file, v.to_string()).unwrap();
            }
        }
        let before = fs::read(&file).unwrap();
        let cold =
            SharedRoomStore::load_for_account(dir.path(), "installation-1", Some(&"a".repeat(64)))
                .unwrap();
        let s = cold.node_labels_snapshot().unwrap();
        assert_eq!(s["status"], "unavailable", "{mode}");
        assert!(s["metadata_revision"].is_null());
        assert!(s["labels"].is_null());
        assert_eq!(
            set(&cold, 0, "overwrite").unwrap_err(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(fs::read(&file).unwrap(), before);
        assert!(SharedRoomStore::load(dir.path(), "installation-1").is_ok());
    }
}
#[test]
fn delayed_writer_never_holds_room_lock_and_old_tuple_never_labels_new_publication() {
    let (_dir, store) = setup(&[1]);
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
    let writer = store.clone();
    let pending = request(&store, 0, json!([{"node_id":1,"display_name":"Old"}]));
    let thread = std::thread::spawn(move || {
        let once = std::sync::atomic::AtomicBool::new(false);
        writer.update_node_labels(pending, || {
            if !once.swap(true, std::sync::atomic::Ordering::SeqCst) {
                entered_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
            }
            true
        })
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert!(store.snapshot().is_some());
    assert_eq!(
        store.node_labels_snapshot().unwrap()["status"],
        "not_reported"
    );
    store.publish(1, room(&[9])).unwrap();
    release_tx.send(()).unwrap();
    assert_eq!(thread.join().unwrap().unwrap_err(), StatusCode::CONFLICT);
    assert_eq!(
        store.node_labels_snapshot().unwrap()["publication"]["node_ids"],
        json!([9])
    );
    assert_eq!(
        store.node_labels_snapshot().unwrap()["status"],
        "not_reported"
    );
    let new = store
        .update_node_labels(
            request(&store, 0, json!([{"node_id":9,"display_name":"New"}])),
            || true,
        )
        .unwrap();
    assert_eq!(new["snapshot"]["metadata_revision"], 1);
}
#[test]
fn profile_deletion_never_recreates_a_directory_and_other_owner_is_independent() {
    let (dir, a) = setup(&[1]);
    let bdir = private_tempdir();
    let b = SharedRoomStore::load_for_account(bdir.path(), "installation-1", Some(&"b".repeat(64)))
        .unwrap();
    b.publish(0, room(&[1])).unwrap();
    set(&a, 0, "A").unwrap();
    set(&b, 0, "B").unwrap();
    let before = fs::read(bdir.path().join(FILE)).unwrap();
    let request = request(&a, 1, json!([]));
    fs::remove_dir_all(dir.path()).unwrap();
    assert_eq!(
        a.update_node_labels(request, || true).unwrap_err(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(!dir.path().exists());
    assert_eq!(fs::read(bdir.path().join(FILE)).unwrap(), before);
}
#[test]
fn agreed_contract_shapes_match_actual_serializer() {
    let cases: Value = serde_json::from_str(include_str!("node_labels_wire.json")).unwrap();
    let current = &cases["cases"]["current"];
    let p: Publication = serde_json::from_value(current["publication"].clone()).unwrap();
    let labels: Vec<Label> = serde_json::from_value(current["labels"].clone()).unwrap();
    assert!(p.valid() && labels_valid(&labels, &p));
    let wire: Update = serde_json::from_value(cases["cases"]["update"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(wire).unwrap(),
        cases["cases"]["update"]
    );
    let dir = private_tempdir();
    let labels = Labels::load(dir.path(), "installation-1", Some(&"a".repeat(64)));
    assert_eq!(
        labels.snapshot_for(Some(p.clone())),
        cases["cases"]["not_reported"]
    );
    *labels.current.lock().unwrap() = Known::Record(Record {
        schema: SCHEMA.into(),
        owner_scope: "a".repeat(64),
        installation_id: "installation-1".into(),
        publication: p.clone(),
        metadata_revision: 2,
        labels: labels_valid_fixture(&cases),
    });
    assert_eq!(labels.snapshot_for(Some(p.clone())), *current);
    labels.invalidate();
    assert_eq!(labels.snapshot_for(Some(p)), cases["cases"]["unavailable"]);
    assert_eq!(labels.snapshot_for(None), cases["cases"]["room_required"]);
}
fn labels_valid_fixture(cases: &Value) -> Vec<Label> {
    serde_json::from_value(cases["cases"]["current"]["labels"].clone()).unwrap()
}

#[test]
fn competing_store_cannot_overwrite_newer_metadata_or_borrow_foreign_owner() {
    let (dir, a) = setup(&[1]);
    let stale =
        SharedRoomStore::load_for_account(dir.path(), "installation-1", Some(&"a".repeat(64)))
            .unwrap();
    let stale_request = request(&stale, 0, json!([{"node_id":1,"display_name":"Stale"}]));
    set(&a, 0, "New").unwrap();
    let before = fs::read(dir.path().join(FILE)).unwrap();
    assert_eq!(
        stale
            .update_node_labels(stale_request, || true)
            .unwrap_err(),
        StatusCode::CONFLICT
    );
    assert_eq!(fs::read(dir.path().join(FILE)).unwrap(), before);
    let foreign =
        SharedRoomStore::load_for_account(dir.path(), "installation-1", Some(&"b".repeat(64)))
            .unwrap();
    assert_eq!(
        foreign.node_labels_snapshot().unwrap()["status"],
        "unavailable"
    );
    assert_eq!(
        set(&foreign, 0, "Adopt").unwrap_err(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(fs::read(dir.path().join(FILE)).unwrap(), before);
}
#[test]
fn refusal_at_staged_barrier_cleans_only_own_stage_and_keeps_original_record() {
    let (dir, store) = setup(&[1]);
    set(&store, 0, "Original").unwrap();
    let before = fs::read(dir.path().join(FILE)).unwrap();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let result = store.update_node_labels(
        request(
            &store,
            1,
            json!([{"node_id":1,"display_name":"Unconfirmed"}]),
        ),
        || calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0,
    );
    assert_eq!(result.unwrap_err(), StatusCode::CONFLICT);
    assert!(!dir.path().join(STAGE).exists());
    assert_eq!(fs::read(dir.path().join(FILE)).unwrap(), before);
    assert_eq!(
        store.node_labels_snapshot().unwrap()["labels"][0]["display_name"],
        "Original"
    );
}
#[test]
fn staging_io_failure_withholds_unknown_bytes_and_never_modifies_retained_model_or_recovery() {
    let (dir, store) = setup(&[1]);
    let model = dir.path().join("synthetic-whole-model.bin");
    let recovery = dir.path().join("synthetic-encrypted-recovery.bin");
    fs::write(&model, b"SYNTHETIC immutable weights").unwrap();
    fs::write(&recovery, b"SYNTHETIC immutable original deadline").unwrap();
    let model_before = fs::read(&model).unwrap();
    let recovery_before = fs::read(&recovery).unwrap();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let result = store.update_node_labels(
        request(
            &store,
            0,
            json!([{"node_id":1,"display_name":"Unconfirmed"}]),
        ),
        || {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                fs::create_dir(dir.path().join(FILE)).unwrap();
            }
            true
        },
    );
    assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        store.node_labels_snapshot().unwrap()["status"],
        "unavailable"
    );
    assert!(fs::metadata(dir.path().join(STAGE)).unwrap().len() <= MAX_BYTES as u64);
    assert_eq!(fs::read(&model).unwrap(), model_before);
    assert_eq!(fs::read(&recovery).unwrap(), recovery_before);
    let cold =
        SharedRoomStore::load_for_account(dir.path(), "installation-1", Some(&"a".repeat(64)))
            .unwrap();
    assert_eq!(
        cold.node_labels_snapshot().unwrap()["status"],
        "unavailable"
    );
    assert_eq!(
        set(&cold, 0, "Retry").unwrap_err(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[test]
fn legacy_integral_json_number_keeps_the_same_validated_node_identity() {
    let (dir, store) = setup(&[1]);
    let mut geometry = room(&[1]);
    geometry["nodes"][0]["nodeId"] = json!(1.0);
    store.publish(1, geometry).unwrap();
    let before = fs::read(&store.path).unwrap();
    assert_eq!(
        store.node_labels_snapshot().unwrap()["publication"]["node_ids"],
        json!([1])
    );
    assert_eq!(
        set(&store, 0, "Label").unwrap()["snapshot"]["status"],
        "current"
    );
    assert_eq!(
        fs::read(dir.path().join("companion-room.json")).unwrap(),
        before
    );
}

#[test]
fn final_ack_cannot_publish_success_for_a_different_publication_between_check_and_projection() {
    let (_dir, store) = setup(&[1]);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let result = store.update_node_labels(
        request(
            &store,
            0,
            json!([{"node_id":1,"display_name":"Old publication"}]),
        ),
        || {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 2 {
                // Deterministic geometry publication at the actual final admission boundary.
                // No SharedState/room memory lock may be retained while invoking this callback.
                store.publish(1, room(&[9])).unwrap();
            }
            true
        },
    );
    assert!(
        matches!(result, Err(StatusCode::CONFLICT)),
        "a newer room must not get the old name-write ACK: {result:?}"
    );
    let current = store.node_labels_snapshot().unwrap();
    assert_eq!(current["publication"]["node_ids"], json!([9]));
    assert_eq!(current["status"], "not_reported");
    assert_eq!(current["labels"], json!([]));
}

#[test]
fn idempotent_final_ack_also_rechecks_publication_after_the_last_admission_callback() {
    let (dir, store) = setup(&[1]);
    let initial = set(&store, 0, "Original label").unwrap();
    assert_eq!(initial["effect"], "updated");
    let original_record = fs::read(dir.path().join(FILE)).unwrap();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let result = store.update_node_labels(
        request(
            &store,
            1,
            json!([{"node_id":1,"display_name":"Original label"}]),
        ),
        || {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 2 {
                // The same-value fsync/readback succeeded, but its ACK still
                // belongs only to the exact original room publication.
                store.publish(1, room(&[9])).unwrap();
            }
            true
        },
    );
    assert!(matches!(result, Err(StatusCode::CONFLICT)), "{result:?}");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    assert_eq!(fs::read(dir.path().join(FILE)).unwrap(), original_record);
    let current = store.node_labels_snapshot().unwrap();
    assert_eq!(current["publication"]["node_ids"], json!([9]));
    assert_eq!(current["status"], "not_reported");
    assert_eq!(current["labels"], json!([]));
}
