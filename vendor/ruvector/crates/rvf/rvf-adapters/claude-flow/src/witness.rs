//! Audit trail using WITNESS_SEG for claude-flow memory operations.
//!
//! Wraps `rvf_crypto::witness` to provide a persistent, append-only
//! witness chain that records every memory store/delete/search action.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rvf_crypto::witness::{WitnessEntry, create_witness_chain, verify_witness_chain};
use rvf_crypto::shake256_256;

/// Witness type constants for claude-flow actions.
pub const WITNESS_STORE: u8 = 0x01;
pub const WITNESS_DELETE: u8 = 0x02;
pub const WITNESS_SEARCH: u8 = 0x03;
pub const WITNESS_COMPACT: u8 = 0x04;

/// Persistent witness chain that records memory operations.
pub struct WitnessChain {
    path: PathBuf,
    /// Cached chain bytes (in-memory mirror of the file).
    chain_data: Vec<u8>,
    /// Number of entries in the chain.
    entry_count: usize,
}

impl WitnessChain {
    /// Create a new (empty) witness chain file at the given path.
    pub fn create(path: &Path) -> Result<Self, WitnessError> {
        File::create(path).map_err(|e| WitnessError::Io(e.to_string()))?;
        Ok(Self {
            path: path.to_path_buf(),
            chain_data: Vec::new(),
            entry_count: 0,
        })
    }

    /// Open an existing witness chain file, verifying its integrity.
    pub fn open(path: &Path) -> Result<Self, WitnessError> {
        let mut file = File::open(path).map_err(|e| WitnessError::Io(e.to_string()))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| WitnessError::Io(e.to_string()))?;

        if data.is_empty() {
            return Ok(Self {
                path: path.to_path_buf(),
                chain_data: Vec::new(),
                entry_count: 0,
            });
        }

        let entries = verify_witness_chain(&data)
            .map_err(|_| WitnessError::ChainCorrupted)?;

        Ok(Self {
            path: path.to_path_buf(),
            chain_data: data,
            entry_count: entries.len(),
        })
    }

    /// Open an existing chain or create a new one.
    pub fn open_or_create(path: &Path) -> Result<Self, WitnessError> {
        if path.exists() {
            Self::open(path)
        } else {
            Self::create(path)
        }
    }

    /// Record a memory store action.
    pub fn record_store(&mut self, key: &str, namespace: &str) -> Result<(), WitnessError> {
        let mut hasher_input = Vec::new();
        hasher_input.extend_from_slice(b"store:");
        hasher_input.extend_from_slice(namespace.as_bytes());
        hasher_input.push(b'/');
        hasher_input.extend_from_slice(key.as_bytes());
        self.append_entry(&hasher_input, WITNESS_STORE)
    }

    /// Record a memory delete action.
    pub fn record_delete(&mut self, key: &str, namespace: &str) -> Result<(), WitnessError> {
        let mut hasher_input = Vec::new();
        hasher_input.extend_from_slice(b"delete:");
        hasher_input.extend_from_slice(namespace.as_bytes());
        hasher_input.push(b'/');
        hasher_input.extend_from_slice(key.as_bytes());
        self.append_entry(&hasher_input, WITNESS_DELETE)
    }

    /// Record a search action.
    pub fn record_search(&mut self, namespace: &str, k: usize) -> Result<(), WitnessError> {
        let mut hasher_input = Vec::new();
        hasher_input.extend_from_slice(b"search:");
        hasher_input.extend_from_slice(namespace.as_bytes());
        hasher_input.push(b':');
        hasher_input.extend_from_slice(k.to_string().as_bytes());
        self.append_entry(&hasher_input, WITNESS_SEARCH)
    }

    /// Record a compaction action.
    pub fn record_compact(&mut self) -> Result<(), WitnessError> {
        self.append_entry(b"compact", WITNESS_COMPACT)
    }

    /// Verify the entire chain is intact.
    pub fn verify(&self) -> Result<usize, WitnessError> {
        if self.chain_data.is_empty() {
            return Ok(0);
        }
        let entries = verify_witness_chain(&self.chain_data)
            .map_err(|_| WitnessError::ChainCorrupted)?;
        Ok(entries.len())
    }

    /// Return the number of entries in the chain.
    pub fn len(&self) -> usize {
        self.entry_count
    }

    /// Return whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    // ── Internal ──────────────────────────────────────────────────────

    fn append_entry(&mut self, action_data: &[u8], witness_type: u8) -> Result<(), WitnessError> {
        let action_hash = shake256_256(action_data);
        let timestamp_ns = now_ns();

        let entry = WitnessEntry {
            prev_hash: [0u8; 32], // create_witness_chain will set this
            action_hash,
            timestamp_ns,
            witness_type,
        };

        // Rebuild the entire chain with the new entry appended.
        // This is correct because create_witness_chain re-links prev_hash.
        let mut all_entries = if self.chain_data.is_empty() {
            Vec::new()
        } else {
            verify_witness_chain(&self.chain_data)
                .map_err(|_| WitnessError::ChainCorrupted)?
        };
        all_entries.push(entry);

        let new_chain = create_witness_chain(&all_entries);

        // Persist atomically: write to temp then rename.
        let tmp_path = self.path.with_extension("bin.tmp");
        {
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|e| WitnessError::Io(e.to_string()))?;
            f.write_all(&new_chain).map_err(|e| WitnessError::Io(e.to_string()))?;
            f.sync_all().map_err(|e| WitnessError::Io(e.to_string()))?;
        }
        std::fs::rename(&tmp_path, &self.path).map_err(|e| WitnessError::Io(e.to_string()))?;

        self.chain_data = new_chain;
        self.entry_count = all_entries.len();
        Ok(())
    }
}

