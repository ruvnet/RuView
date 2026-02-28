# RuVector Data Framework - API Clients Comprehensive Inventory

## Overview
Complete analysis of 12 client modules providing access to 30+ data sources across 10 domains.

**Total Clients Analyzed**: 30
**Total Public Methods**: 150+
**Domain Coverage**: News, Social, Research, Economic, Patent, Space, Genomics, Physics, Medical, Knowledge Graph
**Data Format**: All convert to `SemanticVector` or `DataRecord` with embeddings

---

## 1. api_clients.rs - News & Social Media

### News API Client
**Endpoint**: `https://newsapi.org/v2`
**Authentication**: Required (API key)
**Rate Limit**: 100ms delay (configurable)

#### Methods (4):
- `new(api_key: String)` - Initialize client
- `search_articles(query, from_date, to_date, language)` - Search news articles
- `get_top_headlines(category, country)` - Get top headlines by category/country
- `get_sources(category, language, country)` - List available news sources

#### Rate Limiting:
```rust
const DEFAULT_RATE_LIMIT_DELAY_MS: u64 = 100;
rate_limit_delay: Duration
```

#### Data Transformation:
```rust
NewsArticle -> SemanticVector {
    id: format!("NEWS:{}", hash(url)),
    embedding: embed_text(title + description + content),
    domain: Domain::News,
    metadata: {title, author, source, url, published_at, description}
}
```

#### Error Handling:
- Retry on `TOO_MANY_REQUESTS` (max 3 retries)
- Exponential backoff: `RETRY_DELAY_MS * retries`
- Network error wrapping via `FrameworkError::Network`

---

### Reddit Client
**Endpoint**: `https://oauth.reddit.com`
**Authentication**: Required (client_id, client_secret)
**Rate Limit**: 1000ms delay (Reddit: 60 req/min)

#### Methods (5):
- `new(client_id, client_secret)` - OAuth authentication
- `search_posts(query, subreddit, limit)` - Search posts in subreddit
- `get_hot_posts(subreddit, limit)` - Get hot posts
- `get_top_posts(subreddit, time_filter, limit)` - Get top posts (hour/day/week/month/year/all)
- `get_post_comments(post_id, limit)` - Get post comments

#### Rate Limiting:
```rust
const REDDIT_RATE_LIMIT_MS: u64 = 1000; // 60 req/min
```

#### Data Transformation:
```rust
RedditPost -> SemanticVector {
    id: format!("REDDIT:{}", post_id),
    embedding: embed_text(title + selftext),
    domain: Domain::Social,
    metadata: {subreddit, author, score, num_comments, created_utc, url}
}
```

---

### GitHub Client
**Endpoint**: `https://api.github.com`
**Authentication**: Optional (higher rate limits with token)
**Rate Limit**: 1000ms delay (5000/hour with token, 60/hour without)

#### Methods (4):
- `new(token: Option<String>)` - Initialize with optional token
- `search_repositories(query, sort, limit)` - Search repos
- `get_repository_issues(owner, repo, state)` - Get issues (open/closed/all)
- `search_code(query, language, limit)` - Search code

#### Rate Limiting:
```rust
const GITHUB_RATE_LIMIT_MS: u64 = 1000;
rate_limit_delay: Duration
```

---

### HackerNews Client
**Endpoint**: `https://hacker-news.firebaseio.com/v0`
**Authentication**: Not required
**Rate Limit**: 100ms delay

#### Methods (4):
- `new()` - Initialize client
- `get_top_stories(limit)` - Get top stories
- `get_new_stories(limit)` - Get newest stories
- `get_best_stories(limit)` - Get best stories

#### Data Transformation:
```rust
HnStory -> SemanticVector {
    id: format!("HN:{}", story_id),
    embedding: embed_text(title + text),
    domain: Domain::News,
    metadata: {title, url, score, descendants (comments), by (author)}
}
```

---

## 2. economic_clients.rs - Economic & Financial Data

### World Bank Client
**Endpoint**: `https://api.worldbank.org/v2`
**Authentication**: Not required
**Rate Limit**: 250ms delay

#### Methods (3):
- `new()` - Initialize client
- `get_indicator_data(indicator, country, start_year, end_year)` - Get economic indicators
- `search_indicators(query)` - Search available indicators

