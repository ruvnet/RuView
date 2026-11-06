use super::super::tests::{commit_next, setup, start};
use super::*;
#[test]
fn real_aead_initial_barrier_and_ambiguous_save_fail_closed_without_new_authority() {
    let (_dir, store, mut w) = setup();
    assert!(w
        .persist_with(&store, false, |_, _, _, _| Err(
            "injected-before-save".into()
        ))
        .is_err());
    assert_eq!(w.stage, Stage::BlockedDurability);
    assert!(store.load_temporal(w.last_now_ns).unwrap().is_none());
    assert!(w.begin_slot().is_err());
    let (_dir, store, mut w) = setup();
    let result = w.persist_with(&store, false, |s, d, b, n| {
        s.publish_temporal(d, Some(b), n)?;
        Err("injected-after-real-save-before-ack".into())
    });
    assert!(result.is_err());
    assert_eq!(w.stage, Stage::BlockedDurability);
    assert!(w.begin_slot().is_err());
    let loaded = store
        .load_temporal(w.last_now_ns)
        .unwrap()
        .unwrap()
        .prefix
        .unwrap();
    assert!(decode(loaded.bytes(), loaded.descriptor(), w.last_now_ns).is_ok());
    let c = crate::room_learner::temporal_model_input::managed::tests::context();
    let encrypted = std::fs::read(
        _dir.path()
            .join(&c.owner_scope)
            .join("SensingServer/room-learner-recovery/temporal-prefix-commitment-v1/slot-0.bin"),
    )
    .unwrap();
    assert!(encrypted.starts_with(b"RVTPAY01"));
    assert!(serde_json::from_slice::<serde_json::Value>(&encrypted).is_err());
    assert!(!encrypted
        .windows(SCHEMA.len())
        .any(|w| w == SCHEMA.as_bytes()));
}
#[test]
fn rehashed_checkpoint_role_policy_epoch_fence_expiry_and_fake_receipt_tampering_fail() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    let bytes = w.durable.as_ref().unwrap();
    for case in 0..9 {
        let mut e: Envelope = serde_json::from_slice(bytes).unwrap();
        match case {
            0 => e.payload.rows[0].role = DataRole::HeldOut,
            1 => e.payload.binding.target_mapping = "old-uninterrupted".into(),
            2 => e.payload.identities[0].mapping.receiver_epoch = "".into(),
            3 => e.payload.source_high[0].1 = 1,
            4 => e.payload.descriptor.expires_at_ms += 1,
            5 => e.payload.frozen_model = Some("a".repeat(64)),
            6 => e.payload.quality_issued = true,
            7 => e.payload.ledger.spans[0].observed_start_ns = 1,
            _ => {
                e.payload.rows[0].views[0].frame_ids[0] += 1;
                e.payload.evidence[0].cohort_sha256 = hash(&e.payload.rows[0]).unwrap();
                e.payload.ledger.spans[0].window_sha256 = hash(&e.payload.rows[0]).unwrap();
            }
        };
        e.payload_sha256 = hash(&e.payload).unwrap();
        assert!(
            decode(
                &serde_json::to_vec(&e).unwrap(),
                &w.descriptor,
                w.last_now_ns
            )
            .is_err(),
            "case{case}"
        );
    }
    let expired = w.descriptor.expires_at_ms * 1_000_000;
    assert!(decode(bytes, &w.descriptor, expired).is_err());
}

#[test]
fn actual_encrypted_file_corruption_during_readback_blocks_collection() {
    let (dir, store, mut w) = setup();
    let c = crate::room_learner::temporal_model_input::managed::tests::context();
    let file = dir
        .path()
        .join(&c.owner_scope)
        .join("SensingServer/room-learner-recovery/temporal-prefix-commitment-v1/slot-0.bin");
    assert!(w
        .persist_with(&store, false, |s, d, b, n| {
            let token = s.publish_temporal(d, Some(b), n)?;
            let mut encrypted = std::fs::read(&file).unwrap();
            let i = encrypted.len() - 1;
            encrypted[i] ^= 1;
            std::fs::write(&file, encrypted).unwrap();
            Ok(token)
        })
        .is_err());
    assert_eq!(w.stage, Stage::BlockedDurability);
    assert!(w.begin_slot().is_err());
    assert!(store.load_temporal(w.last_now_ns).is_err());
}

#[test]
fn failed_completed_window_barrier_never_advances_promised_durable_prefix() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "one");
    let mut seq = 0;
    let target = w.begin_slot().unwrap();
    super::super::tests::drive(&mut w, 2_000_000_000, target.end_ns(), &mut seq);
    assert!(w
        .persist_with(&store, true, |_, _, _, _| Err(
            "injected-window-save-failure".into()
        ))
        .is_err());
    assert_eq!(w.accepted(), 0);
    assert_eq!(w.stage, Stage::BlockedDurability);
}

#[test]
fn credited_prefix_cannot_be_replaced_by_a_rehashed_genesis_parent() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "initial");
    let mut seq = 0;
    let next = commit_next(&mut w, &store, 2_000_000_000, &mut seq);
    w.pause().unwrap();
    start(&mut w, &store, next + 10_000_000_000, "resumed");
    let mut e: Envelope = serde_json::from_slice(w.durable.as_ref().unwrap()).unwrap();
    let old_mapping = e.payload.ledger.spans[0].mapping.clone();
    let paused = e.payload.last_now_ns;
    let anchor = GenesisGap::new(
        &e.payload.descriptor,
        &e.payload.run,
        0,
        old_mapping,
        paused,
        e.payload.source_high.clone(),
    )
    .unwrap();
    let gap = &mut e.payload.ledger.gaps[0];
    gap.after_prefix_sha256 = None;
    gap.genesis = Some(anchor);
    gap.accepted_observed_ns = 0;
    gap.previous_receiver_ns = paused;
    gap.new_mapping.observed_start_ns = 0;
    e.payload.mapping = Some(gap.new_mapping.clone());
    e.payload_sha256 = hash(&e.payload).unwrap();
    let err = decode(
        &serde_json::to_vec(&e).unwrap(),
        &w.descriptor,
        w.last_now_ns,
    )
    .err()
    .expect("missing parent after credited prefix must reject");
    assert_eq!(err, "temporal_gap_invalid_genesis");
}

