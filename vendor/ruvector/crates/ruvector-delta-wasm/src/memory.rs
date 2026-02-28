//! Shared memory management for zero-copy operations
//!
//! Provides shared memory buffers for efficient delta operations
//! without copying data between WASM and JavaScript.

use parking_lot::RwLock;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Maximum size for shared memory buffers (256 MB)
const MAX_BUFFER_SIZE: usize = 256 * 1024 * 1024;

/// Shared memory buffer for vector operations
#[wasm_bindgen]
pub struct SharedBuffer {
    data: Arc<RwLock<Vec<f32>>>,
    dimensions: usize,
}

#[wasm_bindgen]
impl SharedBuffer {
    /// Create a new shared buffer with given dimensions
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize) -> Result<SharedBuffer, JsValue> {
        if dimensions == 0 {
            return Err(JsValue::from_str("Dimensions must be > 0"));
        }

        if dimensions * 4 > MAX_BUFFER_SIZE {
            return Err(JsValue::from_str(&format!(
                "Buffer size exceeds maximum: {} > {}",
                dimensions * 4,
                MAX_BUFFER_SIZE
            )));
        }

        Ok(SharedBuffer {
            data: Arc::new(RwLock::new(vec![0.0; dimensions])),
            dimensions,
        })
    }

    /// Create from existing data
    #[wasm_bindgen(js_name = fromData)]
    pub fn from_data(data: js_sys::Float32Array) -> Result<SharedBuffer, JsValue> {
        let dimensions = data.length() as usize;

        if dimensions == 0 {
            return Err(JsValue::from_str("Data cannot be empty"));
        }

        if dimensions * 4 > MAX_BUFFER_SIZE {
            return Err(JsValue::from_str("Data exceeds maximum buffer size"));
        }

        Ok(SharedBuffer {
            data: Arc::new(RwLock::new(data.to_vec())),
            dimensions,
        })
    }

    /// Get dimensions
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Get byte size
    #[wasm_bindgen(getter, js_name = byteSize)]
    pub fn byte_size(&self) -> usize {
        self.dimensions * 4
    }

    /// Copy data to a Float32Array
    #[wasm_bindgen(js_name = toFloat32Array)]
    pub fn to_float32_array(&self) -> js_sys::Float32Array {
        let data = self.data.read();
        js_sys::Float32Array::from(&data[..])
    }

    /// Copy data from a Float32Array
    #[wasm_bindgen(js_name = fromFloat32Array)]
    pub fn from_float32_array(&self, arr: js_sys::Float32Array) -> Result<(), JsValue> {
        if arr.length() as usize != self.dimensions {
            return Err(JsValue::from_str("Array length doesn't match dimensions"));
        }

        let mut data = self.data.write();
        arr.copy_to(&mut data);
        Ok(())
    }

    /// Get value at index
    pub fn get(&self, index: usize) -> Result<f32, JsValue> {
        if index >= self.dimensions {
            return Err(JsValue::from_str("Index out of bounds"));
        }

        let data = self.data.read();
        Ok(data[index])
    }

    /// Set value at index
    pub fn set(&self, index: usize, value: f32) -> Result<(), JsValue> {
        if index >= self.dimensions {
            return Err(JsValue::from_str("Index out of bounds"));
        }

        let mut data = self.data.write();
        data[index] = value;
        Ok(())
    }

    /// Fill with a value
    pub fn fill(&self, value: f32) {
        let mut data = self.data.write();
        data.fill(value);
    }

    /// Reset to zeros
    pub fn zero(&self) {
        self.fill(0.0);
    }

    /// Clone the buffer
    #[wasm_bindgen(js_name = clone)]
    pub fn clone_buffer(&self) -> SharedBuffer {
        let data = self.data.read().clone();
        SharedBuffer {
            data: Arc::new(RwLock::new(data)),
            dimensions: self.dimensions,
        }
    }

    /// Add another buffer in-place
    #[wasm_bindgen(js_name = addAssign)]
    pub fn add_assign(&self, other: &SharedBuffer) -> Result<(), JsValue> {
        if self.dimensions != other.dimensions {
            return Err(JsValue::from_str("Dimension mismatch"));
        }

        let mut self_data = self.data.write();
        let other_data = other.data.read();

        crate::simd::simd_add_assign(&mut self_data, &other_data);

        Ok(())
    }

    /// Subtract another buffer in-place
    #[wasm_bindgen(js_name = subAssign)]
    pub fn sub_assign(&self, other: &SharedBuffer) -> Result<(), JsValue> {
        if self.dimensions != other.dimensions {
            return Err(JsValue::from_str("Dimension mismatch"));
        }

        let mut self_data = self.data.write();
        let other_data = other.data.read();

        crate::simd::simd_sub_assign(&mut self_data, &other_data);

        Ok(())
    }

    /// Scale in-place
    pub fn scale(&self, factor: f32) {
        let mut data = self.data.write();
        crate::simd::simd_scale(&mut data, factor);
    }

    /// Compute dot product with another buffer
    pub fn dot(&self, other: &SharedBuffer) -> Result<f32, JsValue> {
        if self.dimensions != other.dimensions {
            return Err(JsValue::from_str("Dimension mismatch"));
        }

        let self_data = self.data.read();
        let other_data = other.data.read();

        Ok(crate::simd::simd_dot(&self_data, &other_data))
    }

    /// Compute L2 norm
    #[wasm_bindgen(js_name = l2Norm)]
    pub fn l2_norm(&self) -> f32 {
        let data = self.data.read();
        crate::simd::simd_l2_norm_squared(&data).sqrt()
    }

    /// Count non-zero elements
    #[wasm_bindgen(js_name = countNonzero)]
    pub fn count_nonzero(&self, epsilon: f32) -> usize {
        let data = self.data.read();
        crate::simd::simd_count_nonzero(&data, epsilon)
    }

    /// Clamp values to range
    pub fn clamp(&self, min: f32, max: f32) {
        let mut data = self.data.write();
        crate::simd::simd_clamp(&mut data, min, max);
    }

    /// Compute element-wise absolute value
    pub fn abs(&self) {
        let mut data = self.data.write();
        crate::simd::simd_abs(&mut data);
    }
}

