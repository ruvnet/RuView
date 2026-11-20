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
    /// Advances the cursor only when the projected content changed, and returns that content.
    fn advance(&mut self, content: Value) -> Result<Value, StatusCode> {
        if self.content.as_ref() != Some(&content) {
            if self.sequence >= MAX_SAFE { return Err(StatusCode::SERVICE_UNAVAILABLE); }
            self.sequence += 1;
            self.content = Some(content.clone());
        }
        Ok(content)
    }
    pub fn project(&mut self, envelope: &Envelope, privacy_denied: bool) -> Result<Value, StatusCode> {
        let content = self.advance(topology(envelope, privacy_denied)?)?;
        Ok(json!({"protocol_version":2,"stream_epoch":self.epoch,"seq":self.sequence,"topology":content}))
    }
    /// A typed managed-room projection, never an ordinary engine snapshot.
    /// Validate before committing the shared cursor; failed reads do not freshen it.
    pub fn project_engine(&mut self, envelope: &Envelope, privacy_denied: bool) -> Result<Value, StatusCode> {
        let content = topology(envelope, privacy_denied)?;
        let projection = engine_projection(&content)?;
        self.advance(content)?;
        Ok(json!({"protocol_version":3,"stream_epoch":self.epoch,"seq":self.sequence,"projection":projection}))
    }
}

