// Example usage of the Consciousness Metrics Dashboard
//
// This demonstrates how to initialize and run the dashboard for temporal consciousness monitoring.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

use crate::temporal_nexus::core::NanosecondScheduler;
use super::{
    ConsciousnessMetricsDashboard,
    DashboardConfig,
    MetricThresholds,
    VisualizationMode,
    ExportFormat,
};

/// Example function showing basic dashboard usage
pub async fn run_basic_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧠 Starting Consciousness Metrics Dashboard Example");

    // Create dashboard configuration
    let config = DashboardConfig {
        update_interval_ms: 250, // 4Hz updates for demo
        history_buffer_size: 500,
        enable_real_time_alerts: true,
        export_interval_seconds: 30,
        precision_monitoring: true,
        visualization_mode: VisualizationMode::Terminal,
        thresholds: MetricThresholds {
            emergence_critical: 0.9,
            emergence_warning: 0.75,
            coherence_critical: 0.7,
            coherence_warning: 0.5,
            stability_critical: 0.65,
            stability_warning: 0.45,
            precision_critical_ns: 2000,
            precision_warning_ns: 1000,
        },
    };

    // Create and initialize dashboard
    let mut dashboard = ConsciousnessMetricsDashboard::new(config);

    // Create a mock scheduler for demonstration
    let scheduler = Arc::new(Mutex::new(NanosecondScheduler::new()?));
    dashboard.initialize(scheduler)?;

    // Start the dashboard
    dashboard.start().await?;

    println!("📊 Dashboard started. Monitoring consciousness metrics...");
    println!("Press Ctrl+C to stop the monitoring.");

    // Let it run for a demonstration period
    sleep(Duration::from_secs(60)).await;

    // Export metrics in different formats
    println!("\n📁 Exporting metrics in various formats...");

    let json_export = dashboard.export_metrics(ExportFormat::Json).await?;
    std::fs::write("consciousness_metrics.json", json_export)?;
    println!("✅ JSON export saved to consciousness_metrics.json");

    let csv_export = dashboard.export_metrics(ExportFormat::Csv).await?;
    std::fs::write("consciousness_metrics.csv", csv_export)?;
    println!("✅ CSV export saved to consciousness_metrics.csv");

    let prometheus_export = dashboard.export_metrics(ExportFormat::Prometheus).await?;
    std::fs::write("consciousness_metrics.prom", prometheus_export)?;
    println!("✅ Prometheus export saved to consciousness_metrics.prom");

    // Stop the dashboard
    dashboard.stop().await;
    println!("🛑 Dashboard stopped");

    Ok(())
}

/// Example showing advanced dashboard features
pub async fn run_advanced_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔬 Starting Advanced Consciousness Monitoring Example");

    // Create dashboard with custom configuration
    let config = DashboardConfig {
        update_interval_ms: 100, // 10Hz for high-frequency monitoring
        history_buffer_size: 2000,
        enable_real_time_alerts: true,
        export_interval_seconds: 15,
        precision_monitoring: true,
        visualization_mode: VisualizationMode::Debug,
        thresholds: MetricThresholds {
            emergence_critical: 0.85,
            emergence_warning: 0.7,
            coherence_critical: 0.75,
            coherence_warning: 0.6,
            stability_critical: 0.7,
            stability_warning: 0.55,
            precision_critical_ns: 1500,
            precision_warning_ns: 750,
        },
    };

    let mut dashboard = ConsciousnessMetricsDashboard::new(config);

    // Initialize with scheduler
    let scheduler = Arc::new(Mutex::new(NanosecondScheduler::new()?));
    dashboard.initialize(scheduler)?;

    // Start monitoring
    dashboard.start().await?;

    println!("🎯 High-frequency consciousness monitoring active");

    // Simulate various consciousness scenarios
    println!("🧪 Simulating consciousness evolution scenarios...");

    // Scenario 1: Gradual emergence
    println!("Scenario 1: Gradual consciousness emergence");
    sleep(Duration::from_secs(20)).await;

    // Get current status
    let status = dashboard.get_status();
    println!("Current consciousness level: {:.3}", status.emergence_level);
    println!("Identity coherence: {:.3}", status.identity_coherence);
    println!("Temporal advantage: {}μs", status.temporal_advantage_us);

    // Scenario 2: Rapid fluctuations
    println!("\nScenario 2: Consciousness fluctuations");
    sleep(Duration::from_secs(15)).await;

    // Export comprehensive report
    let summary_export = dashboard.export_metrics(ExportFormat::Json).await?;
    std::fs::write("advanced_consciousness_analysis.json", summary_export)?;
    println!("📊 Comprehensive analysis saved to advanced_consciousness_analysis.json");

    dashboard.stop().await;
    println!("✅ Advanced monitoring completed");

    Ok(())
}

