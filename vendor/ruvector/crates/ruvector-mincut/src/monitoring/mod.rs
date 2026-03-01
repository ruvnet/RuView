//! Real-Time Monitoring for Dynamic Minimum Cut
//!
//! Provides event-driven notifications when minimum cut changes,
//! with support for thresholds, callbacks, and metrics collection.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Type of event that occurred
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Minimum cut value increased
    CutIncreased,
    /// Minimum cut value decreased
    CutDecreased,
    /// Cut crossed below a threshold
    ThresholdCrossedBelow,
    /// Cut crossed above a threshold
    ThresholdCrossedAbove,
    /// Graph became disconnected
    Disconnected,
    /// Graph became connected
    Connected,
    /// Edge was inserted
    EdgeInserted,
    /// Edge was deleted
    EdgeDeleted,
}

impl EventType {
    fn as_str(&self) -> &'static str {
        match self {
            EventType::CutIncreased => "cut_increased",
            EventType::CutDecreased => "cut_decreased",
            EventType::ThresholdCrossedBelow => "threshold_crossed_below",
            EventType::ThresholdCrossedAbove => "threshold_crossed_above",
            EventType::Disconnected => "disconnected",
            EventType::Connected => "connected",
            EventType::EdgeInserted => "edge_inserted",
            EventType::EdgeDeleted => "edge_deleted",
        }
    }
}

/// An event from the monitoring system
#[derive(Debug, Clone)]
pub struct MinCutEvent {
    /// Type of event
    pub event_type: EventType,
    /// New minimum cut value
    pub new_value: f64,
    /// Previous minimum cut value
    pub old_value: f64,
    /// Timestamp of event
    pub timestamp: Instant,
    /// Threshold that was crossed (if applicable)
    pub threshold: Option<f64>,
    /// Edge involved (if applicable)
    pub edge: Option<(u64, u64)>,
}

/// Callback type for event handling
pub type EventCallback = Box<dyn Fn(&MinCutEvent) + Send + Sync>;

/// A threshold configuration
#[derive(Debug, Clone)]
pub struct Threshold {
    /// Threshold value
    pub value: f64,
    /// Name/identifier for this threshold
    pub name: String,
    /// Direction: true = alert when cut goes below, false = above
    pub alert_below: bool,
    /// Whether threshold is active
    pub enabled: bool,
    /// Last state (true if was below threshold)
    last_state: Option<bool>,
}

impl Threshold {
    /// Create a new threshold
    pub fn new(value: f64, name: String, alert_below: bool) -> Self {
        Self {
            value,
            name,
            alert_below,
            enabled: true,
            last_state: None,
        }
    }

    /// Check if threshold was crossed (implements hysteresis)
    fn check_crossing(&mut self, old_value: f64, new_value: f64) -> Option<EventType> {
        let new_state = if self.alert_below {
            new_value < self.value
        } else {
            new_value > self.value
        };

        let old_state = if let Some(last) = self.last_state {
            last
        } else {
            // First time checking - initialize state
            let initial_state = if self.alert_below {
                old_value < self.value
            } else {
                old_value > self.value
            };
            self.last_state = Some(initial_state);
            initial_state
        };

        self.last_state = Some(new_state);

        // Check for state change (hysteresis - only alert on transitions)
        if old_state != new_state && new_state {
            Some(if self.alert_below {
                EventType::ThresholdCrossedBelow
            } else {
                EventType::ThresholdCrossedAbove
            })
        } else {
            None
        }
    }
}

/// Metrics collected by the monitor
#[derive(Debug, Clone)]
pub struct MonitorMetrics {
    /// Total events processed
    pub total_events: u64,
    /// Events by type
    pub events_by_type: HashMap<String, u64>,
    /// Minimum cut value over time (sampled)
    pub cut_history: Vec<(Instant, f64)>,
    /// Average cut value
    pub avg_cut: f64,
    /// Minimum cut observed
    pub min_observed: f64,
    /// Maximum cut observed
    pub max_observed: f64,
    /// Number of threshold violations
    pub threshold_violations: u64,
    /// Time since last event
    pub time_since_last_event: Option<Duration>,
}

impl Default for MonitorMetrics {
    fn default() -> Self {
        Self {
            total_events: 0,
            events_by_type: HashMap::new(),
            cut_history: Vec::new(),
            avg_cut: 0.0,
            min_observed: f64::INFINITY,
            max_observed: f64::NEG_INFINITY,
            threshold_violations: 0,
            time_since_last_event: None,
        }
    }
}

