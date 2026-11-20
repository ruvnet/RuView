//! SYNTHETIC metadata and resource oracles. No capture/fit/permission issued.
use super::*;
fn context(count: u8) -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-schedule".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=count)
            .map(|source_id| super::super::observation::SourceSpec {
                source_id,
                device_id: format!("esp32-node-{source_id}"),
                grid_id: "ant1-bins64-ppdu0-usable-drop2-linear-v2".into(),
                feature_count: 56,
                signal_kind: ruview_unified::RfModality::WifiCsi,
                center_frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                link_id: format!("native-{source_id}"),
                link_index: 0,
                geometry: None,
            })
            .collect(),
    }
}
fn plan() -> ManagedScheduleContract {
    ManagedScheduleContract::build(context(3), LeadInPolicy::Train5HeldOut15EngineeringV1).unwrap()
}
#[test]
fn option_a_is_fifteen_real_episode_descriptors_not_ids_as_independence() {
    let p = plan();
    assert_eq!(p.episodes().len(), 15);
    let empty = p
        .episodes()
        .iter()
        .filter(|e| e.role == DataRole::Train && e.condition == RoomLabel::Empty)
        .collect::<Vec<_>>();
    assert_eq!(empty.len(), 1);
    assert_eq!(empty[0].target_active_recording_ms, 600_000);
    assert_eq!(empty[0].slots.len(), 16);
    assert_eq!(empty[0].retained_nominal_window_ms, 32_000);
    assert_eq!(
        empty[0].retained_nominal_window_ms * 10000 / empty[0].target_active_recording_ms,
        533
    );
    for condition in [
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ] {
        let episodes = p
            .episodes()
            .iter()
            .filter(|e| e.role == DataRole::Train && e.condition == condition)
            .collect::<Vec<_>>();
        assert_eq!(episodes.len(), 2);
        assert!(episodes
            .iter()
            .all(|e| e.target_active_recording_ms == 30_000 && e.slots.len() == 8));
    }
    for condition in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ] {
        assert_eq!(
            p.episodes()
                .iter()
                .filter(|e| e.role == DataRole::HeldOut
                    && e.condition == condition
                    && e.target_active_recording_ms == 30_000
                    && e.slots.len() == 8)
                .count(),
            2
        );
    }
    assert!(p.episodes()[..7].iter().all(|e| e.role == DataRole::Train));
    assert!(p.episodes()[7..]
        .iter()
        .all(|e| e.role == DataRole::HeldOut));
    assert!(p
        .episodes()
        .iter()
        .all(|e| e.requires_explicit_operator_start
            && e.requires_conditions_held_confirmation
            && e.separately_executed_episode_required));
    assert_eq!(p.counts().verified_independent_trial_count, None);
    let value: serde_json::Value = serde_json::from_slice(&p.export().unwrap()).unwrap();
    for field in [
        "capture_ready",
        "full_episode_quality_monitoring_implemented",
        "independence_verified",
        "scientific_adequacy_claimed",
        "user_facing_default",
    ] {
        assert_eq!(value["body"]["claims"][field], false);
    }
}
#[test]
fn timing_retention_and_resource_totals_are_derived_and_honest() {
    let p = plan();
    let c = p.counts();
    assert_eq!(
        (
            c.train_episodes,
            c.held_out_episodes,
            c.train_windows,
            c.held_out_windows,
            c.total_windows
        ),
        (7, 8, 64, 64, 128)
    );
    assert_eq!(
        (
            c.train_active_recording_ms,
            c.held_out_active_recording_ms,
            c.active_recording_ms
        ),
        (780_000, 240_000, 1_020_000)
    );
    assert_eq!(
        (
            c.train_lead_in_ms,
            c.held_out_lead_in_ms,
            c.total_lead_in_ms
        ),
        (35_000, 120_000, 155_000)
    );
    assert_eq!(c.nominal_recording_and_lead_in_ms, 1_175_000);
    assert_eq!(c.nominal_joint_window_ms, 256_000);
    assert_eq!(c.nominal_source_view_window_ms, 768_000);
    let r = p.resources();
    assert_eq!(
        (
            r.cohorts,
            r.source_views,
            r.max_retained_rows,
            r.max_numeric_values
        ),
        (128, 384, 29_568, 1_655_808)
    );
    assert!(r.serialized_upper_bound_bytes <= 64 * 1024 * 1024);
    assert!(p.body.serialized_checkpoint_preflight_is_estimate_only);
}
#[test]
fn windows_span_whole_episodes_without_overlap_or_shortened_horizon() {
    let p = plan();
    for e in p.episodes() {
        assert_eq!(e.slots.first().unwrap().target_offset_ms, 0);
        assert_eq!(
            e.slots.last().unwrap().deadline_offset_ms,
            e.target_active_recording_ms
        );
        for s in &e.slots {
            assert_eq!(s.deadline_offset_ms - s.target_offset_ms, 2150);
            assert!(s.deadline_offset_ms <= e.target_active_recording_ms);
        }
        assert!(e
            .slots
            .windows(2)
            .all(|w| w[0].deadline_offset_ms <= w[1].target_offset_ms));
    }
    assert!(slots(30_000, 16, 2150).is_err());
    assert!(slots(30_000, 1, 2150).is_err());
}
#[test]
fn held_out_binding_requires_future_runtime_frozen_identity_without_role_relabeling() {
    let p = plan();
    for i in 7..15 {
        assert!(p
            .bind_episode_start("synthetic-run", i, 0, 0, None)
            .is_err());
    }
    let model = FrozenModelIdentity::from_runtime_verified_artifact(&p, "b".repeat(64)).unwrap();
    for i in 7..15 {
        let b = p
            .bind_episode_start("synthetic-run", i, 0, 0, Some(&model))
            .unwrap();
        assert_eq!(b.episode.role, DataRole::HeldOut);
        assert_eq!(b.frozen_artifact_sha256, Some("b".repeat(64)));
    }
    assert!(p
        .bind_episode_start("synthetic-run", 0, 0, 0, Some(&model))
        .is_err());
    assert!(FrozenModelIdentity::from_runtime_verified_artifact(
        &p,
        "inner-v1-not-a-whole-hash".into()
    )
    .is_err());
    let mut changed = context(3);
    changed.revision += 1;
    let other = ManagedScheduleContract::build(changed, LeadInPolicy::Train5HeldOut15EngineeringV1)
        .unwrap();
    let foreign =
        FrozenModelIdentity::from_runtime_verified_artifact(&other, "b".repeat(64)).unwrap();
    assert!(p
        .bind_episode_start("synthetic-run", 7, 0, 0, Some(&foreign))
        .is_err());
}
#[test]
fn attempts_and_sessions_do_not_create_new_trial_groups_or_unbounded_ids() {
    let p = plan();
    let a = p
        .bind_episode_start(&"r".repeat(128), 0, 0, 0, None)
        .unwrap();
    let b = p
        .bind_episode_start(&"r".repeat(128), 0, 1, 2, None)
        .unwrap();
    assert_eq!(a.episode_id, b.episode_id);
    assert_eq!(a.trial_id, b.trial_id);
    assert_ne!(a.session_id, b.session_id);
    assert_ne!(a.attempt_id, b.attempt_id);
    for id in [&a.episode_id, &a.trial_id, &a.session_id, &a.attempt_id] {
        assert!(bounded_id(id));
    }
    let next = p
        .bind_episode_start("synthetic-run", 1, 0, 0, None)
        .unwrap();
    let distinct = p
        .bind_episode_start("synthetic-run", 2, 0, 0, None)
        .unwrap();
    assert_ne!(next.trial_id, distinct.trial_id);
    assert!(p.bind_episode_start("", 0, 0, 0, None).is_err());
    assert!(p.bind_episode_start("bad run", 0, 0, 0, None).is_err());
    assert!(p
        .bind_episode_start("synthetic-run", 15, 0, 0, None)
        .is_err());
    assert_eq!(p.counts().train_episodes, 7);
}
#[test]
fn canonical_versioned_hash_does_not_relabel_seed_or_accept_unknown_contracts() {
    let p = plan();
    let bytes = p.export().unwrap();
    assert_eq!(
        ManagedScheduleContract::import(&bytes)
            .unwrap()
            .export()
            .unwrap(),
        bytes
    );
    let seed = AcquisitionPlan::build(
        context(3),
        ExperimentalPlanId::Native2000msSlots8,
        &[
            (1, ReceiverCadenceBand::Hz30To36),
            (2, ReceiverCadenceBand::Hz30To36),
            (3, ReceiverCadenceBand::Hz30To36),
        ],
    )
    .unwrap();
    assert_ne!(p.sha256(), seed.sha256());
    // Even canonical, self-consistently rehashed metadata cannot redefine this
    // finite profile's schedule, roles, counts or claims.
    for change in [
        "duration",
        "role",
        "slots",
        "quality",
        "independence",
        "resource",
    ] {
        let mut body = p.body.clone();
        match change {
            "duration" => body.episodes[0].target_active_recording_ms = 300_000,
            "role" => body.episodes[7].role = DataRole::Train,
            "slots" => {
                body.episodes[0].slots.pop();
            }
            "quality" => body.claims.full_episode_quality_monitoring_implemented = true,
            "independence" => body.claims.independence_verified = true,
            "resource" => body.resources.max_numeric_values = 1,
            _ => unreachable!(),
        }
        let forged_hash = hash(&body).unwrap();
        let forged = serde_json::to_vec(&Export {
            schema: SCHEMA,
            contract_sha256: &forged_hash,
            body: &body,
        })
        .unwrap();
        assert!(
            ManagedScheduleContract::import(&forged).is_err(),
            "{change}"
        );
    }
    let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    unknown["body"]["automatic_start"] = true.into();
    assert!(ManagedScheduleContract::import(&serde_json::to_vec(&unknown).unwrap()).is_err());
    let wrong_schema = serde_json::to_vec(&Export {
        schema: "ruview.experimental.native-temporal-acquisition.v2",
        contract_sha256: p.sha256(),
        body: &p.body,
    })
    .unwrap();
    assert!(ManagedScheduleContract::import(&wrong_schema).is_err());
    assert!(ManagedScheduleContract::import(&vec![b' '; MAX_BYTES + 1]).is_err());
}
#[test]
fn native_capacity_and_shape_reject_before_capture_without_fallback() {
    assert!(
        ManagedScheduleContract::build(context(8), LeadInPolicy::Train5HeldOut15EngineeringV1)
            .is_err()
    );
    assert!(
        ManagedScheduleContract::build(context(2), LeadInPolicy::Train5HeldOut15EngineeringV1)
            .is_err()
    );
    let mut wrong = context(3);
    wrong.sources[0].feature_count = 55;
    assert!(
        ManagedScheduleContract::build(wrong, LeadInPolicy::Train5HeldOut15EngineeringV1).is_err()
    );
    let p = plan();
    let mut sources = p.body.native_input.sources.clone();
    sources[0].max_rows = MAX_ROWS + 1;
    assert!(capacity(&context(3), &sources, 64, 128).is_err());
    assert!(capacity(&context(3), &p.body.native_input.sources, 65, 128).is_err());
    assert!(capacity(&context(3), &p.body.native_input.sources, 64, 129).is_err());
    let many = context(8);
    let policies = many
        .sources
        .iter()
        .map(|s| SourceWindowPolicy {
            source_id: s.source_id,
            receiver_cadence: ReceiverCadenceBand::Hz30To36,
            min_rows: 61,
            max_rows: 77,
            min_packet_interval_ns: 1_000_000_000_000 / 36_000,
            max_gap_ns: 50_000_000,
        })
        .collect::<Vec<_>>();
    assert!(capacity(&many, &policies, 64, 128).is_err());
    assert_eq!(p.counts().total_windows, 128);
}

