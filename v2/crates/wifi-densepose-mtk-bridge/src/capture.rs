//! Capture files: record what the radio gave us, replay it later without radio.
//!
//! The format is newline-delimited JSON, one [`MtkRecord`] per line, with a
//! single header line first. Records are stored *as decoded* — every field the
//! transport supplied and nothing it did not — so a replay produces byte-identical
//! MTC1 frames to the live run that recorded it.
//!
//! A capture is deliberately not the same thing as an `mt76-vendor` dump: the
//! dump is MediaTek's format and is read by [`crate::dump_file`]; this is ours,
//! and it can hold records that arrived over the UDP path too.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::record::MtkRecord;
use crate::BridgeError;

pub const CAPTURE_FORMAT: &str = "ruview-mtk-capture";
/// Format tags written by earlier builds of this bridge, before it was
/// upstreamed under its current name. Same layout; still readable.
pub const LEGACY_CAPTURE_FORMATS: &[&str] = &["whitsentry-mtk-capture"];
pub const CAPTURE_VERSION: u32 = 1;

/// First line of a capture file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureHeader {
    pub format: String,
    pub version: u32,
    /// `udp` or `dump` — which transport the records came off.
    pub transport: String,
    /// Operator-supplied node name, for provenance.
    pub node: String,
    /// True when the run that produced this capture was flagged `--synthetic`.
    /// Replaying such a capture requires `--synthetic` again; see
    /// [`crate::provenance`]. Defaults to false so v1 captures still load.
    #[serde(default)]
    pub synthetic: bool,
}

impl CaptureHeader {
    pub fn new(transport: &str, node: &str, synthetic: bool) -> Self {
        Self {
            format: CAPTURE_FORMAT.to_string(),
            version: CAPTURE_VERSION,
            transport: transport.to_string(),
            node: node.to_string(),
            synthetic,
        }
    }
}

/// Streaming writer for `--record`.
pub struct CaptureWriter<W: Write> {
    inner: W,
    wrote_header: bool,
    header: CaptureHeader,
}

impl<W: Write> CaptureWriter<W> {
    pub fn new(inner: W, header: CaptureHeader) -> Self {
        Self {
            inner,
            wrote_header: false,
            header,
        }
    }

    pub fn write_record(&mut self, record: &MtkRecord) -> Result<(), BridgeError> {
        if !self.wrote_header {
            let line = serde_json::to_string(&self.header).map_err(BridgeError::Json)?;
            writeln!(self.inner, "{line}").map_err(BridgeError::Io)?;
            self.wrote_header = true;
        }
        let line = serde_json::to_string(record).map_err(BridgeError::Json)?;
        writeln!(self.inner, "{line}").map_err(BridgeError::Io)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), BridgeError> {
        self.inner.flush().map_err(BridgeError::Io)
    }
}

/// Read a capture written by [`CaptureWriter`].
pub fn read_capture<R: BufRead>(reader: R) -> Result<(CaptureHeader, Vec<MtkRecord>), BridgeError> {
    let mut lines = reader.lines();
    let first = lines
        .next()
        .transpose()
        .map_err(BridgeError::Io)?
        .ok_or(BridgeError::EmptyDump)?;
    let header: CaptureHeader = serde_json::from_str(&first).map_err(BridgeError::Json)?;
    if header.format != CAPTURE_FORMAT && !LEGACY_CAPTURE_FORMATS.contains(&header.format.as_str())
    {
        return Err(BridgeError::UnknownCaptureFormat(header.format));
    }
    if header.version != CAPTURE_VERSION {
        return Err(BridgeError::UnsupportedCaptureVersion(header.version));
    }
    let mut records = Vec::new();
    for line in lines {
        let line = line.map_err(BridgeError::Io)?;
        if line.trim().is_empty() {
            continue;
        }
        let record: MtkRecord = serde_json::from_str(&line).map_err(BridgeError::Json)?;
        record.validate()?;
        records.push(record);
    }
    if records.is_empty() {
        return Err(BridgeError::EmptyDump);
    }
    Ok((header, records))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample(rx: u16) -> MtkRecord {
        MtkRecord {
            ts: 42,
            ta: [1, 2, 3, 4, 5, 6],
            rssi: Some(-60),
            snr: Some(20),
            bw_code: Some(0),
            pri_ch_idx: 1,
            rx_mode: Some(8),
            tx_idx: 0,
            rx_idx: rx,
            chain_info: 7,
            ext_info: 0,
            pkt_sn: Some(11),
            data_i: (0..64).map(|k| k as i16).collect(),
            data_q: (0..64).map(|k| -(k as i16)).collect(),
            trimmed: false,
        }
    }

    #[test]
    fn capture_round_trips_every_field() {
        let mut buf = Vec::new();
        {
            let mut w = CaptureWriter::new(&mut buf, CaptureHeader::new("dump", "node-a", false));
            w.write_record(&sample(0)).unwrap();
            w.write_record(&sample(1)).unwrap();
            w.flush().unwrap();
        }
        let (header, records) = read_capture(Cursor::new(buf)).unwrap();
        assert_eq!(header.transport, "dump");
        assert_eq!(header.node, "node-a");
        assert!(!header.synthetic);
        assert_eq!(records, vec![sample(0), sample(1)]);
    }

    #[test]
    fn a_capture_recorded_under_synthetic_says_so() {
        let mut buf = Vec::new();
        {
            let mut w = CaptureWriter::new(&mut buf, CaptureHeader::new("udp", "n", true));
            w.write_record(&sample(0)).unwrap();
            w.flush().unwrap();
        }
        let (header, _) = read_capture(Cursor::new(buf)).unwrap();
        assert!(header.synthetic);
    }

    /// A v1 capture written before the field existed still loads, as physical.
    #[test]
    fn a_header_without_the_field_defaults_to_not_synthetic() {
        let old = "{\"format\":\"ruview-mtk-capture\",\"version\":1,\
                   \"transport\":\"udp\",\"node\":\"n\"}\n";
        let line = serde_json::to_string(&sample(0)).unwrap();
        let (header, recs) = read_capture(Cursor::new(format!("{old}{line}\n"))).unwrap();
        assert!(!header.synthetic);
        assert_eq!(recs.len(), 1);
    }

    /// Captures recorded under the bridge's earlier format tag still replay.
    #[test]
    fn a_legacy_format_tag_still_loads() {
        let old = "{\"format\":\"whitsentry-mtk-capture\",\"version\":1,\
                   \"transport\":\"dump\",\"node\":\"n\"}\n";
        let line = serde_json::to_string(&sample(0)).unwrap();
        let (header, recs) = read_capture(Cursor::new(format!("{old}{line}\n"))).unwrap();
        assert_eq!(header.transport, "dump");
        assert_eq!(recs.len(), 1);
    }

    #[test]
    fn foreign_format_is_rejected() {
        let bad =
            "{\"format\":\"something-else\",\"version\":1,\"transport\":\"udp\",\"node\":\"n\"}\n";
        assert!(matches!(
            read_capture(Cursor::new(bad)),
            Err(BridgeError::UnknownCaptureFormat(_))
        ));
    }

    #[test]
    fn future_version_is_rejected() {
        let bad = "{\"format\":\"ruview-mtk-capture\",\"version\":99,\"transport\":\"udp\",\"node\":\"n\"}\n";
        assert!(matches!(
            read_capture(Cursor::new(bad)),
            Err(BridgeError::UnsupportedCaptureVersion(99))
        ));
    }
}
