use chrono::{DateTime, Utc, Duration};
use std::collections::VecDeque;

/// Trend direction for coherence values
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trend {
    Rising,
    Falling,
    Stable,
}

/// Forecast result with confidence intervals and anomaly detection
#[derive(Debug, Clone)]
pub struct Forecast {
    pub timestamp: DateTime<Utc>,
    pub predicted_value: f64,
    pub confidence_low: f64,
    pub confidence_high: f64,
    pub trend: Trend,
    pub anomaly_probability: f64,
}

/// Coherence forecaster using exponential smoothing methods
pub struct CoherenceForecaster {
    history: VecDeque<(DateTime<Utc>, f64)>,
    alpha: f64,      // Level smoothing parameter
    beta: f64,       // Trend smoothing parameter
    window: usize,   // Maximum history size
    level: Option<f64>,
    trend: Option<f64>,
    cusum_pos: f64,  // Positive CUSUM for regime change detection
    cusum_neg: f64,  // Negative CUSUM for regime change detection
}

impl CoherenceForecaster {
    /// Create a new forecaster with smoothing parameters
    ///
    /// # Arguments
    /// * `alpha` - Level smoothing parameter (0.0 to 1.0). Higher = more weight on recent values
    /// * `window` - Maximum number of historical observations to keep
    pub fn new(alpha: f64, window: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(window),
            alpha: alpha.clamp(0.0, 1.0),
            beta: 0.1, // Default trend smoothing
            window,
            level: None,
            trend: None,
            cusum_pos: 0.0,
            cusum_neg: 0.0,
        }
    }

    /// Create a forecaster with custom trend smoothing parameter
    pub fn with_beta(mut self, beta: f64) -> Self {
        self.beta = beta.clamp(0.0, 1.0);
        self
    }

    /// Add a new observation to the forecaster
    pub fn add_observation(&mut self, timestamp: DateTime<Utc>, value: f64) {
        // Add to history
        self.history.push_back((timestamp, value));
        if self.history.len() > self.window {
            self.history.pop_front();
        }

        // Update smoothed level and trend (Holt's method)
        match (self.level, self.trend) {
            (None, None) => {
                // Initialize with first observation
                self.level = Some(value);
                self.trend = Some(0.0);
            }
            (Some(prev_level), Some(prev_trend)) => {
                // Update level: L_t = α * Y_t + (1 - α) * (L_{t-1} + T_{t-1})
                let new_level = self.alpha * value + (1.0 - self.alpha) * (prev_level + prev_trend);

                // Update trend: T_t = β * (L_t - L_{t-1}) + (1 - β) * T_{t-1}
                let new_trend = self.beta * (new_level - prev_level) + (1.0 - self.beta) * prev_trend;

                self.level = Some(new_level);
                self.trend = Some(new_trend);

                // Update CUSUM for regime change detection
                self.update_cusum(value, prev_level);
            }
            _ => unreachable!(),
        }
    }

    /// Update CUSUM statistics for regime change detection
    fn update_cusum(&mut self, value: f64, expected: f64) {
        let mean = self.get_mean();
        let std = self.get_std();

        if std > 0.0 {
            let threshold = 0.5 * std;
            let deviation = value - mean;

            // Positive CUSUM (detects upward shifts)
            self.cusum_pos = (self.cusum_pos + deviation - threshold).max(0.0);

            // Negative CUSUM (detects downward shifts)
            self.cusum_neg = (self.cusum_neg - deviation - threshold).max(0.0);
        }
    }

    /// Generate forecasts for future time steps
    ///
    /// # Arguments
    /// * `steps` - Number of future time steps to forecast
    ///
    /// # Returns
    /// Vector of forecast results with confidence intervals
    pub fn forecast(&self, steps: usize) -> Vec<Forecast> {
        if self.history.is_empty() {
            return Vec::new();
        }

        let level = self.level.unwrap_or(0.0);
        let trend = self.trend.unwrap_or(0.0);
        let std_error = self.get_prediction_error_std();

        // Get time delta from last two observations
        let time_delta = if self.history.len() >= 2 {
            let (t1, _) = self.history[self.history.len() - 1];
            let (t0, _) = self.history[self.history.len() - 2];
            t1.signed_duration_since(t0)
        } else {
            Duration::hours(1) // Default 1 hour
        };

        let last_timestamp = self.history.back().unwrap().0;
        let current_trend = self.get_trend();

        let mut forecasts = Vec::with_capacity(steps);

        for h in 1..=steps {
            // Holt's linear trend forecast: F_{t+h} = L_t + h * T_t
            let forecast_value = level + (h as f64) * trend;

            // Prediction interval widens with horizon (sqrt(h))
            let interval_width = 1.96 * std_error * (h as f64).sqrt();

            // Calculate anomaly probability based on deviation and CUSUM
            let anomaly_prob = self.calculate_anomaly_probability(forecast_value);

            forecasts.push(Forecast {
                timestamp: last_timestamp + time_delta * h as i32,
                predicted_value: forecast_value,
                confidence_low: forecast_value - interval_width,
                confidence_high: forecast_value + interval_width,
                trend: current_trend,
                anomaly_probability: anomaly_prob,
            });
        }

        forecasts
    }

    /// Detect probability of regime change using CUSUM statistics
    ///
    /// # Returns
    /// Probability between 0.0 and 1.0 that a regime change is occurring
    pub fn detect_regime_change_probability(&self) -> f64 {
        if self.history.len() < 10 {
            return 0.0; // Not enough data
        }

        let std = self.get_std();
        if std == 0.0 {
            return 0.0;
        }

        // CUSUM threshold (typically 4-5 standard deviations)
        let threshold = 4.0 * std;

        // Combine positive and negative CUSUM
        let max_cusum = self.cusum_pos.max(self.cusum_neg);

        // Convert to probability using sigmoid
        let probability = 1.0 / (1.0 + (-0.5 * (max_cusum - threshold)).exp());

        probability.clamp(0.0, 1.0)
    }

    /// Get current trend direction
    pub fn get_trend(&self) -> Trend {
        let trend_value = self.trend.unwrap_or(0.0);
        let std = self.get_std();

        // Use a fraction of std as threshold for "stable"
        let threshold = 0.1 * std;

        if trend_value > threshold {
            Trend::Rising
        } else if trend_value < -threshold {
            Trend::Falling
        } else {
            Trend::Stable
        }
    }

    /// Calculate mean of historical values
    fn get_mean(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.history.iter().map(|(_, v)| v).sum();
        sum / self.history.len() as f64
    }

    /// Calculate standard deviation of historical values
    fn get_std(&self) -> f64 {
        if self.history.len() < 2 {
            return 0.0;
        }

        let mean = self.get_mean();
        let variance: f64 = self.history
            .iter()
            .map(|(_, v)| (v - mean).powi(2))
            .sum::<f64>() / (self.history.len() - 1) as f64;

        variance.sqrt()
    }

    /// Calculate standard error of predictions
    fn get_prediction_error_std(&self) -> f64 {
        if self.history.len() < 3 {
            return self.get_std();
        }

        // Calculate residuals from one-step-ahead forecasts
        let mut errors = Vec::new();

        for i in 2..self.history.len() {
            let (_, actual) = self.history[i];

            // Simple exponential smoothing forecast using previous data
            let prev_values: Vec<f64> = self.history.iter()
                .take(i)
                .map(|(_, v)| *v)
                .collect();

            if let Some(predicted) = self.simple_forecast(&prev_values, 1) {
                errors.push(actual - predicted);
            }
        }

        if errors.is_empty() {
            return self.get_std();
        }

        // Root mean squared error
        let mse: f64 = errors.iter().map(|e| e.powi(2)).sum::<f64>() / errors.len() as f64;
        mse.sqrt()
    }

    /// Simple exponential smoothing forecast (for error calculation)
    fn simple_forecast(&self, values: &[f64], steps: usize) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        let mut level = values[0];
        for &value in &values[1..] {
            level = self.alpha * value + (1.0 - self.alpha) * level;
        }

        // For SES, forecast is constant
        Some(level)
    }

    /// Calculate anomaly probability for a forecasted value
    fn calculate_anomaly_probability(&self, forecast_value: f64) -> f64 {
        let mean = self.get_mean();
        let std = self.get_std();

        if std == 0.0 {
            return 0.0;
        }

        // Z-score of the forecast
        let z_score = ((forecast_value - mean) / std).abs();

        // Combine with regime change probability
        let regime_prob = self.detect_regime_change_probability();

        // Anomaly if z-score > 2 (95% confidence) or regime change detected
        let z_anomaly_prob = if z_score > 2.0 {
            1.0 / (1.0 + (-(z_score - 2.0)).exp())
        } else {
            0.0
        };

        // Combine probabilities (max gives more sensitivity)
        z_anomaly_prob.max(regime_prob)
    }

    /// Get the number of observations in history
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if forecaster has no observations
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Get the smoothed level value
    pub fn get_level(&self) -> Option<f64> {
        self.level
    }

    /// Get the smoothed trend value
    pub fn get_trend_value(&self) -> Option<f64> {
        self.trend
    }
}

