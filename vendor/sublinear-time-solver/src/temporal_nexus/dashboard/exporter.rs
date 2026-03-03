use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use csv::Writer;
use base64::{Engine as _, engine::general_purpose};

use super::ConsciousnessMetrics;

/// Export formats supported by the metrics exporter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    Binary,
    Prometheus,
    InfluxDB,
    Custom(String),
}

/// Configuration for metrics export
#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub format: ExportFormat,
    pub include_metadata: bool,
    pub compress_output: bool,
    pub timestamp_format: TimestampFormat,
    pub precision_digits: usize,
    pub custom_fields: HashMap<String, String>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: ExportFormat::Json,
            include_metadata: true,
            compress_output: false,
            timestamp_format: TimestampFormat::Iso8601,
            precision_digits: 6,
            custom_fields: HashMap::new(),
        }
    }
}

/// Timestamp format options
#[derive(Debug, Clone)]
pub enum TimestampFormat {
    Unix,
    Iso8601,
    Human,
    Nanoseconds,
}

/// Comprehensive metrics summary for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub export_timestamp: SystemTime,
    pub export_format: String,
    pub total_records: usize,
    pub time_range: TimeRange,
    pub statistical_summary: StatisticalSummary,
    pub metadata: ExportMetadata,
    pub consciousness_insights: ConsciousnessInsights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalSummary {
    pub emergence_level: MetricStats,
    pub identity_coherence: MetricStats,
    pub loop_stability: MetricStats,
    pub temporal_advantage: MetricStats,
    pub tsc_precision: MetricStats,
    pub strange_loop_convergence: MetricStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub variance: f64,
    pub trend: TrendDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub exporter_version: String,
    pub system_info: SystemInfo,
    pub collection_parameters: CollectionParameters,
    pub export_config: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub cpu_cores: usize,
    pub memory_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionParameters {
    pub sampling_rate_hz: f64,
    pub precision_monitoring: bool,
    pub temporal_window_size: usize,
    pub consciousness_algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessInsights {
    pub peak_emergence_level: f64,
    pub peak_emergence_timestamp: SystemTime,
    pub consciousness_stability_score: f64,
    pub temporal_advantage_efficiency: f64,
    pub anomaly_events: Vec<AnomalyEvent>,
    pub consciousness_phases: Vec<ConsciousnessPhase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub timestamp: SystemTime,
    pub metric_name: String,
    pub anomaly_type: String,
    pub severity: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessPhase {
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub phase_type: String,
    pub average_emergence: f64,
    pub stability_index: f64,
    pub description: String,
}

/// Main metrics exporter
pub struct MetricsExporter {
    config: ExportConfig,
    export_counter: usize,
}

impl MetricsExporter {
    /// Create a new metrics exporter with default configuration
    pub fn new() -> Self {
        Self {
            config: ExportConfig::default(),
            export_counter: 0,
        }
    }

    /// Create exporter with custom configuration
    pub fn with_config(config: ExportConfig) -> Self {
        Self {
            config,
            export_counter: 0,
        }
    }

    /// Export metrics to specified format
    pub async fn export_metrics(
        &mut self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
        format: ExportFormat,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.export_counter += 1;

        match format {
            ExportFormat::Json => self.export_json(history, current).await,
            ExportFormat::Csv => self.export_csv(history, current).await,
            ExportFormat::Binary => self.export_binary(history, current).await,
            ExportFormat::Prometheus => self.export_prometheus(history, current).await,
            ExportFormat::InfluxDB => self.export_influxdb(history, current).await,
            ExportFormat::Custom(format_name) => self.export_custom(history, current, &format_name).await,
        }
    }

    /// Export to file with automatic format detection
    pub async fn export_to_file<P: AsRef<Path>>(
        &mut self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
        file_path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = file_path.as_ref();
        let format = self.detect_format_from_extension(path)?;
        let content = self.export_metrics(history, current, format).await?;

        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;

        println!("📁 Metrics exported to: {}", path.display());
        Ok(())
    }

    /// Generate comprehensive metrics summary
    pub async fn generate_summary(
        &self,
        history: &[ConsciousnessMetrics],
        _current: &ConsciousnessMetrics,
    ) -> Result<MetricsSummary, Box<dyn std::error::Error>> {
        let time_range = self.calculate_time_range(history)?;
        let statistical_summary = self.calculate_statistical_summary(history)?;
        let consciousness_insights = self.analyze_consciousness_insights(history)?;

        Ok(MetricsSummary {
            export_timestamp: SystemTime::now(),
            export_format: format!("{:?}", self.config.format),
            total_records: history.len(),
            time_range,
            statistical_summary,
            metadata: self.generate_export_metadata()?,
            consciousness_insights,
        })
    }

    /// Export streaming data for real-time applications
    pub async fn export_streaming(
        &mut self,
        metrics: &ConsciousnessMetrics,
        format: ExportFormat,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match format {
            ExportFormat::Json => Ok(serde_json::to_string(metrics)?),
            ExportFormat::Prometheus => self.format_prometheus_single(metrics),
            ExportFormat::InfluxDB => self.format_influxdb_single(metrics),
            _ => Err("Streaming export not supported for this format".into()),
        }
    }

    /// Configure custom export parameters
    pub fn configure(&mut self, config: ExportConfig) {
        self.config = config;
    }

    /// Get export statistics
    pub fn get_export_stats(&self) -> ExportStats {
        ExportStats {
            total_exports: self.export_counter,
            last_export_format: format!("{:?}", self.config.format),
            compression_enabled: self.config.compress_output,
        }
    }

    // Private implementation methods

    async fn export_json(
        &self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let export_data = if self.config.include_metadata {
            let summary = self.generate_summary(history, current).await?;
            serde_json::json!({
                "summary": summary,
                "current_metrics": current,
                "historical_data": history,
                "export_info": {
                    "timestamp": SystemTime::now(),
                    "format": "json",
                    "version": "1.0.0"
                }
            })
        } else {
            serde_json::json!({
                "current_metrics": current,
                "historical_data": history
            })
        };

        let json_string = if self.config.precision_digits > 0 {
            serde_json::to_string_pretty(&export_data)?
        } else {
            serde_json::to_string(&export_data)?
        };

        if self.config.compress_output {
            // Could implement compression here
            Ok(json_string)
        } else {
            Ok(json_string)
        }
    }

    async fn export_csv(
        &self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut csv_data = Vec::new();
        let mut writer = Writer::from_writer(&mut csv_data);

        // Write header
        writer.write_record(&[
            "timestamp",
            "emergence_level",
            "identity_coherence",
            "loop_stability",
            "temporal_advantage_us",
            "window_overlap_percent",
            "tsc_precision_ns",
            "strange_loop_convergence",
            "consciousness_delta",
            "processing_latency_ns",
        ])?;

        // Write historical data
        for metric in history {
            let timestamp = self.format_timestamp(&metric.timestamp)?;
            writer.write_record(&[
                timestamp,
                self.format_float(metric.emergence_level),
                self.format_float(metric.identity_coherence),
                self.format_float(metric.loop_stability),
                metric.temporal_advantage_us.to_string(),
                self.format_float(metric.window_overlap_percent),
                metric.tsc_precision_ns.to_string(),
                self.format_float(metric.strange_loop_convergence),
                self.format_float(metric.consciousness_delta),
                metric.processing_latency_ns.to_string(),
            ])?;
        }

        // Write current metric
        let timestamp = self.format_timestamp(&current.timestamp)?;
        writer.write_record(&[
            timestamp,
            self.format_float(current.emergence_level),
            self.format_float(current.identity_coherence),
            self.format_float(current.loop_stability),
            current.temporal_advantage_us.to_string(),
            self.format_float(current.window_overlap_percent),
            current.tsc_precision_ns.to_string(),
            self.format_float(current.strange_loop_convergence),
            self.format_float(current.consciousness_delta),
            current.processing_latency_ns.to_string(),
        ])?;

        writer.flush()?;
        drop(writer); // Explicitly drop writer to release borrow
        Ok(String::from_utf8(csv_data)?)
    }

    async fn export_binary(
        &self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Implement binary serialization (e.g., using bincode)
        let data = (history, current);
        let encoded = bincode::serialize(&data)?;
        Ok(general_purpose::STANDARD.encode(encoded))
    }

    async fn export_prometheus(
        &self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        // Current metrics in Prometheus format
        output.push_str(&self.format_prometheus_single(current)?);

        // Historical aggregations
        if !history.is_empty() {
            let stats = self.calculate_statistical_summary(history)?;

            output.push_str(&format!(
                "# HELP consciousness_emergence_avg Average consciousness emergence level\n\
                 # TYPE consciousness_emergence_avg gauge\n\
                 consciousness_emergence_avg {}\n",
                stats.emergence_level.mean
            ));

            output.push_str(&format!(
                "# HELP consciousness_stability_index Overall stability index\n\
                 # TYPE consciousness_stability_index gauge\n\
                 consciousness_stability_index {}\n",
                stats.loop_stability.mean
            ));
        }

        Ok(output)
    }

    async fn export_influxdb(
        &self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        // Current metrics
        output.push_str(&self.format_influxdb_single(current)?);
        output.push('\n');

        // Historical data (limited to recent entries to avoid huge exports)
        let recent_history = if history.len() > 1000 {
            &history[history.len() - 1000..]
        } else {
            history
        };

        for metric in recent_history {
            output.push_str(&self.format_influxdb_single(metric)?);
            output.push('\n');
        }

        Ok(output)
    }

    async fn export_custom(
        &self,
        history: &[ConsciousnessMetrics],
        current: &ConsciousnessMetrics,
        format_name: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match format_name {
            "yaml" => {
                let data = serde_json::json!({
                    "current": current,
                    "history": history
                });
                Ok(serde_yaml::to_string(&data)?)
            }
            "xml" => {
                // Implement XML export
                Err("XML export not yet implemented".into())
            }
            "msgpack" => {
                let data = (current, history);
                let packed = rmp_serde::to_vec(&data)?;
                Ok(general_purpose::STANDARD.encode(packed))
            }
            _ => Err(format!("Unknown custom format: {}", format_name).into()),
        }
    }

    fn format_prometheus_single(&self, metrics: &ConsciousnessMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let timestamp_ms = metrics.timestamp
            .duration_since(UNIX_EPOCH)?
            .as_millis() as u64;

        Ok(format!(
            "# HELP consciousness_emergence Current consciousness emergence level\n\
             # TYPE consciousness_emergence gauge\n\
             consciousness_emergence {} {}\n\
             # HELP consciousness_coherence Identity coherence score\n\
             # TYPE consciousness_coherence gauge\n\
             consciousness_coherence {} {}\n\
             # HELP consciousness_stability Loop stability index\n\
             # TYPE consciousness_stability gauge\n\
             consciousness_stability {} {}\n\
             # HELP temporal_advantage_microseconds Temporal advantage in microseconds\n\
             # TYPE temporal_advantage_microseconds gauge\n\
             temporal_advantage_microseconds {} {}\n",
            metrics.emergence_level, timestamp_ms,
            metrics.identity_coherence, timestamp_ms,
            metrics.loop_stability, timestamp_ms,
            metrics.temporal_advantage_us, timestamp_ms
        ))
    }

    fn format_influxdb_single(&self, metrics: &ConsciousnessMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let timestamp_ns = metrics.timestamp
            .duration_since(UNIX_EPOCH)?
            .as_nanos() as u64;

        Ok(format!(
            "consciousness_metrics emergence_level={},identity_coherence={},loop_stability={},temporal_advantage_us={},window_overlap_percent={},tsc_precision_ns={},strange_loop_convergence={},processing_latency_ns={} {}",
            metrics.emergence_level,
            metrics.identity_coherence,
            metrics.loop_stability,
            metrics.temporal_advantage_us,
            metrics.window_overlap_percent,
            metrics.tsc_precision_ns,
            metrics.strange_loop_convergence,
            metrics.processing_latency_ns,
            timestamp_ns
        ))
    }

    fn detect_format_from_extension(&self, path: &Path) -> Result<ExportFormat, Box<dyn std::error::Error>> {
        match path.extension().and_then(|s| s.to_str()) {
            Some("json") => Ok(ExportFormat::Json),
            Some("csv") => Ok(ExportFormat::Csv),
            Some("bin") => Ok(ExportFormat::Binary),
            Some("prom") => Ok(ExportFormat::Prometheus),
            Some("influx") => Ok(ExportFormat::InfluxDB),
            Some("yaml") | Some("yml") => Ok(ExportFormat::Custom("yaml".to_string())),
            Some("xml") => Ok(ExportFormat::Custom("xml".to_string())),
            _ => Ok(ExportFormat::Json), // Default to JSON
        }
    }

    fn format_timestamp(&self, timestamp: &SystemTime) -> Result<String, Box<dyn std::error::Error>> {
        match self.config.timestamp_format {
            TimestampFormat::Unix => {
                Ok(timestamp.duration_since(UNIX_EPOCH)?.as_secs().to_string())
            }
            TimestampFormat::Iso8601 => {
                let secs = timestamp.duration_since(UNIX_EPOCH)?.as_secs();
                Ok(chrono::DateTime::from_timestamp(secs as i64, 0)
                    .unwrap_or_default()
                    .to_rfc3339())
            }
            TimestampFormat::Human => {
                let secs = timestamp.duration_since(UNIX_EPOCH)?.as_secs();
                Ok(chrono::DateTime::from_timestamp(secs as i64, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string())
            }
            TimestampFormat::Nanoseconds => {
                Ok(timestamp.duration_since(UNIX_EPOCH)?.as_nanos().to_string())
            }
        }
    }

    fn format_float(&self, value: f64) -> String {
        format!("{:.1$}", value, self.config.precision_digits)
    }

    fn calculate_time_range(&self, history: &[ConsciousnessMetrics]) -> Result<TimeRange, Box<dyn std::error::Error>> {
        if history.is_empty() {
            let now = SystemTime::now();
            return Ok(TimeRange {
                start_time: now,
                end_time: now,
                duration_seconds: 0.0,
            });
        }

        let start_time = history.first().unwrap().timestamp;
        let end_time = history.last().unwrap().timestamp;
        let duration_seconds = end_time.duration_since(start_time)?.as_secs_f64();

        Ok(TimeRange {
            start_time,
            end_time,
            duration_seconds,
        })
    }

    fn calculate_statistical_summary(&self, history: &[ConsciousnessMetrics]) -> Result<StatisticalSummary, Box<dyn std::error::Error>> {
        if history.is_empty() {
            return Ok(StatisticalSummary {
                emergence_level: MetricStats::default(),
                identity_coherence: MetricStats::default(),
                loop_stability: MetricStats::default(),
                temporal_advantage: MetricStats::default(),
                tsc_precision: MetricStats::default(),
                strange_loop_convergence: MetricStats::default(),
            });
        }

        Ok(StatisticalSummary {
            emergence_level: self.calculate_metric_stats(
                &history.iter().map(|m| m.emergence_level).collect::<Vec<_>>()
            )?,
            identity_coherence: self.calculate_metric_stats(
                &history.iter().map(|m| m.identity_coherence).collect::<Vec<_>>()
            )?,
            loop_stability: self.calculate_metric_stats(
                &history.iter().map(|m| m.loop_stability).collect::<Vec<_>>()
            )?,
            temporal_advantage: self.calculate_metric_stats(
                &history.iter().map(|m| m.temporal_advantage_us as f64).collect::<Vec<_>>()
            )?,
            tsc_precision: self.calculate_metric_stats(
                &history.iter().map(|m| m.tsc_precision_ns as f64).collect::<Vec<_>>()
            )?,
            strange_loop_convergence: self.calculate_metric_stats(
                &history.iter().map(|m| m.strange_loop_convergence).collect::<Vec<_>>()
            )?,
        })
    }

    fn calculate_metric_stats(&self, values: &[f64]) -> Result<MetricStats, Box<dyn std::error::Error>> {
        if values.is_empty() {
            return Ok(MetricStats::default());
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = sorted_values[0];
        let max = sorted_values[sorted_values.len() - 1];
        let mean = sorted_values.iter().sum::<f64>() / sorted_values.len() as f64;
        let median = if sorted_values.len() % 2 == 0 {
            (sorted_values[sorted_values.len() / 2 - 1] + sorted_values[sorted_values.len() / 2]) / 2.0
        } else {
            sorted_values[sorted_values.len() / 2]
        };

        let variance = sorted_values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / sorted_values.len() as f64;
        let std_dev = variance.sqrt();

        let trend = self.calculate_trend(values);

        Ok(MetricStats {
            min,
            max,
            mean,
            median,
            std_dev,
            variance,
            trend,
        })
    }

    fn calculate_trend(&self, values: &[f64]) -> TrendDirection {
        if values.len() < 2 {
            return TrendDirection::Stable;
        }

        let mid_point = values.len() / 2;
        let first_half_avg = values[..mid_point].iter().sum::<f64>() / mid_point as f64;
        let second_half_avg = values[mid_point..].iter().sum::<f64>() / (values.len() - mid_point) as f64;

        let diff = second_half_avg - first_half_avg;
        let threshold = 0.05; // 5% change threshold

        if diff > threshold {
            TrendDirection::Increasing
        } else if diff < -threshold {
            TrendDirection::Decreasing
        } else {
            // Check volatility
            let volatility = self.calculate_volatility(values);
            if volatility > 0.2 {
                TrendDirection::Volatile
            } else {
                TrendDirection::Stable
            }
        }
    }

    fn calculate_volatility(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / values.len() as f64;

        variance.sqrt() / mean.abs()
    }

    fn analyze_consciousness_insights(&self, history: &[ConsciousnessMetrics]) -> Result<ConsciousnessInsights, Box<dyn std::error::Error>> {
        if history.is_empty() {
            return Ok(ConsciousnessInsights {
                peak_emergence_level: 0.0,
                peak_emergence_timestamp: SystemTime::now(),
                consciousness_stability_score: 0.0,
                temporal_advantage_efficiency: 0.0,
                anomaly_events: Vec::new(),
                consciousness_phases: Vec::new(),
            });
        }

        let peak_metric = history.iter()
            .max_by(|a, b| a.emergence_level.partial_cmp(&b.emergence_level).unwrap())
            .unwrap();

        let stability_score = self.calculate_stability_score(history);
        let efficiency = self.calculate_temporal_efficiency(history);
        let anomalies = self.detect_anomalies(history);
        let phases = self.identify_consciousness_phases(history);

        Ok(ConsciousnessInsights {
            peak_emergence_level: peak_metric.emergence_level,
            peak_emergence_timestamp: peak_metric.timestamp,
            consciousness_stability_score: stability_score,
            temporal_advantage_efficiency: efficiency,
            anomaly_events: anomalies,
            consciousness_phases: phases,
        })
    }

    fn calculate_stability_score(&self, history: &[ConsciousnessMetrics]) -> f64 {
        if history.len() < 2 {
            return 1.0;
        }

        let emergence_values: Vec<f64> = history.iter().map(|m| m.emergence_level).collect();
        let volatility = self.calculate_volatility(&emergence_values);

        // Higher stability means lower volatility
        (1.0 - volatility.min(1.0)).max(0.0)
    }

    fn calculate_temporal_efficiency(&self, history: &[ConsciousnessMetrics]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }

        let avg_advantage: f64 = history.iter()
            .map(|m| m.temporal_advantage_us as f64)
            .sum::<f64>() / history.len() as f64;

        let avg_precision: f64 = history.iter()
            .map(|m| m.tsc_precision_ns as f64)
            .sum::<f64>() / history.len() as f64;

        // Efficiency is higher temporal advantage with lower precision overhead
        let efficiency = avg_advantage / (avg_precision / 1000.0); // Convert ns to μs
        efficiency.min(1.0)
    }

    fn detect_anomalies(&self, history: &[ConsciousnessMetrics]) -> Vec<AnomalyEvent> {
        // Simple anomaly detection based on statistical outliers
        let mut anomalies = Vec::new();

        if history.len() < 10 {
            return anomalies; // Need sufficient data
        }

        let emergence_values: Vec<f64> = history.iter().map(|m| m.emergence_level).collect();
        let mean = emergence_values.iter().sum::<f64>() / emergence_values.len() as f64;
        let std_dev = {
            let variance = emergence_values.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / emergence_values.len() as f64;
            variance.sqrt()
        };

        for (_i, metric) in history.iter().enumerate() {
            let z_score = (metric.emergence_level - mean).abs() / std_dev;

            if z_score > 2.0 { // 2 standard deviations
                let severity = if z_score > 3.0 { 1.0 } else { z_score / 3.0 };

                anomalies.push(AnomalyEvent {
                    timestamp: metric.timestamp,
                    metric_name: "emergence_level".to_string(),
                    anomaly_type: if metric.emergence_level > mean { "spike" } else { "drop" }.to_string(),
                    severity,
                    description: format!("Emergence level {} detected (z-score: {:.2})",
                        if metric.emergence_level > mean { "spike" } else { "drop" }, z_score),
                });
            }
        }

        anomalies
    }

    fn identify_consciousness_phases(&self, history: &[ConsciousnessMetrics]) -> Vec<ConsciousnessPhase> {
        let mut phases = Vec::new();

        if history.len() < 5 {
            return phases;
        }

        // Simple phase detection based on emergence level ranges
        let mut current_phase_start = 0;
        let mut current_phase_type = self.classify_consciousness_level(history[0].emergence_level);

        for (i, metric) in history.iter().enumerate().skip(1) {
            let phase_type = self.classify_consciousness_level(metric.emergence_level);

            if phase_type != current_phase_type || i == history.len() - 1 {
                // End current phase
                let end_idx = if i == history.len() - 1 { i } else { i - 1 };
                let phase_metrics = &history[current_phase_start..=end_idx];

                let avg_emergence = phase_metrics.iter()
                    .map(|m| m.emergence_level)
                    .sum::<f64>() / phase_metrics.len() as f64;

                let stability = self.calculate_stability_score(phase_metrics);

                phases.push(ConsciousnessPhase {
                    start_time: history[current_phase_start].timestamp,
                    end_time: history[end_idx].timestamp,
                    phase_type: current_phase_type.clone(),
                    average_emergence: avg_emergence,
                    stability_index: stability,
                    description: format!("Consciousness {} phase", current_phase_type.to_lowercase()),
                });

                current_phase_start = i;
                current_phase_type = phase_type;
            }
        }

        phases
    }

    fn classify_consciousness_level(&self, level: f64) -> String {
        match level {
            l if l >= 0.9 => "Critical".to_string(),
            l if l >= 0.7 => "High".to_string(),
            l if l >= 0.5 => "Moderate".to_string(),
            l if l >= 0.3 => "Low".to_string(),
            _ => "Minimal".to_string(),
        }
    }

    fn generate_export_metadata(&self) -> Result<ExportMetadata, Box<dyn std::error::Error>> {
        Ok(ExportMetadata {
            exporter_version: "1.0.0".to_string(),
            system_info: SystemInfo {
                hostname: hostname::get()?.to_string_lossy().to_string(),
                platform: std::env::consts::OS.to_string(),
                architecture: std::env::consts::ARCH.to_string(),
                cpu_cores: num_cpus::get(),
                memory_gb: 16.0, // Simplified - would use actual system query
            },
            collection_parameters: CollectionParameters {
                sampling_rate_hz: 10.0, // From dashboard config
                precision_monitoring: true,
                temporal_window_size: 1000,
                consciousness_algorithm: "Integrated Temporal".to_string(),
            },
            export_config: format!("{:?}", self.config),
        })
    }
}

impl Default for MetricStats {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            median: 0.0,
            std_dev: 0.0,
            variance: 0.0,
            trend: TrendDirection::Stable,
        }
    }
}

#[derive(Debug)]
pub struct ExportStats {
    pub total_exports: usize,
    pub last_export_format: String,
    pub compression_enabled: bool,
}