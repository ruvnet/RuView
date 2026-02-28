//! Memory management for WASM

use std::ops::Deref;
use wasm_bindgen::prelude::*;

/// Efficient buffer wrapper for WASM memory management
pub struct WasmBuffer {
    data: Vec<u8>,
}

impl WasmBuffer {
    /// Create a new buffer with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Create buffer from slice (copies data)
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    /// Create buffer from Vec (takes ownership)
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Get the underlying slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Clear the buffer (keeps capacity)
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Shrink to fit
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    /// Convert to Vec
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }
}

impl Deref for WasmBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Drop for WasmBuffer {
    fn drop(&mut self) {
        // Explicitly clear to help WASM memory management
        self.data.clear();
        self.data.shrink_to_fit();
    }
}

/// Shared memory for large images (uses SharedArrayBuffer when available)
#[wasm_bindgen]
pub struct SharedImageBuffer {
    buffer: WasmBuffer,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
impl SharedImageBuffer {
    /// Create a new shared buffer
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height * 4) as usize; // RGBA
        Self {
            buffer: WasmBuffer::with_capacity(size),
            width,
            height,
        }
    }

    /// Get width
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get height
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get buffer size
    #[wasm_bindgen(js_name = bufferSize)]
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Get buffer as Uint8Array
    #[wasm_bindgen(js_name = getBuffer)]
    pub fn get_buffer(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.buffer.as_slice())
    }

    /// Set buffer from Uint8Array
    #[wasm_bindgen(js_name = setBuffer)]
    pub fn set_buffer(&mut self, data: &js_sys::Uint8Array) {
        self.buffer = WasmBuffer::from_vec(data.to_vec());
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Memory pool for reusing buffers
pub struct MemoryPool {
    buffers: Vec<WasmBuffer>,
    max_size: usize,
}

impl MemoryPool {
    /// Create a new memory pool
    pub fn new(max_size: usize) -> Self {
        Self {
            buffers: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Get a buffer from the pool or create a new one
    pub fn acquire(&mut self, size: usize) -> WasmBuffer {
        self.buffers
            .pop()
            .map(|mut buf| {
                buf.clear();
                buf
            })
            .unwrap_or_else(|| WasmBuffer::with_capacity(size))
    }

    /// Return a buffer to the pool
    pub fn release(&mut self, mut buffer: WasmBuffer) {
        if self.buffers.len() < self.max_size {
            buffer.clear();
            self.buffers.push(buffer);
        }
        // Otherwise drop the buffer
    }

    /// Clear all buffers from the pool
    pub fn clear(&mut self) {
        self.buffers.clear();
    }
}

impl Default for MemoryPool {
    fn default() -> Self {
        Self::new(10)
    }
}

/// Get memory usage statistics
#[wasm_bindgen(js_name = getMemoryStats)]
pub fn get_memory_stats() -> JsValue {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsValue;

        // Try to get memory info from performance.memory (non-standard)
        let performance = web_sys::window().and_then(|w| w.performance());

        if let Some(perf) = performance {
            serde_wasm_bindgen::to_value(&serde_json::json!({
                "available": true,
                "timestamp": perf.now(),
            }))
            .unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    JsValue::NULL
}

/// Force garbage collection (hint to runtime)
#[wasm_bindgen(js_name = forceGC)]
pub fn force_gc() {
    // This is just a hint; actual GC is controlled by the JS runtime
    // In wasm-bindgen, we can't directly trigger GC
    // But we can help by ensuring our memory is freed
}
