//! Durable HAP accessory identity, setup verifier, and controller pairings.
#![cfg_attr(not(feature = "hap-server"), allow(dead_code))]

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::RwLock;

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2_11::Sha512;
use srp::ClientG3072;
use tempfile::Builder;
use tokio::sync::watch;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::HapError;

const STORE_VERSION: u32 = 2;
const MAX_STORE_BYTES: u64 = 1024 * 1024;
const MAX_CONTROLLER_ID_BYTES: usize = 64;
const MAX_CONTROLLERS: usize = 16;
const MAX_PAIR_SETUP_FAILURES: u16 = 100;
const SRP_USERNAME: &[u8] = b"Pair-Setup";

/// A controller authorized by a completed HAP pairing ceremony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerPairing {
    /// Case-sensitive HAP controller pairing identifier.
    pub controller_id: String,
    /// Controller Ed25519 long-term public key.
    pub public_key: [u8; 32],
    /// Whether this controller may manage other pairings.
    pub admin: bool,
}

impl ControllerPairing {
    /// Validate bounded identifiers and the encoded Ed25519 point.
    pub fn validate(&self) -> Result<(), HapError> {
        let len = self.controller_id.len();
        if len == 0
            || len > MAX_CONTROLLER_ID_BYTES
            || self.controller_id.chars().any(char::is_control)
        {
            return Err(HapError::InvalidPairingRecord(
                "controller_id must contain 1..=64 bytes and no control characters".into(),
            ));
        }
        VerifyingKey::from_bytes(&self.public_key).map_err(|_| {
            HapError::InvalidPairingRecord("controller public key is not valid Ed25519".into())
        })?;
        Ok(())
    }
}

/// A newly provisioned HAP setup code.
///
/// The code is returned only when a store is created. The persisted file holds
/// an SRP verifier instead of the raw code. Call [`SetupCode::expose`] only at
/// the operator-facing provisioning boundary.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SetupCode {
    bytes: [u8; 10],
}

impl SetupCode {
    pub fn parse(value: &str) -> Result<Self, HapError> {
        let bytes: [u8; 10] = value.as_bytes().try_into().map_err(|_| {
            HapError::InvalidPairingRecord("setup code must use the XXX-XX-XXX format".into())
        })?;
        if bytes[3] != b'-'
            || bytes[6] != b'-'
            || bytes
                .iter()
                .enumerate()
                .any(|(index, byte)| index != 3 && index != 6 && !byte.is_ascii_digit())
        {
            return Err(HapError::InvalidPairingRecord(
                "setup code must use the XXX-XX-XXX format".into(),
            ));
        }
        let digits: Vec<u8> = bytes.iter().copied().filter(u8::is_ascii_digit).collect();
        if is_trivial_setup_code(&digits) {
            return Err(HapError::InvalidPairingRecord(
                "setup code is prohibited because it is trivial".into(),
            ));
        }
        Ok(Self { bytes })
    }

    /// Explicitly expose the code for a display, label, or provisioning UI.
    pub fn expose(&self) -> &str {
        std::str::from_utf8(&self.bytes).expect("validated setup code is ASCII")
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn generate() -> Result<Self, HapError> {
        const RANGE: u32 = 100_000_000;
        const ACCEPT_BELOW: u32 = (u32::MAX / RANGE) * RANGE;
        loop {
            let mut random = [0u8; 4];
            getrandom::getrandom(&mut random)
                .map_err(|error| HapError::PairingStore(format!("generate setup code: {error}")))?;
            let sample = u32::from_le_bytes(random);
            if sample >= ACCEPT_BELOW {
                continue;
            }
            let digits = format!("{:08}", sample % RANGE);
            if is_trivial_setup_code(digits.as_bytes()) {
                continue;
            }
            return Self::parse(&format!(
                "{}-{}-{}",
                &digits[..3],
                &digits[3..5],
                &digits[5..]
            ));
        }
    }
}

impl std::fmt::Debug for SetupCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SetupCode([REDACTED])")
    }
}

