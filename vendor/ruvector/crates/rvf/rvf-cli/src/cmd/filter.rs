//! `rvf filter` -- Create a MEMBERSHIP_SEG with include/exclude filter.

use clap::Args;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use rvf_runtime::RvfStore;

use super::map_rvf_err;

#[derive(Args)]
pub struct FilterArgs {
    /// Path to the RVF store
    pub file: String,
    /// Comma-separated list of vector IDs to include
    #[arg(long, value_delimiter = ',')]
    pub include_ids: Option<Vec<u64>>,
    /// Comma-separated list of vector IDs to exclude
    #[arg(long, value_delimiter = ',')]
    pub exclude_ids: Option<Vec<u64>>,
    /// Output path (if different from input, creates a derived file)
    #[arg(short, long)]
    pub output: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// MEMBERSHIP_SEG magic: "RVMB"
const MEMBERSHIP_MAGIC: u32 = 0x5256_4D42;

pub fn run(args: FilterArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (filter_mode, ids) = match (&args.include_ids, &args.exclude_ids) {
        (Some(inc), None) => (0u8, inc.clone()), // include mode
        (None, Some(exc)) => (1u8, exc.clone()), // exclude mode
        (Some(_), Some(_)) => {
            return Err("Cannot specify both --include-ids and --exclude-ids".into());
        }
        (None, None) => {
            return Err("Must specify either --include-ids or --exclude-ids".into());
        }
    };

    let target_path = args.output.as_deref().unwrap_or(&args.file);

    // If output is different, derive first
    if target_path != args.file {
        let parent = RvfStore::open_readonly(Path::new(&args.file)).map_err(map_rvf_err)?;
        let child = parent
            .derive(
                Path::new(target_path),
                rvf_types::DerivationType::Filter,
                None,
            )
            .map_err(map_rvf_err)?;
        child.close().map_err(map_rvf_err)?;
    }

    let store = RvfStore::open(Path::new(target_path)).map_err(map_rvf_err)?;

    // Build a simple bitmap filter
    let max_id = ids.iter().copied().max().unwrap_or(0);
    let bitmap_bytes = (max_id / 8 + 1) as usize;
    let mut bitmap = vec![0u8; bitmap_bytes];
    for &id in &ids {
        let byte_idx = (id / 8) as usize;
        let bit_idx = (id % 8) as u8;
        if byte_idx < bitmap.len() {
            bitmap[byte_idx] |= 1 << bit_idx;
        }
    }

    // Build the 96-byte MembershipHeader
    let mut header = [0u8; 96];
    header[0..4].copy_from_slice(&MEMBERSHIP_MAGIC.to_le_bytes());
    header[4..6].copy_from_slice(&1u16.to_le_bytes()); // version
    header[6] = 0; // filter_type: bitmap
    header[7] = filter_mode;
    // vector_count: use max_id+1 as approximation
    header[8..16].copy_from_slice(&(max_id + 1).to_le_bytes());
    // member_count
    header[16..24].copy_from_slice(&(ids.len() as u64).to_le_bytes());
    // filter_offset: will be 96 (right after header)
    header[24..32].copy_from_slice(&96u64.to_le_bytes());
    // filter_size
    header[32..36].copy_from_slice(&(bitmap.len() as u32).to_le_bytes());
    // generation_id
    header[36..40].copy_from_slice(&1u32.to_le_bytes());
    // filter_hash: simple hash of bitmap data
    let filter_hash = simple_hash(&bitmap);
    header[40..72].copy_from_slice(&filter_hash);
    // bloom_offset, bloom_size, reserved: all zero (already zeroed)

    // Write the MEMBERSHIP_SEG (0x22) as a raw segment
    let membership_seg_type = 0x22u8;
    let payload = [header.as_slice(), bitmap.as_slice()].concat();

    // Write raw segment to end of file
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(target_path)?;
    let mut writer = BufWriter::new(&file);
    writer.seek(SeekFrom::End(0))?;

    // Write segment header (64 bytes)
    let seg_header = build_segment_header(1, membership_seg_type, payload.len() as u64);
    writer.write_all(&seg_header)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    file.sync_all()?;

    drop(writer);
    drop(file);
    store.close().map_err(map_rvf_err)?;

    let mode_str = if filter_mode == 0 {
        "include"
    } else {
        "exclude"
    };
    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": "filtered",
            "mode": mode_str,
            "ids_count": ids.len(),
            "target": target_path,
        }));
    } else {
        println!("Membership filter created:");
        crate::output::print_kv("Mode:", mode_str);
        crate::output::print_kv("IDs:", &ids.len().to_string());
        crate::output::print_kv("Target:", target_path);
    }
    Ok(())
}

fn simple_hash(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, &b) in data.iter().enumerate() {
        out[i % 32] = out[i % 32].wrapping_add(b);
        let j = (i + 13) % 32;
        out[j] = out[j].wrapping_add(out[i % 32].rotate_left(3));
    }
    out
}

fn build_segment_header(seg_id: u64, seg_type: u8, payload_len: u64) -> Vec<u8> {
    let mut hdr = vec![0u8; 64];
    // magic: RVFS = 0x5256_4653
    hdr[0..4].copy_from_slice(&0x5256_4653u32.to_le_bytes());
    // version
    hdr[4] = 1;
    // seg_type
    hdr[5] = seg_type;
    // flags (2 bytes) - zero
    // seg_id (8 bytes at offset 0x08)
    hdr[0x08..0x10].copy_from_slice(&seg_id.to_le_bytes());
    // payload_length (8 bytes at offset 0x10)
    hdr[0x10..0x18].copy_from_slice(&payload_len.to_le_bytes());
    hdr
}
