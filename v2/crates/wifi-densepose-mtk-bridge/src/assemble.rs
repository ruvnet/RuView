//! Assemble per-chain [`MtkRecord`]s into ADR-267 MTC1 [`CsiFrame`]s.
//!
//! A vendor CSI report covers **one (tx_idx, rx_idx) chain of one PPDU**
//! (`mt76-vendor.h:238-239`), while an MTC1 frame carries the whole
//! `tx × rx × subcarrier` cube plus one RSSI per Rx chain. Several records are
//! therefore folded into one frame.
//!
//! Provenance rules (ADR-267). `CALIBRATED` is never set — nothing reaching this
//! assembler has been through a calibration. `SYNTHETIC` is set only when
//! [`AssemblerConfig::synthetic`] says so, which happens when the operator
//! passes `--synthetic` or the replay input declares itself synthetic (see
//! [`crate::provenance`]); frames off the live radio path leave it clear.
//! `TIME_SYNCHRONIZED` is set only when the operator asserts a disciplined
//! clock. Incomplete groups are dropped, not zero-filled.

use std::collections::BTreeMap;

use wifi_densepose_hardware::mediatek_csi::{
    ChipsetProfile, CsiFlags, CsiFrame, CsiPayload, ReportKind,
};

use crate::record::{MtkRecord, TimestampUnwrapper};

/// Written into `rssi_dbm` and `noise_floor_dbm` when the transport carried no
/// RSSI or SNR. MTC1 has no "absent" encoding for these, so a sentinel is
/// needed; -128 dBm is far below any receiver's noise floor and so cannot be
/// mistaken for a measurement, where 0 would read as a very strong signal.
pub const DBM_NOT_REPORTED: i8 = i8::MIN;
use crate::BridgeError;

/// How records are grouped into one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// Group by `(ta, ts)` and emit once all `tx_count × rx_count` chains of
    /// that packet have arrived. Used for `mt76-vendor` dumps and replays, where
    /// both indices are present.
    ///
    /// The dimensions are configured rather than inferred: a group cannot be
    /// recognised as complete from the records alone, because the first record
    /// of a 2x2 packet is indistinguishable from the only record of a 1x1 one.
    PacketIdentity { tx_count: u8, rx_count: u8 },
    /// Emit one 1x1 frame per record, immediately.
    ///
    /// This is the only correct mode for the MtkCSIdump UDP stream. Its datagram
    /// carries no transmit index and no packet identity
    /// (`motion_detector.h:18-23`), and its sender loop is **antenna-major**:
    /// every packet of antenna 0, then every packet of antenna 1, and so on
    /// (`motion_detector.cpp`, `for (i = 0; i < ANTENNA_NUM; i++)` outside the
    /// per-packet send). `packet_count` is always 1 (`data.size()` of a
    /// one-element vector) and `timestamp` is taken per datagram at send time, so
    /// neither field identifies a PPDU and chains of one PPDU arrive far apart
    /// with different timestamps.
    ///
    /// Correlating them by arrival position would desynchronise silently on any
    /// UDP loss and splice chains from unrelated PPDUs into one frame, which is
    /// worse than reporting what each datagram actually is: one chain of one
    /// packet. Per-chain MIMO structure is available on the dump-file path, which
    /// carries real `tx_idx`/`rx_idx`.
    PerRecord,
    /// Accumulate per transmitter address and close each run on the driver's own
    /// **last-chain** marker, `chain_info & BIT(15)`
    /// (`1001-…-csi-implement-csi-support.patch:450`). Dimensions come from the
    /// chains the run actually contained, so mixed traffic works without
    /// configuration: an HT PPDU closes as 2x2 and a legacy single-stream PPDU
    /// closes as 1x2 on the same radio.
    ///
    /// This replaced grouping by `(ta, ts)` and a fixed rectangle, which was
    /// wrong twice over. `ts` is not a PPDU identity — real captures put 4 to 112
    /// records in one `(ta, ts)` bucket — and a fixed 2x2 can never close the
    /// 1x2 groups a legacy client produces. On a router whose client sent a mix,
    /// that discarded 30.4% of records (15598 of 51346) across 7678 incomplete
    /// groups; the two clients sharing that radio also interleaved, which
    /// per-address buckets fix.
    ///
    /// `fallback_tx`/`fallback_rx` cover an input whose `chain_info` never sets
    /// BIT(15) at all: until one is seen, a bucket that fills the fallback
    /// rectangle closes as one. Once any marker is seen it is trusted from then
    /// on, so a healthy input is never cut short.
    LastChainMarker { fallback_tx: u8, fallback_rx: u8 },
}

