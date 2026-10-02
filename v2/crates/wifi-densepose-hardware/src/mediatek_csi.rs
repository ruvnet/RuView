//! Vendor-neutral MediaTek Filogic MIMO CSI transport and deterministic simulator.
//! This is not a MediaTek firmware ABI; see ADR-266/267.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MEDIATEK_CSI_MAGIC: u32 = 0x3143_544d; // "MTC1" little endian
pub const MEDIATEK_CSI_VERSION: u8 = 1;
pub const MEDIATEK_CSI_HEADER_LEN: usize = 72;
pub const MEDIATEK_CSI_CRC_LEN: usize = 4;
pub const MEDIATEK_CSI_MAX_FRAME_LEN: usize = 65_507;
pub const MEDIATEK_CSI_MAX_ELEMENTS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ReportKind {
    Csi = 1,
    Capabilities = 2,
}

impl TryFrom<u8> for ReportKind {
    type Error = CsiParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Csi),
            2 => Ok(Self::Capabilities),
            _ => Err(CsiParseError::UnknownReportKind(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ChipsetProfile {
    Mt7981Mt7976 = 1,
    Mt7986Mt7975 = 2,
    Mt7988Mt7996 = 3,
}

impl TryFrom<u16> for ChipsetProfile {
    type Error = CsiParseError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Mt7981Mt7976),
            2 => Ok(Self::Mt7986Mt7975),
            3 => Ok(Self::Mt7988Mt7996),
            _ => Err(CsiParseError::UnknownChipset(value)),
        }
    }
}

impl ChipsetProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mt7981Mt7976 => "mt7981-mt7976",
            Self::Mt7986Mt7975 => "mt7986-mt7975",
            Self::Mt7988Mt7996 => "mt7988-mt7996",
        }
    }
    pub fn max_chains(self) -> u8 {
        match self {
            Self::Mt7981Mt7976 => 3,
            Self::Mt7986Mt7975 | Self::Mt7988Mt7996 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ElementFormat {
    ComplexI16 = 1,
    ComplexF32 = 2,
    Bytes = 3,
}

impl TryFrom<u8> for ElementFormat {
    type Error = CsiParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ComplexI16),
            2 => Ok(Self::ComplexF32),
            3 => Ok(Self::Bytes),
            _ => Err(CsiParseError::UnknownElementFormat(value)),
        }
    }
}

/// PPDU format a CSI report was measured on.
///
/// `Legacy` and `Unknown` were added after a real MT7981 capture (a 2.4 GHz
/// client on channel 6) reported `rx_mode = MT_PHY_TYPE_OFDM`. MediaTek's CSI
/// driver treats legacy OFDM as a first-class mode — its tone-mask table gives
/// `MT_PHY_TYPE_OFDM` its own group — so a vendor bridge must be able to carry
/// one. Without these, such a frame had to be dropped, which lost real data.
///
/// Wire note: a decoder older than this change rejects discriminants 6 and 7 as
/// `UnknownPpduType`. Producers should only emit them for frames that genuinely
/// are pre-HT or unrecognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PpduType {
    Ht = 1,
    Vht = 2,
    HeSu = 3,
    HeMu = 4,
    Eht = 5,
    /// Pre-HT: 802.11b CCK or 802.11a/g OFDM.
    Legacy = 6,
    /// A PPDU format the producer could not classify. Carried rather than
    /// guessed, so the frame survives and the ambiguity stays visible.
    Unknown = 7,
}

