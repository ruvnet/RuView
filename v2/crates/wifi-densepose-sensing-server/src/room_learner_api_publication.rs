//! One final binary-only projection for live publication and cached readback.
//! Legacy classification remains a labelled comparison, never a fallback for
//! an unavailable selected model. No count, pose, motion or Vitals authority is
//! inferred from the binary model.
use super::*;
use wifi_densepose_sensing_server::room_learner::{
    model::PresenceDecision, selection::SelectedBinaryEvidence,
};

const SCHEMA: &str = "ruview.room-learner.selected-binary-evidence.v1";

fn unavailable(evidence: SelectedBinaryEvidence, reason: &str) -> SelectedBinaryEvidence {
    let identity = match evidence {
        SelectedBinaryEvidence::None => return SelectedBinaryEvidence::None,
        SelectedBinaryEvidence::Unavailable { identity, .. } => identity,
        SelectedBinaryEvidence::Ready { identity, .. } => Some(identity),
    };
    SelectedBinaryEvidence::Unavailable {
        identity,
        reason: reason.into(),
    }
}

fn temporal_current(state: &AppStateInner) -> Option<serde_json::Value> {
    let rt = state.room_learning.as_ref()?;
    if !rt.temporal_v2_state_present() {
        return None;
    }
    let unavailable = |reason: &str| serde_json::json!({"schema":"ruview.room-learner.selected-binary-evidence.v2","status":"unavailable","identity":null,"reason":reason,"boot_epoch":null,"receiver_epoch":null});
    if state.privacy_mode {
        return Some(unavailable("selected_learning_unavailable_in_privacy_mode"));
    }
    let now = now_ns(rt);
    let value = match state.current_room_store.as_ref() {
        Some(room) => room
            .with_current_binding(|binding| {
                rt.temporal_selected_binary_v2(now, binding, Some(&state.calibration_boot_epoch))
            })
            .unwrap_or_else(|_| {
                rt.temporal_selected_binary_v2(now, None, Some(&state.calibration_boot_epoch))
            }),
        None => rt.temporal_selected_binary_v2(now, None, Some(&state.calibration_boot_epoch)),
    }
    .unwrap_or_else(|_| unavailable("selected_projection_unavailable"));
    if value["status"] == "none" {
        None
    } else {
        Some(value)
    }
}
fn temporal_unavailable(mut value: serde_json::Value, reason: &str) -> serde_json::Value {
    let identity = value
        .get("identity")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let boot = value
        .get("boot_epoch")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let receiver = value
        .get("receiver_epoch")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    value = serde_json::json!({"schema":"ruview.room-learner.selected-binary-evidence.v2","status":"unavailable","identity":identity,"reason":reason,"boot_epoch":boot,"receiver_epoch":receiver});
    value
}
fn temporal_dispatch(state: &AppStateInner) -> bool {
    state
        .room_learning
        .as_ref()
        .is_some_and(|rt| rt.temporal_v2_state_present())
}

pub(crate) fn selection_owns_output(state: &AppStateInner) -> bool {
    if temporal_dispatch(state) {
        return temporal_current(state).is_some();
    }
    state.room_learning.as_ref().is_some_and(|runtime| {
        !matches!(
            runtime.selected_binary_evidence(now_ns(runtime), None),
            SelectedBinaryEvidence::None
        )
    })
}