#### Common Indicators:
- `NY.GDP.MKTP.CD` - GDP (current US$)
- `SP.POP.TOTL` - Population
- `NY.GDP.PCAP.CD` - GDP per capita
- `FP.CPI.TOTL.ZG` - Inflation rate

#### Data Transformation:
```rust
WorldBankIndicator -> SemanticVector {
    id: format!("WB:{}:{}:{}", country, indicator, date),
    embedding: embed_text(indicator_name + country),
    domain: Domain::Economic,
    metadata: {indicator, country, value, date, country_name, indicator_name}
}
```

---

### FRED Client (Federal Reserve Economic Data)
**Endpoint**: `https://api.stlouisfed.org/fred`
**Authentication**: Required (API key from research.stlouisfed.org)
**Rate Limit**: 200ms delay

#### Methods (3):
- `new(api_key)` - Initialize with FRED API key
- `get_series(series_id, start_date, end_date)` - Get time series data
- `search_series(query)` - Search available series

#### Popular Series:
- `GDP` - Gross Domestic Product
- `UNRATE` - Unemployment Rate
- `CPIAUCSL` - Consumer Price Index
- `DFF` - Federal Funds Rate

---

### Alpha Vantage Client
**Endpoint**: `https://www.alphavantage.co/query`
**Authentication**: Required (free tier: 5 req/min, 500/day)
**Rate Limit**: 12000ms delay (5 req/min)

#### Methods (4):
- `new(api_key)` - Initialize client
- `get_stock_price(symbol)` - Real-time stock price
- `get_time_series_daily(symbol, days)` - Historical daily prices
- `get_forex_rate(from_currency, to_currency)` - FX rates

---

### IMF Client (International Monetary Fund)
**Endpoint**: `https://www.imf.org/external/datamapper/api/v1`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (2):
- `new()` - Initialize client
- `get_indicator(indicator_code, countries)` - Get IMF indicators

---

## 3. patent_clients.rs - Patent Data

### USPTO Client (US Patent Office)
**Endpoint**: `https://developer.uspto.gov/ibd-api/v1`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (3):
- `new()` - Initialize client
- `search_patents(query, start_date, end_date)` - Search patents
- `get_patent(patent_number)` - Get specific patent

---

### EPO Client (European Patent Office)
**Endpoint**: `https://ops.epo.org/3.2/rest-services`
**Authentication**: Required (OAuth2)
**Rate Limit**: 1000ms delay

#### Methods (3):
- `new(consumer_key, consumer_secret)` - OAuth2 authentication
- `search_patents(query)` - Search European patents
- `get_patent_details(patent_number)` - Get patent details

---

### Google Patents Client
**Endpoint**: `https://patents.google.com`
**Authentication**: Not required
**Rate Limit**: 1000ms delay (conservative)

#### Methods (2):
- `new()` - Initialize client
- `search_patents(query, max_results)` - Search patents

---

## 4. arxiv_client.rs - Research Papers

### ArXiv Client
**Endpoint**: `http://export.arxiv.org/api/query`
**Authentication**: Not required
**Rate Limit**: 3000ms delay (max 1 req/3sec per ArXiv guidelines)

#### Methods (4):
- `new()` - Initialize client
- `search(query, max_results)` - Search papers by query
- `search_by_category(category, max_results)` - Search by category (cs.AI, physics.gen-ph, etc.)
- `get_paper(arxiv_id)` - Get specific paper by ID

#### Categories Supported:
- `cs.AI` - Artificial Intelligence
- `cs.LG` - Machine Learning
- `physics.gen-ph` - General Physics
- `math.CO` - Combinatorics
- `q-bio.GN` - Genomics

#### Data Transformation:
```rust
ArxivEntry -> SemanticVector {
    id: format!("ARXIV:{}", arxiv_id),
    embedding: embed_text(title + summary),
    domain: Domain::Research,
    metadata: {arxiv_id, title, summary, authors, published, updated, category, pdf_url}
}
```

---

## 5. semantic_scholar.rs - Academic Papers

### Semantic Scholar Client
**Endpoint**: `https://api.semanticscholar.org/graph/v1`
**Authentication**: Optional (API key for higher limits)
**Rate Limit**:
- Without key: 1000ms (100 req/5min)
- With key: 100ms (1000 req/5min)

