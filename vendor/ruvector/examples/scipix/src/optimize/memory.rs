//! Memory optimization utilities
//!
//! Provides object pooling, memory-mapped file loading, and zero-copy operations.

use memmap2::{Mmap, MmapOptions};
use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::memory_opt_enabled;
use crate::error::{Result, ScipixError};

/// Object pool for reusable buffers
pub struct BufferPool<T> {
    pool: Arc<Mutex<VecDeque<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    #[allow(dead_code)]
    max_size: usize,
}

impl<T: Send + 'static> BufferPool<T> {
    /// Create a new buffer pool
    pub fn new<F>(factory: F, initial_size: usize, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let factory = Arc::new(factory);
        let pool = Arc::new(Mutex::new(VecDeque::with_capacity(max_size)));

        // Pre-allocate initial buffers
        if memory_opt_enabled() {
            let mut pool_lock = pool.lock().unwrap();
            for _ in 0..initial_size {
                pool_lock.push_back(factory());
            }
        }

        Self {
            pool,
            factory,
            max_size,
        }
    }

    /// Acquire a buffer from the pool
    pub fn acquire(&self) -> PooledBuffer<T> {
        let buffer = if memory_opt_enabled() {
            self.pool
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| (self.factory)())
        } else {
            (self.factory)()
        };

        PooledBuffer {
            buffer: Some(buffer),
            pool: self.pool.clone(),
        }
    }

    /// Get current pool size
    pub fn size(&self) -> usize {
        self.pool.lock().unwrap().len()
    }

    /// Clear the pool
    pub fn clear(&self) {
        self.pool.lock().unwrap().clear();
    }
}

/// RAII guard for pooled buffers
pub struct PooledBuffer<T> {
    buffer: Option<T>,
    pool: Arc<Mutex<VecDeque<T>>>,
}

impl<T> PooledBuffer<T> {
    /// Get mutable reference to buffer
    pub fn get_mut(&mut self) -> &mut T {
        self.buffer.as_mut().unwrap()
    }

    /// Get immutable reference to buffer
    pub fn get(&self) -> &T {
        self.buffer.as_ref().unwrap()
    }
}

impl<T> Drop for PooledBuffer<T> {
    fn drop(&mut self) {
        if memory_opt_enabled() {
            if let Some(buffer) = self.buffer.take() {
                let mut pool = self.pool.lock().unwrap();
                pool.push_back(buffer);
            }
        }
    }
}

impl<T> std::ops::Deref for PooledBuffer<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.as_mut().unwrap()
    }
}

/// Memory-mapped model file
pub struct MmapModel {
    _mmap: Mmap,
    data: *const u8,
    len: usize,
}

unsafe impl Send for MmapModel {}
unsafe impl Sync for MmapModel {}

impl MmapModel {
    /// Load model from file using memory mapping
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref()).map_err(|e| ScipixError::Io(e))?;

        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|e| ScipixError::Io(e))?
        };

        let data = mmap.as_ptr();
        let len = mmap.len();

        Ok(Self {
            _mmap: mmap,
            data,
            len,
        })
    }

    /// Get slice of model data
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data, self.len) }
    }

    /// Get size of mapped region
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Zero-copy image view
pub struct ImageView<'a> {
    data: &'a [u8],
    width: u32,
    height: u32,
    channels: u8,
}

impl<'a> ImageView<'a> {
    /// Create new image view from raw data
    pub fn new(data: &'a [u8], width: u32, height: u32, channels: u8) -> Result<Self> {
        let expected_len = (width * height * channels as u32) as usize;
        if data.len() != expected_len {
            return Err(ScipixError::InvalidInput(format!(
                "Invalid data length: expected {}, got {}",
                expected_len,
                data.len()
            )));
        }

        Ok(Self {
            data,
            width,
            height,
            channels,
        })
    }

    /// Get pixel at (x, y)
    pub fn pixel(&self, x: u32, y: u32) -> &[u8] {
        let offset = ((y * self.width + x) * self.channels as u32) as usize;
        &self.data[offset..offset + self.channels as usize]
    }

    /// Get raw data slice
    pub fn data(&self) -> &[u8] {
        self.data
    }

    /// Get dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get number of channels
    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// Create subview (region of interest)
    pub fn subview(&self, x: u32, y: u32, width: u32, height: u32) -> Result<Self> {
        if x + width > self.width || y + height > self.height {
            return Err(ScipixError::InvalidInput(
                "Subview out of bounds".to_string(),
            ));
        }

        // For simplicity, this creates a copy. True zero-copy would need stride support
        let mut subview_data = Vec::new();
        for row in y..y + height {
            let start = ((row * self.width + x) * self.channels as u32) as usize;
            let end = start + (width * self.channels as u32) as usize;
            subview_data.extend_from_slice(&self.data[start..end]);
        }

        // This temporarily leaks memory - in production, use arena allocator
        let leaked = Box::leak(subview_data.into_boxed_slice());

        Ok(Self {
            data: leaked,
            width,
            height,
            channels: self.channels,
        })
    }
}