fn is_trivial_setup_code(digits: &[u8]) -> bool {
    digits == b"12345678"
        || digits == b"87654321"
        || (digits.len() == 8 && digits.iter().all(|digit| *digit == digits[0]))
}

/// Result of opening an existing store or provisioning a new one.
pub struct PairingStoreProvisioning {
    pub store: PairingStore,
    /// Present only on first creation. It is never recoverable from the store.
    pub setup_code: Option<SetupCode>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAccessory {
    device_id: String,
    signing_seed: [u8; 32],
}

// Manual, redacted impl: `signing_seed` is the accessory's permanent Ed25519
// identity key, used to sign every Pair-Setup/Pair-Verify transcript for the
// device's whole lifetime with no rotation mechanism. A derived `Debug` would
// print it in plaintext the first time anything formats this struct (a log
// line, a panic message) — same rationale as `SetupCode`'s manual impl below.
impl std::fmt::Debug for StoredAccessory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredAccessory")
            .field("device_id", &self.device_id)
            .field("signing_seed", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSetup {
    salt: [u8; 16],
    verifier: Vec<u8>,
}

// Manual, redacted impl: `salt`/`verifier` are the SRP-6a material derived
// from the setup code. Printing them would hand an attacker exactly what an
// offline dictionary attack against the (8-digit) setup code needs.
impl std::fmt::Debug for StoredSetup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StoredSetup([REDACTED])")
    }
}

impl Drop for StoredAccessory {
    fn drop(&mut self) {
        self.signing_seed.zeroize();
    }
}

