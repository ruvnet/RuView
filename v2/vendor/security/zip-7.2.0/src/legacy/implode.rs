use super::huffman::HuffmanDecoder;
use bitstream_io::{BitRead, BitReader, Endianness, LittleEndian};
use std::io::{self, Cursor, Error, Read, Result};

/// Maximum symbol value in the length Huffman table (6 bits)
/// When this value is decoded, an additional byte is read for extended length
const MAX_LEN_SYMBOL: u16 = 63;

/// Maximum number of symbols in a Huffman table (256 for literals, 64 for lengths/distances)
const MAX_HUFFMAN_SYMBOLS: usize = 1 << 8; // 256

/// Maximum code length in bits for Huffman codes (per ZIP specification)
const MAX_CODE_LENGTH: usize = 16;

/// Size of the code length count array (lengths are actually 1..=16)
const CODE_LENGTH_COUNT_SIZE: usize = MAX_CODE_LENGTH;

/// Initialize the Huffman decoder d with num_lens codeword lengths read from is.
/// Returns false if the input is invalid.
fn read_huffman_code<T: std::io::Read, E: Endianness>(
    is: &mut BitReader<T, E>,
    num_lens: usize,
) -> std::io::Result<Box<HuffmanDecoder>> {
    let mut lens = [0; MAX_HUFFMAN_SYMBOLS];
    let mut len_count = [0; CODE_LENGTH_COUNT_SIZE];

    // Number of bytes representing the Huffman code.
    let byte = is.read::<8, u8>()?;
    let num_bytes = (byte + 1) as usize;

    let mut codeword_idx = 0;
    for _byte_idx in 0..num_bytes {
        let byte = is.read::<8, u16>()?;

        let codeword_len = (byte & 0xf) + 1; /* Low four bits plus one. */
        let run_length = (byte >> 4) + 1; /* High four bits plus one. */

        debug_assert!((1..=16).contains(&codeword_len));
        len_count[codeword_len as usize - 1] += run_length;

        if (codeword_idx + run_length) as usize > num_lens {
            return Err(Error::new(
                io::ErrorKind::InvalidData,
                "too many codeword lengths",
            ));
        }
        for _ in 0..run_length {
            debug_assert!((codeword_idx as usize) < num_lens);
            lens[codeword_idx as usize] = codeword_len as u8;
            codeword_idx += 1;
        }
    }

    debug_assert!(codeword_idx as usize <= num_lens);
    if (codeword_idx as usize) < num_lens {
        return Err(Error::new(
            io::ErrorKind::InvalidData,
            "not enough codeword lengths",
        ));
    }

    // Check that the Huffman tree is full.
    let mut avail_codewords = 1;
    for i in 1..=16 {
        debug_assert!(avail_codewords >= 0);
        avail_codewords *= 2;
        avail_codewords -= len_count[i - 1] as i32;
        if avail_codewords < 0 {
            return Err(Error::new(
                io::ErrorKind::InvalidData,
                "huffman tree is not full",
            ));
        }
    }
    if avail_codewords != 0 {
        // Not all codewords were used.
        return Err(Error::new(
            io::ErrorKind::InvalidData,
            "not all codewords were used",
        ));
    }

    let mut d = Box::new(HuffmanDecoder::default());
    d.init(&lens, num_lens)?;
    Ok(d)
}

