//! All values are SYNTHETIC. No user sensor files or physical grants.
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
pub(crate) fn setup() -> (
    tempfile::TempDir,
    Arc<ReferenceStore>,
    watch::Sender<bool>,
    Grant,
) {
    let dir = tempfile::tempdir().unwrap();
    let proof = crate::companion::secret();
    let disk = json!({"version":1,"installation_id":"synthetic-install","devices":[{"device_id":"phone","device_name":"Synthetic phone","pairing_hash":format!("{:x}",Sha256::digest(proof.as_bytes())),"expires_at":now()+3600}]});
    crate::companion::atomic_private_write(
        &dir.path().join("companion-grants.json"),
        &serde_json::to_vec(&disk).unwrap(),
    )
    .unwrap();
    let companion = Companion::load_for_account(
        dir.path(),
        "synthetic-install".into(),
        "a".repeat(64),
        "https://test.local:3443".into(),
        Some("a".repeat(64)),
    )
    .unwrap();
    let (tx, rx) = watch::channel(false);
    let grant = Grant {
        device_id: "phone".into(),
        expires_at: now() + 3600,
        revoked: rx,
    };
    (dir, ReferenceStore::new(companion), tx, grant)
}
pub(crate) fn lineage() -> Lineage {
    Lineage {
        capture_session_id: "synthetic-capture".into(),
        coordinate_frame_id: "synthetic-frame".into(),
        clock_model_id: "arkit-monotonic+session-wall-offset-v1".into(),
        room_binding_digest: "b".repeat(64),
        room_revision: 1,
    }
}
fn scope() -> ReferenceScope {
    ReferenceScope {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-install".into(),
        runtime_epoch: "synthetic-runtime".into(),
        generation: 1,
        room_revision: 1,
        model_kind: "ruview.room-learner.managed-temporal-input.v3".into(),
        fit_record_sha256: "c".repeat(64),
        training_proof_sha256: "d".repeat(64),
        run: "run-a".into(),
        context_digest: "c".repeat(64),
        room_digest: "b".repeat(64),
        model_sha256: "d".repeat(64),
        trial_id: "synthetic-trial".into(),
        session_id: "synthetic-session".into(),
        role: crate::room_learner::observation::DataRole::HeldOut,
    }
}
pub(crate) fn begin() -> ReferenceMessage {
    ReferenceMessage::Begin {
        schema: SCHEMA.into(),
        lineage: lineage(),
    }
}
pub(crate) fn observation(id: &str, sequence: u64, t: u64) -> ReferenceMessage {
    ReferenceMessage::Observation {
        schema: SCHEMA.into(),
        reference_session_id: id.into(),
        sequence,
        lineage: lineage(),
        monotonic_timestamp_seconds: format!("{sequence}.125"),
        captured_at_unix_ms: t / 1_000_000,
        tracking_state: TrackingState::Normal,
        detector_completed: true,
        visible_body_count: 0,
    }
}
#[test]
fn strict_wire_rejects_unknown_raw_fields_roles_clocks_and_oversize() {
    let base = json!({"type":"begin","schema":SCHEMA,"lineage":{"capture_session_id":"native","coordinate_frame_id":"frame","clock_model_id":"arkit-monotonic+session-wall-offset-v1","room_binding_digest":"b".repeat(64),"room_revision":1}});
    assert!(ReferenceMessage::decode(&serde_json::to_vec(&base).unwrap(), now() * 1000).is_ok());
    for field in [
        "verified",
        "role",
        "owner_scope",
        "pointsMm",
        "rf_frame_ids",
        "rawCamera",
    ] {
        let mut v = base.clone();
        v[field] = json!(true);
        assert!(
            ReferenceMessage::decode(&serde_json::to_vec(&v).unwrap(), now() * 1000).is_err(),
            "{field}"
        );
    }
    assert!(ReferenceMessage::decode(&vec![b' '; MAX_BYTES + 1], 0).is_err());
    let mut v = base;
    v["lineage"]["room_revision"] = json!(0);
    assert!(ReferenceMessage::decode(&serde_json::to_vec(&v).unwrap(), 0).is_err());
}
#[test]
fn separate_purpose_expiry_revoke_replay_lineage_rate_and_recording() {
    let (_dir, s, tx, g) = setup();
    let t = now() * 1_000_000_000;
    assert!(s.admit(&g, scope(), begin(), t).is_err());
    assert_eq!(s.permit("phone", false, now()).unwrap(), now() + 900);
    let r = s.admit(&g, scope(), begin(), t).unwrap();
    assert!(!r.physically_validated);
    assert!(s.snapshot(now()).is_err());
    s.admit(
        &g,
        scope(),
        observation(&r.reference_session_id, 1, t + 1),
        t + 1,
    )
    .unwrap();
    assert!(s
        .admit(
            &g,
            scope(),
            observation(&r.reference_session_id, 1, t + 2),
            t + 2
        )
        .is_err());
    let mut other = scope();
    other.runtime_epoch = "new-runtime".into();
    assert!(s
        .admit(
            &g,
            other,
            observation(&r.reference_session_id, 2, t + 3),
            t + 3
        )
        .is_err());
    tx.send_replace(true);
    assert!(s
        .admit(
            &g,
            scope(),
            observation(&r.reference_session_id, 3, t + 4),
            t + 4
        )
        .is_err());
    s.revoke("phone").unwrap();
    assert_eq!(s.summary(now())["retained_ram_rows"], 0);
}
#[test]
fn explicit_recording_and_budget_are_independent_of_source_claims() {
    let (_dir, s, _tx, g) = setup();
    let t = now() * 1_000_000_000;
    s.permit("phone", true, now()).unwrap();
    let r = s.admit(&g, scope(), begin(), t).unwrap();
    s.admit(
        &g,
        scope(),
        observation(&r.reference_session_id, 1, t + 1),
        t + 1,
    )
    .unwrap();
    assert_eq!(s.snapshot(now()).unwrap().rows.len(), 1);
    for i in 2..MAX_RATE as u64 {
        s.admit(
            &g,
            scope(),
            observation(&r.reference_session_id, i, t + i),
            t + i,
        )
        .unwrap();
    }
    assert_eq!(
        s.admit(
            &g,
            scope(),
            observation(&r.reference_session_id, 20, t + 20),
            t + 20
        )
        .unwrap_err(),
        "reference_rate_limit"
    );
    let mut state = s.inner.lock().unwrap();
    let row = state.rows[0].clone();
    state.rows = vec![row; MAX_ROWS];
    drop(state);
    assert_eq!(
        s.admit(
            &g,
            scope(),
            observation(&r.reference_session_id, 21, t + 2_000_000_000),
            t + 2_000_000_000
        )
        .unwrap_err(),
        "reference_row_budget"
    );
    assert_eq!(s.snapshot(now()).unwrap().rows.len(), MAX_ROWS);
    assert!(s.snapshot(now() + 900).is_err());
    assert!(s.inner.lock().unwrap().rows.is_empty());
}

