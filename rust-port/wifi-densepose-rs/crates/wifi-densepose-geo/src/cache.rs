//! Disk cache for tiles, DEM, and OSM data.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct TileCache {
    pub base_dir: PathBuf,
}

impl TileCache {
    pub fn new(base_dir: &str) -> Self {
        let expanded = base_dir.replace('~', &std::env::var("HOME").unwrap_or_default());
        let path = PathBuf::from(expanded);
        let _ = std::fs::create_dir_all(&path);
        Self { base_dir: path }
    }

    pub fn default_cache() -> Self {
        Self::new("~/.local/share/ruview/geo-cache")
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.key_path(key);
        std::fs::read(&path).ok()
    }

    pub fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.key_path(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, data)?;
        Ok(())
    }

    pub fn has(&self, key: &str) -> bool {
        self.key_path(key).exists()
    }

    pub fn size_bytes(&self) -> u64 {
        walkdir(self.base_dir.as_path())
    }

    fn key_path(&self, key: &str) -> PathBuf {
        // Sanitize key to prevent path traversal
        let safe_key = key.replace("..", "_").replace('/', "_");
        self.base_dir.join(safe_key)
    }
}

fn walkdir(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| {
            if e.path().is_dir() { walkdir(&e.path()) }
            else { e.metadata().map(|m| m.len()).unwrap_or(0) }
        })
        .sum()
}
