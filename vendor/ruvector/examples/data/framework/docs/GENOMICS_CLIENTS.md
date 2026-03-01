# Genomics and DNA Data API Clients

Comprehensive genomics data integration for RuVector's discovery framework, enabling cross-domain pattern detection between genomics, climate, medical, and economic data.

## Overview

The genomics clients module (`genomics_clients.rs`) provides four specialized API clients for accessing the world's largest genomics databases:

1. **NcbiClient** - NCBI Entrez APIs (genes, proteins, nucleotides, SNPs)
2. **UniProtClient** - UniProt protein knowledge base
3. **EnsemblClient** - Ensembl genomic annotations
4. **GwasClient** - GWAS Catalog (genome-wide association studies)

All data is automatically converted to `SemanticVector` format with `Domain::Genomics` for seamless integration with RuVector's vector database and coherence analysis.

## Features

- ✅ **Rate limiting** with exponential backoff (NCBI: 3 req/s without key, 10 req/s with key)
- ✅ **Retry logic** with configurable attempts
- ✅ **NCBI API key support** for higher rate limits
- ✅ **Automatic embedding generation** using SimpleEmbedder (384 dimensions)
- ✅ **Semantic vector conversion** with rich metadata
- ✅ **Cross-domain discovery** enabled (Genomics ↔ Climate, Medical, Economic)
- ✅ **Unit tests** for all clients

## Installation

The genomics clients are included in the `ruvector-data-framework` crate:

```toml
[dependencies]
ruvector-data-framework = "0.1.0"
```

## Quick Start

```rust
use ruvector_data_framework::{
    NcbiClient, UniProtClient, EnsemblClient, GwasClient,
    NativeDiscoveryEngine, NativeEngineConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize discovery engine
    let mut engine = NativeDiscoveryEngine::new(NativeEngineConfig::default());

    // 1. Search for genes related to climate adaptation
    let ncbi = NcbiClient::new(None)?;
    let heat_shock_genes = ncbi.search_genes("heat shock protein", Some("human")).await?;

    for gene in heat_shock_genes {
        engine.add_vector(gene);
    }

    // 2. Search for disease-associated proteins
    let uniprot = UniProtClient::new()?;
    let apoe_proteins = uniprot.search_proteins("APOE", 10).await?;

    for protein in apoe_proteins {
        engine.add_vector(protein);
    }

    // 3. Get genetic variants
    let ensembl = EnsemblClient::new()?;
    if let Some(gene) = ensembl.get_gene_info("ENSG00000157764").await? {
        engine.add_vector(gene);
        let variants = ensembl.get_variants("ENSG00000157764").await?;
        for variant in variants {
            engine.add_vector(variant);
        }
    }

    // 4. Search GWAS for disease associations
    let gwas = GwasClient::new()?;
    let diabetes_assocs = gwas.search_associations("diabetes").await?;

    for assoc in diabetes_assocs {
        engine.add_vector(assoc);
    }

    // Detect cross-domain patterns
    let patterns = engine.detect_patterns();
    println!("Discovered {} patterns", patterns.len());

    Ok(())
}
```

## API Clients

### 1. NcbiClient - NCBI Entrez APIs

Access genes, proteins, nucleotides, and SNPs from NCBI databases.

#### Initialization

```rust
// Without API key (3 requests/second)
let client = NcbiClient::new(None)?;

// With API key (10 requests/second) - recommended
let client = NcbiClient::new(Some("YOUR_API_KEY".to_string()))?;
```

Get your API key at: https://www.ncbi.nlm.nih.gov/account/

#### Methods

```rust
// Search gene database
let genes = client.search_genes("BRCA1", Some("human")).await?;

// Get specific gene by ID
let gene = client.get_gene("672").await?;

// Search proteins
let proteins = client.search_proteins("kinase").await?;

// Search nucleotide sequences
let sequences = client.search_nucleotide("mitochondrial genome").await?;

// Get SNP information by rsID
let snp = client.get_snp("rs429358").await?; // APOE4 variant
```