#[test]
fn owner_installation_time_and_renewed_purpose_cannot_reuse_earlier_snapshot() {
    let (_dir, s, _tx, g) = setup();
    let t = now() * 1_000_000_000;
    s.permit("phone", true, now()).unwrap();
    let mut foreign = scope();
    foreign.owner_scope = "e".repeat(64);
    assert!(s.admit(&g, foreign, begin(), t).is_err());
    let mut foreign = scope();
    foreign.installation_id = "foreign-installation".into();
    assert!(s.admit(&g, foreign, begin(), t).is_err());
    let r = s.admit(&g, scope(), begin(), t).unwrap();
    s.admit(
        &g,
        scope(),
        observation(&r.reference_session_id, 1, t + 1),
        t + 1,
    )
    .unwrap();
    let old = s.snapshot(now()).unwrap();
    s.permit("phone", true, now()).unwrap();
    assert!(old.ensure_live().is_err());
    let r = s.admit(&g, scope(), begin(), t + 2).unwrap();
    let mut foreign = scope();
    foreign.generation += 1;
    assert!(s
        .admit(
            &g,
            foreign,
            observation(&r.reference_session_id, 2, t + 3),
            t + 3
        )
        .is_err());
    s.inner.lock().unwrap().permit.as_mut().unwrap().deadline = std::time::Instant::now();
    assert!(s
        .admit(
            &g,
            scope(),
            observation(&r.reference_session_id, 3, t + 4),
            t + 4
        )
        .is_err());
}