#[derive(Debug, Clone)]
pub struct AssemblerConfig {
    pub device_id: u64,
    pub chipset: ChipsetProfile,
    pub center_freq_khz: u32,
    /// Set `TIME_SYNCHRONIZED` only when the capture host really is disciplined
    /// (PTP/NTP-locked) against the other receivers.
    pub time_synchronized: bool,
    /// Flag every emitted frame `SYNTHETIC`. Set for fabricated inputs, so the
    /// sensing server labels them `mediatek:simulated` and they can never be
    /// mistaken for a hardware capture.
    pub synthetic: bool,
    pub group_by: GroupBy,
}

impl Default for AssemblerConfig {
    fn default() -> Self {
        Self {
            device_id: 0,
            // MT7981B + MT7976C — the Wavlink AX3000 / OpenWrt One silicon.
            chipset: ChipsetProfile::Mt7981Mt7976,
            center_freq_khz: 5_210_000,
            time_synchronized: false,
            synthetic: false,
            // MT7981B is 2x2.
            group_by: GroupBy::PacketIdentity {
                tx_count: 2,
                rx_count: 2,
            },
        }
    }
}

/// Counters describing what the assembler did and did not emit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AssemblerStats {
    pub records_in: u64,
    pub frames_out: u64,
    pub groups_dropped_incomplete: u64,
    pub pkt_sn_gaps: u64,
    /// Runs abandoned because a chain repeated before the last-chain marker
    /// arrived, i.e. the previous run's terminator was missing.
    pub runs_dropped_ragged: u64,
}

pub struct FrameAssembler {
    config: AssemblerConfig,
    pending: BTreeMap<(u16, u16), MtkRecord>,
    pending_key: Option<([u8; 6], u32)>,
    /// Per-transmitter runs, for [`GroupBy::LastChainMarker`].
    ta_runs: BTreeMap<[u8; 6], BTreeMap<(u16, u16), MtkRecord>>,
    seen_last_chain: bool,
    clock: TimestampUnwrapper,
    last_pkt_sn: Option<u32>,
    local_sequence: u32,
    stats: AssemblerStats,
}

impl FrameAssembler {
    pub fn new(config: AssemblerConfig) -> Self {
        Self {
            config,
            pending: BTreeMap::new(),
            pending_key: None,
            ta_runs: BTreeMap::new(),
            seen_last_chain: false,
            clock: TimestampUnwrapper::default(),
            last_pkt_sn: None,
            local_sequence: 0,
            stats: AssemblerStats::default(),
        }
    }

    pub fn stats(&self) -> AssemblerStats {
        self.stats
    }

    /// Feed one record. Returns a frame once a group completes.
    pub fn push(&mut self, record: MtkRecord) -> Result<Option<CsiFrame>, BridgeError> {
        record.validate()?;
        self.stats.records_in += 1;

        match self.config.group_by {
            GroupBy::PacketIdentity { tx_count, rx_count } => {
                if record.tx_idx >= tx_count as u16 || record.rx_idx >= rx_count as u16 {
                    return Err(BridgeError::ChainOutOfRange {
                        tx: record.tx_idx,
                        rx: record.rx_idx,
                        tx_count,
                        rx_count,
                    });
                }
                let key = (record.ta, record.ts);
                if self.pending_key != Some(key) && !self.pending.is_empty() {
                    // A new packet started before the previous one completed.
                    self.pending.clear();
                    self.stats.groups_dropped_incomplete += 1;
                }
                self.pending_key = Some(key);
                self.pending.insert((record.tx_idx, record.rx_idx), record);
                if self.pending.len() < (tx_count as usize) * (rx_count as usize) {
                    return Ok(None);
                }
                self.emit(tx_count, rx_count).map(Some)
            }
            GroupBy::LastChainMarker {
                fallback_tx,
                fallback_rx,
            } => self.push_last_chain(record, fallback_tx, fallback_rx),
            GroupBy::PerRecord => {
                self.pending.clear();
                self.pending_key = Some((record.ta, record.ts));
                self.pending.insert((0, 0), record);
                self.emit(1, 1).map(Some)
            }
        }
    }

