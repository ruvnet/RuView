//! SYNTHETIC fixed-clock public Legacy Runtime status-byte compatibility probe.
use super::*;
use sha2::{Digest, Sha256};
#[test]
fn legacy_public_status_byte_probe() {
    let dir = tempfile::tempdir().unwrap();
    let c = tests::context();
    let path = dir.path().join(&c.owner_scope).join("SensingServer");
    let recovery = crate::room_learner::recovery_store::RecoveryStore::new(
        &path,
        &c.owner_scope,
        &c.installation_id,
        Some(&[7; 32]),
    )
    .unwrap();
    let mut rt = RoomLearningRuntime::new_with_recovery_at(
        &path,
        &c.owner_scope,
        &c.installation_id,
        recovery,
        tests::START,
    )
    .unwrap();
    rt.runtime_epoch = "synthetic-legacy-golden-runtime".into();
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        c.clone(),
        "synthetic-legacy-golden-run".into(),
        true,
        false,
        tests::START,
    )
    .unwrap();
    fn stable(mut value: Value) -> Value {
        if let Some(cap) = value.pointer_mut("/recovery/enable_current_run") {
            if cap["run_id"].is_string() {
                cap["run_id"] = json!("synthetic-generated-ticket");
            }
        }
        value
    }
    let initial = stable(rt.status(tests::START));
    let first = tests::START;
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: "golden-trial".into(),
            session_id: "golden-session".into(),
            role: DataRole::Train,
            label: crate::room_learner::observation::RoomLabel::Empty,
            duration_ms: 12000,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        first,
    )
    .unwrap();
    for bucket in 0..4u64 {
        for row in 0..8u32 {
            let at = first + bucket * 3_000_000_000 + u64::from(row + 1) * 10_000_000;
            for source in &c.sources {
                let amplitudes = vec![2.; source.feature_count];
                rt.ingest(
                    RawSourceFrame {
                        source_id: source.source_id,
                        grid_id: &source.grid_id,
                        frequency_hz: source.center_frequency_hz,
                        bandwidth_hz: source.bandwidth_hz,
                        sequence: bucket as u32 * 8 + row + 1,
                        amplitudes: &amplitudes,
                    },
                    at,
                )
                .unwrap();
            }
        }
    }
    rt.finish_phase(&rt.mutation_guard().unwrap(), first + 12_000_000_000, true)
        .unwrap();
    let sealed = stable(rt.status(first + 12_000_000_000));
    let bytes = serde_json::to_vec(&(initial, sealed)).unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    // The receiver clock and runtime ID are fixed; ONLY the random new-run
    // receipt handle is normalized above. All other public bytes remain exact.
    assert_eq!(
        digest,
        "8e35280c09309b4f039e1e7cc646364bbd7a92934cfe2960df1c4a0dbeb91b5a"
    );
    println!("LEGACY_PUBLIC_STATUS_SHA256={digest} BYTES={}", bytes.len());
}