/// Configuration for the monitor
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Maximum number of callbacks
    pub max_callbacks: usize,
    /// History sample interval
    pub sample_interval: Duration,
    /// Maximum history size
    pub max_history_size: usize,
    /// Enable metrics collection
    pub collect_metrics: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            max_callbacks: 100,
            sample_interval: Duration::from_secs(1),
            max_history_size: 1000,
            collect_metrics: true,
        }
    }
}

/// Wrapper for callbacks with optional event type filtering
struct CallbackEntry {
    name: String,
    callback: EventCallback,
    event_filter: Option<EventType>,
}

/// The minimum cut monitor
pub struct MinCutMonitor {
    /// Callbacks registered for events
    callbacks: RwLock<Vec<CallbackEntry>>,
    /// Thresholds
    thresholds: RwLock<Vec<Threshold>>,
    /// Metrics
    metrics: RwLock<MonitorMetrics>,
    /// Current cut value
    current_cut: RwLock<f64>,
    /// Configuration
    config: MonitorConfig,
    /// Start time
    start_time: Instant,
    /// Last event time
    last_event: RwLock<Option<Instant>>,
    /// Last sample time for history
    last_sample: RwLock<Instant>,
}

impl MinCutMonitor {
    /// Create a new monitor
    pub fn new(config: MonitorConfig) -> Self {
        let now = Instant::now();
        Self {
            callbacks: RwLock::new(Vec::new()),
            thresholds: RwLock::new(Vec::new()),
            metrics: RwLock::new(MonitorMetrics::default()),
            current_cut: RwLock::new(0.0),
            config,
            start_time: now,
            last_event: RwLock::new(None),
            last_sample: RwLock::new(now),
        }
    }

    /// Register a callback for all events
    pub fn on_event<F>(&self, name: &str, callback: F) -> crate::Result<()>
    where
        F: Fn(&MinCutEvent) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.write();
        if callbacks.len() >= self.config.max_callbacks {
            return Err(crate::MinCutError::InvalidParameter(format!(
                "Maximum number of callbacks ({}) reached",
                self.config.max_callbacks
            )));
        }

        callbacks.push(CallbackEntry {
            name: name.to_string(),
            callback: Box::new(callback),
            event_filter: None,
        });

