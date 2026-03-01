//! Zero-dependency LZ77 compression for QR seed microkernels.
//!
//! Simple but effective: 4 KB sliding window, match lengths 3-10,
//! literal runs up to 128 bytes. Typical WASM compression ratio: 1.4-2.5x.
//!
//! Wire format (SCF-1 — Seed Compression Format):
//! - Header: 4 bytes (original size as LE u32)
//! - Token stream:
//!   - `0x00..=0x7F` (bit 7 clear): Literal run, count = byte + 1 (1-128)
//!   - `0x80..=0xFF` (bit 7 set): Back-reference
//!     - length = ((byte >> 4) & 0x07) + 3 (3-10)
//!     - offset = ((byte & 0x0F) << 8) | next_byte + 1 (1-4096)

/// Compression errors.
#[derive(Debug, PartialEq)]
pub enum CompressError {
    /// Compressed data too short to contain header.
    TooShort,
    /// Compressed stream is truncated.
    Truncated,
    /// Back-reference offset exceeds output size.
    InvalidOffset,
    /// Decompressed size doesn't match header.
    SizeMismatch { expected: usize, got: usize },
}

impl core::fmt::Display for CompressError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CompressError::TooShort => write!(f, "compressed data too short"),
            CompressError::Truncated => write!(f, "compressed stream truncated"),
            CompressError::InvalidOffset => write!(f, "invalid back-reference offset"),
            CompressError::SizeMismatch { expected, got } => {
                write!(f, "size mismatch: expected {expected}, got {got}")
            }
        }
    }
}

/// Hash a 3-byte trigram for the LZ77 hash table.
#[inline]
fn trigram_hash(a: u8, b: u8, c: u8) -> usize {
    (((a as usize) << 4) ^ ((b as usize) << 2) ^ (c as usize)) & 0xFFF
}

/// Flush accumulated literals to the output.
fn flush_literals(output: &mut Vec<u8>, literals: &[u8]) {
    let mut offset = 0;
    while offset < literals.len() {
        let chunk = core::cmp::min(128, literals.len() - offset);
        output.push((chunk - 1) as u8); // 0x00..=0x7F
        output.extend_from_slice(&literals[offset..offset + chunk]);
        offset += chunk;
    }
}

/// Compress data using LZ77 with a 4 KB sliding window.
///
/// Returns the compressed payload prefixed with a 4-byte original-size header.
pub fn compress(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());

    // Header: original size (LE u32).
    output.extend_from_slice(&(input.len() as u32).to_le_bytes());

    if input.is_empty() {
        return output;
    }

    // Hash table: maps trigram hash → most recent position.
    let mut table = [0u32; 4096];
    let mut literals: Vec<u8> = Vec::new();
    let mut pos = 0;

    while pos < input.len() {
        let mut best_len = 0usize;
        let mut best_offset = 0usize;

        if pos + 3 <= input.len() {
            let hash = trigram_hash(input[pos], input[pos + 1], input[pos + 2]);
            let candidate = table[hash] as usize;
            table[hash] = pos as u32;

            if candidate < pos && pos - candidate <= 4096 {
                let max_len = core::cmp::min(10, input.len() - pos);
                let mut match_len = 0;
                while match_len < max_len && input[candidate + match_len] == input[pos + match_len]
                {
                    match_len += 1;
                }
                if match_len >= 3 {
                    best_len = match_len;
                    best_offset = pos - candidate;
                }
            }
        }

        if best_len >= 3 {
            // Flush any pending literals first.
            flush_literals(&mut output, &literals);
            literals.clear();

            // Emit match token: 1LLL_OOOO OOOOOOOO
            let len_code = (best_len - 3) as u8; // 0-7
            let offset_val = (best_offset - 1) as u16; // 0-4095
            let offset_hi = ((offset_val >> 8) & 0x0F) as u8;
            let offset_lo = (offset_val & 0xFF) as u8;

            output.push(0x80 | (len_code << 4) | offset_hi);
            output.push(offset_lo);

            // Update hash table for positions within the match.
            for i in 1..best_len {
                if pos + i + 3 <= input.len() {
                    let h = trigram_hash(input[pos + i], input[pos + i + 1], input[pos + i + 2]);
                    table[h] = (pos + i) as u32;
                }
            }

            pos += best_len;
        } else {
            literals.push(input[pos]);
            pos += 1;
        }
    }

    // Flush remaining literals.
    flush_literals(&mut output, &literals);

    output
}