#### Methods (6):
- `new(api_key: Option<String>)` - Initialize client
- `search_papers(query, limit)` - Search papers
- `get_paper(paper_id)` - Get paper by S2 ID or DOI
- `get_paper_citations(paper_id, limit)` - Get citing papers
- `get_paper_references(paper_id, limit)` - Get referenced papers
- `search_authors(query, limit)` - Search authors

#### Data Transformation:
```rust
S2Paper -> SemanticVector {
    id: format!("S2:{}", paper_id),
    embedding: embed_text(title + abstract),
    domain: Domain::Research,
    metadata: {
        paper_id, title, abstract, authors, year,
        citation_count, reference_count, fields_of_study,
        venue, doi, arxiv_id, pubmed_id
    }
}
```

---

## 6. biorxiv_client.rs - Biomedical Preprints

### bioRxiv Client
**Endpoint**: `https://api.biorxiv.org/details/biorxiv`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (4):
- `new()` - Initialize client
- `search_preprints(query, days_back)` - Search preprints
- `get_preprint(doi)` - Get preprint by DOI
- `get_recent(days, limit)` - Get recent preprints

---

### medRxiv Client
**Endpoint**: `https://api.biorxiv.org/details/medrxiv`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (4):
- Same as bioRxiv but for medical preprints

#### Data Transformation:
```rust
BiorxivPreprint -> SemanticVector {
    id: format!("BIORXIV:{}", doi),
    embedding: embed_text(title + abstract),
    domain: Domain::Research,
    metadata: {doi, title, authors, date, category, version, abstract}
}
```

---

## 7. crossref_client.rs - DOI Registry

### CrossRef Client
**Endpoint**: `https://api.crossref.org/works`
**Authentication**: Not required (polite pool with email recommended)
**Rate Limit**: 200ms delay

#### Methods (5):
- `new(mailto: Option<String>)` - Initialize with optional email
- `search_works(query, limit)` - Search scholarly works
- `get_work(doi)` - Get work by DOI
- `get_journal_articles(issn, limit)` - Get articles from journal
- `search_by_type(work_type, query, limit)` - Search by type (journal-article, book-chapter, etc.)

#### Work Types:
- `journal-article`
- `book-chapter`
- `proceedings-article`
- `posted-content`
- `dataset`

---

## 8. space_clients.rs - Space & Astronomy

### NASA APOD Client (Astronomy Picture of the Day)
**Endpoint**: `https://api.nasa.gov/planetary/apod`
**Authentication**: API key (DEMO_KEY for testing)
**Rate Limit**: 1000ms delay

#### Methods (3):
- `new(api_key: Option<String>)` - Use DEMO_KEY if none provided
- `get_today()` - Get today's APOD
- `get_date(date)` - Get APOD for specific date

---

### SpaceX Launch Client
**Endpoint**: `https://api.spacexdata.com/v4`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (4):
- `new()` - Initialize client
- `get_latest_launch()` - Get most recent launch
- `get_upcoming_launches(limit)` - Get upcoming launches
- `get_past_launches(limit)` - Get historical launches

---

### SIMBAD Astronomical Database Client
**Endpoint**: `https://simbad.cds.unistra.fr/simbad/sim-tap`
**Authentication**: Not required
**Rate Limit**: 1000ms delay

#### Methods (3):
- `new()` - Initialize client
- `search_objects(query)` - Search astronomical objects
- `query_region(ra, dec, radius)` - Search by sky coordinates

---

## 9. genomics_clients.rs - Genomics & Proteomics

### NCBI Gene Client
**Endpoint**: `https://eutils.ncbi.nlm.nih.gov/entrez/eutils`
**Authentication**: Optional (API key for higher rate limits)
**Rate Limit**:
- Without key: 334ms (~3 req/sec)
- With key: 100ms (10 req/sec)

#### Methods (4):
- `new(api_key: Option<String>)` - Initialize client
- `search_genes(query, organism, max_results)` - Search genes
- `get_gene(gene_id)` - Get gene details by ID
- `get_gene_summary(gene_id)` - Get gene summary

---

