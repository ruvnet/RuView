//! INDEX_SEG encode/decode: varint delta encoding with restart points.
//!
//! Implements the binary layout from the RVF wire spec for INDEX_SEG payloads.

extern crate alloc;

use alloc::vec::Vec;

/// Default restart interval for varint delta encoding.
pub const DEFAULT_RESTART_INTERVAL: u32 = 64;

/// Index segment header (64-byte aligned).
#[derive(Clone, Debug, PartialEq)]
pub struct IndexSegHeader {
    /// 0 = HNSW, 1 = IVF, 2 = flat.
    pub index_type: u8,
    /// Layer level: 0 = A, 1 = B, 2 = C.
    pub layer_level: u8,
    /// HNSW max neighbors per layer.
    pub m: u16,
    /// ef_construction parameter.
    pub ef_construction: u32,
    /// Number of nodes in this segment.
    pub node_count: u64,
}

/// Encoded adjacency data for a single node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeAdjacency {
    /// The node ID.
    pub node_id: u64,
    /// Neighbor IDs per HNSW layer (index 0 = layer 0).
    pub layers: Vec<Vec<u64>>,
}

/// Full decoded index segment data.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexSegData {
    pub header: IndexSegHeader,
    pub restart_interval: u32,
    pub nodes: Vec<NodeAdjacency>,
}

// ── Varint Encoding (LEB128) ─────────────────────────────────────

