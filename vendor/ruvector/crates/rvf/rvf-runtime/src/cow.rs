//! COW read/write engine for vector-addressed clusters.
//!
//! Cluster addressing: `cluster_id = vector_id / vectors_per_cluster`
//!
//! - **Read**: lookup in map -> LocalOffset (read local) or ParentRef (follow chain)
//! - **Write**: if inherited -> copy parent slab -> local, apply mutation, update map
//! - **Write coalescing**: multiple writes to the same inherited cluster are buffered;
//!   on flush, the parent slab is copied once and all mutations applied.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

use rvf_types::cow_map::CowMapEntry;
use rvf_types::{ErrorCode, RvfError};

use crate::cow_map::CowMap;
use crate::store::simple_shake256_256;

/// Witness event emitted when a COW slab copy or delta occurs.
pub struct WitnessEvent {
    /// Event type: 0x0E = CLUSTER_COW, 0x0F = CLUSTER_DELTA.
    pub event_type: u8,
    /// ID of the cluster affected.
    pub cluster_id: u32,
    /// SHAKE-256-256 hash of the parent cluster data before copy.
    pub parent_cluster_hash: [u8; 32],
    /// SHAKE-256-256 hash of the new local cluster data after copy.
    pub new_cluster_hash: [u8; 32],
}

/// A pending write buffered for coalescing.
struct PendingWrite {
    /// Byte offset of the vector within the cluster.
    vector_offset_in_cluster: u32,
    /// Vector data to write.
    data: Vec<u8>,
}

/// COW read/write engine for vector-addressed clusters.
pub struct CowEngine {
    /// The COW cluster map.
    cow_map: CowMap,
    /// Cluster size in bytes (power of 2).
    cluster_size: u32,
    /// Vectors per cluster.
    vectors_per_cluster: u32,
    /// Bytes per vector (dimension * sizeof(f32)).
    bytes_per_vector: u32,
    /// L0 cache: cluster_id -> resolved local file offset.
    l0_cache: HashMap<u32, u64>,
    /// Write coalescing buffer: cluster_id -> pending writes.
    write_buffer: HashMap<u32, Vec<PendingWrite>>,
    /// Whether this engine is frozen (snapshot).
    frozen: bool,
    /// Snapshot epoch (0 = mutable).
    snapshot_epoch: u32,
}

impl CowEngine {
    /// Create a new COW engine.
    ///
    /// # Panics
    /// Panics if `vectors_per_cluster` is 0 (would cause division by zero on read/write).
    pub fn new(cluster_size: u32, vectors_per_cluster: u32, bytes_per_vector: u32) -> Self {
        assert!(vectors_per_cluster > 0, "vectors_per_cluster must be > 0");
        Self {
            cow_map: CowMap::new_flat(0),
            cluster_size,
            vectors_per_cluster,
            bytes_per_vector,
            l0_cache: HashMap::new(),
            write_buffer: HashMap::new(),
            frozen: false,
            snapshot_epoch: 0,
        }
    }

    /// Create a COW engine initialized from a parent (all clusters point to parent).
    ///
    /// # Panics
    /// Panics if `vectors_per_cluster` is 0 (would cause division by zero on read/write).
    pub fn from_parent(
        cluster_count: u32,
        cluster_size: u32,
        vectors_per_cluster: u32,
        bytes_per_vector: u32,
    ) -> Self {
        assert!(vectors_per_cluster > 0, "vectors_per_cluster must be > 0");
        Self {
            cow_map: CowMap::new_parent_ref(cluster_count),
            cluster_size,
            vectors_per_cluster,
            bytes_per_vector,
            l0_cache: HashMap::new(),
            write_buffer: HashMap::new(),
            frozen: false,
            snapshot_epoch: 0,
        }
    }

    /// Get a reference to the underlying COW map.
    pub fn cow_map(&self) -> &CowMap {
        &self.cow_map
    }

