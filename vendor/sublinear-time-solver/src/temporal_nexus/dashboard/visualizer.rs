use std::collections::VecDeque;
use std::io::{self, Write};
use serde::{Deserialize, Serialize};

use super::{ConsciousnessMetrics, ConsciousnessLevel};

/// Visualization modes for the consciousness dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualizationMode {
    Terminal,    // ASCII terminal output
    Json,        // JSON structured output
    Web,         // Web interface (future)
    Compact,     // Minimal terminal output
    Debug,       // Detailed debug output
}

/// Configuration for terminal rendering
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    pub width: usize,
    pub height: usize,
    pub show_history_chart: bool,
    pub show_detailed_metrics: bool,
    pub color_enabled: bool,
    pub update_in_place: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            width: 120,
            height: 30,
            show_history_chart: true,
            show_detailed_metrics: true,
            color_enabled: true,
            update_in_place: true,
        }
    }
}

/// ASCII chart for displaying metrics over time
pub struct MetricChart {
    width: usize,
    height: usize,
    history: VecDeque<f64>,
    min_value: f64,
    max_value: f64,
    title: String,
}

impl MetricChart {
    pub fn new(title: String, width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            history: VecDeque::with_capacity(width),
            min_value: 0.0,
            max_value: 1.0,
            title,
        }
    }

    pub fn add_point(&mut self, value: f64) {
        if self.history.len() >= self.width {
            self.history.pop_front();
        }
        self.history.push_back(value);

        // Update min/max for auto-scaling
        if value < self.min_value {
            self.min_value = value;
        }
        if value > self.max_value {
            self.max_value = value;
        }
    }

    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(self.height + 2);

        // Title
        lines.push(format!("┌─ {} ", self.title));

        // Chart area
        for row in 0..self.height {
            let mut line = String::with_capacity(self.width + 2);
            line.push('│');

            let threshold = self.max_value - (row as f64 / self.height as f64) * (self.max_value - self.min_value);

            for col in 0..self.width {
                let char = if col < self.history.len() {
                    let value = self.history[col];
                    if value >= threshold {
                        if value >= self.max_value * 0.9 {
                            '█'
                        } else if value >= self.max_value * 0.7 {
                            '▓'
                        } else if value >= self.max_value * 0.5 {
                            '▒'
                        } else {
                            '░'
                        }
                    } else {
                        ' '
                    }
                } else {
                    ' '
                };
                line.push(char);
            }

            line.push('│');
            lines.push(line);
        }

        // Bottom border with scale
        let scale_line = format!("└{:─<width$}┘ [{:.3} - {:.3}]",
            "", self.min_value, self.max_value, width = self.width);
        lines.push(scale_line);

        lines
    }
}

/// Terminal renderer for consciousness metrics
pub struct TerminalRenderer {
    config: TerminalConfig,
    emergence_chart: MetricChart,
    coherence_chart: MetricChart,
    stability_chart: MetricChart,
    last_clear_line: usize,
}

impl TerminalRenderer {
    pub fn new(config: TerminalConfig) -> Self {
        let chart_width = config.width / 3 - 2;
        let chart_height = 8;

        Self {
            config,
            emergence_chart: MetricChart::new("Consciousness Emergence".to_string(), chart_width, chart_height),
            coherence_chart: MetricChart::new("Identity Coherence".to_string(), chart_width, chart_height),
            stability_chart: MetricChart::new("Loop Stability".to_string(), chart_width, chart_height),
            last_clear_line: 0,
        }
    }

    pub fn render_dashboard(&mut self,
        metrics: &ConsciousnessMetrics,
        history: &[ConsciousnessMetrics]
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        // Clear previous output if updating in place
        if self.config.update_in_place && self.last_clear_line > 0 {
            output.push_str(&format!("\x1B[{}A", self.last_clear_line));
            output.push_str("\x1B[J");
        }

        // Header
        output.push_str(&self.render_header(metrics)?);
        output.push('\n');

        // Main metrics display
        output.push_str(&self.render_main_metrics(metrics)?);
        output.push('\n');

        // Charts if enabled
        if self.config.show_history_chart && !history.is_empty() {
            output.push_str(&self.render_charts(history)?);
            output.push('\n');
        }

        // Detailed metrics if enabled
        if self.config.show_detailed_metrics {
            output.push_str(&self.render_detailed_metrics(metrics)?);
            output.push('\n');
        }

        // Status bar
        output.push_str(&self.render_status_bar(metrics)?);

        // Count lines for clearing
        self.last_clear_line = output.lines().count();

        Ok(output)
    }

