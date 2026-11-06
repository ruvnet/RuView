//! SYNTHETIC simulated receiver clocks; no sleeps or physical sensing claims.
use super::super::tests::{context, packet};
use super::*;
use crate::room_learner::managed_schedule_contract::LeadInPolicy;
pub(super) fn setup() -> (tempfile::TempDir, RecoveryStore, TemporalWorkflow) {
    let dir = tempfile::tempdir().unwrap();
    let c = context();
    let data = dir.path().join(&c.owner_scope).join("SensingServer");
    let store =
        RecoveryStore::new(&data, &c.owner_scope, &c.installation_id, Some(&[7; 32])).unwrap();
    let plan =
        ManagedScheduleContract::build(c, LeadInPolicy::Train5HeldOut15EngineeringV1).unwrap();
    let w = TemporalWorkflow::prepare(
        plan,
        "synthetic-prefix".into(),
        FitConfig {
            epochs: 2,
            ..Default::default()
        },
        1_000_000_000,
    )
    .unwrap();
    (dir, store, w)
}
pub(super) fn start(w: &mut TemporalWorkflow, store: &RecoveryStore, time: u64, epoch: &str) {
    let permit = w.synthetic_conditions();
    w.start_segment(&permit, epoch, time, time).unwrap();
    assert!(
        w.begin_slot().is_err(),
        "segment must be durable before collection"
    );
    w.persist_segment(store).unwrap();
}
pub(super) fn drive(w: &mut TemporalWorkflow, from: u64, to: u64, seq: &mut u32) -> u64 {
    let period = 1_000_000_000u64.div_ceil(33);
    let mut at = from;
    while at <= to + period {
        *seq += 1;
        for source in 1..=3u8 {
            let t = at + u64::from(source) * 1000;
            w.observe(&packet(source, *seq, t, RoomLabel::Empty, 0), t)
                .unwrap();
        }
        at += period;
    }
    at
}
pub(super) fn commit_next(
    w: &mut TemporalWorkflow,
    store: &RecoveryStore,
    from: u64,
    seq: &mut u32,
) -> u64 {
    let target = w.begin_slot().unwrap();
    let next = drive(w, from, target.end_ns(), seq);
    w.commit_slot(store, w.last_now_ns).unwrap();
    next
}
#[test]
fn actual_600second_episode_interruption_keeps_only_durable_prefix_and_original_rows() {
    let (_dir, store, mut w) = setup();
    assert!(w.begin_slot().is_err());
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "epoch-one");
    let mut seq = 0;
    let mut next = 2_000_000_000;
    for _ in 0..3 {
        next = commit_next(&mut w, &store, next, &mut seq);
    }
    let saved_rows = serde_json::to_vec(&w.session.rows).unwrap();
    let old_ledger = w.ledger.clone();
    let deadline = w.descriptor.expires_at_ms;
    let accepted = w.accepted();
    let original_trial = w.session.rows[0].trial_id.clone();
    let old_session = w.session.rows[0].session_id.clone();
    let partial = w.begin_slot().unwrap();
    let next = drive(&mut w, next, partial.target_ns() + 1_000_000_000, &mut seq);
    assert!(w.session.active);
    assert!(w.live_observed_ns > accepted);
    let before_high = w.source_high.clone();
    w.pause().unwrap();
    assert_eq!(w.accepted(), accepted);
    assert_eq!(w.source_high, before_high);
    assert_eq!(serde_json::to_vec(&w.session.rows).unwrap(), saved_rows);
    let resumed = next + 300_000_000_000;
    start(&mut w, &store, resumed, "epoch-two");
    assert_eq!(w.ledger.gaps.len(), 1);
    assert!(w.ledger.gaps[0].discarded_live_ns > 0);
    assert_eq!(w.ledger.gaps[0].accepted_observed_ns, accepted);
    assert_eq!(w.ledger.spans, old_ledger.spans);
    assert_eq!(w.mapping.as_ref().unwrap().observed_start_ns, accepted);
    let newtarget = w.begin_slot().unwrap();
    assert_eq!(
        newtarget.target_ns(),
        resumed + w.schedule().unwrap().episodes()[0].slots[3].target_offset_ms * 1_000_000
            - accepted
    );
    assert_eq!(newtarget.identity().trial_id, original_trial);
    assert_ne!(newtarget.identity().session_id, old_session);
    let mut next = drive(&mut w, resumed, newtarget.end_ns(), &mut seq);
    w.commit_slot(&store, w.last_now_ns).unwrap();
    while w.session.rows.len() < 16 {
        next = commit_next(&mut w, &store, next, &mut seq);
    }
    assert_eq!(w.accepted(), 600_000_000_000);
    assert_eq!(w.descriptor.expires_at_ms, deadline);
    assert_eq!(
        serde_json::to_vec(&w.session.rows[..3]).unwrap(),
        saved_rows
    );
    assert!(w.last_now_ns - resumed < 600_000_000_000);
    assert!(w.session.fit().is_err());
    let mut cold = TemporalWorkflow::cold_restore(
        &store,
        &context(),
        w.last_now_ns + 1_000_000,
        "epoch-three",
        w.last_now_ns + 1_000_000,
    )
    .unwrap();
    assert_eq!(cold.accepted(), 600_000_000_000);
    assert_eq!(
        serde_json::to_vec(&cold.session.rows).unwrap(),
        serde_json::to_vec(&w.session.rows).unwrap()
    );
    assert_eq!(cold.descriptor.expires_at_ms, deadline);
    assert_eq!(cold.stage, Stage::PausedNeedsReconfirmation);
    assert!(cold.session.fit().is_err());
}
#[test]
fn complete_native_ready_does_not_commit_unobserved_reservation_or_downtime() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let target = w.begin_slot().unwrap();
    let mut seq = 0;
    drive(
        &mut w,
        2_000_000_000,
        target.target_ns() + 2_100_000_000,
        &mut seq,
    );
    assert!(!w.session.active);
    assert!(w.commit_slot(&store, w.last_now_ns).is_err());
    assert!(w.ledger.spans.is_empty());
    assert_eq!(w.accepted(), 0);
}
#[test]
fn explicit_pause_and_cold_restore_preserve_fences_reject_counter_epoch_and_clock_rescue() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let last = w.last_now_ns;
    assert!(TemporalWorkflow::cold_restore(
        &store,
        &context(),
        last + 1_000_000,
        "bad-clock",
        last - 1
    )
    .is_err());
    w.pause().unwrap();
    let permit = w.synthetic_conditions();
    assert!(w.start_segment(&permit, "two", last - 1, last + 1).is_err());
    start(&mut w, &store, next + 1_000_000_000, "two");
    let t = w.begin_slot().unwrap();
    assert!(w
        .observe(
            &packet(1, 1, t.target_ns(), RoomLabel::Empty, 0),
            t.target_ns()
        )
        .is_err());
    let mut foreign = context();
    foreign.room_digest = "foreign-room".into();
    assert!(TemporalWorkflow::cold_restore(
        &store,
        &foreign,
        w.last_now_ns + 1_000_000,
        "three",
        w.last_now_ns + 1_000_000
    )
    .is_err());
}