#### Vector Format

```rust
SemanticVector {
    id: "GENE:672",
    domain: Domain::Genomics,
    embedding: [384-dimensional vector],
    metadata: {
        "gene_id": "672",
        "symbol": "BRCA1",
        "description": "BRCA1 DNA repair associated",
        "organism": "Homo sapiens",
        "common_name": "human",
        "chromosome": "17",
        "location": "17q21.31",
        "source": "ncbi_gene"
    }
}
```

### 2. UniProtClient - Protein Database

Access comprehensive protein information including function, structure, and pathways.

#### Initialization

```rust
let client = UniProtClient::new()?;
```

#### Methods

```rust
// Search proteins
let proteins = client.search_proteins("p53", 100).await?;

// Get protein by accession
let protein = client.get_protein("P04637").await?; // TP53

// Search by organism
let human_proteins = client.search_by_organism("human").await?;

// Search by function (GO term)
let kinases = client.search_by_function("kinase").await?;
```

#### Vector Format

```rust
SemanticVector {
    id: "UNIPROT:P04637",
    domain: Domain::Genomics,
    embedding: [384-dimensional vector],
    metadata: {
        "accession": "P04637",
        "protein_name": "Cellular tumor antigen p53",
        "organism": "Homo sapiens",
        "genes": "TP53",
        "function": "Acts as a tumor suppressor...",
        "source": "uniprot"
    }
}
```

### 3. EnsemblClient - Genomic Annotations

Access gene information, variants, and homology across species.

#### Initialization

```rust
let client = EnsemblClient::new()?;
```

#### Methods

```rust
// Get gene information
let gene = client.get_gene_info("ENSG00000157764").await?; // BRAF

// Get genetic variants for a gene
let variants = client.get_variants("ENSG00000157764").await?;

// Get homologous genes across species
let homologs = client.get_homologs("ENSG00000157764").await?;
```

#### Vector Format

```rust
SemanticVector {
    id: "ENSEMBL:ENSG00000157764",
    domain: Domain::Genomics,
    embedding: [384-dimensional vector],
    metadata: {
        "ensembl_id": "ENSG00000157764",
        "symbol": "BRAF",
        "description": "B-Raf proto-oncogene, serine/threonine kinase",
        "species": "homo_sapiens",
        "biotype": "protein_coding",
        "chromosome": "7",
        "start": "140719327",
        "end": "140924929",
        "source": "ensembl"
    }
}
```

### 4. GwasClient - GWAS Catalog

Access genome-wide association studies linking genes to diseases and traits.

#### Initialization

```rust
let client = GwasClient::new()?;
```

#### Methods

```rust
// Search trait-gene associations
let associations = client.search_associations("diabetes").await?;

// Get study details
let study = client.get_study("GCST001937").await?;

// Search associations by gene
let gene_assocs = client.search_by_gene("APOE").await?;
```

#### Vector Format

```rust
SemanticVector {
    id: "GWAS:7_140753336_5.0e-8",
    domain: Domain::Genomics,
    embedding: [384-dimensional vector],
    metadata: {
        "trait": "Type 2 diabetes",
        "genes": "BRAF, KIAA1549",
        "risk_allele": "rs7578597-T",
        "pvalue": "5.0e-8",
        "chromosome": "7",
        "position": "140753336",
        "source": "gwas_catalog"
    }
}
```

## Rate Limits

| API | Default Rate | With API Key | Notes |
|-----|-------------|--------------|-------|
| NCBI | 3 req/sec | 10 req/sec | API key recommended for production |
| UniProt | 10 req/sec | - | Conservative limit |
| Ensembl | 15 req/sec | - | Per their guidelines |
| GWAS | 10 req/sec | - | Conservative limit |

All clients implement:
- Automatic rate limiting with delays
- Exponential backoff on 429 errors
- Configurable retry attempts (default: 3)

## Cross-Domain Discovery Examples

### 1. Climate ↔ Genomics

