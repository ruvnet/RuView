use std::io::{self, Error, Seek};

use bitstream_io::{BitRead, BitReader, Endianness};

#[derive(Default, Clone, Copy)]
pub struct TableEntry {
    /// Wide enough to fit the max symbol nbr.
    pub sym: u16,
    /// 0 means no symbol.
    pub len: u8,
}

/// Deflate uses max 288 symbols.
const MAX_HUFFMAN_SYMBOLS: usize = 288;
/// Implode uses max 16-bit codewords.
const MAX_HUFFMAN_BITS: usize = 16;
/// Seems a good trade-off.
const HUFFMAN_LOOKUP_TABLE_BITS: u8 = 8;

pub struct HuffmanDecoder {
    /// Lookup table for fast decoding of short codewords.
    pub table: [TableEntry; 1 << HUFFMAN_LOOKUP_TABLE_BITS],
    /// "Sentinel bits" value for each codeword length.
    pub sentinel_bits: [u32; MAX_HUFFMAN_BITS + 1],
    /// First symbol index minus first codeword mod 2**16 for each length.
    pub offset_first_sym_idx: [u16; MAX_HUFFMAN_BITS + 1],
    /// Map from symbol index to symbol.
    pub syms: [u16; MAX_HUFFMAN_SYMBOLS],
    // num_syms:usize
}

impl Default for HuffmanDecoder {
    fn default() -> Self {
        Self {
            table: [TableEntry::default(); 1 << HUFFMAN_LOOKUP_TABLE_BITS],
            sentinel_bits: [0; MAX_HUFFMAN_BITS + 1],
            offset_first_sym_idx: [0; MAX_HUFFMAN_BITS + 1],
            syms: [0; MAX_HUFFMAN_SYMBOLS],
        }
    }
}

/// Reverse the n least significant bits of x.
/// The (16 - n) most significant bits of the result will be zero.
pub fn reverse_lsb(x: u16, n: usize) -> u16 {
    debug_assert!(n > 0);
    debug_assert!(n <= 16);
    x.reverse_bits() >> (16 - n)
}

/// Initialize huffman decoder d for a code defined by the n codeword lengths.
/// Returns `Err` if the codeword lengths do not correspond to a valid prefix
/// code.
impl HuffmanDecoder {
    pub fn init(&mut self, lengths: &[u8], n: usize) -> std::io::Result<()> {
        let mut count = [0; MAX_HUFFMAN_BITS + 1];
        let mut code = [0; MAX_HUFFMAN_BITS + 1];
        let mut sym_idx = [0u16; MAX_HUFFMAN_BITS + 1];

        // Count the number of codewords of each length.
        for i in 0..n {
            debug_assert!(lengths[i] as usize <= MAX_HUFFMAN_BITS);
            count[lengths[i] as usize] += 1;
        }
        count[0] = 0; // Ignore zero-length codewords.
                      // Compute sentinel_bits and offset_first_sym_idx for each length.
        code[0] = 0;
        sym_idx[0] = 0;
        for l in 1..=MAX_HUFFMAN_BITS {
            // First canonical codeword of this length.
            code[l] = (code[l - 1] + count[l - 1]) << 1;

            if count[l] != 0 && code[l] as u32 + count[l] as u32 - 1 > (1u32 << l) - 1 {
                // The last codeword is longer than l bits.
                return Err(Error::new(
                    io::ErrorKind::InvalidData,
                    "the last codeword is longer than len bits",
                ));
            }

            let s = (code[l] as u32 + count[l] as u32) << (MAX_HUFFMAN_BITS - l);
            self.sentinel_bits[l] = s;
            debug_assert!(self.sentinel_bits[l] >= code[l] as u32, "No overflow!");

            sym_idx[l] = sym_idx[l - 1] + count[l - 1];
            self.offset_first_sym_idx[l] = sym_idx[l].wrapping_sub(code[l]);
        }

        // Zero-initialize the lookup table.
        self.table.fill(TableEntry::default());

        // Build mapping from index to symbol and populate the lookup table.
        lengths
            .iter()
            .enumerate()
            .take(n)
            .for_each(|(i, code_len)| {
                let l = *code_len as usize;
                if l == 0 {
                    return;
                }

                self.syms[sym_idx[l] as usize] = i as u16;
                sym_idx[l] += 1;

                if l <= HUFFMAN_LOOKUP_TABLE_BITS as usize {
                    self.table_insert(i, l, code[l]);
                    code[l] += 1;
                }
            });

        Ok(())
    }

    pub fn table_insert(&mut self, sym: usize, len: usize, codeword: u16) {
        debug_assert!(len <= HUFFMAN_LOOKUP_TABLE_BITS as usize);

        let codeword = reverse_lsb(codeword, len); // Make it LSB-first.
        let pad_len = HUFFMAN_LOOKUP_TABLE_BITS as usize - len;

        // Pad the pad_len upper bits with all bit combinations.
        for padding in 0..(1 << pad_len) {
            let index = (codeword | (padding << len)) as usize;
            debug_assert!(sym <= u16::MAX as usize);
            self.table[index].sym = sym as u16;
            debug_assert!(len <= u8::MAX as usize);
            self.table[index].len = len as u8;
        }
    }

