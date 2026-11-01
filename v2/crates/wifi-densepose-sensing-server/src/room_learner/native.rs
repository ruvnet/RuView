//! Transient native ESP32 I/Q plus amplitude-only canonical features.
//! Receiver timestamps and compatibility metadata never establish a source
//! measurement clock, temporal sample rate, or physical qualification.
use super::observation::{CaptureProvenance, ClockInfo, ClockKind};
use num_complex::Complex64;
use ruview_unified::frame::{CalibrationState, EvidenceLevel, FieldAxis, FrameProvenance,
    PhaseState, ProvenanceClass, RfFrameV2, SignalQuality};
use ruview_unified::RfModality;
use serde::Serialize;
use wifi_densepose_hardware::{Adr018Flags, PpduType};
use wifi_densepose_signal::hardware_norm::HardwareNormalizer;

pub const CANONICAL_PACKET_BINS: usize = 56;

/// Parsed ADR-018 metadata and ORIGINAL payload bytes, excluding the header.
/// Integer-Hz input makes NaN/Inf unrepresentable at this boundary.
pub struct NativePacketInput<'a> {
    pub node_id: u8,
    pub n_antennas: usize,
    pub raw_bins: usize,
    pub frequency_hz: u64,
    pub ppdu_type: PpduType,
    pub sequence: u32,
    pub received_at_ns: u64,
    /// Actual measured receiver/grid cadence, never a source sample-rate claim.
    pub receiver_sample_rate_hz: f64,
    pub iq_bytes: &'a [u8],
    pub flags: Adr018Flags,
    pub provenance: CaptureProvenance,
}

/// Contains raw native samples and deliberately has no Serialize implementation.
/// Only `summary()` is appropriate for public/status export.
///
/// ```compile_fail
/// use wifi_densepose_sensing_server::room_learner::native::CanonicalPacket;
/// fn export(packet: &CanonicalPacket) { let _ = serde_json::to_string(packet); }
/// ```
pub struct CanonicalPacket {
    pub native_frame: RfFrameV2,
    pub source_id: u8,
    pub grid_id: String,
    pub sequence: u32,
    pub frequency_hz: u64,
    pub bandwidth_hz: u64,
    pub amplitudes: Vec<f64>,
    pub clock: ClockInfo,
    pub phase_consumed: bool,
    pub receiver_sample_rate_hz: f64,
    pub temporal_sample_rate_known: bool,
    pub signal_quality_known: bool,
    pub provenance: CaptureProvenance,
}

/// Bounded counts/metadata, never native IQ or derived amplitude rows.
#[derive(Debug, Serialize)]
pub struct NativePacketSummary<'a> {
    pub source_id: u8,
    pub grid_id: &'a str,
    pub raw_bins: usize,
    pub canonical_bins: usize,
    pub frequency_hz: u64,
    pub bandwidth_hz: u64,
    pub clock: ClockInfo,
    pub phase_consumed: bool,
    pub receiver_sample_rate_hz: f64,
    pub temporal_sample_rate_known: bool,
    pub signal_quality_known: bool,
    pub provenance: CaptureProvenance,
}
impl CanonicalPacket {
    pub fn summary(&self) -> NativePacketSummary<'_> {
        NativePacketSummary { source_id: self.source_id, grid_id: &self.grid_id,
            raw_bins: self.native_frame.native_shape[1], canonical_bins: self.amplitudes.len(),
            frequency_hz: self.frequency_hz, bandwidth_hz: self.bandwidth_hz, clock: self.clock,
            phase_consumed: self.phase_consumed, receiver_sample_rate_hz: self.receiver_sample_rate_hz,
            temporal_sample_rate_known: self.temporal_sample_rate_known,
            signal_quality_known: self.signal_quality_known, provenance: self.provenance }
    }
}
impl std::fmt::Debug for CanonicalPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.summary().fmt(f)
    }
}

/// Fixed projection identity binds original hardware grid plus excluded region.
pub fn grid_id(n_antennas: usize, raw_bins: usize, ppdu_type: PpduType) -> Result<String, String> {
    let supported = n_antennas == 1 && match ppdu_type {
        PpduType::HtLegacy => raw_bins == 64,
        PpduType::HeSu | PpduType::HeMu | PpduType::HeTb => raw_bins == 256,
        PpduType::Unknown => false,
    };
    if !supported { return Err("unsupported ESP32 antenna/grid/PPDU".into()); }
    Ok(format!("ant{n_antennas}-bins{raw_bins}-ppdu{}-usable-drop2-v1", ppdu_type.to_byte()))
}

