//! JSON-file persistence layer for OSpipe data.
//!
//! Provides durable storage of frames, configuration, and embedding data
//! using the local filesystem. All data is serialized to JSON (frames and
//! config) or raw bytes (embeddings) inside a configurable data directory.
//!
//! This module is gated behind `cfg(not(target_arch = "wasm32"))` because
//! WASM targets do not have filesystem access.

use crate::capture::CapturedFrame;
use crate::config::OsPipeConfig;
use crate::error::{OsPipeError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A serializable wrapper around [`CapturedFrame`] for disk persistence.
///
/// This mirrors all fields of `CapturedFrame` but is kept as a distinct
/// type so the persistence format can evolve independently of the
/// in-memory representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFrame {
    /// The captured frame data.
    pub frame: CapturedFrame,
    /// Optional text that was stored after safety-gate processing.
    /// If `None`, the original frame text was used unchanged.
    pub safe_text: Option<String>,
}

/// Filesystem-backed persistence for OSpipe data.
///
/// All files are written inside `data_dir`:
/// - `frames.json` - serialized vector of [`StoredFrame`]
/// - `config.json` - serialized [`OsPipeConfig`]
/// - `embeddings.bin` - raw bytes (e.g. HNSW index serialization)
pub struct PersistenceLayer {
    data_dir: PathBuf,
}

impl PersistenceLayer {
    /// Create a new persistence layer rooted at `data_dir`.
    ///
    /// The directory (and any missing parents) will be created if they
    /// do not already exist.
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&data_dir).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to create data directory {}: {}",
                data_dir.display(),
                e
            ))
        })?;
        Ok(Self { data_dir })
    }

    /// Return the path to a named file inside the data directory.
    fn file_path(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    // ---- Frames ----

    /// Persist a slice of stored frames to `frames.json`.
    pub fn save_frames(&self, frames: &[StoredFrame]) -> Result<()> {
        let path = self.file_path("frames.json");
        let json = serde_json::to_string_pretty(frames)?;
        std::fs::write(&path, json).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to write frames to {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Load stored frames from `frames.json`.
    ///
    /// Returns an empty vector if the file does not exist.
    pub fn load_frames(&self) -> Result<Vec<StoredFrame>> {
        let path = self.file_path("frames.json");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let data = std::fs::read_to_string(&path).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to read frames from {}: {}",
                path.display(),
                e
            ))
        })?;
        let frames: Vec<StoredFrame> = serde_json::from_str(&data)?;
        Ok(frames)
    }

    // ---- Config ----

    /// Persist the pipeline configuration to `config.json`.
    pub fn save_config(&self, config: &OsPipeConfig) -> Result<()> {
        let path = self.file_path("config.json");
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(&path, json).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to write config to {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Load the pipeline configuration from `config.json`.
    ///
    /// Returns `None` if the file does not exist.
    pub fn load_config(&self) -> Result<Option<OsPipeConfig>> {
        let path = self.file_path("config.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&path).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to read config from {}: {}",
                path.display(),
                e
            ))
        })?;
        let config: OsPipeConfig = serde_json::from_str(&data)?;
        Ok(Some(config))
    }

    // ---- Embeddings (raw bytes) ----

    /// Persist raw embedding bytes to `embeddings.bin`.
    ///
    /// This is intended for serializing an HNSW index or other binary
    /// data that does not fit the JSON format.
    pub fn save_embeddings(&self, data: &[u8]) -> Result<()> {
        let path = self.file_path("embeddings.bin");
        std::fs::write(&path, data).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to write embeddings to {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Load raw embedding bytes from `embeddings.bin`.
    ///
    /// Returns `None` if the file does not exist.
    pub fn load_embeddings(&self) -> Result<Option<Vec<u8>>> {
        let path = self.file_path("embeddings.bin");
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(&path).map_err(|e| {
            OsPipeError::Storage(format!(
                "Failed to read embeddings from {}: {}",
                path.display(),
                e
            ))
        })?;
        Ok(Some(data))
    }

    /// Return the data directory path.
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturedFrame;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ospipe_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_frames_roundtrip() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let frame = CapturedFrame::new_screen("VSCode", "main.rs", "fn main() {}", 0);
        let stored = vec![StoredFrame {
            frame,
            safe_text: None,
        }];

        layer.save_frames(&stored).unwrap();
        let loaded = layer.load_frames().unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].frame.text_content(), "fn main() {}");
        assert!(loaded[0].safe_text.is_none());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_frames_empty_when_missing() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let loaded = layer.load_frames().unwrap();
        assert!(loaded.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let config = OsPipeConfig::default();
        layer.save_config(&config).unwrap();

        let loaded = layer.load_config().unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.storage.embedding_dim, 384);
        assert_eq!(loaded.capture.fps, 1.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_none_when_missing() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let loaded = layer.load_config().unwrap();
        assert!(loaded.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_embeddings_roundtrip() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let data: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4];
        layer.save_embeddings(&data).unwrap();

        let loaded = layer.load_embeddings().unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap(), data);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_embeddings_none_when_missing() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let loaded = layer.load_embeddings().unwrap();
        assert!(loaded.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_creates_directory_if_missing() {
        let dir = std::env::temp_dir()
            .join(format!("ospipe_test_{}", uuid::Uuid::new_v4()))
            .join("nested")
            .join("deep");
        assert!(!dir.exists());

        let layer = PersistenceLayer::new(dir.clone());
        assert!(layer.is_ok());
        assert!(dir.exists());

        let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn test_multiple_frames_roundtrip() {
        let dir = temp_dir();
        let layer = PersistenceLayer::new(dir.clone()).unwrap();

        let frames: Vec<StoredFrame> = (0..5)
            .map(|i| StoredFrame {
                frame: CapturedFrame::new_screen(
                    "App",
                    &format!("Window {}", i),
                    &format!("Content {}", i),
                    0,
                ),
                safe_text: if i % 2 == 0 {
                    Some(format!("Redacted {}", i))
                } else {
                    None
                },
            })
            .collect();

        layer.save_frames(&frames).unwrap();
        let loaded = layer.load_frames().unwrap();

        assert_eq!(loaded.len(), 5);
        for (i, sf) in loaded.iter().enumerate() {
            assert_eq!(sf.frame.text_content(), &format!("Content {}", i));
            if i % 2 == 0 {
                assert_eq!(sf.safe_text, Some(format!("Redacted {}", i)));
            } else {
                assert!(sf.safe_text.is_none());
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
