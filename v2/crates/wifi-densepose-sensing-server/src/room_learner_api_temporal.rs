//! Version-negotiated adapters. V1 handlers remain the default; malformed V2
//! requests never enter the legacy workflow. Authority stays in Runtime.
use super::*;
use wifi_densepose_sensing_server::room_learner::temporal_wire::*;

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaQuery {
    schema: Option<String>,
}
fn schema_matches(actual: &str, expected: &str) -> Result<(), Json<serde_json::Value>> {
    if actual == expected {
        Ok(())
    } else {
        Err(failure(
            "learner_schema_unsupported",
            "Unsupported learner request schema.",
        ))
    }
}
macro_rules! command {
    ($name:ident, $legacy:ty, $temporal:ty) => {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        pub(crate) enum $name {
            Temporal($temporal),
            Legacy($legacy),
        }
    };
}
command!(
    ConfigureCommand,
    ConfigureRequest,
    TemporalConfigureRequestV2
);
command!(StartCommand, BeginRequest, TemporalPhaseStartRequestV2);
command!(FinishCommand, FinishRequest, TemporalPhaseFinishRequestV2);
command!(TrainCommand, TrainRequest, TemporalTrainRequestV2);
command!(
    EvaluateCommand,
    EvaluationRequest,
    TemporalEvaluateRequestV2
);
command!(QualifyCommand, GuardRequest, TemporalQualifyRequestV2);
command!(
    SelectCommand,
    selection_routes::SelectRequest,
    TemporalSelectRequestV2
);
command!(
    RestoreCommand,
    RestoreCaptureRequest,
    TemporalRecoveryRequestV2
);
command!(
    DiscardCommand,
    DiscardCaptureRequest,
    TemporalDiscardRequestV2
);

command!(DeselectCommand, GuardRequest, TemporalDeselectRequestV2);
command!(PauseCommand, GuardRequest, TemporalCurrentActionRequestV2);
command!(
    CancelOperationCommand,
    GuardRequest,
    TemporalCurrentActionRequestV2
);

