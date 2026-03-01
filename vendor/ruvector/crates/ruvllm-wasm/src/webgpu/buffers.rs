//! GPU Buffer Management for WebGPU WASM
//!
//! This module provides buffer abstractions for GPU memory management
//! in the browser WebGPU environment.

use js_sys::{Float32Array, Uint8Array};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

/// Buffer usage flags
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuBufferUsage {
    /// Can be mapped for reading
    #[wasm_bindgen(skip)]
    pub map_read: bool,
    /// Can be mapped for writing
    #[wasm_bindgen(skip)]
    pub map_write: bool,
    /// Can be used as copy source
    #[wasm_bindgen(skip)]
    pub copy_src: bool,
    /// Can be used as copy destination
    #[wasm_bindgen(skip)]
    pub copy_dst: bool,
    /// Can be used as storage buffer
    #[wasm_bindgen(skip)]
    pub storage: bool,
    /// Can be used as uniform buffer
    #[wasm_bindgen(skip)]
    pub uniform: bool,
}

#[wasm_bindgen]
impl GpuBufferUsage {
    /// Create storage buffer usage (read/write compute)
    #[wasm_bindgen(js_name = storage)]
    pub fn new_storage() -> Self {
        Self {
            storage: true,
            copy_dst: true,
            copy_src: true,
            ..Default::default()
        }
    }

    /// Create uniform buffer usage
    #[wasm_bindgen(js_name = uniform)]
    pub fn new_uniform() -> Self {
        Self {
            uniform: true,
            copy_dst: true,
            ..Default::default()
        }
    }

    /// Create staging buffer for upload
    #[wasm_bindgen(js_name = stagingUpload)]
    pub fn staging_upload() -> Self {
        Self {
            map_write: true,
            copy_src: true,
            ..Default::default()
        }
    }

    /// Create staging buffer for download
    #[wasm_bindgen(js_name = stagingDownload)]
    pub fn staging_download() -> Self {
        Self {
            map_read: true,
            copy_dst: true,
            ..Default::default()
        }
    }

    /// Create read-only storage buffer
    #[wasm_bindgen(js_name = storageReadOnly)]
    pub fn storage_read_only() -> Self {
        Self {
            storage: true,
            copy_dst: true,
            ..Default::default()
        }
    }

    /// Convert to WebGPU usage flags (as raw u32)
    ///
    /// WebGPU buffer usage flags:
    /// - MAP_READ = 0x0001
    /// - MAP_WRITE = 0x0002
    /// - COPY_SRC = 0x0004
    /// - COPY_DST = 0x0008
    /// - INDEX = 0x0010
    /// - VERTEX = 0x0020
    /// - UNIFORM = 0x0040
    /// - STORAGE = 0x0080
    /// - INDIRECT = 0x0100
    /// - QUERY_RESOLVE = 0x0200
    pub fn to_u32(&self) -> u32 {
        let mut flags = 0u32;
        if self.map_read {
            flags |= 0x0001;
        }
        if self.map_write {
            flags |= 0x0002;
        }
        if self.copy_src {
            flags |= 0x0004;
        }
        if self.copy_dst {
            flags |= 0x0008;
        }
        if self.uniform {
            flags |= 0x0040;
        }
        if self.storage {
            flags |= 0x0080;
        }
        flags
    }

    #[wasm_bindgen(getter, js_name = mapRead)]
    pub fn get_map_read(&self) -> bool {
        self.map_read
    }

    #[wasm_bindgen(setter, js_name = mapRead)]
    pub fn set_map_read(&mut self, value: bool) {
        self.map_read = value;
    }

    #[wasm_bindgen(getter, js_name = mapWrite)]
    pub fn get_map_write(&self) -> bool {
        self.map_write
    }

    #[wasm_bindgen(setter, js_name = mapWrite)]
    pub fn set_map_write(&mut self, value: bool) {
        self.map_write = value;
    }

    #[wasm_bindgen(getter, js_name = copySrc)]
    pub fn get_copy_src(&self) -> bool {
        self.copy_src
    }

    #[wasm_bindgen(setter, js_name = copySrc)]
    pub fn set_copy_src(&mut self, value: bool) {
        self.copy_src = value;
    }

    #[wasm_bindgen(getter, js_name = copyDst)]
    pub fn get_copy_dst(&self) -> bool {
        self.copy_dst
    }

    #[wasm_bindgen(setter, js_name = copyDst)]
    pub fn set_copy_dst(&mut self, value: bool) {
        self.copy_dst = value;
    }

