//! Synthetic recovery fixtures, never physical qualification.
use super::*;
use ruview_unified::RfModality;
const START: u64 = 1_000_000_000;
fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "synthetic-owner".into(),
        installation_id: "synthetic-install".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=2)
            .map(|id| SourceSpec {
                source_id: id,
                device_id: format!("synthetic-{id}"),
                grid_id: "ht20-64-raw".into(),
                feature_count: 64,
                signal_kind: RfModality::WifiCsi,
                center_frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                link_id: format!("tx-rx-{id}"),
                link_index: 0,
                geometry: None,
            })
            .collect(),
    }
}
fn session() -> CaptureSession {
    CaptureSession::new(context(), "synthetic-run".into()).unwrap()
}
fn request(role: DataRole, label: RoomLabel, id: &str) -> PhaseRequest {
    PhaseRequest {
        trial_id: format!("synthetic-trial-{id}"),
        session_id: format!("synthetic-session-{id}"),
        role,
        label,
        duration_ms: MIN_PHASE_MS,
        lead_in_ms: 0,
        conditions_confirmed: true,
    }
}
fn packet(source_id: u8, sequence: u32, values: &[f64]) -> RawSourceFrame<'_> {
    RawSourceFrame {
        source_id,
        grid_id: "ht20-64-raw",
        frequency_hz: 2_437_000_000,
        bandwidth_hz: 20_000_000,
        sequence,
        amplitudes: values,
    }
}
fn window(s: &mut CaptureSession, at: u64, start_sequence: u32, gain: f64) {
    let values = vec![gain; 64];
    for t in 0..WINDOW_PACKETS {
        for source in 1..=2 {
            s.ingest(
                packet(source, start_sequence.wrapping_add(t as u32), &values),
                at + t as u64 * 10_000_000 + u64::from(source),
            )
            .unwrap();
        }
    }
}
fn phase(
    s: &mut CaptureSession,
    at: u64,
    seq: u32,
    role: DataRole,
    label: RoomLabel,
    id: &str,
    gain: f64,
) {
    s.begin_phase(request(role, label, id), at).unwrap();
    for quarter in 0..COHORTS_PER_PHASE {
        window(
            s,
            at + quarter as u64 * 3_000_000_000 + 1_000_000,
            seq + quarter as u32 * 8,
            gain,
        );
    }
    s.finish_phase(at + MIN_PHASE_MS * 1_000_000).unwrap();
}

