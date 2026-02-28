//! Append-only write logic for the RVF runtime.
//!
//! All mutations append new segments to the file. The write path:
//! 1. Allocate segment_id (monotonic counter)
//! 2. Build payload (VEC_SEG, META_SEG, JOURNAL_SEG, etc.)
//! 3. Write segment header + payload, fsync
//! 4. Build new MANIFEST_SEG, fsync (two-fsync protocol)

use rvf_types::{SegmentHeader, SegmentType, SEGMENT_HEADER_SIZE};
use std::io::{self, Seek, Write};

/// Segment writer that handles the append-only write protocol.
pub(crate) struct SegmentWriter {
    /// Next segment ID to assign (monotonic counter).
    next_seg_id: u64,
}

impl SegmentWriter {
    pub(crate) fn new(starting_id: u64) -> Self {
        Self {
            next_seg_id: starting_id,
        }
    }

    /// Allocate a new segment ID.
    ///
    /// Uses checked arithmetic to detect overflow (would require 2^64 segments).
    pub(crate) fn alloc_seg_id(&mut self) -> u64 {
        let id = self.next_seg_id;
        self.next_seg_id = self
            .next_seg_id
            .checked_add(1)
            .expect("segment ID counter overflow");
        id
    }

    /// Write a VEC_SEG containing the given f32 vectors.
    ///
    /// Returns the segment ID and byte offset where it was written.
    pub(crate) fn write_vec_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        vectors: &[&[f32]],
        ids: &[u64],
        dimension: u16,
    ) -> io::Result<(u64, u64)> {
        let seg_id = self.alloc_seg_id();

        // Build payload: dimension(u16) + vector_count(u32) + [id(u64) + data(f32 * dim)]
        let vector_count = vectors.len() as u32;
        let bytes_per_vec = (dimension as usize) * 4;
        let payload_size = 2 + 4 + (vectors.len() * (8 + bytes_per_vec));
        let mut payload = Vec::with_capacity(payload_size);

        payload.extend_from_slice(&dimension.to_le_bytes());
        payload.extend_from_slice(&vector_count.to_le_bytes());
        for (vec_data, &vec_id) in vectors.iter().zip(ids.iter()) {
            payload.extend_from_slice(&vec_id.to_le_bytes());
            for &val in *vec_data {
                payload.extend_from_slice(&val.to_le_bytes());
            }
        }

        let offset = self.write_segment(writer, SegmentType::Vec as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Write a JOURNAL_SEG with tombstone entries for deleted vector IDs.
    pub(crate) fn write_journal_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        deleted_ids: &[u64],
        epoch: u32,
    ) -> io::Result<(u64, u64)> {
        let seg_id = self.alloc_seg_id();

        // Journal header (simplified): entry_count(u32) + epoch(u32) + prev_seg_id(u64)
        // Then entries: each is entry_type(u8) + pad(u8) + len(u16) + vector_id(u64)
        let entry_count = deleted_ids.len() as u32;
        let payload_size = 16 + (deleted_ids.len() * 12); // header + entries
        let mut payload = Vec::with_capacity(payload_size);

        // Journal header.
        payload.extend_from_slice(&entry_count.to_le_bytes());
        payload.extend_from_slice(&epoch.to_le_bytes());
        payload.extend_from_slice(&0u64.to_le_bytes()); // prev_journal_seg_id

        // Entries: DELETE_VECTOR (type 0x01).
        for &vid in deleted_ids {
            payload.push(0x01); // DELETE_VECTOR
            payload.push(0x00); // reserved
            payload.extend_from_slice(&8u16.to_le_bytes()); // entry_length
            payload.extend_from_slice(&vid.to_le_bytes());
        }

        let offset = self.write_segment(writer, SegmentType::Journal as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Write a META_SEG for vector metadata.
    #[allow(dead_code)]
    pub(crate) fn write_meta_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        metadata_payload: &[u8],
    ) -> io::Result<(u64, u64)> {
        let seg_id = self.alloc_seg_id();
        let offset =
            self.write_segment(writer, SegmentType::Meta as u8, seg_id, metadata_payload)?;
        Ok((seg_id, offset))
    }

    /// Write a minimal MANIFEST_SEG recording current state.
    ///
    /// This is a simplified manifest that stores:
    /// - epoch, dimension, total_vectors, total_segments, profile_id
    /// - segment directory entries (seg_id, offset, length, type)
    /// - deletion bitmap (vector IDs as simple packed u64 array)
    /// - file identity (68 bytes, appended for lineage provenance)
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn write_manifest_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        epoch: u32,
        dimension: u16,
        total_vectors: u64,
        profile_id: u8,
        segment_dir: &[(u64, u64, u64, u8)], // (seg_id, offset, payload_len, seg_type)
        deleted_ids: &[u64],
    ) -> io::Result<(u64, u64)> {
        self.write_manifest_seg_with_identity(
            writer,
            epoch,
            dimension,
            total_vectors,
            profile_id,
            segment_dir,
            deleted_ids,
            None,
        )
    }

    /// Write a MANIFEST_SEG with optional FileIdentity appended.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_manifest_seg_with_identity<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        epoch: u32,
        dimension: u16,
        total_vectors: u64,
        profile_id: u8,
        segment_dir: &[(u64, u64, u64, u8)],
        deleted_ids: &[u64],
        file_identity: Option<&rvf_types::FileIdentity>,
    ) -> io::Result<(u64, u64)> {
        let seg_id = self.alloc_seg_id();

        // Build manifest payload.
        let seg_count = segment_dir.len() as u32;
        let del_count = deleted_ids.len() as u32;
        let payload_size = 4 + 2 + 8 + 4 + 1 + 3 // header fields
            + (segment_dir.len() * (8 + 8 + 8 + 1)) // directory
            + 4 + (deleted_ids.len() * 8) // deletion bitmap
            + if file_identity.is_some() { 4 + 68 } else { 0 }; // lineage marker + identity

        let mut payload = Vec::with_capacity(payload_size);

        // Manifest header.
        payload.extend_from_slice(&epoch.to_le_bytes());
        payload.extend_from_slice(&dimension.to_le_bytes());
        payload.extend_from_slice(&total_vectors.to_le_bytes());
        payload.extend_from_slice(&seg_count.to_le_bytes());
        payload.push(profile_id);
        payload.extend_from_slice(&[0u8; 3]); // reserved

        // Segment directory.
        for &(sid, off, plen, stype) in segment_dir {
            payload.extend_from_slice(&sid.to_le_bytes());
            payload.extend_from_slice(&off.to_le_bytes());
            payload.extend_from_slice(&plen.to_le_bytes());
            payload.push(stype);
        }

        // Deletion bitmap (simplified: count + packed IDs).
        payload.extend_from_slice(&del_count.to_le_bytes());
        for &did in deleted_ids {
            payload.extend_from_slice(&did.to_le_bytes());
        }

        // FileIdentity (optional, backward-compatible trailer).
        // Magic marker 0x46494449 ("FIDI") followed by 68-byte identity.
        if let Some(fi) = file_identity {
            payload.extend_from_slice(&0x4649_4449u32.to_le_bytes()); // "FIDI"
            payload.extend_from_slice(&fi.to_bytes());
        }

        let offset = self.write_segment(writer, SegmentType::Manifest as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Maximum kernel image size (128 MiB) to prevent DoS via oversized segments.
    #[allow(dead_code)]
    const MAX_KERNEL_IMAGE_SIZE: usize = 128 * 1024 * 1024;

    /// Write a KERNEL_SEG containing a compressed kernel image.
    ///
    /// Payload layout: `kernel_header_bytes` (128) + `kernel_image` + optional `cmdline`.
    /// Returns the segment ID and byte offset where it was written.
    ///
    /// Returns an error if the kernel image exceeds 128 MiB.
    #[allow(dead_code)]
    pub(crate) fn write_kernel_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        kernel_header_bytes: &[u8; 128],
        kernel_image: &[u8],
        cmdline: Option<&[u8]>,
    ) -> io::Result<(u64, u64)> {
        if kernel_image.len() > Self::MAX_KERNEL_IMAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "kernel image too large: {} bytes (max {})",
                    kernel_image.len(),
                    Self::MAX_KERNEL_IMAGE_SIZE
                ),
            ));
        }

        let seg_id = self.alloc_seg_id();

        let cmdline_len = cmdline.map_or(0, |c| c.len());
        let payload_size = 128 + kernel_image.len() + cmdline_len;
        let mut payload = Vec::with_capacity(payload_size);

        payload.extend_from_slice(kernel_header_bytes);
        payload.extend_from_slice(kernel_image);
        if let Some(cl) = cmdline {
            payload.extend_from_slice(cl);
        }

        let offset = self.write_segment(writer, SegmentType::Kernel as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Maximum eBPF program size (16 MiB) to prevent DoS via oversized segments.
    #[allow(dead_code)]
    const MAX_EBPF_PROGRAM_SIZE: usize = 16 * 1024 * 1024;

    /// Write an EBPF_SEG containing eBPF program bytecode and optional BTF data.
    ///
    /// Payload layout: `ebpf_header_bytes` (64) + `program_bytecode` + optional `btf_data`.
    /// Returns the segment ID and byte offset where it was written.
    ///
    /// Returns an error if the combined bytecode + BTF data exceeds 16 MiB.
    #[allow(dead_code)]
    pub(crate) fn write_ebpf_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        ebpf_header_bytes: &[u8; 64],
        program_bytecode: &[u8],
        btf_data: Option<&[u8]>,
    ) -> io::Result<(u64, u64)> {
        let btf_len = btf_data.map_or(0, |b| b.len());
        let total_program_size = program_bytecode.len().saturating_add(btf_len);
        if total_program_size > Self::MAX_EBPF_PROGRAM_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "eBPF program too large: {} bytes (max {})",
                    total_program_size,
                    Self::MAX_EBPF_PROGRAM_SIZE
                ),
            ));
        }

        let seg_id = self.alloc_seg_id();

        let payload_size = 64 + program_bytecode.len() + btf_len;
        let mut payload = Vec::with_capacity(payload_size);

        payload.extend_from_slice(ebpf_header_bytes);
        payload.extend_from_slice(program_bytecode);
        if let Some(btf) = btf_data {
            payload.extend_from_slice(btf);
        }

        let offset = self.write_segment(writer, SegmentType::Ebpf as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Maximum WASM module size (8 MiB) to prevent DoS via oversized segments.
    #[allow(dead_code)]
    const MAX_WASM_MODULE_SIZE: usize = 8 * 1024 * 1024;

    /// Write a WASM_SEG containing WASM bytecode for self-bootstrapping.
    ///
    /// Payload layout: `wasm_header_bytes` (64) + `wasm_bytecode`.
    /// Returns the segment ID and byte offset where it was written.
    ///
    /// Returns an error if the bytecode exceeds 8 MiB.
    #[allow(dead_code)]
    pub(crate) fn write_wasm_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        wasm_header_bytes: &[u8; 64],
        wasm_bytecode: &[u8],
    ) -> io::Result<(u64, u64)> {
        if wasm_bytecode.len() > Self::MAX_WASM_MODULE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "WASM module too large: {} bytes (max {})",
                    wasm_bytecode.len(),
                    Self::MAX_WASM_MODULE_SIZE
                ),
            ));
        }

        let seg_id = self.alloc_seg_id();

        let payload_size = 64 + wasm_bytecode.len();
        let mut payload = Vec::with_capacity(payload_size);

        payload.extend_from_slice(wasm_header_bytes);
        payload.extend_from_slice(wasm_bytecode);

        let offset = self.write_segment(writer, SegmentType::Wasm as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Maximum dashboard bundle size (64 MiB) to prevent DoS via oversized segments.
    #[allow(dead_code)]
    const MAX_DASHBOARD_BUNDLE_SIZE: usize = 64 * 1024 * 1024;

    /// Write a DASHBOARD_SEG containing a pre-built web dashboard bundle.
    ///
    /// Payload layout: `dashboard_header_bytes` (64) + `bundle_data`.
    /// Returns the segment ID and byte offset where it was written.
    ///
    /// Returns an error if the bundle exceeds 64 MiB.
    #[allow(dead_code)]
    pub(crate) fn write_dashboard_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        dashboard_header_bytes: &[u8; 64],
        bundle_data: &[u8],
    ) -> io::Result<(u64, u64)> {
        if bundle_data.len() > Self::MAX_DASHBOARD_BUNDLE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "dashboard bundle too large: {} bytes (max {})",
                    bundle_data.len(),
                    Self::MAX_DASHBOARD_BUNDLE_SIZE
                ),
            ));
        }

        let seg_id = self.alloc_seg_id();

        let payload_size = 64 + bundle_data.len();
        let mut payload = Vec::with_capacity(payload_size);

        payload.extend_from_slice(dashboard_header_bytes);
        payload.extend_from_slice(bundle_data);

        let offset = self.write_segment(writer, SegmentType::Dashboard as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Write a WITNESS_SEG containing a serialized witness entry.
    ///
    /// Payload layout:
    ///   `witness_type` (u8) + `timestamp_ns` (u64 LE) +
    ///   `action_len` (u32 LE) + `action` (bytes) + `prev_hash` (32 bytes)
    ///
    /// Returns the segment ID and byte offset where it was written.
    pub(crate) fn write_witness_seg<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        witness_type: u8,
        timestamp_ns: u64,
        action: &[u8],
        prev_hash: &[u8; 32],
    ) -> io::Result<(u64, u64)> {
        let seg_id = self.alloc_seg_id();

        let action_len = action.len() as u32;
        let payload_size = 1 + 8 + 4 + action.len() + 32;
        let mut payload = Vec::with_capacity(payload_size);

        payload.push(witness_type);
        payload.extend_from_slice(&timestamp_ns.to_le_bytes());
        payload.extend_from_slice(&action_len.to_le_bytes());
        payload.extend_from_slice(action);
        payload.extend_from_slice(prev_hash);

        let offset = self.write_segment(writer, SegmentType::Witness as u8, seg_id, &payload)?;
        Ok((seg_id, offset))
    }

    /// Low-level: write a segment header + payload to the writer.
    /// Returns the byte offset where the segment was written.
    fn write_segment<W: Write + Seek>(
        &self,
        writer: &mut W,
        seg_type: u8,
        seg_id: u64,
        payload: &[u8],
    ) -> io::Result<u64> {
        let offset = writer.stream_position()?;

        let mut header = SegmentHeader::new(seg_type, seg_id);
        header.payload_length = payload.len() as u64;

        // Compute a simple content hash (first 16 bytes of CRC-based hash).
        let hash = content_hash(payload);
        header.content_hash = hash;

        // Write header as raw bytes.
        let header_bytes = header_to_bytes(&header);
        writer.write_all(&header_bytes)?;

        // Write payload.
        writer.write_all(payload)?;

        Ok(offset)
    }

    /// Current next segment ID.
    #[allow(dead_code)]
    pub(crate) fn next_id(&self) -> u64 {
        self.next_seg_id
    }
}

