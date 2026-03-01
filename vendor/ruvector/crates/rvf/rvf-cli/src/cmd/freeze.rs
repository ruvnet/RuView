//! `rvf freeze` -- Snapshot-freeze the current state of an RVF store.

use clap::Args;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use rvf_runtime::RvfStore;

use super::map_rvf_err;

#[derive(Args)]
pub struct FreezeArgs {
    /// Path to the RVF store
    pub file: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// REFCOUNT_SEG magic: "RVRC"
const REFCOUNT_MAGIC: u32 = 0x5256_5243;

pub fn run(args: FreezeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let store = RvfStore::open(Path::new(&args.file)).map_err(map_rvf_err)?;
    let status = store.status();
    let snapshot_epoch = status.current_epoch + 1;

    // Build a 32-byte RefcountHeader with snapshot_epoch set
    let mut header = [0u8; 32];
    header[0..4].copy_from_slice(&REFCOUNT_MAGIC.to_le_bytes());
    header[4..6].copy_from_slice(&1u16.to_le_bytes()); // version
    header[6] = 1; // refcount_width: 1 byte per entry
                   // cluster_count: 0 (no clusters tracked yet)
                   // max_refcount: 0
                   // array_offset: 0 (no array)
                   // snapshot_epoch
    header[0x18..0x1C].copy_from_slice(&snapshot_epoch.to_le_bytes());

    // Write a REFCOUNT_SEG (0x21) with the frozen epoch
    let seg_type = 0x21u8; // Refcount
    let payload = header;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&args.file)?;
    let mut writer = BufWriter::new(&file);
    writer.seek(SeekFrom::End(0))?;

    let seg_header = build_segment_header(1, seg_type, payload.len() as u64);
    writer.write_all(&seg_header)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    file.sync_all()?;

    drop(writer);
    drop(file);

    // Emit a witness event for the snapshot
    // (witness writing would go through the store's witness path when available)

    store.close().map_err(map_rvf_err)?;

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "frozen",
            "snapshot_epoch": snapshot_epoch,
        }));
    } else {
        println!("Store frozen:");
        crate::output::print_kv("Snapshot epoch:", &snapshot_epoch.to_string());
        println!("  All further writes will create a new derived generation.");
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