        Ok(())
    }

    /// Register a callback for specific event type
    pub fn on_event_type<F>(
        &self,
        event_type: EventType,
        name: &str,
        callback: F,
    ) -> crate::Result<()>
    where
        F: Fn(&MinCutEvent) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.write();
        if callbacks.len() >= self.config.max_callbacks {
            return Err(crate::MinCutError::InvalidParameter(format!(
                "Maximum number of callbacks ({}) reached",
                self.config.max_callbacks
            )));
        }

        callbacks.push(CallbackEntry {
            name: name.to_string(),
            callback: Box::new(callback),
            event_filter: Some(event_type),
        });

        Ok(())
    }

    /// Add a threshold
    pub fn add_threshold(&self, threshold: Threshold) -> crate::Result<()> {
        let mut thresholds = self.thresholds.write();

        // Check if threshold with same name already exists
        if thresholds.iter().any(|t| t.name == threshold.name) {
            return Err(crate::MinCutError::InvalidParameter(format!(
                "Threshold with name '{}' already exists",
                threshold.name
            )));
        }

        thresholds.push(threshold);
        Ok(())
    }

    /// Remove a threshold by name
    pub fn remove_threshold(&self, name: &str) -> bool {
        let mut thresholds = self.thresholds.write();
        if let Some(pos) = thresholds.iter().position(|t| t.name == name) {
            thresholds.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove a callback by name
    pub fn remove_callback(&self, name: &str) -> bool {
        let mut callbacks = self.callbacks.write();
        if let Some(pos) = callbacks.iter().position(|c| c.name == name) {
            callbacks.remove(pos);
            true
        } else {
            false
        }
    }

    /// Notify of a cut change (called by DynamicMinCut)
    pub fn notify(&self, old_value: f64, new_value: f64, edge: Option<(u64, u64)>) {
        let now = Instant::now();

        // Update current cut
        *self.current_cut.write() = new_value;

        // Determine basic event type
        let base_event_type = if new_value > old_value {
            EventType::CutIncreased
        } else if new_value < old_value {
            EventType::CutDecreased
        } else {
            // No change in value
            if edge.is_some() {
                // Edge operation occurred but cut didn't change
                EventType::EdgeInserted
            } else {
                return; // No event to fire
            }
        };

        // Collect all events to fire
        let mut events = Vec::new();

        // Add base cut change event
        if new_value != old_value {
            events.push(MinCutEvent {
                event_type: base_event_type,
                new_value,
                old_value,
                timestamp: now,
                threshold: None,
                edge,
            });
        }

        // Check for edge-specific events
        if edge.is_some() {
            let edge_event = if new_value >= old_value {
                EventType::EdgeInserted
            } else {
                EventType::EdgeDeleted
            };

            events.push(MinCutEvent {
                event_type: edge_event,
                new_value,
                old_value,
                timestamp: now,
                threshold: None,
                edge,
            });
        }

        // Check for connectivity changes
        if old_value > 0.0 && new_value == 0.0 {
            events.push(MinCutEvent {
                event_type: EventType::Disconnected,
                new_value,
                old_value,
                timestamp: now,
                threshold: None,
                edge,
            });
        } else if old_value == 0.0 && new_value > 0.0 {
            events.push(MinCutEvent {
                event_type: EventType::Connected,
                new_value,
                old_value,
                timestamp: now,
                threshold: None,
                edge,
            });
        }

        // Check thresholds
        let threshold_events = self.check_thresholds(old_value, new_value);
        for (threshold, event_type) in threshold_events {
            events.push(MinCutEvent {
                event_type,
                new_value,
                old_value,
                timestamp: now,
                threshold: Some(threshold.value),
                edge,
            });
        }

        // Fire all events
        for event in events {
            self.fire_event(event);
        }

        // Update last event time
        *self.last_event.write() = Some(now);
    }

    /// Get current metrics
    pub fn metrics(&self) -> MonitorMetrics {
        let mut metrics = self.metrics.read().clone();

        // Update time since last event
        if let Some(last) = *self.last_event.read() {
            metrics.time_since_last_event = Some(Instant::now().duration_since(last));
        }

        metrics
    }

    /// Reset metrics
    pub fn reset_metrics(&self) {
        let mut metrics = self.metrics.write();
        *metrics = MonitorMetrics::default();
    }

    /// Get current cut value
    pub fn current_cut(&self) -> f64 {
        *self.current_cut.read()
    }

    /// Get threshold status
    pub fn threshold_status(&self) -> Vec<(String, bool)> {
        let thresholds = self.thresholds.read();
        let current = *self.current_cut.read();

        thresholds
            .iter()
            .map(|t| {
                let active = if t.alert_below {
                    current < t.value
                } else {
                    current > t.value
                };
                (t.name.clone(), active && t.enabled)
            })
            .collect()
    }

    // Internal methods

    fn fire_event(&self, event: MinCutEvent) {
        // Update metrics first
        if self.config.collect_metrics {
            self.update_metrics(&event);
        }

        // Fire callbacks (non-blocking, catch panics)
        let callbacks = self.callbacks.read();
        for entry in callbacks.iter() {
            // Check if event matches filter
            if let Some(filter) = entry.event_filter {
                if filter != event.event_type {
                    continue;
                }
            }

            // Call the callback, catching any panics (fire and forget)
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (entry.callback)(&event);
            }));

            if result.is_err() {
                eprintln!(
                    "Warning: Callback '{}' panicked during execution",
                    entry.name
                );
            }
        }
    }

    fn check_thresholds(&self, old_value: f64, new_value: f64) -> Vec<(Threshold, EventType)> {
        let mut thresholds = self.thresholds.write();
        let mut result = Vec::new();

        for threshold in thresholds.iter_mut() {
            if !threshold.enabled {
                continue;
            }

            if let Some(event_type) = threshold.check_crossing(old_value, new_value) {
                result.push((threshold.clone(), event_type));
            }
        }

        result
    }

    fn update_metrics(&self, event: &MinCutEvent) {
        let mut metrics = self.metrics.write();

        // Update total events
        metrics.total_events += 1;

        // Update events by type
        let type_str = event.event_type.as_str().to_string();
        *metrics.events_by_type.entry(type_str).or_insert(0) += 1;

        // Update min/max observed
        if event.new_value < metrics.min_observed {
            metrics.min_observed = event.new_value;
        }
        if event.new_value > metrics.max_observed {
            metrics.max_observed = event.new_value;
        }

        // Update average (running average)
        if metrics.total_events == 1 {
            metrics.avg_cut = event.new_value;
        } else {
            let n = metrics.total_events as f64;
            metrics.avg_cut = (metrics.avg_cut * (n - 1.0) + event.new_value) / n;
        }

        // Count threshold violations
        if matches!(
            event.event_type,
            EventType::ThresholdCrossedBelow | EventType::ThresholdCrossedAbove
        ) {
            metrics.threshold_violations += 1;
        }

        // Sample history if interval passed
        let mut last_sample = self.last_sample.write();
        if event.timestamp.duration_since(*last_sample) >= self.config.sample_interval {
            metrics.cut_history.push((event.timestamp, event.new_value));

            // Limit history size
            if metrics.cut_history.len() > self.config.max_history_size {
                metrics.cut_history.remove(0);
            }

            *last_sample = event.timestamp;
        }
    }
}

