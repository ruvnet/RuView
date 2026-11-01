//! Reader for the `mt76-vendor dump csi <n> <file>` JSON capture.
//!
//! The writer is `mt76_csi_to_json` in `mediatek/mtk-openwrt-feeds`,
//! `feed/app/mt76-vendor/src/csi.c:126-200`. It emits a JSON array of records,
//! each itself an array written field by field at `csi.c:155-184`:
//!
//! ```text
//! [ ts, "aabbccddeeff", rssi, snr, data_bw, pri_ch_idx,
//!   rx_mode, tx_idx, rx_idx, chain_info, ext_info,
//!   [ i0, i1, ... ], [ q0, q1, ... ] ]
//! ```
//!
//! Two properties of that writer matter here:
//!
//! 1. It opens the file with `fopen(name, "a+")` (`csi.c:132`), so repeated
//!    dumps to the same path produce **concatenated** top-level arrays,
//!    `[...][...]`. That is not valid single-document JSON, so this reader uses
//!    a streaming deserializer and accepts any number of arrays back to back.
//! 2. `pkt_sn` is never written. The struct has it
//!    (`mt76-vendor.h:242`) and the firmware event supplies it
//!    (`1001-…-csi-implement-csi-support.patch:371,524`), but `csi.c` neither
//!    parses nor prints it. A 14th element is therefore accepted as `pkt_sn`
//!    for patched dumpers, and is absent for stock captures.
//!
//! Unlike the UDP path these records are **not** trimmed: `data_i` holds the
//! full `data_num` bins for the bandwidth.

use crate::record::MtkRecord;
use crate::BridgeError;

/// Field index of each element in a dump record (`csi.c:155-184`).
const IDX_TS: usize = 0;
const IDX_TA: usize = 1;
const IDX_RSSI: usize = 2;
const IDX_SNR: usize = 3;
const IDX_DATA_BW: usize = 4;
const IDX_PRI_CH_IDX: usize = 5;
const IDX_RX_MODE: usize = 6;
const IDX_TX_IDX: usize = 7;
const IDX_RX_IDX: usize = 8;
const IDX_CHAIN_INFO: usize = 9;
const IDX_EXT_INFO: usize = 10;
const IDX_DATA_I: usize = 11;
const IDX_DATA_Q: usize = 12;
/// Optional extension: a patched dumper may append `pkt_sn` here.
const IDX_PKT_SN: usize = 13;
const MIN_FIELDS: usize = 13;

/// What a parsed dump file looked like, for the caller to warn about.
///
/// Accepting concatenated arrays is deliberate — `mt76-vendor` opens its output
/// with `fopen(name, "a+")` (`csi.c:132`), so a file it wrote twice legitimately
/// holds two top-level arrays. But that same append is a foot-gun: a capture loop
/// that forgets to remove the file between dumps replays every earlier dump again,
/// silently multiplying records into the sensing server. That happened to us.
/// Parsing cannot tell an intentional multi-dump file from an accumulated one, so
/// it reports both signals and lets the caller say something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DumpStats {
    /// Top-level JSON arrays found. More than one means the file was appended to.
    pub arrays: usize,
    /// Records repeating a `(ta, ts, tx_idx, rx_idx)` chain already seen. A
    /// non-zero count on a capture loop almost always means the dump file was not
    /// removed between batches.
    pub duplicate_chain_records: usize,
}

/// Parse a whole `mt76-vendor` dump file, tolerating concatenated arrays.
pub fn parse_dump(text: &str) -> Result<Vec<MtkRecord>, BridgeError> {
    parse_dump_with_stats(text).map(|(records, _)| records)
}

/// As [`parse_dump`], also reporting what the file's shape implies.
pub fn parse_dump_with_stats(text: &str) -> Result<(Vec<MtkRecord>, DumpStats), BridgeError> {
    let stream = serde_json::Deserializer::from_str(text).into_iter::<serde_json::Value>();
    let mut out = Vec::new();
    let mut stats = DumpStats::default();
    let mut seen: std::collections::HashSet<([u8; 6], u32, u16, u16)> =
        std::collections::HashSet::new();
    for doc in stream {
        let doc = doc.map_err(BridgeError::Json)?;
        let arr = doc.as_array().ok_or(BridgeError::DumpNotAnArray)?;
        stats.arrays += 1;
        for entry in arr {
            let record = parse_record(entry)?;
            if !seen.insert((record.ta, record.ts, record.tx_idx, record.rx_idx)) {
                stats.duplicate_chain_records += 1;
            }
            out.push(record);
        }
    }
    if out.is_empty() {
        return Err(BridgeError::EmptyDump);
    }
    Ok((out, stats))
}