/// Cross-domain correlation forecaster
pub struct CrossDomainForecaster {
    forecasters: Vec<(String, CoherenceForecaster)>,
}

impl CrossDomainForecaster {
    /// Create a new cross-domain forecaster
    pub fn new() -> Self {
        Self {
            forecasters: Vec::new(),
        }
    }

    /// Add a domain with its own forecaster
    pub fn add_domain(&mut self, domain: String, forecaster: CoherenceForecaster) {
        self.forecasters.push((domain, forecaster));
    }

    /// Calculate correlation between domains
    pub fn calculate_correlation(&self, domain1: &str, domain2: &str) -> Option<f64> {
        let (_, f1) = self.forecasters.iter().find(|(d, _)| d == domain1)?;
        let (_, f2) = self.forecasters.iter().find(|(d, _)| d == domain2)?;

        if f1.is_empty() || f2.is_empty() {
            return None;
        }

        // Calculate Pearson correlation coefficient
        let min_len = f1.history.len().min(f2.history.len());
        if min_len < 2 {
            return None;
        }

        let values1: Vec<f64> = f1.history.iter().rev().take(min_len).map(|(_, v)| *v).collect();
        let values2: Vec<f64> = f2.history.iter().rev().take(min_len).map(|(_, v)| *v).collect();

        let mean1 = values1.iter().sum::<f64>() / min_len as f64;
        let mean2 = values2.iter().sum::<f64>() / min_len as f64;

        let mut numerator = 0.0;
        let mut sum_sq1 = 0.0;
        let mut sum_sq2 = 0.0;

        for i in 0..min_len {
            let diff1 = values1[i] - mean1;
            let diff2 = values2[i] - mean2;
            numerator += diff1 * diff2;
            sum_sq1 += diff1 * diff1;
            sum_sq2 += diff2 * diff2;
        }

        let denominator = (sum_sq1 * sum_sq2).sqrt();
        if denominator == 0.0 {
            return None;
        }

        Some(numerator / denominator)
    }

