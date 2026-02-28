# bioRxiv and medRxiv Preprint API Clients

This module provides async clients for fetching preprints from **bioRxiv.org** (life sciences) and **medRxiv.org** (medical sciences), converting them to `SemanticVector` format for RuVector discovery.

## Features

- **Free API access** - No authentication required
- **Rate limiting** - Automatic 1 req/sec rate limiting (conservative)
- **Pagination support** - Handles large result sets automatically
- **Retry logic** - Built-in retry for transient failures
- **Domain separation** - bioRxiv → `Domain::Research`, medRxiv → `Domain::Medical`
- **Rich metadata** - DOI, authors, categories, publication status

## API Details

- **Base URL**: `https://api.biorxiv.org/details/[server]/[interval]/[cursor]`
- **Servers**: `biorxiv` or `medrxiv`
- **Interval**: Date range like `2024-01-01/2024-12-31`
- **Response**: JSON with collection array

## BiorxivClient (Life Sciences)

### Methods

```rust
use ruvector_data_framework::BiorxivClient;

let client = BiorxivClient::new();

// Get recent preprints (last N days)
let recent = client.search_recent(7, 100).await?;

// Search by date range
let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
let papers = client.search_by_date_range(start, end, Some(200)).await?;

// Search by category
let neuro = client.search_by_category("neuroscience", 100).await?;
```

### Categories

- `neuroscience` - Neural systems and computation
- `genomics` - Genome sequencing and analysis
- `bioinformatics` - Computational biology
- `cancer-biology` - Oncology research
- `immunology` - Immune system studies
- `microbiology` - Microorganisms
- `molecular-biology` - Molecular mechanisms
- `cell-biology` - Cellular processes
- `biochemistry` - Chemical processes
- `evolutionary-biology` - Evolution and phylogenetics
- `ecology` - Ecosystems and populations
- `genetics` - Heredity and variation
- `developmental-biology` - Organism development
- `synthetic-biology` - Engineered biological systems
- `systems-biology` - System-level understanding

## MedrxivClient (Medical Sciences)

### Methods

```rust
use ruvector_data_framework::MedrxivClient;

let client = MedrxivClient::new();

// Get recent medical preprints
let recent = client.search_recent(7, 100).await?;

// Search by date range
let papers = client.search_by_date_range(start, end, Some(200)).await?;

// Search COVID-19 related papers
let covid = client.search_covid(100).await?;

// Search clinical research
let clinical = client.search_clinical(50).await?;
```

### Specialized Searches

- **COVID-19**: Filters for "covid", "sars-cov-2", "coronavirus", "pandemic" keywords
- **Clinical Research**: Filters for "clinical", "trial", "patient", "treatment", "therapy", "diagnosis"

## SemanticVector Output

Both clients convert preprints to `SemanticVector` with:

```rust
SemanticVector {
    id: "doi:10.1101/2024.01.01.123456",
    embedding: Vec<f32>,  // Generated from title + abstract
    domain: Domain::Research,  // or Domain::Medical for medRxiv
    timestamp: DateTime<Utc>,  // Preprint publication date
    metadata: {
        "doi": "10.1101/2024.01.01.123456",
        "title": "Paper title",
        "abstract": "Full abstract text",
        "authors": "John Doe; Jane Smith",
        "category": "Neuroscience",
        "server": "biorxiv",
        "published_status": "preprint" or journal name,
        "corresponding_author": "John Doe",
        "institution": "MIT",
        "version": "1",
        "type": "new results",
        "source": "biorxiv" or "medrxiv"
    }
}
```

## Example Usage

See `examples/biorxiv_discovery.rs` for a complete example:

```bash
cargo run --example biorxiv_discovery
```

## Rate Limiting

- **Default**: 1 request per second (conservative)
- **Configurable**: Modify `BIORXIV_RATE_LIMIT_MS` constant if needed
- **Retry logic**: 3 retries with exponential backoff

## Pagination

Both clients handle pagination automatically:

- Fetches up to the specified `limit`
- Uses cursor-based pagination
- Safety limit of 10,000 records per query
- Handles empty result sets gracefully

## Integration with RuVector

Use the generated `SemanticVector`s with:

1. **Vector similarity search**: Find related preprints using HNSW index
2. **Graph coherence analysis**: Detect emerging research trends
3. **Cross-domain discovery**: Find connections between life sciences and medical research
4. **Time-series analysis**: Track research evolution over time

## Error Handling

The clients include comprehensive error handling:

- **Network errors**: Automatic retry with exponential backoff
- **Rate limiting**: Built-in delays between requests
- **Parsing errors**: Graceful handling of malformed responses
- **Empty results**: Returns empty vector instead of error

## Testing

Run the unit tests:

```bash
# Run all tests (excluding integration tests)
cargo test --lib biorxiv_client::tests

# Run integration tests (requires network access)
cargo test --lib biorxiv_client::tests -- --ignored
```

Unit tests cover:
- Client creation
- Embedding dimension configuration
- Record to vector conversion
- Date parsing
- Domain assignment
- Metadata extraction

Integration tests (ignored by default):
- Search recent papers
- Search by category
- COVID-19 search
- Clinical research search

## Dependencies

- `reqwest` - Async HTTP client
- `serde` / `serde_json` - JSON parsing
- `chrono` - Date/time handling
- `tokio` - Async runtime
- `urlencoding` - URL encoding for queries
- `SimpleEmbedder` - Text to vector embedding

## Custom Embedding Dimension

```rust
// Default 384 dimensions
let client = BiorxivClient::new();

// Custom dimension
let client = BiorxivClient::with_embedding_dim(512);
```

## Best Practices

1. **Respect rate limits**: The clients enforce conservative rate limiting
2. **Use date ranges**: For large datasets, query by date ranges
3. **Filter locally**: Use category filters for more specific searches
4. **Handle errors**: Network requests can fail, use proper error handling
5. **Cache results**: Consider caching SemanticVectors for repeated use
6. **Batch processing**: Process results in batches for better performance

## Publication Status

The `published_status` metadata field indicates:
- `"preprint"` - Not yet published in journal
- Journal name - Accepted and published (e.g., "Nature Medicine")

This helps distinguish between preliminary and peer-reviewed research.

## Cross-Domain Analysis

Combine bioRxiv and medRxiv for comprehensive analysis:

```rust
let biorxiv = BiorxivClient::new();
let medrxiv = MedrxivClient::new();

let bio_papers = biorxiv.search_recent(7, 100).await?;
let med_papers = medrxiv.search_recent(7, 100).await?;

let mut all_papers = bio_papers;
all_papers.extend(med_papers);

// Use RuVector's discovery engine to find cross-domain patterns
```

## Resources

- **bioRxiv**: https://www.biorxiv.org/
- **medRxiv**: https://www.medrxiv.org/
- **API Docs**: https://api.biorxiv.org/
- **RuVector**: https://github.com/ruvnet/ruvector
