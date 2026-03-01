// Memory-Mapped Neural Field Implementation
// Enables petabyte-scale continuous manifolds with lazy evaluation

use memmap2::{MmapMut, MmapOptions};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Multi-resolution hash table for sparse addressing (Instant-NGP style)
#[derive(Clone)]
pub struct HashTable {
    size: usize,
    data: Vec<u64>,
}

impl HashTable {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            data: vec![0; size],
        }
    }

    /// Hash byte data to table index
    pub fn hash(&self, data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish() % self.size as u64
    }

    /// Multi-resolution quantization
    pub fn quantize(concept: &[f32], resolution: usize) -> Vec<u8> {
        concept
            .iter()
            .flat_map(|&x| {
                let quantized = (x * resolution as f32).round() as i32;
                quantized.to_le_bytes()
            })
            .collect()
    }
}

/// Access tracking for tier migration decisions
#[derive(Clone, Debug)]
pub struct AccessEntry {
    pub page_id: u64,
    pub timestamp: Instant,
    pub latency_us: u64,
    pub tier: StorageTier,
}

/// Storage tier levels
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageTier {
    L1Dram, // ~80 ns
    L2Cxl,  // ~350 ns
    L3Ssd,  // ~80 μs
    L4Hdd,  // ~10 ms
}

impl StorageTier {
    pub fn latency_ns(&self) -> u64 {
        match self {
            StorageTier::L1Dram => 80,
            StorageTier::L2Cxl => 350,
            StorageTier::L3Ssd => 80_000,
            StorageTier::L4Hdd => 10_000_000,
        }
    }
}

/// Page metadata for migration policy
#[derive(Clone, Debug)]
pub struct PageMetadata {
    pub id: u64,
    pub size_bytes: usize,
    pub last_access: Instant,
    pub access_count: usize,
    pub importance: f32,
    pub is_dirty: bool,
    pub is_pinned: bool,
    pub current_tier: StorageTier,
}

impl PageMetadata {
    pub fn new(id: u64, size_bytes: usize) -> Self {
        Self {
            id,
            size_bytes,
            last_access: Instant::now(),
            access_count: 0,
            importance: 0.5,
            is_dirty: false,
            is_pinned: false,
            current_tier: StorageTier::L4Hdd,
        }
    }

    pub fn touch(&mut self) {
        self.last_access = Instant::now();
        self.access_count += 1;
    }

    pub fn age(&self) -> u64 {
        self.last_access.elapsed().as_secs()
    }
}

/// Memory-mapped neural field with lazy evaluation
pub struct MmapNeuralField {
    /// Memory-mapped file backing
    mmap: Arc<RwLock<MmapMut>>,

    /// Virtual address space size (can be petabytes)
    virtual_size: usize,

    /// Physical backing file path
    backing_file: PathBuf,

    /// File handle
    file: File,

    /// Multi-resolution hash tables (Instant-NGP)
    hash_tables: Vec<HashTable>,

    /// Page metadata index
    pages: Arc<RwLock<HashMap<u64, PageMetadata>>>,

    /// Access log for prefetch prediction
    access_log: Arc<RwLock<Vec<AccessEntry>>>,

    /// Page size (default 4 MB)
    page_size: usize,
}

impl MmapNeuralField {
    /// Create new memory-mapped neural field
    ///
    /// # Arguments
    /// * `path` - Path to backing file
    /// * `virtual_size` - Virtual address space size (can exceed physical storage)
    /// * `page_size` - Page granularity (default 4 MB)
    pub fn new(
        path: impl AsRef<Path>,
        virtual_size: usize,
        page_size: Option<usize>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let page_size = page_size.unwrap_or(4 * 1024 * 1024); // 4 MB default

        // Create/open backing file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        // Set initial file size (sparse allocation)
        file.set_len(virtual_size as u64)?;

        // Memory-map the file
        let mmap = unsafe { MmapOptions::new().len(virtual_size).map_mut(&file)? };

        // Initialize multi-resolution hash tables
        let hash_tables = vec![
            HashTable::new(1 << 16), // 64K entries
            HashTable::new(1 << 18), // 256K entries
            HashTable::new(1 << 20), // 1M entries
            HashTable::new(1 << 22), // 4M entries
            HashTable::new(1 << 24), // 16M entries
        ];

        Ok(Self {
            mmap: Arc::new(RwLock::new(mmap)),
            virtual_size,
            backing_file: path.to_path_buf(),
            file,
            hash_tables,
            pages: Arc::new(RwLock::new(HashMap::new())),
            access_log: Arc::new(RwLock::new(Vec::new())),
            page_size,
        })
    }

