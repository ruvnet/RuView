//! Structural Monitor: Brittleness Detection
//!
//! Monitors the fusion graph's structural health by tracking minimum-cut
//! trends, volatility, and generating actionable triggers.

use std::collections::VecDeque;

/// Configuration for the structural monitor
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Window size for trend analysis
    pub window_size: usize,
    /// Threshold for low-cut warning (λ_low)
    pub lambda_low: f64,
    /// Threshold for critical warning (λ_critical)
    pub lambda_critical: f64,
    /// Volatility threshold for instability warning
    pub volatility_threshold: f64,
    /// Trend slope threshold for degradation warning
    pub trend_slope_threshold: f64,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            window_size: 100,
            lambda_low: 3.0,
            lambda_critical: 1.0,
            volatility_threshold: 0.5,
            trend_slope_threshold: -0.1,
        }
    }
}

/// Current state of the monitor
#[derive(Debug, Clone)]
pub struct MonitorState {
    /// Current minimum-cut estimate
    pub lambda_est: f64,
    /// Moving average slope (trend)
    pub lambda_trend: f64,
    /// Variance over window (volatility)
    pub cut_volatility: f64,
    /// Top-k edges crossing current cut
    pub boundary_edges: Vec<(u64, u64)>,
    /// Number of observations in window
    pub observation_count: usize,
    /// Last update timestamp
    pub last_update_ts: u64,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            lambda_est: f64::INFINITY,
            lambda_trend: 0.0,
            cut_volatility: 0.0,
            boundary_edges: Vec::new(),
            observation_count: 0,
            last_update_ts: 0,
        }
    }
}

/// Type of trigger condition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// Min-cut below critical threshold (islanding risk)
    IslandingRisk,
    /// Volatility above threshold (unstable structure)
    Instability,
    /// Negative trend (degrading connectivity)
    Degradation,
    /// Cut increased significantly (over-clustering)
    OverClustering,
    /// Graph became disconnected
    Disconnected,
}

/// A trigger event from the monitor
#[derive(Debug, Clone)]
pub struct Trigger {
    /// Type of trigger
    pub trigger_type: TriggerType,
    /// Current minimum-cut value
    pub lambda_current: f64,
    /// Threshold that was crossed
    pub threshold: f64,
    /// Severity (0.0 - 1.0)
    pub severity: f64,
    /// Recommended action
    pub recommendation: String,
    /// Timestamp
    pub timestamp: u64,
}

impl Trigger {
    /// Create a new trigger
    pub fn new(trigger_type: TriggerType, lambda: f64, threshold: f64) -> Self {
        let severity = match trigger_type {
            TriggerType::Disconnected => 1.0,
            TriggerType::IslandingRisk => 0.8,
            TriggerType::Instability => 0.6,
            TriggerType::Degradation => 0.5,
            TriggerType::OverClustering => 0.3,
        };

        let recommendation = match trigger_type {
            TriggerType::IslandingRisk => {
                "Consider adding bridge edges or merging sparse partitions".to_string()
            }
            TriggerType::Instability => {
                "Structure is volatile; consider stabilizing with explicit relations".to_string()
            }
            TriggerType::Degradation => {
                "Connectivity trending down; review recent deletions".to_string()
            }
            TriggerType::OverClustering => {
                "May have too many clusters; consider relaxing similarity threshold".to_string()
            }
            TriggerType::Disconnected => "Critical: graph has disconnected components".to_string(),
        };

        Self {
            trigger_type,
            lambda_current: lambda,
            threshold,
            severity,
            recommendation,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Signal from the monitor indicating graph health
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrittlenessSignal {
    /// Healthy: good connectivity
    Healthy,
    /// Warning: connectivity getting low
    Warning,
    /// Critical: at risk of fragmentation
    Critical,
    /// Disconnected: already fragmented
    Disconnected,
}

impl BrittlenessSignal {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            BrittlenessSignal::Healthy => "healthy",
            BrittlenessSignal::Warning => "warning",
            BrittlenessSignal::Critical => "critical",
            BrittlenessSignal::Disconnected => "disconnected",
        }
    }
}

/// The structural monitor
#[derive(Debug)]
pub struct StructuralMonitor {
    /// Configuration
    config: MonitorConfig,
    /// Current state
    state: MonitorState,
    /// History of lambda values for trend analysis
    lambda_history: VecDeque<f64>,
    /// Active triggers
    active_triggers: Vec<Trigger>,
    /// Total observations processed
    total_observations: u64,
}

impl StructuralMonitor {
    /// Create a new monitor with default config
    pub fn new() -> Self {
        Self::with_config(MonitorConfig::default())
    }

