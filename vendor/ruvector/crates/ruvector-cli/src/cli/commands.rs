//! CLI command implementations

use crate::cli::{
    export_csv, export_json, format_error, format_search_results, format_stats, format_success,
    ProgressTracker,
};
use crate::config::Config;
use anyhow::{Context, Result};
use colored::*;
use ruvector_core::{
    types::{DbOptions, SearchQuery, VectorEntry},
    VectorDB,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Create a new database
pub fn create_database(path: &str, dimensions: usize, config: &Config) -> Result<()> {
    let mut db_options = config.to_db_options();
    db_options.storage_path = path.to_string();
    db_options.dimensions = dimensions;

    println!(
        "{}",
        format_success(&format!("Creating database at: {}", path))
    );
    println!("  Dimensions: {}", dimensions.to_string().cyan());
    println!("  Distance metric: {:?}", db_options.distance_metric);

    let _db = VectorDB::new(db_options).context("Failed to create database")?;

    println!("{}", format_success("Database created successfully!"));
    Ok(())
}

/// Insert vectors from a file
pub fn insert_vectors(
    db_path: &str,
    input_file: &str,
    format: &str,
    config: &Config,
    show_progress: bool,
) -> Result<()> {
    // Load database
    let mut db_options = config.to_db_options();
    db_options.storage_path = db_path.to_string();

    let db = VectorDB::new(db_options).context("Failed to open database")?;

    // Parse input file
    let entries = match format {
        "json" => parse_json_file(input_file)?,
        "csv" => parse_csv_file(input_file)?,
        "npy" => parse_npy_file(input_file)?,
        _ => return Err(anyhow::anyhow!("Unsupported format: {}", format)),
    };

    let total = entries.len();
    println!(
        "{}",
        format_success(&format!("Loaded {} vectors from {}", total, input_file))
    );

    // Insert with progress
    let start = Instant::now();
    let tracker = ProgressTracker::new();
    let pb = if show_progress {
        Some(tracker.create_bar(total as u64, "Inserting vectors..."))
    } else {
        None
    };

    let batch_size = config.cli.batch_size;
    let mut inserted = 0;

    for chunk in entries.chunks(batch_size) {
        db.insert_batch(chunk.to_vec())
            .context("Failed to insert batch")?;
        inserted += chunk.len();

        if let Some(ref pb) = pb {
            pb.set_position(inserted as u64);
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message("Insertion complete!");
    }

    let elapsed = start.elapsed();
    println!(
        "{}",
        format_success(&format!(
            "Inserted {} vectors in {:.2}s ({:.0} vectors/sec)",
            total,
            elapsed.as_secs_f64(),
            total as f64 / elapsed.as_secs_f64()
        ))
    );

    Ok(())
}

/// Search for similar vectors
pub fn search_vectors(
    db_path: &str,
    query_vector: Vec<f32>,
    k: usize,
    config: &Config,
    show_vectors: bool,
) -> Result<()> {
    let mut db_options = config.to_db_options();
    db_options.storage_path = db_path.to_string();

    let db = VectorDB::new(db_options).context("Failed to open database")?;

    let start = Instant::now();
    let results = db
        .search(SearchQuery {
            vector: query_vector,
            k,
            filter: None,
            ef_search: None,
        })
        .context("Failed to search")?;

    let elapsed = start.elapsed();

    println!("{}", format_search_results(&results, show_vectors));
    println!(
        "\n{}",
        format!(
            "Search completed in {:.2}ms",
            elapsed.as_secs_f64() * 1000.0
        )
        .dimmed()
    );

    Ok(())
}

/// Show database information
pub fn show_info(db_path: &str, config: &Config) -> Result<()> {
    let mut db_options = config.to_db_options();
    db_options.storage_path = db_path.to_string();

    let db = VectorDB::new(db_options).context("Failed to open database")?;

    let count = db.len().context("Failed to get count")?;
    let dimensions = db.options().dimensions;
    let metric = format!("{:?}", db.options().distance_metric);

    println!("{}", format_stats(count, dimensions, &metric));

    if let Some(hnsw_config) = &db.options().hnsw_config {
        println!("{}", "HNSW Configuration:".bold().green());
        println!("  M: {}", hnsw_config.m.to_string().cyan());
        println!(
            "  ef_construction: {}",
            hnsw_config.ef_construction.to_string().cyan()
        );
        println!("  ef_search: {}", hnsw_config.ef_search.to_string().cyan());
    }

    Ok(())
}

/// Run a quick benchmark
pub fn run_benchmark(db_path: &str, config: &Config, num_queries: usize) -> Result<()> {
    let mut db_options = config.to_db_options();
    db_options.storage_path = db_path.to_string();

    let db = VectorDB::new(db_options).context("Failed to open database")?;

    let dimensions = db.options().dimensions;

    println!("{}", "Running benchmark...".bold().green());
    println!("  Queries: {}", num_queries.to_string().cyan());
    println!("  Dimensions: {}", dimensions.to_string().cyan());

    // Generate random query vectors
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let queries: Vec<Vec<f32>> = (0..num_queries)
        .map(|_| (0..dimensions).map(|_| rng.gen()).collect())
        .collect();

    // Warm-up
    for query in queries.iter().take(10) {
        let _ = db.search(SearchQuery {
            vector: query.clone(),
            k: 10,
            filter: None,
            ef_search: None,
        });
    }

    // Benchmark
    let start = Instant::now();
    for query in &queries {
        db.search(SearchQuery {
            vector: query.clone(),
            k: 10,
            filter: None,
            ef_search: None,
        })
        .context("Search failed")?;
    }
    let elapsed = start.elapsed();

    let qps = num_queries as f64 / elapsed.as_secs_f64();
    let avg_latency = elapsed.as_secs_f64() * 1000.0 / num_queries as f64;

    println!("\n{}", "Benchmark Results:".bold().green());
    println!("  Total time: {:.2}s", elapsed.as_secs_f64());
    println!("  Queries per second: {:.0}", qps.to_string().cyan());
    println!("  Average latency: {:.2}ms", avg_latency.to_string().cyan());

    Ok(())
}

/// Export database to file
pub fn export_database(
    db_path: &str,
    output_file: &str,
    format: &str,
    config: &Config,
) -> Result<()> {
    let mut db_options = config.to_db_options();
    db_options.storage_path = db_path.to_string();

    let db = VectorDB::new(db_options).context("Failed to open database")?;

    println!(
        "{}",
        format_success(&format!("Exporting database to: {}", output_file))
    );

    // Export is currently limited - would need to add all_ids() method to VectorDB
    // For now, return an error with a helpful message
    return Err(anyhow::anyhow!(
        "Export functionality requires VectorDB::all_ids() method. This will be implemented in a future update."
    ));

    // TODO: Implement when VectorDB exposes all_ids()
    // let ids = db.all_ids()?;
    // let tracker = ProgressTracker::new();
    // let pb = tracker.create_bar(ids.len() as u64, "Exporting vectors...");
    // ...
}

/// Import from other vector databases
pub fn import_from_external(
    db_path: &str,
    source: &str,
    source_path: &str,
    config: &Config,
) -> Result<()> {
    println!(
        "{}",
        format_success(&format!("Importing from {} database", source))
    );

    match source {
        "faiss" => {
            // TODO: Implement FAISS import
            return Err(anyhow::anyhow!("FAISS import not yet implemented"));
        }
        "pinecone" => {
            // TODO: Implement Pinecone import
            return Err(anyhow::anyhow!("Pinecone import not yet implemented"));
        }
        "weaviate" => {
            // TODO: Implement Weaviate import
            return Err(anyhow::anyhow!("Weaviate import not yet implemented"));
        }
        _ => return Err(anyhow::anyhow!("Unsupported source: {}", source)),
    }
}

// Helper functions

fn parse_json_file(path: &str) -> Result<Vec<VectorEntry>> {
    let content = std::fs::read_to_string(path).context("Failed to read JSON file")?;
    serde_json::from_str(&content).context("Failed to parse JSON")
}

fn parse_csv_file(path: &str) -> Result<Vec<VectorEntry>> {
    let mut reader = csv::Reader::from_path(path).context("Failed to open CSV file")?;

    let mut entries = Vec::new();

    for result in reader.records() {
        let record = result.context("Failed to read CSV record")?;

        let id = if record.get(0).map(|s| s.is_empty()).unwrap_or(true) {
            None
        } else {
            Some(record.get(0).unwrap().to_string())
        };

        let vector: Vec<f32> =
            serde_json::from_str(record.get(1).context("Missing vector column")?)
                .context("Failed to parse vector")?;

        let metadata = if let Some(meta_str) = record.get(2) {
            if !meta_str.is_empty() {
                Some(serde_json::from_str(meta_str).context("Failed to parse metadata")?)
            } else {
                None
            }
        } else {
            None
        };

        entries.push(VectorEntry {
            id,
            vector,
            metadata,
        });
    }

    Ok(entries)
}

fn parse_npy_file(path: &str) -> Result<Vec<VectorEntry>> {
    use ndarray::Array2;
    use ndarray_npy::ReadNpyExt;

    let file = std::fs::File::open(path).context("Failed to open NPY file")?;
    let array: Array2<f32> = Array2::read_npy(file).context("Failed to read NPY file")?;

    let entries: Vec<VectorEntry> = array
        .outer_iter()
        .enumerate()
        .map(|(i, row)| VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: row.to_vec(),
            metadata: None,
        })
        .collect();

    Ok(entries)
}
