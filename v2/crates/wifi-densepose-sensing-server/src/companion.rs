//! Owner-approved, QR-pinned companion pairing; durable secrets are hashed.
use axum::{
    extract::{DefaultBodyLimit, Path as RoutePath, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;
pub fn secret() -> String {
    let mut b = [0; 32];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn valid_secret(s: &str) -> bool {
    s.len() == 43 && URL_SAFE_NO_PAD.decode(s).is_ok_and(|b| b.len() == 32)
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub(crate) fn private_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::other("symlink state directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub(crate) fn private_read(path: &Path) -> io::Result<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > 65536 {
        return Err(io::Error::other("invalid private state file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::other("insecure private state permissions"));
        }
    }
    std::fs::read(path)
}
pub(crate) fn atomic_private_write(path: &Path, data: &[u8]) -> io::Result<()> {
    let temp = path.with_extension(format!("tmp-{}", secret()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&temp)?;
    f.write_all(data)?;
    f.sync_all()?;
    std::fs::rename(&temp, path)?;
    std::fs::File::open(path.parent().ok_or_else(|| io::Error::other("parent"))?)?.sync_all()?;
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
struct Device {
    device_id: String,
    device_name: String,
    pairing_hash: String,
    expires_at: u64,
}
#[derive(Serialize, Deserialize)]
struct Disk {
    version: u32,
    installation_id: String,
    devices: Vec<Device>,
}
struct Invite {
    hash: String,
    expires: u64,
}
struct Pending {
    device: Device,
    code: String,
    expires: u64,
}
struct Access {
    device_id: String,
    expires: u64,
}
struct Inner {
    devices: HashMap<String, Device>,
    invites: HashMap<String, Invite>,
    pending: HashMap<String, Pending>,
    access: HashMap<String, Access>,
    calibration: HashMap<String, u64>,
    revocations: HashMap<String, watch::Sender<bool>>,
}
pub struct Companion {
    pub installation_id: String,
    pub cert_sha256: String,
    pub origin: String,
    path: PathBuf,
    fault: AtomicBool,
    inner: Mutex<Inner>,
}
pub type Shared = Arc<Companion>;
pub type ApiResult = Result<Json<Value>, StatusCode>;
#[derive(Clone)]
pub struct Grant {
    pub device_id: String,
    pub expires_at: u64,
    pub revoked: watch::Receiver<bool>,
}
impl Companion {
    pub fn load(
        dir: &Path,
        installation_id: String,
        cert_sha256: String,
        origin: String,
    ) -> io::Result<Shared> {
        private_dir(dir)?;
        if !origin.starts_with("https://") {
            return Err(io::Error::other("https required"));
        }
        let path = dir.join("companion-grants.json");
        let devices = if path.exists() {
            let disk: Disk =
                serde_json::from_slice(&private_read(&path)?).map_err(io::Error::other)?;
            if disk.version != 1
                || disk.installation_id != installation_id
                || disk.devices.len() > 16
            {
                return Err(io::Error::other("invalid grants"));
            }
            let mut m = HashMap::new();
            for d in disk.devices {
                if !valid_id(&d.device_id)
                    || d.pairing_hash.len() != 64
                    || m.insert(d.device_id.clone(), d).is_some()
                {
                    return Err(io::Error::other("invalid device"));
                }
            }
            m
        } else {
            HashMap::new()
        };
        Ok(Arc::new(Self {
            installation_id,
            cert_sha256,
            origin,
            path,
            fault: AtomicBool::new(false),
            inner: Mutex::new(Inner {
                devices,
                invites: HashMap::new(),
                pending: HashMap::new(),
                access: HashMap::new(),
                calibration: HashMap::new(),
                revocations: HashMap::new(),
            }),
        }))
    }
    fn persist(&self, devices: &HashMap<String, Device>, inner: &Inner) -> Result<(), StatusCode> {
        let disk = Disk {
            version: 1,
            installation_id: self.installation_id.clone(),
            devices: devices.values().cloned().collect(),
        };
        atomic_private_write(
            &self.path,
            &serde_json::to_vec(&disk).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .map_err(|_| {
            self.fault.store(true, Ordering::SeqCst);
            for tx in inner.revocations.values() {
                let _ = tx.send(true);
            }
            StatusCode::SERVICE_UNAVAILABLE
        })
    }
    /// Metadata only; sharing never receives durable pairing credentials.
    pub fn paired_device(&self, id: &str) -> Option<(String, u64)> {
        if self.fault.load(Ordering::SeqCst) { return None; }
        let inner = self.inner.lock().ok()?;
        let device = inner.devices.get(id)?;
        (device.expires_at > now()).then(|| (device.device_name.clone(), device.expires_at))
    }
    pub fn device_revocation(&self, id: &str) -> Option<watch::Receiver<bool>> {
        if self.fault.load(Ordering::SeqCst) { return None; }
        let mut inner = self.inner.lock().ok()?;
        if !inner.devices.get(id).is_some_and(|d| d.expires_at > now()) { return None; }
        Some(inner.revocations.entry(id.into()).or_insert_with(|| watch::channel(false).0).subscribe())
    }
    pub fn authorize(
        &self,
        installation: &str,
        token: &str,
        calibration: bool,
    ) -> Result<Grant, StatusCode> {
        if self.fault.load(Ordering::SeqCst) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        if installation != self.installation_id || !valid_secret(token) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let mut i = self
            .inner
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let t = now();
        let a = i.access.get(&hash(token)).ok_or(StatusCode::UNAUTHORIZED)?;
        let d = i
            .devices
            .get(&a.device_id)
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if a.expires <= t || d.expires_at <= t {
            return Err(StatusCode::UNAUTHORIZED);
        }
        if calibration
            && i.calibration
                .get(&d.device_id)
                .copied()
                .filter(|until| *until > now())
                .unwrap_or(0)
                <= t
        {
            return Err(StatusCode::FORBIDDEN);
        }
        let device_id = d.device_id.clone();
        let expires_at = a.expires.min(d.expires_at);
        let revoked = i
            .revocations
            .entry(device_id.clone())
            .or_insert_with(|| watch::channel(false).0)
            .subscribe();
        Ok(Grant {
            device_id,
            expires_at,
            revoked,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enroll {
    installation_id: String,
    invite_id: String,
    invite_secret: String,
    device_id: String,
    device_name: String,
    pairing_secret: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    installation_id: String,
    device_id: String,
    pairing_secret: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approve {
    comparison_code: String,
}
pub fn public_router(state: Shared) -> Router {
    Router::new()
        .route("/companion/v1/enroll", post(enroll))
        .route("/companion/v1/session", post(session))
        .layer(DefaultBodyLimit::max(4096))
        .with_state(state)
}
/// Caller must wrap these endpoints in loopback-only owner authentication.
pub fn owner_router(state: Shared) -> Router {
    Router::new()
        .route("/api/v1/companion", get(snapshot))
        .route("/api/v1/companion/invites", post(invite))
        .route("/api/v1/companion/requests/:id/approve", post(approve))
        .route("/api/v1/companion/devices/:id", delete(revoke))
        .route(
            "/api/v1/companion/devices/:id/calibration",
            post(calibration),
        )
        .layer(DefaultBodyLimit::max(4096))
        .with_state(state)
}
async fn invite(State(s): State<Shared>) -> ApiResult {
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    i.invites.retain(|_, v| v.expires > now());
    if i.invites.len() >= 4 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let id = secret();
    let token = secret();
    let expires = now() + 300;
    i.invites.insert(
        id.clone(),
        Invite {
            hash: hash(&token),
            expires,
        },
    );
    Ok(Json(
        json!({"schema":"ruview.companion.invite.v1","installation_id":s.installation_id,"cert_sha256":s.cert_sha256,"origin":s.origin,"invite_id":id,"invite_secret":token,"expires_at":expires}),
    ))
}
async fn enroll(State(s): State<Shared>, Json(r): Json<Enroll>) -> ApiResult {
    if r.installation_id != s.installation_id
        || !valid_id(&r.device_id)
        || !valid_secret(&r.pairing_secret)
        || r.device_name.trim().is_empty()
        || r.device_name.len() > 80
        || r.device_name.chars().any(char::is_control)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let t = now();
    i.pending.retain(|_, p| p.expires > t);
    if i.pending.len() >= 8 || i.devices.len() >= 16 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if i.devices.contains_key(&r.device_id)
        || i.pending
            .values()
            .any(|p| p.device.device_id == r.device_id)
    {
        return Err(StatusCode::CONFLICT);
    }
    let inv = i
        .invites
        .get(&r.invite_id)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if inv.expires <= t || inv.hash != hash(&r.invite_secret) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    i.invites.remove(&r.invite_id);
    let request_id = secret();
    let code = format!("{:06}", OsRng.next_u32() % 1_000_000);
    i.pending.insert(
        request_id.clone(),
        Pending {
            device: Device {
                device_id: r.device_id,
                device_name: r.device_name,
                pairing_hash: hash(&r.pairing_secret),
                expires_at: t + 30 * 86400,
            },
            code: code.clone(),
            expires: t + 300,
        },
    );
    Ok(Json(
        json!({"request_id":request_id,"comparison_code":code,"status":"pending"}),
    ))
}
async fn approve(
    State(s): State<Shared>,
    RoutePath(id): RoutePath<String>,
    Json(r): Json<Approve>,
) -> ApiResult {
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let p = i.pending.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if p.expires <= now() || p.code != r.comparison_code {
        return Err(StatusCode::FORBIDDEN);
    }
    // Pending requests can queue while a slot is available. Recheck under the
    // approval lock so another approval cannot exceed the persisted load limit.
    if i.devices.len() >= 16 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let mut devices = i.devices.clone();
    devices.insert(p.device.device_id.clone(), p.device.clone());
    s.persist(&devices, &i)?;
    i.devices = devices;
    i.pending.remove(&id);
    Ok(Json(json!({"status":"approved"})))
}
async fn session(State(s): State<Shared>, Json(r): Json<Session>) -> ApiResult {
    if s.fault.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    if r.installation_id != s.installation_id || !valid_secret(&r.pairing_secret) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let t = now();
    let h = hash(&r.pairing_secret);
    if i.pending
        .values()
        .any(|p| p.device.device_id == r.device_id && p.device.pairing_hash == h && p.expires > t)
    {
        return Ok(Json(json!({"status":"pending"})));
    }
    let d = i
        .devices
        .get(&r.device_id)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if d.pairing_hash != h || d.expires_at <= t {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let expires_at = (t + 12 * 3600).min(d.expires_at);
    i.access
        .retain(|_, a| a.expires > t && a.device_id != r.device_id);
    let token = secret();
    i.access.insert(
        hash(&token),
        Access {
            device_id: r.device_id.clone(),
            expires: expires_at,
        },
    );
    Ok(Json(
        json!({"status":"approved","access_token":token,"expires_at":expires_at,"installation_id":s.installation_id,"device_id":r.device_id,"calibration_until":i.calibration.get(&r.device_id).copied().filter(|until| *until > now()).unwrap_or(0)}),
    ))
}
async fn snapshot(State(s): State<Shared>) -> ApiResult {
    let i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let requests:Vec<_>=i.pending.iter().filter(|(_,p)|p.expires>now()).map(|(id,p)|json!({"request_id":id,"device_id":p.device.device_id,"device_name":p.device.device_name,"comparison_code":p.code,"expires_at":p.expires})).collect();
    let devices:Vec<_>=i.devices.values().map(|d|json!({"device_id":d.device_id,"device_name":d.device_name,"expires_at":d.expires_at,"calibration_until":i.calibration.get(&d.device_id).copied().filter(|until| *until > now()).unwrap_or(0)})).collect();
    Ok(Json(
        json!({"installation_id":s.installation_id,"origin":s.origin,"cert_sha256":s.cert_sha256,"requests":requests,"devices":devices}),
    ))
}
async fn revoke(State(s): State<Shared>, RoutePath(id): RoutePath<String>) -> ApiResult {
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let mut devices = i.devices.clone();
    if devices.remove(&id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    s.persist(&devices, &i)?;
    i.devices = devices;
    i.access.retain(|_, a| a.device_id != id);
    i.calibration.remove(&id);
    if let Some(tx) = i.revocations.remove(&id) {
        let _ = tx.send(true);
    }
    Ok(Json(json!({"status":"revoked"})))
}
async fn calibration(State(s): State<Shared>, RoutePath(id): RoutePath<String>) -> ApiResult {
    let mut i = s
        .inner
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if !i.devices.get(&id).is_some_and(|d| d.expires_at > now()) {
        return Err(StatusCode::NOT_FOUND);
    }
    // An owner grant is an explicit controller handoff, never a second controller.
    let until = now() + 900;
    i.calibration.clear();
    i.calibration.insert(id, until);
    Ok(Json(json!({"calibration_until":until})))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Shared) {
        let d = tempfile::tempdir().unwrap();
        let s = Companion::load(
            d.path(),
            "installation".into(),
            "a".repeat(64),
            "https://ruview.local:3443".into(),
        )
        .unwrap();
        (d, s)
    }
    async fn pending(s: &Shared) -> (String, String, String) {
        pending_device(s, "phone").await
    }
    async fn pending_device(s: &Shared, device_id: &str) -> (String, String, String) {
        let v = invite(State(s.clone())).await.unwrap().0;
        let proof = secret();
        let v = enroll(
            State(s.clone()),
            Json(Enroll {
                installation_id: s.installation_id.clone(),
                invite_id: v["invite_id"].as_str().unwrap().into(),
                invite_secret: v["invite_secret"].as_str().unwrap().into(),
                device_id: device_id.into(),
                device_name: "Phone".into(),
                pairing_secret: proof.clone(),
            }),
        )
        .await
        .unwrap()
        .0;
        (
            v["request_id"].as_str().unwrap().into(),
            v["comparison_code"].as_str().unwrap().into(),
            proof,
        )
    }
    async fn access(s: &Shared, proof: &str) -> ApiResult {
        session(
            State(s.clone()),
            Json(Session {
                installation_id: s.installation_id.clone(),
                device_id: "phone".into(),
                pairing_secret: proof.into(),
            }),
        )
        .await
    }
    #[tokio::test]
    async fn queued_approvals_cannot_exceed_durable_device_capacity() {
        let (dir, s) = setup();
        for index in 0..15 {
            let (id, code, _) = pending_device(&s, &format!("phone-{index}")).await;
            let _ = approve(
                State(s.clone()),
                RoutePath(id),
                Json(Approve {
                    comparison_code: code,
                }),
            )
            .await
            .unwrap();
        }
        let (first_id, first_code, _) = pending_device(&s, "last-slot").await;
        let (waiting_id, waiting_code, _) = pending_device(&s, "waiting").await;
        let _ = approve(
            State(s.clone()),
            RoutePath(first_id),
            Json(Approve {
                comparison_code: first_code,
            }),
        )
        .await
        .unwrap();
        let persisted = std::fs::read(&s.path).unwrap();
        assert_eq!(
            approve(
                State(s.clone()),
                RoutePath(waiting_id.clone()),
                Json(Approve {
                    comparison_code: waiting_code.clone(),
                })
            )
            .await
            .unwrap_err(),
            StatusCode::TOO_MANY_REQUESTS,
        );
        assert_eq!(std::fs::read(&s.path).unwrap(), persisted);
        {
            let inner = s.inner.lock().unwrap();
            assert_eq!(inner.devices.len(), 16);
            assert!(inner.pending.contains_key(&waiting_id));
        }
        let reloaded = Companion::load(
            dir.path(),
            s.installation_id.clone(),
            s.cert_sha256.clone(),
            s.origin.clone(),
        )
        .unwrap();
        assert_eq!(reloaded.inner.lock().unwrap().devices.len(), 16);
        let _ = revoke(State(s.clone()), RoutePath("last-slot".into()))
            .await
            .unwrap();
        let _ = approve(
            State(s.clone()),
            RoutePath(waiting_id.clone()),
            Json(Approve {
                comparison_code: waiting_code,
            }),
        )
        .await
        .unwrap();
        assert!(!s.inner.lock().unwrap().pending.contains_key(&waiting_id));
        let reloaded = Companion::load(
            dir.path(),
            s.installation_id.clone(),
            s.cert_sha256.clone(),
            s.origin.clone(),
        )
        .unwrap();
        let inner = reloaded.inner.lock().unwrap();
        assert_eq!(inner.devices.len(), 16);
        assert!(inner.devices.contains_key("waiting"));
    }

    #[tokio::test]
    async fn approval_read_scope_calibration_revocation() {
        let (_d, s) = setup();
        let (id, code, proof) = pending(&s).await;
        assert_eq!(access(&s, &proof).await.unwrap().0["status"], "pending");
        assert!(approve(
            State(s.clone()),
            RoutePath(id.clone()),
            Json(Approve {
                comparison_code: "wrong".into()
            })
        )
        .await
        .is_err());
        approve(
            State(s.clone()),
            RoutePath(id),
            Json(Approve {
                comparison_code: code,
            }),
        )
        .await
        .unwrap();
        let v = access(&s, &proof).await.unwrap().0;
        let token = v["access_token"].as_str().unwrap();
        let grant = s.authorize("installation", token, false).unwrap();
        assert!(s.authorize("other", token, false).is_err());
        assert!(s.authorize("installation", token, true).is_err());
        calibration(State(s.clone()), RoutePath("phone".into()))
            .await
            .unwrap();
        assert!(s.authorize("installation", token, true).is_ok());
        revoke(State(s.clone()), RoutePath("phone".into()))
            .await
            .unwrap();
        assert!(*grant.revoked.borrow());
        assert!(s.authorize("installation", token, false).is_err());
        assert!(access(&s, &proof).await.is_err());
    }
    #[tokio::test]
    async fn durable_pair_not_access_and_no_plaintext() {
        let (d, s) = setup();
        let (id, code, proof) = pending(&s).await;
        approve(
            State(s.clone()),
            RoutePath(id),
            Json(Approve {
                comparison_code: code,
            }),
        )
        .await
        .unwrap();
        let v = access(&s, &proof).await.unwrap().0;
        let token = v["access_token"].as_str().unwrap();
        let disk = std::fs::read_to_string(&s.path).unwrap();
        assert!(!disk.contains(&proof));
        assert!(!disk.contains(token));
        let loaded = Companion::load(
            d.path(),
            "installation".into(),
            "a".repeat(64),
            s.origin.clone(),
        )
        .unwrap();
        assert!(loaded.authorize("installation", token, false).is_err());
        assert_eq!(
            access(&loaded, &proof).await.unwrap().0["status"],
            "approved"
        );
        assert!(
            Companion::load(d.path(), "wrong".into(), "a".repeat(64), s.origin.clone()).is_err()
        );
    }
    #[tokio::test]
    async fn expires_and_cannot_replay_invite() {
        let (_d, s) = setup();
        let v = invite(State(s.clone())).await.unwrap().0;
        let make = || Enroll {
            installation_id: s.installation_id.clone(),
            invite_id: v["invite_id"].as_str().unwrap().into(),
            invite_secret: v["invite_secret"].as_str().unwrap().into(),
            device_id: secret(),
            device_name: "Phone".into(),
            pairing_secret: secret(),
        };
        enroll(State(s.clone()), Json(make())).await.unwrap();
        assert!(enroll(State(s.clone()), Json(make())).await.is_err());
        let (id, code, proof) = pending(&s).await;
        s.inner
            .lock()
            .unwrap()
            .pending
            .get_mut(&id)
            .unwrap()
            .expires = 0;
        assert!(approve(
            State(s.clone()),
            RoutePath(id),
            Json(Approve {
                comparison_code: code
            })
        )
        .await
        .is_err());
        assert!(access(&s, &proof).await.is_err());
    }
    #[tokio::test]
    async fn persistence_failure_never_approves() {
        let (_d, s) = setup();
        let (id, code, proof) = pending(&s).await;
        std::fs::create_dir(&s.path).unwrap();
        assert_eq!(
            approve(
                State(s.clone()),
                RoutePath(id),
                Json(Approve {
                    comparison_code: code
                })
            )
            .await
            .unwrap_err(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(s.inner.lock().unwrap().devices.is_empty());
        assert!(access(&s, &proof).await.is_err());
    }
    #[tokio::test]
    async fn renewal_invalidates_previous_and_pair_expiry() {
        let (_d, s) = setup();
        let (id, code, proof) = pending(&s).await;
        approve(
            State(s.clone()),
            RoutePath(id),
            Json(Approve {
                comparison_code: code,
            }),
        )
        .await
        .unwrap();
        let first = access(&s, &proof).await.unwrap().0;
        let second = access(&s, &proof).await.unwrap().0;
        assert!(s
            .authorize(
                "installation",
                first["access_token"].as_str().unwrap(),
                false
            )
            .is_err());
        assert!(s
            .authorize(
                "installation",
                second["access_token"].as_str().unwrap(),
                false
            )
            .is_ok());
        s.inner
            .lock()
            .unwrap()
            .devices
            .get_mut("phone")
            .unwrap()
            .expires_at = 0;
        assert!(access(&s, &proof).await.is_err());
        assert!(s
            .authorize(
                "installation",
                second["access_token"].as_str().unwrap(),
                false
            )
            .is_err());
    }
    #[tokio::test]
    async fn router_rejects_unknown_fields_and_oversized_body() {
        use tower::ServiceExt;
        let (_d, s) = setup();
        let app = public_router(s);
        let req=axum::http::Request::builder().method("POST").uri("/companion/v1/session").header("content-type","application/json").body(axum::body::Body::from(serde_json::to_vec(&json!({"installation_id":"installation","device_id":"phone","pairing_secret":secret(),"admin":true})).unwrap())).unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/companion/v1/session")
            .header("content-type", "application/json")
            .body(axum::body::Body::from("x".repeat(5000)))
            .unwrap();
        assert_eq!(
            app.oneshot(req).await.unwrap().status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }
    #[tokio::test]
    async fn invite_bound_and_duplicate_device_cannot_replace_proof() {
        let (_d, s) = setup();
        let (id, code, proof) = pending(&s).await;
        approve(
            State(s.clone()),
            RoutePath(id),
            Json(Approve {
                comparison_code: code,
            }),
        )
        .await
        .unwrap();
        let v = invite(State(s.clone())).await.unwrap().0;
        let attempted = enroll(
            State(s.clone()),
            Json(Enroll {
                installation_id: s.installation_id.clone(),
                invite_id: v["invite_id"].as_str().unwrap().into(),
                invite_secret: v["invite_secret"].as_str().unwrap().into(),
                device_id: "phone".into(),
                device_name: "attacker".into(),
                pairing_secret: secret(),
            }),
        )
        .await;
        assert_eq!(attempted.unwrap_err(), StatusCode::CONFLICT);
        assert_eq!(access(&s, &proof).await.unwrap().0["status"], "approved");
        for _ in 0..3 {
            invite(State(s.clone())).await.unwrap();
        }
        assert_eq!(
            invite(State(s)).await.unwrap_err(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    #[tokio::test]
    async fn calibration_handoff_is_exclusive_and_snapshot_expires() {
        let (_dir, s) = setup();
        let (id, code, proof) = pending(&s).await;
        approve(
            State(s.clone()),
            RoutePath(id),
            Json(Approve {
                comparison_code: code,
            }),
        )
        .await
        .unwrap();
        let first = access(&s, &proof).await.unwrap().0;
        let first_token = first["access_token"].as_str().unwrap();
        // A second approved device has its own credential and access token.
        let second_proof = secret();
        {
            let mut i = s.inner.lock().unwrap();
            i.devices.insert(
                "phone-two".into(),
                Device {
                    device_id: "phone-two".into(),
                    device_name: "Second phone".into(),
                    pairing_hash: hash(&second_proof),
                    expires_at: now() + 86400,
                },
            );
        }
        let second = session(
            State(s.clone()),
            Json(Session {
                installation_id: s.installation_id.clone(),
                device_id: "phone-two".into(),
                pairing_secret: second_proof,
            }),
        )
        .await
        .unwrap()
        .0;
        let second_token = second["access_token"].as_str().unwrap();
        calibration(State(s.clone()), RoutePath("phone".into()))
            .await
            .unwrap();
        assert!(s.authorize("installation", first_token, true).is_ok());
        calibration(State(s.clone()), RoutePath("phone-two".into()))
            .await
            .unwrap();
        assert!(matches!(
            s.authorize("installation", first_token, true),
            Err(StatusCode::FORBIDDEN)
        ));
        assert!(s.authorize("installation", second_token, true).is_ok());
        assert!(s.authorize("installation", first_token, false).is_ok());
        let view = snapshot(State(s.clone())).await.unwrap().0;
        let devices = view["devices"].as_array().unwrap();
        assert_eq!(
            devices.iter().find(|d| d["device_id"] == "phone").unwrap()["calibration_until"],
            0
        );
        assert!(
            devices
                .iter()
                .find(|d| d["device_id"] == "phone-two")
                .unwrap()["calibration_until"]
                .as_u64()
                .unwrap()
                > now()
        );
        s.inner
            .lock()
            .unwrap()
            .calibration
            .insert("phone-two".into(), now() - 1);
        assert!(matches!(
            s.authorize("installation", second_token, true),
            Err(StatusCode::FORBIDDEN)
        ));
        let view = snapshot(State(s)).await.unwrap().0;
        assert!(view["devices"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["calibration_until"] == 0));
    }
}
