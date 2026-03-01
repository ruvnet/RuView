//! Lock-Free Ring Buffer for Event Queues
//!
//! High-performance SPSC/MPSC ring buffer with <100ns push/pop operations.

use super::event::Event;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free ring buffer for event storage
///
/// Optimized for Single-Producer-Single-Consumer (SPSC) pattern
/// with atomic head/tail pointers for wait-free operations.
///
/// # Thread Safety
///
/// This buffer is designed for SPSC (Single-Producer-Single-Consumer) use.
/// While it is `Send + Sync`, concurrent multi-producer or multi-consumer
/// access may lead to data races or lost events. For MPSC patterns,
/// use external synchronization or the `ShardedEventBus` which provides
/// isolation through sharding.
///
/// # Memory Ordering
///
/// - Producer writes data before publishing tail (Release)
/// - Consumer reads head with Acquire before accessing data
/// - This ensures data visibility across threads in SPSC mode
pub struct EventRingBuffer<E: Event + Copy> {
    buffer: Vec<UnsafeCell<E>>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
}

// Safety: UnsafeCell is only accessed via atomic synchronization
unsafe impl<E: Event + Copy> Send for EventRingBuffer<E> {}
unsafe impl<E: Event + Copy> Sync for EventRingBuffer<E> {}

impl<E: Event + Copy> EventRingBuffer<E> {
    /// Create new ring buffer with specified capacity
    ///
    /// Capacity must be power of 2 for efficient modulo operations.
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0 && capacity.is_power_of_two(),
            "Capacity must be power of 2"
        );

        // Initialize with default events (timestamp 0)
        let buffer: Vec<UnsafeCell<E>> = (0..capacity)
            .map(|_| {
                // Create a dummy event with zero values
                // This is safe because E: Copy and we'll overwrite before reading
                unsafe { std::mem::zeroed() }
            })
            .map(UnsafeCell::new)
            .collect();

        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
        }
    }

    /// Push event to buffer
    ///
    /// Returns Err(event) if buffer is full.
    /// Time complexity: O(1), typically <100ns
    #[inline]
    pub fn push(&self, event: E) -> Result<(), E> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & (self.capacity - 1);

        // Check if full
        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(event);
        }

        // Safe: we own this slot until tail is updated
        unsafe {
            *self.buffer[tail].get() = event;
        }

        // Make event visible to consumer
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    /// Pop event from buffer
    ///
    /// Returns None if buffer is empty.
    /// Time complexity: O(1), typically <100ns
    #[inline]
    pub fn pop(&self) -> Option<E> {
        let head = self.head.load(Ordering::Relaxed);

        // Check if empty
        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        // Safe: we own this slot until head is updated
        let event = unsafe { *self.buffer[head].get() };

        let next_head = (head + 1) & (self.capacity - 1);

        // Make slot available to producer
        self.head.store(next_head, Ordering::Release);
        Some(event)
    }

    /// Get current number of events in buffer
    #[inline]
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        if tail >= head {
            tail - head
        } else {
            self.capacity - head + tail
        }
    }

    /// Check if buffer is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    /// Check if buffer is full
    #[inline]
    pub fn is_full(&self) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & (self.capacity - 1);
        next_tail == self.head.load(Ordering::Acquire)
    }

    /// Get buffer capacity
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get fill percentage (0.0 to 1.0)
    pub fn fill_ratio(&self) -> f32 {
        self.len() as f32 / self.capacity as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::event::DVSEvent;
    use std::thread;

    #[test]
    fn test_ring_buffer_creation() {
        let buffer: EventRingBuffer<DVSEvent> = EventRingBuffer::new(1024);
        assert_eq!(buffer.capacity(), 1024);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }

    #[test]
    #[should_panic]
    fn test_non_power_of_two_capacity() {
        let _: EventRingBuffer<DVSEvent> = EventRingBuffer::new(1000);
    }

    #[test]
    fn test_push_pop_single() {
        let buffer = EventRingBuffer::new(16);
        let event = DVSEvent::new(1000, 42, 123, true);

        assert!(buffer.push(event).is_ok());
        assert_eq!(buffer.len(), 1);

        let popped = buffer.pop().unwrap();
        assert_eq!(popped.timestamp(), 1000);
        assert_eq!(popped.source_id(), 42);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_push_until_full() {
        let buffer = EventRingBuffer::new(4);

        // Can push capacity-1 events
        for i in 0..3 {
            let event = DVSEvent::new(i as u64, i as u16, 0, true);
            assert!(buffer.push(event).is_ok());
        }

        assert!(buffer.is_full());

        // Next push should fail
        let event = DVSEvent::new(999, 999, 0, true);
        assert!(buffer.push(event).is_err());
    }

    #[test]
    fn test_fifo_order() {
        let buffer = EventRingBuffer::new(16);

        // Push events with different timestamps
        for i in 0..10 {
            let event = DVSEvent::new(i as u64, i as u16, i as u32, true);
            buffer.push(event).unwrap();
        }

        // Pop and verify order
        for i in 0..10 {
            let event = buffer.pop().unwrap();
            assert_eq!(event.timestamp(), i as u64);
        }
    }

    #[test]
    fn test_wrap_around() {
        let buffer = EventRingBuffer::new(4);

        // Fill buffer
        for i in 0..3 {
            buffer.push(DVSEvent::new(i, 0, 0, true)).unwrap();
        }

        // Pop 2
        buffer.pop();
        buffer.pop();

        // Push 2 more (wraps around)
        buffer.push(DVSEvent::new(100, 0, 0, true)).unwrap();
        buffer.push(DVSEvent::new(101, 0, 0, true)).unwrap();

        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_fill_ratio() {
        let buffer = EventRingBuffer::new(8);

        assert_eq!(buffer.fill_ratio(), 0.0);

        buffer.push(DVSEvent::new(0, 0, 0, true)).unwrap();
        buffer.push(DVSEvent::new(1, 0, 0, true)).unwrap();

        assert!((buffer.fill_ratio() - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_spsc_threaded() {
        let buffer = std::sync::Arc::new(EventRingBuffer::new(1024));
        let buffer_clone = buffer.clone();

        const NUM_EVENTS: usize = 10000;

        // Producer thread
        let producer = thread::spawn(move || {
            for i in 0..NUM_EVENTS {
                let event = DVSEvent::new(i as u64, (i % 256) as u16, i as u32, true);
                while buffer_clone.push(event).is_err() {
                    std::hint::spin_loop();
                }
            }
        });

        // Consumer thread
        let consumer = thread::spawn(move || {
            let mut count = 0;
            let mut last_timestamp = 0u64;

            while count < NUM_EVENTS {
                if let Some(event) = buffer.pop() {
                    assert!(event.timestamp() >= last_timestamp);
                    last_timestamp = event.timestamp();
                    count += 1;
                }
            }
            count
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();
        assert_eq!(received, NUM_EVENTS);
    }

    #[test]
    fn test_concurrent_push_pop() {
        let buffer = std::sync::Arc::new(EventRingBuffer::new(512));
        let mut handles = vec![];

        // Producer
        let buf = buffer.clone();
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                let event = DVSEvent::new(i, 0, 0, true);
                while buf.push(event).is_err() {
                    thread::yield_now();
                }
            }
        }));

        // Consumer
        let buf = buffer.clone();
        let consumer_handle = thread::spawn(move || {
            let mut count = 0;
            while count < 1000 {
                if buf.pop().is_some() {
                    count += 1;
                }
            }
            count
        });

        for handle in handles {
            handle.join().unwrap();
        }

        let received = consumer_handle.join().unwrap();
        assert_eq!(received, 1000);
        assert!(buffer.is_empty());
    }
}
