//! SYNTHETIC packets exercising production public APIs; no hardware evidence.
use super::*;
use ruview_unified::RfModality;

const START: u64 = 1_000_000_000;
fn context() -> CaptureContext {
    CaptureContext { owner_scope:"synthetic-owner".into(),installation_id:"synthetic-install".into(),
        room_digest:"synthetic-room".into(),revision:1,sources:(1..=2).map(|id| SourceSpec {
            source_id:id,device_id:format!("synthetic-{id}"),grid_id:"ht20-64-raw".into(),
            feature_count:64,signal_kind:RfModality::WifiCsi,center_frequency_hz:2_437_000_000,
            bandwidth_hz:20_000_000,link_id:format!("tx-rx-{id}"),link_index:0,geometry:None }).collect() }
}
fn session() -> CaptureSession { CaptureSession::new(context(),"synthetic-run".into()).unwrap() }
fn request(role:DataRole,label:RoomLabel,id:&str) -> PhaseRequest {
    PhaseRequest {trial_id:format!("synthetic-trial-{id}"),session_id:format!("synthetic-session-{id}"),
        role,label,duration_ms:MIN_PHASE_MS, lead_in_ms: 0,conditions_confirmed:true}
}
fn packet(source_id:u8,sequence:u32,values:&[f64]) -> RawSourceFrame<'_> {
    RawSourceFrame {source_id,grid_id:"ht20-64-raw",frequency_hz:2_437_000_000,
        bandwidth_hz:20_000_000,sequence,amplitudes:values}
}
fn window(s:&mut CaptureSession,at:u64,start_sequence:u32,gain:f64) {
    let values=vec![gain;64];
    for t in 0..WINDOW_PACKETS {
        for source in 1..=2 {
            s.ingest(packet(source,start_sequence.wrapping_add(t as u32),&values),
                at+t as u64*10_000_000+u64::from(source)).unwrap();
        }
    }
}
fn phase(s:&mut CaptureSession,at:u64,seq:u32,role:DataRole,label:RoomLabel,id:&str,gain:f64) {
    s.begin_phase(request(role,label,id),at).unwrap();
    for quarter in 0..COHORTS_PER_PHASE {
        window(s,at+quarter as u64*3_000_000_000+1_000_000,seq+quarter as u32*8,gain);
    }
    s.finish_phase(at+MIN_PHASE_MS*1_000_000).unwrap();
}

