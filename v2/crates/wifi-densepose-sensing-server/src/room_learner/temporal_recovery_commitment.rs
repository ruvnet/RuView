//! Bounded selective-file consistency for private temporal prefixes.
//! NOT a trusted monotonic anchor against coordinated whole-profile rollback.
//! Typed APIs take receiver nanoseconds; descriptor/retention fields stay ms.
use super::*;
use crate::room_learner::temporal_model_input::managed::execution::checkpoint::{
    is_new_empty_run, validate_extension, validated_prefix, ValidatedTemporalPrefix,
};
use sha2::{Digest, Sha256};
// Storage directory/AAD framing stay unchanged. The schema field below is
// authenticated version DATA; new kind2 never creates a parallel namespace.
const KIND: &str = "temporal-prefix-commitment-v1";
const FULL_KIND: &str = "temporal-prefix-commitment-v2";
const LOCK: &str = "temporal-prefix.lock";
const HEAD: &str = "temporal-head.bin";
const MAGIC: &[u8; 8] = b"RVTCPR01";
const SMALL: usize = 4096;
const MAX_TRANSITIONS: usize = 512;
const MAX_WITNESSES: usize = 2 * MAX_TRANSITIONS;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Meta {
    schema: String,
    generation: u64,
    revision: usize,
    descriptor: RecoveryDescriptor,
    run: String,
    policy_sha256: String,
    transition_limit: usize,
    window_count: usize,
    receiver_ns: u64,
    slot: Option<u8>,
    payload_sha256: Option<String>,
    previous_sha256: Option<String>,
    retired_at_ms: Option<u64>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Meta {
    fn sha(&self) -> Result<String, String> {
        Ok(hash(
            &serde_json::to_vec(self).map_err(|_| "temporal_metadata_encoding")?,
        ))
    }
    fn token(&self) -> Result<TemporalToken, String> {
        Ok(TemporalToken {
            generation: self.generation,
            revision: self.revision,
            run: self.run.clone(),
            id: self.descriptor.id.clone(),
            commitment: self.sha()?,
        })
    }
    fn witness(&self) -> String {
        format!("witness-{:016x}-{:04x}.bin", self.generation, self.revision)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TemporalToken {
    generation: u64,
    revision: usize,
    run: String,
    id: String,
    commitment: String,
}
/// Read-only commitment DATA. Deserializing this is never a TemporalToken or
/// store/producer permission.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalCommitmentData {
    pub generation:u64,pub revision:usize,pub run:String,pub id:String,pub commitment:String,
}
impl TemporalToken {
    pub(crate) fn data(&self)->TemporalCommitmentData{
        TemporalCommitmentData{generation:self.generation,revision:self.revision,run:self.run.clone(),id:self.id.clone(),commitment:self.commitment.clone()}
    }
}
#[derive(Clone)]
pub(crate) struct TemporalIntent {
    expected: Option<TemporalToken>,
    meta: Meta,
}
/// Authenticated observation only; not a completed durability-barrier certificate.
pub(crate) struct TemporalLoaded {
    pub(crate) token: TemporalToken,
    pub(crate) prefix: Option<ValidatedTemporalPrefix>,
}
// NEGATIVE shape only, not an authentication primitive. Caller must first
// authenticate/canonical-validate metadata and enforce owner/clock/chain rules.
// Never accepts a positive payload or a generic limit+1 terminal.
fn legacy_saturated_terminal_shape(m:&Meta)->bool{
    m.schema==KIND && m.transition_limit==329 && m.revision==329
        && m.slot.is_none() && m.payload_sha256.is_none()
        && m.previous_sha256.as_ref().is_some_and(|s|hex(s))
        && m.window_count<=64
        && m.retired_at_ms.is_some_and(|at|at>=m.descriptor.saved_at_ms && at==m.receiver_ns/1_000_000)
}
struct Components {
    head: Option<Meta>,
    latest: Option<Meta>,
    witnesses: Vec<(String, Meta)>,
}
struct Locked {
    file: File,
    directory: PathBuf,
}
impl Drop for Locked {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
impl RecoveryStore {
    fn temporal_dir(&self) -> PathBuf {
        self.directory.join(KIND)
    }
    fn temporal_lock(&self, create: bool) -> Result<Locked, String> {
        self.verify_directory()?;
        let path = self.directory.join(LOCK);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            #[cfg(target_os = "macos")]
            options.custom_flags(0x100);
            #[cfg(target_os = "linux")]
            options.custom_flags(0x20000);
        }
        let file = options.open(&path).map_err(|_| "temporal_lock_open")?;
        private_metadata(
            &file.metadata().map_err(|_| "temporal_lock_metadata")?,
            false,
        )?;
        file.lock().map_err(|_| "temporal_lock_failed")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = file.metadata().map_err(|_| "temporal_lock_metadata")?;
            let b = fs::symlink_metadata(&path).map_err(|_| "temporal_lock_inode_changed")?;
            if b.file_type().is_symlink() || a.ino() != b.ino() || a.dev() != b.dev() {
                return Err("temporal_lock_inode_changed".into());
            }
        }
        let directory = self.temporal_dir();
        if create {
            private_directory(&directory)?;
            File::open(&self.directory)
                .and_then(|f| f.sync_all())
                .map_err(|_| "temporal_namespace_parent_fsync")?;
        } else if directory.exists() {
            private_directory(&directory)?;
        }
        Ok(Locked { file, directory })
    }
    fn temporal_read(&self, path: &Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let meta = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("temporal_read_failed".into()),
        };
        if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > limit as u64 {
            return Err("temporal_unsafe_file".into());
        }
        private_metadata(&meta, false)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            #[cfg(target_os = "macos")]
            options.custom_flags(0x100);
            #[cfg(target_os = "linux")]
            options.custom_flags(0x20000);
        }
        let file = options.open(path).map_err(|_| "temporal_read_failed")?;
        let opened = file.metadata().map_err(|_| "temporal_read_failed")?;
        private_metadata(&opened, false)?;
        if !opened.is_file() || opened.len() > limit as u64 {
            return Err("temporal_unsafe_file".into());
        }
        let mut bytes = Vec::new();
        file.take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "temporal_read_failed")?;
        if bytes.len() > limit {
            return Err("temporal_read_budget".into());
        }
        Ok(Some(bytes))
    }
    fn temporal_seal(&self, domain: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
        let key = self.key.as_ref().ok_or("recovery_unavailable")?;
        let aad = serde_json::to_vec(&(KIND, domain, &self.owner, &self.installation))
            .map_err(|_| "temporal_aad")?;
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "recovery_random_unavailable")?;
        let mut cipher = plain.to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(&aad),
            &mut cipher,
        )
        .map_err(|_| "temporal_encrypt")?;
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&nonce);
        out.extend(cipher);
        Ok(out)
    }
    fn temporal_open(&self, domain: &str, bytes: &[u8], limit: usize) -> Result<Vec<u8>, String> {
        if bytes.len() < 36 || bytes.len() > limit + 36 || &bytes[..8] != MAGIC {
            return Err("temporal_envelope".into());
        }
        let key = self.key.as_ref().ok_or("recovery_unavailable")?;
        let aad = serde_json::to_vec(&(KIND, domain, &self.owner, &self.installation))
            .map_err(|_| "temporal_aad")?;
        let nonce =
            aead::Nonce::try_assume_unique_for_key(&bytes[8..20]).map_err(|_| "temporal_nonce")?;
        let mut cipher = bytes[20..].to_vec();
        let p = key
            .open_in_place(nonce, aead::Aad::from(&aad), &mut cipher)
            .map_err(|_| "temporal_authentication")?;
        if p.len() > limit {
            return Err("temporal_plaintext_budget".into());
        }
        Ok(p.to_vec())
    }
    fn temporal_meta(&self, path: &Path, domain: &str) -> Result<Option<Meta>, String> {
        let Some(bytes) = self.temporal_read(path, SMALL + 36)? else {
            return Ok(None);
        };
        let p = self.temporal_open(domain, &bytes, SMALL)?;
        let m: Meta = serde_json::from_slice(&p).map_err(|_| "temporal_metadata_schema")?;
        if serde_json::to_vec(&m).map_err(|_| "temporal_metadata_encoding")? != p
            || !matches!(m.schema.as_str(),KIND|FULL_KIND)
            || m.transition_limit > MAX_TRANSITIONS
            || m.transition_limit < 2
            || !id(&m.run)
            || !hex(&m.policy_sha256)
            || m.window_count > if m.schema==FULL_KIND{128}else{64}
        {
            return Err("temporal_metadata_binding".into());
        }
        self.descriptor(&m.descriptor, None)?;
        if m.descriptor.expires_at_ms - m.descriptor.created_at_ms != RECOVERY_TTL_MS {
            return Err("temporal_original_deadline".into());
        }
        if m.receiver_ns / 1_000_000 < m.descriptor.saved_at_ms {
            return Err("temporal_metadata_receiver_clock".into());
        }
        match (m.slot, m.payload_sha256.as_ref(), m.retired_at_ms) {
            (Some(slot), Some(sha), None)
                if slot < 2
                    && hex(sha)
                    && m.receiver_ns / 1_000_000 == m.descriptor.saved_at_ms => {}
            (None, None, Some(t))
                if t >= m.descriptor.saved_at_ms && t == m.receiver_ns / 1_000_000 => {}
            _ => return Err("temporal_metadata_state".into()),
        };
        // Only after AEAD/canonical metadata, known-kind/descriptor/owner and
        // state/clock validation may the old-family NEGATIVE terminal occupy
        // its one reserved boundary. Positive payloads stay strictly <limit.
        if m.revision>=m.transition_limit && !legacy_saturated_terminal_shape(&m){return Err("temporal_metadata_binding".into());}
        Ok(Some(m))
    }
    fn temporal_components(&self, dir: &Path) -> Result<Components, String> {
        if !dir.exists() {
            return Ok(Components {
                head: None,
                latest: None,
                witnesses: vec![],
            });
        }
        let mut witnesses = Vec::new();
        let mut count = 0;
        for entry in fs::read_dir(dir).map_err(|_| "temporal_directory_read")? {
            count += 1;
            if count > MAX_WITNESSES + 6 {
                return Err("temporal_directory_budget".into());
            }
            let e = entry.map_err(|_| "temporal_directory_read")?;
            let name = e
                .file_name()
                .into_string()
                .map_err(|_| "temporal_unknown_path")?;
            let entry_meta =
                fs::symlink_metadata(e.path()).map_err(|_| "temporal_directory_read")?;
            if entry_meta.file_type().is_symlink()
                || !entry_meta.is_file()
                || entry_meta.len()
                    > (if name.starts_with("witness-")
                        || [HEAD, "head.stage", "witness.stage"].contains(&name.as_str())
                    {
                        SMALL + 36
                    } else {
                        MAX_RECOVERY_BYTES + SMALL + 48
                    }) as u64
            {
                return Err("temporal_unsafe_directory_entry".into());
            }
            private_metadata(&entry_meta, false)?;
            if name.starts_with("witness-") && name.ends_with(".bin") {
                let m = self
                    .temporal_meta(&e.path(), "witness")?
                    .ok_or("temporal_witness_missing")?;
                if name != m.witness() {
                    return Err("temporal_witness_name".into());
                }
                witnesses.push((name, m));
            } else if ![
                HEAD,
                "slot-0.bin",
                "slot-1.bin",
                "payload.stage",
                "head.stage",
                "witness.stage",
            ]
            .contains(&name.as_str())
            {
                return Err("temporal_unknown_path".into());
            }
        }
        witnesses.sort_by_key(|(_, m)| (m.generation, m.revision));
        let generations = witnesses
            .iter()
            .map(|(_, m)| m.generation)
            .collect::<std::collections::HashSet<_>>();
        if generations.len() > 2 {
            return Err("temporal_generation_capacity".into());
        }
        // GC may already have removed any prefix of an OBSOLETE generation.
        // Authenticate every remaining record above, but require a COMPLETE
        // contiguous chain for the highest committed-witness generation. The
        // head/payload must match its exact tip before any caller gets authority.
        let highest = witnesses.last().map(|(_, m)| m.generation);
        let mut previous: Option<&Meta> = None;
        for (_, m) in witnesses
            .iter()
            .filter(|(_, m)| Some(m.generation) == highest)
        {
            if let Some(p) = previous {
                if m.revision != p.revision + 1 || m.previous_sha256.as_ref() != Some(&p.sha()?) {
                    return Err("temporal_witness_chain".into());
                }
            } else if m.revision != 0
                || (m.generation == 0 && m.previous_sha256.is_some())
                || (m.generation > 0 && !m.previous_sha256.as_ref().is_some_and(|p| hex(p)))
            {
                return Err("temporal_witness_genesis".into());
            }
            previous = Some(m);
        }
        let latest = witnesses.last().map(|(_, m)| m.clone());
        let head = self.temporal_meta(&dir.join(HEAD), "head")?;
        Ok(Components {
            head,
            latest,
            witnesses,
        })
    }
    fn temporal_current(&self, dir: &Path, now_ns: u64) -> Result<Option<TemporalLoaded>, String> {
        let c = self.temporal_components(dir)?;
        match (c.head, c.latest) {
            (None, None) => Ok(None),
            (Some(h), Some(w)) if h == w => {
                if now_ns < h.receiver_ns
                    || now_ns / 1_000_000 < h.descriptor.saved_at_ms
                    || h.retired_at_ms.is_some_and(|t| now_ns / 1_000_000 < t)
                {
                    return Err("recovery_clock_regression".into());
                }
                let token = h.token()?;
                if h.retired_at_ms.is_some() {
                    return Ok(Some(TemporalLoaded {
                        token,
                        prefix: None,
                    }));
                }
                if now_ns / 1_000_000 >= h.descriptor.expires_at_ms {
                    return Err("recovery_expired".into());
                }
                let prefix = self.temporal_payload(dir, &h, now_ns)?;
                Ok(Some(TemporalLoaded {
                    token,
                    prefix: Some(prefix),
                }))
            }
            _ => Err("temporal_head_witness_mismatch_or_pending".into()),
        }
    }
    fn temporal_payload_seal(
        &self,
        m: &Meta,
        p: &ValidatedTemporalPrefix,
    ) -> Result<Vec<u8>, String> {
        let metadata = serde_json::to_vec(m).map_err(|_| "temporal_metadata_encoding")?;
        if metadata.len() > SMALL {
            return Err("temporal_metadata_budget".into());
        }
        let mut out = b"RVTPAY01".to_vec();
        out.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
        out.extend(metadata);
        out.extend(self.temporal_seal(&format!("payload:{}", m.sha()?), p.bytes())?);
        Ok(out)
    }
    fn temporal_payload_any(
        &self,
        path: &Path,
        now_ns: u64,
    ) -> Result<Option<(Meta, ValidatedTemporalPrefix)>, String> {
        let Some(bytes) = self.temporal_read(path, MAX_RECOVERY_BYTES + SMALL + 48)? else {
            return Ok(None);
        };
        if bytes.len() < 12 || &bytes[..8] != b"RVTPAY01" {
            return Err("temporal_payload_header".into());
        }
        let n = u32::from_be_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| "temporal_payload_header")?,
        ) as usize;
        if n == 0 || n > SMALL || bytes.len() < 12 + n + 36 {
            return Err("temporal_payload_header".into());
        }
        let m: Meta =
            serde_json::from_slice(&bytes[12..12 + n]).map_err(|_| "temporal_payload_metadata")?;
        if serde_json::to_vec(&m).map_err(|_| "temporal_metadata_encoding")? != bytes[12..12 + n]
            || !matches!(m.schema.as_str(),KIND|FULL_KIND)
            || m.retired_at_ms.is_some()
        {
            return Err("temporal_payload_metadata".into());
        }
        let plain = self.temporal_open(
            &format!("payload:{}", m.sha()?),
            &bytes[12 + n..],
            MAX_RECOVERY_BYTES,
        )?;
        if Some(hash(&plain)).as_ref() != m.payload_sha256.as_ref() {
            return Err("temporal_payload_hash".into());
        }
        let p = validated_prefix(&plain, &m.descriptor, now_ns)?;
        if p.run() != m.run
            ||p.metadata_kind()!=m.schema
            || p.policy_sha256() != m.policy_sha256
            || p.window_count() != m.window_count
            || p.transition_limit() != m.transition_limit
            || p.saved_receiver_ns() != m.receiver_ns
        {
            return Err("temporal_payload_metadata".into());
        }
        Ok(Some((m, p)))
    }
    fn temporal_payload(
        &self,
        dir: &Path,
        m: &Meta,
        now_ns: u64,
    ) -> Result<ValidatedTemporalPrefix, String> {
        let slot = m.slot.ok_or("temporal_retired")?;
        let (actual, p) = self
            .temporal_payload_any(&dir.join(format!("slot-{slot}.bin")), now_ns)?
            .ok_or("temporal_payload_missing")?;
        if actual != *m {
            return Err("temporal_named_payload_changed".into());
        }
        Ok(p)
    }
    pub(crate) fn load_temporal(&self, now_ns: u64) -> Result<Option<TemporalLoaded>, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(false)?;
        self.temporal_current(&locked.directory, now_ns)
    }
}

