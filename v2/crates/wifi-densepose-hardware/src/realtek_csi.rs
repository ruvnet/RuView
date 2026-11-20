//! Vendor-neutral Realtek RTL8721Dx (AmebaDplus) 1x1 CSI transport and
//! deterministic simulator. This is not the vendor SDK's raw
//! `rtw_event_csi_report_info` ABI (no CRC, C flexible-array payload); see
//! ADR-323 for the field-by-field mapping and rationale.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const RAC1_MAGIC: u32 = 0x3143_4152; // "RAC1" little endian
pub const RAC1_VERSION: u8 = 1;
pub const RAC1_HEADER_LEN: usize = 49;
pub const RAC1_CRC_LEN: usize = 4;
pub const RAC1_MAX_FRAME_LEN: usize = 65_507;
/// Upper bound on subcarriers a 1x1 20/40 MHz OFDM/HT/VHT/HE capture can
/// plausibly report; guards `to_bytes`/`from_bytes` against runaway sizes.
pub const RAC1_MAX_SUBCARRIERS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CsiMode {
    Passive = 0,
    Active = 1,
    BoardToBoard = 2,
}

impl TryFrom<u8> for CsiMode {
    type Error = CsiParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Passive),
            1 => Ok(Self::Active),
            2 => Ok(Self::BoardToBoard),
            _ => Err(CsiParseError::UnknownCsiMode(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProtocolMode {
    Ofdm = 0,
    Ht = 1,
    Vht = 2,
    He = 3,
}

impl TryFrom<u8> for ProtocolMode {
    type Error = CsiParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ofdm),
            1 => Ok(Self::Ht),
            2 => Ok(Self::Vht),
            3 => Ok(Self::He),
            _ => Err(CsiParseError::UnknownProtocolMode(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsiFlags(pub u8);

impl CsiFlags {
    pub const SYNTHETIC: u8 = 1 << 0;
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsiFrame {
    /// Disambiguates multi-board illuminator/receiver deployments; 0 = unset.
    pub node_id: u8,
    pub csi_mode: CsiMode,
    pub sequence: u32,
    /// Hardware-clock microseconds; wraps roughly every 71 minutes. Consumers
    /// must reorder by `sequence`, not raw timestamp arithmetic, across a wrap.
    pub timestamp_us: u32,
    pub peer_mac: [u8; 6],
    pub trig_mac: [u8; 6],
    pub channel: u8,
    /// 0 = 20 MHz, 1 = 40 MHz.
    pub bandwidth: u8,
    pub rx_rate: u8,
    pub protocol_mode: ProtocolMode,
    pub num_sub_carrier: u16,
    /// 16 (8-bit I + 8-bit Q) or 32 (16-bit I + 16-bit Q).
    pub num_bit_per_tone: u8,
    /// Tone group: 1/2/4/8 (every Nth tone reported).
    pub decimation: u8,
    /// Single RF chain (1x1 hardware).
    pub rssi_dbm: i8,
    pub rxsc: u8,
    pub csi_valid: bool,
    pub flags: CsiFlags,
    /// One (I, Q) pair per reported subcarrier, widened to i16 regardless of
    /// wire precision (an 8-bit wire value fits losslessly).
    pub payload: Vec<(i16, i16)>,
}

impl CsiFrame {
    fn component_bytes(&self) -> Result<usize, CsiParseError> {
        match self.num_bit_per_tone {
            16 => Ok(1),
            32 => Ok(2),
            other => Err(CsiParseError::InvalidToneBits(other)),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, CsiParseError> {
        self.validate()?;
        let comp_bytes = self.component_bytes()?;
        let payload_len = self
            .payload
            .len()
            .checked_mul(comp_bytes * 2)
            .ok_or(CsiParseError::LengthOverflow)?;
        let frame_len = RAC1_HEADER_LEN
            .checked_add(payload_len)
            .and_then(|n| n.checked_add(RAC1_CRC_LEN))
            .ok_or(CsiParseError::LengthOverflow)?;
        if frame_len > RAC1_MAX_FRAME_LEN {
            return Err(CsiParseError::FrameTooLarge(frame_len));
        }
        let mut out = Vec::with_capacity(frame_len);
        out.extend_from_slice(&RAC1_MAGIC.to_le_bytes());
        out.push(RAC1_VERSION);
        out.extend_from_slice(&(RAC1_HEADER_LEN as u16).to_le_bytes());
        out.extend_from_slice(&(frame_len as u32).to_le_bytes());
        out.push(self.node_id);
        out.push(self.csi_mode as u8);
        out.extend_from_slice(&self.sequence.to_le_bytes());
        out.extend_from_slice(&self.timestamp_us.to_le_bytes());
        out.extend_from_slice(&self.peer_mac);
        out.extend_from_slice(&self.trig_mac);
        out.push(self.channel);
        out.push(self.bandwidth);
        out.push(self.rx_rate);
        out.push(self.protocol_mode as u8);
        out.extend_from_slice(&self.num_sub_carrier.to_le_bytes());
        out.push(self.num_bit_per_tone);
        out.push(self.decimation);
        out.push(self.rssi_dbm as u8);
        out.push(self.rxsc);
        out.push(self.csi_valid as u8);
        out.push(self.flags.0);
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        debug_assert_eq!(out.len(), RAC1_HEADER_LEN);
        for (i, q) in &self.payload {
            if comp_bytes == 1 {
                out.push(*i as i8 as u8);
                out.push(*q as i8 as u8);
            } else {
                out.extend_from_slice(&i.to_le_bytes());
                out.extend_from_slice(&q.to_le_bytes());
            }
        }
        out.extend_from_slice(&crc32_ieee(&out).to_le_bytes());
        Ok(out)
    }

    pub fn from_bytes(input: &[u8]) -> Result<(Self, usize), CsiParseError> {
        if input.len() < RAC1_HEADER_LEN {
            return Err(CsiParseError::InsufficientData {
                needed: RAC1_HEADER_LEN,
                got: input.len(),
            });
        }
        let magic = u32_at(input, 0);
        if magic != RAC1_MAGIC {
            return Err(CsiParseError::InvalidMagic(magic));
        }
        if input[4] != RAC1_VERSION {
            return Err(CsiParseError::UnsupportedVersion(input[4]));
        }
        let header_len = u16_at(input, 5) as usize;
        if header_len != RAC1_HEADER_LEN {
            return Err(CsiParseError::InvalidHeaderLength(header_len));
        }
        let frame_len = u32_at(input, 7) as usize;
        if frame_len > RAC1_MAX_FRAME_LEN {
            return Err(CsiParseError::FrameTooLarge(frame_len));
        }
        if frame_len < header_len + RAC1_CRC_LEN {
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

        let node_id = input[11];
        let csi_mode = CsiMode::try_from(input[12])?;
        let sequence = u32_at(input, 13);
        let timestamp_us = u32_at(input, 17);
        let mut peer_mac = [0u8; 6];
        peer_mac.copy_from_slice(&input[21..27]);
        let mut trig_mac = [0u8; 6];
        trig_mac.copy_from_slice(&input[27..33]);
        let channel = input[33];
        let bandwidth = input[34];
        let rx_rate = input[35];
        let protocol_mode = ProtocolMode::try_from(input[36])?;
        let num_sub_carrier = u16_at(input, 37);
        let num_bit_per_tone = input[39];
        let decimation = input[40];
        let rssi_dbm = input[41] as i8;
        let rxsc = input[42];
        let csi_valid = input[43] != 0;
        let flags = CsiFlags(input[44]);
        let payload_len = u32_at(input, 45) as usize;

        if header_len + payload_len + RAC1_CRC_LEN != frame_len {
            return Err(CsiParseError::PayloadLengthMismatch);
        }
        let comp_bytes = match num_bit_per_tone {
            16 => 1usize,
            32 => 2usize,
            other => return Err(CsiParseError::InvalidToneBits(other)),
        };
        let expected_payload_len = (num_sub_carrier as usize)
            .checked_mul(comp_bytes * 2)
            .ok_or(CsiParseError::LengthOverflow)?;
        if payload_len != expected_payload_len {
            return Err(CsiParseError::PayloadLengthMismatch);
        }
        let payload_bytes = &input[header_len..header_len + payload_len];
        let payload = if comp_bytes == 1 {
            payload_bytes
                .chunks_exact(2)
                .map(|b| (b[0] as i8 as i16, b[1] as i8 as i16))
                .collect()
        } else {
            payload_bytes
                .chunks_exact(4)
                .map(|b| {
                    (
                        i16::from_le_bytes([b[0], b[1]]),
                        i16::from_le_bytes([b[2], b[3]]),
                    )
                })
                .collect()
        };

        let frame = Self {
            node_id,
            csi_mode,
            sequence,
            timestamp_us,
            peer_mac,
            trig_mac,
            channel,
            bandwidth,
            rx_rate,
            protocol_mode,
            num_sub_carrier,
            num_bit_per_tone,
            decimation,
            rssi_dbm,
            rxsc,
            csi_valid,
            flags,
            payload,
        };
        frame.validate()?;
        Ok((frame, frame_len))
    }

    fn validate(&self) -> Result<(), CsiParseError> {
        if !matches!(self.bandwidth, 0 | 1) {
            return Err(CsiParseError::InvalidBandwidth(self.bandwidth));
        }
        let comp_bytes = self.component_bytes()?;
        if self.num_sub_carrier == 0 || self.num_sub_carrier as usize > RAC1_MAX_SUBCARRIERS {
            return Err(CsiParseError::InvalidSubcarrierCount(self.num_sub_carrier));
        }
        if self.payload.len() != self.num_sub_carrier as usize {
            return Err(CsiParseError::PayloadLengthMismatch);
        }
        if comp_bytes == 1 {
            let in_range = |v: i16| (i8::MIN as i16..=i8::MAX as i16).contains(&v);
            if self.payload.iter().any(|(i, q)| !in_range(*i) || !in_range(*q)) {
                return Err(CsiParseError::ToneValueOutOfRange);
            }
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
    #[error("unknown CSI mode {0}")]
    UnknownCsiMode(u8),
    #[error("unknown protocol mode {0}")]
    UnknownProtocolMode(u8),
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
    #[error("invalid num_bit_per_tone {0} (must be 16 or 32)")]
    InvalidToneBits(u8),
    #[error("invalid subcarrier count {0}")]
    InvalidSubcarrierCount(u16),
    #[error("invalid bandwidth code {0} (must be 0 or 1)")]
    InvalidBandwidth(u8),
    #[error("tone I/Q value out of range for 8-bit precision")]
    ToneValueOutOfRange,
    #[error("CRC mismatch: expected {expected:#010x}, actual {actual:#010x}")]
    CrcMismatch { expected: u32, actual: u32 },
}

pub mod simulator {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct SimulatorConfig {
        pub seed: u64,
        pub node_id: u8,
        pub csi_mode: CsiMode,
        pub peer_mac: [u8; 6],
        pub trig_mac: [u8; 6],
        pub channel: u8,
        pub bandwidth: u8,
        pub rx_rate: u8,
        pub protocol_mode: ProtocolMode,
        pub num_sub_carrier: u16,
        pub num_bit_per_tone: u8,
        pub decimation: u8,
        pub rssi_dbm: i8,
        pub frame_period_us: u32,
    }

    impl Default for SimulatorConfig {
        fn default() -> Self {
            Self {
                seed: 0x5241_4331_5254_4c31, // "RAC1RTL1"-ish
                node_id: 1,
                csi_mode: CsiMode::Passive,
                peer_mac: [0xa4, 0x39, 0xb3, 0xa4, 0xbe, 0x2d],
                trig_mac: [0x00, 0xe0, 0x4c, 0x00, 0x0a, 0xa3],
                channel: 6,
                bandwidth: 0,
                rx_rate: 12, // RTW_RATE_6M
                protocol_mode: ProtocolMode::Ofdm,
                num_sub_carrier: 52,
                num_bit_per_tone: 16,
                decimation: 1,
                rssi_dbm: -45,
                frame_period_us: 64_000,
            }
        }
    }

    pub struct RealtekCsiSimulator {
        config: SimulatorConfig,
        rng: u64,
        sequence: u32,
        timestamp_us: u32,
        motion_phase: f32,
    }

    impl RealtekCsiSimulator {
        pub fn new(config: SimulatorConfig) -> Result<Self, CsiParseError> {
            let s = Self {
                rng: config.seed,
                config,
                sequence: 0,
                timestamp_us: 0,
                motion_phase: 0.0,
            };
            s.csi_frame().validate()?;
            Ok(s)
        }

        pub fn next_frame(&mut self) -> CsiFrame {
            let frame = self.csi_frame();
            self.sequence = self.sequence.wrapping_add(1);
            self.timestamp_us = self.timestamp_us.wrapping_add(self.config.frame_period_us);
            self.motion_phase += 0.041;
            frame
        }

        fn csi_frame(&self) -> CsiFrame {
            let mut rng = self.rng ^ self.sequence as u64;
            let comp_max = if self.config.num_bit_per_tone == 16 {
                110.0
            } else {
                1800.0
            };
            let payload = (0..self.config.num_sub_carrier as usize)
                .map(|tone| {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    let noise = ((rng >> 48) as i16 % 8) as f32;
                    let phase = tone as f32 * 0.11 + self.motion_phase;
                    (
                        ((phase.cos() * comp_max) + noise) as i16,
                        ((phase.sin() * comp_max) - noise) as i16,
                    )
                })
                .collect();
            CsiFrame {
                node_id: self.config.node_id,
                csi_mode: self.config.csi_mode,
                sequence: self.sequence,
                timestamp_us: self.timestamp_us,
                peer_mac: self.config.peer_mac,
                trig_mac: self.config.trig_mac,
                channel: self.config.channel,
                bandwidth: self.config.bandwidth,
                rx_rate: self.config.rx_rate,
                protocol_mode: self.config.protocol_mode,
                num_sub_carrier: self.config.num_sub_carrier,
                num_bit_per_tone: self.config.num_bit_per_tone,
                decimation: self.config.decimation,
                rssi_dbm: self.config.rssi_dbm,
                rxsc: 0,
                csi_valid: true,
                flags: CsiFlags(CsiFlags::SYNTHETIC),
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
        let mut a = RealtekCsiSimulator::new(cfg.clone()).unwrap();
        let mut b = RealtekCsiSimulator::new(cfg).unwrap();
        let wa = a.next_frame().to_bytes().unwrap();
        assert_eq!(wa, b.next_frame().to_bytes().unwrap());
        let (decoded, n) = CsiFrame::from_bytes(&wa).unwrap();
        assert_eq!(n, wa.len());
        assert!(decoded.flags.contains(CsiFlags::SYNTHETIC));
        assert_eq!(decoded.payload.len(), 52);
    }

    #[test]
    fn sixteen_bit_precision_round_trip() {
        let cfg = SimulatorConfig {
            num_bit_per_tone: 32,
            ..Default::default()
        };
        let mut sim = RealtekCsiSimulator::new(cfg).unwrap();
        let frame = sim.next_frame();
        let wire = frame.to_bytes().unwrap();
        let (decoded, n) = CsiFrame::from_bytes(&wire).unwrap();
        assert_eq!(n, wire.len());
        assert_eq!(decoded, frame);
    }

    #[test]
    fn crc_corruption_is_rejected() {
        let mut s = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut w = s.next_frame().to_bytes().unwrap();
        let last = w.len() - 1;
        w[last] ^= 1;
        assert!(matches!(
            CsiFrame::from_bytes(&w),
            Err(CsiParseError::CrcMismatch { .. })
        ));
    }

    #[test]
    fn truncation_is_rejected() {
        let mut s = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let w = s.next_frame().to_bytes().unwrap();
        assert!(matches!(
            CsiFrame::from_bytes(&w[..w.len() - 1]),
            Err(CsiParseError::InsufficientData { .. })
        ));
    }

    #[test]
    fn zero_subcarriers_is_rejected() {
        let cfg = SimulatorConfig {
            num_sub_carrier: 0,
            ..Default::default()
        };
        assert!(matches!(
            RealtekCsiSimulator::new(cfg),
            Err(CsiParseError::InvalidSubcarrierCount(0))
        ));
    }

    #[test]
    fn invalid_tone_bits_is_rejected() {
        let cfg = SimulatorConfig {
            num_bit_per_tone: 24,
            ..Default::default()
        };
        assert!(matches!(
            RealtekCsiSimulator::new(cfg),
            Err(CsiParseError::InvalidToneBits(24))
        ));
    }

    #[test]
    fn parser_never_panics_on_prefixes() {
        let mut s = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let w = s.next_frame().to_bytes().unwrap();
        for end in 0..w.len() {
            let _ = CsiFrame::from_bytes(&w[..end]);
        }
    }

    #[test]
    fn magic_mismatch_is_rejected() {
        let mut s = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut w = s.next_frame().to_bytes().unwrap();
        w[0] ^= 0xff;
        assert!(matches!(
            CsiFrame::from_bytes(&w),
            Err(CsiParseError::InvalidMagic(_))
        ));
    }
}