#[test]
fn operator_eight_step_is_exact_interleaved_user_sequence_with_bounded_reserved_checks() {
    let old = plan().export().unwrap();
    let p=ManagedScheduleContract::build(context(3),LeadInPolicy::OperatorEightStepV1).unwrap();
    assert_eq!(p.profile_id(),OPERATOR_EIGHT_PROFILE);
    let expected=[
        (DataRole::Train,RoomLabel::Empty,15_000,600_000,16),
        (DataRole::Validation,RoomLabel::Empty,15_000,15_000,4),
        (DataRole::Train,RoomLabel::OccupiedStatic,5_000,60_000,16),
        (DataRole::Validation,RoomLabel::OccupiedStatic,5_000,15_000,4),
        (DataRole::Train,RoomLabel::OccupiedMoving,5_000,60_000,16),
        (DataRole::Validation,RoomLabel::OccupiedMoving,5_000,15_000,4),
        (DataRole::Train,RoomLabel::AdjacentOnly,15_000,60_000,16),
        (DataRole::Validation,RoomLabel::AdjacentOnly,15_000,15_000,4),
    ];
    assert_eq!(p.episodes().len(),8);
    for (episode,(role,label,lead,duration,windows)) in p.episodes().iter().zip(expected) {
        assert_eq!((episode.role,episode.condition,episode.lead_in_ms,episode.target_active_recording_ms,episode.slots.len()),(role,label,lead,duration,windows));
        assert!(episode.requires_explicit_operator_start&&episode.requires_conditions_held_confirmation);
        assert!(episode.slots.windows(2).all(|pair|pair[0].deadline_offset_ms<=pair[1].target_offset_ms));
        assert!(episode.slots.last().unwrap().deadline_offset_ms<=duration);
    }
    let c=p.counts();
    assert_eq!((c.train_episodes,c.held_out_episodes,c.train_windows,c.held_out_windows,c.total_windows),(4,4,64,16,80));
    assert_eq!(c.nominal_recording_and_lead_in_ms,920_000);
    assert_eq!(p.resources().max_numeric_values,1_034_880);
    assert_eq!(p.resources().max_retained_rows,18_480);
    assert_eq!(ManagedScheduleContract::import(&p.export().unwrap()).unwrap().export().unwrap(),p.export().unwrap());
    assert_eq!(ManagedScheduleContract::import(&old).unwrap().export().unwrap(),old);
    assert_ne!(p.sha256(),plan().sha256());
}

