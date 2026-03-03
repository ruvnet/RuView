use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRecord {
    pub timestamp: u64,
    pub name: String,
    pub value: f64,
    pub labels: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy)]
pub enum TimeWindow {
    Minutes(u32),
    Hours(u32),
    Days(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum AggregateFunction {
    Average,
    Sum,
    Count,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Weather,
    Calendar,
    None,
}

#[derive(Debug, Clone)]
pub struct LLMMessage {
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub intent: Option<Intent>,
    pub tool_response: Option<String>,
}

#[async_trait]
pub trait StreamProcessor {
    async fn process_stream(&self) -> Result<Vec<LLMMessage>, Box<dyn std::error::Error>>;
    async fn get_metrics(&self) -> Vec<MetricRecord>;
    async fn get_average_sentiment(&self, window: Duration) -> Result<f64, Box<dyn std::error::Error>>;
}

pub trait LLMClient: Send + Sync {
    fn stream(&self) -> BoxStream<'static, String>;
}

#[async_trait]
pub trait HyprService: Send + Sync {
    async fn ingest_metric(&self, metric: MetricRecord) -> Result<(), Box<dyn std::error::Error>>;
    async fn query_aggregate(&self, window: TimeWindow, func: AggregateFunction) -> Result<f64, Box<dyn std::error::Error>>;
}

pub trait ToolIntegration: Send + Sync {
    fn handle_weather_intent(&self, content: &str) -> Result<String, Box<dyn std::error::Error>>;
    fn handle_calendar_intent(&self, content: &str) -> Result<String, Box<dyn std::error::Error>>;
}

pub struct Midstream {
    llm_client: Box<dyn LLMClient>,
    hypr_service: Box<dyn HyprService>,
    tool_integration: Option<Box<dyn ToolIntegration>>,
    metrics: Arc<Mutex<Vec<MetricRecord>>>,
}

impl Midstream {
    pub fn new(
        llm_client: Box<dyn LLMClient>,
        hypr_service: Box<dyn HyprService>,
    ) -> Self {
        Self {
            llm_client,
            hypr_service,
            tool_integration: None,
            metrics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_tool_integration(
        llm_client: Box<dyn LLMClient>,
        hypr_service: Box<dyn HyprService>,
        tool_integration: Box<dyn ToolIntegration>,
    ) -> Self {
        Self {
            llm_client,
            hypr_service,
            tool_integration: Some(tool_integration),
            metrics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn detect_intent(&self, content: &str) -> Intent {
        let content_lower = content.to_lowercase();
        if content_lower.contains("weather") {
            Intent::Weather
        } else if content_lower.contains("schedule") || content_lower.contains("meeting") {
            Intent::Calendar
        } else {
            Intent::None
        }
    }

    fn is_urgent(&self, content: &str) -> bool {
        content.to_uppercase().starts_with("URGENT")
    }

    async fn process_message(&self, content: String) -> Result<LLMMessage, Box<dyn std::error::Error>> {
        // Validate content
        if content.is_empty() {
            return Err("Empty message content".into());
        }

        let timestamp = chrono::Utc::now();
        let intent = self.detect_intent(&content);
        let mut tool_response = None;

        // Handle urgent requests immediately
        if self.is_urgent(&content) && intent != Intent::None {
            if let Some(tool) = &self.tool_integration {
                tool_response = match intent {
                    Intent::Weather => Some(tool.handle_weather_intent(&content)?),
                    Intent::Calendar => Some(tool.handle_calendar_intent(&content)?),
                    Intent::None => None,
                };
            }
        }

        let message = LLMMessage { 
            content, 
            timestamp,
            intent: Some(intent),
            tool_response,
        };

        // Create and ingest metric
        let metric = MetricRecord {
            timestamp: timestamp.timestamp() as u64,
            name: "llm_stream".to_string(),
            value: message.content.len() as f64,
            labels: vec![
                ("type".to_string(), "message".to_string()),
                ("size".to_string(), message.content.len().to_string()),
                ("intent".to_string(), format!("{:?}", message.intent)),
                ("urgent".to_string(), self.is_urgent(&message.content).to_string()),
            ],
        };

        // Attempt to ingest metric and handle errors
        if let Err(e) = self.hypr_service.ingest_metric(metric.clone()).await {
            return Err(format!("Failed to ingest metric: {}", e).into());
        }

        // Update internal metrics
        let mut metrics = self.metrics.lock().await;
        metrics.push(metric);

        Ok(message)
    }
}

#[async_trait]
impl StreamProcessor for Midstream {
    async fn process_stream(&self) -> Result<Vec<LLMMessage>, Box<dyn std::error::Error>> {
        use futures::StreamExt;
        
        let mut messages = Vec::new();
        let mut stream = self.llm_client.stream();

        while let Some(content) = stream.next().await {
            let message = self.process_message(content).await?;
            messages.push(message);
        }

        Ok(messages)
    }

    async fn get_metrics(&self) -> Vec<MetricRecord> {
        self.metrics.lock().await.clone()
    }

    async fn get_average_sentiment(&self, window: Duration) -> Result<f64, Box<dyn std::error::Error>> {
        let minutes = window.as_secs() / 60;
        self.hypr_service.query_aggregate(
            TimeWindow::Minutes(minutes as u32),
            AggregateFunction::Average,
        ).await
    }
}