### Ensembl Client
**Endpoint**: `https://rest.ensembl.org`
**Authentication**: Not required
**Rate Limit**: 200ms delay (15 req/sec limit)

#### Methods (5):
- `new()` - Initialize client
- `search_genes(query, species)` - Search genes in species
- `get_sequence(gene_id)` - Get gene sequence
- `get_homology(gene_id)` - Get homologous genes across species
- `get_variants(gene_id)` - Get genetic variants

---

### UniProt Client
**Endpoint**: `https://rest.uniprot.org`
**Authentication**: Not required
**Rate Limit**: 200ms delay

#### Methods (4):
- `new()` - Initialize client
- `search_proteins(query, limit)` - Search proteins
- `get_protein(accession)` - Get protein by accession
- `get_protein_features(accession)` - Get protein features

---

### PDB Client (Protein Data Bank)
**Endpoint**: `https://search.rcsb.org/rcsbsearch/v2/query`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (3):
- `new()` - Initialize client
- `search_structures(query, limit)` - Search protein structures
- `get_structure(pdb_id)` - Get structure by PDB ID

---

## 10. physics_clients.rs - Physics & Earth Science

### USGS Earthquake Client
**Endpoint**: `https://earthquake.usgs.gov/fdsnws/event/1`
**Authentication**: Not required
**Rate Limit**: 200ms delay (~5 req/sec)

#### Methods (5):
- `new()` - Initialize client
- `get_recent(min_magnitude, days)` - Recent earthquakes
- `search_by_region(lat, lon, radius_km, days)` - Regional search
- `get_significant(days)` - Significant earthquakes (mag ≥6.0 or sig ≥600)
- `get_by_magnitude_range(min, max, days)` - Magnitude range

#### Data Transformation:
```rust
UsgsEarthquake -> SemanticVector {
    id: format!("USGS:{}", earthquake_id),
    embedding: embed_text("Magnitude {mag} earthquake at {place}"),
    domain: Domain::Seismic,
    metadata: {
        magnitude, place, latitude, longitude, depth_km,
        tsunami, significance, status, alert
    }
}
```

---

### CERN Open Data Client
**Endpoint**: `https://opendata.cern.ch/api/records`
**Authentication**: Not required
**Rate Limit**: 500ms delay

#### Methods (3):
- `new()` - Initialize client
- `search_datasets(query)` - Search LHC datasets
- `get_dataset(recid)` - Get dataset by record ID
- `search_by_experiment(experiment)` - Search by experiment (CMS, ATLAS, LHCb, ALICE)

#### Data Transformation:
```rust
CernRecord -> SemanticVector {
    id: format!("CERN:{}", recid),
    embedding: embed_text(title + description + experiment),
    domain: Domain::Physics,
    metadata: {
        recid, title, experiment, collision_energy,
        collision_type, data_type
    }
}
```

---

### Argo Ocean Data Client
**Endpoint**: `https://data-argo.ifremer.fr`
**Authentication**: Not required
**Rate Limit**: 300ms delay (~3 req/sec)

#### Methods (4):
- `new()` - Initialize client
- `get_recent_profiles(days)` - Recent ocean profiles
- `search_by_region(lat, lon, radius_km)` - Regional ocean data
- `get_temperature_profiles()` - Temperature-focused profiles
- `create_sample_profiles(count)` - Generate sample data for testing

---

### Materials Project Client
**Endpoint**: `https://api.materialsproject.org`
**Authentication**: Required (API key from materialsproject.org)
**Rate Limit**: 1000ms delay (1 req/sec for free tier)

#### Methods (3):
- `new(api_key)` - Initialize with API key
- `search_materials(formula)` - Search by chemical formula (Si, Fe2O3, LiFePO4)
- `get_material(material_id)` - Get material by MP ID (mp-149)
- `search_by_property(property, min, max)` - Search by property range (band_gap, density)

---

## 11. wiki_clients.rs - Knowledge Graphs

### Wikipedia Client
**Endpoint**: `https://{lang}.wikipedia.org/w/api.php`
**Authentication**: Not required
**Rate Limit**: 100ms delay

#### Methods (4):
- `new(language)` - Initialize for language (en, de, fr, etc.)
- `search(query, limit)` - Search articles (max 500)
- `get_article(title)` - Get article by title
- `get_categories(title)` - Get article categories
- `get_links(title)` - Get outgoing links