pub(crate) async fn status_dispatch(
    State(state): State<SharedState>,
    Query(query): Query<SchemaQuery>,
) -> Json<serde_json::Value> {
    let Some(schema) = query.schema else {
        return status(State(state)).await;
    };
    if let Err(error) = schema_matches(&schema, STATUS_SCHEMA) {
        return error;
    }
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    if let Err(e)=rt.temporal_default_operator_profile_v2(){return failure("learner_profile_unavailable",e);}
    temporal_status(&state, &rt).await
}
fn temporal_selection(
    state: &AppStateInner,
    rt: &RoomLearningRuntime,
    now: u64,
) -> Result<serde_json::Value, String> {
    let projection = match state.current_room_store.as_ref() {
        Some(room) => room
            .with_current_binding(|binding| {
                rt.temporal_selection_v2(now, binding, Some(&state.calibration_boot_epoch))
            })
            .unwrap_or_else(|_| {
                rt.temporal_selection_v2(now, None, Some(&state.calibration_boot_epoch))
            }),
        None => rt.temporal_selection_v2(now, None, Some(&state.calibration_boot_epoch)),
    }?;
    validate_temporal_selection(projection)
}
fn validate_temporal_selection(projection: serde_json::Value) -> Result<serde_json::Value, String> {
    if projection["schema"] != SELECTION_SCHEMA
        || projection["success"] != true
        || projection["model_selection"]["schema"] != SELECTION_STATUS_SCHEMA
    {
        return Err("temporal_selection_projection_invalid".into());
    }
    Ok(projection)
}
async fn temporal_status(
    state: &SharedState,
    rt: &Arc<RoomLearningRuntime>,
) -> Json<serde_json::Value> {
    temporal_status_with_reader(state, rt, |reader, now| reader.temporal_status_snapshot_v2(now)).await
}
// The reader seam keeps the blocking boundary explicit and allows a
// deterministic concurrency regression without a production runtime hook.
pub(super) async fn temporal_status_with_reader<F>(
    state: &SharedState,
    rt: &Arc<RoomLearningRuntime>,
    read_snapshot: F,
) -> Json<serde_json::Value>
where
    F: FnOnce(Arc<RoomLearningRuntime>, u64) -> Result<serde_json::Value, String> + Send + 'static,
{
    // Concurrent polls wait as async futures, without owning an RF lock or
    // spawning another blocking worker. Explicit cancellation removes a waiter.
    let permit = match rt.acquire_temporal_status_reader().await {
        Ok(permit) => permit,
        Err(error) => return failure("learner_status_unavailable", error),
    };
    // Clock advancement stays ordered with native ingress, but recovery disk
    // discovery must never hold the shared RF lock while waiting on fsync.
    let (now, expected, room_store, room_binding, boot_epoch) = {
        let current = state.write().await;
        if current.privacy_mode {
            return failure("learner_privacy_mode", "Room learning unavailable in privacy mode.");
        }
        if !current.room_learning.as_ref().is_some_and(|live| Arc::ptr_eq(live, rt)) {
            return failure("learner_state_changed", "learner_state_changed");
        }
        let now = now_ns(rt);
        if let Err(error) = rt.temporal_poll_status_v2(now) {
            return failure("learner_status_unavailable", error);
        }
        let expected = match rt.mutation_guard() {
            Ok(expected) => expected,
            Err(error) => return failure("learner_status_unavailable", error),
        };
        let room_binding = match current.current_room_store.as_ref().map(|room| room.current_binding_digest()).transpose() {
            Ok(binding) => binding,
            Err(_) => return failure("learner_state_changed", "current_room_binding_unavailable"),
        };
        (now, expected, current.current_room_store.clone(), room_binding, current.calibration_boot_epoch.clone())
    };
    let reader = rt.clone();
    let read_binding = room_binding.clone();
    let read_epoch = boot_epoch.clone();
    let snapshot = tokio::task::spawn_blocking(move || {
        // Cancellation of the awaiting HTTP request cannot drop this permit.
        let _permit = permit;
        let mut status = read_snapshot(reader.clone(), now)?;
        let projection = reader.temporal_selection_v2(now, read_binding.as_ref().and_then(|binding| binding.as_deref()), Some(&read_epoch))?;
        status["model_selection"] = validate_temporal_selection(projection)?["model_selection"].clone();
        Ok::<_, String>(status)
    }).await;
    let current = state.write().await;
    if current.privacy_mode {
        return failure("learner_privacy_mode", "Room learning unavailable in privacy mode.");
    }
    let same_room_store = match (&room_store, &current.current_room_store) {
        (None, None) => true,
        (Some(before), Some(after)) => Arc::ptr_eq(before, after),
        _ => false,
    };
    if !current.room_learning.as_ref().is_some_and(|live| Arc::ptr_eq(live, rt))
        || !same_room_store
        || current.calibration_boot_epoch != boot_epoch
        || current.current_room_store.as_ref().map(|room| room.current_binding_digest()).transpose().ok().as_ref() != Some(&room_binding)
        || rt.mutation_guard().as_ref() != Ok(&expected)
    {
        return failure("learner_state_changed", "learner_state_changed");
    }
    let result = match snapshot {
        Ok(result) => result,
        Err(_) => return failure("learner_status_unavailable", "learner_status_worker_unavailable"),
    };
    match result.and_then(|mut status| {
        if status["mutation_guard"] != serde_json::to_value(&expected).map_err(|_| "learner_status_guard_invalid")? {
            return Err("learner_state_changed".into());
        }
        status["native_source_diagnostics"] = ingress::source_rejection_diagnostics(&current, std::time::Instant::now());
        Ok(status)
    }) {
        Ok(status) => Json(serde_json::json!({"success":true,"status":status})),
        Err(error) => failure("learner_status_unavailable", error),
    }
}
async fn operation_status(
    state: &SharedState,
    rt: &Arc<RoomLearningRuntime>,
    id: String,
    operation: &'static str,
) -> Json<serde_json::Value> {
    if id
        .parse::<u64>()
        .ok()
        .is_none_or(|value| value.to_string() != id)
    {
        return failure(
            "learner_operation_invalid",
            "Operation identity is unavailable.",
        );
    }
    let mut result = temporal_status(state, rt).await;
    if result.0["success"] == true {
        result.0["operation_id"] = serde_json::json!(id);
        result.0["operation"] = serde_json::json!(operation);
        result.0["effect"] = serde_json::json!("started");
    }
    result
}
fn field_capture_active(state: &AppStateInner) -> bool {
    state.field_model.as_ref().is_some_and(|f| {
        matches!(f.status(), CalibrationStatus::Collecting)
            || (state.calibration_session_id.is_some()
                && matches!(f.status(), CalibrationStatus::Uncalibrated))
    })
}
fn require_live_context(state: &AppStateInner, rt: &RoomLearningRuntime) -> Result<(), String> {
    let context = rt.context().ok_or("learner_source_unready")?;
    if context
        .sources
        .iter()
        .all(|spec| source_ready(state, spec, std::time::Instant::now()))
    {
        Ok(())
    } else {
        Err("learner_source_unready".into())
    }
}

