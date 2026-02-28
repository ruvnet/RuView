//! Metal buffer management and pooling
//!
//! Provides efficient buffer allocation and reuse.

use metal::{Buffer, Device, MTLResourceOptions};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{Result, RuvLLMError};

/// A wrapper around Metal buffer with metadata
pub struct MetalBuffer {
    /// Underlying Metal buffer
    pub buffer: Buffer,
    /// Size in bytes
    pub size: usize,
    /// Whether this buffer is from a pool
    pub pooled: bool,
}

impl MetalBuffer {
    /// Create a new buffer
    pub fn new(device: &Device, size: usize) -> Self {
        let buffer = device.new_buffer(size as u64, MTLResourceOptions::StorageModeShared);
        Self {
            buffer,
            size,
            pooled: false,
        }
    }

    /// Create a buffer with initial data
    pub fn with_data<T: Copy>(device: &Device, data: &[T]) -> Self {
        let size = data.len() * std::mem::size_of::<T>();
        let buffer = device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size as u64,
            MTLResourceOptions::StorageModeShared,
        );
        Self {
            buffer,
            size,
            pooled: false,
        }
    }

    /// Get buffer contents as a slice
    pub fn as_slice<T: Copy>(&self) -> &[T] {
        let ptr = self.buffer.contents() as *const T;
        let len = self.size / std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    /// Get buffer contents as a mutable slice
    pub fn as_mut_slice<T: Copy>(&mut self) -> &mut [T] {
        let ptr = self.buffer.contents() as *mut T;
        let len = self.size / std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }

    /// Copy data into the buffer
    pub fn copy_from<T: Copy>(&mut self, data: &[T]) -> Result<()> {
        let required = data.len() * std::mem::size_of::<T>();
        if required > self.size {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Buffer too small: {} < {}",
                self.size, required
            )));
        }

        let ptr = self.buffer.contents() as *mut T;
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        }
        Ok(())
    }

    /// Copy data from the buffer
    pub fn copy_to<T: Copy + Default>(&self, count: usize) -> Vec<T> {
        let ptr = self.buffer.contents() as *const T;
        let mut result = vec![T::default(); count];
        unsafe {
            std::ptr::copy_nonoverlapping(ptr, result.as_mut_ptr(), count);
        }
        result
    }
}

/// Buffer pool for efficient memory reuse
pub struct MetalBufferPool {
    /// Device for allocation
    device: Device,
    /// Free buffers by size class
    free_buffers: Mutex<HashMap<usize, Vec<Buffer>>>,
    /// Maximum pool size in bytes
    max_pool_size: usize,
    /// Current pool size in bytes
    current_size: Mutex<usize>,
    /// Size classes for bucketing
    size_classes: Vec<usize>,
}

impl MetalBufferPool {
    /// Create a new buffer pool
    pub fn new(device: Device, max_pool_size: usize) -> Self {
        // Size classes: powers of 2 from 256 bytes to 256MB
        let size_classes: Vec<usize> = (8..=28).map(|i| 1 << i).collect();

        Self {
            device,
            free_buffers: Mutex::new(HashMap::new()),
            max_pool_size,
            current_size: Mutex::new(0),
            size_classes,
        }
    }

    /// Get the size class for a given size
    fn get_size_class(&self, size: usize) -> usize {
        for &class in &self.size_classes {
            if class >= size {
                return class;
            }
        }
        // Round up to next power of 2
        size.next_power_of_two()
    }

    /// Allocate a buffer from the pool
    pub fn allocate(&self, size: usize) -> MetalBuffer {
        let size_class = self.get_size_class(size);

        // Try to get from pool
        {
            let mut free = self.free_buffers.lock().unwrap();
            if let Some(buffers) = free.get_mut(&size_class) {
                if let Some(buffer) = buffers.pop() {
                    let mut current = self.current_size.lock().unwrap();
                    *current -= size_class;
                    return MetalBuffer {
                        buffer,
                        size: size_class,
                        pooled: true,
                    };
                }
            }
        }

        // Allocate new buffer
        let buffer = self
            .device
            .new_buffer(size_class as u64, MTLResourceOptions::StorageModeShared);

        MetalBuffer {
            buffer,
            size: size_class,
            pooled: true,
        }
    }

