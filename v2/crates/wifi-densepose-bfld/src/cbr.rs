//! Compressed Beamforming Report (CBR) decoder for raw 802.11 beamforming
//! feedback.
//!
//! The BFLD pipeline (ADR-118/119) currently consumes the beamforming feedback
//! only as an opaque `compressed_angle_matrix` blob. This module turns a real
//! VHT (802.11ac) Compressed Beamforming Action frame body into structured
//! `Phi`/`Psi` Givens angles, per-stream average SNR, and reported dimensions,
//! so a sensing model can learn from the actual feedback rather than a
//! pre-computed proxy.
//!
//! Scope and honesty boundary (the `MEASURED`/`CLAIMED`/`SYNTHETIC` rule):
//! - **VHT SU and MU** follow IEEE 802.11-2020 §9.4.1.51. The angle bit widths
//!   (Table 9-92) are cross-checked against the `Wi-BFI` and `WiPiCap` reference
//!   decoders, which agree; the structure is exercised by round-trip and
//!   spec-table unit tests on `SYNTHETIC` frames. Decoded angle *values* stay
//!   `CLAIMED` until a real captured frame decodes identically (e.g. matrix
//!   unitarity, as `WiPiCap` asserts) — no over-the-air capture yet.
//! - **HE** and **EHT** angle bitstreams are [`CbrError::Unsupported`].

#![cfg(feature = "std")]
// `phi`/`psi` and `nr`/`nc` are the IEEE 802.11 names for these quantities;
// renaming them for lint purposes would obscure the spec mapping.
#![allow(clippy::similar_names)]

/// Errors from CBR parsing. All are recoverable — the caller drops the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CbrError {
    /// Frame body too short for the field being read.
    Truncated {
        /// Bytes required to read the field.
        need: usize,
        /// Bytes actually present.
        have: usize,
    },
    /// Category/Action octets are not a supported beamforming report.
    NotBeamforming {
        /// Action-frame category octet.
        category: u8,
        /// Action octet within the category.
        action: u8,
    },
    /// A dimension field is outside 802.11 limits (Nr/Nc in `1..=4`, `Nc<=Nr`).
    BadDimension {
        /// Number of rows (receive chains) reported.
        nr: u8,
        /// Number of columns (space-time streams) reported.
        nc: u8,
    },
    /// A recognized-but-not-implemented format (HE/EHT angles, MU codebook 1).
    Unsupported(&'static str),
}

impl core::fmt::Display for CbrError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated { need, have } => {
                write!(f, "truncated CBR: need {need} bytes, have {have}")
            }
            Self::NotBeamforming { category, action } => {
                write!(f, "not a beamforming report: category={category} action={action}")
            }
            Self::BadDimension { nr, nc } => write!(f, "bad CBR dimension nr={nr} nc={nc}"),
            Self::Unsupported(w) => write!(f, "unsupported CBR format: {w}"),
        }
    }
}

impl std::error::Error for CbrError {}

/// Channel width reported in the VHT MIMO Control field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelWidth {
    /// 20 MHz.
    W20,
    /// 40 MHz.
    W40,
    /// 80 MHz.
    W80,
    /// 160 MHz (or 80+80).
    W160,
}

impl ChannelWidth {
    fn from_bits(v: u8) -> Self {
        match v & 0x3 {
            0 => Self::W20,
            1 => Self::W40,
            2 => Self::W80,
            _ => Self::W160,
        }
    }

    /// Nominal channel width in MHz, for logging and link metadata.
    #[must_use]
    pub fn mhz(self) -> u16 {
        match self {
            Self::W20 => 20,
            Self::W40 => 40,
            Self::W80 => 80,
            Self::W160 => 160,
        }
    }
}

/// Subcarrier grouping Ng (1, 2 or 4 subcarriers per feedback point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    /// One feedback point per subcarrier.
    Ng1,
    /// One feedback point per two subcarriers.
    Ng2,
    /// One feedback point per four subcarriers.
    Ng4,
}

impl Grouping {
    fn from_bits(v: u8) -> Result<Self, CbrError> {
        match v & 0x3 {
            0 => Ok(Self::Ng1),
            1 => Ok(Self::Ng2),
            2 => Ok(Self::Ng4),
            _ => Err(CbrError::Unsupported("reserved grouping Ng=3")),
        }
    }
}

