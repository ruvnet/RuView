//! Basic usage example for Ruvector
//!
//! Demonstrates:
//! - Creating a database
//! - Inserting vectors
//! - Searching for similar vectors
//! - Basic configuration

use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DbOptions, Result};

fn main() -> Result<()> {
    println!("🚀 Ruvector Basic Usage Example\n");

    // 1. Create a database
    println!("1. Creating database...");
    let mut options = DbOptions::default();
    options.dimensions = 128;
    options.storage_path = "./examples_basic.db".to_string();

    let db = VectorDB::new(options)?;
    println!("   ✓ Database created with 128 dimensions\n");

    // 2. Insert a single vector
    println!("2. Inserting single vector...");
    let entry = VectorEntry {
        id: Some("doc_001".to_string()),
        vector: vec![0.1; 128],
        metadata: None,
    };

    let id = db.insert(entry)?;
    println!("   ✓ Inserted vector: {}\n", id);

    // 3. Insert multiple vectors
    println!("3. Inserting multiple vectors...");
    let entries: Vec<VectorEntry> = (0..100)
        .map(|i| VectorEntry {
            id: Some(format!("doc_{:03}", i + 2)),
            vector: vec![0.1 + (i as f32) * 0.001; 128],
            metadata: None,
        })
        .collect();

    let ids = db.insert_batch(entries)?;
    println!("   ✓ Inserted {} vectors\n", ids.len());

    // 4. Search for similar vectors
    println!("4. Searching for similar vectors...");
    let query = SearchQuery {
        vector: vec![0.15; 128],
        k: 5,
        filter: None,
        include_vectors: false,
    };

    let results = db.search(&query)?;
    println!("   ✓ Found {} results:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("     {}. ID: {}, Distance: {:.6}",
            i + 1, result.id, result.distance
        );
    }
    println!();

    // 5. Get database stats
    println!("5. Database statistics:");
    let total = db.count();
    println!("   ✓ Total vectors: {}\n", total);

    println!("✅ Example completed successfully!");

    Ok(())
}
