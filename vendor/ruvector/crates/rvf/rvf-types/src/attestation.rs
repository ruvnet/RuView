//! Attestation types for Confidential Computing integration.
//!
//! These types describe hardware TEE platforms, attestation metadata,
//! and the wire format for attestation records stored in WITNESS_SEG
//! and CRYPTO_SEG payloads.

/// Hardware TEE platform identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum TeePlatform {
    /// Intel SGX.
    Sgx = 0,
    /// AMD SEV-SNP.
    SevSnp = 1,
    /// Intel TDX.
    Tdx = 2,
    /// ARM CCA.
    ArmCca = 3,
    /// Software-emulated (testing only).
    SoftwareTee = 0xFE,
}

impl TryFrom<u8> for TeePlatform {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Sgx),
            1 => Ok(Self::SevSnp),
            2 => Ok(Self::Tdx),
            3 => Ok(Self::ArmCca),
            0xFE => Ok(Self::SoftwareTee),
            other => Err(other),
        }
    }
}

/// Attestation witness type discriminant.
///
/// These extend the existing witness_type values (0x01=PROVENANCE,
/// 0x02=COMPUTATION used by claude-flow adapter).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum AttestationWitnessType {
    /// TEE identity and measurement.
    PlatformAttestation = 0x05,
    /// Encryption key bound to TEE measurement.
    KeyBinding = 0x06,
    /// Operations performed inside the TEE.
    ComputationProof = 0x07,
    /// Chain of custody from model to TEE to RVF.
    DataProvenance = 0x08,
}

impl TryFrom<u8> for AttestationWitnessType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x05 => Ok(Self::PlatformAttestation),
            0x06 => Ok(Self::KeyBinding),
            0x07 => Ok(Self::ComputationProof),
            0x08 => Ok(Self::DataProvenance),
            other => Err(other),
        }
    }
}

/// Key type for keys bound to a TEE measurement.
pub const KEY_TYPE_TEE_BOUND: u8 = 4;

/// Wire-format attestation header (exactly 112 bytes, `repr(C)`).
///
/// ```text
/// Offset  Type        Field
/// 0x00    u8          platform (TeePlatform discriminant)
/// 0x01    u8          attestation_type (AttestationWitnessType discriminant)
/// 0x02    u16         quote_length (LE, length of opaque quote blob)
/// 0x04    u32         reserved_0 (must be zero)
/// 0x08    [u8; 32]    measurement (MRENCLAVE / launch digest)
/// 0x28    [u8; 32]    signer_id (MRSIGNER / author key hash)
/// 0x48    u64         timestamp_ns (LE, when attestation was captured)
/// 0x50    [u8; 16]    nonce (anti-replay nonce)
/// 0x60    u16         svn (LE, security version number)
/// 0x62    u16         sig_algo (LE, SignatureAlgo of the quote)
/// 0x64    u8          flags (attestation flags)
/// 0x65    [u8; 3]     reserved_1 (must be zero)
/// 0x68    u64         report_data_len (LE, length of custom report data)
/// ```
///
/// Total: 112 bytes (0x70).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct AttestationHeader {
    /// TEE platform discriminant (see [`TeePlatform`]).
    pub platform: u8,
    /// Attestation witness type discriminant (see [`AttestationWitnessType`]).
    pub attestation_type: u8,
    /// Length of the opaque quote blob (little-endian).
    pub quote_length: u16,
    /// Reserved, must be zero.
    pub reserved_0: u32,
    /// MRENCLAVE / launch digest.
    pub measurement: [u8; 32],
    /// MRSIGNER / author key hash.
    pub signer_id: [u8; 32],
    /// Timestamp in nanoseconds when attestation was captured (little-endian).
    pub timestamp_ns: u64,
    /// Anti-replay nonce.
    pub nonce: [u8; 16],
    /// Security version number (little-endian).
    pub svn: u16,
    /// Signature algorithm of the quote (little-endian).
    pub sig_algo: u16,
    /// Attestation flags.
    pub flags: u8,
    /// Reserved, must be zero.
    pub reserved_1: [u8; 3],
    /// Length of custom report data (little-endian).
    pub report_data_len: u64,
}

// Compile-time size assertion.
const _: () = assert!(core::mem::size_of::<AttestationHeader>() == 112);

impl AttestationHeader {
    /// TEE is in debug mode.
    pub const FLAG_DEBUGGABLE: u8 = 0x01;
    /// Custom report data present.
    pub const FLAG_HAS_REPORT_DATA: u8 = 0x02;
    /// Combined from multiple TEEs.
    pub const FLAG_MULTI_PLATFORM: u8 = 0x04;

    /// Create a new attestation header with all fields zeroed except
    /// `platform` and `attestation_type`.
    pub const fn new(platform: u8, attestation_type: u8) -> Self {
        Self {
            platform,
            attestation_type,
            quote_length: 0,
            reserved_0: 0,
            measurement: [0u8; 32],
            signer_id: [0u8; 32],
            timestamp_ns: 0,
            nonce: [0u8; 16],
            svn: 0,
            sig_algo: 0,
            flags: 0,
            reserved_1: [0u8; 3],
            report_data_len: 0,
        }
    }