/// Parsed VHT MIMO Control field (802.11-2020 §9.4.1.50): 24 bits / 3 octets,
/// packed least-significant-bit first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VhtMimoControl {
    /// Number of columns Nc (space-time streams fed back), `1..=4`.
    pub nc: u8,
    /// Number of rows Nr (beamformee receive chains), `1..=4`.
    pub nr: u8,
    /// Reported channel width.
    pub width: ChannelWidth,
    /// Subcarrier grouping Ng.
    pub grouping: Grouping,
    /// Codebook-information bit (selects the angle quantization widths).
    pub codebook: bool,
    /// Feedback type: `false` = SU, `true` = MU.
    pub mu: bool,
    /// Remaining feedback segments (segmentation, 802.11 §9.4.1.50).
    pub remaining_segments: u8,
    /// First-feedback-segment flag.
    pub first_segment: bool,
    /// Sounding dialog token this report answers.
    pub sounding_token: u8,
}

fn take_u8(v: u32, shift: u32, width: u32) -> u8 {
    let mask = (1u32 << width) - 1;
    u8::try_from((v >> shift) & mask).expect("field width <= 8 bits")
}

impl VhtMimoControl {
    fn parse(b: &[u8]) -> Result<Self, CbrError> {
        if b.len() < 3 {
            return Err(CbrError::Truncated { need: 3, have: b.len() });
        }
        let v = u32::from(b[0]) | (u32::from(b[1]) << 8) | (u32::from(b[2]) << 16);
        let nc = take_u8(v, 0, 3) + 1;
        let nr = take_u8(v, 3, 3) + 1;
        if !(1..=4).contains(&nr) || !(1..=4).contains(&nc) || nc > nr {
            return Err(CbrError::BadDimension { nr, nc });
        }
        Ok(Self {
            nc,
            nr,
            width: ChannelWidth::from_bits(take_u8(v, 6, 2)),
            grouping: Grouping::from_bits(take_u8(v, 8, 2))?,
            codebook: take_u8(v, 10, 1) == 1,
            mu: take_u8(v, 11, 1) == 1,
            remaining_segments: take_u8(v, 12, 3),
            first_segment: take_u8(v, 15, 1) == 1,
            sounding_token: take_u8(v, 18, 6),
        })
    }

    /// `(psi_bits, phi_bits)` per IEEE 802.11-2020 Table 9-92, cross-checked
    /// against the `Wi-BFI` and `WiPiCap` reference decoders (which agree): SU uses
    /// the smaller (2,4)/(4,6) widths, MU the larger (5,7)/(7,9).
    fn angle_bits(self) -> (u32, u32) {
        match (self.mu, self.codebook) {
            (false, false) => (2, 4), // SU codebook 0
            (false, true) => (4, 6),  // SU codebook 1
            (true, false) => (5, 7),  // MU codebook 0
            (true, true) => (7, 9),   // MU codebook 1
        }
    }
}

/// Number of `Phi` angles (equal to the number of `Psi` angles) per subcarrier
/// for an `(Nr, Nc)` feedback matrix.
///
/// From the ordered Givens decomposition (802.11-2020 §19.3.12.3.6):
/// `sum_{i=1}^{min(Nc, Nr-1)} (Nr - i)`. Verified against the standard's angle
/// table in unit tests.
#[must_use]
pub fn angles_per_subcarrier(nr: u8, nc: u8) -> usize {
    let last = core::cmp::min(nc, nr.saturating_sub(1));
    (1..=last).map(|i| usize::from(nr - i)).sum()
}