fn reopen(snapshot: &CaptureCheckpoint, now: u64) -> Result<CaptureRestore, String> {
    let bytes = serde_json::to_vec(snapshot).unwrap();
    let decoded: CaptureCheckpoint = serde_json::from_slice(&bytes).unwrap();
    CaptureSession::restore_checkpoint(decoded, &context(), now)
}
#[test]
fn completed_600s_requires_confirmation_after_typed_u128_roundtrip() {
    let mut s = session();
    let mut r = request(DataRole::Train, RoomLabel::Empty, "600");
    r.duration_ms = 600_000;
    s.begin_phase(r, START).unwrap();
    for bucket in 0..4 {
        window(
            &mut s,
            START + bucket * 150_000_000_000 + 1_000_000,
            1 + bucket as u32 * 8,
            2.0,
        );
    }
    let end = START + 600_000_000_000;
    let snapshot = s.checkpoint(end).unwrap();
    assert!(snapshot.summary().phase.unwrap().complete);
    assert!(
        snapshot.phase.as_ref().unwrap().representatives[0].views[0].frame_ids[0]
            > u128::from(u64::MAX)
    );
    let mut restored = reopen(&snapshot, end + 100_000_000_000).unwrap();
    assert_eq!(
        restored.disposition,
        RecoveryPhaseDisposition::CompletedNeedsConfirmation
    );
    assert!(restored.session.cohorts().is_empty());
    assert!(restored.session.live_cohort(end + 100_000_000_000).is_err());
    assert!(restored
        .session
        .progress(end)
        .sources
        .iter()
        .all(|s| s.live_buffer_frames == 0 && s.phase_buffer_frames == 0));
    let receipt = restored
        .session
        .finish_phase(end + 100_000_000_000)
        .unwrap();
    assert_eq!(receipt.status, PhaseStatus::Sealed);
    assert_eq!(restored.session.cohorts().len(), 4);
    assert_eq!(
        serde_json::to_vec(restored.session.cohorts()).unwrap(),
        serde_json::to_vec(&snapshot.phase.unwrap().representatives).unwrap()
    );
}
#[test]
fn interrupted_phase_cannot_count_downtime_or_reuse_provisional_samples() {
    let mut s = session();
    s.begin_phase(request(DataRole::Train, RoomLabel::Empty, "partial"), START)
        .unwrap();
    window(&mut s, START + 1_000_000, 1, 2.0);
    let snapshot = s.checkpoint(START + 2_000_000_000).unwrap();
    let mut recovered = reopen(&snapshot, START + 20_000_000_000).unwrap();
    assert_eq!(
        recovered.disposition,
        RecoveryPhaseDisposition::InterruptedNeedsRestart
    );
    assert!(!recovered.session.phase_active());
    assert!(recovered.session.cohorts().is_empty());
    assert!(recovered
        .session
        .finish_phase(START + 20_000_000_000)
        .is_err());
    assert_eq!(recovered.session.history()[0].status, PhaseStatus::Failed);
    assert!(recovered
        .session
        .begin_phase(
            request(DataRole::HeldOut, RoomLabel::Empty, "partial"),
            START + 20_000_000_000
        )
        .is_err());
    recovered
        .session
        .begin_phase(
            request(DataRole::Train, RoomLabel::Empty, "partial"),
            START + 20_000_000_000,
        )
        .unwrap();
    assert!(recovered
        .session
        .live_cohort(START + 20_000_000_000)
        .is_err());
}
#[test]
fn four_representatives_before_deadline_still_restore_as_interrupted() {
    let mut s = session();
    s.begin_phase(request(DataRole::Train, RoomLabel::Empty, "early"), START)
        .unwrap();
    for bucket in 0..4 {
        window(
            &mut s,
            START + bucket * 3_000_000_000 + 1_000_000,
            1 + bucket as u32 * 8,
            2.0,
        );
    }
    let snapshot = s.checkpoint(START + 10_000_000_000).unwrap();
    assert!(!snapshot.summary().phase.unwrap().complete);
    assert_eq!(
        reopen(&snapshot, START + 20_000_000_000)
            .unwrap()
            .disposition,
        RecoveryPhaseDisposition::InterruptedNeedsRestart
    );
}
#[test]
fn sealed_roles_watermarks_and_replay_guard_survive_without_live_buffers() {
    let mut s = session();
    phase(
        &mut s,
        START,
        1,
        DataRole::Train,
        RoomLabel::Empty,
        "sealed",
        2.0,
    );
    let at = START + MIN_PHASE_MS * 1_000_000;
    let snapshot = s.checkpoint(at).unwrap();
    let mut restored = reopen(&snapshot, at + 1).unwrap();
    assert_eq!(
        restored.disposition,
        RecoveryPhaseDisposition::NoActivePhase
    );
    assert_eq!(
        restored.session.validated_dataset().unwrap().train().len(),
        4
    );
    assert!(restored
        .session
        .begin_phase(
            request(DataRole::Validation, RoomLabel::Empty, "sealed"),
            at + 2
        )
        .is_err());
    let values = vec![2.0; 64];
    assert!(restored
        .session
        .ingest(packet(1, 32, &values), at + 3)
        .is_err());
    assert!(restored
        .session
        .ingest(packet(1, 1, &values), at + 3)
        .is_err());
    restored
        .session
        .ingest(packet(1, 33, &values), at + 3)
        .unwrap();
    assert_eq!(restored.session.sources[&1].ordinal, 33);
    assert_eq!(restored.session.sources[&1].live.len(), 1);
    assert!(restored.session.live_cohort(at + 3).is_err());
}
#[test]
fn malformed_context_times_sources_roles_and_frame_reuse_fail_closed() {
    let mut s = session();
    phase(
        &mut s,
        START,
        1,
        DataRole::Train,
        RoomLabel::Empty,
        "sealed",
        2.0,
    );
    let now = START + MIN_PHASE_MS * 1_000_000;
    let original = s.checkpoint(now).unwrap();
    let mut wrong = context();
    wrong.revision += 1;
    assert!(CaptureSession::restore_checkpoint(original.clone(), &wrong, now).is_err());
    assert!(reopen(&original, now - 1).is_err());
    for mutant in 0..8 {
        let mut c = original.clone();
        match mutant {
            0 => c.sources[0].source_id = 99,
            1 => c.sources[0].last_time_ns = now + 1,
            2 => c.sources[0].ordinal = 1,
            3 => c.history[0].role = DataRole::HeldOut,
            4 => c.cohorts[1].views[0].frame_ids[0] = c.cohorts[0].views[0].frame_ids[0],
            5 => c.cohorts[0].views[0].sequence_ids[0] = 0,
            6 => c.cohorts[0].views[0].samples[0][0] = f32::NAN,
            _ => c.cohorts[0].views[0].last_time_ns = now + 1,
        }
        assert!(
            CaptureSession::restore_checkpoint(c, &context(), now).is_err(),
            "mutant {mutant}"
        );
    }
    let mut bytes = serde_json::to_vec(&original).unwrap();
    bytes.pop();
    bytes.extend_from_slice(br#","unknown":1}"#);
    assert!(serde_json::from_slice::<CaptureCheckpoint>(&bytes).is_err());
}
#[test]
fn marker_changes_only_at_capture_milestones_and_original_deadline() {
    let mut s = session();
    let initial = s.recovery_marker(START);
    let values = vec![2.0; 64];
    s.ingest(packet(1, 1, &values), START).unwrap();
    assert_eq!(initial, s.recovery_marker(START + 1));
    s.begin_phase(
        request(DataRole::Train, RoomLabel::Empty, "marker"),
        START + 1,
    )
    .unwrap();
    let began = s.recovery_marker(START + 2);
    assert_ne!(initial, began);
    s.ingest(packet(1, 2, &values), START + 3).unwrap();
    assert_eq!(began, s.recovery_marker(START + 4));
    window(&mut s, START + 1_000_000, 3, 2.0);
    let representative = s.recovery_marker(START + 100_000_000);
    assert_ne!(began, representative);
    assert_eq!(representative, s.recovery_marker(START + 1_000_000_000));
    assert_ne!(
        representative,
        s.recovery_marker(START + MIN_PHASE_MS * 1_000_000 + 1)
    );
}

#[test]
fn unlabelled_live_windows_and_partial_packet_buffers_are_never_serialized() {
    let mut s = session();
    window(&mut s, START, 1, 2.0);
    assert!(s.live_cohort(START + 100_000_000).is_ok());
    let saved = s.checkpoint(START + 100_000_000).unwrap();
    let encoded = String::from_utf8(serde_json::to_vec(&saved).unwrap()).unwrap();
    assert!(!encoded.contains("\"samples\""));
    assert!(!encoded.contains("\"live\""));
    assert!(!encoded.contains("\"pending\""));
    let restored = reopen(&saved, START + 100_000_001).unwrap();
    assert!(restored.session.live_cohort(START + 100_000_001).is_err());
    s.begin_phase(
        request(DataRole::Train, RoomLabel::Empty, "pending"),
        START + 100_000_001,
    )
    .unwrap();
    let values = vec![2.0; 64];
    s.ingest(packet(1, 9, &values), START + 100_000_002)
        .unwrap();
    let saved = s.checkpoint(START + 100_000_003).unwrap();
    assert!(saved.phase.as_ref().unwrap().representatives.is_empty());
    assert!(!String::from_utf8(serde_json::to_vec(&saved).unwrap())
        .unwrap()
        .contains("\"samples\""));
}