    /// Use the decoder d to decode a symbol from the LSB-first zero-padded bits.
    /// Returns the decoded symbol number or an error if no symbol could be decoded.
    /// *num_used_bits will be set to the number of bits used to decode the symbol,
    /// or zero if no symbol could be decoded.
    pub fn huffman_decode<T: std::io::Read + Seek, E: Endianness>(
        &mut self,
        length: u64,
        is: &mut BitReader<T, E>,
    ) -> std::io::Result<u16> {
        // First try the lookup table.
        let read_bits1 = (HUFFMAN_LOOKUP_TABLE_BITS as u64).min(length - is.position_in_bits()?);
        let lookup_bits = !is.read_var::<u8>(read_bits1 as u32)? as usize;
        debug_assert!(lookup_bits < self.table.len());
        if self.table[lookup_bits].len != 0 {
            debug_assert!(self.table[lookup_bits].len <= HUFFMAN_LOOKUP_TABLE_BITS);
            is.seek_bits(io::SeekFrom::Current(
                -(read_bits1 as i64) + self.table[lookup_bits].len as i64,
            ))?;
            return Ok(self.table[lookup_bits].sym);
        }

        // Then do canonical decoding with the bits in MSB-first order.
        let read_bits2 = (HUFFMAN_LOOKUP_TABLE_BITS as u64).min(length - is.position_in_bits()?);
        let mut bits = reverse_lsb(
            (lookup_bits | ((!is.read_var::<u8>(read_bits2 as u32)? as usize) << read_bits1))
                as u16,
            MAX_HUFFMAN_BITS,
        );

        for l in (HUFFMAN_LOOKUP_TABLE_BITS as usize + 1)..=MAX_HUFFMAN_BITS {
            if (bits as u32) < self.sentinel_bits[l] {
                bits >>= MAX_HUFFMAN_BITS - l;
                let sym_idx = (self.offset_first_sym_idx[l] as usize + bits as usize) & 0xFFFF;
                //assert(sym_idx < self.num_syms);
                is.seek_bits(io::SeekFrom::Current(
                    -(read_bits1 as i64 + read_bits2 as i64) + l as i64,
                ))?;
                return Ok(self.syms[sym_idx]);
            }
        }
        Err(Error::new(
            io::ErrorKind::InvalidData,
            "huffman decode failed",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bitstream_io::{BitReader, LittleEndian};

    use super::HuffmanDecoder;
    #[test]
    fn test_huffman_decode_basic() {
        let lens = [
            3, // sym 0:  000
            3, // sym 1:  001
            3, // sym 2:  010
            3, // sym 3:  011
            3, // sym 4:  100
            3, // sym 5:  101
            4, // sym 6:  1100
            4, // sym 7:  1101
            0, // sym 8:
            0, // sym 9:
            0, // sym 10:
            0, // sym 11:
            0, // sym 12:
            0, // sym 13:
            0, // sym 14:
            0, // sym 15:
            6, // sym 16: 111110
            5, // sym 17: 11110
            4, // sym 18: 1110
        ];

        let mut d = HuffmanDecoder::default();
        d.init(&lens, lens.len()).unwrap();

        // 000 (msb-first) -> 000 (lsb-first)
        assert_eq!(
            d.huffman_decode(
                8,
                &mut BitReader::endian(&mut Cursor::new(&[!0x0]), LittleEndian)
            )
            .unwrap(),
            0
        );

        /* 011 (msb-first) -> 110 (lsb-first)*/
        assert_eq!(
            d.huffman_decode(
                8,
                &mut BitReader::endian(&mut Cursor::new(&[!0b110]), LittleEndian)
            )
            .unwrap(),
            0b011
        );

        /* 11110 (msb-first) -> 01111 (lsb-first)*/
        assert_eq!(
            d.huffman_decode(
                8,
                &mut BitReader::endian(&mut Cursor::new(&[!0b1111]), LittleEndian)
            )
            .unwrap(),
            0b10001
        );

        /* 111110 (msb-first) -> 011111 (lsb-first)*/
        assert_eq!(
            d.huffman_decode(
                8,
                &mut BitReader::endian(&mut Cursor::new(&[!0b11111]), LittleEndian)
            )
            .unwrap(),
            0b10000
        );

        /* 1111111 (msb-first) -> 1111111 (lsb-first)*/
        assert!(d
            .huffman_decode(
                8,
                &mut BitReader::endian(&mut Cursor::new(&[!0x7f]), LittleEndian)
            )
            .is_err());
    }
}
