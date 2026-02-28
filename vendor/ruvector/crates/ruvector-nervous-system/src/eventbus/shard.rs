//! Region-Based Event Bus Sharding
//!
//! Spatial/temporal partitioning for parallel event processing.

use super::event::Event;
use super::queue::EventRingBuffer;

/// Sharded event bus for parallel processing
///
/// Distributes events across multiple lock-free queues based on
/// spatial/temporal characteristics for improved throughput.
pub struct ShardedEventBus<E: Event + Copy> {
    shards: Vec<EventRingBuffer<E>>,
    shard_fn: Box<dyn Fn(&E) -> usize + Send + Sync>,
}

impl<E: Event + Copy> ShardedEventBus<E> {
    /// Create new sharded event bus
    ///
    /// # Arguments
    /// * `num_shards` - Number of shards (typically power of 2)
    /// * `shard_capacity` - Capacity per shard
    /// * `shard_fn` - Function to compute shard index from event
    pub fn new(
        num_shards: usize,
        shard_capacity: usize,
        shard_fn: impl Fn(&E) -> usize + Send + Sync + 'static,
    ) -> Self {
        assert!(num_shards > 0, "Must have at least one shard");
        assert!(
            shard_capacity.is_power_of_two(),
            "Shard capacity must be power of 2"
        );

        let shards = (0..num_shards)
            .map(|_| EventRingBuffer::new(shard_capacity))
            .collect();

        Self {
            shards,
            shard_fn: Box::new(shard_fn),
        }
    }

    /// Create spatial sharding (by source_id)
    pub fn new_spatial(num_shards: usize, shard_capacity: usize) -> Self {
        Self::new(num_shards, shard_capacity, move |event| {
            event.source_id() as usize % num_shards
        })
    }

    /// Create temporal sharding (by timestamp ranges)
    ///
    /// # Panics
    ///
    /// Panics if `window_size` is 0 (would cause division by zero).
    pub fn new_temporal(num_shards: usize, shard_capacity: usize, window_size: u64) -> Self {
        assert!(
            window_size > 0,
            "window_size must be > 0 to avoid division by zero"
        );
        Self::new(num_shards, shard_capacity, move |event| {
            ((event.timestamp() / window_size) as usize) % num_shards
        })
    }

    /// Create hybrid sharding (spatial + temporal)
    ///
    /// # Panics
    ///
    /// Panics if `window_size` is 0 (would cause division by zero).
    pub fn new_hybrid(num_shards: usize, shard_capacity: usize, window_size: u64) -> Self {
        assert!(
            window_size > 0,
            "window_size must be > 0 to avoid division by zero"
        );
        Self::new(num_shards, shard_capacity, move |event| {
            let spatial = event.source_id() as usize;
            let temporal = (event.timestamp() / window_size) as usize;
            (spatial ^ temporal) % num_shards
        })
    }

    /// Push event to appropriate shard
    #[inline]
    pub fn push(&self, event: E) -> Result<(), E> {
        let shard_idx = (self.shard_fn)(&event) % self.shards.len();
        self.shards[shard_idx].push(event)
    }

    /// Pop event from specific shard
    #[inline]
    pub fn pop_shard(&self, shard: usize) -> Option<E> {
        if shard < self.shards.len() {
            self.shards[shard].pop()
        } else {
            None
        }
    }

    /// Drain all events from a shard
    pub fn drain_shard(&self, shard: usize) -> Vec<E> {
        if shard >= self.shards.len() {
            return Vec::new();
        }

        let mut events = Vec::new();
        while let Some(event) = self.shards[shard].pop() {
            events.push(event);
        }
        events
    }

    /// Get number of shards
    pub fn num_shards(&self) -> usize {
        self.shards.len()
    }

    /// Get events in specific shard
    pub fn shard_len(&self, shard: usize) -> usize {
        if shard < self.shards.len() {
            self.shards[shard].len()
        } else {
            0
        }
    }

    /// Get total events across all shards
    pub fn total_len(&self) -> usize {
        self.shards.iter().map(|s| s.len()).sum()
    }

    /// Get fill ratio for specific shard
    pub fn shard_fill_ratio(&self, shard: usize) -> f32 {
        if shard < self.shards.len() {
            self.shards[shard].fill_ratio()
        } else {
            0.0
        }
    }

    /// Get average fill ratio across all shards
    pub fn avg_fill_ratio(&self) -> f32 {
        if self.shards.is_empty() {
            return 0.0;
        }

        let total: f32 = self.shards.iter().map(|s| s.fill_ratio()).sum();

        total / self.shards.len() as f32
    }

    /// Get max fill ratio across all shards
    pub fn max_fill_ratio(&self) -> f32 {
        self.shards
            .iter()
            .map(|s| s.fill_ratio())
            .fold(0.0f32, |a, b| a.max(b))
    }

    /// Check if any shard is full
    pub fn any_full(&self) -> bool {
        self.shards.iter().any(|s| s.is_full())
    }

