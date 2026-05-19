//! Cog manifest — see ADR-100 §"manifest.json schema".
//!
//! The `cog-pose-estimation manifest` subcommand emits the embedded spec
//! (no signature fields); the build pipeline post-processes it after
//! computing `binary_sha256` + `binary_signature`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestSpec {
    pub id: String,
    pub version: String,
    pub binary_url: Option<String>,
    pub binary_bytes: Option<u64>,
    pub binary_sha256: Option<String>,
    pub binary_signature: Option<String>,
    pub installed_at: Option<u64>,
    pub status: Option<String>,
}

impl ManifestSpec {
    /// The skeleton emitted by `cog-pose-estimation manifest` before the
    /// release pipeline fills in the signature/hash/url fields.
    pub fn embedded(id: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            version: version.to_string(),
            binary_url: None,
            binary_bytes: None,
            binary_sha256: None,
            binary_signature: None,
            installed_at: None,
            status: None,
        }
    }
}