/// Number of subcarriers Ns carrying angles, from bandwidth and grouping
/// (802.11-2020 Table 9-91, VHT).
fn num_subcarriers(width: ChannelWidth, ng: Grouping) -> usize {
    use ChannelWidth::{W160, W20, W40, W80};
    use Grouping::{Ng1, Ng2, Ng4};
    // Verbatim Table 9-91; distinct (width, grouping) rows are kept separate for
    // legibility even where two happen to share a subcarrier count.
    #[allow(clippy::match_same_arms)]
    match (width, ng) {
        (W20, Ng1) => 52,
        (W20, Ng2) => 30,
        (W20, Ng4) => 16,
        (W40, Ng1) => 108,
        (W40, Ng2) => 58,
        (W40, Ng4) => 30,
        (W80, Ng1) => 234,
        (W80, Ng2) => 122,
        (W80, Ng4) => 62,
        (W160, Ng1) => 468,
        (W160, Ng2) => 244,
        (W160, Ng4) => 124,
    }
}

/// A fully decoded VHT compressed beamforming report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VhtBeamform {
    /// Parsed MIMO Control field.
    pub control: VhtMimoControl,
    /// Average SNR per space-time stream, raw `i8` (0.25 dB units per spec).
    pub avg_snr: Vec<i8>,
    /// Number of subcarriers carrying angles.
    pub num_subcarriers: usize,
    /// `Phi` (= `Psi`) angle count per subcarrier.
    pub angles_per_sub: usize,
    /// Bit width of each `Psi` code.
    pub psi_bits: u32,
    /// Bit width of each `Phi` code.
    pub phi_bits: u32,
    /// `Phi` codes, flattened as `phi[sub * angles_per_sub + k]`.
    pub phi: Vec<u16>,
    /// `Psi` codes, same layout as [`Self::phi`].
    pub psi: Vec<u16>,
}

/// Dequantize a `Phi` code: `phi = (k + 1/2) * pi / 2^(bphi-1)`, in `(0, 2*pi)`.
/// Matches the `WiPiCap` `quantized_angle_formulas` (pinned by a unit test).
#[must_use]
pub fn dequant_phi(code: u16, phi_bits: u32) -> f64 {
    (f64::from(code) + 0.5) * core::f64::consts::PI / f64::from(1u32 << (phi_bits - 1))
}

/// Dequantize a `Psi` code: `psi = (k + 1/2) * pi / 2^(bpsi+1)`, in `(0, pi/2)`.
#[must_use]
pub fn dequant_psi(code: u16, psi_bits: u32) -> f64 {
    (f64::from(code) + 0.5) * core::f64::consts::PI / f64::from(1u32 << (psi_bits + 1))
}

impl VhtBeamform {
    /// Dequantize a `Phi` code to radians, spanning `(0, 2*pi)`.
    #[must_use]
    pub fn phi_radians(&self, code: u16) -> f64 {
        dequant_phi(code, self.phi_bits)
    }

    /// Dequantize a `Psi` code to radians, spanning `(0, pi/2)`.
    #[must_use]
    pub fn psi_radians(&self, code: u16) -> f64 {
        dequant_psi(code, self.psi_bits)
    }
}

/// Least-significant-bit-first bit reader (802.11 on-air order: B0 = LSB of
/// octet 0). Reads at most 16 bits per call.
struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn read(&mut self, count: u32) -> Result<u16, CbrError> {
        let mut out = 0u16;
        for i in 0..count {
            let byte_idx = self.pos / 8;
            if byte_idx >= self.bytes.len() {
                return Err(CbrError::Truncated { need: byte_idx + 1, have: self.bytes.len() });
            }
            let one_bit = (self.bytes[byte_idx] >> (self.pos % 8)) & 1;
            out |= u16::from(one_bit) << i;
            self.pos += 1;
        }
        Ok(out)
    }
}

/// Unpack `ns` subcarriers of Givens angles from an LSB-first bitstream. Per
/// subcarrier the order is, for each level i=1..min(Nc,Nr-1): (Nr-i) Phi codes
/// then (Nr-i) Psi codes. Flattened into `phi`/`psi`; `(nr,nc)` recovers levels.
fn unpack_angles(
    stream: &[u8],
    nr: u8,
    nc: u8,
    phi_bits: u32,
    psi_bits: u32,
    ns: usize,
) -> Result<(Vec<u16>, Vec<u16>), CbrError> {
    let per = angles_per_subcarrier(nr, nc);
    let last = core::cmp::min(nc, nr.saturating_sub(1));
    let mut reader = BitReader::new(stream);
    let mut phi = Vec::with_capacity(ns * per);
    let mut psi = Vec::with_capacity(ns * per);
    for _ in 0..ns {
        for i in 1..=last {
            let level_count = nr - i;
            for _ in 0..level_count {
                phi.push(reader.read(phi_bits)?);
            }
            for _ in 0..level_count {
                psi.push(reader.read(psi_bits)?);
            }
        }
    }
    debug_assert_eq!(phi.len(), ns * per);
    Ok((phi, psi))
}

