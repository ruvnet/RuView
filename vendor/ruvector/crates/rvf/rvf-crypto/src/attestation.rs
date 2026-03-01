//! Confidential Core attestation module.
//!
//! Provides encoding/decoding of attestation records for WITNESS_SEG,
//! attestation-aware witness chain extensions, key-binding helpers for
//! CRYPTO_SEG, and a trait for pluggable platform-specific verification.

use alloc::vec::Vec;
use rvf_types::{AttestationHeader, AttestationWitnessType, ErrorCode, RvfError, TeePlatform};

use crate::hash::shake256_256;
use crate::witness::{create_witness_chain, verify_witness_chain, WitnessEntry};

// ---------------------------------------------------------------------------
// 1. AttestationHeader Codec
// ---------------------------------------------------------------------------

/// Size of a serialized `AttestationHeader` on the wire.
const ATTESTATION_HEADER_SIZE: usize = 112;

/// Size of one serialized witness entry (must match witness module).
const WITNESS_ENTRY_SIZE: usize = 73;

/// Encode an `AttestationHeader` to its 112-byte wire representation.
pub fn encode_attestation_header(header: &AttestationHeader) -> [u8; ATTESTATION_HEADER_SIZE] {
    let mut buf = [0u8; ATTESTATION_HEADER_SIZE];
    buf[0x00] = header.platform;
    buf[0x01] = header.attestation_type;
    buf[0x02..0x04].copy_from_slice(&header.quote_length.to_le_bytes());
    buf[0x04..0x08].copy_from_slice(&header.reserved_0.to_le_bytes());
    buf[0x08..0x28].copy_from_slice(&header.measurement);
    buf[0x28..0x48].copy_from_slice(&header.signer_id);
    buf[0x48..0x50].copy_from_slice(&header.timestamp_ns.to_le_bytes());
    buf[0x50..0x60].copy_from_slice(&header.nonce);
    buf[0x60..0x62].copy_from_slice(&header.svn.to_le_bytes());
    buf[0x62..0x64].copy_from_slice(&header.sig_algo.to_le_bytes());
    buf[0x64] = header.flags;
    buf[0x65..0x68].copy_from_slice(&header.reserved_1);
    buf[0x68..0x70].copy_from_slice(&header.report_data_len.to_le_bytes());
    buf
}

/// Decode an `AttestationHeader` from wire bytes.
///
/// Returns `ErrorCode::TruncatedSegment` if `data.len() < 112`.
pub fn decode_attestation_header(data: &[u8]) -> Result<AttestationHeader, RvfError> {
    if data.len() < ATTESTATION_HEADER_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let platform = data[0x00];
    let attestation_type = data[0x01];
    let quote_length = u16::from_le_bytes([data[0x02], data[0x03]]);
    let reserved_0 = u32::from_le_bytes(data[0x04..0x08].try_into().unwrap());

    let mut measurement = [0u8; 32];
    measurement.copy_from_slice(&data[0x08..0x28]);

    let mut signer_id = [0u8; 32];
    signer_id.copy_from_slice(&data[0x28..0x48]);

    let timestamp_ns = u64::from_le_bytes(data[0x48..0x50].try_into().unwrap());

    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&data[0x50..0x60]);

    let svn = u16::from_le_bytes([data[0x60], data[0x61]]);
    let sig_algo = u16::from_le_bytes([data[0x62], data[0x63]]);
    let flags = data[0x64];

    let mut reserved_1 = [0u8; 3];
    reserved_1.copy_from_slice(&data[0x65..0x68]);

    let report_data_len = u64::from_le_bytes(data[0x68..0x70].try_into().unwrap());

    Ok(AttestationHeader {
        platform,
        attestation_type,
        quote_length,
        reserved_0,
        measurement,
        signer_id,
        timestamp_ns,
        nonce,
        svn,
        sig_algo,
        flags,
        reserved_1,
        report_data_len,
    })
}

// ---------------------------------------------------------------------------
// 2. Full Attestation Record Codec
// ---------------------------------------------------------------------------

/// Encode a complete attestation record: header + report_data + quote.
pub fn encode_attestation_record(
    header: &AttestationHeader,
    report_data: &[u8],
    quote: &[u8],
) -> Vec<u8> {
    let hdr_bytes = encode_attestation_header(header);
    let total = ATTESTATION_HEADER_SIZE + report_data.len() + quote.len();
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&hdr_bytes);
    buf.extend_from_slice(report_data);
    buf.extend_from_slice(quote);
    buf
}

