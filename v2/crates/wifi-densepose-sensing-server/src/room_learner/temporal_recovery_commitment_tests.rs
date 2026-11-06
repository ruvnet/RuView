use super::*;
use crate::room_learner::temporal_model_input::managed::execution::checkpoint::{
    synthetic_changed_old_row, synthetic_segment_fork, synthetic_storage_prefixes,
};
fn setup() -> (
    tempfile::TempDir,
    RecoveryStore,
    Vec<ValidatedTemporalPrefix>,
) {
    static PREFIXES: std::sync::OnceLock<Vec<ValidatedTemporalPrefix>> = std::sync::OnceLock::new();
    let p = PREFIXES.get_or_init(synthetic_storage_prefixes).clone();
    let d = p[0].descriptor();
    let t = tempfile::tempdir().unwrap();
    let store = RecoveryStore::new(
        &t.path().join(&d.owner_scope).join("SensingServer"),
        &d.owner_scope,
        &d.installation_id,
        Some(&[7; 32]),
    )
    .unwrap();
    (t, store, p)
}
fn now(p: &ValidatedTemporalPrefix) -> u64 {
    p.saved_receiver_ns()
}
fn commit(
    store: &RecoveryStore,
    expected: Option<&TemporalToken>,
    p: &ValidatedTemporalPrefix,
) -> TemporalToken {
    let i = store.prepare_temporal(expected, p, now(p)).unwrap();
    store.publish_temporal(&i, Some(p), now(p)).unwrap()
}
#[test]
fn actual_aead_prefix_commits_two_slots_and_preserves_v1() {
    let (_t, store, p) = setup();
    store
        .save(p[0].descriptor(), p[0].bytes(), now(&p[0]) / 1_000_000)
        .unwrap();
    let v1 = fs::read(store.directory.join(FILE)).unwrap();
    let mut token = None;
    for value in &p {
        let t = commit(&store, token.as_ref(), value);
        let loaded = store.load_temporal(now(value)).unwrap().unwrap();
        assert_eq!(loaded.token, t);
        assert_eq!(loaded.prefix.unwrap().bytes(), value.bytes());
        token = Some(t);
    }
    assert_eq!(fs::read(store.directory.join(FILE)).unwrap(), v1);
    let files = fs::read_dir(store.temporal_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(files.iter().filter(|n| n.starts_with("slot-")).count(), 2);
    assert_eq!(
        files.iter().filter(|n| n.starts_with("witness-")).count(),
        p.len()
    );
}
#[test]
fn selective_head_payload_witness_replacement_fails_closed() {
    for which in ["head", "payload", "witness"] {
        let (_t, store, p) = setup();
        let t0 = commit(&store, None, &p[0]);
        let dir = store.temporal_dir();
        let old_head = fs::read(dir.join(HEAD)).unwrap();
        let old_payload = fs::read(dir.join("slot-0.bin")).unwrap();
        let old_witness = fs::read(dir.join(format!(
            "witness-{:016x}-{:04x}.bin",
            t0.generation, t0.revision
        )))
        .unwrap();
        let t1 = commit(&store, Some(&t0), &p[1]);
        match which {
            "head" => fs::write(dir.join(HEAD), old_head).unwrap(),
            "payload" => fs::write(dir.join("slot-1.bin"), old_payload).unwrap(),
            _ => fs::write(
                dir.join(format!(
                    "witness-{:016x}-{:04x}.bin",
                    t1.generation, t1.revision
                )),
                old_witness,
            )
            .unwrap(),
        };
        assert!(store.load_temporal(now(&p[1])).is_err(), "{which}");
    }
}
#[test]
fn stale_token_changed_old_row_and_same_revision_conflict_reject() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let a = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    let fork = synthetic_segment_fork(&p[0]);
    let b = store
        .prepare_temporal(Some(&t0), &fork, now(&fork))
        .unwrap();
    let t1 = store.publish_temporal(&a, Some(&p[1]), now(&p[1])).unwrap();
    assert!(store.publish_temporal(&b, Some(&fork), now(&fork)).is_err());
    assert!(store
        .prepare_temporal(Some(&t0), &p[2], now(&p[2]))
        .is_err());
    let t2 = commit(&store, Some(&t1), &p[2]);
    let changed = synthetic_changed_old_row(&p[3]);
    assert!(store
        .prepare_temporal(Some(&t2), &changed, now(&changed))
        .is_err());
    let same = store
        .prepare_temporal(Some(&t2), &p[2], now(&p[2]))
        .unwrap();
    assert_eq!(
        store
            .publish_temporal(&same, Some(&p[2]), now(&p[2]))
            .unwrap(),
        t2
    );
}
#[test]
fn known_pending_same_revision_repair_never_promotes_other_intent() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let intent = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    let mut hook = |point: &str| {
        if point == "after:head.stage:directory_fsync" {
            Err("injected-head-ambiguous".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&intent, Some(&p[1]), now(&p[1]), false, &mut hook)
        .is_err());
    assert!(store.load_temporal(now(&p[1])).is_err());
    let token = store
        .repair_same_temporal(&intent, Some(&p[1]), now(&p[1]))
        .unwrap();
    assert_eq!(token.revision, 1);
    assert_eq!(
        store
            .load_temporal(now(&p[1]))
            .unwrap()
            .unwrap()
            .prefix
            .unwrap()
            .bytes(),
        p[1].bytes()
    );
}

