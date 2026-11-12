//! Immutable input claims and hash from Train rows only; no raw rows retained.
use crate::room_learner::observation::{
    CaptureCohort, CaptureProvenance, ClockKind, DataRole, RoomLabel,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsumedChannel {
    Magnitude,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrainingManifest {
    pub schema: String,
    pub training_provenance: Vec<CaptureProvenance>,
    pub training_data_sha256: String,
    pub consumed_channels: Vec<ConsumedChannel>,
    /// Validated input clock declarations, never an independent hardware receipt.
    pub clock_claims: Vec<ClockKind>,
    pub measurement_clock_available: bool,
    pub training_cohorts: usize,
}
fn provenance_rank(value: &CaptureProvenance) -> u8 {
    match value {
        CaptureProvenance::Synthetic => 0,
        CaptureProvenance::CapturedReplay => 1,
        CaptureProvenance::LiveReceiverObserved => 2,
    }
}
fn clock_rank(value: &ClockKind) -> u8 {
    match value {
        ClockKind::Unknown => 0,
        ClockKind::ReceiverOnly => 1,
        ClockKind::SourceMeasurement => 2,
    }
}
struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl TrainingManifest {
    pub(crate) fn from_training(rows: &[CaptureCohort]) -> Result<Self, String> {
        if !(4..=64).contains(&rows.len())
            || rows
                .iter()
                .any(|c| c.role != DataRole::Train || c.label == RoomLabel::Unknown)
        {
            return Err("training manifest requires bounded labeled Train rows".into());
        }
        if rows.iter().any(|c| {
            c.views.iter().any(|v| {
                v.provenance != c.provenance
                    || (v.signal_kind == ruview_unified::RfModality::Synthetic
                        && v.provenance != CaptureProvenance::Synthetic)
            })
        }) {
            return Err(
                "synthetic RF or view provenance cannot claim captured/live training".into(),
            );
        }
        let mut provenance: Vec<_> = rows.iter().map(|r| r.provenance).collect();
        provenance.sort_by_key(provenance_rank);
        provenance.dedup();
        let mut clocks: Vec<_> = rows
            .iter()
            .flat_map(|r| r.views.iter().map(|v| v.clock.kind))
            .collect();
        clocks.sort_by_key(clock_rank);
        clocks.dedup();
        let available = !provenance.contains(&CaptureProvenance::Synthetic)
            && clocks == [ClockKind::SourceMeasurement];
        let mut hash = HashWriter(Sha256::new());
        serde_json::to_writer(&mut hash, rows).map_err(|e| e.to_string())?;
        let manifest = Self {
            schema: "ruview.room-learner.train-manifest.v1".into(),
            training_provenance: provenance,
            training_data_sha256: format!("{:x}", hash.0.finalize()),
            consumed_channels: vec![ConsumedChannel::Magnitude],
            clock_claims: clocks,
            measurement_clock_available: available,
            training_cohorts: rows.len(),
        };
        manifest.validate()?;
        Ok(manifest)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        let expected_clock = !self
            .training_provenance
            .contains(&CaptureProvenance::Synthetic)
            && self.clock_claims == [ClockKind::SourceMeasurement];
        if self.schema != "ruview.room-learner.train-manifest.v1"
            || !(4..=64).contains(&self.training_cohorts)
            || self.training_data_sha256.len() != 64
            || !self
                .training_data_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || self.training_provenance.is_empty()
            || self.training_provenance.len() > 3
            || self
                .training_provenance
                .windows(2)
                .any(|w| provenance_rank(&w[0]) >= provenance_rank(&w[1]))
            || self.clock_claims.is_empty()
            || self.clock_claims.len() > 3
            || self
                .clock_claims
                .windows(2)
                .any(|w| clock_rank(&w[0]) >= clock_rank(&w[1]))
            || self.consumed_channels != [ConsumedChannel::Magnitude]
            || self.measurement_clock_available != expected_clock
        {
            return Err("invalid immutable Train provenance manifest".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
