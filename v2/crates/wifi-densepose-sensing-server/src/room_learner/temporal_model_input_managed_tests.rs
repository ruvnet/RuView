//! SYNTHETIC source integration only: no whole-trial quality/confirmation proof.
use super::*;
use crate::room_learner::{
    artifact::ArtifactStore,
    managed_schedule_contract::{LeadInPolicy, ManagedScheduleContract},
    native::{canonicalize_esp32, NativePacketInput},
    temporal_live_driver::{ContinuousTemporalInput, ObservationAvailability},
};
use std::sync::OnceLock;
use wifi_densepose_hardware::{Adr018Flags, PpduType};
pub(super) fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-managed-v3".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=3)
            .map(|source_id| SourceSpec {
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
fn schedule() -> ManagedScheduleContract {
    ManagedScheduleContract::build(context(), LeadInPolicy::Train5HeldOut15EngineeringV1).unwrap()
}
pub(super) fn packet(
    source: u8,
    seq: u32,
    at: u64,
    label: RoomLabel,
    row: usize,
) -> CanonicalPacket {
    let gain = match label {
        RoomLabel::Empty => 3,
        RoomLabel::AdjacentOnly => 4,
        RoomLabel::OccupiedStatic => 8,
        RoomLabel::OccupiedMoving => 10 + (row % 2) as u8,
        RoomLabel::Unknown => 8,
    };
    let iq = (0..64)
        .flat_map(|bin| [gain + source + (bin % 2) as u8, 4])
        .collect::<Vec<_>>();
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence: seq,
        received_at_ns: at,
        receiver_sample_rate_hz: 33.,
        iq_bytes: &iq,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::Synthetic,
    })
    .unwrap()
}
fn collect(
    session: &mut TemporalTrainingSession,
    target: &ManagedWindowTarget,
    index: u32,
    label: RoomLabel,
) {
    session.begin_managed_window(target).unwrap();
    for row in 0..100usize {
        for source in 1..=3u8 {
            let at = target.target_ns()
                + row as u64 * 1_000_000_000u64.div_ceil(33)
                + u64::from(source) * 1000;
            if session
                .ingest(&packet(
                    source,
                    index * 1000 + row as u32 + 1,
                    at,
                    label,
                    row,
                ))
                .unwrap()
                == WindowAdmission::Ready
            {
                session.finish_window(at).unwrap();
                return;
            }
        }
    }
    panic!("missing actual2s native window");
}
fn train() -> (TemporalTrainingSession, TemporalModel) {
    let plan = schedule();
    let mut session = TemporalTrainingSession::managed(
        plan.clone(),
        "synthetic-v3-run".into(),
        FitConfig {
            epochs: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(session.fit().is_err());
    let mut start = 10_000_000_000u64;
    let mut index = 0;
    for episode in plan.episodes().iter().filter(|e| e.role == DataRole::Train) {
        let bound = plan
            .bind_episode_start("synthetic-v3-run", episode.index, 0, 0, None)
            .unwrap();
        for slot in 0..episode.slots.len() {
            let target = ManagedWindowTarget::from_runtime_episode(
                &plan,
                &bound,
                "synthetic-v3-run",
                slot,
                start,
            )
            .unwrap();
            collect(&mut session, &target, index, episode.condition);
            index += 1;
        }
        start += episode.target_active_recording_ms * 1_000_000 + 10_000_000_000;
    }
    assert_eq!(session.dataset().unwrap().train().len(), 64);
    assert_eq!(
        session
            .dataset()
            .unwrap()
            .train()
            .iter()
            .map(|row| row.trial_id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        7
    );
    let permit = session.synthetic_test_fit_admission().unwrap();
    let (model, report) = session
        .fit_managed_with_cancel(permit, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(report.training_cohorts, 64);
    assert_eq!(report.completed_epochs, 2);
    assert!(report.backbone_delta_l2 > 0.);
    (session, model)
}
fn model() -> TemporalModel {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    TemporalModel::import(BYTES.get_or_init(|| train().1.export().unwrap())).unwrap()
}
fn live(model: TemporalModel) -> serde_json::Value {
    let mut driver = ContinuousTemporalInput::new(model).unwrap();
    driver.activate();
    let target = 10_000_000_000_000;
    for row in 0..100usize {
        for source in 1..=3u8 {
            let at = target + row as u64 * 1_000_000_000u64.div_ceil(33) + u64::from(source) * 1000;
            let d = driver.ingest(
                &packet(source, 1_000_000 + row as u32, at, RoomLabel::Unknown, row),
                at,
            );
            if d.observation.availability == ObservationAvailability::Available {
                return serde_json::to_value(d.current_prediction).unwrap();
            }
        }
    }
    panic!("continuous v3 input did not complete")
}
fn seed_v2_same_context() -> TemporalModel {
    let mut session = TemporalTrainingSession::new(
        context(),
        ExperimentalPlanId::Native2000msSlots2,
        &(1..=3)
            .map(|s| (s, ReceiverCadenceBand::Hz30To36))
            .collect::<Vec<_>>(),
        "synthetic-seed-compatible".into(),
        FitConfig {
            epochs: 2,
            ..Default::default()
        },
    )
    .unwrap();
    for index in 0..16u32 {
        let target = 10_000_000_000 + u64::from(index) * 3_000_000_000;
        let label =
            session.plan.schedule()[session.rows.len() / session.plan.slots_per_trial()].label;
        session
            .begin_window(target, target + 3_000_000_000)
            .unwrap();
        'window: for row in 0..100usize {
            for source in 1..=3u8 {
                let at =
                    target + row as u64 * 1_000_000_000u64.div_ceil(33) + u64::from(source) * 1000;
                if session
                    .ingest(&packet(
                        source,
                        index * 1000 + row as u32 + 1,
                        at,
                        label,
                        row,
                    ))
                    .unwrap()
                    == WindowAdmission::Ready
                {
                    session.finish_window(at).unwrap();
                    break 'window;
                }
            }
        }
    }
    session.fit().unwrap().0
}
#[test]
fn actual_native_managed_schedule_same_fitter_whole_v3_store_cold_and_same_driver() {
    let m = model();
    let bytes = m.export().unwrap();
    assert_eq!(m.artifact_kind(), SCHEMA_V3);
    assert!(
        m.input_policy().is_err(),
        "v3 must not expose seed-v2 plan/hash"
    );
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema"], SCHEMA_V3);
    assert_eq!(value["payload"]["evidence"].as_array().unwrap().len(), 64);
    assert_eq!(value["payload"]["identities"].as_array().unwrap().len(), 64);
    assert_eq!(
        value["payload"]["claims"]["whole_episode_quality_issued"],
        false
    );
    assert_eq!(
        value["payload"]["claims"]["conditions_confirmed_by_runtime"],
        false
    );
    let root = tempfile::tempdir().unwrap();
    let c = context();
    let path = root.path().join(&c.owner_scope).join("SensingServer");
    let store = ArtifactStore::new(&path, &c.owner_scope, &c.installation_id).unwrap();
    let seed = seed_v2_same_context();
    let seed_bytes = seed.export().unwrap();
    let seed_receipt = store.save_temporal_candidate(&seed).unwrap();
    let directory = path
        .join("room-learner")
        .join("temporal-models")
        .join(&seed_receipt.context_digest);
    let pointer = directory.join("candidate-temporal-v2.json");
    let before_pointer = std::fs::read(&pointer).unwrap();
    let receipt = store.save_temporal_candidate(&m).unwrap();
    assert_eq!(receipt.artifact_kind, SCHEMA_V3);
    assert_eq!(receipt.artifact_sha256, bytes_hash(&bytes));
    assert!(receipt.diagnostic_only && !receipt.qualification_granted);
    assert_eq!(
        std::fs::read(&pointer).unwrap(),
        before_pointer,
        "V3 may not rewrite V2 pointer"
    );
    assert_eq!(
        store
            .load_temporal_candidate(&c)
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        seed_bytes
    );
    assert_eq!(
        store
            .load_exact_temporal_candidate(&c, &seed_receipt.artifact_sha256)
            .unwrap()
            .export()
            .unwrap(),
        seed_bytes
    );
    let cold = ArtifactStore::new(&path, &c.owner_scope, &c.installation_id)
        .unwrap()
        .load_managed_temporal_candidate(&c)
        .unwrap()
        .unwrap();
    assert_eq!(cold.export().unwrap(), bytes);
    assert_eq!(live(m), live(cold));
}
#[test]
fn heldout_uses_frozen_whole_artifact_and_never_changes_train_codec_or_profiles() {
    let (mut session, m) = train();
    let before = m.export().unwrap();
    let frozen = bytes_hash(&before);
    let plan = schedule();
    let identity =
        FrozenModelIdentity::from_runtime_verified_artifact(&plan, frozen.clone()).unwrap();
    let mut start = 2_000_000_000_000;
    let mut index = 64;
    for episode in plan
        .episodes()
        .iter()
        .filter(|e| e.role == DataRole::HeldOut)
    {
        let bound = plan
            .bind_episode_start("synthetic-v3-run", episode.index, 0, 0, Some(&identity))
            .unwrap();
        for slot in 0..episode.slots.len() {
            let target = ManagedWindowTarget::from_runtime_episode(
                &plan,
                &bound,
                "synthetic-v3-run",
                slot,
                start,
            )
            .unwrap();
            collect(&mut session, &target, index, episode.condition);
            index += 1;
        }
        start += episode.target_active_recording_ms * 1_000_000 + 20_000_000_000;
    }
    assert_eq!(session.dataset().unwrap().held_out().len(), 64);
    assert_eq!(m.export().unwrap(), before);
    let permit = session.synthetic_test_fit_admission().unwrap();
    assert!(session
        .fit_managed_with_cancel(permit, &AtomicBool::new(false))
        .is_err());
    let v: serde_json::Value = serde_json::from_slice(&before).unwrap();
    assert!(v["payload"]["identities"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["role"] == "Train"));
}
#[test]
fn managed_cannot_take_seed_labels_skip_slots_or_admit_wrong_frozen_model() {
    let plan = schedule();
    let mut session = TemporalTrainingSession::managed(
        plan.clone(),
        "guard-run".into(),
        FitConfig {
            epochs: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(session.begin_window(10, 3_000_000_010).is_err());
    let bound = plan.bind_episode_start("guard-run", 0, 0, 0, None).unwrap();
    let target =
        ManagedWindowTarget::from_runtime_episode(&plan, &bound, "guard-run", 1, 10_000_000_000)
            .unwrap();
    assert!(session.begin_managed_window(&target).is_err());
    let foreign = plan
        .bind_episode_start("foreign-run", 0, 0, 0, None)
        .unwrap();
    let foreign_target = ManagedWindowTarget::from_runtime_episode(
        &plan,
        &foreign,
        "foreign-run",
        0,
        10_000_000_000,
    )
    .unwrap();
    assert!(session.begin_managed_window(&foreign_target).is_err());
    assert!(ManagedWindowTarget::from_runtime_episode(
        &plan,
        &bound,
        "other-run",
        0,
        10_000_000_000
    )
    .is_err());
    let frozen =
        FrozenModelIdentity::from_runtime_verified_artifact(&plan, "b".repeat(64)).unwrap();
    let held = plan
        .bind_episode_start("guard-run", 7, 0, 0, Some(&frozen))
        .unwrap();
    let target =
        ManagedWindowTarget::from_runtime_episode(&plan, &held, "guard-run", 0, 1_000_000_000_000)
            .unwrap();
    assert!(session.begin_managed_window(&target).is_err());
}
#[test]
fn canonical_v3_schedule_role_encoder_frame_provenance_and_authority_tampering_fail() {
    let original = model().export().unwrap();
    for case in [
        "schedule",
        "role",
        "encoder",
        "frame",
        "provenance",
        "authority",
    ] {
        let mut e: EnvelopeV3 = serde_json::from_slice(&original).unwrap();
        match case {
            "schedule" => {
                let mut v: serde_json::Value =
                    serde_json::from_str(&e.payload.binding.schedule_json).unwrap();
                v["body"]["episodes"][0]["target_active_recording_ms"] = 300000.into();
                e.payload.binding.schedule_json = v.to_string();
            }
            "role" => {
                e.payload.identities[0].role = DataRole::HeldOut;
                e.payload.identity_evidence_sha256 = hash(&e.payload.identities).unwrap();
            }
            "encoder" => e.payload.binding.encoder.temporal_tokens = 8,
            "frame" => {
                e.payload.evidence[0].sources[0].frame_ids[0] = "0".into();
                e.payload.input_evidence_sha256 = hash(&e.payload.evidence).unwrap();
            }
            "provenance" => {
                e.payload.evidence[0].provenance = CaptureProvenance::LiveReceiverObserved;
                e.payload.input_evidence_sha256 = hash(&e.payload.evidence).unwrap();
            }
            "authority" => e.payload.claims.whole_episode_quality_issued = true,
            _ => unreachable!(),
        }
        e.payload_sha256 = hash(&e.payload).unwrap();
        let bad = serde_json::to_vec(&e).unwrap();
        assert!(TemporalModel::import(&bad).is_err(), "{case}");
    }
}
#[test]
fn v2_codec_remains_unwrapped_and_managed_fit_permit_cannot_be_omitted_or_forged() {
    // The exact old V2 codec structs/field order are unchanged. Existing V2 native
    // fit/cold/import golden suite is also run with this integration.
    let plan = schedule();
    let mut s = TemporalTrainingSession::managed(
        plan,
        "permit-run".into(),
        FitConfig {
            epochs: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(s.fit().is_err());
    let fake = ManagedFitAdmission {
        policy: "a".repeat(64),
        data: "b".repeat(64),
        evidence: "c".repeat(64),
        identities: "d".repeat(64),
    };
    assert!(s
        .fit_managed_with_cancel(fake, &AtomicBool::new(false))
        .is_err());
    assert!(RoomModel::import(&model().export().unwrap()).is_err());
}

#[test]
fn mode_correct_policy_identity_tracks_managed_targets_and_preserves_seed_v2_identity() {
    let plan = schedule();
    let config = FitConfig {
        epochs: 2,
        ..Default::default()
    };
    let expected = hash(&ManagedBinding::build(&plan, config.clone()).unwrap()).unwrap();
    let mut session =
        TemporalTrainingSession::managed(plan.clone(), "identity-run".into(), config.clone())
            .unwrap();
    assert_eq!(session.plan_sha256(), expected, "managed session must expose the whole managed input binding, not its seed validation carrier");
    assert_ne!(session.plan_sha256(), session.plan.sha256());
    let bound = plan
        .bind_episode_start("identity-run", 0, 0, 0, None)
        .unwrap();
    let target =
        ManagedWindowTarget::from_runtime_episode(&plan, &bound, "identity-run", 0, 10_000_000_000)
            .unwrap();
    assert_eq!(target.identity().contract_sha256, plan.sha256());
    // Actual native target admission -> assembled window -> evidence conversion
    // rejects any policy identity different from the session's binding.
    collect(&mut session, &target, 0, RoomLabel::Empty);
    assert_eq!(session.evidence.len(), 1);
    assert_eq!(model().plan_sha256(), expected);
    let bands = (1..=3)
        .map(|id| (id, ReceiverCadenceBand::Hz30To36))
        .collect::<Vec<_>>();
    let seed = TemporalTrainingSession::new(
        context(),
        ExperimentalPlanId::Native2000msSlots2,
        &bands,
        "seed-identity-run".into(),
        config,
    )
    .unwrap();
    let old =
        AcquisitionPlan::build(context(), ExperimentalPlanId::Native2000msSlots2, &bands).unwrap();
    assert_eq!(seed.plan_sha256(), old.sha256());
    assert_eq!(seed.plan_sha256(), seed.binding.plan_sha256);
}


fn collect_operator(validation_changed:bool)->TemporalTrainingSession {
    let plan=ManagedScheduleContract::build(context(),LeadInPolicy::OperatorEightStepV1).unwrap();
    let mut session=TemporalTrainingSession::managed(plan.clone(),"synthetic-eight-run".into(),FitConfig{epochs:2,..Default::default()}).unwrap();
    let mut start=10_000_000_000u64;let mut index=0;
    for episode in plan.episodes(){
        let bound=plan.bind_episode_start("synthetic-eight-run",episode.index,0,0,None).unwrap();
        for slot in 0..episode.slots.len(){
            let target=ManagedWindowTarget::from_runtime_episode(&plan,&bound,"synthetic-eight-run",slot,start).unwrap();
            if validation_changed&&episode.role==DataRole::Validation {
                session.begin_managed_window(&target).unwrap();
                'window: for row in 0..100usize {for source in 1..=3u8 {
                    let at=target.target_ns()+row as u64*1_000_000_000u64.div_ceil(33)+u64::from(source)*1000;
                    let iq=vec![120u8;128];
                    let p=canonicalize_esp32(NativePacketInput{node_id:source,n_antennas:1,raw_bins:64,frequency_hz:2_437_000_000,
                        ppdu_type:PpduType::HtLegacy,sequence:index*1000+row as u32+1,received_at_ns:at,receiver_sample_rate_hz:33.,
                        iq_bytes:&iq,flags:Adr018Flags::default(),provenance:CaptureProvenance::Synthetic}).unwrap();
                    if session.ingest(&p).unwrap()==WindowAdmission::Ready{session.finish_window(at).unwrap();break 'window;}
                }}
            } else {collect(&mut session,&target,index,episode.condition);}
            index+=1;
        }
        start+=episode.target_active_recording_ms*1_000_000+10_000_000_000;
    }
    assert_eq!(session.rows.len(),80);
    assert_eq!(session.rows.iter().filter(|r|r.role==DataRole::Validation).count(),16);
    session
}
#[test]
fn operator_reserved_checks_never_change_actual_fitter_normalization_or_model_bytes(){
    let mut first=collect_operator(false);let mut changed=collect_operator(true);
    assert_ne!(hash(&first.rows).unwrap(),hash(&changed.rows).unwrap());
    first.retain_managed_train_only().unwrap();changed.retain_managed_train_only().unwrap();
    assert_eq!(first.rows.len(),64);assert_eq!(hash(&first.rows).unwrap(),hash(&changed.rows).unwrap());
    assert_eq!(first.rows.iter().map(|r|&r.trial_id).collect::<HashSet<_>>().len(),4);
    let a=first.synthetic_test_fit_admission().unwrap();let b=changed.synthetic_test_fit_admission().unwrap();
    let(ma,ra)=first.fit_managed_with_cancel(a,&AtomicBool::new(false)).unwrap();
    let(mb,rb)=changed.fit_managed_with_cancel(b,&AtomicBool::new(false)).unwrap();
    assert_eq!(ma.export().unwrap(),mb.export().unwrap());
    assert_eq!(serde_json::to_value(ra).unwrap(),serde_json::to_value(rb).unwrap());
    assert!(ma.managed_is_operator_eight_step().unwrap());
    assert_eq!(TemporalModel::import(&ma.export().unwrap()).unwrap().export().unwrap(),ma.export().unwrap());
}
#[test]
fn operator_training_partition_rejects_relabelled_validation_before_fitting(){
    let mut session=collect_operator(false);
    let index=session.rows.iter().position(|r|r.role==DataRole::Validation).unwrap();
    let original=(*session.rows[index]).clone();
    for label_or_role in [false,true] {
        let mut forged=original.clone();if label_or_role{forged.role=DataRole::Train}else{forged.label=RoomLabel::OccupiedMoving}
        session.rows[index]=SharedRecord::new(forged);
        assert!(session.retain_managed_train_only().is_err());
        assert!(!session.fitted);
    }
}

#[test]
fn variable_profile_builds_bound_models_for_every_supported_source_count() {
    for count in 2..=8u8 {
        let mut c=context();
        let template=c.sources[0].clone();
        c.sources=(1..=count).map(|source_id| SourceSpec { source_id, device_id:format!("esp32-node-{source_id}"), link_id:format!("native-{source_id}"), ..template.clone() }).collect();
        let schedule=ManagedScheduleContract::build(c,LeadInPolicy::OperatorVariableSourcesV2).unwrap();
        let session=TemporalTrainingSession::managed(schedule.clone(),"synthetic-variable-run".into(),FitConfig::for_source_count(usize::from(count))).unwrap();
        let binding=&session.managed.as_ref().unwrap().binding;
        assert_eq!(binding.encoder.source_count,usize::from(count));
        assert_eq!(binding.native.source_policies().len(),usize::from(count));
        assert!(binding.native.source_policies().iter().all(|s|s.receiver_cadence==ReceiverCadenceBand::Arrival18To22Burst200MsV1&&s.max_rows==52));
        assert_eq!(binding.contract().unwrap().sha256(),schedule.sha256());
    }
}

#[test]
fn variable_profile_assembles_four_c5_sources_with_bursty_twenty_hz_receiver_arrivals() {
    use crate::room_learner::native::{C5_LLTF8_V1_GRID,validate_adr018_native_format,canonicalize_esp32_with_format};
    let mut c=context();let template=c.sources[0].clone();
    c.sources=(20..=23).map(|source_id|SourceSpec{source_id,device_id:format!("esp32-node-{source_id}"),link_id:format!("native-{source_id}"),grid_id:C5_LLTF8_V1_GRID.into(),center_frequency_hz:5_180_000_000,..template.clone()}).collect();
    let plan=ManagedScheduleContract::build(c,LeadInPolicy::OperatorVariableSourcesV2).unwrap();
    let mut session=TemporalTrainingSession::managed(plan.clone(),"synthetic-four-c5".into(),FitConfig::for_source_count(4)).unwrap();
    let bound=plan.bind_episode_start("synthetic-four-c5",0,0,0,None).unwrap();
    let target=ManagedWindowTarget::from_runtime_episode(&plan,&bound,"synthetic-four-c5",0,10_000_000_000).unwrap();
    session.begin_managed_window(&target).unwrap();
    let mut completed=false;
    'window: for row in 0..45u32 { for source in 20..=23u8 {
        let sequence=row+1;let at=target.target_ns()+u64::from(row/2)*100_000_000+u64::from(row%2)*10_000+u64::from(source)*1000;
        let iq=vec![5u8;106];let mut wire=vec![0u8;20];
        wire[..4].copy_from_slice(&0xC5110001u32.to_le_bytes());wire[4]=source;wire[5]=1;
        wire[6..8].copy_from_slice(&53u16.to_le_bytes());wire[8..12].copy_from_slice(&5180u32.to_le_bytes());wire[12..16].copy_from_slice(&sequence.to_le_bytes());
        wire.extend(&iq);wire.extend([b'C',b'5',b'L',b'8',1,8,1,1,1,1,0,20,1,0,106,0]);
        let flags=Adr018Flags::default();
        let format=validate_adr018_native_format(&wire,1,53,PpduType::HtLegacy,flags).unwrap();
        let packet=canonicalize_esp32_with_format(NativePacketInput{node_id:source,n_antennas:1,raw_bins:53,frequency_hz:5_180_000_000,ppdu_type:PpduType::HtLegacy,sequence,received_at_ns:at,receiver_sample_rate_hz:if row%2==1 {100_000.}else{10.001},iq_bytes:&iq,flags,provenance:CaptureProvenance::Synthetic},format).unwrap();
        if session.ingest(&packet).unwrap()==WindowAdmission::Ready { session.finish_window(at).unwrap();completed=true;break 'window; }
    }}
    assert!(completed,"all four original C5 sources must close one native two-second window");
    assert_eq!(session.rows.len(),1);
    assert_eq!(session.rows[0].views.len(),4);
}

#[test]
fn variable_profile_four_source_training_keeps_reserved_checks_and_cold_model_identity() {
    let mut c=context();let mut fourth=c.sources[2].clone();fourth.source_id=4;fourth.device_id="esp32-node-4".into();fourth.link_id="native-4".into();c.sources.push(fourth);
    let plan=ManagedScheduleContract::build(c,LeadInPolicy::OperatorVariableSourcesV2).unwrap();
    let mut session=TemporalTrainingSession::managed(plan.clone(),"synthetic-variable-fit".into(),FitConfig{epochs:2,..FitConfig::for_source_count(4)}).unwrap();
    let mut start=10_000_000_000u64;let mut index=0u32;
    for episode in plan.episodes() {
        let bound=plan.bind_episode_start("synthetic-variable-fit",episode.index,0,0,None).unwrap();
        for slot in 0..episode.slots.len() {
            let target=ManagedWindowTarget::from_runtime_episode(&plan,&bound,"synthetic-variable-fit",slot,start).unwrap();
            session.begin_managed_window(&target).unwrap();let mut completed=false;
            'window: for row in 0..45u32 { for source in 1..=4u8 {
                let at=target.target_ns()+u64::from(row)*50_000_000+u64::from(source)*1000;
                let mut p=packet(source,index*1000+row+1,at,episode.condition,row as usize);
                // Synthetic receiver-only fixture: row intervals and cadence agree.
                p.receiver_sample_rate_hz=20.;
                if session.ingest(&p).unwrap()==WindowAdmission::Ready {session.finish_window(at).unwrap();completed=true;break 'window;}
            }}
            assert!(completed);index+=1;
        }
        start+=episode.target_active_recording_ms*1_000_000+10_000_000_000;
    }
    assert_eq!(session.rows.len(),80);
    assert_eq!(session.rows.iter().filter(|r|r.role==DataRole::Validation).count(),16);
    assert!(session.rows.iter().all(|r|r.views.len()==4));
    session.retain_managed_train_only().unwrap();assert_eq!(session.rows.len(),64);
    let admission=session.synthetic_test_fit_admission().unwrap();
    let (model,_)=session.fit_managed_with_cancel(admission,&AtomicBool::new(false)).unwrap();
    let bytes=model.export().unwrap();let cold=TemporalModel::import(&bytes).unwrap();
    assert_eq!(cold.export().unwrap(),bytes);assert_eq!(cold.context().sources.len(),4);
    assert!(cold.managed_is_operator_eight_step().unwrap());
}

#[test]
fn variable_profile_arrival_bursts_still_reject_long_gaps_and_packet_floods() {
    for flood in [false,true] {
        let plan=ManagedScheduleContract::build(context(),LeadInPolicy::OperatorVariableSourcesV2).unwrap();
        let mut session=TemporalTrainingSession::managed(plan.clone(),"synthetic-burst-negative".into(),FitConfig::default()).unwrap();
        let bound=plan.bind_episode_start("synthetic-burst-negative",0,0,0,None).unwrap();
        let target=ManagedWindowTarget::from_runtime_episode(&plan,&bound,"synthetic-burst-negative",0,10_000_000_000).unwrap();
        session.begin_managed_window(&target).unwrap();
        let mut failure=None;
        'input: for row in 0..60u32 { for source in 1..=3u8 {
            let delta=if flood {u64::from(row)*1_000_000}else{u64::from(row)*250_000_000};
            let mut p=packet(source,row+1,target.target_ns()+delta+u64::from(source)*1000,RoomLabel::Empty,row as usize);
            p.receiver_sample_rate_hz=if flood {1000.}else{4.};
            match session.ingest(&p) {
                Ok(WindowAdmission::Ready)=>panic!("unacceptable arrivals may not create a model window"),
                Ok(_)=>{},Err(e)=>{failure=Some(e);break 'input;}
            }
        }}
        assert_eq!(failure.as_deref(),Some(if flood {"acquisition_row_capacity"}else{"acquisition_packet_gap_or_rate"}));
        assert!(session.rows.is_empty());
    }
}
