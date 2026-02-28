//! NumPy `.npy` importer for RVF stores.
//!
//! Parses the NumPy v1/v2 `.npy` format (little-endian float32 only).
//! The shape `(N, D)` is read from the header; IDs are assigned
//! sequentially starting from `start_id` (default 0).
//!
//! Reference: <https://numpy.org/devdocs/reference/generated/numpy.lib.format.html>

use crate::VectorRecord;
use std::io::Read;
use std::path::Path;

/// Configuration for NumPy import.
#[derive(Clone, Debug, Default)]
pub struct NpyConfig {
    /// Starting ID for auto-assigned vector IDs.
    pub start_id: u64,
}

/// Parsed header from a `.npy` file.
#[derive(Debug)]
struct NpyHeader {
    /// Number of rows (vectors).
    rows: usize,
    /// Number of columns (dimensions per vector).
    cols: usize,
}

/// Parse the `.npy` header from a reader, returning the shape and
/// advancing the reader past the header.
fn parse_npy_header<R: Read>(reader: &mut R) -> Result<NpyHeader, String> {
    // Magic: \x93NUMPY
    let mut magic = [0u8; 6];
    reader
        .read_exact(&mut magic)
        .map_err(|e| format!("failed to read npy magic: {e}"))?;
    if magic[0] != 0x93 || &magic[1..6] != b"NUMPY" {
        return Err("not a valid .npy file (bad magic)".to_string());
    }

    // Version
    let mut version = [0u8; 2];
    reader
        .read_exact(&mut version)
        .map_err(|e| format!("failed to read npy version: {e}"))?;
    let major = version[0];

    // Header length
    let header_len: usize = if major <= 1 {
        let mut buf = [0u8; 2];
        reader
            .read_exact(&mut buf)
            .map_err(|e| format!("failed to read header length: {e}"))?;
        u16::from_le_bytes(buf) as usize
    } else {
        let mut buf = [0u8; 4];
        reader
            .read_exact(&mut buf)
            .map_err(|e| format!("failed to read header length: {e}"))?;
        u32::from_le_bytes(buf) as usize
    };

    // Read the header dict string
    let mut header_bytes = vec![0u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|e| format!("failed to read header dict: {e}"))?;
    let header_str =
        std::str::from_utf8(&header_bytes).map_err(|e| format!("header is not utf8: {e}"))?;

    // Validate dtype is float32
    if !header_str.contains("'<f4'") && !header_str.contains("'float32'") {
        return Err(format!(
            "unsupported dtype in npy header (only float32/<f4 supported): {header_str}"
        ));
    }

    // Parse shape: look for 'shape': (N, D) or 'shape': (N,)
    let shape = parse_shape(header_str)?;

    Ok(shape)
}

fn parse_shape(header: &str) -> Result<NpyHeader, String> {
    // Find the shape tuple in the header dict
    let shape_start = header
        .find("'shape':")
        .or_else(|| header.find("\"shape\":"))
        .ok_or_else(|| format!("no 'shape' key in npy header: {header}"))?;

    let after_key = &header[shape_start..];
    let paren_open = after_key
        .find('(')
        .ok_or_else(|| "no opening paren in shape".to_string())?;
    let paren_close = after_key
        .find(')')
        .ok_or_else(|| "no closing paren in shape".to_string())?;

    let shape_content = &after_key[paren_open + 1..paren_close];
    let parts: Vec<&str> = shape_content
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    match parts.len() {
        1 => {
            let rows: usize = parts[0]
                .parse()
                .map_err(|e| format!("bad shape dim: {e}"))?;
            // 1-D array: each element is a 1-d vector
            Ok(NpyHeader { rows, cols: 1 })
        }
        2 => {
            let rows: usize = parts[0]
                .parse()
                .map_err(|e| format!("bad shape row: {e}"))?;
            let cols: usize = parts[1]
                .parse()
                .map_err(|e| format!("bad shape col: {e}"))?;
            Ok(NpyHeader { rows, cols })
        }
        _ => Err(format!(
            "unsupported shape rank {}: {shape_content}",
            parts.len()
        )),
    }
}

