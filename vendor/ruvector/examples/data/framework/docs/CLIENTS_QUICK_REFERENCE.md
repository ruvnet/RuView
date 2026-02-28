# Data Source Clients - Quick Reference

## Summary Statistics

**Total Clients**: 30 across 12 modules
**Total Public Methods**: 150+
**Domain Coverage**: 10 (News, Social, Research, Economic, Patent, Space, Genomics, Physics, Medical, Knowledge)
**Embedding Dimensions**: 256 (standard), 384 (medical/scientific)

---

## Client Index by Domain

### News & Social (4 clients, 17 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| News API | newsapi.org | Required | 100ms | 4 |
| Reddit | reddit.com | Required | 1000ms | 5 |
| GitHub | github.com | Optional | 1000ms | 4 |
| HackerNews | hacker-news.firebase | None | 100ms | 4 |

### Economic & Financial (4 clients, 12 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| World Bank | worldbank.org | None | 250ms | 3 |
| FRED | stlouisfed.org | Required | 200ms | 3 |
| Alpha Vantage | alphavantage.co | Required | 12000ms | 4 |
| IMF | imf.org | None | 500ms | 2 |

### Patents (3 clients, 8 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| USPTO | uspto.gov | None | 500ms | 3 |
| EPO | ops.epo.org | Required | 1000ms | 3 |
| Google Patents | patents.google.com | None | 1000ms | 2 |

### Research Papers (4 clients, 19 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| ArXiv | arxiv.org | None | 3000ms | 4 |
| Semantic Scholar | semanticscholar.org | Optional | 1000ms/100ms | 6 |
| bioRxiv | biorxiv.org | None | 500ms | 4 |
| medRxiv | medrxiv.org | None | 500ms | 4 |
| CrossRef | crossref.org | None | 200ms | 5 |

### Space & Astronomy (3 clients, 10 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| NASA APOD | api.nasa.gov | Optional | 1000ms | 3 |
| SpaceX | spacexdata.com | None | 500ms | 4 |
| SIMBAD | simbad.cds.unistra.fr | None | 1000ms | 3 |

### Genomics & Proteomics (4 clients, 16 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| NCBI Gene | ncbi.nlm.nih.gov | Optional | 334ms/100ms | 4 |
| Ensembl | ensembl.org | None | 200ms | 5 |
| UniProt | uniprot.org | None | 200ms | 4 |
| PDB | rcsb.org | None | 500ms | 3 |

### Physics & Earth Science (4 clients, 14 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| USGS Earthquake | earthquake.usgs.gov | None | 200ms | 5 |
| CERN Open Data | opendata.cern.ch | None | 500ms | 3 |
| Argo Ocean | data-argo.ifremer.fr | None | 300ms | 4 |
| Materials Project | materialsproject.org | Required | 1000ms | 3 |

### Knowledge Graphs (2 clients, 11 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| Wikipedia | wikipedia.org | None | 100ms | 4 |
| Wikidata | wikidata.org | None | 100ms | 7 |

### Medical & Health (3 clients, 9 methods)
| Client | Endpoint | Auth | Rate Limit | Methods |
|--------|----------|------|------------|---------|
| PubMed | ncbi.nlm.nih.gov | Optional | 334ms/100ms | 4 |
| ClinicalTrials | clinicaltrials.gov | None | 100ms | 2 |
| FDA OpenFDA | fda.gov | None | 250ms | 3 |

---

## Rate Limiting Quick Reference

### Strictest Limits (Use Sparingly)
- **Alpha Vantage**: 12000ms (5 req/min, 500/day)
- **ArXiv**: 3000ms (1 req/3sec per guidelines)

### Standard Limits (Typical Usage)
- **1000ms**: Reddit, GitHub, EPO, Google Patents, SIMBAD, NASA, Materials Project
- **500ms**: USPTO, bioRxiv, medRxiv, IMF, SpaceX, PDB, CERN

### Fast Limits (High-Volume OK)
- **100-200ms**: News API, HackerNews, FRED, CrossRef, Ensembl, UniProt, Wikipedia, Wikidata, ClinicalTrials
- **With API Key**: NCBI Gene, PubMed, Semantic Scholar drop to 100ms

---

## Authentication Quick Reference

