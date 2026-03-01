//! Segment flags bitfield for the RVF format.

/// Bitfield wrapper around the 16-bit segment flags.
///
/// Bits 12-15 are reserved and must be zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct SegmentFlags(u16);

impl SegmentFlags {
    /// Payload is compressed per the `compression` header field.
    pub const COMPRESSED: u16 = 0x0001;
    /// Payload is encrypted (key info in CRYPTO_SEG / manifest).
    pub const ENCRYPTED: u16 = 0x0002;
    /// A signature footer follows the payload.
    pub const SIGNED: u16 = 0x0004;
    /// Segment is immutable (compaction output).
    pub const SEALED: u16 = 0x0008;
    /// Segment is a partial / streaming write.
    pub const PARTIAL: u16 = 0x0010;
    /// Segment logically deletes a prior segment.
    pub const TOMBSTONE: u16 = 0x0020;
    /// Segment contains temperature-promoted (hot) data.
    pub const HOT: u16 = 0x0040;
    /// Segment contains overlay / delta data.
    pub const OVERLAY: u16 = 0x0080;
    /// Segment contains a full snapshot (not delta).
    pub const SNAPSHOT: u16 = 0x0100;
    /// Segment is a safe rollback point.
    pub const CHECKPOINT: u16 = 0x0200;
    /// Segment was produced inside an attested TEE environment.
    pub const ATTESTED: u16 = 0x0400;
    /// File carries DNA-style lineage provenance metadata.
    pub const HAS_LINEAGE: u16 = 0x0800;

    /// Mask for all defined flag bits.
    const KNOWN_MASK: u16 = 0x0FFF;

    /// Create an empty flags value (no flags set).
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create flags from a raw `u16`. Reserved bits are masked off.
    #[inline]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw & Self::KNOWN_MASK)
    }

    /// Return the raw `u16` representation.
    #[inline]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Check whether a specific flag bit is set.
    #[inline]
    pub const fn contains(self, flag: u16) -> bool {
        self.0 & flag == flag
    }

    /// Set a flag bit.
    #[inline]
    pub const fn with(self, flag: u16) -> Self {
        Self(self.0 | (flag & Self::KNOWN_MASK))
    }

    /// Clear a flag bit.
    #[inline]
    pub const fn without(self, flag: u16) -> Self {
        Self(self.0 & !flag)
    }

    /// Returns true if no flags are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_flags() {
        let f = SegmentFlags::empty();
        assert!(f.is_empty());
        assert_eq!(f.bits(), 0);
    }

    #[test]
    fn set_and_check_flags() {
        let f = SegmentFlags::empty()
            .with(SegmentFlags::COMPRESSED)
            .with(SegmentFlags::SEALED);
        assert!(f.contains(SegmentFlags::COMPRESSED));
        assert!(f.contains(SegmentFlags::SEALED));
        assert!(!f.contains(SegmentFlags::ENCRYPTED));
    }

    #[test]
    fn clear_flag() {
        let f = SegmentFlags::empty()
            .with(SegmentFlags::COMPRESSED)
            .with(SegmentFlags::SIGNED)
            .without(SegmentFlags::COMPRESSED);
        assert!(!f.contains(SegmentFlags::COMPRESSED));
        assert!(f.contains(SegmentFlags::SIGNED));
    }

    #[test]
    fn reserved_bits_masked() {
        let f = SegmentFlags::from_raw(0xFFFF);
        assert_eq!(f.bits(), 0x0FFF);
    }

    #[test]
    fn all_known_flags() {
        let all = SegmentFlags::empty()
            .with(SegmentFlags::COMPRESSED)
            .with(SegmentFlags::ENCRYPTED)
            .with(SegmentFlags::SIGNED)
            .with(SegmentFlags::SEALED)
            .with(SegmentFlags::PARTIAL)
            .with(SegmentFlags::TOMBSTONE)
            .with(SegmentFlags::HOT)
            .with(SegmentFlags::OVERLAY)
            .with(SegmentFlags::SNAPSHOT)
            .with(SegmentFlags::CHECKPOINT)
            .with(SegmentFlags::ATTESTED)
            .with(SegmentFlags::HAS_LINEAGE);
        assert_eq!(all.bits(), 0x0FFF);
    }
}