    /// Create a monitor with custom config
    pub fn with_config(config: MonitorConfig) -> Self {
        Self {
            config,
            state: MonitorState::default(),
            lambda_history: VecDeque::new(),
            active_triggers: Vec::new(),
            total_observations: 0,
        }
    }

    /// Get current state
    pub fn state(&self) -> &MonitorState {
        &self.state
    }

    /// Get current brittleness signal
    pub fn signal(&self) -> BrittlenessSignal {
        if self.state.lambda_est == 0.0 {
            BrittlenessSignal::Disconnected
        } else if self.state.lambda_est < self.config.lambda_critical {
            BrittlenessSignal::Critical
        } else if self.state.lambda_est < self.config.lambda_low {
            BrittlenessSignal::Warning
        } else {
            BrittlenessSignal::Healthy
        }
    }

    /// Get active triggers
    pub fn triggers(&self) -> &[Trigger] {
        &self.active_triggers
    }

    /// Update the monitor with a new minimum-cut observation
    pub fn observe(&mut self, lambda: f64, boundary_edges: Vec<(u64, u64)>) -> Vec<Trigger> {
        let mut new_triggers = Vec::new();

        // Update history
        self.lambda_history.push_back(lambda);
        if self.lambda_history.len() > self.config.window_size {
            self.lambda_history.pop_front();
        }

        // Update state
        self.state.lambda_est = lambda;
        self.state.boundary_edges = boundary_edges;
        self.state.observation_count = self.lambda_history.len();
        self.state.last_update_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.total_observations += 1;

        // Compute trend (linear regression slope)
        self.state.lambda_trend = self.compute_trend();

        // Compute volatility (variance)
        self.state.cut_volatility = self.compute_volatility();

        // Check triggers
        if lambda == 0.0 || lambda.is_infinite() && lambda.is_sign_negative() {
            new_triggers.push(Trigger::new(TriggerType::Disconnected, lambda, 0.0));
        } else if lambda < self.config.lambda_critical {
            new_triggers.push(Trigger::new(
                TriggerType::IslandingRisk,
                lambda,
                self.config.lambda_critical,
            ));
        }

        if self.state.cut_volatility > self.config.volatility_threshold {
            new_triggers.push(Trigger::new(
                TriggerType::Instability,
                lambda,
                self.config.volatility_threshold,
            ));
        }

        if self.state.lambda_trend < self.config.trend_slope_threshold {
            new_triggers.push(Trigger::new(
                TriggerType::Degradation,
                lambda,
                self.config.trend_slope_threshold,
            ));
        }

        // Update active triggers
        self.active_triggers = new_triggers.clone();

        new_triggers
    }

    /// Check if immediate action is needed
    pub fn needs_action(&self) -> bool {
        !self.active_triggers.is_empty()
    }

    /// Get summary report
    pub fn report(&self) -> String {
        let signal = self.signal();
        let trend_dir = if self.state.lambda_trend > 0.01 {
            "↑ improving"
        } else if self.state.lambda_trend < -0.01 {
            "↓ degrading"
        } else {
            "→ stable"
        };

        format!(
            "Signal: {} | λ={:.2} | Trend: {} ({:.3}) | Volatility: {:.3} | Boundary: {} edges",
            signal.as_str(),
            self.state.lambda_est,
            trend_dir,
            self.state.lambda_trend,
            self.state.cut_volatility,
            self.state.boundary_edges.len()
        )
    }

