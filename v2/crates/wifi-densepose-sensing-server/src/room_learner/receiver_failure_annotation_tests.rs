//! SYNTHETIC private-core metadata update; no actual capture or durability claim.
use super::*;
use super::super::tests::bridge_setup;
const PRIMARY: &str = "temporal_observed_gap_or_rate: source=1 direction=below_min receiver_interval_ns=1000000 min_ns=27777777 max_ns=50000000";
#[test]
fn failure_annotation_only_replaces_exact_failed_metadata_and_preserves_all_snapshot_facts() {
    let (_dir, _store, mut c) = bridge_setup();
    let initial = c.snapshot();
    assert!(!c.annotate_receiver_failure(PRIMARY, format!("{PRIMARY} deq_us=30303")));
    assert_eq!(c.snapshot(), initial, "nonfailed capture must be untouched");
    c.invalidate(PRIMARY);
    let before = c.snapshot();
    let floor = c.workflow.source_high.clone();
    let receiver_clock = c.workflow.last_now_ns;
    let durable = c.workflow.durable.clone();
    let detail = format!("{PRIMARY} deq_us=30303 parse_us=10 lock_us=50000 stamp_age_us=50015");
    assert!(c.annotate_receiver_failure(PRIMARY, detail.clone()));
    let mut after = c.snapshot();
    assert_eq!(after["failure"], detail);
    after["failure"] = before["failure"].clone();
    assert_eq!(after, before, "no stage, time, pending ACK, retained rows, or counters may change");
    assert_eq!(c.workflow.source_high, floor);
    assert_eq!(c.workflow.last_now_ns, receiver_clock);
    assert_eq!(c.workflow.durable, durable);
    let annotated = c.snapshot();
    assert!(!c.annotate_receiver_failure(PRIMARY, format!("{PRIMARY} stale_expected=1")));
    assert_eq!(c.snapshot(), annotated);
}
#[test]
fn failure_annotation_refuses_oversized_or_unrelated_metadata_without_cutting_primary() {
    let (_dir, _store, mut c) = bridge_setup(); c.invalidate(PRIMARY);
    let before = c.snapshot();
    assert!(!c.annotate_receiver_failure(PRIMARY, format!("{PRIMARY}{}", "x".repeat(256))));
    assert!(!c.annotate_receiver_failure(PRIMARY, "different failure".into()));
    assert_eq!(c.snapshot(), before);
}
