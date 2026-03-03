use crate::config::HyprSettings;
use crate::midstream::{HyprService, MetricRecord, TimeWindow, AggregateFunction};
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;

type BoxError = Box<dyn std::error::Error>;

pub struct HyprServiceImpl {
    metrics: Arc<Mutex<Vec<MetricRecord>>>,
}

impl HyprServiceImpl {
    pub async fn new(_settings: &HyprSettings) -> Result<Self, BoxError> {
        Ok(Self {
            metrics: Arc::new(Mutex::new(Vec::new())),
        })
    }

    async fn calculate_aggregate(&self, window: TimeWindow, func: AggregateFunction) -> Result<f64, BoxError> {
        let metrics = self.metrics.lock().await;
        let now = chrono::Utc::now().timestamp() as u64;
        let window_secs = match window {
            TimeWindow::Minutes(m) => m as u64 * 60,
            TimeWindow::Hours(h) => h as u64 * 3600,
            TimeWindow::Days(d) => d as u64 * 86400,
        };

        let filtered: Vec<_> = metrics
            .iter()
            .filter(|m| now - m.timestamp <= window_secs)
            .collect();

        match func {
            AggregateFunction::Average => {
                if filtered.is_empty() {
                    Ok(0.0)
                } else {
                    let sum: f64 = filtered.iter().map(|m| m.value).sum();
                    Ok(sum / filtered.len() as f64)
                }
            }
            AggregateFunction::Sum => {
                Ok(filtered.iter().map(|m| m.value).sum())
            }
            AggregateFunction::Count => {
                Ok(filtered.len() as f64)
            }
        }
    }
}

#[async_trait]
impl HyprService for HyprServiceImpl {
    async fn ingest_metric(&self, metric: MetricRecord) -> Result<(), BoxError> {
        let mut metrics = self.metrics.lock().await;
        metrics.push(metric);
        Ok(())
    }

    async fn query_aggregate(&self, window: TimeWindow, func: AggregateFunction) -> Result<f64, BoxError> {
        self.calculate_aggregate(window, func).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_hypr_service_creation() {
        let settings = HyprSettings::default();
        let service = HyprServiceImpl::new(&settings).await;
        assert!(service.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_metric_ingestion() {
        let settings = HyprSettings::default();
        let service = HyprServiceImpl::new(&settings).await.unwrap();

        let metric = MetricRecord {
            timestamp: chrono::Utc::now().timestamp() as u64,
            name: "test_metric".to_string(),
            value: 1.0,
            labels: vec![("test".to_string(), "true".to_string())],
        };

        let result = service.ingest_metric(metric).await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_aggregation_query() {
        let settings = HyprSettings::default();
        let service = HyprServiceImpl::new(&settings).await.unwrap();

        // First ingest some metrics
        let metric = MetricRecord {
            timestamp: chrono::Utc::now().timestamp() as u64,
            name: "test_metric".to_string(),
            value: 1.0,
            labels: vec![("test".to_string(), "true".to_string())],
        };
        service.ingest_metric(metric).await.unwrap();

        // Now query the aggregate
        let result = service.query_aggregate(
            TimeWindow::Minutes(5),
            AggregateFunction::Average,
        ).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1.0);
    }
}