pub(crate) async fn configure_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<ConfigureCommand>,
) -> Json<serde_json::Value> {
    let ConfigureCommand::Temporal(mut request) = command else {
        let ConfigureCommand::Legacy(request) = command else {
            unreachable!()
        };
        return configure(State(state), Json(request)).await;
    };
    if let Err(e) = schema_matches(&request.schema, CONFIGURE_REQUEST_SCHEMA) {
        return e;
    }
    if !request.new_training || !request.recovery_consent {
        return failure(
            "learner_recovery_consent_required",
            "New Temporal training requires explicit encrypted recovery consent.",
        );
    }
    if !valid_binding_digest(&request.binding_digest)
        || request.revision == 0
        || !valid_source_node_ids(&request.source_node_ids)
        || request.source_node_ids.len() != 3
    {
        return failure("learner_context_invalid", "Temporal engineering profile requires a current room revision and exactly three sorted unique sources.");
    }
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    request.expected=match rt.temporal_select_profile_guarded_v2(request.profile_id.as_deref(),&request.expected){Ok(guard)=>guard,Err(e)=>return failure("learner_profile_rejected",e)};
    let s = state.read().await;
    if field_capture_active(&s) {
        return failure(
            "learner_capture_conflict",
            "Finish or cancel active field calibration first.",
        );
    }
    let now = std::time::Instant::now();
    let mut sources = Vec::new();
    for &id in &request.source_node_ids {
        if !s.node_states.contains_key(&id) {
            return failure(
                "learner_source_missing",
                "A selected node is not receiving.",
            );
        }
        let Some((key, meta)) =
            native_candidates(&s, id, now)
                .into_iter()
                .max_by(|(ka, a), (kb, b)| {
                    a.receiver_rate_hz
                        .total_cmp(&b.receiver_rate_hz)
                        .then(ka.raw_bins.cmp(&kb.raw_bins))
                })
        else {
            return failure(
                "learner_source_unready",
                "A selected native source/PHY stream is not ready.",
            );
        };
        let grid_id = match key.grid_id() {
            Ok(grid) => grid,
            Err(e) => return failure("learner_grid_unsupported", e),
        };
        let Some(bandwidth_hz) = meta.bandwidth_hz else {
            return failure("learner_source_unready", "Native bandwidth is unavailable.");
        };
        sources.push(SourceSpec {
            source_id: id,
            device_id: format!("esp32-node-{id}"),
            grid_id,
            feature_count: 56,
            signal_kind: RfModality::WifiCsi,
            center_frequency_hz: meta.frequency_hz,
            bandwidth_hz,
            link_id: format!("receiver-{id}-transmitter-unverified"),
            link_index: 0,
            geometry: None,
        });
    }
    let context = match rt.bind_context(request.binding_digest, request.revision, sources) {
        Ok(context) => context,
        Err(e) => return failure("learner_context_invalid", e),
    };
    let result = rt.temporal_configure_v2(
        &request.expected,
        context,
        opaque_calibration_id("learner-temporal-run"),
        request.recovery_consent,
        now_ns(&rt),
    );
    drop(s);
    match result {
        Ok(()) => temporal_status(&state, &rt).await,
        Err(e) => failure("learner_configure_failed", e),
    }
}

