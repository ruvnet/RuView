//! Owner-only learner controls, model selection and bounded read projection.
use super::*;

/// Selection mutations are restricted to the local owner, even if ordinary
/// API bearer authentication is disabled or a peer has calibration access.
pub(crate) fn owner_selection_routes(
    auth: wifi_densepose_sensing_server::bearer_auth::AuthState,
) -> Router<SharedState> {
    Router::new()
        .route("/api/v1/room-learner/recovery/enable-current", post(enable_current_recovery))
        .route("/api/v1/room-learner/recovery/retry", post(super::temporal_routes::retry_dispatch).layer(axum::middleware::from_fn(retry_requires_unpaired_owner)))
        .route("/api/v1/room-learner/qualify", post(super::temporal_routes::qualify_dispatch))
        .route("/api/v1/room-learner/select", post(super::temporal_routes::select_dispatch))
        .route("/api/v1/room-learner/deselect", post(super::temporal_routes::deselect_dispatch))
        .route_layer(axum::middleware::from_fn_with_state(
            auth,
            companion_integration::require_owner,
        ))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectRequest {
    expected: MutationGuard,
    model_sha256: String,
    qualification_sha256: String,
}

pub(super) fn selection_snapshot(
    state: &AppStateInner,
    rt: &RoomLearningRuntime,
    now: u64,
) -> wifi_densepose_sensing_server::room_learner::selection::ModelSelectionStatus {
    let snapshot = match state.current_room_store.as_ref() {
        Some(room) => room
            .with_current_binding(|binding| rt.selection_snapshot(now, binding))
            .unwrap_or_else(|_| rt.selection_snapshot(now, None)),
        None => rt.selection_snapshot(now, None),
    };
    snapshot
}

pub(super) fn enriched_status(
    state: &AppStateInner,
    rt: &RoomLearningRuntime,
) -> serde_json::Value {
    let now = now_ns(rt);
    let mut status = rt.status(now);
    status["model_selection"] = serde_json::to_value(selection_snapshot(state, rt, now))
        .expect("bounded model selection status serializes");
    status["native_source_diagnostics"] = ingress::source_rejection_diagnostics(state, std::time::Instant::now());
    status
}

pub(super) fn with_current_room<T>(
    state: &AppStateInner,
    rt: &RoomLearningRuntime,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let room = state
        .current_room_store
        .as_ref()
        .ok_or("current published room unavailable")?;
    room.with_current_binding(|binding| {
        let context = rt.context().ok_or("learner context unavailable")?;
        if binding != Some(context.room_digest.as_str())
            || !context
                .sources
                .iter()
                .all(|source| source_ready(state, source, std::time::Instant::now()))
        {
            return Err("current room or trained source binding mismatch".into());
        }
        operation()
    })
    .map_err(|_| "current published room unavailable".to_string())?
}

pub(crate) async fn qualify(
    State(state): State<SharedState>,
    Json(request): Json<GuardRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    // Serialize authority transitions with live producers and legacy sink admission.
    let state = state.write().await;
    match with_current_room(&state, &rt, || rt.qualify(&request.expected)) {
        Ok(receipt) => Json(
            serde_json::json!({"success":true,"qualification":receipt,"status":enriched_status(&state, &rt)}),
        ),
        Err(e) => failure("learner_qualification_rejected", e),
    }
}

pub(crate) async fn select_model(
    State(state): State<SharedState>,
    Json(request): Json<SelectRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    // Serialize authority transitions with live producers and legacy sink admission.
    let state = state.write().await;
    match with_current_room(&state, &rt, || {
        rt.select_model(
            &request.expected,
            &request.model_sha256,
            &request.qualification_sha256,
        )
    }) {
        Ok(receipt) => Json(
            serde_json::json!({"success":true,"selection":receipt,"status":enriched_status(&state, &rt)}),
        ),
        Err(e) => failure("learner_selection_rejected", e),
    }
}

pub(crate) async fn deselect_model(
    State(state): State<SharedState>,
    Json(request): Json<GuardRequest>,
) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    // Serialize authority transitions with live producers and legacy sink admission.
    let state = state.write().await;
    match rt.deselect_model(&request.expected) {
        Ok(receipt) => Json(
            serde_json::json!({"success":true,"selection":receipt,"status":enriched_status(&state, &rt)}),
        ),
        Err(e) => failure("learner_deselection_rejected", e),
    }
}

/// Read-scoped phone projection: no owner scope, mutation guard, raw context,
/// training examples or model weights are returned.
pub(crate) async fn selection(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let rt = match runtime(&state).await {
        Ok(rt) => rt,
        Err(e) => return e,
    };
    let state = state.read().await;
    Json(
        serde_json::json!({"schema":"ruview.room-learner.selection.v1","success":true,
        "progress_schema":"ruview.room-learner.progress.v1",
        "model_selection":selection_snapshot(&state, &rt, now_ns(&rt))}),
    )
}

async fn retry_requires_unpaired_owner(request:axum::extract::Request,next:axum::middleware::Next)->axum::response::Response{
    if request.extensions().get::<wifi_densepose_sensing_server::companion::Grant>().is_some(){return axum::response::IntoResponse::into_response(StatusCode::FORBIDDEN);}
    next.run(request).await
}