    /// Reset the monitor
    pub fn reset(&mut self) {
        self.state = MonitorState::default();
        self.lambda_history.clear();
        self.active_triggers.clear();
    }

    /// Compute linear regression slope for trend
    fn compute_trend(&self) -> f64 {
        let n = self.lambda_history.len();
        if n < 2 {
            return 0.0;
        }

        let n_f64 = n as f64;
        let sum_x: f64 = (0..n).map(|i| i as f64).sum();
        let sum_y: f64 = self.lambda_history.iter().sum();
        let sum_xy: f64 = self
            .lambda_history
            .iter()
            .enumerate()
            .map(|(i, &y)| i as f64 * y)
            .sum();
        let sum_xx: f64 = (0..n).map(|i| (i * i) as f64).sum();

        let denominator = n_f64 * sum_xx - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        (n_f64 * sum_xy - sum_x * sum_y) / denominator
    }

    /// Compute variance for volatility
    fn compute_volatility(&self) -> f64 {
        let n = self.lambda_history.len();
        if n < 2 {
            return 0.0;
        }

        let mean: f64 = self.lambda_history.iter().sum::<f64>() / n as f64;
        let variance: f64 = self
            .lambda_history
            .iter()
            .map(|&x| (x - mean) * (x - mean))
            .sum::<f64>()
            / (n - 1) as f64;

        variance.sqrt()
    }
}

impl Default for StructuralMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = StructuralMonitor::new();
        // Initial state has lambda_est = INFINITY (no observations yet)
        // which counts as Healthy since it's above all thresholds
        assert_eq!(monitor.signal(), BrittlenessSignal::Healthy);
    }

    #[test]
    fn test_healthy_observation() {
        let mut monitor = StructuralMonitor::new();
        monitor.observe(5.0, vec![]);
        assert_eq!(monitor.signal(), BrittlenessSignal::Healthy);
    }

    #[test]
    fn test_warning_observation() {
        let mut monitor = StructuralMonitor::new();
        monitor.observe(2.0, vec![]);
        assert_eq!(monitor.signal(), BrittlenessSignal::Warning);
    }

    #[test]
    fn test_critical_observation() {
        let mut monitor = StructuralMonitor::new();
        monitor.observe(0.5, vec![]);
        assert_eq!(monitor.signal(), BrittlenessSignal::Critical);
    }

    #[test]
    fn test_trigger_generation() {
        let mut monitor = StructuralMonitor::new();
        let triggers = monitor.observe(0.5, vec![(1, 2)]);

        assert!(!triggers.is_empty());
        assert!(triggers
            .iter()
            .any(|t| t.trigger_type == TriggerType::IslandingRisk));
    }

    #[test]
    fn test_trend_computation() {
        let mut monitor = StructuralMonitor::new();

        // Simulate decreasing trend
        for i in (0..10).rev() {
            monitor.observe(i as f64, vec![]);
        }

        assert!(monitor.state().lambda_trend < 0.0);
    }

    #[test]
    fn test_volatility_computation() {
        let mut monitor = StructuralMonitor::new();

        // Simulate volatile observations
        for i in 0..10 {
            let value = if i % 2 == 0 { 5.0 } else { 1.0 };
            monitor.observe(value, vec![]);
        }

        assert!(monitor.state().cut_volatility > 0.0);
    }

    #[test]
    fn test_report() {
        let mut monitor = StructuralMonitor::new();
        monitor.observe(3.5, vec![(1, 2), (2, 3)]);

        let report = monitor.report();
        assert!(report.contains("healthy"));
        assert!(report.contains("3.50"));
    }
}
