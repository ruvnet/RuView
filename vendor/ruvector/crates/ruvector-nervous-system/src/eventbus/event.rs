//! Event Types and Trait Definitions
//!
//! Implements DVS (Dynamic Vision Sensor) events and sparse event surfaces.

use std::sync::atomic::{AtomicU64, Ordering};

/// Core event trait for timestamped event streams
pub trait Event: Send + Sync {
    /// Get event timestamp (microseconds)
    fn timestamp(&self) -> u64;

    /// Get source identifier (e.g., pixel coordinate hash)
    fn source_id(&self) -> u16;

    /// Get event payload/data
    fn payload(&self) -> u32;
}

/// Dynamic Vision Sensor event
///
/// Represents a single event from a DVS camera or general event source.
/// Typically 10-1000× more efficient than frame-based data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DVSEvent {
    /// Event timestamp in microseconds
    pub timestamp: u64,

    /// Source identifier (e.g., pixel index or sensor ID)
    pub source_id: u16,

    /// Payload data (application-specific)
    pub payload_id: u32,

    /// Polarity (on/off, increase/decrease)
    pub polarity: bool,

    /// Optional confidence score
    pub confidence: Option<f32>,
}

impl Event for DVSEvent {
    #[inline]
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    #[inline]
    fn source_id(&self) -> u16 {
        self.source_id
    }

    #[inline]
    fn payload(&self) -> u32 {
        self.payload_id
    }
}

impl DVSEvent {
    /// Create a new DVS event
    pub fn new(timestamp: u64, source_id: u16, payload_id: u32, polarity: bool) -> Self {
        Self {
            timestamp,
            source_id,
            payload_id,
            polarity,
            confidence: None,
        }
    }

    /// Create event with confidence score
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }
}

/// Sparse event surface for tracking last event per source
///
/// Efficiently tracks active events across a 2D surface (e.g., DVS camera pixels)
/// using atomic operations for lock-free updates.
pub struct EventSurface {
    surface: Vec<AtomicU64>,
    width: usize,
    height: usize,
}

impl EventSurface {
    /// Create new event surface
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        let mut surface = Vec::with_capacity(size);
        for _ in 0..size {
            surface.push(AtomicU64::new(0));
        }

        Self {
            surface,
            width,
            height,
        }
    }

    /// Update surface with new event
    #[inline]
    pub fn update(&self, event: &DVSEvent) {
        let idx = event.source_id as usize;
        if idx < self.surface.len() {
            self.surface[idx].store(event.timestamp, Ordering::Relaxed);
        }
    }

    /// Get all events that occurred since timestamp
    pub fn get_active_events(&self, since: u64) -> Vec<(usize, usize, u64)> {
        let mut active = Vec::new();

        for (idx, timestamp_atom) in self.surface.iter().enumerate() {
            let timestamp = timestamp_atom.load(Ordering::Relaxed);
            if timestamp > since {
                let x = idx % self.width;
                let y = idx / self.width;
                active.push((x, y, timestamp));
            }
        }

        active
    }

    /// Get timestamp at specific coordinate
    pub fn get_timestamp(&self, x: usize, y: usize) -> Option<u64> {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            Some(self.surface[idx].load(Ordering::Relaxed))
        } else {
            None
        }
    }

    /// Clear all events
    pub fn clear(&self) {
        for atom in &self.surface {
            atom.store(0, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvs_event_creation() {
        let event = DVSEvent::new(1000, 42, 123, true);
        assert_eq!(event.timestamp(), 1000);
        assert_eq!(event.source_id(), 42);
        assert_eq!(event.payload(), 123);
        assert_eq!(event.polarity, true);
        assert_eq!(event.confidence, None);
    }

    #[test]
    fn test_dvs_event_with_confidence() {
        let event = DVSEvent::new(1000, 42, 123, false).with_confidence(0.95);

        assert_eq!(event.confidence, Some(0.95));
    }

    #[test]
    fn test_event_surface_update() {
        let surface = EventSurface::new(640, 480);

        let event1 = DVSEvent::new(1000, 0, 0, true);
        let event2 = DVSEvent::new(2000, 100, 0, false);

        surface.update(&event1);
        surface.update(&event2);

        assert_eq!(surface.get_timestamp(0, 0), Some(1000));
        assert_eq!(surface.get_timestamp(100, 0), Some(2000));
    }

    #[test]
    fn test_event_surface_active_events() {
        let surface = EventSurface::new(10, 10);

        // Add events at different times
        for i in 0..5 {
            let event = DVSEvent::new(1000 + i * 100, i as u16, 0, true);
            surface.update(&event);
        }

        // Query events since timestamp 1200
        let active = surface.get_active_events(1200);
        assert_eq!(active.len(), 2); // Events at 1300 and 1400
    }

    #[test]
    fn test_event_surface_clear() {
        let surface = EventSurface::new(10, 10);

        let event = DVSEvent::new(1000, 5, 0, true);
        surface.update(&event);

        assert_eq!(surface.get_timestamp(5, 0), Some(1000));

        surface.clear();
        assert_eq!(surface.get_timestamp(5, 0), Some(0));
    }

    #[test]
    fn test_event_surface_bounds() {
        let surface = EventSurface::new(10, 10);

        // Out of bounds should return None
        assert_eq!(surface.get_timestamp(10, 0), None);
        assert_eq!(surface.get_timestamp(0, 10), None);
    }
}