    /// Read a vector by ID. Returns byte slice of vector data.
    pub fn read_vector(
        &self,
        vector_id: u64,
        file: &File,
        parent: Option<&File>,
    ) -> Result<Vec<u8>, RvfError> {
        let cluster_id = (vector_id / self.vectors_per_cluster as u64) as u32;
        let vector_index_in_cluster = (vector_id % self.vectors_per_cluster as u64) as u32;
        let vector_offset = vector_index_in_cluster * self.bytes_per_vector;

        let cluster_data = self.read_cluster(cluster_id, file, parent)?;

        let start = vector_offset as usize;
        let end = start + self.bytes_per_vector as usize;
        if end > cluster_data.len() {
            return Err(RvfError::Code(ErrorCode::ClusterNotFound));
        }

        Ok(cluster_data[start..end].to_vec())
    }

    /// Read an entire cluster. Returns cluster data.
    pub fn read_cluster(
        &self,
        cluster_id: u32,
        file: &File,
        parent: Option<&File>,
    ) -> Result<Vec<u8>, RvfError> {
        // Check L0 cache first
        if let Some(&cached_offset) = self.l0_cache.get(&cluster_id) {
            return read_bytes_at(file, cached_offset, self.cluster_size as usize);
        }

        match self.cow_map.lookup(cluster_id) {
            CowMapEntry::LocalOffset(offset) => {
                read_bytes_at(file, offset, self.cluster_size as usize)
            }
            CowMapEntry::ParentRef => {
                let parent_file = parent.ok_or(RvfError::Code(ErrorCode::ParentChainBroken))?;
                let parent_offset = cluster_id as u64 * self.cluster_size as u64;
                read_bytes_at(parent_file, parent_offset, self.cluster_size as usize)
            }
            CowMapEntry::Unallocated => {
                // Return a zeroed cluster for unallocated
                Ok(vec![0u8; self.cluster_size as usize])
            }
        }
    }

    /// Write a vector. Handles COW: copies parent slab if inherited.
    ///
    /// Writes are buffered for coalescing. Call `flush_writes` to commit.
    pub fn write_vector(&mut self, vector_id: u64, data: &[u8]) -> Result<(), RvfError> {
        if self.frozen {
            return Err(RvfError::Code(ErrorCode::SnapshotFrozen));
        }
        if data.len() != self.bytes_per_vector as usize {
            return Err(RvfError::Code(ErrorCode::DimensionMismatch));
        }

        let cluster_id = (vector_id / self.vectors_per_cluster as u64) as u32;
        let vector_index_in_cluster = (vector_id % self.vectors_per_cluster as u64) as u32;
        let vector_offset = vector_index_in_cluster * self.bytes_per_vector;

        self.write_buffer
            .entry(cluster_id)
            .or_default()
            .push(PendingWrite {
                vector_offset_in_cluster: vector_offset,
                data: data.to_vec(),
            });

        Ok(())
    }