#[test]
fn four_confirmed_phase_quarters_seal_complete_source_cohorts_without_raw_receipt() {
    let mut s=session();
    phase(&mut s,START,1,DataRole::Train,RoomLabel::Empty,"empty",2.0);
    assert_eq!(s.cohorts().len(),4);
    assert!(s.cohorts().iter().all(|c|c.views.len()==2 && c.views.iter().all(|v|v.samples.len()==8)));
    assert_eq!(s.history()[0].status,PhaseStatus::Sealed);
    let json=serde_json::to_string(&s.history()[0]).unwrap();
    assert!(!json.contains("samples") && !json.contains("amplitudes"));
    assert!(!s.validated_dataset().unwrap().measurement_clock_available());
}
#[test]
fn live_prediction_is_unknown_and_receiver_only_never_an_empty_reference() {
    let mut s=session();window(&mut s,START,1,7.0);
    let c=s.live_cohort(START+80_000_000).unwrap();
    assert_eq!((c.role,c.label),(DataRole::Inference,RoomLabel::Unknown));
    assert!(c.teacher.is_none());
    assert!(c.views.iter().all(|v|v.clock.kind==ClockKind::ReceiverOnly
        && v.phase==PhaseAvailability::MagnitudeOnly && v.provenance==CaptureProvenance::LiveReceiverObserved));
    assert!(ValidatedDataset::new(context(),vec![c]).is_err());
}
#[test]
fn gain_and_mean_shift_survive_distinct_operator_phase_labels() {
    let mut s=session();
    phase(&mut s,START,1,DataRole::Train,RoomLabel::Empty,"empty",2.0);
    phase(&mut s,START+13_000_000_000,33,DataRole::Train,RoomLabel::OccupiedStatic,"static",8.0);
    assert!(s.cohorts()[0].views[0].samples.iter().flatten().all(|v|*v==2.0));
    assert!(s.cohorts()[4].views[0].samples.iter().flatten().all(|v|*v==8.0));
}
#[test]
fn missing_source_dropout_never_pads_or_seals_partial_phase() {
    let mut s=session();s.begin_phase(request(DataRole::HeldOut,RoomLabel::OccupiedMoving,"dropout"),START).unwrap();
    let values=vec![3.0;64];
    for t in 0..64 {s.ingest(packet(1,t,&values),START+1+t as u64*10_000_000).unwrap();}
    assert!(s.finish_phase(START+MIN_PHASE_MS*1_000_000).is_err());
    assert!(s.cohorts().is_empty());
    assert_eq!(s.history()[0].status,PhaseStatus::Failed);
    assert!(s.history()[0].reason.as_ref().unwrap().contains("four complete"));
}
#[test]
fn cancel_drops_only_inflight_phase_and_keeps_sealed_role_history() {
    let mut s=session();phase(&mut s,START,1,DataRole::Train,RoomLabel::Empty,"sealed",2.0);
    s.begin_phase(request(DataRole::Validation,RoomLabel::Empty,"cancel"),START+13_000_000_000).unwrap();
    window(&mut s,START+13_001_000_000,33,5.0);s.cancel_phase();
    assert_eq!(s.cohorts().len(),4);assert_eq!(s.history()[1].status,PhaseStatus::Cancelled);
    assert!(!s.phase_active());
}
#[test]
fn source_phy_grid_unknown_source_and_nonfinite_changes_fail_active_phase() {
    for change in 0..5 {
        let mut s=session();s.begin_phase(request(DataRole::Train,RoomLabel::Empty,"bad"),START).unwrap();
        let mut values=vec![2.0;64];if change==4{values[0]=f64::NAN;}
        let mut f=packet(1,1,&values);
        match change {0=>f.source_id=3,1=>f.grid_id="different-phy",2=>f.frequency_hz+=5_000_000,
            3=>f.bandwidth_hz*=2,_=>{}}
        assert!(s.ingest(f,START+1).is_err());assert!(!s.phase_active());
        assert_eq!(s.history()[0].status,PhaseStatus::Failed);
    }
}
#[test]
fn forward_sequence_wrap_extends_ordinals_and_replay_is_rejected() {
    let mut s=session();window(&mut s,START,u32::MAX-3,2.0);
    let c=s.live_cohort(START+80_000_000).unwrap();
    assert!(c.views.iter().all(|v|v.sequence_ids.windows(2).all(|w|w[1]>w[0])));
    let values=vec![2.0;64];
    assert!(s.ingest(packet(1,3,&values),START+90_000_000).unwrap_err().contains("replayed"));
    assert!(s.ingest(packet(1,2,&values),START+91_000_000).is_err());
}
#[test]
fn phase_boundary_never_reuses_live_window_or_previous_phase_frames() {
    let mut s=session();window(&mut s,START,1,99.0);
    let begin=START+100_000_000;s.begin_phase(request(DataRole::Train,RoomLabel::Empty,"first"),begin).unwrap();
    let values=vec![2.0;64];
    for t in 0..7 {for source in 1..=2 {s.ingest(packet(source,9+t,&values),
        begin+1+t as u64*10_000_000+u64::from(source)).unwrap();}}
    assert!(s.finish_phase(begin+MIN_PHASE_MS*1_000_000).is_err());
    assert!(s.cohorts().is_empty());
}
#[test]
fn interleaved_train_validation_holdout_keep_distinct_frames_and_frozen_roles() {
    let mut s=session();
    for (i,role) in [DataRole::Train,DataRole::HeldOut,DataRole::Validation].into_iter().enumerate() {
        phase(&mut s,START+i as u64*13_000_000_000,1+i as u32*32,role,RoomLabel::Empty,&format!("{i}"),2.0);
    }
    let dataset=s.validated_dataset().unwrap();
    assert_eq!((dataset.train().len(),dataset.validation().len(),dataset.held_out().len()),(4,4,4));
    let mut wrong=request(DataRole::Validation,RoomLabel::Empty,"0");wrong.session_id="new-session".into();
    assert!(s.begin_phase(wrong,START+40_000_000_000).unwrap_err().contains("frozen"));
}
#[test]
fn preconditions_duration_and_unlabelled_training_requests_are_rejected() {
    for change in 0..5 {
        let mut s=session();let mut p=request(DataRole::Train,RoomLabel::Empty,"invalid");
        match change {0=>p.conditions_confirmed=false,1=>p.duration_ms=MIN_PHASE_MS-1,
            2=>p.duration_ms=MAX_PHASE_MS+1,3=>p.label=RoomLabel::Unknown,_=>p.role=DataRole::Inference}
        assert!(s.begin_phase(p,START).is_err());
    }
}
#[test]
fn early_finish_leaves_phase_active_and_live_staleness_is_checked() {
    let mut s=session();s.begin_phase(request(DataRole::Train,RoomLabel::Empty,"early"),START).unwrap();
    window(&mut s,START+1_000_000,1,2.0);
    assert!(s.finish_phase(START+1_000_000_000).is_err());assert!(s.phase_active());
    assert!(s.live_cohort(START+MAX_AGE_NS+100_000_000).is_err());
}
#[test]
fn quarter_windows_are_nonoverlapping_even_under_high_packet_rate() {
    let mut s=session();s.begin_phase(request(DataRole::Train,RoomLabel::Empty,"rate"),START).unwrap();
    for w in 0..10 {window(&mut s,START+1_000_000+w as u64*100_000_000,1+w*8,2.0);}
    for q in 1..4 {window(&mut s,START+q as u64*3_000_000_000+1_000_000,81+(q-1)*8,2.0);}
    let receipt=s.finish_phase(START+MIN_PHASE_MS*1_000_000).unwrap();
    assert_eq!(receipt.cohort_count,4);
    for pair in s.cohorts().windows(2) {assert!(pair[0].views[0].last_time_ns<pair[1].views[0].first_time_ns);}
}