    #[wasm_bindgen(getter, js_name = isStorage)]
    pub fn get_storage(&self) -> bool {
        self.storage
    }

    #[wasm_bindgen(setter, js_name = isStorage)]
    pub fn set_storage(&mut self, value: bool) {
        self.storage = value;
    }

    #[wasm_bindgen(getter, js_name = isUniform)]
    pub fn get_uniform(&self) -> bool {
        self.uniform
    }

    #[wasm_bindgen(setter, js_name = isUniform)]
    pub fn set_uniform(&mut self, value: bool) {
        self.uniform = value;
    }
}

/// GPU buffer handle
///
/// Wraps a WebGPU buffer with metadata for safe operations.
#[wasm_bindgen]
pub struct GpuBuffer {
    /// Internal buffer handle (web_sys::GpuBuffer when on wasm32)
    #[cfg(target_arch = "wasm32")]
    buffer: web_sys::GpuBuffer,

    /// Placeholder for non-wasm32 builds
    #[cfg(not(target_arch = "wasm32"))]
    buffer: Vec<u8>,

    /// Buffer size in bytes
    size: usize,

    /// Buffer usage flags
    usage: GpuBufferUsage,

    /// Optional label for debugging
    label: Option<String>,
}

#[wasm_bindgen]
impl GpuBuffer {
    /// Get buffer size in bytes
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get buffer label
    #[wasm_bindgen(getter)]
    pub fn label(&self) -> Option<String> {
        self.label.clone()
    }

    /// Check if buffer supports mapping for read
    #[wasm_bindgen(getter, js_name = canMapRead)]
    pub fn can_map_read(&self) -> bool {
        self.usage.map_read
    }

    /// Check if buffer supports mapping for write
    #[wasm_bindgen(getter, js_name = canMapWrite)]
    pub fn can_map_write(&self) -> bool {
        self.usage.map_write
    }

    /// Get size as number of f32 elements
    #[wasm_bindgen(js_name = sizeAsF32)]
    pub fn size_as_f32(&self) -> usize {
        self.size / 4
    }

    /// Get the raw web_sys buffer (for advanced usage)
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(getter, js_name = rawBuffer)]
    pub fn raw_buffer(&self) -> web_sys::GpuBuffer {
        self.buffer.clone()
    }
}

impl GpuBuffer {
    /// Create a new GPU buffer (internal constructor)
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(
        buffer: web_sys::GpuBuffer,
        size: usize,
        usage: GpuBufferUsage,
        label: Option<String>,
    ) -> Self {
        Self {
            buffer,
            size,
            usage,
            label,
        }
    }

    /// Create a new GPU buffer (non-wasm32 placeholder)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn new(size: usize, usage: GpuBufferUsage, label: Option<String>) -> Self {
        Self {
            buffer: vec![0u8; size],
            size,
            usage,
            label,
        }
    }

    /// Get internal buffer reference
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn inner(&self) -> &web_sys::GpuBuffer {
        &self.buffer
    }
}

/// Staging buffer pool for efficient CPU<->GPU transfers
#[wasm_bindgen]
pub struct StagingBufferPool {
    /// Pool of upload staging buffers
    upload_pool: RefCell<Vec<GpuBuffer>>,
    /// Pool of download staging buffers
    download_pool: RefCell<Vec<GpuBuffer>>,
    /// Maximum buffers per pool
    max_per_pool: usize,
    /// Total bytes allocated
    total_allocated: RefCell<usize>,
}

#[wasm_bindgen]
impl StagingBufferPool {
    /// Create a new staging buffer pool
    #[wasm_bindgen(constructor)]
    pub fn new(max_per_pool: usize) -> Self {
        Self {
            upload_pool: RefCell::new(Vec::with_capacity(max_per_pool)),
            download_pool: RefCell::new(Vec::with_capacity(max_per_pool)),
            max_per_pool,
            total_allocated: RefCell::new(0),
        }
    }

    /// Get the number of upload buffers in pool
    #[wasm_bindgen(getter, js_name = uploadBufferCount)]
    pub fn upload_buffer_count(&self) -> usize {
        self.upload_pool.borrow().len()
    }

    /// Get the number of download buffers in pool
    #[wasm_bindgen(getter, js_name = downloadBufferCount)]
    pub fn download_buffer_count(&self) -> usize {
        self.download_pool.borrow().len()
    }