### No Auth Required (17 clients)
World Bank, IMF, USPTO, Google Patents, ArXiv, bioRxiv, medRxiv, CrossRef, SpaceX, SIMBAD, Ensembl, UniProt, PDB, USGS, CERN, Argo, Wikipedia, Wikidata, ClinicalTrials, FDA

### Optional Auth (Higher Limits) (5 clients)
GitHub, Semantic Scholar, NASA APOD, NCBI Gene, PubMed

### Required Auth (8 clients)
News API, Reddit, FRED, Alpha Vantage, EPO, Materials Project

---

## Method Count by Category

### Search Methods
- **Text Search**: All 30 clients support text-based search
- **ID Lookup**: 22 clients support direct ID/identifier lookup
- **Advanced Filters**: 18 clients support filtered searches (date, category, status, etc.)
- **Batch Operations**: 4 clients (PubMed, NCBI Gene, ArXiv, Semantic Scholar)

### Specialized Methods
- **Time-Series**: World Bank, FRED, Alpha Vantage (economic data)
- **Geographic**: USGS (earthquakes), Argo (ocean), SIMBAD (sky coordinates)
- **Graph Traversal**: Semantic Scholar (citations/references), Wikipedia (categories/links), Wikidata (SPARQL)
- **Relationships**: Wikipedia (15 avg links/article), Wikidata (structured claims)

---

## Data Transformation Patterns

### SemanticVector Output
```rust
SemanticVector {
    id: "SOURCE:identifier",      // Unique ID with source prefix
    embedding: Vec<f32>,           // 256 or 384 dimensions
    domain: Domain::*,             // News, Research, Medical, etc.
    timestamp: DateTime<Utc>,      // Publication/event date
    metadata: HashMap<String, String>  // Source-specific fields
}
```

### DataRecord Output (Wikipedia, Wikidata)
```rust
DataRecord {
    id: "source_identifier",
    source: "wikipedia|wikidata",
    record_type: "article|entity",
    timestamp: DateTime<Utc>,
    data: serde_json::Value,       // Full structured data
    embedding: Option<Vec<f32>>,   // Optional embeddings
    relationships: Vec<Relationship>  // Graph connections
}
```

---

## Domain Classification

### Domain::News
News API, HackerNews

### Domain::Social
Reddit, GitHub

### Domain::Research
ArXiv, Semantic Scholar, bioRxiv, medRxiv, CrossRef

### Domain::Economic
World Bank, FRED, Alpha Vantage, IMF

### Domain::Patent
USPTO, EPO, Google Patents

### Domain::Space
NASA APOD, SpaceX, SIMBAD

### Domain::Genomics
NCBI Gene, Ensembl, UniProt

### Domain::Protein
PDB

### Domain::Seismic
USGS Earthquake

### Domain::Ocean
Argo

### Domain::Physics
CERN Open Data, Materials Project

### Domain::Medical
PubMed, ClinicalTrials, FDA

---

## Error Handling

All clients implement:

### Retry Logic
- **Max Retries**: 3
- **Base Delay**: 1000ms
- **Backoff**: Exponential (delay × retry_count)
- **Triggers**: Network errors, HTTP 429 (Too Many Requests)

### Error Types
```rust
FrameworkError::Network(reqwest::Error)  // Connection issues
FrameworkError::Config(String)           // Configuration/parsing errors
FrameworkError::Discovery(String)        // Data not found
```

### Graceful Degradation
- Returns empty Vec on 404 (no results)
- Continues on partial failures in batch operations
- Logs warnings for rate limit hits

---

## Embedding Configuration

### Standard (256 dimensions)
Used by: News, Social, Economic, Patent, Research, Space, Physics clients
- Good for general text, titles, abstracts
- Fast computation
- Lower memory footprint

### Enhanced (384 dimensions)
Used by: Medical clients (PubMed, ClinicalTrials, FDA)
- Richer semantic representation
- Better for technical/medical terminology
- Higher accuracy for domain-specific searches

### Implementation
```rust
SimpleEmbedder::new(dimension: usize)
// Deterministic hash-based embeddings
// Consistent across runs
// No external model dependencies
```

---

## Usage Patterns

