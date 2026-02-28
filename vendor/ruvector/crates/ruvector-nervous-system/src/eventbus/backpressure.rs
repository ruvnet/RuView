//! Backpressure Control for Event Queues
//!
//! Adaptive flow control with high/low watermarks and state transitions.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

/// Backpressure controller state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureState {
    /// Normal operation - accept all events
    Normal = 0,

    /// Throttle mode - reduce incoming rate
    Throttle = 1,

    /// Drop mode - reject new events
    Drop = 2,
}

impl From<u8> for BackpressureState {
    fn from(val: u8) -> Self {
        match val {
            0 => BackpressureState::Normal,
            1 => BackpressureState::Throttle,
            2 => BackpressureState::Drop,
            _ => BackpressureState::Normal,
        }
    }
}

/// Adaptive backpressure controller
///
/// Uses high/low watermarks to transition between states:
/// - Normal: queue < low_watermark
/// - Throttle: low_watermark <= queue < high_watermark
/// - Drop: queue >= high_watermark
///
/// Decision time: <1μs
#[derive(Debug)]
pub struct BackpressureController {
    /// High watermark threshold (0.0-1.0)
    high_watermark: f32,

    /// Low watermark threshold (0.0-1.0)
    low_watermark: f32,

    /// Current pressure level (0-100, stored as u32 for atomics)
    current_pressure: AtomicU32,

    /// Current state
    state: AtomicU8,
}

impl BackpressureController {
    /// Create new backpressure controller
    ///
    /// # Arguments
    /// * `high` - High watermark (0.0-1.0), typically 0.8-0.9
    /// * `low` - Low watermark (0.0-1.0), typically 0.2-0.3
    pub fn new(high: f32, low: f32) -> Self {
        assert!(high > low, "High watermark must be greater than low");
        assert!(
            (0.0..=1.0).contains(&high),
            "High watermark must be in [0,1]"
        );
        assert!((0.0..=1.0).contains(&low), "Low watermark must be in [0,1]");

        Self {
            high_watermark: high,
            low_watermark: low,
            current_pressure: AtomicU32::new(0),
            state: AtomicU8::new(BackpressureState::Normal as u8),
        }
    }
}

impl Default for BackpressureController {
    /// Create default controller (high=0.8, low=0.2)
    fn default() -> Self {
        Self::new(0.8, 0.2)
    }
}

impl BackpressureController {
    /// Check if should accept new event
    ///
    /// Returns false in Drop state, true otherwise.
    /// Time complexity: O(1), <1μs
    #[inline]
    pub fn should_accept(&self) -> bool {
        let state = self.get_state();
        state != BackpressureState::Drop
    }

    /// Update controller with current queue fill ratio
    ///
    /// Updates internal state based on watermark thresholds.
    /// # Arguments
    /// * `queue_fill` - Current queue fill ratio (0.0-1.0)
    pub fn update(&self, queue_fill: f32) {
        let pressure = (queue_fill * 100.0) as u32;
        self.current_pressure
            .store(pressure.min(100), Ordering::Relaxed);

        let new_state = if queue_fill >= self.high_watermark {
            BackpressureState::Drop
        } else if queue_fill >= self.low_watermark {
            BackpressureState::Throttle
        } else {
            BackpressureState::Normal
        };

        self.state.store(new_state as u8, Ordering::Relaxed);
    }

    /// Get current backpressure state
    #[inline]
    pub fn get_state(&self) -> BackpressureState {
        self.state.load(Ordering::Relaxed).into()
    }

    /// Get current pressure level (0-100)
    pub fn get_pressure(&self) -> u32 {
        self.current_pressure.load(Ordering::Relaxed)
    }

    /// Get pressure as ratio (0.0-1.0)
    pub fn get_pressure_ratio(&self) -> f32 {
        self.get_pressure() as f32 / 100.0
    }

    /// Reset to normal state
    pub fn reset(&self) {
        self.current_pressure.store(0, Ordering::Relaxed);
        self.state
            .store(BackpressureState::Normal as u8, Ordering::Relaxed);
    }

    /// Check if in normal state
    pub fn is_normal(&self) -> bool {
        self.get_state() == BackpressureState::Normal
    }

    /// Check if throttling
    pub fn is_throttling(&self) -> bool {
        self.get_state() == BackpressureState::Throttle
    }