/// Decode an attestation record, returning `(header, report_data, quote)`.
///
/// Returns `ErrorCode::TruncatedSegment` if data is too short for the
/// declared `report_data_len` and `quote_length`.
pub fn decode_attestation_record(
    data: &[u8],
) -> Result<(AttestationHeader, Vec<u8>, Vec<u8>), RvfError> {
    let header = decode_attestation_header(data)?;

    let rd_len = header.report_data_len as usize;
    let q_len = header.quote_length as usize;
    let total_needed = ATTESTATION_HEADER_SIZE + rd_len + q_len;

    if data.len() < total_needed {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let rd_start = ATTESTATION_HEADER_SIZE;
    let rd_end = rd_start + rd_len;
    let report_data = data[rd_start..rd_end].to_vec();

    let q_start = rd_end;
    let q_end = q_start + q_len;
    let quote = data[q_start..q_end].to_vec();

    Ok((header, report_data, quote))
}

// ---------------------------------------------------------------------------
// 3. Witness Chain Integration
// ---------------------------------------------------------------------------

/// Create a witness chain entry for an attestation event.
///
/// The `action_hash` is SHAKE-256-256 of the full attestation record bytes.
pub fn attestation_witness_entry(
    attestation_record: &[u8],
    timestamp_ns: u64,
    witness_type: AttestationWitnessType,
) -> WitnessEntry {
    WitnessEntry {
        prev_hash: [0u8; 32], // will be set by create_witness_chain
        action_hash: shake256_256(attestation_record),
        timestamp_ns,
        witness_type: witness_type as u8,
    }
}

/// Build a WITNESS_SEG payload for attestation records.
///
/// Wire layout:
///   `chain_entry_count`: u32 (LE)
///   `record_offsets`: [u64; count] (LE, byte offsets into records section)
///   `witness_chain`: [WitnessEntry; count] (73 bytes each, linked via SHAKE-256)
///   `records`: concatenated attestation record bytes
pub fn build_attestation_witness_payload(
    records: &[Vec<u8>],
    timestamps: &[u64],
    witness_types: &[AttestationWitnessType],
) -> Result<Vec<u8>, RvfError> {
    let count = records.len();

    // 1. Create witness entries for each record.
    let entries: Vec<WitnessEntry> = records
        .iter()
        .enumerate()
        .map(|(i, rec)| attestation_witness_entry(rec, timestamps[i], witness_types[i]))
        .collect();

    // 2. Run create_witness_chain to link entries via hashes.
    let chain_bytes = create_witness_chain(&entries);

    // 3. Compute record offsets (cumulative sums of record lengths).
    let mut offsets = Vec::with_capacity(count);
    let mut cumulative: u64 = 0;
    for rec in records {
        offsets.push(cumulative);
        cumulative = cumulative
            .checked_add(rec.len() as u64)
            .ok_or(RvfError::Code(ErrorCode::SegmentTooLarge))?;
    }

    // 4. Concatenate: count(u32) + offsets([u64; n]) + chain_bytes + records.
    let total = 4 + count * 8 + chain_bytes.len() + cumulative as usize;
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&(count as u32).to_le_bytes());
    for off in &offsets {
        buf.extend_from_slice(&off.to_le_bytes());
    }
    buf.extend_from_slice(&chain_bytes);
    for rec in records {
        buf.extend_from_slice(rec);
    }
    Ok(buf)
}

/// A verified attestation entry: `(WitnessEntry, AttestationHeader, report_data, quote)`.
pub type VerifiedAttestationEntry = (WitnessEntry, AttestationHeader, Vec<u8>, Vec<u8>);

