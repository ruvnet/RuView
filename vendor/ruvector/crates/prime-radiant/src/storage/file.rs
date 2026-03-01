//! File-Based Storage Implementation
//!
//! Persistent file storage with write-ahead logging (WAL) for durability.
//! Supports both JSON and bincode serialization formats.
//!
//! # Security
//!
//! All identifiers used in file paths are sanitized to prevent path traversal attacks.
//! Only alphanumeric characters, dashes, underscores, and dots are allowed.

use super::{GovernanceStorage, GraphStorage, StorageConfig, StorageError};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Maximum allowed identifier length for security
const MAX_ID_LENGTH: usize = 256;

/// Validate and sanitize an identifier for use in file paths.
///
/// # Security
///
/// This function prevents path traversal attacks by:
/// - Rejecting empty identifiers
/// - Rejecting identifiers over MAX_ID_LENGTH
/// - Only allowing alphanumeric, dash, underscore, and dot characters
/// - Rejecting "." and ".." path components
/// - Rejecting identifiers starting with a dot (hidden files)
fn validate_path_id(id: &str) -> Result<(), StorageError> {
    if id.is_empty() {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Identifier cannot be empty",
        )));
    }

    if id.len() > MAX_ID_LENGTH {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Identifier too long: {} (max: {})", id.len(), MAX_ID_LENGTH),
        )));
    }

    // Reject path traversal attempts
    if id == "." || id == ".." {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path traversal detected",
        )));
    }

    // Reject hidden files (starting with dot)
    if id.starts_with('.') {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Identifiers cannot start with '.'",
        )));
    }

    // Check each character is safe
    for c in id.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.' {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid character '{}' in identifier", c),
            )));
        }
    }

    // Reject path separators
    if id.contains('/') || id.contains('\\') {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path separators not allowed in identifier",
        )));
    }

    Ok(())
}

/// File storage format for serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageFormat {
    /// JSON format (human-readable, larger)
    Json,
    /// Bincode format (compact, faster)
    #[default]
    Bincode,
}

/// Write-ahead log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub sequence: u64,
    pub operation: WalOperation,
    pub checksum: [u8; 32],
    pub timestamp: i64,
    pub committed: bool,
}

/// WAL operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOperation {
    StoreNode {
        node_id: String,
        state: Vec<f32>,
    },
    DeleteNode {
        node_id: String,
    },
    StoreEdge {
        source: String,
        target: String,
        weight: f32,
    },
    DeleteEdge {
        source: String,
        target: String,
    },
    StorePolicy {
        policy_id: String,
        data: Vec<u8>,
    },
    StoreWitness {
        witness_id: String,
        data: Vec<u8>,
    },
    StoreLineage {
        lineage_id: String,
        data: Vec<u8>,
    },
}

impl WalEntry {
    fn new(sequence: u64, operation: WalOperation) -> Self {
        let op_bytes = bincode::serde::encode_to_vec(&operation, bincode::config::standard())
            .unwrap_or_default();
        let checksum = *blake3::hash(&op_bytes).as_bytes();
        Self {
            sequence,
            operation,
            checksum,
            timestamp: chrono::Utc::now().timestamp_millis(),
            committed: false,
        }
    }

    fn verify(&self) -> bool {
        match bincode::serde::encode_to_vec(&self.operation, bincode::config::standard()) {
            Ok(bytes) => self.checksum == *blake3::hash(&bytes).as_bytes(),
            Err(_) => false,
        }
    }
}

/// Storage metadata persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageMetadata {
    pub version: u32,
    pub format: String,
    pub node_count: u64,
    pub edge_count: u64,
    pub last_wal_sequence: u64,
    pub created_at: i64,
    pub modified_at: i64,
}

/// File-based storage implementation with WAL
#[derive(Debug)]
pub struct FileStorage {
    root: PathBuf,
    format: StorageFormat,
    wal_enabled: bool,
    wal_sequence: Mutex<u64>,
    wal_file: Mutex<Option<BufWriter<File>>>,
    node_cache: RwLock<HashMap<String, Vec<f32>>>,
    edge_cache: RwLock<HashMap<(String, String), f32>>,
    adjacency_cache: RwLock<HashMap<String, HashSet<String>>>,
    cache_dirty: RwLock<bool>,
    metadata: RwLock<StorageMetadata>,
}