#[test]
fn fault_every_barrier_preserves_known_commit_or_blocks_then_exact_repair() {
    let (_t, store, p) = setup();
    let token = commit(&store, None, &p[0]);
    let intent = store
        .prepare_temporal(Some(&token), &p[1], now(&p[1]))
        .unwrap();
    let mut points = Vec::new();
    store
        .publish_temporal_with(&intent, Some(&p[1]), now(&p[1]), false, &mut |s| {
            points.push(s.to_owned());
            Ok(())
        })
        .unwrap();
    for point in points {
        let (_t, store, p) = setup();
        let token = commit(&store, None, &p[0]);
        let intent = store
            .prepare_temporal(Some(&token), &p[1], now(&p[1]))
            .unwrap();
        let mut hook = |s: &str| {
            if s == point {
                Err(format!("injected:{s}"))
            } else {
                Ok(())
            }
        };
        assert!(
            store
                .publish_temporal_with(&intent, Some(&p[1]), now(&p[1]), false, &mut hook)
                .is_err(),
            "{point}"
        );
        if let Ok(Some(loaded)) = store.load_temporal(now(&p[1])) {
            let b = loaded.prefix.unwrap();
            assert!(
                b.bytes() == p[0].bytes() || b.bytes() == p[1].bytes(),
                "{point}"
            );
        }
        let repaired = store
            .repair_same_temporal(&intent, Some(&p[1]), now(&p[1]))
            .unwrap_or_else(|e| panic!("repair {point}: {e}"));
        assert_eq!(repaired.revision, 1);
        assert_eq!(
            store
                .load_temporal(now(&p[1]))
                .unwrap()
                .unwrap()
                .prefix
                .unwrap()
                .bytes(),
            p[1].bytes()
        );
        assert!(!store.temporal_dir().join("payload.stage").exists());
    }
}
#[test]
fn simultaneous_independent_handles_allow_only_one_expected_revision() {
    let (t, store, p) = setup();
    let token = commit(&store, None, &p[0]);
    let a = store
        .prepare_temporal(Some(&token), &p[1], now(&p[1]))
        .unwrap();
    let fork = synthetic_segment_fork(&p[0]);
    let b = store
        .prepare_temporal(Some(&token), &fork, now(&fork))
        .unwrap();
    let d = p[0].descriptor();
    let data = t.path().join(&d.owner_scope).join("SensingServer");
    let other =
        RecoveryStore::new(&data, &d.owner_scope, &d.installation_id, Some(&[7; 32])).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let c = barrier.clone();
    let p1 = p[1].clone();
    let first = std::thread::spawn(move || {
        c.wait();
        store.publish_temporal(&a, Some(&p1), now(&p1))
    });
    let second = std::thread::spawn(move || {
        barrier.wait();
        other.publish_temporal(&b, Some(&fork), now(&fork))
    });
    let outcomes = [
        first.join().unwrap().is_ok(),
        second.join().unwrap().is_ok(),
    ];
    assert_eq!(outcomes.into_iter().filter(|x| *x).count(), 1);
}
#[test]
fn discard_expiry_and_explicit_new_generation_keep_stale_tokens_fenced() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_new_empty_run;
    let (_t, store, p) = setup();
    let token = commit(&store, None, &p[0]);
    let lock = fs::metadata(store.directory.join(LOCK)).unwrap();
    assert!(store
        .load_temporal(p[0].descriptor().expires_at_ms * 1_000_000)
        .is_err());
    let retired = store
        .prepare_retire_temporal(&token, p[0].descriptor().expires_at_ms * 1_000_000, true)
        .unwrap();
    let tombstone = store
        .publish_temporal(&retired, None, p[0].descriptor().expires_at_ms * 1_000_000)
        .unwrap();
    assert!(store
        .load_temporal(p[0].descriptor().expires_at_ms * 1_000_000)
        .unwrap()
        .unwrap()
        .prefix
        .is_none());
    assert!(!store.temporal_dir().join("slot-0.bin").exists());
    assert!(store
        .prepare_temporal(Some(&token), &p[1], now(&p[1]))
        .is_err());
    let fresh = synthetic_new_empty_run(&p[0], p[0].descriptor().expires_at_ms + 1);
    assert!(store.prepare_temporal(None, &fresh, now(&fresh)).is_err());
    let next = store
        .prepare_temporal(Some(&tombstone), &fresh, now(&fresh))
        .unwrap();
    let next_token = store
        .publish_temporal(&next, Some(&fresh), now(&fresh))
        .unwrap();
    assert_eq!(next_token.generation, tombstone.generation + 1);
    assert_eq!(next_token.revision, 0);
    assert!(store
        .prepare_temporal(Some(&tombstone), &fresh, now(&fresh))
        .is_err());
    assert_eq!(
        fs::read_dir(store.temporal_dir())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("witness-"))
            .count(),
        1
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            lock.ino(),
            fs::metadata(store.directory.join(LOCK)).unwrap().ino()
        );
    }
}
#[test]
fn coordinated_head_and_witness_rollback_is_documented_limit_not_safety_success() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let dir = store.temporal_dir();
    let old_head = fs::read(dir.join(HEAD)).unwrap();
    let t1 = commit(&store, Some(&t0), &p[1]);
    fs::write(dir.join(HEAD), old_head).unwrap();
    fs::remove_file(dir.join(format!(
        "witness-{:016x}-{:04x}.bin",
        t1.generation, t1.revision
    )))
    .unwrap();
    let old = store.load_temporal(now(&p[1])).unwrap().unwrap();
    assert_eq!(
        old.token, t0,
        "deliberately demonstrates excluded coordinated rollback scope"
    );
}
#[test]
fn clock_regression_unknown_entries_and_metadata_capacity_fail_closed() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    assert!(store.load_temporal(now(&p[0]) - 1).is_err());
    assert!(store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]) - 1)
        .is_err());
    assert_eq!(p[0].transition_limit(), 329);
    assert!(p[0].transition_limit() <= MAX_TRANSITIONS);
    let keep = store.temporal_dir().join("unrelated.db");
    fs::write(&keep, b"preserve").unwrap();
    assert!(store.load_temporal(now(&p[0])).is_err());
    assert!(keep.exists());
}
#[test]
fn os_lock_is_shared_with_an_actual_child_process() {
    let (t, store, p) = setup();
    let locked = store.temporal_lock(true).unwrap();
    let marker = t.path().join("child-acquired");
    let ready = t.path().join("child-ready");
    let d = p[0].descriptor();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "room_learner::recovery_store::temporal_commitment::tests::os_lock_child_probe",
            "--nocapture",
        ])
        .env(
            "RUVIEW_SYNTH_LOCK_DATA",
            t.path().join(&d.owner_scope).join("SensingServer"),
        )
        .env("RUVIEW_SYNTH_LOCK_OWNER", &d.owner_scope)
        .env("RUVIEW_SYNTH_LOCK_INSTALL", &d.installation_id)
        .env("RUVIEW_SYNTH_LOCK_READY", &ready)
        .env("RUVIEW_SYNTH_LOCK_ACQUIRED", &marker)
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    while !ready.exists() && started.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::yield_now();
    }
    assert!(ready.exists());
    assert!(
        !marker.exists(),
        "child cannot acquire while parent holds same stable inode"
    );
    drop(locked);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(marker.exists());
}
#[test]
fn os_lock_child_probe() {
    let Some(data) = std::env::var_os("RUVIEW_SYNTH_LOCK_DATA") else {
        return;
    };
    let owner = std::env::var("RUVIEW_SYNTH_LOCK_OWNER").unwrap();
    let installation = std::env::var("RUVIEW_SYNTH_LOCK_INSTALL").unwrap();
    let store =
        RecoveryStore::new(Path::new(&data), &owner, &installation, Some(&[7; 32])).unwrap();
    fs::write(
        std::env::var_os("RUVIEW_SYNTH_LOCK_READY").unwrap(),
        b"ready",
    )
    .unwrap();
    let _lock = store.temporal_lock(true).unwrap();
    fs::write(
        std::env::var_os("RUVIEW_SYNTH_LOCK_ACQUIRED").unwrap(),
        b"acquired",
    )
    .unwrap();
}