#### Data Transformation:
```rust
WikiPage -> DataRecord {
    id: format!("wikipedia_{}_{}", language, pageid),
    source: "wikipedia",
    record_type: "article",
    embedding: embed_text(title + extract),
    relationships: [
        {target: category, rel_type: "in_category", weight: 1.0},
        {target: linked_page, rel_type: "links_to", weight: 0.5}
    ]
}
```

---

### Wikidata Client
**Endpoint**: `https://www.wikidata.org/w/api.php`
**SPARQL Endpoint**: `https://query.wikidata.org/sparql`
**Authentication**: Not required
**Rate Limit**: 100ms delay

#### Methods (7):
- `new()` - Initialize client
- `search_entities(query)` - Search Wikidata entities
- `get_entity(qid)` - Get entity by Q-identifier (Q42 = Douglas Adams)
- `sparql_query(query)` - Execute SPARQL query
- `query_climate_entities()` - Predefined climate change query
- `query_pharmaceutical_companies()` - Pharma companies query
- `query_disease_outbreaks()` - Disease outbreaks query

#### Predefined SPARQL Queries (5):
- `CLIMATE_CHANGE` - Climate change entities
- `PHARMACEUTICAL_COMPANIES` - Pharma companies with founding dates, employees
- `DISEASE_OUTBREAKS` - Epidemic events with locations, casualties
- `RESEARCH_INSTITUTIONS` - Research institutes by country
- `NOBEL_LAUREATES` - Nobel Prize winners by field and year

---

## 12. medical_clients.rs - Medical & Health Data

### PubMed Client
**Endpoint**: `https://eutils.ncbi.nlm.nih.gov/entrez/eutils`
**Authentication**: Optional (NCBI API key)
**Rate Limit**:
- Without key: 334ms (~3 req/sec)
- With key: 100ms (10 req/sec)

#### Methods (4):
- `new(api_key: Option<String>)` - Initialize client
- `search_articles(query, max_results)` - Search medical literature
- `search_pmids(query, max_results)` - Get PMIDs only
- `fetch_abstracts(pmids)` - Fetch full abstracts (batches of 200)

#### Data Transformation:
```rust
PubmedArticle -> SemanticVector {
    id: format!("PMID:{}", pmid),
    embedding: embed_text(title + abstract),
    domain: Domain::Medical,
    metadata: {pmid, title, abstract, authors, publication_date},
    embedding_dimension: 384 // Higher for medical text
}
```

---

### ClinicalTrials.gov Client
**Endpoint**: `https://clinicaltrials.gov/api/v2`
**Authentication**: Not required
**Rate Limit**: 100ms delay

#### Methods (2):
- `new()` - Initialize client
- `search_trials(condition, status)` - Search trials by condition and status
  - Status: RECRUITING, COMPLETED, ACTIVE_NOT_RECRUITING, etc.

#### Data Transformation:
```rust
ClinicalStudy -> SemanticVector {
    id: format!("NCT:{}", nct_id),
    embedding: embed_text(title + summary + conditions),
    domain: Domain::Medical,
    metadata: {nct_id, title, summary, conditions, status}
}
```

---

### FDA OpenFDA Client
**Endpoint**: `https://api.fda.gov`
**Authentication**: Not required
**Rate Limit**: 250ms delay (~4 req/sec)

#### Methods (3):
- `new()` - Initialize client
- `search_drug_events(drug_name)` - Search adverse drug events
- `search_recalls(reason)` - Search device recalls

#### Data Transformation:
```rust
FdaDrugEvent -> SemanticVector {
    id: format!("FDA_EVENT:{}", safety_report_id),
    embedding: embed_text("Drug: {drugs} Reactions: {reactions}"),
    domain: Domain::Medical,
    metadata: {report_id, drugs, reactions, serious}
}

FdaRecall -> SemanticVector {
    id: format!("FDA_RECALL:{}", recall_number),
    embedding: embed_text("Product: {product} Reason: {reason}"),
    domain: Domain::Medical,
    metadata: {recall_number, reason, product, classification}
}
```

---

## Common Patterns Across All Clients

