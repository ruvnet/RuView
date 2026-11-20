//! SYNTHETIC contract fixtures only; these tests make no sensing-accuracy claim.
use super::*;
use ruview_unified::frame::{CalibrationState, EvidenceLevel, FrameProvenance, SignalQuality};

fn context() -> CaptureContext {
    CaptureContext { owner_scope: "synthetic-owner".into(), installation_id: "synthetic-install".into(),
        room_digest: "synthetic-room".into(), revision: 1, sources: vec![SourceSpec {
            source_id: 1, device_id: "synthetic-device".into(), grid_id: "phy-20mhz-2bins".into(),
            feature_count: 2, signal_kind: RfModality::Synthetic, center_frequency_hz: 2_400_000_000, bandwidth_hz: 20_000_000, link_id: "tx-a-rx-b".into(), link_index: 0, geometry: None }] }
}
fn cohort(context: &CaptureContext, id: u64, role: DataRole) -> CaptureCohort {
    CaptureCohort { id: format!("synthetic-{id}"), context_digest: context.digest(),
        trial_id: format!("synthetic-trial-{id}"), session_id: format!("synthetic-session-{id}"), role,
        label: RoomLabel::Empty, teacher: None, provenance: CaptureProvenance::Synthetic,
        views: vec![SourceView { source_id: 1, device_id: "synthetic-device".into(),
            grid_id: "phy-20mhz-2bins".into(), signal_kind: RfModality::Synthetic, center_frequency_hz: 2_400_000_000, bandwidth_hz: 20_000_000,
            link_id: "tx-a-rx-b".into(), link_index: 0, geometry: None, samples: vec![vec![2.0,3.0], vec![4.0,5.0]],
            frame_ids: vec![u128::from(id)*2+1,u128::from(id)*2+2], sequence_ids: vec![1,2],
            first_time_ns: 100+id*10, last_time_ns: 102+id*10,
            clock: ClockInfo {kind: ClockKind::Unknown, uncertainty_ns: u64::MAX}, sample_age_ns: 0,
            phase: PhaseAvailability::MagnitudeOnly, provenance: CaptureProvenance::Synthetic }] }
}
fn native_frame() -> RfFrameV2 {
    // Get the real complex element type through the actual native contract;
    // no lookalike tensor type or direct ndarray dependency is introduced.
    let iq = [2.0,4.0,3.0,5.0].into_iter().map(Into::into).collect();
    RfFrameV2::new(7, 102, RfModality::Synthetic,
        vec![FieldAxis::Antenna,FieldAxis::Frequency,FieldAxis::Time],
        2.4e9,20e6,100.0,vec![1,2,2],iq,vec![true;4],None,None,vec![],0,
        CalibrationState {phase_state: PhaseState::Unavailable,gain_calibrated:false,clock_ppm:20.0,
            baseline_id:None,confidence:0.0},
        SignalQuality {rssi_dbm:-50.0,noise_floor_dbm:-90.0,packet_loss:0.0,interference:0.0},
        FrameProvenance {class:ProvenanceClass::Synthetic,evidence:EvidenceLevel::L0Simulation,
            device_id:"synthetic-device".into(),firmware:"SYNTHETIC".into(),receipt_id:7}).unwrap()
}
fn metadata() -> ViewMetadata {
    ViewMetadata {frame_ids:vec![7,7],sequence_ids:vec![1,2],first_time_ns:100,last_time_ns:102,
        clock:ClockInfo {kind:ClockKind::Unknown,uncertainty_ns:u64::MAX},
        phase:PhaseAvailability::MagnitudeOnly,link_index:0,provenance:CaptureProvenance::Synthetic}
}