    /// Forecast all domains and return combined results
    pub fn forecast_all(&self, steps: usize) -> Vec<(String, Vec<Forecast>)> {
        self.forecasters
            .iter()
            .map(|(domain, forecaster)| {
                (domain.clone(), forecaster.forecast(steps))
            })
            .collect()
    }

    /// Detect synchronized regime changes across domains
    pub fn detect_synchronized_regime_changes(&self) -> Vec<(String, f64)> {
        self.forecasters
            .iter()
            .map(|(domain, forecaster)| {
                (domain.clone(), forecaster.detect_regime_change_probability())
            })
            .filter(|(_, prob)| *prob > 0.5)
            .collect()
    }
}

impl Default for CrossDomainForecaster {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forecaster_creation() {
        let forecaster = CoherenceForecaster::new(0.3, 100);
        assert!(forecaster.is_empty());
        assert_eq!(forecaster.len(), 0);
    }

    #[test]
    fn test_add_observation() {
        let mut forecaster = CoherenceForecaster::new(0.3, 100);
        let now = Utc::now();

        forecaster.add_observation(now, 0.5);
        assert_eq!(forecaster.len(), 1);
        assert!(forecaster.get_level().is_some());
    }

    #[test]
    fn test_trend_detection() {
        let mut forecaster = CoherenceForecaster::new(0.3, 100);
        let now = Utc::now();

        // Add rising values
        for i in 0..10 {
            forecaster.add_observation(
                now + Duration::hours(i),
                0.5 + (i as f64) * 0.1
            );
        }

        let trend = forecaster.get_trend();
        assert_eq!(trend, Trend::Rising);
    }

