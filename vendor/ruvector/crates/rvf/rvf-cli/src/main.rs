use clap::{Parser, Subcommand};
use std::process;

mod cmd;
mod output;

#[derive(Parser)]
#[command(name = "rvf", version, about = "RuVector Format CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new empty RVF store
    Create(cmd::create::CreateArgs),
    /// Ingest vectors from a JSON file
    Ingest(cmd::ingest::IngestArgs),
    /// Query nearest neighbors
    Query(cmd::query::QueryArgs),
    /// Delete vectors by ID or filter
    Delete(cmd::delete::DeleteArgs),
    /// Show store status
    Status(cmd::status::StatusArgs),
    /// Inspect segments and lineage
    Inspect(cmd::inspect::InspectArgs),
    /// Compact to reclaim dead space
    Compact(cmd::compact::CompactArgs),
    /// Derive a child store from a parent
    Derive(cmd::derive::DeriveArgs),
    /// Start HTTP server (requires 'serve' feature)
    Serve(cmd::serve::ServeArgs),
    /// Boot RVF in QEMU microVM
    Launch(cmd::launch::LaunchArgs),
    /// Embed a kernel image into an RVF file
    EmbedKernel(cmd::embed_kernel::EmbedKernelArgs),
    /// Embed an eBPF program into an RVF file
    EmbedEbpf(cmd::embed_ebpf::EmbedEbpfArgs),
    /// Create a membership filter for shared HNSW
    Filter(cmd::filter::FilterArgs),
    /// Snapshot-freeze the current state
    Freeze(cmd::freeze::FreezeArgs),
    /// Verify all witness events in chain
    VerifyWitness(cmd::verify_witness::VerifyWitnessArgs),
    /// Verify KernelBinding and attestation
    VerifyAttestation(cmd::verify_attestation::VerifyAttestationArgs),
    /// Rebuild REFCOUNT_SEG from COW map chain
    RebuildRefcounts(cmd::rebuild_refcounts::RebuildRefcountsArgs),
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Create(args) => cmd::create::run(args),
        Commands::Ingest(args) => cmd::ingest::run(args),
        Commands::Query(args) => cmd::query::run(args),
        Commands::Delete(args) => cmd::delete::run(args),
        Commands::Status(args) => cmd::status::run(args),
        Commands::Inspect(args) => cmd::inspect::run(args),
        Commands::Compact(args) => cmd::compact::run(args),
        Commands::Derive(args) => cmd::derive::run(args),
        Commands::Serve(args) => cmd::serve::run(args),
        Commands::Launch(args) => cmd::launch::run(args),
        Commands::EmbedKernel(args) => cmd::embed_kernel::run(args),
        Commands::EmbedEbpf(args) => cmd::embed_ebpf::run(args),
        Commands::Filter(args) => cmd::filter::run(args),
        Commands::Freeze(args) => cmd::freeze::run(args),
        Commands::VerifyWitness(args) => cmd::verify_witness::run(args),
        Commands::VerifyAttestation(args) => cmd::verify_attestation::run(args),
        Commands::RebuildRefcounts(args) => cmd::rebuild_refcounts::run(args),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
