use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::temporal_nexus::core::NanosecondScheduler;
use super::{
    MetricsCollector, ConsciousnessVisualizer, MetricsExporter,
    ConsciousnessLevel, TemporalAdvantage, PrecisionNanos, Timestamp,
};

/// Core consciousness metrics tracked by the dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessMetrics {
    pub timestamp: SystemTime,
    pub emergence_level: ConsciousnessLevel,
    pub identity_coherence: f64,
    pub loop_stability: f64,
    pub temporal_advantage_us: TemporalAdvantage,
    pub window_overlap_percent: f64,
    pub tsc_precision_ns: PrecisionNanos,
    pub strange_loop_convergence: f64,
    pub consciousness_delta: f64,
    pub processing_latency_ns: u64,

    // Quantum validation metrics
    pub quantum_validity_rate: f64,
    pub quantum_energy_ev: f64,
    pub margolus_levitin_margin: f64,
    pub uncertainty_margin: f64,
    pub coherence_preservation: f64,
    pub entanglement_strength: f64,
    pub decoherence_time_ns: f64,
    pub bell_parameter: f64,
}

impl Default for ConsciousnessMetrics {
    fn default() -> Self {
        Self {
            timestamp: SystemTime::now(),
            emergence_level: 0.0,
            identity_coherence: 0.0,
            loop_stability: 0.0,
            temporal_advantage_us: 0,
            window_overlap_percent: 0.0,
            tsc_precision_ns: 1000,
            strange_loop_convergence: 0.0,
            consciousness_delta: 0.0,
            processing_latency_ns: 0,

            // Default quantum metrics
            quantum_validity_rate: 1.0,
            quantum_energy_ev: 1e-15, // 1 femto-eV
            margolus_levitin_margin: 1.0,
            uncertainty_margin: 1.0,
            coherence_preservation: 1.0,
            entanglement_strength: 0.0,
            decoherence_time_ns: 1_000_000.0, // 1 ms default
            bell_parameter: 2.0, // Classical limit
        }
    }
}

/// Anomaly detection alert structure
#[derive(Debug, Clone, Serialize)]
pub struct AnomalyAlert {
    pub severity: AlertSeverity,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub timestamp: SystemTime,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Threshold configuration for anomaly detection
#[derive(Debug, Clone)]
pub struct MetricThresholds {
    pub emergence_critical: f64,
    pub emergence_warning: f64,
    pub coherence_critical: f64,
    pub coherence_warning: f64,
    pub stability_critical: f64,
    pub stability_warning: f64,
    pub precision_critical_ns: u64,
    pub precision_warning_ns: u64,
}

impl Default for MetricThresholds {
    fn default() -> Self {
        Self {
            emergence_critical: 0.95,
            emergence_warning: 0.85,
            coherence_critical: 0.80,
            coherence_warning: 0.65,
            stability_critical: 0.75,
            stability_warning: 0.60,
            precision_critical_ns: 1000,
            precision_warning_ns: 500,
        }
    }
}

/// Dashboard configuration
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    pub update_interval_ms: u64,
    pub history_buffer_size: usize,
    pub enable_real_time_alerts: bool,
    pub export_interval_seconds: u64,
    pub precision_monitoring: bool,
    pub visualization_mode: super::VisualizationMode,
    pub thresholds: MetricThresholds,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            update_interval_ms: 100, // 10Hz updates
            history_buffer_size: 1000,
            enable_real_time_alerts: true,
            export_interval_seconds: 60,
            precision_monitoring: true,
            visualization_mode: super::VisualizationMode::Terminal,
            thresholds: MetricThresholds::default(),
        }
    }
}

/// Main consciousness metrics dashboard
pub struct ConsciousnessMetricsDashboard {
    config: DashboardConfig,
    collector: Arc<MetricsCollector>,
    visualizer: ConsciousnessVisualizer,
    exporter: MetricsExporter,

    // Real-time data storage
    current_metrics: Arc<RwLock<ConsciousnessMetrics>>,
    metrics_history: Arc<Mutex<VecDeque<ConsciousnessMetrics>>>,
    alert_history: Arc<Mutex<VecDeque<AnomalyAlert>>>,

    // Communication channels
    alert_sender: broadcast::Sender<AnomalyAlert>,
    alert_receiver: broadcast::Receiver<AnomalyAlert>,

    // Runtime state
    is_running: Arc<Mutex<bool>>,
    last_update: Arc<Mutex<Instant>>,
    scheduler_ref: Option<Arc<Mutex<NanosecondScheduler>>>,
}