    /// Get total bytes allocated
    #[wasm_bindgen(getter, js_name = totalAllocated)]
    pub fn total_allocated(&self) -> usize {
        *self.total_allocated.borrow()
    }

    /// Clear all pooled buffers
    #[wasm_bindgen]
    pub fn clear(&self) {
        self.upload_pool.borrow_mut().clear();
        self.download_pool.borrow_mut().clear();
        *self.total_allocated.borrow_mut() = 0;
    }
}

/// Tensor descriptor for buffer allocation
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct TensorDescriptor {
    /// Shape dimensions
    shape: Vec<u32>,
    /// Data type (0=f32, 1=f16, 2=i32, 3=u8)
    dtype: u8,
}

#[wasm_bindgen]
impl TensorDescriptor {
    /// Create tensor descriptor for a matrix
    #[wasm_bindgen(js_name = matrix)]
    pub fn matrix(rows: u32, cols: u32) -> Self {
        Self {
            shape: vec![rows, cols],
            dtype: 0, // f32
        }
    }

    /// Create tensor descriptor for a vector
    #[wasm_bindgen(js_name = vector)]
    pub fn vector(len: u32) -> Self {
        Self {
            shape: vec![len],
            dtype: 0,
        }
    }

    /// Create tensor descriptor with arbitrary shape
    #[wasm_bindgen(constructor)]
    pub fn new(shape: Vec<u32>, dtype: u8) -> Self {
        Self { shape, dtype }
    }

    /// Get total number of elements
    #[wasm_bindgen(js_name = numElements)]
    pub fn num_elements(&self) -> usize {
        self.shape.iter().map(|&d| d as usize).product()
    }

    /// Get size in bytes
    #[wasm_bindgen(js_name = sizeBytes)]
    pub fn size_bytes(&self) -> usize {
        let element_size = match self.dtype {
            0 => 4, // f32
            1 => 2, // f16
            2 => 4, // i32
            3 => 1, // u8
            _ => 4, // default to f32
        };
        self.num_elements() * element_size
    }

    /// Get shape dimensions
    #[wasm_bindgen(getter)]
    pub fn shape(&self) -> Vec<u32> {
        self.shape.clone()
    }

    /// Get data type
    #[wasm_bindgen(getter)]
    pub fn dtype(&self) -> u8 {
        self.dtype
    }

    /// Get number of dimensions
    #[wasm_bindgen(getter)]
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }
}

/// Helper functions for creating typed arrays from GPU buffers
#[wasm_bindgen]
pub struct BufferHelpers;

#[wasm_bindgen]
impl BufferHelpers {
    /// Create a Float32Array view from a Uint8Array
    #[wasm_bindgen(js_name = asFloat32Array)]
    pub fn as_float32_array(data: &Uint8Array) -> Float32Array {
        Float32Array::new(&data.buffer())
    }

    /// Calculate aligned size for GPU buffers (must be multiple of 4)
    #[wasm_bindgen(js_name = alignedSize)]
    pub fn aligned_size(size: usize) -> usize {
        (size + 3) & !3
    }

    /// Calculate workgroup count for a given dimension
    #[wasm_bindgen(js_name = workgroupCount)]
    pub fn workgroup_count(total: u32, workgroup_size: u32) -> u32 {
        (total + workgroup_size - 1) / workgroup_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_usage() {
        let storage = GpuBufferUsage::new_storage();
        assert!(storage.storage);
        assert!(storage.copy_dst);
        assert!(storage.copy_src);
        assert!(!storage.uniform);
    }

    #[test]
    fn test_tensor_descriptor() {
        let matrix = TensorDescriptor::matrix(1024, 768);
        assert_eq!(matrix.num_elements(), 1024 * 768);
        assert_eq!(matrix.size_bytes(), 1024 * 768 * 4);
        assert_eq!(matrix.ndim(), 2);
    }

    #[test]
    fn test_aligned_size() {
        assert_eq!(BufferHelpers::aligned_size(0), 0);
        assert_eq!(BufferHelpers::aligned_size(1), 4);
        assert_eq!(BufferHelpers::aligned_size(4), 4);
        assert_eq!(BufferHelpers::aligned_size(5), 8);
    }

    #[test]
    fn test_workgroup_count() {
        assert_eq!(BufferHelpers::workgroup_count(1000, 256), 4);
        assert_eq!(BufferHelpers::workgroup_count(256, 256), 1);
        assert_eq!(BufferHelpers::workgroup_count(257, 256), 2);
    }
}
