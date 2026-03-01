//! Cryptographic signature integration tests.
//!
//! Tests rvf-crypto segment signing and verification, SHAKE-256 hashing,
//! and witness chain integrity.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rvf_crypto::hash::{shake256_128, shake256_256};
use rvf_crypto::sign::{sign_segment, verify_segment};
use rvf_crypto::witness::{create_witness_chain, verify_witness_chain, WitnessEntry};
use rvf_types::SegmentHeader;

fn make_test_header(seg_id: u64) -> SegmentHeader {
    let mut h = SegmentHeader::new(0x01, seg_id);
    h.timestamp_ns = 1_000_000_000;
    h.payload_length = 100;
    h
}

#[test]
fn shake256_hash_deterministic() {
    let data = b"RuVector Format test data";
    let h1 = shake256_128(data);
    let h2 = shake256_128(data);
    assert_eq!(h1, h2, "SHAKE-256 should be deterministic");
}

#[test]
fn shake256_different_inputs_different_hashes() {
    let h1 = shake256_128(b"input A");
    let h2 = shake256_128(b"input B");
    assert_ne!(h1, h2, "different inputs should produce different hashes");
}

#[test]
fn shake256_128_is_prefix_of_256() {
    let data = b"consistency check";
    let h128 = shake256_128(data);
    let h256 = shake256_256(data);

    assert_eq!(h128.len(), 16, "SHAKE-256-128 should produce 16 bytes");
    assert_eq!(h256.len(), 32, "SHAKE-256-256 should produce 32 bytes");
    assert_eq!(
        &h128[..],
        &h256[..16],
        "128-bit should be prefix of 256-bit"
    );
}

#[test]
fn sign_and_verify_segment_ed25519() {
    let key = SigningKey::generate(&mut OsRng);
    let header = make_test_header(42);
    let payload = b"segment payload containing vectors";

    let footer = sign_segment(&header, payload, &key);
    let pubkey = key.verifying_key();

    assert!(
        verify_segment(&header, payload, &footer, &pubkey),
        "valid signature should verify"
    );
}

#[test]
fn verify_fails_on_corrupted_payload() {
    let key = SigningKey::generate(&mut OsRng);
    let header = make_test_header(1);
    let payload = b"original payload";

    let footer = sign_segment(&header, payload, &key);
    let pubkey = key.verifying_key();

    let corrupted = b"corrupted payload";
    assert!(
        !verify_segment(&header, corrupted, &footer, &pubkey),
        "corrupted payload should fail verification"
    );
}

#[test]
fn verify_fails_on_wrong_key() {
    let key1 = SigningKey::generate(&mut OsRng);
    let key2 = SigningKey::generate(&mut OsRng);
    let header = make_test_header(1);
    let payload = b"payload data";

    let footer = sign_segment(&header, payload, &key1);
    let wrong_pubkey = key2.verifying_key();

    assert!(
        !verify_segment(&header, payload, &footer, &wrong_pubkey),
        "wrong public key should fail verification"
    );
}

#[test]
fn verify_fails_on_tampered_header() {
    let key = SigningKey::generate(&mut OsRng);
    let header = make_test_header(42);
    let payload = b"payload";

    let footer = sign_segment(&header, payload, &key);
    let pubkey = key.verifying_key();

    let mut bad_header = header;
    bad_header.segment_id = 999;
    assert!(
        !verify_segment(&bad_header, payload, &footer, &pubkey),
        "tampered header should fail verification"
    );
}

#[test]
fn witness_chain_create_and_verify() {
    let entries: Vec<WitnessEntry> = (0..5)
        .map(|i| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(&[i as u8]),
            timestamp_ns: 1_000_000_000 + i as u64,
            witness_type: 0x01,
        })
        .collect();

    let chain = create_witness_chain(&entries);
    assert!(!chain.is_empty());

    let verified = verify_witness_chain(&chain).unwrap();
    assert_eq!(verified.len(), entries.len());

    // Action hashes should match.
    for (i, entry) in verified.iter().enumerate() {
        assert_eq!(entry.action_hash, entries[i].action_hash);
        assert_eq!(entry.timestamp_ns, entries[i].timestamp_ns);
    }
}

#[test]
fn witness_chain_detects_tampering() {
    let entries: Vec<WitnessEntry> = (0..3)
        .map(|i| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(&[i as u8]),
            timestamp_ns: 1_000_000_000 + i as u64,
            witness_type: 0x01,
        })
        .collect();

    let mut chain = create_witness_chain(&entries);

    // Tamper with the second entry's action_hash (offset 73 is start of entry 1,
    // action_hash is at offset +32 within entry).
    chain[73 + 32] ^= 0xFF;

    assert!(
        verify_witness_chain(&chain).is_err(),
        "tampered chain should fail verification"
    );
}

#[test]
fn witness_chain_empty_is_valid() {
    let chain = create_witness_chain(&[]);
    assert!(chain.is_empty());
    let verified = verify_witness_chain(&chain).unwrap();
    assert!(verified.is_empty());
}
