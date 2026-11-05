//! SYNTHETIC managed lifecycle, private persistence and cold diagnostics.
use super::tests::{context, finished, phase, runtime, train_phases, START};
use super::*;
async fn fitted(root: &Path) -> Arc<RoomLearningRuntime> {
    let rt = runtime(root);
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context(),
        "evaluation-fixture".into(),
        true,
    )
    .unwrap();
    train_phases(&rt);
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 2,
            ..FitConfig::default()
        },
    )
    .unwrap();
    let status = finished(&rt).await;
    assert_eq!(status["state"], "ready", "{status}");
    assert!(
        status["training_report"]["backbone_delta_l2"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    rt
}
fn evaluation_directory(root: &Path, report: &Value) -> std::path::PathBuf {
    root.join("a".repeat(64))
        .join("SensingServer/room-learner")
        .join(report["context_digest"].as_str().unwrap())
        .join("evaluations")
        .join(report["model_sha256"].as_str().unwrap())
        .join("held-out")
}
#[tokio::test]
async fn frozen_capture_evaluate_persist_cold_resume_restores_exact_diagnostic_without_current_train_claim(
) {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4);
    phase(&rt, DataRole::HeldOut, RoomLabel::OccupiedStatic, 5);
    let result = rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut)
        .unwrap();
    assert_eq!(result["durable"], true);
    assert_eq!(result["train_lineage_checked"], true);
    assert_eq!(result["report"]["samples"], 8);
    assert_eq!(result["report"]["declared_trial_count"], 2);
    assert_eq!(result["report"]["independence_verified"], false);
    assert_eq!(result["adoption_authorized"], false);
    let key = result["receipt_sha256"].as_str().unwrap();
    assert!(evaluation_directory(dir.path(), &result)
        .join(format!("{key}.json"))
        .is_file());
    let cold = runtime(dir.path());
    cold.configure(
        &cold.mutation_guard().unwrap(),
        context(),
        "cold-evaluation".into(),
        false,
    )
    .unwrap();
    let status = cold.status(START);
    let restored = &status["last_evaluation"];
    assert_eq!(status["candidate_loaded"], true);
    assert!(status["evaluation_warning"].is_null());
    assert_eq!(restored["receipt_sha256"], result["receipt_sha256"]);
    assert_eq!(restored["report"], result["report"]);
    assert_eq!(restored["restored"], true);
    assert_eq!(restored["train_lineage_checked"], false);
    assert_eq!(restored["historical_train_lineage_checked"], true);
    assert_eq!(status["physically_validated"], false);
    assert_eq!(status["control_detector_changed"], false);
    phase(&cold, DataRole::HeldOut, RoomLabel::Empty, 6);
    let fresh = cold
        .evaluate(&cold.mutation_guard().unwrap(), DataRole::HeldOut)
        .unwrap();
    assert_eq!(fresh["train_lineage_checked"], false);
    for raw in ["frame_ids", "sequence_ids", "\"views\"", "\"samples\":["] {
        assert!(!serde_json::to_string(&cold.status(START))
            .unwrap()
            .contains(raw));
    }
}
#[tokio::test]
async fn corrupted_report_is_retained_and_warned_while_the_valid_candidate_remains_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4);
    let result = rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut)
        .unwrap();
    let pointer = evaluation_directory(dir.path(), &result).join("latest-v1.json");
    crate::companion::atomic_private_write(&pointer, b"broken evaluation pointer").unwrap();
    let cold = runtime(dir.path());
    cold.configure(
        &cold.mutation_guard().unwrap(),
        context(),
        "corrupt-eval".into(),
        false,
    )
    .unwrap();
    let status = cold.status(START);
    assert_eq!(status["candidate_loaded"], true);
    assert_eq!(status["state"], "ready");
    assert!(status["last_evaluation"].is_null());
    let warning = status["evaluation_warning"].as_str().unwrap();
    assert!(!warning.is_empty() && warning.len() <= 256);
    assert_eq!(
        std::fs::read(&pointer).unwrap(),
        b"broken evaluation pointer"
    );
    assert_eq!(status["physically_validated"], false);
    phase(&cold, DataRole::HeldOut, RoomLabel::Empty, 6);
    assert!(cold
        .evaluate(&cold.mutation_guard().unwrap(), DataRole::HeldOut)
        .is_err());
    let failed = cold.status(START);
    assert!(failed["last_evaluation"].is_null());
    assert!(failed["evaluation_warning"].is_string());
    assert_eq!(failed["candidate_loaded"], true);
}
#[tokio::test]
async fn persistence_failure_never_claims_a_durable_evaluation_or_changes_control() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4);
    let root = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner")
        .join(context().digest());
    crate::companion::atomic_private_write(&root.join("evaluations"), b"filesystem obstacle")
        .unwrap();
    assert!(rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut)
        .is_err());
    let status = rt.status(START);
    assert!(status["last_evaluation"].is_null());
    assert!(status["evaluation_warning"].is_string());
    assert_eq!(status["candidate_loaded"], true);
    assert_eq!(status["control_detector_changed"], false);
    assert!(rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::Train)
        .is_err());
}