    /// Check if all shards are empty
    pub fn all_empty(&self) -> bool {
        self.shards.iter().all(|s| s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::event::DVSEvent;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_sharded_bus_creation() {
        let bus: ShardedEventBus<DVSEvent> = ShardedEventBus::new_spatial(4, 256);
        assert_eq!(bus.num_shards(), 4);
        assert_eq!(bus.total_len(), 0);
        assert!(bus.all_empty());
    }

    #[test]
    fn test_spatial_sharding() {
        let bus = ShardedEventBus::new_spatial(4, 256);

        // Events with same source_id % 4 should go to same shard
        let event1 = DVSEvent::new(1000, 0, 0, true); // shard 0
        let event2 = DVSEvent::new(1001, 4, 0, true); // shard 0
        let event3 = DVSEvent::new(1002, 1, 0, true); // shard 1

        bus.push(event1).unwrap();
        bus.push(event2).unwrap();
        bus.push(event3).unwrap();

        assert_eq!(bus.shard_len(0), 2);
        assert_eq!(bus.shard_len(1), 1);
        assert_eq!(bus.shard_len(2), 0);
        assert_eq!(bus.total_len(), 3);
    }

    #[test]
    fn test_temporal_sharding() {
        let window_size = 1000;
        let bus = ShardedEventBus::new_temporal(4, 256, window_size);

        // Events in different time windows
        let event1 = DVSEvent::new(500, 0, 0, true); // window 0, shard 0
        let event2 = DVSEvent::new(1500, 0, 0, true); // window 1, shard 1
        let event3 = DVSEvent::new(2500, 0, 0, true); // window 2, shard 2

        bus.push(event1).unwrap();
        bus.push(event2).unwrap();
        bus.push(event3).unwrap();

        assert_eq!(bus.total_len(), 3);
        // Each should be in different shard (or same based on modulo)
    }

    #[test]
    fn test_hybrid_sharding() {
        let bus = ShardedEventBus::new_hybrid(8, 256, 1000);

        // Hybrid combines spatial and temporal
        for i in 0..100 {
            let event = DVSEvent::new(i * 10, (i % 20) as u16, 0, true);
            bus.push(event).unwrap();
        }

        assert_eq!(bus.total_len(), 100);
        // Events should be distributed across shards
        assert!(!bus.all_empty());
    }

    #[test]
    fn test_pop_from_shard() {
        let bus = ShardedEventBus::new_spatial(4, 256);

        let event = DVSEvent::new(1000, 0, 42, true);
        bus.push(event).unwrap();

        // Pop from correct shard (source_id 0 % 4 = 0)
        let popped = bus.pop_shard(0).unwrap();
        assert_eq!(popped.timestamp(), 1000);
        assert_eq!(popped.payload(), 42);

        // Other shards should be empty
        assert!(bus.pop_shard(1).is_none());
        assert!(bus.pop_shard(2).is_none());
    }

    #[test]
    fn test_drain_shard() {
        let bus = ShardedEventBus::new_spatial(4, 256);

        // Add multiple events to shard 0
        for i in 0..10 {
            let event = DVSEvent::new(i as u64, 0, i as u32, true);
            bus.push(event).unwrap();
        }

        let drained = bus.drain_shard(0);
        assert_eq!(drained.len(), 10);
        assert_eq!(bus.shard_len(0), 0);

        // Verify order
        for (i, event) in drained.iter().enumerate() {
            assert_eq!(event.timestamp(), i as u64);
        }
    }

    #[test]
    fn test_fill_ratios() {
        let bus = ShardedEventBus::new_spatial(4, 16);

        // Fill shard 0 to 50%
        for i in 0..7 {
            // 7 events in capacity 16 ≈ 50%
            bus.push(DVSEvent::new(i, 0, 0, true)).unwrap();
        }

        let fill = bus.shard_fill_ratio(0);
        assert!(fill > 0.4 && fill < 0.5);

        assert_eq!(bus.avg_fill_ratio(), fill / 4.0);
        assert_eq!(bus.max_fill_ratio(), fill);
    }

    #[test]
    fn test_custom_shard_function() {
        // Shard by payload value
        let bus = ShardedEventBus::new(4, 256, |event: &DVSEvent| event.payload() as usize);

        let event1 = DVSEvent::new(1000, 0, 0, true); // shard 0
        let event2 = DVSEvent::new(1001, 0, 5, true); // shard 1
        let event3 = DVSEvent::new(1002, 0, 10, true); // shard 2

        bus.push(event1).unwrap();
        bus.push(event2).unwrap();
        bus.push(event3).unwrap();

        assert_eq!(bus.shard_len(0), 1);
        assert_eq!(bus.shard_len(1), 1);
        assert_eq!(bus.shard_len(2), 1);
    }

    #[test]
    fn test_parallel_shard_processing() {
        let bus = Arc::new(ShardedEventBus::new_spatial(4, 1024));
        let mut consumer_handles = vec![];

        // Producer: push 1000 events
        let bus_clone = bus.clone();
        let producer = thread::spawn(move || {
            for i in 0..1000 {
                let event = DVSEvent::new(i, (i % 256) as u16, 0, true);
                while bus_clone.push(event).is_err() {
                    thread::yield_now();
                }
            }
        });

        // Consumers: one per shard
        for shard_id in 0..4 {
            let bus_clone = bus.clone();
            consumer_handles.push(thread::spawn(move || {
                let mut count = 0;
                loop {
                    if let Some(_event) = bus_clone.pop_shard(shard_id) {
                        count += 1;
                    } else if bus_clone.all_empty() {
                        break;
                    } else {
                        thread::yield_now();
                    }
                }
                count
            }));
        }

        // Wait for producer
        producer.join().unwrap();

        // Wait for all consumers and sum counts
        let total: usize = consumer_handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .sum();

        assert_eq!(total, 1000);
        assert!(bus.all_empty());
    }

    #[test]
    fn test_shard_distribution() {
        let bus = ShardedEventBus::new_spatial(8, 256);

        // Push 1000 events with random source_ids
        for i in 0..1000 {
            let event = DVSEvent::new(i, (i % 256) as u16, 0, true);
            bus.push(event).unwrap();
        }

        // Verify distribution is reasonably balanced
        let avg = bus.total_len() / bus.num_shards();
        for shard in 0..bus.num_shards() {
            let len = bus.shard_len(shard);
            // Should be within 50% of average
            assert!(len > avg / 2 && len < avg * 2);
        }
    }
}
