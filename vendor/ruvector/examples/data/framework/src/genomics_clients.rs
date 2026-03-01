//! Genomics and DNA data API integrations for NCBI, UniProt, Ensembl, and GWAS Catalog
//!
//! This module provides async clients for fetching genomics data including genes, proteins,
//! variants, and genome-wide association studies, converting responses to SemanticVector
//! format for RuVector discovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const NCBI_RATE_LIMIT_MS: u64 = 334; // ~3 requests/second without API key
const NCBI_WITH_KEY_RATE_LIMIT_MS: u64 = 100; // 10 requests/second with key
const UNIPROT_RATE_LIMIT_MS: u64 = 100; // Conservative rate limit
const ENSEMBL_RATE_LIMIT_MS: u64 = 67; // 15 requests/second
const GWAS_RATE_LIMIT_MS: u64 = 100; // Conservative rate limit
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// NCBI Entrez Client (Genes, Proteins, Nucleotides, SNPs)
// ============================================================================

/// NCBI ESearch response
#[derive(Debug, Deserialize)]
struct NcbiSearchResponse {
    esearchresult: NcbiSearchResult,
}

#[derive(Debug, Deserialize)]
struct NcbiSearchResult {
    #[serde(default)]
    idlist: Vec<String>,
    #[serde(default)]
    count: String,
}

/// NCBI Gene summary response
#[derive(Debug, Deserialize)]
struct NcbiGeneSummaryResponse {
    result: HashMap<String, NcbiGeneSummary>,
}

#[derive(Debug, Deserialize)]
struct NcbiGeneSummary {
    #[serde(default)]
    uid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    organism: NcbiOrganism,
    #[serde(default)]
    chromosome: String,
    #[serde(default)]
    maplocation: String,
}

#[derive(Debug, Deserialize, Default)]
struct NcbiOrganism {
    #[serde(default)]
    scientificname: String,
    #[serde(default)]
    commonname: String,
}

/// NCBI SNP docsum response
#[derive(Debug, Deserialize)]
struct NcbiSnpResponse {
    result: HashMap<String, NcbiSnpSummary>,
}

#[derive(Debug, Deserialize)]
struct NcbiSnpSummary {
    #[serde(default)]
    uid: String,
    #[serde(default)]
    snp_id: String,
    #[serde(default)]
    genes: Vec<NcbiGene>,
    #[serde(default)]
    chr: String,
    #[serde(default)]
    chrpos: String,
    #[serde(default)]
    fxn_class: String,
}

#[derive(Debug, Deserialize)]
struct NcbiGene {
    #[serde(default)]
    name: String,
}

/// Client for NCBI Entrez APIs (genes, proteins, nucleotides, SNPs)
pub struct NcbiClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl NcbiClient {
    /// Create a new NCBI Entrez client
    ///
    /// # Arguments
    /// * `api_key` - Optional NCBI API key (get from https://www.ncbi.nlm.nih.gov/account/)
    ///   Without a key: 3 requests/second
    ///   With a key: 10 requests/second
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/1.0 (genomics discovery)")
            .build()
            .map_err(FrameworkError::Network)?;

        let rate_limit_delay = if api_key.is_some() {
            Duration::from_millis(NCBI_WITH_KEY_RATE_LIMIT_MS)
        } else {
            Duration::from_millis(NCBI_RATE_LIMIT_MS)
        };

