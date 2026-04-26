//! `MagFrame` — fixed-layout binary frame emitted per sensor per timestep.
//!
//! Per implementation plan §1.4: magic `0xC51A_6E70` (`C51` lineage / `A`
//! for Anomaly / `6E70` ASCII "np" for NV-pipeline). 60-byte payload —
//! fixed for v1.
//!
//! Layout (little-endian, packed):
//!
//! | Offset | Field             | Width | Notes                                 |
//! |--------|-------------------|-------|---------------------------------------|
//! | 0      | `magic`           | u32   | [`MAG_FRAME_MAGIC`]                   |
//! | 4      | `version`         | u16   | [`MAG_FRAME_VERSION`]                 |
//! | 6      | `flags`           | u16   | bit-set (see [`flag`] constants)      |
//! | 8      | `sensor_id`       | u16   | which sensor in `Scene::sensors`      |
//! | 10     | `_reserved`       | u16   | zero in v1                            |
//! | 12     | `t_us`            | u64   | sample timestamp, μs since pipeline   |
//! | 20     | `bx, by, bz`      | 3×f32 | demodulated B in pT (post-lockin)     |
//! | 32     | `sigma_x,y,z`     | 3×f32 | per-axis 1σ noise estimate, pT        |
//! | 44     | `noise_floor`     | f32   | shot-noise δB pT/√Hz at this sample   |
//! | 48     | `temperature_k`   | f32   | sensor temperature K (default 295)    |
//! | 52     | `_pad`            | 8 B   | zero in v1, future-proofing           |

use serde::{Deserialize, Serialize};

/// Frame magic. Distinct from ADR-018 CSI (`0xC51F...`) and ADR-084 sketch
/// (`0xC511_0084`). See implementation plan §1.4.
pub const MAG_FRAME_MAGIC: u32 = 0xC51A_6E70;

/// Wire-format schema version. Bumped on any field reordering or addition.
pub const MAG_FRAME_VERSION: u16 = 1;

/// Total payload size in bytes for v1.
pub const MAG_FRAME_BYTES: usize = 60;

/// Per-frame status flag bits. Combined into `MagFrame::flags` as a `u16`
/// bit-set; see [`MagFrame::has_flag`] for ergonomic reads.
pub mod flag {
    /// Sensor near-field saturation (source < 1 mm away). Plan §2.1.
    pub const SATURATION_NEAR_FIELD: u16 = 1 << 0;
    /// ADC saturated on at least one axis at this sample.
    pub const ADC_SATURATED: u16 = 1 << 1;
    /// Reinforced-concrete-grade attenuation flagged on LoS.
    pub const HEAVY_ATTENUATION: u16 = 1 << 2;
    /// Pipeline ran with shot-noise disabled (analytic mode).
    pub const SHOT_NOISE_DISABLED: u16 = 1 << 3;
}

/// Decoded `rv_mag_feature_state_t` frame.
///
/// Round-trips through `to_bytes` / `from_bytes` byte-exact; the
/// deserialiser validates magic + version + length and never panics on
/// malformed input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MagFrame {
    /// Per-frame status bit-set ([`flag`] constants).
    pub flags: u16,
    /// Sensor index in `Scene::sensors`.
    pub sensor_id: u16,
    /// Sample timestamp, μs since pipeline start.
    pub t_us: u64,
    /// Demodulated 3-axis B field (pT).
    pub b_pt: [f32; 3],
    /// Per-axis 1σ noise estimate (pT).
    pub sigma_pt: [f32; 3],
    /// Shot-noise floor (pT/√Hz) at this sample.
    pub noise_floor_pt_sqrt_hz: f32,
    /// Sensor temperature (K). Default 295.
    pub temperature_k: f32,
}

impl MagFrame {
    /// Construct a zero-filled frame at room temperature for the given sensor.
    pub fn empty(sensor_id: u16) -> Self {
        Self {
            flags: 0,
            sensor_id,
            t_us: 0,
            b_pt: [0.0; 3],
            sigma_pt: [0.0; 3],
            noise_floor_pt_sqrt_hz: 0.0,
            temperature_k: 295.0,
        }
    }

    /// True iff `flag_bit` is set in `self.flags`.
    #[inline]
    pub fn has_flag(&self, flag_bit: u16) -> bool {
        self.flags & flag_bit != 0
    }

    /// Set `flag_bit` in `self.flags`.
    #[inline]
    pub fn set_flag(&mut self, flag_bit: u16) {
        self.flags |= flag_bit;
    }

    /// Serialise to the fixed-layout 60-byte buffer.
    pub fn to_bytes(&self) -> [u8; MAG_FRAME_BYTES] {
        let mut buf = [0u8; MAG_FRAME_BYTES];
        buf[0..4].copy_from_slice(&MAG_FRAME_MAGIC.to_le_bytes());
        buf[4..6].copy_from_slice(&MAG_FRAME_VERSION.to_le_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_le_bytes());
        buf[8..10].copy_from_slice(&self.sensor_id.to_le_bytes());
        // [10..12] reserved, stays zero.
        buf[12..20].copy_from_slice(&self.t_us.to_le_bytes());
        buf[20..24].copy_from_slice(&self.b_pt[0].to_le_bytes());
        buf[24..28].copy_from_slice(&self.b_pt[1].to_le_bytes());
        buf[28..32].copy_from_slice(&self.b_pt[2].to_le_bytes());
        buf[32..36].copy_from_slice(&self.sigma_pt[0].to_le_bytes());
        buf[36..40].copy_from_slice(&self.sigma_pt[1].to_le_bytes());
        buf[40..44].copy_from_slice(&self.sigma_pt[2].to_le_bytes());
        buf[44..48].copy_from_slice(&self.noise_floor_pt_sqrt_hz.to_le_bytes());
        buf[48..52].copy_from_slice(&self.temperature_k.to_le_bytes());
        // [52..60] padding stays zero.
        buf
    }

