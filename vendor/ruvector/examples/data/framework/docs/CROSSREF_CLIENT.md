# CrossRef API Client

The CrossRef client provides seamless integration with CrossRef.org's scholarly publication API, enabling researchers to discover and analyze academic works within the RuVector data discovery framework.

## Features

- **Free API Access**: No authentication required (polite pool recommended)
- **Comprehensive Search**: Search by keywords, DOI, funder, subject, type, and date
- **Citation Analysis**: Find citing works and references
- **Rate Limiting**: Automatic rate limiting with retry logic
- **Polite Pool**: Better rate limits with email configuration
- **SemanticVector Conversion**: Automatic conversion to RuVector's semantic vector format

## Quick Start

```rust
use ruvector_data_framework::CrossRefClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Create client with polite pool email
    let client = CrossRefClient::new(Some("your-email@university.edu".to_string()));

    // Search publications
    let vectors = client.search_works("machine learning", 20).await?;

    // Process results
    for vector in vectors {
        println!("Title: {}", vector.metadata.get("title").unwrap());
        println!("DOI: {}", vector.metadata.get("doi").unwrap());
        println!("Citations: {}", vector.metadata.get("citation_count").unwrap());
    }

    Ok(())
}
```

## API Methods

### 1. Search Works

Search publications by keywords:

```rust
let vectors = client.search_works("quantum computing", 50).await?;
```

Searches across title, abstract, author, and other fields.

### 2. Get Work by DOI

Retrieve a specific publication:

```rust
let work = client.get_work("10.1038/nature12373").await?;
```

DOI formats accepted:
- `10.1038/nature12373`
- `http://doi.org/10.1038/nature12373`
- `https://dx.doi.org/10.1038/nature12373`

### 3. Search by Funder

Find research funded by specific organizations:

```rust
// NSF-funded research
let nsf_works = client.search_by_funder("10.13039/100000001", 20).await?;

// NIH-funded research
let nih_works = client.search_by_funder("10.13039/100000002", 20).await?;
```

Common funder DOIs:
- NSF: `10.13039/100000001`
- NIH: `10.13039/100000002`
- DOE: `10.13039/100000015`
- European Commission: `10.13039/501100000780`

### 4. Search by Subject

Filter publications by subject area:

```rust
let bio_works = client.search_by_subject("molecular biology", 30).await?;
```

### 5. Get Citations

Find papers that cite a specific work:

```rust
let citing_papers = client.get_citations("10.1038/nature12373", 15).await?;
```

### 6. Search Recent Publications

Find publications since a specific date:

```rust
let recent = client.search_recent("artificial intelligence", "2024-01-01", 25).await?;
```

Date format: `YYYY-MM-DD`

### 7. Search by Type

Filter by publication type:

```rust
// Find datasets
let datasets = client.search_by_type("dataset", Some("climate"), 10).await?;

// Find journal articles
let articles = client.search_by_type("journal-article", None, 20).await?;
```

Supported types:
- `journal-article` - Journal articles
- `book-chapter` - Book chapters
- `proceedings-article` - Conference proceedings
- `dataset` - Research datasets
- `monograph` - Monographs
- `report` - Technical reports

## SemanticVector Output

All methods return `Vec<SemanticVector>` with the following structure:

```rust
SemanticVector {
    id: "doi:10.1038/nature12373",           // Unique identifier
    embedding: Vec<f32>,                       // 384-dim embedding (default)
    domain: Domain::Research,                  // Research domain
    timestamp: DateTime<Utc>,                  // Publication date
    metadata: HashMap<String, String> {
        "doi": "10.1038/nature12373",
        "title": "Paper Title",
        "abstract": "Abstract text...",
        "authors": "John Doe; Jane Smith",
        "journal": "Nature",
        "citation_count": "142",
        "references_count": "35",
        "subjects": "Biology, Genetics",
        "funders": "NSF, NIH",
        "type": "journal-article",
        "publisher": "Nature Publishing Group",
        "source": "crossref"
    }
}
```

## Configuration

### Polite Pool

For better rate limits, provide your email:

```rust
let client = CrossRefClient::new(Some("researcher@university.edu".to_string()));
```

Benefits:
- Higher rate limits (~50 req/sec vs ~10 req/sec)
- Better API responsiveness
- Good citizenship in the scholarly community

### Custom Embedding Dimension

Adjust embedding dimension for your use case:

```rust
let client = CrossRefClient::with_embedding_dim(
    Some("researcher@university.edu".to_string()),
    512  // Use 512-dimensional embeddings
);
```

## Rate Limiting

The client automatically enforces conservative rate limits:
- **Default**: 1 request per second
- **With polite pool**: Can handle ~50 requests/second
- **Automatic retry**: Up to 3 retries with exponential backoff

## Error Handling

```rust
use ruvector_data_framework::{CrossRefClient, Result, FrameworkError};

match client.search_works("query", 10).await {
    Ok(vectors) => {
        println!("Found {} publications", vectors.len());
    }
    Err(FrameworkError::Network(e)) => {
        eprintln!("Network error: {}", e);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Advanced Usage

### Multi-Source Discovery

Combine CrossRef with other data sources:

```rust
use ruvector_data_framework::{CrossRefClient, ArxivClient};

let crossref = CrossRefClient::new(Some("email@example.com".to_string()));
let arxiv = ArxivClient::new();

// Search both sources
let crossref_results = crossref.search_works("quantum computing", 20).await?;
let arxiv_results = arxiv.search("quantum computing", 20).await?;

// Combine results
let all_results = [crossref_results, arxiv_results].concat();
```

### Citation Network Analysis

Build citation networks:

```rust
let seed_doi = "10.1038/nature12373";
let seed_work = client.get_work(seed_doi).await?.unwrap();

// Get papers that cite this work
let citing_papers = client.get_citations(seed_doi, 50).await?;

// Get papers this work cites (from references_count metadata)
// Note: CrossRef API doesn't directly provide references, but you can use metadata
```

### Temporal Analysis

Analyze publication trends over time:

```rust
use chrono::{Utc, Duration};

let mut all_papers = Vec::new();

// Fetch papers by year
for year in 2020..=2024 {
    let from_date = format!("{}-01-01", year);
    let to_date = format!("{}-12-31", year);

    let papers = client.search_recent(
        "climate change",
        &from_date,
        100
    ).await?;

    all_papers.extend(papers);
}

// Analyze trends
for year in 2020..=2024 {
    let count = all_papers.iter()
        .filter(|p| p.timestamp.format("%Y").to_string() == year.to_string())
        .count();
    println!("{}: {} papers", year, count);
}
```

## Examples

See `examples/crossref_demo.rs` for a comprehensive demonstration:

```bash
cargo run --example crossref_demo
```

## API Documentation

For complete CrossRef API documentation, visit:
- [CrossRef REST API](https://api.crossref.org)
- [CrossRef API Documentation](https://github.com/CrossRef/rest-api-doc)

## Limitations

1. **Abstract availability**: Not all works have abstracts in CrossRef
2. **Full-text access**: CrossRef provides metadata only, not full text
3. **Rate limits**: Conservative rate limiting to respect API usage policies
4. **Data completeness**: Metadata quality varies by publisher

## Testing

Run the test suite:

```bash
# Run all tests (offline tests only)
cargo test crossref_client --lib

# Run integration tests (requires network)
cargo test crossref_client --lib -- --ignored
```

## License

This client is part of the RuVector Data Discovery Framework.