/// Encode a u64 as LEB128 varint.
pub fn encode_varint(mut value: u64, buf: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Decode a LEB128 varint from a byte slice. Returns `(value, bytes_consumed)`.
pub fn decode_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in data.iter().enumerate() {
        if shift >= 64 {
            return None; // Overflow.
        }
        value |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None // Incomplete.
}

// ── Delta Encoding ───────────────────────────────────────────────

/// Delta-encode a sorted sequence of u64 values.
pub fn delta_encode(sorted_ids: &[u64]) -> Vec<u64> {
    if sorted_ids.is_empty() {
        return Vec::new();
    }
    let mut deltas = Vec::with_capacity(sorted_ids.len());
    deltas.push(sorted_ids[0]);
    for i in 1..sorted_ids.len() {
        deltas.push(sorted_ids[i] - sorted_ids[i - 1]);
    }
    deltas
}

/// Decode delta-encoded values back to absolute IDs.
pub fn delta_decode(deltas: &[u64]) -> Vec<u64> {
    if deltas.is_empty() {
        return Vec::new();
    }
    let mut ids = Vec::with_capacity(deltas.len());
    ids.push(deltas[0]);
    for i in 1..deltas.len() {
        ids.push(ids[i - 1] + deltas[i]);
    }
    ids
}

// ── INDEX_SEG Encode ─────────────────────────────────────────────

/// Encode an INDEX_SEG payload.
///
/// Layout:
/// 1. Index header (padded to 64 bytes)
/// 2. Restart point index (padded to 64 bytes)
/// 3. Adjacency data with delta-encoded neighbor lists
pub fn encode_index_seg(data: &IndexSegData) -> Vec<u8> {
    let mut buf = Vec::new();

    // 1. Header (pad to 64 bytes).
    buf.push(data.header.index_type);
    buf.push(data.header.layer_level);
    buf.extend_from_slice(&data.header.m.to_le_bytes());
    buf.extend_from_slice(&data.header.ef_construction.to_le_bytes());
    buf.extend_from_slice(&data.header.node_count.to_le_bytes());
    pad_to_alignment(&mut buf, 64);

    // 2. Encode adjacency data with restart points.
    let restart_interval = data.restart_interval;
    let mut adj_buf = Vec::new();
    let mut restart_offsets: Vec<u32> = Vec::new();

    for (idx, node) in data.nodes.iter().enumerate() {
        if (idx as u32).is_multiple_of(restart_interval) {
            restart_offsets.push(adj_buf.len() as u32);
        }

        // Encode layer count.
        encode_varint(node.layers.len() as u64, &mut adj_buf);

        // Encode each layer's neighbors.
        for neighbors in &node.layers {
            encode_varint(neighbors.len() as u64, &mut adj_buf);
            // Delta-encode sorted neighbor IDs.
            let mut sorted = neighbors.clone();
            sorted.sort();

            let is_restart = (idx as u32).is_multiple_of(restart_interval);
            if is_restart {
                // At restart points, encode absolute IDs.
                for &nid in &sorted {
                    encode_varint(nid, &mut adj_buf);
                }
            } else {
                // Delta encode.
                let deltas = delta_encode(&sorted);
                for &d in &deltas {
                    encode_varint(d, &mut adj_buf);
                }
            }
        }
    }

    // Write restart point index.
    buf.extend_from_slice(&restart_interval.to_le_bytes());
    let restart_count = restart_offsets.len() as u32;
    buf.extend_from_slice(&restart_count.to_le_bytes());
    for offset in &restart_offsets {
        buf.extend_from_slice(&offset.to_le_bytes());
    }
    pad_to_alignment(&mut buf, 64);

    // Write adjacency data.
    buf.extend_from_slice(&adj_buf);
    pad_to_alignment(&mut buf, 64);

    buf
}

/// Decode an INDEX_SEG payload.
pub fn decode_index_seg(data: &[u8]) -> Result<IndexSegData, CodecError> {
    if data.len() < 64 {
        return Err(CodecError::TooShort);
    }

    // 1. Parse header.
    let index_type = data[0];
    let layer_level = data[1];
    let m = u16::from_le_bytes([data[2], data[3]]);
    let ef_construction = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let node_count = u64::from_le_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let header = IndexSegHeader {
        index_type,
        layer_level,
        m,
        ef_construction,
        node_count,
    };

    // Skip header padding.
    let mut pos = 64;

    // 2. Parse restart point index.
    if pos + 8 > data.len() {
        return Err(CodecError::TooShort);
    }
    let restart_interval =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
    pos += 4;
    let restart_count =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
    pos += 4;

    let mut restart_offsets = Vec::with_capacity(restart_count as usize);
    for _ in 0..restart_count {
        if pos + 4 > data.len() {
            return Err(CodecError::TooShort);
        }
        let offset = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        restart_offsets.push(offset);
        pos += 4;
    }

    // Skip padding to 64-byte alignment.
    pos = align_up(pos, 64);

    // 3. Parse adjacency data.
    let adj_start = pos;
    let adj_data = &data[adj_start..];

    let mut nodes = Vec::new();
    let mut adj_pos = 0;

    for node_idx in 0..node_count as usize {
        let is_restart = (node_idx as u32).is_multiple_of(restart_interval);

        // Decode layer count.
        let (layer_count, consumed) =
            decode_varint(&adj_data[adj_pos..]).ok_or(CodecError::InvalidVarint)?;
        adj_pos += consumed;

        let mut layers = Vec::with_capacity(layer_count as usize);

        for _ in 0..layer_count {
            let (neighbor_count, consumed) =
                decode_varint(&adj_data[adj_pos..]).ok_or(CodecError::InvalidVarint)?;
            adj_pos += consumed;

            let mut neighbor_ids = Vec::with_capacity(neighbor_count as usize);

            if is_restart {
                // Absolute IDs at restart points.
                for _ in 0..neighbor_count {
                    let (nid, consumed) =
                        decode_varint(&adj_data[adj_pos..]).ok_or(CodecError::InvalidVarint)?;
                    adj_pos += consumed;
                    neighbor_ids.push(nid);
                }
            } else {
                // Delta-encoded IDs.
                let mut deltas = Vec::with_capacity(neighbor_count as usize);
                for _ in 0..neighbor_count {
                    let (d, consumed) =
                        decode_varint(&adj_data[adj_pos..]).ok_or(CodecError::InvalidVarint)?;
                    adj_pos += consumed;
                    deltas.push(d);
                }
                neighbor_ids = delta_decode(&deltas);
            }

            layers.push(neighbor_ids);
        }

        nodes.push(NodeAdjacency {
            node_id: node_idx as u64,
            layers,
        });
    }

    Ok(IndexSegData {
        header,
        restart_interval,
        nodes,
    })
}

/// Errors that can occur during INDEX_SEG codec operations.
#[derive(Clone, Debug, PartialEq)]
pub enum CodecError {
    /// Input data is shorter than expected.
    TooShort,
    /// Invalid varint encountered.
    InvalidVarint,
    /// Unknown index type.
    UnknownIndexType(u8),
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "input data too short"),
            Self::InvalidVarint => write!(f, "invalid varint encoding"),
            Self::UnknownIndexType(t) => write!(f, "unknown index type: {}", t),
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────

/// Pad `buf` with zeros to the next multiple of `alignment`.
fn pad_to_alignment(buf: &mut Vec<u8>, alignment: usize) {
    let rem = buf.len() % alignment;
    if rem != 0 {
        buf.resize(buf.len() + (alignment - rem), 0);
    }
}

/// Round `offset` up to the next multiple of `alignment`.
fn align_up(offset: usize, alignment: usize) -> usize {
    let rem = offset % alignment;
    if rem == 0 {
        offset
    } else {
        offset + (alignment - rem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        let values = [0, 1, 127, 128, 16383, 16384, 2097151, u64::MAX];
        for &val in &values {
            let mut buf = Vec::new();
            encode_varint(val, &mut buf);
            let (decoded, consumed) = decode_varint(&buf).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn varint_encoding_sizes() {
        let mut buf = Vec::new();

        encode_varint(0, &mut buf);
        assert_eq!(buf.len(), 1);
        buf.clear();

        encode_varint(127, &mut buf);
        assert_eq!(buf.len(), 1);
        buf.clear();

        encode_varint(128, &mut buf);
        assert_eq!(buf.len(), 2);
        buf.clear();

        encode_varint(16383, &mut buf);
        assert_eq!(buf.len(), 2);
        buf.clear();

        encode_varint(16384, &mut buf);
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn delta_encode_decode_round_trip() {
        let ids = vec![100, 105, 108, 120, 200];
        let deltas = delta_encode(&ids);
        assert_eq!(deltas, vec![100, 5, 3, 12, 80]);
        let decoded = delta_decode(&deltas);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn delta_encode_empty() {
        assert!(delta_encode(&[]).is_empty());
        assert!(delta_decode(&[]).is_empty());
    }

    #[test]
    fn index_seg_round_trip() {
        let data = IndexSegData {
            header: IndexSegHeader {
                index_type: 0,  // HNSW
                layer_level: 2, // Layer C
                m: 16,
                ef_construction: 200,
                node_count: 5,
            },
            restart_interval: 3,
            nodes: vec![
                NodeAdjacency {
                    node_id: 0,
                    layers: vec![vec![1, 2, 3], vec![1]],
                },
                NodeAdjacency {
                    node_id: 1,
                    layers: vec![vec![0, 2, 4]],
                },
                NodeAdjacency {
                    node_id: 2,
                    layers: vec![vec![0, 1, 3, 4]],
                },
                NodeAdjacency {
                    node_id: 3,
                    layers: vec![vec![0, 2, 4], vec![4]],
                },
                NodeAdjacency {
                    node_id: 4,
                    layers: vec![vec![1, 2, 3]],
                },
            ],
        };

        let encoded = encode_index_seg(&data);
        let decoded = decode_index_seg(&encoded).unwrap();

        assert_eq!(decoded.header, data.header);
        assert_eq!(decoded.restart_interval, data.restart_interval);
        assert_eq!(decoded.nodes.len(), data.nodes.len());

        // Verify each node's adjacency. Note: neighbors are sorted during encoding.
        for (orig, dec) in data.nodes.iter().zip(decoded.nodes.iter()) {
            assert_eq!(dec.node_id, orig.node_id);
            assert_eq!(dec.layers.len(), orig.layers.len());
            for (ol, dl) in orig.layers.iter().zip(dec.layers.iter()) {
                let mut sorted_orig = ol.clone();
                sorted_orig.sort();
                assert_eq!(*dl, sorted_orig);
            }
        }
    }

    #[test]
    fn index_seg_larger_with_restart() {
        // Test with enough nodes to exercise multiple restart groups.
        let num_nodes = 200;
        let restart_interval = 64;
        let nodes: Vec<NodeAdjacency> = (0..num_nodes)
            .map(|i| {
                let neighbors: Vec<u64> =
                    (0..8).map(|j| ((i + j + 1) % num_nodes) as u64).collect();
                NodeAdjacency {
                    node_id: i as u64,
                    layers: vec![neighbors],
                }
            })
            .collect();

        let data = IndexSegData {
            header: IndexSegHeader {
                index_type: 0,
                layer_level: 2,
                m: 16,
                ef_construction: 200,
                node_count: num_nodes as u64,
            },
            restart_interval,
            nodes,
        };

        let encoded = encode_index_seg(&data);
        let decoded = decode_index_seg(&encoded).unwrap();

        assert_eq!(decoded.header, data.header);
        assert_eq!(decoded.nodes.len(), data.nodes.len());

        for (orig, dec) in data.nodes.iter().zip(decoded.nodes.iter()) {
            assert_eq!(dec.layers.len(), orig.layers.len());
            for (ol, dl) in orig.layers.iter().zip(dec.layers.iter()) {
                let mut sorted_orig = ol.clone();
                sorted_orig.sort();
                assert_eq!(*dl, sorted_orig);
            }
        }
    }

    #[test]
    fn delta_encoding_sorted_u64_sequences() {
        // Verify exact round-trip for various sorted u64 sequences.
        let sequences: Vec<Vec<u64>> = vec![
            vec![0, 1, 2, 3, 4],
            vec![1000, 2000, 3000, 4000],
            vec![0, 100, 200, 300, 400, 500],
            vec![
                u64::MAX - 4,
                u64::MAX - 3,
                u64::MAX - 2,
                u64::MAX - 1,
                u64::MAX,
            ],
        ];

        for seq in sequences {
            let deltas = delta_encode(&seq);
            let decoded = delta_decode(&deltas);
            assert_eq!(decoded, seq, "Failed for sequence: {:?}", seq);
        }
    }
}
