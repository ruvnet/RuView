//! Shared Memory Types for Web Workers
//!
//! Provides zero-copy memory sharing between the main thread and Web Workers
//! using SharedArrayBuffer.

use js_sys::{Float32Array, Int32Array, Object, Reflect, SharedArrayBuffer, Uint8Array};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Alignment for tensor data (16 bytes for SIMD)
const TENSOR_ALIGNMENT: usize = 16;

/// A tensor backed by SharedArrayBuffer for zero-copy sharing.
///
/// When SharedArrayBuffer is available, data can be shared between
/// the main thread and workers without copying.
#[derive(Clone)]
pub struct SharedTensor {
    buffer: SharedArrayBuffer,
    view: Float32Array,
    shape: Vec<usize>,
    byte_offset: usize,
}

impl SharedTensor {
    /// Create a new SharedTensor with the given shape.
    ///
    /// # Arguments
    /// * `shape` - Tensor dimensions
    ///
    /// # Returns
    /// A new SharedTensor with zero-initialized data
    pub fn new(shape: &[usize]) -> Result<Self, JsValue> {
        let num_elements: usize = shape.iter().product();
        let byte_length = num_elements * std::mem::size_of::<f32>();

        // Align to TENSOR_ALIGNMENT
        let aligned_length = (byte_length + TENSOR_ALIGNMENT - 1) & !(TENSOR_ALIGNMENT - 1);

        let buffer = SharedArrayBuffer::new(aligned_length as u32);
        let view = Float32Array::new(&buffer);

        Ok(SharedTensor {
            buffer,
            view,
            shape: shape.to_vec(),
            byte_offset: 0,
        })
    }

    /// Create a SharedTensor from existing data.
    ///
    /// # Arguments
    /// * `data` - Tensor data as f32 slice
    /// * `shape` - Tensor dimensions
    ///
    /// # Returns
    /// A new SharedTensor containing a copy of the data
    pub fn from_slice(data: &[f32], shape: &[usize]) -> Result<Self, JsValue> {
        let expected_len: usize = shape.iter().product();
        if data.len() != expected_len {
            return Err(JsValue::from_str(&format!(
                "Data length {} doesn't match shape {:?} (expected {})",
                data.len(),
                shape,
                expected_len
            )));
        }

        let tensor = Self::new(shape)?;
        tensor.view.copy_from(data);
        Ok(tensor)
    }

    /// Create a SharedTensor as a view into an existing SharedArrayBuffer.
    ///
    /// # Arguments
    /// * `buffer` - The SharedArrayBuffer to view
    /// * `byte_offset` - Offset into the buffer (in bytes)
    /// * `shape` - Tensor dimensions
    pub fn from_buffer(
        buffer: SharedArrayBuffer,
        byte_offset: usize,
        shape: &[usize],
    ) -> Result<Self, JsValue> {
        let num_elements: usize = shape.iter().product();

        let view = Float32Array::new_with_byte_offset_and_length(
            &buffer,
            byte_offset as u32,
            num_elements as u32,
        );

        Ok(SharedTensor {
            buffer,
            view,
            shape: shape.to_vec(),
            byte_offset,
        })
    }

    /// Get the tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }

    /// Check if tensor is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the underlying SharedArrayBuffer.
    pub fn buffer(&self) -> &SharedArrayBuffer {
        &self.buffer
    }

    /// Get the Float32Array view.
    pub fn view(&self) -> &Float32Array {
        &self.view
    }

    /// Get byte offset into the buffer.
    pub fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    /// Get the byte length of the tensor data.
    pub fn byte_length(&self) -> usize {
        self.len() * std::mem::size_of::<f32>()
    }

    /// Copy data to a Vec<f32>.
    pub fn to_vec(&self) -> Vec<f32> {
        self.view.to_vec()
    }

    /// Copy data from a slice.
    ///
    /// # Safety Note (SECURITY)
    /// This method uses non-atomic write operations. When sharing memory
    /// between Web Workers, ensure proper synchronization (e.g., barriers)
    /// before and after bulk copies to prevent data races.
    pub fn copy_from(&self, data: &[f32]) -> Result<(), JsValue> {
        if data.len() != self.len() {
            return Err(JsValue::from_str(&format!(
                "Data length {} doesn't match tensor length {}",
                data.len(),
                self.len()
            )));
        }
        self.view.copy_from(data);
        Ok(())
    }