#[test]
fn missing_full_period_packets_and_stale_confirmation_cannot_turn_time_into_observation() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let old = w.synthetic_conditions();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let accepted = w.accepted();
    let bad_at = next + 10_000_000_000;
    assert!(w
        .observe(&packet(1, seq + 1, bad_at, RoomLabel::Empty, 0), bad_at)
        .is_err());
    assert_eq!(w.accepted(), accepted);
    w.pause().unwrap();
    assert!(w
        .start_segment(&old, "two", bad_at + 1, bad_at + 1)
        .is_err());
}

#[test]
fn two_successive_aead_cold_resumes_extend_prefix_without_renewing_descriptor() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let identity = (
        w.descriptor.id.clone(),
        w.descriptor.created_at_ms,
        w.descriptor.expires_at_ms,
    );
    let original = serde_json::to_vec(&w.session.rows).unwrap();
    for epoch in ["two", "three"] {
        let at = w.last_now_ns + 10_000_000_000;
        w = TemporalWorkflow::cold_restore(&store, &context(), at, epoch, at).unwrap();
        start(&mut w, &store, at + 1_000_000_000, epoch);
        commit_next(&mut w, &store, at + 1_000_000_000, &mut seq);
        assert_eq!(
            (
                w.descriptor.id.clone(),
                w.descriptor.created_at_ms,
                w.descriptor.expires_at_ms
            ),
            identity
        );
    }
    assert_eq!(w.session.rows.len(), 3);
    assert_eq!(w.ledger.gaps.len(), 2);
    assert_eq!(serde_json::to_vec(&w.session.rows[..1]).unwrap(), original);
}