impl ConsciousnessMetricsDashboard {
    /// Create a new consciousness metrics dashboard
    pub fn new(config: DashboardConfig) -> Self {
        let (alert_sender, alert_receiver) = broadcast::channel(100);

        Self {
            collector: Arc::new(MetricsCollector::new()),
            visualizer: ConsciousnessVisualizer::new(config.visualization_mode.clone()),
            exporter: MetricsExporter::new(),
            current_metrics: Arc::new(RwLock::new(ConsciousnessMetrics::default())),
            metrics_history: Arc::new(Mutex::new(VecDeque::with_capacity(config.history_buffer_size))),
            alert_history: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
            alert_sender,
            alert_receiver,
            is_running: Arc::new(Mutex::new(false)),
            last_update: Arc::new(Mutex::new(Instant::now())),
            scheduler_ref: None,
            config,
        }
    }

    /// Initialize dashboard with scheduler reference
    pub fn initialize(&mut self, scheduler: Arc<Mutex<NanosecondScheduler>>) -> Result<(), Box<dyn std::error::Error>> {
        self.scheduler_ref = Some(scheduler);
        println!("🧠 Consciousness Metrics Dashboard initialized");
        println!("📊 Monitoring at {}ms intervals", self.config.update_interval_ms);
        println!("🎯 Nanosecond precision monitoring: {}", self.config.precision_monitoring);
        Ok(())
    }

