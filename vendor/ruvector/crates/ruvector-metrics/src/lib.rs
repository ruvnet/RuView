use lazy_static::lazy_static;
use prometheus::{
    register_counter, register_counter_vec, register_gauge, register_gauge_vec,
    register_histogram_vec, Counter, CounterVec, Encoder, Gauge, GaugeVec, HistogramVec, Opts,
    Registry, TextEncoder,
};

pub mod health;
pub mod recorder;

pub use health::{
    CollectionHealth, HealthChecker, HealthResponse, HealthStatus, ReadinessResponse,
};
pub use recorder::MetricsRecorder;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Search metrics
    pub static ref SEARCH_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        Opts::new("ruvector_search_requests_total", "Total search requests"),
        &["collection", "status"]
    ).unwrap();

    pub static ref SEARCH_LATENCY_SECONDS: HistogramVec = register_histogram_vec!(
        "ruvector_search_latency_seconds",
        "Search latency in seconds",
        &["collection"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();

    // Insert metrics
    pub static ref INSERT_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        Opts::new("ruvector_insert_requests_total", "Total insert requests"),
        &["collection", "status"]
    ).unwrap();

    pub static ref INSERT_LATENCY_SECONDS: HistogramVec = register_histogram_vec!(
        "ruvector_insert_latency_seconds",
        "Insert latency in seconds",
        &["collection"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();

    pub static ref VECTORS_INSERTED_TOTAL: CounterVec = register_counter_vec!(
        Opts::new("ruvector_vectors_inserted_total", "Total vectors inserted"),
        &["collection"]
    ).unwrap();

    // Delete metrics
    pub static ref DELETE_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        Opts::new("ruvector_delete_requests_total", "Total delete requests"),
        &["collection", "status"]
    ).unwrap();

    // Collection metrics
    pub static ref VECTORS_TOTAL: GaugeVec = register_gauge_vec!(
        Opts::new("ruvector_vectors_total", "Total vectors stored"),
        &["collection"]
    ).unwrap();

    pub static ref COLLECTIONS_TOTAL: Gauge = register_gauge!(
        Opts::new("ruvector_collections_total", "Total number of collections")
    ).unwrap();

    // System metrics
    pub static ref MEMORY_USAGE_BYTES: Gauge = register_gauge!(
        Opts::new("ruvector_memory_usage_bytes", "Memory usage in bytes")
    ).unwrap();

    pub static ref UPTIME_SECONDS: Counter = register_counter!(
        Opts::new("ruvector_uptime_seconds", "Uptime in seconds")
    ).unwrap();
}

/// Gather all metrics in Prometheus text format
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gather_metrics() {
        let metrics = gather_metrics();
        assert!(metrics.contains("ruvector"));
    }

    #[test]
    fn test_record_search() {
        SEARCH_REQUESTS_TOTAL
            .with_label_values(&["test", "success"])
            .inc();

        SEARCH_LATENCY_SECONDS
            .with_label_values(&["test"])
            .observe(0.001);
    }
}