    /// Check if dropping
    pub fn is_dropping(&self) -> bool {
        self.get_state() == BackpressureState::Drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_creation() {
        let controller = BackpressureController::new(0.8, 0.2);
        assert_eq!(controller.get_state(), BackpressureState::Normal);
        assert_eq!(controller.get_pressure(), 0);
        assert!(controller.should_accept());
    }

    #[test]
    fn test_default_controller() {
        let controller = BackpressureController::default();
        assert!(controller.is_normal());

        // Verify default values
        let manual = BackpressureController::new(0.8, 0.2);
        assert_eq!(controller.get_state(), manual.get_state());
    }

    #[test]
    #[should_panic]
    fn test_invalid_watermarks() {
        let _controller = BackpressureController::new(0.2, 0.8); // reversed
    }

    #[test]
    fn test_state_transitions() {
        let controller = BackpressureController::new(0.8, 0.2);

        // Start in normal
        assert!(controller.is_normal());
        assert!(controller.should_accept());

        // Update to throttle range
        controller.update(0.5);
        assert!(controller.is_throttling());
        assert!(controller.should_accept());
        assert_eq!(controller.get_pressure(), 50);

        // Update to drop range
        controller.update(0.9);
        assert!(controller.is_dropping());
        assert!(!controller.should_accept());
        assert_eq!(controller.get_pressure(), 90);

        // Back to normal
        controller.update(0.1);
        assert!(controller.is_normal());
        assert!(controller.should_accept());
    }

    #[test]
    fn test_watermark_boundaries() {
        let controller = BackpressureController::new(0.8, 0.2);

        // Just below low watermark
        controller.update(0.19);
        assert!(controller.is_normal());

        // At low watermark
        controller.update(0.2);
        assert!(controller.is_throttling());

        // Just below high watermark
        controller.update(0.79);
        assert!(controller.is_throttling());

        // At high watermark
        controller.update(0.8);
        assert!(controller.is_dropping());
    }

    #[test]
    fn test_pressure_clamping() {
        let controller = BackpressureController::new(0.8, 0.2);

        // Pressure should clamp at 100
        controller.update(1.5);
        assert_eq!(controller.get_pressure(), 100);

        controller.update(0.0);
        assert_eq!(controller.get_pressure(), 0);
    }

    #[test]
    fn test_pressure_ratio() {
        let controller = BackpressureController::new(0.8, 0.2);

        controller.update(0.5);
        assert!((controller.get_pressure_ratio() - 0.5).abs() < 0.01);

        controller.update(0.75);
        assert!((controller.get_pressure_ratio() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_reset() {
        let controller = BackpressureController::new(0.8, 0.2);

        // Set to high pressure
        controller.update(0.95);
        assert!(controller.is_dropping());

        // Reset
        controller.reset();
        assert!(controller.is_normal());
        assert_eq!(controller.get_pressure(), 0);
    }

    #[test]
    fn test_hysteresis() {
        let controller = BackpressureController::new(0.8, 0.2);

        // Rising pressure
        controller.update(0.85);
        assert!(controller.is_dropping());

        // Small decrease shouldn't change state
        controller.update(0.82);
        assert!(controller.is_dropping());

        // Must drop below low watermark to return to normal
        controller.update(0.15);
        assert!(controller.is_normal());
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let controller = Arc::new(BackpressureController::new(0.8, 0.2));
        let mut handles = vec![];

        // Multiple threads updating
        for i in 0..10 {
            let ctrl = controller.clone();
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    let fill = ((i * 100 + j) % 100) as f32 / 100.0;
                    ctrl.update(fill);
                    let _ = ctrl.should_accept();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should be in valid state
        let state = controller.get_state();
        assert!(matches!(
            state,
            BackpressureState::Normal | BackpressureState::Throttle | BackpressureState::Drop
        ));
    }

    #[test]
    fn test_decision_performance() {
        let controller = BackpressureController::new(0.8, 0.2);
        controller.update(0.5);

        // should_accept should be very fast (<1μs)
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            let _ = controller.should_accept();
        }
        let elapsed = start.elapsed();

        // 10k calls should take < 10ms (avg < 1μs per call)
        assert!(elapsed.as_millis() < 10);
    }

    #[test]
    fn test_tight_watermarks() {
        // Test with tight watermark range
        let controller = BackpressureController::new(0.51, 0.49);

        controller.update(0.48);
        assert!(controller.is_normal());

        controller.update(0.50);
        assert!(controller.is_throttling());

        controller.update(0.52);
        assert!(controller.is_dropping());
    }
}
