//! Decoder for the MtkCSIdump UDP datagram.
//!
//! Wire layout, from `MtkWifiRev/MtkCSIdump`:
//!
//! ```text
//! struct CsiPacketHeader {      // motion_detector.h:18-23, __attribute__((packed))
//!     uint64_t timestamp;       // ms since UNIX epoch (system_clock)  [+0]
//!     uint32_t antenna_idx;     // rx chain index                      [+8]
//!     uint32_t packet_count;    // CSI packets in this datagram        [+12]
//!     uint32_t total_samples;   // I/Q pairs that follow               [+16]
//! };                            // 20 bytes
//! struct CsiSample {            // motion_detector.h:25-28
//!     double i;                 // 8 bytes                             [+0]
//!     double q;                 // 8 bytes                             [+8]
//! };                            // 16 bytes
//! ```
//!
//! Encoding is `memcpy` of the packed structs (`motion_detector.cpp:291-303`), so
//! it is host byte order — little-endian on MT7981 (ARMv8) and on the x86_64 /
//! aarch64 hosts that receive it. The samples are `static_cast<double>` of the
//! firmware's `s16` values (`parsers/parser_mt76.cpp:54-55`), so they are exact
//! integers and are narrowed back to `i16` here without loss.
//!
//! What this datagram does **not** carry: `rssi`, `snr`, `pkt_sn`, `ta`,
//! `tx_idx`, `ch_bw`, `rx_mode`, `chain_info`. The sender only forwards the
//! parser's trimmed I/Q vector plus the antenna index. Everything absent is
//! left absent here rather than invented; see `dump_file` for the richer path.
//!
//! The sender also trims edge subcarriers — `parsers/parser_mt76.cpp:47-48`
//! keeps bins `[2, n-1)`, i.e. `n - 3` of `n` — so `trimmed` is set and the
//! bandwidth is recovered from `len + 3`.

use crate::record::MtkRecord;
use crate::BridgeError;

/// Byte length of `struct CsiPacketHeader`.
pub const MTKCSIDUMP_HEADER_LEN: usize = 20;
/// Byte length of `struct CsiSample`.
pub const MTKCSIDUMP_SAMPLE_LEN: usize = 16;

/// The registration datagram a client sends to a CSIdump server before it will
/// receive anything (`motion_detector.cpp:73`, `csi_udp_client_gui.py:51`).
pub const REGISTER_PAYLOAD: &[u8] = b"register";

/// Decoded MtkCSIdump datagram header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatagramHeader {
    pub timestamp_ms: u64,
    pub antenna_idx: u32,
    pub packet_count: u32,
    pub total_samples: u32,
}

/// Decode one MtkCSIdump datagram into a per-chain record.
pub fn decode_datagram(buf: &[u8]) -> Result<(DatagramHeader, MtkRecord), BridgeError> {
    if buf.len() < MTKCSIDUMP_HEADER_LEN {
        return Err(BridgeError::ShortDatagram {
            needed: MTKCSIDUMP_HEADER_LEN,
            got: buf.len(),
        });
    }
    let header = DatagramHeader {
        timestamp_ms: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
        antenna_idx: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
        packet_count: u32::from_le_bytes(buf[12..16].try_into().unwrap()),
        total_samples: u32::from_le_bytes(buf[16..20].try_into().unwrap()),
    };

    let body = &buf[MTKCSIDUMP_HEADER_LEN..];
    let declared = header.total_samples as usize;
    let available = body.len() / MTKCSIDUMP_SAMPLE_LEN;
    if declared != available {
        return Err(BridgeError::SampleCountMismatch {
            declared,
            available,
        });
    }
    if declared == 0 {
        return Err(BridgeError::EmptyRecord);
    }

    let mut data_i = Vec::with_capacity(declared);
    let mut data_q = Vec::with_capacity(declared);
    for chunk in body[..declared * MTKCSIDUMP_SAMPLE_LEN].chunks_exact(MTKCSIDUMP_SAMPLE_LEN) {
        let i = f64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let q = f64::from_le_bytes(chunk[8..16].try_into().unwrap());
        data_i.push(narrow(i)?);
        data_q.push(narrow(q)?);
    }

    let rx_idx = u16::try_from(header.antenna_idx)
        .map_err(|_| BridgeError::AntennaIndexOutOfRange(header.antenna_idx))?;

    let record = MtkRecord {
        // The datagram timestamp is wall-clock milliseconds, not the vendor TSF.
        // Keep microsecond units by scaling, and let the assembler treat it as
        // the record clock for this transport.
        ts: (header.timestamp_ms.wrapping_mul(1_000) & 0xffff_ffff) as u32,
        ta: [0; 6],
        // This datagram carries neither, so they stay absent rather than being
        // reported as 0 dBm, which reads like a very strong real measurement.
        rssi: None,
        snr: None,
        bw_code: None,
        pri_ch_idx: 0,
        rx_mode: None,
        tx_idx: 0,
        rx_idx,
        chain_info: 0,
        ext_info: 0,
        pkt_sn: None,
        data_i,
        data_q,
        trimmed: true,
    };
    record.validate()?;
    Ok((header, record))
}

