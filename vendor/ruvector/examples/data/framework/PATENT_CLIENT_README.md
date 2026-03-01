# Patent Database API Client

A comprehensive patent data discovery client for the RuVector framework, providing access to USPTO and EPO patent databases.

## Features

### USPTO PatentsView API Client

The `UsptoPatentClient` provides free, unauthenticated access to the USPTO PatentsView API with the following capabilities:

#### Search Methods

1. **Keyword Search** - `search_patents(query, max_results)`
   - Search patents by keywords in title and abstract
   - Example: `client.search_patents("quantum computing", 100).await?`

2. **Assignee Search** - `search_by_assignee(company_name, max_results)`
   - Find all patents by a specific company or organization
   - Example: `client.search_by_assignee("IBM", 50).await?`

3. **CPC Classification Search** - `search_by_cpc(cpc_class, max_results)`
   - Search by Cooperative Patent Classification code
   - Example: `client.search_by_cpc("Y02E", 200).await?`

4. **Patent Details** - `get_patent(patent_number)`
   - Get detailed information for a specific patent
   - Example: `client.get_patent("10000000").await?`

5. **Citation Analysis** - `get_citations(patent_number)`
   - Get both citing and cited patents for citation network analysis
   - Returns: `(citing_patents, cited_patents)`

#### CPC Classification Codes of Interest

- **Y02** - Climate Change Mitigation Technologies
  - `Y02E` - Energy generation, transmission, distribution
  - `Y02T` - Climate change mitigation technologies related to transportation
  - `Y02P` - Climate change mitigation technologies in production processes

- **G06N** - Computing Arrangements Based on AI/ML
  - `G06N3` - Computing based on biological models (neural networks)
  - `G06N5` - Computing based on knowledge-based models
  - `G06N20` - Machine learning

- **A61** - Medical or Veterinary Science
  - `A61K` - Preparations for medical, dental, or toilet purposes
  - `A61P` - Specific therapeutic activity of chemical compounds

- **H01** - Electric Elements
  - `H01L` - Semiconductor devices
  - `H01M` - Batteries, fuel cells, capacitors

## Data Format

All patent data is converted to `SemanticVector` format:

```rust
SemanticVector {
    id: "US10123456",              // Patent number with US prefix
    embedding: Vec<f32>,            // 512-dimension embedding from title + abstract
    domain: Domain::Research,       // Could be Domain::Innovation if added
    timestamp: DateTime<Utc>,       // Grant date or filing date
    metadata: HashMap {
        "patent_number": "10123456",
        "title": "Quantum computing system...",
        "abstract": "A quantum computing system comprising...",
        "assignee": "IBM Corporation",
        "inventors": "John Doe, Jane Smith",
        "cpc_codes": "G06N10/00, G06N99/00",
        "citations_count": "42",    // Number of patents citing this one
        "cited_count": "15",        // Number of patents cited by this one
        "source": "uspto"
    }
}
```

## Rate Limiting

- **USPTO**: 200ms between requests (~5 req/sec) - follows PatentsView API guidelines
- **EPO**: 1000ms between requests (~1 req/sec) - conservative rate limiting
- Automatic retry with exponential backoff (max 3 retries)

## Usage Example

```rust
use ruvector_data_framework::{UsptoPatentClient, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Create client (no authentication required)
    let client = UsptoPatentClient::new()?;

    // Search for climate tech patents
    let patents = client.search_by_cpc("Y02E", 100).await?;

    for patent in patents {
        println!("Patent: {} - {}",
            patent.id,
            patent.metadata.get("title").unwrap_or(&"Untitled".to_string())
        );
    }

    // Search by company
    let tesla_patents = client.search_by_assignee("Tesla", 50).await?;

    // Get specific patent with citations
    if let Some(patent) = client.get_patent("10000000").await? {
        let (citing, cited) = client.get_citations(&patent.id[2..]).await?;
        println!("Cited by {} patents, cites {} patents", citing.len(), cited.len());
    }

    Ok(())
}
```

## Run the Example

```bash
cargo run --example patent_discovery
```

## EPO Client (Future Development)

The `EpoClient` is a placeholder for European Patent Office integration. Implementation requires:
- OAuth authentication flow
- EPO developer registration at https://developers.epo.org/
- Consumer key and secret

## API Documentation

- **USPTO PatentsView**: https://patentsview.org/apis/api-endpoints
- **EPO Open Patent Services**: https://developers.epo.org/

## Testing

```bash
# Run all tests
cargo test --lib patent_clients

# Run integration tests (requires network)
cargo test --lib patent_clients -- --ignored

# Test specific functionality
cargo test --lib patent_clients::tests::test_cpc_classification_mapping
```

## Integration with Discovery Framework

The patent client integrates seamlessly with the RuVector discovery framework:

```rust
use ruvector_data_framework::{
    UsptoPatentClient,
    DiscoveryPipeline,
    PipelineConfig,
};

// 1. Fetch patent data
let client = UsptoPatentClient::new()?;
let vectors = client.search_by_cpc("G06N", 1000).await?;

// 2. Add to discovery engine
let mut pipeline = DiscoveryPipeline::new(PipelineConfig::default());
// ... add vectors to pipeline ...

// 3. Discover patterns across patent citation networks
let patterns = pipeline.run(source).await?;
```

## Features

- ✅ Free API access (no authentication)
- ✅ Comprehensive patent metadata
- ✅ Citation network analysis
- ✅ CPC classification search
- ✅ Rate limiting and retry logic
- ✅ SemanticVector conversion with embeddings
- ✅ Unit and integration tests
- 🔄 EPO integration (planned)

## Contributing

To add new patent sources:
1. Implement the client following the pattern in `patent_clients.rs`
2. Add conversion to `SemanticVector` format
3. Implement rate limiting and error handling
4. Add comprehensive tests
5. Update this documentation