#[test]
fn actual_protocol_producer_fixture_is_synthetic_and_derived_only() {
    let (_dir, s, _tx, g) = setup();
    let t = now() * 1_000_000_000;
    s.permit("phone", true, now()).unwrap();
    let lineage = json!({"capture_session_id":"synthetic-capture","coordinate_frame_id":"synthetic-frame","clock_model_id":"arkit-monotonic+session-wall-offset-v1","room_binding_digest":"b".repeat(64),"room_revision":1});
    let begin = json!({"type":"begin","schema":SCHEMA,"lineage":lineage});
    let begin_result = s
        .admit(
            &g,
            scope(),
            ReferenceMessage::decode(&serde_json::to_vec(&begin).unwrap(), t / 1_000_000).unwrap(),
            t,
        )
        .unwrap();
    let observation = json!({"type":"observation","schema":SCHEMA,"reference_session_id":begin_result.reference_session_id,"sequence":1,"lineage":lineage,"monotonic_timestamp_seconds":"42.125","captured_at_unix_ms":t/1_000_000,"tracking_state":"normal","detector_completed":true,"visible_body_count":0});
    let result = s
        .admit(
            &g,
            scope(),
            ReferenceMessage::decode(&serde_json::to_vec(&observation).unwrap(), t / 1_000_000)
                .unwrap(),
            t + 1,
        )
        .unwrap();
    let fixture = json!({"schema":"ruview.test.reference-producer.v1","evidence":"SYNTHETIC_NOT_PHYSICAL_QUALIFICATION","begin_request":begin,"begin_response":begin_result,"observation_request":observation,"observation_response":result,"owner_summary":s.summary(now())});
    if let Ok(path) = std::env::var("RUVIEW_R1_PROTOCOL_FIXTURE") {
        std::fs::write(path, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    }
    for field in ["visible_body_count", "monotonic_timestamp_seconds"] {
        let mut bad = observation.clone();
        bad[field] = json!(-1);
        assert!(
            ReferenceMessage::decode(&serde_json::to_vec(&bad).unwrap(), t / 1_000_000).is_err()
        );
    }
}

#[test]
fn invalidation_erases_ram_and_withdraws_existing_snapshots_without_budget_eviction() {
    for cause in ["purpose", "deadline", "device", "access", "access_revoke"] {
        let (_dir, store, access_stop, grant) = setup();
        let at = now() * 1_000_000_000;
        store.permit("phone", true, now()).unwrap();
        let begin = store.admit(&grant, scope(), begin(), at).unwrap();
        store
            .admit(
                &grant,
                scope(),
                observation(&begin.reference_session_id, 1, at + 1),
                at + 1,
            )
            .unwrap();
        let snapshot = store.snapshot(now()).unwrap();
        let mut device_stop = None;
        {
            let mut inner = store.inner.lock().unwrap();
            let permit = inner.permit.as_mut().unwrap();
            match cause {
                "purpose" => permit.until = now(),
                "deadline" => permit.deadline = std::time::Instant::now(),
                "device" => {
                    let (tx, rx) = watch::channel(false);
                    permit.revoked = rx;
                    device_stop = Some(tx);
                }
                "access" => permit.access_until = now(),
                "access_revoke" => {
                    access_stop.send_replace(true);
                }
                _ => unreachable!(),
            }
        }
        if let Some(stop) = &device_stop {
            stop.send_replace(true);
        }
        assert_eq!(store.summary(now())["retained_ram_rows"], 0, "{cause}");
        let inner = store.inner.lock().unwrap();
        assert!(inner.rows.is_empty(), "{cause} retained observations");
        assert!(inner.permit.is_none(), "{cause} retained a session/permit");
        drop(inner);
        assert!(
            snapshot.ensure_live().is_err(),
            "{cause} revived a snapshot"
        );
        assert!(store.snapshot(now()).is_err());
    }
}

#[tokio::test]
async fn idle_expiry_task_purges_rows_without_polling_and_does_not_own_the_store() {
    let (_dir, store, _access_stop, grant) = setup();
    let at = now() * 1_000_000_000;
    store.permit("phone", true, now()).unwrap();
    let begin = store.admit(&grant, scope(), begin(), at).unwrap();
    store
        .admit(
            &grant,
            scope(),
            observation(&begin.reference_session_id, 1, at + 1),
            at + 1,
        )
        .unwrap();
    store
        .inner
        .lock()
        .unwrap()
        .permit
        .as_mut()
        .unwrap()
        .deadline = std::time::Instant::now();
    store.spawn_expiry_sweeper();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if store.inner.lock().unwrap().rows.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("idle reference data outlived the bounded cleanup interval");
    let weak = Arc::downgrade(&store);
    drop(store);
    assert!(
        weak.upgrade().is_none(),
        "sweeper retained account reference state"
    );
}

#[test]
fn current_whole_model_fit_train_run_and_publication_cannot_cross_reference_session() {
    let (_dir, store, _revoked, grant) = setup();
    let now = now() * 1_000_000_000;
    store.permit("phone", true, now / 1_000_000_000).unwrap();
    store.admit(&grant, scope(), begin(), now).unwrap();
    for field in 0..5 {
        let mut changed = scope();
        match field {
            0 => changed.model_kind = "ruview.room-learner.temporal-input.v2".into(),
            1 => changed.fit_record_sha256 = "e".repeat(64),
            2 => changed.training_proof_sha256 = "e".repeat(64),
            3 => changed.run = "another-run".into(),
            _ => changed.room_revision = 2,
        }
        assert!(store
            .admit(
                &grant,
                changed,
                begin(),
                now + (field as u64 + 1) * 100_000_000
            )
            .is_err());
    }
    assert_eq!(store.summary(now / 1_000_000_000)["retained_ram_rows"], 0);
}

/// Causal recording-lease boundary probe. Uses the real permit/snapshot/store
/// and existing private atomic writer. HTTP ACK composition is tested separately.
#[test]
fn purpose_withdrawal_must_drain_an_entered_recording_before_returning() {
    use std::{sync::mpsc, time::Duration};
    for withdraw in [false, true] {
        let (directory, store, _revoked, grant) = setup();
        let received = now() * 1_000_000_000;
        store
            .permit("phone", true, received / 1_000_000_000)
            .unwrap();
        let begun = store.admit(&grant, scope(), begin(), received).unwrap();
        store
            .admit(
                &grant,
                scope(),
                observation(&begun.reference_session_id, 1, received + 100_000),
                received + 100_000,
            )
            .unwrap();
        let snapshot = store.snapshot(received / 1_000_000_000).unwrap();
        let target = directory.path().join("synthetic-comparison-lease.json");
        let (entered_send, entered) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            snapshot.with_recording(|| {
                entered_send.send(()).unwrap();
                released.recv().unwrap();
                crate::companion::atomic_private_write(&target, b"SYNTHETIC_DERIVED_METADATA")
                    .map_err(|_| "synthetic_recording_io".to_string())
            })
        });
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        let (ack_send, ack) = mpsc::channel();
        let revoker = std::thread::spawn(move || {
            if withdraw {
                store.withdraw();
            } else {
                store.revoke("phone").unwrap();
            }
            ack_send.send(()).unwrap();
        });
        let returned_before_writer = ack.recv_timeout(Duration::from_millis(80)).is_ok();
        release.send(()).unwrap();
        let written = worker.join().unwrap();
        revoker.join().unwrap();
        assert!(
            !returned_before_writer,
            "purpose withdrawal acknowledged while its entered writer could still commit"
        );
        assert!(
            written.is_ok(),
            "already-consented recording finishes before withdrawal acknowledgment"
        );
    }
}

