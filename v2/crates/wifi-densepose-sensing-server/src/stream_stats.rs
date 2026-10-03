//! Measured streaming statistics for `/api/v1/stream/status` and the
//! `stream` component of `/health/system` (#2087).
//!
//! Two things used to be guessed: the stream rate (a literal 10 fps) and the
//! client count (`broadcast::Sender::receiver_count`, which also counts the
//! recorder and the MQTT bridge and so is not a count of browsers). This module
//! measures both.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Window over which the sensing-update rate is averaged.
pub const UPDATE_RATE_WINDOW: Duration = Duration::from_secs(5);

/// Hard bound on retained timestamps. At 50 fps per node this covers a
/// 5-node mesh for the whole window; beyond that the rate is taken over a
/// shorter span, which is still a measurement and never an allocation spike.
const UPDATE_RATE_MAX_SAMPLES: usize = 1024;

/// Sliding-window rate of sensing updates (one per `tick` advance).
#[derive(Debug, Default)]
pub struct UpdateRateMeter {
    times: VecDeque<Instant>,
}

impl UpdateRateMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one sensing update at `now`.
    pub fn observe(&mut self, now: Instant) {
        if self.times.len() == UPDATE_RATE_MAX_SAMPLES {
            self.times.pop_front();
        }
        self.times.push_back(now);
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        while let Some(&oldest) = self.times.front() {
            match now.checked_duration_since(oldest) {
                Some(age) if age > UPDATE_RATE_WINDOW => {
                    self.times.pop_front();
                }
                _ => break,
            }
        }
    }

    /// Updates per second over the window ending at `now`. Zero when fewer
    /// than two updates fall inside the window, so a stalled source reads 0
    /// instead of holding its last rate.
    pub fn rate_hz(&self, now: Instant) -> f64 {
        let mut in_window = self.times.iter().filter(|t| {
            now.checked_duration_since(**t)
                .is_some_and(|age| age <= UPDATE_RATE_WINDOW)
        });
        let Some(first) = in_window.next() else {
            return 0.0;
        };
        let (count, last) = in_window.fold((1usize, first), |(n, _), t| (n + 1, t));
        let span = last.duration_since(*first).as_secs_f64();
        if count < 2 || span <= 0.0 {
            return 0.0;
        }
        (count - 1) as f64 / span
    }
}

/// Live count of browser-facing WebSocket clients (`/ws/sensing` and
/// `/api/v1/stream/pose`). Ticketed and unticketed sockets count the same:
/// the count is taken after the upgrade, so it only includes sockets the auth
/// layer admitted.
#[derive(Debug, Clone, Default)]
pub struct WsClientCounter(Arc<AtomicUsize>);

impl WsClientCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    /// Count one client until the returned guard is dropped.
    pub fn enter(&self) -> WsClientGuard {
        self.0.fetch_add(1, Ordering::Relaxed);
        WsClientGuard(self.0.clone())
    }
}

/// Decrements the client count when the socket task ends, on every exit path.
#[derive(Debug)]
pub struct WsClientGuard(Arc<AtomicUsize>);

impl Drop for WsClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Round to one decimal place for JSON output.
pub fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_is_zero_without_samples() {
        let m = UpdateRateMeter::new();
        assert_eq!(m.rate_hz(Instant::now()), 0.0);
    }

    #[test]
    fn rate_measures_a_steady_25_hz_stream() {
        let mut m = UpdateRateMeter::new();
        let t0 = Instant::now();
        for i in 0..=100 {
            m.observe(t0 + Duration::from_millis(40 * i));
        }
        let now = t0 + Duration::from_millis(4000);
        let r = m.rate_hz(now);
        assert!((r - 25.0).abs() < 0.01, "expected 25 Hz, got {r}");
    }

    #[test]
    fn rate_is_not_a_constant() {
        // The old endpoint said 10 fps for every source. A 40 ms tick must
        // read 25 and a 200 ms tick must read 5.
        let t0 = Instant::now();
        let mut fast = UpdateRateMeter::new();
        let mut slow = UpdateRateMeter::new();
        for i in 0..=20 {
            fast.observe(t0 + Duration::from_millis(40 * i));
            slow.observe(t0 + Duration::from_millis(200 * i));
        }
        assert!((fast.rate_hz(t0 + Duration::from_millis(800)) - 25.0).abs() < 0.01);
        assert!((slow.rate_hz(t0 + Duration::from_millis(4000)) - 5.0).abs() < 0.01);
    }

    #[test]
    fn rate_decays_to_zero_when_the_source_stalls() {
        let mut m = UpdateRateMeter::new();
        let t0 = Instant::now();
        for i in 0..50 {
            m.observe(t0 + Duration::from_millis(100 * i));
        }
        let later = t0 + Duration::from_millis(100 * 49) + UPDATE_RATE_WINDOW * 2;
        assert_eq!(m.rate_hz(later), 0.0);
    }

    #[test]
    fn meter_memory_is_bounded() {
        let mut m = UpdateRateMeter::new();
        let t0 = Instant::now();
        for i in 0..(UPDATE_RATE_MAX_SAMPLES as u64 * 3) {
            m.observe(t0 + Duration::from_micros(i));
        }
        assert!(m.times.len() <= UPDATE_RATE_MAX_SAMPLES);
    }

    #[test]
    fn client_counter_tracks_guards() {
        let c = WsClientCounter::new();
        assert_eq!(c.get(), 0);
        let a = c.enter();
        let b = c.clone().enter();
        assert_eq!(c.get(), 2);
        drop(a);
        assert_eq!(c.get(), 1);
        drop(b);
        assert_eq!(c.get(), 0);
    }

    #[test]
    fn round1_rounds_to_one_decimal() {
        assert_eq!(round1(24.96), 25.0);
        assert_eq!(round1(41.04), 41.0);
    }
}
