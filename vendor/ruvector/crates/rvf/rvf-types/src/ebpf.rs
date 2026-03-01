//! EBPF_SEG (0x0F) types for the RVF computational container.
//!
//! Defines the 64-byte `EbpfHeader` and associated enums per ADR-030.
//! The EBPF_SEG embeds an eBPF program for kernel-level fast-path
//! vector distance computation (L0 cache in BPF maps).

use crate::error::RvfError;

/// Magic number for `EbpfHeader`: "RVBP" in big-endian.
pub const EBPF_MAGIC: u32 = 0x5256_4250;

/// eBPF program type classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum EbpfProgramType {
    /// XDP program for distance computation on packets.
    XdpDistance = 0x00,
    /// TC classifier for query routing.
    TcFilter = 0x01,
    /// Socket filter for query preprocessing.
    SocketFilter = 0x02,
    /// Tracepoint for performance monitoring.
    Tracepoint = 0x03,
    /// Kprobe for dynamic instrumentation.
    Kprobe = 0x04,
    /// Cgroup socket buffer filter.
    CgroupSkb = 0x05,
    /// Custom program type.
    Custom = 0xFF,
}

impl TryFrom<u8> for EbpfProgramType {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::XdpDistance),
            0x01 => Ok(Self::TcFilter),
            0x02 => Ok(Self::SocketFilter),
            0x03 => Ok(Self::Tracepoint),
            0x04 => Ok(Self::Kprobe),
            0x05 => Ok(Self::CgroupSkb),
            0xFF => Ok(Self::Custom),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "EbpfProgramType",
                value: value as u64,
            }),
        }
    }
}

/// eBPF attach point classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum EbpfAttachType {
    /// XDP hook on NIC ingress.
    XdpIngress = 0x00,
    /// TC ingress qdisc.
    TcIngress = 0x01,
    /// TC egress qdisc.
    TcEgress = 0x02,
    /// Socket filter attachment.
    SocketFilter = 0x03,
    /// Cgroup ingress.
    CgroupIngress = 0x04,
    /// Cgroup egress.
    CgroupEgress = 0x05,
    /// No automatic attachment.
    None = 0xFF,
}

impl TryFrom<u8> for EbpfAttachType {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::XdpIngress),
            0x01 => Ok(Self::TcIngress),
            0x02 => Ok(Self::TcEgress),
            0x03 => Ok(Self::SocketFilter),
            0x04 => Ok(Self::CgroupIngress),
            0x05 => Ok(Self::CgroupEgress),
            0xFF => Ok(Self::None),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "EbpfAttachType",
                value: value as u64,
            }),
        }
    }
}

/// 64-byte header for EBPF_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EbpfHeader {
    /// Magic: `EBPF_MAGIC` (0x52564250, "RVBP").
    pub ebpf_magic: u32,
    /// EbpfHeader format version (currently 1).
    pub header_version: u16,
    /// eBPF program type (see `EbpfProgramType`).
    pub program_type: u8,
    /// eBPF attach point (see `EbpfAttachType`).
    pub attach_type: u8,
    /// Bitfield flags for the eBPF program.
    pub program_flags: u32,
    /// Number of BPF instructions (max 65535).
    pub insn_count: u16,
    /// Maximum vector dimension this program handles.
    pub max_dimension: u16,
    /// ELF object size (bytes).
    pub program_size: u64,
    /// Number of BPF maps defined.
    pub map_count: u32,
    /// BTF (BPF Type Format) section size.
    pub btf_size: u32,
    /// SHAKE-256-256 of the ELF object.
    pub program_hash: [u8; 32],
}

// Compile-time assertion: EbpfHeader must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<EbpfHeader>() == 64);