/// EXECUTED LIMITATION, not a security success: AEAD authenticates bytes but
/// published593 RecoveryStore has no independent latest-revision anchor.
#[test]
fn legacy_v1_authenticated_slot_remains_unchanged_and_has_no_new_freshness_claim() {
    let (dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let c = context();
    let file = dir
        .path()
        .join(&c.owner_scope)
        .join("SensingServer/room-learner-recovery/capture-recovery-v1.bin");
    store
        .save(
            &w.descriptor,
            w.durable.as_ref().unwrap(),
            w.last_now_ns / 1_000_000,
        )
        .unwrap();
    let old = std::fs::read(&file).unwrap();
    commit_next(&mut w, &store, next, &mut seq);
    assert_eq!(w.session.rows.len(), 2);
    store
        .save(
            &w.descriptor,
            w.durable.as_ref().unwrap(),
            w.last_now_ns / 1_000_000,
        )
        .unwrap();
    std::fs::write(&file, &old).unwrap();
    let now = w.last_now_ns + 1_000_000;
    let legacy = store.load(now / 1_000_000).unwrap().unwrap();
    let mut restored = checkpoint::decode(&legacy.plaintext, &legacy.descriptor, now).unwrap();
    assert_eq!(restored.session.rows.len(),1,"V1 legacy API bytes/meaning remain unchanged; typed temporal path uses independent commitments");
    assert!(restored.session.fit().is_err());
}

#[test]
fn independent_resumed_segment_cannot_credit_period_before_all_sources_arrive() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "initial");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let original = w.accepted();
    w.pause().unwrap();
    let resume = next + 10_000_000_000;
    start(&mut w, &store, resume, "resumed");
    let target = w.begin_slot().unwrap();
    assert!(target.target_ns() - resume > 1_000_000_000);
    let period = 1_000_000_000u64.div_ceil(33);
    let mut at = resume;
    let mut node3_started = false;
    while at <= target.end_ns() + period {
        seq += 1;
        for source in 1..=3u8 {
            if source == 3 && at < target.target_ns() {
                continue;
            }
            if source == 3 {
                node3_started = true;
            }
            let t = at + u64::from(source) * 1000;
            let result = w.observe(&packet(source, seq, t, RoomLabel::Empty, 0), t);
            if result.is_err() {
                assert_eq!(w.accepted(), original);
                return;
            }
        }
        at += period;
    }
    assert!(node3_started);
    assert!(!w.session.active);
    let result = w.commit_slot(&store, w.last_now_ns);
    assert!(
        result.is_err(),
        "missing source before sparse window was credited as a fully observed reservation"
    );
    assert_eq!(w.accepted(), original);
}

#[test]
fn initial_segment_missing_source_is_rejected_at_native_first_arrival_limit() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let start_ns = 2_000_000_000;
    start(&mut w, &store, start_ns, "initial");
    let limit = w
        .session
        .managed
        .as_ref()
        .unwrap()
        .binding
        .native
        .max_first_packet_delay_ms()
        * 1_000_000;
    assert!(w.observe_tick(start_ns + limit).is_ok());
    assert_eq!(w.live_observed_ns, 0);
    assert_eq!(w.accepted(), 0);
    assert!(w.observe_tick(start_ns + limit + 1).is_err());
    assert!(w.custody.failed);
    assert_eq!(w.live_observed_ns, 0);
    assert_eq!(w.accepted(), 0);
    assert!(w
        .observe(
            &packet(3, 1, start_ns + limit + 2, RoomLabel::Empty, 0),
            start_ns + limit + 2
        )
        .is_err());
}