pub(super) fn assert_no_raw_observation_arrays(value: &Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                assert!(
                    !([
                        "samples",
                        "views",
                        "frame_ids",
                        "sequence_ids",
                        "amplitudes",
                        "native_iq",
                        "valid_mask"
                    ]
                    .contains(&key.as_str())
                        && child.is_array()),
                    "raw observation array: {key}"
                );
                assert_no_raw_observation_arrays(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_raw_observation_arrays(item);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn comparison_optin_records_bound_outcomes_without_changing_default_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4);
    phase(&rt, DataRole::HeldOut, RoomLabel::OccupiedStatic, 5);
    let expected = rt.mutation_guard().unwrap();
    let ordinary = rt.evaluate(&expected, DataRole::HeldOut).unwrap();
    assert!(ordinary.get("comparison_ledger").is_none());
    assert!(!evaluation_directory(dir.path(), &ordinary)
        .join("comparison-ledgers-v1")
        .exists());
    let recorded = rt
        .evaluate_with_comparison(&rt.mutation_guard().unwrap(), DataRole::HeldOut, true)
        .unwrap();
    assert_eq!(recorded["receipt"], ordinary["receipt"]);
    assert_eq!(recorded["receipt_sha256"], ordinary["receipt_sha256"]);
    assert_eq!(recorded["comparison_recording_requested"], true);
    assert_eq!(recorded["comparison_ledger"]["cohort_count"], 8);
    assert_eq!(recorded["comparison_ledger"]["physically_validated"], false);
    assert_eq!(recorded["comparison_ledger"]["adoption_authorized"], false);
    assert!(recorded.get("comparison_warning").is_none());
    // Later sealed phases append the replay fence. Historical sidecar custody
    // resolves its original fence in the verified chain; it never rewinds head.
    phase(&rt, DataRole::HeldOut, RoomLabel::OccupiedMoving, 6);
    let loaded_model = rt.inner.lock().unwrap().model.as_ref().unwrap().clone();
    let ledger = rt
        .store
        .load_comparison_ledger_summary(
            &loaded_model,
            DataRole::HeldOut,
            ordinary["receipt_sha256"].as_str().unwrap(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(ledger.cohort_count, 8);
    let cold = runtime(dir.path());
    cold.configure(
        &cold.mutation_guard().unwrap(),
        context(),
        "comparison-cold".into(),
        false,
    )
    .unwrap();
    let restored = cold.status(START);
    assert_eq!(
        restored["last_evaluation"]["receipt_sha256"],
        ordinary["receipt_sha256"]
    );
    assert!(restored["last_evaluation"]
        .get("comparison_ledger")
        .is_none());
    assert_eq!(restored["last_evaluation"]["train_lineage_checked"], false);
}
#[tokio::test]
async fn optional_comparison_failure_keeps_successful_rf_receipt_and_candidate_visible() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4);
    let expected = rt.mutation_guard().unwrap();
    let ordinary = rt.evaluate(&expected, DataRole::HeldOut).unwrap();
    crate::companion::atomic_private_write(
        &evaluation_directory(dir.path(), &ordinary).join("comparison-ledgers-v1"),
        b"blocked",
    )
    .unwrap();
    let result = rt
        .evaluate_with_comparison(&rt.mutation_guard().unwrap(), DataRole::HeldOut, true)
        .unwrap();
    assert_eq!(result["receipt"], ordinary["receipt"]);
    assert_eq!(result["durable"], true);
    assert!(result["comparison_ledger"].is_null());
    assert!(result["comparison_warning"].is_string());
    let status = rt.status(START);
    assert_eq!(status["candidate_loaded"], true);
    assert_eq!(
        status["last_evaluation"]["receipt_sha256"],
        ordinary["receipt_sha256"]
    );
    assert!(status["evaluation_warning"].is_null());
}