impl EbpfHeader {
    /// Serialize the header to a 64-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0x00..0x04].copy_from_slice(&self.ebpf_magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.header_version.to_le_bytes());
        buf[0x06] = self.program_type;
        buf[0x07] = self.attach_type;
        buf[0x08..0x0C].copy_from_slice(&self.program_flags.to_le_bytes());
        buf[0x0C..0x0E].copy_from_slice(&self.insn_count.to_le_bytes());
        buf[0x0E..0x10].copy_from_slice(&self.max_dimension.to_le_bytes());
        buf[0x10..0x18].copy_from_slice(&self.program_size.to_le_bytes());
        buf[0x18..0x1C].copy_from_slice(&self.map_count.to_le_bytes());
        buf[0x1C..0x20].copy_from_slice(&self.btf_size.to_le_bytes());
        buf[0x20..0x40].copy_from_slice(&self.program_hash);
        buf
    }

    /// Deserialize an `EbpfHeader` from a 64-byte slice.
    pub fn from_bytes(data: &[u8; 64]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != EBPF_MAGIC {
            return Err(RvfError::BadMagic {
                expected: EBPF_MAGIC,
                got: magic,
            });
        }

        Ok(Self {
            ebpf_magic: magic,
            header_version: u16::from_le_bytes([data[0x04], data[0x05]]),
            program_type: data[0x06],
            attach_type: data[0x07],
            program_flags: u32::from_le_bytes([data[0x08], data[0x09], data[0x0A], data[0x0B]]),
            insn_count: u16::from_le_bytes([data[0x0C], data[0x0D]]),
            max_dimension: u16::from_le_bytes([data[0x0E], data[0x0F]]),
            program_size: u64::from_le_bytes([
                data[0x10], data[0x11], data[0x12], data[0x13], data[0x14], data[0x15], data[0x16],
                data[0x17],
            ]),
            map_count: u32::from_le_bytes([data[0x18], data[0x19], data[0x1A], data[0x1B]]),
            btf_size: u32::from_le_bytes([data[0x1C], data[0x1D], data[0x1E], data[0x1F]]),
            program_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x20..0x40]);
                h
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> EbpfHeader {
        EbpfHeader {
            ebpf_magic: EBPF_MAGIC,
            header_version: 1,
            program_type: EbpfProgramType::XdpDistance as u8,
            attach_type: EbpfAttachType::XdpIngress as u8,
            program_flags: 0,
            insn_count: 256,
            max_dimension: 1536,
            program_size: 4096,
            map_count: 2,
            btf_size: 512,
            program_hash: [0xDE; 32],
        }
    }

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<EbpfHeader>(), 64);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = EBPF_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVBP");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = EbpfHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.ebpf_magic, EBPF_MAGIC);
        assert_eq!(decoded.header_version, 1);
        assert_eq!(decoded.program_type, EbpfProgramType::XdpDistance as u8);
        assert_eq!(decoded.attach_type, EbpfAttachType::XdpIngress as u8);
        assert_eq!(decoded.program_flags, 0);
        assert_eq!(decoded.insn_count, 256);
        assert_eq!(decoded.max_dimension, 1536);
        assert_eq!(decoded.program_size, 4096);
        assert_eq!(decoded.map_count, 2);
        assert_eq!(decoded.btf_size, 512);
        assert_eq!(decoded.program_hash, [0xDE; 32]);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = EbpfHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, EBPF_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.ebpf_magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.header_version as *const _ as usize - base, 0x04);
        assert_eq!(&h.program_type as *const _ as usize - base, 0x06);
        assert_eq!(&h.attach_type as *const _ as usize - base, 0x07);
        assert_eq!(&h.program_flags as *const _ as usize - base, 0x08);
        assert_eq!(&h.insn_count as *const _ as usize - base, 0x0C);
        assert_eq!(&h.max_dimension as *const _ as usize - base, 0x0E);
        assert_eq!(&h.program_size as *const _ as usize - base, 0x10);
        assert_eq!(&h.map_count as *const _ as usize - base, 0x18);
        assert_eq!(&h.btf_size as *const _ as usize - base, 0x1C);
        assert_eq!(&h.program_hash as *const _ as usize - base, 0x20);
    }

    #[test]
    fn ebpf_program_type_try_from() {
        assert_eq!(
            EbpfProgramType::try_from(0x00),
            Ok(EbpfProgramType::XdpDistance)
        );
        assert_eq!(
            EbpfProgramType::try_from(0x01),
            Ok(EbpfProgramType::TcFilter)
        );
        assert_eq!(
            EbpfProgramType::try_from(0x02),
            Ok(EbpfProgramType::SocketFilter)
        );
        assert_eq!(
            EbpfProgramType::try_from(0x03),
            Ok(EbpfProgramType::Tracepoint)
        );
        assert_eq!(EbpfProgramType::try_from(0x04), Ok(EbpfProgramType::Kprobe));
        assert_eq!(
            EbpfProgramType::try_from(0x05),
            Ok(EbpfProgramType::CgroupSkb)
        );
        assert_eq!(EbpfProgramType::try_from(0xFF), Ok(EbpfProgramType::Custom));
        assert!(EbpfProgramType::try_from(0x06).is_err());
        assert!(EbpfProgramType::try_from(0x80).is_err());
    }

    #[test]
    fn ebpf_attach_type_try_from() {
        assert_eq!(
            EbpfAttachType::try_from(0x00),
            Ok(EbpfAttachType::XdpIngress)
        );
        assert_eq!(
            EbpfAttachType::try_from(0x01),
            Ok(EbpfAttachType::TcIngress)
        );
        assert_eq!(EbpfAttachType::try_from(0x02), Ok(EbpfAttachType::TcEgress));
        assert_eq!(
            EbpfAttachType::try_from(0x03),
            Ok(EbpfAttachType::SocketFilter)
        );
        assert_eq!(
            EbpfAttachType::try_from(0x04),
            Ok(EbpfAttachType::CgroupIngress)
        );
        assert_eq!(
            EbpfAttachType::try_from(0x05),
            Ok(EbpfAttachType::CgroupEgress)
        );
        assert_eq!(EbpfAttachType::try_from(0xFF), Ok(EbpfAttachType::None));
        assert!(EbpfAttachType::try_from(0x06).is_err());
        assert!(EbpfAttachType::try_from(0x80).is_err());
    }

    #[test]
    fn max_dimension_round_trip() {
        let mut h = sample_header();
        h.max_dimension = 2048;
        let bytes = h.to_bytes();
        let decoded = EbpfHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.max_dimension, 2048);
    }

    #[test]
    fn large_program_size_round_trip() {
        let mut h = sample_header();
        h.program_size = 1_048_576; // 1 MiB
        h.insn_count = 65535;
        let bytes = h.to_bytes();
        let decoded = EbpfHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.program_size, 1_048_576);
        assert_eq!(decoded.insn_count, 65535);
    }
}