/// Parse a VHT Compressed Beamforming Action frame body.
///
/// `body` starts at the Category octet:
/// `[Category=21][VHTAction=0][MIMO ctl x3][avg SNR xNc][angle bitstream…]`.
///
/// # Errors
/// Returns [`CbrError`] when `body` is truncated, is not a VHT compressed
/// beamforming report, carries an out-of-range dimension, or uses a format that
/// is recognized but not yet decoded (MU codebook 1).
pub fn parse_vht_action(body: &[u8]) -> Result<VhtBeamform, CbrError> {
    if body.len() < 2 {
        return Err(CbrError::Truncated { need: 2, have: body.len() });
    }
    let (category, action) = (body[0], body[1]);
    // Category 21 = VHT, VHT Action 0 = Compressed Beamforming.
    if category != 21 || action != 0 {
        return Err(CbrError::NotBeamforming { category, action });
    }
    parse_vht_report(&body[2..])
}

/// Parse the VHT report starting at the MIMO Control field (no Category/Action).
///
/// # Errors
/// Same conditions as [`parse_vht_action`].
pub fn parse_vht_report(rep: &[u8]) -> Result<VhtBeamform, CbrError> {
    let control = VhtMimoControl::parse(rep)?;
    let (psi_bits, phi_bits) = control.angle_bits();
    let nc = usize::from(control.nc);

    let snr_start = 3;
    let snr_end = snr_start + nc;
    if rep.len() < snr_end {
        return Err(CbrError::Truncated { need: snr_end, have: rep.len() });
    }
    let avg_snr: Vec<i8> = rep[snr_start..snr_end].iter().map(|&b| b.cast_signed()).collect();

    let ns = num_subcarriers(control.width, control.grouping);
    let per = angles_per_subcarrier(control.nr, control.nc);
    let (phi, psi) =
        unpack_angles(&rep[snr_end..], control.nr, control.nc, phi_bits, psi_bits, ns)?;

    Ok(VhtBeamform {
        control,
        avg_snr,
        num_subcarriers: ns,
        angles_per_sub: per,
        psi_bits,
        phi_bits,
        phi,
        psi,
    })
}

// ---- HE (802.11ax) ----

/// Parsed HE MIMO Control field (802.11ax §9.4.1.63): 40 bits / 5 octets,
/// LSB-first. Field offsets confirmed against the `WiPiCap` reference decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeMimoControl {
    /// Number of columns Nc, `1..=4`.
    pub nc: u8,
    /// Number of rows Nr, `1..=4`.
    pub nr: u8,
    /// Bandwidth code (0=20,1=40,2=80,3=160/80+80).
    pub bw: u8,
    /// Grouping: false = Ng4, true = Ng16.
    pub grouping_ng16: bool,
    /// Codebook-information bit.
    pub codebook: bool,
    /// Feedback type (0=SU, 1=MU, 2=CQI).
    pub feedback_type: u8,
    /// RU start index (7-bit).
    pub ru_start: u8,
    /// RU end index (7-bit).
    pub ru_end: u8,
}

impl HeMimoControl {
    fn parse(b: &[u8]) -> Result<Self, CbrError> {
        if b.len() < 5 {
            return Err(CbrError::Truncated { need: 5, have: b.len() });
        }
        let v = u64::from(b[0])
            | (u64::from(b[1]) << 8)
            | (u64::from(b[2]) << 16)
            | (u64::from(b[3]) << 24)
            | (u64::from(b[4]) << 32);
        let take = |shift: u32, width: u32| -> u8 {
            u8::try_from((v >> shift) & ((1u64 << width) - 1)).expect("field <= 8 bits")
        };
        let nc = take(0, 3) + 1;
        let nr = take(3, 3) + 1;
        if !(1..=4).contains(&nr) || !(1..=4).contains(&nc) || nc > nr {
            return Err(CbrError::BadDimension { nr, nc });
        }
        Ok(Self {
            nc,
            nr,
            bw: take(6, 2),
            grouping_ng16: take(8, 1) == 1,
            codebook: take(9, 1) == 1,
            feedback_type: take(10, 2),
            ru_start: take(16, 7),
            ru_end: take(23, 7),
        })
    }

