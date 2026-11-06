//! SYNTHETIC guarded enable without a device, clock change or capture reset.
use super::*;
use crate::room_learner::recovery_store::RECOVERY_TTL_MS;
fn token(rt: &RoomLearningRuntime, now: u64) -> String {
    rt.status(now)["recovery"]["enable_current_run"]["run_id"]
        .as_str()
        .unwrap()
        .into()
}
fn ram_run(root: &Path) -> Arc<RoomLearningRuntime> {
    let rt = runtime(root, BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "new-ram".into(),
        true,
        false,
        BASE,
    )
    .unwrap();
    rt
}
fn completed(rt: &RoomLearningRuntime) -> u64 {
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(12_000), BASE)
        .unwrap();
    for q in 0..4 {
        window(
            rt,
            BASE + u64::from(q) * 3_000_000_000 + 1_000_000,
            q * 8 + 1,
        );
    }
    let at = BASE + 12_000_000_000;
    rt.finish_phase(&rt.mutation_guard().unwrap(), at, true)
        .unwrap();
    at
}
#[test]
fn current_new_run_enable_preserves_exact_rows_history_watermarks_and_cold_restores() {
    let dir = tempfile::tempdir().unwrap();
    let rt = ram_run(dir.path());
    let at = completed(&rt);
    let id = token(&rt, at);
    let before = rt
        .inner
        .lock()
        .unwrap()
        .legacy_capture()
        .unwrap()
        .checkpoint(at)
        .unwrap();
    rt.enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, at)
        .unwrap();
    let after = rt
        .inner
        .lock()
        .unwrap()
        .legacy_capture()
        .unwrap()
        .checkpoint(at)
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(&after).unwrap()
    );
    let status = rt.status(at);
    assert_eq!(status["capture"]["sealed_cohorts"], 4);
    assert_eq!(status["recovery"]["enabled"], true);
    assert!(status["candidate_loaded"] == false);
    assert_eq!(
        status["recovery"]["saved"]["created_at_ms"],
        BASE / 1_000_000
    );
    assert_eq!(
        status["recovery"]["saved"]["expires_at_ms"],
        BASE / 1_000_000 + RECOVERY_TTL_MS
    );
    let cold = runtime(dir.path(), at + 1, true);
    cold.restore_capture_recovery(&cold.mutation_guard().unwrap(), &id, &context(), at + 1)
        .unwrap();
    assert_eq!(cold.status(at + 1)["capture"]["sealed_cohorts"], 4);
    assert!(cold
        .enable_current_capture_recovery(&cold.mutation_guard().unwrap(), &id, true, at + 2)
        .is_err());
}
#[test]
fn wrong_ticket_consent_stale_guard_and_active_phase_do_not_enable_or_change_capture() {
    let dir = tempfile::tempdir().unwrap();
    let rt = ram_run(dir.path());
    let id = token(&rt, BASE);
    let stale = rt.mutation_guard().unwrap();
    assert!(rt
        .enable_current_capture_recovery(&stale, &id, false, BASE)
        .is_err());
    assert!(rt
        .enable_current_capture_recovery(&stale, "wrong", true, BASE)
        .is_err());
    assert!(rt
        .enable_current_capture_recovery(&stale, &id, true, BASE)
        .is_err());
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(12_000), BASE)
        .unwrap();
    let before = rt
        .inner
        .lock()
        .unwrap()
        .legacy_capture()
        .unwrap()
        .checkpoint(BASE)
        .unwrap();
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, BASE)
        .is_err());
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(
            &rt.inner
                .lock()
                .unwrap()
                .legacy_capture()
                .unwrap()
                .checkpoint(BASE)
                .unwrap()
        )
        .unwrap()
    );
    assert!(rt
        .recovery_store
        .as_ref()
        .unwrap()
        .inspect()
        .unwrap()
        .is_none());
}
#[test]
fn original_new_run_deadline_never_renews_even_after_regressed_time_or_discard() {
    let dir = tempfile::tempdir().unwrap();
    let rt = ram_run(dir.path());
    let id = token(&rt, BASE);
    let deadline = BASE + RECOVERY_TTL_MS * 1_000_000;
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, deadline)
        .is_err());
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, deadline - 1)
        .is_err());
    assert_eq!(
        rt.status(deadline - 1)["recovery"]["enable_current_run"]["eligible"],
        false
    );
    let second = tempfile::tempdir().unwrap();
    let rt = ram_run(second.path());
    let id = token(&rt, BASE);
    rt.enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, BASE)
        .unwrap();
    rt.discard_capture_recovery(&rt.mutation_guard().unwrap(), &id)
        .unwrap();
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, BASE + 1)
        .is_err());
}
#[test]
fn foreign_and_expired_slots_are_never_overwritten_and_new_run_stays_intact() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path(), BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "old".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    let retained = rt
        .recovery_store
        .as_ref()
        .unwrap()
        .load(BASE / 1_000_000)
        .unwrap()
        .unwrap();
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "new".into(),
        true,
        false,
        BASE + 1,
    )
    .unwrap();
    let id = token(&rt, BASE + 1);
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, BASE + 2)
        .is_err());
    let again = rt
        .recovery_store
        .as_ref()
        .unwrap()
        .load(BASE / 1_000_000)
        .unwrap()
        .unwrap();
    assert_eq!(again.descriptor, retained.descriptor);
    assert_eq!(again.plaintext, retained.plaintext);
    let later = BASE + (RECOVERY_TTL_MS + 1) * 1_000_000;
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "later-new".into(),
        true,
        false,
        later,
    )
    .unwrap();
    let id = token(&rt, later);
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, later)
        .is_err());
    assert_eq!(
        rt.recovery_store
            .as_ref()
            .unwrap()
            .inspect()
            .unwrap()
            .unwrap(),
        retained.descriptor
    );
}
#[test]
fn write_failure_keeps_ram_and_ticket_then_explicit_retry_can_succeed_without_reset() {
    let dir = tempfile::tempdir().unwrap();
    let rt = ram_run(dir.path());
    let at = completed(&rt);
    let id = token(&rt, at);
    let path = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer")
        .join("room-learner-recovery")
        .join("capture-recovery-v1.bin");
    std::fs::create_dir(&path).unwrap();
    let before = serde_json::to_vec(
        &rt.inner
            .lock()
            .unwrap()
            .legacy_capture()
            .unwrap()
            .checkpoint(at)
            .unwrap(),
    )
    .unwrap();
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, at)
        .is_err());
    assert!(!rt.inner.lock().unwrap().recovery.enabled);
    assert!(rt.status(at)["recovery"]["derived_samples_persisted"].is_null());
    assert_eq!(
        serde_json::to_vec(
            &rt.inner
                .lock()
                .unwrap()
                .legacy_capture()
                .unwrap()
                .checkpoint(at)
                .unwrap()
        )
        .unwrap(),
        before
    );
    std::fs::remove_dir(&path).unwrap();
    rt.enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, at + 1)
        .unwrap();
    assert!(rt.inner.lock().unwrap().recovery.enabled);
}