    /// Deserialise from a byte buffer. Validates magic, version, and
    /// length; rejects any payload that doesn't match v1's exact 60-byte
    /// shape with a typed [`crate::NvsimError`].
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::NvsimError> {
        if buf.len() != MAG_FRAME_BYTES {
            return Err(crate::NvsimError::FrameLengthMismatch {
                got: buf.len(),
                expected: MAG_FRAME_BYTES,
            });
        }
        let magic = u32::from_le_bytes(buf[0..4].try_into().expect("4-byte slice"));
        if magic != MAG_FRAME_MAGIC {
            return Err(crate::NvsimError::MagicMismatch {
                got: magic,
                expected: MAG_FRAME_MAGIC,
            });
        }
        let version = u16::from_le_bytes(buf[4..6].try_into().expect("2-byte slice"));
        if version != MAG_FRAME_VERSION {
            return Err(crate::NvsimError::UnsupportedVersion {
                got: version,
                supported: MAG_FRAME_VERSION,
            });
        }
        let flags = u16::from_le_bytes(buf[6..8].try_into().expect("2-byte slice"));
        let sensor_id = u16::from_le_bytes(buf[8..10].try_into().expect("2-byte slice"));
        let t_us = u64::from_le_bytes(buf[12..20].try_into().expect("8-byte slice"));
        let bx = f32::from_le_bytes(buf[20..24].try_into().expect("4-byte slice"));
        let by = f32::from_le_bytes(buf[24..28].try_into().expect("4-byte slice"));
        let bz = f32::from_le_bytes(buf[28..32].try_into().expect("4-byte slice"));
        let sx = f32::from_le_bytes(buf[32..36].try_into().expect("4-byte slice"));
        let sy = f32::from_le_bytes(buf[36..40].try_into().expect("4-byte slice"));
        let sz = f32::from_le_bytes(buf[40..44].try_into().expect("4-byte slice"));
        let noise_floor = f32::from_le_bytes(buf[44..48].try_into().expect("4-byte slice"));
        let temperature = f32::from_le_bytes(buf[48..52].try_into().expect("4-byte slice"));
        Ok(Self {
            flags,
            sensor_id,
            t_us,
            b_pt: [bx, by, bz],
            sigma_pt: [sx, sy, sz],
            noise_floor_pt_sqrt_hz: noise_floor,
            temperature_k: temperature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_is_locked_to_documented_value() {
        // Plan §1.4 commits to 0xC51A_6E70. Any change must update the plan.
        assert_eq!(MAG_FRAME_MAGIC, 0xC51A_6E70);
    }

    #[test]
    fn frame_round_trip_byte_exact() {
        let mut f = MagFrame::empty(7);
        f.set_flag(flag::ADC_SATURATED);
        f.set_flag(flag::SHOT_NOISE_DISABLED);
        f.t_us = 123_456_789;
        f.b_pt = [1.5, -2.5, 3.5];
        f.sigma_pt = [0.1, 0.2, 0.3];
        f.noise_floor_pt_sqrt_hz = 100.0;
        f.temperature_k = 295.0;

        let bytes = f.to_bytes();
        assert_eq!(bytes.len(), MAG_FRAME_BYTES);
        let f2 = MagFrame::from_bytes(&bytes).unwrap();
        assert_eq!(f, f2);
        assert!(f2.has_flag(flag::ADC_SATURATED));
        assert!(f2.has_flag(flag::SHOT_NOISE_DISABLED));
        assert!(!f2.has_flag(flag::SATURATION_NEAR_FIELD));
    }

    #[test]
    fn frame_size_is_fixed_60_bytes() {
        let f = MagFrame::empty(0);
        assert_eq!(f.to_bytes().len(), 60);
    }

    #[test]
    fn frame_rejects_short_buffer() {
        let err = MagFrame::from_bytes(&[0u8; 10]).unwrap_err();
        assert!(matches!(err, crate::NvsimError::FrameLengthMismatch { .. }));
    }

    #[test]
    fn frame_rejects_bad_magic() {
        let mut bytes = MagFrame::empty(0).to_bytes();
        bytes[0..4].copy_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());
        let err = MagFrame::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, crate::NvsimError::MagicMismatch { .. }));
    }

    #[test]
    fn frame_rejects_unsupported_version() {
        let mut bytes = MagFrame::empty(0).to_bytes();
        bytes[4..6].copy_from_slice(&99_u16.to_le_bytes());
        let err = MagFrame::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, crate::NvsimError::UnsupportedVersion { got: 99, .. }));
    }

    #[test]
    fn frame_byte_order_is_deterministic() {
        // Identical input must produce identical bytes — no allocator
        // randomisation, no hashmap iteration order, no time-of-day field.
        let f = MagFrame {
            flags: 0,
            sensor_id: 42,
            t_us: 999,
            b_pt: [1.0, 2.0, 3.0],
            sigma_pt: [0.1, 0.2, 0.3],
            noise_floor_pt_sqrt_hz: 50.0,
            temperature_k: 295.0,
        };
        let bytes_a = f.to_bytes();
        let bytes_b = f.to_bytes();
        assert_eq!(bytes_a, bytes_b);
    }

    #[test]
    fn flag_helpers_set_and_check() {
        let mut f = MagFrame::empty(0);
        assert!(!f.has_flag(flag::ADC_SATURATED));
        f.set_flag(flag::ADC_SATURATED);
        assert!(f.has_flag(flag::ADC_SATURATED));
        assert!(!f.has_flag(flag::HEAVY_ATTENUATION));
    }
}