#[test]
fn admission_clock_checks_do_not_retime_transport_receipts_or_accept_queued_future_data() {
    let (_directory, store, _revoked, grant) = setup();
    let received = now() * 1_000_000_000;
    store
        .permit("phone", true, received / 1_000_000_000)
        .unwrap();
    let session = store
        .admit_at(&grant, scope(), begin(), received, received + 100_000_000)
        .unwrap();
    let stamp = received + 200_000_000;
    store
        .admit_at(
            &grant,
            scope(),
            observation(&session.reference_session_id, 1, stamp),
            stamp,
            stamp + 500_000_000,
        )
        .unwrap();
    let snapshot = store.snapshot(received / 1_000_000_000).unwrap();
    assert_eq!(
        snapshot.rows[0].received_ns, stamp,
        "admission cannot freshen original receipt time"
    );
    for (at, admitted) in [
        (stamp + 2_000_000_000, stamp + 1_000_000_000),
        (stamp, stamp + 3_000_000_001),
    ] {
        assert!(store
            .admit_at(
                &grant,
                scope(),
                observation(&session.reference_session_id, 2, at),
                at,
                admitted
            )
            .is_err());
    }
    assert_eq!(
        store.snapshot(received / 1_000_000_000).unwrap().rows.len(),
        1
    );
    // A queued call may not use the earlier received time to revive an expired purpose.
    let expired = received + 901_000_000_000;
    assert!(store
        .admit_at(
            &grant,
            scope(),
            observation(&session.reference_session_id, 2, expired - 1_000_000),
            expired - 1_000_000,
            expired
        )
        .is_err());
    assert!(store.snapshot(expired / 1_000_000_000).is_err());
}

#[path="reference_publication_boundary_tests.rs"]
mod publication_boundaries;
