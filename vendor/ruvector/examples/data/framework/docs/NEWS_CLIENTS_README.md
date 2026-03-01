# News & Social Media API Clients

Comprehensive Rust client module for News & Social APIs, following TDD approach and RuVector patterns.

## Overview

This module provides async clients for fetching data from news and social media APIs, converting responses into RuVector's `DataRecord` format with semantic embeddings.

## Implemented Clients

### 1. HackerNewsClient

**Base URL**: `https://hacker-news.firebaseio.com/v0`

**Features**:
- ✅ `get_top_stories(limit)` - Top story IDs
- ✅ `get_new_stories(limit)` - New stories
- ✅ `get_best_stories(limit)` - Best stories
- ✅ `get_item(id)` - Get story/comment by ID
- ✅ `get_user(username)` - User profile

**Authentication**: None required

**Rate Limits**: Generous (no strict limits)

**Status**: ✅ Fully working with real data

```rust
use ruvector_data_framework::HackerNewsClient;

let client = HackerNewsClient::new()?;
let stories = client.get_top_stories(10).await?;
```

### 2. GuardianClient

**Base URL**: `https://content.guardianapis.com`

**Features**:
- ✅ `search(query, limit)` - Search articles
- ✅ `get_article(id)` - Get article by ID
- ✅ `get_sections()` - List sections
- ✅ `search_by_tag(tag, limit)` - Tag-based search

**Authentication**: API key required (`GUARDIAN_API_KEY`)

**Rate Limits**: Free tier - 12 calls/sec, 5000/day

**Mock Fallback**: ✅ Synthetic data when no API key

**Get API Key**: https://open-platform.theguardian.com/

```rust
use ruvector_data_framework::GuardianClient;

let client = GuardianClient::new(Some("your_api_key".to_string()))?;
let articles = client.search("technology", 10).await?;
```

### 3. NewsDataClient

**Base URL**: `https://newsdata.io/api/1`

**Features**:
- ✅ `get_latest(query, country, category)` - Latest news
- ✅ `get_archive(query, from_date, to_date)` - Historical news

**Authentication**: API key required (`NEWSDATA_API_KEY`)

**Rate Limits**: Free tier - 200 requests/day

**Mock Fallback**: ✅ Synthetic data when no API key

**Get API Key**: https://newsdata.io/

```rust
use ruvector_data_framework::NewsDataClient;

let client = NewsDataClient::new(Some("your_api_key".to_string()))?;
let news = client.get_latest(Some("AI"), Some("us"), Some("technology")).await?;
```

### 4. RedditClient

**Base URL**: `https://www.reddit.com` (JSON endpoints)

**Features**:
- ✅ `get_subreddit_posts(subreddit, sort, limit)` - Subreddit posts
- ✅ `get_post_comments(post_id)` - Post comments
- ✅ `search(query, subreddit, limit)` - Search posts

**Authentication**: None (uses public `.json` endpoints)

**Rate Limits**: Be respectful (1 req/sec implemented)

**Special Handling**: ✅ Reddit's `.json` suffix pattern

```rust
use ruvector_data_framework::RedditClient;

let client = RedditClient::new()?;
let posts = client.get_subreddit_posts("programming", "hot", 10).await?;
```

## Architecture

### Data Flow

```
API Response → Deserialize → Convert to DataRecord → Generate Embedding → Return
```

### Key Components

1. **Response Structures**: Serde deserialization for API JSON responses
2. **Conversion Methods**: `*_to_record()` methods convert API data to `DataRecord`
3. **Embedding Generation**: Uses `SimpleEmbedder` (128-dim bag-of-words)
4. **Retry Logic**: Exponential backoff with 3 max retries
5. **Rate Limiting**: Client-specific delays to respect API limits

### DataRecord Structure

```rust
pub struct DataRecord {
    pub id: String,                      // Unique ID
    pub source: String,                  // "hackernews", "guardian", etc.
    pub record_type: String,             // "story", "article", "post", etc.
    pub timestamp: DateTime<Utc>,        // Publication time
    pub data: serde_json::Value,         // Raw data
    pub embedding: Option<Vec<f32>>,     // 128-dim semantic vector
    pub relationships: Vec<Relationship>, // Graph relationships
}
```

## Testing

### Test Coverage

- ✅ 16 comprehensive tests (all passing)
- Client creation tests
- Conversion function tests
- Synthetic data generation tests
- Embedding normalization tests
- Timestamp parsing tests

### Run Tests

```bash
# Run all news_clients tests
cargo test news_clients --lib

# Run specific test
cargo test news_clients::tests::test_hackernews_item_conversion

# Run with output
cargo test news_clients --lib -- --nocapture
```

### Test Results

```
test result: ok. 16 passed; 0 failed; 0 ignored
```

## Demo Example

Run the comprehensive demo:

```bash
# Basic demo (uses HackerNews without auth)
cargo run --example news_social_demo

# With API keys
GUARDIAN_API_KEY=your_key \
NEWSDATA_API_KEY=your_key \
cargo run --example news_social_demo
```