#[test]
fn original_native_receiver_nanoseconds_are_never_rounded_for_cold_validation() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let t1 = commit(&store, Some(&t0), &p[1]);
    let _t2 = commit(&store, Some(&t1), &p[2]);
    let exact = p[2].saved_receiver_ns();
    let truncated = p[2].descriptor().saved_at_ms * 1_000_000;
    assert!(exact > truncated);
    assert!(store.load_temporal(truncated).is_err());
    let cold = store.load_temporal(exact).unwrap().unwrap().prefix.unwrap();
    assert_eq!(cold.saved_receiver_ns(), exact);
    assert_eq!(cold.bytes(), p[2].bytes());
}

#[test]
fn valid_looking_lower_background_floor_cannot_extend_committed_prefix() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_floor_regression;
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let t1 = commit(&store, Some(&t0), &p[1]);
    let t2 = commit(&store, Some(&t1), &p[2]);
    let regressed = synthetic_floor_regression(&p[2]);
    let err = store
        .prepare_temporal(Some(&t2), &regressed, now(&regressed))
        .err()
        .expect("floor must not regress despite internal prefix validity");
    assert_eq!(err, "temporal_commit_floor_regressed");
}
#[test]
fn conflicting_pending_payload_cannot_be_overwritten_by_other_same_revision_repair() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let a = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    let fork = synthetic_segment_fork(&p[0]);
    let b = store
        .prepare_temporal(Some(&t0), &fork, now(&fork))
        .unwrap();
    let mut hook = |s: &str| {
        if s == "after:payload.stage:directory_fsync" {
            Err("injected-before-head".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&a, Some(&p[1]), now(&p[1]), false, &mut hook)
        .is_err());
    let before = fs::read(store.temporal_dir().join("slot-1.bin")).unwrap();
    assert!(store
        .repair_same_temporal(&b, Some(&fork), now(&fork))
        .is_err());
    assert_eq!(
        fs::read(store.temporal_dir().join("slot-1.bin")).unwrap(),
        before
    );
    assert!(store
        .repair_same_temporal(&a, Some(&p[1]), now(&p[1]))
        .is_ok());
}
#[test]
fn incomplete_unauthenticated_stage_blocks_repair_but_keeps_prior_commit_loadable() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let intent = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    let file = store.temporal_dir().join("payload.stage");
    let mut hook = |s: &str| {
        if s == "after:payload.stage:write" {
            let mut b = fs::read(&file).unwrap();
            b.truncate(b.len() / 2);
            fs::write(&file, b).unwrap();
            Err("injected-partial-storage-write".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&intent, Some(&p[1]), now(&p[1]), false, &mut hook)
        .is_err());
    let old = store.load_temporal(now(&p[1])).unwrap().unwrap();
    assert_eq!(old.token, t0);
    assert!(
        store
            .repair_same_temporal(&intent, Some(&p[1]), now(&p[1]))
            .is_err(),
        "unverifiable stage cannot be relabeled as same intent"
    );
    let retire = store
        .prepare_retire_temporal(&t0, now(&p[1]), false)
        .unwrap();
    store.publish_temporal(&retire, None, now(&p[1])).unwrap();
    assert!(!file.exists());
}