Discover how environmental factors correlate with gene expression:

```rust
// Fetch heat shock proteins (climate stress response)
let hsp_genes = ncbi.search_genes("heat shock protein", Some("human")).await?;

// Fetch temperature data from NOAA
let climate_data = noaa_client.fetch_temperature_data("2020-01-01", "2024-01-01").await?;

// Add to discovery engine
for gene in hsp_genes {
    engine.add_vector(gene);
}
for record in climate_data {
    engine.add_vector(record);
}

// Detect cross-domain patterns
let patterns = engine.detect_patterns();
// May discover: "Heat shock protein expression correlates with extreme temperature events"
```

### 2. Medical ↔ Genomics

Link genetic variants to disease outcomes:

```rust
// Get APOE4 variant (Alzheimer's risk)
let apoe4 = ncbi.get_snp("rs429358").await?;

// Search PubMed for Alzheimer's research
let papers = pubmed.search_articles("Alzheimer's disease APOE", 100).await?;

// Detect gene-disease associations
let patterns = engine.detect_patterns();
```

### 3. Economic ↔ Genomics

Correlate biotech market trends with genomic research:

```rust
// Fetch CRISPR-related genes
let crispr_genes = ncbi.search_genes("CRISPR", None).await?;

// Fetch biotech stock data
let biotech_stocks = alpha_vantage.fetch_stock("CRSP", "monthly").await?;

// Discover market-science correlations
let patterns = engine.detect_patterns();
```

## Error Handling

All clients return `Result<T, FrameworkError>`:

```rust
match ncbi.search_genes("BRCA1", Some("human")).await {
    Ok(genes) => {
        println!("Found {} genes", genes.len());
        for gene in genes {
            engine.add_vector(gene);
        }
    }
    Err(FrameworkError::Network(e)) => {
        eprintln!("Network error: {}", e);
    }
    Err(FrameworkError::Serialization(e)) => {
        eprintln!("JSON parsing error: {}", e);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Testing

Run the unit tests:

```bash
cargo test --lib genomics
```

Run the example:

```bash
cargo run --example genomics_discovery
```

## Performance Tips

1. **Use NCBI API key** for production workloads (10x rate limit)
2. **Batch operations** when possible (e.g., fetch 200 genes at once)
3. **Cache results** to avoid redundant API calls
4. **Use async/await** for concurrent requests across different APIs

```rust
// Concurrent fetching
let (genes, proteins, variants) = tokio::join!(
    ncbi.search_genes("BRCA1", Some("human")),
    uniprot.search_proteins("BRCA1", 10),
    ensembl.get_variants("ENSG00000012048")
);
```

## Real-World Use Cases

### 1. Pharmacogenomics

Discover drug-gene interactions:
- Fetch CYP450 genes from NCBI
- Get protein structures from UniProt
- Find drug adverse events from FDA
- Detect patterns linking gene variants to drug response

### 2. Climate Adaptation Research

Study genetic adaptation to climate change:
- Fetch stress response genes (heat shock, cold tolerance)
- Get climate data (temperature, precipitation)
- Find GWAS associations for environmental traits
- Discover gene-environment correlations

### 3. Disease Risk Assessment

Build genetic risk profiles:
- Get disease-associated SNPs from GWAS
- Fetch gene function from UniProt
- Find variants from Ensembl
- Compute polygenic risk scores

## Contributing

When adding new genomics data sources:

1. Follow the existing client pattern (rate limiting, retry logic)
2. Convert to `SemanticVector` with `Domain::Genomics`
3. Include rich metadata for discovery
4. Add unit tests
5. Update this documentation

## References

- [NCBI Entrez API](https://www.ncbi.nlm.nih.gov/books/NBK25501/)
- [UniProt REST API](https://www.uniprot.org/help/api)
- [Ensembl REST API](https://rest.ensembl.org/)
- [GWAS Catalog API](https://www.ebi.ac.uk/gwas/rest/docs/api)

## License

Part of the RuVector project. See root LICENSE file.
