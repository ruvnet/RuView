//! Global workspace for broadcast communication
//!
//! Based on Global Workspace Theory (Baars, Dehaene): A limited-capacity
//! buffer where representations compete for broadcast to all modules.
//! Implements attention and conscious access mechanisms.

use std::collections::{HashMap, VecDeque};

/// Module identifier (u16 for compact representation)
pub type ModuleId = u16;

/// Item in the global workspace
#[derive(Debug, Clone)]
pub struct WorkspaceItem {
    /// Content vector
    pub content: Vec<f32>,
    /// Salience score (determines competitive strength)
    pub salience: f32,
    /// Source module ID
    pub source_module: ModuleId,
    /// Timestamp of entry
    pub timestamp: u64,
    /// Decay rate per time unit
    pub decay_rate: f32,
    /// Maximum lifetime in time units
    pub lifetime: u64,
    /// Unique identifier
    pub id: u64,
}

/// Legacy alias for backward compatibility
pub type Representation = WorkspaceItem;

// Convenience methods for Representation compatibility
impl Representation {
    /// Create a new representation (convenience method with usize source)
    pub fn new_compat(
        content: Vec<f32>,
        salience: f32,
        source_module: usize,
        timestamp: u64,
    ) -> Self {
        Self::new(content, salience, source_module as ModuleId, timestamp)
    }
}

impl WorkspaceItem {
    /// Create a new workspace item
    pub fn new(content: Vec<f32>, salience: f32, source_module: ModuleId, timestamp: u64) -> Self {
        Self {
            content,
            salience,
            source_module,
            timestamp,
            decay_rate: 0.95,
            lifetime: 1000,
            id: timestamp, // Simple ID scheme
        }
    }

    /// Create with custom decay and lifetime
    pub fn with_decay(
        content: Vec<f32>,
        salience: f32,
        source_module: ModuleId,
        timestamp: u64,
        decay_rate: f32,
        lifetime: u64,
    ) -> Self {
        Self {
            content,
            salience,
            source_module,
            timestamp,
            decay_rate,
            lifetime,
            id: timestamp,
        }
    }

    /// Compute content magnitude (L2 norm)
    pub fn magnitude(&self) -> f32 {
        self.content.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Update salience (e.g., based on relevance)
    pub fn update_salience(&mut self, new_salience: f32) {
        self.salience = new_salience.max(0.0);
    }

    /// Apply temporal decay to salience
    pub fn apply_decay(&mut self, dt: f32) {
        self.salience *= self.decay_rate.powf(dt);
    }

    /// Check if item has exceeded lifetime
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time.saturating_sub(self.timestamp) > self.lifetime
    }
}

/// Access request for the global workspace
#[derive(Debug, Clone)]
pub struct AccessRequest {
    /// Requesting module
    pub module: ModuleId,
    /// Content to broadcast
    pub content: Vec<f32>,
    /// Request priority/salience
    pub priority: f32,
    /// Request timestamp
    pub timestamp: u64,
}

impl AccessRequest {
    pub fn new(module: ModuleId, content: Vec<f32>, priority: f32, timestamp: u64) -> Self {
        Self {
            module,
            content,
            priority,
            timestamp,
        }
    }
}

/// Broadcast event record
#[derive(Debug, Clone)]
pub struct BroadcastEvent {
    /// Broadcasted item
    pub item: WorkspaceItem,
    /// Recipient modules
    pub recipients: Vec<ModuleId>,
    /// Broadcast timestamp
    pub timestamp: u64,
}

impl BroadcastEvent {
    pub fn new(item: WorkspaceItem, recipients: Vec<ModuleId>, timestamp: u64) -> Self {
        Self {
            item,
            recipients,
            timestamp,
        }
    }
}

/// Content type for module subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    Query,
    Result,
    Error,
    Control,
    Learning,
}

/// Module registration information
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: ModuleId,
    pub name: String,
    pub priority: f32,
    pub subscriptions: Vec<ContentType>,
}