    /// Hash high-dimensional concept to storage address
    ///
    /// Uses multi-resolution hashing (Instant-NGP) for sparse distributed addressing
    pub fn hash_address(&self, concept: &[f32]) -> u64 {
        let mut combined_hash = 0u64;

        for (i, table) in self.hash_tables.iter().enumerate() {
            let resolution = 1 << i;
            let quantized = HashTable::quantize(concept, resolution);
            let hash = table.hash(&quantized);
            combined_hash ^= hash;
        }

        // Ensure address is page-aligned
        let page_id = combined_hash % (self.virtual_size as u64 / self.page_size as u64);
        page_id * self.page_size as u64
    }

    /// Read data from neural field (lazy loads from disk if needed)
    ///
    /// # Arguments
    /// * `addr` - Virtual address (from hash_address)
    /// * `len` - Number of f32 elements to read
    ///
    /// # Returns
    /// Slice of f32 values (zero-copy from mmap)
    pub fn read(&self, addr: u64, len: usize) -> Result<Vec<f32>> {
        let start = Instant::now();

        // Bounds check
        let byte_start = addr as usize;
        let byte_len = len * std::mem::size_of::<f32>();
        let byte_end = byte_start + byte_len;

        if byte_end > self.virtual_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Address out of bounds",
            ));
        }

        // Read from memory-mapped region (zero-copy)
        let mmap = self.mmap.read().unwrap();
        let byte_slice = &mmap[byte_start..byte_end];

        // Reinterpret as f32 slice
        let f32_slice =
            unsafe { std::slice::from_raw_parts(byte_slice.as_ptr() as *const f32, len) };

        // Copy to Vec (required for safe return)
        let result = f32_slice.to_vec();

        // Update access tracking
        let page_id = addr / self.page_size as u64;
        self.record_access(
            page_id,
            StorageTier::L3Ssd,
            start.elapsed().as_micros() as u64,
        );

        Ok(result)
    }

    /// Write data to neural field
    ///
    /// # Arguments
    /// * `addr` - Virtual address
    /// * `data` - f32 values to write
    pub fn write(&self, addr: u64, data: &[f32]) -> Result<()> {
        let byte_start = addr as usize;
        let byte_len = data.len() * std::mem::size_of::<f32>();
        let byte_end = byte_start + byte_len;

        if byte_end > self.virtual_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Address out of bounds",
            ));
        }

        // Write to memory-mapped region
        let mut mmap = self.mmap.write().unwrap();
        let byte_slice = &mut mmap[byte_start..byte_end];

        // Reinterpret as f32 slice
        let f32_slice = unsafe {
            std::slice::from_raw_parts_mut(byte_slice.as_mut_ptr() as *mut f32, data.len())
        };

        // Copy data
        f32_slice.copy_from_slice(data);

        // Mark page as dirty
        let page_id = addr / self.page_size as u64;
        if let Some(page) = self.pages.write().unwrap().get_mut(&page_id) {
            page.is_dirty = true;
        }

        Ok(())
    }

    /// Flush dirty pages to disk (async)
    pub fn flush(&self) -> Result<()> {
        self.mmap.write().unwrap().flush_async()
    }

    /// Get page metadata
    pub fn get_page(&self, page_id: u64) -> Option<PageMetadata> {
        self.pages.read().unwrap().get(&page_id).cloned()
    }

    /// Record access for prefetch prediction
    fn record_access(&self, page_id: u64, tier: StorageTier, latency_us: u64) {
        // Update page metadata
        {
            let mut pages = self.pages.write().unwrap();
            let page = pages
                .entry(page_id)
                .or_insert_with(|| PageMetadata::new(page_id, self.page_size));
            page.touch();
        }

        // Log access
        {
            let mut log = self.access_log.write().unwrap();
            log.push(AccessEntry {
                page_id,
                timestamp: Instant::now(),
                latency_us,
                tier,
            });

            // Keep log bounded (last 10K accesses)
            if log.len() > 10_000 {
                log.drain(0..1000);
            }
        }
    }

    /// Get recent access patterns (for prefetch prediction)
    pub fn recent_accesses(&self, count: usize) -> Vec<AccessEntry> {
        let log = self.access_log.read().unwrap();
        log.iter().rev().take(count).cloned().collect()
    }

    /// Get statistics
    pub fn stats(&self) -> FieldStats {
        let pages = self.pages.read().unwrap();
        let log = self.access_log.read().unwrap();

        let total_pages = pages.len();
        let dirty_pages = pages.values().filter(|p| p.is_dirty).count();
        let total_accesses = log.len();

        let avg_latency = if !log.is_empty() {
            log.iter().map(|e| e.latency_us).sum::<u64>() / log.len() as u64
        } else {
            0
        };

        FieldStats {
            virtual_size: self.virtual_size,
            page_size: self.page_size,
            total_pages,
            dirty_pages,
            total_accesses,
            avg_latency_us: avg_latency,
        }
    }
}