    /// Get an element at the given index.
    ///
    /// # Safety Note (SECURITY)
    /// This method uses non-atomic read operations. When sharing memory
    /// between Web Workers, use `get_atomic()` instead to avoid data races.
    /// Non-atomic reads may return torn values if another thread is writing.
    #[inline]
    pub fn get(&self, index: usize) -> Option<f32> {
        if index < self.len() {
            Some(self.view.get_index(index as u32))
        } else {
            None
        }
    }

    /// Set an element at the given index.
    ///
    /// # Safety Note (SECURITY)
    /// This method uses non-atomic write operations. When sharing memory
    /// between Web Workers, use `set_atomic()` instead to avoid data races.
    /// Non-atomic writes may cause torn writes visible to other threads.
    #[inline]
    pub fn set(&self, index: usize, value: f32) -> Result<(), JsValue> {
        if index >= self.len() {
            return Err(JsValue::from_str("Index out of bounds"));
        }
        self.view.set_index(index as u32, value);
        Ok(())
    }

    /// Create a subview of this tensor.
    ///
    /// # Arguments
    /// * `start` - Start index (in elements)
    /// * `shape` - Shape of the subview
    pub fn subview(&self, start: usize, shape: &[usize]) -> Result<Self, JsValue> {
        let num_elements: usize = shape.iter().product();
        if start + num_elements > self.len() {
            return Err(JsValue::from_str("Subview exceeds tensor bounds"));
        }

        let byte_offset = self.byte_offset + start * std::mem::size_of::<f32>();

        Self::from_buffer(self.buffer.clone(), byte_offset, shape)
    }

    /// Fill with a constant value using Atomics (thread-safe).
    pub fn fill_atomic(&self, value: f32) {
        // Convert f32 to its bit representation for atomic operations
        let bits = value.to_bits() as i32;
        let int_view = Int32Array::new(&self.buffer);
        let offset = (self.byte_offset / 4) as u32;

        for i in 0..self.len() as u32 {
            js_sys::Atomics::store(&int_view, offset + i, bits).expect("Atomics::store failed");
        }
    }

    /// Get a value using Atomics (thread-safe).
    pub fn get_atomic(&self, index: usize) -> Option<f32> {
        if index >= self.len() {
            return None;
        }

        let int_view = Int32Array::new(&self.buffer);
        let offset = (self.byte_offset / 4 + index) as u32;

        let bits = js_sys::Atomics::load(&int_view, offset).expect("Atomics::load failed") as u32;
        Some(f32::from_bits(bits))
    }

    /// Set a value using Atomics (thread-safe).
    pub fn set_atomic(&self, index: usize, value: f32) -> Result<(), JsValue> {
        if index >= self.len() {
            return Err(JsValue::from_str("Index out of bounds"));
        }

        let int_view = Int32Array::new(&self.buffer);
        let offset = (self.byte_offset / 4 + index) as u32;
        let bits = value.to_bits() as i32;

        js_sys::Atomics::store(&int_view, offset, bits).expect("Atomics::store failed");
        Ok(())
    }
}

impl std::fmt::Debug for SharedTensor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedTensor")
            .field("shape", &self.shape)
            .field("byte_offset", &self.byte_offset)
            .field("len", &self.len())
            .finish()
    }
}

/// Region descriptor for shared memory allocation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MemoryRegion {
    /// Offset in bytes from the start of the shared buffer
    pub offset: usize,
    /// Size in bytes
    pub size: usize,
}

impl MemoryRegion {
    /// Create a new memory region.
    pub fn new(offset: usize, size: usize) -> Self {
        MemoryRegion { offset, size }
    }

    /// Get end offset (exclusive).
    pub fn end(&self) -> usize {
        self.offset + self.size
    }

    /// Check if this region overlaps with another.
    pub fn overlaps(&self, other: &MemoryRegion) -> bool {
        self.offset < other.end() && other.offset < self.end()
    }
}

/// Manager for shared memory buffers.
///
/// Handles allocation and deallocation of regions within a large
/// SharedArrayBuffer for efficient memory management.
pub struct SharedBufferManager {
    /// Main shared buffer (allocated on demand)
    buffer: Option<SharedArrayBuffer>,
    /// Current buffer size in bytes
    buffer_size: usize,
    /// Allocated regions
    regions: HashMap<String, MemoryRegion>,
    /// Next allocation offset
    next_offset: usize,
    /// Alignment for allocations
    alignment: usize,
}

impl SharedBufferManager {
    /// Create a new SharedBufferManager.
    pub fn new() -> Self {
        SharedBufferManager {
            buffer: None,
            buffer_size: 0,
            regions: HashMap::new(),
            next_offset: 0,
            alignment: TENSOR_ALIGNMENT,
        }
    }