impl ModuleInfo {
    pub fn new(id: ModuleId, name: String, priority: f32, subscriptions: Vec<ContentType>) -> Self {
        Self {
            id,
            name,
            priority,
            subscriptions,
        }
    }
}

/// Simple ring buffer for broadcast history
#[derive(Debug, Clone)]
struct RingBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }
}

/// Global workspace with limited capacity and competitive dynamics
#[derive(Debug, Clone)]
pub struct GlobalWorkspace {
    /// Current representations in workspace (max capacity)
    buffer: Vec<WorkspaceItem>,
    /// Maximum number of representations (typically 4-7)
    capacity: usize,
    /// Access request queue
    access_queue: VecDeque<AccessRequest>,
    /// Broadcast history (ring buffer)
    broadcast_history: RingBuffer<BroadcastEvent>,
    /// Minimum salience threshold for entry
    salience_threshold: f32,
    /// Current timestamp counter
    timestamp: u64,
    /// Decay rate for salience over time
    salience_decay: f32,
    /// Module access locks
    module_locks: HashMap<ModuleId, bool>,
}

impl GlobalWorkspace {
    /// Create a new global workspace
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of representations (typically 4-7)
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            access_queue: VecDeque::new(),
            broadcast_history: RingBuffer::new(100),
            salience_threshold: 0.1,
            timestamp: 0,
            salience_decay: 0.95,
            module_locks: HashMap::new(),
        }
    }

    /// Create with custom threshold
    pub fn with_threshold(capacity: usize, threshold: f32) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            access_queue: VecDeque::new(),
            broadcast_history: RingBuffer::new(100),
            salience_threshold: threshold,
            timestamp: 0,
            salience_decay: 0.95,
            module_locks: HashMap::new(),
        }
    }

    /// Set salience decay rate (applied each competition step)
    pub fn set_decay_rate(&mut self, decay: f32) {
        self.salience_decay = decay.clamp(0.0, 1.0);
    }

    /// Request access to the workspace
    /// Returns true if request was queued successfully
    pub fn request_access(&mut self, request: AccessRequest) -> bool {
        // Check if module already has access
        if self
            .module_locks
            .get(&request.module)
            .copied()
            .unwrap_or(false)
        {
            return false;
        }

        self.access_queue.push_back(request);
        true
    }

    /// Release workspace access for a module
    pub fn release(&mut self, module: ModuleId) {
        self.module_locks.remove(&module);
    }

    /// Update salience with temporal decay
    pub fn update_salience(&mut self, decay_dt: f32) {
        for item in &mut self.buffer {
            item.apply_decay(decay_dt);
        }

        // Remove expired items
        self.buffer.retain(|item| !item.is_expired(self.timestamp));

        // Remove items below threshold
        self.buffer
            .retain(|item| item.salience >= self.salience_threshold);
    }

    /// Attempt to broadcast a representation to the workspace
    ///
    /// # Returns
    /// * `true` if accepted into workspace
    /// * `false` if rejected (too low salience or capacity full with stronger items)
    pub fn broadcast(&mut self, mut rep: Representation) -> bool {
        self.timestamp += 1;
        rep.timestamp = self.timestamp;

        // Reject if below threshold
        if rep.salience < self.salience_threshold {
            return false;
        }

        // If workspace not full, add directly
        if self.buffer.len() < self.capacity {
            self.buffer.push(rep);
            return true;
        }

        // If full, compete with weakest item
        if let Some(min_idx) = self.find_weakest() {
            if self.buffer[min_idx].salience < rep.salience {
                // Replace weakest with new representation
                self.buffer.swap_remove(min_idx);
                self.buffer.push(rep);
                return true;
            }
        }

        false
    }

    /// Run competitive dynamics (salience decay and pruning)
    /// Returns items that survived competition
    pub fn compete(&mut self) -> Vec<WorkspaceItem> {
        // Apply salience decay to all representations
        for rep in self.buffer.iter_mut() {
            rep.salience *= self.salience_decay;
        }

        // Remove representations below threshold
        self.buffer
            .retain(|rep| rep.salience >= self.salience_threshold);

        // Return surviving items
        self.buffer.clone()
    }

    /// Retrieve all current representations (read-only access)
    pub fn retrieve(&self) -> Vec<Representation> {
        self.buffer.iter().cloned().collect()
    }

    /// Retrieve top-k most salient representations
    pub fn retrieve_top_k(&self, k: usize) -> Vec<Representation> {
        let mut reps = self.retrieve();
        // NaN-safe sorting: treat NaN salience as less than any value
        reps.sort_by(|a, b| {
            b.salience
                .partial_cmp(&a.salience)
                .unwrap_or(std::cmp::Ordering::Less)
        });
        reps.truncate(k);
        reps
    }

    /// Check if workspace is at capacity
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    /// Check if workspace is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get current number of representations
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Get workspace capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all representations
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Find representation by ID
    pub fn find(&self, id: u64) -> Option<&Representation> {
        self.buffer.iter().find(|rep| rep.id == id)
    }

    /// Get most salient representation
    pub fn most_salient(&self) -> Option<&Representation> {
        self.buffer.iter().max_by(|a, b| {
            a.salience
                .partial_cmp(&b.salience)
                .unwrap_or(std::cmp::Ordering::Less)
        })
    }

    /// Find index of weakest (least salient) representation
    fn find_weakest(&self) -> Option<usize> {
        if self.buffer.is_empty() {
            return None;
        }

        let mut min_idx = 0;
        let mut min_salience = self.buffer[0].salience;

        for (i, rep) in self.buffer.iter().enumerate().skip(1) {
            if rep.salience < min_salience {
                min_salience = rep.salience;
                min_idx = i;
            }
        }

        Some(min_idx)
    }

    /// Get average salience of all representations
    pub fn average_salience(&self) -> f32 {
        if self.buffer.is_empty() {
            return 0.0;
        }

        let sum: f32 = self.buffer.iter().map(|r| r.salience).sum();
        sum / self.buffer.len() as f32
    }

    /// Broadcast to specific target modules (new API method)
    pub fn broadcast_to(&mut self, item: WorkspaceItem, targets: &[ModuleId]) -> Vec<ModuleId> {
        if self.broadcast(item.clone()) {
            // Record broadcast event
            let event = BroadcastEvent::new(item, targets.to_vec(), self.timestamp);
            self.broadcast_history.push(event);
            targets.to_vec()
        } else {
            Vec::new()
        }
    }

    /// Retrieve all items (reference-based)
    pub fn retrieve_all(&self) -> Vec<&WorkspaceItem> {
        self.buffer.iter().collect()
    }

    /// Retrieve item by source module
    pub fn retrieve_by_module(&self, module: ModuleId) -> Option<&WorkspaceItem> {
        self.buffer.iter().find(|item| item.source_module == module)
    }

    /// Retrieve n most recent items
    pub fn retrieve_recent(&self, n: usize) -> Vec<&WorkspaceItem> {
        let mut items: Vec<&WorkspaceItem> = self.buffer.iter().collect();
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        items.truncate(n);
        items
    }

    /// Get number of available slots
    pub fn available_slots(&self) -> usize {
        self.capacity.saturating_sub(self.buffer.len())
    }

    /// Get current workspace load (0.0 to 1.0)
    pub fn current_load(&self) -> f32 {
        self.buffer.len() as f32 / self.capacity as f32
    }
}