impl FileStorage {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::with_options(root, StorageFormat::Bincode, true)
    }

    pub fn with_options(
        root: impl AsRef<Path>,
        format: StorageFormat,
        wal_enabled: bool,
    ) -> Result<Self, StorageError> {
        let root = root.as_ref().to_path_buf();
        for dir in ["nodes", "edges", "policies", "witnesses", "lineages", "wal"] {
            fs::create_dir_all(root.join(dir))?;
        }

        let metadata_path = root.join("metadata.json");
        let metadata: StorageMetadata = if metadata_path.exists() {
            serde_json::from_reader(File::open(&metadata_path)?).unwrap_or_default()
        } else {
            StorageMetadata::default()
        };

        let storage = Self {
            root,
            format,
            wal_enabled,
            wal_sequence: Mutex::new(metadata.last_wal_sequence),
            wal_file: Mutex::new(None),
            node_cache: RwLock::new(HashMap::new()),
            edge_cache: RwLock::new(HashMap::new()),
            adjacency_cache: RwLock::new(HashMap::new()),
            cache_dirty: RwLock::new(false),
            metadata: RwLock::new(metadata),
        };

        if wal_enabled {
            storage.open_wal_file()?;
            storage.recover_from_wal()?;
        }
        storage.load_cache()?;
        Ok(storage)
    }

    pub fn from_config(config: &StorageConfig) -> Result<Self, StorageError> {
        Self::with_options(
            &config.graph_path,
            StorageFormat::Bincode,
            config.enable_wal,
        )
    }

    fn open_wal_file(&self) -> Result<(), StorageError> {
        let seq = *self.wal_sequence.lock();
        let path = self.root.join("wal").join(format!("{:06}.wal", seq / 1000));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        *self.wal_file.lock() = Some(BufWriter::new(file));
        Ok(())
    }

    fn write_wal(&self, operation: WalOperation) -> Result<u64, StorageError> {
        if !self.wal_enabled {
            return Ok(0);
        }
        let seq = {
            let mut g = self.wal_sequence.lock();
            *g += 1;
            *g
        };
        let entry = WalEntry::new(seq, operation);
        let bytes = bincode::serde::encode_to_vec(&entry, bincode::config::standard())
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        if let Some(ref mut wal) = *self.wal_file.lock() {
            wal.write_all(&(bytes.len() as u32).to_le_bytes())?;
            wal.write_all(&bytes)?;
            wal.flush()?;
        }
        Ok(seq)
    }

    fn commit_wal(&self, _seq: u64) -> Result<(), StorageError> {
        if let Some(ref mut wal) = *self.wal_file.lock() {
            wal.flush()?;
        }
        Ok(())
    }

    fn recover_from_wal(&self) -> Result<(), StorageError> {
        let wal_dir = self.root.join("wal");
        let mut entries = Vec::new();
        for entry in fs::read_dir(&wal_dir)? {
            let path = entry?.path();
            if path.extension().map_or(false, |e| e == "wal") {
                let mut reader = BufReader::new(File::open(&path)?);
                loop {
                    let mut len_bytes = [0u8; 4];
                    if reader.read_exact(&mut len_bytes).is_err() {
                        break;
                    }
                    let mut buf = vec![0u8; u32::from_le_bytes(len_bytes) as usize];
                    reader.read_exact(&mut buf)?;
                    if let Ok((e, _)) = bincode::serde::decode_from_slice::<WalEntry, _>(
                        &buf,
                        bincode::config::standard(),
                    ) {
                        if e.verify() && !e.committed {
                            entries.push(e);
                        }
                    }
                }
            }
        }
        entries.sort_by_key(|e| e.sequence);
        for e in entries {
            self.apply_wal_operation(&e.operation)?;
        }
        Ok(())
    }

    fn apply_wal_operation(&self, op: &WalOperation) -> Result<(), StorageError> {
        match op {
            WalOperation::StoreNode { node_id, state } => {
                self.write_node_file(node_id, state)?;
                self.node_cache
                    .write()
                    .insert(node_id.clone(), state.clone());
            }
            WalOperation::DeleteNode { node_id } => {
                self.delete_node_file(node_id)?;
                self.node_cache.write().remove(node_id);
            }
            WalOperation::StoreEdge {
                source,
                target,
                weight,
            } => {
                self.write_edge_file(source, target, *weight)?;
                self.edge_cache
                    .write()
                    .insert((source.clone(), target.clone()), *weight);
            }
            WalOperation::DeleteEdge { source, target } => {
                self.delete_edge_file(source, target)?;
                self.edge_cache
                    .write()
                    .remove(&(source.clone(), target.clone()));
            }
            WalOperation::StorePolicy { policy_id, data } => {
                self.write_data_file("policies", policy_id, data)?;
            }
            WalOperation::StoreWitness { witness_id, data } => {
                self.write_data_file("witnesses", witness_id, data)?;
            }
            WalOperation::StoreLineage { lineage_id, data } => {
                self.write_data_file("lineages", lineage_id, data)?;
            }
        }
        Ok(())
    }

    fn load_cache(&self) -> Result<(), StorageError> {
        let nodes_dir = self.root.join("nodes");
        if nodes_dir.exists() {
            for entry in fs::read_dir(&nodes_dir)? {
                let path = entry?.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(state) = self.read_node_file(stem) {
                        self.node_cache.write().insert(stem.to_string(), state);
                    }
                }
            }
        }
        let edges_dir = self.root.join("edges");
        if edges_dir.exists() {
            for entry in fs::read_dir(&edges_dir)? {
                let path = entry?.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let parts: Vec<&str> = stem.splitn(2, '_').collect();
                    if parts.len() == 2 {
                        if let Ok(weight) = self.read_edge_file(parts[0], parts[1]) {
                            self.edge_cache
                                .write()
                                .insert((parts[0].to_string(), parts[1].to_string()), weight);
                            let mut adj = self.adjacency_cache.write();
                            adj.entry(parts[0].to_string())
                                .or_default()
                                .insert(parts[1].to_string());
                            adj.entry(parts[1].to_string())
                                .or_default()
                                .insert(parts[0].to_string());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn write_node_file(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError> {
        let path = self.node_path(node_id);
        let mut writer = BufWriter::new(File::create(&path)?);
        match self.format {
            StorageFormat::Json => serde_json::to_writer(&mut writer, state)
                .map_err(|e| StorageError::Serialization(e.to_string()))?,
            StorageFormat::Bincode => {
                let bytes = bincode::serde::encode_to_vec(state, bincode::config::standard())
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                writer.write_all(&bytes)?;
            }
        }
        writer.flush()?;
        Ok(())
    }

    fn read_node_file(&self, node_id: &str) -> Result<Vec<f32>, StorageError> {
        let mut reader = BufReader::new(File::open(self.node_path(node_id))?);
        match self.format {
            StorageFormat::Json => serde_json::from_reader(reader)
                .map_err(|e| StorageError::Serialization(e.to_string())),
            StorageFormat::Bincode => {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                let (result, _) =
                    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                        .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(result)
            }
        }
    }

    fn delete_node_file(&self, node_id: &str) -> Result<(), StorageError> {
        let path = self.node_path(node_id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn node_path(&self, node_id: &str) -> PathBuf {
        // Note: Caller must validate node_id first using validate_path_id()
        let ext = if self.format == StorageFormat::Json {
            "json"
        } else {
            "bin"
        };
        self.root.join("nodes").join(format!("{}.{}", node_id, ext))
    }

    /// Validate node_id and return the safe path
    fn safe_node_path(&self, node_id: &str) -> Result<PathBuf, StorageError> {
        validate_path_id(node_id)?;
        Ok(self.node_path(node_id))
    }

    fn write_edge_file(&self, source: &str, target: &str, weight: f32) -> Result<(), StorageError> {
        let mut writer = BufWriter::new(File::create(self.edge_path(source, target))?);
        match self.format {
            StorageFormat::Json => serde_json::to_writer(&mut writer, &weight)
                .map_err(|e| StorageError::Serialization(e.to_string()))?,
            StorageFormat::Bincode => {
                let bytes = bincode::serde::encode_to_vec(&weight, bincode::config::standard())
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                writer.write_all(&bytes)?;
            }
        }
        writer.flush()?;
        Ok(())
    }

    fn read_edge_file(&self, source: &str, target: &str) -> Result<f32, StorageError> {
        let mut reader = BufReader::new(File::open(self.edge_path(source, target))?);
        match self.format {
            StorageFormat::Json => serde_json::from_reader(reader)
                .map_err(|e| StorageError::Serialization(e.to_string())),
            StorageFormat::Bincode => {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                let (result, _) =
                    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                        .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(result)
            }
        }
    }

    fn delete_edge_file(&self, source: &str, target: &str) -> Result<(), StorageError> {
        let path = self.edge_path(source, target);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn edge_path(&self, source: &str, target: &str) -> PathBuf {
        // Note: Caller must validate source and target first using validate_path_id()
        let ext = if self.format == StorageFormat::Json {
            "json"
        } else {
            "bin"
        };
        self.root
            .join("edges")
            .join(format!("{}_{}.{}", source, target, ext))
    }

    /// Validate edge identifiers and return the safe path
    fn safe_edge_path(&self, source: &str, target: &str) -> Result<PathBuf, StorageError> {
        validate_path_id(source)?;
        validate_path_id(target)?;
        Ok(self.edge_path(source, target))
    }

    fn write_data_file(&self, dir: &str, id: &str, data: &[u8]) -> Result<(), StorageError> {
        // Validate both directory name and id to prevent path traversal
        validate_path_id(dir)?;
        validate_path_id(id)?;
        let mut file = File::create(self.root.join(dir).join(format!("{}.bin", id)))?;
        file.write_all(data)?;
        file.flush()?;
        Ok(())
    }

    fn read_data_file(&self, dir: &str, id: &str) -> Result<Vec<u8>, StorageError> {
        // Validate both directory name and id to prevent path traversal
        validate_path_id(dir)?;
        validate_path_id(id)?;
        let mut data = Vec::new();
        File::open(self.root.join(dir).join(format!("{}.bin", id)))?.read_to_end(&mut data)?;
        Ok(data)
    }

    fn save_metadata(&self) -> Result<(), StorageError> {
        let mut metadata = self.metadata.write();
        metadata.modified_at = chrono::Utc::now().timestamp_millis();
        metadata.last_wal_sequence = *self.wal_sequence.lock();
        serde_json::to_writer_pretty(
            BufWriter::new(File::create(self.root.join("metadata.json"))?),
            &*metadata,
        )
        .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(())
    }

    pub fn sync(&self) -> Result<(), StorageError> {
        if *self.cache_dirty.read() {
            self.save_metadata()?;
            *self.cache_dirty.write() = false;
        }
        Ok(())
    }

    pub fn compact_wal(&self) -> Result<(), StorageError> {
        self.save_metadata()
    }

    #[must_use]
    pub fn stats(&self) -> StorageStats {
        let metadata = self.metadata.read();
        StorageStats {
            node_count: self.node_cache.read().len(),
            edge_count: self.edge_cache.read().len(),
            wal_sequence: *self.wal_sequence.lock(),
            root_path: self.root.clone(),
            format: self.format,
            wal_enabled: self.wal_enabled,
            created_at: metadata.created_at,
            modified_at: metadata.modified_at,
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub wal_sequence: u64,
    pub root_path: PathBuf,
    pub format: StorageFormat,
    pub wal_enabled: bool,
    pub created_at: i64,
    pub modified_at: i64,
}

impl Drop for FileStorage {
    fn drop(&mut self) {
        let _ = self.sync();
    }
}

impl GraphStorage for FileStorage {
    fn store_node(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError> {
        // Validate node_id to prevent path traversal
        validate_path_id(node_id)?;
        let seq = self.write_wal(WalOperation::StoreNode {
            node_id: node_id.to_string(),
            state: state.to_vec(),
        })?;
        self.write_node_file(node_id, state)?;
        self.node_cache
            .write()
            .insert(node_id.to_string(), state.to_vec());
        {
            let mut m = self.metadata.write();
            m.node_count = self.node_cache.read().len() as u64;
        }
        self.commit_wal(seq)?;
        *self.cache_dirty.write() = true;
        Ok(())
    }

    fn get_node(&self, node_id: &str) -> Result<Option<Vec<f32>>, StorageError> {
        // Validate node_id to prevent path traversal
        validate_path_id(node_id)?;
        if let Some(state) = self.node_cache.read().get(node_id) {
            return Ok(Some(state.clone()));
        }
        match self.read_node_file(node_id) {
            Ok(state) => {
                self.node_cache
                    .write()
                    .insert(node_id.to_string(), state.clone());
                Ok(Some(state))
            }
            Err(StorageError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn store_edge(&self, source: &str, target: &str, weight: f32) -> Result<(), StorageError> {
        // Validate identifiers to prevent path traversal
        validate_path_id(source)?;
        validate_path_id(target)?;
        let seq = self.write_wal(WalOperation::StoreEdge {
            source: source.to_string(),
            target: target.to_string(),
            weight,
        })?;
        self.write_edge_file(source, target, weight)?;
        self.edge_cache
            .write()
            .insert((source.to_string(), target.to_string()), weight);
        {
            let mut adj = self.adjacency_cache.write();
            adj.entry(source.to_string())
                .or_default()
                .insert(target.to_string());
            adj.entry(target.to_string())
                .or_default()
                .insert(source.to_string());
        }
        {
            let mut m = self.metadata.write();
            m.edge_count = self.edge_cache.read().len() as u64;
        }
        self.commit_wal(seq)?;
        *self.cache_dirty.write() = true;
        Ok(())
    }

    fn delete_edge(&self, source: &str, target: &str) -> Result<(), StorageError> {
        // Validate identifiers to prevent path traversal
        validate_path_id(source)?;
        validate_path_id(target)?;
        let seq = self.write_wal(WalOperation::DeleteEdge {
            source: source.to_string(),
            target: target.to_string(),
        })?;
        self.delete_edge_file(source, target)?;
        self.edge_cache
            .write()
            .remove(&(source.to_string(), target.to_string()));
        {
            let mut adj = self.adjacency_cache.write();
            if let Some(n) = adj.get_mut(source) {
                n.remove(target);
            }
            if let Some(n) = adj.get_mut(target) {
                n.remove(source);
            }
        }
        {
            let mut m = self.metadata.write();
            m.edge_count = self.edge_cache.read().len() as u64;
        }
        self.commit_wal(seq)?;
        *self.cache_dirty.write() = true;
        Ok(())
    }

    fn find_similar(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, StorageError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let nodes = self.node_cache.read();
        let mut sims: Vec<_> = nodes
            .iter()
            .map(|(id, s)| (id.clone(), Self::cosine_similarity(query, s)))
            .collect();
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sims.truncate(k);
        Ok(sims)
    }
}

impl GovernanceStorage for FileStorage {
    fn store_policy(&self, bundle: &[u8]) -> Result<String, StorageError> {
        let id = Uuid::new_v4().to_string();
        let seq = self.write_wal(WalOperation::StorePolicy {
            policy_id: id.clone(),
            data: bundle.to_vec(),
        })?;
        self.write_data_file("policies", &id, bundle)?;
        self.commit_wal(seq)?;
        *self.cache_dirty.write() = true;
        Ok(id)
    }

    fn get_policy(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match self.read_data_file("policies", id) {
            Ok(d) => Ok(Some(d)),
            Err(StorageError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn store_witness(&self, witness: &[u8]) -> Result<String, StorageError> {
        let id = Uuid::new_v4().to_string();
        let seq = self.write_wal(WalOperation::StoreWitness {
            witness_id: id.clone(),
            data: witness.to_vec(),
        })?;
        self.write_data_file("witnesses", &id, witness)?;
        self.commit_wal(seq)?;
        *self.cache_dirty.write() = true;
        Ok(id)
    }

    fn get_witnesses_for_action(&self, action_id: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        let mut results = Vec::new();
        let dir = self.root.join("witnesses");
        if dir.exists() {
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(data) = self.read_data_file("witnesses", stem) {
                        if data
                            .windows(action_id.len())
                            .any(|w| w == action_id.as_bytes())
                        {
                            results.push(data);
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    fn store_lineage(&self, lineage: &[u8]) -> Result<String, StorageError> {
        let id = Uuid::new_v4().to_string();
        let seq = self.write_wal(WalOperation::StoreLineage {
            lineage_id: id.clone(),
            data: lineage.to_vec(),
        })?;
        self.write_data_file("lineages", &id, lineage)?;
        self.commit_wal(seq)?;
        *self.cache_dirty.write() = true;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_storage_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();
        storage.store_node("node-1", &[1.0, 0.0, 0.0]).unwrap();
        let state = storage.get_node("node-1").unwrap();
        assert!(state.is_some());
        assert_eq!(state.unwrap(), vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_file_storage_edges() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();
        storage.store_edge("a", "b", 1.0).unwrap();
        storage.delete_edge("a", "b").unwrap();
        assert_eq!(storage.stats().edge_count, 0);
    }

    #[test]
    fn test_storage_format_json() {
        let temp_dir = TempDir::new().unwrap();
        let storage =
            FileStorage::with_options(temp_dir.path(), StorageFormat::Json, false).unwrap();
        storage.store_node("json-node", &[1.0, 2.0]).unwrap();
        let state = storage.get_node("json-node").unwrap();
        assert_eq!(state.unwrap(), vec![1.0, 2.0]);
    }
}