impl TryFrom<u8> for PpduType {
    type Error = CsiParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Ht),
            2 => Ok(Self::Vht),
            3 => Ok(Self::HeSu),
            4 => Ok(Self::HeMu),
            5 => Ok(Self::Eht),
            6 => Ok(Self::Legacy),
            7 => Ok(Self::Unknown),
            _ => Err(CsiParseError::UnknownPpduType(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsiFlags(pub u16);

impl CsiFlags {
    pub const CALIBRATED: u16 = 1 << 0;
    pub const SATURATED: u16 = 1 << 1;
    pub const TIME_SYNCHRONIZED: u16 = 1 << 2;
    pub const DROPPED_PREDECESSOR: u16 = 1 << 3;
    pub const SYNTHETIC: u16 = 1 << 15;
    pub fn contains(self, flag: u16) -> bool {
        self.0 & flag != 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CsiPayload {
    ComplexI16 {
        rssi_dbm: Vec<i8>,
        values: Vec<[i16; 2]>,
    },
    ComplexF32 {
        rssi_dbm: Vec<i8>,
        values: Vec<[f32; 2]>,
    },
    Bytes(Vec<u8>),
}

impl CsiPayload {
    pub fn len(&self) -> usize {
        match self {
            Self::ComplexI16 { values, .. } => values.len(),
            Self::ComplexF32 { values, .. } => values.len(),
            Self::Bytes(values) => values.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn rssi_dbm(&self) -> &[i8] {
        match self {
            Self::ComplexI16 { rssi_dbm, .. } | Self::ComplexF32 { rssi_dbm, .. } => rssi_dbm,
            Self::Bytes(_) => &[],
        }
    }
    fn format(&self) -> ElementFormat {
        match self {
            Self::ComplexI16 { .. } => ElementFormat::ComplexI16,
            Self::ComplexF32 { .. } => ElementFormat::ComplexF32,
            Self::Bytes(_) => ElementFormat::Bytes,
        }
    }
    fn encoded_len(&self) -> usize {
        match self {
            Self::ComplexI16 { rssi_dbm, values } => rssi_dbm.len() + values.len() * 4,
            Self::ComplexF32 { rssi_dbm, values } => rssi_dbm.len() + values.len() * 8,
            Self::Bytes(values) => values.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsiFrame {
    pub report_kind: ReportKind,
    pub sequence: u32,
    pub timestamp_us: u64,
    pub device_id: u64,
    pub chipset: ChipsetProfile,
    pub bandwidth_mhz: u16,
    pub center_freq_khz: u32,
    pub flags: CsiFlags,
    pub tx_count: u8,
    pub rx_count: u8,
    pub ppdu_type: PpduType,
    pub subcarrier_count: u16,
    pub noise_floor_dbm: i8,
    pub scale: f32,
    pub subcarrier_spacing_hz: f32,
    pub calibration_id: u32,
    pub payload: CsiPayload,
}

impl CsiFrame {
    pub fn to_bytes(&self) -> Result<Vec<u8>, CsiParseError> {
        self.validate()?;
        let payload_len = self.payload.encoded_len();
        let frame_len = MEDIATEK_CSI_HEADER_LEN
            .checked_add(payload_len)
            .and_then(|n| n.checked_add(MEDIATEK_CSI_CRC_LEN))
            .ok_or(CsiParseError::LengthOverflow)?;
        if frame_len > MEDIATEK_CSI_MAX_FRAME_LEN {
            return Err(CsiParseError::FrameTooLarge(frame_len));
        }
        let mut out = Vec::with_capacity(frame_len);
        out.extend_from_slice(&MEDIATEK_CSI_MAGIC.to_le_bytes());
        out.push(MEDIATEK_CSI_VERSION);
        out.push(self.report_kind as u8);
        out.extend_from_slice(&(MEDIATEK_CSI_HEADER_LEN as u16).to_le_bytes());
        out.extend_from_slice(&(frame_len as u32).to_le_bytes());
        out.extend_from_slice(&self.sequence.to_le_bytes());
        out.extend_from_slice(&self.timestamp_us.to_le_bytes());
        out.extend_from_slice(&self.device_id.to_le_bytes());
        out.extend_from_slice(&(self.chipset as u16).to_le_bytes());
        out.extend_from_slice(&self.bandwidth_mhz.to_le_bytes());
        out.extend_from_slice(&self.center_freq_khz.to_le_bytes());
        out.extend_from_slice(&self.flags.0.to_le_bytes());
        out.push(self.tx_count);
        out.push(self.rx_count);
        out.push(self.payload.format() as u8);
        out.push(self.ppdu_type as u8);
        out.extend_from_slice(&self.subcarrier_count.to_le_bytes());
        out.push(self.payload.rssi_dbm().len() as u8);
        out.push(self.noise_floor_dbm as u8);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.scale.to_le_bytes());
        out.extend_from_slice(&self.subcarrier_spacing_hz.to_le_bytes());
        out.extend_from_slice(&self.calibration_id.to_le_bytes());
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        debug_assert_eq!(out.len(), MEDIATEK_CSI_HEADER_LEN);
        match &self.payload {
            CsiPayload::ComplexI16 { rssi_dbm, values } => {
                out.extend(rssi_dbm.iter().map(|v| *v as u8));
                for [i, q] in values {
                    out.extend_from_slice(&i.to_le_bytes());
                    out.extend_from_slice(&q.to_le_bytes());
                }
            }
            CsiPayload::ComplexF32 { rssi_dbm, values } => {
                out.extend(rssi_dbm.iter().map(|v| *v as u8));
                for [i, q] in values {
                    out.extend_from_slice(&i.to_le_bytes());
                    out.extend_from_slice(&q.to_le_bytes());
                }
            }
            CsiPayload::Bytes(values) => out.extend_from_slice(values),
        }
        out.extend_from_slice(&crc32_ieee(&out).to_le_bytes());
        Ok(out)
    }

    pub fn from_bytes(input: &[u8]) -> Result<(Self, usize), CsiParseError> {
        if input.len() < MEDIATEK_CSI_HEADER_LEN {
            return Err(CsiParseError::InsufficientData {
                needed: MEDIATEK_CSI_HEADER_LEN,
                got: input.len(),
            });
        }
        let magic = u32_at(input, 0);
        if magic != MEDIATEK_CSI_MAGIC {
            return Err(CsiParseError::InvalidMagic(magic));
        }
        if input[4] != MEDIATEK_CSI_VERSION {
            return Err(CsiParseError::UnsupportedVersion(input[4]));
        }
        let report_kind = ReportKind::try_from(input[5])?;
        let header_len = u16_at(input, 6) as usize;
        if header_len != MEDIATEK_CSI_HEADER_LEN {
            return Err(CsiParseError::InvalidHeaderLength(header_len));
        }
        let frame_len = u32_at(input, 8) as usize;
        if frame_len > MEDIATEK_CSI_MAX_FRAME_LEN {
            return Err(CsiParseError::FrameTooLarge(frame_len));
        }
        if frame_len < header_len + MEDIATEK_CSI_CRC_LEN {
            return Err(CsiParseError::InvalidFrameLength(frame_len));
        }
        if input.len() < frame_len {
            return Err(CsiParseError::InsufficientData {
                needed: frame_len,
                got: input.len(),
            });
        }
        let expected_crc = u32_at(input, frame_len - 4);
        let actual_crc = crc32_ieee(&input[..frame_len - 4]);
        if expected_crc != actual_crc {
            return Err(CsiParseError::CrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }
        let chipset = ChipsetProfile::try_from(u16_at(input, 32))?;
        let format = ElementFormat::try_from(input[44])?;
        let ppdu_type = PpduType::try_from(input[45])?;
        let tx_count = input[42];
        let rx_count = input[43];
        let subcarrier_count = u16_at(input, 46);
        let rssi_count = input[48] as usize;
        let payload_len = u32_at(input, 64) as usize;
        if header_len + payload_len + 4 != frame_len {
            return Err(CsiParseError::PayloadLengthMismatch);
        }
        let payload_bytes = &input[header_len..header_len + payload_len];
        let elements = (tx_count as usize)
            .checked_mul(rx_count as usize)
            .and_then(|n| n.checked_mul(subcarrier_count as usize))
            .ok_or(CsiParseError::LengthOverflow)?;
        let payload = match format {
            ElementFormat::Bytes => CsiPayload::Bytes(payload_bytes.to_vec()),
            ElementFormat::ComplexI16 => {
                if rssi_count > payload_bytes.len()
                    || payload_bytes.len() - rssi_count != elements * 4
                {
                    return Err(CsiParseError::PayloadLengthMismatch);
                }
                let rssi_dbm = payload_bytes[..rssi_count]
                    .iter()
                    .map(|v| *v as i8)
                    .collect();
                let values = payload_bytes[rssi_count..]
                    .chunks_exact(4)
                    .map(|b| {
                        [
                            i16::from_le_bytes([b[0], b[1]]),
                            i16::from_le_bytes([b[2], b[3]]),
                        ]
                    })
                    .collect();
                CsiPayload::ComplexI16 { rssi_dbm, values }
            }
            ElementFormat::ComplexF32 => {
                if rssi_count > payload_bytes.len()
                    || payload_bytes.len() - rssi_count != elements * 8
                {
                    return Err(CsiParseError::PayloadLengthMismatch);
                }
                let rssi_dbm = payload_bytes[..rssi_count]
                    .iter()
                    .map(|v| *v as i8)
                    .collect();
                let mut values = Vec::with_capacity(elements);
                for b in payload_bytes[rssi_count..].chunks_exact(8) {
                    let i = f32::from_le_bytes(b[0..4].try_into().unwrap());
                    let q = f32::from_le_bytes(b[4..8].try_into().unwrap());
                    if !i.is_finite() || !q.is_finite() {
                        return Err(CsiParseError::NonFiniteValue);
                    }
                    values.push([i, q]);
                }
                CsiPayload::ComplexF32 { rssi_dbm, values }
            }
        };
        let frame = Self {
            report_kind,
            sequence: u32_at(input, 12),
            timestamp_us: u64_at(input, 16),
            device_id: u64_at(input, 24),
            chipset,
            bandwidth_mhz: u16_at(input, 34),
            center_freq_khz: u32_at(input, 36),
            flags: CsiFlags(u16_at(input, 40)),
            tx_count,
            rx_count,
            ppdu_type,
            subcarrier_count,
            noise_floor_dbm: input[49] as i8,
            scale: f32_at(input, 52),
            subcarrier_spacing_hz: f32_at(input, 56),
            calibration_id: u32_at(input, 60),
            payload,
        };
        frame.validate()?;
        Ok((frame, frame_len))
    }

    fn validate(&self) -> Result<(), CsiParseError> {
        if !matches!(self.bandwidth_mhz, 20 | 40 | 80 | 160) {
            return Err(CsiParseError::InvalidBandwidth(self.bandwidth_mhz));
        }
        if self.tx_count == 0
            || self.rx_count == 0
            || self.tx_count > self.chipset.max_chains()
            || self.rx_count > self.chipset.max_chains()
        {
            return Err(CsiParseError::InvalidDimensions);
        }
        if !self.scale.is_finite()
            || self.scale <= 0.0
            || !self.subcarrier_spacing_hz.is_finite()
            || self.subcarrier_spacing_hz <= 0.0
        {
            return Err(CsiParseError::NonFiniteValue);
        }
        match (&self.report_kind, &self.payload) {
            (ReportKind::Csi, CsiPayload::ComplexI16 { rssi_dbm, values }) => {
                self.validate_csi(rssi_dbm, values.len())
            }
            (ReportKind::Csi, CsiPayload::ComplexF32 { rssi_dbm, values }) => {
                if !values.iter().flatten().all(|v| v.is_finite()) {
                    return Err(CsiParseError::NonFiniteValue);
                }
                self.validate_csi(rssi_dbm, values.len())
            }
            (ReportKind::Capabilities, CsiPayload::Bytes(v)) if !v.is_empty() => Ok(()),
            _ => Err(CsiParseError::PayloadTypeMismatch),
        }
    }
    fn validate_csi(&self, rssi: &[i8], values: usize) -> Result<(), CsiParseError> {
        let expected =
            self.tx_count as usize * self.rx_count as usize * self.subcarrier_count as usize;
        if expected == 0 || expected > MEDIATEK_CSI_MAX_ELEMENTS {
            return Err(CsiParseError::InvalidDimensions);
        }
        if values != expected || rssi.len() != self.rx_count as usize {
            return Err(CsiParseError::PayloadLengthMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum CsiParseError {
    #[error("insufficient data: needed {needed}, got {got}")]
    InsufficientData { needed: usize, got: usize },
    #[error("invalid magic {0:#010x}")]
    InvalidMagic(u32),
    #[error("unsupported version {0}")]
    UnsupportedVersion(u8),
    #[error("unknown report kind {0}")]
    UnknownReportKind(u8),
    #[error("unknown chipset profile {0}")]
    UnknownChipset(u16),
    #[error("unknown element format {0}")]
    UnknownElementFormat(u8),
    #[error("unknown PPDU type {0}")]
    UnknownPpduType(u8),
    #[error("invalid header length {0}")]
    InvalidHeaderLength(usize),
    #[error("invalid frame length {0}")]
    InvalidFrameLength(usize),
    #[error("frame too large: {0}")]
    FrameTooLarge(usize),
    #[error("length arithmetic overflow")]
    LengthOverflow,
    #[error("payload length mismatch")]
    PayloadLengthMismatch,
    #[error("payload type does not match report kind")]
    PayloadTypeMismatch,
    #[error("invalid MIMO dimensions")]
    InvalidDimensions,
    #[error("invalid bandwidth {0} MHz")]
    InvalidBandwidth(u16),
    #[error("non-finite or non-positive numeric metadata/value")]
    NonFiniteValue,
    #[error("CRC mismatch: expected {expected:#010x}, actual {actual:#010x}")]
    CrcMismatch { expected: u32, actual: u32 },
}

pub mod simulator {
    use super::*;
    #[derive(Debug, Clone)]
    pub struct SimulatorConfig {
        pub seed: u64,
        pub device_id: u64,
        pub chipset: ChipsetProfile,
        pub bandwidth_mhz: u16,
        pub center_freq_khz: u32,
        pub tx_count: u8,
        pub rx_count: u8,
        pub subcarriers: u16,
        pub frame_period_us: u64,
    }
    impl Default for SimulatorConfig {
        fn default() -> Self {
            Self {
                seed: 0x4d54_4b43_5349_0001,
                device_id: 0x4f57_5254_4d54_4b31,
                chipset: ChipsetProfile::Mt7981Mt7976,
                bandwidth_mhz: 80,
                center_freq_khz: 5_210_000,
                tx_count: 2,
                rx_count: 3,
                subcarriers: 256,
                frame_period_us: 20_000,
            }
        }
    }
    pub struct MediatekCsiSimulator {
        config: SimulatorConfig,
        rng: u64,
        sequence: u32,
        timestamp_us: u64,
        motion_phase: f32,
    }
    impl MediatekCsiSimulator {
        pub fn new(config: SimulatorConfig) -> Result<Self, CsiParseError> {
            let s = Self {
                rng: config.seed,
                config,
                sequence: 0,
                timestamp_us: 0,
                motion_phase: 0.0,
            };
            s.csi_frame()?.validate()?;
            Ok(s)
        }
        pub fn capabilities_frame(&self) -> CsiFrame {
            self.base(
                ReportKind::Capabilities,
                CsiPayload::Bytes(vec![
                    1,
                    1,
                    self.config.chipset.max_chains(),
                    2,
                    1,
                    0b0000_1111,
                    3,
                    2,
                    (self.config.subcarriers & 255) as u8,
                    (self.config.subcarriers >> 8) as u8,
                ]),
            )
        }
        pub fn next_frame(&mut self) -> CsiFrame {
            let frame = self.csi_frame().expect("validated simulator config");
            self.sequence = self.sequence.wrapping_add(1);
            self.timestamp_us = self.timestamp_us.wrapping_add(self.config.frame_period_us);
            self.motion_phase += 0.037;
            frame
        }
        fn csi_frame(&self) -> Result<CsiFrame, CsiParseError> {
            let mut rng = self.rng ^ self.sequence as u64;
            let count = self.config.tx_count as usize
                * self.config.rx_count as usize
                * self.config.subcarriers as usize;
            let values = (0..count)
                .map(|idx| {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    let noise = ((rng >> 48) as i16 % 24) as f32;
                    let sc = (idx % self.config.subcarriers as usize) as f32;
                    let chain = (idx / self.config.subcarriers as usize) as f32;
                    let phase = sc * 0.031 + chain * 0.23 + self.motion_phase;
                    [
                        ((phase.cos() * 1800.0) + noise) as i16,
                        ((phase.sin() * 1800.0) - noise) as i16,
                    ]
                })
                .collect();
            Ok(self.base(
                ReportKind::Csi,
                CsiPayload::ComplexI16 {
                    rssi_dbm: (0..self.config.rx_count)
                        .map(|i| -42 - i as i8 * 2)
                        .collect(),
                    values,
                },
            ))
        }
        fn base(&self, kind: ReportKind, payload: CsiPayload) -> CsiFrame {
            CsiFrame {
                report_kind: kind,
                sequence: self.sequence,
                timestamp_us: self.timestamp_us,
                device_id: self.config.device_id,
                chipset: self.config.chipset,
                bandwidth_mhz: self.config.bandwidth_mhz,
                center_freq_khz: self.config.center_freq_khz,
                flags: CsiFlags(CsiFlags::CALIBRATED | CsiFlags::SYNTHETIC),
                tx_count: self.config.tx_count,
                rx_count: self.config.rx_count,
                ppdu_type: PpduType::HeSu,
                subcarrier_count: self.config.subcarriers,
                noise_floor_dbm: -95,
                scale: 1.0 / 2048.0,
                subcarrier_spacing_hz: 312_500.0,
                calibration_id: 1,
                payload,
            }
        }
    }
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ ((0u32.wrapping_sub(crc & 1)) & 0xedb8_8320);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use simulator::*;
    #[test]
    fn simulator_round_trip_is_deterministic() {
        let cfg = SimulatorConfig::default();
        let mut a = MediatekCsiSimulator::new(cfg.clone()).unwrap();
        let mut b = MediatekCsiSimulator::new(cfg).unwrap();
        let wa = a.next_frame().to_bytes().unwrap();
        assert_eq!(wa, b.next_frame().to_bytes().unwrap());
        let (decoded, n) = CsiFrame::from_bytes(&wa).unwrap();
        assert_eq!(n, wa.len());
        assert!(decoded.flags.contains(CsiFlags::SYNTHETIC));
        assert_eq!(decoded.payload.len(), 2 * 3 * 256);
    }
    #[test]
    fn capabilities_round_trip() {
        let s = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let f = s.capabilities_frame();
        let w = f.to_bytes().unwrap();
        assert_eq!(CsiFrame::from_bytes(&w).unwrap().0, f);
    }
    #[test]
    fn legacy_and_unknown_ppdu_types_round_trip() {
        // A real MT7981 capture reported MT_PHY_TYPE_OFDM, which has no HT/VHT/HE
        // equivalent; these two discriminants exist so such a frame is carried
        // rather than dropped.
        let mut s = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        for want in [PpduType::Legacy, PpduType::Unknown] {
            let mut f = s.next_frame();
            f.ppdu_type = want;
            let w = f.to_bytes().unwrap();
            let (decoded, _) = CsiFrame::from_bytes(&w).unwrap();
            assert_eq!(decoded.ppdu_type, want);
        }
        assert_eq!(PpduType::try_from(6u8).unwrap(), PpduType::Legacy);
        assert_eq!(PpduType::try_from(7u8).unwrap(), PpduType::Unknown);
        assert!(matches!(
            PpduType::try_from(8u8),
            Err(CsiParseError::UnknownPpduType(8))
        ));
    }

    #[test]
    fn crc_corruption_is_rejected() {
        let mut s = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut w = s.next_frame().to_bytes().unwrap();
        w[80] ^= 1;
        assert!(matches!(
            CsiFrame::from_bytes(&w),
            Err(CsiParseError::CrcMismatch { .. })
        ));
    }
    #[test]
    fn truncation_is_rejected() {
        let mut s = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let w = s.next_frame().to_bytes().unwrap();
        assert!(matches!(
            CsiFrame::from_bytes(&w[..w.len() - 1]),
            Err(CsiParseError::InsufficientData { .. })
        ));
    }
    #[test]
    fn invalid_dimensions_are_rejected() {
        let cfg = SimulatorConfig {
            rx_count: 4,
            chipset: ChipsetProfile::Mt7981Mt7976,
            ..Default::default()
        };
        assert!(matches!(
            MediatekCsiSimulator::new(cfg),
            Err(CsiParseError::InvalidDimensions)
        ));
    }
    #[test]
    fn non_finite_float_is_rejected() {
        let mut s = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut f = s.next_frame();
        f.payload = CsiPayload::ComplexF32 {
            rssi_dbm: vec![-40, -42, -44],
            values: vec![[f32::NAN, 0.0]; 2 * 3 * 256],
        };
        assert_eq!(f.to_bytes().unwrap_err(), CsiParseError::NonFiniteValue);
    }
    #[test]
    fn parser_never_panics_on_prefixes() {
        let mut s = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let w = s.next_frame().to_bytes().unwrap();
        for end in 0..w.len() {
            let _ = CsiFrame::from_bytes(&w[..end]);
        }
    }
}
