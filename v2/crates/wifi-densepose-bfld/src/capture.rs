//! Offline capture plumbing for BFI: pcap -> radiotap -> 802.11 action frame ->
//! [`crate::cbr`] decoder, with a report-rate / gap / failure summary.
//!
//! This is the host-side ingest the BFLD front-end needs before a live capture
//! exists (ADR-365). It parses the classic pcap container itself (no libpcap
//! dependency — the crate stays dependency-light), skips the radiotap header by
//! its length field, extracts management **Action** / **Action No Ack** frame
//! bodies, and feeds each to the VHT (category 21) or HE (category 30) decoder.
//! Everything here is exercised on `SYNTHETIC` pcap bytes; it has not yet seen a
//! real capture.
//!
//! Only `LINKTYPE_IEEE802_11_RADIOTAP` (127) pcap files are handled.

#![cfg(feature = "std")]

use crate::cbr::{
    dequant_phi, dequant_psi, parse_he_action, parse_vht_action, CbrError, HeBeamform, VhtBeamform,
};

/// A decoded beamforming report of either supported format.
#[derive(Debug, Clone)]
pub enum Report {
    /// 802.11ac VHT compressed beamforming report.
    Vht(VhtBeamform),
    /// 802.11ax HE compressed beamforming report.
    He(HeBeamform),
}

impl Report {
    /// `Phi` codes, flattened `phi[sub * angles_per_sub + k]`.
    #[must_use]
    pub fn phi(&self) -> &[u16] {
        match self {
            Self::Vht(r) => &r.phi,
            Self::He(r) => &r.phi,
        }
    }

    /// `Psi` codes, same layout as [`Self::phi`].
    #[must_use]
    pub fn psi(&self) -> &[u16] {
        match self {
            Self::Vht(r) => &r.psi,
            Self::He(r) => &r.psi,
        }
    }

    /// `Phi` code bit width.
    #[must_use]
    pub const fn phi_bits(&self) -> u32 {
        match self {
            Self::Vht(r) => r.phi_bits,
            Self::He(r) => r.phi_bits,
        }
    }

    /// `Psi` code bit width.
    #[must_use]
    pub const fn psi_bits(&self) -> u32 {
        match self {
            Self::Vht(r) => r.psi_bits,
            Self::He(r) => r.psi_bits,
        }
    }

    /// Average SNR per space-time stream (raw `i8`).
    #[must_use]
    pub fn avg_snr(&self) -> &[i8] {
        match self {
            Self::Vht(r) => &r.avg_snr,
            Self::He(r) => &r.avg_snr,
        }
    }

    /// `(Nr, Nc)` feedback dimensions.
    #[must_use]
    pub const fn dims(&self) -> (u8, u8) {
        match self {
            Self::Vht(r) => (r.control.nr, r.control.nc),
            Self::He(r) => (r.control.nr, r.control.nc),
        }
    }

    /// Subcarriers carrying angles.
    #[must_use]
    pub const fn num_subcarriers(&self) -> usize {
        match self {
            Self::Vht(r) => r.num_subcarriers,
            Self::He(r) => r.num_subcarriers,
        }
    }

    /// Format label for logs and dashboards.
    #[must_use]
    pub const fn format(&self) -> &'static str {
        match self {
            Self::Vht(_) => "VHT",
            Self::He(_) => "HE",
        }
    }

    /// Dequantize a `Phi` code to radians.
    #[must_use]
    pub fn phi_radians(&self, code: u16) -> f64 {
        dequant_phi(code, self.phi_bits())
    }

    /// Dequantize a `Psi` code to radians.
    #[must_use]
    pub fn psi_radians(&self, code: u16) -> f64 {
        dequant_psi(code, self.psi_bits())
    }
}

/// pcap link-layer type for radiotap-prefixed 802.11.
pub const LINKTYPE_IEEE802_11_RADIOTAP: u32 = 127;

const PCAP_MAGIC_US: u32 = 0xa1b2_c3d4; // microsecond, host-endian
const PCAP_MAGIC_NS: u32 = 0xa1b2_3c4d; // nanosecond, host-endian

/// Errors from the pcap container layer (frame-level decode errors are counted,
/// not raised).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// Buffer too short for the pcap global header.
    ShortHeader,
    /// Magic number is not a recognized pcap file.
    BadMagic(u32),
    /// Link type is not radiotap-prefixed 802.11.
    UnsupportedLinkType(u32),
}