### Single Source Query
```rust
let client = ArxivClient::new()?;
let papers = client.search("quantum computing", 50).await?;
```

### Multi-Source Aggregation
```rust
let (arxiv, s2, pubmed) = tokio::join!(
    arxiv_client.search(query, 50),
    s2_client.search_papers(query, 50),
    pubmed_client.search_articles(query, 50)
);
```

### Filtered Search
```rust
// ClinicalTrials by status
let trials = ct_client.search_trials("diabetes", Some("RECRUITING")).await?;

// ArXiv by category
let papers = arxiv_client.search_by_category("cs.AI", 100).await?;

// USGS by magnitude range
let quakes = usgs_client.get_by_magnitude_range(4.0, 6.0, 30).await?;
```

### Batch Retrieval
```rust
// PubMed: Fetch up to 200 abstracts per request
let pmids = vec!["12345678", "87654321", ...];
let abstracts = pubmed_client.fetch_abstracts(&pmids).await?;
```

---

## Performance Tips

1. **Rate Limit Management**
   - Use API keys when available (10x speed boost for NCBI, Semantic Scholar)
   - Batch requests when supported (PubMed, NCBI Gene)
   - Parallel queries to independent sources

2. **Caching Strategy**
   - Cache immutable data (historical papers, patents)
   - Short TTL for dynamic data (news, social media)
   - Store embeddings to avoid recomputation

3. **Query Optimization**
   - Use specific filters to reduce result size
   - Leverage ID lookups over full-text search when possible
   - For knowledge graphs (Wikidata), use SPARQL for complex queries

4. **Resource Management**
   - Reuse HTTP clients (already implemented via Arc)
   - Consider connection pooling for high-volume usage
   - Monitor rate limit headers (future enhancement)

---

## Common Use Cases

### Academic Research
- **ArXiv + Semantic Scholar + CrossRef**: Comprehensive paper discovery
- **PubMed + bioRxiv**: Medical/biomedical research
- **NCBI Gene + Ensembl + UniProt**: Genomics research

### Market Intelligence
- **World Bank + FRED + IMF**: Macroeconomic analysis
- **Alpha Vantage**: Stock market data
- **USPTO + EPO**: Patent landscape analysis

### News Aggregation
- **News API**: Current events
- **Reddit + HackerNews**: Tech community discussions
- **GitHub**: Developer activity

### Scientific Data
- **USGS**: Earthquake monitoring
- **CERN**: Particle physics datasets
- **Materials Project**: Computational materials science
- **Argo**: Ocean climate data

### Knowledge Discovery
- **Wikipedia**: Structured articles with categories
- **Wikidata**: Entity relationships via SPARQL
- **Semantic Scholar**: Citation network analysis

---

## File Locations

| File | Clients | LOC |
|------|---------|-----|
| `api_clients.rs` | News, Reddit, GitHub, HackerNews | ~800 |
| `economic_clients.rs` | World Bank, FRED, Alpha Vantage, IMF | ~600 |
| `patent_clients.rs` | USPTO, EPO, Google Patents | ~500 |
| `arxiv_client.rs` | ArXiv | ~300 |
| `semantic_scholar.rs` | Semantic Scholar | ~400 |
| `biorxiv_client.rs` | bioRxiv, medRxiv | ~400 |
| `crossref_client.rs` | CrossRef | ~300 |
| `space_clients.rs` | NASA, SpaceX, SIMBAD | ~600 |
| `genomics_clients.rs` | NCBI Gene, Ensembl, UniProt, PDB | ~900 |
| `physics_clients.rs` | USGS, CERN, Argo, Materials Project | ~1200 |
| `wiki_clients.rs` | Wikipedia, Wikidata | ~900 |
| `medical_clients.rs` | PubMed, ClinicalTrials, FDA | ~900 |

**Total**: ~7,800 lines of client implementation code

---

## Next Steps

1. Review full inventory: `/home/user/ruvector/examples/data/framework/docs/API_CLIENTS_INVENTORY.md`
2. Check example usage: `/home/user/ruvector/examples/data/framework/examples/`
3. Run tests: `cargo test --features data-framework`
4. API key setup: Store in environment variables for optimal performance

---

**Generated**: 2026-01-04
**Framework Version**: RuVector Data Framework v0.1.0