#[test]
fn concurrent_enable_consumes_one_guard_and_cannot_renew_the_ticket() {
    let dir = tempfile::tempdir().unwrap();
    let rt = ram_run(dir.path());
    let id = token(&rt, BASE);
    let guard = rt.mutation_guard().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let rt = rt.clone();
            let id = id.clone();
            let guard = guard.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                rt.enable_current_capture_recovery(&guard, &id, true, BASE)
            })
        })
        .collect();
    barrier.wait();
    let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    let saved = rt
        .recovery_store
        .as_ref()
        .unwrap()
        .load(BASE / 1_000_000)
        .unwrap()
        .unwrap();
    assert_eq!(saved.descriptor.id, id);
    assert_eq!(
        saved.descriptor.expires_at_ms,
        BASE / 1_000_000 + RECOVERY_TTL_MS
    );
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, BASE + 1)
        .is_err());
}
#[test]
fn partial_committed_same_ticket_is_retried_with_unchanged_original_deadline() {
    use super::super::recovery_control::{RuntimeCheckpoint, SCHEMA};
    use crate::room_learner::recovery_store::{RecoveryDescriptor, RECOVERY_SCHEMA};
    let dir = tempfile::tempdir().unwrap();
    let rt = ram_run(dir.path());
    let at = completed(&rt);
    let id = token(&rt, at);
    let payload = RuntimeCheckpoint {
        schema: SCHEMA.into(),
        capture: rt
            .inner
            .lock()
            .unwrap()
            .legacy_capture()
            .unwrap()
            .checkpoint(at)
            .unwrap(),
        frozen_training: false,
        job_phase: JobPhase::Idle,
        model_sha256: None,
    };
    let descriptor = RecoveryDescriptor {
        schema: RECOVERY_SCHEMA.into(),
        id: id.clone(),
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-install".into(),
        context_digest: context().digest(),
        created_at_ms: BASE / 1_000_000,
        saved_at_ms: at / 1_000_000,
        expires_at_ms: BASE / 1_000_000 + RECOVERY_TTL_MS,
    };
    // Model a prior call whose rename committed but whose final barrier failed:
    // durable bytes exist, yet the runtime never claimed persistence enabled.
    rt.recovery_store
        .as_ref()
        .unwrap()
        .save(
            &descriptor,
            &serde_json::to_vec(&payload).unwrap(),
            at / 1_000_000,
        )
        .unwrap();
    assert!(!rt.inner.lock().unwrap().recovery.enabled);
    rt.enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, at + 1_000_000)
        .unwrap();
    let stored = rt
        .recovery_store
        .as_ref()
        .unwrap()
        .load((at + 1_000_000) / 1_000_000)
        .unwrap()
        .unwrap();
    assert_eq!(stored.descriptor.created_at_ms, descriptor.created_at_ms);
    assert_eq!(stored.descriptor.expires_at_ms, descriptor.expires_at_ms);
    assert_eq!(rt.status(at + 1_000_000)["capture"]["sealed_cohorts"], 4);
}
#[test]
fn unavailable_key_and_other_runtime_guard_cannot_enable() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path(), BASE, false);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "ram-no-key".into(),
        true,
        false,
        BASE,
    )
    .unwrap();
    let id = token(&rt, BASE);
    assert!(rt
        .enable_current_capture_recovery(&rt.mutation_guard().unwrap(), &id, true, BASE)
        .is_err());
    assert!(!rt.inner.lock().unwrap().recovery.enabled);
    let otherdir = tempfile::tempdir().unwrap();
    let other = ram_run(otherdir.path());
    assert!(other
        .enable_current_capture_recovery(
            &rt.mutation_guard().unwrap(),
            &token(&other, BASE),
            true,
            BASE
        )
        .is_err());
    assert!(other
        .recovery_store
        .as_ref()
        .unwrap()
        .inspect()
        .unwrap()
        .is_none());
}