    /// Return a buffer to the pool
    pub fn release(&self, metal_buffer: MetalBuffer) {
        if !metal_buffer.pooled {
            return;
        }

        let mut current = self.current_size.lock().unwrap();
        if *current + metal_buffer.size > self.max_pool_size {
            // Pool is full, let buffer be dropped
            return;
        }

        let mut free = self.free_buffers.lock().unwrap();
        let buffers = free.entry(metal_buffer.size).or_insert_with(Vec::new);
        buffers.push(metal_buffer.buffer);
        *current += metal_buffer.size;
    }

    /// Clear all pooled buffers
    pub fn clear(&self) {
        let mut free = self.free_buffers.lock().unwrap();
        free.clear();
        let mut current = self.current_size.lock().unwrap();
        *current = 0;
    }

    /// Get pool statistics
    pub fn stats(&self) -> BufferPoolStats {
        let free = self.free_buffers.lock().unwrap();
        let current = self.current_size.lock().unwrap();

        let mut total_buffers = 0;
        let mut size_class_counts = HashMap::new();

        for (&size_class, buffers) in free.iter() {
            total_buffers += buffers.len();
            size_class_counts.insert(size_class, buffers.len());
        }

        BufferPoolStats {
            total_buffers,
            current_size: *current,
            max_size: self.max_pool_size,
            size_class_counts,
        }
    }
}

/// Buffer pool statistics
#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    /// Total number of pooled buffers
    pub total_buffers: usize,
    /// Current pool size in bytes
    pub current_size: usize,
    /// Maximum pool size in bytes
    pub max_size: usize,
    /// Number of buffers per size class
    pub size_class_counts: HashMap<usize, usize>,
}

/// Scoped buffer that returns to pool on drop
pub struct ScopedBuffer<'a> {
    buffer: Option<MetalBuffer>,
    pool: &'a MetalBufferPool,
}

impl<'a> ScopedBuffer<'a> {
    /// Create a new scoped buffer
    pub fn new(pool: &'a MetalBufferPool, size: usize) -> Self {
        Self {
            buffer: Some(pool.allocate(size)),
            pool,
        }
    }

    /// Get the underlying buffer
    pub fn buffer(&self) -> &MetalBuffer {
        self.buffer.as_ref().unwrap()
    }

    /// Get the underlying buffer mutably
    pub fn buffer_mut(&mut self) -> &mut MetalBuffer {
        self.buffer.as_mut().unwrap()
    }
}

impl<'a> Drop for ScopedBuffer<'a> {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            self.pool.release(buffer);
        }
    }
}

impl<'a> std::ops::Deref for ScopedBuffer<'a> {
    type Target = MetalBuffer;

    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

impl<'a> std::ops::DerefMut for ScopedBuffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_size_class() {
        if metal::Device::system_default().is_none() {
            println!("Metal not available, skipping test");
            return;
        }

        let device = metal::Device::system_default().unwrap();
        let pool = MetalBufferPool::new(device, 1024 * 1024);

        assert_eq!(pool.get_size_class(100), 256);
        assert_eq!(pool.get_size_class(1000), 1024);
        assert_eq!(pool.get_size_class(1024), 1024);
        assert_eq!(pool.get_size_class(1025), 2048);
    }

    #[test]
    fn test_buffer_reuse() {
        if metal::Device::system_default().is_none() {
            println!("Metal not available, skipping test");
            return;
        }

        let device = metal::Device::system_default().unwrap();
        let pool = MetalBufferPool::new(device, 1024 * 1024);

        // Allocate and release
        let buf1 = pool.allocate(1000);
        let ptr1 = buf1.buffer.contents();
        pool.release(buf1);

        // Allocate again - should reuse
        let buf2 = pool.allocate(1000);
        let ptr2 = buf2.buffer.contents();

        assert_eq!(ptr1, ptr2, "Buffer should be reused from pool");
    }

    #[test]
    fn test_scoped_buffer() {
        if metal::Device::system_default().is_none() {
            println!("Metal not available, skipping test");
            return;
        }

        let device = metal::Device::system_default().unwrap();
        let pool = MetalBufferPool::new(device, 1024 * 1024);

        let ptr = {
            let scoped = ScopedBuffer::new(&pool, 1000);
            scoped.buffer.buffer.contents()
        };

        // Buffer should be back in pool
        let stats = pool.stats();
        assert_eq!(stats.total_buffers, 1);

        // Allocate again - should get same buffer
        let buf = pool.allocate(1000);
        assert_eq!(buf.buffer.contents(), ptr);
    }
}