/// Convert a SegmentHeader to its 64-byte wire representation.
fn header_to_bytes(h: &SegmentHeader) -> [u8; SEGMENT_HEADER_SIZE] {
    let mut buf = [0u8; SEGMENT_HEADER_SIZE];
    buf[0x00..0x04].copy_from_slice(&h.magic.to_le_bytes());
    buf[0x04] = h.version;
    buf[0x05] = h.seg_type;
    buf[0x06..0x08].copy_from_slice(&h.flags.to_le_bytes());
    buf[0x08..0x10].copy_from_slice(&h.segment_id.to_le_bytes());
    buf[0x10..0x18].copy_from_slice(&h.payload_length.to_le_bytes());
    buf[0x18..0x20].copy_from_slice(&h.timestamp_ns.to_le_bytes());
    buf[0x20] = h.checksum_algo;
    buf[0x21] = h.compression;
    buf[0x22..0x24].copy_from_slice(&h.reserved_0.to_le_bytes());
    buf[0x24..0x28].copy_from_slice(&h.reserved_1.to_le_bytes());
    buf[0x28..0x38].copy_from_slice(&h.content_hash);
    buf[0x38..0x3C].copy_from_slice(&h.uncompressed_len.to_le_bytes());
    buf[0x3C..0x40].copy_from_slice(&h.alignment_pad.to_le_bytes());
    buf
}