/// A configured layout projection, deliberately distinct from an engine snapshot.
/// Engine schema v2 requires numbers where this source has unknown values.
#[derive(serde::Serialize)]
struct ManagedEngineProjection {
    schema: &'static str,
    provenance: &'static str,
    coordinate_frame: &'static str,
    axis_convention: &'static str,
    status: ProjectionStatus,
    registration: (),
    next_id: u64,
    next_edge_id: u64,
    nodes: Vec<ProjectedNode>,
    edges: Vec<ProjectedEdge>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ProjectionStatus { Configured, NotConfigured, PrivacyDenied }
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ProjectedNode {
    Room { id: u64, area_id: Option<String>, name: String, bounds_enu: ProjectedBounds, floor: Option<i32> },
    Sensor { id: u64, device_id: String, position: ProjectedPosition, modality: &'static str },
}
impl ProjectedNode {
    fn id(&self) -> u64 { match self { Self::Room { id, .. } | Self::Sensor { id, .. } => *id } }
}
#[derive(serde::Serialize)]
struct ProjectedBounds { shape: &'static str, min_e: f64, min_n: f64, max_e: f64, max_n: f64 }
#[derive(serde::Serialize)]
struct ProjectedPosition { east_m: f64, north_m: f64, up_m: Option<f64> }
#[derive(serde::Serialize)]
struct ProjectedEdge { id: u64, from: u64, to: u64, edge: ProjectedMembership }
#[derive(serde::Serialize)]
struct ProjectedMembership { rel: &'static str, since_unix_ms: Option<u64> }

fn engine_projection(topology: &Value) -> Result<ManagedEngineProjection, StatusCode> {
    let status = match topology["status"].as_str() {
        Some("configured") => ProjectionStatus::Configured,
        Some("not_configured") => ProjectionStatus::NotConfigured,
        Some("privacy_denied") => ProjectionStatus::PrivacyDenied,
        _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    if matches!(status, ProjectionStatus::Configured) {
        let room = &topology["room"];
        let (Some(width), Some(length), Some(name)) =
            (room["width_m"].as_f64(), room["length_m"].as_f64(), room["name"].as_str())
            else { return Err(StatusCode::INTERNAL_SERVER_ERROR); };
        if !width.is_finite() || !length.is_finite() || width <= 0.0 || length <= 0.0 {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        nodes.push(ProjectedNode::Room {
            id: 1, area_id: None, name: name.to_owned(), floor: None,
            bounds_enu: ProjectedBounds { shape: "rectangle", min_e: 0.0, min_n: 0.0, max_e: width, max_n: length },
        });
        let source_nodes = topology["nodes"].as_array().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        for n in source_nodes {
            let (Some(id), Some(node_id), Some(x), Some(y)) =
                (n["id"].as_u64(), n["node_id"].as_u64(), n["x_m"].as_f64(), n["y_m"].as_f64())
                else { return Err(StatusCode::INTERNAL_SERVER_ERROR); };
            if !x.is_finite() || !y.is_finite() || !(0.0..=width).contains(&x) || !(0.0..=length).contains(&y) {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            nodes.push(ProjectedNode::Sensor { id, device_id: format!("node-{node_id}"),
                position: ProjectedPosition { east_m: x, north_m: length - y, up_m: None }, modality: "wifi_csi" });
            // This is configured membership, not an observed relationship time.
            edges.push(ProjectedEdge { id: edges.len() as u64 + 1, from: id, to: 1,
                edge: ProjectedMembership { rel: "located_in", since_unix_ms: None } });
        }
    }
    let next_id = nodes.iter().map(ProjectedNode::id).max().unwrap_or(0)
        .checked_add(1).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(ManagedEngineProjection {
        schema: "ruview.worldgraph.managed-engine-projection.v1", provenance: "operator_entered",
        coordinate_frame: "managed_room_local", axis_convention: "x_right_y_up_metres", status,
        registration: (), next_id, next_edge_id: edges.len() as u64 + 1, nodes, edges,
    })
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

pub async fn engine_snapshot(State(state): State<SharedState>) -> Result<Response, StatusCode> {
    let state = state.read().await;
    let store = state.current_room_store.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let value = store.worldgraph_engine_snapshot(state.privacy_mode)?;
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
    fn engine_projection_preserves_unknown_registration() {
        let wire = Cursor::default().project_engine(&envelope(), false).unwrap();
        assert_eq!(wire["protocol_version"], 3);
        assert!(wire.get("graph").is_none());
        assert_eq!(wire["projection"]["schema"], "ruview.worldgraph.managed-engine-projection.v1");
        assert!(wire["projection"].get("registration").unwrap().is_null());
    }
    #[test]
    fn engine_projection_preserves_unknown_vertical_and_time_values() {
        let wire = Cursor::default().project_engine(&envelope(), false).unwrap();
        let projection = &wire["projection"];
        assert!(projection["nodes"][0].get("floor").unwrap().is_null());
        assert!(projection["nodes"][1]["position"].get("up_m").unwrap().is_null());
        assert!(projection["edges"][0]["edge"].get("since_unix_ms").unwrap().is_null());
    }
    #[test]
    fn failed_engine_projection_does_not_advance_the_shared_cursor() {
        let mut cursor = Cursor::default();
        let mut malformed = envelope();
        malformed.room.as_mut().unwrap()["widthM"] = json!("invalid");
        assert!(cursor.project_engine(&malformed, false).is_err());
        assert_eq!(cursor.project_engine(&envelope(), false).unwrap()["seq"], 1);
    }
    #[test]
    fn planar_projection_retains_valid_zero_and_room_boundaries() {
        let mut room = envelope();
        room.room.as_mut().unwrap()["nodes"][0]["xM"] = json!(0.0);
        room.room.as_mut().unwrap()["nodes"][0]["yM"] = json!(3.0);
        let p = Cursor::default().project_engine(&room, false).unwrap();
        assert_eq!(p["projection"]["nodes"][1]["position"], json!({"east_m":0.0,"north_m":0.0,"up_m":null}));
        room.room.as_mut().unwrap()["nodes"][0]["xM"] = json!(2.5);
        room.room.as_mut().unwrap()["nodes"][0]["yM"] = json!(0.0);
        let p = Cursor::default().project_engine(&room, false).unwrap();
        assert_eq!(p["projection"]["nodes"][1]["position"]["north_m"], 3.0);
        assert_eq!(p["projection"]["nodes"][1]["position"]["east_m"], 2.5);
    }
    #[test]
    fn invalid_planar_projection_does_not_issue_a_cursor() {
        for (field, value) in [("xM", json!(-0.1)), ("xM", json!(2.6)), ("yM", json!(3.1)), ("yM", Value::Null)] {
            let mut malformed = envelope();
            malformed.room.as_mut().unwrap()["nodes"][0][field] = value;
            let mut cursor = Cursor::default();
            assert!(cursor.project_engine(&malformed, false).is_err());
            assert_eq!(cursor.project_engine(&envelope(), false).unwrap()["seq"], 1);
        }
    }
    #[test]
    fn projection_announces_source_status_and_withholds_private_geometry() {
        let original = envelope();
        for (private, missing, status) in [(false, false, "configured"), (false, true, "not_configured"), (true, false, "privacy_denied")] {
            let mut source = original.clone();
            if missing { source.room = None; source.binding_digest = None; }
            let p = Cursor::default().project_engine(&source, private).unwrap();
            let p = &p["projection"];
            assert_eq!(p["status"], status);
            assert_eq!(p["provenance"], "operator_entered");
            assert_eq!(p["coordinate_frame"], "managed_room_local");
            assert_eq!(p["axis_convention"], "x_right_y_up_metres");
            if private || missing {
                assert_eq!(p["nodes"], json!([])); assert_eq!(p["edges"], json!([]));
                assert_eq!(p["next_id"], 1); assert_eq!(p["next_edge_id"], 1);
                assert!(!p.to_string().contains("Configured room"));
            }
        }
    }
    #[test]
    fn projection_keeps_stable_reads_at_sequence_limit_but_refuses_changed_content() {
        let mut cursor = Cursor::default();
        cursor.project_engine(&envelope(), false).unwrap();
        cursor.sequence = MAX_SAFE;
        assert_eq!(cursor.project_engine(&envelope(), false).unwrap()["seq"], MAX_SAFE);
        assert!(cursor.project_engine(&envelope(), true).is_err());
        assert_eq!(cursor.project_engine(&envelope(), false).unwrap()["seq"], MAX_SAFE);
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
    #[test]
    fn engine_projection_matches_fixture_and_flips_y_to_north() {
        let mut cursor = Cursor { epoch: format!("wg-{}", "1".repeat(64)), sequence: 0, content: None };
        let wire = cursor.project_engine(&envelope(), false).unwrap();
        let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/managed-worldgraph-engine-projection-v1.json")).unwrap();
        assert_eq!(wire, fixture);
        // y_m = 2.0 down from the top of a 3.0 m room is 1.0 m north of the south wall.
        assert_eq!(wire["projection"]["nodes"][1]["position"]["north_m"], 1.0);
    }
    #[test]
    fn engine_projection_is_empty_unless_configured_and_shares_the_cursor() {
        let mut cursor = Cursor::default();
        let empty = Envelope { installation_id: "i".into(), revision: 1, binding_digest: None, room: None };
        let g = cursor.project_engine(&empty, false).unwrap();
        assert_eq!((g["projection"]["nodes"].clone(), g["projection"]["edges"].clone(), g["projection"]["next_id"].clone(), g["projection"]["next_edge_id"].clone()), (json!([]), json!([]), json!(1), json!(1)));
        assert_eq!(g["seq"], 1);
        // Same content through either projection never advances seq; a changed room advances it once.
        assert_eq!(cursor.project(&empty, false).unwrap()["seq"], 1);
        assert_eq!(cursor.project_engine(&envelope(), false).unwrap()["seq"], 2);
        assert_eq!(cursor.project(&envelope(), false).unwrap()["seq"], 2);
        let denied = cursor.project_engine(&envelope(), true).unwrap();
        assert_eq!(denied["projection"]["nodes"], json!([]));
        assert_eq!(denied["seq"], 3);
    }
    #[test]
    fn engine_projection_never_serialises_forbidden_fields() {
        let text = Cursor::default().project_engine(&envelope(), false).unwrap().to_string();
        for forbidden in ["raw_csi", "person_track", "forbidden", "installation-1", "binding"] { assert!(!text.contains(forbidden), "{forbidden}"); }
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
        let app = Router::new().route("/api/v1/worldgraph/snapshot", get(snapshot)).route("/api/v1/worldgraph/engine", get(engine_snapshot)).with_state(state.clone())
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
        let engine_request = || Request::builder().uri("/api/v1/worldgraph/engine").header("authorization","Bearer owner-test-token").body(Body::empty()).unwrap();
        let engine_denied = app.clone().oneshot(Request::builder().uri("/api/v1/worldgraph/engine").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(engine_denied.status(), StatusCode::UNAUTHORIZED);
        let engine_response = app.clone().oneshot(engine_request()).await.unwrap();
        assert_eq!(engine_response.status(), StatusCode::OK); assert_eq!(engine_response.headers()[header::CACHE_CONTROL], "private, no-store");
        let engine: Value = serde_json::from_slice(&to_bytes(engine_response.into_body(), 16_384).await.unwrap()).unwrap();
        assert_eq!((engine["protocol_version"].clone(), engine["projection"]["schema"].clone(), engine["seq"].clone()), (json!(3), json!("ruview.worldgraph.managed-engine-projection.v1"), first["seq"].clone()));
        assert_eq!(store.snapshot().unwrap().revision, original.revision);
        let head = app.clone().oneshot(Request::builder().method("HEAD").uri("/api/v1/worldgraph/engine")
            .header("authorization", "Bearer owner-test-token").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[header::CACHE_CONTROL], "private, no-store");
        assert!(to_bytes(head.into_body(), 16_384).await.unwrap().is_empty());
        let post = app.clone().oneshot(Request::builder().method("POST").uri("/api/v1/worldgraph/engine")
            .header("authorization", "Bearer owner-test-token").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
        let unchanged = app.clone().oneshot(request()).await.unwrap();
        let unchanged: Value = serde_json::from_slice(&to_bytes(unchanged.into_body(), 16_384).await.unwrap()).unwrap();
        assert_eq!(unchanged, first);
        state.write().await.privacy_mode = true;
        let response = app.oneshot(request()).await.unwrap();
        let privacy: Value = serde_json::from_slice(&to_bytes(response.into_body(), 16_384).await.unwrap()).unwrap();
        assert_eq!(privacy["topology"]["status"],"privacy_denied"); assert!(privacy["topology"]["room"].is_null());
        assert_eq!(privacy["seq"].as_u64().unwrap(), first["seq"].as_u64().unwrap()+1);
    }
}