#[test]
fn synthetic_dataset_exposes_frozen_roles_and_no_physical_clock_claim() {
    let c = context();
    let ds = ValidatedDataset::new(c.clone(),vec![cohort(&c,1,DataRole::Train),
        cohort(&c,2,DataRole::Validation),cohort(&c,3,DataRole::HeldOut)]).unwrap();
    assert_eq!((ds.train().len(),ds.validation().len(),ds.held_out().len()),(1,1,1));
    assert!(!ds.measurement_clock_available());
}
#[test]
fn native_adapter_preserves_raw_gain_and_static_mean_shift() {
    let c = context();
    let f = native_frame();
    let first = SourceView::from_frame(&c.sources[0],&f,metadata()).unwrap();
    let mut stronger = f.clone();
    for z in &mut stronger.native_iq { *z *= 3.0; }
    let second = SourceView::from_frame(&c.sources[0],&stronger,metadata()).unwrap();
    assert_eq!(first.samples,vec![vec![2.0,3.0],vec![4.0,5.0]]);
    assert_eq!(second.samples,vec![vec![6.0,9.0],vec![12.0,15.0]]);
    assert_eq!(f.native_iq,native_frame().native_iq);
}
#[test]
fn actual_canonical_adapter_gain_normalization_is_refused() {
    let c = context();
    let t = native_frame().to_canonical(vec![LinkGeometry{tx_pos:[0.0;3],rx_pos:[1.0,0.0,0.0]}]).unwrap();
    assert!(SourceView::from_tensor(&c.sources[0],&t,metadata()).unwrap_err().contains("gain-normalized"));
}
#[test]
fn tensor_adapter_reads_real_tensor_without_centering() {
    let mut c = context();
    c.sources[0].feature_count = 56;
    let mut t = native_frame().to_canonical(vec![LinkGeometry{tx_pos:[0.0;3],rx_pos:[1.0,0.0,0.0]}]).unwrap();
    // Explicitly synthetic raw-amplitude tensor: remove fixture adapter scale
    // and overwrite every sample. No assertion is made about measured gain.
    t.calibration.gain_offset_db = 0.0;
    for z in &mut t.data { z.re = 7.0; z.im = 0.0; }
    let mut m = metadata(); m.frame_ids = vec![7;8]; m.sequence_ids = (1..=8).collect();
    let view = SourceView::from_tensor(&c.sources[0],&t,m).unwrap();
    assert!(view.samples.iter().flatten().all(|x| *x == 7.0));
}
#[test]
fn tensor_mutant_nonfinite_samples_are_rejected_despite_public_tensor_fields() {
    let mut c = context(); c.sources[0].feature_count = 56;
    let mut t = native_frame().to_canonical(vec![LinkGeometry{tx_pos:[0.0;3],rx_pos:[1.0,0.0,0.0]}]).unwrap();
    t.calibration.gain_offset_db = 0.0;
    t.data[[0,0,0]].re = f64::NAN;
    let mut m = metadata(); m.frame_ids = vec![7;8]; m.sequence_ids = (1..=8).collect();
    assert!(SourceView::from_tensor(&c.sources[0],&t,m).is_err());
}
#[test]
fn native_invalid_mask_and_phase_are_not_invented_as_measurements() {
    let c = context(); let mut f = native_frame(); f.valid_mask[0] = false;
    assert!(SourceView::from_frame(&c.sources[0],&f,metadata()).is_err());
    f.valid_mask[0] = true;
    let mut m = metadata(); m.phase = PhaseAvailability::Calibrated;
    assert!(SourceView::from_frame(&c.sources[0],&f,m).is_err());
}
#[test]
fn synthetic_boundary_mutants_are_rejected() {
    let c = context();
    let original = cohort(&c,1,DataRole::Train);
    type Mutant = fn(&mut CaptureCohort);
    let mutants: &[(&str,Mutant)] = &[
        ("context",|x|x.context_digest="wrong-room-owner-revision".into()),
        ("missing source",|x|x.views.clear()),
        ("unknown source",|x|x.views[0].source_id=2),
        ("duplicate source",|x|x.views.push(x.views[0].clone())),
        ("changed PHY #1440",|x|x.views[0].grid_id="phy-40mhz".into()),
        ("changed frequency #1440",|x|x.views[0].center_frequency_hz+=5_000_000),
        ("changed bandwidth #1440",|x|x.views[0].bandwidth_hz*=2),
        ("changed link",|x|x.views[0].link_id="other-link".into()),
        ("changed device",|x|x.views[0].device_id="other-device".into()),
        ("NaN",|x|x.views[0].samples[0][0]=f32::NAN),
        ("oversized rows",|x|x.views[0].samples.resize(MAX_ROWS+1,vec![1.0,2.0])),
        ("wrong feature count",|x|x.views[0].samples[0].push(1.0)),
        ("stale",|x|x.views[0].sample_age_ns=MAX_AGE_NS+1),
        ("sequence reversal",|x|x.views[0].sequence_ids=vec![2,1]),
        ("source clock uncertainty #1710",|x|x.views[0].clock=ClockInfo{kind:ClockKind::SourceMeasurement,uncertainty_ns:u64::MAX}),
        ("nonmonotonic time",|x|x.views[0].last_time_ns=1),
        ("long window",|x|x.views[0].last_time_ns=MAX_WINDOW_NS+200),
        ("forged provenance",|x|x.provenance=CaptureProvenance::LiveReceiverObserved),
    ];
    for (name,mutate) in mutants {
        let mut candidate = original.clone(); mutate(&mut candidate);
        assert!(ValidatedDataset::new(c.clone(),vec![candidate]).is_err(),"survived mutant: {name}");
    }
}
#[test]
fn frame_reuse_cannot_cross_roles_even_when_trial_and_session_are_renamed() {
    let c = context(); let a = cohort(&c,1,DataRole::Train);
    let mut b = cohort(&c,2,DataRole::HeldOut); b.views[0].frame_ids = a.views[0].frame_ids.clone();
    assert!(ValidatedDataset::new(c,vec![a,b]).unwrap_err().contains("frame overlap"));
}
#[test]
fn trial_or_session_cannot_cross_roles() {
    for use_trial in [true,false] {
        let c = context(); let a = cohort(&c,1,DataRole::Train); let mut b = cohort(&c,2,DataRole::Validation);
        if use_trial {b.trial_id=a.trial_id.clone();} else {b.session_id=a.session_id.clone();}
        assert!(ValidatedDataset::new(c,vec![a,b]).unwrap_err().contains("frozen roles"));
    }
}
#[test]
fn source_clock_is_not_inferred_from_receive_time_or_time_of_day() {
    let mut c = context(); c.sources[0].signal_kind=RfModality::WifiCsi;
    let mut a = cohort(&c,1,DataRole::Train); a.views[0].signal_kind=RfModality::WifiCsi;
    a.provenance=CaptureProvenance::LiveReceiverObserved;
    a.views[0].provenance=a.provenance;
    a.views[0].clock.kind=ClockKind::ReceiverOnly;
    let dataset=ValidatedDataset::new(c,vec![a]).unwrap();
    assert!(!dataset.measurement_clock_available());
}
#[test]
fn context_digest_binds_owner_room_revision_and_phy_not_time_of_day() {
    let c=context();
    for change in 0..4 {
        let mut other=c.clone();
        match change {0=>other.owner_scope="other-owner".into(),1=>other.room_digest="other-room".into(),
            2=>other.revision+=1,_=>other.sources[0].grid_id="other-PHY".into()}
        assert_ne!(other.digest(),c.digest());
    }
}
#[test]
fn missing_teacher_never_yields_a_camera_derived_empty_label() {
    let c=context(); let a=cohort(&c,1,DataRole::Train);
    let ds=ValidatedDataset::new(c,vec![a]).unwrap();
    assert!(ds.train()[0].teacher.is_none());
}
fn teacher(c:&CaptureContext,a:&CaptureCohort)->TeacherEvidence {
    TeacherEvidence {kind:TeacherKind::Radar,source_id:"synthetic-teacher".into(),
        validation_receipt:"synthetic-receipt-NOT-qualification".into(),context_digest:c.digest(),
        room_digest:c.room_digest.clone(),trial_id:a.trial_id.clone(),label:a.label,
        frame_ids:a.views[0].frame_ids.clone(),first_time_ns:a.views[0].first_time_ns,
        last_time_ns:a.views[0].last_time_ns,clock:ClockInfo{kind:ClockKind::SourceMeasurement,uncertainty_ns:1}}
}
#[test]
fn receive_time_only_radar_cannot_supply_empty_teacher_evidence() {
    let c=context(); let mut a=cohort(&c,1,DataRole::Train); let mut t=teacher(&c,&a);
    t.clock.kind=ClockKind::ReceiverOnly; a.teacher=Some(t);
    assert!(ValidatedDataset::new(c,vec![a]).is_err());
}
#[test]
fn teacher_room_phase_time_frame_and_validated_source_are_bound() {
    let c=context(); let a=cohort(&c,1,DataRole::Train); let t=teacher(&c,&a);
    let mutants: &[fn(&mut TeacherEvidence)] = &[
        |x|x.room_digest="elsewhere".into(),|x|x.label=RoomLabel::OccupiedMoving,
        |x|x.first_time_ns=x.last_time_ns+1,|x|x.frame_ids=vec![999],|x|x.validation_receipt.clear(),
        |x|x.source_id.clear(),|x|x.trial_id="different-trial".into()];
    for mutate in mutants {
        let mut candidate=a.clone(); let mut evidence=t.clone(); mutate(&mut evidence);
        candidate.teacher=Some(evidence);
        assert!(ValidatedDataset::new(c.clone(),vec![candidate]).is_err());
    }
}
#[test]
fn context_rejects_unicode_unbounded_ids_and_unsorted_sources() {
    let mut c=context(); c.owner_scope="untrusted-\u{202e}".into(); assert!(c.validate().is_err());
    c=context();c.owner_scope="x".repeat(257);assert!(c.validate().is_err());
    c=context();let mut second=c.sources[0].clone();second.source_id=0;second.link_id="other".into();
    c.sources.push(second);assert!(c.validate().is_err());
}

