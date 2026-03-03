use midstream::{Midstream, HyprSettings, HyprServiceImpl, StreamProcessor, LLMClient};
use futures::stream::BoxStream;
use futures::stream::iter;
use std::time::Duration;

// Example LLM client implementation
struct ExampleLLMClient;

impl LLMClient for ExampleLLMClient {
    fn stream(&self) -> BoxStream<'static, String> {
        Box::pin(iter(vec![
            "URGENT: What's the weather like?".to_string(),
            "Schedule a meeting for tomorrow".to_string(),
            "Just a normal message".to_string(),
        ]))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize settings
    let settings = HyprSettings::new()?;
    
    // Create hyprstream service
    let hypr_service = HyprServiceImpl::new(&settings).await?;
    
    // Create LLM client
    let llm_client = ExampleLLMClient;
    
    // Initialize Midstream
    let midstream = Midstream::new(
        Box::new(llm_client),
        Box::new(hypr_service),
    );
    
    // Process stream
    let messages = midstream.process_stream().await?;
    println!("\nProcessed messages:");
    for msg in &messages {
        println!("- Content: {}", msg.content);
        println!("  Intent: {:?}", msg.intent);
        if let Some(response) = &msg.tool_response {
            println!("  Tool Response: {}", response);
        }
        println!();
    }
    
    // Get metrics
    let metrics = midstream.get_metrics().await;
    println!("\nCollected metrics:");
    for metric in &metrics {
        println!("- Name: {}", metric.name);
        println!("  Value: {}", metric.value);
        println!("  Labels: {:?}", metric.labels);
        println!();
    }
    
    // Get average sentiment for last 5 minutes
    let avg = midstream.get_average_sentiment(Duration::from_secs(300)).await?;
    println!("\nAverage sentiment: {}", avg);
    
    Ok(())
}