#[test]
fn busy_sources_detect_dropout_between_sparse_windows_and_preserve_prefix_floors() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "initial");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let accepted = w.accepted();
    let raw = w.live_observed_ns;
    let third_floor = w
        .source_high
        .iter()
        .find(|(id, _, _)| *id == 3)
        .copied()
        .unwrap();
    let saved = serde_json::to_vec(&w.session.rows).unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    let mut rejected = false;
    for i in 0..5 {
        seq += 1;
        for source in 1..=2u8 {
            let at = next + i * period + u64::from(source) * 1000;
            if w.observe(&packet(source, seq, at, RoomLabel::Empty, 0), at)
                .is_err()
            {
                rejected = true;
                break;
            }
        }
        if rejected {
            break;
        }
    }
    assert!(
        rejected,
        "busy nodes must detect absent third node between retained windows"
    );
    assert!(w.custody.failed);
    assert_eq!(w.accepted(), accepted);
    let known_frontier = w.mapping.as_ref().unwrap().observed_start_ns + third_floor.2
        - w.mapping.as_ref().unwrap().receiver_start_ns;
    assert!(
        w.live_observed_ns >= raw && w.live_observed_ns <= known_frontier,
        "busy sources cannot advance beyond the absent source's last actual receiver timestamp"
    );
    let failed_live = w.live_observed_ns;
    assert_eq!(
        w.source_high.iter().find(|(id, _, _)| *id == 3).copied(),
        Some(third_floor)
    );
    let at = w.last_now_ns + period;
    assert!(w
        .observe(&packet(3, seq + 1, at, RoomLabel::Empty, 0), at)
        .is_err());
    assert_eq!(w.live_observed_ns, failed_live);
    w.pause().unwrap();
    assert_eq!(w.accepted(), accepted);
    assert_eq!(serde_json::to_vec(&w.session.rows).unwrap(), saved);
    assert!(w
        .source_high
        .iter()
        .find(|(id, _, _)| *id == 3)
        .is_some_and(|(_, s, t)| *s >= third_floor.1 && *t >= third_floor.2));
}

#[test]
fn receiver_tick_withdraws_stale_segment_without_advancing_observed_time() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "initial");
    let mut seq = 0;
    drive(&mut w, 2_000_000_000, 2_000_000_000, &mut seq);
    let raw = w.live_observed_ns;
    let gap = w
        .session
        .managed
        .as_ref()
        .unwrap()
        .binding
        .native
        .source_policies()[0]
        .max_gap_ns;
    assert!(w.observe_tick(w.last_now_ns + gap + 1).is_err());
    assert!(w.custody.failed);
    assert_eq!(w.live_observed_ns, raw);
    assert_eq!(w.accepted(), 0);
}

#[test]
fn initial_source_skew_uses_native_limit_even_within_first_arrival_deadline() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let start_ns = 2_000_000_000;
    start(&mut w, &store, start_ns, "initial");
    for row in 0..3u32 {
        for source in 1..=2u8 {
            let at = start_ns + u64::from(row) * 30_000_000 + u64::from(source) * 1000;
            w.observe(&packet(source, row + 1, at, RoomLabel::Empty, 0), at)
                .unwrap();
        }
    }
    let at = start_ns + 75_000_000;
    assert!(w
        .observe(&packet(3, 3, at, RoomLabel::Empty, 0), at)
        .is_err());
    assert!(w.custody.failed);
    assert_eq!(w.live_observed_ns, 0);
    assert_eq!(w.accepted(), 0);
    assert!(!w.source_high.iter().any(|(id, _, _)| *id == 3));
}

#[test]
fn independent_zero_prefix_partial_pause_then_saved_segment_cold_restores() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let id = w.descriptor.id.clone();
    let expiry = w.descriptor.expires_at_ms;
    start(&mut w, &store, 2_000_000_000, "zero-initial");
    let target = w.begin_slot().unwrap();
    let mut seq = 0;
    let next = drive(
        &mut w,
        2_000_000_000,
        target.target_ns() + 1_000_000_000,
        &mut seq,
    );
    assert!(w.session.active);
    assert!(w.session.rows.is_empty());
    let floors = w.source_high.clone();
    w.pause().unwrap();
    assert_eq!(w.accepted(), 0);
    assert!(w.session.rows.is_empty());
    let resume = next + 10_000_000_000;
    start(&mut w, &store, resume, "zero-resumed");
    assert_eq!(w.ledger.gaps.len(), 1);
    assert!(w.ledger.gaps[0].after_prefix_sha256.is_none());
    let at = w.last_now_ns + 1_000_000;
    let restored = TemporalWorkflow::cold_restore(&store, &context(), at, "zero-cold", at)
        .expect("legitimate zero-prefix pause gap must cold-restore without inventing rows");
    assert!(restored.session.rows.is_empty());
    assert_eq!(restored.accepted(), 0);
    assert_eq!(restored.descriptor.id, id);
    assert_eq!(restored.descriptor.expires_at_ms, expiry);
    for floor in floors {
        assert!(restored
            .source_high
            .iter()
            .any(|s| s.0 == floor.0 && s.1 >= floor.1 && s.2 >= floor.2));
    }
    assert_eq!(restored.stage, Stage::PausedNeedsReconfirmation);
}

