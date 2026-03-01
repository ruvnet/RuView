//! Kernel binding types for the RVF computational container.
//!
//! Defines the 128-byte `KernelBinding` struct per ADR-031 (revised).
//! A `KernelBinding` cryptographically ties a manifest root to a
//! policy hash with a version stamp, ensuring tamper-evident linkage.
//!
//! Padded to 128 bytes to avoid future wire-format breaks. Active fields
//! occupy 76 bytes; the remaining 52 bytes are reserved/padding (must be zero).

/// 128-byte kernel binding record (padded for future evolution).
///
/// Layout:
/// | Offset | Size | Field                |
/// |--------|------|----------------------|
/// | 0x00   | 32   | manifest_root_hash   |
/// | 0x20   | 32   | policy_hash          |
/// | 0x40   | 2    | binding_version      |
/// | 0x42   | 2    | min_runtime_version  |
/// | 0x44   | 4    | _pad0 (alignment)    |
/// | 0x48   | 8    | allowed_segment_mask |
/// | 0x50   | 48   | _reserved            |
///
/// All multi-byte fields are little-endian on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct KernelBinding {
    /// SHAKE-256-256 of the manifest root node.
    pub manifest_root_hash: [u8; 32],
    /// SHAKE-256-256 of the policy document.
    pub policy_hash: [u8; 32],
    /// Binding format version (currently 1).
    pub binding_version: u16,
    /// Minimum runtime version required (0 = any).
    pub min_runtime_version: u16,
    /// Alignment padding (must be zero).
    pub _pad0: u32,
    /// Bitmask of allowed segment types (0 = no restriction).
    pub allowed_segment_mask: u64,
    /// Reserved for future use (must be zero).
    pub _reserved: [u8; 48],
}

// Compile-time assertion: KernelBinding must be exactly 128 bytes.
const _: () = assert!(core::mem::size_of::<KernelBinding>() == 128);

impl KernelBinding {
    /// Serialize the binding to a 128-byte array.
    pub fn to_bytes(&self) -> [u8; 128] {
        let mut buf = [0u8; 128];
        buf[0x00..0x20].copy_from_slice(&self.manifest_root_hash);
        buf[0x20..0x40].copy_from_slice(&self.policy_hash);
        buf[0x40..0x42].copy_from_slice(&self.binding_version.to_le_bytes());
        buf[0x42..0x44].copy_from_slice(&self.min_runtime_version.to_le_bytes());
        buf[0x44..0x48].copy_from_slice(&self._pad0.to_le_bytes());
        buf[0x48..0x50].copy_from_slice(&self.allowed_segment_mask.to_le_bytes());
        buf[0x50..0x80].copy_from_slice(&self._reserved);
        buf
    }

    /// Deserialize a `KernelBinding` from a 128-byte slice (unchecked).
    ///
    /// Does NOT validate reserved fields. Use `from_bytes_validated` for
    /// security-critical paths that must reject non-zero padding/reserved.
    pub fn from_bytes(data: &[u8; 128]) -> Self {
        Self {
            manifest_root_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x00..0x20]);
                h
            },
            policy_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x20..0x40]);
                h
            },
            binding_version: u16::from_le_bytes([data[0x40], data[0x41]]),
            min_runtime_version: u16::from_le_bytes([data[0x42], data[0x43]]),
            _pad0: u32::from_le_bytes([data[0x44], data[0x45], data[0x46], data[0x47]]),
            allowed_segment_mask: u64::from_le_bytes([
                data[0x48], data[0x49], data[0x4A], data[0x4B], data[0x4C], data[0x4D], data[0x4E],
                data[0x4F],
            ]),
            _reserved: {
                let mut r = [0u8; 48];
                r.copy_from_slice(&data[0x50..0x80]);
                r
            },
        }
    }

    /// Deserialize and validate a `KernelBinding` from a 128-byte slice.
    ///
    /// Rejects bindings where:
    /// - `binding_version` is 0 (uninitialized)
    /// - `_pad0` is non-zero (spec violation)
    /// - `_reserved` contains non-zero bytes (spec violation / data smuggling)
    pub fn from_bytes_validated(data: &[u8; 128]) -> Result<Self, &'static str> {
        let binding = Self::from_bytes(data);
        if binding.binding_version == 0 {
            return Err("binding_version must be > 0");
        }
        if binding._pad0 != 0 {
            return Err("_pad0 must be zero");
        }
        if binding._reserved.iter().any(|&b| b != 0) {
            return Err("_reserved must be all zeros");
        }
        Ok(binding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_binding() -> KernelBinding {
        KernelBinding {
            manifest_root_hash: [0xAA; 32],
            policy_hash: [0xBB; 32],
            binding_version: 1,
            min_runtime_version: 0,
            _pad0: 0,
            allowed_segment_mask: 0,
            _reserved: [0; 48],
        }
    }

    #[test]
    fn binding_size_is_128() {
        assert_eq!(core::mem::size_of::<KernelBinding>(), 128);
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_binding();
        let bytes = original.to_bytes();
        let decoded = KernelBinding::from_bytes(&bytes);

        assert_eq!(decoded.manifest_root_hash, [0xAA; 32]);
        assert_eq!(decoded.policy_hash, [0xBB; 32]);
        assert_eq!(decoded.binding_version, 1);
        assert_eq!(decoded.min_runtime_version, 0);
        assert_eq!(decoded._pad0, 0);
        assert_eq!(decoded.allowed_segment_mask, 0);
        assert_eq!(decoded._reserved, [0; 48]);
    }

    #[test]
    fn round_trip_with_fields() {
        let binding = KernelBinding {
            manifest_root_hash: [0x11; 32],
            policy_hash: [0x22; 32],
            binding_version: 2,
            min_runtime_version: 3,
            _pad0: 0,
            allowed_segment_mask: 0x00FF_FFFF,
            _reserved: [0; 48],
        };
        let bytes = binding.to_bytes();
        let decoded = KernelBinding::from_bytes(&bytes);
        assert_eq!(decoded.binding_version, 2);
        assert_eq!(decoded.min_runtime_version, 3);
        assert_eq!(decoded.allowed_segment_mask, 0x00FF_FFFF);
    }

    #[test]
    fn field_offsets() {
        let b = sample_binding();
        let base = &b as *const _ as usize;

        assert_eq!(&b.manifest_root_hash as *const _ as usize - base, 0x00);
        assert_eq!(&b.policy_hash as *const _ as usize - base, 0x20);
        assert_eq!(&b.binding_version as *const _ as usize - base, 0x40);
        assert_eq!(&b.min_runtime_version as *const _ as usize - base, 0x42);
        assert_eq!(&b._pad0 as *const _ as usize - base, 0x44);
        assert_eq!(&b.allowed_segment_mask as *const _ as usize - base, 0x48);
        assert_eq!(&b._reserved as *const _ as usize - base, 0x50);
    }

    #[test]
    fn reserved_must_be_zero_in_new_bindings() {
        let b = sample_binding();
        assert!(b._reserved.iter().all(|&x| x == 0));
        assert_eq!(b._pad0, 0);
    }
}