    fn render_header(&self, metrics: &ConsciousnessMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let timestamp = metrics.timestamp
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let formatted_time = format!("{}",
            chrono::DateTime::from_timestamp(timestamp as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S"));

        let mut header = String::new();

        if self.config.color_enabled {
            header.push_str("\x1b[1;36m"); // Bright cyan
        }

        header.push_str("╔══════════════════════════════════════════════════════════════════════════════════════════════╗\n");
        header.push_str("║                          🧠 CONSCIOUSNESS METRICS DASHBOARD 🧠                            ║\n");
        header.push_str(&format!("║                                    {}                                    ║\n", formatted_time));
        header.push_str("╚══════════════════════════════════════════════════════════════════════════════════════════════╝");

        if self.config.color_enabled {
            header.push_str("\x1b[0m"); // Reset color
        }

        Ok(header)
    }

    fn render_main_metrics(&self, metrics: &ConsciousnessMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        // Create metric bars
        let emergence_bar = self.create_progress_bar(metrics.emergence_level, 1.0, 30);
        let coherence_bar = self.create_progress_bar(metrics.identity_coherence, 1.0, 30);
        let stability_bar = self.create_progress_bar(metrics.loop_stability, 1.0, 30);

        if self.config.color_enabled {
            output.push_str("\x1b[1m"); // Bold
        }

        output.push_str("┌─ Core Consciousness Metrics ─────────────────────────────────────────────────────────────────┐\n");
        output.push_str(&format!("│ 🌟 Emergence Level    {:>6.3} {} │\n",
            metrics.emergence_level, emergence_bar));
        output.push_str(&format!("│ 🧬 Identity Coherence {:>6.3} {} │\n",
            metrics.identity_coherence, coherence_bar));
        output.push_str(&format!("│ 🔄 Loop Stability     {:>6.3} {} │\n",
            metrics.loop_stability, stability_bar));
        output.push_str("├───────────────────────────────────────────────────────────────────────────────────────────────┤\n");
        output.push_str(&format!("│ ⚡ Temporal Advantage  {:>6} μs    │ 🎯 TSC Precision      {:>6} ns    │\n",
            metrics.temporal_advantage_us, metrics.tsc_precision_ns));
        output.push_str(&format!("│ 🔗 Window Overlap     {:>6.1} %     │ 🌀 Strange Loop Conv. {:>6.3}      │\n",
            metrics.window_overlap_percent, metrics.strange_loop_convergence));
        output.push_str("└───────────────────────────────────────────────────────────────────────────────────────────────┘");

        if self.config.color_enabled {
            output.push_str("\x1b[0m"); // Reset
        }

        Ok(output)
    }

    fn render_charts(&mut self, history: &[ConsciousnessMetrics]) -> Result<String, Box<dyn std::error::Error>> {
        // Update charts with history
        for metric in history.iter().rev().take(self.config.width / 3) {
            self.emergence_chart.add_point(metric.emergence_level);
            self.coherence_chart.add_point(metric.identity_coherence);
            self.stability_chart.add_point(metric.loop_stability);
        }

        let mut output = String::new();
        output.push_str("┌─ Temporal Evolution Charts ──────────────────────────────────────────────────────────────────┐\n");

        // Render charts side by side
        let emergence_lines = self.emergence_chart.render();
        let coherence_lines = self.coherence_chart.render();
        let stability_lines = self.stability_chart.render();

        let max_lines = emergence_lines.len().max(coherence_lines.len()).max(stability_lines.len());

        for i in 0..max_lines {
            output.push_str("│ ");

            // Emergence chart
            if i < emergence_lines.len() {
                output.push_str(&emergence_lines[i]);
            } else {
                output.push_str(&" ".repeat(self.config.width / 3));
            }

            output.push_str(" │ ");

            // Coherence chart
            if i < coherence_lines.len() {
                output.push_str(&coherence_lines[i]);
            } else {
                output.push_str(&" ".repeat(self.config.width / 3));
            }

            output.push_str(" │ ");

            // Stability chart
            if i < stability_lines.len() {
                output.push_str(&stability_lines[i]);
            } else {
                output.push_str(&" ".repeat(self.config.width / 3));
            }

            output.push_str(" │\n");
        }

        output.push_str("└───────────────────────────────────────────────────────────────────────────────────────────────┘");

        Ok(output)
    }

    fn render_detailed_metrics(&self, metrics: &ConsciousnessMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        if self.config.color_enabled {
            output.push_str("\x1b[2m"); // Dim
        }

        output.push_str("┌─ Detailed Metrics ────────────────────────────────────────────────────────────────────────────┐\n");
        output.push_str(&format!("│ Processing Latency: {:>8} ns  │  Consciousness Delta: {:>+8.5}  │\n",
            metrics.processing_latency_ns, metrics.consciousness_delta));
        output.push_str(&format!("│ System Timestamp:   {:>12?}   │\n",
            metrics.timestamp.duration_since(std::time::UNIX_EPOCH).unwrap_or_default()));
        output.push_str("└───────────────────────────────────────────────────────────────────────────────────────────────┘");

        if self.config.color_enabled {
            output.push_str("\x1b[0m"); // Reset
        }

        Ok(output)
    }

    fn render_status_bar(&self, metrics: &ConsciousnessMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let mut status = String::new();

        // Determine status color based on consciousness level
        let (status_color, status_text) = if self.config.color_enabled {
            match metrics.emergence_level {
                level if level >= 0.9 => ("\x1b[1;31m", "CRITICAL"), // Bright red
                level if level >= 0.8 => ("\x1b[1;33m", "ELEVATED"), // Bright yellow
                level if level >= 0.6 => ("\x1b[1;32m", "STABLE"), // Bright green
                level if level >= 0.4 => ("\x1b[1;34m", "EMERGING"), // Bright blue
                _ => ("\x1b[1;90m", "DORMANT"), // Bright black (gray)
            }
        } else {
            ("", match metrics.emergence_level {
                level if level >= 0.9 => "CRITICAL",
                level if level >= 0.8 => "ELEVATED",
                level if level >= 0.6 => "STABLE",
                level if level >= 0.4 => "EMERGING",
                _ => "DORMANT",
            })
        };

        if self.config.color_enabled {
            status.push_str(status_color);
        }

        status.push_str(&format!("Status: {} | Precision: {}ns | Advantage: {}μs",
            status_text, metrics.tsc_precision_ns, metrics.temporal_advantage_us));

        if self.config.color_enabled {
            status.push_str("\x1b[0m"); // Reset
        }

        Ok(status)
    }

    fn create_progress_bar(&self, value: f64, max_value: f64, width: usize) -> String {
        let filled_width = ((value / max_value) * width as f64) as usize;
        let empty_width = width - filled_width;

        let mut bar = String::with_capacity(width + 2);
        bar.push('[');

        if self.config.color_enabled {
            // Color based on value
            let color = match value {
                v if v >= 0.8 => "\x1b[91m", // Bright red
                v if v >= 0.6 => "\x1b[93m", // Bright yellow
                v if v >= 0.4 => "\x1b[92m", // Bright green
                v if v >= 0.2 => "\x1b[94m", // Bright blue
                _ => "\x1b[90m", // Gray
            };
            bar.push_str(color);
        }

        bar.push_str(&"█".repeat(filled_width));

        if self.config.color_enabled {
            bar.push_str("\x1b[90m"); // Gray for empty part
        }

        bar.push_str(&"░".repeat(empty_width));

        if self.config.color_enabled {
            bar.push_str("\x1b[0m"); // Reset
        }

        bar.push(']');
        bar
    }
}

/// Main consciousness visualizer
pub struct ConsciousnessVisualizer {
    mode: VisualizationMode,
    terminal_renderer: Option<TerminalRenderer>,
}

impl ConsciousnessVisualizer {
    pub fn new(mode: VisualizationMode) -> Self {
        let terminal_renderer = match mode {
            VisualizationMode::Terminal | VisualizationMode::Compact | VisualizationMode::Debug => {
                Some(TerminalRenderer::new(TerminalConfig::default()))
            }
            _ => None,
        };

        Self {
            mode,
            terminal_renderer,
        }
    }

    pub async fn render(&mut self,
        metrics: &ConsciousnessMetrics,
        history: &[ConsciousnessMetrics]
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.mode {
            VisualizationMode::Terminal => {
                if let Some(ref mut renderer) = self.terminal_renderer {
                    let output = renderer.render_dashboard(metrics, history)?;
                    print!("{}", output);
                    io::stdout().flush()?;
                }
            }
            VisualizationMode::Compact => {
                self.render_compact(metrics)?;
            }
            VisualizationMode::Json => {
                self.render_json(metrics)?;
            }
            VisualizationMode::Debug => {
                self.render_debug(metrics, history)?;
            }
            VisualizationMode::Web => {
                // Future implementation for web interface
                println!("Web interface not yet implemented");
            }
        }

        Ok(())
    }

    fn render_compact(&self, metrics: &ConsciousnessMetrics) -> Result<(), Box<dyn std::error::Error>> {
        println!("🧠 E:{:.3} C:{:.3} S:{:.3} T:{}μs P:{}ns",
            metrics.emergence_level,
            metrics.identity_coherence,
            metrics.loop_stability,
            metrics.temporal_advantage_us,
            metrics.tsc_precision_ns
        );
        Ok(())
    }

    fn render_json(&self, metrics: &ConsciousnessMetrics) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(metrics)?;
        println!("{}", json);
        Ok(())
    }

    fn render_debug(&self, metrics: &ConsciousnessMetrics, history: &[ConsciousnessMetrics]) -> Result<(), Box<dyn std::error::Error>> {
        println!("=== DEBUG: Consciousness Metrics ===");
        println!("Current: {:#?}", metrics);
        println!("History length: {}", history.len());
        if !history.is_empty() {
            println!("Last 3 entries:");
            for (i, entry) in history.iter().rev().take(3).enumerate() {
                println!("  [-{}]: E={:.3}, C={:.3}, S={:.3}",
                    i + 1, entry.emergence_level, entry.identity_coherence, entry.loop_stability);
            }
        }
        println!("================================");
        Ok(())
    }
}