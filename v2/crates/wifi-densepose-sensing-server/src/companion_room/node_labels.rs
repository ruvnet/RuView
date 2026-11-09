//! Cosmetic metadata only; no room publication, model or learner authority.
use super::{Envelope, SharedRoomStore, MAX_SAFE};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(super) const MAX_BYTES: usize = 8 * 1024;
const SCHEMA: &str = "ruview.companion.room-node-labels.v1";
const UPDATE: &str = "ruview.companion.room-node-labels-update.v1";
const FILE: &str = "companion-room-node-labels-v1.json";
const STAGE: &str = ".companion-room-node-labels-v1.stage";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Publication {
    binding_digest: String,
    publication_revision: u64,
    node_ids: Vec<u8>,
}
impl Publication {
    fn from_room(room: &Envelope) -> Option<Self> {
        Some(Self {
            binding_digest: room.binding_digest.clone()?,
            publication_revision: room.revision,
            node_ids: room.room.as_ref()?["nodes"]
                .as_array()?
                .iter()
                .map(|n| n["nodeId"].as_f64().map(|id| id as u8))
                .collect::<Option<Vec<_>>>()?,
        })
    }
    fn valid(&self) -> bool {
        self.binding_digest.len() == 64
            && self
                .binding_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && (1..=MAX_SAFE).contains(&self.publication_revision)
            && !self.node_ids.is_empty()
            && self.node_ids.len() <= 16
            && self.node_ids.windows(2).all(|p| p[0] < p[1])
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Label {
    node_id: u8,
    display_name: String,
}
fn labels_valid(labels: &[Label], publication: &Publication) -> bool {
    labels.len() <= 16 && labels.windows(2).all(|p| p[0].node_id < p[1].node_id)
        && labels.iter().all(|label| {
            let name = &label.display_name;
            publication.node_ids.contains(&label.node_id)
                && (1..=40).contains(&name.encode_utf16().count())
                && name.chars().all(|c| c >= ' ' && c != '\u{7f}')
                // JS producer normalizes whitespace first; unnormalized wire is refused.
                && name.split(is_js_whitespace).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ") == *name
        })
}
fn is_js_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Update {
    schema: String,
    installation_id: String,
    expected_publication: Publication,
    expected_metadata_revision: u64,
    labels: Vec<Label>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    owner_scope: String,
    installation_id: String,
    publication: Publication,
    metadata_revision: u64,
    labels: Vec<Label>,
}
#[derive(Clone)]
enum Known {
    Absent,
    Record(Record),
    Unavailable,
}
/// A separate in-memory snapshot and writer lock: never hold room/RF locks over IO.
pub(super) struct Labels {
    dir: PathBuf,
    owner: Option<String>,
    installation: String,
    current: Mutex<Known>,
    writer: Mutex<()>,
    #[cfg(unix)]
    directory_identity: (u64, u64),
    directory_valid_at_load: bool,
}
fn private_regular(path: &Path, writable: bool) -> std::io::Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err(std::io::Error::other("invalid label record"));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 || metadata.nlink() != 1 {
        return Err(std::io::Error::other("unprotected label record"));
    }
    let mut options = OpenOptions::new();
    options.read(true).write(writable);
    #[cfg(target_os = "macos")]
    options.custom_flags(0x100); // O_NOFOLLOW
    #[cfg(target_os = "linux")]
    options.custom_flags(0x20000); // O_NOFOLLOW
    let file = options.open(path)?;
    #[cfg(unix)]
    if (file.metadata()?.dev(), file.metadata()?.ino()) != (metadata.dev(), metadata.ino()) {
        return Err(std::io::Error::other("label record changed"));
    }
    Ok(file)
}
impl Labels {
    pub(super) fn load(dir: &Path, installation: &str, owner: Option<&str>) -> Self {
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
        let identity = fs::symlink_metadata(&canonical).ok();
        let directory_valid_at_load =
            fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_dir());
        let labels = Self {
            dir: canonical,
            owner: owner.map(str::to_owned),
            installation: installation.into(),
            current: Mutex::new(Known::Unavailable),
            writer: Mutex::new(()),
            #[cfg(unix)]
            directory_identity: identity.map(|m| (m.dev(), m.ino())).unwrap_or((0, 0)),
            directory_valid_at_load,
        };
        if labels.owner.as_ref().is_some_and(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            let disk = labels.read_disk();
            if let Ok(mut state) = labels.current.lock() {
                *state = disk;
            }
        }
        labels
    }
    fn valid_directory(&self) -> bool {
        if !self.directory_valid_at_load {
            return false;
        }
        let Ok(meta) = fs::symlink_metadata(&self.dir) else {
            return false;
        };
        if !meta.file_type().is_dir() {
            return false;
        }
        #[cfg(unix)]
        if meta.permissions().mode() & 0o077 != 0
            || (meta.dev(), meta.ino()) != self.directory_identity
        {
            return false;
        }
        true
    }
    fn read_disk(&self) -> Known {
        if self.owner.is_none() || !self.valid_directory() {
            return Known::Unavailable;
        }
        // A recognized unfinished stage is never promoted or silently overwritten.
        match fs::symlink_metadata(self.dir.join(STAGE)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Known::Unavailable,
        }
        let path = self.dir.join(FILE);
        let mut file = match private_regular(&path, false) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Known::Absent,
            Err(_) => return Known::Unavailable,
        };
        let mut bytes = Vec::new();
        if Read::by_ref(&mut file)
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > MAX_BYTES
        {
            return Known::Unavailable;
        }
        let Ok(record) = serde_json::from_slice::<Record>(&bytes) else {
            return Known::Unavailable;
        };
        if record.schema != SCHEMA
            || Some(&record.owner_scope) != self.owner.as_ref()
            || record.installation_id != self.installation
            || !(1..=MAX_SAFE).contains(&record.metadata_revision)
            || !record.publication.valid()
            || !labels_valid(&record.labels, &record.publication)
        {
            return Known::Unavailable;
        }
        Known::Record(record)
    }
    fn snapshot_for(&self, publication: Option<Publication>) -> Value {
        let state = self.current.lock();
        let (status, revision, labels) = match (&publication, state.as_deref()) {
            (None, _) => ("room_required", Value::Null, Value::Null),
            (_, Err(_)) | (_, Ok(Known::Unavailable)) => ("unavailable", Value::Null, Value::Null),
            (Some(p), Ok(Known::Record(record))) if p == &record.publication => (
                "current",
                json!(record.metadata_revision),
                json!(record.labels),
            ),
            _ => ("not_reported", json!(0), json!([])),
        };
        json!({"schema":SCHEMA,"success":true,"installation_id":self.installation,"publication":publication,"status":status,"metadata_revision":revision,"labels":labels})
    }
    fn invalidate(&self) {
        if let Ok(mut state) = self.current.lock() {
            *state = Known::Unavailable;
        }
    }
    fn persist(
        &self,
        record: &Record,
        admit: &impl Fn() -> bool,
        same: bool,
    ) -> Result<(), StatusCode> {
        let bytes = serde_json::to_vec(record).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if bytes.len() > MAX_BYTES || !self.valid_directory() {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        let path = self.dir.join(FILE);
        if same {
            let mut file =
                private_regular(&path, true).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            file.sync_all()
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            File::open(&self.dir)
                .and_then(|f| f.sync_all())
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            let mut readback = Vec::new();
            file.read_to_end(&mut readback)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            if readback != bytes || !admit() {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            return Ok(());
        }
        let stage = self.dir.join(STAGE);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&stage)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        // Any uncertain stage/rename barrier remains unavailable; never guess at promotion.
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let owned_stage = || {
            let (Ok(open), Ok(named)) = (file.metadata(), fs::symlink_metadata(&stage)) else {
                return false;
            };
            if !named.file_type().is_file() {
                return false;
            }
            #[cfg(unix)]
            if (open.dev(), open.ino()) != (named.dev(), named.ino()) {
                return false;
            }
            true
        };
        if !owned_stage() {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        if !self.valid_directory() || !admit() {
            if !self.valid_directory() || !owned_stage() {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            fs::remove_file(&stage).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            File::open(&self.dir)
                .and_then(|f| f.sync_all())
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            return Err(StatusCode::CONFLICT);
        }
        fs::rename(&stage, &path)
            .and_then(|_| File::open(&self.dir)?.sync_all())
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let mut readback = Vec::new();
        private_regular(&path, false)
            .and_then(|f| f.take(MAX_BYTES as u64 + 1).read_to_end(&mut readback))
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if readback != bytes {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Ok(())
    }
}
impl SharedRoomStore {
    pub(crate) fn node_labels_snapshot(&self) -> Result<Value, StatusCode> {
        if self.labels.owner.is_none() {
            return Err(StatusCode::FORBIDDEN);
        }
        // Both locks are memory only and taken in room -> label order.
        let room = self
            .current
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        Ok(self.labels.snapshot_for(Publication::from_room(&room)))
    }
    pub(crate) fn update_node_labels(
        &self,
        request: Update,
        admit: impl Fn() -> bool,
    ) -> Result<Value, StatusCode> {
        let labels = &self.labels;
        if labels.owner.is_none() {
            return Err(StatusCode::FORBIDDEN);
        }
        if request.schema != UPDATE
            || request.installation_id != labels.installation
            || !request.expected_publication.valid()
            || request.expected_metadata_revision > MAX_SAFE
            || !labels_valid(&request.labels, &request.expected_publication)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        let _writer = labels
            .writer
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        // Fixed owned lock inode survives every write and profile erasure removes it with the profile.
        if !labels.valid_directory() {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        let lock_path = labels.dir.join(".companion-room-node-labels-v1.lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        #[cfg(target_os = "macos")]
        options.custom_flags(0x100);
        #[cfg(target_os = "linux")]
        options.custom_flags(0x20000);
        let lock = options
            .open(lock_path)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let lock_meta = lock
            .metadata()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if !lock_meta.is_file() || lock_meta.len() != 0 {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        #[cfg(unix)]
        if lock_meta.permissions().mode() & 0o077 != 0 || lock_meta.nlink() != 1 {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        lock.try_lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let disk = labels.read_disk();
        let old = match &disk {
            Known::Unavailable => {
                labels.invalidate();
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            Known::Record(r) if r.publication == request.expected_publication => Some(r),
            _ => None,
        };
        let revision = old.map_or(0, |r| r.metadata_revision);
        if revision != request.expected_metadata_revision {
            return Err(StatusCode::CONFLICT);
        }
        let current_matches = || {
            self.current.lock().is_ok_and(|room| {
                Publication::from_room(&room).as_ref() == Some(&request.expected_publication)
            })
        };
        if !current_matches() || !admit() {
            return Err(StatusCode::CONFLICT);
        }
        let same = old.is_some_and(|r| r.labels == request.labels);
        if !same && revision == MAX_SAFE {
            return Err(StatusCode::CONFLICT);
        }
        let record = Record {
            schema: SCHEMA.into(),
            owner_scope: labels.owner.clone().unwrap(),
            installation_id: labels.installation.clone(),
            publication: request.expected_publication.clone(),
            metadata_revision: if same { revision } else { revision + 1 },
            labels: request.labels,
        };
        if let Err(error) = labels.persist(&record, &|| current_matches() && admit(), same) {
            if error == StatusCode::CONFLICT {
                let reloaded = labels.read_disk();
                *labels
                    .current
                    .lock()
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)? = reloaded;
            } else {
                labels.invalidate();
            }
            return Err(error);
        }
        *labels
            .current
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)? = Known::Record(record);
        // Admission callbacks may observe/revoke authority or publish another
        // room. Invoke them without the room lock, then validate and project
        // the ACK under one room -> labels memory critical section. No disk IO
        // or callback is allowed between that final tuple check and projection.
        if !admit() {
            return Err(StatusCode::CONFLICT);
        }
        let room = self
            .current
            .lock()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let publication = Publication::from_room(&room);
        if publication.as_ref() != Some(&request.expected_publication) {
            return Err(StatusCode::CONFLICT);
        }
        let snapshot = labels.snapshot_for(publication);
        let acknowledgement = json!({"schema":UPDATE,"success":true,"effect":if same {"already_current"} else {"updated"},"snapshot":snapshot});
        drop(room);
        Ok(acknowledgement)
    }
}

#[cfg(test)]
#[path = "node_labels_tests.rs"]
mod tests;