### 1. Error Handling Pattern
```rust
async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
    let mut retries = 0;
    loop {
        match self.client.get(url).send().await {
            Ok(response) => {
                if response.status() == StatusCode::TOO_MANY_REQUESTS
                   && retries < MAX_RETRIES {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                    continue;
                }
                return Ok(response);
            }
            Err(_) if retries < MAX_RETRIES => {
                retries += 1;
                sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
            }
            Err(e) => return Err(FrameworkError::Network(e)),
        }
    }
}
```

**Constants**:
- `MAX_RETRIES: u32 = 3`
- `RETRY_DELAY_MS: u64 = 1000`
- Exponential backoff: `delay * retries`

---

### 2. Rate Limiting Pattern
```rust
// Before each API call
sleep(self.rate_limit_delay).await;
let response = self.fetch_with_retry(&url).await?;
```

**Rate Limit Table**:
| Client | Delay (ms) | Req/Sec | Notes |
|--------|-----------|---------|-------|
| News API | 100 | ~10 | Configurable |
| Reddit | 1000 | 1 | 60 req/min limit |
| GitHub | 1000 | 1 | 5000/hr with token |
| HackerNews | 100 | ~10 | No auth required |
| World Bank | 250 | 4 | No auth required |
| FRED | 200 | 5 | API key required |
| Alpha Vantage | 12000 | 0.08 | 5 req/min limit |
| IMF | 500 | 2 | No auth required |
| USPTO | 500 | 2 | No auth required |
| EPO | 1000 | 1 | OAuth2 required |
| Google Patents | 1000 | 1 | Conservative |
| ArXiv | 3000 | 0.33 | 1 req/3sec guideline |
| Semantic Scholar (no key) | 1000 | 1 | 100 req/5min |
| Semantic Scholar (with key) | 100 | 10 | 1000 req/5min |
| bioRxiv/medRxiv | 500 | 2 | No auth required |
| CrossRef | 200 | 5 | Polite pool with email |
| NASA APOD | 1000 | 1 | DEMO_KEY available |
| SpaceX | 500 | 2 | No auth required |
| SIMBAD | 1000 | 1 | TAP service |
| NCBI Gene (no key) | 334 | 3 | NCBI guidelines |
| NCBI Gene (with key) | 100 | 10 | API key required |
| Ensembl | 200 | 5 | 15 req/sec limit |
| UniProt | 200 | 5 | No auth required |
| PDB | 500 | 2 | No auth required |
| USGS | 200 | 5 | Real-time seismic |
| CERN | 500 | 2 | Open data portal |
| Argo | 300 | 3 | Ocean float data |
| Materials Project | 1000 | 1 | 1 req/sec free tier |
| Wikipedia | 100 | ~10 | No auth required |
| Wikidata | 100 | ~10 | SPARQL available |
| PubMed (no key) | 334 | 3 | NCBI guidelines |
| PubMed (with key) | 100 | 10 | API key required |
| ClinicalTrials | 100 | ~10 | No auth required |
| FDA OpenFDA | 250 | 4 | No auth required |

---

### 3. Embedding Pattern
```rust
// SimpleEmbedder - deterministic hash-based embeddings
embedder: Arc<SimpleEmbedder> = Arc::new(SimpleEmbedder::new(dimension));

// Dimensions by domain:
// - 256: Most clients (news, social, research)
// - 384: Medical/scientific (PubMed, ClinicalTrials, FDA)
// - Configurable per client based on text complexity
```

---

### 4. Metadata Pattern
```rust
let mut metadata = HashMap::new();
metadata.insert("source".to_string(), "client_name".to_string());
metadata.insert("id".to_string(), record_id);
// Domain-specific fields
```

**Common Metadata Fields**:
- `source` - Client identifier
- `title` - Record title
- `url` - Source URL
- `timestamp` - Publication/update date
- Domain-specific fields (authors, categories, scores, etc.)

---

## Summary Statistics

