//! Genomics Data Discovery Example
//!
//! This example demonstrates how to use the genomics API clients to fetch
//! gene, protein, variant, and GWAS data for cross-domain discovery with
//! climate and medical data.
//!
//! Run with:
//! ```bash
//! cargo run --example genomics_discovery
//! ```

use ruvector_data_framework::{
    EnsemblClient, GwasClient, NcbiClient, NativeDiscoveryEngine, NativeEngineConfig,
    UniProtClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the discovery engine
    let config = NativeEngineConfig::default();
    let mut engine = NativeDiscoveryEngine::new(config);

    println!("🧬 Genomics Data Discovery Example\n");
    println!("{}", "=".repeat(80));

    // ========================================================================
    // Example 1: Search for BRCA1 gene (breast cancer gene)
    // ========================================================================
    println!("\n📌 Example 1: Searching for BRCA1 gene (breast cancer susceptibility)");
    println!("{}", "-".repeat(80));

    let ncbi_client = NcbiClient::new(None)?;
    println!("Searching NCBI for BRCA1...");
    let brca1_genes = ncbi_client.search_genes("BRCA1", Some("human")).await?;

    for gene in &brca1_genes {
        println!("  ✓ Found gene: {}", gene.id);
        println!("    Symbol: {}", gene.metadata.get("symbol").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Description: {}", gene.metadata.get("description").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Chromosome: {}", gene.metadata.get("chromosome").map(|s| s.as_str()).unwrap_or("N/A"));

        // Add to discovery engine
        engine.add_vector(gene.clone());
    }

    // ========================================================================
    // Example 2: Search for climate-related stress response genes
    // ========================================================================
    println!("\n📌 Example 2: Searching for heat shock proteins (climate adaptation)");
    println!("{}", "-".repeat(80));

    println!("Searching for heat shock proteins...");
    let hsp_genes = ncbi_client.search_genes("heat shock protein", Some("human")).await?;

    for (i, gene) in hsp_genes.iter().take(5).enumerate() {
        println!("  ✓ [{}/5] {}", i + 1, gene.id);
        println!("    Symbol: {}", gene.metadata.get("symbol").map(|s| s.as_str()).unwrap_or("N/A"));

        // Add to discovery engine
        engine.add_vector(gene.clone());
    }

    // ========================================================================
    // Example 3: Search UniProt for APOE protein (Alzheimer's risk)
    // ========================================================================
    println!("\n📌 Example 3: Searching UniProt for APOE protein");
    println!("{}", "-".repeat(80));

    let uniprot_client = UniProtClient::new()?;
    println!("Searching for APOE protein...");
    let apoe_proteins = uniprot_client.search_proteins("APOE", 5).await?;

    for protein in &apoe_proteins {
        println!("  ✓ Protein: {}", protein.id);
        println!("    Name: {}", protein.metadata.get("protein_name").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Function: {}...",
            protein.metadata.get("function")
                .map(|s| s.as_str()).unwrap_or("N/A")
                .chars()
                .take(80)
                .collect::<String>()
        );

        // Add to discovery engine
        engine.add_vector(protein.clone());
    }

    // ========================================================================
    // Example 4: Get SNP information for APOE4 variant (rs429358)
    // ========================================================================
    println!("\n📌 Example 4: Looking up APOE4 SNP (rs429358)");
    println!("{}", "-".repeat(80));

    if let Some(snp) = ncbi_client.get_snp("rs429358").await? {
        println!("  ✓ SNP: {}", snp.id);
        println!("    Chromosome: {}", snp.metadata.get("chromosome").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Position: {}", snp.metadata.get("position").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Associated genes: {}", snp.metadata.get("genes").map(|s| s.as_str()).unwrap_or("N/A"));

        // Add to discovery engine
        engine.add_vector(snp);
    } else {
        println!("  ✗ SNP not found");
    }

    // ========================================================================
    // Example 5: Get Ensembl gene information and variants
    // ========================================================================
    println!("\n📌 Example 5: Querying Ensembl for BRAF gene (cancer gene)");
    println!("{}", "-".repeat(80));

    let ensembl_client = EnsemblClient::new()?;
    let braf_id = "ENSG00000157764"; // BRAF gene

    if let Some(gene) = ensembl_client.get_gene_info(braf_id).await? {
        println!("  ✓ Gene: {}", gene.id);
        println!("    Symbol: {}", gene.metadata.get("symbol").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Description: {}", gene.metadata.get("description").map(|s| s.as_str()).unwrap_or("N/A"));

        engine.add_vector(gene);

        // Get variants for this gene
        println!("\n  Fetching genetic variants for BRAF...");
        let variants = ensembl_client.get_variants(braf_id).await?;
        println!("  ✓ Found {} variants", variants.len());

        for variant in variants.iter().take(3) {
            println!("    - {} (consequence: {})",
                variant.id,
                variant.metadata.get("consequence").map(|s| s.as_str()).unwrap_or("unknown")
            );
            engine.add_vector(variant.clone());
        }
    }

    // ========================================================================
    // Example 6: Search GWAS Catalog for diabetes associations
    // ========================================================================
    println!("\n📌 Example 6: Searching GWAS Catalog for diabetes associations");
    println!("{}", "-".repeat(80));

    let gwas_client = GwasClient::new()?;
    println!("Searching for type 2 diabetes associations...");
    let diabetes_assocs = gwas_client.search_associations("diabetes").await?;

    for (i, assoc) in diabetes_assocs.iter().take(5).enumerate() {
        println!("  ✓ [{}/5] Association:", i + 1);
        println!("    Trait: {}", assoc.metadata.get("trait").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    Genes: {}", assoc.metadata.get("genes").map(|s| s.as_str()).unwrap_or("N/A"));
        println!("    P-value: {}", assoc.metadata.get("pvalue").map(|s| s.as_str()).unwrap_or("N/A"));

        engine.add_vector(assoc.clone());
    }

    // ========================================================================
    // Example 7: Cross-domain discovery - Climate + Genomics
    // ========================================================================
    println!("\n📌 Example 7: Cross-Domain Pattern Detection");
    println!("{}", "-".repeat(80));

    // Compute coherence
    let coherence = engine.compute_coherence();
    println!("\n🔍 Discovery Engine Stats:");
    println!("  Nodes: {}", coherence.node_count);
    println!("  Edges: {}", coherence.edge_count);
    println!("  Min-cut value: {:.4}", coherence.mincut_value);
    println!("  Avg edge weight: {:.4}", coherence.avg_edge_weight);

    // Detect patterns
    let patterns = engine.detect_patterns();
    println!("\n🎯 Detected {} patterns", patterns.len());

    for (i, pattern) in patterns.iter().enumerate() {
        println!("\n  Pattern {}: {:?}", i + 1, pattern.pattern_type);
        println!("    Confidence: {:.2}", pattern.confidence);
        println!("    Description: {}", pattern.description);
        println!("    Affected nodes: {}", pattern.affected_nodes.len());

        if !pattern.cross_domain_links.is_empty() {
            println!("    Cross-domain links:");
            for link in &pattern.cross_domain_links {
                println!("      - {:?} ↔ {:?} (strength: {:.2})",
                    link.source_domain,
                    link.target_domain,
                    link.link_strength
                );
            }
        }
    }

    // ========================================================================
    // Example 8: Potential Discoveries
    // ========================================================================
    println!("\n📌 Example 8: Potential Cross-Domain Discoveries");
    println!("{}", "-".repeat(80));
    println!("\nThis framework enables discoveries like:");
    println!("  🌡️  Climate ↔ Genomics:");
    println!("     • Heat shock protein expression correlates with temperature data");
    println!("     • UV radiation exposure linked to skin cancer gene mutations");
    println!("     • Seasonal variations affect metabolic gene expression\n");

    println!("  💊 Medical ↔ Genomics:");
    println!("     • Drug response variants in CYP450 genes");
    println!("     • Disease risk alleles (BRCA1/2, APOE4)");
    println!("     • Pharmacogenomic interactions\n");

    println!("  📊 Economic ↔ Genomics:");
    println!("     • Healthcare costs correlated with genetic disease burden");
    println!("     • Agricultural productivity and crop stress response genes");
    println!("     • Biotech market trends and genomic research output\n");

    println!("\n✅ Genomics discovery example completed!");
    println!("{}", "=".repeat(80));

    Ok(())
}
