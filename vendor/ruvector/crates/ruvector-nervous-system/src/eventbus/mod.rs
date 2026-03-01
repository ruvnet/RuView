//! Event Bus Module - DVS Event Stream Processing
//!
//! Provides lock-free event queues, region-based sharding, and backpressure management
//! for high-throughput event processing (10,000+ events/millisecond).

pub mod backpressure;
pub mod event;
pub mod queue;
pub mod shard;

pub use backpressure::{BackpressureController, BackpressureState};
pub use event::{DVSEvent, Event, EventSurface};
pub use queue::EventRingBuffer;
pub use shard::ShardedEventBus;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _event = DVSEvent {
            timestamp: 0,
            source_id: 0,
            payload_id: 0,
            polarity: true,
            confidence: None,
        };

        let _buffer: EventRingBuffer<DVSEvent> = EventRingBuffer::new(1024);
        let _controller = BackpressureController::new(0.8, 0.2);
        let _surface = EventSurface::new(640, 480);
    }
}
