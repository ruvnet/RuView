//! Firmware registry for pull-based OTA.
//!
//! Holds the currently-blessed ESP32 firmware binary in memory, along with
//! its SHA-256 hash and version metadata. Nodes poll the server to discover
//! whether an update is available and can then download the binary via HTTP.
//!
//! Workflow:
//! 1. Operator uploads a new firmware binary via `POST /api/v1/firmware/upload`.
//! 2. Server computes SHA-256, parses version from filename/sidecar, and stores
//!    it on disk at the configured firmware path.
//! 3. Server loads the metadata into the in-memory registry.
//! 4. Nodes `GET /api/v1/firmware/latest` periodically. Response includes
//!    version, sha256, size, and download_url.
//! 5. If a node's running version differs, it calls `GET /api/v1/firmware/download`
//!    to fetch the bytes and applies them via the existing ota_update handler.
//!
//! This module is deliberately transport-agnostic: HTTP handlers live in
//! `main.rs`. The registry provides pure data and file I/O helpers.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Errors that can arise while loading or registering firmware.
#[derive(Debug, thiserror::Error)]
pub enum FirmwareRegistryError {
    /// File not found at the given path.
    #[error("firmware file not found: {0}")]
    NotFound(PathBuf),

    /// I/O error reading the file.
    #[error("firmware I/O error: {0}")]
    Io(String),

    /// Could not parse a version from the filename or sidecar manifest.
    #[error("could not parse firmware version from {0}")]
    VersionParse(String),

    /// File is empty or smaller than the minimum expected binary size.
    #[error("firmware file too small ({size} bytes, need >= {min})")]
    TooSmall { size: u64, min: u64 },
}

impl From<std::io::Error> for FirmwareRegistryError {
    fn from(err: std::io::Error) -> Self {
        FirmwareRegistryError::Io(err.to_string())
    }
}

/// Metadata describing a single firmware binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareMetadata {
    /// Semver-ish version string (e.g. "0.8.0-watchdog").
    pub version: String,
    /// Lowercase hex SHA-256 of the binary bytes.
    pub sha256: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Absolute path to the binary on disk.
    pub file_path: PathBuf,
    /// Optional human-readable compile time (ISO-8601 or whatever was in the
    /// sidecar manifest). None if not known.
    pub compile_time: Option<String>,
}

/// Optional sidecar manifest shipped next to a firmware binary. If a file
/// `<name>.manifest.json` exists alongside the binary, its contents are
/// loaded and used to populate version/compile_time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FirmwareManifest {
    /// Explicit version override. Takes precedence over filename parsing.
    pub version: Option<String>,
    /// Optional compile-time string.
    pub compile_time: Option<String>,
}

/// In-memory registry of the current firmware.
#[derive(Debug, Clone, Default)]
pub struct FirmwareRegistry {
    current: Option<FirmwareMetadata>,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// Minimum plausible firmware size — anything smaller is rejected as
/// corrupt/truncated (ESP32-S3 app binaries are always >> 256 KB).
const MIN_FIRMWARE_SIZE_BYTES: u64 = 256 * 1024;

impl FirmwareRegistry {
    /// Create an empty registry with no current firmware loaded.
    pub fn new() -> Self {
        Self { current: None }
    }

    /// Return the currently-blessed firmware metadata, if any.
    pub fn current(&self) -> Option<&FirmwareMetadata> {
        self.current.as_ref()
    }

    /// Clear the current firmware.
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// Set the current firmware from a file on disk.
    ///
    /// Reads the file, computes SHA-256, and parses the version from either:
    /// 1. A sidecar `<name>.manifest.json` file, or
    /// 2. The filename itself (looking for a substring like `-0.8.0.bin`).
    ///
    /// Returns the resulting metadata on success.
    pub fn set_current<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<FirmwareMetadata, FirmwareRegistryError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(FirmwareRegistryError::NotFound(path.to_path_buf()));
        }

        let metadata = fs::metadata(path)?;
        let size_bytes = metadata.len();
        if size_bytes < MIN_FIRMWARE_SIZE_BYTES {
            return Err(FirmwareRegistryError::TooSmall {
                size: size_bytes,
                min: MIN_FIRMWARE_SIZE_BYTES,
            });
        }

        let sha256 = sha256_file(path)?;
        let (version, compile_time) = resolve_version(path)?;

