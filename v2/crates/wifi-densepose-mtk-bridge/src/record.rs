//! One MediaTek `struct csi_data` report, as the *public* userspace paths expose it.
//!
//! Field set and semantics are taken from MediaTek's own sources:
//!
//! * `struct csi_data` — `mediatek/mtk-openwrt-feeds`,
//!   `feed/app/mt76-vendor/src/mt76-vendor.h:223-244`.
//! * Netlink attribute → struct mapping — same repo, `src/csi.c:87-118`.
//! * Bandwidth → subcarrier count (`CSI_BW{20,40,80,160,320}_DATA_COUNT` =
//!   64/128/256/512/1024) — `mt76-vendor.h` and
//!   `MtkWifiRev/MtkCSIdump:wifi_drv_api/mt76_api.h:16-20`.
//! * `rx_mode` values follow `enum mt76_phy_type` — `openwrt/mt76:mt76.h:334-348`.
//!
//! A record is **one (tx_idx, rx_idx) chain of one PPDU**. A full MIMO frame is
//! assembled from several of these; see [`crate::assemble`].

use serde::{Deserialize, Serialize};
use wifi_densepose_hardware::mediatek_csi::PpduType;

use crate::BridgeError;

/// Bandwidth code as carried in `MTK_VENDOR_ATTR_CSI_DATA_BW`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bandwidth {
    Bw20,
    Bw40,
    Bw80,
    Bw160,
}

impl Bandwidth {
    /// `ch_bw` / `data_bw` code as used by `ParserMT76::processRawData`
    /// (`MtkCSIdump:parsers/parser_mt76.cpp:26-32`).
    pub fn from_code(code: u8) -> Result<Self, BridgeError> {
        match code {
            0 => Ok(Self::Bw20),
            1 => Ok(Self::Bw40),
            2 => Ok(Self::Bw80),
            3 => Ok(Self::Bw160),
            other => Err(BridgeError::UnknownBandwidthCode(other)),
        }
    }

    /// Full (untrimmed) subcarrier count the firmware reports at this bandwidth.
    pub fn subcarriers(self) -> u16 {
        match self {
            Self::Bw20 => 64,
            Self::Bw40 => 128,
            Self::Bw80 => 256,
            Self::Bw160 => 512,
        }
    }

    pub fn mhz(self) -> u16 {
        match self {
            Self::Bw20 => 20,
            Self::Bw40 => 40,
            Self::Bw80 => 80,
            Self::Bw160 => 160,
        }
    }

    /// Recover the bandwidth from an *untrimmed* subcarrier array length.
    pub fn from_subcarriers(n: usize) -> Result<Self, BridgeError> {
        match n {
            64 => Ok(Self::Bw20),
            128 => Ok(Self::Bw40),
            256 => Ok(Self::Bw80),
            512 => Ok(Self::Bw160),
            other => Err(BridgeError::UnknownSubcarrierCount(other)),
        }
    }

    /// Reporting-grid subcarrier spacing. The CSI grid is always
    /// `bandwidth / subcarriers` = 312.5 kHz, independent of the PPDU format.
    pub fn subcarrier_spacing_hz(self) -> f32 {
        (self.mhz() as f32 * 1.0e6) / self.subcarriers() as f32
    }
}

