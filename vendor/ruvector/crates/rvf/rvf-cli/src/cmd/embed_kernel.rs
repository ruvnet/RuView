//! `rvf embed-kernel` -- Embed a kernel image into an RVF file.

use clap::Args;
use std::path::Path;

use rvf_runtime::RvfStore;

use super::map_rvf_err;

#[derive(Args)]
pub struct EmbedKernelArgs {
    /// Path to the RVF store
    pub file: String,
    /// Target architecture: x86_64, aarch64
    #[arg(long, default_value = "x86_64")]
    pub arch: String,
    /// Use prebuilt kernel image instead of building
    #[arg(long)]
    pub prebuilt: bool,
    /// Path to kernel image file (bzImage or similar)
    #[arg(long)]
    pub image_path: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

fn parse_arch(s: &str) -> Result<u8, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "x86_64" | "x86-64" | "amd64" => Ok(1),
        "aarch64" | "arm64" => Ok(2),
        "riscv64" => Ok(3),
        other => Err(format!("Unknown architecture: {other}").into()),
    }
}

pub fn run(args: EmbedKernelArgs) -> Result<(), Box<dyn std::error::Error>> {
    let arch = parse_arch(&args.arch)?;

    let image_path = args
        .image_path
        .as_deref()
        .ok_or("No kernel image path provided. Use --image-path <path> or --prebuilt")?;

    let kernel_image = std::fs::read(image_path)
        .map_err(|e| format!("Failed to read kernel image '{}': {}", image_path, e))?;

    let mut store = RvfStore::open(Path::new(&args.file)).map_err(map_rvf_err)?;

    let seg_id = store
        .embed_kernel(
            arch,
            0,    // kernel_type: unikernel
            0x01, // kernel_flags: KERNEL_FLAG_SIGNED placeholder
            &kernel_image,
            8080,
            None,
        )
        .map_err(map_rvf_err)?;

    store.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "embedded",
            "segment_id": seg_id,
            "arch": args.arch,
            "image_size": kernel_image.len(),
        }));
    } else {
        println!("Kernel embedded successfully:");
        crate::output::print_kv("Segment ID:", &seg_id.to_string());
        crate::output::print_kv("Architecture:", &args.arch);
        crate::output::print_kv("Image size:", &format!("{} bytes", kernel_image.len()));
    }
    Ok(())
}