    /// `(psi_bits, phi_bits)`. HE SU uses the same (2,4)/(4,6) widths as VHT SU
    /// (`Wi-BFI` + `WiPiCap`). MU/CQI HE feedback is not decoded here.
    fn angle_bits(self) -> Result<(u32, u32), CbrError> {
        if self.feedback_type != 0 {
            return Err(CbrError::Unsupported("HE MU/CQI feedback not decoded"));
        }
        Ok(if self.codebook { (4, 6) } else { (2, 4) })
    }
}

/// A decoded HE compressed beamforming report.
///
/// The subcarrier count is derived from the payload length (per `WiPiCap`)
/// rather than an RU table, so it adapts
/// to the reported RU range without hardcoding 802.11ax grouping tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeBeamform {
    /// Parsed HE MIMO Control.
    pub control: HeMimoControl,
    /// Average SNR per space-time stream (raw `i8`).
    pub avg_snr: Vec<i8>,
    /// Subcarriers decoded (derived from payload length).
    pub num_subcarriers: usize,
    /// Phi (= Psi) angle count per subcarrier.
    pub angles_per_sub: usize,
    /// Psi code bit width.
    pub psi_bits: u32,
    /// Phi code bit width.
    pub phi_bits: u32,
    /// Phi codes, flattened `phi[sub * angles_per_sub + k]`.
    pub phi: Vec<u16>,
    /// Psi codes, same layout.
    pub psi: Vec<u16>,
}

/// Parse an HE Compressed Beamforming/CQI Action frame body.
///
/// `body` starts at the Category octet: `[Category=30][HEAction][MIMO ctl x5]
/// [avg SNR xNc][angle bitstream…]`.
///
/// # Errors
/// Returns [`CbrError`] when truncated, not an HE beamforming report, carrying
/// a bad dimension, or using MU/CQI feedback (not decoded).
pub fn parse_he_action(body: &[u8]) -> Result<HeBeamform, CbrError> {
    if body.len() < 2 {
        return Err(CbrError::Truncated { need: 2, have: body.len() });
    }
    let (category, action) = (body[0], body[1]);
    // Category 30 = HE; HE Action 0 = HE Compressed Beamforming/CQI.
    if category != 30 || action != 0 {
        return Err(CbrError::NotBeamforming { category, action });
    }
    parse_he_report(&body[2..])
}

