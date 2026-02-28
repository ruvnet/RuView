//! Segment parsing and inspection exports for WASM.

extern crate alloc;

use alloc::vec::Vec;
use rvf_types::constants::{SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};

/// Parsed segment info for WASM consumers.
pub struct SegmentInfo {
    pub seg_id: u64,
    pub seg_type: u8,
    pub payload_length: u64,
    pub offset: usize,
}

/// Parse all segments from a raw .rvf byte buffer.
pub fn parse_segments(buf: &[u8]) -> Vec<SegmentInfo> {
    let mut segments = Vec::new();
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();

    if buf.len() < SEGMENT_HEADER_SIZE {
        return segments;
    }

    let mut i = 0;
    let last = buf.len().saturating_sub(SEGMENT_HEADER_SIZE);

    while i <= last {
        if buf[i..i + 4] == magic_bytes {
            let version = buf[i + 4];
            if version != 1 {
                i += 1;
                continue;
            }
            let seg_type = buf[i + 5];
            let seg_id = u64::from_le_bytes([
                buf[i + 8],
                buf[i + 9],
                buf[i + 10],
                buf[i + 11],
                buf[i + 12],
                buf[i + 13],
                buf[i + 14],
                buf[i + 15],
            ]);
            let payload_length = u64::from_le_bytes([
                buf[i + 16],
                buf[i + 17],
                buf[i + 18],
                buf[i + 19],
                buf[i + 20],
                buf[i + 21],
                buf[i + 22],
                buf[i + 23],
            ]);

            segments.push(SegmentInfo {
                seg_id,
                seg_type,
                payload_length,
                offset: i,
            });

            // Skip past this segment
            let total = SEGMENT_HEADER_SIZE + payload_length as usize;
            if let Some(next) = i.checked_add(total) {
                if next > i {
                    i = next;
                    continue;
                }
            }
        }
        i += 1;
    }

    segments
}

/// Parse a segment header from raw bytes.
/// Writes to out_ptr: [magic: u32, version: u8, type: u8, flags: u16, seg_id: u64, payload_len: u64]
/// = 24 bytes
pub fn parse_header_to_buf(buf: &[u8], out_ptr: *mut u8) -> i32 {
    if buf.len() < SEGMENT_HEADER_SIZE {
        return -1;
    }

    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != SEGMENT_MAGIC {
        return -2;
    }

    // Copy first 24 bytes of header (magic through payload_length)
    unsafe {
        for i in 0..24 {
            *out_ptr.add(i) = buf[i];
        }
    }
    0
}

/// Verify CRC32C of a buffer. Returns 1 if valid (matches expected), 0 if not.
pub fn verify_crc32c(buf: &[u8], expected: u32) -> i32 {
    let computed = crc32c_compute(buf);
    if computed == expected {
        1
    } else {
        0
    }
}

fn crc32c_compute(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x82F6_3B78;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}