impl RecoveryStore {
    pub(crate) fn prepare_temporal(
        &self,
        expected: Option<&TemporalToken>,
        prefix: &ValidatedTemporalPrefix,
        now_ns: u64,
    ) -> Result<TemporalIntent, String> {
        if !prefix.authorizes_write(expected){return Err("temporal_full_run_producer_write_grant_required".into());}
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(true)?;
        self.descriptor(prefix.descriptor(), Some(now_ns / 1_000_000))?;
        if now_ns < prefix.saved_receiver_ns() {
            return Err("recovery_receiver_clock_regression".into());
        }
        let current = self.temporal_current(&locked.directory, now_ns)?;
        let c = self.temporal_components(&locked.directory)?;
        let (generation, revision, previous, slot) = match (current, expected) {
            (None, None) => {
                if fs::read_dir(&locked.directory)
                    .map_err(|_| "temporal_directory_read")?
                    .next()
                    .is_some()
                    || !is_new_empty_run(prefix)?
                {
                    return Err("temporal_explicit_empty_genesis_required".into());
                }
                (0, 0, None, 0)
            }
            (Some(loaded), Some(expected)) if &loaded.token == expected => {
                let old = c.head.as_ref().ok_or("temporal_head_missing")?;
                if let Some(previous_prefix) = loaded.prefix {
                    validate_extension(&previous_prefix, prefix)?;
                    if previous_prefix.bytes() == prefix.bytes() {
                        return Ok(TemporalIntent {
                            expected: Some(expected.clone()),
                            meta: old.clone(),
                        });
                    }
                    if self.temporal_cleanup_pending(&locked.directory, &c, old)? {
                        return Err("temporal_cleanup_repair_required".into());
                    }
                    (
                        old.generation,
                        old.revision
                            .checked_add(1)
                            .ok_or("temporal_revision_overflow")?,
                        Some(old.sha()?),
                        1 - old.slot.ok_or("temporal_retired")?,
                    )
                } else {
                    if self.temporal_cleanup_pending(&locked.directory, &c, old)? {
                        return Err("temporal_cleanup_repair_required".into());
                    }
                    if !is_new_empty_run(prefix)?
                        || prefix.descriptor().id == old.descriptor.id
                        || prefix.run() == old.run
                        || prefix.descriptor().created_at_ms
                            < old.retired_at_ms.ok_or("temporal_retirement_missing")?
                        || prefix.saved_receiver_ns() < old.receiver_ns
                    {
                        return Err("temporal_explicit_new_empty_run_required".into());
                    }
                    (
                        old.generation
                            .checked_add(1)
                            .ok_or("temporal_generation_overflow")?,
                        0,
                        Some(old.sha()?),
                        0,
                    )
                }
            }
            _ => return Err("temporal_expected_token_changed".into()),
        };
        if revision >= prefix.transition_limit() {
            return Err("temporal_transition_budget".into());
        }
        Ok(TemporalIntent {
            expected: expected.cloned(),
            meta: Meta {
                schema: prefix.metadata_kind().into(),
                generation,
                revision,
                descriptor: prefix.descriptor().clone(),
                run: prefix.run().into(),
                policy_sha256: prefix.policy_sha256().into(),
                transition_limit: prefix.transition_limit(),
                window_count: prefix.window_count(),
                receiver_ns: prefix.saved_receiver_ns(),
                slot: Some(slot),
                payload_sha256: Some(hash(prefix.bytes())),
                previous_sha256: previous,
                retired_at_ms: None,
            },
        })
    }
    pub(crate) fn prepare_retire_temporal(
        &self,
        expected: &TemporalToken,
        now_ns: u64,
        expiry_only: bool,
    ) -> Result<TemporalIntent, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(false)?;
        let c = self.temporal_components(&locked.directory)?;
        let mut meta = c
            .head
            .filter(|h| Some(h) == c.latest.as_ref())
            .ok_or("temporal_head_witness_mismatch_or_pending")?;
        if meta.token()? != *expected
            || now_ns < meta.receiver_ns
            || now_ns / 1_000_000 < meta.descriptor.saved_at_ms
            || meta.retired_at_ms.is_some_and(|t| now_ns / 1_000_000 < t)
            || expiry_only && now_ns / 1_000_000 < meta.descriptor.expires_at_ms
        {
            return Err("temporal_retirement_identity_or_clock".into());
        }
        if meta.retired_at_ms.is_none() {
            meta.previous_sha256 = Some(meta.sha()?);
            meta.revision = meta
                .revision
                .checked_add(1)
                .ok_or("temporal_revision_overflow")?;
            meta.slot = None;
            meta.payload_sha256 = None;
            meta.retired_at_ms = Some(now_ns / 1_000_000);
            meta.receiver_ns = now_ns;
            if meta.revision >= meta.transition_limit && !legacy_saturated_terminal_shape(&meta) {
                return Err("temporal_transition_budget".into());
            }
        }
        Ok(TemporalIntent {
            expected: Some(expected.clone()),
            meta,
        })
    }
    fn temporal_write_stage(
        &self,
        dir: &Path,
        stage: &str,
        bytes: &[u8],
        hook: &mut impl FnMut(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        hook(&format!("before:{stage}:write"))?;
        let path = dir.join(stage);
        if let Some(existing) = self.temporal_read(&path, MAX_RECOVERY_BYTES + SMALL + 48)? {
            if existing != bytes {
                return Err("temporal_conflicting_stage_requires_repair".into());
            }
            let f = private_existing_file(&path, true)?;
            f.sync_all().map_err(|_| "temporal_stage_fsync")?;
            return Ok(());
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|_| "temporal_stage_create")?;
        file.write_all(bytes).map_err(|_| "temporal_stage_write")?;
        hook(&format!("after:{stage}:write"))?;
        hook(&format!("before:{stage}:fsync"))?;
        file.sync_all().map_err(|_| "temporal_stage_fsync")?;
        hook(&format!("after:{stage}:fsync"))?;
        Ok(())
    }
    fn temporal_replace(
        &self,
        dir: &Path,
        stage: &str,
        name: &str,
        hook: &mut impl FnMut(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        hook(&format!("before:{stage}:rename"))?;
        fs::rename(dir.join(stage), dir.join(name)).map_err(|_| "temporal_stage_rename")?;
        hook(&format!("after:{stage}:rename"))?;
        hook(&format!("before:{stage}:directory_fsync"))?;
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| "temporal_directory_fsync")?;
        hook(&format!("after:{stage}:directory_fsync"))?;
        Ok(())
    }
    fn temporal_cleanup_pending(
        &self,
        dir: &Path,
        c: &Components,
        current: &Meta,
    ) -> Result<bool, String> {
        if c.witnesses
            .iter()
            .any(|(_, m)| m.generation < current.generation)
        {
            return Ok(true);
        }
        let mut names = vec!["payload.stage", "head.stage", "witness.stage"];
        if current.retired_at_ms.is_some() {
            names.extend(["slot-0.bin", "slot-1.bin"]);
        }
        for name in names {
            match fs::symlink_metadata(dir.join(name)) {
                Ok(_) => return Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("temporal_cleanup_inspection".into()),
            }
        }
        Ok(false)
    }
    fn temporal_cleanup(&self, dir: &Path, current: &Meta) -> Result<(), String> {
        let c = self.temporal_components(dir)?;
        for (name, m) in c.witnesses {
            if m.generation < current.generation {
                fs::remove_file(dir.join(name)).map_err(|_| "temporal_metadata_cleanup")?;
            }
        }
        for name in ["payload.stage", "head.stage", "witness.stage"] {
            if self
                .temporal_read(&dir.join(name), MAX_RECOVERY_BYTES + SMALL + 48)?
                .is_some()
            {
                fs::remove_file(dir.join(name)).map_err(|_| "temporal_stage_cleanup")?;
            }
        }
        if current.retired_at_ms.is_some() {
            for name in ["slot-0.bin", "slot-1.bin"] {
                if self
                    .temporal_read(&dir.join(name), MAX_RECOVERY_BYTES + SMALL + 48)?
                    .is_some()
                {
                    fs::remove_file(dir.join(name)).map_err(|_| "temporal_payload_cleanup")?;
                }
            }
        }
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| "temporal_cleanup_fsync")?;
        Ok(())
    }
    pub(crate) fn publish_temporal(
        &self,
        intent: &TemporalIntent,
        prefix: Option<&ValidatedTemporalPrefix>,
        now_ns: u64,
    ) -> Result<TemporalToken, String> {
        self.publish_temporal_with(intent, prefix, now_ns, false, &mut |_| Ok(()))
    }
    pub(crate) fn repair_same_temporal(
        &self,
        intent: &TemporalIntent,
        prefix: Option<&ValidatedTemporalPrefix>,
        now_ns: u64,
    ) -> Result<TemporalToken, String> {
        self.publish_temporal_with(intent, prefix, now_ns, true, &mut |_| Ok(()))
    }
    fn publish_temporal_with(
        &self,
        intent: &TemporalIntent,
        prefix: Option<&ValidatedTemporalPrefix>,
        now_ns: u64,
        repair: bool,
        hook: &mut impl FnMut(&str) -> Result<(), String>,
    ) -> Result<TemporalToken, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(true)?;
        let dir = &locked.directory;
        let meta = &intent.meta;
        if now_ns < meta.receiver_ns {
            return Err("recovery_receiver_clock_regression".into());
        }
        if let Some(p) = prefix {
            self.descriptor(&meta.descriptor, Some(now_ns / 1_000_000))?;
            if meta.retired_at_ms.is_some()
                || p.descriptor() != &meta.descriptor
                || p.run() != meta.run
                ||p.metadata_kind()!=meta.schema
                || p.policy_sha256() != meta.policy_sha256
                || Some(hash(p.bytes())).as_ref() != meta.payload_sha256.as_ref()
            {
                return Err("temporal_intended_payload_changed".into());
            }
        } else if meta.retired_at_ms.is_none() || now_ns / 1_000_000 < meta.retired_at_ms.unwrap() {
            return Err("temporal_intended_retirement_changed".into());
        }
        let c = self.temporal_components(dir)?;
        let latest = c.latest.as_ref().map(Meta::token).transpose()?;
        let target = meta.token()?;
        let head = c.head.as_ref().map(Meta::token).transpose()?;
        let already = latest.as_ref() == Some(&target);
        if already {
            if !repair && intent.expected.as_ref() != Some(&target) {
                return Err("temporal_exact_repair_required".into());
            }
            if head.as_ref() != Some(&target) && !repair {
                return Err("temporal_head_witness_mismatch_or_pending".into());
            }
        } else {
            if latest != intent.expected {
                return Err("temporal_expected_token_changed".into());
            }
            if head != intent.expected && !(repair && head.as_ref() == Some(&target)) {
                return Err("temporal_exact_pending_repair_required".into());
            }
        }
        if already && !repair && head.as_ref() == Some(&target) && meta.retired_at_ms.is_none() {
            if self.temporal_cleanup_pending(dir, &c, meta)? {
                return Err("temporal_cleanup_repair_required".into());
            }
            let committed = self
                .temporal_current(dir, now_ns)?
                .ok_or("temporal_committed_readback")?;
            if let (Some(a), Some(b)) = (committed.prefix, prefix) {
                if a.bytes() != b.bytes() {
                    return Err("temporal_noop_conflict".into());
                }
            }
            return Ok(target);
        }
        if !already {
            if let (Some(old), Some(p)) = (c.latest.as_ref(), prefix) {
                if old.retired_at_ms.is_none() {
                    let previous = self.temporal_payload(dir, old, now_ns)?;
                    validate_extension(&previous, p)?;
                } else if !is_new_empty_run(p)? {
                    return Err("temporal_new_run_not_empty".into());
                }
            }
        }
        if !already {
            if let Some(slot) = meta.slot {
                if let Some((pending, _)) =
                    self.temporal_payload_any(&dir.join(format!("slot-{slot}.bin")), now_ns)?
                {
                    let candidate = pending.previous_sha256.as_ref()
                        == meta.previous_sha256.as_ref()
                        && pending.generation == meta.generation
                        && pending.revision == meta.revision;
                    if candidate && pending != *meta {
                        return Err("temporal_conflicting_pending_payload".into());
                    }
                }
            }
        }
        // A repair removes only known staging artifacts under the stable lock; its
        // immutable SAME intended metadata/payload is re-established through barriers.
        if repair {
            for name in ["payload.stage", "head.stage", "witness.stage"] {
                if self
                    .temporal_read(&dir.join(name), MAX_RECOVERY_BYTES + SMALL + 48)?
                    .is_some()
                {
                    let matching = if name == "payload.stage" {
                        self.temporal_payload_any(&dir.join(name), now_ns)?
                            .is_some_and(|(m, _)| m == *meta)
                    } else {
                        self.temporal_meta(
                            &dir.join(name),
                            if name == "head.stage" {
                                "head"
                            } else {
                                "witness"
                            },
                        )?
                        .as_ref()
                            == Some(meta)
                    };
                    if !matching {
                        return Err("temporal_conflicting_pending_stage".into());
                    }
                    fs::remove_file(dir.join(name)).map_err(|_| "temporal_repair_stage_cleanup")?;
                }
            }
            File::open(dir)
                .and_then(|f| f.sync_all())
                .map_err(|_| "temporal_repair_directory_fsync")?;
        }
        if let Some(p) = prefix {
            let cipher = self.temporal_payload_seal(meta, p)?;
            self.temporal_write_stage(dir, "payload.stage", &cipher, hook)?;
            self.temporal_replace(
                dir,
                "payload.stage",
                &format!("slot-{}.bin", meta.slot.ok_or("temporal_slot_missing")?),
                hook,
            )?;
            hook("before:payload:readback")?;
            let actual = self.temporal_payload(dir, meta, now_ns)?;
            if actual.bytes() != p.bytes() {
                return Err("temporal_payload_readback".into());
            }
            hook("after:payload:readback")?;
        }
        let plain = serde_json::to_vec(meta).map_err(|_| "temporal_metadata_encoding")?;
        if plain.len() > SMALL {
            return Err("temporal_metadata_budget".into());
        }
        let head_cipher = self.temporal_seal("head", &plain)?;
        self.temporal_write_stage(dir, "head.stage", &head_cipher, hook)?;
        self.temporal_replace(dir, "head.stage", HEAD, hook)?;
        hook("before:head:readback")?;
        if self.temporal_meta(&dir.join(HEAD), "head")?.as_ref() != Some(meta) {
            return Err("temporal_head_readback".into());
        }
        hook("after:head:readback")?;
        let witness = dir.join(meta.witness());
        if let Some(existing) = self.temporal_meta(&witness, "witness")? {
            if existing != *meta {
                return Err("temporal_conflicting_committed_witness".into());
            }
            private_existing_file(&witness, false)?
                .sync_all()
                .map_err(|_| "temporal_witness_fsync")?;
            File::open(dir)
                .and_then(|f| f.sync_all())
                .map_err(|_| "temporal_witness_directory_fsync")?;
        } else {
            let cipher = self.temporal_seal("witness", &plain)?;
            self.temporal_write_stage(dir, "witness.stage", &cipher, hook)?;
            hook("before:witness:install")?;
            fs::hard_link(dir.join("witness.stage"), &witness)
                .map_err(|_| "temporal_witness_install")?;
            hook("after:witness:install")?;
            File::open(dir)
                .and_then(|f| f.sync_all())
                .map_err(|_| "temporal_witness_directory_fsync")?;
            hook("after:witness:directory_fsync")?;
            fs::remove_file(dir.join("witness.stage"))
                .map_err(|_| "temporal_witness_stage_cleanup")?;
        }
        hook("before:committed:readback")?;
        let loaded = self
            .temporal_current(dir, now_ns)?
            .ok_or("temporal_committed_readback")?;
        if loaded.token != target {
            return Err("temporal_committed_readback".into());
        }
        hook("after:committed:readback")?;
        hook("before:cleanup")?;
        self.temporal_cleanup(dir, meta)?;
        hook("after:cleanup")?;
        Ok(target)
    }
}