#[test]
fn missing_head_or_witness_never_reinitializes_from_stale_ram() {
    for target in ["head", "witness"] {
        let (_t, store, p) = setup();
        let t0 = commit(&store, None, &p[0]);
        let path = if target == "head" {
            store.temporal_dir().join(HEAD)
        } else {
            store.temporal_dir().join(format!(
                "witness-{:016x}-{:04x}.bin",
                t0.generation, t0.revision
            ))
        };
        fs::remove_file(path).unwrap();
        assert!(store.load_temporal(now(&p[0])).is_err());
        assert!(store.prepare_temporal(None, &p[0], now(&p[0])).is_err());
        assert!(store
            .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
            .is_err());
    }
}
#[test]
fn no_op_commit_does_not_rewrite_payload_or_consume_revision() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let snapshot = |store: &RecoveryStore| {
        let mut out = std::collections::BTreeMap::new();
        for e in fs::read_dir(store.temporal_dir()).unwrap() {
            let e = e.unwrap();
            out.insert(e.file_name(), fs::read(e.path()).unwrap());
        }
        out
    };
    let before = snapshot(&store);
    let same = store
        .prepare_temporal(Some(&t0), &p[0], now(&p[0]))
        .unwrap();
    assert_eq!(
        store
            .publish_temporal(&same, Some(&p[0]), now(&p[0]))
            .unwrap(),
        t0
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn cleanup_ambiguity_cannot_be_skipped_by_new_generation_or_noop_ack() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_new_empty_run;
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let retirement = store
        .prepare_retire_temporal(&t0, now(&p[1]), false)
        .unwrap();
    let mut hook = |point: &str| {
        if point == "before:cleanup" {
            Err("injected-cleanup-ambiguity".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&retirement, None, now(&p[1]), false, &mut hook)
        .is_err());
    assert!(store.temporal_dir().join("slot-0.bin").exists());
    let retired = store.load_temporal(now(&p[1])).unwrap().unwrap().token;
    let fresh = synthetic_new_empty_run(&p[0], now(&p[1]) / 1_000_000 + 1);
    assert!(store
        .prepare_temporal(Some(&retired), &fresh, now(&fresh))
        .is_err());
    store
        .repair_same_temporal(&retirement, None, now(&p[1]))
        .unwrap();
    let new = store
        .prepare_temporal(Some(&retired), &fresh, now(&fresh))
        .unwrap();
    let mut hook = |point: &str| {
        if point == "before:cleanup" {
            Err("injected-new-generation-cleanup".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&new, Some(&fresh), now(&fresh), false, &mut hook)
        .is_err());
    let loaded = store.load_temporal(now(&fresh)).unwrap().unwrap();
    let noop = store
        .prepare_temporal(Some(&loaded.token), &fresh, now(&fresh))
        .unwrap();
    assert!(store
        .publish_temporal(&noop, Some(&fresh), now(&fresh))
        .is_err());
    store
        .repair_same_temporal(&new, Some(&fresh), now(&fresh))
        .unwrap();
    assert_eq!(
        fs::read_dir(store.temporal_dir())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("witness-"))
            .count(),
        1
    );
}

#[test]
fn explicit_abort_only_uncommitted_stage_preserves_exact_prefix_token_and_expiry() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let intent = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    assert!(store
        .synthetic_partial_write(&intent, &p[1], now(&p[1]))
        .is_err());
    let tickets = store.inspect_abortable_temporal(&t0, now(&p[1])).unwrap();
    let ticket = tickets
        .iter()
        .find(|t| t.artifact == AbortArtifact::PayloadStage)
        .unwrap();
    let returned = store.abort_temporal_pending(ticket, now(&p[1])).unwrap();
    assert_eq!(returned.token, t0);
    let restored = returned.prefix.unwrap();
    assert_eq!(restored.bytes(), p[0].bytes());
    assert_eq!(
        restored.descriptor().expires_at_ms,
        p[0].descriptor().expires_at_ms
    );
    assert!(!store.temporal_dir().join("payload.stage").exists());
    let next = commit(&store, Some(&t0), &p[1]);
    assert_eq!(next.revision, t0.revision + 1);
}
#[test]
fn abort_rejects_stale_digest_token_and_any_committed_reference() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let intent = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    assert!(store
        .synthetic_partial_write(&intent, &p[1], now(&p[1]))
        .is_err());
    let ticket = store
        .inspect_abortable_temporal(&t0, now(&p[1]))
        .unwrap()
        .into_iter()
        .find(|t| t.artifact == AbortArtifact::PayloadStage)
        .unwrap();
    let file = store.temporal_dir().join("payload.stage");
    let mut changed = fs::read(&file).unwrap();
    changed.push(0);
    fs::write(&file, &changed).unwrap();
    assert!(store.abort_temporal_pending(&ticket, now(&p[1])).is_err());
    assert_eq!(fs::read(&file).unwrap(), changed);
    let fresh = store
        .inspect_abortable_temporal(&t0, now(&p[1]))
        .unwrap()
        .into_iter()
        .find(|t| t.artifact == AbortArtifact::PayloadStage)
        .unwrap();
    store.abort_temporal_pending(&fresh, now(&p[1])).unwrap();
    let t1 = commit(&store, Some(&t0), &p[1]);
    assert!(store.abort_temporal_pending(&fresh, now(&p[1])).is_err());
    let dir = store.temporal_dir();
    fs::copy(dir.join("slot-1.bin"), dir.join("payload.stage")).unwrap();
    assert!(!store
        .inspect_abortable_temporal(&t1, now(&p[1]))
        .unwrap()
        .iter()
        .any(|t| t.artifact == AbortArtifact::PayloadStage));
    let forged = TemporalAbortTicket {
        expected: t1,
        artifact: AbortArtifact::InactiveSlot(1),
        digest: hash(&fs::read(dir.join("slot-1.bin")).unwrap()),
    };
    assert!(store.abort_temporal_pending(&forged, now(&p[1])).is_err());
}
#[test]
fn post_head_ambiguity_cannot_abort_to_a_guessed_prior_revision() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let intent = store
        .prepare_temporal(Some(&t0), &p[1], now(&p[1]))
        .unwrap();
    let mut hook = |s: &str| {
        if s == "after:head.stage:directory_fsync" {
            Err("injected-pending-head".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&intent, Some(&p[1]), now(&p[1]), false, &mut hook)
        .is_err());
    assert!(store.inspect_abortable_temporal(&t0, now(&p[1])).is_err());
    assert!(store
        .repair_same_temporal(&intent, Some(&p[1]), now(&p[1]))
        .is_ok());
}