/// Narrow an exact-integer f64 sample back to the firmware's `i16`.
fn narrow(v: f64) -> Result<i16, BridgeError> {
    if !v.is_finite() || v.fract() != 0.0 {
        return Err(BridgeError::NonIntegerSample(v));
    }
    if v < i16::MIN as f64 || v > i16::MAX as f64 {
        return Err(BridgeError::SampleOutOfRange(v));
    }
    Ok(v as i16)
}

/// Build a datagram in the MtkCSIdump layout. Used by the tests and by
/// `--replay` when re-emitting a capture over UDP.
pub fn encode_datagram(header: &DatagramHeader, i: &[i16], q: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MTKCSIDUMP_HEADER_LEN + i.len() * MTKCSIDUMP_SAMPLE_LEN);
    out.extend_from_slice(&header.timestamp_ms.to_le_bytes());
    out.extend_from_slice(&header.antenna_idx.to_le_bytes());
    out.extend_from_slice(&header.packet_count.to_le_bytes());
    out.extend_from_slice(&header.total_samples.to_le_bytes());
    for (a, b) in i.iter().zip(q.iter()) {
        out.extend_from_slice(&(*a as f64).to_le_bytes());
        out.extend_from_slice(&(*b as f64).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Bandwidth;

    fn hand_built(samples: usize, antenna: u32) -> Vec<u8> {
        // Build the bytes by hand, field by field, rather than via the encoder,
        // so the test pins the layout and not our own helper.
        let mut buf = Vec::new();
        buf.extend_from_slice(&1_726_000_000_123u64.to_le_bytes()); // timestamp ms
        buf.extend_from_slice(&antenna.to_le_bytes()); // antenna_idx
        buf.extend_from_slice(&1u32.to_le_bytes()); // packet_count
        buf.extend_from_slice(&(samples as u32).to_le_bytes()); // total_samples
        for n in 0..samples {
            buf.extend_from_slice(&((n as i16) as f64).to_le_bytes());
            buf.extend_from_slice(&((-(n as i32) as i16) as f64).to_le_bytes());
        }
        buf
    }

    #[test]
    fn header_is_twenty_bytes_and_sample_is_sixteen() {
        assert_eq!(MTKCSIDUMP_HEADER_LEN, 20);
        assert_eq!(MTKCSIDUMP_SAMPLE_LEN, 16);
    }

    #[test]
    fn hand_built_bw80_datagram_decodes_to_the_expected_record() {
        // BW80 => 256 firmware bins, trimmed to 253 by parser_mt76.cpp:47-48.
        let buf = hand_built(253, 1);
        let (header, rec) = decode_datagram(&buf).unwrap();
        assert_eq!(header.antenna_idx, 1);
        assert_eq!(header.total_samples, 253);
        assert_eq!(rec.rx_idx, 1);
        assert_eq!(rec.subcarrier_count(), 253);
        assert!(rec.trimmed);
        assert_eq!(rec.bandwidth().unwrap(), Bandwidth::Bw80);
        assert_eq!(rec.data_i[0], 0);
        assert_eq!(rec.data_i[7], 7);
        assert_eq!(rec.data_q[7], -7);
        // Nothing the datagram does not carry may be fabricated.
        assert_eq!(rec.pkt_sn, None);
        assert_eq!(rec.bw_code, None);
        assert_eq!(rec.rx_mode, None);
        assert_eq!(rec.rssi, None);
        assert_eq!(rec.snr, None);
        assert_eq!(rec.ta, [0; 6]);
    }

    #[test]
    fn bw20_datagram_decodes_as_bw20() {
        let (_, rec) = decode_datagram(&hand_built(61, 0)).unwrap();
        assert_eq!(rec.bandwidth().unwrap(), Bandwidth::Bw20);
    }

    #[test]
    fn declared_sample_count_must_match_the_body() {
        let mut buf = hand_built(61, 0);
        buf.truncate(buf.len() - MTKCSIDUMP_SAMPLE_LEN);
        assert!(matches!(
            decode_datagram(&buf),
            Err(BridgeError::SampleCountMismatch {
                declared: 61,
                available: 60
            })
        ));
    }

    #[test]
    fn short_datagram_is_rejected() {
        assert!(matches!(
            decode_datagram(&[0u8; 4]),
            Err(BridgeError::ShortDatagram { .. })
        ));
    }

    #[test]
    fn decoder_never_panics_on_prefixes() {
        let buf = hand_built(61, 2);
        for end in 0..buf.len() {
            let _ = decode_datagram(&buf[..end]);
        }
    }

    /// The committed fixture is a raw datagram built byte-for-byte to the layout
    /// documented above, from the patched CSIdump source
    /// (`firmware/openwrt-wn586x3/patches/csidump-nl80211-attr-enum.patch`
    /// fixes the netlink ABI and leaves this wire format untouched). It is FABRICATED,
    /// not captured off a router — it pins the parser against the spec so a real
    /// datagram that disagrees points at the sender, not at us.
    #[test]
    fn the_committed_raw_datagram_fixture_parses_to_the_expected_record() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("FABRICATED-mtkcsidump-datagram-bw20.synthetic.bin");
        let buf = std::fs::read(&path).expect("fixture should be committed");

        // 20-byte header + 61 samples x 16 bytes.
        assert_eq!(
            buf.len(),
            MTKCSIDUMP_HEADER_LEN + 61 * MTKCSIDUMP_SAMPLE_LEN
        );
        assert_eq!(buf.len(), 996);

        let (header, rec) = decode_datagram(&buf).unwrap();
        assert_eq!(
            header,
            DatagramHeader {
                timestamp_ms: 1_726_000_000_123,
                antenna_idx: 1,
                packet_count: 1, // always 1: the sender wraps one packet per datagram
                total_samples: 61,
            }
        );
        assert_eq!(rec.rx_idx, 1);
        assert_eq!(rec.subcarrier_count(), 61);
        assert!(rec.trimmed);
        // 61 carried bins => 64 firmware bins => BW20.
        assert_eq!(rec.bandwidth().unwrap(), Bandwidth::Bw20);
        // Samples are f64 in the datagram, narrowed back to the firmware's i16.
        assert_eq!(rec.data_i[0], -30);
        assert_eq!(rec.data_q[0], 60);
        assert_eq!(rec.data_i[30], 0);
        // Everything this transport does not carry stays absent.
        assert_eq!(rec.rssi, None);
        assert_eq!(rec.snr, None);
        assert_eq!(rec.bw_code, None);
        assert_eq!(rec.rx_mode, None);
        assert_eq!(rec.pkt_sn, None);
        assert_eq!(rec.ta, [0; 6]);
    }

    #[test]
    fn round_trips_through_the_encoder() {
        let buf = hand_built(253, 2);
        let (header, rec) = decode_datagram(&buf).unwrap();
        let re = encode_datagram(&header, &rec.data_i, &rec.data_q);
        assert_eq!(re, buf);
    }
}