pub(crate) async fn begin_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<StartCommand>,
) -> Json<serde_json::Value> {
    match command {
        StartCommand::Legacy(request) => begin_phase(State(state), Json(request)).await,
        StartCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, PHASE_START_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            let s = state.write().await;
            if s.privacy_mode {
                return failure(
                    "learner_privacy_mode",
                    "Room learning unavailable in privacy mode.",
                );
            }
            if field_capture_active(&s) {
                return failure(
                    "legacy_empty_capture_active",
                    "Finish or cancel active field calibration first.",
                );
            }
            if let Err(e) = require_live_context(&s, &rt) {
                return failure("learner_source_unready", e);
            }
            let result = rt.temporal_begin_phase_v2(request, now_ns(&rt));
            drop(s);
            match result {
                Ok(()) => temporal_status(&state, &rt).await,
                Err(e) => failure("learner_phase_rejected", e),
            }
        }
    }
}
pub(crate) async fn finish_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<FinishCommand>,
) -> Json<serde_json::Value> {
    match command {
        FinishCommand::Legacy(request) => finish_phase(State(state), Json(request)).await,
        FinishCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, PHASE_FINISH_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            match rt.temporal_finish_phase_v2(request, now_ns(&rt)) {
                Ok(()) => temporal_status(&state, &rt).await,
                Err(e) => failure("learner_phase_incomplete", e),
            }
        }
    }
}
macro_rules! operation_dispatch {
    ($handler:ident, $command:ident, $legacy:ident, $schema:ident, $method:ident, $failure:literal) => {
        pub(crate) async fn $handler(
            State(state): State<SharedState>,
            Json(command): Json<$command>,
        ) -> Json<serde_json::Value> {
            match command {
                $command::Legacy(request) => $legacy(State(state), Json(request)).await,
                $command::Temporal(request) => {
                    if let Err(e) = schema_matches(&request.schema, $schema) {
                        return e;
                    }
                    let rt = match runtime(&state).await {
                        Ok(rt) => rt,
                        Err(e) => return e,
                    };
                    match rt.$method(request, now_ns(&rt)) {
                        Ok(id) => operation_status(&state, &rt, id, stringify!($legacy)).await,
                        Err(e) => failure($failure, e),
                    }
                }
            }
        }
    };
}
operation_dispatch!(
    train_dispatch,
    TrainCommand,
    train,
    TRAIN_REQUEST_SCHEMA,
    temporal_train_v2,
    "learner_train_rejected"
);
operation_dispatch!(
    evaluate_dispatch,
    EvaluateCommand,
    evaluate,
    EVALUATE_REQUEST_SCHEMA,
    temporal_evaluate_v2,
    "learner_evaluation_rejected"
);

pub(crate) async fn qualify_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<QualifyCommand>,
) -> Json<serde_json::Value> {
    match command {
        QualifyCommand::Legacy(request) => {
            selection_routes::qualify(State(state), Json(request)).await
        }
        QualifyCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, QUALIFY_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            let s = state.write().await;
            let result = selection_routes::with_current_room(&s, &rt, || {
                rt.temporal_qualify_v2(request, now_ns(&rt))
            });
            drop(s);
            match result {
                Ok(id) => operation_status(&state, &rt, id, "qualify").await,
                Err(e) => failure("learner_qualification_rejected", e),
            }
        }
    }
}
pub(crate) async fn select_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<SelectCommand>,
) -> Json<serde_json::Value> {
    match command {
        SelectCommand::Legacy(request) => {
            selection_routes::select_model(State(state), Json(request)).await
        }
        SelectCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, SELECT_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            let s = state.write().await;
            let result = selection_routes::with_current_room(&s, &rt, || {
                rt.temporal_select_v2(request, now_ns(&rt))
            });
            match result {
                Ok(value) => Json(value),
                Err(e) => failure("learner_selection_rejected", e),
            }
        }
    }
}
pub(crate) async fn selection_dispatch(
    State(state): State<SharedState>,
    Query(query): Query<SchemaQuery>,
) -> Json<serde_json::Value> {
    if let Some(schema) = query.schema.as_deref() {
        if let Err(e) = schema_matches(schema, SELECTION_SCHEMA) {
            return e;
        }
    } else {
        let actual_temporal = state
            .read()
            .await
            .room_learning
            .as_ref()
            .is_some_and(|rt| rt.temporal_v2_state_present());
        if !actual_temporal {
            return selection_routes::selection(State(state)).await;
        }
    }
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    let s = state.read().await;
    if s.privacy_mode {
        return failure(
            "learner_privacy_mode",
            "Room learning unavailable in privacy mode.",
        );
    }
    match temporal_selection(&s, &rt, now_ns(&rt)) {
        Ok(mut value) => {
            value["progress_schema"] = serde_json::json!(PAIRED_PROGRESS_SCHEMA);
            Json(value)
        }
        Err(e) => failure("learner_selection_unavailable", e),
    }
}
pub(crate) async fn restore_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<RestoreCommand>,
) -> Json<serde_json::Value> {
    match command {
        RestoreCommand::Legacy(request) => restore_capture(State(state), Json(request)).await,
        RestoreCommand::Temporal(mut request) => {
            if let Err(e) = schema_matches(&request.schema, RECOVERY_RESTORE_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            request.expected=match rt.temporal_select_profile_guarded_v2(request.profile_id.as_deref(),&request.expected){Ok(guard)=>guard,Err(e)=>return failure("learner_profile_rejected",e)};
            let now = now_ns(&rt);
            let context = match rt.temporal_recovery_context_v2(
                &request.binding_digest,
                request.revision,
                &request.source_node_ids,
                now,
            ) {
                Ok(context) => context,
                Err(e) => return failure("learner_recovery_restore_failed", e),
            };
            let s = state.read().await;
            if field_capture_active(&s) {
                return failure(
                    "learner_capture_conflict",
                    "Finish or cancel active field calibration first.",
                );
            }
            if context
                .sources
                .iter()
                .any(|spec| !source_ready(&s, spec, std::time::Instant::now()))
            {
                return failure(
                    "learner_recovery_source_unready",
                    "Restore the exact native source/PHY streams before recovery.",
                );
            }
            let result = rt.temporal_restore_v2(
                &request.expected,
                &context,
                &request.capture_generation_id,
                now,
            );
            drop(s);
            match result {
                Ok(()) => temporal_status(&state, &rt).await,
                Err(e) => failure("learner_recovery_restore_failed", e),
            }
        }
    }
}
pub(crate) async fn discard_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<DiscardCommand>,
) -> Json<serde_json::Value> {
    match command {
        DiscardCommand::Legacy(request) => discard_capture(State(state), Json(request)).await,
        DiscardCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, RECOVERY_DISCARD_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            match rt.temporal_discard_v2(
                &request.expected,
                &request.capture_generation_id,
                now_ns(&rt),
            ) {
                Ok(()) => temporal_status(&state, &rt).await,
                Err(e) => failure("learner_recovery_discard_failed", e),
            }
        }
    }
}

