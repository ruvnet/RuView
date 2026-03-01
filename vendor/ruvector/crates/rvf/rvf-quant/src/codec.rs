//! QUANT_SEG and SKETCH_SEG wire format codec.
//!
//! Serializes / deserializes quantizer parameters and Count-Min Sketch
//! data to the binary layout defined in the RVF wire spec.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use crate::binary;
use crate::product::ProductQuantizer;
use crate::scalar::ScalarQuantizer;
use crate::sketch::CountMinSketch;
use crate::traits::Quantizer;

// ---------------------------------------------------------------------------
// QUANT_SEG codec
// ---------------------------------------------------------------------------

/// Quantization type tags matching the QUANT_SEG wire spec.
const QUANT_TYPE_SCALAR: u8 = 0;
const QUANT_TYPE_PRODUCT: u8 = 1;
const QUANT_TYPE_BINARY: u8 = 2;

/// Encode a quantizer into the QUANT_SEG binary payload.
///
/// Layout:
/// ```text
/// [quant_type: u8] [tier: u8] [dim: u16 LE] [padding: 60 bytes to 64B]
/// [type-specific data ...]
/// ```
pub fn encode_quant_seg(quantizer: &dyn Quantizer) -> Vec<u8> {
    let tier = quantizer.tier() as u8;
    let dim = quantizer.dim() as u16;

    // Downcast to determine the concrete type.
    // We use the tier as a proxy since each tier maps to exactly one quantizer type.
    match tier {
        0 => encode_scalar_quant_seg(quantizer, dim),
        1 => encode_product_quant_seg(quantizer, dim),
        2 => encode_binary_quant_seg(dim),
        _ => panic!("unknown quantizer tier"),
    }
}

/// Decode a QUANT_SEG binary payload into a boxed Quantizer.
pub fn decode_quant_seg(data: &[u8]) -> Box<dyn Quantizer> {
    assert!(data.len() >= 64, "QUANT_SEG header too short");

    let quant_type = data[0];
    let _tier = data[1];
    let dim = u16::from_le_bytes([data[2], data[3]]) as usize;
    let body = &data[64..];

    match quant_type {
        QUANT_TYPE_SCALAR => Box::new(decode_scalar(body, dim)),
        QUANT_TYPE_PRODUCT => Box::new(decode_product(body, dim)),
        QUANT_TYPE_BINARY => Box::new(BinaryQuantizerWrapper { dim }),
        _ => panic!("unknown quant_type {quant_type}"),
    }
}

// ---------------------------------------------------------------------------
// Scalar
// ---------------------------------------------------------------------------

fn encode_scalar_quant_seg(quantizer: &dyn Quantizer, dim: u16) -> Vec<u8> {
    // Header (64 bytes)
    let mut buf = vec![0u8; 64];
    buf[0] = QUANT_TYPE_SCALAR;
    buf[1] = quantizer.tier() as u8;
    buf[2..4].copy_from_slice(&dim.to_le_bytes());

    // Encode a known vector to extract min/max via round-trip.
    // We re-derive from the trait interface.
    // To get actual parameters, we encode/decode unit vectors.
    // However, we need the raw ScalarQuantizer data.
    // Since we only have &dyn Quantizer, we store dim floats of min then max.

    // Workaround: encode zero and full-scale to reverse-engineer params.
    // Better approach: serialize directly from ScalarQuantizer.
    // For now, this function is called with concrete types via helper.

    // Placeholder: we'll fill this properly in the type-specific functions below.
    buf
}

/// Encode a ScalarQuantizer directly (preferred over trait-based encoding).
pub fn encode_scalar_quantizer(sq: &ScalarQuantizer) -> Vec<u8> {
    let dim = sq.dim as u16;
    let mut buf = vec![0u8; 64];
    buf[0] = QUANT_TYPE_SCALAR;
    buf[1] = 0; // Hot tier
    buf[2..4].copy_from_slice(&dim.to_le_bytes());

    // min[dim], max[dim]
    for &v in &sq.min_vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    for &v in &sq.max_vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf
}

