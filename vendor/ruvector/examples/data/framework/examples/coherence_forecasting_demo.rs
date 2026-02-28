use chrono::{Duration, Utc};
use ruvector_data_framework::forecasting::{CoherenceForecaster, CrossDomainForecaster};

fn main() {
    println!("=== RuVector Coherence Forecasting Demo ===\n");

    // Example 1: Simple trend forecasting
    println!("1. Simple Trend Forecasting");
    println!("{}", "-".repeat(50));

    let mut forecaster = CoherenceForecaster::new(0.3, 100);
    let now = Utc::now();

    // Simulate rising coherence trend (e.g., emerging research field)
    println!("Adding observations with rising trend...");
    for i in 0..20 {
        let value = 0.3 + (i as f64) * 0.02;
        forecaster.add_observation(now + Duration::hours(i), value);
    }

    let trend = forecaster.get_trend();
    println!("Detected trend: {:?}", trend);
    println!("Current level: {:.3}", forecaster.get_level().unwrap());
    println!("Current trend value: {:.3}", forecaster.get_trend_value().unwrap());

    // Generate forecasts
    let forecasts = forecaster.forecast(10);
    println!("\nForecasts for next 10 time steps:");
    for (i, forecast) in forecasts.iter().enumerate() {
        println!(
            "  Step {}: {:.3} (CI: {:.3} - {:.3}), Anomaly prob: {:.2}%",
            i + 1,
            forecast.predicted_value,
            forecast.confidence_low,
            forecast.confidence_high,
            forecast.anomaly_probability * 100.0
        );
    }

    // Example 2: Regime change detection
    println!("\n2. Regime Change Detection");
    println!("{}", "-".repeat(50));

    let mut regime_forecaster = CoherenceForecaster::new(0.3, 200);
    let start = Utc::now();

    // Stable period
    println!("Phase 1: Stable coherence around 0.5...");
    for i in 0..30 {
        regime_forecaster.add_observation(start + Duration::hours(i), 0.5);
    }
    println!("Regime change probability: {:.2}%",
             regime_forecaster.detect_regime_change_probability() * 100.0);

    // Sudden shift (e.g., breakthrough discovery)
    println!("\nPhase 2: Sudden shift to 0.85 (breakthrough detected)...");
    for i in 30..40 {
        regime_forecaster.add_observation(start + Duration::hours(i), 0.85);
    }
    println!("Regime change probability: {:.2}%",
             regime_forecaster.detect_regime_change_probability() * 100.0);

    // Example 3: Cross-domain correlation forecasting
    println!("\n3. Cross-Domain Correlation Forecasting");
    println!("{}", "-".repeat(50));

    let mut cross_domain = CrossDomainForecaster::new();

    // Create forecasters for different domains
    let mut climate_forecaster = CoherenceForecaster::new(0.3, 100);
    let mut economics_forecaster = CoherenceForecaster::new(0.3, 100);
    let mut policy_forecaster = CoherenceForecaster::new(0.3, 100);

    // Simulate correlated trends (climate -> economics -> policy)
    println!("Simulating correlated trends across domains...");
    for i in 0..30 {
        let base = 0.4 + (i as f64) * 0.01;

        // Climate science leads
        climate_forecaster.add_observation(
            start + Duration::days(i),
            base + 0.1
        );

        // Economics follows with lag
        if i >= 5 {
            economics_forecaster.add_observation(
                start + Duration::days(i),
                base
            );
        }

        // Policy follows with more lag
        if i >= 10 {
            policy_forecaster.add_observation(
                start + Duration::days(i),
                base - 0.05
            );
        }
    }

    cross_domain.add_domain("climate".to_string(), climate_forecaster);
    cross_domain.add_domain("economics".to_string(), economics_forecaster);
    cross_domain.add_domain("policy".to_string(), policy_forecaster);

    // Calculate correlations
    println!("\nCross-domain correlations:");
    if let Some(corr) = cross_domain.calculate_correlation("climate", "economics") {
        println!("  Climate <-> Economics: {:.3}", corr);
    }
    if let Some(corr) = cross_domain.calculate_correlation("climate", "policy") {
        println!("  Climate <-> Policy: {:.3}", corr);
    }
    if let Some(corr) = cross_domain.calculate_correlation("economics", "policy") {
        println!("  Economics <-> Policy: {:.3}", corr);
    }

    // Forecast all domains
    println!("\nForecasts for all domains (5 steps ahead):");
    let all_forecasts = cross_domain.forecast_all(5);
    for (domain, forecasts) in all_forecasts {
        if let Some(last) = forecasts.last() {
            println!(
                "  {}: {:.3} (trend: {:?})",
                domain,
                last.predicted_value,
                last.trend
            );
        }
    }

    // Detect synchronized regime changes
    println!("\nSynchronized regime changes:");
    let regime_changes = cross_domain.detect_synchronized_regime_changes();
    if regime_changes.is_empty() {
        println!("  None detected");
    } else {
        for (domain, prob) in regime_changes {
            println!("  {}: {:.2}% probability", domain, prob * 100.0);
        }
    }

    // Example 4: Anomaly prediction
    println!("\n4. Anomaly Prediction");
    println!("{}", "-".repeat(50));

    let mut anomaly_forecaster = CoherenceForecaster::new(0.3, 100);

    // Normal behavior
    println!("Establishing baseline with normal fluctuations...");
    for i in 0..50 {
        let noise = (i as f64 * 0.1).sin() * 0.05;
        anomaly_forecaster.add_observation(
            start + Duration::hours(i),
            0.6 + noise
        );
    }

    // Predict next values
    let predictions = anomaly_forecaster.forecast(10);
    println!("\nPredictions with anomaly detection:");
    for (i, pred) in predictions.iter().enumerate() {
        let status = if pred.anomaly_probability > 0.5 {
            "⚠️  ANOMALY"
        } else if pred.anomaly_probability > 0.3 {
            "⚡ WATCH"
        } else {
            "✓ NORMAL"
        };

        println!(
            "  Step {}: {:.3} ({}) - Anomaly: {:.1}%",
            i + 1,
            pred.predicted_value,
            status,
            pred.anomaly_probability * 100.0
        );
    }

    println!("\n=== Demo Complete ===");
}