    /// `chain_info & BIT(15)` — the driver's "last chain of this report" flag.
    fn is_last_chain(record: &MtkRecord) -> bool {
        record.chain_info & (1 << 15) != 0
    }

    fn push_last_chain(
        &mut self,
        record: MtkRecord,
        fallback_tx: u8,
        fallback_rx: u8,
    ) -> Result<Option<CsiFrame>, BridgeError> {
        let cap = {
            let max = self.config.chipset.max_chains() as usize;
            max * max
        };
        let ta = record.ta;
        let slot = (record.tx_idx, record.rx_idx);
        let closes = Self::is_last_chain(&record);
        if closes {
            self.seen_last_chain = true;
        }

        let run = self.ta_runs.entry(ta).or_default();
        if run.contains_key(&slot) {
            // This chain already appeared, so the previous run never received its
            // terminator. Abandon it rather than splice two PPDUs together.
            run.clear();
            self.stats.runs_dropped_ragged += 1;
        }
        run.insert(slot, record);
        let filled_fallback =
            !self.seen_last_chain && run.len() >= (fallback_tx as usize) * (fallback_rx as usize);
        if run.len() > cap {
            // Bound the buffer: a stream with no usable marker must not grow.
            run.clear();
            self.stats.runs_dropped_ragged += 1;
            return Ok(None);
        }
        if !closes && !filled_fallback {
            return Ok(None);
        }

        let records = self.ta_runs.remove(&ta).unwrap_or_default();
        self.emit_records(records)
    }

    /// Emit one frame from a completed run, deriving dimensions from the chains
    /// it contains. A run that is not a full rectangle is dropped and counted.
    fn emit_records(
        &mut self,
        records: BTreeMap<(u16, u16), MtkRecord>,
    ) -> Result<Option<CsiFrame>, BridgeError> {
        let tx_count = records.keys().map(|(t, _)| *t).max().unwrap_or(0) + 1;
        let rx_count = records.keys().map(|(_, r)| *r).max().unwrap_or(0) + 1;
        if records.len() != (tx_count as usize) * (rx_count as usize) {
            self.stats.groups_dropped_incomplete += 1;
            return Ok(None);
        }
        let dims_err = || BridgeError::ChainOutOfRange {
            tx: tx_count,
            rx: rx_count,
            tx_count: u8::MAX,
            rx_count: u8::MAX,
        };
        let tx = u8::try_from(tx_count).map_err(|_| dims_err())?;
        let rx = u8::try_from(rx_count).map_err(|_| dims_err())?;
        self.pending = records;
        self.emit(tx, rx).map(Some)
    }