        Ok(Self {
            client,
            base_url: "https://eutils.ncbi.nlm.nih.gov/entrez/eutils".to_string(),
            api_key,
            rate_limit_delay,
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search gene database
    ///
    /// # Arguments
    /// * `query` - Search query (e.g., "BRCA1", "alzheimer's disease")
    /// * `organism` - Optional organism filter (e.g., "human", "mouse")
    pub async fn search_genes(
        &self,
        query: &str,
        organism: Option<&str>,
    ) -> Result<Vec<SemanticVector>> {
        let mut search_query = query.to_string();
        if let Some(org) = organism {
            search_query.push_str(&format!(" AND {}[Organism]", org));
        }

        let gene_ids = self.search_database("gene", &search_query, 100).await?;
        if gene_ids.is_empty() {
            return Ok(Vec::new());
        }

        self.fetch_gene_summaries(&gene_ids).await
    }

    /// Get gene details by gene ID
    pub async fn get_gene(&self, gene_id: &str) -> Result<Option<SemanticVector>> {
        let vectors = self.fetch_gene_summaries(&[gene_id.to_string()]).await?;
        Ok(vectors.into_iter().next())
    }

    /// Search protein database
    pub async fn search_proteins(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let protein_ids = self.search_database("protein", query, 100).await?;
        if protein_ids.is_empty() {
            return Ok(Vec::new());
        }

        self.fetch_protein_summaries(&protein_ids).await
    }

    /// Search nucleotide sequences
    pub async fn search_nucleotide(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let seq_ids = self.search_database("nucleotide", query, 100).await?;
        if seq_ids.is_empty() {
            return Ok(Vec::new());
        }

        self.fetch_nucleotide_summaries(&seq_ids).await
    }

    /// Get SNP/variant information by rsID
    ///
    /// # Arguments
    /// * `rsid` - SNP reference ID (e.g., "rs429358" for APOE4)
    pub async fn get_snp(&self, rsid: &str) -> Result<Option<SemanticVector>> {
        let clean_rsid = rsid.trim_start_matches("rs");
        let snp_ids = self.search_database("snp", clean_rsid, 1).await?;

        if snp_ids.is_empty() {
            return Ok(None);
        }

        let vectors = self.fetch_snp_summaries(&snp_ids).await?;
        Ok(vectors.into_iter().next())
    }

    /// Search any NCBI database
    async fn search_database(
        &self,
        db: &str,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<String>> {
        let mut url = format!(
            "{}/esearch.fcgi?db={}&term={}&retmode=json&retmax={}",
            self.base_url,
            db,
            urlencoding::encode(query),
            max_results
        );

        if let Some(key) = &self.api_key {
            url.push_str(&format!("&api_key={}", key));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: NcbiSearchResponse = response.json().await?;

        Ok(search_response.esearchresult.idlist)
    }

    /// Fetch gene summaries
    async fn fetch_gene_summaries(&self, gene_ids: &[String]) -> Result<Vec<SemanticVector>> {
        if gene_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_vectors = Vec::new();

        for chunk in gene_ids.chunks(200) {
            let id_list = chunk.join(",");
            let mut url = format!(
                "{}/esummary.fcgi?db=gene&id={}&retmode=json",
                self.base_url, id_list
            );

            if let Some(key) = &self.api_key {
                url.push_str(&format!("&api_key={}", key));
            }

            sleep(self.rate_limit_delay).await;
            let response = self.fetch_with_retry(&url).await?;
            let summary_response: NcbiGeneSummaryResponse = response.json().await?;

            for (id, summary) in summary_response.result {
                if id == "uids" {
                    continue; // Skip metadata entry
                }

                let description = if !summary.summary.is_empty() {
                    summary.summary.clone()
                } else {
                    summary.description.clone()
                };

                let text = format!(
                    "{} {} {}",
                    summary.name, description, summary.organism.scientificname
                );
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("gene_id".to_string(), summary.uid.clone());
                metadata.insert("symbol".to_string(), summary.name.clone());
                metadata.insert("description".to_string(), description);
                metadata.insert("organism".to_string(), summary.organism.scientificname);
                metadata.insert("common_name".to_string(), summary.organism.commonname);
                metadata.insert("chromosome".to_string(), summary.chromosome);
                metadata.insert("location".to_string(), summary.maplocation);
                metadata.insert("source".to_string(), "ncbi_gene".to_string());

                all_vectors.push(SemanticVector {
                    id: format!("GENE:{}", summary.uid),
                    embedding,
                    domain: Domain::Genomics,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(all_vectors)
    }

    /// Fetch protein summaries (simplified)
    async fn fetch_protein_summaries(&self, protein_ids: &[String]) -> Result<Vec<SemanticVector>> {
        // For proteins, we use a simplified approach with just IDs
        // In production, you'd parse full protein records
        let mut vectors = Vec::new();

        for id in protein_ids {
            let text = format!("Protein {}", id);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("protein_id".to_string(), id.clone());
            metadata.insert("source".to_string(), "ncbi_protein".to_string());

            vectors.push(SemanticVector {
                id: format!("PROTEIN:{}", id),
                embedding,
                domain: Domain::Genomics,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Fetch nucleotide summaries (simplified)
    async fn fetch_nucleotide_summaries(&self, seq_ids: &[String]) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for id in seq_ids {
            let text = format!("Nucleotide sequence {}", id);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("sequence_id".to_string(), id.clone());
            metadata.insert("source".to_string(), "ncbi_nucleotide".to_string());

            vectors.push(SemanticVector {
                id: format!("NUCLEOTIDE:{}", id),
                embedding,
                domain: Domain::Genomics,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Fetch SNP summaries
    async fn fetch_snp_summaries(&self, snp_ids: &[String]) -> Result<Vec<SemanticVector>> {
        if snp_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_vectors = Vec::new();

        for chunk in snp_ids.chunks(200) {
            let id_list = chunk.join(",");
            let mut url = format!(
                "{}/esummary.fcgi?db=snp&id={}&retmode=json",
                self.base_url, id_list
            );

            if let Some(key) = &self.api_key {
                url.push_str(&format!("&api_key={}", key));
            }

            sleep(self.rate_limit_delay).await;
            let response = self.fetch_with_retry(&url).await?;
            let snp_response: NcbiSnpResponse = response.json().await?;

            for (id, summary) in snp_response.result {
                if id == "uids" {
                    continue;
                }

                let gene_names: Vec<String> = summary.genes.iter()
                    .map(|g| g.name.clone())
                    .collect();

                let text = format!(
                    "SNP rs{} chromosome {} position {} function {} genes {}",
                    summary.snp_id,
                    summary.chr,
                    summary.chrpos,
                    summary.fxn_class,
                    gene_names.join(",")
                );
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("rsid".to_string(), format!("rs{}", summary.snp_id));
                metadata.insert("chromosome".to_string(), summary.chr);
                metadata.insert("position".to_string(), summary.chrpos);
                metadata.insert("function".to_string(), summary.fxn_class);
                metadata.insert("genes".to_string(), gene_names.join(", "));
                metadata.insert("source".to_string(), "ncbi_snp".to_string());

                all_vectors.push(SemanticVector {
                    id: format!("SNP:rs{}", summary.snp_id),
                    embedding,
                    domain: Domain::Genomics,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(all_vectors)
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

// ============================================================================
// UniProt Client (Protein Database)
// ============================================================================

/// UniProt search response
#[derive(Debug, Deserialize)]
struct UniProtSearchResponse {
    results: Vec<UniProtEntry>,
}

#[derive(Debug, Deserialize)]
struct UniProtEntry {
    #[serde(rename = "primaryAccession")]
    primary_accession: String,
    #[serde(default)]
    organism: Option<UniProtOrganism>,
    #[serde(rename = "proteinDescription", default)]
    protein_description: Option<UniProtDescription>,
    #[serde(default)]
    genes: Vec<UniProtGene>,
    #[serde(default)]
    comments: Vec<UniProtComment>,
}

#[derive(Debug, Deserialize)]
struct UniProtOrganism {
    #[serde(rename = "scientificName", default)]
    scientific_name: String,
}

#[derive(Debug, Deserialize)]
struct UniProtDescription {
    #[serde(rename = "recommendedName", default)]
    recommended_name: Option<UniProtName>,
}

#[derive(Debug, Deserialize)]
struct UniProtName {
    #[serde(rename = "fullName", default)]
    full_name: Option<UniProtValue>,
}

#[derive(Debug, Deserialize)]
struct UniProtValue {
    #[serde(default)]
    value: String,
}

#[derive(Debug, Deserialize)]
struct UniProtGene {
    #[serde(rename = "geneName", default)]
    gene_name: Option<UniProtValue>,
}

#[derive(Debug, Deserialize)]
struct UniProtComment {
    #[serde(rename = "commentType", default)]
    comment_type: String,
    #[serde(default)]
    texts: Vec<UniProtValue>,
}

/// Client for UniProt protein database
pub struct UniProtClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl UniProtClient {
    /// Create a new UniProt client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/1.0 (genomics discovery)")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://rest.uniprot.org/uniprotkb".to_string(),
            rate_limit_delay: Duration::from_millis(UNIPROT_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search proteins
    ///
    /// # Arguments
    /// * `query` - Search query (e.g., "kinase", "p53")
    /// * `limit` - Maximum results (default 100)
    pub async fn search_proteins(&self, query: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/search?query={}&format=json&size={}",
            self.base_url,
            urlencoding::encode(query),
            limit.min(500)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: UniProtSearchResponse = response.json().await?;

        let mut vectors = Vec::new();
        for entry in search_response.results {
            vectors.push(self.entry_to_vector(entry)?);
        }

        Ok(vectors)
    }

    /// Get protein by accession ID
    pub async fn get_protein(&self, accession: &str) -> Result<Option<SemanticVector>> {
        let url = format!("{}/{}.json", self.base_url, accession);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let entry: UniProtEntry = response.json().await?;
        Ok(Some(self.entry_to_vector(entry)?))
    }

    /// Search proteins by organism
    pub async fn search_by_organism(&self, organism: &str) -> Result<Vec<SemanticVector>> {
        let query = format!("organism:{}", organism);
        self.search_proteins(&query, 100).await
    }

    /// Search proteins by GO term/function
    pub async fn search_by_function(&self, function: &str) -> Result<Vec<SemanticVector>> {
        let query = format!("cc:{}", function); // cc = cellular component, also use mf/bp
        self.search_proteins(&query, 100).await
    }

    /// Convert UniProt entry to SemanticVector
    fn entry_to_vector(&self, entry: UniProtEntry) -> Result<SemanticVector> {
        let protein_name = entry
            .protein_description
            .as_ref()
            .and_then(|pd| pd.recommended_name.as_ref())
            .and_then(|rn| rn.full_name.as_ref())
            .map(|fn_| fn_.value.clone())
            .unwrap_or_else(|| "Unnamed protein".to_string());

        let organism = entry
            .organism
            .as_ref()
            .map(|o| o.scientific_name.clone())
            .unwrap_or_default();

        let gene_names: Vec<String> = entry
            .genes
            .iter()
            .filter_map(|g| g.gene_name.as_ref().map(|gn| gn.value.clone()))
            .collect();

        // Extract function comments
        let function_text = entry
            .comments
            .iter()
            .filter(|c| c.comment_type == "FUNCTION")
            .flat_map(|c| c.texts.iter().map(|t| t.value.clone()))
            .collect::<Vec<_>>()
            .join(" ");

        let text = format!(
            "{} {} {} {}",
            protein_name, organism, gene_names.join(","), function_text
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("accession".to_string(), entry.primary_accession.clone());
        metadata.insert("protein_name".to_string(), protein_name);
        metadata.insert("organism".to_string(), organism);
        metadata.insert("genes".to_string(), gene_names.join(", "));
        metadata.insert("function".to_string(), function_text);
        metadata.insert("source".to_string(), "uniprot".to_string());

        Ok(SemanticVector {
            id: format!("UNIPROT:{}", entry.primary_accession),
            embedding,
            domain: Domain::Genomics,
            timestamp: Utc::now(),
            metadata,
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for UniProtClient {
    fn default() -> Self {
        Self::new().expect("Failed to create UniProt client")
    }
}

// ============================================================================
// Ensembl REST Client (Gene Info, Variants, Homologs)
// ============================================================================

/// Ensembl gene response
#[derive(Debug, Deserialize)]
struct EnsemblGene {
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    species: String,
    #[serde(default)]
    biotype: String,
    #[serde(default)]
    seq_region_name: String,
    #[serde(default)]
    start: i64,
    #[serde(default)]
    end: i64,
}

/// Ensembl variant response
#[derive(Debug, Deserialize)]
struct EnsemblVariant {
    #[serde(default)]
    id: String,
    #[serde(default)]
    seq_region_name: String,
    #[serde(default)]
    start: i64,
    #[serde(default)]
    most_severe_consequence: String,
}

/// Ensembl homology response
#[derive(Debug, Deserialize)]
struct EnsemblHomologyResponse {
    #[serde(default)]
    data: Vec<EnsemblHomology>,
}

#[derive(Debug, Deserialize)]
struct EnsemblHomology {
    #[serde(default)]
    homologies: Vec<EnsemblHomologyEntry>,
}

#[derive(Debug, Deserialize)]
struct EnsemblHomologyEntry {
    #[serde(default)]
    target: EnsemblTarget,
    #[serde(rename = "type", default)]
    homology_type: String,
}

#[derive(Debug, Deserialize, Default)]
struct EnsemblTarget {
    #[serde(default)]
    id: String,
    #[serde(default)]
    species: String,
    #[serde(default)]
    protein_id: String,
}

/// Client for Ensembl REST API
pub struct EnsemblClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl EnsemblClient {
    /// Create a new Ensembl client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/1.0 (genomics discovery)")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://rest.ensembl.org".to_string(),
            rate_limit_delay: Duration::from_millis(ENSEMBL_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Get gene information
    ///
    /// # Arguments
    /// * `gene_id` - Ensembl gene ID (e.g., "ENSG00000157764" for BRAF)
    pub async fn get_gene_info(&self, gene_id: &str) -> Result<Option<SemanticVector>> {
        let url = format!("{}/lookup/id/{}?content-type=application/json", self.base_url, gene_id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let gene: EnsemblGene = response.json().await?;

        let text = format!(
            "{} {} {} {}",
            gene.display_name, gene.description, gene.species, gene.biotype
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("ensembl_id".to_string(), gene.id.clone());
        metadata.insert("symbol".to_string(), gene.display_name);
        metadata.insert("description".to_string(), gene.description);
        metadata.insert("species".to_string(), gene.species);
        metadata.insert("biotype".to_string(), gene.biotype);
        metadata.insert("chromosome".to_string(), gene.seq_region_name);
        metadata.insert("start".to_string(), gene.start.to_string());
        metadata.insert("end".to_string(), gene.end.to_string());
        metadata.insert("source".to_string(), "ensembl".to_string());

        Ok(Some(SemanticVector {
            id: format!("ENSEMBL:{}", gene.id),
            embedding,
            domain: Domain::Genomics,
            timestamp: Utc::now(),
            metadata,
        }))
    }

    /// Get genetic variants for a gene
    pub async fn get_variants(&self, gene_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/overlap/id/{}?feature=variation;content-type=application/json",
            self.base_url, gene_id
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let variants: Vec<EnsemblVariant> = response.json().await?;

        let mut vectors = Vec::new();
        for variant in variants.into_iter().take(100) {
            let text = format!(
                "Variant {} chromosome {} position {} consequence {}",
                variant.id, variant.seq_region_name, variant.start, variant.most_severe_consequence
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("variant_id".to_string(), variant.id.clone());
            metadata.insert("chromosome".to_string(), variant.seq_region_name);
            metadata.insert("position".to_string(), variant.start.to_string());
            metadata.insert("consequence".to_string(), variant.most_severe_consequence);
            metadata.insert("gene_id".to_string(), gene_id.to_string());
            metadata.insert("source".to_string(), "ensembl_variant".to_string());

            vectors.push(SemanticVector {
                id: format!("VARIANT:{}", variant.id),
                embedding,
                domain: Domain::Genomics,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get homologous genes across species
    pub async fn get_homologs(&self, gene_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/homology/id/{}?content-type=application/json;format=condensed",
            self.base_url, gene_id
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let homology_response: EnsemblHomologyResponse = response.json().await?;

        let mut vectors = Vec::new();
        for data in homology_response.data {
            for homology in data.homologies {
                let text = format!(
                    "Homolog {} in {} type {}",
                    homology.target.id, homology.target.species, homology.homology_type
                );
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("homolog_id".to_string(), homology.target.id.clone());
                metadata.insert("species".to_string(), homology.target.species);
                metadata.insert("protein_id".to_string(), homology.target.protein_id);
                metadata.insert("homology_type".to_string(), homology.homology_type);
                metadata.insert("source_gene".to_string(), gene_id.to_string());
                metadata.insert("source".to_string(), "ensembl_homology".to_string());

                vectors.push(SemanticVector {
                    id: format!("HOMOLOG:{}:{}", gene_id, homology.target.id),
                    embedding,
                    domain: Domain::Genomics,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for EnsemblClient {
    fn default() -> Self {
        Self::new().expect("Failed to create Ensembl client")
    }
}

// ============================================================================
// GWAS Catalog Client (EBI)
// ============================================================================

/// GWAS association response
#[derive(Debug, Deserialize)]
struct GwasAssociationResponse {
    #[serde(rename = "_embedded")]
    embedded: Option<GwasEmbedded>,
}

#[derive(Debug, Deserialize)]
struct GwasEmbedded {
    #[serde(default)]
    associations: Vec<GwasAssociation>,
}

#[derive(Debug, Deserialize)]
struct GwasAssociation {
    #[serde(default)]
    riskAllele: String,
    #[serde(default)]
    pvalue: f64,
    #[serde(default, rename = "trait")]
    trait_name: String,
    #[serde(default)]
    chromosomeName: String,
    #[serde(default)]
    chromosomePosition: i64,
    #[serde(default)]
    loci: Vec<GwasLocus>,
}

#[derive(Debug, Deserialize)]
struct GwasLocus {
    #[serde(default)]
    authorReportedGene: String,
}

/// GWAS study response
#[derive(Debug, Deserialize)]
struct GwasStudyResponse {
    #[serde(rename = "_embedded")]
    embedded: Option<GwasStudyEmbedded>,
}

#[derive(Debug, Deserialize)]
struct GwasStudyEmbedded {
    #[serde(default)]
    studies: Vec<GwasStudy>,
}

#[derive(Debug, Deserialize)]
struct GwasStudy {
    #[serde(default)]
    accessionId: String,
    #[serde(default)]
    publicationDate: Option<String>,
    #[serde(default)]
    diseaseTrait: String,
    #[serde(default)]
    initialSampleSize: String,
}

/// Client for GWAS Catalog (EBI)
pub struct GwasClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl GwasClient {
    /// Create a new GWAS Catalog client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/1.0 (genomics discovery)")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://www.ebi.ac.uk/gwas/rest/api".to_string(),
            rate_limit_delay: Duration::from_millis(GWAS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search trait-gene associations
    ///
    /// # Arguments
    /// * `trait_name` - Disease or trait name (e.g., "diabetes", "height")
    pub async fn search_associations(&self, trait_name: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/associations/search/findByEfoTrait?efoTrait={}&size=100",
            self.base_url,
            urlencoding::encode(trait_name)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let assoc_response: GwasAssociationResponse = response.json().await?;

        let mut vectors = Vec::new();
        if let Some(embedded) = assoc_response.embedded {
            for assoc in embedded.associations {
                let genes: Vec<String> = assoc.loci.iter()
                    .map(|l| l.authorReportedGene.clone())
                    .collect();

                let text = format!(
                    "GWAS association trait {} genes {} chromosome {} position {} p-value {}",
                    assoc.trait_name,
                    genes.join(","),
                    assoc.chromosomeName,
                    assoc.chromosomePosition,
                    assoc.pvalue
                );
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("trait".to_string(), assoc.trait_name.clone());
                metadata.insert("genes".to_string(), genes.join(", "));
                metadata.insert("risk_allele".to_string(), assoc.riskAllele.clone());
                metadata.insert("pvalue".to_string(), assoc.pvalue.to_string());
                metadata.insert("chromosome".to_string(), assoc.chromosomeName.clone());
                metadata.insert("position".to_string(), assoc.chromosomePosition.to_string());
                metadata.insert("source".to_string(), "gwas_catalog".to_string());

                vectors.push(SemanticVector {
                    id: format!("GWAS:{}_{}_{}", assoc.chromosomeName, assoc.chromosomePosition, assoc.pvalue),
                    embedding,
                    domain: Domain::Genomics,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Get study details
    pub async fn get_study(&self, study_id: &str) -> Result<Option<SemanticVector>> {
        let url = format!("{}/studies/{}", self.base_url, study_id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let study: GwasStudy = response.json().await?;

        let text = format!(
            "GWAS study {} trait {} sample size {}",
            study.accessionId, study.diseaseTrait, study.initialSampleSize
        );
        let embedding = self.embedder.embed_text(&text);

        let timestamp = study
            .publicationDate
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut metadata = HashMap::new();
        metadata.insert("study_id".to_string(), study.accessionId.clone());
        metadata.insert("trait".to_string(), study.diseaseTrait);
        metadata.insert("sample_size".to_string(), study.initialSampleSize);
        metadata.insert("source".to_string(), "gwas_study".to_string());

        Ok(Some(SemanticVector {
            id: format!("GWAS_STUDY:{}", study.accessionId),
            embedding,
            domain: Domain::Genomics,
            timestamp,
            metadata,
        }))
    }

    /// Search associations by gene
    pub async fn search_by_gene(&self, gene: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/associations/search/findByGene?geneName={}&size=100",
            self.base_url,
            urlencoding::encode(gene)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let assoc_response: GwasAssociationResponse = response.json().await?;

        let mut vectors = Vec::new();
        if let Some(embedded) = assoc_response.embedded {
            for assoc in embedded.associations {
                let text = format!(
                    "Gene {} associated with trait {} p-value {}",
                    gene, assoc.trait_name, assoc.pvalue
                );
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("gene".to_string(), gene.to_string());
                metadata.insert("trait".to_string(), assoc.trait_name.clone());
                metadata.insert("pvalue".to_string(), assoc.pvalue.to_string());
                metadata.insert("chromosome".to_string(), assoc.chromosomeName.clone());
                metadata.insert("source".to_string(), "gwas_gene_association".to_string());

                vectors.push(SemanticVector {
                    id: format!("GWAS_GENE:{}:{}", gene, assoc.trait_name),
                    embedding,
                    domain: Domain::Genomics,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries))).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for GwasClient {
    fn default() -> Self {
        Self::new().expect("Failed to create GWAS client")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ncbi_client_creation() {
        let client = NcbiClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_ncbi_rate_limiting() {
        let without_key = NcbiClient::new(None).unwrap();
        assert_eq!(
            without_key.rate_limit_delay,
            Duration::from_millis(NCBI_RATE_LIMIT_MS)
        );

        let with_key = NcbiClient::new(Some("test_key".to_string())).unwrap();
        assert_eq!(
            with_key.rate_limit_delay,
            Duration::from_millis(NCBI_WITH_KEY_RATE_LIMIT_MS)
        );
    }

    #[tokio::test]
    async fn test_uniprot_client_creation() {
        let client = UniProtClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_ensembl_client_creation() {
        let client = EnsemblClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_gwas_client_creation() {
        let client = GwasClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_genomics_domain() {
        // Ensure Domain::Genomics is available
        let _domain = Domain::Genomics;
    }
}
