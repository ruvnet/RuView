//! Physics, seismic, and ocean data discovery example
//!
//! Demonstrates using USGS, CERN, Argo, and Materials Project clients
//! to discover cross-disciplinary patterns.
//!
//! Run with:
//! ```bash
//! cargo run --example physics_discovery
//! ```

use ruvector_data_framework::{
    ArgoClient, CernOpenDataClient, GeoUtils, MaterialsProjectClient, NativeDiscoveryEngine,
    NativeEngineConfig, UsgsEarthquakeClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌊 Physics, Seismic, and Ocean Data Discovery");
    println!("{}", "=".repeat(60));

    // Initialize discovery engine
    let config = NativeEngineConfig {
        dimension: 256,
        cross_domain: true,
        similarity_threshold: 0.6,
        ..Default::default()
    };
    let mut engine = NativeDiscoveryEngine::new(config);

    // =========================================================================
    // 1. USGS Earthquake Data
    // =========================================================================
    println!("\n📊 Fetching USGS Earthquake Data...");
    let usgs_client = UsgsEarthquakeClient::new()?;

    // Get recent significant earthquakes (magnitude 5.0+, last 7 days)
    match usgs_client.get_recent(5.0, 7).await {
        Ok(earthquakes) => {
            println!("   ✓ Found {} recent earthquakes (mag 5.0+)", earthquakes.len());
            for eq in earthquakes.iter().take(3) {
                let mag = eq.metadata.get("magnitude").map(|s| s.as_str()).unwrap_or("N/A");
                let place = eq.metadata.get("place").map(|s| s.as_str()).unwrap_or("Unknown");
                println!("     - Magnitude {} at {}", mag, place);

                // Add to discovery engine
                let node_id = engine.add_vector(eq.clone());
                println!("       → Added as node {}", node_id);
            }
        }
        Err(e) => println!("   ⚠ Error fetching earthquakes: {}", e),
    }

    // Get regional earthquakes (Southern California)
    println!("\n📍 Searching earthquakes near Los Angeles...");
    match usgs_client.search_by_region(34.05, -118.25, 200.0, 30).await {
        Ok(regional) => {
            println!("   ✓ Found {} earthquakes within 200km", regional.len());
            for eq in regional.iter().take(2) {
                engine.add_vector(eq.clone());
            }
        }
        Err(e) => println!("   ⚠ Error: {}", e),
    }

    // =========================================================================
    // 2. CERN Open Data
    // =========================================================================
    println!("\n⚛️  Fetching CERN Open Data...");
    let cern_client = CernOpenDataClient::new()?;

    // Search for Higgs boson datasets
    match cern_client.search_datasets("Higgs").await {
        Ok(datasets) => {
            println!("   ✓ Found {} Higgs-related datasets", datasets.len());
            for dataset in datasets.iter().take(3) {
                let title = dataset.metadata.get("title").map(|s| s.as_str()).unwrap_or("N/A");
                let experiment = dataset.metadata.get("experiment").map(|s| s.as_str()).unwrap_or("N/A");
                println!("     - {} ({})", title, experiment);

                engine.add_vector(dataset.clone());
            }
        }
        Err(e) => println!("   ⚠ Error fetching CERN data: {}", e),
    }

    // Search CMS experiment data
    println!("\n🔬 Fetching CMS experiment data...");
    match cern_client.search_by_experiment("CMS").await {
        Ok(cms_data) => {
            println!("   ✓ Found {} CMS datasets", cms_data.len());
            for dataset in cms_data.iter().take(2) {
                engine.add_vector(dataset.clone());
            }
        }
        Err(e) => println!("   ⚠ Error: {}", e),
    }

    // =========================================================================
    // 3. Argo Ocean Data (Demo with sample data)
    // =========================================================================
    println!("\n🌊 Creating sample Argo ocean profiles...");
    let argo_client = ArgoClient::new()?;

    // Create sample ocean profiles (real API would fetch from Argo GDAC)
    match argo_client.create_sample_profiles(20) {
        Ok(profiles) => {
            println!("   ✓ Created {} sample ocean profiles", profiles.len());
            for profile in profiles.iter().take(3) {
                let lat = profile.metadata.get("latitude").map(|s| s.as_str()).unwrap_or("N/A");
                let lon = profile.metadata.get("longitude").map(|s| s.as_str()).unwrap_or("N/A");
                let temp = profile.metadata.get("temperature").map(|s| s.as_str()).unwrap_or("N/A");
                println!("     - Ocean at ({}, {}): {}°C", lat, lon, temp);

                engine.add_vector(profile.clone());
            }
        }
        Err(e) => println!("   ⚠ Error: {}", e),
    }

    // =========================================================================
    // 4. Materials Project (requires API key)
    // =========================================================================
    println!("\n🔬 Materials Project Integration (API key required)");
    println!("   Note: Set MATERIALS_PROJECT_API_KEY environment variable");

    if let Ok(api_key) = std::env::var("MATERIALS_PROJECT_API_KEY") {
        let mp_client = MaterialsProjectClient::new(api_key)?;

        // Search for silicon materials
        match mp_client.search_materials("Si").await {
            Ok(materials) => {
                println!("   ✓ Found {} silicon materials", materials.len());
                for material in materials.iter().take(3) {
                    let formula = material.metadata.get("formula").map(|s| s.as_str()).unwrap_or("N/A");
                    let band_gap = material.metadata.get("band_gap").map(|s| s.as_str()).unwrap_or("N/A");
                    println!("     - {} (band gap: {} eV)", formula, band_gap);

                    engine.add_vector(material.clone());
                }
            }
            Err(e) => println!("   ⚠ Error: {}", e),
        }

        // Search for semiconductors (band gap 1-3 eV)
        println!("\n🔋 Searching for semiconductors...");
        match mp_client.search_by_property("band_gap", 1.0, 3.0).await {
            Ok(semiconductors) => {
                println!("   ✓ Found {} semiconductors", semiconductors.len());
                for material in semiconductors.iter().take(2) {
                    engine.add_vector(material.clone());
                }
            }
            Err(e) => println!("   ⚠ Error: {}", e),
        }
    } else {
        println!("   ℹ Skipping Materials Project (no API key)");
        println!("   Get free key at: https://materialsproject.org");
    }

    // =========================================================================
    // 5. Cross-Domain Pattern Discovery
    // =========================================================================
    println!("\n🔍 Discovering Cross-Domain Patterns...");
    println!("{}", "=".repeat(60));

    // Get engine statistics
    let stats = engine.stats();
    println!("\nEngine Statistics:");
    println!("  - Total nodes: {}", stats.total_nodes);
    println!("  - Total edges: {}", stats.total_edges);
    println!("  - Cross-domain edges: {}", stats.cross_domain_edges);
    println!("\nDomain breakdown:");
    for (domain, count) in &stats.domain_counts {
        println!("  - {:?}: {} nodes", domain, count);
    }

    // Compute coherence
    println!("\n📊 Computing Network Coherence...");
    let coherence = engine.compute_coherence();
    println!("  - Min-cut value: {:.3}", coherence.mincut_value);
    println!("  - Partition sizes: {:?}", coherence.partition_sizes);
    println!("  - Boundary nodes: {}", coherence.boundary_nodes.len());
    println!("  - Average edge weight: {:.3}", coherence.avg_edge_weight);

    // Detect patterns
    println!("\n🎯 Detecting Patterns...");
    let patterns = engine.detect_patterns();
    println!("  ✓ Found {} patterns", patterns.len());

    for (i, pattern) in patterns.iter().enumerate() {
        println!("\nPattern {}: {:?}", i + 1, pattern.pattern_type);
        println!("  - Confidence: {:.2}", pattern.confidence);
        println!("  - Description: {}", pattern.description);
        println!("  - Affected nodes: {}", pattern.affected_nodes.len());

        if !pattern.cross_domain_links.is_empty() {
            println!("  - Cross-domain connections:");
            for link in &pattern.cross_domain_links {
                println!(
                    "    → {:?} ↔ {:?} (strength: {:.3})",
                    link.source_domain, link.target_domain, link.link_strength
                );
            }
        }
    }

    // =========================================================================
    // 6. Geographic Utilities Demo
    // =========================================================================
    println!("\n🌍 Geographic Utilities Demo:");
    println!("{}", "=".repeat(60));

    // Calculate distance between two cities
    let nyc = (40.7128, -74.0060);
    let la = (34.0522, -118.2437);
    let distance = GeoUtils::distance_km(nyc.0, nyc.1, la.0, la.1);
    println!("Distance NYC → LA: {:.1} km", distance);

    // Check if point is within radius
    let san_diego = (32.7157, -117.1611);
    let within_500km = GeoUtils::within_radius(la.0, la.1, san_diego.0, san_diego.1, 500.0);
    println!("San Diego within 500km of LA: {}", within_500km);

    // =========================================================================
    // 7. Discovery Use Cases
    // =========================================================================
    println!("\n💡 Potential Discovery Use Cases:");
    println!("{}", "=".repeat(60));
    println!("  1. Earthquake-Climate Correlations");
    println!("     → Link seismic activity with ocean temperature changes");
    println!("\n  2. Materials for Seismic Sensors");
    println!("     → Discover piezoelectric materials optimal for earthquake detection");
    println!("\n  3. Ocean-Particle Physics Patterns");
    println!("     → Correlate ocean neutrino detection with particle collision data");
    println!("\n  4. Cross-Domain Anomaly Detection");
    println!("     → Find simultaneous anomalies across physics, seismic, ocean domains");
    println!("\n  5. Materials-Physics Discovery");
    println!("     → Identify new materials with properties matching particle detector needs");

    println!("\n✅ Discovery pipeline complete!");

    Ok(())
}
