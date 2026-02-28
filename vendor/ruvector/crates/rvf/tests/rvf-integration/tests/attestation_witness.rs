//! Attestation system integration tests.
//!
//! Exercises the Confidential Core attestation APIs end-to-end:
//! record encoding/decoding, witness chain integrity, tamper detection,
//! TEE-bound key lifecycle, segment flags, and mixed witness type chains.

use rvf_crypto::attestation::{
    build_attestation_witness_payload, decode_attestation_record, decode_tee_bound_key,
    encode_attestation_record, encode_tee_bound_key, verify_attestation_witness_payload,
    verify_key_binding, TeeBoundKeyRecord,
};
use rvf_crypto::hash::{shake256_128, shake256_256};
use rvf_crypto::witness::{create_witness_chain, verify_witness_chain, WitnessEntry};
use rvf_types::{
    AttestationHeader, AttestationWitnessType, ErrorCode, RvfError, SegmentFlags, TeePlatform,
    KEY_TYPE_TEE_BOUND,
};

// --------------------------------------------------------------------------
// 1. Attestation record round trip
// --------------------------------------------------------------------------
#[test]
fn attestation_record_round_trip() {
    let mut header = AttestationHeader::new(
        TeePlatform::SoftwareTee as u8,
        AttestationWitnessType::PlatformAttestation as u8,
    );
    header.measurement = shake256_256(b"test-enclave");
    header.nonce = [0x42; 16];
    header.quote_length = 64;
    header.report_data_len = 32;
    header.flags = AttestationHeader::FLAG_HAS_REPORT_DATA;

    let report_data: Vec<u8> = (0..32).map(|i| (i * 3) as u8).collect();
    let quote: Vec<u8> = (0..64).map(|i| (i ^ 0xAB) as u8).collect();

    // Encode.
    let encoded = encode_attestation_record(&header, &report_data, &quote);
    assert_eq!(
        encoded.len(),
        112 + 32 + 64,
        "total record should be header + report_data + quote"
    );

    // Decode.
    let (dec_hdr, dec_rd, dec_q) = decode_attestation_record(&encoded).unwrap();

    // Verify all header fields match.
    assert_eq!(dec_hdr.platform, TeePlatform::SoftwareTee as u8);
    assert_eq!(
        dec_hdr.attestation_type,
        AttestationWitnessType::PlatformAttestation as u8
    );
    assert_eq!(dec_hdr.measurement, header.measurement);
    assert_eq!(dec_hdr.nonce, [0x42; 16]);
    assert_eq!(dec_hdr.quote_length, 64);
    assert_eq!(dec_hdr.report_data_len, 32);
    assert_eq!(dec_hdr.flags, AttestationHeader::FLAG_HAS_REPORT_DATA);
    assert!(dec_hdr.has_report_data());
    assert!(!dec_hdr.is_debuggable());

    // Verify variable-length sections.
    assert_eq!(dec_rd, report_data);
    assert_eq!(dec_q, quote);
}

// --------------------------------------------------------------------------
// 2. Attestation witness chain integrity
// --------------------------------------------------------------------------
#[test]
fn attestation_witness_chain_integrity() {
    // Create 3 attestation records with different platforms and witness types.
    let configs: &[(TeePlatform, AttestationWitnessType)] = &[
        (
            TeePlatform::Sgx,
            AttestationWitnessType::PlatformAttestation,
        ),
        (
            TeePlatform::SevSnp,
            AttestationWitnessType::ComputationProof,
        ),
        (TeePlatform::Tdx, AttestationWitnessType::DataProvenance),
    ];

    let mut records: Vec<Vec<u8>> = Vec::new();
    let mut timestamps: Vec<u64> = Vec::new();
    let mut witness_types: Vec<AttestationWitnessType> = Vec::new();

    for (i, &(platform, wit_type)) in configs.iter().enumerate() {
        let mut header = AttestationHeader::new(platform as u8, wit_type as u8);
        header.measurement = shake256_256(format!("enclave-{i}").as_bytes());
        header.nonce = [(i + 1) as u8; 16];
        header.quote_length = 32;
        header.report_data_len = 16;
        header.flags = AttestationHeader::FLAG_HAS_REPORT_DATA;

        let report_data: Vec<u8> = vec![i as u8; 16];
        let quote: Vec<u8> = vec![(i + 0x10) as u8; 32];

        records.push(encode_attestation_record(&header, &report_data, &quote));
        timestamps.push(1_000_000_000 + i as u64);
        witness_types.push(wit_type);
    }

    // Build witness payload.
    let payload = build_attestation_witness_payload(&records, &timestamps, &witness_types).unwrap();

    // Verify.
    let verified = verify_attestation_witness_payload(&payload).unwrap();
    assert_eq!(verified.len(), 3, "should have 3 verified entries");

    // Check each entry has the correct action_hash and witness type.
    for (i, (entry, header, rd, q)) in verified.iter().enumerate() {
        let expected_hash = shake256_256(&records[i]);
        assert_eq!(
            entry.action_hash, expected_hash,
            "entry {i}: action_hash should match SHAKE-256 of record"
        );
        assert_eq!(
            entry.witness_type, witness_types[i] as u8,
            "entry {i}: witness_type mismatch"
        );
        assert_eq!(
            header.platform, configs[i].0 as u8,
            "entry {i}: platform mismatch"
        );
        assert_eq!(rd.len(), 16, "entry {i}: report_data length");
        assert_eq!(q.len(), 32, "entry {i}: quote length");
    }
}