#[test]
fn canonical_56_features_preserve_raw_he256_grid_identity_and_gain() {
    let mut c=context();for spec in &mut c.sources {spec.grid_id="he20-256-raw".into();spec.feature_count=56;}
    let mut s=CaptureSession::new(c,"synthetic-canonical".into()).unwrap();
    let values=vec![8.0;56];
    for t in 0..8 {for source in 1..=2 {
        let mut f=packet(source,t,&values);f.grid_id="he20-256-raw";
        s.ingest(f,START+1+t as u64*10_000_000+u64::from(source)).unwrap();
    }}
    let cohort=s.live_cohort(START+80_000_000).unwrap();
    assert!(cohort.views.iter().all(|v|v.grid_id=="he20-256-raw" && v.samples[0].len()==56
        && v.samples.iter().flatten().all(|x|*x==8.0)));
}

#[test]
fn progress_exposes_admitted_counts_missing_stale_reasons_and_no_raw_data() {
    let mut s=session();let initial=s.progress(START);
    assert!(initial.sources.iter().all(|v|v.reason.as_deref()==Some("missing_source") && v.admitted_packets==0));
    s.begin_phase(request(DataRole::Train,RoomLabel::Empty,"progress"),START).unwrap();
    window(&mut s,START+1_000_000,1,123.0);
    let progress=s.progress(START+100_000_000);
    assert!(progress.sources.iter().all(|v|v.admitted_packets==8 && v.phase_admitted_packets==8
        && v.live_buffer_frames==8 && v.window_eligible));
    let values=vec![123.0;64];assert!(s.ingest(packet(1,8,&values),START+200_000_000).is_err());
    assert_eq!(s.progress(START+200_000_000).sources[0].admitted_packets,8);
    let json=serde_json::to_string(&progress).unwrap();
    assert!(!json.contains("samples") && !json.contains("amplitudes") && !json.contains("frame_ids") && !json.contains("123.0"));
    assert!(s.progress(START+MAX_AGE_NS+200_000_000).sources.iter().all(|v|v.reason.as_deref()==Some("stale_source") && !v.window_eligible));
}

#[test]
fn countdown_excludes_operator_transition_packets_from_every_labelled_window() {
    let mut s=session();let mut req=request(DataRole::Train,RoomLabel::Empty,"countdown");req.lead_in_ms=5_000;
    s.begin_phase(req,START).unwrap();
    window(&mut s,START+1_000_000,1,99.0);
    let p=s.progress(START+1_000_000_000).phase.unwrap();
    assert_eq!(p.stage,"lead_in");assert_eq!(p.capture_elapsed_ms,0);assert_eq!(p.lead_in_remaining_ms,4_000);
    for quarter in 0..4 {window(&mut s,START+5_000_000_000+quarter*3_000_000_000+1_000_000,9+quarter as u32*8,2.0);}
    assert!(s.finish_phase(START+MIN_PHASE_MS*1_000_000).is_err(),"countdown cannot satisfy capture duration");
    s.finish_phase(START+(MIN_PHASE_MS+5_000)*1_000_000).unwrap();
    assert!(s.cohorts().iter().all(|c|c.views.iter().all(|v|v.samples.iter().flatten().all(|x|*x==2.0))));
}
