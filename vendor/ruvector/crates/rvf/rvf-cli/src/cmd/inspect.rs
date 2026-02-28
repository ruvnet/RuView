//! `rvf inspect` -- Inspect segments and lineage.

use clap::Args;
use std::path::Path;

use rvf_runtime::RvfStore;
use rvf_types::SegmentType;

use super::map_rvf_err;

#[derive(Args)]
pub struct InspectArgs {
    /// Path to the RVF store
    path: String,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

fn segment_type_name(seg_type: u8) -> &'static str {
    match seg_type {
        t if t == SegmentType::Vec as u8 => "Vec",
        t if t == SegmentType::Index as u8 => "Index",
        t if t == SegmentType::Overlay as u8 => "Overlay",
        t if t == SegmentType::Journal as u8 => "Journal",
        t if t == SegmentType::Manifest as u8 => "Manifest",
        t if t == SegmentType::Quant as u8 => "Quant",
        t if t == SegmentType::Meta as u8 => "Meta",
        t if t == SegmentType::Hot as u8 => "Hot",
        t if t == SegmentType::Sketch as u8 => "Sketch",
        t if t == SegmentType::Witness as u8 => "Witness",
        t if t == SegmentType::Profile as u8 => "Profile",
        t if t == SegmentType::Crypto as u8 => "Crypto",
        t if t == SegmentType::MetaIdx as u8 => "MetaIdx",
        t if t == SegmentType::Kernel as u8 => "Kernel",
        t if t == SegmentType::Ebpf as u8 => "Ebpf",
        t if t == SegmentType::CowMap as u8 => "CowMap",
        t if t == SegmentType::Refcount as u8 => "Refcount",
        t if t == SegmentType::Membership as u8 => "Membership",
        t if t == SegmentType::Delta as u8 => "Delta",
        _ => "Unknown",
    }
}

pub fn run(args: InspectArgs) -> Result<(), Box<dyn std::error::Error>> {
    let store = RvfStore::open_readonly(Path::new(&args.path)).map_err(map_rvf_err)?;
    let seg_dir = store.segment_dir();
    let dimension = store.dimension();
    let identity = store.file_identity();
    let status = store.status();

    if args.json {
        let segments: Vec<serde_json::Value> = seg_dir
            .iter()
            .map(|&(seg_id, offset, payload_len, seg_type)| {
                serde_json::json!({
                    "seg_id": seg_id,
                    "offset": offset,
                    "payload_length": payload_len,
                    "seg_type": seg_type,
                    "seg_type_name": segment_type_name(seg_type),
                })
            })
            .collect();

        crate::output::print_json(&serde_json::json!({
            "path": args.path,
            "dimension": dimension,
            "epoch": status.current_epoch,
            "total_vectors": status.total_vectors,
            "total_segments": status.total_segments,
            "file_size": status.file_size,
            "segments": segments,
            "lineage": {
                "file_id": crate::output::hex(&identity.file_id),
                "parent_id": crate::output::hex(&identity.parent_id),
                "parent_hash": crate::output::hex(&identity.parent_hash),
                "lineage_depth": identity.lineage_depth,
                "is_root": identity.is_root(),
            },
        }));
    } else {
        println!("RVF Store: {}", args.path);
        crate::output::print_kv("Dimension:", &dimension.to_string());
        crate::output::print_kv("Epoch:", &status.current_epoch.to_string());
        crate::output::print_kv("Vectors:", &status.total_vectors.to_string());
        crate::output::print_kv("File size:", &format!("{} bytes", status.file_size));
        println!();

        println!("Segments ({}):", seg_dir.len());
        for &(seg_id, offset, payload_len, seg_type) in seg_dir {
            println!(
                "  seg_id={:<4} type={:<10} offset={:<10} payload={} bytes",
                seg_id,
                segment_type_name(seg_type),
                offset,
                payload_len,
            );
        }

        println!();
        println!("Lineage:");
        crate::output::print_kv("File ID:", &crate::output::hex(&identity.file_id));
        crate::output::print_kv("Parent ID:", &crate::output::hex(&identity.parent_id));
        crate::output::print_kv("Lineage depth:", &identity.lineage_depth.to_string());
        crate::output::print_kv("Is root:", &identity.is_root().to_string());
    }
    Ok(())
}
