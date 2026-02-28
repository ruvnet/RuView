//! Tail-scan algorithm for finding the latest manifest segment.
//!
//! An RVF file is discovered from its tail: the Level 0 root manifest is
//! always the last 4096 bytes. If that's invalid, we scan backward at
//! 64-byte boundaries looking for a MANIFEST_SEG header.

use crate::reader::read_segment_header;
use rvf_types::{
    ErrorCode, RvfError, SegmentHeader, SegmentType, ROOT_MANIFEST_MAGIC, ROOT_MANIFEST_SIZE,
    SEGMENT_ALIGNMENT, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC, SEGMENT_VERSION,
};

/// Find the latest manifest segment in `data` by scanning from the tail.
///
/// **Fast path**: check the last 4096 bytes for the root manifest magic
/// (`RVM0`). If valid, scan backward from that point for the enclosing
/// MANIFEST_SEG header.
///
/// **Slow path**: scan backward from the end of `data` at 64-byte aligned
/// boundaries, looking for a segment header with magic `RVFS` and type
/// `MANIFEST_SEG` (0x05).
///
/// Returns `(byte_offset, SegmentHeader)` of the manifest segment.
///
/// # Errors
///
/// - `ManifestNotFound` if no valid MANIFEST_SEG is found in the entire file.
/// - `TruncatedSegment` if the file is too short to contain any segment.
pub fn find_latest_manifest(data: &[u8]) -> Result<(usize, SegmentHeader), RvfError> {
    if data.len() < SEGMENT_HEADER_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    // Fast path: check last 4096 bytes for RVM0 magic
    if data.len() >= ROOT_MANIFEST_SIZE {
        let root_start = data.len() - ROOT_MANIFEST_SIZE;
        let root_slice = &data[root_start..];
        if root_slice.len() >= 4 {
            let root_magic =
                u32::from_le_bytes([root_slice[0], root_slice[1], root_slice[2], root_slice[3]]);
            if root_magic == ROOT_MANIFEST_MAGIC {
                // Scan backward from root_start for the enclosing MANIFEST_SEG header
                let scan_limit = root_start.saturating_sub(64 * 1024);
                let mut scan_pos = root_start & !(SEGMENT_ALIGNMENT - 1);
                loop {
                    if scan_pos + SEGMENT_HEADER_SIZE <= data.len() {
                        if let Ok(header) = read_segment_header(&data[scan_pos..]) {
                            if header.seg_type == SegmentType::Manifest as u8 {
                                let seg_end =
                                    scan_pos + SEGMENT_HEADER_SIZE + header.payload_length as usize;
                                if seg_end >= root_start + ROOT_MANIFEST_SIZE
                                    || seg_end >= data.len()
                                {
                                    return Ok((scan_pos, header));
                                }
                            }
                        }
                    }
                    if scan_pos <= scan_limit || scan_pos == 0 {
                        break;
                    }
                    scan_pos = scan_pos.saturating_sub(SEGMENT_ALIGNMENT);
                }
            }
        }
    }

    // Slow path: scan backward using the first magic byte ('R' = 0x52) as
    // an anchor. On aligned boundaries within the data, we search for the
    // magic byte first (memchr-style) to skip large runs of non-magic data,
    // then verify the full 4-byte magic + version + type.
    let magic_le = SEGMENT_MAGIC.to_le_bytes();
    let magic_first = magic_le[0]; // 'R' = 0x52
    let last_aligned = (data.len().saturating_sub(SEGMENT_ALIGNMENT)) & !(SEGMENT_ALIGNMENT - 1);
    let mut scan_pos = last_aligned;
    loop {
        if scan_pos + SEGMENT_HEADER_SIZE <= data.len() {
            // Quick rejection: check first byte of magic before doing full comparison.
            if data[scan_pos] == magic_first {
                let magic = u32::from_le_bytes([
                    data[scan_pos],
                    data[scan_pos + 1],
                    data[scan_pos + 2],
                    data[scan_pos + 3],
                ]);
                if magic == SEGMENT_MAGIC
                    && data[scan_pos + 4] == SEGMENT_VERSION
                    && data[scan_pos + 5] == SegmentType::Manifest as u8
                {
                    if let Ok(header) = read_segment_header(&data[scan_pos..]) {
                        return Ok((scan_pos, header));
                    }
                }
            }
        }
        if scan_pos == 0 {
            break;
        }
        scan_pos -= SEGMENT_ALIGNMENT;
    }

    Err(RvfError::Code(ErrorCode::ManifestNotFound))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::write_segment;
    use rvf_types::SegmentFlags;

    fn make_manifest_segment(segment_id: u64, payload: &[u8]) -> Vec<u8> {
        write_segment(
            SegmentType::Manifest as u8,
            payload,
            SegmentFlags::empty(),
            segment_id,
        )
    }

    #[test]
    fn find_single_manifest() {
        let vec_seg = write_segment(
            SegmentType::Vec as u8,
            &[1u8; 100],
            SegmentFlags::empty(),
            0,
        );
        let manifest_payload = vec![0u8; 64];
        let manifest_seg = make_manifest_segment(1, &manifest_payload);
        let mut file = vec_seg.clone();
        let manifest_offset = file.len();
        file.extend_from_slice(&manifest_seg);

        let (offset, header) = find_latest_manifest(&file).unwrap();
        assert_eq!(offset, manifest_offset);
        assert_eq!(header.seg_type, SegmentType::Manifest as u8);
        assert_eq!(header.segment_id, 1);
    }

    #[test]
    fn find_latest_of_multiple_manifests() {
        let vec_seg = write_segment(SegmentType::Vec as u8, &[0u8; 32], SegmentFlags::empty(), 0);
        let m1 = make_manifest_segment(1, &[0u8; 32]);
        let m2 = make_manifest_segment(2, &[0u8; 32]);
        let mut file = vec_seg;
        file.extend_from_slice(&m1);
        let m2_offset = file.len();
        file.extend_from_slice(&m2);

        let (offset, header) = find_latest_manifest(&file).unwrap();
        assert_eq!(offset, m2_offset);
        assert_eq!(header.segment_id, 2);
    }

    #[test]
    fn no_manifest_returns_error() {
        let vec_seg = write_segment(
            SegmentType::Vec as u8,
            &[0u8; 100],
            SegmentFlags::empty(),
            0,
        );
        let result = find_latest_manifest(&vec_seg);
        assert!(result.is_err());
    }

    #[test]
    fn too_short_returns_error() {
        let result = find_latest_manifest(&[0u8; 10]);
        assert!(result.is_err());
    }
}