pub(crate) async fn pause_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<PauseCommand>,
) -> Json<serde_json::Value> {
    match command {
        PauseCommand::Legacy(request) => cancel_phase(State(state), Json(request)).await,
        PauseCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, PHASE_CANCEL_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            match rt.temporal_pause_phase_v2(request, now_ns(&rt)) {
                Ok(()) => temporal_status(&state, &rt).await,
                Err(e) => failure("learner_phase_cancel_failed", e),
            }
        }
    }
}
pub(crate) async fn cancel_operation_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<CancelOperationCommand>,
) -> Json<serde_json::Value> {
    match command {
        CancelOperationCommand::Legacy(request) => {
            cancel_training(State(state), Json(request)).await
        }
        CancelOperationCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, TRAIN_CANCEL_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            match rt.temporal_cancel_operation_v2(request, now_ns(&rt)) {
                Ok(()) => temporal_status(&state, &rt).await,
                Err(e) => failure("learner_cancel_failed", e),
            }
        }
    }
}

pub(crate) async fn deselect_dispatch(
    State(state): State<SharedState>,
    Json(command): Json<DeselectCommand>,
) -> Json<serde_json::Value> {
    match command {
        DeselectCommand::Legacy(request) => {
            selection_routes::deselect_model(State(state), Json(request)).await
        }
        DeselectCommand::Temporal(request) => {
            if let Err(e) = schema_matches(&request.schema, DESELECT_REQUEST_SCHEMA) {
                return e;
            }
            let rt = match runtime(&state).await {
                Ok(rt) => rt,
                Err(e) => return e,
            };
            let _state = state.write().await;
            match rt.temporal_deselect_v2(request, now_ns(&rt)) {
                Ok(value) => Json(value),
                Err(e) => failure("learner_deselection_rejected", e),
            }
        }
    }
}


pub(crate) async fn retry_dispatch(State(state):State<SharedState>,Json(request):Json<TemporalRetryRequestV2>)->Json<serde_json::Value>{
    if let Err(e)=schema_matches(&request.schema,RECOVERY_RETRY_REQUEST_SCHEMA){return e;}
    let rt=match runtime(&state).await{Ok(rt)=>rt,Err(e)=>return e};
    {let s=state.read().await;if field_capture_active(&s){return failure("learner_capture_conflict","Finish active field calibration first.");}}
    // Retry is metadata recovery, not capture. Its Runtime CAS still fences the
    // owner/context/profile; never hold the UDP producer's SharedState lock
    // across writer drain, filesystem barriers or cryptographic readback.
    let result=rt.temporal_retry_v2(request,now_ns(&rt));
    match result{Ok(())=>temporal_status(&state,&rt).await,Err(e)=>failure("learner_recovery_retry_failed",e)}
}