    /// Create with a pre-allocated buffer of the given size.
    pub fn with_capacity(capacity_bytes: usize) -> Result<Self, JsValue> {
        let aligned_capacity = (capacity_bytes + TENSOR_ALIGNMENT - 1) & !(TENSOR_ALIGNMENT - 1);

        let buffer = SharedArrayBuffer::new(aligned_capacity as u32);

        Ok(SharedBufferManager {
            buffer: Some(buffer),
            buffer_size: aligned_capacity,
            regions: HashMap::new(),
            next_offset: 0,
            alignment: TENSOR_ALIGNMENT,
        })
    }

    /// Ensure buffer has at least the given capacity.
    pub fn ensure_capacity(&mut self, min_capacity: usize) -> Result<(), JsValue> {
        let aligned_capacity = (min_capacity + TENSOR_ALIGNMENT - 1) & !(TENSOR_ALIGNMENT - 1);

        if self.buffer_size >= aligned_capacity {
            return Ok(());
        }

        // Need to reallocate
        let new_buffer = SharedArrayBuffer::new(aligned_capacity as u32);

        // Copy existing data if any
        if let Some(old_buffer) = &self.buffer {
            let old_view = Uint8Array::new(old_buffer);
            let new_view = Uint8Array::new(&new_buffer);
            new_view.set(&old_view, 0);
        }

        self.buffer = Some(new_buffer);
        self.buffer_size = aligned_capacity;

        Ok(())
    }

    /// Allocate a region for a tensor.
    ///
    /// # Arguments
    /// * `name` - Unique name for this region
    /// * `shape` - Tensor shape
    ///
    /// # Returns
    /// A SharedTensor backed by the allocated region
    pub fn allocate(&mut self, name: &str, shape: &[usize]) -> Result<SharedTensor, JsValue> {
        if self.regions.contains_key(name) {
            return Err(JsValue::from_str(&format!(
                "Region '{}' already allocated",
                name
            )));
        }

        let num_elements: usize = shape.iter().product();
        let size_bytes = num_elements * std::mem::size_of::<f32>();
        let aligned_size = (size_bytes + self.alignment - 1) & !(self.alignment - 1);

        // Align the offset
        let aligned_offset = (self.next_offset + self.alignment - 1) & !(self.alignment - 1);

        // Ensure buffer has capacity
        self.ensure_capacity(aligned_offset + aligned_size)?;

        let region = MemoryRegion::new(aligned_offset, aligned_size);
        self.regions.insert(name.to_string(), region);
        self.next_offset = aligned_offset + aligned_size;

        let buffer = self.buffer.as_ref().unwrap().clone();
        SharedTensor::from_buffer(buffer, aligned_offset, shape)
    }

    /// Get an existing tensor by name.
    pub fn get(&self, name: &str, shape: &[usize]) -> Result<SharedTensor, JsValue> {
        let region = self
            .regions
            .get(name)
            .ok_or_else(|| JsValue::from_str(&format!("Region '{}' not found", name)))?;

        let buffer = self
            .buffer
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Buffer not initialized"))?;

        SharedTensor::from_buffer(buffer.clone(), region.offset, shape)
    }

    /// Free a region.
    pub fn free(&mut self, name: &str) -> bool {
        self.regions.remove(name).is_some()
    }

    /// Reset all allocations (but keep the buffer).
    pub fn reset(&mut self) {
        self.regions.clear();
        self.next_offset = 0;
    }

    /// Clear everything including the buffer.
    pub fn clear(&mut self) {
        self.buffer = None;
        self.buffer_size = 0;
        self.regions.clear();
        self.next_offset = 0;
    }

    /// Get the underlying SharedArrayBuffer.
    pub fn buffer(&self) -> Option<&SharedArrayBuffer> {
        self.buffer.as_ref()
    }

    /// Get total allocated bytes.
    pub fn allocated_bytes(&self) -> usize {
        self.next_offset
    }

    /// Get buffer capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.buffer_size
    }

    /// Get remaining available bytes.
    pub fn remaining(&self) -> usize {
        self.buffer_size.saturating_sub(self.next_offset)
    }

    /// Get statistics about the buffer.
    pub fn stats(&self) -> SharedBufferStats {
        SharedBufferStats {
            capacity: self.buffer_size,
            allocated: self.next_offset,
            num_regions: self.regions.len(),
            regions: self.regions.clone(),
        }
    }
}