/// Preserves original signed int8 complex samples; uses only the existing
/// length-only normalizer for the derived magnitude features. Never calls
/// `normalize()` or `RfFrameV2::to_canonical()` (both remove amplitude gain).
pub fn canonicalize_esp32(input: NativePacketInput<'_>) -> Result<CanonicalPacket, String> {
    let grid_id = grid_id(input.n_antennas, input.raw_bins, input.ppdu_type)?;
    if input.flags.bw40 { return Err("native capture does not support the reported 40 MHz bandwidth".into()); }
    if input.received_at_ns == 0 { return Err("native receiver time is unavailable".into()); }
    if !input.receiver_sample_rate_hz.is_finite() || input.receiver_sample_rate_hz <= 0.0 {
        return Err("measured receiver cadence is unavailable".into());
    }
    let mhz = input.frequency_hz / 1_000_000;
    if input.frequency_hz % 1_000_000 != 0
        || !((2412..=2484).contains(&mhz) || (4900..=5900).contains(&mhz) || (5925..=7125).contains(&mhz)) {
        return Err("unknown or unsupported ESP32 centre frequency".into());
    }
    let expected = input.raw_bins * 2;
    if input.iq_bytes.len() != expected {
        return Err("ESP32 native IQ payload must exactly match bounded declared grid".into());
    }
    // ADR-356 / firmware csi_collector.c: the invalid first word is FOUR
    // bytes = TWO complex int8 I/Q samples. Keep those bytes unchanged and
    // record their invalidity; every packet uses the same drop2 projection.
    let mut valid_mask = vec![true; input.raw_bins];
    if input.flags.first_word_sanitized { valid_mask[..2].fill(false); }
    let iq: Vec<_> = input.iq_bytes.chunks_exact(2).map(|pair|
        Complex64::new(f64::from(pair[0] as i8), f64::from(pair[1] as i8))).collect();
    let frame_id = (u128::from(input.received_at_ns) << 40)
        | (u128::from(input.node_id) << 32) | u128::from(input.sequence);
    let (class, evidence) = match input.provenance {
        CaptureProvenance::Synthetic => (ProvenanceClass::Synthetic, EvidenceLevel::L0Simulation),
        CaptureProvenance::CapturedReplay | CaptureProvenance::LiveReceiverObserved =>
            (ProvenanceClass::Measured, EvidenceLevel::L1CapturedReplay),
    };
    let native_frame = RfFrameV2::new(frame_id, input.received_at_ns, RfModality::WifiCsi,
        vec![FieldAxis::Antenna, FieldAxis::Frequency, FieldAxis::Time],
        input.frequency_hz as f64, 20_000_000.0,
        // This complete frame is receiver-observed. The time axis and rate
        // describe measured host reception, never a native/source clock.
        input.receiver_sample_rate_hz, vec![1, input.raw_bins, 1], iq, valid_mask, None, None, vec![], 0,
        CalibrationState { phase_state: PhaseState::Raw, gain_calibrated: false,
            clock_ppm: f64::MAX, baseline_id: None, confidence: 0.0 },
        // No RSSI/noise/loss metadata in this narrow input: neutral constructor
        // placeholders, explicitly unavailable via signal_quality_known=false.
        SignalQuality { rssi_dbm: 0.0, noise_floor_dbm: 0.0, packet_loss: 0.0, interference: 0.0 },
        FrameProvenance { class, evidence, device_id: format!("esp32-node-{}", input.node_id),
            firmware: "unreported".into(), receipt_id: frame_id }).map_err(|e| e.to_string())?;
    if native_frame.valid_mask[2..].iter().any(|valid| !valid) {
        return Err("retained native region contains an invalid sample".into());
    }
    let magnitudes: Vec<_> = native_frame.native_iq[2..].iter().map(|z| z.norm()).collect();
    let amplitudes = HardwareNormalizer::new().resample_to_canonical(&magnitudes);
    if amplitudes.len() != CANONICAL_PACKET_BINS || amplitudes.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err("length-only magnitude projection produced an invalid feature vector".into());
    }
    Ok(CanonicalPacket { native_frame, source_id: input.node_id,
        grid_id,
        sequence: input.sequence, frequency_hz: input.frequency_hz, bandwidth_hz: 20_000_000,
        amplitudes, clock: ClockInfo { kind: ClockKind::ReceiverOnly, uncertainty_ns: u64::MAX },
        phase_consumed: false, receiver_sample_rate_hz: input.receiver_sample_rate_hz, temporal_sample_rate_known: false, signal_quality_known: false,
        provenance: input.provenance })
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