impl Drop for StoredSetup {
    fn drop(&mut self) {
        self.salt.zeroize();
        self.verifier.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PairingFile {
    version: u32,
    accessory: StoredAccessory,
    setup: StoredSetup,
    controllers: Vec<ControllerPairing>,
}

#[derive(Debug, Clone)]
struct StoreState {
    accessory: StoredAccessory,
    setup: StoredSetup,
    controllers: BTreeMap<String, ControllerPairing>,
}

/// Thread-safe, atomically persisted HAP security store.
#[derive(Debug)]
pub struct PairingStore {
    path: PathBuf,
    state: RwLock<StoreState>,
    pair_setup_active: AtomicBool,
    pair_setup_failures: AtomicU16,
    changes: watch::Sender<u64>,
}

impl PairingStore {
    /// Open an existing v2 store.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, HapError> {
        let path = path.into();
        if !path.exists() {
            return Err(HapError::PairingStore(format!(
                "{} does not exist; use PairingStore::load_or_create for first provisioning",
                path.display()
            )));
        }
        Self::from_state(path.clone(), load_file(&path)?)
    }

    /// Open a store, or securely create identity/setup material and return the
    /// one-time setup code.
    pub fn load_or_create(path: impl Into<PathBuf>) -> Result<PairingStoreProvisioning, HapError> {
        let path = path.into();
        if path.exists() {
            return Ok(PairingStoreProvisioning {
                store: Self::open(path)?,
                setup_code: None,
            });
        }
        let setup_code = SetupCode::generate()?;
        let state = new_state(&setup_code, None)?;
        persist_file(&path, &state, false)?;
        Ok(PairingStoreProvisioning {
            store: Self::from_state(path, state)?,
            setup_code: Some(setup_code),
        })
    }

    /// Deterministic provisioning entry point for an externally generated
    /// per-accessory setup code and optional device ID.
    pub fn create(
        path: impl Into<PathBuf>,
        setup_code: SetupCode,
        device_id: Option<String>,
    ) -> Result<Self, HapError> {
        let path = path.into();
        if path.exists() {
            return Err(HapError::PairingStore(format!(
                "{} already exists",
                path.display()
            )));
        }
        let state = new_state(&setup_code, device_id)?;
        persist_file(&path, &state, false)?;
        Self::from_state(path, state)
    }

    fn from_state(path: PathBuf, state: StoreState) -> Result<Self, HapError> {
        validate_state(&state)?;
        let (changes, _) = watch::channel(0);
        Ok(Self {
            path,
            state: RwLock::new(state),
            pair_setup_active: AtomicBool::new(false),
            pair_setup_failures: AtomicU16::new(0),
            changes,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn accessory_id(&self) -> Result<String, HapError> {
        Ok(self.read_state()?.accessory.device_id.clone())
    }

    pub fn accessory_public_key(&self) -> Result<[u8; 32], HapError> {
        Ok(self.signing_key()?.verifying_key().to_bytes())
    }

    pub fn list(&self) -> Result<Vec<ControllerPairing>, HapError> {
        Ok(self.read_state()?.controllers.values().cloned().collect())
    }

    pub fn get(&self, controller_id: &str) -> Result<Option<ControllerPairing>, HapError> {
        Ok(self.read_state()?.controllers.get(controller_id).cloned())
    }

    pub fn is_paired(&self) -> Result<bool, HapError> {
        Ok(!self.read_state()?.controllers.is_empty())
    }

    /// Add the first administrator after a fully authenticated Pair-Setup M5.
    pub(crate) fn add_initial(&self, pairing: ControllerPairing) -> Result<(), HapError> {
        pairing.validate()?;
        if !pairing.admin {
            return Err(HapError::InvalidPairingRecord(
                "the first controller must be an administrator".into(),
            ));
        }
        self.update(|state| {
            if !state.controllers.is_empty() {
                return Err(HapError::PairingAlreadyExists(
                    pairing.controller_id.clone(),
                ));
            }
            state
                .controllers
                .insert(pairing.controller_id.clone(), pairing);
            Ok(())
        })?;
        self.pair_setup_failures.store(0, Ordering::Release);
        Ok(())
    }

    /// Add or update a pairing according to HAP Add Pairing semantics.
    pub(crate) fn upsert(&self, pairing: ControllerPairing) -> Result<(), HapError> {
        pairing.validate()?;
        self.update(|state| {
            if let Some(existing) = state.controllers.get(&pairing.controller_id) {
                if existing.public_key != pairing.public_key {
                    return Err(HapError::InvalidPairingRecord(
                        "existing pairing public key cannot be replaced".into(),
                    ));
                }
            } else if state.controllers.len() >= MAX_CONTROLLERS {
                return Err(HapError::PairingCapacity);
            }
            state
                .controllers
                .insert(pairing.controller_id.clone(), pairing);
            Ok(())
        })
    }

    /// Remove a pairing idempotently. Removing the final administrator clears
    /// every pairing as required by HAP.
    pub(crate) fn remove_hap(&self, controller_id: &str) -> Result<bool, HapError> {
        let mut removed = false;
        self.update(|state| {
            removed = state.controllers.remove(controller_id).is_some();
            if removed
                && !state.controllers.is_empty()
                && !state.controllers.values().any(|pairing| pairing.admin)
            {
                state.controllers.clear();
            }
            Ok(())
        })?;
        Ok(removed)
    }

    pub(crate) fn signing_key(&self) -> Result<SigningKey, HapError> {
        Ok(SigningKey::from_bytes(
            &self.read_state()?.accessory.signing_seed,
        ))
    }

    pub(crate) fn setup_record(&self) -> Result<([u8; 16], Vec<u8>), HapError> {
        let state = self.read_state()?;
        Ok((state.setup.salt, state.setup.verifier.clone()))
    }

    pub(crate) fn try_begin_pair_setup(&self) -> bool {
        self.pair_setup_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn end_pair_setup(&self) {
        self.pair_setup_active.store(false, Ordering::Release);
    }

    pub(crate) fn pair_setup_locked_out(&self) -> bool {
        self.pair_setup_failures.load(Ordering::Acquire) >= MAX_PAIR_SETUP_FAILURES
    }

    pub(crate) fn record_pair_setup_failure(&self) {
        let _ = self.pair_setup_failures.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |attempts| Some(attempts.saturating_add(1)),
        );
    }

    pub(crate) fn subscribe_changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, StoreState>, HapError> {
        self.state
            .read()
            .map_err(|_| HapError::PairingStore("pairing store lock poisoned".into()))
    }

    fn update(
        &self,
        mutate: impl FnOnce(&mut StoreState) -> Result<(), HapError>,
    ) -> Result<(), HapError> {
        let mut guard = self
            .state
            .write()
            .map_err(|_| HapError::PairingStore("pairing store lock poisoned".into()))?;
        let mut next = guard.clone();
        mutate(&mut next)?;
        validate_state(&next)?;
        persist_file(&self.path, &next, true)?;
        *guard = next;
        self.changes.send_modify(|revision| *revision += 1);
        Ok(())
    }
}

fn new_state(setup_code: &SetupCode, device_id: Option<String>) -> Result<StoreState, HapError> {
    let mut signing_seed = [0u8; 32];
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut signing_seed)
        .and_then(|_| getrandom::getrandom(&mut salt))
        .map_err(|error| HapError::PairingStore(format!("generate accessory identity: {error}")))?;
    let device_id = match device_id {
        Some(device_id) => device_id,
        None => generate_device_id()?,
    };
    let verifier = padded_verifier(ClientG3072::<Sha512>::new().compute_verifier(
        SRP_USERNAME,
        setup_code.as_bytes(),
        &salt,
    ));
    let state = StoreState {
        accessory: StoredAccessory {
            device_id,
            signing_seed,
        },
        setup: StoredSetup { salt, verifier },
        controllers: BTreeMap::new(),
    };
    validate_state(&state)?;
    Ok(state)
}

/// Width of a 3072-bit SRP group element, the verifier's stored size.
const SRP_VERIFIER_LEN: usize = 384;

/// `compute_verifier` returns the minimal big-endian encoding, so about one
/// verifier in 256 has a zero top byte and comes back shorter. Left-pad to the
/// group width: the same number, and the fixed length the store validates.
fn padded_verifier(verifier: Vec<u8>) -> Vec<u8> {
    if verifier.len() >= SRP_VERIFIER_LEN {
        return verifier;
    }
    let mut padded = vec![0u8; SRP_VERIFIER_LEN - verifier.len()];
    padded.extend_from_slice(&verifier);
    padded
}

fn generate_device_id() -> Result<String, HapError> {
    let mut bytes = [0u8; 6];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| HapError::PairingStore(format!("generate device ID: {error}")))?;
    Ok(bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":"))
}

fn validate_device_id(device_id: &str) -> Result<(), HapError> {
    let parts: Vec<&str> = device_id.split(':').collect();
    if parts.len() != 6
        || parts
            .iter()
            .any(|part| part.len() != 2 || !part.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(HapError::InvalidPairingRecord(
            "accessory device ID must be six colon-separated hexadecimal octets".into(),
        ));
    }
    Ok(())
}

fn validate_state(state: &StoreState) -> Result<(), HapError> {
    validate_device_id(&state.accessory.device_id)?;
    SigningKey::from_bytes(&state.accessory.signing_seed);
    if state.setup.verifier.len() != SRP_VERIFIER_LEN {
        return Err(HapError::InvalidPairingRecord(
            "SRP verifier must contain exactly 384 bytes".into(),
        ));
    }
    if state.controllers.len() > MAX_CONTROLLERS {
        return Err(HapError::InvalidPairingRecord(
            "too many persisted controller pairings".into(),
        ));
    }
    for (id, pairing) in &state.controllers {
        pairing.validate()?;
        if id != &pairing.controller_id {
            return Err(HapError::InvalidPairingRecord(
                "controller map key does not match identifier".into(),
            ));
        }
    }
    if !state.controllers.is_empty() && !state.controllers.values().any(|pairing| pairing.admin) {
        return Err(HapError::InvalidPairingRecord(
            "persisted pairings have no administrator".into(),
        ));
    }
    Ok(())
}

fn load_file(path: &Path) -> Result<StoreState, HapError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| HapError::PairingStore(format!("metadata {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HapError::PairingStore(format!(
            "{} must be a regular, non-symlink file",
            path.display()
        )));
    }
    if metadata.len() > MAX_STORE_BYTES {
        return Err(HapError::PairingStore(format!(
            "{} exceeds the {MAX_STORE_BYTES}-byte limit",
            path.display()
        )));
    }
    validate_permissions(path, &metadata)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    File::open(path)
        .and_then(|file| file.take(MAX_STORE_BYTES + 1).read_to_end(bytes.as_mut()))
        .map_err(|error| HapError::PairingStore(format!("read {}: {error}", path.display())))?;
    if bytes.len() as u64 > MAX_STORE_BYTES {
        return Err(HapError::PairingStore(
            "pairing store exceeds size limit".into(),
        ));
    }
    let file: PairingFile = serde_json::from_slice(bytes.as_slice())
        .map_err(|error| HapError::PairingStore(format!("parse {}: {error}", path.display())))?;
    if file.version != STORE_VERSION {
        return Err(HapError::PairingStore(format!(
            "unsupported pairing store version {}; v1 controller-only stores cannot be used because they lack accessory identity and setup material",
            file.version
        )));
    }
    let mut controllers = BTreeMap::new();
    for pairing in file.controllers {
        pairing.validate()?;
        let id = pairing.controller_id.clone();
        if controllers.insert(id.clone(), pairing).is_some() {
            return Err(HapError::InvalidPairingRecord(format!(
                "duplicate controller_id {id}"
            )));
        }
    }
    let state = StoreState {
        accessory: file.accessory,
        setup: file.setup,
        controllers,
    };
    validate_state(&state)?;
    Ok(state)
}

