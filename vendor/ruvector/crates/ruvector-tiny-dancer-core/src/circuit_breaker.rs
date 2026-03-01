//! Circuit breaker pattern for graceful degradation

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// State of the circuit breaker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service has recovered
    HalfOpen,
}

/// Circuit breaker for graceful degradation
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    threshold: u32,
    timeout: Duration,
    half_open_requests: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    /// * `threshold` - Number of failures before opening the circuit
    pub fn new(threshold: u32) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: Arc::new(RwLock::new(None)),
            threshold,
            timeout: Duration::from_secs(60), // Default 60 second timeout
            half_open_requests: AtomicU64::new(0),
        }
    }

    /// Create a circuit breaker with custom timeout
    pub fn with_timeout(threshold: u32, timeout: Duration) -> Self {
        let mut cb = Self::new(threshold);
        cb.timeout = timeout;
        cb
    }

    /// Check if the circuit is closed (allowing requests)
    pub fn is_closed(&self) -> bool {
        let state = *self.state.read();

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.read() {
                    if last_failure.elapsed() >= self.timeout {
                        // Move to half-open state
                        *self.state.write() = CircuitState::HalfOpen;
                        self.half_open_requests.store(0, Ordering::SeqCst);
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                // Allow limited requests in half-open state
                self.half_open_requests.fetch_add(1, Ordering::SeqCst) < 3
            }
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        self.success_count.fetch_add(1, Ordering::SeqCst);

        let state = *self.state.read();
        if state == CircuitState::HalfOpen {
            // After 3 successful requests in half-open, close the circuit
            if self.success_count.load(Ordering::SeqCst) >= 3 {
                *self.state.write() = CircuitState::Closed;
                self.failure_count.store(0, Ordering::SeqCst);
                self.success_count.store(0, Ordering::SeqCst);
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure_time.write() = Some(Instant::now());

        let state = *self.state.read();

        match state {
            CircuitState::Closed => {
                if failures >= self.threshold {
                    *self.state.write() = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open immediately opens the circuit
                *self.state.write() = CircuitState::Open;
            }
            CircuitState::Open => {}
        }
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }

    /// Get failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Get success count
    pub fn success_count(&self) -> u32 {
        self.success_count.load(Ordering::SeqCst)
    }

    /// Reset the circuit breaker
    pub fn reset(&self) {
        *self.state.write() = CircuitState::Closed;
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        *self.last_failure_time.write() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_closed() {
        let cb = CircuitBreaker::new(3);
        assert!(cb.is_closed());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new(3);

        cb.record_failure();
        assert!(cb.is_closed());

        cb.record_failure();
        assert!(cb.is_closed());

        cb.record_failure();
        assert!(!cb.is_closed());
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_half_open_after_timeout() {
        let cb = CircuitBreaker::with_timeout(2, Duration::from_millis(100));

        cb.record_failure();
        cb.record_failure();
        assert!(!cb.is_closed());

        std::thread::sleep(Duration::from_millis(150));
        assert!(cb.is_closed());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_closes_after_successes() {
        let cb = CircuitBreaker::with_timeout(2, Duration::from_millis(100));

        cb.record_failure();
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(150));

        // Move to half-open
        assert!(cb.is_closed());

        // Record successes
        cb.record_success();
        cb.record_success();
        cb.record_success();

        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