/// Map `rx_mode` onto the MTC1 `PpduType`.
///
/// `rx_mode` is `enum mt76_phy_type` (`openwrt/mt76:mt76.h:334-348`). The CSI
/// patch confirms it: `mt7915_vendor_csi_tone_mask` compares it against
/// `MT_PHY_TYPE_CCK` and indexes a `mode_map` keyed by `MT_PHY_TYPE_OFDM`,
/// `_HT`, `_VHT` and `_HE_SU`
/// (`1001-…-csi-implement-csi-support.patch:1070-1088`).
///
/// That same `mode_map` gives `MT_PHY_TYPE_OFDM` its own tone-mask group, so
/// **legacy OFDM is a first-class CSI mode, not an anomaly** — the first real
/// MT7981 capture off a WN586X3 (a 2.4 GHz client on channel 6) reported exactly
/// that. Pre-HT modes therefore map to `PpduType::Legacy`, and anything
/// unrecognised maps to `PpduType::Unknown`. This function never fails: dropping
/// a frame because its PPDU format has no HT-or-later equivalent loses real
/// measurements.
pub fn ppdu_from_rx_mode(rx_mode: u8) -> PpduType {
    match rx_mode {
        // MT_PHY_TYPE_CCK (802.11b), MT_PHY_TYPE_OFDM (802.11a/g).
        0 | 1 => PpduType::Legacy,
        2 | 3 => PpduType::Ht,    // MT_PHY_TYPE_HT, _HT_GF
        4 => PpduType::Vht,       // MT_PHY_TYPE_VHT
        8..=10 => PpduType::HeSu, // HE_SU, HE_EXT_SU, HE_TB
        11 => PpduType::HeMu,     // MT_PHY_TYPE_HE_MU
        13..=15 => PpduType::Eht, // EHT_SU, EHT_TRIG, EHT_MU
        // 5..7 and 12 are gaps in the enum; anything here is a mode this build
        // does not know, which is recorded as such rather than guessed at.
        _ => PpduType::Unknown,
    }
}

/// A single per-chain vendor CSI report.
///
/// `pkt_sn` is `Option` on purpose. The firmware event carries it
/// (`1001-wifi-mt76-mt7915-csi-implement-csi-support.patch:524`, copied into
/// `struct csi_data` at patch line 371), but **neither public userspace path
/// exposes it**: `csi.c:87-118` never reads a `pkt_sn` attribute and the JSON
/// writer at `csi.c:155-184` never emits one. It is therefore `None` for stock
/// captures, and `Some` only when a patched dumper provides it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MtkRecord {
    /// Vendor `ts`, u32. Treated as MAC-TSF microseconds; wraps every ~71.6 min.
    pub ts: u32,
    /// Transmitter address of the PPDU the estimate came from.
    pub ta: [u8; 6],
    /// Per-chain RSSI in dBm, or `None` when the transport does not carry it.
    /// The MtkCSIdump UDP datagram carries no RSSI at all.
    pub rssi: Option<i8>,
    /// SNR in dB, or `None` when the transport does not carry it.
    pub snr: Option<u8>,
    /// Bandwidth code; `None` when the transport did not carry one (UDP path).
    pub bw_code: Option<u8>,
    pub pri_ch_idx: u8,
    /// `enum mt76_phy_type`; `None` when the transport did not carry one.
    pub rx_mode: Option<u8>,
    pub tx_idx: u16,
    pub rx_idx: u16,
    pub chain_info: u32,
    pub ext_info: u32,
    /// See the type-level note: absent on every stock public path.
    pub pkt_sn: Option<u32>,
    /// In-phase samples, one per reported subcarrier.
    pub data_i: Vec<i16>,
    /// Quadrature samples, same length as `data_i`.
    pub data_q: Vec<i16>,
    /// True when the transport trimmed edge subcarriers, so `data_i.len()` is
    /// smaller than the bandwidth's full count. MtkCSIdump does this at
    /// `parsers/parser_mt76.cpp:47-48` (drops bins `0`, `1` and `n-1`).
    pub trimmed: bool,
}

impl MtkRecord {
    /// Number of subcarriers actually carried.
    pub fn subcarrier_count(&self) -> usize {
        self.data_i.len()
    }

    /// Bandwidth of this report, from the explicit code when present, otherwise
    /// recovered from the array length (accounting for MtkCSIdump's 3 trimmed
    /// bins). Never guesses: an unrecognised length is an error.
    pub fn bandwidth(&self) -> Result<Bandwidth, BridgeError> {
        if let Some(code) = self.bw_code {
            return Bandwidth::from_code(code);
        }
        let full = if self.trimmed {
            self.data_i.len() + 3
        } else {
            self.data_i.len()
        };
        Bandwidth::from_subcarriers(full)
    }