fn current(state: &AppStateInner) -> SelectedBinaryEvidence {
    let Some(rt) = state.room_learning.as_ref() else {
        return SelectedBinaryEvidence::None;
    };
    let now = now_ns(rt);
    let evidence = match state.current_room_store.as_ref() {
        Some(room) => room
            .with_current_binding(|binding| rt.selected_binary_evidence(now, binding))
            .unwrap_or_else(|_| rt.selected_binary_evidence(now, None)),
        None => rt.selected_binary_evidence(now, None),
    };
    if state.privacy_mode {
        return match evidence {
            SelectedBinaryEvidence::None => SelectedBinaryEvidence::None,
            _ => SelectedBinaryEvidence::Unavailable {
                identity: None,
                reason: "selected_learning_unavailable_in_privacy_mode".into(),
            },
        };
    }
    match &evidence {
        SelectedBinaryEvidence::Ready {
            decision,
            presence,
            occupied_probability,
            ..
        } if !occupied_probability.is_finite()
            || !(0.0..=1.0).contains(occupied_probability)
            || !matches!(
                (decision, presence),
                (PresenceDecision::Empty, false) | (PresenceDecision::Occupied, true)
            ) =>
        {
            unavailable(evidence, "invalid_selected_binary_decision")
        }
        _ => evidence,
    }
}

fn envelope(state: &AppStateInner, evidence: SelectedBinaryEvidence) -> Option<serde_json::Value> {
    if matches!(evidence, SelectedBinaryEvidence::None) {
        return None;
    }
    let mut value =
        serde_json::to_value(evidence).expect("bounded selected binary proof serializes");
    let object = value
        .as_object_mut()
        .expect("tagged selected binary proof is an object");
    object.insert("schema".into(), serde_json::json!(SCHEMA));
    object.insert(
        "boot_epoch".into(),
        if state.privacy_mode {
            serde_json::Value::Null
        } else {
            serde_json::json!(state.calibration_boot_epoch)
        },
    );
    Some(value)
}

fn apply(update: &mut SensingUpdate, proof: Option<serde_json::Value>) {
    update.selected_binary_evidence = proof;
    if update.selected_binary_evidence.is_none() {
        update.classification_source = None;
        return;
    }
    update.classification_source = Some("legacy_control_comparison".into());
    update.calibrated_presence_evidence = None;
    update.detector_diagnostics = None;
    update.estimated_persons = None;
    update.persons = None;
    update.pose_keypoints = None;
    update.vital_signs = None;
    update.vital_evidence = None;
    update.enhanced_breathing = None;
    update.enhanced_motion = None;
    update.posture = None;
    // Node and room classification diagnostics retain their own legacy source;
    // they are not selected binary claims. None carries a qualified count.
}

pub(crate) fn project_live_selection(state: &AppStateInner, update: &mut SensingUpdate) {
    if temporal_dispatch(state) {
        let proof = temporal_current(state).map(|value| {
            if update.source == "esp32" {
                value
            } else {
                temporal_unavailable(value, "selected_native_source_update_required")
            }
        });
        apply(update, proof);
        return;
    }
    let mut evidence = current(state);
    if update.source != "esp32" {
        evidence = unavailable(evidence, "selected_native_source_update_required");
    }
    apply(update, envelope(state, evidence));
}

