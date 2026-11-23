//! Runs C5L8/v1 datagrams through the pinned RuView server validator and canonicalizer
//! (`room_learner::native`), unchanged and by path. SYNTHETIC IQ only: this is format
//! evidence, never sensing, packing or hardware qualification.
use wifi_densepose_hardware::{Adr018Flags, PpduType};
use wifi_densepose_sensing_server::room_learner::native::{
    canonicalize_esp32_with_format, validate_adr018_native_format, CanonicalPacket, NativeFormatId,
    NativePacketInput, C5_LLTF8_V1_GRID,
};
use wifi_densepose_sensing_server::room_learner::observation::CaptureProvenance;

pub fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// Parses the datagram's own ADR-018 header the way the server's UDP path does, then
/// requires the exact C5L8/v1 claim and a canonical packet on the C5 grid.
pub fn validate_c5l8(wire: &[u8]) -> Result<CanonicalPacket, String> {
    if wire.len() < 20 {
        return Err("short datagram".into());
    }
    let antennas = usize::from(wire[5]);
    let bins = usize::from(u16::from_le_bytes([wire[6], wire[7]]));
    let ppdu = PpduType::from_byte(wire[18]);
    let flags = Adr018Flags::from_byte(wire[19]);
    let format =
        validate_adr018_native_format(wire, antennas, bins, ppdu, flags).map_err(str::to_string)?;
    if format.id() != NativeFormatId::C5Lltf8V1 {
        return Err("validator did not return the C5L8/v1 format".into());
    }
    let freq_mhz = u32::from_le_bytes(wire[8..12].try_into().unwrap());
    let input = NativePacketInput {
        node_id: wire[4],
        n_antennas: antennas,
        raw_bins: bins,
        frequency_hz: u64::from(freq_mhz) * 1_000_000,
        ppdu_type: ppdu,
        sequence: u32::from_le_bytes(wire[12..16].try_into().unwrap()),
        received_at_ns: 1_234_000_000,
        receiver_sample_rate_hz: 19.36,
        iq_bytes: &wire[20..20 + antennas * bins * 2],
        flags,
        provenance: CaptureProvenance::Synthetic,
    };
    let packet = canonicalize_esp32_with_format(input, format)?;
    if packet.grid_id != C5_LLTF8_V1_GRID {
        return Err(format!("unexpected grid {}", packet.grid_id));
    }
    Ok(packet)
}