    pub fn ppdu_type(&self) -> PpduType {
        match self.rx_mode {
            Some(m) => ppdu_from_rx_mode(m),
            // The MtkCSIdump UDP datagram carries no rx_mode at all, so the PPDU
            // format is genuinely not known on that transport.
            None => PpduType::Unknown,
        }
    }

    /// One-line dump of the vendor scalars in `csi.c` print order, for bring-up
    /// against real hardware. `chain_info` is shown raw: the only bit the CSI
    /// patch documents is BIT(15), "last chain" (patch line 450), so the rest is
    /// not decoded here rather than guessed at.
    pub fn describe(&self) -> String {
        let ta = self
            .ta
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let bw = match self.bandwidth() {
            Ok(b) => format!("{} MHz", b.mhz()),
            Err(_) => "?".to_string(),
        };
        format!(
            "ts={} ta={ta} rssi={:?} snr={:?} data_bw={:?} ({bw}) pri_ch_idx={} \
             rx_mode={:?} -> ppdu={:?} tx_idx={} rx_idx={} \
             chain_info={:#x} (bit15/last_chain={}) ext_info={:#x} \
             subcarriers={} trimmed={} pkt_sn={:?}",
            self.ts,
            self.rssi,
            self.snr,
            self.bw_code,
            self.pri_ch_idx,
            self.rx_mode,
            self.ppdu_type(),
            self.tx_idx,
            self.rx_idx,
            self.chain_info,
            self.chain_info & (1 << 15) != 0,
            self.ext_info,
            self.subcarrier_count(),
            self.trimmed,
            self.pkt_sn,
        )
    }

    /// True when any sample sits on an i16 rail — a real saturation indication.
    pub fn saturated(&self) -> bool {
        self.data_i
            .iter()
            .chain(self.data_q.iter())
            .any(|v| *v == i16::MAX || *v == i16::MIN)
    }

    pub fn validate(&self) -> Result<(), BridgeError> {
        if self.data_i.len() != self.data_q.len() {
            return Err(BridgeError::IqLengthMismatch {
                i: self.data_i.len(),
                q: self.data_q.len(),
            });
        }
        if self.data_i.is_empty() {
            return Err(BridgeError::EmptyRecord);
        }
        self.bandwidth()?;
        Ok(())
    }
}

