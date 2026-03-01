//! `rvf delete` -- Delete vectors by ID or filter.

use clap::Args;
use std::path::Path;

use rvf_runtime::RvfStore;

use super::map_rvf_err;

#[derive(Args)]
pub struct DeleteArgs {
    /// Path to the RVF store
    path: String,
    /// Comma-separated vector IDs to delete (e.g. "1,2,3")
    #[arg(long)]
    ids: Option<String>,
    /// Filter expression as JSON (e.g. '{"gt":{"field":0,"value":{"u64":10}}}')
    #[arg(long)]
    filter: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(args: DeleteArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.ids.is_none() && args.filter.is_none() {
        return Err("must specify --ids or --filter".into());
    }

    let mut store = RvfStore::open(Path::new(&args.path)).map_err(map_rvf_err)?;

    let result = if let Some(ids_str) = &args.ids {
        let ids: Vec<u64> = ids_str
            .split(',')
            .map(|s| {
                s.trim()
                    .parse::<u64>()
                    .map_err(|e| format!("Invalid ID '{s}': {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        store.delete(&ids).map_err(map_rvf_err)?
    } else {
        let filter_str = args.filter.as_ref().unwrap();
        let filter_expr = super::query::parse_filter_json(filter_str)?;
        store.delete_by_filter(&filter_expr).map_err(map_rvf_err)?
    };

    store.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "deleted": result.deleted,
            "epoch": result.epoch,
        }));
    } else {
        println!("Delete complete:");
        crate::output::print_kv("Deleted:", &result.deleted.to_string());
        crate::output::print_kv("Epoch:", &result.epoch.to_string());
    }
    Ok(())
}