impl core::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ShortHeader => write!(f, "buffer shorter than pcap global header"),
            Self::BadMagic(m) => write!(f, "not a pcap file (magic {m:#010x})"),
            Self::UnsupportedLinkType(t) => {
                write!(f, "unsupported pcap link type {t} (need radiotap 127)")
            }
        }
    }
}

impl std::error::Error for CaptureError {}

/// One decoded beamforming report with its capture metadata.
#[derive(Debug, Clone)]
pub struct CapturedReport {
    /// Capture timestamp in microseconds since the Unix epoch.
    pub ts_us: u64,
    /// 802.11 sequence number (12-bit) of the frame, for gap/dup detection.
    pub sequence: u16,
    /// The decoded report (VHT or HE).
    pub report: Report,
}

/// Aggregate report-rate / integrity summary over a capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportRateSummary {
    /// Total 802.11 frames examined.
    pub frames: usize,
    /// Management action frames seen (candidates for a beamforming report).
    pub action_frames: usize,
    /// Successfully decoded VHT beamforming reports.
    pub decoded: usize,
    /// Frames that looked like a report but failed to decode.
    pub decode_failures: usize,
    /// Duplicate sequence numbers among decoded reports.
    pub sequence_dupes: usize,
    /// Missing sequence numbers among decoded reports (gaps).
    pub sequence_gaps: usize,
    /// Span between first and last decoded report, microseconds.
    pub span_us: u64,
}

impl ReportRateSummary {
    /// Decoded reports per second over the capture span (0 if under two).
    #[must_use]
    pub fn reports_per_sec(&self) -> f64 {
        if self.span_us == 0 || self.decoded < 2 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let (n, span) = ((self.decoded - 1) as f64, self.span_us as f64);
        n / (span / 1_000_000.0)
    }
}

/// Read a little-endian `u16`/`u32` from `b` at `off`, or `None` if truncated.
fn le16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn le32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Extract the beamforming Action-frame body from one radiotap+802.11 packet.
///
/// Returns `(sequence, action_body)` when the packet is a management Action /
/// Action No Ack frame, else `None`.
fn action_body(pkt: &[u8]) -> Option<(u16, &[u8])> {
    // radiotap: version(1) pad(1) it_len(2 LE); skip it_len bytes.
    let rt_len = usize::from(le16(pkt, 2)?);
    let frame = pkt.get(rt_len..)?;
    if frame.len() < 24 {
        return None;
    }
    let fc = frame[0];
    let ftype = (fc >> 2) & 0x3;
    let subtype = (fc >> 4) & 0xf;
    // type 0 = management; subtype 13 = Action, 14 = Action No Ack.
    if ftype != 0 || (subtype != 13 && subtype != 14) {
        return None;
    }
    // Sequence Control at offset 22 (after FC, dur, addr1..3); seq = high 12 bits.
    let seq = le16(frame, 22)? >> 4;
    Some((seq, &frame[24..]))
}

/// Parse a whole radiotap pcap buffer, decoding every VHT beamforming report.
///
/// Frame-level decode failures are counted in the summary, not returned as an
/// error — only container-level problems (bad magic, wrong link type) fail.
///
/// # Errors
/// Returns [`CaptureError`] when `buf` is not a radiotap pcap file.
pub fn decode_pcap(buf: &[u8]) -> Result<(Vec<CapturedReport>, ReportRateSummary), CaptureError> {
    if buf.len() < 24 {
        return Err(CaptureError::ShortHeader);
    }
    let magic = le32(buf, 0).ok_or(CaptureError::ShortHeader)?;
    let nanos = match magic {
        PCAP_MAGIC_US => false,
        PCAP_MAGIC_NS => true,
        other => return Err(CaptureError::BadMagic(other)),
    };
    let linktype = le32(buf, 20).ok_or(CaptureError::ShortHeader)?;
    if linktype != LINKTYPE_IEEE802_11_RADIOTAP {
        return Err(CaptureError::UnsupportedLinkType(linktype));
    }

    let mut reports = Vec::new();
    let mut summary = ReportRateSummary::default();
    let mut off = 24;
    while off + 16 <= buf.len() {
        let ts_hi = u64::from(le32(buf, off).unwrap_or(0));
        let ts_lo = u64::from(le32(buf, off + 4).unwrap_or(0));
        let incl = le32(buf, off + 8).unwrap_or(0) as usize;
        off += 16;
        let Some(pkt) = buf.get(off..off + incl) else { break };
        off += incl;
        summary.frames += 1;
        let ts_us = ts_hi * 1_000_000 + if nanos { ts_lo / 1000 } else { ts_lo };

        let Some((sequence, body)) = action_body(pkt) else { continue };
        summary.action_frames += 1;
        // Dispatch on the action category: 21 = VHT, 30 = HE.
        let decoded = match body.first() {
            Some(21) => parse_vht_action(body).map(Report::Vht),
            Some(30) => parse_he_action(body).map(Report::He),
            _ => continue, // some other action frame
        };
        match decoded {
            Ok(report) => {
                summary.decoded += 1;
                reports.push(CapturedReport { ts_us, sequence, report });
            }
            Err(CbrError::NotBeamforming { .. }) => {} // right category, other action
            Err(_) => summary.decode_failures += 1,
        }
    }

    summarize_sequences(&reports, &mut summary);
    Ok((reports, summary))
}