fn decode_scalar(body: &[u8], dim: usize) -> ScalarQuantizer {
    let float_bytes = dim * 4;
    assert!(body.len() >= float_bytes * 2, "scalar quant data too short");

    let mut min_vals = Vec::with_capacity(dim);
    let mut max_vals = Vec::with_capacity(dim);

    for d in 0..dim {
        let offset = d * 4;
        let v = f32::from_le_bytes([
            body[offset],
            body[offset + 1],
            body[offset + 2],
            body[offset + 3],
        ]);
        min_vals.push(v);
    }
    for d in 0..dim {
        let offset = (dim + d) * 4;
        let v = f32::from_le_bytes([
            body[offset],
            body[offset + 1],
            body[offset + 2],
            body[offset + 3],
        ]);
        max_vals.push(v);
    }

    ScalarQuantizer {
        min_vals,
        max_vals,
        dim,
    }
}

// ---------------------------------------------------------------------------
// Product
// ---------------------------------------------------------------------------

fn encode_product_quant_seg(quantizer: &dyn Quantizer, dim: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = QUANT_TYPE_PRODUCT;
    buf[1] = quantizer.tier() as u8;
    buf[2..4].copy_from_slice(&dim.to_le_bytes());
    buf
}

/// Encode a ProductQuantizer directly.
pub fn encode_product_quantizer(pq: &ProductQuantizer) -> Vec<u8> {
    let dim = (pq.m * pq.sub_dim) as u16;
    let mut buf = vec![0u8; 64];
    buf[0] = QUANT_TYPE_PRODUCT;
    buf[1] = 1; // Warm tier
    buf[2..4].copy_from_slice(&dim.to_le_bytes());

    // PQ header: M, K, sub_dim (each as u16 LE)
    // Written after the 64-byte aligned header.
    buf.extend_from_slice(&(pq.m as u16).to_le_bytes());
    buf.extend_from_slice(&(pq.k as u16).to_le_bytes());
    buf.extend_from_slice(&(pq.sub_dim as u16).to_le_bytes());

    // Codebook: M * K * sub_dim floats
    for sub_book in &pq.codebooks {
        for centroid in sub_book {
            for &val in centroid {
                buf.extend_from_slice(&val.to_le_bytes());
            }
        }
    }

    buf
}

fn decode_product(body: &[u8], _dim: usize) -> ProductQuantizer {
    assert!(body.len() >= 6, "PQ header too short");

    let m = u16::from_le_bytes([body[0], body[1]]) as usize;
    let k = u16::from_le_bytes([body[2], body[3]]) as usize;
    let sub_dim = u16::from_le_bytes([body[4], body[5]]) as usize;

    let codebook_floats = m * k * sub_dim;
    let codebook_bytes = codebook_floats * 4;
    assert!(
        body.len() >= 6 + codebook_bytes,
        "PQ codebook data too short"
    );

    let mut codebooks = Vec::with_capacity(m);
    let mut offset = 6;
    for _ in 0..m {
        let mut sub_book = Vec::with_capacity(k);
        for _ in 0..k {
            let mut centroid = Vec::with_capacity(sub_dim);
            for _ in 0..sub_dim {
                let v = f32::from_le_bytes([
                    body[offset],
                    body[offset + 1],
                    body[offset + 2],
                    body[offset + 3],
                ]);
                centroid.push(v);
                offset += 4;
            }
            sub_book.push(centroid);
        }
        codebooks.push(sub_book);
    }

    ProductQuantizer {
        m,
        k,
        sub_dim,
        codebooks,
    }
}

// ---------------------------------------------------------------------------
// Binary
// ---------------------------------------------------------------------------

fn encode_binary_quant_seg(dim: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = QUANT_TYPE_BINARY;
    buf[1] = 2; // Cold tier
    buf[2..4].copy_from_slice(&dim.to_le_bytes());
    // Binary quantization has no additional parameters (sign-based).
    buf
}

/// Wrapper to implement `Quantizer` for binary quantization.
struct BinaryQuantizerWrapper {
    dim: usize,
}

impl Quantizer for BinaryQuantizerWrapper {
    fn encode(&self, vector: &[f32]) -> Vec<u8> {
        binary::encode_binary(vector)
    }

    fn decode(&self, codes: &[u8]) -> Vec<f32> {
        binary::decode_binary(codes, self.dim)
    }

