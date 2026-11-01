//! Bounded owner-private metadata discovery. It does not select/adopt authority.
use super::*;
#[derive(Debug, Clone, Serialize)]
pub struct SavedCandidate {
    pub receipt: CandidateReceipt,
    pub context: CaptureContext,
    /// Selection convenience only; never evidence freshness or qualification.
    pub stored_at_unix_ms: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct CandidateCatalog {
    pub candidates: Vec<SavedCandidate>,
    pub unreadable_contexts: usize,
    pub truncated: bool,
}
impl ArtifactStore {
    pub fn catalog(&self) -> Result<CandidateCatalog, String> {
        self.catalog_filtered(None)
    }
    pub fn catalog_for_room(&self, room_digest: &str) -> Result<CandidateCatalog, String> {
        if !bounded_id(room_digest) {
            return Err("invalid catalog room".into());
        }
        self.catalog_filtered(Some(room_digest))
    }
    fn catalog_filtered(&self, room_digest: Option<&str>) -> Result<CandidateCatalog, String> {
        self.verify_owner()?;
        let mut contexts = Vec::new();
        let mut truncated = false;
        for (index, entry) in std::fs::read_dir(&self.root)
            .map_err(|e| e.to_string())?
            .take(257)
            .enumerate()
        {
            if index == 256 {
                truncated = true;
                break;
            }
            let entry = entry.map_err(|e| e.to_string())?;
            if contexts.len() >= 256 {
                truncated = true;
                break;
            }
            let Some(key) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if hex_digest(&key) {
                contexts.push((key, entry.path()));
            }
        }
        let mut result = CandidateCatalog {
            candidates: Vec::new(),
            unreadable_contexts: 0,
            truncated,
        };
        contexts.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, dir) in contexts {
            let candidate = (|| {
                let meta = std::fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
                if !meta.is_dir() || meta.file_type().is_symlink() {
                    return Err("invalid catalog context directory".to_string());
                }
                private_dir(&dir).map_err(|e| e.to_string())?;
                let pointer = dir.join(POINTER);
                let receipt: CandidateReceipt =
                    serde_json::from_slice(&private_read(&pointer).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if receipt.owner_scope != self.owner_scope
                    || receipt.installation_id != self.installation_id
                    || receipt.context_digest != key
                    || !hex_digest(&receipt.model_sha256)
                {
                    return Err("catalog owner/context mismatch".into());
                }
                let envelope: Envelope = serde_json::from_slice(
                    &private_read(&dir.join(format!("{}.json", receipt.model_sha256)))
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let model = RoomModel::import(envelope.learned_model_json.as_bytes())?;
                self.validate_context(model.context())?;
                self.read_model(&dir, &receipt, model.context())?;
                let stored_at_unix_ms = std::fs::symlink_metadata(&pointer)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(0);
                Ok::<_, String>(SavedCandidate {
                    receipt,
                    context: model.context().clone(),
                    stored_at_unix_ms,
                })
            })();
            match candidate {
                Ok(c) if room_digest.is_none_or(|room| c.context.room_digest == room) => {
                    result.candidates.push(c)
                }
                Ok(_) => {}
                Err(_) => result.unreadable_contexts += 1,
            }
        }
        result.candidates.sort_by(|a, b| {
            b.stored_at_unix_ms
                .cmp(&a.stored_at_unix_ms)
                .then(a.receipt.context_digest.cmp(&b.receipt.context_digest))
        });
        if result.candidates.len() > 32 {
            result.candidates.truncate(32);
            result.truncated = true;
        }
        Ok(result)
    }
}