fn persist_file(path: &Path, state: &StoreState, overwrite: bool) -> Result<(), HapError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    create_private_dir(parent)?;
    let payload = Zeroizing::new(
        serde_json::to_vec_pretty(&PairingFile {
            version: STORE_VERSION,
            accessory: state.accessory.clone(),
            setup: state.setup.clone(),
            controllers: state.controllers.values().cloned().collect(),
        })
        .map_err(|error| HapError::PairingStore(format!("serialize pairings: {error}")))?,
    );
    let mut temp = Builder::new()
        .prefix(".homecore-hap-security-")
        .tempfile_in(parent)
        .map_err(|error| HapError::PairingStore(format!("create temporary store: {error}")))?;
    set_private_file_permissions(temp.as_file())?;
    temp.write_all(&payload)
        .and_then(|_| temp.flush())
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|error| HapError::PairingStore(format!("write temporary store: {error}")))?;
    if overwrite {
        temp.persist(path).map_err(|error| {
            HapError::PairingStore(format!("replace {}: {}", path.display(), error.error))
        })?;
    } else {
        temp.persist_noclobber(path).map_err(|error| {
            HapError::PairingStore(format!("create {}: {}", path.display(), error.error))
        })?;
    }
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| HapError::PairingStore(format!("sync {}: {error}", parent.display())))?;
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<(), HapError> {
    let created = !path.exists();
    if created {
        fs::create_dir_all(path).map_err(|error| {
            HapError::PairingStore(format!("create {}: {error}", path.display()))
        })?;
    }
    #[cfg(unix)]
    if created {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
            HapError::PairingStore(format!("chmod {}: {error}", path.display()))
        })?;
    }
    Ok(())
}