#[test]
fn zero_prefix_anchor_binds_original_run_clock_and_source_floor_history() {
    let (_dir, store, mut w) = setup();
    w.persist_initial(&store).unwrap();
    start(&mut w, &store, 2_000_000_000, "initial");
    let target = w.begin_slot().unwrap();
    let mut seq = 0;
    let next = super::super::tests::drive(
        &mut w,
        2_000_000_000,
        target.target_ns() + 1_000_000_000,
        &mut seq,
    );
    w.pause().unwrap();
    start(&mut w, &store, next + 10_000_000_000, "resumed");
    let bytes = w.durable.as_ref().unwrap();
    for case in 0..4 {
        let mut e: Envelope = serde_json::from_slice(bytes).unwrap();
        match case {
            0 => e.payload.run = "foreign-run".into(),
            1 => {
                e.payload.ledger.gaps[0]
                    .genesis
                    .as_mut()
                    .unwrap()
                    .paused_receiver_ns = 0
            }
            2 => {
                e.payload.ledger.gaps[0]
                    .genesis
                    .as_mut()
                    .unwrap()
                    .source_floors[0]
                    .1 = 1
            }
            _ => e.payload.ledger.gaps[0].genesis = None,
        };
        e.payload_sha256 = hash(&e.payload).unwrap();
        assert!(
            decode(
                &serde_json::to_vec(&e).unwrap(),
                &w.descriptor,
                w.last_now_ns
            )
            .is_err(),
            "case{case}"
        );
    }
}

#[test]
fn bridge_prefix_version_is_strict_and_rehashed_quality_tampering_is_rejected() {
    let (_dir, store, mut capture) = super::super::tests::bridge_setup();
    capture
        .start_segment("bridge-checkpoint", 2_000_000_000, &store)
        .unwrap();
    let mut sequence = 0;
    let now = super::super::tests::drive_bridge(
        &mut capture,
        &store,
        2_000_000_000,
        5_000_000_000,
        &mut sequence,
    );
    let loaded = store.load_temporal(now).unwrap().unwrap();
    let prefix = loaded.prefix.unwrap();
    let mut envelope: Envelope = serde_json::from_slice(prefix.bytes()).unwrap();
    assert_eq!(envelope.schema, BRIDGE_SCHEMA);
    assert!(!envelope.payload.quality_issued && !envelope.payload.confirmation_issued);
    envelope.payload.bridge.as_mut().unwrap().parts[0].sources[0].packets = 1;
    envelope.payload_sha256 = hash(&envelope.payload).unwrap();
    let bytes = serde_json::to_vec(&envelope).unwrap();
    assert!(validated_prefix(&bytes, prefix.descriptor(), now).is_err());
}
#[test]
fn a_rehashed_prefix_cannot_claim_an_unearned_next_train_phase() {
    let (_dir, store, capture) = super::super::tests::bridge_setup();
    let now = 1_000_000_000;
    let loaded = store.load_temporal(now).unwrap().unwrap();
    let prefix = loaded.prefix.unwrap();
    let mut envelope: Envelope = serde_json::from_slice(prefix.bytes()).unwrap();
    envelope.payload.episode = 1;
    envelope.payload_sha256 = hash(&envelope.payload).unwrap();
    assert!(validated_prefix(
        &serde_json::to_vec(&envelope).unwrap(),
        prefix.descriptor(),
        now
    )
    .is_err());
    assert_eq!(capture.snapshot()["sealed_train_episodes"], 0);
}

#[test]
fn bridge_quality_observed_tail_must_remain_covered_by_durable_replay_floors() {
    let (_dir, store, mut capture) = super::super::tests::bridge_setup();
    capture
        .start_segment("quality-floor", 2_000_000_000, &store)
        .unwrap();
    let mut sequence = 0;
    let now = super::super::tests::drive_bridge(
        &mut capture,
        &store,
        2_000_000_000,
        5_000_000_000,
        &mut sequence,
    );
    let prefix = store.load_temporal(now).unwrap().unwrap().prefix.unwrap();
    let mut envelope: Envelope = serde_json::from_slice(prefix.bytes()).unwrap();
    let view = &envelope.payload.rows.last().unwrap().views[0];
    let observed = envelope
        .payload
        .bridge
        .as_ref()
        .unwrap()
        .parts
        .last()
        .unwrap()
        .sources
        .iter()
        .find(|s| s.source_id == view.source_id)
        .unwrap();
    assert!(
        observed.last_sequence > *view.sequence_ids.last().unwrap(),
        "the actual reservation tail must include packets after the retained native horizon"
    );
    let floor = envelope
        .payload
        .source_high
        .iter_mut()
        .find(|s| s.0 == view.source_id)
        .unwrap();
    floor.1 = *view.sequence_ids.last().unwrap();
    floor.2 = view.last_time_ns;
    envelope.payload_sha256 = hash(&envelope.payload).unwrap();
    let bytes = serde_json::to_vec(&envelope).unwrap();
    assert!(
        validated_prefix(&bytes, prefix.descriptor(), now).is_err(),
        "rehashed valid rows cannot hide replay-floor rollback below the observed quality tail"
    );
}