fn hwexplode(
    src: &[u8],
    uncomp_len: usize,
    large_wnd: bool,
    lit_tree: bool,
    pk101_bug_compat: bool,
    dst: &mut Vec<u8>,
) -> std::io::Result<()> {
    // Pre-allocate capacity
    dst.reserve(uncomp_len);

    let bit_length = src.len() as u64 * 8;
    let mut is = BitReader::endian(Cursor::new(&src), LittleEndian);
    let mut lit_decoder_opt = if lit_tree {
        Some(read_huffman_code(&mut is, 256)?)
    } else {
        None
    };
    let mut len_decoder = read_huffman_code(&mut is, 64)?;
    let mut dist_decoder = read_huffman_code(&mut is, 64)?;
    let min_len = if (pk101_bug_compat && large_wnd) || (!pk101_bug_compat && lit_tree) {
        3
    } else {
        2
    };
    let dist_low_bits = if large_wnd { 7 } else { 6 };

    while dst.len() < uncomp_len {
        let is_literal = is.read_bit()?;
        if is_literal {
            // Literal.
            let sym;
            if let Some(lit_decoder) = &mut lit_decoder_opt {
                sym = lit_decoder.huffman_decode(bit_length, &mut is)?;
            } else {
                sym = is.read::<8, u8>()? as u16;
            }
            debug_assert!(sym <= u8::MAX as u16);
            dst.push(sym as u8);
            continue;
        }

        // Read the low dist bits.
        let mut dist = is.read_var::<u16>(dist_low_bits)?;
        // Read the Huffman-encoded high dist bits.
        let sym = dist_decoder.huffman_decode(bit_length, &mut is)?;
        dist |= (sym as u16) << dist_low_bits;
        dist += 1;

        // Read the Huffman-encoded len.
        let sym = len_decoder.huffman_decode(bit_length, &mut is)?;
        let mut len = (sym + min_len) as usize;

        if sym == MAX_LEN_SYMBOL {
            // Read an extra len byte.
            len += is.read::<8, u16>()? as usize;
        }
        let len = len.min(uncomp_len - dst.len());

        // Optimize the copy loop using extend_from_within when possible
        if dist as usize <= dst.len() {
            if dist as usize >= len {
                // No overlap in copy, can use extend_from_within
                let start = dst.len() - dist as usize;
                dst.extend_from_within(start..start + len);
            } else {
                // Overlapping copy
                for _ in 0..len {
                    let byte = dst[dst.len() - dist as usize];
                    dst.push(byte);
                }
            }
        } else {
            // Copy with implicit zeros
            for _ in 0..len {
                if dist as usize > dst.len() {
                    dst.push(0);
                } else {
                    let byte = dst[dst.len() - dist as usize];
                    dst.push(byte);
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct ImplodeDecoder<R> {
    compressed_reader: R,
    uncompressed_size: u64,
    stream_read: bool,
    large_wnd: bool,
    lit_tree: bool,
    stream: Vec<u8>,
    read_pos: usize, // Add read position tracker
}

impl<R: Read> ImplodeDecoder<R> {
    pub fn new(inner: R, uncompressed_size: u64, flags: u16) -> Self {
        let large_wnd = (flags & 2) != 0;
        let lit_tree = (flags & 4) != 0;
        ImplodeDecoder {
            compressed_reader: inner,
            uncompressed_size,
            stream_read: false,
            large_wnd,
            lit_tree,
            stream: Vec::new(),
            read_pos: 0,
        }
    }

    pub fn into_inner(self) -> R {
        self.compressed_reader
    }
}

impl<R: Read> Read for ImplodeDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.stream_read {
            self.stream_read = true;
            let mut compressed_bytes = Vec::new();
            self.compressed_reader.read_to_end(&mut compressed_bytes)?;

            // Pre-allocate stream buffer
            self.stream.reserve(self.uncompressed_size as usize);

            hwexplode(
                &compressed_bytes,
                self.uncompressed_size as usize,
                self.large_wnd,
                self.lit_tree,
                false,
                &mut self.stream,
            )?;
        }

        let available = self.stream.len() - self.read_pos;
        let bytes_to_read = available.min(buf.len());
        buf[..bytes_to_read]
            .copy_from_slice(&self.stream[self.read_pos..self.read_pos + bytes_to_read]);
        self.read_pos += bytes_to_read;
        Ok(bytes_to_read)
    }
}

#[cfg(test)]
mod tests {
    use super::hwexplode;

    const HAMLET_256: &[u8; 249] = include_bytes!("../../tests/data/legacy/implode_hamlet_256.bin");
    const HAMLET_256_OUT: &[u8; 256] =
        include_bytes!("../../tests/data/legacy/implode_hamlet_256.out");

    #[test]
    fn test_explode_hamlet_256() {
        let mut dst = Vec::new();
        hwexplode(HAMLET_256, 256, false, false, false, &mut dst).unwrap();
        assert_eq!(dst.len(), 256);
        assert_eq!(&dst, &HAMLET_256_OUT);
    }
}