// --------------------------------------------------------------------------
// 3. Attestation witness tamper detection
// --------------------------------------------------------------------------
#[test]
fn attestation_witness_tamper_detection() {
    // Build a payload with 2 entries.
    let mut records: Vec<Vec<u8>> = Vec::new();
    let mut timestamps: Vec<u64> = Vec::new();
    let mut witness_types: Vec<AttestationWitnessType> = Vec::new();

    for i in 0..2 {
        let mut header = AttestationHeader::new(
            TeePlatform::SoftwareTee as u8,
            AttestationWitnessType::PlatformAttestation as u8,
        );
        header.measurement = shake256_256(format!("tamper-test-{i}").as_bytes());
        header.quote_length = 48;
        header.report_data_len = 24;
        header.flags = AttestationHeader::FLAG_HAS_REPORT_DATA;

        let report_data: Vec<u8> = vec![i as u8; 24];
        let quote: Vec<u8> = vec![0xDD; 48];

        records.push(encode_attestation_record(&header, &report_data, &quote));
        timestamps.push(2_000_000_000 + i);
        witness_types.push(AttestationWitnessType::PlatformAttestation);
    }

    let mut payload =
        build_attestation_witness_payload(&records, &timestamps, &witness_types).unwrap();

    // The payload layout is:
    //   [4 bytes: count][2*8 bytes: offsets][2*73 bytes: chain][records...]
    // Records start at offset = 4 + 16 + 146 = 166.
    // Flip a byte somewhere in the records section to simulate tampering.
    let records_start = 4 + 2 * 8 + 2 * 73;
    assert!(
        records_start + 50 < payload.len(),
        "payload should be large enough to tamper"
    );
    payload[records_start + 50] ^= 0xFF;

    // Verification should fail with InvalidChecksum.
    let result = verify_attestation_witness_payload(&payload);
    assert!(result.is_err(), "tampered payload should fail verification");
    assert_eq!(
        result.unwrap_err(),
        RvfError::Code(ErrorCode::InvalidChecksum),
        "error should be InvalidChecksum"
    );
}

// --------------------------------------------------------------------------
// 4. TEE-bound key lifecycle
// --------------------------------------------------------------------------
#[test]
fn tee_bound_key_lifecycle() {
    let measurement = shake256_256(b"test-measurement");
    let sealed_key: Vec<u8> = vec![0xAA; 32];

    let record = TeeBoundKeyRecord {
        key_type: KEY_TYPE_TEE_BOUND,
        algorithm: 1,
        sealed_key_length: sealed_key.len() as u16,
        key_id: shake256_128(b"test-key-id"),
        measurement,
        platform: TeePlatform::SoftwareTee as u8,
        reserved: [0u8; 3],
        valid_from: 0,
        valid_until: 0, // no expiry
        sealed_key: sealed_key.clone(),
    };

    // Encode and decode round-trip.
    let encoded = encode_tee_bound_key(&record);
    let decoded = decode_tee_bound_key(&encoded).unwrap();

    assert_eq!(decoded.key_type, KEY_TYPE_TEE_BOUND);
    assert_eq!(decoded.measurement, measurement);
    assert_eq!(decoded.sealed_key, sealed_key);
    assert_eq!(decoded.platform, TeePlatform::SoftwareTee as u8);
    assert_eq!(decoded.sealed_key_length, 32);

    // Verify key binding with matching platform and measurement -> Ok.
    let result = verify_key_binding(&decoded, TeePlatform::SoftwareTee, &measurement, 1_000_000);
    assert!(result.is_ok(), "matching binding should succeed");

    // Wrong platform -> KeyNotBound.
    let result = verify_key_binding(
        &decoded,
        TeePlatform::Sgx, // wrong
        &measurement,
        1_000_000,
    );
    assert_eq!(
        result,
        Err(RvfError::Code(ErrorCode::KeyNotBound)),
        "wrong platform should return KeyNotBound"
    );

    // Wrong measurement -> KeyNotBound.
    let wrong_measurement = shake256_256(b"wrong-measurement");
    let result = verify_key_binding(
        &decoded,
        TeePlatform::SoftwareTee,
        &wrong_measurement,
        1_000_000,
    );
    assert_eq!(
        result,
        Err(RvfError::Code(ErrorCode::KeyNotBound)),
        "wrong measurement should return KeyNotBound"
    );
}

