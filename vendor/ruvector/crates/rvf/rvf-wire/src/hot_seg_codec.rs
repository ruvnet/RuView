//! HOT_SEG codec.
//!
//! The hot segment stores the most-accessed vectors in an interleaved
//! (row-major) layout with their neighbor lists co-located for cache
//! locality. Each entry is 64-byte aligned.

/// Hot segment header, stored at the start of the HOT_SEG payload.
#[derive(Clone, Debug, PartialEq)]
pub struct HotHeader {
    pub vector_count: u32,
    pub dim: u16,
    pub dtype: u8,
    pub neighbor_m: u16,
}

/// A single interleaved hot entry.
#[derive(Clone, Debug, PartialEq)]
pub struct HotEntry {
    pub vector_id: u64,
    pub vector_data: Vec<u8>,
    pub neighbor_ids: Vec<u64>,
}

/// Size of an element in bytes for a given dtype.
fn dtype_element_size(dtype: u8) -> usize {
    match dtype {
        0x00 => 4, // f32
        0x01 => 2, // f16
        0x02 => 2, // bf16
        0x03 => 1, // i8
        0x04 => 1, // u8
        _ => 1,
    }
}

const ALIGN: usize = 64;

fn align_up(n: usize) -> usize {
    (n + ALIGN - 1) & !(ALIGN - 1)
}

/// Write the HOT_SEG payload from a header and a list of hot entries.
///
/// The header is padded to 64 bytes. Each entry is individually 64-byte
/// aligned.
pub fn write_hot_seg(header: &HotHeader, entries: &[HotEntry]) -> Vec<u8> {
    let mut buf = Vec::new();

    // Hot header: vector_count(4) + dim(2) + dtype(1) + neighbor_M(2) = 9 bytes
    buf.extend_from_slice(&header.vector_count.to_le_bytes());
    buf.extend_from_slice(&header.dim.to_le_bytes());
    buf.push(header.dtype);
    buf.extend_from_slice(&header.neighbor_m.to_le_bytes());
    // Pad header to 64 bytes
    buf.resize(align_up(buf.len()), 0);

    // Write each entry, 64-byte aligned
    for entry in entries {
        let entry_start = buf.len();
        // vector_id: u64
        buf.extend_from_slice(&entry.vector_id.to_le_bytes());
        // vector data: dtype * dim bytes
        buf.extend_from_slice(&entry.vector_data);
        // neighbor_count: u16
        let neighbor_count = entry.neighbor_ids.len() as u16;
        buf.extend_from_slice(&neighbor_count.to_le_bytes());
        // neighbor_ids: u64 * count
        for &nid in &entry.neighbor_ids {
            buf.extend_from_slice(&nid.to_le_bytes());
        }
        // Pad this entry to 64-byte alignment
        let entry_raw_size = buf.len() - entry_start;
        let entry_padded = align_up(entry_raw_size);
        buf.resize(entry_start + entry_padded, 0);
    }

    buf
}

/// Read the HOT_SEG header from the start of the payload.
///
/// Returns the header and the byte offset after the (64-byte aligned) header.
pub fn read_hot_header(data: &[u8]) -> Result<(HotHeader, usize), &'static str> {
    if data.len() < 9 {
        return Err("hot header truncated");
    }
    let vector_count = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let dim = u16::from_le_bytes([data[4], data[5]]);
    let dtype = data[6];
    let neighbor_m = u16::from_le_bytes([data[7], data[8]]);
    let consumed = align_up(9);
    Ok((
        HotHeader {
            vector_count,
            dim,
            dtype,
            neighbor_m,
        },
        consumed,
    ))
}

/// Read all hot entries from the payload (after the header).
///
/// `data` should start at the first entry (after the aligned header).
pub fn read_hot_entries(data: &[u8], header: &HotHeader) -> Result<Vec<HotEntry>, &'static str> {
    let elem_size = dtype_element_size(header.dtype);
    let vector_byte_len = header.dim as usize * elem_size;
    let mut entries = Vec::with_capacity(header.vector_count as usize);
    let mut pos = 0;

    for _ in 0..header.vector_count {
        let entry_start = pos;
        // Need at least: 8 (id) + vector_byte_len + 2 (neighbor_count)
        let min_size = 8 + vector_byte_len + 2;
        if data.len() < pos + min_size {
            return Err("hot entry truncated");
        }
        let vector_id = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let vector_data = data[pos..pos + vector_byte_len].to_vec();
        pos += vector_byte_len;
        let neighbor_count = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if data.len() < pos + neighbor_count * 8 {
            return Err("neighbor IDs truncated");
        }
        let mut neighbor_ids = Vec::with_capacity(neighbor_count);
        for _ in 0..neighbor_count {
            neighbor_ids.push(u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()));
            pos += 8;
        }
        entries.push(HotEntry {
            vector_id,
            vector_data,
            neighbor_ids,
        });
        // Advance to next 64-byte boundary
        let entry_raw_size = pos - entry_start;
        pos = entry_start + align_up(entry_raw_size);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_entries(dim: u16, dtype: u8, count: u32, neighbor_count: usize) -> Vec<HotEntry> {
        let elem_size = dtype_element_size(dtype);
        let vector_byte_len = dim as usize * elem_size;
        (0..count)
            .map(|i| HotEntry {
                vector_id: (i as u64) * 100 + 1,
                vector_data: vec![(i % 256) as u8; vector_byte_len],
                neighbor_ids: (0..neighbor_count as u64).map(|n| n + 1000).collect(),
            })
            .collect()
    }

    #[test]
    fn round_trip_hot_seg() {
        let dim = 4u16;
        let dtype = 0u8; // f32
        let entries = make_test_entries(dim, dtype, 3, 2);
        let header = HotHeader {
            vector_count: 3,
            dim,
            dtype,
            neighbor_m: 16,
        };
        let payload = write_hot_seg(&header, &entries);

        let (decoded_header, header_end) = read_hot_header(&payload).unwrap();
        assert_eq!(decoded_header, header);

        let decoded_entries = read_hot_entries(&payload[header_end..], &decoded_header).unwrap();
        assert_eq!(decoded_entries.len(), 3);
        assert_eq!(decoded_entries[0].vector_id, 1);
        assert_eq!(decoded_entries[1].vector_id, 101);
        assert_eq!(decoded_entries[2].vector_id, 201);
        for (orig, dec) in entries.iter().zip(decoded_entries.iter()) {
            assert_eq!(orig.vector_data, dec.vector_data);
            assert_eq!(orig.neighbor_ids, dec.neighbor_ids);
        }
    }

    #[test]
    fn empty_hot_seg() {
        let header = HotHeader {
            vector_count: 0,
            dim: 128,
            dtype: 1,
            neighbor_m: 16,
        };
        let payload = write_hot_seg(&header, &[]);
        let (decoded_header, header_end) = read_hot_header(&payload).unwrap();
        assert_eq!(decoded_header.vector_count, 0);
        let entries = read_hot_entries(&payload[header_end..], &decoded_header).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn alignment_respected() {
        let dim = 384u16;
        let dtype = 1u8; // f16
        let entries = make_test_entries(dim, dtype, 2, 16);
        let header = HotHeader {
            vector_count: 2,
            dim,
            dtype,
            neighbor_m: 16,
        };
        let payload = write_hot_seg(&header, &entries);
        // Total payload should be aligned
        assert_eq!(payload.len() % 64, 0);
    }
}
