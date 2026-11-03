//! Installation-owned room publication. Authentication is supplied by the caller.
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_SAFE: u64 = 9_007_199_254_740_991;
const SLOTS: &[&str] = &[
    "north_west",
    "north_center",
    "north_east",
    "west_center",
    "east_center",
    "south_west",
    "south_center",
    "south_east",
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub installation_id: String,
    pub revision: u64,
    pub room: Option<Value>,
    pub binding_digest: Option<String>,
}
pub struct SharedRoomStore {
    path: PathBuf,
    current: Mutex<Envelope>,
}
fn invalid() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid shared room state")
}
fn keys(v: &Value, allowed: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|o| o.keys().all(|k| allowed.contains(&k.as_str())))
}
fn text(v: &Value, min: usize, max: usize) -> bool {
    v.as_str().is_some_and(|s| {
        (min..=max).contains(&s.encode_utf16().count())
            && s.chars().all(|c| c >= ' ' && c != '\u{7f}')
    })
}
fn number(v: &Value, min: f64, max: f64, scale: f64) -> bool {
    v.as_f64().is_some_and(|n| {
        n.is_finite() && n >= min && n <= max && (n * scale - (n * scale).round()).abs() < 1e-8
    })
}
fn position(v: &Value, width: f64, length: f64, slots: &HashSet<String>) -> bool {
    v["slot"]
        .as_str()
        .is_some_and(|s| SLOTS.contains(&s) && !slots.contains(s))
        && number(&v["xM"], 0., width, 1000.)
        && number(&v["yM"], 0., length, 1000.)
        && v["confirmedAtUnixMs"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= MAX_SAFE)
}
fn validate(mut r: Value) -> Result<Value, std::io::Error> {
    if !keys(
        &r,
        &[
            "schema",
            "id",
            "name",
            "widthM",
            "lengthM",
            "heightM",
            "nodes",
            "accessPoint",
        ],
    ) || r["schema"] != "ruview.companion.room.v1"
        || !text(&r["id"], 4, 96)
        || !text(&r["name"], 1, 48)
        || !number(&r["widthM"], 1.5, 30., 1000.)
        || !number(&r["lengthM"], 1.5, 30., 1000.)
        || !number(&r["heightM"], 1.8, 6., 1000.)
        || r.get("accessPoint").is_none()
    {
        return Err(invalid());
    }
    let name = r["name"].as_str().unwrap();
    if name.split_whitespace().collect::<Vec<_>>().join(" ") != name {
        return Err(invalid());
    }
    let width = r["widthM"].as_f64().unwrap();
    let length = r["lengthM"].as_f64().unwrap();
    let nodes = r["nodes"].as_array().ok_or_else(invalid)?;
    if nodes.is_empty() || nodes.len() > 16 {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    let mut slots = HashSet::new();
    let mut base = false;
    let mut receiver = false;
    for n in nodes {
        if !keys(
            n,
            &[
                "nodeId",
                "slot",
                "xM",
                "yM",
                "role",
                "headingDeg",
                "orientationConfirmed",
                "confirmedAtUnixMs",
            ],
        ) || !number(&n["nodeId"], 0., 255., 1.)
            || !position(n, width, length, &slots)
            || n["orientationConfirmed"] != true
            || !number(&n["headingDeg"], 0., 360., 10.)
        {
            return Err(invalid());
        }
        let id = n["nodeId"].as_f64().unwrap() as u8;
        if !ids.insert(id) {
            return Err(invalid());
        }
        slots.insert(n["slot"].as_str().unwrap().to_string());
        match n["role"].as_str() {
            Some("receiver") => receiver = true,
            Some("base_station") => base = true,
            _ => return Err(invalid()),
        }
    }
    let ap = &r["accessPoint"];
    if !ap.is_null()
        && (!keys(ap, &["slot", "xM", "yM", "provenance", "confirmedAtUnixMs"])
            || !position(ap, width, length, &slots)
            || ap["provenance"] != "operator_entered")
    {
        return Err(invalid());
    }
    if (receiver && ap.is_null() && !base) || (base && !receiver) {
        return Err(invalid());
    }
    r["nodes"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|n| n["nodeId"].as_f64().unwrap() as u8);
    Ok(r)
}
fn scaled(v: &Value, scale: f64) -> i64 {
    (v.as_f64().unwrap() * scale).round() as i64
}
fn digest(room: &Value, installation: &str, revision: u64) -> String {
    let nodes: Vec<Value> = room["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            json!([
                scaled(&n["nodeId"], 1.),
                n["slot"],
                scaled(&n["xM"], 1000.),
                scaled(&n["yM"], 1000.),
                n["role"],
                scaled(&n["headingDeg"], 10.)
            ])
        })
        .collect();
    let ap = &room["accessPoint"];
    let ap = if ap.is_null() {
        Value::Null
    } else {
        json!([
            ap["slot"],
            scaled(&ap["xM"], 1000.),
            scaled(&ap["yM"], 1000.),
            "operator_entered"
        ])
    };
    let canonical = json!([
        "ruview.room-calibration-binding.v2",
        installation,
        revision,
        room["id"],
        [
            scaled(&room["widthM"], 1000.),
            scaled(&room["lengthM"], 1000.),
            scaled(&room["heightM"], 1000.)
        ],
        nodes,
        ap
    ]);
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).unwrap())
    )
}
impl SharedRoomStore {
    pub fn load(dir: &Path, installation_id: &str) -> std::io::Result<Arc<Self>> {
        if !text(&json!(installation_id), 1, 128) {
            return Err(invalid());
        }
        fs::create_dir_all(dir)?;
        let path = dir.join("companion-room.json");
        let current = match fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.file_type().is_file() || meta.len() > 65536 {
                    return Err(invalid());
                }
                #[cfg(unix)]
                if meta.permissions().mode() & 0o077 != 0 {
                    return Err(invalid());
                }
                let e: Envelope =
                    serde_json::from_slice(&fs::read(&path)?).map_err(|_| invalid())?;
                if e.installation_id != installation_id || e.revision == 0 || e.revision > MAX_SAFE
                {
                    return Err(invalid());
                }
                let room = validate(e.room.clone().ok_or_else(invalid)?)?;
                if e.binding_digest.as_deref()
                    != Some(digest(&room, installation_id, e.revision).as_str())
                {
                    return Err(invalid());
                }
                Envelope {
                    room: Some(room),
                    ..e
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Envelope {
                installation_id: installation_id.into(),
                revision: 0,
                room: None,
                binding_digest: None,
            },
            Err(e) => return Err(e),
        };
        Ok(Arc::new(Self {
            path,
            current: Mutex::new(current),
        }))
    }
    pub fn snapshot(&self) -> Option<Envelope> { self.current.lock().ok().map(|v| v.clone()) }
    pub fn matches_binding(&self, binding: &str, node_ids: &[u8]) -> bool {
        let Ok(e) = self.current.lock() else {
            return false;
        };
        let Some(room) = &e.room else { return false };
        let ids: HashSet<u8> = node_ids.iter().copied().collect();
        let expected: HashSet<u8> = room["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| scaled(&n["nodeId"], 1.) as u8)
            .collect();
        e.binding_digest.as_deref() == Some(binding)
            && ids.len() == node_ids.len()
            && ids == expected
    }
    fn publish(&self, expected: u64, room: Value) -> Result<Envelope, StatusCode> {
        let room = validate(room).map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut current = self
            .current
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if current.revision != expected {
            return Err(StatusCode::CONFLICT);
        }
        if expected >= MAX_SAFE {
            return Err(StatusCode::CONFLICT);
        }
        let next = Envelope {
            installation_id: current.installation_id.clone(),
            revision: expected + 1,
            binding_digest: Some(digest(&room, &current.installation_id, expected + 1)),
            room: Some(room),
        };
        let persist = || -> std::io::Result<()> {
            let tmp = self
                .path
                .with_extension(format!("{}.tmp", rand::random::<u64>()));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(&tmp)?;
            file.write_all(&serde_json::to_vec(&next)?)?;
            file.sync_all()?;
            fs::rename(&tmp, &self.path)?;
            fs::File::open(self.path.parent().unwrap())?.sync_all()?;
            Ok(())
        };
        persist().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        *current = next.clone();
        Ok(next)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publication {
    expected_revision: u64,
    room: Value,
}
async fn read(State(s): State<Arc<SharedRoomStore>>) -> Result<Json<Envelope>, StatusCode> {
    Ok(Json(
        s.current
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .clone(),
    ))
}
async fn publish(
    State(s): State<Arc<SharedRoomStore>>,
    Json(p): Json<Publication>,
) -> Result<Json<Envelope>, StatusCode> {
    s.publish(p.expected_revision, p.room).map(Json)
}
pub fn owner_router(store: Arc<SharedRoomStore>) -> Router {
    Router::new()
        .route("/api/v1/companion/room", get(read).put(publish))
        .with_state(store)
}
pub fn read_router(store: Arc<SharedRoomStore>) -> Router {
    Router::new()
        .route("/companion/v1/room", get(read))
        .with_state(store)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn room() -> Value {
        json!({"schema":"ruview.companion.room.v1","id":"room-1","name":"Living room","widthM":2.5,"lengthM":2.5,"heightM":2.4,"nodes":[{"nodeId":17,"slot":"north_west","xM":0,"yM":0,"role":"receiver","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1}],"accessPoint":{"slot":"south_east","xM":2.5,"yM":2.5,"provenance":"operator_entered","confirmedAtUnixMs":1}})
    }
    #[test]
    fn cross_language_fixture() {
        let mut r = room();
        r["widthM"] = json!(5);
        r["lengthM"] = json!(6);
        r["heightM"] = json!(2.6);
        r["nodes"][0]["nodeId"] = json!(1);
        r["nodes"][0]["xM"] = json!(0.45);
        r["nodes"][0]["yM"] = json!(0.45);
        r["accessPoint"]["slot"] = json!("north_east");
        r["accessPoint"]["xM"] = json!(4.55);
        r["accessPoint"]["yM"] = json!(0.45);
        let r = validate(r).unwrap();
        assert_eq!(
            digest(&r, "installation-1", 1),
            "4b9276991f7d676f220ea053c3474e120f52698682c246cc92f2c66109a6be28"
        );
    }
    #[test]
    fn persistence_cas_and_binding_are_installation_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let s = SharedRoomStore::load(dir.path(), "install-1").unwrap();
        assert_eq!(s.current.lock().unwrap().revision, 0);
        let e = s.publish(0, room()).unwrap();
        assert!(s.matches_binding(e.binding_digest.as_ref().unwrap(), &[17]));
        assert!(!s.matches_binding(e.binding_digest.as_ref().unwrap(), &[17, 17]));
        assert!(!s.matches_binding(e.binding_digest.as_ref().unwrap(), &[18]));
        assert!(matches!(s.publish(0, room()), Err(StatusCode::CONFLICT)));
        let restored = SharedRoomStore::load(dir.path(), "install-1").unwrap();
        assert!(restored.matches_binding(e.binding_digest.as_ref().unwrap(), &[17]));
        assert!(SharedRoomStore::load(dir.path(), "other").is_err());
    }
    #[test]
    fn malformed_geometry_unknown_evidence_and_duplicate_slots_rejected() {
        for (key, value) in [
            ("widthM", json!(2.5001)),
            ("baseline", json!({})),
            ("name", json!(" bad")),
        ] {
            let mut r = room();
            r[key] = value;
            assert!(validate(r).is_err());
        }
        let mut r = room();
        r["accessPoint"]["slot"] = json!("north_west");
        assert!(validate(r).is_err());
        let mut r = room();
        r["nodes"][0]["orientationConfirmed"] = json!(false);
        assert!(validate(r).is_err());
    }
    #[test]
    fn disk_tampering_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let s = SharedRoomStore::load(dir.path(), "install-1").unwrap();
        s.publish(0, room()).unwrap();
        let mut e: Value = serde_json::from_slice(&fs::read(&s.path).unwrap()).unwrap();
        e["room"]["widthM"] = json!(3);
        fs::write(&s.path, serde_json::to_vec(&e).unwrap()).unwrap();
        assert!(SharedRoomStore::load(dir.path(), "install-1").is_err());
    }
}
