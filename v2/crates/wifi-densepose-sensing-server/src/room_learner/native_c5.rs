//! A bounded producer-format claim. It is not a hardware attestation or a
//! sensing/phase qualification. Bare legacy 106-byte payloads stay unsupported.
use wifi_densepose_hardware::{Adr018Flags, PpduType};

pub const C5_LLTF8_V1_SUFFIX_BYTES: usize = 16;
pub const C5_LLTF8_V1_GRID: &str =
    "ant1-bins53-ppdu0-c5lltf8-keep53-drop2-linear-v3";
pub const C5_LLTF53_TONES: [i16; 53] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 26, -26, -25, -24, -23, -22, -21, -20, -19, -18, -17, -16, -15, -14, -13, -12,
    -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1,
];

fn supported_5ghz_primary_mhz(mhz: u32) -> bool {
    if !(5180..=5885).contains(&mhz) || (mhz-5000)%5 != 0 {return false;}
    let channel=(mhz-5000)/5;
    ((36..=64).contains(&channel) || (100..=144).contains(&channel)) && channel%4==0
        || (149..=177).contains(&channel) && (channel-149)%4==0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeFormatId {
    LegacyV2,
    C5Lltf8V1,
}

/// Private fields prevent callers from constructing a C5 claim without the
/// bounded decoder. The claim only identifies packing and acquisition metadata.
#[derive(Debug, Clone, Copy)]
pub struct ValidatedNativeFormat {
    id: NativeFormatId,
    first_word_invalid: bool,
    header_binding: Option<(u8, u32, u64)>,
}
impl ValidatedNativeFormat {
    pub(super) fn legacy() -> Self {
        Self { id: NativeFormatId::LegacyV2, first_word_invalid: false, header_binding: None }
    }
    pub fn id(self) -> NativeFormatId { self.id }
    pub(super) fn first_word_invalid(self) -> bool { self.first_word_invalid }
    pub(super) fn header_matches(self, node: u8, sequence: u32, frequency_hz: u64) -> bool {
        self.header_binding == Some((node, sequence, frequency_hz))
    }
}

/// Decode the exact C5L8/v1 suffix after the declared ADR-018 IQ bytes. No
/// unknown/trailing metadata is accepted for managed input. Original ADR-018
/// live ingestion is a separate caller and is not modified by this decoder.
pub fn validate_adr018_native_format(
    packet: &[u8], antennas: usize, bins: usize, ppdu: PpduType, flags: Adr018Flags,
) -> Result<ValidatedNativeFormat, &'static str> {
    if packet.len() < 20 || packet[..4] != 0xC5110001u32.to_le_bytes()
        || usize::from(packet[5]) != antennas
        || usize::from(u16::from_le_bytes([packet[6],packet[7]])) != bins
        || PpduType::from_byte(packet[18]) != ppdu
        || Adr018Flags::from_byte(packet[19]) != flags {
        return Err("native format inputs contradict the original ADR-018 header");
    }
    let payload = antennas.checked_mul(bins).and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(20)).ok_or("native payload extent overflow")?;
    if packet.len() == payload {
        if bins == 53 {
            return Err("C5 53-slot native input requires an explicit verified 8-bit C5L8/v1 format claim; legacy 106 bytes may be 12-bit packed");
        }
        return Ok(ValidatedNativeFormat::legacy());
    }
    let suffix = packet.get(payload..).ok_or("native payload is truncated")?;
    if suffix.len() != C5_LLTF8_V1_SUFFIX_BYTES || &suffix[..4] != b"C5L8" {
        return Err("native payload has unknown or ambiguous trailing metadata");
    }
    if antennas != 1 || bins != 53 || ppdu != PpduType::HtLegacy || flags.bw40 {
        return Err("C5L8/v1 requires one antenna, 53 native slots, non-HT OFDM and 20 MHz");
    }
    if packet[19] & !0x20 != 0 {
        return Err("C5L8/v1 contains unsupported ADR-018 flag variants");
    }
    let frequency_mhz=u32::from_le_bytes(packet[8..12].try_into().expect("bounded header"));
    if !supported_5ghz_primary_mhz(frequency_mhz) {
        return Err("C5L8/v1 is limited to the explicit 5 GHz producer profile");
    }
    // Version, component bits/order, tone order, SDK readback, raw BB format
    // (11g/11a=1), secondary channel, STA bandwidth readback in MHz and
    // estimator validity. C5 has no per-frame cwb field.
    if suffix[4..13] != [1, 8, 1, 1, 1, 1, 0, 20, 1] {
        return Err("C5L8/v1 packing, SDK readback or original PHY validity metadata is unsupported");
    }
    if suffix[13] > 1 || u16::from_le_bytes([suffix[14], suffix[15]]) != 106 {
        return Err("C5L8/v1 invalid-prefix or byte-count metadata is malformed");
    }
    let first_word_invalid = suffix[13] == 1;
    if first_word_invalid != flags.first_word_sanitized {
        return Err("C5L8/v1 invalid-prefix metadata contradicts ADR-018 sanitation");
    }
    if first_word_invalid && packet.get(20..24) != Some(&[0, 0, 0, 0]) {
        return Err("C5L8/v1 sanitized prefix bytes are not zero");
    }
    Ok(ValidatedNativeFormat { id: NativeFormatId::C5Lltf8V1, first_word_invalid,
        header_binding: Some((packet[4],u32::from_le_bytes(packet[12..16].try_into().expect("bounded header")),u64::from(frequency_mhz)*1_000_000)) })
}