/// Workspace registry for module management and routing
pub struct WorkspaceRegistry {
    modules: HashMap<ModuleId, ModuleInfo>,
    workspace: GlobalWorkspace,
    next_id: ModuleId,
}

impl WorkspaceRegistry {
    /// Create a new workspace registry
    pub fn new(workspace_capacity: usize) -> Self {
        Self {
            modules: HashMap::new(),
            workspace: GlobalWorkspace::new(workspace_capacity),
            next_id: 0,
        }
    }

    /// Register a new module and return its ID
    pub fn register(&mut self, mut info: ModuleInfo) -> ModuleId {
        let id = self.next_id;
        info.id = id;
        self.modules.insert(id, info);
        self.next_id += 1;
        id
    }

    /// Unregister a module
    pub fn unregister(&mut self, id: ModuleId) {
        self.modules.remove(&id);
        self.workspace.release(id);
    }

    /// Route item to subscribed modules based on content type
    pub fn route(&mut self, item: WorkspaceItem) -> Vec<ModuleId> {
        // For now, broadcast to all modules
        // In a full implementation, this would filter by ContentType subscriptions
        let recipients: Vec<ModuleId> = self.modules.keys().copied().collect();
        self.workspace.broadcast_to(item, &recipients)
    }

    /// Get workspace reference
    pub fn workspace(&self) -> &GlobalWorkspace {
        &self.workspace
    }

