//! `rvf create` -- Create a new empty RVF store.

use clap::Args;
use std::path::Path;

use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{RvfOptions, RvfStore};

use super::map_rvf_err;

#[derive(Args)]
pub struct CreateArgs {
    /// Path for the new RVF store file
    path: String,
    /// Vector dimensionality
    #[arg(short, long)]
    dimension: u32,
    /// Distance metric: l2, ip, cosine
    #[arg(short, long, default_value = "l2")]
    metric: String,
    /// Hardware profile: 0-3
    #[arg(short, long, default_value = "0")]
    profile: u8,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(args: CreateArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.dimension == 0 || args.dimension > u16::MAX as u32 {
        return Err(format!(
            "Dimension must be between 1 and {} (got {})",
            u16::MAX,
            args.dimension
        )
        .into());
    }

    let metric = match args.metric.as_str() {
        "l2" | "L2" => DistanceMetric::L2,
        "ip" | "inner_product" => DistanceMetric::InnerProduct,
        "cosine" => DistanceMetric::Cosine,
        other => return Err(format!("Unknown metric: {other}").into()),
    };

    let opts = RvfOptions {
        dimension: args.dimension as u16,
        metric,
        profile: args.profile,
        ..Default::default()
    };

    let store = RvfStore::create(Path::new(&args.path), opts).map_err(map_rvf_err)?;
    store.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "created",
            "path": args.path,
            "dimension": args.dimension,
            "metric": args.metric,
            "profile": args.profile,
        }));
    } else {
        println!("Created RVF store: {}", args.path);
        crate::output::print_kv("Dimension:", &args.dimension.to_string());
        crate::output::print_kv("Metric:", &args.metric);
        crate::output::print_kv("Profile:", &args.profile.to_string());
    }
    Ok(())
}