/// Arena allocator for temporary allocations
pub struct Arena {
    buffer: Vec<u8>,
    offset: usize,
}

impl Arena {
    /// Create new arena with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            offset: 0,
        }
    }

    /// Allocate aligned memory
    pub fn alloc(&mut self, size: usize, align: usize) -> &mut [u8] {
        // Align offset
        let padding = (align - (self.offset % align)) % align;
        self.offset += padding;

        let start = self.offset;
        let end = start + size;

        if end > self.buffer.capacity() {
            // Grow buffer
            self.buffer.reserve(end - self.buffer.len());
        }

        unsafe {
            self.buffer.set_len(end);
        }

        self.offset = end;
        &mut self.buffer[start..end]
    }

    /// Reset arena (keeps capacity)
    pub fn reset(&mut self) {
        self.offset = 0;
        self.buffer.clear();
    }

    /// Get current usage
    pub fn usage(&self) -> usize {
        self.offset
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }
}

/// Global buffer pools for common sizes
pub struct GlobalPools {
    small: BufferPool<Vec<u8>>,  // 1KB buffers
    medium: BufferPool<Vec<u8>>, // 64KB buffers
    large: BufferPool<Vec<u8>>,  // 1MB buffers
}

impl GlobalPools {
    fn new() -> Self {
        Self {
            small: BufferPool::new(|| Vec::with_capacity(1024), 10, 100),
            medium: BufferPool::new(|| Vec::with_capacity(64 * 1024), 5, 50),
            large: BufferPool::new(|| Vec::with_capacity(1024 * 1024), 2, 20),
        }
    }

    /// Get the global pools instance
    pub fn get() -> &'static Self {
        static POOLS: std::sync::OnceLock<GlobalPools> = std::sync::OnceLock::new();
        POOLS.get_or_init(GlobalPools::new)
    }

    /// Acquire small buffer (1KB)
    pub fn acquire_small(&self) -> PooledBuffer<Vec<u8>> {
        self.small.acquire()
    }

    /// Acquire medium buffer (64KB)
    pub fn acquire_medium(&self) -> PooledBuffer<Vec<u8>> {
        self.medium.acquire()
    }

    /// Acquire large buffer (1MB)
    pub fn acquire_large(&self) -> PooledBuffer<Vec<u8>> {
        self.large.acquire()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(|| Vec::with_capacity(1024), 2, 10);

        assert_eq!(pool.size(), 2);

        let mut buf1 = pool.acquire();
        assert_eq!(buf1.capacity(), 1024);
        buf1.extend_from_slice(b"test");

        drop(buf1);
        assert_eq!(pool.size(), 3); // Returned to pool
    }

    #[test]
    fn test_mmap_model() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test model data").unwrap();
        temp.flush().unwrap();

        let mmap = MmapModel::from_file(temp.path()).unwrap();
        assert_eq!(mmap.as_slice(), b"test model data");
        assert_eq!(mmap.len(), 15);
    }

    #[test]
    fn test_image_view() {
        let data = vec![
            255, 0, 0, 255, // Red pixel
            0, 255, 0, 255, // Green pixel
            0, 0, 255, 255, // Blue pixel
            255, 255, 255, 255, // White pixel
        ];

        let view = ImageView::new(&data, 2, 2, 4).unwrap();
        assert_eq!(view.dimensions(), (2, 2));
        assert_eq!(view.pixel(0, 0), &[255, 0, 0, 255]);
        assert_eq!(view.pixel(1, 1), &[255, 255, 255, 255]);
    }

    #[test]
    fn test_arena() {
        let mut arena = Arena::with_capacity(1024);

        let slice1 = arena.alloc(100, 8);
        assert_eq!(slice1.len(), 100);

        let slice2 = arena.alloc(200, 8);
        assert_eq!(slice2.len(), 200);

        assert!(arena.usage() >= 300);

        arena.reset();
        assert_eq!(arena.usage(), 0);
    }

    #[test]
    fn test_global_pools() {
        let pools = GlobalPools::get();

        let small = pools.acquire_small();
        assert!(small.capacity() >= 1024);

        let medium = pools.acquire_medium();
        assert!(medium.capacity() >= 64 * 1024);

        let large = pools.acquire_large();
        assert!(large.capacity() >= 1024 * 1024);
    }
}
