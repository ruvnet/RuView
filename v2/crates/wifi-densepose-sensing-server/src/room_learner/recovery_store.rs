//! Explicitly consented encrypted reduced-sample recovery; no plaintext fallback.
use ring::{
    aead, hkdf,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const RECOVERY_SCHEMA: &str = "ruview.room-learner.capture-recovery.v1";
pub const MAX_RECOVERY_BYTES: usize = 64 * 1024 * 1024;
pub const RECOVERY_TTL_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_DESCRIPTOR: usize = 4096;
const MAGIC: &[u8; 8] = b"RVCREC01";
const FILE: &str = "capture-recovery-v1.bin";
const MAX_FILE: usize = MAX_RECOVERY_BYTES + MAX_DESCRIPTOR + 40;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryDescriptor {
    pub schema: String,
    pub id: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub context_digest: String,
    pub created_at_ms: u64,
    pub saved_at_ms: u64,
    pub expires_at_ms: u64,
}
pub struct LoadedRecovery {
    pub descriptor: RecoveryDescriptor,
    pub plaintext: Vec<u8>,
}
pub struct RecoveryStore {
    directory: PathBuf,
    owner: String,
    installation: String,
    key: Option<aead::LessSafeKey>,
    serial: Mutex<()>,
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|b| b.is_ascii_control())
}
fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[cfg(unix)]
fn private_metadata(meta: &fs::Metadata, directory: bool) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no arguments or memory side effects.
    let uid = unsafe { geteuid() };
    if meta.uid() != uid
        || meta.permissions().mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
    {
        return Err("recovery_private_permissions".into());
    }
    Ok(())
}
#[cfg(not(unix))]
fn private_metadata(_: &fs::Metadata, _: bool) -> Result<(), String> {
    Err("recovery_private_permissions_unsupported".into())
}
fn private_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() || !m.is_dir() => {
            return Err("recovery_unsafe_directory".into())
        }
        Ok(m) => return private_metadata(&m, true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("recovery_directory_unavailable".into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut b = fs::DirBuilder::new();
        b.mode(0o700);
        match b.create(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err("recovery_directory_unavailable".into()),
        }
    }
    let m = fs::symlink_metadata(path).map_err(|_| "recovery_directory_unavailable")?;
    if m.file_type().is_symlink() || !m.is_dir() {
        return Err("recovery_unsafe_directory".into());
    }
    private_metadata(&m, true)
}
fn managed_seed(stable: Option<&str>, value: Option<&str>) -> Option<[u8; 32]> {
    if stable != Some("1") {
        return None;
    }
    let value = value?;
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}
impl RecoveryStore {
    pub fn new(
        data_dir: &Path,
        owner: &str,
        installation: &str,
        seed: Option<&[u8; 32]>,
    ) -> Result<Self, String> {
        if !hex(owner) || !id(installation) {
            return Err("recovery_invalid_identity".into());
        }
        // Check existing permissions before the legacy owner preparer can repair them.
        for path in [
            data_dir.to_path_buf(),
            data_dir.join("account-owner-v1.json"),
        ] {
            if let Ok(m) = fs::symlink_metadata(&path) {
                if m.file_type().is_symlink() {
                    return Err("recovery_unsafe_owner_path".into());
                }
                private_metadata(&m, path == data_dir)?;
            }
        }
        if data_dir
            .parent()
            .is_some_and(|p| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            return Err("recovery_unsafe_owner_path".into());
        }
        let owned = crate::account_profile::prepare_directory(owner, data_dir)
            .map_err(|_| "recovery_owner_mismatch")?;
        let directory = owned.join("room-learner-recovery");
        private_directory(&directory)?;
        let key = seed
            .map(|seed| {
                let mut info = Vec::new();
                for item in [
                    b"ruview.capture-recovery.aes256gcm.v1".as_slice(),
                    owner.as_bytes(),
                    installation.as_bytes(),
                ] {
                    info.extend_from_slice(&(item.len() as u32).to_be_bytes());
                    info.extend_from_slice(item);
                }
                let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"ruview.capture-recovery.hkdf.v1");
                let prk = salt.extract(seed);
                let infos = [info.as_slice()];
                let okm = prk
                    .expand(&infos, &aead::AES_256_GCM)
                    .map_err(|_| "recovery_key_unavailable")?;
                let mut material = [0u8; 32];
                okm.fill(&mut material)
                    .map_err(|_| "recovery_key_unavailable")?;
                let result = aead::UnboundKey::new(&aead::AES_256_GCM, &material)
                    .map(aead::LessSafeKey::new)
                    .map_err(|_| "recovery_key_unavailable");
                material.fill(0);
                result
            })
            .transpose()?;
        Ok(Self {
            directory,
            owner: owner.into(),
            installation: installation.into(),
            key,
            serial: Mutex::new(()),
        })
    }
    pub fn from_managed_env(
        data_dir: &Path,
        owner: &str,
        installation: &str,
    ) -> Result<Self, String> {
        let stable = std::env::var("RUVIEW_CAPTURE_RECOVERY_KEY_STABLE").ok();
        let value = if stable.as_deref() == Some("1") {
            std::env::var("WDP_RUFIELD_SIGNING_SEED").ok()
        } else {
            None
        };
        let mut seed = managed_seed(stable.as_deref(), value.as_deref());
        let result = Self::new(data_dir, owner, installation, seed.as_ref());
        if let Some(ref mut bytes) = seed {
            bytes.fill(0);
        }
        result
    }
    pub fn available(&self) -> bool {
        self.key.is_some()
    }
    fn verify_directory(&self) -> Result<(), String> {
        let owned = self.directory.parent().ok_or("recovery_owner_mismatch")?;
        for (path, is_directory) in [
            (owned.to_path_buf(), true),
            (owned.join("account-owner-v1.json"), false),
        ] {
            let m = fs::symlink_metadata(path).map_err(|_| "recovery_owner_mismatch")?;
            if m.file_type().is_symlink()
                || (is_directory && !m.is_dir())
                || (!is_directory && !m.is_file())
            {
                return Err("recovery_unsafe_owner_path".into());
            }
            private_metadata(&m, is_directory)?;
        }
        if owned
            .parent()
            .is_some_and(|p| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            return Err("recovery_unsafe_owner_path".into());
        }
        let verified = crate::account_profile::prepare_directory(&self.owner, owned)
            .map_err(|_| "recovery_owner_mismatch")?;
        if verified != owned {
            return Err("recovery_owner_mismatch".into());
        }
        private_directory(&self.directory)
    }
    /// Non-repairing owner validation for read-scoped status discovery.
    /// Unlike prepare_directory this never creates directories or chmods them.
    fn verify_directory_read_only(&self)->Result<(),String>{
        let owned=self.directory.parent().ok_or("recovery_owner_mismatch")?;
        if !hex(&self.owner)||owned.file_name().and_then(|v|v.to_str())!=Some("SensingServer")||owned.parent().and_then(Path::file_name).and_then(|v|v.to_str())!=Some(self.owner.as_str())||owned.canonicalize().map_err(|_|"recovery_owner_mismatch")?!=owned{return Err("recovery_owner_mismatch".into());}
        for(path,directory)in[(owned.to_path_buf(),true),(owned.join("account-owner-v1.json"),false),(self.directory.clone(),true)]{
            let m=fs::symlink_metadata(&path).map_err(|_|"recovery_owner_mismatch")?;
            if m.file_type().is_symlink()||(directory&&!m.is_dir())||(!directory&&!m.is_file()){return Err("recovery_unsafe_owner_path".into());}private_metadata(&m,directory)?;
        }
        let witness=crate::companion::private_read(&owned.join("account-owner-v1.json")).map_err(|_|"recovery_owner_mismatch")?;
        let value:serde_json::Value=serde_json::from_slice(&witness).map_err(|_|"recovery_owner_mismatch")?;
        if value.get("schema").and_then(|v|v.as_str())!=Some("ruview.account-owner.v1")||value.get("scope_key").and_then(|v|v.as_str())!=Some(self.owner.as_str()){return Err("recovery_owner_mismatch".into());}Ok(())
    }
    fn descriptor(&self, d: &RecoveryDescriptor, now: Option<u64>) -> Result<(), String> {
        if d.schema != RECOVERY_SCHEMA
            || !id(&d.id)
            || d.owner_scope != self.owner
            || d.installation_id != self.installation
            || !hex(&d.context_digest)
            || d.created_at_ms == 0
            || d.saved_at_ms < d.created_at_ms
            || d.expires_at_ms <= d.created_at_ms
            || d.expires_at_ms - d.created_at_ms > RECOVERY_TTL_MS
            || d.saved_at_ms >= d.expires_at_ms
        {
            return Err("recovery_descriptor_invalid".into());
        }
        if let Some(now) = now {
            if now < d.created_at_ms || now < d.saved_at_ms {
                return Err("recovery_clock_regression".into());
            }
            if now >= d.expires_at_ms {
                return Err("recovery_expired".into());
            }
        }
        Ok(())
    }
    fn read(&self, now: Option<u64>) -> Result<Option<LoadedRecovery>, String> {
        let key = self.key.as_ref().ok_or("recovery_unavailable")?;
        self.verify_directory()?;
        let path = self.directory.join(FILE);
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("recovery_read_failed".into()),
        };
        if !m.is_file() || m.file_type().is_symlink() || m.len() > MAX_FILE as u64 {
            return Err("recovery_unsafe_file".into());
        }
        private_metadata(&m, false)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            #[cfg(target_os = "macos")]
            options.custom_flags(0x100); // O_NOFOLLOW
            #[cfg(target_os = "linux")]
            options.custom_flags(0x20000); // O_NOFOLLOW
        }
        let file = options.open(&path).map_err(|_| "recovery_read_failed")?;
        let opened = file.metadata().map_err(|_| "recovery_read_failed")?;
        private_metadata(&opened, false)?;
        if !opened.is_file() || opened.len() > MAX_FILE as u64 {
            return Err("recovery_unsafe_file".into());
        }
        let mut bytes = Vec::new();
        file.take((MAX_FILE + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "recovery_read_failed")?;
        if bytes.len() > MAX_FILE || bytes.len() < 40 || &bytes[..8] != MAGIC {
            return Err("recovery_corrupt".into());
        }
        let n =
            u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| "recovery_corrupt")?) as usize;
        if n == 0 || n > MAX_DESCRIPTOR || bytes.len() < 12 + n + 12 + 16 {
            return Err("recovery_corrupt".into());
        }
        let aad = bytes[..12 + n].to_vec();
        let d: RecoveryDescriptor = serde_json::from_slice(&bytes[12..12 + n])
            .map_err(|_| "recovery_descriptor_invalid")?;
        let nonce = aead::Nonce::try_assume_unique_for_key(&bytes[12 + n..24 + n])
            .map_err(|_| "recovery_corrupt")?;
        let mut payload = bytes[24 + n..].to_vec();
        let plain = key
            .open_in_place(nonce, aead::Aad::from(&aad), &mut payload)
            .map_err(|_| "recovery_authentication_failed")?;
        if plain.len() > MAX_RECOVERY_BYTES {
            return Err("recovery_budget_exceeded".into());
        }
        let plaintext = plain.to_vec();
        self.descriptor(&d, now)?;
        Ok(Some(LoadedRecovery {
            descriptor: d,
            plaintext,
        }))
    }
    /// Authenticate retained metadata for an explicit restore/discard decision.
    /// Expiry and current-clock checks belong to load; inspection never renews
    /// the original deadline and never exposes decrypted sample bytes.
    pub fn inspect(&self) -> Result<Option<RecoveryDescriptor>, String> {
        let _lock = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        Ok(self.read(None)?.map(|loaded| loaded.descriptor))
    }
    pub fn load(&self, now_ms: u64) -> Result<Option<LoadedRecovery>, String> {
        let _lock = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        self.read(Some(now_ms))
    }
    pub fn save(
        &self,
        d: &RecoveryDescriptor,
        plaintext: &[u8],
        now_ms: u64,
    ) -> Result<(), String> {
        let _lock = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let key = self.key.as_ref().ok_or("recovery_unavailable")?;
        self.descriptor(d, Some(now_ms))?;
        if plaintext.len() > MAX_RECOVERY_BYTES {
            return Err("recovery_budget_exceeded".into());
        }
        if let Some(previous) = self.read(Some(now_ms))? {
            let p = previous.descriptor;
            if p.id != d.id
                || p.context_digest != d.context_digest
                || p.created_at_ms != d.created_at_ms
                || p.expires_at_ms != d.expires_at_ms
                || d.saved_at_ms < p.saved_at_ms
            {
                return Err("recovery_discard_required".into());
            }
        }
        let descriptor = serde_json::to_vec(d).map_err(|_| "recovery_descriptor_invalid")?;
        if descriptor.len() > MAX_DESCRIPTOR {
            return Err("recovery_descriptor_invalid".into());
        }
        let mut output = MAGIC.to_vec();
        output.extend_from_slice(&(descriptor.len() as u32).to_be_bytes());
        output.extend_from_slice(&descriptor);
        let mut nonce = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "recovery_random_unavailable")?;
        let mut ciphertext = plaintext.to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(&output),
            &mut ciphertext,
        )
        .map_err(|_| "recovery_encrypt_failed")?;
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        let mut suffix = [0u8; 16];
        SystemRandom::new()
            .fill(&mut suffix)
            .map_err(|_| "recovery_random_unavailable")?;
        let temp = self
            .directory
            .join(format!("recovery-stage-{:x}", u128::from_be_bytes(suffix)));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp).map_err(|_| "recovery_write_failed")?;
            file.write_all(&output)
                .and_then(|_| file.sync_all())
                .map_err(|_| "recovery_write_failed")?;
            fs::rename(&temp, self.directory.join(FILE)).map_err(|_| "recovery_write_failed")?;
            File::open(&self.directory)
                .and_then(|f| f.sync_all())
                .map_err(|_| "recovery_write_failed")?;
            let restored = self.read(Some(now_ms))?.ok_or("recovery_readback_failed")?;
            if restored.descriptor != *d || restored.plaintext != plaintext {
                return Err("recovery_readback_failed".into());
            }
            Ok(())
        })();
        if temp.exists() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    pub fn discard(&self, expected_id: &str) -> Result<(), String> {
        let _lock = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        if !id(expected_id) {
            return Err("recovery_invalid_identity".into());
        }
        if let Some(existing) = self.read(None)? {
            if existing.descriptor.id != expected_id {
                return Err("recovery_identity_mismatch".into());
            }
            fs::remove_file(self.directory.join(FILE)).map_err(|_| "recovery_discard_failed")?;
            File::open(&self.directory)
                .and_then(|f| f.sync_all())
                .map_err(|_| "recovery_discard_failed")?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "recovery_store_tests.rs"]
mod tests;

// Typed temporal consistency uses the SAME profile directory/key, not V1 files.
#[path = "temporal_recovery_commitment.rs"]
pub(crate) mod temporal_commitment;