    /// Get mutable workspace reference
    pub fn workspace_mut(&mut self) -> &mut GlobalWorkspace {
        &mut self.workspace
    }

    /// Get module info
    pub fn get_module(&self, id: ModuleId) -> Option<&ModuleInfo> {
        self.modules.get(&id)
    }

    /// List all registered modules
    pub fn list_modules(&self) -> Vec<&ModuleInfo> {
        self.modules.values().collect()
    }
}

// Additional tests for new features
#[cfg(test)]
mod extended_tests {
    use super::*;

    #[test]
    fn test_access_request() {
        let mut workspace = GlobalWorkspace::new(5);

        let request1 = AccessRequest::new(1, vec![1.0, 2.0], 0.8, 0);
        assert!(workspace.request_access(request1));

        // Same module can queue another request (until it's processed and locked)
        let request2 = AccessRequest::new(1, vec![1.0, 2.0], 0.8, 1);
        assert!(workspace.request_access(request2));

        // But if we manually lock the module, future requests should fail
        workspace.module_locks.insert(1, true);
        let request3 = AccessRequest::new(1, vec![1.0, 2.0], 0.8, 2);
        assert!(!workspace.request_access(request3));
    }

    #[test]
    fn test_broadcast_to() {
        let mut workspace = GlobalWorkspace::new(5);

        let item = WorkspaceItem::new(vec![1.0], 0.8, 1, 0);
        let targets = vec![2, 3, 4];
        let recipients = workspace.broadcast_to(item, &targets);

        assert_eq!(recipients, targets);
    }