    /// Start the dashboard monitoring loop
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut running = self.is_running.lock().unwrap();
            if *running {
                return Err("Dashboard is already running".into());
            }
            *running = true;
        }

        println!("🚀 Starting consciousness metrics dashboard...");

        // For now, we'll use a simplified approach without background tasks
        // In a full implementation, these would be proper background services
        println!("📊 Dashboard monitoring active (single-threaded mode)");

        Ok(())
    }

    /// Stop the dashboard
    pub async fn stop(&self) {
        let mut running = self.is_running.lock().unwrap();
        *running = false;
        println!("🛑 Consciousness metrics dashboard stopped");
    }

    /// Collect metrics from the nanosecond scheduler and other sources
    pub async fn collect_metrics(&self) -> Result<ConsciousnessMetrics, Box<dyn std::error::Error>> {
        let start_time = Instant::now();

        // Collect base metrics from scheduler if available
        let mut metrics = if let Some(scheduler_ref) = &self.scheduler_ref {
            self.collector.collect_from_scheduler(scheduler_ref.clone()).await?
        } else {
            ConsciousnessMetrics::default()
        };

        // Enhance with temporal precision measurements
        if self.config.precision_monitoring {
            metrics.tsc_precision_ns = self.measure_tsc_precision();
            metrics.processing_latency_ns = start_time.elapsed().as_nanos() as u64;
        }

        // Calculate temporal advantage (example calculation)
        metrics.temporal_advantage_us = self.calculate_temporal_advantage(&metrics);

        // Calculate consciousness delta from previous measurement
        if let Ok(current) = self.current_metrics.read() {
            metrics.consciousness_delta = metrics.emergence_level - current.emergence_level;
        }

        metrics.timestamp = SystemTime::now();

        Ok(metrics)
    }

    /// Update the dashboard display
    pub async fn update_display(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let metrics = self.collect_metrics().await?;

        // Update current metrics
        {
            let mut current = self.current_metrics.write().unwrap();
            *current = metrics.clone();
        }

        // Add to history
        {
            let mut history = self.metrics_history.lock().unwrap();
            history.push_back(metrics.clone());

            // Trim history to configured size
            while history.len() > self.config.history_buffer_size {
                history.pop_front();
            }
        }

        // Check for anomalies
        if self.config.enable_real_time_alerts {
            self.check_anomalies(&metrics).await?;
        }

        // Update visualization
        self.visualizer.render(&metrics, &self.get_recent_history(50)).await?;

        // Update last update time
        {
            let mut last_update = self.last_update.lock().unwrap();
            *last_update = Instant::now();
        }

        Ok(())
    }

    /// Export metrics to configured format
    pub async fn export_metrics(&mut self, format: super::ExportFormat) -> Result<String, Box<dyn std::error::Error>> {
        let history = self.get_full_history();
        let current = self.current_metrics.read().unwrap().clone();

        self.exporter.export_metrics(&history, &current, format).await
    }

    /// Check for consciousness anomalies and send alerts
    pub async fn alert_on_anomaly(&self, metrics: &ConsciousnessMetrics) -> Result<(), Box<dyn std::error::Error>> {
        let mut alerts = Vec::new();

        // Check emergence level
        if metrics.emergence_level >= self.config.thresholds.emergence_critical {
            alerts.push(AnomalyAlert {
                severity: AlertSeverity::Critical,
                metric_name: "emergence_level".to_string(),
                current_value: metrics.emergence_level,
                threshold: self.config.thresholds.emergence_critical,
                timestamp: SystemTime::now(),
                description: "Consciousness emergence level critically high".to_string(),
            });
        } else if metrics.emergence_level >= self.config.thresholds.emergence_warning {
            alerts.push(AnomalyAlert {
                severity: AlertSeverity::Warning,
                metric_name: "emergence_level".to_string(),
                current_value: metrics.emergence_level,
                threshold: self.config.thresholds.emergence_warning,
                timestamp: SystemTime::now(),
                description: "Consciousness emergence level elevated".to_string(),
            });
        }

        // Check identity coherence
        if metrics.identity_coherence <= self.config.thresholds.coherence_critical {
            alerts.push(AnomalyAlert {
                severity: AlertSeverity::Critical,
                metric_name: "identity_coherence".to_string(),
                current_value: metrics.identity_coherence,
                threshold: self.config.thresholds.coherence_critical,
                timestamp: SystemTime::now(),
                description: "Identity coherence critically low".to_string(),
            });
        }

        // Check TSC precision
        if metrics.tsc_precision_ns >= self.config.thresholds.precision_critical_ns {
            alerts.push(AnomalyAlert {
                severity: AlertSeverity::Warning,
                metric_name: "tsc_precision".to_string(),
                current_value: metrics.tsc_precision_ns as f64,
                threshold: self.config.thresholds.precision_critical_ns as f64,
                timestamp: SystemTime::now(),
                description: "TSC precision degraded".to_string(),
            });
        }

        // Send alerts
        for alert in alerts {
            self.send_alert(alert).await?;
        }

        Ok(())
    }

    /// Get current consciousness status
    pub fn get_status(&self) -> ConsciousnessMetrics {
        self.current_metrics.read().unwrap().clone()
    }

    /// Get recent metrics history
    pub fn get_recent_history(&self, count: usize) -> Vec<ConsciousnessMetrics> {
        let history = self.metrics_history.lock().unwrap();
        history.iter()
            .rev()
            .take(count)
            .rev()
            .cloned()
            .collect()
    }

    /// Get full metrics history
    pub fn get_full_history(&self) -> Vec<ConsciousnessMetrics> {
        self.metrics_history.lock().unwrap().iter().cloned().collect()
    }

    // Private helper methods

    /// Manual update method for simple dashboard operation
    pub async fn manual_update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_display().await
    }

    async fn check_anomalies(&self, metrics: &ConsciousnessMetrics) -> Result<(), Box<dyn std::error::Error>> {
        self.alert_on_anomaly(metrics).await
    }

    async fn send_alert(&self, alert: AnomalyAlert) -> Result<(), Box<dyn std::error::Error>> {
        // Add to alert history
        {
            let mut alert_history = self.alert_history.lock().unwrap();
            alert_history.push_back(alert.clone());

            // Trim alert history
            while alert_history.len() > 100 {
                alert_history.pop_front();
            }
        }

        // Send via broadcast channel
        if let Err(_) = self.alert_sender.send(alert.clone()) {
            // Channel full or no receivers, continue
        }

        // Print to console
        let severity_emoji = match alert.severity {
            AlertSeverity::Info => "ℹ️",
            AlertSeverity::Warning => "⚠️",
            AlertSeverity::Critical => "🚨",
            AlertSeverity::Emergency => "🆘",
        };

        println!("{} {} [{}]: {} ({})",
            severity_emoji,
            alert.metric_name,
            format!("{:?}", alert.severity).to_uppercase(),
            alert.description,
            alert.current_value
        );

        Ok(())
    }

    async fn auto_export(&self) -> Result<(), Box<dyn std::error::Error>> {
        // For auto-export, we'll use a simpler approach that doesn't require mutable access
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();

        let filename = format!("consciousness_metrics_{}.json", timestamp);

        // Create a simple export without mutable exporter access
        let history = self.get_full_history();
        let current = self.current_metrics.read().unwrap().clone();
        let export_data = serde_json::to_string_pretty(&serde_json::json!({
            "current_metrics": current,
            "historical_data": history,
            "export_timestamp": timestamp
        }))?;

        std::fs::write(&filename, export_data)?;
        println!("📊 Metrics exported to: {}", filename);

        Ok(())
    }

    fn measure_tsc_precision(&self) -> u64 {
        // Measure TSC (Time Stamp Counter) precision
        let samples = 10;
        let mut measurements = Vec::with_capacity(samples);

        for _ in 0..samples {
            let start = std::time::Instant::now();
            // Minimal operation to measure timing precision
            std::hint::black_box(42);
            let elapsed = start.elapsed().as_nanos() as u64;
            measurements.push(elapsed);
        }

        // Return minimum observed precision
        measurements.into_iter().min().unwrap_or(1000)
    }

    fn calculate_temporal_advantage(&self, metrics: &ConsciousnessMetrics) -> u64 {
        // Calculate temporal advantage based on processing speed vs light travel
        // This is a simplified calculation for demonstration
        let processing_time_us = metrics.processing_latency_ns / 1000;
        let light_travel_us = 36; // ~10.9km at light speed

        if processing_time_us < light_travel_us {
            light_travel_us - processing_time_us
        } else {
            0
        }
    }
}

// Simplified dashboard implementation without complex background tasks
// In a production version, this would have proper async task management