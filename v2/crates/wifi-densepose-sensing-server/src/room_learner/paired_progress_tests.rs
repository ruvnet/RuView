//! SYNTHETIC current/read-only progress, not physical qualification.
use super::*;
use super::super::tests::{context,runtime,phase,train_phases,finished,START};
use crate::room_learner::recovery_store::RecoveryStore;
fn binding()->String {"a".repeat(64)}
fn publication()->Option<PublishedProgressRoom>{Some(PublishedProgressRoom{binding_digest:binding(),revision:1})}
fn configured(root:&Path)->Arc<RoomLearningRuntime>{
    let rt=runtime(root);let mut ctx=context();ctx.room_digest=binding();
    rt.configure(&rt.mutation_guard().unwrap(),ctx,"paired-synthetic-run".into(),true).unwrap();rt
}
fn snapshot(rt:&RoomLearningRuntime,now:u64)->PairedCalibrationProgress {
    rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",publication(),&[1,2],rt.context().as_ref().map(|c|c.digest()).as_deref(),now).unwrap()
}
fn assert_private_free(value:&PairedCalibrationProgress) {
    let s=serde_json::to_string(value).unwrap();
    for forbidden in ["owner_scope","mutation_guard","trial_id","session_id","frame_ids","sequence_ids","last_accepted_sequence","source_id","encoder_weights","head_weights","occupied_probability","heart_rate","amplitudes","\"samples\"","training_data_sha256","\"geometry\""] {
        assert!(!s.contains(forbidden),"paired progress leaked {forbidden}");
    }
    assert!(!s.contains("NaN")&&!s.contains("Infinity"));
}
#[test]
fn setup_and_foreign_publication_withhold_current_counts_and_private_identity() {
    let dir=tempfile::tempdir().unwrap();let rt=configured(dir.path());
    let missing=rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",None,&[1,2],None,START).unwrap();
    assert_eq!(missing.blocker,Some("room_not_published"));assert!(missing.conditions.is_none());
    let mut other=publication().unwrap();other.binding_digest="b".repeat(64);
    let mismatch=rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",Some(other),&[1,2],None,START).unwrap();
    assert_eq!(mismatch.blocker,Some("context_mismatch"));assert!(!mismatch.context_match);
    assert!(mismatch.conditions.is_none()&&mismatch.phase.is_none()&&mismatch.sources.is_none());
    assert_eq!(mismatch.qualification.state,"not_reported");assert_eq!(mismatch.selection.state,"not_reported");
    let foreign=rt.paired_progress_snapshot("other-install","cal-boot-synthetic",publication(),&[1,2],None,START).unwrap();
    assert_eq!(foreign.blocker,Some("context_mismatch"));assert_private_free(&foreign);
}
#[test]
fn phases_are_bounded_and_recorded_counts_are_separate_from_qualification_and_save() {
    let dir=tempfile::tempdir().unwrap();let rt=configured(dir.path());
    phase(&rt,DataRole::Train,RoomLabel::Empty,0);
    let before=snapshot(&rt,START+12_000_000_000);
    assert_eq!(before.conditions.as_ref().unwrap()[0].train_recorded,4);
    assert_eq!(before.qualification.state,"not_reported");assert_eq!(before.save_status,"not_reported");
    rt.begin_phase(&rt.mutation_guard().unwrap(),PhaseRequest {trial_id:"private-trial-name".into(),session_id:"private-session-name".into(),role:DataRole::Train,label:RoomLabel::OccupiedStatic,duration_ms:12_000,lead_in_ms:5000,conditions_confirmed:true},START+13_000_000_000).unwrap();
    let guard=rt.mutation_guard().unwrap();let preparing=snapshot(&rt,START+14_000_000_000);
    assert_eq!(preparing.phase.as_ref().unwrap().stage,"lead_in");assert_eq!(preparing.phase.as_ref().unwrap().lead_in_remaining_ms,4000);
    assert_eq!(preparing.observed_at_unix_ms+10_000,preparing.expires_at_unix_ms);
    let incomplete=snapshot(&rt,START+100_000_000_000);
    let p=incomplete.phase.as_ref().unwrap();assert_eq!(p.stage,"incomplete");assert_eq!(p.capture_elapsed_ms,p.target_ms);assert_eq!(p.representative_cohorts,0);
    assert_eq!(guard,rt.mutation_guard().unwrap(),"GET projection must not consume workflow revisions");
    let s=serde_json::to_string(&incomplete).unwrap();assert!(!s.contains("private-trial-name")&&!s.contains("private-session-name"));assert_private_free(&incomplete);
}
#[test]
fn progress_does_not_checkpoint_or_advance_expired_recovery() {
    let dir=tempfile::tempdir().unwrap();let owner="a".repeat(64);let data=dir.path().join(&owner).join("SensingServer");
    let store=RecoveryStore::new(&data,&owner,"synthetic-install",Some(&[7;32])).unwrap();
    let rt=RoomLearningRuntime::new_with_recovery_at(&data,&owner,"synthetic-install",store,START).unwrap();
    let mut ctx=context();ctx.room_digest=binding();rt.configure_for_recovery(&rt.mutation_guard().unwrap(),ctx,"private-recovery-run".into(),true,true,START).unwrap();
    let file=data.join("room-learner-recovery/capture-recovery-v1.bin");let bytes=std::fs::read(&file).unwrap();
    let guard=rt.mutation_guard().unwrap();
    let view=snapshot(&rt,START+86_400_000_000_000);
    assert_eq!(view.save_status,"not_reported");assert_eq!(rt.mutation_guard().unwrap(),guard);
    assert!(rt.inner.lock().unwrap().recovery.enabled);
    assert_eq!(std::fs::read(file).unwrap(),bytes);
}
#[test]
fn invalid_bounds_times_and_nonfinite_candidate_are_unavailable_without_raw_values() {
    let dir=tempfile::tempdir().unwrap();let rt=configured(dir.path());
    assert!(rt.paired_progress_snapshot("synthetic-install","public\ninvalid",publication(),&[],None,START).is_err());
    assert!(rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",publication(),&[],None,0).is_err());
    assert!(rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",Some(PublishedProgressRoom{binding_digest:binding(),revision:MAX_SAFE+1}),&[],None,START).is_err());
    assert!(rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",Some(PublishedProgressRoom{binding_digest:binding(),revision:0}),&[],None,START).is_err());
    assert!(rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",publication(),&[1,2],Some(&"b".repeat(64)),START).is_err(),"ready-source class from a prior context must not supply current readiness");
    assert_private_free(&snapshot(&rt,START));
}
async fn fitted(root:&Path)->Arc<RoomLearningRuntime>{
    let rt=configured(root);train_phases(&rt);
    rt.start_training(&rt.mutation_guard().unwrap(),FitConfig{epochs:8,learning_rate:0.5,hidden_dim:2,embedding_dim:2,temporal_tokens:2,..FitConfig::default()}).unwrap();
    assert_eq!(finished(&rt).await["state"],"ready");
    for (i,l) in [RoomLabel::Empty,RoomLabel::OccupiedStatic,RoomLabel::OccupiedMoving,RoomLabel::AdjacentOnly].into_iter().enumerate(){phase(&rt,DataRole::HeldOut,l,i as u32+4);}
    rt
}
#[tokio::test]
async fn verified_selection_is_not_mac_save_and_corrupt_artifacts_withhold_claims() {
    let dir=tempfile::tempdir().unwrap();let rt=fitted(dir.path()).await;
    let recorded=snapshot(&rt,START+104_000_000_000);assert!(recorded.conditions.as_ref().unwrap().iter().all(|c|c.train_recorded==4&&c.held_out_recorded==4));assert_eq!(recorded.qualification.state,"not_qualified");
    let q=rt.qualify(&rt.mutation_guard().unwrap()).unwrap();let qualified=snapshot(&rt,START+104_000_000_000);
    assert_eq!(qualified.qualification.state,"qualified");assert_eq!(qualified.stage,"selection");
    rt.select_model(&rt.mutation_guard().unwrap(),&q.receipt.model_sha256,&q.receipt_sha256).unwrap();
    let now=super::super::selection_tests::feed(&rt,START+105_000_000_000,&[1,2],6.0);
    let selected=snapshot(&rt,now);assert_eq!(selected.selection.state,"selected");assert!(selected.selection.current_usable);assert_eq!(selected.save_status,"not_reported");assert_private_free(&selected);
    let context=rt.context().unwrap().digest();let unready=rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",publication(),&[],Some(&context),now).unwrap();
    assert_eq!(unready.selection.state,"selected");assert!(!unready.selection.current_usable);assert_eq!(unready.blocker,Some("sources_unready"));
    let path=dir.path().join("a".repeat(64)).join("SensingServer/room-learner").join(rt.context().unwrap().digest()).join("qualifications").join(format!("{}.json",q.receipt_sha256));
    let original=std::fs::read(&path).unwrap();
    std::fs::write(&path,b"{\"synthetic_corruption\":true}").unwrap();
    let corrupt=snapshot(&rt,now);assert_ne!(corrupt.qualification.state,"qualified");assert_eq!(corrupt.selection.state,"unavailable");assert!(!corrupt.selection.current_usable);assert_private_free(&corrupt);
    #[cfg(unix)] {
        let outside=dir.path().join("synthetic-other-file.json");std::fs::write(&outside,original).unwrap();std::fs::remove_file(&path).unwrap();std::os::unix::fs::symlink(outside,&path).unwrap();
        let alias=snapshot(&rt,now);assert_ne!(alias.qualification.state,"qualified");assert_eq!(alias.selection.state,"unavailable");assert!(!alias.selection.current_usable);
    }
}
#[tokio::test]
async fn nonfinite_model_and_foreign_owner_witness_do_not_grant_current_progress() {
    let dir=tempfile::tempdir().unwrap();let rt=fitted(dir.path()).await;
    {let mut state=rt.inner.lock().unwrap();let model=Arc::make_mut(state.model.as_mut().unwrap());model.encoder_weights[0]=f32::NAN;}
    assert!(rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",publication(),&[1,2],rt.context().as_ref().map(|c|c.digest()).as_deref(),START+104_000_000_000).is_err());
    let dir2=tempfile::tempdir().unwrap();let good=fitted(dir2.path()).await;let q=good.qualify(&good.mutation_guard().unwrap()).unwrap();good.select_model(&good.mutation_guard().unwrap(),&q.receipt.model_sha256,&q.receipt_sha256).unwrap();
    std::fs::write(dir2.path().join("a".repeat(64)).join("SensingServer/account-owner-v1.json"),br#"{"schema":"ruview.account-owner.v1","scope_key":"foreign"}"#).unwrap();
    let denied=snapshot(&good,START+104_000_000_000);assert_eq!(denied.selection.state,"unavailable");assert!(!denied.selection.current_usable);
}

#[tokio::test]
async fn restored_model_with_no_temporary_rows_is_not_reported_as_lost_or_invalid() {
    let dir=tempfile::tempdir().unwrap();let rt=fitted(dir.path()).await;let ctx=rt.context().unwrap();
    rt.configure(&rt.mutation_guard().unwrap(),ctx,"new-progress-session".into(),false).unwrap();
    let progress=snapshot(&rt,START+104_000_000_000);
    assert_eq!(progress.state,"ready");assert_eq!(progress.stage,"held_out");
    assert_eq!(progress.blocker,Some("training_lineage_not_reported"));
    assert!(progress.conditions.as_ref().unwrap().iter().all(|c|c.train_recorded==0&&c.held_out_recorded==0));
    assert_ne!(progress.stage,"blocked");assert_eq!(progress.save_status,"not_reported");
}

#[tokio::test]
async fn another_rooms_selected_model_does_not_block_or_label_current_room_setup() {
    let dir=tempfile::tempdir().unwrap();let rt=fitted(dir.path()).await;
    let q=rt.qualify(&rt.mutation_guard().unwrap()).unwrap();rt.select_model(&rt.mutation_guard().unwrap(),&q.receipt.model_sha256,&q.receipt_sha256).unwrap();
    let mut next=rt.context().unwrap();next.room_digest="b".repeat(64);rt.configure(&rt.mutation_guard().unwrap(),next,"current-room-b".into(),true).unwrap();
    let context=rt.context().unwrap().digest();
    let view=rt.paired_progress_snapshot("synthetic-install","cal-boot-synthetic",Some(PublishedProgressRoom{binding_digest:"b".repeat(64),revision:2}),&[],Some(&context),START+104_000_000_000).unwrap();
    assert!(view.context_match);assert_eq!(view.stage,"train");assert_eq!(view.selection.state,"not_reported");assert!(view.selection.model_sha256.is_none());
    assert_eq!(view.qualification.state,"not_reported");assert!(view.qualification.model_sha256.is_none());
    assert!(view.conditions.as_ref().unwrap().iter().all(|c|c.train_recorded==0&&c.held_out_recorded==0));assert_private_free(&view);
}
