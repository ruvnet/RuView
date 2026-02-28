//! Geospatial API Client Demo
//!
//! Demonstrates usage of all geospatial mapping clients:
//! - NominatimClient (OpenStreetMap geocoding)
//! - OverpassClient (OSM data queries)
//! - GeonamesClient (place name database)
//! - OpenElevationClient (elevation data)
//!
//! Run with: cargo run --example geospatial_demo

use ruvector_data_framework::{
    GeonamesClient, NominatimClient, OpenElevationClient, OverpassClient,
    GeoUtils, Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== RuVector Geospatial API Client Demo ===\n");

    // 1. Nominatim Geocoding Demo
    println!("1. NOMINATIM GEOCODING (OpenStreetMap)");
    println!("   Rate limit: 1 request/second (STRICT)\n");

    demo_nominatim().await?;

    println!("\n{}\n", "=".repeat(60));

    // 2. Overpass API Demo
    println!("2. OVERPASS API (OSM Data Queries)");
    println!("   Rate limit: ~2 requests/second\n");

    demo_overpass().await?;

    println!("\n{}\n", "=".repeat(60));

    // 3. GeoNames Demo
    println!("3. GEONAMES (Place Name Database)");
    println!("   Rate limit: ~0.5 requests/second (free tier)\n");
    println!("   NOTE: Requires GEONAMES_USERNAME env var\n");

    if let Ok(username) = std::env::var("GEONAMES_USERNAME") {
        demo_geonames(&username).await?;
    } else {
        println!("   Skipping GeoNames demo - set GEONAMES_USERNAME env var");
    }

    println!("\n{}\n", "=".repeat(60));

    // 4. Open Elevation Demo
    println!("4. OPEN ELEVATION API");
    println!("   Rate limit: ~5 requests/second\n");

    demo_open_elevation().await?;

    println!("\n{}\n", "=".repeat(60));

    // 5. Geographic Distance Calculations
    println!("5. GEOGRAPHIC UTILITIES");
    println!("   Distance calculations using Haversine formula\n");

    demo_geo_utils();

    Ok(())
}