// --------------------------------------------------------------------------
// 5. Attested segment flag
// --------------------------------------------------------------------------
#[test]
fn attested_segment_flag() {
    // ATTESTED flag alone.
    let flags = SegmentFlags::empty().with(SegmentFlags::ATTESTED);
    assert!(
        flags.contains(SegmentFlags::ATTESTED),
        "ATTESTED flag should be set"
    );
    assert!(
        !flags.contains(SegmentFlags::SIGNED),
        "SIGNED should not be set when only ATTESTED is"
    );
    assert!(
        !flags.contains(SegmentFlags::SEALED),
        "SEALED should not be set when only ATTESTED is"
    );

    // Combined flags: SIGNED | SEALED | ATTESTED.
    let combined = SegmentFlags::empty()
        .with(SegmentFlags::SIGNED)
        .with(SegmentFlags::SEALED)
        .with(SegmentFlags::ATTESTED);
    assert!(combined.contains(SegmentFlags::SIGNED));
    assert!(combined.contains(SegmentFlags::SEALED));
    assert!(combined.contains(SegmentFlags::ATTESTED));

    // Verify bit positions.
    assert_eq!(SegmentFlags::ATTESTED, 0x0400, "ATTESTED should be bit 10");
    let expected_bits = 0x0004 | 0x0008 | 0x0400;
    assert_eq!(
        combined.bits(),
        expected_bits,
        "combined bits should be SIGNED|SEALED|ATTESTED"
    );
}

// --------------------------------------------------------------------------
// 6. Mixed witness types in chain
// --------------------------------------------------------------------------
#[test]
fn mixed_witness_types_in_chain() {
    // Build a chain with both standard and attestation witness types.
    let entries = vec![
        // Entry 1: standard PROVENANCE (0x01).
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"provenance-data"),
            timestamp_ns: 1_000_000_001,
            witness_type: 0x01,
        },
        // Entry 2: new PLATFORM_ATTESTATION (0x05).
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"platform-attestation-data"),
            timestamp_ns: 1_000_000_002,
            witness_type: AttestationWitnessType::PlatformAttestation as u8,
        },
        // Entry 3: standard COMPUTATION (0x02).
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"computation-data"),
            timestamp_ns: 1_000_000_003,
            witness_type: 0x02,
        },
        // Entry 4: new COMPUTATION_PROOF (0x07).
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"computation-proof-data"),
            timestamp_ns: 1_000_000_004,
            witness_type: AttestationWitnessType::ComputationProof as u8,
        },
    ];

    // Create the chain (links entries via prev_hash).
    let chain = create_witness_chain(&entries);
    assert_eq!(
        chain.len(),
        4 * 73,
        "chain should have 4 entries of 73 bytes each"
    );

    // Verify chain integrity.
    let verified = verify_witness_chain(&chain).unwrap();
    assert_eq!(verified.len(), 4, "all 4 entries should verify");

    // Check witness_type values.
    assert_eq!(verified[0].witness_type, 0x01, "entry 0: PROVENANCE");
    assert_eq!(
        verified[1].witness_type, 0x05,
        "entry 1: PLATFORM_ATTESTATION"
    );
    assert_eq!(verified[2].witness_type, 0x02, "entry 2: COMPUTATION");
    assert_eq!(verified[3].witness_type, 0x07, "entry 3: COMPUTATION_PROOF");

    // Verify action hashes are preserved.
    assert_eq!(verified[0].action_hash, shake256_256(b"provenance-data"));
    assert_eq!(
        verified[1].action_hash,
        shake256_256(b"platform-attestation-data")
    );
    assert_eq!(verified[2].action_hash, shake256_256(b"computation-data"));
    assert_eq!(
        verified[3].action_hash,
        shake256_256(b"computation-proof-data")
    );

    // First entry has zero prev_hash, subsequent are chained.
    assert_eq!(
        verified[0].prev_hash, [0u8; 32],
        "first entry should have zero prev_hash"
    );
    assert_ne!(
        verified[1].prev_hash, [0u8; 32],
        "second entry should have non-zero prev_hash"
    );
}
