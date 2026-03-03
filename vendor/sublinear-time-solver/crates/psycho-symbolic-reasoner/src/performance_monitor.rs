use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub operation_name: String,
    pub duration: Duration,
    pub memory_used: Option<usize>,
    pub throughput: Option<f64>,
    pub timestamp: std::time::SystemTime,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub operation_name: String,
    pub total_calls: u64,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub p95_duration: Duration,
    pub p99_duration: Duration,
    pub total_memory: Option<usize>,
    pub avg_throughput: Option<f64>,
    pub first_seen: std::time::SystemTime,
    pub last_seen: std::time::SystemTime,
}

pub struct PerformanceMonitor {
    metrics: Vec<PerformanceMetrics>,
    aggregated: HashMap<String, AggregatedMetrics>,
    thresholds: HashMap<String, PerformanceThreshold>,
}

#[derive(Debug, Clone)]
pub struct PerformanceThreshold {
    pub max_duration: Option<Duration>,
    pub max_memory: Option<usize>,
    pub min_throughput: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub operation_name: String,
    pub alert_type: AlertType,
    pub current_value: String,
    pub threshold_value: String,
    pub severity: AlertSeverity,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub enum AlertType {
    DurationExceeded,
    MemoryExceeded,
    ThroughputBelow,
    PerformanceRegression,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
            aggregated: HashMap::new(),
            thresholds: HashMap::new(),
        }
    }

    pub fn set_threshold(&mut self, operation: &str, threshold: PerformanceThreshold) {
        self.thresholds.insert(operation.to_string(), threshold);
    }

    pub fn record_operation<F, R>(&mut self, operation_name: &str, operation: F) -> (R, Option<PerformanceAlert>)
    where
        F: FnOnce() -> R,
    {
        let start_memory = memory_stats::memory_stats().map(|stats| stats.physical_mem);
        let start_time = Instant::now();
        let timestamp = std::time::SystemTime::now();

        let result = operation();

        let duration = start_time.elapsed();
        let end_memory = memory_stats::memory_stats().map(|stats| stats.physical_mem);
        let memory_used = match (start_memory, end_memory) {
            (Some(start), Some(end)) => Some(end.saturating_sub(start)),
            _ => None,
        };

        let metric = PerformanceMetrics {
            operation_name: operation_name.to_string(),
            duration,
            memory_used,
            throughput: None,
            timestamp,
            metadata: HashMap::new(),
        };

        let alert = self.check_thresholds(&metric);
        self.record_metric(metric);

        (result, alert)
    }

    pub fn record_metric(&mut self, metric: PerformanceMetrics) {
        self.update_aggregated(&metric);
        self.metrics.push(metric);

        // Keep only recent metrics to prevent memory bloat
        if self.metrics.len() > 10000 {
            self.metrics.drain(..1000);
        }
    }

    fn update_aggregated(&mut self, metric: &PerformanceMetrics) {
        let aggregated = self.aggregated
            .entry(metric.operation_name.clone())
            .or_insert_with(|| AggregatedMetrics {
                operation_name: metric.operation_name.clone(),
                total_calls: 0,
                avg_duration: Duration::new(0, 0),
                min_duration: metric.duration,
                max_duration: metric.duration,
                p95_duration: metric.duration,
                p99_duration: metric.duration,
                total_memory: None,
                avg_throughput: None,
                first_seen: metric.timestamp,
                last_seen: metric.timestamp,
            });

        aggregated.total_calls += 1;
        aggregated.last_seen = metric.timestamp;

        // Update duration statistics
        if metric.duration < aggregated.min_duration {
            aggregated.min_duration = metric.duration;
        }
        if metric.duration > aggregated.max_duration {
            aggregated.max_duration = metric.duration;
        }

        // Recalculate average duration
        let total_duration_nanos = aggregated.avg_duration.as_nanos() as u64 * (aggregated.total_calls - 1)
            + metric.duration.as_nanos() as u64;
        aggregated.avg_duration = Duration::from_nanos(total_duration_nanos / aggregated.total_calls);

        // Update memory statistics
        if let Some(memory_used) = metric.memory_used {
            aggregated.total_memory = Some(
                aggregated.total_memory.unwrap_or(0) + memory_used
            );
        }

        // Update percentiles (simplified calculation)
        self.update_percentiles(&metric.operation_name);
    }

    fn update_percentiles(&mut self, operation_name: &str) {
        let durations: Vec<Duration> = self.metrics
            .iter()
            .filter(|m| m.operation_name == operation_name)
            .map(|m| m.duration)
            .collect();

        if durations.len() < 2 {
            return;
        }

        let mut sorted_durations = durations.clone();
        sorted_durations.sort();

        let p95_index = (sorted_durations.len() as f64 * 0.95) as usize;
        let p99_index = (sorted_durations.len() as f64 * 0.99) as usize;

        if let Some(aggregated) = self.aggregated.get_mut(operation_name) {
            aggregated.p95_duration = sorted_durations[p95_index.min(sorted_durations.len() - 1)];
            aggregated.p99_duration = sorted_durations[p99_index.min(sorted_durations.len() - 1)];
        }
    }

    fn check_thresholds(&self, metric: &PerformanceMetrics) -> Option<PerformanceAlert> {
        let threshold = self.thresholds.get(&metric.operation_name)?;

        // Check duration threshold
        if let Some(max_duration) = threshold.max_duration {
            if metric.duration > max_duration {
                return Some(PerformanceAlert {
                    operation_name: metric.operation_name.clone(),
                    alert_type: AlertType::DurationExceeded,
                    current_value: format!("{:?}", metric.duration),
                    threshold_value: format!("{:?}", max_duration),
                    severity: if metric.duration > max_duration * 2 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    timestamp: metric.timestamp,
                });
            }
        }

        // Check memory threshold
        if let (Some(memory_used), Some(max_memory)) = (metric.memory_used, threshold.max_memory) {
            if memory_used > max_memory {
                return Some(PerformanceAlert {
                    operation_name: metric.operation_name.clone(),
                    alert_type: AlertType::MemoryExceeded,
                    current_value: format!("{} bytes", memory_used),
                    threshold_value: format!("{} bytes", max_memory),
                    severity: if memory_used > max_memory * 2 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    timestamp: metric.timestamp,
                });
            }
        }

        // Check throughput threshold
        if let (Some(throughput), Some(min_throughput)) = (metric.throughput, threshold.min_throughput) {
            if throughput < min_throughput {
                return Some(PerformanceAlert {
                    operation_name: metric.operation_name.clone(),
                    alert_type: AlertType::ThroughputBelow,
                    current_value: format!("{:.2} ops/sec", throughput),
                    threshold_value: format!("{:.2} ops/sec", min_throughput),
                    severity: if throughput < min_throughput * 0.5 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    timestamp: metric.timestamp,
                });
            }
        }

        None
    }

    pub fn get_aggregated_metrics(&self) -> &HashMap<String, AggregatedMetrics> {
        &self.aggregated
    }

    pub fn get_recent_metrics(&self, operation_name: &str, count: usize) -> Vec<&PerformanceMetrics> {
        self.metrics
            .iter()
            .filter(|m| m.operation_name == operation_name)
            .rev()
            .take(count)
            .collect()
    }

    pub fn detect_regressions(&self, operation_name: &str, window_size: usize) -> Option<PerformanceAlert> {
        let recent_metrics = self.get_recent_metrics(operation_name, window_size * 2);

        if recent_metrics.len() < window_size * 2 {
            return None;
        }

        let current_window = &recent_metrics[..window_size];
        let baseline_window = &recent_metrics[window_size..];

        let current_avg: Duration = current_window.iter()
            .map(|m| m.duration)
            .sum::<Duration>()
            / current_window.len() as u32;

        let baseline_avg: Duration = baseline_window.iter()
            .map(|m| m.duration)
            .sum::<Duration>()
            / baseline_window.len() as u32;

        let regression_ratio = current_avg.as_nanos() as f64 / baseline_avg.as_nanos() as f64;

        if regression_ratio > 1.2 { // 20% regression
            Some(PerformanceAlert {
                operation_name: operation_name.to_string(),
                alert_type: AlertType::PerformanceRegression,
                current_value: format!("{:?}", current_avg),
                threshold_value: format!("{:?}", baseline_avg),
                severity: if regression_ratio > 2.0 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                },
                timestamp: std::time::SystemTime::now(),
            })
        } else {
            None
        }
    }

    pub fn export_metrics(&self, format: &str) -> Result<String, Box<dyn std::error::Error>> {
        match format {
            "json" => Ok(serde_json::to_string_pretty(&self.metrics)?),
            "csv" => {
                let mut csv = String::from("operation,duration_ms,memory_bytes,throughput,timestamp\n");
                for metric in &self.metrics {
                    csv.push_str(&format!(
                        "{},{},{},{},{:?}\n",
                        metric.operation_name,
                        metric.duration.as_millis(),
                        metric.memory_used.unwrap_or(0),
                        metric.throughput.unwrap_or(0.0),
                        metric.timestamp
                    ));
                }
                Ok(csv)
            }
            _ => Err("Unsupported format".into()),
        }
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::from("# Performance Monitor Report\n\n");

        report.push_str(&format!("**Generated:** {:?}\n", std::time::SystemTime::now()));
        report.push_str(&format!("**Total Operations Tracked:** {}\n\n", self.metrics.len()));

        report.push_str("## Aggregated Metrics\n\n");

        for (operation, metrics) in &self.aggregated {
            report.push_str(&format!("### {}\n\n", operation));
            report.push_str(&format!("- **Total Calls:** {}\n", metrics.total_calls));
            report.push_str(&format!("- **Average Duration:** {:?}\n", metrics.avg_duration));
            report.push_str(&format!("- **Min Duration:** {:?}\n", metrics.min_duration));
            report.push_str(&format!("- **Max Duration:** {:?}\n", metrics.max_duration));
            report.push_str(&format!("- **P95 Duration:** {:?}\n", metrics.p95_duration));
            report.push_str(&format!("- **P99 Duration:** {:?}\n", metrics.p99_duration));

            if let Some(total_memory) = metrics.total_memory {
                report.push_str(&format!("- **Total Memory Used:** {} bytes\n", total_memory));
                report.push_str(&format!("- **Average Memory per Call:** {} bytes\n",
                    total_memory / metrics.total_calls as usize));
            }

            report.push_str("\n");
        }

        report.push_str("## Performance Thresholds\n\n");

        for (operation, threshold) in &self.thresholds {
            report.push_str(&format!("### {}\n\n", operation));

            if let Some(max_duration) = threshold.max_duration {
                report.push_str(&format!("- **Max Duration:** {:?}\n", max_duration));
            }

            if let Some(max_memory) = threshold.max_memory {
                report.push_str(&format!("- **Max Memory:** {} bytes\n", max_memory));
            }

            if let Some(min_throughput) = threshold.min_throughput {
                report.push_str(&format!("- **Min Throughput:** {:.2} ops/sec\n", min_throughput));
            }

            report.push_str("\n");
        }

        report
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// Global performance monitor instance
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;

static GLOBAL_MONITOR: OnceLock<Arc<Mutex<PerformanceMonitor>>> = OnceLock::new();

pub fn get_global_monitor() -> Arc<Mutex<PerformanceMonitor>> {
    GLOBAL_MONITOR.get_or_init(|| {
        Arc::new(Mutex::new(PerformanceMonitor::new()))
    }).clone()
}

// Convenience macros for performance monitoring
#[macro_export]
macro_rules! monitor_performance {
    ($operation:expr, $block:expr) => {{
        let monitor = $crate::performance_monitor::get_global_monitor();
        let mut monitor = monitor.lock().unwrap();
        monitor.record_operation($operation, || $block)
    }};
}

#[macro_export]
macro_rules! set_performance_threshold {
    ($operation:expr, $threshold:expr) => {{
        let monitor = $crate::performance_monitor::get_global_monitor();
        let mut monitor = monitor.lock().unwrap();
        monitor.set_threshold($operation, $threshold);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_performance_monitoring() {
        let mut monitor = PerformanceMonitor::new();

        let (result, alert) = monitor.record_operation("test_operation", || {
            thread::sleep(Duration::from_millis(10));
            42
        });

        assert_eq!(result, 42);
        assert!(alert.is_none());

        let aggregated = monitor.get_aggregated_metrics();
        assert!(aggregated.contains_key("test_operation"));

        let test_metrics = &aggregated["test_operation"];
        assert_eq!(test_metrics.total_calls, 1);
        assert!(test_metrics.avg_duration >= Duration::from_millis(10));
    }

    #[test]
    fn test_threshold_alerts() {
        let mut monitor = PerformanceMonitor::new();

        monitor.set_threshold("slow_operation", PerformanceThreshold {
            max_duration: Some(Duration::from_millis(5)),
            max_memory: None,
            min_throughput: None,
        });

        let (_, alert) = monitor.record_operation("slow_operation", || {
            thread::sleep(Duration::from_millis(10));
        });

        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert!(matches!(alert.alert_type, AlertType::DurationExceeded));
    }

    #[test]
    fn test_regression_detection() {
        let mut monitor = PerformanceMonitor::new();

        // Add baseline metrics (fast)
        for _ in 0..10 {
            monitor.record_operation("regression_test", || {
                thread::sleep(Duration::from_millis(1));
            });
        }

        // Add recent metrics (slow)
        for _ in 0..10 {
            monitor.record_operation("regression_test", || {
                thread::sleep(Duration::from_millis(5));
            });
        }

        let regression = monitor.detect_regressions("regression_test", 10);
        assert!(regression.is_some());

        let regression = regression.unwrap();
        assert!(matches!(regression.alert_type, AlertType::PerformanceRegression));
    }
}