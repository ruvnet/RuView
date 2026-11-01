//! Managed integration; raw RF enters only from the existing receiver, never
//! caller-supplied learning scores or uploaded synthetic examples.
use super::*;
use ruview_unified::RfModality;
use wifi_densepose_sensing_server::room_learner::{
    capture::PhaseRequest,
    lifecycle::{MutationGuard, RoomLearningRuntime},
    native::{self, NativePacketInput},
    observation::{CaptureProvenance, DataRole, SourceSpec},
    training::FitConfig,
};

#[path = "room_learner_ingress.rs"]
mod ingress;
use ingress::native_candidates;
pub(super) use ingress::{observe, LiveSource, NativeSourceKey};

fn failure(code: &str, error: impl ToString) -> Json<serde_json::Value> {
    let code = if error.to_string() == "learner_state_changed" {
        "learner_state_changed"
    } else {
        code
    };
    Json(serde_json::json!({"success":false,"error_code":code,
        "error":error.to_string().chars().filter(|c|!c.is_control()).take(256).collect::<String>()}))
}
async fn runtime(state: &SharedState) -> Result<Arc<RoomLearningRuntime>, Json<serde_json::Value>> {
    let s = state.read().await;
    if s.privacy_mode {
        return Err(failure(
            "learner_privacy_mode",
            "Room learning is unavailable in privacy-redacted mode.",
        ));
    }
    s.room_learning.clone().ok_or_else(|| {
        failure(
            "learner_disabled",
            "This helper does not enable the managed room learner.",
        )
    })
}
fn now_ns(runtime: &RoomLearningRuntime) -> u64 {
    runtime.receiver_now_ns()
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfigureRequest {
    expected: MutationGuard,
    binding_digest: String,
    revision: u64,
    source_node_ids: Vec<u8>,
    new_training: bool,
}
fn source_ready(state: &AppStateInner, spec: &SourceSpec, now: std::time::Instant) -> bool {
    native_candidates(state, spec.source_id, now)
        .iter()
        .any(|(key, meta)| {
            native::grid_id(
                key.n_antennas as usize,
                key.raw_bins as usize,
                wifi_densepose_hardware::PpduType::from_byte(key.ppdu_type),
            )
            .ok()
            .as_deref()
                == Some(spec.grid_id.as_str())
                && meta.frequency_hz == spec.center_frequency_hz
                && meta.bandwidth_hz == Some(spec.bandwidth_hz)
        })
        && spec.feature_count == 56
        && spec.signal_kind == RfModality::WifiCsi
        && spec.device_id == format!("esp32-node-{}", spec.source_id)
}
pub(super) async fn configure(
    State(state): State<SharedState>,
    Json(request): Json<ConfigureRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !valid_binding_digest(&request.binding_digest)
        || !valid_source_node_ids(&request.source_node_ids)
        || !(2..=8).contains(&request.source_node_ids.len())
        || request.revision == 0
    {
        return failure(
            "learner_context_invalid",
            "Use a current room digest/revision and 2 to 8 sorted unique node IDs.",
        );
    }
    let now = std::time::Instant::now();
    let s = state.read().await;
    if !request.new_training {
        let catalog = match rt.saved_candidates(&request.binding_digest) {
            Ok(c) => c,
            Err(e) => return failure("learner_saved_models_unavailable", e),
        };
        let found = catalog.candidates.iter().find(|entry| {
            entry.context.revision == request.revision
                && entry
                    .context
                    .sources
                    .iter()
                    .map(|spec| spec.source_id)
                    .collect::<Vec<_>>()
                    == request.source_node_ids
                && entry
                    .context
                    .sources
                    .iter()
                    .all(|spec| source_ready(&s, spec, now))
        });
        let Some(saved) = found else {
            return failure(
                "learner_saved_context_unavailable",
                if catalog.candidates.is_empty() {
                    "No readable saved candidate for this exact room. Stored history is preserved; start a new capture or inspect model recovery."
                } else {
                    "Saved candidates are retained, but their trained source/PHY context is not currently receiving. Restore those inputs or explicitly train a new model."
                },
            );
        };
        let context = saved.context.clone();
        drop(s);
        return match rt.configure(
            &request.expected,
            context,
            opaque_calibration_id("learner-resume"),
            false,
        ) {
            Ok(()) => Json(serde_json::json!({"success":true,"status":rt.status(now_ns(&rt))})),
            Err(e) => failure("learner_resume_failed", e),
        };
    }
    let mut sources = Vec::new();
    for &id in &request.source_node_ids {
        let Some(_) = s.node_states.get(&id) else {
            return failure(
                "learner_source_missing",
                format!("Node {id} is not receiving."),
            );
        };
        let Some((key, meta)) =
            native_candidates(&s, id, now)
                .into_iter()
                .max_by(|(ka, a), (kb, b)| {
                    a.receiver_rate_hz
                        .total_cmp(&b.receiver_rate_hz)
                        .then(ka.raw_bins.cmp(&kb.raw_bins))
                })
        else {
            return failure("learner_source_unready",format!("Node {id} needs a stable, supported exact native PHY cohort and a fresh eight-packet window."));
        };
        let bandwidth_hz = meta
            .bandwidth_hz
            .expect("eligible native cohort has supported bandwidth");
        sources.push(SourceSpec {
            source_id: id,
            device_id: format!("esp32-node-{id}"),
            grid_id: match native::grid_id(
                key.n_antennas as usize,
                key.raw_bins as usize,
                wifi_densepose_hardware::PpduType::from_byte(key.ppdu_type),
            ) {
                Ok(id) => id,
                Err(error) => return failure("learner_grid_unsupported", error),
            },
            feature_count: 56,
            signal_kind: RfModality::WifiCsi,
            center_frequency_hz: meta.frequency_hz,
            bandwidth_hz,
            link_id: format!("receiver-{id}-transmitter-unverified"),
            link_index: 0,
            geometry: None,
        });
    }
    drop(s);
    let context = match rt.bind_context(request.binding_digest, request.revision, sources) {
        Ok(c) => c,
        Err(e) => return failure("learner_context_invalid", e),
    };
    let run = opaque_calibration_id("learner-run");
    match rt.configure(&request.expected, context, run, request.new_training) {
        Ok(()) => Json(serde_json::json!({"success":true,"status":rt.status(now_ns(&rt))})),
        Err(e) => failure("learner_configure_failed", e),
    }
}
pub(super) async fn status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    match runtime(&state).await {
        Ok(rt) => Json(rt.status(now_ns(&rt))),
        Err(e) => e,
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BeginRequest {
    expected: MutationGuard,
    phase: PhaseRequest,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GuardRequest {
    expected: MutationGuard,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrainRequest {
    expected: MutationGuard,
    config: FitConfig,
}
pub(super) async fn begin_phase(
    State(state): State<SharedState>,
    Json(request): Json<BeginRequest>,
) -> Json<serde_json::Value> {
    let state = state.write().await;
    if state.privacy_mode {
        return failure(
            "learner_privacy_mode",
            "Room learning is unavailable in privacy-redacted mode.",
        );
    }
    if state.field_model.as_ref().is_some_and(|f| {
        matches!(f.status(), CalibrationStatus::Collecting)
            || (state.calibration_session_id.is_some()
                && matches!(f.status(), CalibrationStatus::Uncalibrated))
    }) {
        return failure(
            "legacy_empty_capture_active",
            "Finish or cancel the legacy empty capture before learner example capture.",
        );
    }
    let Some(rt) = state.room_learning.as_ref() else {
        return failure("learner_disabled", "Managed learner is not enabled.");
    };
    if rt.context().as_ref().is_none_or(|context| {
        context
            .sources
            .iter()
            .any(|spec| !source_ready(&state, spec, std::time::Instant::now()))
    }) {
        return failure("learner_source_unready","A selected source/grid is no longer fresh. Restore its stream and refresh source readiness before capture.");
    }
    match rt.begin_phase(&request.expected, request.phase, now_ns(rt)) {
        Ok(()) => Json(serde_json::json!({"success":true,"status":rt.status(now_ns(rt))})),
        Err(e) => failure("learner_phase_rejected", e),
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinishRequest {
    expected: MutationGuard,
    conditions_held: bool,
}
pub(super) async fn finish_phase(
    State(state): State<SharedState>,
    Json(request): Json<FinishRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match rt.finish_phase(&request.expected, now_ns(&rt), request.conditions_held) {
        Ok(receipt) => Json(serde_json::json!({"success":true,"receipt":receipt})),
        Err(e) => failure("learner_phase_incomplete", e),
    }
}
pub(super) async fn cancel_phase(
    State(state): State<SharedState>,
    Json(request): Json<GuardRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match rt.cancel_phase(&request.expected) {
        Ok(()) => Json(serde_json::json!({"success":true})),
        Err(e) => failure("learner_phase_cancel_failed", e),
    }
}
pub(super) async fn train(
    State(state): State<SharedState>,
    Json(request): Json<TrainRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match rt.start_training(&request.expected, request.config) {
        Ok(generation) => Json(
            serde_json::json!({"success":true,"generation":generation,"status":rt.status(now_ns(&rt))}),
        ),
        Err(e) => failure("learner_train_rejected", e),
    }
}
pub(super) async fn cancel_training(
    State(state): State<SharedState>,
    Json(request): Json<GuardRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match rt.cancel_training(&request.expected) {
        Ok(phase) => Json(serde_json::json!({"success":true,"state":phase})),
        Err(e) => failure("learner_cancel_failed", e),
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvaluationRequest {
    expected: MutationGuard,
    role: DataRole,
}
pub(super) async fn evaluate(
    State(state): State<SharedState>,
    Json(request): Json<EvaluationRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match rt.evaluate(&request.expected, request.role) {
        Ok(report) => Json(serde_json::json!({"success":true,"evaluation":report})),
        Err(e) => failure("learner_evaluation_rejected", e),
    }
}
pub(super) async fn prediction(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match rt.prediction(now_ns(&rt)) {
        Ok((mutation_guard, prediction)) => Json(
            serde_json::json!({"success":true,"mutation_guard":mutation_guard,"candidate_prediction":prediction,"authority":"experimental_candidate_only"}),
        ),
        Err(e) => failure("learner_prediction_unavailable", e),
    }
}

#[cfg(test)]
#[path = "room_learner_api_tests.rs"]
mod tests;