/// Fill span / gap / dup fields from the decoded reports.
fn summarize_sequences(reports: &[CapturedReport], summary: &mut ReportRateSummary) {
    if let (Some(first), Some(last)) = (reports.first(), reports.last()) {
        summary.span_us = last.ts_us.saturating_sub(first.ts_us);
    }
    for pair in reports.windows(2) {
        let (a, b) = (pair[0].sequence, pair[1].sequence);
        let step = (b.wrapping_sub(a)) & 0x0fff; // 12-bit sequence space
        if step == 0 {
            summary.sequence_dupes += 1;
        } else if step > 1 {
            summary.sequence_gaps += usize::from(step - 1).min(0x0fff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_pcap, CaptureError, LINKTYPE_IEEE802_11_RADIOTAP};

    /// Minimal VHT compressed beamforming action body (2x2/80MHz/Ng1/SU-cb0)
    /// with zeroed angles, just enough for the decoder to accept it.
    fn vht_action_body() -> Vec<u8> {
        // MIMO control: nc=2,nr=2,width=80(2),ng=0,cb=0,mu=0,token=1
        let v: u32 = (2 - 1) | ((2 - 1) << 3) | (2 << 6) | (1 << 18);
        let ctl = &v.to_le_bytes()[..3];
        let ns = 234usize; // 80MHz Ng1
        let per = 1usize; // 2x2
        let angle_bytes = (ns * per * (4 + 2)).div_ceil(8); // SU cb0: phi=4, psi=2
        let mut body = vec![21u8, 0]; // category=21 VHT, action=0
        body.extend_from_slice(ctl);
        body.extend_from_slice(&[0, 0]); // avg SNR, Nc=2
        body.extend(std::iter::repeat_n(0u8, angle_bytes));
        body
    }

    /// Wrap an action body in an 802.11 action MPDU with the given sequence.
    fn mpdu(seq: u16, body: &[u8]) -> Vec<u8> {
        let mut f = vec![0xd0u8, 0x00]; // FC: type=mgmt(0) subtype=Action(13)
        f.extend_from_slice(&[0, 0]); // duration
        f.extend_from_slice(&[0xff; 6]); // addr1
        f.extend_from_slice(&[0x02; 6]); // addr2
        f.extend_from_slice(&[0x03; 6]); // addr3
        f.extend_from_slice(&(seq << 4).to_le_bytes()); // seq control
        f.extend_from_slice(body);
        f
    }

    /// Prepend an 8-byte empty radiotap header (`it_len`=8).
    fn radiotap(frame: &[u8]) -> Vec<u8> {
        let mut p = vec![0u8, 0, 8, 0, 0, 0, 0, 0];
        p.extend_from_slice(frame);
        p
    }

    fn pcap(records: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![];
        out.extend_from_slice(&0xa1b2_c3d4u32.to_le_bytes()); // magic (us)
        out.extend_from_slice(&2u16.to_le_bytes()); // ver major
        out.extend_from_slice(&4u16.to_le_bytes()); // ver minor
        out.extend_from_slice(&[0u8; 8]); // thiszone + sigfigs
        out.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
        out.extend_from_slice(&LINKTYPE_IEEE802_11_RADIOTAP.to_le_bytes());
        for (ts_s, ts_us, pkt) in records {
            out.extend_from_slice(&ts_s.to_le_bytes());
            out.extend_from_slice(&ts_us.to_le_bytes());
            out.extend_from_slice(&(u32::try_from(pkt.len()).unwrap()).to_le_bytes());
            out.extend_from_slice(&(u32::try_from(pkt.len()).unwrap()).to_le_bytes());
            out.extend_from_slice(pkt);
        }
        out
    }

    #[test]
    fn decodes_reports_and_measures_rate() {
        let body = vht_action_body();
        // three reports at seq 1,2,4 (one gap), 100 ms apart
        let recs = vec![
            (0u32, 0u32, radiotap(&mpdu(1, &body))),
            (0, 100_000, radiotap(&mpdu(2, &body))),
            (0, 300_000, radiotap(&mpdu(4, &body))),
        ];
        let (reports, s) = decode_pcap(&pcap(&recs)).unwrap();
        assert_eq!(reports.len(), 3);
        assert_eq!(s.frames, 3);
        assert_eq!(s.action_frames, 3);
        assert_eq!(s.decoded, 3);
        assert_eq!(s.decode_failures, 0);
        assert_eq!(s.sequence_gaps, 1); // seq 3 missing
        assert_eq!(s.sequence_dupes, 0);
        assert_eq!(s.span_us, 300_000);
        // 2 intervals over 0.3 s → ~6.67 reports/s
        assert!((s.reports_per_sec() - 6.667).abs() < 0.01);
    }

    /// Minimal HE compressed beamforming action body (2x2/80MHz/SU-cb0), with
    /// 64 subcarriers of zeroed angles (6 bits each -> 48 bytes).
    fn he_action_body() -> Vec<u8> {
        // nc=2,nr=2,bw=2(80MHz),grouping=0,codebook=0,feedback=0(SU),ru 0..8
        let v: u64 = (2 - 1) | ((2 - 1) << 3) | (2 << 6) | (8 << 23);
        let mut body = vec![30u8, 0]; // category=30 HE, action=0
        body.extend_from_slice(&v.to_le_bytes()[..5]);
        body.extend_from_slice(&[0, 0]); // avg SNR, Nc=2
        body.extend(std::iter::repeat_n(0u8, 64 * 6 / 8));
        body
    }

    #[test]
    fn he_reports_flow_through_pcap() {
        let (vht, he) = (vht_action_body(), he_action_body());
        let recs = vec![
            (0u32, 0u32, radiotap(&mpdu(1, &vht))),
            (0, 50_000, radiotap(&mpdu(2, &he))),
            (0, 100_000, radiotap(&mpdu(3, &he))),
        ];
        let (reports, s) = decode_pcap(&pcap(&recs)).unwrap();
        assert_eq!(s.decoded, 3);
        assert_eq!(s.decode_failures, 0);
        let formats: Vec<&str> = reports.iter().map(|r| r.report.format()).collect();
        assert_eq!(formats, vec!["VHT", "HE", "HE"]);
        assert_eq!(reports[1].report.dims(), (2, 2));
        assert_eq!(reports[1].report.num_subcarriers(), 64);
    }

    #[test]
    fn non_beamforming_action_is_ignored_not_failed() {
        // category 5 (radio measurement), not beamforming
        let mut body = vec![5u8, 0];
        body.extend_from_slice(&[0u8; 8]);
        let recs = vec![(0u32, 0u32, radiotap(&mpdu(1, &body)))];
        let (reports, s) = decode_pcap(&pcap(&recs)).unwrap();
        assert!(reports.is_empty());
        assert_eq!(s.action_frames, 1);
        assert_eq!(s.decode_failures, 0);
    }

    #[test]
    fn rejects_non_pcap_and_wrong_linktype() {
        assert!(matches!(decode_pcap(&[0u8; 40]), Err(CaptureError::BadMagic(_))));
        assert!(matches!(decode_pcap(&[0u8; 4]), Err(CaptureError::ShortHeader)));
        let mut wrong = pcap(&[]);
        wrong[20..24].copy_from_slice(&1u32.to_le_bytes()); // LINKTYPE_ETHERNET
        assert!(matches!(decode_pcap(&wrong), Err(CaptureError::UnsupportedLinkType(1))));
    }
}