async fn demo_nominatim() -> Result<()> {
    let client = NominatimClient::new()?;

    // Geocoding: Address to coordinates
    println!("   Geocoding: 'Eiffel Tower, Paris'");
    match client.geocode("Eiffel Tower, Paris").await {
        Ok(results) => {
            if let Some(result) = results.first() {
                println!("   ✓ Found: {}", result.id);
                println!("     - Lat: {}", result.metadata.get("latitude").unwrap_or(&"N/A".to_string()));
                println!("     - Lon: {}", result.metadata.get("longitude").unwrap_or(&"N/A".to_string()));
                println!("     - Display: {}", result.metadata.get("display_name").unwrap_or(&"N/A".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Reverse geocoding: Coordinates to address
    println!("\n   Reverse Geocoding: (40.7128, -74.0060) [NYC]");
    match client.reverse_geocode(40.7128, -74.0060).await {
        Ok(results) => {
            if let Some(result) = results.first() {
                println!("   ✓ Found: {}", result.metadata.get("display_name").unwrap_or(&"N/A".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Place search
    println!("\n   Search: 'Times Square' (limit 3)");
    match client.search("Times Square", 3).await {
        Ok(results) => {
            println!("   ✓ Found {} results", results.len());
            for (i, result) in results.iter().take(3).enumerate() {
                println!("     {}. {}", i + 1, result.metadata.get("display_name").unwrap_or(&"N/A".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    Ok(())
}

async fn demo_overpass() -> Result<()> {
    let client = OverpassClient::new()?;

    // Find nearby cafes in Paris
    println!("   Finding cafes near Eiffel Tower (48.8584, 2.2945, 500m radius)");
    match client.get_nearby_pois(48.8584, 2.2945, 500.0, "cafe").await {
        Ok(results) => {
            println!("   ✓ Found {} cafes", results.len());
            for (i, result) in results.iter().take(5).enumerate() {
                println!("     {}. {}", i + 1, result.metadata.get("name").unwrap_or(&"Unnamed".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Get roads in a bounding box
    println!("\n   Getting roads in small area of Paris");
    match client.get_roads(48.85, 2.29, 48.86, 2.30).await {
        Ok(results) => {
            println!("   ✓ Found {} road segments", results.len());
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    Ok(())
}

async fn demo_geonames(username: &str) -> Result<()> {
    let client = GeonamesClient::new(username.to_string())?;

    // Search for places
    println!("   Searching for 'London' (limit 5)");
    match client.search("London", 5).await {
        Ok(results) => {
            println!("   ✓ Found {} results", results.len());
            for (i, result) in results.iter().enumerate() {
                println!("     {}. {} ({}, population: {})",
                    i + 1,
                    result.metadata.get("name").unwrap_or(&"N/A".to_string()),
                    result.metadata.get("country_name").unwrap_or(&"N/A".to_string()),
                    result.metadata.get("population").unwrap_or(&"0".to_string())
                );
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Get nearby places
    println!("\n   Finding nearby places to (51.5074, -0.1278) [London]");
    match client.get_nearby(51.5074, -0.1278).await {
        Ok(results) => {
            println!("   ✓ Found {} nearby places", results.len());
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Get timezone
    println!("\n   Getting timezone for (40.7128, -74.0060) [NYC]");
    match client.get_timezone(40.7128, -74.0060).await {
        Ok(results) => {
            if let Some(result) = results.first() {
                println!("   ✓ Timezone: {}", result.metadata.get("timezone_id").unwrap_or(&"N/A".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Get country info
    println!("\n   Getting country info for 'US'");
    match client.get_country_info("US").await {
        Ok(results) => {
            if let Some(result) = results.first() {
                println!("   ✓ Country: {}", result.metadata.get("country_name").unwrap_or(&"N/A".to_string()));
                println!("     - Capital: {}", result.metadata.get("capital").unwrap_or(&"N/A".to_string()));
                println!("     - Population: {}", result.metadata.get("population").unwrap_or(&"0".to_string()));
                println!("     - Area: {} sq km", result.metadata.get("area_sq_km").unwrap_or(&"0".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    Ok(())
}

async fn demo_open_elevation() -> Result<()> {
    let client = OpenElevationClient::new()?;

    // Single point elevation
    println!("   Getting elevation for Mount Everest base (27.9881, 86.9250)");
    match client.get_elevation(27.9881, 86.9250).await {
        Ok(results) => {
            if let Some(result) = results.first() {
                println!("   ✓ Elevation: {} meters", result.metadata.get("elevation_m").unwrap_or(&"N/A".to_string()));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Batch elevation lookup
    println!("\n   Getting elevations for multiple cities:");
    let locations = vec![
        (40.7128, -74.0060),  // NYC
        (48.8566, 2.3522),    // Paris
        (35.6762, 139.6503),  // Tokyo
        (-33.8688, 151.2093), // Sydney
    ];

    match client.get_elevations(locations).await {
        Ok(results) => {
            let cities = ["NYC", "Paris", "Tokyo", "Sydney"];
            println!("   ✓ Found {} elevations", results.len());
            for (i, result) in results.iter().enumerate() {
                if i < cities.len() {
                    println!("     - {}: {} meters",
                        cities[i],
                        result.metadata.get("elevation_m").unwrap_or(&"N/A".to_string())
                    );
                }
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    Ok(())
}

fn demo_geo_utils() {
    // Distance calculations
    println!("   Calculating distances between major cities:\n");

    let cities = vec![
        ("New York", 40.7128, -74.0060),
        ("London", 51.5074, -0.1278),
        ("Tokyo", 35.6762, 139.6503),
        ("Sydney", -33.8688, 151.2093),
        ("Paris", 48.8566, 2.3522),
    ];

    // Calculate distance from NYC to other cities
    let (nyc_name, nyc_lat, nyc_lon) = cities[0];
    println!("   Distances from {}:", nyc_name);

    for (name, lat, lon) in &cities[1..] {
        let distance = GeoUtils::distance_km(nyc_lat, nyc_lon, *lat, *lon);
        println!("     → {}: {:.2} km", name, distance);
    }

    // Check if points are within radius
    println!("\n   Checking if cities are within 2000km of Paris:");
    let (paris_name, paris_lat, paris_lon) = cities[4];

    for (name, lat, lon) in &cities {
        if *name == paris_name {
            continue;
        }

        let within = GeoUtils::within_radius(paris_lat, paris_lon, *lat, *lon, 2000.0);
        let distance = GeoUtils::distance_km(paris_lat, paris_lon, *lat, *lon);
        println!("     {} ({:.2} km): {}", name, distance, if within { "✓" } else { "✗" });
    }
}
