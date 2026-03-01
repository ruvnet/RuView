//! LEB128 unsigned varint encoding and decoding.
//!
//! Values up to `u64::MAX` are encoded in 1-10 bytes. Each byte uses the
//! high bit as a continuation flag: `1` means more bytes follow, `0` means
//! this is the last byte. The remaining 7 bits contribute to the value
//! in little-endian order.

use rvf_types::{ErrorCode, RvfError};

/// Maximum number of bytes a u64 varint can occupy.
pub const MAX_VARINT_LEN: usize = 10;

/// Encode a `u64` value as a LEB128 varint into `buf`.
///
/// Returns the number of bytes written. The caller must ensure `buf` is at
/// least `MAX_VARINT_LEN` bytes long.
///
/// # Panics
///
/// Panics if `buf` is shorter than the number of bytes required.
pub fn encode_varint(mut value: u64, buf: &mut [u8]) -> usize {
    let mut i = 0;
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf[i] = byte;
            return i + 1;
        }
        buf[i] = byte | 0x80;
        i += 1;
    }
}

/// Decode a LEB128 varint from `buf`.
///
/// Returns `(value, bytes_consumed)` on success.
///
/// Uses branchless fast paths for the common 1-byte and 2-byte cases,
/// falling back to a loop for longer encodings.
///
/// # Errors
///
/// Returns `RvfError` if the buffer is too short or the varint exceeds 10
/// bytes (which would overflow a u64).
pub fn decode_varint(buf: &[u8]) -> Result<(u64, usize), RvfError> {
    if buf.is_empty() {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    // Fast path: 1-byte varint (values 0-127, most common case).
    let b0 = buf[0];
    if b0 & 0x80 == 0 {
        return Ok((b0 as u64, 1));
    }

    // Fast path: 2-byte varint (values 128-16383).
    if buf.len() < 2 {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let b1 = buf[1];
    if b1 & 0x80 == 0 {
        let value = ((b0 & 0x7F) as u64) | ((b1 as u64) << 7);
        return Ok((value, 2));
    }

    // Slow path: 3+ byte varint.
    let mut value = ((b0 & 0x7F) as u64) | (((b1 & 0x7F) as u64) << 7);
    let mut shift: u32 = 14;
    let limit = buf.len().min(MAX_VARINT_LEN);
    for (i, &byte) in buf.iter().enumerate().take(limit).skip(2) {
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
        shift += 7;
    }
    Err(RvfError::Code(ErrorCode::TruncatedSegment))
}

/// Returns the number of bytes required to encode `value` as a varint.
pub fn varint_size(mut value: u64) -> usize {
    let mut size = 1;
    while value >= 0x80 {
        value >>= 7;
        size += 1;
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(value: u64) {
        let mut buf = [0u8; MAX_VARINT_LEN];
        let written = encode_varint(value, &mut buf);
        let (decoded, consumed) = decode_varint(&buf[..written]).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(consumed, written);
    }

    #[test]
    fn single_byte_values() {
        round_trip(0);
        round_trip(1);
        round_trip(127);
    }

    #[test]
    fn two_byte_values() {
        round_trip(128);
        round_trip(255);
        round_trip(16383);
    }

    #[test]
    fn multi_byte_values() {
        round_trip(16384);
        round_trip(2_097_151);
        round_trip(u32::MAX as u64);
    }

    #[test]
    fn max_u64() {
        round_trip(u64::MAX);
    }

    #[test]
    fn encode_size_matches() {
        for &val in &[0u64, 1, 127, 128, 16383, 16384, u32::MAX as u64, u64::MAX] {
            let mut buf = [0u8; MAX_VARINT_LEN];
            let written = encode_varint(val, &mut buf);
            assert_eq!(varint_size(val), written);
        }
    }

    #[test]
    fn decode_truncated_returns_error() {
        // A single byte with continuation bit set, but no following byte
        let result = decode_varint(&[0x80]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_empty_returns_error() {
        let result = decode_varint(&[]);
        assert!(result.is_err());
    }
}