    /// Returns `true` if the TEE is in debug mode.
    pub const fn is_debuggable(&self) -> bool {
        self.flags & Self::FLAG_DEBUGGABLE != 0
    }

    /// Returns `true` if custom report data is present.
    pub const fn has_report_data(&self) -> bool {
        self.flags & Self::FLAG_HAS_REPORT_DATA != 0
    }

    /// Returns the total record length:
    /// 112 (header) + report_data_len + quote_length.
    pub const fn total_record_length(&self) -> u64 {
        112 + self.report_data_len + self.quote_length as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tee_platform_round_trip() {
        let variants: &[(u8, TeePlatform)] = &[
            (0, TeePlatform::Sgx),
            (1, TeePlatform::SevSnp),
            (2, TeePlatform::Tdx),
            (3, TeePlatform::ArmCca),
            (0xFE, TeePlatform::SoftwareTee),
        ];
        for &(raw, expected) in variants {
            let parsed = TeePlatform::try_from(raw).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed as u8, raw);
        }
    }

    #[test]
    fn tee_platform_invalid() {
        assert_eq!(TeePlatform::try_from(4), Err(4));
        assert_eq!(TeePlatform::try_from(0xFF), Err(0xFF));
        assert_eq!(TeePlatform::try_from(0x80), Err(0x80));
    }

    #[test]
    fn attestation_witness_type_round_trip() {
        let variants: &[(u8, AttestationWitnessType)] = &[
            (0x05, AttestationWitnessType::PlatformAttestation),
            (0x06, AttestationWitnessType::KeyBinding),
            (0x07, AttestationWitnessType::ComputationProof),
            (0x08, AttestationWitnessType::DataProvenance),
        ];
        for &(raw, expected) in variants {
            let parsed = AttestationWitnessType::try_from(raw).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed as u8, raw);
        }
    }

    #[test]
    fn attestation_witness_type_invalid() {
        assert_eq!(AttestationWitnessType::try_from(0x00), Err(0x00));
        assert_eq!(AttestationWitnessType::try_from(0x04), Err(0x04));
        assert_eq!(AttestationWitnessType::try_from(0x09), Err(0x09));
        assert_eq!(AttestationWitnessType::try_from(0xFF), Err(0xFF));
    }

    #[test]
    fn attestation_header_size() {
        assert_eq!(core::mem::size_of::<AttestationHeader>(), 112);
    }

    #[test]
    fn attestation_header_new() {
        let hdr = AttestationHeader::new(
            TeePlatform::SevSnp as u8,
            AttestationWitnessType::PlatformAttestation as u8,
        );
        assert_eq!(hdr.platform, 1);
        assert_eq!(hdr.attestation_type, 0x05);
        assert_eq!(hdr.quote_length, 0);
        assert_eq!(hdr.reserved_0, 0);
        assert_eq!(hdr.measurement, [0u8; 32]);
        assert_eq!(hdr.signer_id, [0u8; 32]);
        assert_eq!(hdr.timestamp_ns, 0);
        assert_eq!(hdr.nonce, [0u8; 16]);
        assert_eq!(hdr.svn, 0);
        assert_eq!(hdr.sig_algo, 0);
        assert_eq!(hdr.flags, 0);
        assert_eq!(hdr.reserved_1, [0u8; 3]);
        assert_eq!(hdr.report_data_len, 0);
    }

    #[test]
    fn flag_is_debuggable() {
        let mut hdr = AttestationHeader::new(0, 0);
        assert!(!hdr.is_debuggable());
        hdr.flags = AttestationHeader::FLAG_DEBUGGABLE;
        assert!(hdr.is_debuggable());
        // combined flags
        hdr.flags = AttestationHeader::FLAG_DEBUGGABLE | AttestationHeader::FLAG_HAS_REPORT_DATA;
        assert!(hdr.is_debuggable());
    }

    #[test]
    fn flag_has_report_data() {
        let mut hdr = AttestationHeader::new(0, 0);
        assert!(!hdr.has_report_data());
        hdr.flags = AttestationHeader::FLAG_HAS_REPORT_DATA;
        assert!(hdr.has_report_data());
    }

    #[test]
    fn total_record_length() {
        let mut hdr = AttestationHeader::new(0, 0);
        assert_eq!(hdr.total_record_length(), 112);

        hdr.quote_length = 256;
        assert_eq!(hdr.total_record_length(), 112 + 256);

        hdr.report_data_len = 64;
        assert_eq!(hdr.total_record_length(), 112 + 64 + 256);
    }

    #[test]
    fn key_type_tee_bound_value() {
        assert_eq!(KEY_TYPE_TEE_BOUND, 4);
    }
}