### By Domain Coverage
```
News & Social: 4 clients (News API, Reddit, GitHub, HackerNews)
Economic: 4 clients (World Bank, FRED, Alpha Vantage, IMF)
Patents: 3 clients (USPTO, EPO, Google Patents)
Research: 4 clients (ArXiv, Semantic Scholar, bioRxiv, CrossRef)
Space: 3 clients (NASA APOD, SpaceX, SIMBAD)
Genomics: 4 clients (NCBI Gene, Ensembl, UniProt, PDB)
Physics: 4 clients (USGS, CERN, Argo, Materials Project)
Knowledge: 2 clients (Wikipedia, Wikidata)
Medical: 3 clients (PubMed, ClinicalTrials, FDA)
```

### By Authentication Requirements
```
No Auth Required: 17 clients (57%)
Optional Auth: 5 clients (17%) - improved rate limits
Required Auth: 8 clients (26%)
```

### By Method Count
```
Total Public Methods: 150+
Average per client: ~5 methods
Range: 2-7 methods per client
```

### By Rate Limit Strictness
```
Very Strict (>1000ms): 2 clients - ArXiv (3000ms), Alpha Vantage (12000ms)
Strict (500-1000ms): 11 clients
Moderate (200-500ms): 11 clients
Permissive (<200ms): 6 clients
```

### By Embedding Dimensions
```
256 dimensions: 26 clients (87%)
384 dimensions: 4 clients (13%) - medical/scientific domains
```

---

## Data Flow Architecture

```
API Source → Client → Response Parser → SemanticVector/DataRecord
                                              ↓
                                       Embedding (SimpleEmbedder)
                                              ↓
                                       Domain Classification
                                              ↓
                                       Metadata Extraction
                                              ↓
                                       RuVector Storage
```

---

## Usage Recommendations

### 1. Rate Limit Compliance
- Always use provided rate limit delays
- Consider API key registration for higher limits
- Batch requests when possible (e.g., PubMed: 200 PMIDs/request)

### 2. Error Handling
- All clients implement retry logic with exponential backoff
- Handle `FrameworkError::Network` for connectivity issues
- Check for empty results (some APIs return 404 for no matches)

### 3. Authentication
- Store API keys in environment variables
- Use optional auth when available for better rate limits
- OAuth2 clients (Reddit, EPO) require credential management

### 4. Performance Optimization
- Use parallel requests for independent queries
- Leverage batch endpoints (PubMed abstracts, etc.)
- Cache results when appropriate
- Consider semantic search with embeddings vs. full-text search

### 5. Domain-Specific Considerations
- **Medical**: Higher embedding dimensions (384) for richer semantics
- **Research**: Check multiple sources (ArXiv + Semantic Scholar + CrossRef)
- **Economic**: Time-series data requires date range management
- **Genomics**: Species-specific searches (Ensembl supports 100+ species)
- **Physics**: Geographic searches use Haversine distance calculations

---

## Integration Example

```rust
use ruvector_data_framework::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize multiple clients
    let arxiv = ArxivClient::new()?;
    let s2 = SemanticScholarClient::new(Some("API_KEY".to_string()))?;
    let pubmed = PubMedClient::new(Some("NCBI_KEY".to_string()))?;

    // Parallel search across domains
    let query = "machine learning healthcare";

    let (arxiv_results, s2_results, pubmed_results) = tokio::join!(
        arxiv.search(query, 50),
        s2.search_papers(query, 50),
        pubmed.search_articles(query, 50)
    );

    // Combine vectors
    let mut all_vectors = Vec::new();
    all_vectors.extend(arxiv_results?);
    all_vectors.extend(s2_results?);
    all_vectors.extend(pubmed_results?);

    // Store in RuVector for semantic search
    // ... vector storage code ...

    Ok(())
}
```

---

## Future Enhancements

1. **Dynamic Rate Limiting**: Adjust based on response headers
2. **Circuit Breakers**: Fail-fast on repeated errors
3. **Response Caching**: Redis/disk cache for repeated queries
4. **Streaming APIs**: Support for SSE/WebSocket endpoints
5. **Advanced Embeddings**: Integration with transformer models
6. **Relationship Graphs**: Enhanced Wikipedia/Wikidata graph traversal
7. **Multi-language Support**: Expand beyond English for international sources
8. **Specialized Domains**: Climate, energy, agriculture data sources

---

**Last Updated**: 2026-01-04
**Total Clients**: 30
**Total Methods**: 150+
**API Coverage**: 10 domains across research, economic, medical, and scientific data
