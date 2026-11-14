//! Bounded HAP TLV8 primitives.

use std::collections::BTreeMap;

use crate::error::HapError;

pub const TLV_METHOD: u8 = 0x00;
pub const TLV_IDENTIFIER: u8 = 0x01;
pub const TLV_SALT: u8 = 0x02;
pub const TLV_PUBLIC_KEY: u8 = 0x03;
pub const TLV_PROOF: u8 = 0x04;
pub const TLV_ENCRYPTED_DATA: u8 = 0x05;
pub const TLV_STATE: u8 = 0x06;
pub const TLV_ERROR: u8 = 0x07;
pub const TLV_SIGNATURE: u8 = 0x0a;
pub const TLV_PERMISSIONS: u8 = 0x0b;
pub const TLV_FLAGS: u8 = 0x13;
pub const TLV_SEPARATOR: u8 = 0xff;

pub const TLV_ERROR_UNKNOWN: u8 = 0x01;
pub const TLV_ERROR_AUTHENTICATION: u8 = 0x02;
pub const TLV_ERROR_MAX_PEERS: u8 = 0x04;
pub const TLV_ERROR_MAX_TRIES: u8 = 0x05;
pub const TLV_ERROR_UNAVAILABLE: u8 = 0x06;
pub const TLV_ERROR_BUSY: u8 = 0x07;

const MAX_TLV_BYTES: usize = 4096;
const MAX_TLV_TYPES: usize = 32;

/// Parsed TLV8 values. Repeated fragments of a type are concatenated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tlv8 {
    values: BTreeMap<u8, Vec<u8>>,
}

impl Tlv8 {
    pub fn parse(input: &[u8]) -> Result<Self, HapError> {
        if input.len() > MAX_TLV_BYTES {
            return Err(HapError::Protocol("TLV8 body exceeds 4096 bytes".into()));
        }
        let mut values: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
        let mut offset = 0usize;
        while offset < input.len() {
            if input.len() - offset < 2 {
                return Err(HapError::Protocol("truncated TLV8 header".into()));
            }
            let kind = input[offset];
            let len = input[offset + 1] as usize;
            offset += 2;
            let end = offset
                .checked_add(len)
                .filter(|end| *end <= input.len())
                .ok_or_else(|| HapError::Protocol("truncated TLV8 value".into()))?;
            if !values.contains_key(&kind) && values.len() == MAX_TLV_TYPES {
                return Err(HapError::Protocol("too many TLV8 types".into()));
            }
            values
                .entry(kind)
                .or_default()
                .extend_from_slice(&input[offset..end]);
            offset = end;
        }
        Ok(Self { values })
    }

    pub fn get(&self, kind: u8) -> Option<&[u8]> {
        self.values.get(&kind).map(Vec::as_slice)
    }

    pub fn byte(&self, kind: u8) -> Option<u8> {
        let value = self.get(kind)?;
        (value.len() == 1).then(|| value[0])
    }

    pub fn insert(&mut self, kind: u8, value: impl Into<Vec<u8>>) {
        self.values.insert(kind, value.into());
    }

    pub fn encode(&self) -> Vec<u8> {
        encode_items(
            self.values
                .iter()
                .map(|(&kind, value)| (kind, value.as_slice())),
        )
    }
}

/// Encode ordered TLV8 items, including repeated items separated by a
/// zero-length `Separator` for `/pairings` list responses.
pub fn encode_items<'a>(items: impl IntoIterator<Item = (u8, &'a [u8])>) -> Vec<u8> {
    let mut encoded = Vec::new();
    for (kind, value) in items {
        if value.is_empty() {
            encoded.extend_from_slice(&[kind, 0]);
            continue;
        }
        for chunk in value.chunks(u8::MAX as usize) {
            encoded.push(kind);
            encoded.push(chunk.len() as u8);
            encoded.extend_from_slice(chunk);
        }
    }
    encoded
}

pub fn error_response(state: u8, error: u8) -> Vec<u8> {
    encode_items([
        (TLV_STATE, [state].as_slice()),
        (TLV_ERROR, [error].as_slice()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scalar_tlv_returns_none_without_panicking() {
        let parsed = Tlv8::parse(&[TLV_STATE, 0]).unwrap();
        assert_eq!(parsed.byte(TLV_STATE), None);
        assert_eq!(parsed.byte(TLV_METHOD), None);
        let multi = Tlv8::parse(&[TLV_STATE, 2, 1, 2]).unwrap();
        assert_eq!(multi.byte(TLV_STATE), None);
    }

    #[test]
    fn tlv8_roundtrip_supports_fragmented_values() {
        let mut tlv = Tlv8::default();
        tlv.insert(TLV_PUBLIC_KEY, vec![3; 300]);
        tlv.insert(TLV_STATE, vec![1]);
        let encoded = tlv.encode();
        let decoded = Tlv8::parse(&encoded).unwrap();
        assert_eq!(decoded, tlv);
    }

    #[test]
    fn malformed_or_oversized_tlv_is_rejected() {
        assert!(Tlv8::parse(&[TLV_STATE]).is_err());
        assert!(Tlv8::parse(&[TLV_STATE, 2, 1]).is_err());
        assert!(Tlv8::parse(&vec![0; MAX_TLV_BYTES + 1]).is_err());
    }

    #[test]
    fn error_response_is_not_success_shaped() {
        let response = error_response(2, TLV_ERROR_UNAVAILABLE);
        let decoded = Tlv8::parse(&response).unwrap();
        assert_eq!(decoded.byte(TLV_STATE), Some(2));
        assert_eq!(decoded.byte(TLV_ERROR), Some(TLV_ERROR_UNAVAILABLE));
    }
}
