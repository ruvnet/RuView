//! INDEX_SEG codec.
//!
//! Reads and writes HNSW index segments: the index header, restart point
//! index, and adjacency data with varint delta encoding.

use crate::varint::{decode_varint, encode_varint, MAX_VARINT_LEN};

const ALIGN: usize = 64;

fn align_up(n: usize) -> usize {
    (n + ALIGN - 1) & !(ALIGN - 1)
}

/// Index header at the start of an INDEX_SEG payload.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexHeader {
    /// Index type: 0=HNSW, 1=IVF, 2=flat.
    pub index_type: u8,
    /// HNSW layer level (A=0, B=1, C=2).
    pub layer_level: u8,
    /// Maximum neighbors per layer.
    pub m: u16,
    /// HNSW ef_construction parameter.
    pub ef_construction: u32,
    /// Number of nodes in this index segment.
    pub node_count: u64,
}

/// INDEX_SEG header size before padding: 1+1+2+4+8 = 16 bytes.
const INDEX_HEADER_RAW_SIZE: usize = 16;

/// Restart point index.
#[derive(Clone, Debug, PartialEq)]
pub struct RestartPointIndex {
    pub restart_interval: u32,
    pub offsets: Vec<u32>,
}

/// A single node's adjacency information.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeAdjacency {
    /// Neighbor lists, one per layer. Each layer is a sorted list of node IDs.
    pub layers: Vec<Vec<u64>>,
}

/// Write an INDEX_SEG payload from the header, restart index, and adjacency data.
pub fn write_index_seg(
    header: &IndexHeader,
    restart: &RestartPointIndex,
    adjacency: &[NodeAdjacency],
) -> Vec<u8> {
    let mut buf = Vec::new();

    // Index header
    buf.push(header.index_type);
    buf.push(header.layer_level);
    buf.extend_from_slice(&header.m.to_le_bytes());
    buf.extend_from_slice(&header.ef_construction.to_le_bytes());
    buf.extend_from_slice(&header.node_count.to_le_bytes());
    buf.resize(align_up(buf.len()), 0);

    // Restart point index
    let restart_count = restart.offsets.len() as u32;
    buf.extend_from_slice(&restart.restart_interval.to_le_bytes());
    buf.extend_from_slice(&restart_count.to_le_bytes());
    for &offset in &restart.offsets {
        buf.extend_from_slice(&offset.to_le_bytes());
    }
    buf.resize(align_up(buf.len()), 0);

    // Adjacency data
    let mut tmp = [0u8; MAX_VARINT_LEN];
    for node in adjacency {
        let n = encode_varint(node.layers.len() as u64, &mut tmp);
        buf.extend_from_slice(&tmp[..n]);
        for layer in &node.layers {
            let n = encode_varint(layer.len() as u64, &mut tmp);
            buf.extend_from_slice(&tmp[..n]);
            // Delta-encode neighbor IDs within each layer
            let mut prev = 0u64;
            for &nid in layer {
                let delta = nid - prev;
                let n = encode_varint(delta, &mut tmp);
                buf.extend_from_slice(&tmp[..n]);
                prev = nid;
            }
        }
    }
    // Pad to 64-byte boundary
    buf.resize(align_up(buf.len()), 0);

    buf
}

/// Read the INDEX_SEG header from the start of the payload.
///
/// Returns the header and the byte offset after the 64-byte aligned header.
pub fn read_index_header(data: &[u8]) -> Result<(IndexHeader, usize), &'static str> {
    if data.len() < INDEX_HEADER_RAW_SIZE {
        return Err("index header truncated");
    }
    let header = IndexHeader {
        index_type: data[0],
        layer_level: data[1],
        m: u16::from_le_bytes([data[2], data[3]]),
        ef_construction: u32::from_le_bytes(data[4..8].try_into().unwrap()),
        node_count: u64::from_le_bytes(data[8..16].try_into().unwrap()),
    };
    Ok((header, align_up(INDEX_HEADER_RAW_SIZE)))
}

/// Read the restart point index from the payload at the given offset.
///
/// Returns the restart index and the byte offset after it (64-byte aligned).
pub fn read_restart_index(data: &[u8]) -> Result<(RestartPointIndex, usize), &'static str> {
    if data.len() < 8 {
        return Err("restart index truncated");
    }
    let restart_interval = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let restart_count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let offsets_end = 8 + restart_count * 4;
    if data.len() < offsets_end {
        return Err("restart offsets truncated");
    }
    let mut offsets = Vec::with_capacity(restart_count);
    for i in 0..restart_count {
        let base = 8 + i * 4;
        offsets.push(u32::from_le_bytes(data[base..base + 4].try_into().unwrap()));
    }
    let consumed = align_up(offsets_end);
    Ok((
        RestartPointIndex {
            restart_interval,
            offsets,
        },
        consumed,
    ))
}