        let meta = FirmwareMetadata {
            version,
            sha256,
            size_bytes,
            file_path: path.to_path_buf(),
            compile_time,
        };
        self.current = Some(meta.clone());
        Ok(meta)
    }

    /// Check whether the registry considers a given running version to be
    /// out-of-date. Returns `true` if the current firmware version is non-None
    /// and differs from `running_version`. Nodes that don't report a version
    /// pass `None` and are always told to update.
    pub fn is_update_available(&self, running_version: Option<&str>) -> bool {
        match (&self.current, running_version) {
            (Some(cur), Some(running)) => cur.version != running,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the lowercase-hex SHA-256 of a file on disk.
fn sha256_file(path: &Path) -> Result<String, FirmwareRegistryError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

/// Compute SHA-256 of an in-memory byte slice (lowercase hex).
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

/// Minimal hex encoder — avoids pulling in an extra crate.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Resolve firmware version from either a sidecar manifest or the filename.
///
/// Sidecar priority: `<path>.manifest.json` — if present, its `version` field
/// wins. Otherwise, parse the filename for a version token like `0.8.0` or
/// `0.8.0-watchdog`. If neither yields a version, return an error.
fn resolve_version(
    path: &Path,
) -> Result<(String, Option<String>), FirmwareRegistryError> {
    // 1. Sidecar manifest.
    let mut manifest_path = path.as_os_str().to_os_string();
    manifest_path.push(".manifest.json");
    let manifest_path = PathBuf::from(manifest_path);

    if manifest_path.exists() {
        if let Ok(bytes) = fs::read(&manifest_path) {
            if let Ok(m) = serde_json::from_slice::<FirmwareManifest>(&bytes) {
                if let Some(v) = m.version {
                    return Ok((v, m.compile_time));
                }
            }
        }
    }

    // 2. Filename parsing — expects patterns like:
    //    esp32-csi-node-0.8.0.bin
    //    esp32-csi-node-0.8.0-watchdog.bin
    //    current-0.8.0.bin
    //    0.8.0.bin
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if let Some(v) = parse_version_from_filename(filename) {
        return Ok((v, None));
    }

    Err(FirmwareRegistryError::VersionParse(filename.to_string()))
}

/// Extract a semver-ish version token from a firmware filename stem.
///
/// Looks for the first run of digits containing at least one dot, optionally
/// followed by a `-suffix`. Examples:
///   "esp32-csi-node-0.8.0"         → "0.8.0"
///   "esp32-csi-node-0.8.0-watchdog" → "0.8.0-watchdog"
///   "current-0.8.0-rc1"             → "0.8.0-rc1"
///   "esp32-csi-node"                → None
fn parse_version_from_filename(stem: &str) -> Option<String> {
    // Find the first character that starts a digit-dot pattern.
    let bytes = stem.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            // Scan forward while we see digits, dots, then optionally a
            // single dash followed by [alphanumeric | dot].
            let start = i;
            let mut saw_dot = false;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                if bytes[i] == b'.' {
                    saw_dot = true;
                }
                i += 1;
            }
            if saw_dot {
                // Extend into an optional pre-release suffix like "-rc1" or "-watchdog"
                if i < bytes.len() && bytes[i] == b'-' {
                    let mut j = i + 1;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'.' || bytes[j] == b'-')
                    {
                        j += 1;
                    }
                    if j > i + 1 {
                        return Some(stem[start..j].to_string());
                    }
                }
                return Some(stem[start..i].to_string());
            }
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_fake_firmware(path: &Path, size: usize) {
        let mut f = fs::File::create(path).unwrap();
        let chunk = vec![0xABu8; 4096];
        let mut written = 0;
        while written < size {
            let n = chunk.len().min(size - written);
            f.write_all(&chunk[..n]).unwrap();
            written += n;
        }
    }

    #[test]
    fn test_sha256_bytes_known_vector() {
        // SHA-256 of "abc" = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let s = sha256_bytes(b"abc");
        assert_eq!(
            s,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(hex_encode(&[]), "");
        assert_eq!(hex_encode(&[0x00, 0xff]), "00ff");
    }

    #[test]
    fn test_parse_version_from_filename() {
        assert_eq!(
            parse_version_from_filename("esp32-csi-node-0.8.0"),
            Some("0.8.0".to_string())
        );
        assert_eq!(
            parse_version_from_filename("esp32-csi-node-0.8.0-watchdog"),
            Some("0.8.0-watchdog".to_string())
        );
        assert_eq!(
            parse_version_from_filename("current-0.8.0-rc1"),
            Some("0.8.0-rc1".to_string())
        );
        assert_eq!(
            parse_version_from_filename("0.8.0"),
            Some("0.8.0".to_string())
        );
        assert_eq!(parse_version_from_filename("esp32-csi-node"), None);
    }

    #[test]
    fn test_set_current_with_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("esp32-csi-node.bin");
        write_fake_firmware(&bin_path, 300 * 1024);

        let manifest_path = dir.path().join("esp32-csi-node.bin.manifest.json");
        let manifest = r#"{"version":"0.9.0-test","compile_time":"2026-04-09T17:00:00Z"}"#;
        fs::write(&manifest_path, manifest).unwrap();

        let mut registry = FirmwareRegistry::new();
        let meta = registry.set_current(&bin_path).unwrap();
        assert_eq!(meta.version, "0.9.0-test");
        assert_eq!(meta.compile_time.as_deref(), Some("2026-04-09T17:00:00Z"));
        assert_eq!(meta.size_bytes, 300 * 1024);
        assert_eq!(meta.sha256.len(), 64);
    }

    #[test]
    fn test_set_current_with_filename_version() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("esp32-csi-node-0.8.0-watchdog.bin");
        write_fake_firmware(&bin_path, 300 * 1024);

        let mut registry = FirmwareRegistry::new();
        let meta = registry.set_current(&bin_path).unwrap();
        assert_eq!(meta.version, "0.8.0-watchdog");
        assert!(meta.compile_time.is_none());
    }

    #[test]
    fn test_set_current_rejects_too_small() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("tiny-0.1.0.bin");
        write_fake_firmware(&bin_path, 1024);

        let mut registry = FirmwareRegistry::new();
        let err = registry.set_current(&bin_path).unwrap_err();
        assert!(matches!(err, FirmwareRegistryError::TooSmall { .. }));
    }

    #[test]
    fn test_set_current_rejects_missing_file() {
        let mut registry = FirmwareRegistry::new();
        let err = registry.set_current("/nonexistent/firmware.bin").unwrap_err();
        assert!(matches!(err, FirmwareRegistryError::NotFound(_)));
    }

    #[test]
    fn test_set_current_rejects_unparseable_version() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("esp32-csi-node.bin");
        write_fake_firmware(&bin_path, 300 * 1024);

        let mut registry = FirmwareRegistry::new();
        let err = registry.set_current(&bin_path).unwrap_err();
        assert!(matches!(err, FirmwareRegistryError::VersionParse(_)));
    }

    #[test]
    fn test_is_update_available() {
        let mut registry = FirmwareRegistry::new();
        // Empty registry never offers updates
        assert!(!registry.is_update_available(Some("0.1.0")));
        assert!(!registry.is_update_available(None));

        // Seed with a known version
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("fw-0.8.0.bin");
        write_fake_firmware(&bin_path, 300 * 1024);
        registry.set_current(&bin_path).unwrap();

        // Same version -> no update
        assert!(!registry.is_update_available(Some("0.8.0")));
        // Different version -> update
        assert!(registry.is_update_available(Some("0.7.0")));
        // Unknown version -> update (safest assumption)
        assert!(registry.is_update_available(None));
    }

    #[test]
    fn test_sha256_file_matches_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("fw-0.1.0.bin");
        let size = 300 * 1024;
        write_fake_firmware(&bin_path, size);

        let mut registry = FirmwareRegistry::new();
        let meta = registry.set_current(&bin_path).unwrap();

        let bytes = fs::read(&bin_path).unwrap();
        let direct = sha256_bytes(&bytes);
        assert_eq!(meta.sha256, direct);
    }

    #[test]
    fn test_clear() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("fw-0.1.0.bin");
        write_fake_firmware(&bin_path, 300 * 1024);

        let mut registry = FirmwareRegistry::new();
        registry.set_current(&bin_path).unwrap();
        assert!(registry.current().is_some());
        registry.clear();
        assert!(registry.current().is_none());
    }
}