#[tokio::test]
async fn reference_actual_decoder_server_phase_same_rf_cohorts_stay_unknown_and_keep_receipt(){
    use crate::companion_references::{ReferenceMessage,tests as phone};
    let dir=tempfile::tempdir().unwrap();let rt=runtime(dir.path());let mut ctx=context();ctx.room_digest="b".repeat(64);
    rt.configure(&rt.mutation_guard().unwrap(),ctx.clone(),"reference-synthetic-run".into(),true).unwrap();train_phases(&rt);
    rt.start_training(&rt.mutation_guard().unwrap(),FitConfig{epochs:2,..FitConfig::default()}).unwrap();assert_eq!(finished(&rt).await["state"],"ready");
    let (_phone_dir,refs,_revoke,grant)=phone::setup();let at=rt.receiver_now_ns();refs.permit("phone",true,at/1_000_000_000).unwrap();
    assert!(rt.with_reference_scope(&ctx.room_digest,&[1,2],at,|_|Ok(())).is_err());
    rt.begin_phase(&rt.mutation_guard().unwrap(),PhaseRequest{trial_id:"reference-check".into(),session_id:"reference-check-session".into(),role:DataRole::HeldOut,label:RoomLabel::Empty,duration_ms:12_000,lead_in_ms:0,conditions_confirmed:true},at).unwrap();
    let guard=rt.mutation_guard().unwrap();
    let begin_bytes=serde_json::to_vec(&json!({"type":"begin","schema":"ruview.companion.calibration-reference.v1","lineage":{"capture_session_id":"synthetic-capture","coordinate_frame_id":"synthetic-frame","clock_model_id":"arkit-monotonic+session-wall-offset-v1","room_binding_digest":ctx.room_digest,"room_revision":1}})).unwrap();
    let begin=ReferenceMessage::decode(&begin_bytes,at/1_000_000).unwrap();
    let ready=rt.with_reference_scope(&ctx.room_digest,&[1,2],at,|scope|refs.admit(&grant,scope,begin,at)).unwrap();
    assert_eq!(rt.mutation_guard().unwrap(),guard,"reference ingress consumed the RF mutation guard");
    assert!(rt.with_reference_scope("different-room",&[1,2],at,|_|Ok(())).is_err());
    assert!(rt.with_reference_scope(&ctx.room_digest,&[1,9],at,|_|Ok(())).is_err());
    for quarter in 0..4u64 {
        for sample in 0..8u64 {for source in 1..=2u8 {
            let t=at+quarter*3_000_000_000+1_000_000+sample*10_000_000+u64::from(source);
            rt.ingest(RawSourceFrame{source_id:source,grid_id:"ht64-canonical8",frequency_hz:2_437_000_000,bandwidth_hz:20_000_000,sequence:1000+quarter as u32*8+sample as u32,amplitudes:&[2.2;8]},t).unwrap();
        }}
        let t=at+quarter*3_000_000_000+35_000_000;
        let obs=phone::observation(&ready.reference_session_id,quarter+1,t);
        rt.with_reference_scope(&ctx.room_digest,&[1,2],t,|scope|refs.admit(&grant,scope,obs,t)).unwrap();
    }
    let snapshot=refs.snapshot(at/1_000_000_000).unwrap();assert_eq!(snapshot.rows.len(),4);
    rt.finish_phase(&rt.mutation_guard().unwrap(),at+12_000_000_000,true).unwrap();
    assert!(rt.with_reference_scope(&ctx.room_digest,&[1,2],at+12_000_000_001,|_|Ok(())).is_err());
    let plain=rt.evaluate(&rt.mutation_guard().unwrap(),DataRole::HeldOut).unwrap();
    let compared=rt.evaluate_with_references(&rt.mutation_guard().unwrap(),DataRole::HeldOut,&snapshot).unwrap();
    assert_eq!(plain["receipt"],compared["receipt"]);assert_eq!(plain["receipt_sha256"],compared["receipt_sha256"]);
    assert_eq!(compared["reference_comparison"]["cohort_count"],4);assert_eq!(compared["reference_comparison"]["unknown_cohorts"],4);
    assert_eq!(compared["reference_comparison"]["associated_reference_rows"],4);assert_eq!(compared["reference_comparison"]["physically_validated"],false);
    let parent=evaluation_directory(dir.path(),&compared).join("reference-comparisons-v1");let path=parent.join(format!("{}.json",compared["reference_comparison"]["sidecar_sha256"].as_str().unwrap()));
    let bytes=std::fs::read(path).unwrap();let text=String::from_utf8(bytes).unwrap();
    for raw in ["visible_body_count","source_time","received_ns","camera","samples","views"]{assert!(!text.contains(raw),"unexpected source payload {raw}");}
    // Optional storage failure cannot change the already-committed ordinary RF receipt.
    std::fs::rename(&parent,parent.with_extension("retained")).unwrap();
    crate::companion::atomic_private_write(&parent,b"synthetic sidecar obstacle").unwrap();
    let warning=rt.evaluate_with_references(&rt.mutation_guard().unwrap(),DataRole::HeldOut,&snapshot).unwrap();
    assert_eq!(warning["receipt_sha256"],plain["receipt_sha256"]);assert!(warning["reference_comparison"].is_null());assert!(warning["comparison_warning"].is_string());
    rt.configure(&rt.mutation_guard().unwrap(),ctx,"replacement-run-same-binding".into(),false).unwrap();
    assert!(rt.evaluate_with_references(&rt.mutation_guard().unwrap(),DataRole::HeldOut,&snapshot).is_err(),"same binding cannot revive an earlier run's references");
    refs.revoke("phone").unwrap();assert!(rt.evaluate_with_references(&rt.mutation_guard().unwrap(),DataRole::HeldOut,&snapshot).is_err());
    assert_eq!(rt.status(at)["candidate_loaded"],true);
}

#[tokio::test]
async fn reference_scope_keeps_lead_in_and_overdue_phase_unlabelled_without_advancing_rf_guard(){
    let dir=tempfile::tempdir().unwrap();let rt=fitted(dir.path()).await;let at=rt.receiver_now_ns();
    rt.begin_phase(&rt.mutation_guard().unwrap(),PhaseRequest{trial_id:"reference-lead-in".into(),session_id:"reference-lead-in-session".into(),role:DataRole::HeldOut,label:RoomLabel::OccupiedStatic,duration_ms:12_000,lead_in_ms:5_000,conditions_confirmed:true},at).unwrap();
    let before=rt.mutation_guard().unwrap();
    assert!(rt.with_reference_scope(&context().room_digest,&[1,2],at,|_|Ok(())).is_err());
    assert!(rt.with_reference_scope(&context().room_digest,&[1,2],at+5_000_000_000,|_|Ok(())).is_ok());
    assert!(rt.with_reference_scope(&context().room_digest,&[1,2],at+17_000_000_000,|_|Ok(())).is_err());
    assert_eq!(rt.mutation_guard().unwrap(),before);
}
