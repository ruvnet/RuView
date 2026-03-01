//! Security policy and error types for ADR-033 mandatory manifest signatures.
//!
//! Defines the `SecurityPolicy` mount policy (default: Strict) and
//! structured `SecurityError` diagnostics for deterministic failure reasons.

/// Manifest signature verification policy.
///
/// Controls how the runtime handles unsigned or invalid signatures
/// when opening an RVF file. Default is `Strict` — no signature means
/// no mount in production.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SecurityPolicy {
    /// No signature verification. For development and testing only.
    Permissive = 0x00,
    /// Warn on missing or invalid signatures, but allow open.
    WarnOnly = 0x01,
    /// Require valid signature on Level 0 manifest.
    /// DEFAULT for production.
    Strict = 0x02,
    /// Require valid signatures on Level 0, Level 1, and all
    /// hotset-referenced segments. Full chain verification.
    Paranoid = 0x03,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::Strict
    }
}

impl SecurityPolicy {
    /// Returns true if signature verification is required at mount time.
    pub const fn requires_signature(&self) -> bool {
        matches!(*self, Self::Strict | Self::Paranoid)
    }

    /// Returns true if content hash verification is performed on hotset access.
    pub const fn verifies_content_hashes(&self) -> bool {
        matches!(*self, Self::WarnOnly | Self::Strict | Self::Paranoid)
    }

    /// Returns true if Level 1 manifest is also signature-verified.
    pub const fn verifies_level1(&self) -> bool {
        matches!(*self, Self::Paranoid)
    }
}

/// Structured security error with deterministic, stable error codes.
///
/// Every variant includes enough context for logging and diagnostics
/// without exposing internal state that could aid an attacker.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SecurityError {
    /// Level 0 manifest has no signature (sig_algo = 0).
    UnsignedManifest {
        /// Byte offset of the rejected manifest.
        manifest_offset: u64,
    },

    /// Signature is present but cryptographically invalid.
    InvalidSignature {
        /// Byte offset of the rejected manifest.
        manifest_offset: u64,
        /// Phase where rejection occurred.
        rejection_phase: &'static str,
    },

    /// Signature is valid but from an unknown/untrusted signer.
    UnknownSigner {
        /// Byte offset of the rejected manifest.
        manifest_offset: u64,
        /// Fingerprint of the actual signer (first 16 bytes of public key hash).
        actual_signer: [u8; 16],
        /// Fingerprint of the expected signer from trust store (if known).
        expected_signer: Option<[u8; 16]>,
    },

    /// Content hash of a hotset-referenced segment does not match.
    ContentHashMismatch {
        /// Name of the pointer that failed (e.g., "centroid_seg_offset").
        pointer_name: &'static str,
        /// Content hash stored in Level 0.
        expected_hash: [u8; 16],
        /// Actual hash of the segment at the pointed offset.
        actual_hash: [u8; 16],
        /// Byte offset that was followed.
        seg_offset: u64,
    },

    /// Centroid epoch drift exceeds maximum allowed.
    EpochDriftExceeded {
        /// Current epoch drift value.
        epoch_drift: u32,
        /// Maximum allowed drift.
        max_epoch_drift: u32,
    },

    /// Level 1 manifest signature invalid (Paranoid mode only).
    Level1InvalidSignature {
        /// Byte offset of the Level 1 manifest.
        manifest_offset: u64,
    },
}

impl core::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsignedManifest { manifest_offset } => {
                write!(f, "unsigned manifest at offset 0x{manifest_offset:X}")
            }
            Self::InvalidSignature {
                manifest_offset,
                rejection_phase,
            } => {
                write!(
                    f,
                    "invalid signature at offset 0x{manifest_offset:X} \
                     (phase: {rejection_phase})"
                )
            }
            Self::UnknownSigner {
                manifest_offset, ..
            } => {
                write!(f, "unknown signer at offset 0x{manifest_offset:X}")
            }
            Self::ContentHashMismatch {
                pointer_name,
                seg_offset,
                ..
            } => {
                write!(
                    f,
                    "content hash mismatch for {pointer_name} \
                     at offset 0x{seg_offset:X}"
                )
            }
            Self::EpochDriftExceeded {
                epoch_drift,
                max_epoch_drift,
            } => {
                write!(
                    f,
                    "centroid epoch drift {epoch_drift} exceeds max {max_epoch_drift}"
                )
            }
            Self::Level1InvalidSignature { manifest_offset } => {
                write!(
                    f,
                    "Level 1 manifest invalid signature at offset 0x{manifest_offset:X}"
                )
            }
        }
    }
}