/// Stable 64-bit device id derived from a node name (FNV-1a 64).
///
/// Two receivers with different `--node` names cannot collide unless the names
/// collide, and the mapping is stable across runs and hosts.
pub fn device_id_from_node(node: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in node.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Extends the vendor's 32-bit `ts` into a monotonic 64-bit microsecond clock.
#[derive(Debug, Default, Clone)]
pub struct TimestampUnwrapper {
    last: Option<u32>,
    epoch: u64,
}

impl TimestampUnwrapper {
    pub fn unwrap_ts(&mut self, ts: u32) -> u64 {
        if let Some(prev) = self.last {
            if ts < prev {
                self.epoch = self.epoch.wrapping_add(1u64 << 32);
            }
        }
        self.last = Some(ts);
        self.epoch + ts as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bandwidth_grid_is_always_312_5_khz() {
        for bw in [
            Bandwidth::Bw20,
            Bandwidth::Bw40,
            Bandwidth::Bw80,
            Bandwidth::Bw160,
        ] {
            assert_eq!(bw.subcarrier_spacing_hz(), 312_500.0, "{bw:?}");
        }
    }

    #[test]
    fn trimmed_lengths_recover_the_right_bandwidth() {
        // MtkCSIdump drops 3 bins, so 64/128/256/512 arrive as 61/125/253/509.
        for (len, want) in [
            (61, Bandwidth::Bw20),
            (125, Bandwidth::Bw40),
            (253, Bandwidth::Bw80),
            (509, Bandwidth::Bw160),
        ] {
            let rec = MtkRecord {
                ts: 0,
                ta: [0; 6],
                rssi: Some(-50),
                snr: Some(20),
                bw_code: None,
                pri_ch_idx: 0,
                rx_mode: None,
                tx_idx: 0,
                rx_idx: 0,
                chain_info: 0,
                ext_info: 0,
                pkt_sn: None,
                data_i: vec![1; len],
                data_q: vec![1; len],
                trimmed: true,
            };
            assert_eq!(rec.bandwidth().unwrap(), want, "len {len}");
        }
    }

    #[test]
    fn unrecognised_length_is_an_error_not_a_guess() {
        let rec = MtkRecord {
            ts: 0,
            ta: [0; 6],
            rssi: Some(-50),
            snr: Some(20),
            bw_code: None,
            pri_ch_idx: 0,
            rx_mode: None,
            tx_idx: 0,
            rx_idx: 0,
            chain_info: 0,
            ext_info: 0,
            pkt_sn: None,
            data_i: vec![1; 100],
            data_q: vec![1; 100],
            trimmed: true,
        };
        assert!(matches!(
            rec.bandwidth(),
            Err(BridgeError::UnknownSubcarrierCount(103))
        ));
    }

    /// The driver's own `mode_map` keys on `MT_PHY_TYPE_OFDM`, so a legacy frame
    /// is expected CSI and must be carried, not dropped. Our first real capture
    /// was exactly this case.
    #[test]
    fn pre_ht_modes_map_to_legacy_rather_than_being_dropped() {
        assert_eq!(ppdu_from_rx_mode(0), PpduType::Legacy); // CCK
        assert_eq!(ppdu_from_rx_mode(1), PpduType::Legacy); // OFDM
    }

    #[test]
    fn known_modern_modes_keep_their_own_types() {
        assert_eq!(ppdu_from_rx_mode(2), PpduType::Ht);
        assert_eq!(ppdu_from_rx_mode(3), PpduType::Ht);
        assert_eq!(ppdu_from_rx_mode(4), PpduType::Vht);
        assert_eq!(ppdu_from_rx_mode(8), PpduType::HeSu);
        assert_eq!(ppdu_from_rx_mode(11), PpduType::HeMu);
        assert_eq!(ppdu_from_rx_mode(13), PpduType::Eht);
    }

    /// Enum gaps and future values are recorded as unknown, never guessed.
    #[test]
    fn unrecognised_modes_map_to_unknown_and_never_error() {
        for m in [5u8, 6, 7, 12, 16, 200, 255] {
            assert_eq!(ppdu_from_rx_mode(m), PpduType::Unknown, "rx_mode {m}");
        }
    }

    /// The UDP transport carries no rx_mode, so the format is truly not known.
    #[test]
    fn a_record_without_rx_mode_reports_unknown() {
        let rec = MtkRecord {
            ts: 0,
            ta: [0; 6],
            rssi: Some(-50),
            snr: Some(20),
            bw_code: None,
            pri_ch_idx: 0,
            rx_mode: None,
            tx_idx: 0,
            rx_idx: 0,
            chain_info: 0,
            ext_info: 0,
            pkt_sn: None,
            data_i: vec![1; 61],
            data_q: vec![1; 61],
            trimmed: true,
        };
        assert_eq!(rec.ppdu_type(), PpduType::Unknown);
    }

    #[test]
    fn distinct_nodes_get_distinct_device_ids() {
        let a = device_id_from_node("wn586x3-livingroom");
        let b = device_id_from_node("wn586x3-kitchen");
        assert_ne!(a, b);
        assert_eq!(a, device_id_from_node("wn586x3-livingroom"));
    }

    #[test]
    fn timestamp_unwraps_across_u32_rollover() {
        let mut u = TimestampUnwrapper::default();
        assert_eq!(u.unwrap_ts(u32::MAX - 1), (u32::MAX - 1) as u64);
        assert_eq!(u.unwrap_ts(5), (1u64 << 32) + 5);
        assert_eq!(u.unwrap_ts(9), (1u64 << 32) + 9);
    }
}