#[test]
fn nonempty_teacher_receipt_claim_never_replaces_validated_registry() {
    let mut c=context(); c.sources[0].signal_kind=RfModality::WifiCsi;
    let mut a=cohort(&c,1,DataRole::Train); a.views[0].signal_kind=RfModality::WifiCsi;
    a.provenance=CaptureProvenance::CapturedReplay; a.views[0].provenance=a.provenance;
    a.teacher=Some(teacher(&c,&a));
    assert!(ValidatedDataset::new(c,vec![a]).unwrap_err().contains("registry resolver unavailable"));
}

#[test]
fn selected_sources_require_complete_aligned_windows_and_bound_link_indices() {
    let mut c=context(); let mut second=c.sources[0].clone();
    second.source_id=2; second.link_id="second-link".into(); c.sources.push(second);
    let mut a=cohort(&c,1,DataRole::Train);
    let mut view=a.views[0].clone();view.source_id=2;view.link_id="second-link".into();
    a.views.push(view);
    assert!(validate_cohort(&c,&a).is_ok());
    a.views[1].first_time_ns=1000;a.views[1].last_time_ns=1002;
    assert!(validate_cohort(&c,&a).is_err());
    a.views[1].first_time_ns=a.views[0].first_time_ns;a.views[1].last_time_ns=a.views[0].last_time_ns;
    a.views[1].link_index=1;assert!(validate_cohort(&c,&a).is_err());
}
#[test]
fn unrecognized_fields_cannot_silently_claim_validated_sources() {
    let c=context();let a=cohort(&c,1,DataRole::Train);
    let mut json=serde_json::to_value(a).unwrap();json["validated"]=true.into();
    assert!(serde_json::from_value::<CaptureCohort>(json).is_err());
}
#[test]
fn tensor_cannot_promote_unknown_phase_to_calibrated() {
    let mut c=context();c.sources[0].feature_count=56;
    let mut t=native_frame().to_canonical(vec![LinkGeometry{tx_pos:[0.0;3],rx_pos:[1.0,0.0,0.0]}]).unwrap();
    t.calibration.gain_offset_db=0.0;t.calibration.phase_calibrated=false;
    let mut m=metadata();m.phase=PhaseAvailability::Calibrated;
    m.frame_ids=vec![7;8];m.sequence_ids=(1..=8).collect();
    assert!(SourceView::from_tensor(&c.sources[0],&t,m).unwrap_err().contains("phase state"));
}

#[test]
fn inference_is_unlabelled_and_cannot_enter_training_or_evaluation_dataset() {
    let c=context();let mut a=cohort(&c,1,DataRole::Inference);
    assert!(validate_cohort(&c,&a).is_err());
    a.label=RoomLabel::Unknown;assert!(validate_cohort(&c,&a).is_ok());
    assert!(ValidatedDataset::new(c.clone(),vec![a.clone()]).is_err());
    a.role=DataRole::Train;assert!(validate_cohort(&c,&a).is_err());
}