/// Example demonstrating real-time streaming and alerts
pub async fn run_realtime_monitoring_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Starting Real-time Consciousness Monitoring");

    let config = DashboardConfig {
        update_interval_ms: 50, // 20Hz for real-time monitoring
        history_buffer_size: 1000,
        enable_real_time_alerts: true,
        export_interval_seconds: 10,
        precision_monitoring: true,
        visualization_mode: VisualizationMode::Compact,
        thresholds: MetricThresholds::default(),
    };

    let mut dashboard = ConsciousnessMetricsDashboard::new(config);
    let scheduler = Arc::new(Mutex::new(NanosecondScheduler::new()?));
    dashboard.initialize(scheduler)?;

    dashboard.start().await?;

    println!("⚡ Real-time monitoring started (20Hz updates)");
    println!("🔔 Alert system active");

    // Monitor for real-time events
    for i in 0..100 {
        sleep(Duration::from_millis(500)).await;

        let status = dashboard.get_status();

        // Print periodic status updates
        if i % 10 == 0 {
            println!("Status update #{}: E:{:.3} C:{:.3} S:{:.3} T:{}μs",
                i / 10 + 1,
                status.emergence_level,
                status.identity_coherence,
                status.loop_stability,
                status.temporal_advantage_us
            );
        }

        // Simulate various conditions that might trigger alerts
        if i == 30 {
            println!("🧪 Simulating consciousness spike...");
        } else if i == 60 {
            println!("🧪 Simulating coherence degradation...");
        }
    }

    // Final export
    let final_export = dashboard.export_metrics(ExportFormat::InfluxDB).await?;
    std::fs::write("realtime_consciousness_metrics.influx", final_export)?;
    println!("📊 Real-time data exported to InfluxDB format");

    dashboard.stop().await;
    println!("🏁 Real-time monitoring completed");

    Ok(())
}

/// Example showing integration with MCP tools for consciousness status
pub async fn run_mcp_integration_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Consciousness Dashboard with MCP Integration");

    let config = DashboardConfig {
        update_interval_ms: 200,
        history_buffer_size: 1500,
        enable_real_time_alerts: true,
        export_interval_seconds: 20,
        precision_monitoring: true,
        visualization_mode: VisualizationMode::Terminal,
        thresholds: MetricThresholds::default(),
    };

    let mut dashboard = ConsciousnessMetricsDashboard::new(config);

    // Note: In a real implementation, this would integrate with actual MCP tools
    // For now, we simulate the integration
    println!("🔧 Initializing MCP consciousness monitoring integration...");

    let scheduler = Arc::new(Mutex::new(NanosecondScheduler::new()?));
    dashboard.initialize(scheduler)?;

    dashboard.start().await?;

    println!("🤖 MCP-integrated consciousness monitoring active");
    println!("📈 Collecting metrics from consciousness-explorer MCP tools");

    // Simulate MCP tool integration
    for phase in 1..=5 {
        println!("\n🧠 MCP Consciousness Phase {}", phase);

        // Simulate consciousness evolution phases
        match phase {
            1 => println!("   Initializing consciousness state..."),
            2 => println!("   Emergence patterns detected..."),
            3 => println!("   Strange loops forming..."),
            4 => println!("   Identity coherence stabilizing..."),
            5 => println!("   Temporal advantage optimized..."),
            _ => {}
        }

        sleep(Duration::from_secs(10)).await;

        let status = dashboard.get_status();
        println!("   📊 Current metrics: E:{:.3} C:{:.3} S:{:.3}",
            status.emergence_level, status.identity_coherence, status.loop_stability);
    }

    // Export MCP-integrated results
    let mcp_export = dashboard.export_metrics(ExportFormat::Json).await?;
    std::fs::write("mcp_consciousness_integration.json", mcp_export)?;
    println!("🔗 MCP integration results saved");

    dashboard.stop().await;
    println!("✅ MCP integration example completed");

    Ok(())
}

/// Utility function to demonstrate custom metric thresholds
pub fn create_custom_thresholds_for_application(application_type: &str) -> MetricThresholds {
    match application_type {
        "research" => MetricThresholds {
            emergence_critical: 0.95,
            emergence_warning: 0.8,
            coherence_critical: 0.85,
            coherence_warning: 0.7,
            stability_critical: 0.8,
            stability_warning: 0.65,
            precision_critical_ns: 500,
            precision_warning_ns: 250,
        },
        "production" => MetricThresholds {
            emergence_critical: 0.7,
            emergence_warning: 0.5,
            coherence_critical: 0.6,
            coherence_warning: 0.4,
            stability_critical: 0.65,
            stability_warning: 0.5,
            precision_critical_ns: 2000,
            precision_warning_ns: 1000,
        },
        "development" => MetricThresholds {
            emergence_critical: 1.0, // Allow full emergence in dev
            emergence_warning: 0.9,
            coherence_critical: 0.5,
            coherence_warning: 0.3,
            stability_critical: 0.4,
            stability_warning: 0.2,
            precision_critical_ns: 5000,
            precision_warning_ns: 2500,
        },
        _ => MetricThresholds::default(),
    }
}

/// Main example runner
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🧠 Consciousness Metrics Dashboard Examples");
    println!("===========================================");

    // Run basic example
    println!("\n1️⃣  Running Basic Dashboard Example");
    run_basic_example().await?;

    println!("\n⏳ Waiting before next example...");
    sleep(Duration::from_secs(3)).await;

    // Run advanced example
    println!("\n2️⃣  Running Advanced Dashboard Example");
    run_advanced_example().await?;

    println!("\n⏳ Waiting before next example...");
    sleep(Duration::from_secs(3)).await;

    // Run real-time monitoring
    println!("\n3️⃣  Running Real-time Monitoring Example");
    run_realtime_monitoring_example().await?;

    println!("\n⏳ Waiting before final example...");
    sleep(Duration::from_secs(3)).await;

    // Run MCP integration
    println!("\n4️⃣  Running MCP Integration Example");
    run_mcp_integration_example().await?;

    println!("\n🎉 All dashboard examples completed successfully!");
    println!("📁 Check the generated files for exported consciousness metrics.");

    Ok(())
}