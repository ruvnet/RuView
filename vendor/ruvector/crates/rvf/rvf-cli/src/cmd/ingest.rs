//! `rvf ingest` -- Ingest vectors from a JSON file.

use clap::Args;
use serde::Deserialize;
use std::fs;
use std::path::Path;

use rvf_runtime::RvfStore;

use super::map_rvf_err;

#[derive(Args)]
pub struct IngestArgs {
    /// Path to the RVF store
    path: String,
    /// Path to the JSON input file (array of {id, vector} objects)
    #[arg(short, long)]
    input: String,
    /// Batch size for ingestion
    #[arg(short, long, default_value = "1000")]
    batch_size: usize,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Deserialize)]
struct VectorRecord {
    id: u64,
    vector: Vec<f32>,
}

pub fn run(args: IngestArgs) -> Result<(), Box<dyn std::error::Error>> {
    let json_str = fs::read_to_string(&args.input)?;
    let records: Vec<VectorRecord> = serde_json::from_str(&json_str)?;

    if records.is_empty() {
        if args.json {
            crate::output::print_json(&serde_json::json!({
                "accepted": 0,
                "rejected": 0,
                "epoch": 0,
            }));
        } else {
            println!("No records to ingest.");
        }
        return Ok(());
    }

    let mut store = RvfStore::open(Path::new(&args.path)).map_err(map_rvf_err)?;

    let batch_size = args.batch_size.max(1);
    let mut total_accepted = 0u64;
    let mut total_rejected = 0u64;
    let mut last_epoch = 0u32;

    for chunk in records.chunks(batch_size) {
        let vec_data: Vec<Vec<f32>> = chunk.iter().map(|r| r.vector.clone()).collect();
        let vec_refs: Vec<&[f32]> = vec_data.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = chunk.iter().map(|r| r.id).collect();

        let result = store
            .ingest_batch(&vec_refs, &ids, None)
            .map_err(map_rvf_err)?;
        total_accepted += result.accepted;
        total_rejected += result.rejected;
        last_epoch = result.epoch;
    }

    store.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "accepted": total_accepted,
            "rejected": total_rejected,
            "epoch": last_epoch,
        }));
    } else {
        println!("Ingestion complete:");
        crate::output::print_kv("Accepted:", &total_accepted.to_string());
        crate::output::print_kv("Rejected:", &total_rejected.to_string());
        crate::output::print_kv("Epoch:", &last_epoch.to_string());
    }
    Ok(())
}