/// Decompress SCF-1 data back to original bytes.
pub fn decompress(compressed: &[u8]) -> Result<Vec<u8>, CompressError> {
    if compressed.len() < 4 {
        return Err(CompressError::TooShort);
    }

    let original_size =
        u32::from_le_bytes([compressed[0], compressed[1], compressed[2], compressed[3]]) as usize;

    let mut output = Vec::with_capacity(original_size);
    let mut pos = 4;

    while output.len() < original_size && pos < compressed.len() {
        let control = compressed[pos];
        pos += 1;

        if control & 0x80 == 0 {
            // Literal run.
            let count = (control as usize) + 1;
            if pos + count > compressed.len() {
                return Err(CompressError::Truncated);
            }
            output.extend_from_slice(&compressed[pos..pos + count]);
            pos += count;
        } else {
            // Back-reference.
            if pos >= compressed.len() {
                return Err(CompressError::Truncated);
            }
            let length = (((control >> 4) & 0x07) as usize) + 3;
            let offset_hi = (control & 0x0F) as usize;
            let offset_lo = compressed[pos] as usize;
            pos += 1;
            let offset = (offset_hi << 8 | offset_lo) + 1;

            if offset > output.len() {
                return Err(CompressError::InvalidOffset);
            }

            let start = output.len() - offset;
            for i in 0..length {
                let byte = output[start + i];
                output.push(byte);
            }
        }
    }

    if output.len() != original_size {
        return Err(CompressError::SizeMismatch {
            expected: original_size,
            got: output.len(),
        });
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_round_trip() {
        let compressed = compress(b"");
        assert_eq!(compressed, [0, 0, 0, 0]); // Just the size header.
        let decompressed = decompress(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn short_literal_round_trip() {
        let input = b"Hello, World!";
        let compressed = compress(input);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(&decompressed, input);
    }

    #[test]
    fn repeated_data_compresses() {
        // Highly repetitive data should compress well.
        let input: Vec<u8> = (0..1000).map(|i| (i % 7) as u8).collect();
        let compressed = compress(&input);
        assert!(
            compressed.len() < input.len(),
            "compressed {} >= original {}",
            compressed.len(),
            input.len()
        );
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn wasm_like_data_compresses() {
        // Simulate WASM module: lots of zero runs and repeated patterns.
        let mut wasm = Vec::new();
        // Magic + version.
        wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]);
        // Repeated section patterns.
        for _ in 0..100 {
            wasm.extend_from_slice(&[0x01, 0x06, 0x01, 0x60, 0x01, 0x7F, 0x01, 0x7F]);
        }
        // Zero fill.
        wasm.resize(wasm.len() + 500, 0x00);

        let compressed = compress(&wasm);
        assert!(
            compressed.len() < wasm.len(),
            "compressed {} >= original {}",
            compressed.len(),
            wasm.len()
        );
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, wasm);
    }

    #[test]
    fn random_like_data_round_trips() {
        // Incompressible data should still round-trip correctly.
        let input: Vec<u8> = (0..500).map(|i| ((i * 131 + 17) % 256) as u8).collect();
        let compressed = compress(&input);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn large_data_round_trip() {
        let input: Vec<u8> = (0..8000)
            .map(|i| ((i * 37 + i / 100) % 256) as u8)
            .collect();
        let compressed = compress(&input);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn all_zeros_compress_well() {
        let input = vec![0u8; 4096];
        let compressed = compress(&input);
        // 4096 zeros with 4KB window and match length 10 should compress very well.
        assert!(compressed.len() < input.len() / 2);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn decompress_truncated_fails() {
        let compressed = compress(b"test data for truncation");
        // Truncate the compressed data.
        let truncated = &compressed[..compressed.len() / 2];
        assert!(decompress(truncated).is_err());
    }

    #[test]
    fn decompress_too_short_fails() {
        assert_eq!(decompress(&[0, 0]), Err(CompressError::TooShort));
    }

    #[test]
    fn compress_error_display() {
        let e = CompressError::SizeMismatch {
            expected: 100,
            got: 50,
        };
        assert!(format!("{e}").contains("100"));
    }

    #[test]
    fn exactly_128_byte_literal_run() {
        // 128 unique bytes forces exactly one max-length literal run.
        let input: Vec<u8> = (0..128).map(|i| (i * 2 + 1) as u8).collect();
        let compressed = compress(&input);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }
}
