//! `rvf rebuild-refcounts` -- Recompute REFCOUNT_SEG from COW map chain.

use clap::Args;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use rvf_runtime::RvfStore;
use rvf_types::{SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};

use super::map_rvf_err;

#[derive(Args)]
pub struct RebuildRefcountsArgs {
    /// Path to the RVF store
    pub file: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// COW_MAP_SEG magic: "RVCM"
const COW_MAP_MAGIC: u32 = 0x5256_434D;
/// REFCOUNT_SEG magic: "RVRC"
const REFCOUNT_MAGIC: u32 = 0x5256_5243;
/// COW_MAP_SEG type
const COW_MAP_TYPE: u8 = 0x20;

pub fn run(args: RebuildRefcountsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let store = RvfStore::open_readonly(Path::new(&args.file)).map_err(map_rvf_err)?;

    // Read the raw file to scan for COW map segments
    let file = std::fs::File::open(&args.file)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(0))?;
    let mut raw_bytes = Vec::new();
    reader.read_to_end(&mut raw_bytes)?;

    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut cluster_count = 0u32;
    let mut local_cluster_count = 0u32;

    // Scan for COW_MAP_SEG entries
    let mut i = 0usize;
    while i + SEGMENT_HEADER_SIZE <= raw_bytes.len() {
        if raw_bytes[i..i + 4] == magic_bytes && raw_bytes[i + 5] == COW_MAP_TYPE {
            let payload_len = u64::from_le_bytes([
                raw_bytes[i + 0x10],
                raw_bytes[i + 0x11],
                raw_bytes[i + 0x12],
                raw_bytes[i + 0x13],
                raw_bytes[i + 0x14],
                raw_bytes[i + 0x15],
                raw_bytes[i + 0x16],
                raw_bytes[i + 0x17],
            ]);

            let payload_start = i + SEGMENT_HEADER_SIZE;
            let payload_end = payload_start + payload_len as usize;

            if payload_end <= raw_bytes.len() && payload_len >= 64 {
                // Read CowMapHeader fields
                let cow_magic = u32::from_le_bytes([
                    raw_bytes[payload_start],
                    raw_bytes[payload_start + 1],
                    raw_bytes[payload_start + 2],
                    raw_bytes[payload_start + 3],
                ]);
                if cow_magic == COW_MAP_MAGIC {
                    cluster_count = u32::from_le_bytes([
                        raw_bytes[payload_start + 0x48],
                        raw_bytes[payload_start + 0x49],
                        raw_bytes[payload_start + 0x4A],
                        raw_bytes[payload_start + 0x4B],
                    ]);
                    local_cluster_count = u32::from_le_bytes([
                        raw_bytes[payload_start + 0x4C],
                        raw_bytes[payload_start + 0x4D],
                        raw_bytes[payload_start + 0x4E],
                        raw_bytes[payload_start + 0x4F],
                    ]);
                }
            }

            let advance = SEGMENT_HEADER_SIZE + payload_len as usize;
            if advance > 0 && i.checked_add(advance).is_some() {
                i += advance;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    drop(store);

    if cluster_count == 0 {
        if args.json {
            crate::output::print_json(&serde_json::json!({
                "status": "no_cow_map",
                "message": "No COW map found; nothing to rebuild",
            }));
        } else {
            println!("No COW map found in file. Nothing to rebuild.");
        }
        return Ok(());
    }

    // Build refcount array: 1 byte per cluster, all set to 1 (base reference)
    let refcount_array = vec![1u8; cluster_count as usize];

    // Build 32-byte RefcountHeader
    let mut header = [0u8; 32];
    header[0..4].copy_from_slice(&REFCOUNT_MAGIC.to_le_bytes());
    header[4..6].copy_from_slice(&1u16.to_le_bytes()); // version
    header[6] = 1; // refcount_width: 1 byte
    header[8..12].copy_from_slice(&cluster_count.to_le_bytes());
    header[12..16].copy_from_slice(&1u32.to_le_bytes()); // max_refcount
    header[16..24].copy_from_slice(&32u64.to_le_bytes()); // array_offset (after header)
                                                          // snapshot_epoch: 0 (mutable)
                                                          // reserved: 0

    let payload = [header.as_slice(), refcount_array.as_slice()].concat();

    // Write REFCOUNT_SEG to end of file
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&args.file)?;
    let mut writer = BufWriter::new(&file);
    writer.seek(SeekFrom::End(0))?;

    let seg_header = build_segment_header(1, 0x21, payload.len() as u64);
    writer.write_all(&seg_header)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    file.sync_all()?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "rebuilt",
            "cluster_count": cluster_count,
            "local_clusters": local_cluster_count,
        }));
    } else {
        println!("Refcounts rebuilt:");
        crate::output::print_kv("Cluster count:", &cluster_count.to_string());
        crate::output::print_kv("Local clusters:", &local_cluster_count.to_string());
    }
    Ok(())
}

fn build_segment_header(seg_id: u64, seg_type: u8, payload_len: u64) -> Vec<u8> {
    let mut hdr = vec![0u8; 64];
    hdr[0..4].copy_from_slice(&0x5256_4653u32.to_le_bytes());
    hdr[4] = 1;
    hdr[5] = seg_type;
    hdr[0x08..0x10].copy_from_slice(&seg_id.to_le_bytes());
    hdr[0x10..0x18].copy_from_slice(&payload_len.to_le_bytes());
    hdr
}