fn parse_record(value: &serde_json::Value) -> Result<MtkRecord, BridgeError> {
    let f = value.as_array().ok_or(BridgeError::DumpRecordNotAnArray)?;
    if f.len() < MIN_FIELDS {
        return Err(BridgeError::DumpRecordTooShort {
            needed: MIN_FIELDS,
            got: f.len(),
        });
    }
    let record = MtkRecord {
        ts: int_at(f, IDX_TS)? as u32,
        ta: parse_ta(f[IDX_TA].as_str().ok_or(BridgeError::DumpTaNotAString)?)?,
        rssi: Some(int_at(f, IDX_RSSI)? as i8),
        snr: Some(int_at(f, IDX_SNR)? as u8),
        bw_code: Some(int_at(f, IDX_DATA_BW)? as u8),
        pri_ch_idx: int_at(f, IDX_PRI_CH_IDX)? as u8,
        rx_mode: Some(int_at(f, IDX_RX_MODE)? as u8),
        tx_idx: int_at(f, IDX_TX_IDX)? as u16,
        rx_idx: int_at(f, IDX_RX_IDX)? as u16,
        chain_info: int_at(f, IDX_CHAIN_INFO)? as u32,
        ext_info: int_at(f, IDX_EXT_INFO)? as u32,
        pkt_sn: match f.get(IDX_PKT_SN) {
            Some(serde_json::Value::Null) | None => None,
            Some(v) => Some(as_int(v)? as u32),
        },
        data_i: int_array(&f[IDX_DATA_I])?,
        data_q: int_array(&f[IDX_DATA_Q])?,
        trimmed: false,
    };
    record.validate()?;
    Ok(record)
}

fn int_at(fields: &[serde_json::Value], idx: usize) -> Result<i64, BridgeError> {
    as_int(&fields[idx])
}

fn as_int(value: &serde_json::Value) -> Result<i64, BridgeError> {
    value.as_i64().ok_or(BridgeError::DumpFieldNotAnInteger)
}

fn int_array(value: &serde_json::Value) -> Result<Vec<i16>, BridgeError> {
    let arr = value.as_array().ok_or(BridgeError::DumpFieldNotAnArray)?;
    arr.iter()
        .map(|v| {
            let n = as_int(v)?;
            // `csi.c:112` reads the I/Q attributes with `nla_get_u16` into an
            // `s16` field and prints them with `%d`, so a stock dump holds
            // values already in i16 range. Anything outside it is a real error.
            i16::try_from(n).map_err(|_| BridgeError::SampleOutOfRange(n as f64))
        })
        .collect()
}