    /// Flush write coalescing buffer. Performs actual slab copies for inherited
    /// clusters and applies all pending mutations.
    pub fn flush_writes(
        &mut self,
        file: &mut File,
        parent: Option<&File>,
    ) -> Result<Vec<WitnessEvent>, RvfError> {
        if self.frozen {
            return Err(RvfError::Code(ErrorCode::SnapshotFrozen));
        }

        let pending: Vec<(u32, Vec<PendingWrite>)> = self.write_buffer.drain().collect();

        let mut witness_events = Vec::new();

        for (cluster_id, writes) in pending {
            let entry = self.cow_map.lookup(cluster_id);

            // Get or create local cluster data
            let mut cluster_data = match entry {
                CowMapEntry::LocalOffset(offset) => {
                    // Already local: read existing data
                    read_bytes_at(file, offset, self.cluster_size as usize)?
                }
                CowMapEntry::ParentRef => {
                    // COW: copy parent slab to local
                    let parent_file = parent.ok_or(RvfError::Code(ErrorCode::ParentChainBroken))?;
                    let parent_offset = cluster_id as u64 * self.cluster_size as u64;
                    let parent_data =
                        read_bytes_at(parent_file, parent_offset, self.cluster_size as usize)?;
                    let parent_hash = simple_shake256_256(&parent_data);

                    // Allocate space at end of file
                    let new_offset = file
                        .seek(SeekFrom::End(0))
                        .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;

                    // Write parent data as initial local copy
                    file.write_all(&parent_data)
                        .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;

                    // Update map
                    self.cow_map
                        .update(cluster_id, CowMapEntry::LocalOffset(new_offset));
                    self.l0_cache.insert(cluster_id, new_offset);

                    // We'll compute new hash after mutations and emit witness then
                    witness_events.push(WitnessEvent {
                        event_type: 0x0E, // CLUSTER_COW
                        cluster_id,
                        parent_cluster_hash: parent_hash,
                        new_cluster_hash: [0u8; 32], // placeholder, updated below
                    });

                    parent_data
                }
                CowMapEntry::Unallocated => {
                    // Allocate a new zeroed cluster
                    let zeroed = vec![0u8; self.cluster_size as usize];
                    let new_offset = file
                        .seek(SeekFrom::End(0))
                        .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;
                    file.write_all(&zeroed)
                        .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;
                    self.cow_map
                        .update(cluster_id, CowMapEntry::LocalOffset(new_offset));
                    self.l0_cache.insert(cluster_id, new_offset);
                    zeroed
                }
            };

            // Apply all pending writes to the cluster data
            for pw in &writes {
                let start = pw.vector_offset_in_cluster as usize;
                let end = start + pw.data.len();
                if end > cluster_data.len() {
                    return Err(RvfError::Code(ErrorCode::ClusterNotFound));
                }
                cluster_data[start..end].copy_from_slice(&pw.data);
            }

            // Write the mutated cluster back to its local offset
            if let CowMapEntry::LocalOffset(offset) = self.cow_map.lookup(cluster_id) {
                file.seek(SeekFrom::Start(offset))
                    .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;
                file.write_all(&cluster_data)
                    .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;

                // Update witness event hash if we emitted one for this cluster
                let new_hash = simple_shake256_256(&cluster_data);
                for event in witness_events.iter_mut().rev() {
                    if event.cluster_id == cluster_id {
                        event.new_cluster_hash = new_hash;
                        break;
                    }
                }
            }
        }

        file.sync_all()
            .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;

        Ok(witness_events)
    }

    /// Snapshot-freeze: set epoch, prevent further writes to this generation.
    pub fn freeze(&mut self, epoch: u32) -> Result<(), RvfError> {
        if self.frozen {
            return Err(RvfError::Code(ErrorCode::SnapshotFrozen));
        }
        if !self.write_buffer.is_empty() {
            return Err(RvfError::Code(ErrorCode::FsyncFailed));
        }
        self.frozen = true;
        self.snapshot_epoch = epoch;
        Ok(())
    }

    /// Check if frozen.
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Get the snapshot epoch.
    pub fn snapshot_epoch(&self) -> u32 {
        self.snapshot_epoch
    }

    /// Get COW statistics.
    pub fn stats(&self) -> CowStats {
        CowStats {
            cluster_count: self.cow_map.cluster_count(),
            local_cluster_count: self.cow_map.local_cluster_count(),
            cluster_size: self.cluster_size,
            vectors_per_cluster: self.vectors_per_cluster,
            frozen: self.frozen,
            snapshot_epoch: self.snapshot_epoch,
            pending_writes: self.write_buffer.values().map(|v| v.len()).sum(),
        }
    }
}

/// Statistics about the COW engine state.
pub struct CowStats {
    /// Total clusters in the map.
    pub cluster_count: u32,
    /// Clusters with local data (COW-copied or newly written).
    pub local_cluster_count: u32,
    /// Cluster size in bytes.
    pub cluster_size: u32,
    /// Vectors per cluster.
    pub vectors_per_cluster: u32,
    /// Whether the engine is frozen.
    pub frozen: bool,
    /// Snapshot epoch (0 = mutable).
    pub snapshot_epoch: u32,
    /// Number of pending writes in the coalescing buffer.
    pub pending_writes: usize,
}

/// Read `len` bytes from a file at the given offset.
///
/// Uses `pread` on Unix to avoid seek + BufReader overhead on the hot path.
#[cfg(unix)]
fn read_bytes_at(file: &File, offset: u64, len: usize) -> Result<Vec<u8>, RvfError> {
    use std::os::unix::fs::FileExt;
    let mut buf = vec![0u8; len];
    file.read_exact_at(&mut buf, offset)
        .map_err(|_| RvfError::Code(ErrorCode::ClusterNotFound))?;
    Ok(buf)
}