/// Pool of shared buffers for efficient reuse
#[wasm_bindgen]
pub struct BufferPool {
    buffers: Vec<SharedBuffer>,
    dimensions: usize,
    available: Vec<usize>,
}

#[wasm_bindgen]
impl BufferPool {
    /// Create a new buffer pool
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize, initial_count: usize) -> Result<BufferPool, JsValue> {
        let mut buffers = Vec::with_capacity(initial_count);
        let mut available = Vec::with_capacity(initial_count);

        for i in 0..initial_count {
            buffers.push(SharedBuffer::new(dimensions)?);
            available.push(i);
        }

        Ok(BufferPool {
            buffers,
            dimensions,
            available,
        })
    }

    /// Get the pool dimensions
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Get number of available buffers
    #[wasm_bindgen(getter, js_name = availableCount)]
    pub fn available_count(&self) -> usize {
        self.available.len()
    }

    /// Get total number of buffers
    #[wasm_bindgen(getter, js_name = totalCount)]
    pub fn total_count(&self) -> usize {
        self.buffers.len()
    }

    /// Acquire a buffer from the pool
    pub fn acquire(&mut self) -> Result<SharedBuffer, JsValue> {
        if let Some(idx) = self.available.pop() {
            // Clone the buffer for exclusive use
            Ok(self.buffers[idx].clone_buffer())
        } else {
            // Pool exhausted, create new buffer
            let buffer = SharedBuffer::new(self.dimensions)?;
            self.buffers.push(buffer.clone_buffer());
            Ok(buffer)
        }
    }

    /// Release a buffer back to the pool (just tracks availability)
    pub fn release(&mut self, _buffer: SharedBuffer) {
        // In WASM, we can't actually return ownership
        // The buffer will be dropped when JS releases it
        // This method is for tracking purposes
    }

    /// Pre-allocate more buffers
    #[wasm_bindgen(js_name = grow)]
    pub fn grow(&mut self, count: usize) -> Result<(), JsValue> {
        let start = self.buffers.len();
        for i in 0..count {
            self.buffers.push(SharedBuffer::new(self.dimensions)?);
            self.available.push(start + i);
        }
        Ok(())
    }

    /// Clear the pool
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.available.clear();
    }
}

/// Memory statistics
#[wasm_bindgen]
pub struct MemoryStats {
    /// Total allocated bytes
    pub total_bytes: usize,
    /// Number of buffers
    pub buffer_count: usize,
    /// Average buffer size
    pub avg_buffer_size: usize,
}

#[wasm_bindgen]
impl MemoryStats {
    /// Create from pool
    #[wasm_bindgen(js_name = fromPool)]
    pub fn from_pool(pool: &BufferPool) -> MemoryStats {
        let total_bytes = pool.buffers.len() * pool.dimensions * 4;
        MemoryStats {
            total_bytes,
            buffer_count: pool.buffers.len(),
            avg_buffer_size: if pool.buffers.is_empty() {
                0
            } else {
                total_bytes / pool.buffers.len()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_buffer_creation() {
        let buffer = SharedBuffer::new(100).unwrap();
        assert_eq!(buffer.dimensions(), 100);
        assert_eq!(buffer.byte_size(), 400);
    }

    #[test]
    fn test_buffer_operations() {
        let buffer = SharedBuffer::new(4).unwrap();

        buffer.set(0, 1.0).unwrap();
        buffer.set(1, 2.0).unwrap();
        buffer.set(2, 3.0).unwrap();
        buffer.set(3, 4.0).unwrap();

        assert!((buffer.get(0).unwrap() - 1.0).abs() < 1e-6);
        assert!((buffer.get(3).unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_buffer_math() {
        let a = SharedBuffer::new(4).unwrap();
        let b = SharedBuffer::new(4).unwrap();

        a.fill(1.0);
        b.fill(2.0);

        a.add_assign(&b).unwrap();

        assert!((a.get(0).unwrap() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_buffer_pool() {
        let mut pool = BufferPool::new(100, 5).unwrap();

        assert_eq!(pool.available_count(), 5);
        assert_eq!(pool.total_count(), 5);

        let _buf1 = pool.acquire().unwrap();
        let _buf2 = pool.acquire().unwrap();

        assert_eq!(pool.available_count(), 3);
    }
}
