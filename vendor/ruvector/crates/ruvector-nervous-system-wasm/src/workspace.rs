//! Global Workspace WASM bindings
//!
//! Based on Global Workspace Theory (Baars, Dehaene):
//! - 4-7 item capacity (Miller's law)
//! - Broadcast/compete architecture
//! - Relevance-based ignition

use wasm_bindgen::prelude::*;

/// Item in the global workspace
#[wasm_bindgen]
#[derive(Clone)]
pub struct WorkspaceItem {
    content: Vec<f32>,
    salience: f32,
    source_module: u16,
    timestamp: u64,
    decay_rate: f32,
    lifetime: u64,
    id: u64,
}

#[wasm_bindgen]
impl WorkspaceItem {
    /// Create a new workspace item
    #[wasm_bindgen(constructor)]
    pub fn new(
        content: &[f32],
        salience: f32,
        source_module: u16,
        timestamp: u64,
    ) -> WorkspaceItem {
        Self {
            content: content.to_vec(),
            salience,
            source_module,
            timestamp,
            decay_rate: 0.95,
            lifetime: 1000,
            id: timestamp,
        }
    }

    /// Create with custom decay and lifetime
    #[wasm_bindgen]
    pub fn with_decay(
        content: &[f32],
        salience: f32,
        source_module: u16,
        timestamp: u64,
        decay_rate: f32,
        lifetime: u64,
    ) -> WorkspaceItem {
        Self {
            content: content.to_vec(),
            salience,
            source_module,
            timestamp,
            decay_rate,
            lifetime,
            id: timestamp,
        }
    }

    /// Get content as Float32Array
    #[wasm_bindgen]
    pub fn get_content(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.content.as_slice())
    }

    /// Get salience
    #[wasm_bindgen(getter)]
    pub fn salience(&self) -> f32 {
        self.salience
    }

    /// Get source module
    #[wasm_bindgen(getter)]
    pub fn source_module(&self) -> u16 {
        self.source_module
    }

    /// Get timestamp
    #[wasm_bindgen(getter)]
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get ID
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Compute content magnitude (L2 norm)
    #[wasm_bindgen]
    pub fn magnitude(&self) -> f32 {
        self.content.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Update salience
    #[wasm_bindgen]
    pub fn update_salience(&mut self, new_salience: f32) {
        self.salience = new_salience.max(0.0);
    }

    /// Apply temporal decay
    #[wasm_bindgen]
    pub fn apply_decay(&mut self, dt: f32) {
        self.salience *= self.decay_rate.powf(dt);
    }

    /// Check if expired
    #[wasm_bindgen]
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time.saturating_sub(self.timestamp) > self.lifetime
    }
}

/// Global workspace with limited capacity and competitive dynamics
///
/// Implements attention and conscious access mechanisms based on
/// Global Workspace Theory.
#[wasm_bindgen]
pub struct GlobalWorkspace {
    buffer: Vec<WorkspaceItem>,
    capacity: usize,
    salience_threshold: f32,
    timestamp: u64,
    salience_decay: f32,
}

#[wasm_bindgen]
impl GlobalWorkspace {
    /// Create a new global workspace
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of representations (typically 4-7)
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: usize) -> GlobalWorkspace {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            salience_threshold: 0.1,
            timestamp: 0,
            salience_decay: 0.95,
        }
    }

    /// Create with custom threshold
    #[wasm_bindgen]
    pub fn with_threshold(capacity: usize, threshold: f32) -> GlobalWorkspace {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            salience_threshold: threshold,
            timestamp: 0,
            salience_decay: 0.95,
        }
    }

    /// Set salience decay rate
    #[wasm_bindgen]
    pub fn set_decay_rate(&mut self, decay: f32) {
        self.salience_decay = decay.clamp(0.0, 1.0);
    }

    /// Broadcast a representation to the workspace
    ///
    /// Returns true if accepted, false if rejected.
    #[wasm_bindgen]
    pub fn broadcast(&mut self, item: WorkspaceItem) -> bool {
        self.timestamp += 1;
        let mut item = item;
        item.timestamp = self.timestamp;

        // Reject if below threshold
        if item.salience < self.salience_threshold {
            return false;
        }

        // If workspace not full, add directly
        if self.buffer.len() < self.capacity {
            self.buffer.push(item);
            return true;
        }

        // If full, compete with weakest item
        if let Some(min_idx) = self.find_weakest() {
            if self.buffer[min_idx].salience < item.salience {
                self.buffer.swap_remove(min_idx);
                self.buffer.push(item);
                return true;
            }
        }

        false
    }

    /// Run competitive dynamics (salience decay and pruning)
    #[wasm_bindgen]
    pub fn compete(&mut self) {
        // Apply salience decay
        for item in self.buffer.iter_mut() {
            item.salience *= self.salience_decay;
        }

        // Remove items below threshold
        self.buffer
            .retain(|item| item.salience >= self.salience_threshold);
    }

    /// Retrieve all current representations as JSON
    #[wasm_bindgen]
    pub fn retrieve(&self) -> JsValue {
        let items: Vec<_> = self
            .buffer
            .iter()
            .map(|item| {
                serde_json::json!({
                    "content": item.content,
                    "salience": item.salience,
                    "source_module": item.source_module,
                    "timestamp": item.timestamp,
                    "id": item.id
                })
            })
            .collect();

        serde_wasm_bindgen::to_value(&items).unwrap_or(JsValue::NULL)
    }

    /// Retrieve top-k most salient representations
    #[wasm_bindgen]
    pub fn retrieve_top_k(&self, k: usize) -> JsValue {
        let mut items: Vec<_> = self.buffer.iter().collect();
        items.sort_by(|a, b| {
            b.salience
                .partial_cmp(&a.salience)
                .unwrap_or(std::cmp::Ordering::Less)
        });
        items.truncate(k);

        let result: Vec<_> = items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "content": item.content,
                    "salience": item.salience,
                    "source_module": item.source_module,
                    "timestamp": item.timestamp,
                    "id": item.id
                })
            })
            .collect();

        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Get most salient item
    #[wasm_bindgen]
    pub fn most_salient(&self) -> Option<WorkspaceItem> {
        self.buffer
            .iter()
            .max_by(|a, b| {
                a.salience
                    .partial_cmp(&b.salience)
                    .unwrap_or(std::cmp::Ordering::Less)
            })
            .cloned()
    }

    /// Check if workspace is at capacity
    #[wasm_bindgen]
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    /// Check if workspace is empty
    #[wasm_bindgen]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get current number of representations
    #[wasm_bindgen(getter)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Get workspace capacity
    #[wasm_bindgen(getter)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all representations
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get average salience
    #[wasm_bindgen]
    pub fn average_salience(&self) -> f32 {
        if self.buffer.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.buffer.iter().map(|r| r.salience).sum();
        sum / self.buffer.len() as f32
    }

    /// Get available slots
    #[wasm_bindgen]
    pub fn available_slots(&self) -> usize {
        self.capacity.saturating_sub(self.buffer.len())
    }

    /// Get current load (0.0 to 1.0)
    #[wasm_bindgen]
    pub fn current_load(&self) -> f32 {
        self.buffer.len() as f32 / self.capacity as f32
    }

    /// Find index of weakest representation
    fn find_weakest(&self) -> Option<usize> {
        if self.buffer.is_empty() {
            return None;
        }

        let mut min_idx = 0;
        let mut min_salience = self.buffer[0].salience;

        for (i, item) in self.buffer.iter().enumerate().skip(1) {
            if item.salience < min_salience {
                min_salience = item.salience;
                min_idx = i;
            }
        }

        Some(min_idx)
    }
}
