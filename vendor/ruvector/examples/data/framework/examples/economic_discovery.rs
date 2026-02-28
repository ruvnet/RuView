//! Economic Data Discovery Example
//!
//! This example demonstrates using the FRED, World Bank, and Alpha Vantage clients
//! to discover patterns in economic data using RuVector's discovery framework.
//!
//! Run with:
//! ```bash
//! cargo run --example economic_discovery
//! ```

use ruvector_data_framework::{
    AlphaVantageClient, FredClient, NativeDiscoveryEngine, NativeEngineConfig, Result,
    WorldBankClient,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🏦 Economic Data Discovery with RuVector\n");
    println!("=========================================\n");

    // ========================================================================
    // 1. FRED Client - US Economic Indicators
    // ========================================================================
    println!("📊 Fetching FRED economic indicators...\n");

    let fred_client = FredClient::new(None)?;

    // Get US GDP
    println!("  • Fetching US GDP data...");
    let gdp_vectors = fred_client.get_gdp().await?;
    println!("    ✓ Retrieved {} GDP observations", gdp_vectors.len());

    // Get unemployment rate
    println!("  • Fetching unemployment rate...");
    let unemployment_vectors = fred_client.get_unemployment().await?;
    println!("    ✓ Retrieved {} unemployment observations", unemployment_vectors.len());

    // Get CPI (inflation indicator)
    println!("  • Fetching CPI (inflation)...");
    let cpi_vectors = fred_client.get_cpi().await?;
    println!("    ✓ Retrieved {} CPI observations", cpi_vectors.len());

    // Get interest rates
    println!("  • Fetching Federal Funds Rate...");
    let interest_vectors = fred_client.get_interest_rate().await?;
    println!("    ✓ Retrieved {} interest rate observations", interest_vectors.len());

    // Search for specific economic series
    println!("  • Searching for 'housing price' series...");
    let housing_search = fred_client.search_series("housing price").await?;
    println!("    ✓ Found {} related series", housing_search.len());

    println!("\n  Total FRED vectors: {}\n",
        gdp_vectors.len() + unemployment_vectors.len() + cpi_vectors.len() + interest_vectors.len());

    // ========================================================================
    // 2. World Bank Client - Global Development Data
    // ========================================================================
    println!("🌍 Fetching World Bank global indicators...\n");

    let wb_client = WorldBankClient::new()?;

    // Get global GDP per capita
    println!("  • Fetching global GDP per capita...");
    let global_gdp = wb_client.get_gdp_global().await?;
    println!("    ✓ Retrieved {} country-year observations", global_gdp.len());

    // Get climate indicators
    println!("  • Fetching climate indicators (CO2, renewable energy)...");
    let climate_indicators = wb_client.get_climate_indicators().await?;
    println!("    ✓ Retrieved {} climate observations", climate_indicators.len());

    // Get health indicators
    println!("  • Fetching health expenditure indicators...");
    let health_indicators = wb_client.get_health_indicators().await?;
    println!("    ✓ Retrieved {} health observations", health_indicators.len());

    // Get population data
    println!("  • Fetching global population data...");
    let population = wb_client.get_population().await?;
    println!("    ✓ Retrieved {} population observations", population.len());

    // Get specific indicator for a country
    println!("  • Fetching US GDP per capita...");
    let us_gdp = wb_client.get_indicator("USA", "NY.GDP.PCAP.CD").await?;
    println!("    ✓ Retrieved {} US GDP per capita observations", us_gdp.len());

    println!("\n  Total World Bank vectors: {}\n",
        global_gdp.len() + climate_indicators.len() + health_indicators.len() + population.len());

    // ========================================================================
    // 3. Alpha Vantage Client - Stock Market Data (Optional)
    // ========================================================================
    println!("📈 Stock Market Data (Alpha Vantage)...\n");

    // Note: Requires API key from https://www.alphavantage.co/support/#api-key
    let av_api_key = std::env::var("ALPHAVANTAGE_API_KEY").ok();

    if let Some(api_key) = av_api_key {
        let av_client = AlphaVantageClient::new(api_key)?;

        println!("  • Fetching AAPL stock data...");
        let aapl_vectors = av_client.get_daily_stock("AAPL").await?;
        println!("    ✓ Retrieved {} daily price observations", aapl_vectors.len());

        println!("  • Fetching MSFT stock data...");
        let msft_vectors = av_client.get_daily_stock("MSFT").await?;
        println!("    ✓ Retrieved {} daily price observations", msft_vectors.len());

        println!("\n  Total stock market vectors: {}\n", aapl_vectors.len() + msft_vectors.len());
    } else {
        println!("  ⚠ Skipped (set ALPHAVANTAGE_API_KEY to enable)\n");
    }

    // ========================================================================
    // 4. Pattern Discovery with RuVector
    // ========================================================================
    println!("🔍 Discovering patterns in economic data...\n");

    let config = NativeEngineConfig {
        similarity_threshold: 0.6,
        mincut_sensitivity: 0.2,
        cross_domain: true,
        ..Default::default()
    };

    let mut engine = NativeDiscoveryEngine::new(config);

    // Add all FRED vectors
    println!("  • Adding FRED economic indicators to discovery engine...");
    let mut total_nodes = 0;
    for vector in gdp_vectors.iter().take(20)
        .chain(unemployment_vectors.iter().take(20))
        .chain(cpi_vectors.iter().take(20))
        .chain(interest_vectors.iter().take(20))
    {
        engine.add_vector(vector.clone());
        total_nodes += 1;
    }
    println!("    ✓ Added {} FRED nodes", total_nodes);

    // Add sample World Bank vectors
    println!("  • Adding World Bank indicators to discovery engine...");
    let mut wb_nodes = 0;
    for vector in global_gdp.iter().take(30)
        .chain(climate_indicators.iter().take(20))
    {
        engine.add_vector(vector.clone());
        wb_nodes += 1;
    }
    println!("    ✓ Added {} World Bank nodes", wb_nodes);

    // Compute initial coherence
    println!("\n  • Computing network coherence...");
    let coherence = engine.compute_coherence();
    println!("    ✓ Min-cut value: {:.3}", coherence.mincut_value);
    println!("    ✓ Network: {} nodes, {} edges", coherence.node_count, coherence.edge_count);
    println!("    ✓ Partition sizes: {} vs {}", coherence.partition_sizes.0, coherence.partition_sizes.1);

    // Detect patterns
    println!("\n  • Detecting economic patterns...");
    let patterns = engine.detect_patterns();
    println!("    ✓ Found {} patterns", patterns.len());

    for (i, pattern) in patterns.iter().enumerate() {
        println!("\n    Pattern {} ({:?}):", i + 1, pattern.pattern_type);
        println!("      Confidence: {:.2}", pattern.confidence);
        println!("      Description: {}", pattern.description);
        println!("      Affected nodes: {}", pattern.affected_nodes.len());

        if !pattern.cross_domain_links.is_empty() {
            println!("      Cross-domain connections:");
            for link in &pattern.cross_domain_links {
                println!("        → {:?} ↔ {:?} (strength: {:.3})",
                    link.source_domain, link.target_domain, link.link_strength);
            }
        }
    }

    // Display engine statistics
    println!("\n📊 Discovery Engine Statistics:");
    println!("─────────────────────────────────");
    let stats = engine.stats();
    println!("  Total nodes:        {}", stats.total_nodes);
    println!("  Total edges:        {}", stats.total_edges);
    println!("  Total vectors:      {}", stats.total_vectors);
    println!("  Cross-domain edges: {}", stats.cross_domain_edges);
    println!("  History length:     {}", stats.history_length);

    println!("\n  Domain distribution:");
    for (domain, count) in &stats.domain_counts {
        println!("    {:?}: {}", domain, count);
    }

    println!("\n✅ Economic discovery complete!\n");

    // ========================================================================
    // 5. Example Use Cases
    // ========================================================================
    println!("💡 Example Use Cases:");
    println!("─────────────────────");
    println!("  1. Correlation Analysis:");
    println!("     Discover relationships between GDP, unemployment, and inflation");
    println!();
    println!("  2. Cross-Domain Discovery:");
    println!("     Find connections between US economic indicators and global climate data");
    println!();
    println!("  3. Economic Forecasting:");
    println!("     Use historical patterns to predict future economic trends");
    println!();
    println!("  4. Market Intelligence:");
    println!("     Combine stock prices with economic indicators for trading signals");
    println!();
    println!("  5. Policy Impact Analysis:");
    println!("     Measure how economic policies affect multiple indicators over time");

    println!("\n📚 API Key Resources:");
    println!("─────────────────────");
    println!("  • FRED API (optional for higher limits):");
    println!("    https://fred.stlouisfed.org/docs/api/api_key.html");
    println!();
    println!("  • Alpha Vantage (free tier - 5 calls/min):");
    println!("    https://www.alphavantage.co/support/#api-key");
    println!();
    println!("  • World Bank Open Data (no key required):");
    println!("    https://datahelpdesk.worldbank.org/knowledgebase/articles/889392");

    Ok(())
}