/// Parse the HE report starting at the MIMO Control field (no Category/Action).
///
/// # Errors
/// Same conditions as [`parse_he_action`].
pub fn parse_he_report(rep: &[u8]) -> Result<HeBeamform, CbrError> {
    let control = HeMimoControl::parse(rep)?;
    let (psi_bits, phi_bits) = control.angle_bits()?;
    let nc = usize::from(control.nc);

    let snr_start = 5;
    let snr_end = snr_start + nc;
    if rep.len() < snr_end {
        return Err(CbrError::Truncated { need: snr_end, have: rep.len() });
    }
    let avg_snr: Vec<i8> = rep[snr_start..snr_end].iter().map(|&b| b.cast_signed()).collect();

    // Derive Ns from the angle-region length (WiPiCap approach): each subcarrier
    // consumes `per * (phi_bits + psi_bits)` bits.
    let per = angles_per_subcarrier(control.nr, control.nc);
    let per_sub_bits = per * (phi_bits + psi_bits) as usize;
    if per_sub_bits == 0 {
        return Err(CbrError::Unsupported("degenerate HE dimensions"));
    }
    let ns = (rep[snr_end..].len() * 8) / per_sub_bits;
    let (phi, psi) = unpack_angles(&rep[snr_end..], control.nr, control.nc, phi_bits, psi_bits, ns)?;

    Ok(HeBeamform {
        control,
        avg_snr,
        num_subcarriers: ns,
        angles_per_sub: per,
        psi_bits,
        phi_bits,
        phi,
        psi,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        angles_per_subcarrier, parse_he_action, parse_vht_action, CbrError, ChannelWidth, Grouping,
        HeMimoControl, VhtMimoControl,
    };

    /// Build a VHT MIMO Control (3 octets, LSB-first) from fields.
    fn mk_ctl(nc: u8, nr: u8, width: u8, ng: u8, codebook: u8, mu: u8, token: u8) -> [u8; 3] {
        let v: u32 = u32::from(nc - 1)
            | (u32::from(nr - 1) << 3)
            | (u32::from(width) << 6)
            | (u32::from(ng) << 8)
            | (u32::from(codebook) << 10)
            | (u32::from(mu) << 11)
            | (u32::from(token) << 18);
        v.to_le_bytes()[..3].try_into().expect("3 bytes")
    }

    /// LSB-first bit packer mirroring the decoder, for building test frames.
    #[derive(Default)]
    struct BitPacker {
        out: Vec<u8>,
        acc: u32,
        held: u32,
    }
    impl BitPacker {
        fn push(&mut self, value: u16, width: u32) {
            let mask = (1u32 << width) - 1;
            self.acc |= (u32::from(value) & mask) << self.held;
            self.held += width;
            while self.held >= 8 {
                self.out.push(u8::try_from(self.acc & 0xff).expect("masked"));
                self.acc >>= 8;
                self.held -= 8;
            }
        }
        fn finish(mut self) -> Vec<u8> {
            if self.held > 0 {
                self.out.push(u8::try_from(self.acc & 0xff).expect("masked"));
            }
            self.out
        }
    }

    #[test]
    fn angle_count_table_matches_spec() {
        // 802.11-2020 Table 9-92 Na/2 (Phi count) for each (Nr, Nc).
        let cases = [
            ((2, 1), 1),
            ((2, 2), 1),
            ((3, 1), 2),
            ((3, 2), 3),
            ((3, 3), 3),
            ((4, 1), 3),
            ((4, 2), 5),
            ((4, 3), 6),
            ((4, 4), 6),
        ];
        for ((nr, nc), expect) in cases {
            assert_eq!(angles_per_subcarrier(nr, nc), expect, "Nr{nr}xNc{nc}");
        }
    }

    #[test]
    fn mimo_control_roundtrip() {
        let raw = mk_ctl(2, 2, 2, 0, 0, 0, 42); // 2x2, 80MHz, Ng1, SU cb0
        let c = VhtMimoControl::parse(&raw).unwrap();
        assert_eq!(c.nc, 2);
        assert_eq!(c.nr, 2);
        assert_eq!(c.width, ChannelWidth::W80);
        assert_eq!(c.grouping, Grouping::Ng1);
        assert!(!c.codebook && !c.mu);
        assert_eq!(c.sounding_token, 42);
        assert_eq!(c.angle_bits(), (2, 4)); // SU codebook 0: (psi, phi)
    }

    #[test]
    fn angle_bits_match_reference_decoders() {
        // (mu, codebook) -> (psi, phi), per Wi-BFI + WiPiCap (they agree).
        let bits = |mu, cb| {
            let raw = mk_ctl(1, 2, 0, 0, u8::from(cb), u8::from(mu), 0);
            VhtMimoControl::parse(&raw).unwrap().angle_bits()
        };
        assert_eq!(bits(false, false), (2, 4)); // SU cb0
        assert_eq!(bits(false, true), (4, 6)); // SU cb1
        assert_eq!(bits(true, false), (5, 7)); // MU cb0
        assert_eq!(bits(true, true), (7, 9)); // MU cb1
    }

    #[test]
    fn rejects_non_beamforming() {
        assert!(matches!(
            parse_vht_action(&[0x00, 0x00, 0, 0, 0]),
            Err(CbrError::NotBeamforming { .. })
        ));
    }

    #[test]
    fn mu_codebook1_now_decodes() {
        // Previously refused as "unverified"; the reference decoders define it
        // as (psi=7, phi=9), so it must decode.
        let raw = mk_ctl(2, 2, 2, 0, 1, 1, 0); // MU + codebook1, 2x2/80MHz/Ng1
        let mut body = vec![21u8, 0];
        body.extend_from_slice(&raw);
        body.extend_from_slice(&[0, 0]); // avg SNR, Nc=2
        let ns = 234usize; // 80MHz Ng1
        body.extend(std::iter::repeat_n(0u8, ns * (7 + 9) / 8 + 1));
        let r = parse_vht_action(&body).unwrap();
        assert_eq!((r.psi_bits, r.phi_bits), (7, 9));
    }

    /// End-to-end: synthesize a 2x2/80MHz/Ng1/SU-cb0 report with known angle
    /// codes, decode it, and confirm every code and the SNR round-trips.
    #[test]
    fn vht_2x2_roundtrip() {
        let (nr, nc) = (2u8, 2u8);
        let ctl = mk_ctl(nc, nr, 2, 0, 0, 0, 7); // 80MHz Ng1
        let ns = 234usize;
        let per = angles_per_subcarrier(nr, nc); // 1
        let (psi_bits, phi_bits) = (2u32, 4u32); // SU codebook 0

        let phi_codes: Vec<u16> =
            (0..ns * per).map(|i| u16::try_from(i * 3 % (1 << phi_bits)).unwrap()).collect();
        let psi_codes: Vec<u16> =
            (0..ns * per).map(|i| u16::try_from(i * 5 % (1 << psi_bits)).unwrap()).collect();

        let mut packer = BitPacker::default();
        for s in 0..ns {
            for k in 0..per {
                packer.push(phi_codes[s * per + k], phi_bits);
            }
            for k in 0..per {
                packer.push(psi_codes[s * per + k], psi_bits);
            }
        }

        let mut body = vec![21u8, 0];
        body.extend_from_slice(&ctl);
        body.extend_from_slice(&[10i8.cast_unsigned(), (-4i8).cast_unsigned()]); // avg SNR
        body.extend_from_slice(&packer.finish());

        let r = parse_vht_action(&body).unwrap();
        assert_eq!(r.num_subcarriers, ns);
        assert_eq!(r.angles_per_sub, per);
        assert_eq!(r.avg_snr, vec![10, -4]);
        assert_eq!(r.phi, phi_codes);
        assert_eq!(r.psi, psi_codes);

        let two_pi = 2.0 * core::f64::consts::PI;
        for &c in &r.phi {
            let a = r.phi_radians(c);
            assert!(a > 0.0 && a < two_pi, "phi {a} out of (0,2pi)");
        }
        for &c in &r.psi {
            let a = r.psi_radians(c);
            assert!(a > 0.0 && a < core::f64::consts::FRAC_PI_2, "psi {a} out of (0,pi/2)");
        }
    }

    /// Build a 5-octet HE MIMO Control (LSB-first) for SU.
    fn mk_he_ctl(nc: u8, nr: u8, bw: u8, codebook: u8, ru_start: u8, ru_end: u8) -> [u8; 5] {
        let v: u64 = u64::from(nc - 1)
            | (u64::from(nr - 1) << 3)
            | (u64::from(bw) << 6)
            | (u64::from(codebook) << 9)
            // feedback_type 0 = SU at bits 10-11
            | (u64::from(ru_start) << 16)
            | (u64::from(ru_end) << 23);
        v.to_le_bytes()[..5].try_into().expect("5 bytes")
    }

    #[test]
    fn he_mimo_control_parse() {
        let raw = mk_he_ctl(2, 2, 2, 0, 0, 8); // 2x2, 80MHz, SU cb0
        let c = HeMimoControl::parse(&raw).unwrap();
        assert_eq!((c.nc, c.nr, c.bw), (2, 2, 2));
        assert!(!c.codebook);
        assert_eq!(c.feedback_type, 0);
        assert_eq!(c.ru_end, 8);
    }

    #[test]
    fn he_2x2_roundtrip_length_derived() {
        let (nr, nc) = (2u8, 2u8);
        let ctl = mk_he_ctl(nc, nr, 2, 0, 0, 8);
        let per = angles_per_subcarrier(nr, nc); // 1
        let (psi_bits, phi_bits) = (2u32, 4u32); // HE SU cb0
        let ns = 64usize; // arbitrary; decoder derives it from length
        let phi_codes: Vec<u16> = (0..ns).map(|i| u16::try_from(i % (1 << phi_bits)).unwrap()).collect();
        let psi_codes: Vec<u16> = (0..ns).map(|i| u16::try_from(i % (1 << psi_bits)).unwrap()).collect();

        let mut packer = BitPacker::default();
        for s in 0..ns {
            packer.push(phi_codes[s], phi_bits);
            packer.push(psi_codes[s], psi_bits);
        }
        // pad to a whole number of subcarriers only (ns*6 bits = ns*6/8 bytes)
        let mut body = vec![30u8, 0]; // category 30 HE, action 0
        body.extend_from_slice(&ctl);
        body.extend_from_slice(&[1i8.cast_unsigned(), 2i8.cast_unsigned()]); // avg SNR Nc=2
        body.extend_from_slice(&packer.finish());

        let r = parse_he_action(&body).unwrap();
        assert_eq!(r.angles_per_sub, per);
        assert_eq!((r.psi_bits, r.phi_bits), (2, 4));
        assert_eq!(r.avg_snr, vec![1, 2]);
        assert!(r.num_subcarriers >= ns); // length-derived, >= what we packed
        assert_eq!(&r.phi[..ns], &phi_codes[..]);
        assert_eq!(&r.psi[..ns], &psi_codes[..]);
    }

    #[test]
    fn he_mu_feedback_unsupported() {
        // feedback_type=1 (MU) at bits 10-11.
        let mut raw = mk_he_ctl(2, 2, 2, 0, 0, 8);
        raw[1] |= 1 << (10 - 8); // set bit 10
        let mut body = vec![30u8, 0];
        body.extend_from_slice(&raw);
        body.extend_from_slice(&[0u8; 32]);
        assert_eq!(parse_he_action(&body), Err(CbrError::Unsupported("HE MU/CQI feedback not decoded")));
    }

    #[test]
    fn dequant_matches_wipicap_formula() {
        // WiPiCap quantized_angle_formulas:
        //   phi = PI*a/2^(bphi-1) + PI/2^bphi
        //   psi = PI*a/2^(bpsi+1) + PI/2^(bpsi+2)
        // Our phi_radians/psi_radians must equal these for every code.
        use core::f64::consts::PI;
        let r = parse_vht_action(&{
            let mut b = vec![21u8, 0];
            b.extend_from_slice(&mk_ctl(2, 2, 2, 0, 0, 0, 0)); // SU cb0 -> phi=4, psi=2
            b.extend_from_slice(&[0, 0]);
            b.extend(std::iter::repeat_n(0u8, 234 * 6 / 8 + 1));
            b
        })
        .unwrap();
        let (bphi, bpsi) = (r.phi_bits, r.psi_bits);
        let (iphi, ipsi) = (i32::try_from(bphi).unwrap(), i32::try_from(bpsi).unwrap());
        for a in 0..(1u16 << bphi) {
            let want = PI * f64::from(a) / 2f64.powi(iphi - 1) + PI / 2f64.powi(iphi);
            assert!((r.phi_radians(a) - want).abs() < 1e-12, "phi code {a}");
        }
        for a in 0..(1u16 << bpsi) {
            let want = PI * f64::from(a) / 2f64.powi(ipsi + 1) + PI / 2f64.powi(ipsi + 2);
            assert!((r.psi_radians(a) - want).abs() < 1e-12, "psi code {a}");
        }
    }

    #[test]
    fn truncated_stream_errs() {
        let ctl = mk_ctl(2, 2, 0, 0, 0, 0, 0); // 20MHz Ng1 -> 52 subs
        let mut body = vec![21u8, 0];
        body.extend_from_slice(&ctl);
        body.extend_from_slice(&[0, 0]); // SNR
        body.extend_from_slice(&[0u8; 4]); // far too few angle bytes
        assert!(matches!(parse_vht_action(&body), Err(CbrError::Truncated { .. })));
    }
}
