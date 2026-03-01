//! `rvf embed-ebpf` -- Compile and embed an eBPF program into an RVF file.

use clap::Args;
use std::path::Path;

use rvf_runtime::RvfStore;

use super::map_rvf_err;

#[derive(Args)]
pub struct EmbedEbpfArgs {
    /// Path to the RVF store
    pub file: String,
    /// Path to the eBPF program (compiled .o or raw bytecode)
    #[arg(long)]
    pub program: String,
    /// eBPF program type: xdp, socket_filter, tc_classifier
    #[arg(long, default_value = "xdp")]
    pub program_type: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

fn parse_program_type(s: &str) -> Result<u8, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "xdp" => Ok(2),
        "socket_filter" | "socket-filter" => Ok(1),
        "tc_classifier" | "tc-classifier" | "tc" => Ok(3),
        other => Err(format!("Unknown eBPF program type: {other}").into()),
    }
}

pub fn run(args: EmbedEbpfArgs) -> Result<(), Box<dyn std::error::Error>> {
    let program_type = parse_program_type(&args.program_type)?;

    let bytecode = std::fs::read(&args.program)
        .map_err(|e| format!("Failed to read eBPF program '{}': {}", args.program, e))?;

    let mut store = RvfStore::open(Path::new(&args.file)).map_err(map_rvf_err)?;

    let seg_id = store
        .embed_ebpf(
            program_type,
            0, // attach_type
            0, // max_dimension (auto)
            &bytecode,
            None, // no BTF
        )
        .map_err(map_rvf_err)?;

    store.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "embedded",
            "segment_id": seg_id,
            "program_type": args.program_type,
            "bytecode_size": bytecode.len(),
        }));
    } else {
        println!("eBPF program embedded successfully:");
        crate::output::print_kv("Segment ID:", &seg_id.to_string());
        crate::output::print_kv("Program type:", &args.program_type);
        crate::output::print_kv("Bytecode size:", &format!("{} bytes", bytecode.len()));
    }
    Ok(())
}
