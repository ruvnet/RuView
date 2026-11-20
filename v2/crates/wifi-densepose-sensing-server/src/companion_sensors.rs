//! Transient geometry preview. A room digest never implies spatial registration.
use crate::companion::{Companion, Grant};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;
const SAFE: u64 = 9_007_199_254_740_991;
const MAX_BYTES: usize = 65_536;
fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SensorFrame {
    #[serde(rename = "type")]
    pub kind: String,
    pub schema: String,
    pub share_session_id: String,
    pub capture_session_id: String,
    pub coordinate_frame_id: String,
    pub sequence: u64,
    pub captured_at_unix_ms: u64,
    pub tracking_state: String,
    pub points_mm: Vec<i32>,
    pub confidences: Option<Vec<u8>>,
    pub camera_transform: [f64; 16],
    pub room_binding_digest: Option<String>,
}
impl SensorFrame {
    fn valid(&self, now: u64) -> bool {
        let tracking = self.tracking_state.as_str();
        self.kind == "sensor_frame"
            && self.schema == "ruview.companion.sensor-frame.v1"
            && identifier(&self.share_session_id)
            && identifier(&self.capture_session_id)
            && identifier(&self.coordinate_frame_id)
            && self.sequence <= SAFE
            && self.captured_at_unix_ms > 0
            && self.captured_at_unix_ms <= SAFE
            && self.captured_at_unix_ms <= now.saturating_add(1000)
            && now <= self.captured_at_unix_ms.saturating_add(3000)
            && matches!(
                tracking,
                "normal"
                    | "unavailable"
                    | "limited_initializing"
                    | "limited_excessiveMotion"
                    | "limited_insufficientFeatures"
                    | "limited_relocalizing"
            )
            && !self.points_mm.is_empty()
            && self.points_mm.len() <= 512 * 3
            && self.points_mm.len() % 3 == 0
            && self
                .points_mm
                .iter()
                .all(|x| (-30_000..=30_000).contains(x))
            && self
                .confidences
                .as_ref()
                .is_none_or(|c| c.len() == self.points_mm.len() / 3 && c.iter().all(|x| *x <= 2))
            && self.room_binding_digest.as_ref().is_none_or(|d| {
                d.len() == 64
                    && d.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            && rigid(&self.camera_transform)
    }
    fn lineage(&self) -> (&str, &str, &str) {
        (
            &self.share_session_id,
            &self.capture_session_id,
            &self.coordinate_frame_id,
        )
    }
}
fn decode(text: &str) -> Result<SensorFrame, ()> {
    if text.len() > MAX_BYTES {
        return Err(());
    }
    let value: Value = serde_json::from_str(text).map_err(|_| ())?;
    if !value.as_object().is_some_and(|m| {
        m.len() == 12 && m.contains_key("confidences") && m.contains_key("roomBindingDigest")
    }) {
        return Err(());
    }
    serde_json::from_str(text).map_err(|_| ())
}
fn rigid(m: &[f64; 16]) -> bool {
    if !m.iter().all(|v| v.is_finite())
        || [3, 7, 11].iter().any(|i| m[*i].abs() > 1e-5)
        || (m[15] - 1.).abs() > 1e-5
        || [12, 13, 14].iter().any(|i| m[*i].abs() > 30.)
    {
        return false;
    }
    for a in 0..3 {
        for b in 0..3 {
            let dot: f64 = (0..3).map(|r| m[a * 4 + r] * m[b * 4 + r]).sum();
            if (dot - if a == b { 1. } else { 0. }).abs() > 1e-3 {
                return false;
            }
        }
    }
    let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
        + m[8] * (m[1] * m[6] - m[5] * m[2]);
    (det - 1.).abs() < 1e-3
}
struct Entry {
    permission_revoked: watch::Receiver<bool>,
    until: u64,
    generation: u64,
    stop: watch::Sender<u64>,
    received: Option<u64>,
    frame: Option<SensorFrame>,
    access_until: u64,
    access_revoked: Option<watch::Receiver<bool>>,
    state: &'static str,
}
pub struct SensorStore {
    companion: Arc<Companion>,
    entries: Mutex<HashMap<String, Entry>>,
}
pub type Shared = Arc<SensorStore>;
impl SensorStore {
    pub fn new(companion: Arc<Companion>) -> Shared {
        Arc::new(Self {
            companion,
            entries: Mutex::new(HashMap::new()),
        })
    }
    fn permit(&self, id: &str) -> Result<u64, StatusCode> {
        let (_, expires) = self
            .companion
            .paired_device(id)
            .ok_or(StatusCode::NOT_FOUND)?;
        let permission_revoked = self
            .companion
            .device_revocation(id)
            .ok_or(StatusCode::NOT_FOUND)?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        // Pairing membership is bounded. Forget departed devices rather than
        // accumulating historical sensor grants across repeated enrollments.
        entries.retain(|device_id, _| self.companion.paired_device(device_id).is_some());
        let entry = entries.entry(id.into()).or_insert_with(|| Entry {
            permission_revoked: permission_revoked.clone(),
            until: 0,
            generation: 0,
            stop: watch::channel(0).0,
            received: None,
            frame: None,
            access_until: 0,
            access_revoked: None,
            state: "waiting",
        });
        entry.permission_revoked = permission_revoked;
        entry.generation = entry.generation.wrapping_add(1);
        entry.stop.send_replace(entry.generation);
        entry.until = (now_ms() / 1000 + 900).min(expires);
        entry.frame = None;
        entry.received = None;
        entry.state = "waiting";
        entry.access_until = 0;
        entry.access_revoked = None;
        Ok(entry.until)
    }
    fn revoke(&self, id: &str) -> Result<(), StatusCode> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let entry = entries.get_mut(id).ok_or(StatusCode::NOT_FOUND)?;
        entry.until = 0;
        entry.generation = entry.generation.wrapping_add(1);
        entry.stop.send_replace(entry.generation);
        entry.frame = None;
        entry.received = None;
        entry.state = "stopped";
        Ok(())
    }
    fn begin(&self, grant: &Grant) -> Result<(u64, u64, watch::Receiver<u64>), StatusCode> {
        if self.companion.paired_device(&grant.device_id).is_none()
            || *grant.revoked.borrow()
            || grant.expires_at <= now_ms() / 1000
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let e = entries
            .get_mut(&grant.device_id)
            .filter(|e| e.until > now_ms() / 1000 && !*e.permission_revoked.borrow())
            .ok_or(StatusCode::FORBIDDEN)?;
        e.generation = e.generation.wrapping_add(1);
        e.stop.send_replace(e.generation);
        e.frame = None;
        e.received = None;
        e.state = "waiting";
        e.access_until = grant.expires_at;
        e.access_revoked = Some(grant.revoked.clone());
        Ok((
            e.generation,
            e.until.min(grant.expires_at),
            e.stop.subscribe(),
        ))
    }
    fn stop(&self, id: &str, generation: u64) {
        if let Ok(mut entries) = self.entries.lock() {
            if let Some(e) = entries.get_mut(id).filter(|e| e.generation == generation) {
                e.frame = None;
                e.received = None;
                e.state = "stopped";
            }
        }
    }
    fn accept(&self, id: &str, generation: u64, frame: SensorFrame) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        let Some(e) = entries.get_mut(id).filter(|e| {
            e.generation == generation
                && !*e.permission_revoked.borrow()
                && e.until > now_ms() / 1000
                && e.access_until > now_ms() / 1000
                && !e.access_revoked.as_ref().is_some_and(|r| *r.borrow())
        }) else {
            return false;
        };
        e.frame = Some(frame);
        e.received = Some(now_ms());
        e.state = "live";
        true
    }
    fn snapshot(&self) -> Result<Value, StatusCode> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let now = now_ms();
        let mut ids: Vec<_> = entries.keys().collect();
        ids.sort();
        let devices:Vec<_>=ids.into_iter().filter_map(|id| {
            let (name,_)=self.companion.paired_device(id)?;let e=&entries[id];
            let authorized=!*e.permission_revoked.borrow() && e.until>now/1000 && (e.access_until==0 || e.access_until>now/1000) && !e.access_revoked.as_ref().is_some_and(|r| *r.borrow());
            let state=if !authorized {"stopped"} else if e.state=="live" && e.received.is_some_and(|t| now.saturating_sub(t)>3000) {"stale"} else {e.state};
            Some(json!({"deviceId":id,"deviceName":name,"sensorUntil":e.until,"state":state,"receivedAtUnixMs":if authorized {e.received} else {None},"frame":if state=="live" {e.frame.as_ref()} else {None}}))
        }).collect();
        Ok(json!({"schema":"ruview.companion.sensor-snapshot.v1","devices":devices}))
    }
}
async fn snapshot(State(s): State<Shared>) -> Result<Json<Value>, StatusCode> {
    s.snapshot().map(Json)
}
async fn permit(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    s.permit(&id)
        .map(|until| Json(json!({"sensor_until":until})))
}
async fn revoke(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    s.revoke(&id)?;
    Ok(Json(json!({"sensor_until":0})))
}
/// Must be wrapped by the exact-owner-token and loopback-peer middleware.
pub fn owner_router(s: Shared) -> Router {
    Router::new()
        .route("/api/v1/companion/sensors", get(snapshot))
        .route(
            "/api/v1/companion/devices/:id/sensors",
            post(permit).delete(revoke),
        )
        .with_state(s)
}
/// Must be wrapped by pinned companion authentication, which supplies Grant.
pub fn companion_router(s: Shared) -> Router {
    Router::new()
        .route("/companion/v1/sensors", get(upgrade))
        .with_state(s)
}
async fn upgrade(
    State(s): State<Shared>,
    Extension(grant): Extension<Grant>,
    ws: WebSocketUpgrade,
) -> Response {
    let (generation, until, stop) = match s.begin(&grant) {
        Ok(v) => v,
        Err(code) => return code.into_response(),
    };
    ws.max_message_size(MAX_BYTES)
        .max_frame_size(MAX_BYTES)
        .on_upgrade(move |socket| run(s, grant, generation, until, stop, socket))
        .into_response()
}
async fn send(socket: &mut WebSocket, text: String) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_millis(500), socket.send(Message::Text(text))).await,
        Ok(Ok(()))
    )
}
async fn run(
    s: Shared,
    grant: Grant,
    generation: u64,
    until: u64,
    mut stop: watch::Receiver<u64>,
    mut socket: WebSocket,
) {
    let id = grant.device_id;
    let mut revoked = grant.revoked;
    let ready =
        json!({"type":"sensor_ready","schema":"ruview.companion.sensor-stream.v1"}).to_string();
    let mut previous: Option<SensorFrame> = None;
    let mut last_at: Option<Instant> = None;
    let deadline = tokio::time::sleep(Duration::from_millis(
        (until * 1000).saturating_sub(now_ms()),
    ));
    tokio::pin!(deadline);
    if send(&mut socket, ready).await {
        loop {
            tokio::select! {
                _=&mut deadline=>break, _=stop.changed()=>break, _=revoked.changed()=>break,
                message=socket.recv()=>{
                    if *revoked.borrow() || *stop.borrow()!=generation || until<=now_ms()/1000 {break;}
                    let text=match message {Some(Ok(Message::Text(t)))=>t,Some(Ok(Message::Ping(_)|Message::Pong(_)))=>continue,_=>break};
                    if text.len()>MAX_BYTES {break;}
                    let frame=match decode(&text){Ok(f)=>f,Err(_)=>break};
                    if !frame.valid(now_ms()) || last_at.is_some_and(|t|t.elapsed()<Duration::from_millis(200)) || previous.as_ref().is_some_and(|p| p.lineage()!=frame.lineage() || frame.sequence<=p.sequence) {break;}
                    if !s.accept(&id,generation,frame.clone()){break;}
                    last_at=Some(Instant::now());previous=Some(frame);
                    if !send(&mut socket,json!({"type":"sensor_ack","sequence":previous.as_ref().unwrap().sequence}).to_string()).await {break;}
                }
            }
        }
    }
    s.stop(&id, generation);
    let _ = tokio::time::timeout(
        Duration::from_millis(200),
        socket.send(Message::Text(json!({"type":"sensor_stopped"}).to_string())),
    )
    .await;
    let _ = tokio::time::timeout(Duration::from_millis(200), socket.close()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use sha2::{Digest, Sha256};
    use tokio_tungstenite::{connect_async, tungstenite::Message as ClientMessage};
    fn setup() -> (tempfile::TempDir, Shared, watch::Sender<bool>, Grant) {
        let dir = tempfile::tempdir().unwrap();
        let proof = crate::companion::secret();
        let disk = json!({"version":1,"installation_id":"test","devices":[{"device_id":"phone","device_name":"Test phone","pairing_hash":format!("{:x}",Sha256::digest(proof.as_bytes())),"expires_at":now_ms()/1000+3600}]});
        crate::companion::private_dir(dir.path()).unwrap();
        crate::companion::atomic_private_write(
            &dir.path().join("companion-grants.json"),
            &serde_json::to_vec(&disk).unwrap(),
        )
        .unwrap();
        let companion = Companion::load(
            dir.path(),
            "test".into(),
            "a".repeat(64),
            "https://test.local:3443".into(),
        )
        .unwrap();
        let store = SensorStore::new(companion);
        let (tx, rx) = watch::channel(false);
        let grant = Grant {
            device_id: "phone".into(),
            expires_at: now_ms() / 1000 + 3600,
            revoked: rx,
        };
        (dir, store, tx, grant)
    }
    fn frame() -> SensorFrame {
        serde_json::from_value(json!({"type":"sensor_frame","schema":"ruview.companion.sensor-frame.v1","shareSessionId":"share","captureSessionId":"capture","coordinateFrameId":"frame","sequence":1,"capturedAtUnixMs":now_ms(),"trackingState":"normal","pointsMm":[1,2,3],"confidences":[2],"cameraTransform":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],"roomBindingDigest":null})).unwrap()
    }
    #[test]
    fn geometry_validation_and_exact_nullable_keys() {
        let f = frame();
        assert!(f.valid(now_ms()));
        let mut value = serde_json::to_value(&f).unwrap();
        value.as_object_mut().unwrap().remove("confidences");
        assert!(decode(&value.to_string()).is_err());
        let mut empty = f.clone();
        empty.points_mm.clear();
        empty.confidences = Some(vec![]);
        assert!(!empty.valid(now_ms()));
        let mut uppercase = f.clone();
        uppercase.room_binding_digest = Some("A".repeat(64));
        assert!(!uppercase.valid(now_ms()));
        let mut bad = f.clone();
        bad.points_mm = vec![1, 2];
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.camera_transform[0] = -1.;
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.camera_transform[3] = 1.;
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.captured_at_unix_ms = now_ms() - 3001;
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.captured_at_unix_ms = now_ms() + 1001;
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.confidences = Some(vec![]);
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.points_mm = vec![1; 1539];
        assert!(!bad.valid(now_ms()));
        bad = f.clone();
        bad.room_binding_digest = Some("x".repeat(64));
        assert!(!bad.valid(now_ms()));
        value = serde_json::to_value(&f).unwrap();
        value["deviceId"] = json!("spoof");
        assert!(decode(&value.to_string()).is_err());
    }
    #[test]
    fn permission_expiry_revocation_supersession_and_stale() {
        let (_dir, s, tx, g) = setup();
        assert!(matches!(s.begin(&g), Err(StatusCode::FORBIDDEN)));
        s.permit("phone").unwrap();
        let (old, _, _) = s.begin(&g).unwrap();
        assert!(s.accept("phone", old, frame()));
        let (current, _, _) = s.begin(&g).unwrap();
        s.stop("phone", old);
        assert!(s.accept("phone", current, frame()));
        assert_eq!(s.snapshot().unwrap()["devices"][0]["state"], "live");
        s.entries.lock().unwrap().get_mut("phone").unwrap().received = Some(now_ms() - 3001);
        assert_eq!(s.snapshot().unwrap()["devices"][0]["state"], "stale");
        assert!(s.snapshot().unwrap()["devices"][0]["frame"].is_null());
        tx.send_replace(true);
        assert!(s.snapshot().unwrap()["devices"][0]["frame"].is_null());
        assert!(!s.accept("phone", current, frame()));
        s.permit("phone").unwrap();
        s.entries.lock().unwrap().get_mut("phone").unwrap().until = now_ms() / 1000;
        assert_eq!(s.snapshot().unwrap()["devices"][0]["state"], "stopped");
        assert!(matches!(s.begin(&g), Err(StatusCode::UNAUTHORIZED)));
        s.revoke("phone").unwrap();
        assert_eq!(s.snapshot().unwrap()["devices"][0]["sensorUntil"], 0);
        assert!(matches!(s.permit("unknown"), Err(StatusCode::NOT_FOUND)));
    }
    async fn server(s: Shared, grant: Option<Grant>) -> (String, tokio::task::JoinHandle<()>) {
        let mut router = companion_router(s);
        if let Some(grant) = grant {
            router = router.layer(Extension(grant));
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("ws://{addr}/companion/v1/sensors"), task)
    }
    #[tokio::test]
    async fn actual_upgrade_requires_permission_and_immediate_ready_then_revoke() {
        let (_d, s, _tx, g) = setup();
        let (url, task) = server(s.clone(), Some(g)).await;
        assert!(connect_async(&url).await.is_err());
        s.permit("phone").unwrap();
        let (mut socket, _) = connect_async(&url).await.unwrap();
        let ready = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(ready.to_text().unwrap().contains("sensor_ready"));
        socket
            .send(ClientMessage::Text(
                serde_json::to_string(&frame()).unwrap(),
            ))
            .await
            .unwrap();
        assert!(socket
            .next()
            .await
            .unwrap()
            .unwrap()
            .to_text()
            .unwrap()
            .contains("sensor_ack"));
        assert_eq!(s.snapshot().unwrap()["devices"][0]["deviceId"], "phone");
        use tower::ServiceExt;
        let response = owner_router(s.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/companion/devices/phone/sensors")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let receipt: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt, json!({"sensor_until":0}));
        assert!(s.snapshot().unwrap()["devices"][0]["frame"].is_null());
        let stopped = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(stopped.to_text().unwrap().contains("sensor_stopped"));
        task.abort();
    }
    #[tokio::test]
    async fn actual_socket_rejects_replay_lineage_rate_and_unknown_input() {
        for mode in ["replay", "lineage", "rate", "unknown"] {
            let (_d, s, _tx, g) = setup();
            s.permit("phone").unwrap();
            let (url, task) = server(s.clone(), Some(g)).await;
            let (mut socket, _) = connect_async(&url).await.unwrap();
            socket.next().await.unwrap().unwrap();
            socket
                .send(ClientMessage::Text(
                    serde_json::to_string(&frame()).unwrap(),
                ))
                .await
                .unwrap();
            socket.next().await.unwrap().unwrap();
            if mode != "rate" {
                tokio::time::sleep(Duration::from_millis(210)).await;
            }
            let mut next = frame();
            if mode != "replay" {
                next.sequence = 2;
            }
            if mode == "lineage" {
                next.coordinate_frame_id = "changed".into();
            }
            let mut value = serde_json::to_value(next).unwrap();
            if mode == "unknown" {
                value["rgb"] = json!("forbidden");
            }
            socket
                .send(ClientMessage::Text(value.to_string()))
                .await
                .unwrap();
            let stopped = tokio::time::timeout(Duration::from_secs(1), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(
                stopped.to_text().unwrap().contains("sensor_stopped"),
                "{mode}"
            );
            assert!(s.snapshot().unwrap()["devices"][0]["frame"].is_null());
            task.abort();
        }
    }
    #[tokio::test]
    async fn pairing_revocation_removes_preview_and_prevents_regrant() {
        use tower::ServiceExt;
        let (_d, s, _tx, g) = setup();
        s.permit("phone").unwrap();
        let (generation, _, _) = s.begin(&g).unwrap();
        assert!(s.accept("phone", generation, frame()));
        let response = crate::companion::owner_router(s.companion.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/companion/devices/phone")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(s.snapshot().unwrap()["devices"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(matches!(s.permit("phone"), Err(StatusCode::NOT_FOUND)));
        assert!(!s.accept("phone", generation, frame()));
    }
    #[tokio::test]
    async fn established_socket_expires_sensor_and_access_permission() {
        for sensor_expiry in [true, false] {
            let (_d, s, _tx, mut g) = setup();
            s.permit("phone").unwrap();
            if sensor_expiry {
                s.entries.lock().unwrap().get_mut("phone").unwrap().until = now_ms() / 1000 + 1;
            } else {
                g.expires_at = now_ms() / 1000 + 1;
            }
            let (url, task) = server(s.clone(), Some(g)).await;
            let (mut socket, _) = connect_async(&url).await.unwrap();
            socket.next().await.unwrap().unwrap();
            socket
                .send(ClientMessage::Text(
                    serde_json::to_string(&frame()).unwrap(),
                ))
                .await
                .unwrap();
            socket.next().await.unwrap().unwrap();
            let stopped = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(stopped.to_text().unwrap().contains("sensor_stopped"));
            assert!(s.snapshot().unwrap()["devices"][0]["frame"].is_null());
            task.abort();
        }
    }
}