/// Read `len` bytes from a file at the given offset (non-Unix fallback).
#[cfg(not(unix))]
fn read_bytes_at(file: &File, offset: u64, len: usize) -> Result<Vec<u8>, RvfError> {
    use std::io::Read;
    let mut reader = std::io::BufReader::new(file);
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|_| RvfError::Code(ErrorCode::FsyncFailed))?;
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|_| RvfError::Code(ErrorCode::ClusterNotFound))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_parent_file(cluster_size: u32, cluster_count: u32) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for cluster_id in 0..cluster_count {
            let mut data = vec![0u8; cluster_size as usize];
            // Fill each cluster with its ID byte for identification
            for b in data.iter_mut() {
                *b = (cluster_id & 0xFF) as u8;
            }
            f.write_all(&data).unwrap();
        }
        f.flush().unwrap();
        f
    }

    #[test]
    fn cow_read_from_parent() {
        let cluster_size = 256u32;
        let vecs_per_cluster = 4u32;
        let bytes_per_vec = 64u32; // 16 floats * 4 bytes

        let parent_file = create_parent_file(cluster_size, 4);
        let child_file = NamedTempFile::new().unwrap();

        let engine = CowEngine::from_parent(4, cluster_size, vecs_per_cluster, bytes_per_vec);

        // Read cluster 2 from parent
        let data = engine
            .read_cluster(2, child_file.as_file(), Some(parent_file.as_file()))
            .unwrap();
        assert_eq!(data.len(), cluster_size as usize);
        assert!(data.iter().all(|&b| b == 2));
    }

    #[test]
    fn cow_write_triggers_copy() {
        let cluster_size = 128u32;
        let vecs_per_cluster = 2u32;
        let bytes_per_vec = 64u32;

        let parent_file = create_parent_file(cluster_size, 2);
        let child_file = NamedTempFile::new().unwrap();

        let mut engine = CowEngine::from_parent(2, cluster_size, vecs_per_cluster, bytes_per_vec);

        // Write vector 0 (cluster 0)
        let new_data = vec![0xAA; bytes_per_vec as usize];
        engine.write_vector(0, &new_data).unwrap();

        let events = engine
            .flush_writes(
                &mut child_file.as_file().try_clone().unwrap(),
                Some(parent_file.as_file()),
            )
            .unwrap();

        // Should have one COW event
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, 0x0E);
        assert_eq!(events[0].cluster_id, 0);

        // Now the cluster should be local
        assert_eq!(engine.cow_map().local_cluster_count(), 1);
    }

    #[test]
    fn cow_write_coalescing() {
        let cluster_size = 128u32;
        let vecs_per_cluster = 2u32;
        let bytes_per_vec = 64u32;

        let parent_file = create_parent_file(cluster_size, 2);
        let child_file = NamedTempFile::new().unwrap();

        let mut engine = CowEngine::from_parent(2, cluster_size, vecs_per_cluster, bytes_per_vec);

        // Write both vectors in cluster 0
        let data_a = vec![0xAA; bytes_per_vec as usize];
        let data_b = vec![0xBB; bytes_per_vec as usize];
        engine.write_vector(0, &data_a).unwrap();
        engine.write_vector(1, &data_b).unwrap();

        let events = engine
            .flush_writes(
                &mut child_file.as_file().try_clone().unwrap(),
                Some(parent_file.as_file()),
            )
            .unwrap();

        // Only one COW copy event even though two writes
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].cluster_id, 0);
    }

    #[test]
    fn cow_frozen_rejects_writes() {
        let mut engine = CowEngine::new(128, 2, 64);
        engine.freeze(1).unwrap();
        assert!(engine.is_frozen());

        let result = engine.write_vector(0, &vec![0u8; 64]);
        assert!(result.is_err());
    }

    #[test]
    fn cow_read_unallocated_returns_zeros() {
        let engine = CowEngine::new(128, 2, 64);
        let child_file = NamedTempFile::new().unwrap();

        let data = engine.read_cluster(0, child_file.as_file(), None).unwrap();
        assert_eq!(data.len(), 128);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn cow_stats() {
        let mut engine = CowEngine::from_parent(4, 256, 4, 64);
        let stats = engine.stats();
        assert_eq!(stats.cluster_count, 4);
        assert_eq!(stats.local_cluster_count, 0);
        assert!(!stats.frozen);

        // Buffer a write
        engine.write_vector(0, &vec![0u8; 64]).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.pending_writes, 1);
    }
}