/// Verify an attestation witness payload.
///
/// Returns decoded entries paired with their attestation records.
pub fn verify_attestation_witness_payload(
    data: &[u8],
) -> Result<Vec<VerifiedAttestationEntry>, RvfError> {
    // 1. Read count from first 4 bytes.
    if data.len() < 4 {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;

    if count == 0 {
        return Ok(Vec::new());
    }

    // 2. Read offset table.
    let offsets_end = 4 + count * 8;
    if data.len() < offsets_end {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let o = 4 + i * 8;
        let offset = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
        offsets.push(offset as usize);
    }

    // 3. Extract witness chain bytes and verify.
    let chain_start = offsets_end;
    let chain_len = count * WITNESS_ENTRY_SIZE;
    let chain_end = chain_start + chain_len;
    if data.len() < chain_end {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let chain_bytes = &data[chain_start..chain_end];
    let entries = verify_witness_chain(chain_bytes)?;

    // 4. Records start after the chain.
    let records_base = chain_end;
    let records_data = if records_base <= data.len() {
        &data[records_base..]
    } else {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    };

    // 5. For each entry, decode the attestation record at the corresponding offset.
    let mut results = Vec::with_capacity(count);
    for (i, entry) in entries.iter().enumerate() {
        let rec_start = offsets[i];
        // Determine record end from the next offset, or from total records length.
        let rec_end = if i + 1 < count {
            offsets[i + 1]
        } else {
            records_data.len()
        };

        if rec_start > records_data.len() || rec_end > records_data.len() {
            return Err(RvfError::Code(ErrorCode::TruncatedSegment));
        }

        let record_bytes = &records_data[rec_start..rec_end];

        // Verify action_hash matches shake256_256(record_bytes).
        let expected_hash = shake256_256(record_bytes);
        if entry.action_hash != expected_hash {
            return Err(RvfError::Code(ErrorCode::InvalidChecksum));
        }

        let (header, report_data, quote) = decode_attestation_record(record_bytes)?;
        results.push((entry.clone(), header, report_data, quote));
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// 4. TEE-Bound Key Record
// ---------------------------------------------------------------------------

/// A TEE-bound key record for CRYPTO_SEG.
#[derive(Clone, Debug, PartialEq)]
pub struct TeeBoundKeyRecord {
    /// Always `KEY_TYPE_TEE_BOUND` (4).
    pub key_type: u8,
    /// `SignatureAlgo` / KEM algo discriminant.
    pub algorithm: u8,
    /// Length of the sealed key material.
    pub sealed_key_length: u16,
    /// SHAKE-256-128 of the public key.
    pub key_id: [u8; 16],
    /// TEE measurement that seals this key.
    pub measurement: [u8; 32],
    /// `TeePlatform` discriminant.
    pub platform: u8,
    /// Reserved, must be zero.
    pub reserved: [u8; 3],
    /// Timestamp (nanoseconds) when key becomes valid.
    pub valid_from: u64,
    /// Timestamp (nanoseconds) when key expires. 0 = no expiry.
    pub valid_until: u64,
    /// Sealed key material.
    pub sealed_key: Vec<u8>,
}

/// Size of the fixed header portion of a `TeeBoundKeyRecord`.
const TEE_KEY_HEADER_SIZE: usize = 72;

/// Encode a `TeeBoundKeyRecord` to wire format.
pub fn encode_tee_bound_key(record: &TeeBoundKeyRecord) -> Vec<u8> {
    let total = TEE_KEY_HEADER_SIZE + record.sealed_key.len();
    let mut buf = Vec::with_capacity(total);

    buf.push(record.key_type); // 0x00
    buf.push(record.algorithm); // 0x01
    buf.extend_from_slice(&record.sealed_key_length.to_le_bytes()); // 0x02..0x04
    buf.extend_from_slice(&record.key_id); // 0x04..0x14
    buf.extend_from_slice(&record.measurement); // 0x14..0x34
    buf.push(record.platform); // 0x34
    buf.extend_from_slice(&record.reserved); // 0x35..0x38
    buf.extend_from_slice(&record.valid_from.to_le_bytes()); // 0x38..0x40
    buf.extend_from_slice(&record.valid_until.to_le_bytes()); // 0x40..0x48
    buf.extend_from_slice(&record.sealed_key); // 0x48..

    buf
}

/// Decode a `TeeBoundKeyRecord` from wire format.
///
/// Returns `ErrorCode::TruncatedSegment` if data is too short.
pub fn decode_tee_bound_key(data: &[u8]) -> Result<TeeBoundKeyRecord, RvfError> {
    if data.len() < TEE_KEY_HEADER_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let key_type = data[0x00];
    let algorithm = data[0x01];
    let sealed_key_length = u16::from_le_bytes([data[0x02], data[0x03]]);

    let mut key_id = [0u8; 16];
    key_id.copy_from_slice(&data[0x04..0x14]);

    let mut measurement = [0u8; 32];
    measurement.copy_from_slice(&data[0x14..0x34]);

    let platform = data[0x34];

    let mut reserved = [0u8; 3];
    reserved.copy_from_slice(&data[0x35..0x38]);

    let valid_from = u64::from_le_bytes(data[0x38..0x40].try_into().unwrap());
    let valid_until = u64::from_le_bytes(data[0x40..0x48].try_into().unwrap());

    let sk_len = sealed_key_length as usize;
    if data.len() < TEE_KEY_HEADER_SIZE + sk_len {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let sealed_key = data[0x48..0x48 + sk_len].to_vec();

    Ok(TeeBoundKeyRecord {
        key_type,
        algorithm,
        sealed_key_length,
        key_id,
        measurement,
        platform,
        reserved,
        valid_from,
        valid_until,
        sealed_key,
    })
}

// ---------------------------------------------------------------------------
// 5. Key Binding Verification
// ---------------------------------------------------------------------------

/// Verify that a TEE-bound key is accessible in the current environment.
///
/// Checks platform, measurement, and expiry.
pub fn verify_key_binding(
    key: &TeeBoundKeyRecord,
    current_platform: TeePlatform,
    current_measurement: &[u8; 32],
    current_time_ns: u64,
) -> Result<(), RvfError> {
    // Check platform matches.
    if key.platform != current_platform as u8 {
        return Err(RvfError::Code(ErrorCode::KeyNotBound));
    }

    // Check measurement matches.
    if key.measurement != *current_measurement {
        return Err(RvfError::Code(ErrorCode::KeyNotBound));
    }

    // Check not expired (valid_until == 0 means no expiry).
    if key.valid_until != 0 && current_time_ns > key.valid_until {
        return Err(RvfError::Code(ErrorCode::KeyExpired));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. QuoteVerifier Trait
// ---------------------------------------------------------------------------

/// Platform-specific attestation quote verifier.
///
/// Object-safe for dynamic dispatch.
pub trait QuoteVerifier {
    /// The TEE platform this verifier handles.
    fn platform(&self) -> TeePlatform;

    /// Verify a quote against its header and report data.
    ///
    /// Returns `Ok(true)` if valid, `Ok(false)` if invalid, or an error
    /// if verification could not be performed.
    fn verify_quote(
        &self,
        header: &AttestationHeader,
        report_data: &[u8],
        quote: &[u8],
    ) -> Result<bool, RvfError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::shake256_128;
    use alloc::vec;
    use rvf_types::KEY_TYPE_TEE_BOUND;

    /// Helper: build a fully-populated AttestationHeader.
    fn make_test_header(report_data_len: u64, quote_length: u16) -> AttestationHeader {
        let mut measurement = [0u8; 32];
        measurement[0] = 0xAA;
        measurement[31] = 0xBB;

        let mut signer_id = [0u8; 32];
        signer_id[0] = 0xCC;
        signer_id[31] = 0xDD;

        let mut nonce = [0u8; 16];
        nonce[0] = 0x01;
        nonce[15] = 0x0F;

        AttestationHeader {
            platform: TeePlatform::SevSnp as u8,
            attestation_type: AttestationWitnessType::PlatformAttestation as u8,
            quote_length,
            reserved_0: 0,
            measurement,
            signer_id,
            timestamp_ns: 1_700_000_000_000_000_000,
            nonce,
            svn: 42,
            sig_algo: 1,
            flags: AttestationHeader::FLAG_HAS_REPORT_DATA,
            reserved_1: [0u8; 3],
            report_data_len,
        }
    }

    /// Helper: build a test record with given report_data and quote sizes.
    fn make_test_record(rd_len: usize, q_len: usize) -> (AttestationHeader, Vec<u8>, Vec<u8>) {
        let report_data: Vec<u8> = (0..rd_len).map(|i| (i & 0xFF) as u8).collect();
        let quote: Vec<u8> = (0..q_len).map(|i| ((i + 0x80) & 0xFF) as u8).collect();
        let header = make_test_header(rd_len as u64, q_len as u16);
        (header, report_data, quote)
    }

    /// Helper: build a TeeBoundKeyRecord for testing.
    fn make_test_key_record() -> TeeBoundKeyRecord {
        let mut measurement = [0u8; 32];
        measurement[0] = 0xAA;
        measurement[31] = 0xBB;

        let sealed_key = vec![0x10, 0x20, 0x30, 0x40, 0x50];
        let public_key = b"test-public-key-material";
        let key_id = shake256_128(public_key);

        TeeBoundKeyRecord {
            key_type: KEY_TYPE_TEE_BOUND,
            algorithm: 1,
            sealed_key_length: sealed_key.len() as u16,
            key_id,
            measurement,
            platform: TeePlatform::SevSnp as u8,
            reserved: [0u8; 3],
            valid_from: 1_000_000_000,
            valid_until: 2_000_000_000,
            sealed_key,
        }
    }

    // -----------------------------------------------------------------------
    // 1. header_codec_round_trip
    // -----------------------------------------------------------------------
    #[test]
    fn header_codec_round_trip() {
        let header = make_test_header(64, 256);
        let encoded = encode_attestation_header(&header);
        assert_eq!(encoded.len(), ATTESTATION_HEADER_SIZE);

        let decoded = decode_attestation_header(&encoded).unwrap();
        assert_eq!(decoded.platform, header.platform);
        assert_eq!(decoded.attestation_type, header.attestation_type);
        assert_eq!(decoded.quote_length, header.quote_length);
        assert_eq!(decoded.reserved_0, header.reserved_0);
        assert_eq!(decoded.measurement, header.measurement);
        assert_eq!(decoded.signer_id, header.signer_id);
        assert_eq!(decoded.timestamp_ns, header.timestamp_ns);
        assert_eq!(decoded.nonce, header.nonce);
        assert_eq!(decoded.svn, header.svn);
        assert_eq!(decoded.sig_algo, header.sig_algo);
        assert_eq!(decoded.flags, header.flags);
        assert_eq!(decoded.reserved_1, header.reserved_1);
        assert_eq!(decoded.report_data_len, header.report_data_len);
    }

    // -----------------------------------------------------------------------
    // 2. header_decode_truncated
    // -----------------------------------------------------------------------
    #[test]
    fn header_decode_truncated() {
        let data = [0u8; 111]; // One byte short
        let result = decode_attestation_header(&data);
        assert!(matches!(
            result,
            Err(RvfError::Code(ErrorCode::TruncatedSegment))
        ));
    }

    // -----------------------------------------------------------------------
    // 3. record_codec_round_trip
    // -----------------------------------------------------------------------
    #[test]
    fn record_codec_round_trip() {
        let (header, report_data, quote) = make_test_record(64, 128);
        let encoded = encode_attestation_record(&header, &report_data, &quote);
        assert_eq!(encoded.len(), ATTESTATION_HEADER_SIZE + 64 + 128);

        let (dec_hdr, dec_rd, dec_q) = decode_attestation_record(&encoded).unwrap();
        assert_eq!(dec_hdr.platform, header.platform);
        assert_eq!(dec_hdr.quote_length, header.quote_length);
        assert_eq!(dec_hdr.report_data_len, header.report_data_len);
        assert_eq!(dec_rd, report_data);
        assert_eq!(dec_q, quote);
    }

    // -----------------------------------------------------------------------
    // 4. record_empty_report_data
    // -----------------------------------------------------------------------
    #[test]
    fn record_empty_report_data() {
        let (header, report_data, quote) = make_test_record(0, 32);
        let encoded = encode_attestation_record(&header, &report_data, &quote);

        let (dec_hdr, dec_rd, dec_q) = decode_attestation_record(&encoded).unwrap();
        assert!(dec_rd.is_empty());
        assert_eq!(dec_q, quote);
        assert_eq!(dec_hdr.report_data_len, 0);
        assert_eq!(dec_hdr.quote_length, 32);
    }

    // -----------------------------------------------------------------------
    // 5. record_empty_quote
    // -----------------------------------------------------------------------
    #[test]
    fn record_empty_quote() {
        let (header, report_data, quote) = make_test_record(48, 0);
        let encoded = encode_attestation_record(&header, &report_data, &quote);

        let (dec_hdr, dec_rd, dec_q) = decode_attestation_record(&encoded).unwrap();
        assert_eq!(dec_rd, report_data);
        assert!(dec_q.is_empty());
        assert_eq!(dec_hdr.report_data_len, 48);
        assert_eq!(dec_hdr.quote_length, 0);
    }

    // -----------------------------------------------------------------------
    // 6. witness_entry_hash_binding
    // -----------------------------------------------------------------------
    #[test]
    fn witness_entry_hash_binding() {
        let (header, report_data, quote) = make_test_record(32, 64);
        let record = encode_attestation_record(&header, &report_data, &quote);
        let expected_hash = shake256_256(&record);

        let entry = attestation_witness_entry(
            &record,
            1_000_000_000,
            AttestationWitnessType::PlatformAttestation,
        );
        assert_eq!(entry.action_hash, expected_hash);
        assert_eq!(entry.timestamp_ns, 1_000_000_000);
        assert_eq!(
            entry.witness_type,
            AttestationWitnessType::PlatformAttestation as u8
        );
    }

    // -----------------------------------------------------------------------
    // 7. witness_payload_round_trip
    // -----------------------------------------------------------------------
    #[test]
    fn witness_payload_round_trip() {
        let records: Vec<Vec<u8>> = (0..3)
            .map(|i| {
                let (h, rd, q) = make_test_record(16 + i * 4, 32 + i * 8);
                encode_attestation_record(&h, &rd, &q)
            })
            .collect();

        let timestamps = vec![100, 200, 300];
        let witness_types = vec![
            AttestationWitnessType::PlatformAttestation,
            AttestationWitnessType::KeyBinding,
            AttestationWitnessType::ComputationProof,
        ];

        let payload =
            build_attestation_witness_payload(&records, &timestamps, &witness_types).unwrap();
        let results = verify_attestation_witness_payload(&payload).unwrap();

        assert_eq!(results.len(), 3);
        for (i, (entry, header, rd, q)) in results.iter().enumerate() {
            assert_eq!(entry.timestamp_ns, timestamps[i]);
            assert_eq!(entry.witness_type, witness_types[i] as u8);
            // Re-encode and compare the record bytes.
            let re_encoded = encode_attestation_record(header, rd, q);
            assert_eq!(re_encoded, records[i]);
        }
    }

    // -----------------------------------------------------------------------
    // 8. witness_payload_single_entry
    // -----------------------------------------------------------------------
    #[test]
    fn witness_payload_single_entry() {
        let (h, rd, q) = make_test_record(8, 16);
        let record = encode_attestation_record(&h, &rd, &q);
        let records = vec![record.clone()];
        let timestamps = vec![42];
        let witness_types = vec![AttestationWitnessType::DataProvenance];

        let payload =
            build_attestation_witness_payload(&records, &timestamps, &witness_types).unwrap();
        let results = verify_attestation_witness_payload(&payload).unwrap();

        assert_eq!(results.len(), 1);
        let (entry, header, dec_rd, dec_q) = &results[0];
        assert_eq!(entry.timestamp_ns, 42);
        assert_eq!(
            entry.witness_type,
            AttestationWitnessType::DataProvenance as u8
        );
        assert_eq!(*dec_rd, rd);
        assert_eq!(*dec_q, q);
        assert_eq!(header.platform, h.platform);
    }

    // -----------------------------------------------------------------------
    // 9. witness_payload_tamper_detected
    // -----------------------------------------------------------------------
    #[test]
    fn witness_payload_tamper_detected() {
        let (h, rd, q) = make_test_record(16, 32);
        let record = encode_attestation_record(&h, &rd, &q);
        let records = vec![record];
        let timestamps = vec![999];
        let witness_types = vec![AttestationWitnessType::PlatformAttestation];

        let mut payload =
            build_attestation_witness_payload(&records, &timestamps, &witness_types).unwrap();

        // Flip a byte in the attestation record (after count + offsets + chain).
        let records_offset = 4 + 8 + WITNESS_ENTRY_SIZE;
        if records_offset + 50 < payload.len() {
            payload[records_offset + 50] ^= 0xFF;
        }

        let result = verify_attestation_witness_payload(&payload);
        assert!(matches!(
            result,
            Err(RvfError::Code(ErrorCode::InvalidChecksum))
        ));
    }

    // -----------------------------------------------------------------------
    // 10. tee_key_codec_round_trip
    // -----------------------------------------------------------------------
    #[test]
    fn tee_key_codec_round_trip() {
        let record = make_test_key_record();
        let encoded = encode_tee_bound_key(&record);
        assert_eq!(encoded.len(), TEE_KEY_HEADER_SIZE + record.sealed_key.len());

        let decoded = decode_tee_bound_key(&encoded).unwrap();
        assert_eq!(decoded.key_type, record.key_type);
        assert_eq!(decoded.algorithm, record.algorithm);
        assert_eq!(decoded.sealed_key_length, record.sealed_key_length);
        assert_eq!(decoded.key_id, record.key_id);
        assert_eq!(decoded.measurement, record.measurement);
        assert_eq!(decoded.platform, record.platform);
        assert_eq!(decoded.reserved, record.reserved);
        assert_eq!(decoded.valid_from, record.valid_from);
        assert_eq!(decoded.valid_until, record.valid_until);
        assert_eq!(decoded.sealed_key, record.sealed_key);
    }

    // -----------------------------------------------------------------------
    // 11. tee_key_decode_truncated
    // -----------------------------------------------------------------------
    #[test]
    fn tee_key_decode_truncated() {
        // Header too short.
        let data = [0u8; TEE_KEY_HEADER_SIZE - 1];
        let result = decode_tee_bound_key(&data);
        assert_eq!(result, Err(RvfError::Code(ErrorCode::TruncatedSegment)));

        // Header present but sealed_key truncated.
        let record = make_test_key_record();
        let encoded = encode_tee_bound_key(&record);
        let truncated = &encoded[..TEE_KEY_HEADER_SIZE + 2]; // 2 < sealed_key_length (5)
        let result = decode_tee_bound_key(truncated);
        assert_eq!(result, Err(RvfError::Code(ErrorCode::TruncatedSegment)));
    }

    // -----------------------------------------------------------------------
    // 12. key_binding_valid
    // -----------------------------------------------------------------------
    #[test]
    fn key_binding_valid() {
        let record = make_test_key_record();
        let mut measurement = [0u8; 32];
        measurement[0] = 0xAA;
        measurement[31] = 0xBB;

        let result = verify_key_binding(
            &record,
            TeePlatform::SevSnp,
            &measurement,
            1_500_000_000, // between valid_from and valid_until
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // 13. key_binding_wrong_platform
    // -----------------------------------------------------------------------
    #[test]
    fn key_binding_wrong_platform() {
        let record = make_test_key_record();
        let mut measurement = [0u8; 32];
        measurement[0] = 0xAA;
        measurement[31] = 0xBB;

        let result = verify_key_binding(
            &record,
            TeePlatform::Sgx, // wrong platform
            &measurement,
            1_500_000_000,
        );
        assert_eq!(result, Err(RvfError::Code(ErrorCode::KeyNotBound)));
    }

    // -----------------------------------------------------------------------
    // 14. key_binding_wrong_measurement
    // -----------------------------------------------------------------------
    #[test]
    fn key_binding_wrong_measurement() {
        let record = make_test_key_record();
        let wrong_measurement = [0xFF; 32]; // does not match

        let result = verify_key_binding(
            &record,
            TeePlatform::SevSnp,
            &wrong_measurement,
            1_500_000_000,
        );
        assert_eq!(result, Err(RvfError::Code(ErrorCode::KeyNotBound)));
    }

    // -----------------------------------------------------------------------
    // 15. key_binding_expired
    // -----------------------------------------------------------------------
    #[test]
    fn key_binding_expired() {
        let record = make_test_key_record(); // valid_until = 2_000_000_000
        let mut measurement = [0u8; 32];
        measurement[0] = 0xAA;
        measurement[31] = 0xBB;

        let result = verify_key_binding(
            &record,
            TeePlatform::SevSnp,
            &measurement,
            3_000_000_000, // past valid_until
        );
        assert_eq!(result, Err(RvfError::Code(ErrorCode::KeyExpired)));
    }

    // -----------------------------------------------------------------------
    // 16. key_binding_no_expiry
    // -----------------------------------------------------------------------
    #[test]
    fn key_binding_no_expiry() {
        let mut record = make_test_key_record();
        record.valid_until = 0; // no expiry

        let mut measurement = [0u8; 32];
        measurement[0] = 0xAA;
        measurement[31] = 0xBB;

        let result = verify_key_binding(
            &record,
            TeePlatform::SevSnp,
            &measurement,
            u64::MAX, // far future -- should still pass
        );
        assert!(result.is_ok());
    }
}
