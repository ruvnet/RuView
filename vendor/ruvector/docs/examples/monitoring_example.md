# Real-Time Monitoring Example

This example demonstrates the event-driven monitoring system for the dynamic minimum cut algorithm.

## Basic Usage

```rust
use ruvector_mincut::monitoring::{MinCutMonitor, MonitorConfig, EventType};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Create a monitor with default configuration
let monitor = MinCutMonitor::new(MonitorConfig::default());

// Register a callback for all events
let counter = Arc::new(AtomicU64::new(0));
let counter_clone = counter.clone();

monitor.on_event("event_counter", move |event| {
    counter_clone.fetch_add(1, Ordering::SeqCst);
    println!("Event: {:?}, New cut: {}", event.event_type, event.new_value);
}).unwrap();

// Simulate cut changes
monitor.notify(0.0, 10.0, None);
monitor.notify(10.0, 5.0, None);

// Check metrics
let metrics = monitor.metrics();
println!("Total events: {}", metrics.total_events);
println!("Average cut: {}", metrics.avg_cut);
```

## Event Type Filtering

```rust
use std::sync::atomic::AtomicU64;

let monitor = MinCutMonitor::new(MonitorConfig::default());
let decrease_counter = Arc::new(AtomicU64::new(0));
let counter_clone = decrease_counter.clone();

// Only track when cut decreases
monitor.on_event_type(EventType::CutDecreased, "decrease_tracker", move |event| {
    counter_clone.fetch_add(1, Ordering::SeqCst);
    println!("Cut decreased from {} to {}", event.old_value, event.new_value);
}).unwrap();

monitor.notify(10.0, 5.0, None);  // Triggers callback
monitor.notify(5.0, 15.0, None);  // Does not trigger
```

## Threshold Monitoring

```rust
use ruvector_mincut::monitoring::{Threshold, MonitorBuilder};

let alert_counter = Arc::new(AtomicU64::new(0));
let counter_clone = alert_counter.clone();

// Build monitor with thresholds
let monitor = MonitorBuilder::new()
    .threshold_below(10.0, "critical")    // Alert when cut goes below 10
    .threshold_above(100.0, "warning")    // Alert when cut goes above 100
    .on_event_type(EventType::ThresholdCrossedBelow, "alert", move |event| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
        println!("CRITICAL: Cut crossed below threshold!");
    })
    .build();

// Cross the threshold
monitor.notify(50.0, 5.0, None);  // Triggers alert

// Check threshold status
let status = monitor.threshold_status();
for (name, active) in status {
    println!("Threshold '{}': {}", name, if active { "ACTIVE" } else { "inactive" });
}
```

## Connectivity Monitoring

```rust
let disconnected_counter = Arc::new(AtomicU64::new(0));
let connected_counter = Arc::new(AtomicU64::new(0));

let disc_clone = disconnected_counter.clone();
let conn_clone = connected_counter.clone();

let monitor = MinCutMonitor::new(MonitorConfig::default());

monitor.on_event_type(EventType::Disconnected, "disc", move |_| {
    disc_clone.fetch_add(1, Ordering::SeqCst);
    println!("WARNING: Graph became disconnected!");
}).unwrap();

monitor.on_event_type(EventType::Connected, "conn", move |_| {
    conn_clone.fetch_add(1, Ordering::SeqCst);
    println!("Graph reconnected");
}).unwrap();

// Simulate disconnection
monitor.notify(10.0, 0.0, None);  // Graph disconnected

// Simulate reconnection
monitor.notify(0.0, 5.0, None);   // Graph connected
```

## Custom Configuration

```rust
use std::time::Duration;

let config = MonitorConfig {
    max_callbacks: 50,                        // Allow up to 50 callbacks
    sample_interval: Duration::from_millis(100),  // Sample history every 100ms
    max_history_size: 500,                    // Keep last 500 samples
    collect_metrics: true,                    // Enable metrics collection
};

let monitor = MinCutMonitor::new(config);
```

## Metrics Collection

```rust
let monitor = MinCutMonitor::new(MonitorConfig::default());

// Simulate various events
for i in 0..100 {
    let value = (i as f64 * 10.0) % 100.0;
    monitor.notify(value, value + 5.0, Some((i, i + 1)));
}

// Get metrics
let metrics = monitor.metrics();
println!("Total events: {}", metrics.total_events);
println!("Average cut: {:.2}", metrics.avg_cut);
println!("Min observed: {:.2}", metrics.min_observed);
println!("Max observed: {:.2}", metrics.max_observed);
println!("Events by type:");
for (event_type, count) in &metrics.events_by_type {
    println!("  {}: {}", event_type, count);
}
println!("History samples: {}", metrics.cut_history.len());
```

## Thread-Safe Concurrent Monitoring

```rust
use std::thread;

let monitor = Arc::new(MinCutMonitor::new(MonitorConfig::default()));
let counter = Arc::new(AtomicU64::new(0));

// Register callbacks from multiple threads
for i in 0..10 {
    let monitor_clone = monitor.clone();
    let counter_clone = counter.clone();

    monitor_clone.on_event(&format!("callback_{}", i), move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    }).unwrap();
}

// Trigger events from multiple threads
let handles: Vec<_> = (0..5).map(|i| {
    let monitor_clone = monitor.clone();
    thread::spawn(move || {
        monitor_clone.notify(i as f64, (i + 1) as f64, None);
    })
}).collect();

for handle in handles {
    handle.join().unwrap();
}

println!("Total callback invocations: {}", counter.load(Ordering::SeqCst));
```

## Key Features

### Event-Driven Architecture
- **Non-blocking callbacks**: Callbacks are executed synchronously but errors are caught
- **Event filtering**: Register callbacks for specific event types
- **Panic safety**: Callbacks that panic are caught and logged

### Threshold Monitoring
- **Hysteresis**: Prevents alert storms by only triggering on state transitions
- **Bi-directional**: Support for "below" and "above" threshold alerts
- **Dynamic management**: Add/remove thresholds at runtime

### Metrics Collection
- **Running statistics**: Average, min, max cut values
- **Event counting**: Track events by type
- **Sampled history**: Time-series data with configurable sampling
- **Resource bounded**: Automatic history size management

### Thread Safety
- **Lock-free reads**: RwLock allows concurrent reads
- **Safe updates**: Write locks protect critical sections
- **Arc-friendly**: Safe to share across threads