/// Errors from witness chain operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WitnessError {
    /// I/O error (stringified for Clone/Eq compatibility).
    Io(String),
    /// Chain integrity verification failed.
    ChainCorrupted,
}

impl std::fmt::Display for WitnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "witness I/O error: {msg}"),
            Self::ChainCorrupted => write!(f, "witness chain integrity check failed"),
        }
    }
}

impl std::error::Error for WitnessError {}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_and_open_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("witness.bin");

        let chain = WitnessChain::create(&path).unwrap();
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());

        let reopened = WitnessChain::open(&path).unwrap();
        assert_eq!(reopened.len(), 0);
    }

    #[test]
    fn record_and_verify() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("witness.bin");

        let mut chain = WitnessChain::create(&path).unwrap();
        chain.record_store("key1", "default").unwrap();
        chain.record_search("default", 5).unwrap();
        chain.record_delete("key1", "default").unwrap();
        assert_eq!(chain.len(), 3);

        let count = chain.verify().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn persistence_across_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("witness.bin");

        {
            let mut chain = WitnessChain::create(&path).unwrap();
            chain.record_store("a", "ns").unwrap();
            chain.record_store("b", "ns").unwrap();
        }

        let chain = WitnessChain::open(&path).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.verify().unwrap(), 2);
    }

    #[test]
    fn tampered_chain_detected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("witness.bin");

        {
            let mut chain = WitnessChain::create(&path).unwrap();
            chain.record_store("x", "ns").unwrap();
            chain.record_store("y", "ns").unwrap();
        }

        // Tamper with the file
        let mut data = std::fs::read(&path).unwrap();
        if data.len() > 40 {
            data[40] ^= 0xFF;
        }
        std::fs::write(&path, &data).unwrap();

        let result = WitnessChain::open(&path);
        assert!(result.is_err() || result.unwrap().verify().is_err());
    }

    #[test]
    fn open_or_create_new() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("witness.bin");

        let chain = WitnessChain::open_or_create(&path).unwrap();
        assert!(chain.is_empty());
    }

    #[test]
    fn open_or_create_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("witness.bin");

        {
            let mut chain = WitnessChain::create(&path).unwrap();
            chain.record_compact().unwrap();
        }

        let chain = WitnessChain::open_or_create(&path).unwrap();
        assert_eq!(chain.len(), 1);
    }
}