/// Builder for MinCutMonitor
pub struct MonitorBuilder {
    config: MonitorConfig,
    thresholds: Vec<Threshold>,
    callbacks: Vec<(String, EventCallback, Option<EventType>)>,
}

impl MonitorBuilder {
    /// Create a new monitor builder
    pub fn new() -> Self {
        Self {
            config: MonitorConfig::default(),
            thresholds: Vec::new(),
            callbacks: Vec::new(),
        }
    }

    /// Set the monitor configuration
    pub fn with_config(mut self, config: MonitorConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a threshold that alerts when cut goes below the given value
    pub fn threshold_below(mut self, value: f64, name: &str) -> Self {
        self.thresholds
            .push(Threshold::new(value, name.to_string(), true));
        self
    }

    /// Add a threshold that alerts when cut goes above the given value
    pub fn threshold_above(mut self, value: f64, name: &str) -> Self {
        self.thresholds
            .push(Threshold::new(value, name.to_string(), false));
        self
    }

    /// Add a callback for all cut change events
    pub fn on_change<F>(mut self, name: &str, callback: F) -> Self
    where
        F: Fn(&MinCutEvent) + Send + Sync + 'static,
    {
        self.callbacks
            .push((name.to_string(), Box::new(callback), None));
        self
    }

    /// Add a callback for a specific event type
    pub fn on_event_type<F>(mut self, event_type: EventType, name: &str, callback: F) -> Self
    where
        F: Fn(&MinCutEvent) + Send + Sync + 'static,
    {
        self.callbacks
            .push((name.to_string(), Box::new(callback), Some(event_type)));
        self
    }

    /// Build the monitor
    pub fn build(self) -> MinCutMonitor {
        let monitor = MinCutMonitor::new(self.config);

        // Add thresholds
        for threshold in self.thresholds {
            let _ = monitor.add_threshold(threshold);
        }

        // Add callbacks
        for (name, callback, filter) in self.callbacks {
            let mut callbacks = monitor.callbacks.write();
            callbacks.push(CallbackEntry {
                name,
                callback,
                event_filter: filter,
            });
        }

        monitor
    }
}

impl Default for MonitorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_event_type_str() {
        assert_eq!(EventType::CutIncreased.as_str(), "cut_increased");
        assert_eq!(EventType::CutDecreased.as_str(), "cut_decreased");
        assert_eq!(
            EventType::ThresholdCrossedBelow.as_str(),
            "threshold_crossed_below"
        );
    }

    #[test]
    fn test_threshold_crossing_below() {
        let mut threshold = Threshold::new(10.0, "test".to_string(), true);

        // First check: 15 -> 5 (crosses below)
        let event = threshold.check_crossing(15.0, 5.0);
        assert_eq!(event, Some(EventType::ThresholdCrossedBelow));

        // Second check: 5 -> 3 (already below, no event)
        let event = threshold.check_crossing(5.0, 3.0);
        assert_eq!(event, None);

        // Third check: 3 -> 15 (crosses above, but we only alert below)
        let event = threshold.check_crossing(3.0, 15.0);
        assert_eq!(event, None);

        // Fourth check: 15 -> 5 (crosses below again)
        let event = threshold.check_crossing(15.0, 5.0);
        assert_eq!(event, Some(EventType::ThresholdCrossedBelow));
    }

    #[test]
    fn test_threshold_crossing_above() {
        let mut threshold = Threshold::new(10.0, "test".to_string(), false);

        // 5 -> 15 (crosses above)
        let event = threshold.check_crossing(5.0, 15.0);
        assert_eq!(event, Some(EventType::ThresholdCrossedAbove));

        // 15 -> 20 (already above, no event)
        let event = threshold.check_crossing(15.0, 20.0);
        assert_eq!(event, None);

        // 20 -> 5 (crosses below, but we only alert above)
        let event = threshold.check_crossing(20.0, 5.0);
        assert_eq!(event, None);

        // 5 -> 15 (crosses above again)
        let event = threshold.check_crossing(5.0, 15.0);
        assert_eq!(event, Some(EventType::ThresholdCrossedAbove));
    }

