//! Explicit, bounded artifact handoff. Imported geometry never grants RF/AR authority.
use super::companion_room::SharedRoomStore;
use axum::{extract::State, http::StatusCode, routing::get, Extension, Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use wifi_densepose_sensing_server::companion::{Companion, Grant};
const MAX_PAYLOAD: usize = 48 * 1024;
fn now() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Surface {
    id: String,
    kind: String,
    dimensions_m: [f64; 3],
    transform: [f64; 16],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Node {
    id: String,
    role: String,
    position_m: [f64; 3],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Geometry {
    schema: String,
    installation_id: String,
    room_revision: u64,
    room_binding_digest: String,
    coordinate_frame_id: String,
    source_scan_id: String,
    captured_at_unix_ms: u64,
    exported_at_unix_ms: u64,
    provenance: String,
    calibration_digest: Option<String>,
    surfaces: Vec<Surface>,
    nodes: Vec<Node>,
}
impl Geometry {
    fn valid(&self, time: u64) -> bool {
        self.schema == "ruview.companion.spatial-artifact.v1"
            && id(&self.installation_id)
            && self.room_revision > 0
            && self.room_revision <= 9_007_199_254_740_991
            && hex(&self.room_binding_digest)
            && id(&self.coordinate_frame_id)
            && id(&self.source_scan_id)
            && self.captured_at_unix_ms > 0
            && self.captured_at_unix_ms <= self.exported_at_unix_ms
            && self.exported_at_unix_ms <= time.saturating_add(5000)
            && time <= self.exported_at_unix_ms.saturating_add(900_000)
            && self.provenance == "historical_geometry_only"
            && self.calibration_digest.as_deref().is_none_or(hex)
            && !self.surfaces.is_empty()
            && self.surfaces.len() <= 128
            && self.nodes.len() <= 64
            && self.surfaces.iter().all(|s| {
                id(&s.id)
                    && ["wall", "door", "window", "opening", "floor"].contains(&s.kind.as_str())
                    && s.dimensions_m
                        .iter()
                        .all(|v| v.is_finite() && *v >= 0. && *v <= 1000.)
                    && s.transform
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 1000.)
                    && rigid(&s.transform)
            })
            && self.nodes.iter().all(|n| {
                id(&n.id)
                    && ["esp32_csi_receiver", "wifi_transmitter", "reference"]
                        .contains(&n.role.as_str())
                    && n.position_m
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 1000.)
            })
            && self
                .surfaces
                .iter()
                .map(|s| &s.id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.surfaces.len()
            && self
                .nodes
                .iter()
                .map(|s| &s.id)
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.nodes.len()
    }
}
fn rigid(m: &[f64; 16]) -> bool {
    if m[3].abs() > 1e-4 || m[7].abs() > 1e-4 || m[11].abs() > 1e-4 || (m[15] - 1.).abs() > 1e-4 {
        return false;
    }
    for a in 0..3 {
        for b in 0..3 {
            let dot = (0..3).map(|r| m[a * 4 + r] * m[b * 4 + r]).sum::<f64>();
            if (dot - if a == b { 1. } else { 0. }).abs() > 0.015 {
                return false;
            }
        }
    }
    if m[12..15].iter().any(|v| v.abs() > 30.) {
        return false;
    }
    let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
        + m[8] * (m[1] * m[6] - m[5] * m[2]);
    (det - 1.).abs() < 0.02
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Artifact {
    digest: String,
    payload: String,
    publisher_device_id: String,
    received_at_unix_ms: u64,
    accepted_at_unix_ms: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Saved {
    schema: String,
    owner_scope: String,
    installation_id: String,
    accepted: Vec<Artifact>,
}
struct Receipt {
    nonce: String,
    until: u64,
    pending: Option<Artifact>,
    revoked: tokio::sync::watch::Receiver<bool>,
}
struct Inner {
    saved: Saved,
    receipts: HashMap<String, Receipt>,
}
pub struct SpatialStore {
    fault: AtomicBool,
    path: PathBuf,
    service: Arc<Companion>,
    room: Arc<SharedRoomStore>,
    inner: Mutex<Inner>,
}
type Shared = Arc<SpatialStore>;
impl SpatialStore {
    pub fn load(
        dir: &Path,
        scope: &str,
        installation: &str,
        service: Arc<Companion>,
        room: Arc<SharedRoomStore>,
    ) -> std::io::Result<Shared> {
        if !hex(scope) {
            return Err(std::io::Error::other(
                "spatial handoff requires account ownership",
            ));
        }
        let path = dir.join("accepted-spatial-artifacts.json");
        let saved = match (|| -> std::io::Result<Vec<u8>> {
            let meta = fs::symlink_metadata(&path)?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 2 * 1024 * 1024 {
                return Err(std::io::Error::other("invalid spatial history file"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o077 != 0 {
                    return Err(std::io::Error::other(
                        "spatial history permissions are not private",
                    ));
                }
            }
            fs::read(&path)
        })() {
            Ok(bytes) => {
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err(std::io::Error::other("spatial history too large"));
                }
                let s: Saved = serde_json::from_slice(&bytes)?;
                if s.schema != "ruview.companion.spatial-history.v1"
                    || s.owner_scope != scope
                    || s.installation_id != installation
                    || s.accepted.len() > 32
                    || s.accepted.iter().any(|a| {
                        a.payload.len() > MAX_PAYLOAD
                            || hash(&a.payload) != a.digest
                            || a.accepted_at_unix_ms.is_none()
                    })
                {
                    return Err(std::io::Error::other(
                        "invalid spatial history ownership or digest",
                    ));
                }
                for a in &s.accepted {
                    let g: Geometry = serde_json::from_str(&a.payload)?;
                    if !g.valid(g.exported_at_unix_ms) || g.installation_id != installation {
                        return Err(std::io::Error::other("invalid spatial geometry"));
                    }
                }
                s
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Saved {
                schema: "ruview.companion.spatial-history.v1".into(),
                owner_scope: scope.into(),
                installation_id: installation.into(),
                accepted: vec![],
            },
            Err(e) => return Err(e),
        };
        Ok(Arc::new(Self {
            fault: AtomicBool::new(false),
            path,
            service,
            room,
            inner: Mutex::new(Inner {
                saved,
                receipts: HashMap::new(),
            }),
        }))
    }
    fn persist(&self, s: &Saved) -> std::io::Result<()> {
        let tmp = self
            .path
            .with_extension(format!("{}.tmp", rand::random::<u64>()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&tmp)?;
        file.write_all(&serde_json::to_vec(s)?)?;
        file.sync_all()?;
        fs::rename(tmp, &self.path)?;
        fs::File::open(self.path.parent().unwrap())?.sync_all()?;
        Ok(())
    }
    fn matches(&self, g: &Geometry) -> bool {
        let r = self.room.snapshot();
        r.is_some_and(|r| {
            r.installation_id == g.installation_id
                && r.revision == g.room_revision
                && r.binding_digest.as_deref() == Some(&g.room_binding_digest)
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Upload {
    nonce: String,
    digest: String,
    payload: String,
}
#[derive(Deserialize)]
#[serde(tag = "action", deny_unknown_fields, rename_all = "camelCase")]
enum Action {
    Receive {
        #[serde(rename = "deviceId")]
        device_id: String,
    },
    Accept {
        digest: String,
    },
    Reject {
        digest: String,
    },
}
fn active(r: &Receipt, t: u64) -> bool {
    r.until > t && !*r.revoked.borrow()
}
async fn snapshot(State(s): State<Shared>) -> Result<Json<serde_json::Value>, StatusCode> {
    if s.fault.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let inner = s
        .inner
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let t = now();
    let pending: Vec<_> = inner
        .receipts
        .values()
        .filter(|r| active(r, t))
        .filter_map(|r| r.pending.as_ref())
        .collect();
    Ok(Json(
        serde_json::json!({"schema":"ruview.companion.spatial-inbox.v1","pending":pending,"accepted":inner.saved.accepted}),
    ))
}
async fn ticket(
    State(s): State<Shared>,
    Extension(g): Extension<Grant>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if s.fault.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let inner = s
        .inner
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let t = now();
    if *g.revoked.borrow() || g.expires_at * 1000 <= t {
        return Err(StatusCode::FORBIDDEN);
    }
    let r = inner
        .receipts
        .get(&g.device_id)
        .filter(|r| active(r, t) && r.pending.is_none())
        .ok_or(StatusCode::FORBIDDEN)?;
    Ok(Json(serde_json::json!({"nonce":r.nonce,"until":r.until})))
}
async fn upload(
    State(s): State<Shared>,
    Extension(g): Extension<Grant>,
    Json(u): Json<Upload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if s.fault.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let t = now();
    if *g.revoked.borrow() || g.expires_at * 1000 <= t {
        return Err(StatusCode::FORBIDDEN);
    }
    if u.payload.len() > MAX_PAYLOAD || !hex(&u.digest) || hash(&u.payload) != u.digest {
        return Err(StatusCode::BAD_REQUEST);
    }
    let raw: serde_json::Value =
        serde_json::from_str(&u.payload).map_err(|_| StatusCode::BAD_REQUEST)?;
    if raw.get("calibrationDigest").is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let geometry: Geometry =
        serde_json::from_str(&u.payload).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !geometry.valid(t) || !s.matches(&geometry) {
        return Err(StatusCode::CONFLICT);
    }
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if i.saved.accepted.iter().any(|a| a.digest == u.digest) {
        return Err(StatusCode::CONFLICT);
    }
    let r = i
        .receipts
        .get_mut(&g.device_id)
        .filter(|r| active(r, t) && r.nonce == u.nonce && r.pending.is_none())
        .ok_or(StatusCode::FORBIDDEN)?;
    r.pending = Some(Artifact {
        digest: u.digest.clone(),
        payload: u.payload,
        publisher_device_id: g.device_id,
        received_at_unix_ms: t,
        accepted_at_unix_ms: None,
    });
    Ok(Json(
        serde_json::json!({"digest":u.digest,"state":"pending_review"}),
    ))
}
async fn action(
    State(s): State<Shared>,
    Json(a): Json<Action>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if s.fault.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let t = now();
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let accept = matches!(&a, Action::Accept { .. });
    match a {
        Action::Receive { device_id } => {
            let (_, expiry) = s
                .service
                .paired_device(&device_id)
                .ok_or(StatusCode::FORBIDDEN)?;
            let revoked = s
                .service
                .device_revocation(&device_id)
                .ok_or(StatusCode::FORBIDDEN)?;
            i.receipts.retain(|_, r| active(r, t));
            if i.receipts.len() >= 256 || i.saved.accepted.len() >= 32 {
                return Err(StatusCode::CONFLICT);
            }
            let until = (t + 900_000).min(expiry * 1000);
            if until <= t {
                return Err(StatusCode::FORBIDDEN);
            }
            i.receipts.insert(
                device_id,
                Receipt {
                    nonce: format!("{:032x}", rand::random::<u128>()),
                    until,
                    pending: None,
                    revoked,
                },
            );
        }
        Action::Accept { digest } | Action::Reject { digest } => {
            let (device, r) = i
                .receipts
                .iter()
                .find(|(_, r)| {
                    active(r, t) && r.pending.as_ref().is_some_and(|p| p.digest == digest)
                })
                .ok_or(StatusCode::CONFLICT)?;
            let device = device.clone();
            let mut artifact = r.pending.clone().unwrap();
            if accept {
                let geometry: Geometry =
                    serde_json::from_str(&artifact.payload).map_err(|_| StatusCode::BAD_REQUEST)?;
                if !s.matches(&geometry) || i.saved.accepted.len() >= 32 {
                    return Err(StatusCode::CONFLICT);
                }
                artifact.accepted_at_unix_ms = Some(t);
                i.saved.accepted.push(artifact);
                if s.persist(&i.saved).is_err() {
                    s.fault.store(true, Ordering::SeqCst);
                    i.saved.accepted.pop();
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
            i.receipts.remove(&device);
        }
    }
    Ok(Json(serde_json::json!({"state":"ok"})))
}
pub fn owner_router(s: Shared) -> Router {
    Router::new()
        .route("/api/v1/companion/spatial", get(snapshot).post(action))
        .with_state(s)
}
pub fn peer_router(s: Shared) -> Router {
    Router::new()
        .route("/companion/v1/spatial", get(ticket).post(upload))
        .with_state(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tower::ServiceExt;
    async fn setup() -> (tempfile::TempDir, Shared, Grant) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("companion-grants.json"),serde_json::to_vec(&json!({"version":1,"installation_id":"test","devices":[{"device_id":"phone","device_name":"Phone","pairing_hash":"a".repeat(64),"expires_at":now()/1000+3600}]})).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                d.path().join("companion-grants.json"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        let c = Companion::load(
            d.path(),
            "test".into(),
            "a".repeat(64),
            "https://test.local:3443".into(),
        )
        .unwrap();
        let room = SharedRoomStore::load(d.path(), "test").unwrap();
        let body = json!({"expected_revision":0,"room":{"schema":"ruview.companion.room.v1","id":"room-1","name":"Room","widthM":2.5,"lengthM":2.5,"heightM":2.4,"nodes":[{"nodeId":15,"slot":"north_west","xM":0,"yM":0,"role":"receiver","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1}],"accessPoint":{"slot":"south_east","xM":2.5,"yM":2.5,"provenance":"operator_entered","confirmedAtUnixMs":1}}});
        let r = super::super::companion_room::owner_router(room.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri("/api/v1/companion/room")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let grant = Grant {
            device_id: "phone".into(),
            expires_at: now() / 1000 + 3600,
            revoked: c.device_revocation("phone").unwrap(),
        };
        let s = SpatialStore::load(d.path(), &"a".repeat(64), "test", c, room).unwrap();
        (d, s, grant)
    }
    fn payload(s: &Shared) -> String {
        json!({"schema":"ruview.companion.spatial-artifact.v1","installationId":"test","roomRevision":1,"roomBindingDigest":s.room.snapshot().unwrap().binding_digest,"coordinateFrameId":"frame","sourceScanId":"scan","capturedAtUnixMs":1,"exportedAtUnixMs":now(),"provenance":"historical_geometry_only","calibrationDigest":null,"surfaces":[{"id":"wall-1","kind":"wall","dimensionsM":[2.5,2.4,0.1],"transform":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]}],"nodes":[]}).to_string()
    }
    async fn allow(s: &Shared, g: &Grant) -> String {
        let _ = action(
            State(s.clone()),
            Json(Action::Receive {
                device_id: g.device_id.clone(),
            }),
        )
        .await
        .unwrap();
        ticket(State(s.clone()), Extension(g.clone()))
            .await
            .unwrap()
            .0["nonce"]
            .as_str()
            .unwrap()
            .into()
    }
    #[tokio::test]
    async fn explicit_review_retains_across_restart_and_rejects_other_owner() {
        let (d, s, g) = setup().await;
        let nonce = allow(&s, &g).await;
        let p = payload(&s);
        let digest = hash(&p);
        let _ = upload(
            State(s.clone()),
            Extension(g.clone()),
            Json(Upload {
                nonce: nonce.clone(),
                digest: digest.clone(),
                payload: p.clone(),
            }),
        )
        .await
        .unwrap();
        assert!(!s.path.exists());
        assert_eq!(
            snapshot(State(s.clone())).await.unwrap().0["pending"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(upload(
            State(s.clone()),
            Extension(g),
            Json(Upload {
                nonce,
                digest: digest.clone(),
                payload: p
            })
        )
        .await
        .is_err());
        let _ = action(
            State(s.clone()),
            Json(Action::Accept {
                digest: digest.clone(),
            }),
        )
        .await
        .unwrap();
        let restored = SpatialStore::load(
            d.path(),
            &"a".repeat(64),
            "test",
            s.service.clone(),
            s.room.clone(),
        )
        .unwrap();
        assert_eq!(
            snapshot(State(restored)).await.unwrap().0["accepted"][0]["digest"],
            digest
        );
        assert!(SpatialStore::load(
            d.path(),
            &"b".repeat(64),
            "test",
            s.service.clone(),
            s.room.clone()
        )
        .is_err());
        assert!(action(State(s), Json(Action::Accept { digest }))
            .await
            .is_err());
    }
    #[tokio::test]
    async fn rejects_expiry_wrong_device_nonce_binding_digest_and_authority() {
        let (_d, s, g) = setup().await;
        let nonce = allow(&s, &g).await;
        let p = payload(&s);
        for mode in [
            "wrong-device",
            "nonce",
            "binding",
            "integrity",
            "authority",
            "reflection",
            "unknown",
            "expired",
            "missing-null",
        ] {
            let mut g = g.clone();
            let mut v: serde_json::Value = serde_json::from_str(&p).unwrap();
            let mut n = nonce.clone();
            match mode {
                "wrong-device" => g.device_id = "other".into(),
                "nonce" => n = "wrong".into(),
                "binding" => v["roomRevision"] = json!(2),
                "authority" => v["provenance"] = json!("current_alignment"),
                "reflection" => v["surfaces"][0]["transform"][0] = json!(-1),
                "unknown" => v["cameraImage"] = json!("raw"),
                "expired" => g.expires_at = 0,
                "missing-null" => {
                    v.as_object_mut().unwrap().remove("calibrationDigest");
                }
                _ => {}
            }
            let p = v.to_string();
            let digest = if mode == "integrity" {
                "0".repeat(64)
            } else {
                hash(&p)
            };
            assert!(
                upload(
                    State(s.clone()),
                    Extension(g),
                    Json(Upload {
                        nonce: n,
                        digest,
                        payload: p
                    })
                )
                .await
                .is_err(),
                "{mode}"
            );
        }
        s.inner
            .lock()
            .unwrap()
            .receipts
            .get_mut("phone")
            .unwrap()
            .until = 0;
        assert!(ticket(State(s), Extension(g)).await.is_err());
    }
    #[tokio::test]
    async fn revocation_during_review_removes_pending_authority() {
        let (_d, s, g) = setup().await;
        let nonce = allow(&s, &g).await;
        let p = payload(&s);
        let digest = hash(&p);
        let _ = upload(
            State(s.clone()),
            Extension(g.clone()),
            Json(Upload {
                nonce,
                digest: digest.clone(),
                payload: p,
            }),
        )
        .await
        .unwrap();
        let response = wifi_densepose_sensing_server::companion::owner_router(s.service.clone())
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
        assert!(action(State(s.clone()), Json(Action::Accept { digest }))
            .await
            .is_err());
        assert_eq!(snapshot(State(s)).await.unwrap().0["pending"], json!([]));
    }
    #[tokio::test]
    async fn uncertain_disk_failure_never_claims_acceptance_or_allows_more_writes() {
        let (_d, s, g) = setup().await;
        let nonce = allow(&s, &g).await;
        let p = payload(&s);
        let digest = hash(&p);
        let _ = upload(
            State(s.clone()),
            Extension(g),
            Json(Upload {
                nonce,
                digest: digest.clone(),
                payload: p,
            }),
        )
        .await
        .unwrap();
        fs::create_dir(&s.path).unwrap();
        assert!(action(State(s.clone()), Json(Action::Accept { digest }))
            .await
            .is_err());
        assert_eq!(
            snapshot(State(s.clone())).await.unwrap_err(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(s.inner.lock().unwrap().saved.accepted.is_empty());
    }
    #[tokio::test]
    async fn unknown_actions_are_rejected_by_real_router() {
        let (_d, s, _g) = setup().await;
        for body in [
            json!({"action":"receive","deviceId":"unknown"}).to_string(),
            json!({"action":"accept","digest":"a".repeat(64),"promote":true}).to_string(),
        ] {
            let response = owner_router(s.clone())
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/api/v1/companion/spatial")
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(!response.status().is_success());
        }
    }
}

#[cfg(test)]
mod hostile_storage_tests {
    use super::*;
    #[test]
    fn rejects_non_private_and_symlinked_history_before_reading() {
        let d = tempfile::tempdir().unwrap();
        let c = Companion::load(
            d.path(),
            "test".into(),
            "a".repeat(64),
            "https://test.local:3443".into(),
        )
        .unwrap();
        let room = SharedRoomStore::load(d.path(), "test").unwrap();
        let path = d.path().join("accepted-spatial-artifacts.json");
        fs::write(&path, b"{}").unwrap();
        assert!(
            SpatialStore::load(d.path(), &"a".repeat(64), "test", c.clone(), room.clone()).is_err()
        );
        #[cfg(unix)]
        {
            fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink("missing", &path).unwrap();
            assert!(SpatialStore::load(d.path(), &"a".repeat(64), "test", c, room).is_err());
        }
    }
}