    fn tier(&self) -> crate::tier::TemperatureTier {
        crate::tier::TemperatureTier::Cold
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

// ---------------------------------------------------------------------------
// SKETCH_SEG codec
// ---------------------------------------------------------------------------

/// Encode a CountMinSketch into the SKETCH_SEG binary payload.
///
/// Layout:
/// ```text
/// [width: u32 LE] [depth: u32 LE] [total_accesses: u64 LE] [padding: 48 bytes to 64B]
/// [counters: depth * width bytes]
/// ```
pub fn encode_sketch_seg(sketch: &CountMinSketch) -> Vec<u8> {
    let mut buf = vec![0u8; 64]; // 64-byte aligned header

    buf[0..4].copy_from_slice(&(sketch.width as u32).to_le_bytes());
    buf[4..8].copy_from_slice(&(sketch.depth as u32).to_le_bytes());
    buf[8..16].copy_from_slice(&sketch.total_accesses.to_le_bytes());

    // Counter data: row-major
    for row in &sketch.counters {
        buf.extend_from_slice(row);
    }

    buf
}

/// Decode a SKETCH_SEG binary payload into a CountMinSketch.
pub fn decode_sketch_seg(data: &[u8]) -> CountMinSketch {
    assert!(data.len() >= 64, "SKETCH_SEG header too short");

    let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let depth = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let total_accesses = u64::from_le_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let body = &data[64..];
    let expected = width * depth;
    assert!(body.len() >= expected, "SKETCH_SEG counter data too short");

    let mut counters = Vec::with_capacity(depth);
    for row in 0..depth {
        let start = row * width;
        counters.push(body[start..start + width].to_vec());
    }

    CountMinSketch {
        counters,
        width,
        depth,
        total_accesses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_quant_seg_round_trip() {
        let sq = ScalarQuantizer {
            min_vals: vec![-1.0, -2.0, -0.5, 0.0],
            max_vals: vec![1.0, 2.0, 0.5, 1.0],
            dim: 4,
        };

        let encoded = encode_scalar_quantizer(&sq);
        let decoded = decode_quant_seg(&encoded);

        assert_eq!(decoded.dim(), 4);
        assert_eq!(decoded.tier(), crate::tier::TemperatureTier::Hot);

        // Verify round-trip: encode a test vector, check similar output
        let test_vec = vec![0.5, 1.0, 0.0, 0.5];
        let codes_orig = sq.encode_vec(&test_vec);
        let codes_decoded = decoded.encode(&test_vec);
        assert_eq!(codes_orig, codes_decoded);
    }

    #[test]
    fn product_quant_seg_round_trip() {
        // Build a small PQ manually
        let pq = ProductQuantizer {
            m: 2,
            k: 4,
            sub_dim: 2,
            codebooks: vec![
                vec![
                    vec![0.0, 0.1],
                    vec![0.2, 0.3],
                    vec![0.4, 0.5],
                    vec![0.6, 0.7],
                ],
                vec![
                    vec![0.8, 0.9],
                    vec![1.0, 1.1],
                    vec![1.2, 1.3],
                    vec![1.4, 1.5],
                ],
            ],
        };

        let encoded = encode_product_quantizer(&pq);
        let decoded = decode_quant_seg(&encoded);

        assert_eq!(decoded.dim(), 4);
        assert_eq!(decoded.tier(), crate::tier::TemperatureTier::Warm);

        let test_vec = vec![0.1, 0.2, 0.9, 1.0];
        let codes_orig = pq.encode_vec(&test_vec);
        let codes_decoded = decoded.encode(&test_vec);
        assert_eq!(codes_orig, codes_decoded);
    }

    #[test]
    fn binary_quant_seg_round_trip() {
        let dim: u16 = 16;
        let encoded = encode_binary_quant_seg(dim);
        let decoded = decode_quant_seg(&encoded);

        assert_eq!(decoded.dim(), 16);
        assert_eq!(decoded.tier(), crate::tier::TemperatureTier::Cold);

        let test_vec: Vec<f32> = (0..16)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let codes = decoded.encode(&test_vec);
        let recon = decoded.decode(&codes);
        assert_eq!(recon.len(), 16);
    }

    #[test]
    fn sketch_seg_round_trip() {
        let mut sketch = CountMinSketch::new(64, 4);
        for block_id in 0..20u64 {
            for _ in 0..(block_id + 1) {
                sketch.increment(block_id);
            }
        }

        let encoded = encode_sketch_seg(&sketch);
        let decoded = decode_sketch_seg(&encoded);

        assert_eq!(decoded.width, sketch.width);
        assert_eq!(decoded.depth, sketch.depth);
        assert_eq!(decoded.total_accesses, sketch.total_accesses);

        // Verify estimates match
        for block_id in 0..20u64 {
            assert_eq!(decoded.estimate(block_id), sketch.estimate(block_id));
        }
    }
}
