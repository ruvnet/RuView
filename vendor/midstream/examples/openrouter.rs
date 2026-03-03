use midstream::{Midstream, HyprSettings, HyprServiceImpl, StreamProcessor, LLMClient};
use futures::stream::{BoxStream, StreamExt};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use eventsource_stream::Eventsource;
use dotenv::dotenv;

struct OpenRouterClient {
    client: Client,
    api_key: String,
}

impl OpenRouterClient {
    fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
}

impl LLMClient for OpenRouterClient {
    fn stream(&self) -> BoxStream<'static, String> {
        let prompt = "Tell me a short story about a robot learning to paint. Make it emotional and stream it word by word.".to_string();
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        
        Box::pin(async_stream::stream! {
            let url = "https://openrouter.ai/api/v1/chat/completions";
            let referer = std::env::var("OPENROUTER_REFERER").unwrap_or_else(|_| "http://localhost:3000".to_string());
            let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "anthropic/claude-2".to_string());
            
            println!("Sending request to OpenRouter API...");
            println!("Model: {}", model);
            
            let payload = json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "stream": true
            });

            let response = client
                .post(url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("HTTP-Referer", referer)
                .json(&payload)
                .send()
                .await
                .expect("Failed to send request");

            println!("Response status: {}", response.status());
            
            let mut stream = response
                .bytes_stream()
                .eventsource()
                .map(|event| {
                    match event {
                        Ok(event) => {
                            println!("Received event: {}", event.data);
                            if event.data == "[DONE]" {
                                String::new()
                            } else {
                                match serde_json::from_str::<Value>(&event.data) {
                                    Ok(value) => {
                                        let content = value["choices"][0]["delta"]["content"]
                                            .as_str()
                                            .unwrap_or("");
                                        if !content.is_empty() {
                                            println!("Content: {}", content);
                                        }
                                        content.to_string()
                                    }
                                    Err(e) => {
                                        println!("Failed to parse JSON: {}", e);
                                        String::new()
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("Stream error: {}", e);
                            format!("Error: {}", e)
                        }
                    }
                });

            while let Some(s) = stream.next().await {
                if !s.is_empty() {
                    yield s.trim().to_string();
                }
            }
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv().ok();
    
    // Get API key from environment
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY must be set in .env file");

    // Initialize settings
    let settings = HyprSettings::new()?;
    
    // Create hyprstream service
    let hypr_service = HyprServiceImpl::new(&settings).await?;
    
    // Create OpenRouter client
    let llm_client = OpenRouterClient::new(api_key);
    
    // Initialize Midstream
    let midstream = Midstream::new(
        Box::new(llm_client),
        Box::new(hypr_service),
    );
    
    println!("\nStreaming story from Claude-2...\n");

    // Process stream
    let messages = midstream.process_stream().await?;
    
    println!("\nFinal story:");
    for msg in &messages {
        print!("{}", msg.content);
    }
    println!("\n");
    
    // Get metrics
    let metrics = midstream.get_metrics().await;
    println!("\nMetrics collected:");
    for metric in &metrics {
        println!("- Token count: {}", metric.value);
        println!("  Labels: {:?}", metric.labels);
        println!();
    }
    
    // Get average sentiment for last 5 minutes
    let avg = midstream.get_average_sentiment(Duration::from_secs(300)).await?;
    println!("\nAverage tokens per message: {:.2}", avg);
    
    Ok(())
}