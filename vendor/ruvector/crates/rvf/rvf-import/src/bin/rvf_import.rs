//! CLI binary for importing data into RVF stores.
//!
//! Usage examples:
//!   rvf-import --format json --input vectors.json --output data.rvf --dimension 384
//!   rvf-import --format csv  --input data.csv   --output data.rvf --id-column 0 --vector-start 1
//!   rvf-import --format npy  --input embeddings.npy --output data.rvf

use clap::Parser;
use rvf_import::progress::StderrProgress;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "rvf-import", about = "Import vectors into an RVF store")]
struct Cli {
    /// Input format: json, csv, tsv, or npy.
    #[arg(long)]
    format: String,

    /// Path to the input file.
    #[arg(long)]
    input: PathBuf,

    /// Path to the output .rvf file (will be created).
    #[arg(long)]
    output: PathBuf,

    /// Vector dimension. Required for json/csv; auto-detected for npy.
    #[arg(long)]
    dimension: Option<u16>,

    /// (CSV) Column index for the vector ID (0-based, default 0).
    #[arg(long, default_value_t = 0)]
    id_column: usize,

    /// (CSV) Column index where vector components start (0-based, default 1).
    #[arg(long, default_value_t = 1)]
    vector_start: usize,

    /// (CSV) Disable header row detection.
    #[arg(long)]
    no_header: bool,

    /// (NPY) Starting ID for auto-assigned vector IDs.
    #[arg(long, default_value_t = 0)]
    start_id: u64,

    /// Batch size for ingestion (default 1000).
    #[arg(long, default_value_t = 1000)]
    batch_size: usize,

    /// Suppress progress output.
    #[arg(long)]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();

    let records = match cli.format.as_str() {
        "json" => match rvf_import::json::parse_json_file(&cli.input) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        "csv" => {
            let config = rvf_import::csv_import::CsvConfig {
                id_column: cli.id_column,
                vector_start: cli.vector_start,
                delimiter: b',',
                has_header: !cli.no_header,
                dimension: cli.dimension.map(|d| d as usize),
            };
            match rvf_import::csv_import::parse_csv_file(&cli.input, &config) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        "tsv" => {
            let config = rvf_import::csv_import::CsvConfig {
                id_column: cli.id_column,
                vector_start: cli.vector_start,
                delimiter: b'\t',
                has_header: !cli.no_header,
                dimension: cli.dimension.map(|d| d as usize),
            };
            match rvf_import::csv_import::parse_csv_file(&cli.input, &config) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        "npy" => {
            let config = rvf_import::numpy::NpyConfig {
                start_id: cli.start_id,
            };
            match rvf_import::numpy::parse_npy_file(&cli.input, &config) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        other => {
            eprintln!("error: unknown format '{other}'. Use: json, csv, tsv, npy");
            process::exit(1);
        }
    };

    if records.is_empty() {
        eprintln!("warning: no records parsed from input file");
        process::exit(0);
    }

    // Determine dimension
    let dimension = match cli.dimension {
        Some(d) => d,
        None => {
            let inferred = records[0].vector.len() as u16;
            if inferred == 0 {
                eprintln!("error: cannot infer dimension (first vector is empty). Use --dimension");
                process::exit(1);
            }
            eprintln!("info: inferred dimension = {inferred} from first record");
            inferred
        }
    };

    let progress: Option<&dyn rvf_import::progress::ProgressReporter> = if cli.quiet {
        None
    } else {
        Some(&StderrProgress)
    };

    match rvf_import::import_to_new_store(
        &cli.output,
        dimension,
        &records,
        cli.batch_size,
        progress,
    ) {
        Ok(result) => {
            if !cli.quiet {
                eprintln!();
            }
            eprintln!(
                "done: imported {} vectors, rejected {}, in {} batches -> {}",
                result.total_imported,
                result.total_rejected,
                result.batches,
                cli.output.display()
            );
        }
        Err(e) => {
            eprintln!("\nerror: import failed: {e}");
            process::exit(1);
        }
    }
}
