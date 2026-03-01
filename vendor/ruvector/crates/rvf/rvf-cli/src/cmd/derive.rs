//! `rvf derive` -- Derive a child store from a parent.

use clap::Args;
use std::path::Path;

use rvf_runtime::RvfStore;
use rvf_types::DerivationType;

use super::map_rvf_err;

#[derive(Args)]
pub struct DeriveArgs {
    /// Path to the parent RVF store
    parent: String,
    /// Path for the new child RVF store
    child: String,
    /// Derivation type: clone, filter, merge, quantize, reindex, transform, snapshot
    #[arg(short = 't', long, default_value = "clone")]
    derivation_type: String,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

fn parse_derivation_type(s: &str) -> Result<DerivationType, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "clone" => Ok(DerivationType::Clone),
        "filter" => Ok(DerivationType::Filter),
        "merge" => Ok(DerivationType::Merge),
        "quantize" => Ok(DerivationType::Quantize),
        "reindex" => Ok(DerivationType::Reindex),
        "transform" => Ok(DerivationType::Transform),
        "snapshot" => Ok(DerivationType::Snapshot),
        other => Err(format!("Unknown derivation type: {other}").into()),
    }
}

pub fn run(args: DeriveArgs) -> Result<(), Box<dyn std::error::Error>> {
    let dt = parse_derivation_type(&args.derivation_type)?;

    let parent = RvfStore::open_readonly(Path::new(&args.parent)).map_err(map_rvf_err)?;
    let child = parent
        .derive(Path::new(&args.child), dt, None)
        .map_err(map_rvf_err)?;

    let child_identity = *child.file_identity();
    child.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "derived",
            "parent": args.parent,
            "child": args.child,
            "derivation_type": args.derivation_type,
            "child_file_id": crate::output::hex(&child_identity.file_id),
            "parent_file_id": crate::output::hex(&child_identity.parent_id),
            "lineage_depth": child_identity.lineage_depth,
        }));
    } else {
        println!("Derived child store: {}", args.child);
        crate::output::print_kv("Parent:", &args.parent);
        crate::output::print_kv("Type:", &args.derivation_type);
        crate::output::print_kv(
            "Child file ID:",
            &crate::output::hex(&child_identity.file_id),
        );
        crate::output::print_kv("Lineage depth:", &child_identity.lineage_depth.to_string());
    }
    Ok(())
}
