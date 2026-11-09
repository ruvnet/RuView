//! SYNTHETIC current-clock boundary checks only; no native recording/fit.
//! The obsolete fixed-clock artifact exporter is archived outside this source.
use super::tests as fixture;
use super::*;
/// Actual Runtime serialization with no native data/fit/grant. An old receipt
/// time can predate a later status tick; absence of an actual C4C proof MUST
/// remain refused, but this is not a reversed admission clock.
#[test]
fn received_reference_time_before_later_tick_is_not_admission_clock_reversal() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Arc::new(fixture::runtime(directory.path(), fixture::START));
    let context = fixture::context();
    runtime
        .configure_temporal_capture_with(
            &runtime.mutation_guard().unwrap(),
            context.clone(),
            "synthetic-reference-clock-only".into(),
            fixture::START,
            TemporalTrainCapture::prepare_retained,
        )
        .unwrap();
    let received = runtime.receiver_now_ns();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let advanced = runtime.receiver_now_ns();
    runtime
        .tick_temporal_capture(&runtime.mutation_guard().unwrap(), advanced)
        .unwrap();
    let error = runtime
        .with_reference_scope(
            &context.room_digest,
            context.revision,
            &[1, 2, 3],
            received,
            |_, _| Ok(()),
        )
        .unwrap_err();
    assert_eq!(
        error, "reference_training_proof_required",
        "serialized admission may be newer than transport receipt; no proof may be manufactured"
    );
}
