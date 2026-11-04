//! Pair-read calibration progress only; never expose the owner's status object.
use super::*;
use axum::response::{IntoResponse, Response};
use wifi_densepose_sensing_server::room_learner::lifecycle::PublishedProgressRoom;
const SCHEMA:&str="ruview.room-learner.progress.v1";
fn denied(status:StatusCode,code:&'static str)->Response {
    (status,Json(serde_json::json!({"schema":SCHEMA,"success":false,"error_code":code}))).into_response()
}
pub(crate) async fn progress(State(state):State<SharedState>, grant:Option<axum::Extension<wifi_densepose_sensing_server::companion::Grant>>)->Response {
    let state=state.read().await;
    if state.privacy_mode {return denied(StatusCode::FORBIDDEN,"privacy_redacted");}
    let Some(runtime)=state.room_learning.as_ref() else {return denied(StatusCode::SERVICE_UNAVAILABLE,"learner_unavailable");};
    let Some(store)=state.current_room_store.as_ref() else {return denied(StatusCode::SERVICE_UNAVAILABLE,"publication_unavailable");};
    let Some(publication)=store.snapshot() else {return denied(StatusCode::SERVICE_UNAVAILABLE,"publication_unavailable");};
    if state.installation_id.as_deref()!=Some(publication.installation_id.as_str()) {return denied(StatusCode::SERVICE_UNAVAILABLE,"publication_unavailable");}
    let identity=publication.binding_digest.as_ref().map(|binding|PublishedProgressRoom{binding_digest:binding.clone(),revision:publication.revision});
    // The binding hashes installation, published revision and complete room.
    // Hold its lock through projection; reject a publication changed since the
    // initial envelope snapshot instead of mixing its identity with old counts.
    match store.with_current_binding(|binding| {
        if binding!=publication.binding_digest.as_deref() {return Err("publication_changed");}
        let context=runtime.context();
        let ready:Vec<_>=context.as_ref().map(|c|c.sources.iter().filter(|s|source_ready(&state,s,std::time::Instant::now())).map(|s|s.source_id).collect()).unwrap_or_default();
        let ready_context=context.as_ref().map(|c|c.digest());
        runtime.paired_progress_snapshot(&publication.installation_id,&state.calibration_boot_epoch,identity,&ready,ready_context.as_deref(),runtime.receiver_now_ns())
            .map_err(|_|"progress_unavailable")
    }) {
        Ok(Ok(mut snapshot))=>{
            if let Some(axum::Extension(grant))=grant {
                let Some(until)=grant.expires_at.checked_mul(1000) else {return StatusCode::UNAUTHORIZED.into_response();};
                let now=chrono::Utc::now().timestamp_millis().max(0) as u64;
                if *grant.revoked.borrow() || until<=snapshot.observed_at_unix_ms || until<=now {return StatusCode::UNAUTHORIZED.into_response();}
                snapshot.expires_at_unix_ms=snapshot.expires_at_unix_ms.min(until);
            }
            Json(snapshot).into_response()
        },
        Ok(Err("publication_changed"))=>denied(StatusCode::CONFLICT,"publication_changed"),
        _=>denied(StatusCode::SERVICE_UNAVAILABLE,"progress_unavailable"),
    }
}
