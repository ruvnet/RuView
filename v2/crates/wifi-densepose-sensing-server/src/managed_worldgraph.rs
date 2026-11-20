//! Configured topology, not a geographic registration or perception result.
use axum::{extract::State, http::{header, StatusCode}, response::{IntoResponse, Response}, Json};
use serde_json::{json, Value};
use crate::{companion_room::Envelope, SharedState};

const MAX_SAFE: u64 = 9_007_199_254_740_991;

/// Ephemeral per-authority cursor. Reads of identical content never freshen it.
pub struct Cursor { epoch: String, sequence: u64, content: Option<Value> }
impl Default for Cursor {
    fn default() -> Self {
        let random: [u8; 32] = rand::random();
        Self { epoch: format!("wg-{}", random.iter().map(|b| format!("{b:02x}")).collect::<String>()), sequence: 0, content: None }
    }
}
impl Cursor {
    pub fn project(&mut self, envelope: &Envelope, privacy_denied: bool) -> Result<Value, StatusCode> {
        let content = topology(envelope, privacy_denied)?;
        if self.content.as_ref() != Some(&content) {
            if self.sequence >= MAX_SAFE { return Err(StatusCode::SERVICE_UNAVAILABLE); }
            self.sequence += 1;
            self.content = Some(content.clone());
        }
        Ok(json!({"protocol_version":2,"stream_epoch":self.epoch,"seq":self.sequence,"topology":content}))
    }
}