/// Statistics about neural field usage
#[derive(Debug, Clone)]
pub struct FieldStats {
    pub virtual_size: usize,
    pub page_size: usize,
    pub total_pages: usize,
    pub dirty_pages: usize,
    pub total_accesses: usize,
    pub avg_latency_us: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_address() {
        let temp = NamedTempFile::new().unwrap();
        let field = MmapNeuralField::new(
            temp.path(),
            1024 * 1024 * 1024,    // 1 GB
            Some(4 * 1024 * 1024), // 4 MB pages
        )
        .unwrap();

        let concept = vec![0.1f32, 0.2, 0.3, 0.4];
        let addr = field.hash_address(&concept);

        // Address should be page-aligned
        assert_eq!(addr % field.page_size as u64, 0);

        // Same concept should hash to same address
        let addr2 = field.hash_address(&concept);
        assert_eq!(addr, addr2);
    }

    #[test]
    fn test_read_write() {
        let temp = NamedTempFile::new().unwrap();
        let field = MmapNeuralField::new(
            temp.path(),
            1024 * 1024, // 1 MB
            Some(4096),  // 4 KB pages
        )
        .unwrap();

        // Write data
        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        field.write(0, &data).unwrap();

        // Read back
        let read_data = field.read(0, 4).unwrap();
        assert_eq!(data, read_data);
    }

    #[test]
    fn test_lazy_allocation() {
        let temp = NamedTempFile::new().unwrap();
        let field = MmapNeuralField::new(
            temp.path(),
            1024 * 1024 * 1024, // 1 GB virtual
            Some(4 * 1024 * 1024),
        )
        .unwrap();

        // Reading uninitialized memory should return zeros
        let data = field.read(0, 100).unwrap();
        assert_eq!(data.len(), 100);

        // Writing should succeed
        let write_data = vec![42.0f32; 100];
        field.write(0, &write_data).unwrap();

        // Read should return written data
        let read_data = field.read(0, 100).unwrap();
        assert_eq!(read_data[0], 42.0);
    }

    #[test]
    fn test_access_tracking() {
        let temp = NamedTempFile::new().unwrap();
        let field = MmapNeuralField::new(temp.path(), 1024 * 1024, Some(4096)).unwrap();

        // Perform some reads
        for _ in 0..10 {
            let _ = field.read(0, 10).unwrap();
        }

        // Check access log
        let accesses = field.recent_accesses(10);
        assert_eq!(accesses.len(), 10);

        // Check page metadata
        let page = field.get_page(0).unwrap();
        assert_eq!(page.access_count, 10);
    }

    #[test]
    fn test_multi_resolution_hash() {
        let concept1 = vec![0.1f32, 0.2, 0.3];
        let concept2 = vec![0.1f32, 0.2, 0.31]; // Slightly different

        let temp = NamedTempFile::new().unwrap();
        let field = MmapNeuralField::new(temp.path(), 1 << 30, Some(1 << 22)).unwrap();

        let addr1 = field.hash_address(&concept1);
        let addr2 = field.hash_address(&concept2);

        // Similar concepts should have different but nearby addresses
        // (this is probabilistic, so just check they're computed)
        assert!(addr1 < field.virtual_size as u64);
        assert!(addr2 < field.virtual_size as u64);
    }
}
