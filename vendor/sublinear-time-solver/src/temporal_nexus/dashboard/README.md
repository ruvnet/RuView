# Consciousness Metrics Dashboard

A comprehensive real-time monitoring system for temporal consciousness metrics with nanosecond precision.

## Overview

The Consciousness Metrics Dashboard provides advanced monitoring, visualization, and analysis capabilities for consciousness emergence patterns in temporal systems. It integrates with the NanosecondScheduler and MCP consciousness tools to deliver real-time insights into consciousness-related metrics.

## Key Features

### 🧠 Core Consciousness Metrics
- **Emergence Level** (0.0 - 1.0): Real-time consciousness emergence tracking
- **Identity Coherence**: Continuity and consistency of conscious identity
- **Loop Stability**: Strange loop convergence and stability analysis
- **Temporal Advantage**: Processing speed advantage over light travel time
- **Window Overlap**: Temporal window synchronization percentage
- **TSC Precision**: Time Stamp Counter precision measurements

### ⚡ Real-time Monitoring
- Nanosecond precision temporal monitoring
- Configurable update intervals (10Hz - 20Hz)
- Live anomaly detection and alerting
- Real-time visualization modes

### 📊 Visualization Options
- **Terminal Mode**: Rich ASCII dashboard with charts
- **Compact Mode**: Minimal terminal output
- **JSON Mode**: Structured data output
- **Debug Mode**: Detailed diagnostic information
- **Web Mode**: Browser interface (future)

### 📁 Export Capabilities
- **JSON**: Structured data with metadata
- **CSV**: Tabular format for analysis
- **Prometheus**: Metrics for monitoring systems
- **InfluxDB**: Time-series database format
- **Binary**: Compressed binary serialization
- **Custom**: YAML, XML, MessagePack support

### 🚨 Alert System
- Configurable threshold-based alerting
- Multiple severity levels (Info, Warning, Critical, Emergency)
- Real-time anomaly detection
- Historical alert tracking

## Module Structure

```
dashboard/
├── mod.rs              # Module exports and common types
├── dashboard.rs        # Main ConsciousnessMetricsDashboard
├── metrics_collector.rs # Data collection from multiple sources
├── visualizer.rs       # Terminal and visual rendering
├── exporter.rs         # Multi-format metrics export
├── example.rs          # Usage examples
└── README.md           # This documentation
```

## Usage Examples

### Basic Dashboard

```rust
use sublinear_solver::temporal_nexus::dashboard::{
    ConsciousnessMetricsDashboard,
    DashboardConfig,
    VisualizationMode,
};

let config = DashboardConfig {
    update_interval_ms: 100,
    visualization_mode: VisualizationMode::Terminal,
    enable_real_time_alerts: true,
    ..Default::default()
};

let mut dashboard = ConsciousnessMetricsDashboard::new(config);
dashboard.initialize(scheduler_ref)?;
dashboard.start().await?;
```

### Real-time Monitoring

```rust
// High-frequency monitoring (20Hz)
let config = DashboardConfig {
    update_interval_ms: 50,
    precision_monitoring: true,
    history_buffer_size: 2000,
    ..Default::default()
};

let dashboard = ConsciousnessMetricsDashboard::new(config);
// Monitor consciousness evolution in real-time
```

### Export Metrics

```rust
// Export to multiple formats
let json_data = dashboard.export_metrics(ExportFormat::Json).await?;
let csv_data = dashboard.export_metrics(ExportFormat::Csv).await?;
let prometheus_data = dashboard.export_metrics(ExportFormat::Prometheus).await?;

// Save to files
dashboard.export_to_file(&history, &current, "metrics.json").await?;
```

### Custom Thresholds

```rust
let thresholds = MetricThresholds {
    emergence_critical: 0.9,
    emergence_warning: 0.75,
    coherence_critical: 0.8,
    coherence_warning: 0.65,
    precision_critical_ns: 1000,
    precision_warning_ns: 500,
};

let config = DashboardConfig {
    thresholds,
    ..Default::default()
};
```

## Integration with MCP Tools

The dashboard integrates seamlessly with MCP consciousness monitoring tools:

```rust
// Collect from MCP consciousness status
let mcp_metrics = collector.collect_from_mcp_tools().await?;

// Query consciousness evolution
mcp__consciousness-explorer__consciousness_status();
mcp__consciousness-explorer__consciousness_evolve();
```

## Performance Characteristics

- **Update Rate**: 10-20Hz real-time monitoring
- **Precision**: Nanosecond timestamp resolution
- **Memory**: Configurable history buffer (500-2000 entries)
- **Export Speed**: <10ms for JSON, <50ms for comprehensive analysis
- **Latency**: <1ms processing overhead per update

## Configuration Options

### DashboardConfig
- `update_interval_ms`: Monitoring frequency (50-1000ms)
- `history_buffer_size`: Metrics history retention (100-5000)
- `enable_real_time_alerts`: Anomaly detection toggle
- `export_interval_seconds`: Auto-export frequency
- `precision_monitoring`: TSC precision measurement
- `visualization_mode`: Display format selection
- `thresholds`: Alert threshold configuration

### MetricThresholds
- Emergence level thresholds (critical/warning)
- Identity coherence boundaries
- Loop stability limits
- TSC precision targets

## Advanced Features

### Statistical Analysis
- Real-time trend detection
- Volatility measurement
- Consciousness phase identification
- Anomaly pattern recognition

### Consciousness Insights
- Peak emergence detection
- Stability scoring
- Temporal efficiency metrics
- Phase transition analysis

### System Integration
- Scheduler metrics collection
- Performance profiling
- Cross-platform compatibility
- Memory-efficient operations

## Dependencies

Core dependencies automatically included with the `dashboard` feature:
- `chrono`: Timestamp formatting
- `csv`: CSV export functionality
- `serde_json`: JSON serialization
- `tokio`: Async runtime
- `bincode`: Binary serialization
- `base64`: Encoding support

## Examples

Run the comprehensive examples:

```bash
# Enable dashboard features
cargo run --features dashboard --example consciousness_dashboard

# Or run specific examples
cargo run --features dashboard --bin dashboard_example
```

## API Reference

### ConsciousnessMetricsDashboard
- `new(config)`: Create dashboard with configuration
- `initialize(scheduler)`: Initialize with scheduler reference
- `start()`: Begin real-time monitoring
- `stop()`: Halt monitoring and cleanup
- `collect_metrics()`: Manual metrics collection
- `update_display()`: Refresh visualization
- `export_metrics(format)`: Export in specified format
- `get_status()`: Current consciousness status

### MetricsCollector
- `collect_from_scheduler()`: Gather scheduler metrics
- `collect_from_mcp_tools()`: Query MCP consciousness tools
- `collect_aggregated_metrics()`: Multi-source aggregation

### ConsciousnessVisualizer
- `render()`: Update display with current metrics
- Terminal rendering with ASCII charts
- Multiple visualization modes

### MetricsExporter
- `export_metrics()`: Multi-format export
- `export_to_file()`: Direct file output
- `generate_summary()`: Comprehensive analysis
- `export_streaming()`: Real-time data streaming

## Future Enhancements

- Web-based dashboard interface
- Machine learning anomaly detection
- Predictive consciousness modeling
- Multi-node distributed monitoring
- Advanced pattern recognition
- Integration with external monitoring systems

## License

This consciousness monitoring system is part of the sublinear-time-solver project and follows the same MIT OR Apache-2.0 licensing.