    /// Drop whatever is buffered. Counts as an incomplete group if non-empty.
    pub fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.pending.clear();
            self.stats.groups_dropped_incomplete += 1;
        }
        self.pending_key = None;
    }

    fn emit(&mut self, tx_count: u8, rx_count: u8) -> Result<CsiFrame, BridgeError> {
        let records = std::mem::take(&mut self.pending);
        self.pending_key = None;

        let first = records
            .values()
            .next()
            .ok_or(BridgeError::EmptyRecord)?
            .clone();
        let bandwidth = first.bandwidth()?;
        let subcarriers = first.subcarrier_count();

        // Every chain of one PPDU must report the same grid.
        if records
            .values()
            .any(|r| r.subcarrier_count() != subcarriers)
        {
            return Err(BridgeError::RaggedGroup);
        }

        let mut values = Vec::with_capacity(tx_count as usize * rx_count as usize * subcarriers);
        let mut rssi_dbm = vec![0i8; rx_count as usize];
        let mut saturated = false;
        for tx in 0..tx_count as u16 {
            for rx in 0..rx_count as u16 {
                let rec = records
                    .get(&(tx, rx))
                    .ok_or(BridgeError::MissingChain { tx, rx })?;
                rssi_dbm[rx as usize] = rec.rssi.unwrap_or(DBM_NOT_REPORTED);
                saturated |= rec.saturated();
                for k in 0..subcarriers {
                    values.push([rec.data_i[k], rec.data_q[k]]);
                }
            }
        }

        let (sequence, dropped_predecessor) = self.next_sequence(first.pkt_sn);

        let mut flags = 0u16;
        if saturated {
            flags |= CsiFlags::SATURATED;
        }
        if self.config.time_synchronized {
            flags |= CsiFlags::TIME_SYNCHRONIZED;
        }
        if dropped_predecessor {
            flags |= CsiFlags::DROPPED_PREDECESSOR;
        }
        if self.config.synthetic {
            flags |= CsiFlags::SYNTHETIC;
        }
        // CALIBRATED is deliberately never set here.

        let frame = CsiFrame {
            report_kind: ReportKind::Csi,
            sequence,
            timestamp_us: self.clock.unwrap_ts(first.ts),
            device_id: self.config.device_id,
            chipset: self.config.chipset,
            bandwidth_mhz: bandwidth.mhz(),
            center_freq_khz: self.config.center_freq_khz,
            flags: CsiFlags(flags),
            tx_count,
            rx_count,
            ppdu_type: first.ppdu_type(),
            subcarrier_count: u16::try_from(subcarriers)
                .map_err(|_| BridgeError::UnknownSubcarrierCount(subcarriers))?,
            noise_floor_dbm: noise_floor(first.rssi, first.snr),
            // Raw firmware s16 units. Nothing here is calibrated, so the scale
            // is 1.0 rather than a made-up conversion factor.
            scale: 1.0,
            subcarrier_spacing_hz: bandwidth.subcarrier_spacing_hz(),
            calibration_id: 0,
            payload: CsiPayload::ComplexI16 { rssi_dbm, values },
        };
        frame.to_bytes().map_err(BridgeError::Mtc1)?;
        self.stats.frames_out += 1;
        Ok(frame)
    }

    /// Sequence comes from `pkt_sn` when the capture carries one; otherwise a
    /// local counter. A gap is only claimed when there is a real `pkt_sn` to
    /// compare, because no stock userspace path exposes one.
    fn next_sequence(&mut self, pkt_sn: Option<u32>) -> (u32, bool) {
        match pkt_sn {
            Some(sn) => {
                let gap = match self.last_pkt_sn {
                    Some(prev) => sn.wrapping_sub(prev) != 1,
                    None => false,
                };
                if gap {
                    self.stats.pkt_sn_gaps += 1;
                }
                self.last_pkt_sn = Some(sn);
                (sn, gap)
            }
            None => {
                let seq = self.local_sequence;
                self.local_sequence = self.local_sequence.wrapping_add(1);
                (seq, false)
            }
        }
    }
}

/// Derive a noise floor from the per-chain RSSI and SNR the vendor reports
/// (`snr` is an unsigned dB value, `csi.c:88`). With either missing there is
/// nothing to derive it from, so the sentinel is used rather than a guess.
fn noise_floor(rssi_dbm: Option<i8>, snr_db: Option<u8>) -> i8 {
    match (rssi_dbm, snr_db) {
        (Some(r), Some(s)) => (r as i32 - s as i32).clamp(i8::MIN as i32, i8::MAX as i32) as i8,
        _ => DBM_NOT_REPORTED,
    }
}