    #[test]
    fn test_retrieve_all() {
        let mut workspace = GlobalWorkspace::new(5);

        workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.8, 1, 0));
        workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.7, 2, 0));

        let all_items = workspace.retrieve_all();
        assert_eq!(all_items.len(), 2);
    }

    #[test]
    fn test_retrieve_by_module() {
        let mut workspace = GlobalWorkspace::new(5);

        workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.8, 1, 0));
        workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.7, 2, 0));

        let item = workspace.retrieve_by_module(1);
        assert!(item.is_some());
        assert_eq!(item.unwrap().source_module, 1);

        let not_found = workspace.retrieve_by_module(99);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_retrieve_recent() {
        let mut workspace = GlobalWorkspace::new(5);

        workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.8, 1, 0));
        workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.7, 2, 0));
        workspace.broadcast(WorkspaceItem::new(vec![3.0], 0.6, 3, 0));

        let recent = workspace.retrieve_recent(2);
        assert_eq!(recent.len(), 2);
        // Most recent should be first (timestamps are 3, 2, 1 after broadcast)
        assert!(recent[0].timestamp > recent[1].timestamp);
        assert_eq!(recent[0].source_module, 3); // Last broadcast
        assert_eq!(recent[1].source_module, 2); // Second to last
    }

    #[test]
    fn test_available_slots() {
        let mut workspace = GlobalWorkspace::new(5);
        assert_eq!(workspace.available_slots(), 5);

        workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.8, 1, 0));
        assert_eq!(workspace.available_slots(), 4);

        workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.7, 2, 0));
        assert_eq!(workspace.available_slots(), 3);
    }

    #[test]
    fn test_current_load() {
        let mut workspace = GlobalWorkspace::new(4);
        assert_eq!(workspace.current_load(), 0.0);

        workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.8, 1, 0));
        assert_eq!(workspace.current_load(), 0.25);

        workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.7, 2, 0));
        assert_eq!(workspace.current_load(), 0.5);
    }

    #[test]
    fn test_update_salience_decay() {
        let mut workspace = GlobalWorkspace::new(5);

        let item = WorkspaceItem::with_decay(vec![1.0], 0.8, 1, 0, 0.9, 1000);
        workspace.broadcast(item);

        workspace.update_salience(1.0);

        let items = workspace.retrieve();
        assert!(items[0].salience < 0.8);
    }

    #[test]
    fn test_workspace_registry() {
        let mut registry = WorkspaceRegistry::new(7);

        let module1 = ModuleInfo::new(0, "module1".to_string(), 1.0, vec![ContentType::Query]);
        let module2 = ModuleInfo::new(0, "module2".to_string(), 0.8, vec![ContentType::Result]);

        let id1 = registry.register(module1);
        let id2 = registry.register(module2);

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        assert_eq!(registry.list_modules().len(), 2);
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = WorkspaceRegistry::new(7);

        let module = ModuleInfo::new(0, "test".to_string(), 1.0, vec![]);
        let id = registry.register(module);

        assert!(registry.get_module(id).is_some());

        registry.unregister(id);

        assert!(registry.get_module(id).is_none());
    }

    #[test]
    fn test_registry_routing() {
        let mut registry = WorkspaceRegistry::new(7);

        let module1 = ModuleInfo::new(0, "mod1".to_string(), 1.0, vec![ContentType::Query]);
        let module2 = ModuleInfo::new(0, "mod2".to_string(), 0.8, vec![ContentType::Result]);

        registry.register(module1);
        registry.register(module2);

        let item = WorkspaceItem::new(vec![1.0, 2.0], 0.9, 0, 0);
        let routed = registry.route(item);

        assert_eq!(routed.len(), 2);
    }

    #[test]
    fn test_item_decay_and_expiry() {
        let mut item = WorkspaceItem::with_decay(vec![1.0], 0.8, 1, 0, 0.9, 100);

        // Apply decay
        item.apply_decay(1.0);
        assert!((item.salience - 0.72).abs() < 0.01); // 0.8 * 0.9

        // Check expiry
        assert!(!item.is_expired(50));
        assert!(item.is_expired(150));
    }

    #[test]
    fn test_capacity_enforcement() {
        let mut workspace = GlobalWorkspace::new(4);

        for i in 0..4 {
            let item = WorkspaceItem::new(vec![1.0], 0.8, i, 0);
            assert!(workspace.broadcast(item));
        }

        assert!(workspace.is_full());
        assert_eq!(workspace.available_slots(), 0);
        assert_eq!(workspace.current_load(), 1.0);
    }

    #[test]
    fn test_competition_fairness() {
        let mut workspace = GlobalWorkspace::new(3);

        // Add items with different saliences
        workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.9, 1, 0));
        workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.5, 2, 0));
        workspace.broadcast(WorkspaceItem::new(vec![3.0], 0.7, 3, 0));

        // Competition should keep high-salience items
        workspace.compete();

        let items = workspace.retrieve();
        for item in items {
            assert!(item.salience >= 0.1); // Above threshold
        }
    }

    #[test]
    fn test_performance_access_request() {
        let mut workspace = GlobalWorkspace::new(100);

        let start = std::time::Instant::now();
        for i in 0..1000 {
            let request = AccessRequest::new(i, vec![1.0], 0.8, i as u64);
            workspace.request_access(request);
        }
        let elapsed = start.elapsed();

        // Should be < 1μs per request on average
        let avg_us = elapsed.as_micros() / 1000;
        assert!(avg_us < 10, "Access request too slow: {}μs", avg_us);
    }

    #[test]
    fn test_performance_broadcast() {
        let mut workspace = GlobalWorkspace::new(100);

        let start = std::time::Instant::now();
        for i in 0..100 {
            let item = WorkspaceItem::new(vec![1.0; 128], 0.8, i, 0);
            workspace.broadcast(item);
        }
        let elapsed = start.elapsed();

        // Should be fast even with 128-dim vectors
        let avg_us = elapsed.as_micros() / 100;
        assert!(avg_us < 100, "Broadcast too slow: {}μs", avg_us);
    }
}
mod tests {
    use super::*;

