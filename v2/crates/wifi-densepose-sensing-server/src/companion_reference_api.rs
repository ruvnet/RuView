//! Binary adapter: current publication + account/privacy + runtime custody.
use super::*;
use axum::response::Response;
use wifi_densepose_sensing_server::{
    companion::Grant,
    companion_references::{ReferenceMessage, ReferenceStore, MAX_BYTES},
};
#[derive(Clone)]
pub struct ReferenceApi {
    pub state: SharedState,
    pub room: Arc<companion_room::SharedRoomStore>,
    pub references: Arc<ReferenceStore>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PermitRequest {
    #[serde(default)]
    record_comparison_evidence: bool,
}
fn current(api: &ReferenceApi, state: &AppStateInner) -> bool {
    state
        .calibration_references
        .as_ref()
        .is_some_and(|s| Arc::ptr_eq(s, &api.references))
        && state
            .current_room_store
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(r, &api.room))
}
fn error(message: String) -> Response {
    (StatusCode::CONFLICT,Json(serde_json::json!({"error_code":message,"comparison_only":true,"physically_validated":false}))).into_response()
}
async fn withdraw_off_locks(references: Arc<ReferenceStore>) {
    let _ = tokio::task::spawn_blocking(move || references.withdraw()).await;
}
async fn permit(
    State(api): State<ReferenceApi>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(request): Json<PermitRequest>,
) -> Response {
    let state = api.state.read().await;
    if !current(&api, &state) {
        return error("reference_service_changed".into());
    }
    if state.privacy_mode || state.room_learning.is_none() {
        drop(state);
        withdraw_off_locks(api.references.clone()).await;
        return StatusCode::FORBIDDEN.into_response();
    }
    drop(state);
    let refs = api.references.clone();
    let result = tokio::task::spawn_blocking(move || {
        refs.permit(
            &id,
            request.record_comparison_evidence,
            chrono::Utc::now().timestamp().max(0) as u64,
        )
    })
    .await;
    let state = api.state.read().await;
    if !current(&api, &state) || state.privacy_mode || state.room_learning.is_none() {
        drop(state);
        withdraw_off_locks(api.references.clone()).await;
        return StatusCode::FORBIDDEN.into_response();
    }
    match result{
        Ok(Ok(until))=>Json(serde_json::json!({"purpose":"calibration_comparison","expires_at_unix_seconds":until,"recording_consented":request.record_comparison_evidence,"physically_validated":false})).into_response(),
        Ok(Err(e))=>error(e),Err(_)=>error("reference_recording_unavailable".into()),
    }
}
async fn revoke(
    State(api): State<ReferenceApi>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let refs = api.references.clone();
    match tokio::task::spawn_blocking(move || refs.revoke(&id)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(e)) => error(e),
        Err(_) => error("reference_recording_unavailable".into()),
    }
}
async fn summary(State(api): State<ReferenceApi>) -> Response {
    let s = api.state.read().await;
    if !current(&api, &s) {
        return error("reference_service_changed".into());
    }
    if s.privacy_mode {
        drop(s);
        withdraw_off_locks(api.references.clone()).await;
        return StatusCode::FORBIDDEN.into_response();
    }
    Json(
        api.references
            .summary(chrono::Utc::now().timestamp().max(0) as u64),
    )
    .into_response()
}
async fn temporal_comparison(State(api): State<ReferenceApi>) -> Response {
    let (runtime, binding, revision) = {
        let state = api.state.read().await;
        if !current(&api, &state) || state.privacy_mode {
            return StatusCode::FORBIDDEN.into_response();
        }
        let Some(runtime) = &state.room_learning else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let publication = api
            .room
            .with_reference_publication(|p| p.binding_digest.clone().map(|b| (b, p.revision)));
        let Ok(Some((binding, revision))) = publication else {
            return StatusCode::CONFLICT.into_response();
        };
        (runtime.clone(), binding, revision)
    };
    // Owner-private historical bytes, not a live grant or a fresh teacher. Do
    // not hold App/Room/Runtime locks while bounded private filesystem IO runs.
    let reader = runtime.clone();
    let read_binding = binding.clone();
    let read = tokio::task::spawn_blocking(move || {
        reader.temporal_comparison_readback(&read_binding, revision)
    })
    .await;
    let token = match read {
        Ok(Ok(token)) => token,
        Ok(Err(e)) => return error(e),
        Err(_) => return error("reference_read_unavailable".into()),
    };
    let state = api.state.read().await;
    if !current(&api, &state)
        || state.privacy_mode
        || !state
            .room_learning
            .as_ref()
            .is_some_and(|rt| Arc::ptr_eq(rt, &runtime))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = api.room.with_reference_publication(|publication| {
        if publication.binding_digest.as_deref() != Some(binding.as_str())
            || publication.revision != revision
        {
            return Err("reference_publication_changed".into());
        }
        token.finish(
            &runtime,
            publication.binding_digest.as_deref().unwrap_or(""),
            publication.revision,
        )
    });
    match result {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(e)) => error(e),
        Err(_) => StatusCode::CONFLICT.into_response(),
    }
}
async fn ingest(
    State(api): State<ReferenceApi>,
    Extension(grant): Extension<Grant>,
    request: axum::extract::Request,
) -> Response {
    // Consume/time-limit the body before acquiring publication, app or learner locks.
    let bytes = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), MAX_BYTES),
    )
    .await
    {
        Ok(Ok(b)) => b,
        Ok(Err(_)) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(_) => return StatusCode::REQUEST_TIMEOUT.into_response(),
    };
    let state = api.state.read().await;
    if !current(&api, &state) {
        return error("reference_service_changed".into());
    }
    if state.privacy_mode {
        drop(state);
        withdraw_off_locks(api.references.clone()).await;
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(runtime) = &state.room_learning else {
        drop(state);
        withdraw_off_locks(api.references.clone()).await;
        return StatusCode::NOT_FOUND.into_response();
    };
    let now_ns = runtime.receiver_now_ns();
    let message = match ReferenceMessage::decode(&bytes, now_ns / 1_000_000) {
        Ok(m) => m,
        Err(e) => return error(e),
    };
    // Lock order is app -> published room -> learner -> reference store.
    match api.room.with_reference_publication(|publication| {
        let lineage = message.lineage();
        if publication.revision != lineage.room_revision
            || publication.binding_digest.as_deref() != Some(lineage.room_binding_digest.as_str())
        {
            return Err("reference_publication_mismatch".into());
        }
        let mut ids = publication
            .room
            .as_ref()
            .and_then(|r| r["nodes"].as_array())
            .ok_or("reference_publication_missing")?
            .iter()
            .map(|n| {
                n["nodeId"]
                    .as_u64()
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or("reference_publication_invalid")
            })
            .collect::<Result<Vec<_>, _>>()?;
        ids.sort_unstable();
        runtime.with_reference_scope(
            &lineage.room_binding_digest.clone(),
            publication.revision,
            &ids,
            now_ns,
            |scope, admitted| {
                api.references
                    .admit_at(&grant, scope, message, now_ns, admitted)
            },
        )
    }) {
        Ok(Ok(reply)) => Json(reply).into_response(),
        Ok(Err(e)) => error(e),
        Err(code) => code.into_response(),
    }
}
pub fn owner_router(api: ReferenceApi) -> Router {
    Router::new()
        .route("/api/v1/companion/references", get(summary))
        .route(
            "/api/v1/companion/references/temporal-comparison",
            get(temporal_comparison),
        )
        .route(
            "/api/v1/companion/devices/:id/references",
            post(permit).delete(revoke),
        )
        .with_state(api)
}
pub fn peer_router(api: ReferenceApi) -> Router {
    Router::new()
        .route("/companion/v1/calibration-references", post(ingest))
        .with_state(api)
}