/// Compute a simple 16-byte content hash (CRC32-based, rotated for distinct bytes).
fn content_hash(data: &[u8]) -> [u8; 16] {
    let mut hash = [0u8; 16];
    let crc = crc32_slice(data);
    // Use different rotations of CRC to fill 16 bytes with distinct values.
    for i in 0..4 {
        let rotated = crc.rotate_left(i as u32 * 8);
        hash[i * 4..(i + 1) * 4].copy_from_slice(&rotated.to_le_bytes());
    }
    hash
}

/// Simple CRC32 computation.
fn crc32_slice(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvf_types::SEGMENT_MAGIC;
    use std::io::Cursor;

    #[test]
    fn write_vec_seg_round_trip() {
        let mut buf = Cursor::new(Vec::new());
        let mut writer = SegmentWriter::new(1);

        let v1: Vec<f32> = vec![1.0, 2.0, 3.0];
        let v2: Vec<f32> = vec![4.0, 5.0, 6.0];
        let vectors: Vec<&[f32]> = vec![&v1, &v2];
        let ids = vec![10u64, 20u64];

        let (seg_id, offset) = writer.write_vec_seg(&mut buf, &vectors, &ids, 3).unwrap();
        assert_eq!(seg_id, 1);
        assert_eq!(offset, 0);

        // Verify the data was written.
        let data = buf.into_inner();
        assert!(data.len() > SEGMENT_HEADER_SIZE);

        // Check magic.
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(magic, SEGMENT_MAGIC);

        // Check seg_type.
        assert_eq!(data[5], SegmentType::Vec as u8);
    }

    #[test]
    fn seg_id_monotonic() {
        let mut writer = SegmentWriter::new(10);
        assert_eq!(writer.alloc_seg_id(), 10);
        assert_eq!(writer.alloc_seg_id(), 11);
        assert_eq!(writer.alloc_seg_id(), 12);
    }

    #[test]
    fn header_to_bytes_size() {
        let h = SegmentHeader::new(0x01, 42);
        let bytes = header_to_bytes(&h);
        assert_eq!(bytes.len(), SEGMENT_HEADER_SIZE);
    }

    #[test]
    fn write_kernel_seg_round_trip() {
        let mut buf = Cursor::new(Vec::new());
        let mut writer = SegmentWriter::new(1);

        let kernel_header = [0xAAu8; 128];
        let kernel_image = b"fake-kernel-image-data";

        let (seg_id, offset) = writer
            .write_kernel_seg(
                &mut buf,
                &kernel_header,
                kernel_image,
                Some(b"console=ttyS0"),
            )
            .unwrap();
        assert_eq!(seg_id, 1);
        assert_eq!(offset, 0);

        let data = buf.into_inner();
        assert!(data.len() > SEGMENT_HEADER_SIZE);

        // Check magic.
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(magic, SEGMENT_MAGIC);

        // Check seg_type == Kernel (0x0E).
        assert_eq!(data[5], SegmentType::Kernel as u8);

        // Verify payload starts with kernel header bytes.
        let payload_start = SEGMENT_HEADER_SIZE;
        assert_eq!(&data[payload_start..payload_start + 128], &[0xAAu8; 128]);
    }

    #[test]
    fn write_witness_seg_round_trip() {
        let mut buf = Cursor::new(Vec::new());
        let mut writer = SegmentWriter::new(1);

        let witness_type = 0x01u8; // Computation
        let timestamp_ns = 1_700_000_000_000_000_000u64;
        let action = b"ingest:count=10,epoch=1";
        let prev_hash = [0u8; 32];

        let (seg_id, offset) = writer
            .write_witness_seg(&mut buf, witness_type, timestamp_ns, action, &prev_hash)
            .unwrap();
        assert_eq!(seg_id, 1);
        assert_eq!(offset, 0);

        let data = buf.into_inner();
        assert!(data.len() > SEGMENT_HEADER_SIZE);

        // Check magic.
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(magic, SEGMENT_MAGIC);

        // Check seg_type == Witness (0x0A).
        assert_eq!(data[5], SegmentType::Witness as u8);

        // Verify payload starts with witness_type byte.
        let payload_start = SEGMENT_HEADER_SIZE;
        assert_eq!(data[payload_start], witness_type);

        // Verify timestamp.
        let ts_bytes: [u8; 8] = data[payload_start + 1..payload_start + 9]
            .try_into()
            .unwrap();
        assert_eq!(u64::from_le_bytes(ts_bytes), timestamp_ns);

        // Verify action length.
        let action_len_bytes: [u8; 4] = data[payload_start + 9..payload_start + 13]
            .try_into()
            .unwrap();
        assert_eq!(u32::from_le_bytes(action_len_bytes), action.len() as u32);

        // Verify action bytes.
        let action_start = payload_start + 13;
        let action_end = action_start + action.len();
        assert_eq!(&data[action_start..action_end], action);

        // Verify prev_hash (32 zero bytes).
        let hash_start = action_end;
        let hash_end = hash_start + 32;
        assert_eq!(&data[hash_start..hash_end], &[0u8; 32]);
    }

    #[test]
    fn write_ebpf_seg_round_trip() {
        let mut buf = Cursor::new(Vec::new());
        let mut writer = SegmentWriter::new(10);

        let ebpf_header = [0xBBu8; 64];
        let bytecode = b"ebpf-bytecode";

        let (seg_id, offset) = writer
            .write_ebpf_seg(&mut buf, &ebpf_header, bytecode, None)
            .unwrap();
        assert_eq!(seg_id, 10);
        assert_eq!(offset, 0);

        let data = buf.into_inner();
        assert!(data.len() > SEGMENT_HEADER_SIZE);

        // Check seg_type == Ebpf (0x0F).
        assert_eq!(data[5], SegmentType::Ebpf as u8);

        // Verify payload starts with eBPF header bytes.
        let payload_start = SEGMENT_HEADER_SIZE;
        assert_eq!(&data[payload_start..payload_start + 64], &[0xBBu8; 64]);
    }
}
