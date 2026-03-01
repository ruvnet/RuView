//! Delta encoding with restart points for sorted integer sequences.
//!
//! Sorted ID sequences are delta-encoded: each value (except the first) is
//! stored as the difference from the previous value. Every `restart_interval`
//! entries, the value is stored absolute (not delta) to allow random access
//! into the middle of a sequence.

use crate::varint::{decode_varint, encode_varint, MAX_VARINT_LEN};

/// Encode a sorted slice of `u64` IDs using delta-varint encoding with restart
/// points. Appends encoded bytes to `buf`.
///
/// Every `restart_interval` entries (counting from 0), the full absolute value
/// is stored. All other entries are stored as the delta from the previous value.
///
/// # Panics
///
/// Panics if `restart_interval` is 0.
pub fn encode_delta(sorted_ids: &[u64], restart_interval: u32, buf: &mut Vec<u8>) {
    assert!(restart_interval > 0, "restart_interval must be > 0");
    let mut tmp = [0u8; MAX_VARINT_LEN];
    let mut prev = 0u64;
    for (i, &id) in sorted_ids.iter().enumerate() {
        let value = if (i as u32).is_multiple_of(restart_interval) {
            prev = id;
            id
        } else {
            let delta = id - prev;
            prev = id;
            delta
        };
        let n = encode_varint(value, &mut tmp);
        buf.extend_from_slice(&tmp[..n]);
    }
}

/// Decode `count` delta-varint encoded IDs from `buf`.
///
/// Every `restart_interval` entries the stored value is absolute; all others
/// are deltas from the previous decoded value.
///
/// # Panics
///
/// Panics if `restart_interval` is 0 or the buffer contains insufficient data.
pub fn decode_delta(buf: &[u8], count: usize, restart_interval: u32) -> Vec<u64> {
    assert!(restart_interval > 0, "restart_interval must be > 0");
    let mut result = Vec::with_capacity(count);
    let mut offset = 0;
    let mut prev = 0u64;
    for i in 0..count {
        let (val, consumed) =
            decode_varint(&buf[offset..]).expect("delta decode: unexpected end of data");
        offset += consumed;
        if (i as u32).is_multiple_of(restart_interval) {
            prev = val;
        } else {
            prev += val;
        }
        result.push(prev);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_simple() {
        let ids = vec![100, 105, 108, 120, 200];
        let mut buf = Vec::new();
        encode_delta(&ids, 128, &mut buf);
        let decoded = decode_delta(&buf, ids.len(), 128);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn round_trip_with_restart_points() {
        let ids: Vec<u64> = (0..20).map(|i| i * 10 + 100).collect();
        let mut buf = Vec::new();
        encode_delta(&ids, 4, &mut buf);
        let decoded = decode_delta(&buf, ids.len(), 4);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn single_element() {
        let ids = vec![42u64];
        let mut buf = Vec::new();
        encode_delta(&ids, 1, &mut buf);
        let decoded = decode_delta(&buf, 1, 1);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn empty_sequence() {
        let ids: Vec<u64> = vec![];
        let mut buf = Vec::new();
        encode_delta(&ids, 8, &mut buf);
        let decoded = decode_delta(&buf, 0, 8);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn restart_at_every_entry() {
        // When restart_interval=1, every value is absolute
        let ids = vec![1000, 2000, 3000, 4000];
        let mut buf = Vec::new();
        encode_delta(&ids, 1, &mut buf);
        let decoded = decode_delta(&buf, ids.len(), 1);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn large_values() {
        let ids = vec![u64::MAX - 100, u64::MAX - 50, u64::MAX - 10, u64::MAX];
        let mut buf = Vec::new();
        encode_delta(&ids, 128, &mut buf);
        let decoded = decode_delta(&buf, ids.len(), 128);
        assert_eq!(decoded, ids);
    }
}