#[cfg(test)]
#[path = "temporal_recovery_commitment_tests.rs"]
mod tests;

fn private_existing_file(path: &Path, write: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(write);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(target_os = "macos")]
        options.custom_flags(0x100);
        #[cfg(target_os = "linux")]
        options.custom_flags(0x20000);
    }
    let file = options.open(path).map_err(|_| "temporal_private_open")?;
    let m = file.metadata().map_err(|_| "temporal_private_metadata")?;
    if !m.is_file() {
        return Err("temporal_private_file_required".into());
    }
    private_metadata(&m, false)?;
    Ok(file)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AbortArtifact {
    PayloadStage,
    HeadStage,
    WitnessStage,
    InactiveSlot(u8),
}
impl AbortArtifact {
    fn name(&self) -> String {
        match self {
            Self::PayloadStage => "payload.stage".into(),
            Self::HeadStage => "head.stage".into(),
            Self::WitnessStage => "witness.stage".into(),
            Self::InactiveSlot(slot) => format!("slot-{slot}.bin"),
        }
    }
}
/// Private explicit-abort snapshot, never a caller-provided path or grant.
#[derive(Clone)]
pub(crate) struct TemporalAbortTicket {
    expected: TemporalToken,
    artifact: AbortArtifact,
    digest: String,
}
impl RecoveryStore {
    fn temporal_abortable(
        &self,
        dir: &Path,
        c: &Components,
        artifact: &AbortArtifact,
        bytes: &[u8],
        now_ns: u64,
    ) -> Result<bool, String> {
        let head = c.head.as_ref().ok_or("temporal_head_missing")?;
        if head.retired_at_ms.is_some() {
            return Ok(false);
        }
        if let AbortArtifact::InactiveSlot(slot) = artifact {
            if *slot > 1 || Some(*slot) == head.slot {
                return Ok(false);
            }
        }
        let digest = hash(bytes);
        // Both exact ciphertext copies/hardlinks and independently encrypted records
        // naming a committed semantic identity are protected from ABORT.
        if self
            .temporal_read(&dir.join(HEAD), SMALL + 36)?
            .is_some_and(|b| hash(&b) == digest)
        {
            return Ok(false);
        }
        for (name, m) in &c.witnesses {
            if self
                .temporal_read(&dir.join(name), SMALL + 36)?
                .is_some_and(|b| hash(&b) == digest)
            {
                return Ok(false);
            }
            if let Some(slot) = m.slot {
                let file = dir.join(format!("slot-{slot}.bin"));
                if let Ok(Some((actual, _))) = self.temporal_payload_any(&file, now_ns) {
                    if actual.sha()? == m.sha()?
                        && self
                            .temporal_read(&file, MAX_RECOVERY_BYTES + SMALL + 48)?
                            .is_some_and(|b| hash(&b) == digest)
                    {
                        return Ok(false);
                    }
                }
            }
        }
        let identity = match artifact {
            AbortArtifact::PayloadStage | AbortArtifact::InactiveSlot(_) => self
                .temporal_payload_any(&dir.join(artifact.name()), now_ns)
                .ok()
                .flatten()
                .map(|(m, _)| m),
            AbortArtifact::HeadStage => self
                .temporal_meta(&dir.join(artifact.name()), "head")
                .ok()
                .flatten(),
            AbortArtifact::WitnessStage => self
                .temporal_meta(&dir.join(artifact.name()), "witness")
                .ok()
                .flatten(),
        };
        if let Some(m) = identity {
            if c.witnesses
                .iter()
                .any(|(_, w)| w.sha().ok() == m.sha().ok())
            {
                return Ok(false);
            }
            if matches!(artifact, AbortArtifact::InactiveSlot(_))
                && (m.previous_sha256.as_ref() != Some(&head.sha()?)
                    || m.generation != head.generation
                    || m.revision != head.revision + 1)
            {
                return Ok(false);
            }
        } else if matches!(artifact, AbortArtifact::InactiveSlot(_)) {
            return Ok(false);
        }
        Ok(true)
    }
    pub(crate) fn inspect_abortable_temporal(
        &self,
        expected: &TemporalToken,
        now_ns: u64,
    ) -> Result<Vec<TemporalAbortTicket>, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(false)?;
        let loaded = self
            .temporal_current(&locked.directory, now_ns)?
            .ok_or("temporal_committed_prefix_missing")?;
        if loaded.token != *expected || loaded.prefix.is_none() {
            return Err("temporal_abort_expected_changed".into());
        }
        let c = self.temporal_components(&locked.directory)?;
        let slot = c
            .head
            .as_ref()
            .and_then(|m| m.slot)
            .ok_or("temporal_committed_slot_missing")?;
        let mut out = Vec::new();
        for artifact in [
            AbortArtifact::PayloadStage,
            AbortArtifact::HeadStage,
            AbortArtifact::WitnessStage,
            AbortArtifact::InactiveSlot(1 - slot),
        ] {
            if let Some(bytes) = self.temporal_read(
                &locked.directory.join(artifact.name()),
                MAX_RECOVERY_BYTES + SMALL + 48,
            )? {
                if self.temporal_abortable(&locked.directory, &c, &artifact, &bytes, now_ns)? {
                    out.push(TemporalAbortTicket {
                        expected: expected.clone(),
                        artifact,
                        digest: hash(&bytes),
                    });
                }
            }
        }
        Ok(out)
    }
    pub(crate) fn abort_temporal_pending(
        &self,
        ticket: &TemporalAbortTicket,
        now_ns: u64,
    ) -> Result<TemporalLoaded, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(false)?;
        let before = self
            .temporal_current(&locked.directory, now_ns)?
            .ok_or("temporal_committed_prefix_missing")?;
        if before.token != ticket.expected || before.prefix.is_none() {
            return Err("temporal_abort_expected_changed".into());
        }
        let c = self.temporal_components(&locked.directory)?;
        let path = locked.directory.join(ticket.artifact.name());
        if let Some(bytes) = self.temporal_read(&path, MAX_RECOVERY_BYTES + SMALL + 48)? {
            if hash(&bytes) != ticket.digest {
                return Err("temporal_abort_artifact_changed".into());
            }
            if !self.temporal_abortable(&locked.directory, &c, &ticket.artifact, &bytes, now_ns)? {
                return Err("temporal_abort_committed_reference".into());
            }
            fs::remove_file(&path).map_err(|_| "temporal_abort_remove")?;
        }
        File::open(&locked.directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| "temporal_abort_directory_fsync")?;
        let after = self
            .temporal_current(&locked.directory, now_ns)?
            .ok_or("temporal_abort_committed_readback")?;
        if after.token != before.token
            || after.prefix.as_ref().map(|p| p.bytes()) != before.prefix.as_ref().map(|p| p.bytes())
            || path.exists()
        {
            return Err("temporal_abort_committed_changed".into());
        }
        Ok(after)
    }
    #[cfg(test)]
    pub(crate) fn synthetic_partial_write(
        &self,
        intent: &TemporalIntent,
        prefix: &ValidatedTemporalPrefix,
        now_ns: u64,
    ) -> Result<TemporalToken, String> {
        let path = self.temporal_dir().join("payload.stage");
        self.publish_temporal_with(intent, Some(prefix), now_ns, false, &mut |point| {
            if point == "after:payload.stage:write" {
                let mut bytes = fs::read(&path).unwrap();
                bytes.truncate(bytes.len() / 2);
                fs::write(&path, bytes).unwrap();
                Err("injected-partial-native-prefix-write".into())
            } else {
                Ok(())
            }
        })
    }
}
impl TemporalAbortTicket {
    pub(crate) fn expected_token(&self) -> &TemporalToken {
        &self.expected
    }
}

