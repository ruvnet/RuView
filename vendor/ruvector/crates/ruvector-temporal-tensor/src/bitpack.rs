//! Bitstream packer/unpacker for arbitrary bit widths (1-8).
//!
//! Uses a 64-bit accumulator for sub-byte codes with no alignment padding.

/// Pack unsigned codes of `bits` width into a byte stream.
///
/// Each code occupies exactly `bits` bits in the output with no alignment
/// padding between codes. A trailing partial byte is emitted if needed.
///
/// For 8-bit codes, writes bytes directly without bit accumulation.
#[inline]
pub fn pack(codes: &[u32], bits: u32, out: &mut Vec<u8>) {
    // Fast path: 8-bit codes map 1:1 to bytes.
    if bits == 8 {
        out.extend(codes.iter().map(|&c| c as u8));
        return;
    }

    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;

    for &code in codes {
        acc |= (code as u64) << acc_bits;
        acc_bits += bits;
        while acc_bits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            acc_bits -= 8;
        }
    }

    if acc_bits > 0 {
        out.push((acc & 0xFF) as u8);
    }
}

/// Unpack `count` unsigned codes of `bits` width from a byte stream.
///
/// Stops early if the data is exhausted before `count` codes are extracted.
///
/// For 8-bit codes, reads bytes directly without bit accumulation.
#[inline]
pub fn unpack(data: &[u8], bits: u32, count: usize, out: &mut Vec<u32>) {
    // Fast path: 8-bit codes map 1:1 from bytes.
    if bits == 8 {
        let n = count.min(data.len());
        out.extend(data[..n].iter().map(|&b| b as u32));
        return;
    }

    let mask = (1u64 << bits) - 1;
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut byte_idx = 0usize;
    let mut decoded = 0usize;

    while decoded < count {
        while acc_bits < bits && byte_idx < data.len() {
            acc |= (data[byte_idx] as u64) << acc_bits;
            acc_bits += 8;
            byte_idx += 1;
        }
        if acc_bits < bits {
            break;
        }

        out.push((acc & mask) as u32);
        acc >>= bits;
        acc_bits -= bits;
        decoded += 1;
    }
}

/// Compute qmax for a given bit width: `2^(bits-1) - 1`.
///
/// Returns 0 for invalid bit widths (0 or >8).
///
/// | bits | qmax |
/// |------|------|
/// | 8    | 127  |
/// | 7    | 63   |
/// | 5    | 15   |
/// | 3    | 3    |
#[inline]
pub fn qmax_from_bits(bits: u8) -> i32 {
    if bits == 0 || bits > 8 {
        return 0;
    }
    (1i32 << (bits - 1)) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_8bit() {
        let codes: Vec<u32> = (0..256).collect();
        let mut packed = Vec::new();
        pack(&codes, 8, &mut packed);
        assert_eq!(packed.len(), 256);

        let mut unpacked = Vec::new();
        unpack(&packed, 8, 256, &mut unpacked);
        assert_eq!(codes, unpacked);
    }

    #[test]
    fn test_roundtrip_3bit() {
        let codes: Vec<u32> = (0..7).collect();
        let mut packed = Vec::new();
        pack(&codes, 3, &mut packed);

        let mut unpacked = Vec::new();
        unpack(&packed, 3, 7, &mut unpacked);
        assert_eq!(codes, unpacked);
    }

    #[test]
    fn test_roundtrip_5bit() {
        let codes: Vec<u32> = (0..31).collect();
        let mut packed = Vec::new();
        pack(&codes, 5, &mut packed);

        let mut unpacked = Vec::new();
        unpack(&packed, 5, 31, &mut unpacked);
        assert_eq!(codes, unpacked);
    }

    #[test]
    fn test_roundtrip_7bit() {
        let codes: Vec<u32> = (0..127).collect();
        let mut packed = Vec::new();
        pack(&codes, 7, &mut packed);

        let mut unpacked = Vec::new();
        unpack(&packed, 7, 127, &mut unpacked);
        assert_eq!(codes, unpacked);
    }

    #[test]
    fn test_packing_density() {
        let codes = vec![5u32; 100];
        let mut packed = Vec::new();
        pack(&codes, 3, &mut packed);
        assert_eq!(packed.len(), 38); // ceil(300/8) = 38
    }

    #[test]
    fn test_qmax() {
        assert_eq!(qmax_from_bits(8), 127);
        assert_eq!(qmax_from_bits(7), 63);
        assert_eq!(qmax_from_bits(5), 15);
        assert_eq!(qmax_from_bits(3), 3);
        assert_eq!(qmax_from_bits(1), 0);
        assert_eq!(qmax_from_bits(0), 0);
    }
}