#[test]
fn genesis_history_cold_restores_before_and_after_the_first_committed_window() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let original = (
        w.descriptor.id.clone(),
        w.descriptor.created_at_ms,
        w.descriptor.expires_at_ms,
    );
    start(&mut w, &store, 2_000_000_000, "initial");
    let mut seq = 0;
    let target = w.begin_slot().unwrap();
    let next = drive(
        &mut w,
        2_000_000_000,
        target.target_ns() + 1_000_000_000,
        &mut seq,
    );
    let floors = w.source_high.clone();
    w.pause().unwrap();
    start(&mut w, &store, next + 10_000_000_000, "resume-one");
    let now = w.last_now_ns + 1_000_000;
    w = TemporalWorkflow::cold_restore(&store, &context(), now, "cold-one", now).unwrap();
    assert_eq!(w.accepted(), 0);
    assert_eq!(w.session.rows.len(), 0);
    let resume = now + 1_000_000_000;
    start(&mut w, &store, resume, "resume-two");
    assert_eq!(w.ledger.gaps.len(), 2);
    assert!(w
        .ledger
        .gaps
        .iter()
        .all(|g| g.genesis.is_some() && g.after_prefix_sha256.is_none()));
    commit_next(&mut w, &store, resume, &mut seq);
    let committed = serde_json::to_vec(&w.session.rows).unwrap();
    let now = w.last_now_ns + 1_000_000;
    let cold = TemporalWorkflow::cold_restore(&store, &context(), now, "cold-two", now).unwrap();
    assert_eq!(cold.session.rows.len(), 1);
    assert_eq!(serde_json::to_vec(&cold.session.rows).unwrap(), committed);
    assert_eq!(cold.ledger.gaps.len(), 2);
    assert_eq!(
        (
            cold.descriptor.id.clone(),
            cold.descriptor.created_at_ms,
            cold.descriptor.expires_at_ms
        ),
        original
    );
    for (id, seq, t) in floors {
        assert!(cold
            .source_high
            .iter()
            .any(|(s, q, n)| *s == id && *q >= seq && *n >= t));
    }
}

#[test]
fn reducer_acknowledges_only_explicit_same_intent_repair_then_requires_reconfirmation() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let original_expiry = w.descriptor.expires_at_ms;
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let target = w.begin_slot().unwrap();
    drive(&mut w, 2_000_000_000, target.end_ns(), &mut seq);
    assert!(w
        .persist_with(&store, true, |s, i, p, n| {
            s.publish_temporal(i, Some(p), n)?;
            Err("injected-ack-ambiguity".into())
        })
        .is_err());
    assert_eq!(w.accepted(), 0);
    assert_eq!(w.stage, Stage::BlockedDurability);
    assert!(w.begin_slot().is_err());
    let now = w.last_now_ns + 1_000_000;
    w.repair_pending(&store, now).unwrap();
    assert_eq!(w.session.rows.len(), 1);
    assert!(w.accepted() > 0);
    assert_eq!(w.stage, Stage::PausedNeedsReconfirmation);
    assert_eq!(w.descriptor.expires_at_ms, original_expiry);
    assert!(w.begin_slot().is_err());
}