fn topology(envelope: &Envelope, privacy_denied: bool) -> Result<Value, StatusCode> {
    let mut result = json!({"schema":"ruview.worldgraph.managed-topology.v1",
        "coordinate_frame":"managed_room_local","axis_convention":"x_right_y_down_metres","provenance":"operator_entered",
        "registration":null,"installation_id":envelope.installation_id,
        "room_revision":envelope.revision,"binding_digest":null,
        "status":if privacy_denied {"privacy_denied"} else {"not_configured"},
        "room":null,"access_point":null,"nodes":[],"edges":[]});
    if privacy_denied { return Ok(result); }
    let Some(room) = &envelope.room else { return Ok(result); };
    let binding = envelope.binding_digest.as_deref().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let source_nodes = room["nodes"].as_array().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if envelope.revision == 0 || source_nodes.is_empty() || source_nodes.len() > 16 {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    // The store validates/sorts this immutable publication before exposing it.
    // Allowlist fields: never serialize live engine/default room, tracks, media,
    // semantic prose, learned history, raw CSI, credentials or vital estimates.
    let mut nodes: Vec<Value> = source_nodes.iter().map(|node| json!({
        "kind":"sensor","id":257 + node["nodeId"].as_u64().unwrap_or(256),
        "node_id":node["nodeId"],"slot":node["slot"],"x_m":node["xM"],"y_m":node["yM"],
        "height_m":null,"role":node["role"],"heading_deg":node["headingDeg"]
    })).collect();
    nodes.sort_by_key(|node| node["id"].as_u64().unwrap_or(0));
    let edges: Vec<Value> = nodes.iter().map(|node| json!({"from":node["id"],"to":1,"rel":"located_in"})).collect();
    result["status"] = json!("configured");
    result["binding_digest"] = json!(binding);
    result["room"] = json!({"kind":"room","id":1,"room_id":room["id"],"name":room["name"],
        "width_m":room["widthM"],"length_m":room["lengthM"],"height_m":room["heightM"]});
    let ap = &room["accessPoint"];
    if !ap.is_null() { result["access_point"] = json!({"slot":ap["slot"],"x_m":ap["xM"],"y_m":ap["yM"],"provenance":"operator_entered"}); }
    result["nodes"] = json!(nodes);
    result["edges"] = json!(edges);
    Ok(result)
}

pub async fn snapshot(State(state): State<SharedState>) -> Result<Response, StatusCode> {
    let state = state.read().await;
    let store = state.current_room_store.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let value = store.worldgraph_snapshot(state.privacy_mode)?;
    Ok(([(header::CACHE_CONTROL, "private, no-store")], Json(value)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::{Body, to_bytes}, http::Request, routing::get, Router};
    use tower::ServiceExt;
    fn envelope() -> Envelope {
        Envelope { installation_id: "installation-1".into(), revision: 7, binding_digest: Some("a".repeat(64)), room: Some(json!({
            "id":"room-1","name":"Configured room","widthM":2.5,"lengthM":3.0,"heightM":2.4,
            "nodes":[{"nodeId":15,"slot":"north_west","xM":1.25,"yM":2.0,"role":"receiver","headingDeg":150}],
            "raw_csi":"forbidden","person_track":"forbidden","accessPoint":{"slot":"north_east","xM":0,"yM":0}
        })) }
    }
    #[test]
    fn configured_projection_is_planar_and_allowlisted() {
        let value = topology(&envelope(), false).unwrap();
        assert_eq!(value["room"]["width_m"], 2.5);
        assert_eq!(value["nodes"][0]["x_m"], 1.25);
        assert_eq!(value["nodes"][0]["id"], 272);
        assert!(value["nodes"][0]["height_m"].is_null());
        assert!(value["registration"].is_null());
        assert_eq!(value["edges"][0], json!({"from":272,"to":1,"rel":"located_in"}));
        let text = value.to_string();
        for forbidden in ["raw_csi","person_track","accessPoint","confidence","latitude"] { assert!(!text.contains(forbidden)); }
    }
    #[test]
    fn cursors_cover_privacy_binding_readiness_and_restart() {
        let mut cursor = Cursor::default(); let original = envelope();
        let a = cursor.project(&original, false).unwrap();
        assert_eq!(a, cursor.project(&original, false).unwrap());
        let denied = cursor.project(&original, true).unwrap();
        assert_eq!(denied["seq"], 2);
        assert!(denied["topology"]["room"].is_null()); assert_eq!(denied["topology"]["nodes"], json!([]));
        let mut changed = original.clone(); changed.revision += 1; changed.binding_digest = Some("b".repeat(64));
        assert_eq!(cursor.project(&changed, false).unwrap()["seq"], 3);
        let mut empty = changed; empty.room = None; empty.binding_digest = None;
        let absent = cursor.project(&empty, false).unwrap(); assert_eq!(absent["topology"]["status"], "not_configured");
        assert_ne!(a["stream_epoch"], Cursor::default().project(&original, false).unwrap()["stream_epoch"]);
        assert_eq!(original.revision, 7);
    }
    #[test]
    fn refuses_missing_binding_and_cursor_overflow() {
        let mut missing = envelope(); missing.binding_digest = None; assert!(topology(&missing, false).is_err());
        let mut cursor = Cursor::default(); cursor.sequence = MAX_SAFE; assert!(cursor.project(&envelope(), false).is_err());
    }
    #[test]
    fn actual_projection_matches_cross_language_fixture() {
        let mut cursor = Cursor { epoch: format!("wg-{}", "1".repeat(64)), sequence: 0, content: None };
        let wire = cursor.project(&envelope(), false).unwrap();
        let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/managed-worldgraph-v1.json")).unwrap();
        assert_eq!(wire, fixture);
    }
    #[tokio::test]
    async fn real_route_requires_read_authority_and_never_publishes_or_selects() {
        use wifi_densepose_sensing_server::bearer_auth::{AuthState, require_bearer};
        let directory = tempfile::tempdir().unwrap();
        let store = crate::companion_room::SharedRoomStore::load(directory.path(), "installation-1").unwrap();
        let publication = json!({"expected_revision":0,"room":{
            "schema":"ruview.companion.room.v1","id":"room-1","name":"Configured room","widthM":2.5,"lengthM":3,"heightM":2.4,
            "nodes":[{"nodeId":15,"slot":"north_west","xM":1.25,"yM":2,"role":"receiver","headingDeg":150,"orientationConfirmed":true,"confirmedAtUnixMs":1}],
            "accessPoint":{"slot":"south_east","xM":2,"yM":2,"provenance":"operator_entered","confirmedAtUnixMs":1}
        }});
        let published = crate::companion_room::owner_router(store.clone()).oneshot(Request::builder().method("PUT")
            .uri("/api/v1/companion/room").header("content-type","application/json").body(Body::from(publication.to_string())).unwrap()).await.unwrap();
        assert_eq!(published.status(), StatusCode::OK);
        let original = store.snapshot().unwrap();
        let mut inner = crate::AppStateInner::minimal(); inner.current_room_store = Some(store.clone());
        let state = std::sync::Arc::new(tokio::sync::RwLock::new(inner));
        let app = Router::new().route("/api/v1/worldgraph/snapshot", get(snapshot)).with_state(state.clone())
            .layer(axum::middleware::from_fn_with_state(AuthState::from_token("owner-test-token"), require_bearer));
        let request = || Request::builder().uri("/api/v1/worldgraph/snapshot").header("authorization","Bearer owner-test-token").body(Body::empty()).unwrap();
        let denied = app.clone().oneshot(Request::builder().uri("/api/v1/worldgraph/snapshot").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let response = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK); assert_eq!(response.headers()[header::CACHE_CONTROL], "private, no-store");
        let first: Value = serde_json::from_slice(&to_bytes(response.into_body(), 16_384).await.unwrap()).unwrap();
        assert_eq!(first["topology"]["status"],"configured");
        assert_eq!(store.snapshot().unwrap().revision, original.revision);
        assert_eq!(store.snapshot().unwrap().binding_digest, original.binding_digest);
        state.write().await.privacy_mode = true;
        let response = app.oneshot(request()).await.unwrap();
        let privacy: Value = serde_json::from_slice(&to_bytes(response.into_body(), 16_384).await.unwrap()).unwrap();
        assert_eq!(privacy["topology"]["status"],"privacy_denied"); assert!(privacy["topology"]["room"].is_null());
        assert_eq!(privacy["seq"].as_u64().unwrap(), first["seq"].as_u64().unwrap()+1);
    }
}