/// Metadata-only erasure capability: deliberately not a TemporalToken accepted
/// by active commit/resume. Neither expired sample bytes nor fit authority escape.
pub(crate) struct TemporalRetirementInspection {
    descriptor: RecoveryDescriptor,
    run: String,
    observed_ns: u64,
    expired: bool,
    token: TemporalToken,
}
impl TemporalRetirementInspection {
    pub(crate) fn descriptor(&self) -> &RecoveryDescriptor {
        &self.descriptor
    }
    pub(crate) fn expired(&self) -> bool {
        self.expired
    }
    pub(crate) fn run(&self) -> &str {
        &self.run
    }
}
/// Only explicit recertification constructs this durability-bearing result.
pub(crate) struct TemporalRestored {
    pub(crate) token: TemporalToken,
    pub(crate) prefix: ValidatedTemporalPrefix,
}
impl RecoveryStore {
    pub(crate) fn inspect_temporal_retirement(
        &self,
        now_ns: u64,
    ) -> Result<Option<TemporalRetirementInspection>, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(false)?;
        let c = self.temporal_components(&locked.directory)?;
        match (c.head, c.latest) {
            (None, None) => Ok(None),
            (Some(head), Some(latest)) if head == latest => {
                if now_ns < head.receiver_ns {
                    return Err("recovery_receiver_clock_regression".into());
                }
                Ok(Some(TemporalRetirementInspection {
                    descriptor: head.descriptor.clone(),
                    run: head.run.clone(),
                    observed_ns: now_ns,
                    expired: now_ns / 1_000_000 >= head.descriptor.expires_at_ms,
                    token: head.token()?,
                }))
            }
            _ => Err("temporal_head_witness_mismatch_or_pending".into()),
        }
    }
    pub(crate) fn prepare_inspected_retirement(
        &self,
        inspection: &TemporalRetirementInspection,
        now_ns: u64,
        expiry_only: bool,
    ) -> Result<TemporalIntent, String> {
        if now_ns < inspection.observed_ns {
            return Err("recovery_receiver_clock_regression".into());
        }
        self.prepare_retire_temporal(&inspection.token, now_ns, expiry_only)
    }
    pub(crate) fn fresh_train_genesis_predecessor(&self,now_ns:u64)->Result<Option<TemporalToken>,String>{
        let _serial=self.serial.lock().map_err(|_|"recovery_store_unavailable")?;
        let locked=self.temporal_lock(false)?;
        let c=self.temporal_components(&locked.directory)?;
        match (c.head,c.latest){
            (None,None)=>Ok(None),
            (Some(head),Some(latest)) if head==latest=>{
                if now_ns<head.receiver_ns || now_ns/1_000_000<head.descriptor.saved_at_ms || head.retired_at_ms.is_some_and(|t|now_ns/1_000_000<t){return Err("recovery_clock_regression".into());}
                if head.retired_at_ms.is_none(){return Err("temporal_active_generation_requires_retirement".into());}
                Ok(Some(head.token()?))
            },_=>Err("temporal_head_witness_mismatch_or_pending".into()),
        }
    }
    pub(crate) fn recertify_temporal(
        &self,
        expected: &TemporalToken,
        now_ns: u64,
    ) -> Result<TemporalRestored, String> {
        self.recertify_temporal_with(expected, now_ns, &mut |_| Ok(()))
    }
    fn recertify_temporal_with(
        &self,
        expected: &TemporalToken,
        now_ns: u64,
        hook: &mut impl FnMut(&str) -> Result<(), String>,
    ) -> Result<TemporalRestored, String> {
        let _serial = self
            .serial
            .lock()
            .map_err(|_| "recovery_store_unavailable")?;
        let locked = self.temporal_lock(false)?;
        let before = self
            .temporal_current(&locked.directory, now_ns)?
            .ok_or("temporal_committed_prefix_missing")?;
        if before.token != *expected || before.prefix.is_none() {
            return Err("temporal_restore_expected_changed".into());
        }
        let components = self.temporal_components(&locked.directory)?;
        let head = components.head.as_ref().ok_or("temporal_head_missing")?;
        let mut names = vec![
            HEAD.to_string(),
            format!("slot-{}.bin", head.slot.ok_or("temporal_retired")?),
        ];
        names.extend(
            components
                .witnesses
                .iter()
                .filter(|(_, m)| m.generation == head.generation)
                .map(|(n, _)| n.clone()),
        );
        for name in names {
            hook("before:restore:file_fsync")?;
            private_existing_file(&locked.directory.join(name), false)?
                .sync_all()
                .map_err(|_| "temporal_restore_file_fsync")?;
            hook("after:restore:file_fsync")?;
        }
        hook("before:restore:directory_fsync")?;
        File::open(&locked.directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| "temporal_restore_directory_fsync")?;
        File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| "temporal_restore_parent_fsync")?;
        hook("after:restore:directory_fsync")?;
        hook("before:restore:readback")?;
        let after = self
            .temporal_current(&locked.directory, now_ns)?
            .ok_or("temporal_restore_readback")?;
        if after.token != before.token
            || after.prefix.as_ref().map(|p| p.bytes()) != before.prefix.as_ref().map(|p| p.bytes())
        {
            return Err("temporal_restore_readback_changed".into());
        }
        hook("after:restore:readback")?;
        Ok(TemporalRestored {
            token: after.token,
            prefix: after.prefix.ok_or("temporal_retired")?,
        })
    }
}

impl RecoveryStore {
    #[cfg(test)]
    pub(crate) fn synthetic_lost_witness_ack(
        &self,
        intent: &TemporalIntent,
        prefix: &ValidatedTemporalPrefix,
        now_ns: u64,
    ) -> Result<TemporalToken, String> {
        self.publish_temporal_with(intent, Some(prefix), now_ns, false, &mut |point| {
            if point == "after:witness:install" {
                Err("injected-lost-witness-directory-ack".into())
            } else {
                Ok(())
            }
        })
    }
}