    #[test]
    fn test_forecast_generation() {
        let mut forecaster = CoherenceForecaster::new(0.3, 100);
        let now = Utc::now();

        // Add some observations
        for i in 0..10 {
            forecaster.add_observation(
                now + Duration::hours(i),
                0.5 + (i as f64) * 0.05
            );
        }

        let forecasts = forecaster.forecast(5);
        assert_eq!(forecasts.len(), 5);

        // Check that forecasts are in the future
        for forecast in forecasts {
            assert!(forecast.timestamp > now + Duration::hours(9));
            assert!(forecast.confidence_low < forecast.predicted_value);
            assert!(forecast.confidence_high > forecast.predicted_value);
        }
    }

    #[test]
    fn test_regime_change_detection() {
        let mut forecaster = CoherenceForecaster::new(0.3, 100);
        let now = Utc::now();

        // Add stable values
        for i in 0..20 {
            forecaster.add_observation(now + Duration::hours(i), 0.5);
        }

        // Should have low regime change probability
        let prob1 = forecaster.detect_regime_change_probability();
        assert!(prob1 < 0.3);

        // Add sudden shift
        for i in 20..25 {
            forecaster.add_observation(now + Duration::hours(i), 0.9);
        }

        // Should detect regime change
        let prob2 = forecaster.detect_regime_change_probability();
        assert!(prob2 > prob1);
    }

    #[test]
    fn test_cross_domain_correlation() {
        let mut cross = CrossDomainForecaster::new();

        let mut f1 = CoherenceForecaster::new(0.3, 100);
        let mut f2 = CoherenceForecaster::new(0.3, 100);
        let now = Utc::now();

        // Add correlated data
        for i in 0..20 {
            let value = 0.5 + (i as f64) * 0.01;
            f1.add_observation(now + Duration::hours(i), value);
            f2.add_observation(now + Duration::hours(i), value + 0.1);
        }

        cross.add_domain("domain1".to_string(), f1);
        cross.add_domain("domain2".to_string(), f2);

        let correlation = cross.calculate_correlation("domain1", "domain2");
        assert!(correlation.is_some());

        // Should be highly correlated
        let corr_value = correlation.unwrap();
        assert!(corr_value > 0.9, "Correlation was {}", corr_value);
    }

    #[test]
    fn test_window_size_limit() {
        let mut forecaster = CoherenceForecaster::new(0.3, 10);
        let now = Utc::now();

        // Add more observations than window size
        for i in 0..20 {
            forecaster.add_observation(now + Duration::hours(i), 0.5);
        }

        // Should only keep last 10
        assert_eq!(forecaster.len(), 10);
    }
}