    #[test]
    fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = MinCutMonitor::new(config);

        assert_eq!(monitor.current_cut(), 0.0);
        assert_eq!(monitor.metrics().total_events, 0);
    }

    #[test]
    fn test_callback_registration() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        monitor
            .on_event("test", move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        monitor.notify(0.0, 10.0, None);

        // Give callbacks time to execute
        std::thread::sleep(Duration::from_millis(10));

        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_event_type_filtering() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        monitor
            .on_event_type(EventType::CutIncreased, "test", move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        // This should trigger the callback
        monitor.notify(5.0, 10.0, None);
        std::thread::sleep(Duration::from_millis(10));
        let count1 = counter.load(Ordering::SeqCst);
        assert!(count1 > 0);

        // This should not trigger the callback (decrease, not increase)
        monitor.notify(10.0, 5.0, None);
        std::thread::sleep(Duration::from_millis(10));
        let count2 = counter.load(Ordering::SeqCst);
        assert_eq!(count1, count2);
    }

    #[test]
    fn test_threshold_monitoring() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        let threshold = Threshold::new(10.0, "critical".to_string(), true);
        monitor.add_threshold(threshold).unwrap();

        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        monitor
            .on_event_type(
                EventType::ThresholdCrossedBelow,
                "threshold_cb",
                move |_| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                },
            )
            .unwrap();

        // Cross below threshold
        monitor.notify(15.0, 5.0, None);
        std::thread::sleep(Duration::from_millis(10));

        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_metrics_collection() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        monitor.notify(0.0, 10.0, None);
        monitor.notify(10.0, 20.0, None);
        monitor.notify(20.0, 15.0, None);

        std::thread::sleep(Duration::from_millis(10));

        let metrics = monitor.metrics();
        assert!(metrics.total_events >= 3); // At least the cut change events
        assert!(metrics.max_observed >= 20.0);
        assert!(metrics.min_observed <= 10.0);
    }

    #[test]
    fn test_callback_removal() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        monitor
            .on_event("test", move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        monitor.notify(0.0, 10.0, None);
        std::thread::sleep(Duration::from_millis(10));
        let count1 = counter.load(Ordering::SeqCst);
        assert!(count1 > 0);

        // Remove callback
        assert!(monitor.remove_callback("test"));

        monitor.notify(10.0, 20.0, None);
        std::thread::sleep(Duration::from_millis(10));
        let count2 = counter.load(Ordering::SeqCst);

        // Count should not increase
        assert_eq!(count1, count2);
    }

    #[test]
    fn test_threshold_removal() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        let threshold = Threshold::new(10.0, "test".to_string(), true);
        monitor.add_threshold(threshold).unwrap();

        assert!(monitor.remove_threshold("test"));
        assert!(!monitor.remove_threshold("test")); // Already removed
    }

    #[test]
    fn test_builder_pattern() {
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        let monitor = MonitorBuilder::new()
            .threshold_below(10.0, "low")
            .threshold_above(100.0, "high")
            .on_change("test", move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .build();

        monitor.notify(50.0, 5.0, None);
        std::thread::sleep(Duration::from_millis(10));

        assert!(counter.load(Ordering::SeqCst) > 0);

        let status = monitor.threshold_status();
        assert_eq!(status.len(), 2);
    }

    #[test]
    fn test_connectivity_events() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());
        let disconnected = Arc::new(AtomicU64::new(0));
        let connected = Arc::new(AtomicU64::new(0));

        let disc_clone = disconnected.clone();
        let conn_clone = connected.clone();

        monitor
            .on_event_type(EventType::Disconnected, "disc", move |_| {
                disc_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        monitor
            .on_event_type(EventType::Connected, "conn", move |_| {
                conn_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        // Become disconnected
        monitor.notify(10.0, 0.0, None);
        std::thread::sleep(Duration::from_millis(10));
        assert!(disconnected.load(Ordering::SeqCst) > 0);

        // Become connected
        monitor.notify(0.0, 5.0, None);
        std::thread::sleep(Duration::from_millis(10));
        assert!(connected.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_max_callbacks_limit() {
        let config = MonitorConfig {
            max_callbacks: 2,
            ..Default::default()
        };
        let monitor = MinCutMonitor::new(config);

        assert!(monitor.on_event("cb1", |_| {}).is_ok());
        assert!(monitor.on_event("cb2", |_| {}).is_ok());
        assert!(monitor.on_event("cb3", |_| {}).is_err());
    }

    #[test]
    fn test_duplicate_threshold_name() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        let t1 = Threshold::new(10.0, "test".to_string(), true);
        let t2 = Threshold::new(20.0, "test".to_string(), false);

        assert!(monitor.add_threshold(t1).is_ok());
        assert!(monitor.add_threshold(t2).is_err());
    }

    #[test]
    fn test_metrics_reset() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        monitor.notify(0.0, 10.0, None);
        monitor.notify(10.0, 20.0, None);

        let metrics1 = monitor.metrics();
        assert!(metrics1.total_events > 0);

        monitor.reset_metrics();

        let metrics2 = monitor.metrics();
        assert_eq!(metrics2.total_events, 0);
    }

    #[test]
    fn test_edge_events() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        monitor
            .on_event_type(EventType::EdgeInserted, "edge", move |event| {
                assert!(event.edge.is_some());
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        monitor.notify(10.0, 15.0, Some((1, 2)));
        std::thread::sleep(Duration::from_millis(10));

        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_threshold_hysteresis() {
        let mut threshold = Threshold::new(10.0, "test".to_string(), true);

        // Cross below
        assert_eq!(
            threshold.check_crossing(15.0, 5.0),
            Some(EventType::ThresholdCrossedBelow)
        );

        // Stay below (no event)
        assert_eq!(threshold.check_crossing(5.0, 3.0), None);
        assert_eq!(threshold.check_crossing(3.0, 8.0), None);

        // Cross above (no event for alert_below threshold)
        assert_eq!(threshold.check_crossing(8.0, 15.0), None);

        // Cross below again (should trigger)
        assert_eq!(
            threshold.check_crossing(15.0, 5.0),
            Some(EventType::ThresholdCrossedBelow)
        );
    }

    #[test]
    fn test_concurrent_callbacks() {
        let monitor = Arc::new(MinCutMonitor::new(MonitorConfig::default()));
        let counter = Arc::new(AtomicU64::new(0));

        // Register multiple callbacks
        for i in 0..10 {
            let counter_clone = counter.clone();
            monitor
                .on_event(&format!("cb{}", i), move |_| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
        }

        // Trigger events from multiple threads
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let monitor_clone = monitor.clone();
                std::thread::spawn(move || {
                    monitor_clone.notify(i as f64, (i + 1) as f64, None);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        std::thread::sleep(Duration::from_millis(50));

        // Each notify triggers multiple events, and each event fires 10 callbacks
        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_average_calculation() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        monitor.notify(0.0, 10.0, None);
        monitor.notify(10.0, 20.0, None);
        monitor.notify(20.0, 30.0, None);

        let metrics = monitor.metrics();
        // Average should be calculated from all events
        assert!(metrics.avg_cut > 0.0);
    }

    #[test]
    fn test_history_sampling() {
        let config = MonitorConfig {
            sample_interval: Duration::from_millis(1),
            max_history_size: 5,
            ..Default::default()
        };
        let monitor = MinCutMonitor::new(config);

        // Generate events with delays to trigger sampling
        for i in 0..10 {
            monitor.notify(i as f64, (i + 1) as f64, None);
            std::thread::sleep(Duration::from_millis(2));
        }

        let metrics = monitor.metrics();
        // History should be limited to max_history_size
        assert!(metrics.cut_history.len() <= 5);
    }

    #[test]
    fn test_no_change_no_event() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        monitor
            .on_event("test", move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        // Same value, no edge - should not trigger event
        monitor.notify(10.0, 10.0, None);
        std::thread::sleep(Duration::from_millis(10));

        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_threshold_status() {
        let monitor = MinCutMonitor::new(MonitorConfig::default());

        monitor
            .add_threshold(Threshold::new(10.0, "low".to_string(), true))
            .unwrap();
        monitor
            .add_threshold(Threshold::new(100.0, "high".to_string(), false))
            .unwrap();

        // Set current cut to 50
        monitor.notify(0.0, 50.0, None);

        let status = monitor.threshold_status();
        assert_eq!(status.len(), 2);

        // At 50: not below 10 (inactive), not above 100 (inactive)
        for (name, active) in &status {
            assert!(!active, "Threshold {} should not be active at 50", name);
        }
    }
}
