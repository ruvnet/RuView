#[cfg(test)]
mod tests {
    use crate::midstream::{Midstream, StreamProcessor, Intent, LLMClient, HyprService, ToolIntegration, MetricRecord, TimeWindow, AggregateFunction};
    use std::time::Duration;
    use mockall::*;
    use futures::stream::{self, BoxStream};

    type BoxError = Box<dyn std::error::Error>;

    mock! {
        pub LLMClient {}
        impl LLMClient for LLMClient {
            fn stream(&self) -> BoxStream<'static, String>;
        }
    }

    mock! {
        pub HyprService {}
        impl HyprService for HyprService {
            fn ingest_metric(&self, metric: MetricRecord) -> Result<(), BoxError>;
            fn query_aggregate(&self, window: TimeWindow, func: AggregateFunction) -> Result<f64, BoxError>;
        }
    }

    mock! {
        pub ToolClient {}
        impl ToolIntegration for ToolClient {
            fn handle_weather_intent(&self, content: &str) -> Result<String, BoxError>;
            fn handle_calendar_intent(&self, content: &str) -> Result<String, BoxError>;
        }
    }

    #[tokio::test]
    async fn test_stream_processing_with_metrics() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();
        
        mock_llm.expect_stream()
            .times(1)
            .return_once(move || {
                Box::pin(stream::iter(vec![
                    "Process".to_string(),
                    "this".to_string(),
                    "stream".to_string(),
                ]))
            });

        mock_hypr.expect_ingest_metric()
            .returning(|_| Ok(()));

        let midstream = Midstream::new(
            Box::new(mock_llm),
            Box::new(mock_hypr),
        );

        let result = midstream.process_stream().await;
        assert!(result.is_ok());

        let metrics = midstream.get_metrics().await;
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_real_time_aggregation() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();

        mock_hypr.expect_query_aggregate()
            .times(1)
            .return_once(|_, _| Ok(0.75));

        let midstream = Midstream::new(
            Box::new(mock_llm),
            Box::new(mock_hypr),
        );

        let avg = midstream.get_average_sentiment(Duration::from_secs(300)).await;
        assert!(avg.is_ok());
        assert_eq!(avg.unwrap(), 0.75);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();

        mock_hypr.expect_ingest_metric()
            .times(1)
            .return_once(|_| Err("Ingestion error".into()));

        mock_llm.expect_stream()
            .times(1)
            .return_once(|| {
                Box::pin(stream::iter(vec!["test message".to_string()]))
            });

        let midstream = Midstream::new(
            Box::new(mock_llm),
            Box::new(mock_hypr),
        );

        let result = midstream.process_stream().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to ingest metric"));
    }

    #[tokio::test]
    async fn test_empty_stream() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();

        mock_llm.expect_stream()
            .times(1)
            .return_once(|| {
                Box::pin(stream::iter(Vec::<String>::new()))
            });

        let midstream = Midstream::new(
            Box::new(mock_llm),
            Box::new(mock_hypr),
        );

        let result = midstream.process_stream().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_large_message_processing() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();
        
        let large_message = "x".repeat(1_000_000);
        mock_llm.expect_stream()
            .times(1)
            .return_once(move || {
                Box::pin(stream::iter(vec![large_message.clone()]))
            });

        mock_hypr.expect_ingest_metric()
            .returning(|_| Ok(()));

        let midstream = Midstream::new(
            Box::new(mock_llm),
            Box::new(mock_hypr),
        );

        let result = midstream.process_stream().await;
        assert!(result.is_ok());
        
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.len(), 1_000_000);
    }

    #[tokio::test]
    async fn test_inflight_decision_making() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();
        let mut mock_tool = MockToolClient::new();

        mock_llm.expect_stream()
            .times(1)
            .return_once(|| {
                Box::pin(stream::iter(vec![
                    "URGENT: What's the weather".to_string(),
                ]))
            });

        mock_tool.expect_handle_weather_intent()
            .times(1)
            .return_once(|_| Ok("Weather info (urgent response)".to_string()));

        mock_hypr.expect_ingest_metric()
            .returning(|_| Ok(()));

        let midstream = Midstream::with_tool_integration(
            Box::new(mock_llm),
            Box::new(mock_hypr),
            Box::new(mock_tool),
        );

        let result = midstream.process_stream().await;
        assert!(result.is_ok());
        let messages = result.unwrap();

        assert_eq!(messages[0].intent, Some(Intent::Weather));
        assert!(messages[0].tool_response.as_ref().unwrap().contains("urgent response"));
    }

    #[tokio::test]
    async fn test_empty_message_handling() {
        let mut mock_llm = MockLLMClient::new();
        let mut mock_hypr = MockHyprService::new();

        mock_llm.expect_stream()
            .times(1)
            .return_once(|| {
                Box::pin(stream::iter(vec!["".to_string()]))
            });

        let midstream = Midstream::new(
            Box::new(mock_llm),
            Box::new(mock_hypr),
        );

        let result = midstream.process_stream().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty message content"));
    }
}