/// Parse a `.npy` file from a reader.
pub fn parse_npy<R: Read>(mut reader: R, config: &NpyConfig) -> Result<Vec<VectorRecord>, String> {
    let header = parse_npy_header(&mut reader)?;

    let total_floats = header.rows * header.cols;
    let total_bytes = total_floats * 4;
    let mut raw = vec![0u8; total_bytes];
    reader
        .read_exact(&mut raw)
        .map_err(|e| format!("failed to read npy data ({total_bytes} bytes expected): {e}"))?;

    let mut records = Vec::with_capacity(header.rows);
    for i in 0..header.rows {
        let offset = i * header.cols * 4;
        let mut vector = Vec::with_capacity(header.cols);
        for j in 0..header.cols {
            let byte_offset = offset + j * 4;
            let bytes: [u8; 4] = [
                raw[byte_offset],
                raw[byte_offset + 1],
                raw[byte_offset + 2],
                raw[byte_offset + 3],
            ];
            vector.push(f32::from_le_bytes(bytes));
        }
        records.push(VectorRecord {
            id: config.start_id + i as u64,
            vector,
            metadata: Vec::new(),
        });
    }

    Ok(records)
}

/// Parse a `.npy` file from a file path.
pub fn parse_npy_file(path: &Path, config: &NpyConfig) -> Result<Vec<VectorRecord>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let reader = std::io::BufReader::new(file);
    parse_npy(reader, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid .npy file in memory with the given shape and f32 data.
    fn build_npy(rows: usize, cols: usize, data: &[f32]) -> Vec<u8> {
        let header_dict =
            format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({rows}, {cols}), }}");
        // Pad header to 64-byte alignment (magic=6 + version=2 + header_len=2 + dict)
        let preamble_len = 6 + 2 + 2;
        let total_header = preamble_len + header_dict.len();
        let padding = (64 - (total_header % 64)) % 64;
        let padded_dict_len = header_dict.len() + padding;

        let mut buf = Vec::new();
        // Magic
        buf.push(0x93);
        buf.extend_from_slice(b"NUMPY");
        // Version 1.0
        buf.push(1);
        buf.push(0);
        // Header length (u16 LE)
        buf.extend_from_slice(&(padded_dict_len as u16).to_le_bytes());
        // Dict
        buf.extend_from_slice(header_dict.as_bytes());
        // Padding (spaces + newline)
        buf.extend(std::iter::repeat_n(b' ', padding.saturating_sub(1)));
        if padding > 0 {
            buf.push(b'\n');
        }
        // Data
        for &val in data {
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf
    }

    #[test]
    fn parse_2d_npy() {
        let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let npy = build_npy(2, 3, &data);

        let records = parse_npy(npy.as_slice(), &NpyConfig::default()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 0);
        assert_eq!(records[0].vector, vec![1.0, 2.0, 3.0]);
        assert_eq!(records[1].id, 1);
        assert_eq!(records[1].vector, vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn parse_npy_custom_start_id() {
        let data = vec![0.5f32, 0.6];
        let npy = build_npy(1, 2, &data);

        let config = NpyConfig { start_id: 100 };
        let records = parse_npy(npy.as_slice(), &config).unwrap();
        assert_eq!(records[0].id, 100);
    }

    #[test]
    fn bad_magic_rejected() {
        let bad = b"NOT_NUMPY_DATA";
        let result = parse_npy(bad.as_slice(), &NpyConfig::default());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad magic"));
    }

    #[test]
    fn shape_parsing() {
        let h = parse_shape("{'descr': '<f4', 'shape': (100, 384), }").unwrap();
        assert_eq!(h.rows, 100);
        assert_eq!(h.cols, 384);

        let h = parse_shape("{'descr': '<f4', 'shape': (50,), }").unwrap();
        assert_eq!(h.rows, 50);
        assert_eq!(h.cols, 1);
    }
}