#[test]
fn independent_interrupted_old_generation_gc_is_same_intent_repairable() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_new_empty_run;
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let retirement = store
        .prepare_retire_temporal(&t0, now(&p[1]), false)
        .unwrap();
    let retired = store
        .publish_temporal(&retirement, None, now(&p[1]))
        .unwrap();
    let fresh = synthetic_new_empty_run(&p[0], now(&p[1]) / 1_000_000 + 1);
    let new = store
        .prepare_temporal(Some(&retired), &fresh, now(&fresh))
        .unwrap();
    let mut hook = |point: &str| {
        if point == "before:cleanup" {
            Err("stop-before-gc".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&new, Some(&fresh), now(&fresh), false, &mut hook)
        .is_err());
    assert_eq!(
        store
            .load_temporal(now(&fresh))
            .unwrap()
            .unwrap()
            .prefix
            .unwrap()
            .bytes(),
        fresh.bytes()
    );
    fs::remove_file(store.temporal_dir().join(format!(
        "witness-{:016x}-{:04x}.bin",
        t0.generation, t0.revision
    )))
    .unwrap();
    let repaired = store.repair_same_temporal(&new, Some(&fresh), now(&fresh));
    assert!(
        repaired.is_ok(),
        "same intent repair after one completed GC unlink: {:?}",
        repaired.err()
    );
}