pub(crate) fn project_cached_selection(state: &AppStateInner, update: &mut SensingUpdate) {
    if temporal_dispatch(state) {
        let proof = temporal_current(state).map(|value| {
            if value["status"] != "ready" {
                return value;
            }
            let now = state
                .room_learning
                .as_ref()
                .map(|rt| now_ns(rt) / 1_000_000);
            let cached = update.selected_binary_evidence.as_ref();
            let valid = update.source == "esp32"
                && cached == Some(&value)
                && value["observed_at_unix_ms"]
                    .as_u64()
                    .zip(now)
                    .and_then(|(observed, now)| now.checked_sub(observed))
                    .is_some_and(|age| {
                        age <= wifi_densepose_sensing_server::room_learner::observation::MAX_AGE_NS
                            / 1_000_000
                    })
                && value["valid_until_unix_ms"]
                    .as_u64()
                    .zip(now)
                    .is_some_and(|(until, now)| now < until);
            if valid {
                cached.cloned().expect("matched admitted proof")
            } else {
                temporal_unavailable(value, "selected_cached_observation_unavailable")
            }
        });
        apply(update, proof);
        return;
    }
    let mut evidence = current(state);
    if let SelectedBinaryEvidence::Ready { identity, .. } = &evidence {
        let cached = update.selected_binary_evidence.as_ref();
        let now_ms = state
            .room_learning
            .as_ref()
            .map(|rt| now_ns(rt) / 1_000_000);
        let identity =
            serde_json::to_value(identity).expect("bounded selected identity serializes");
        let matches = cached.is_some_and(|proof| {
            let fresh = proof["observed_at_unix_ms"]
                .as_u64()
                .zip(now_ms)
                .and_then(|(observed, now)| now.checked_sub(observed))
                .is_some_and(|age| {
                    age <= wifi_densepose_sensing_server::room_learner::observation::MAX_AGE_NS
                        / 1_000_000
                });
            proof["schema"] == SCHEMA
                && proof["boot_epoch"] == state.calibration_boot_epoch
                && proof["status"] == "ready"
                && proof["identity"] == identity
                && proof["observation_digest"].as_str().is_some_and(|digest| {
                    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
                })
                && fresh
        });
        if update.source != "esp32" || !matches {
            evidence = unavailable(evidence, "selected_cached_observation_unavailable");
        } else {
            // A cache read cannot manufacture a newly observed measurement or
            // advance its timestamp. Keep the original exact proof.
            let cached = cached.cloned();
            apply(update, cached);
            return;
        }
    }
    apply(update, envelope(state, evidence));
}

/// A queued frame may predate selection, rollback, a room change or source
/// withdrawal. Revalidate its authority at the subscriber boundary without
/// converting it into a new measurement. Non-sensing control messages remain
/// byte-identical; legacy edge-vitals messages are withheld while selected.
pub(crate) fn refresh_queued_update(state: &AppStateInner, json: &str) -> Option<String> {
    if !selection_owns_output(state) && !json.contains("\"selected_binary_evidence\"") {
        return Some(json.to_owned());
    }
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("sensing_update") => {
            let mut update: SensingUpdate = serde_json::from_value(value).ok()?;
            project_cached_selection(state, &mut update);
            serde_json::to_string(&update).ok()
        }
        Some("edge_vitals" | "edge_fused_vitals") if selection_owns_output(state) => None,
        _ => Some(json.to_owned()),
    }
}

/// Legacy MQTT/automation bridges cannot reinterpret selected-model frames,
/// even after deselection or when an older unmarked frame was queued before
/// selection. This is an admission guard, not new MQTT presence authority.
#[cfg(any(feature = "mqtt", test))]
pub(crate) fn legacy_sink_frame_allowed(selection_owned: bool, frame: &serde_json::Value) -> bool {
    !selection_owned
        && frame.get("selected_binary_evidence").is_none()
        && frame.get("classification_source").is_none()
}

#[cfg(test)]
mod legacy_sink_tests {
    use super::*;
    #[test]
    fn selected_current_state_and_queued_markers_both_withhold_legacy_bridges() {
        let legacy =
            serde_json::json!({"type":"sensing_update","classification":{"presence":true}});
        assert!(legacy_sink_frame_allowed(false, &legacy));
        assert!(
            !legacy_sink_frame_allowed(true, &legacy),
            "pre-selection queued legacy data must be withheld after selection"
        );
        for status in ["ready", "unavailable", "none"] {
            let mut selected = legacy.clone();
            selected["selected_binary_evidence"] = serde_json::json!({"status":status});
            assert!(
                !legacy_sink_frame_allowed(false, &selected),
                "old selected frame must not regain legacy authority after deselection"
            );
            assert!(!legacy_sink_frame_allowed(true, &selected));
        }
        let mut malformed = legacy.clone();
        malformed["selected_binary_evidence"] = serde_json::Value::Null;
        assert!(!legacy_sink_frame_allowed(false, &malformed));
        let mut comparison = legacy;
        comparison["classification_source"] = serde_json::json!("legacy_control_comparison");
        assert!(!legacy_sink_frame_allowed(false, &comparison));
    }
}