/// Read adjacency data for `node_count` nodes from the payload.
///
/// Each node stores: layer_count(varint), then for each layer:
/// neighbor_count(varint) followed by delta-encoded neighbor IDs (varints).
pub fn read_adjacency(data: &[u8], node_count: u64) -> Result<Vec<NodeAdjacency>, &'static str> {
    let mut nodes = Vec::with_capacity(node_count as usize);
    let mut pos = 0;
    for _ in 0..node_count {
        let (layer_count, consumed) =
            decode_varint(&data[pos..]).map_err(|_| "adjacency layer_count decode failed")?;
        pos += consumed;
        let mut layers = Vec::with_capacity(layer_count as usize);
        for _ in 0..layer_count {
            let (neighbor_count, consumed) = decode_varint(&data[pos..])
                .map_err(|_| "adjacency neighbor_count decode failed")?;
            pos += consumed;
            let mut neighbors = Vec::with_capacity(neighbor_count as usize);
            let mut prev = 0u64;
            for _ in 0..neighbor_count {
                let (delta, consumed) =
                    decode_varint(&data[pos..]).map_err(|_| "adjacency delta decode failed")?;
                pos += consumed;
                prev += delta;
                neighbors.push(prev);
            }
            layers.push(neighbors);
        }
        nodes.push(NodeAdjacency { layers });
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_header_round_trip() {
        let header = IndexHeader {
            index_type: 0,
            layer_level: 1,
            m: 16,
            ef_construction: 200,
            node_count: 10000,
        };
        let restart = RestartPointIndex {
            restart_interval: 64,
            offsets: vec![0, 1024, 2048],
        };
        let adjacency = vec![]; // empty for header test
        let buf = write_index_seg(&header, &restart, &adjacency);

        let (decoded_header, header_end) = read_index_header(&buf).unwrap();
        assert_eq!(decoded_header, header);

        let (decoded_restart, _restart_end) = read_restart_index(&buf[header_end..]).unwrap();
        assert_eq!(decoded_restart, restart);
    }

    #[test]
    fn adjacency_round_trip() {
        let header = IndexHeader {
            index_type: 0,
            layer_level: 0,
            m: 8,
            ef_construction: 100,
            node_count: 3,
        };
        let restart = RestartPointIndex {
            restart_interval: 64,
            offsets: vec![0],
        };
        let adjacency = vec![
            NodeAdjacency {
                layers: vec![vec![10, 20, 30]],
            },
            NodeAdjacency {
                layers: vec![vec![5, 15, 25, 35]],
            },
            NodeAdjacency {
                layers: vec![vec![100, 200], vec![50]],
            },
        ];

        let buf = write_index_seg(&header, &restart, &adjacency);
        let (_, header_end) = read_index_header(&buf).unwrap();
        let (_, restart_end) = read_restart_index(&buf[header_end..]).unwrap();
        let adj_data = &buf[header_end + restart_end..];

        let decoded = read_adjacency(adj_data, 3).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].layers[0], vec![10, 20, 30]);
        assert_eq!(decoded[1].layers[0], vec![5, 15, 25, 35]);
        assert_eq!(decoded[2].layers[0], vec![100, 200]);
        assert_eq!(decoded[2].layers[1], vec![50]);
    }

    #[test]
    fn empty_index() {
        let header = IndexHeader {
            index_type: 2,
            layer_level: 0,
            m: 0,
            ef_construction: 0,
            node_count: 0,
        };
        let restart = RestartPointIndex {
            restart_interval: 1,
            offsets: vec![],
        };
        let buf = write_index_seg(&header, &restart, &[]);

        let (decoded_header, header_end) = read_index_header(&buf).unwrap();
        assert_eq!(decoded_header.node_count, 0);

        let (decoded_restart, restart_end) = read_restart_index(&buf[header_end..]).unwrap();
        assert!(decoded_restart.offsets.is_empty());

        let adj_data = &buf[header_end + restart_end..];
        let decoded_adj = read_adjacency(adj_data, 0).unwrap();
        assert!(decoded_adj.is_empty());
    }

    #[test]
    fn alignment() {
        let header = IndexHeader {
            index_type: 0,
            layer_level: 0,
            m: 16,
            ef_construction: 200,
            node_count: 1,
        };
        let restart = RestartPointIndex {
            restart_interval: 64,
            offsets: vec![0],
        };
        let adjacency = vec![NodeAdjacency {
            layers: vec![vec![1, 2, 3, 4, 5]],
        }];
        let buf = write_index_seg(&header, &restart, &adjacency);
        assert_eq!(buf.len() % 64, 0);
    }
}