/// Content hash fields stored in the Level 0 reserved area (ADR-033 §1).
///
/// 96 bytes total: 5 content hashes (16 bytes each) + centroid_epoch (4) +
/// max_epoch_drift (4) + reserved (8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct HardeningFields {
    /// SHAKE-256 truncated to 128 bits of the entrypoint segment payload.
    pub entrypoint_content_hash: [u8; 16],
    /// SHAKE-256 truncated to 128 bits of the toplayer segment payload.
    pub toplayer_content_hash: [u8; 16],
    /// SHAKE-256 truncated to 128 bits of the centroid segment payload.
    pub centroid_content_hash: [u8; 16],
    /// SHAKE-256 truncated to 128 bits of the quantdict segment payload.
    pub quantdict_content_hash: [u8; 16],
    /// SHAKE-256 truncated to 128 bits of the hot_cache segment payload.
    pub hot_cache_content_hash: [u8; 16],
    /// Monotonic counter incremented on centroid recomputation.
    pub centroid_epoch: u32,
    /// Maximum allowed drift before forced recompute.
    pub max_epoch_drift: u32,
    /// Reserved for future hardening fields.
    pub reserved: [u8; 8],
}

const _: () = assert!(core::mem::size_of::<HardeningFields>() == 96);

impl HardeningFields {
    /// Offset within the Level 0 reserved area (0xF00 + 109 = 0xF6D).
    /// Starts after FileIdentity (68 bytes), COW pointers (24 bytes),
    /// and double-root mechanism (17 bytes).
    pub const RESERVED_OFFSET: usize = 109;

    /// Create zeroed hardening fields.
    pub const fn zeroed() -> Self {
        Self {
            entrypoint_content_hash: [0u8; 16],
            toplayer_content_hash: [0u8; 16],
            centroid_content_hash: [0u8; 16],
            quantdict_content_hash: [0u8; 16],
            hot_cache_content_hash: [0u8; 16],
            centroid_epoch: 0,
            max_epoch_drift: 64,
            reserved: [0u8; 8],
        }
    }