#[test]
fn each_completed_old_gc_unlink_keeps_current_prefix_loadable_and_repairable() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_new_empty_run;
    for cut in 0..=5 {
        let (_t, store, p) = setup();
        let mut token = commit(&store, None, &p[0]);
        for value in &p[1..] {
            token = commit(&store, Some(&token), value);
        }
        let retired_intent = store
            .prepare_retire_temporal(&token, now(&p[3]), false)
            .unwrap();
        let retired = store
            .publish_temporal(&retired_intent, None, now(&p[3]))
            .unwrap();
        let original = synthetic_new_empty_run(&p[0], now(&p[3]) / 1_000_000 + 1);
        let intent = store
            .prepare_temporal(Some(&retired), &original, now(&original))
            .unwrap();
        let mut hook = |point: &str| {
            if point == "before:cleanup" {
                Err("injected-before-obsolete-unlinks".into())
            } else {
                Ok(())
            }
        };
        assert!(store
            .publish_temporal_with(&intent, Some(&original), now(&original), false, &mut hook)
            .is_err());
        let components = store.temporal_components(&store.temporal_dir()).unwrap();
        let old = components
            .witnesses
            .iter()
            .filter(|(_, m)| m.generation == retired.generation)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        assert_eq!(old.len(), 5);
        for name in old.iter().take(cut) {
            fs::remove_file(store.temporal_dir().join(name)).unwrap();
        }
        let current = store.load_temporal(now(&original)).unwrap().unwrap();
        assert_eq!(
            current.prefix.as_ref().unwrap().bytes(),
            original.bytes(),
            "cut{cut}"
        );
        let repaired = store
            .repair_same_temporal(&intent, Some(&original), now(&original))
            .unwrap();
        assert_eq!(repaired, current.token);
        let loaded = store
            .load_temporal(now(&original))
            .unwrap()
            .unwrap()
            .prefix
            .unwrap();
        assert_eq!(loaded.descriptor(), original.descriptor());
        assert_eq!(loaded.bytes(), original.bytes());
        assert_eq!(
            store
                .temporal_components(&store.temporal_dir())
                .unwrap()
                .witnesses
                .len(),
            1
        );
    }
}
#[test]
fn obsolete_gc_fragments_never_authorize_old_head_or_incomplete_current_chain() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_new_empty_run;
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let retire = store
        .prepare_retire_temporal(&t0, now(&p[1]), false)
        .unwrap();
    let retired = store.publish_temporal(&retire, None, now(&p[1])).unwrap();
    let old_head = fs::read(store.temporal_dir().join(HEAD)).unwrap();
    let fresh = synthetic_new_empty_run(&p[0], now(&p[1]) / 1_000_000 + 1);
    let intent = store
        .prepare_temporal(Some(&retired), &fresh, now(&fresh))
        .unwrap();
    assert!(store
        .publish_temporal_with(&intent, Some(&fresh), now(&fresh), false, &mut |s| if s
            == "before:cleanup"
        {
            Err("pause-gc".into())
        } else {
            Ok(())
        })
        .is_err());
    fs::remove_file(
        store
            .temporal_dir()
            .join(format!("witness-{:016x}-{:04x}.bin", t0.generation, 0)),
    )
    .unwrap();
    fs::write(store.temporal_dir().join(HEAD), old_head).unwrap();
    assert!(store.load_temporal(now(&fresh)).is_err());
    assert!(store.inspect_temporal_retirement(now(&fresh)).is_err());
    store
        .repair_same_temporal(&intent, Some(&fresh), now(&fresh))
        .unwrap();
    let current = store.load_temporal(now(&fresh)).unwrap().unwrap().token;
    fs::remove_file(
        store
            .temporal_dir()
            .join(format!("witness-{:016x}-{:04x}.bin", current.generation, 0)),
    )
    .unwrap();
    assert!(store.load_temporal(now(&fresh)).is_err());
    assert!(store.inspect_temporal_retirement(now(&fresh)).is_err());
}
#[test]
fn cold_expired_handle_can_inspect_only_metadata_retire_and_begin_new_empty_run() {
    use crate::room_learner::temporal_model_input::managed::execution::checkpoint::synthetic_new_empty_run;
    let (t, store, p) = setup();
    commit(&store, None, &p[0]);
    let d = p[0].descriptor().clone();
    drop(store);
    let cold = RecoveryStore::new(
        &t.path().join(&d.owner_scope).join("SensingServer"),
        &d.owner_scope,
        &d.installation_id,
        Some(&[7; 32]),
    )
    .unwrap();
    let expired = d.expires_at_ms * 1_000_000;
    assert!(cold.load_temporal(expired).is_err());
    let inspection = cold.inspect_temporal_retirement(expired).unwrap().unwrap();
    assert!(inspection.expired());
    assert_eq!(inspection.descriptor(), &d);
    assert_eq!(inspection.run(), p[0].run());
    assert!(cold
        .prepare_inspected_retirement(&inspection, expired - 1, true)
        .is_err());
    let retire = cold
        .prepare_inspected_retirement(&inspection, expired, true)
        .unwrap();
    let retired = cold.publish_temporal(&retire, None, expired).unwrap();
    assert!(cold
        .load_temporal(expired)
        .unwrap()
        .unwrap()
        .prefix
        .is_none());
    let fresh = synthetic_new_empty_run(&p[0], d.expires_at_ms + 1);
    commit(&cold, Some(&retired), &fresh);
    assert!(cold
        .prepare_inspected_retirement(&inspection, now(&fresh), false)
        .is_err());
}
#[test]
fn metadata_inspection_never_blesses_missing_corrupt_or_foreign_head() {
    for target in ["missing", "corrupt"] {
        let (_t, store, p) = setup();
        commit(&store, None, &p[0]);
        let file = store.temporal_dir().join(HEAD);
        if target == "missing" {
            fs::remove_file(file).unwrap();
        } else {
            let mut b = fs::read(&file).unwrap();
            let i = b.len() - 1;
            b[i] ^= 1;
            fs::write(file, b).unwrap();
        }
        assert!(store
            .inspect_temporal_retirement(p[0].descriptor().expires_at_ms * 1_000_000)
            .is_err());
    }
}
#[test]
fn visible_lost_ack_requires_real_recertification_before_durable_result() {
    let (_t, store, p) = setup();
    let t0 = commit(&store, None, &p[0]);
    let t1 = commit(&store, Some(&t0), &p[1]);
    let intent = store
        .prepare_temporal(Some(&t1), &p[2], now(&p[2]))
        .unwrap();
    let mut hook = |point: &str| {
        if point == "after:witness:install" {
            Err("injected-lost-witness-directory-ack".into())
        } else {
            Ok(())
        }
    };
    assert!(store
        .publish_temporal_with(&intent, Some(&p[2]), now(&p[2]), false, &mut hook)
        .is_err());
    let observed = store.load_temporal(now(&p[2])).unwrap().unwrap();
    assert_eq!(observed.prefix.as_ref().unwrap().bytes(), p[2].bytes());
    for fail in [
        "before:restore:file_fsync",
        "after:restore:file_fsync",
        "before:restore:directory_fsync",
        "after:restore:directory_fsync",
        "before:restore:readback",
        "after:restore:readback",
    ] {
        assert!(
            store
                .recertify_temporal_with(&observed.token, now(&p[2]), &mut |point| if point == fail
                {
                    Err("injected-restore-barrier".into())
                } else {
                    Ok(())
                })
                .is_err(),
            "{fail}"
        );
    }
    let certified = store
        .recertify_temporal(&observed.token, now(&p[2]))
        .unwrap();
    assert_eq!(certified.token, observed.token);
    assert_eq!(certified.prefix.bytes(), p[2].bytes());
}

#[test]
fn metadata_only_cold_retirement_rejects_copied_foreign_owner_records() {
    let (t, store, p) = setup();
    commit(&store, None, &p[0]);
    let owner = "b".repeat(64);
    let foreign = RecoveryStore::new(
        &t.path().join(&owner).join("SensingServer"),
        &owner,
        &p[0].descriptor().installation_id,
        Some(&[7; 32]),
    )
    .unwrap();
    drop(foreign.temporal_lock(true).unwrap());
    for entry in fs::read_dir(store.temporal_dir()).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == HEAD || name.to_string_lossy().starts_with("witness-") {
            fs::copy(entry.path(), foreign.temporal_dir().join(name)).unwrap();
        }
    }
    assert!(foreign
        .inspect_temporal_retirement(p[0].descriptor().expires_at_ms * 1_000_000)
        .is_err());
}