    #[test]
    fn test_new_workspace() {
        let workspace = GlobalWorkspace::new(7);

        assert_eq!(workspace.capacity(), 7);
        assert_eq!(workspace.len(), 0);
        assert!(workspace.is_empty());
        assert!(!workspace.is_full());
    }

    #[test]
    fn test_representation_creation() {
        let rep = Representation::new(vec![1.0, 2.0, 3.0], 0.8, 0, 100);

        assert_eq!(rep.content.len(), 3);
        assert_eq!(rep.salience, 0.8);
        assert_eq!(rep.source_module, 0);
        assert_eq!(rep.timestamp, 100);
    }

    #[test]
    fn test_representation_magnitude() {
        let rep = Representation::new(vec![3.0, 4.0], 1.0, 0, 0);

        // Magnitude of [3,4] should be 5
        assert!((rep.magnitude() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_broadcast_accept() {
        let mut workspace = GlobalWorkspace::new(3);

        let rep = Representation::new(vec![1.0], 0.5, 0, 0);
        let accepted = workspace.broadcast(rep);

        assert!(accepted);
        assert_eq!(workspace.len(), 1);
    }

    #[test]
    fn test_broadcast_reject_low_salience() {
        let mut workspace = GlobalWorkspace::with_threshold(3, 0.5);

        let weak_rep = Representation::new(vec![1.0], 0.3, 0, 0);
        let accepted = workspace.broadcast(weak_rep);

        assert!(!accepted);
        assert_eq!(workspace.len(), 0);
    }

    #[test]
    fn test_competitive_replacement() {
        let mut workspace = GlobalWorkspace::new(2);

        // Fill workspace with weak representations
        let rep1 = Representation::new(vec![1.0], 0.3, 0, 0);
        let rep2 = Representation::new(vec![1.0], 0.4, 1, 0);
        workspace.broadcast(rep1);
        workspace.broadcast(rep2);

        assert_eq!(workspace.len(), 2);
        assert!(workspace.is_full());

        // Try to broadcast strong representation
        let strong_rep = Representation::new(vec![1.0], 0.9, 2, 0);
        let accepted = workspace.broadcast(strong_rep);

        assert!(accepted);
        assert_eq!(workspace.len(), 2); // Still at capacity

        // Weakest should have been replaced
        let reps = workspace.retrieve();
        assert!(reps.iter().any(|r| r.salience == 0.9));
        assert!(reps.iter().all(|r| r.salience >= 0.4)); // Weakest (0.3) removed
    }

    #[test]
    fn test_competition_decay() {
        let mut workspace = GlobalWorkspace::new(3);
        workspace.set_decay_rate(0.9);

        let rep = Representation::new(vec![1.0], 0.5, 0, 0);
        workspace.broadcast(rep);

        let initial_salience = workspace.retrieve()[0].salience;

        workspace.compete();

        let final_salience = workspace.retrieve()[0].salience;

        // Salience should decay
        assert!((final_salience - initial_salience * 0.9).abs() < 0.001);
    }

    #[test]
    fn test_competition_pruning() {
        let mut workspace = GlobalWorkspace::with_threshold(3, 0.2);
        workspace.set_decay_rate(0.5);

        let rep = Representation::new(vec![1.0], 0.3, 0, 0);
        workspace.broadcast(rep);

        assert_eq!(workspace.len(), 1);

        // After one competition, salience = 0.3 * 0.5 = 0.15 < threshold
        workspace.compete();

        assert_eq!(workspace.len(), 0); // Should be pruned
    }

    #[test]
    fn test_retrieve_top_k() {
        let mut workspace = GlobalWorkspace::new(5);

        workspace.broadcast(Representation::new(vec![1.0], 0.5, 0, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.8, 1, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.3, 2, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.9, 3, 0));

        let top2 = workspace.retrieve_top_k(2);

        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].salience, 0.9);
        assert_eq!(top2[1].salience, 0.8);
    }

    #[test]
    fn test_most_salient() {
        let mut workspace = GlobalWorkspace::new(5);

        workspace.broadcast(Representation::new(vec![1.0], 0.5, 0, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.8, 1, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.3, 2, 0));

        let most = workspace.most_salient().unwrap();

        assert_eq!(most.salience, 0.8);
        assert_eq!(most.source_module, 1);
    }

    #[test]
    fn test_find_by_id() {
        let mut workspace = GlobalWorkspace::new(3);

        let rep = Representation::new(vec![1.0], 0.5, 0, 123);
        workspace.broadcast(rep);

        // ID is set when item is created (id = timestamp at creation = 123)
        // Even though broadcast() updates the timestamp to 1, the id remains 123
        let found = workspace.find(123);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 123);
        assert_eq!(found.unwrap().timestamp, 1); // Timestamp updated by broadcast
        assert_eq!(found.unwrap().source_module, 0);

        let not_found = workspace.find(999);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_average_salience() {
        let mut workspace = GlobalWorkspace::new(5);

        workspace.broadcast(Representation::new(vec![1.0], 0.4, 0, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.6, 1, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.5, 2, 0));

        let avg = workspace.average_salience();

        assert!((avg - 0.5).abs() < 0.001); // (0.4 + 0.6 + 0.5) / 3 = 0.5
    }

    #[test]
    fn test_capacity_limit() {
        let mut workspace = GlobalWorkspace::new(4);

        // Fill to capacity with strong representations
        for i in 0..4 {
            let rep = Representation::new(vec![1.0], 0.9, i, 0);
            workspace.broadcast(rep);
        }

        assert!(workspace.is_full());

        // Try to add weak representation - should fail
        let weak = Representation::new(vec![1.0], 0.5, 99, 0);
        let accepted = workspace.broadcast(weak);

        assert!(!accepted);
        assert_eq!(workspace.len(), 4);
    }

    #[test]
    fn test_typical_capacity_seven() {
        // Miller's Law: 7±2 items in working memory
        let workspace = GlobalWorkspace::new(7);
        assert_eq!(workspace.capacity(), 7);
    }

    #[test]
    fn test_clear() {
        let mut workspace = GlobalWorkspace::new(3);

        workspace.broadcast(Representation::new(vec![1.0], 0.5, 0, 0));
        workspace.broadcast(Representation::new(vec![1.0], 0.6, 1, 0));

        assert_eq!(workspace.len(), 2);

        workspace.clear();

        assert_eq!(workspace.len(), 0);
        assert!(workspace.is_empty());
    }

    #[test]
    fn test_update_salience() {
        let mut rep = Representation::new(vec![1.0], 0.5, 0, 0);

        rep.update_salience(0.8);
        assert_eq!(rep.salience, 0.8);

        // Should clamp negative values to 0
        rep.update_salience(-0.5);
        assert_eq!(rep.salience, 0.0);
    }
}