#[test]
fn operator_reserved_checks_bind_without_a_model_but_cannot_be_relabelled_as_training() {
    let p=ManagedScheduleContract::build(context(3),LeadInPolicy::OperatorEightStepV1).unwrap();
    let check=p.bind_episode_start("operator-run",1,0,0,None).unwrap();
    assert_eq!(check.episode().role,DataRole::Validation);
    let frozen=FrozenModelIdentity::from_runtime_verified_artifact(&p,"f".repeat(64)).unwrap();
    assert!(p.bind_episode_start("operator-run",1,0,0,Some(&frozen)).is_err());
    assert!(plan().bind_episode_start("old-run",7,0,0,None).is_err());
    let mut value:serde_json::Value=serde_json::from_slice(&p.export().unwrap()).unwrap();
    value["body"]["episodes"][1]["role"]=serde_json::json!("Train");
    assert!(ManagedScheduleContract::import(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn operator_default_is_explicit_and_rehashed_authority_claims_stay_rejected(){
    let p=ManagedScheduleContract::build(context(3),LeadInPolicy::OperatorEightStepV1).unwrap();
    assert!(p.body.claims.user_facing_default);
    assert!(!p.body.claims.capture_ready&&!p.body.claims.scientific_adequacy_claimed&&!p.body.claims.independence_verified);
    assert!(!plan().body.claims.user_facing_default);
    let mut forged=p.clone();forged.body.claims.capture_ready=true;forged.sha256=hash(&forged.body).unwrap();
    assert!(ManagedScheduleContract::import(&forged.export().unwrap()).is_err());
    let mut forged=plan();forged.body.claims.user_facing_default=true;forged.sha256=hash(&forged.body).unwrap();
    assert!(ManagedScheduleContract::import(&forged.export().unwrap()).is_err());
}