impl Default for SharedBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about shared buffer usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedBufferStats {
    /// Total capacity in bytes
    pub capacity: usize,
    /// Currently allocated bytes
    pub allocated: usize,
    /// Number of allocated regions
    pub num_regions: usize,
    /// All allocated regions
    pub regions: HashMap<String, MemoryRegion>,
}

/// Synchronization primitive using SharedArrayBuffer and Atomics.
///
/// Provides wait/notify functionality for coordinating between workers.
pub struct SharedBarrier {
    /// Shared state buffer
    state: SharedArrayBuffer,
    /// Int32 view for Atomics operations
    int_view: Int32Array,
    /// Number of participants
    count: usize,
}

impl SharedBarrier {
    /// Create a new barrier for the given number of participants.
    pub fn new(count: usize) -> Self {
        // Allocate buffer for: [generation, arrived_count]
        let buffer = SharedArrayBuffer::new(8);
        let int_view = Int32Array::new(&buffer);

        // Initialize
        js_sys::Atomics::store(&int_view, 0, 0).expect("Atomics::store failed"); // generation
        js_sys::Atomics::store(&int_view, 1, 0).expect("Atomics::store failed"); // arrived

        SharedBarrier {
            state: buffer,
            int_view,
            count,
        }
    }

    /// Get the underlying SharedArrayBuffer for sharing with workers.
    pub fn buffer(&self) -> &SharedArrayBuffer {
        &self.state
    }

    /// Arrive at the barrier and wait for all participants.
    ///
    /// Returns the generation number.
    pub fn wait(&self) -> Result<i32, JsValue> {
        let gen = js_sys::Atomics::load(&self.int_view, 0).expect("Atomics::load failed");
        let arrived = js_sys::Atomics::add(&self.int_view, 1, 1).expect("Atomics::add failed") + 1;

        if arrived as usize == self.count {
            // Last to arrive - reset and notify
            js_sys::Atomics::store(&self.int_view, 1, 0).expect("Atomics::store failed");
            js_sys::Atomics::add(&self.int_view, 0, 1).expect("Atomics::add failed");
            js_sys::Atomics::notify(&self.int_view, 0).expect("Atomics::notify failed");
        } else {
            // Wait for generation to change
            let _ = js_sys::Atomics::wait(&self.int_view, 0, gen);
        }

        Ok(js_sys::Atomics::load(&self.int_view, 0).expect("Atomics::load failed"))
    }

    /// Reset the barrier.
    pub fn reset(&self) {
        js_sys::Atomics::store(&self.int_view, 0, 0).expect("Atomics::store failed");
        js_sys::Atomics::store(&self.int_view, 1, 0).expect("Atomics::store failed");
    }
}

impl Clone for SharedBarrier {
    fn clone(&self) -> Self {
        SharedBarrier {
            state: self.state.clone(),
            int_view: Int32Array::new(&self.state),
            count: self.count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_region() {
        let r1 = MemoryRegion::new(0, 100);
        let r2 = MemoryRegion::new(50, 100);
        let r3 = MemoryRegion::new(100, 100);

        assert!(r1.overlaps(&r2));
        assert!(!r1.overlaps(&r3));
        assert_eq!(r1.end(), 100);
    }

    // Note: SharedTensor tests require wasm32 target due to SharedArrayBuffer
    #[cfg(target_arch = "wasm32")]
    mod wasm_tests {
        use super::*;
        use wasm_bindgen_test::*;

        wasm_bindgen_test_configure!(run_in_browser);

        #[wasm_bindgen_test]
        fn test_shared_tensor_new() {
            let tensor = SharedTensor::new(&[2, 3]).unwrap();
            assert_eq!(tensor.shape(), &[2, 3]);
            assert_eq!(tensor.len(), 6);
        }

        #[wasm_bindgen_test]
        fn test_shared_tensor_from_slice() {
            let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
            let tensor = SharedTensor::from_slice(&data, &[2, 3]).unwrap();

            let result = tensor.to_vec();
            assert_eq!(result, data);
        }

        #[wasm_bindgen_test]
        fn test_shared_buffer_manager() {
            let mut manager = SharedBufferManager::new();

            let tensor1 = manager.allocate("input", &[10, 10]).unwrap();
            assert_eq!(tensor1.len(), 100);

            let tensor2 = manager.allocate("output", &[10, 10]).unwrap();
            assert_eq!(tensor2.len(), 100);

            assert!(manager.allocated_bytes() >= 800); // 200 floats * 4 bytes
        }
    }
}