fn parse_ta(text: &str) -> Result<[u8; 6], BridgeError> {
    // `csi.c:156` prints exactly 12 hex digits with no separators.
    if text.len() != 12 {
        return Err(BridgeError::DumpBadTa(text.to_string()));
    }
    let mut ta = [0u8; 6];
    for (n, slot) in ta.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[n * 2..n * 2 + 2], 16)
            .map_err(|_| BridgeError::DumpBadTa(text.to_string()))?;
    }
    Ok(ta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Bandwidth;
    use wifi_densepose_hardware::mediatek_csi::PpduType;

    fn record_json(ts: u32, rx: u16, tx: u16, n: usize, pkt_sn: Option<u32>) -> String {
        let bw_code = match n {
            64 => 0,
            128 => 1,
            256 => 2,
            _ => 3,
        };
        let i: Vec<String> = (0..n).map(|k| (k as i32 - 32).to_string()).collect();
        let q: Vec<String> = (0..n).map(|k| (64 - k as i32).to_string()).collect();
        let tail = match pkt_sn {
            Some(sn) => format!(",{sn}"),
            None => String::new(),
        };
        format!(
            "[{ts},\"aabbccddeeff\",-57,23,{bw_code},1,8,{tx},{rx},32768,0,[{}],[{}]{tail}]",
            i.join(","),
            q.join(",")
        )
    }

    #[test]
    fn parses_a_stock_thirteen_field_record() {
        let text = format!("[{}]", record_json(123_456, 1, 0, 256, None));
        let recs = parse_dump(&text).unwrap();
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.ts, 123_456);
        assert_eq!(r.ta, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(r.rssi, Some(-57));
        assert_eq!(r.snr, Some(23));
        assert_eq!(r.rx_idx, 1);
        assert_eq!(r.tx_idx, 0);
        assert_eq!(r.chain_info, 32768);
        assert_eq!(r.subcarrier_count(), 256);
        assert!(!r.trimmed);
        assert_eq!(r.bandwidth().unwrap(), Bandwidth::Bw80);
        assert_eq!(r.ppdu_type(), PpduType::HeSu); // rx_mode 8 = MT_PHY_TYPE_HE_SU
        assert_eq!(r.data_i[0], -32);
        assert_eq!(r.data_q[0], 64);
        // Stock dumps carry no sequence number.
        assert_eq!(r.pkt_sn, None);
    }

    #[test]
    fn accepts_the_optional_pkt_sn_extension() {
        let text = format!("[{}]", record_json(1, 0, 0, 64, Some(4097)));
        let recs = parse_dump(&text).unwrap();
        assert_eq!(recs[0].pkt_sn, Some(4097));
        assert_eq!(recs[0].bandwidth().unwrap(), Bandwidth::Bw20);
    }

    #[test]
    fn concatenated_dumps_from_repeated_append_are_accepted() {
        // `csi.c:132` opens the file "a+", so two dumps give `[...][...]`.
        let text = format!(
            "[{}][{}]",
            record_json(1, 0, 0, 64, None),
            record_json(2, 1, 0, 64, None)
        );
        let (recs, stats) = parse_dump_with_stats(&text).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].ts, 2);
        // Accepted, but the shape is reported so the caller can warn.
        assert_eq!(stats.arrays, 2);
        assert_eq!(stats.duplicate_chain_records, 0);
    }

    /// The accumulation foot-gun: a loop that never removes the dump file replays
    /// every earlier dump again. Identical chains reappear, and that is the signal.
    #[test]
    fn an_accumulated_file_is_detected_by_repeated_chains() {
        let first = record_json(100, 0, 0, 64, None);
        let second = record_json(200, 1, 0, 64, None);
        // Batch 1 wrote [first]; batch 2 appended [first, second] to the same file.
        let text = format!("[{first}][{first},{second}]");
        let (recs, stats) = parse_dump_with_stats(&text).unwrap();
        assert_eq!(recs.len(), 3, "every record is still parsed");
        assert_eq!(stats.arrays, 2);
        assert_eq!(
            stats.duplicate_chain_records, 1,
            "the repeated chain is what reveals the accumulation"
        );
    }

    #[test]
    fn a_single_clean_dump_reports_one_array_and_no_duplicates() {
        let text = format!(
            "[{},{}]",
            record_json(1, 0, 0, 64, None),
            record_json(1, 1, 0, 64, None)
        );
        let (_, stats) = parse_dump_with_stats(&text).unwrap();
        assert_eq!(stats.arrays, 1);
        assert_eq!(stats.duplicate_chain_records, 0);
    }

    #[test]
    fn short_record_is_rejected() {
        assert!(matches!(
            parse_dump("[[1,\"aabbccddeeff\",-57]]"),
            Err(BridgeError::DumpRecordTooShort { .. })
        ));
    }

    #[test]
    fn malformed_ta_is_rejected() {
        let text = "[[1,\"zz\",-57,23,0,1,8,0,0,0,0,[1],[1]]]";
        assert!(matches!(parse_dump(text), Err(BridgeError::DumpBadTa(_))));
    }

    #[test]
    fn empty_dump_is_an_error() {
        assert!(matches!(parse_dump("[]"), Err(BridgeError::EmptyDump)));
    }
}
