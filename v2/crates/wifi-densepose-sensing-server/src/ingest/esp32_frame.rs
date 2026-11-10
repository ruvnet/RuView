//! ADR-018 ESP32 CSI binary frame structure and parser.
//!
//! Complies with ADR-018 (magic `0xC511_0001`), ADR-110 (byte 18 PPDU type,
//! byte 19 IEEE 802.15.4 mesh-time flags), and issue #1005 / #1009 (u16 LE subcarriers).

/// ADR-018 ESP32 CSI binary frame header (20 bytes) and payload.
#[derive(Debug, Clone)]
pub struct Esp32Frame {
    pub magic: u32,
    pub node_id: u8,
    pub n_antennas: u8,
    /// u16 since ADR-110 / issue #1005: ESP32-C6 HE-SU frames carry 256
    /// subcarrier bins (242 active HE20 tones). HT frames stay ≤128.
    pub n_subcarriers: u16,
    pub freq_mhz: u16,
    pub sequence: u32,
    pub rssi: i8,
    pub noise_floor: i8,
    /// ADR-110 byte 18: PPDU type the CSI was sampled from. Pre-ADR-110
    /// firmware sends 0 ⇒ `PpduType::HtLegacy`.
    pub ppdu_type: wifi_densepose_hardware::PpduType,
    /// ADR-110 byte 19 metadata, including whether this frame was captured
    /// while the node had a valid IEEE 802.15.4 mesh-time solution.
    pub adr018_flags: wifi_densepose_hardware::Adr018Flags,
    pub amplitudes: Vec<f64>,
    pub phases: Vec<f64>,
}

impl Esp32Frame {
    /// The `(n_subcarriers, ppdu_type)` symbol-grid identity of this frame.
    /// HT-LTF and HE-LTF grids are not bin-comparable (ADR-110 / #1005).
    #[inline]
    pub fn grid(&self) -> (u16, wifi_densepose_hardware::PpduType) {
        (self.n_subcarriers, self.ppdu_type)
    }
}

/// Parse an ADR-018 ESP32 CSI UDP frame (magic `0xC511_0001`).
pub fn parse_esp32_frame(buf: &[u8]) -> Option<Esp32Frame> {
    if buf.len() < 20 {
        return None;
    }

    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0001 {
        return None;
    }

    // Frame layout (must match firmware csi_collector.c):
    //   [0..3]   magic (u32 LE)
    //   [4]      node_id (u8)
    //   [5]      n_antennas (u8)
    //   [6..7]   n_subcarriers (u16 LE)
    //   [8..11]  freq_mhz (u32 LE)
    //   [12..15] sequence (u32 LE)
    //   [16]     rssi (i8)
    //   [17]     noise_floor (i8)
    //   [18]     PPDU type
    //   [19]     ADR-018 flags (bit 4 = IEEE 802.15.4 sync valid)
    //   [20..]   I/Q data
    let node_id = buf[4];
    let n_antennas = buf[5];
    let n_subcarriers = u16::from_le_bytes([buf[6], buf[7]]);
    let freq_mhz =
        u16::try_from(u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]])).unwrap_or(0);
    let sequence = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let rssi_raw = buf[16] as i8;

    // Fix RSSI sign: ensure it's always negative (dBm convention).
    let rssi = if rssi_raw > 0 {
        rssi_raw.saturating_neg()
    } else {
        rssi_raw
    };
    let noise_floor = buf[17] as i8;
    let ppdu_type = wifi_densepose_hardware::PpduType::from_byte(buf[18]);
    let adr018_flags = wifi_densepose_hardware::Adr018Flags::from_byte(buf[19]);

    // A real CSI frame has at least one antenna and one subcarrier.
    if n_antennas == 0 || n_subcarriers == 0 {
        return None;
    }

    let iq_start = 20;
    let n_pairs = n_antennas as usize * n_subcarriers as usize;
    let expected_len = iq_start + n_pairs * 2;

    if buf.len() < expected_len {
        return None;
    }

    let mut amplitudes = Vec::with_capacity(n_pairs);
    let mut phases = Vec::with_capacity(n_pairs);

    for k in 0..n_pairs {
        let i_val = buf[iq_start + k * 2] as i8 as f64;
        let q_val = buf[iq_start + k * 2 + 1] as i8 as f64;
        amplitudes.push((i_val * i_val + q_val * q_val).sqrt());
        phases.push(q_val.atan2(i_val));
    }

    Some(Esp32Frame {
        magic,
        node_id,
        n_antennas,
        n_subcarriers,
        freq_mhz,
        sequence,
        rssi,
        noise_floor,
        ppdu_type,
        adr018_flags,
        amplitudes,
        phases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_csi_frame(n_subcarriers: u16) -> Vec<u8> {
        let mut buf = vec![0u8; 20 + n_subcarriers as usize * 2];
        buf[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        buf[4] = 7;
        buf[5] = 1;
        buf[6..8].copy_from_slice(&n_subcarriers.to_le_bytes());
        buf[8..12].copy_from_slice(&5180u32.to_le_bytes());
        buf[12..16].copy_from_slice(&42u32.to_le_bytes());
        buf[16] = (-40i8) as u8;
        buf[17] = (-90i8) as u8;
        buf[18] = 0;
        buf[19] = 0x10;
        for k in 0..n_subcarriers as usize {
            buf[20 + k * 2] = (5 + (k % 40) as i8) as u8;
            buf[20 + k * 2 + 1] = (k % 30) as u8;
        }
        buf
    }

    #[test]
    fn parse_esp32_frame_he20_256_bins_not_truncated() {
        let buf = build_csi_frame(256);
        assert_eq!(buf.len(), 532);
        let frame = parse_esp32_frame(&buf).expect("256-bin HE20 frame must parse");
        assert_eq!(frame.n_subcarriers, 256);
        assert_eq!(frame.amplitudes.len(), 256);
        assert_eq!(frame.node_id, 7);
        assert_eq!(frame.rssi, -40);
        assert_eq!(frame.sequence, 42);
        assert!(frame.adr018_flags.ieee802154_sync_valid);
    }

    #[test]
    fn parse_esp32_frame_ht20_64_bins_still_parses() {
        let buf = build_csi_frame(64);
        let frame = parse_esp32_frame(&buf).expect("64-bin HT20 frame must parse");
        assert_eq!(frame.n_subcarriers, 64);
        assert_eq!(frame.amplitudes.len(), 64);
    }

    #[test]
    fn zero_antennas_is_rejected() {
        let mut buf = build_csi_frame(4);
        buf[5] = 0;
        assert!(parse_esp32_frame(&buf).is_none());
    }

    #[test]
    fn zero_subcarriers_is_rejected() {
        let mut buf = build_csi_frame(4);
        buf[6..8].copy_from_slice(&0u16.to_le_bytes());
        assert!(parse_esp32_frame(&buf).is_none());
    }

    #[test]
    fn bare_header_with_magic_is_rejected() {
        let mut buf = vec![0u8; 20];
        buf[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        assert!(parse_esp32_frame(&buf).is_none());
    }
}

