//! Roundtrip + corruption-detection tests for the temporal head's
//! weight-blob wire format. The format is the contract between
//! host-side training and firmware-side inference — when this test
//! file changes, both ends update in lockstep.

use wifi_densepose_temporal::{
    WeightBlob, WeightBlobHeader, WeightDtype, WEIGHT_BLOB_HEADER_LEN, WEIGHT_BLOB_MAGIC,
    WEIGHT_BLOB_VERSION,
};

fn header_default() -> WeightBlobHeader {
    WeightBlobHeader {
        dtype: WeightDtype::F32,
        input_dim: 16,
        n_q_heads: 4,
        n_kv_heads: 1,
        head_dim: 32,
        n_layers: 2,
        n_classes: 4,
    }
}

#[test]
fn roundtrip_fp32() {
    let header = header_default();
    let weights: Vec<u8> = (0..1024).map(|i| (i & 0xFF) as u8).collect();
    let blob = WeightBlob::new(header, weights).expect("ok");
    let serialized = blob.serialize();
    let parsed = WeightBlob::parse(&serialized).expect("parse");
    assert_eq!(parsed.header.input_dim, 16);
    assert_eq!(parsed.header.n_q_heads, 4);
    assert_eq!(parsed.header.n_kv_heads, 1);
    assert_eq!(parsed.header.head_dim, 32);
    assert_eq!(parsed.header.n_layers, 2);
    assert_eq!(parsed.header.n_classes, 4);
    assert_eq!(parsed.header.dtype, WeightDtype::F32);
    assert_eq!(parsed.weights.len(), 1024);
}

#[test]
fn roundtrip_fp16() {
    let header = WeightBlobHeader {
        dtype: WeightDtype::F16,
        ..header_default()
    };
    // FP16 means 2 bytes per element — 512 bytes = 256 elements.
    let weights: Vec<u8> = (0..512).map(|i| (i & 0xFF) as u8).collect();
    let blob = WeightBlob::new(header, weights).expect("ok");
    let serialized = blob.serialize();
    let parsed = WeightBlob::parse(&serialized).expect("parse");
    assert_eq!(parsed.header.dtype, WeightDtype::F16);
    assert_eq!(parsed.weights.len(), 512);
}

#[test]
fn parse_rejects_bad_magic() {
    let header = header_default();
    let blob = WeightBlob::new(header, vec![0u8; 16]).expect("ok");
    let mut bytes = blob.serialize();
    bytes[0] = 0xFF; // corrupt magic
    let err = WeightBlob::parse(&bytes).err().expect("rejected");
    assert!(format!("{err}").contains("magic"));
}

#[test]
fn parse_rejects_wrong_version() {
    let header = header_default();
    let blob = WeightBlob::new(header, vec![0u8; 16]).expect("ok");
    let mut bytes = blob.serialize();
    bytes[4] = 99; // bump version
    bytes[5] = 0;
    let err = WeightBlob::parse(&bytes).err().expect("rejected");
    assert!(format!("{err}").contains("version"));
}

#[test]
fn parse_rejects_size_mismatch() {
    let header = header_default();
    let blob = WeightBlob::new(header, vec![0u8; 64]).expect("ok");
    let mut bytes = blob.serialize();
    // truncate the weights region by 4 bytes — total length now
    // doesn't match the weights_len field.
    bytes.drain(WEIGHT_BLOB_HEADER_LEN..WEIGHT_BLOB_HEADER_LEN + 4);
    let err = WeightBlob::parse(&bytes).err().expect("rejected");
    assert!(format!("{err}").contains("length") || format!("{err}").contains("CRC"));
}

#[test]
fn parse_rejects_crc_corruption() {
    let header = header_default();
    let blob = WeightBlob::new(header, vec![0xAAu8; 32]).expect("ok");
    let mut bytes = blob.serialize();
    // flip a bit in the middle of the weights region
    let mid = WEIGHT_BLOB_HEADER_LEN + 5;
    bytes[mid] ^= 0x01;
    let err = WeightBlob::parse(&bytes).err().expect("rejected");
    assert!(format!("{err}").contains("CRC"));
}

#[test]
fn parse_rejects_invalid_gqa_ratio_in_header() {
    // Manually craft bytes where n_q_heads % n_kv_heads != 0 to ensure
    // header.validate() fires from inside parse(). Easiest: build a
    // valid blob then patch the n_kv_heads field.
    let header = header_default();
    let blob = WeightBlob::new(header, vec![0u8; 16]).expect("ok");
    let mut bytes = blob.serialize();
    // n_kv_heads is at offset 12..14; set it to 3 so 4 % 3 != 0.
    bytes[12] = 3;
    bytes[13] = 0;
    // Re-CRC so we can be sure the validator (not the CRC) is what
    // rejects this case.
    let new_crc = crc32_ieee(&bytes[..bytes.len() - 4]);
    let crc_off = bytes.len() - 4;
    bytes[crc_off..].copy_from_slice(&new_crc.to_le_bytes());
    let err = WeightBlob::parse(&bytes).err().expect("rejected");
    assert!(format!("{err}").to_lowercase().contains("gqa"));
}

#[test]
fn header_constants_match_wire_layout() {
    // Anchor the public constants so they can't drift silently.
    assert_eq!(WEIGHT_BLOB_MAGIC, 0x5256_4E45);
    assert_eq!(WEIGHT_BLOB_VERSION, 1);
    assert_eq!(WEIGHT_BLOB_HEADER_LEN, 24);
}

// Mirror of the production CRC32 so the size-mismatch / GQA tests can
// re-CRC after their patch. Kept out of the public API.
fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