**Demo Output**:
- Fetches top HackerNews stories
- Searches Guardian articles
- Gets latest NewsData news
- Retrieves Reddit posts
- Shows embedding information

## Implementation Patterns

### Following api_clients.rs Patterns

✅ **Async/await with tokio**
```rust
pub async fn get_top_stories(&self, limit: usize) -> Result<Vec<DataRecord>>
```

✅ **Retry logic with exponential backoff**
```rust
async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
    let mut retries = 0;
    loop {
        match self.client.get(url).send().await {
            Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
                retries += 1;
                sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
            }
            Ok(response) => return Ok(response),
            Err(_) if retries < MAX_RETRIES => {
                retries += 1;
                sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
            }
            Err(e) => return Err(FrameworkError::Network(e)),
        }
    }
}
```

✅ **Mock fallback for API key clients**
```rust
if self.api_key.is_none() {
    return Ok(self.generate_synthetic_articles(query, limit)?);
}
```

✅ **Timestamp parsing**
```rust
// Unix timestamp (HackerNews, Reddit)
let timestamp = DateTime::from_timestamp(unix_time, 0).unwrap_or_else(Utc::now);

// RFC3339 (Guardian)
let timestamp = DateTime::parse_from_rfc3339(&date_string)
    .map(|dt| dt.with_timezone(&Utc))
    .unwrap_or_else(|_| Utc::now());

// Custom format (NewsData)
let timestamp = NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M:%S")
    .ok()
    .map(|ndt| ndt.and_utc())
    .unwrap_or_else(Utc::now);
```

✅ **DataSource trait implementation**
```rust
#[async_trait]
impl DataSource for HackerNewsClient {
    fn source_id(&self) -> &str {
        "hackernews"
    }

    async fn fetch_batch(
        &self,
        _cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let records = self.get_top_stories(batch_size).await?;
        Ok((records, None))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self.client.get(format!("{}/maxitem.json", self.base_url)).send().await?;
        Ok(response.status().is_success())
    }
}
```

## Special Implementations

### Reddit .json Pattern

Reddit's public API uses `.json` suffix:

```rust
let url = format!("{}/r/{}/{}.json?limit={}",
    self.base_url,      // https://www.reddit.com
    subreddit,          // "programming"
    sort,               // "hot"
    limit               // 25
);
// Results in: https://www.reddit.com/r/programming/hot.json?limit=25
```

### Guardian Tag Relationships

Creates graph relationships for tags:

```rust
if let Some(tags) = article.tags {
    for tag in tags {
        relationships.push(Relationship {
            target_id: format!("guardian_tag_{}", tag.id),
            rel_type: "has_tag".to_string(),
            weight: 1.0,
            properties: {
                let mut props = HashMap::new();
                props.insert("tag_type".to_string(), serde_json::json!(tag.tag_type));
                props.insert("tag_title".to_string(), serde_json::json!(tag.web_title));
                props
            },
        });
    }
}
```

### HackerNews Relationships

Creates author and comment relationships:

```rust
// Author relationship
if let Some(author) = &item.by {
    relationships.push(Relationship {
        target_id: format!("hn_user_{}", author),
        rel_type: "authored_by".to_string(),
        weight: 1.0,
        properties: HashMap::new(),
    });
}

// Comment relationships
for &kid_id in &item.kids {
    relationships.push(Relationship {
        target_id: format!("hn_item_{}", kid_id),
        rel_type: "has_comment".to_string(),
        weight: 1.0,
        properties: HashMap::new(),
    });
}
```

## Error Handling

All clients use the framework's `Result` type:

```rust
pub type Result<T> = std::result::Result<T, FrameworkError>;

pub enum FrameworkError {
    Ingestion(String),
    Coherence(String),
    Discovery(String),
    Network(#[from] reqwest::Error),
    Serialization(#[from] serde_json::Error),
    Graph(String),
    Config(String),
}
```

## Rate Limiting

Each client respects API limits:

| Client | Rate Limit | Implementation |
|--------|-----------|----------------|
| HackerNews | Generous | 100ms delay |
| Guardian | 12/sec, 5000/day | 100ms delay |
| NewsData | 200/day | 500ms delay |
| Reddit | Be respectful | 1000ms delay |

## Future Enhancements

Potential improvements:

- [ ] Twitter/X API integration
- [ ] Mastodon API client
- [ ] Discord message fetching
- [ ] Telegram channel scraping
- [ ] Advanced rate limit handling with token buckets
- [ ] Caching layer for repeated requests
- [ ] Streaming updates for real-time feeds
- [ ] Sentiment analysis integration
- [ ] Topic modeling on aggregated news

## Contributing

When adding new news/social clients:

1. Follow the patterns in `api_clients.rs`
2. Implement `DataSource` trait
3. Add comprehensive tests
4. Generate embeddings for all text content
5. Create relationships where applicable
6. Handle timestamps correctly
7. Implement retry logic
8. Add mock/synthetic data fallback for API key clients

## License

Part of RuVector data discovery framework.
