//! News & Social Media API client demo
//!
//! Demonstrates fetching data from news and social media APIs:
//! - HackerNews: Top tech stories
//! - Guardian: News articles
//! - NewsData: Latest news
//! - Reddit: Subreddit posts
//!
//! Run with:
//! ```bash
//! # No API keys needed for HackerNews and Reddit
//! cargo run --example news_social_demo
//!
//! # With Guardian API key
//! GUARDIAN_API_KEY=your_key cargo run --example news_social_demo
//!
//! # With NewsData API key
//! NEWSDATA_API_KEY=your_key cargo run --example news_social_demo
//! ```

use ruvector_data_framework::{
    GuardianClient, HackerNewsClient, NewsDataClient, RedditClient,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== News & Social Media API Client Demo ===\n");

    // 1. HackerNews - No auth required
    println!("1. Fetching top stories from Hacker News...");
    let hn_client = HackerNewsClient::new()?;
    match hn_client.get_top_stories(5).await {
        Ok(stories) => {
            println!("   ✓ Fetched {} top stories", stories.len());
            for (i, story) in stories.iter().enumerate() {
                if let Some(data) = story.data.as_object() {
                    println!(
                        "   {}. {} (score: {})",
                        i + 1,
                        data.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No title"),
                        data.get("score")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    );
                }
            }
        }
        Err(e) => println!("   ✗ Failed: {}", e),
    }
    println!();

    // 2. Guardian - Requires API key or uses synthetic data
    println!("2. Fetching articles from The Guardian...");
    let guardian_api_key = env::var("GUARDIAN_API_KEY").ok();
    if guardian_api_key.is_none() {
        println!("   ℹ No GUARDIAN_API_KEY found, using synthetic data");
    }
    let guardian_client = GuardianClient::new(guardian_api_key)?;
    match guardian_client.search("technology", 5).await {
        Ok(articles) => {
            println!("   ✓ Fetched {} articles", articles.len());
            for (i, article) in articles.iter().enumerate() {
                if let Some(data) = article.data.as_object() {
                    println!(
                        "   {}. {}",
                        i + 1,
                        data.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No title")
                    );
                }
            }
        }
        Err(e) => println!("   ✗ Failed: {}", e),
    }
    println!();

    // 3. NewsData - Requires API key or uses synthetic data
    println!("3. Fetching latest news from NewsData.io...");
    let newsdata_api_key = env::var("NEWSDATA_API_KEY").ok();
    if newsdata_api_key.is_none() {
        println!("   ℹ No NEWSDATA_API_KEY found, using synthetic data");
    }
    let newsdata_client = NewsDataClient::new(newsdata_api_key)?;
    match newsdata_client
        .get_latest(Some("artificial intelligence"), None, Some("technology"))
        .await
    {
        Ok(news) => {
            println!("   ✓ Fetched {} news articles", news.len());
            for (i, article) in news.iter().enumerate() {
                if let Some(data) = article.data.as_object() {
                    println!(
                        "   {}. {}",
                        i + 1,
                        data.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No title")
                    );
                }
            }
        }
        Err(e) => println!("   ✗ Failed: {}", e),
    }
    println!();

    // 4. Reddit - No auth required for .json endpoints
    println!("4. Fetching posts from Reddit r/programming...");
    let reddit_client = RedditClient::new()?;
    match reddit_client.get_subreddit_posts("programming", "hot", 5).await {
        Ok(posts) => {
            println!("   ✓ Fetched {} posts", posts.len());
            for (i, post) in posts.iter().enumerate() {
                if let Some(data) = post.data.as_object() {
                    println!(
                        "   {}. {} (score: {})",
                        i + 1,
                        data.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No title"),
                        data.get("score")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    );
                }
            }
        }
        Err(e) => println!("   ✗ Failed: {}", e),
    }
    println!();

    // Show embedding info
    if let Ok(stories) = hn_client.get_top_stories(1).await {
        if let Some(story) = stories.first() {
            if let Some(embedding) = &story.embedding {
                println!("=== Embedding Information ===");
                println!("Dimension: {}", embedding.len());
                println!(
                    "Sample values: [{:.4}, {:.4}, {:.4}, ...]",
                    embedding[0], embedding[1], embedding[2]
                );
                println!();
            }
        }
    }

    println!("=== Demo Complete ===");
    println!();
    println!("Tips:");
    println!("- HackerNews and Reddit work without API keys");
    println!("- Guardian: Get free API key from https://open-platform.theguardian.com/");
    println!("- NewsData: Get free API key from https://newsdata.io/");
    println!("- All clients convert data to SemanticVector with embeddings");
    println!("- All clients support the DataSource trait for batch processing");

    Ok(())
}