fn set_private_file_permissions(_file: &File) -> Result<(), HapError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        _file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| HapError::PairingStore(format!("chmod temporary store: {error}")))?;
    }
    Ok(())
}

fn validate_permissions(path: &Path, metadata: &fs::Metadata) -> Result<(), HapError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(HapError::InsecurePermissions {
                path: path.to_path_buf(),
                mode: mode & 0o777,
            });
        }
    }
    let _ = (path, metadata);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(directory: &tempfile::TempDir) -> PairingStore {
        PairingStore::create(
            directory.path().join("pairings.json"),
            SetupCode::parse("518-26-003").unwrap(),
            Some("AA:BB:CC:DD:EE:FF".into()),
        )
        .unwrap()
    }

    fn pairing(id: &str, byte: u8, admin: bool) -> ControllerPairing {
        ControllerPairing {
            controller_id: id.into(),
            public_key: SigningKey::from_bytes(&[byte; 32])
                .verifying_key()
                .to_bytes(),
            admin,
        }
    }

    #[test]
    fn short_verifier_is_left_padded_to_the_group_width() {
        // Seen in CI: a verifier with a zero top byte encoded to 383 bytes and
        // failed validation, about one provisioning in 256.
        let short = vec![0xAB; SRP_VERIFIER_LEN - 1];
        let padded = padded_verifier(short.clone());
        assert_eq!(padded.len(), SRP_VERIFIER_LEN);
        assert_eq!(padded[0], 0);
        assert_eq!(&padded[1..], &short[..], "the same big-endian number");
        let full = vec![0xCD; SRP_VERIFIER_LEN];
        assert_eq!(
            padded_verifier(full.clone()),
            full,
            "a full-width verifier is untouched"
        );
    }

    #[test]
    fn first_provisioning_returns_code_but_restart_does_not() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pairings.json");
        let provisioned = PairingStore::load_or_create(&path).unwrap();
        let id = provisioned.store.accessory_id().unwrap();
        assert!(provisioned.setup_code.is_some());
        drop(provisioned);
        let reopened = PairingStore::load_or_create(path).unwrap();
        assert!(reopened.setup_code.is_none());
        assert_eq!(reopened.store.accessory_id().unwrap(), id);
    }

    #[test]
    fn identity_and_pairings_survive_atomic_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pairings.json");
        let store = store(&directory);
        let public_key = store.accessory_public_key().unwrap();
        store.add_initial(pairing("controller-1", 7, true)).unwrap();
        drop(store);
        let reopened = PairingStore::open(path).unwrap();
        assert_eq!(reopened.accessory_public_key().unwrap(), public_key);
        assert_eq!(
            reopened.list().unwrap(),
            vec![pairing("controller-1", 7, true)]
        );
    }

    #[test]
    fn prohibited_setup_codes_are_rejected_and_debug_is_redacted() {
        assert!(SetupCode::parse("111-11-111").is_err());
        assert!(SetupCode::parse("123-45-678").is_err());
        let code = SetupCode::parse("518-26-003").unwrap();
        assert_eq!(format!("{code:?}"), "SetupCode([REDACTED])");
    }

    /// A stray `format!("{store:?}")` (a future debug log line, a panic
    /// message) must never print the accessory's permanent Ed25519 signing
    /// seed or the SRP salt/verifier — both are compromise-forever secrets
    /// with no rotation mechanism.
    #[test]
    fn stored_accessory_and_setup_debug_never_print_secret_material() {
        let accessory = StoredAccessory {
            device_id: "AA:BB:CC:DD:EE:FF".into(),
            signing_seed: [0x42; 32],
        };
        let rendered = format!("{accessory:?}");
        assert!(rendered.contains("device_id"));
        assert!(rendered.contains("AA:BB:CC:DD:EE:FF"));
        assert!(!rendered.contains("66"), "hex of 0x42 must not leak: {rendered}");
        assert_eq!(
            rendered,
            "StoredAccessory { device_id: \"AA:BB:CC:DD:EE:FF\", signing_seed: \"[REDACTED]\" }"
        );

        let setup = StoredSetup { salt: [0x7a; 16], verifier: vec![0x13; 8] };
        assert_eq!(format!("{setup:?}"), "StoredSetup([REDACTED])");

        // The redaction must propagate through every derived-Debug container
        // that embeds these structs, with no further code changes needed.
        let state = StoreState {
            accessory,
            setup,
            controllers: std::collections::BTreeMap::new(),
        };
        let rendered_state = format!("{state:?}");
        assert!(!rendered_state.contains("0x42"));
        assert!(rendered_state.contains("[REDACTED]"));
    }

    #[test]
    fn removing_last_admin_clears_all_pairings() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);
        store.add_initial(pairing("admin", 1, true)).unwrap();
        store.upsert(pairing("member", 2, false)).unwrap();
        assert!(store.remove_hap("admin").unwrap());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn malformed_or_legacy_records_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pairings.json");
        fs::write(&path, br#"{"version":1,"controllers":[]}"#).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(PairingStore::open(path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn permissive_existing_file_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pairings.json");
        let _ = store(&directory);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            PairingStore::open(path),
            Err(HapError::InsecurePermissions { .. })
        ));
    }
}