    /// Serialize to 96 bytes (little-endian).
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut buf = [0u8; 96];
        buf[0..16].copy_from_slice(&self.entrypoint_content_hash);
        buf[16..32].copy_from_slice(&self.toplayer_content_hash);
        buf[32..48].copy_from_slice(&self.centroid_content_hash);
        buf[48..64].copy_from_slice(&self.quantdict_content_hash);
        buf[64..80].copy_from_slice(&self.hot_cache_content_hash);
        buf[80..84].copy_from_slice(&self.centroid_epoch.to_le_bytes());
        buf[84..88].copy_from_slice(&self.max_epoch_drift.to_le_bytes());
        buf[88..96].copy_from_slice(&self.reserved);
        buf
    }

    /// Deserialize from 96 bytes (little-endian).
    pub fn from_bytes(buf: &[u8; 96]) -> Self {
        let mut entrypoint_content_hash = [0u8; 16];
        let mut toplayer_content_hash = [0u8; 16];
        let mut centroid_content_hash = [0u8; 16];
        let mut quantdict_content_hash = [0u8; 16];
        let mut hot_cache_content_hash = [0u8; 16];
        let mut reserved = [0u8; 8];

        entrypoint_content_hash.copy_from_slice(&buf[0..16]);
        toplayer_content_hash.copy_from_slice(&buf[16..32]);
        centroid_content_hash.copy_from_slice(&buf[32..48]);
        quantdict_content_hash.copy_from_slice(&buf[48..64]);
        hot_cache_content_hash.copy_from_slice(&buf[64..80]);

        let centroid_epoch = u32::from_le_bytes([buf[80], buf[81], buf[82], buf[83]]);
        let max_epoch_drift = u32::from_le_bytes([buf[84], buf[85], buf[86], buf[87]]);
        reserved.copy_from_slice(&buf[88..96]);

        Self {
            entrypoint_content_hash,
            toplayer_content_hash,
            centroid_content_hash,
            quantdict_content_hash,
            hot_cache_content_hash,
            centroid_epoch,
            max_epoch_drift,
            reserved,
        }
    }

    /// Check if all content hashes are zero (no hardening data stored).
    pub fn is_empty(&self) -> bool {
        self.entrypoint_content_hash == [0u8; 16]
            && self.toplayer_content_hash == [0u8; 16]
            && self.centroid_content_hash == [0u8; 16]
            && self.quantdict_content_hash == [0u8; 16]
            && self.hot_cache_content_hash == [0u8; 16]
            && self.centroid_epoch == 0
    }

    /// Get the content hash for a named pointer.
    pub fn hash_for_pointer(&self, pointer_name: &str) -> Option<&[u8; 16]> {
        match pointer_name {
            "entrypoint" => Some(&self.entrypoint_content_hash),
            "toplayer" => Some(&self.toplayer_content_hash),
            "centroid" => Some(&self.centroid_content_hash),
            "quantdict" => Some(&self.quantdict_content_hash),
            "hot_cache" => Some(&self.hot_cache_content_hash),
            _ => None,
        }
    }

    /// Compute epoch drift relative to the manifest's global epoch.
    pub fn epoch_drift(&self, manifest_epoch: u32) -> u32 {
        manifest_epoch.saturating_sub(self.centroid_epoch)
    }

    /// Check if epoch drift exceeds the maximum allowed.
    pub fn is_epoch_drift_exceeded(&self, manifest_epoch: u32) -> bool {
        self.epoch_drift(manifest_epoch) > self.max_epoch_drift
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_policy_default_is_strict() {
        assert_eq!(SecurityPolicy::default(), SecurityPolicy::Strict);
    }

    #[test]
    fn security_policy_signature_required() {
        assert!(!SecurityPolicy::Permissive.requires_signature());
        assert!(!SecurityPolicy::WarnOnly.requires_signature());
        assert!(SecurityPolicy::Strict.requires_signature());
        assert!(SecurityPolicy::Paranoid.requires_signature());
    }

    #[test]
    fn security_policy_content_hashes() {
        assert!(!SecurityPolicy::Permissive.verifies_content_hashes());
        assert!(SecurityPolicy::WarnOnly.verifies_content_hashes());
        assert!(SecurityPolicy::Strict.verifies_content_hashes());
        assert!(SecurityPolicy::Paranoid.verifies_content_hashes());
    }

    #[test]
    fn security_policy_level1() {
        assert!(!SecurityPolicy::Strict.verifies_level1());
        assert!(SecurityPolicy::Paranoid.verifies_level1());
    }

    #[test]
    fn security_policy_repr() {
        assert_eq!(SecurityPolicy::Permissive as u8, 0x00);
        assert_eq!(SecurityPolicy::WarnOnly as u8, 0x01);
        assert_eq!(SecurityPolicy::Strict as u8, 0x02);
        assert_eq!(SecurityPolicy::Paranoid as u8, 0x03);
    }

    #[test]
    fn hardening_fields_size() {
        assert_eq!(core::mem::size_of::<HardeningFields>(), 96);
    }

    #[test]
    fn hardening_fields_round_trip() {
        let fields = HardeningFields {
            entrypoint_content_hash: [1u8; 16],
            toplayer_content_hash: [2u8; 16],
            centroid_content_hash: [3u8; 16],
            quantdict_content_hash: [4u8; 16],
            hot_cache_content_hash: [5u8; 16],
            centroid_epoch: 42,
            max_epoch_drift: 64,
            reserved: [0u8; 8],
        };
        let bytes = fields.to_bytes();
        let decoded = HardeningFields::from_bytes(&bytes);
        assert_eq!(fields, decoded);
    }

    #[test]
    fn hardening_fields_zeroed() {
        let fields = HardeningFields::zeroed();
        assert!(fields.is_empty());
        assert_eq!(fields.max_epoch_drift, 64);
    }

    #[test]
    fn hardening_fields_hash_for_pointer() {
        let mut fields = HardeningFields::zeroed();
        fields.centroid_content_hash = [0xAB; 16];
        assert_eq!(fields.hash_for_pointer("centroid"), Some(&[0xAB; 16]));
        assert_eq!(fields.hash_for_pointer("unknown"), None);
    }

    #[test]
    fn hardening_fields_epoch_drift() {
        let fields = HardeningFields {
            centroid_epoch: 10,
            max_epoch_drift: 64,
            ..HardeningFields::zeroed()
        };
        assert_eq!(fields.epoch_drift(50), 40);
        assert!(!fields.is_epoch_drift_exceeded(50));
        assert!(fields.is_epoch_drift_exceeded(100));
    }

    #[test]
    fn security_error_display() {
        let err = SecurityError::UnsignedManifest {
            manifest_offset: 0x1000,
        };
        let s = alloc::format!("{err}");
        assert!(s.contains("unsigned manifest"));

        let err = SecurityError::ContentHashMismatch {
            pointer_name: "centroid",
            expected_hash: [0xAA; 16],
            actual_hash: [0xBB; 16],
            seg_offset: 0x2000,
        };
        let s = alloc::format!("{err}");
        assert!(s.contains("centroid"));
        assert!(s.contains("2000"));
    }

    #[test]
    fn security_error_unknown_signer() {
        let err = SecurityError::UnknownSigner {
            manifest_offset: 0x3000,
            actual_signer: [0x11; 16],
            expected_signer: Some([0x22; 16]),
        };
        let s = alloc::format!("{err}");
        assert!(s.contains("unknown signer"));
    }

    #[test]
    fn reserved_offset_fits() {
        // 109 + 96 = 205 <= 252 (reserved area size)
        assert!(HardeningFields::RESERVED_OFFSET + 96 <= 252);
    }
}