#[test]
fn crashed_truncated_stage_can_be_explicitly_aborted_then_resume_saved_run() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let original = (w.descriptor.id.clone(), w.descriptor.expires_at_ms);
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let saved = serde_json::to_vec(&w.session.rows).unwrap();
    let floors = w.source_high.clone();
    let target = w.begin_slot().unwrap();
    drive(&mut w, next, target.end_ns(), &mut seq);
    assert!(w
        .persist_with(&store, true, |s, i, p, n| s
            .synthetic_partial_write(i, p, n))
        .is_err());
    let now = w.last_now_ns + 1_000_000;
    drop(w);
    let loaded = store.load_temporal(now).unwrap().unwrap();
    let token = loaded.token;
    let tickets = store.inspect_abortable_temporal(&token, now).unwrap();
    assert!(!tickets.is_empty());
    store.abort_temporal_pending(&tickets[0], now).unwrap();
    let mut restored =
        TemporalWorkflow::cold_restore(&store, &context(), now, "after-abort", now).unwrap();
    assert_eq!(restored.stage, Stage::PausedNeedsReconfirmation);
    assert_eq!(serde_json::to_vec(&restored.session.rows).unwrap(), saved);
    assert_eq!(
        (
            restored.descriptor.id.clone(),
            restored.descriptor.expires_at_ms
        ),
        original
    );
    assert_eq!(restored.source_high, floors);
    let start_ns = now + 1_000_000_000;
    start(&mut restored, &store, start_ns, "resume");
    commit_next(&mut restored, &store, start_ns, &mut seq);
    assert_eq!(restored.session.rows.len(), 2);
    assert_eq!(
        serde_json::to_vec(&restored.session.rows[..1]).unwrap(),
        saved
    );
}

#[test]
fn live_abort_retains_known_ram_highwater_while_dropping_only_unsaved_rows() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let saved = serde_json::to_vec(&w.session.rows).unwrap();
    let target = w.begin_slot().unwrap();
    drive(&mut w, next, target.end_ns(), &mut seq);
    let high = w.source_high.clone();
    assert!(w
        .persist_with(&store, true, |s, i, p, n| s
            .synthetic_partial_write(i, p, n))
        .is_err());
    let now = w.last_now_ns + 1_000_000;
    let tickets = store
        .inspect_abortable_temporal(w.commit_token.as_ref().unwrap(), now)
        .unwrap();
    w.abort_pending_artifact(&store, &tickets[0], now).unwrap();
    assert_eq!(w.source_high, high);
    assert_eq!(serde_json::to_vec(&w.session.rows).unwrap(), saved);
    assert_eq!(w.stage, Stage::PausedNeedsReconfirmation);
    let start_ns = now + 1_000_000_000;
    start(&mut w, &store, start_ns, "resumed");
    commit_next(&mut w, &store, start_ns, &mut seq);
    assert_eq!(w.session.rows.len(), 2);
    assert_eq!(serde_json::to_vec(&w.session.rows[..1]).unwrap(), saved);
}

#[test]
fn cold_restore_cannot_ack_visible_saved_rows_when_recertification_fails() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    let original = (w.descriptor.id.clone(), w.descriptor.expires_at_ms);
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let target = w.begin_slot().unwrap();
    drive(&mut w, 2_000_000_000, target.end_ns(), &mut seq);
    assert!(w
        .persist_with(&store, true, |s, i, p, n| s
            .synthetic_lost_witness_ack(i, p, n))
        .is_err());
    assert_eq!(w.accepted(), 0);
    let expected_rows = serde_json::to_vec(&w.session.rows).unwrap();
    let now = w.last_now_ns + 1_000_000;
    drop(w);
    assert!(store.load_temporal(now).unwrap().unwrap().prefix.is_some());
    let mut called = false;
    assert!(TemporalWorkflow::cold_restore_with(
        &store,
        &context(),
        now,
        "cold",
        now,
        |_, _, _| {
            called = true;
            Err("injected-restoration-durability-denied".into())
        }
    )
    .is_err());
    assert!(called);
    let restored = TemporalWorkflow::cold_restore(&store, &context(), now, "cold", now).unwrap();
    assert_eq!(restored.stage, Stage::PausedNeedsReconfirmation);
    assert_eq!(
        serde_json::to_vec(&restored.session.rows).unwrap(),
        expected_rows
    );
    assert_eq!(
        (
            restored.descriptor.id.clone(),
            restored.descriptor.expires_at_ms
        ),
        original
    );
}
