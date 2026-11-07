//! Experimental calibration-only bank: one existing FieldModel per source.
//!
//! No sensing-server caller or detection promotion is introduced here. A frozen
//! bank is learned statistics, not an owner/room receipt or occupied acceptance.
//! The caller must enforce the production capture duration, room binding and
//! independent physical qualification. Tests use explicitly synthetic inputs.

use std::collections::BTreeMap;
use thiserror::Error;

use super::field_model::{
    CalibrationStatus, EmptyRoomMatch, FieldModel, FieldModelConfig, FieldModelError,
    FieldModelSnapshotV1,
};

const MAX_SOURCES: usize = 32;
const MAX_SUBCARRIERS: usize = 256;
const MAX_SOURCE_AGE_US: u64 = 5_000_000;
const MAX_AMPLITUDE: f64 = 1_000_000.0;

pub struct SourceFieldSpec {
    pub source_id: u8,
    pub grid_id: String,
    pub config: FieldModelConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceBankPhase {
    Collecting,
    Frozen,
    Invalid,
}

#[derive(Debug, Error)]
pub enum SourceBankError {
    #[error("invalid source-bank configuration: {0}")]
    Configuration(&'static str),
    #[error("duplicate source {0}")]
    DuplicateSource(u8),
    #[error("unknown source {0}")]
    UnknownSource(u8),
    #[error("source {0} changed its grid, sample order, timing or frame shape")]
    SourceChanged(u8),
    #[error("source {0} has not completed its independent calibration")]
    IncompleteSource(u8),
    #[error("source {0} is missing, stale or future-dated")]
    StaleSource(u8),
    #[error("bank is not in the required phase")]
    Phase,
    #[error("field-model calibration failed: {0}")]
    Model(#[from] FieldModelError),
}

pub struct FrozenSourceField {
    pub source_id: u8,
    pub grid_id: String,
    pub snapshot: FieldModelSnapshotV1,
}

/// A bounded, memory-only window from the caller's admitted/epoch-bound source
/// recorder. The caller must describe its actual transport trust; this type does
/// not authenticate a radio. Comparisons are empty-background tests, not person labels.
pub struct SourceFieldWindow<'a> {
    pub source_id: u8,
    pub grid_id: &'a str,
    pub observed_at_us: u64,
    pub frames: &'a [Vec<f64>],
}

struct SourceTraining {
    grid_id: String,
    width: usize,
    model: FieldModel,
    last_sample: Option<(u64, u64)>,
}

pub struct SourceFieldBank {
    sources: BTreeMap<u8, SourceTraining>,
    phase: SourceBankPhase,
    source_age_us: u64,
    frozen_at_us: Option<u64>,
}

impl SourceFieldBank {
    pub fn new(specs: Vec<SourceFieldSpec>, source_age_us: u64) -> Result<Self, SourceBankError> {
        if !(2..=MAX_SOURCES).contains(&specs.len())
            || source_age_us == 0
            || source_age_us > MAX_SOURCE_AGE_US
        {
            return Err(SourceBankError::Configuration(
                "bounded source set and freshness window required",
            ));
        }
        // Validate every allocation shape before constructing any model.
        for spec in &specs {
            let c = &spec.config;
            if c.n_links != 1
                || !(1..=MAX_SUBCARRIERS).contains(&c.n_subcarriers)
                || !(1..=5).contains(&c.n_modes)
                || !(2..=1_000_000).contains(&c.min_calibration_frames)
                || !c.min_calibration_duration_s.is_finite()
                || !(0.0..=86_400.0).contains(&c.min_calibration_duration_s)
                || !c.baseline_expiry_s.is_finite()
                || c.baseline_expiry_s <= 0.0
                || c.baseline_expiry_s > 86_400.0
                || spec.grid_id.is_empty()
                || spec.grid_id.len() > 128
                || !spec
                    .grid_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            {
                return Err(SourceBankError::Configuration(
                    "one bounded finite single-link model and grid per source required",
                ));
            }
        }
        let mut sources = BTreeMap::new();
        for spec in specs {
            if sources.contains_key(&spec.source_id) {
                return Err(SourceBankError::DuplicateSource(spec.source_id));
            }
            sources.insert(
                spec.source_id,
                SourceTraining {
                    grid_id: spec.grid_id,
                    width: spec.config.n_subcarriers,
                    model: FieldModel::new(spec.config)?,
                    last_sample: None,
                },
            );
        }
        Ok(Self {
            sources,
            phase: SourceBankPhase::Collecting,
            source_age_us,
            frozen_at_us: None,
        })
    }

    pub fn phase(&self) -> SourceBankPhase {
        self.phase
    }

    /// `ordinal` is the server's validated per-source ordinal, not an unchecked
    /// raw device counter. Device wrap/transport authentication remain upstream.
    pub fn feed(
        &mut self,
        source_id: u8,
        grid_id: &str,
        ordinal: u64,
        observed_at_us: u64,
        amplitudes: &[f64],
    ) -> Result<(), SourceBankError> {
        if self.phase != SourceBankPhase::Collecting {
            return Err(SourceBankError::Phase);
        }
        let source = self
            .sources
            .get_mut(&source_id)
            .ok_or(SourceBankError::UnknownSource(source_id))?;
        let changed = grid_id != source.grid_id
            || observed_at_us == 0
            || amplitudes.len() != source.width
            || amplitudes
                .iter()
                .any(|x| !x.is_finite() || x.abs() > MAX_AMPLITUDE)
            || source.last_sample.is_some_and(|(last_ordinal, last_at)| {
                ordinal <= last_ordinal
                    || observed_at_us <= last_at
                    || observed_at_us.saturating_sub(last_at) > self.source_age_us
            });
        if changed {
            self.phase = SourceBankPhase::Invalid;
            return Err(SourceBankError::SourceChanged(source_id));
        }
        if let Err(error) = source.model.feed_calibration(&[amplitudes.to_vec()]) {
            self.phase = SourceBankPhase::Invalid;
            return Err(error.into());
        }
        source.last_sample = Some((ordinal, observed_at_us));
        Ok(())
    }

    pub fn frame_counts(&self) -> BTreeMap<u8, u64> {
        self.sources
            .iter()
            .map(|(id, s)| (*id, s.model.calibration_frame_count()))
            .collect()
    }

    /// Check all members before mutating any model. A model-level failure
    /// invalidates the whole generation; partially frozen members cannot escape.
    pub fn freeze(&mut self, timestamp_us: u64, geometry_hash: u64) -> Result<(), SourceBankError> {
        if self.phase != SourceBankPhase::Collecting {
            return Err(SourceBankError::Phase);
        }
        for (id, source) in &self.sources {
            if source.model.calibration_frame_count() < source.model.min_calibration_frames() as u64
                || source.model.calibration_elapsed_s() < source.model.min_calibration_duration_s()
            {
                return Err(SourceBankError::IncompleteSource(*id));
            }
            if source
                .last_sample
                .is_none_or(|(_, at)| timestamp_us < at || timestamp_us - at > self.source_age_us)
            {
                return Err(SourceBankError::StaleSource(*id));
            }
        }
        for source in self.sources.values_mut() {
            if let Err(error) = source
                .model
                .finalize_calibration(timestamp_us, geometry_hash)
            {
                self.phase = SourceBankPhase::Invalid;
                return Err(error.into());
            }
        }
        self.phase = SourceBankPhase::Frozen;
        self.frozen_at_us = Some(timestamp_us);
        Ok(())
    }

    /// Statistics only. The server must wrap these in a new versioned,
    /// account/installation/room-bound artifact before persistence or adoption.
    pub fn frozen_models(&self) -> Result<Vec<FrozenSourceField>, SourceBankError> {
        if self.phase != SourceBankPhase::Frozen {
            return Err(SourceBankError::Phase);
        }
        self.sources
            .iter()
            .map(|(id, source)| {
                Ok(FrozenSourceField {
                    source_id: *id,
                    grid_id: source.grid_id.clone(),
                    snapshot: source.model.export_snapshot()?,
                })
            })
            .collect()
    }

    /// Shadow diagnostics only: require every configured source and return its
    /// individual empty-background conformance. No aggregate presence is emitted.
    pub fn compare_empty_windows(
        &self,
        timestamp_us: u64,
        windows: &[SourceFieldWindow<'_>],
    ) -> Result<BTreeMap<u8, EmptyRoomMatch>, SourceBankError> {
        let freshness = self.freshness(timestamp_us)?;
        if windows.len() != self.sources.len() {
            return Err(SourceBankError::Configuration(
                "complete runtime window set required",
            ));
        }
        let mut result = BTreeMap::new();
        for window in windows {
            let id = window.source_id;
            let source = self
                .sources
                .get(&id)
                .ok_or(SourceBankError::UnknownSource(id))?;
            if result.contains_key(&id)
                || window.grid_id != source.grid_id
                || window.frames.is_empty()
                || window.frames.len() > 200
                || window.frames.iter().any(|frame| {
                    frame.len() != source.width
                        || frame
                            .iter()
                            .any(|v| !v.is_finite() || v.abs() > MAX_AMPLITUDE)
                })
            {
                return Err(SourceBankError::SourceChanged(id));
            }
            if freshness[&id] != CalibrationStatus::Fresh
                || timestamp_us < window.observed_at_us
                || timestamp_us - window.observed_at_us > self.source_age_us
            {
                return Err(SourceBankError::StaleSource(id));
            }
            let compared = source
                .model
                .empty_room_match(window.frames)
                .ok_or(SourceBankError::IncompleteSource(id))?;
            result.insert(id, compared);
        }
        if result.len() != self.sources.len() {
            return Err(SourceBankError::Configuration(
                "required source window missing",
            ));
        }
        Ok(result)
    }

    pub fn freshness(
        &self,
        timestamp_us: u64,
    ) -> Result<BTreeMap<u8, CalibrationStatus>, SourceBankError> {
        if self.phase != SourceBankPhase::Frozen
            || self.frozen_at_us.is_none_or(|at| timestamp_us < at)
        {
            return Err(SourceBankError::Phase);
        }
        Ok(self
            .sources
            .iter()
            .map(|(id, s)| (*id, s.model.check_freshness(timestamp_us)))
            .collect())
    }
}

#[cfg(test)]
#[path = "source_field_bank_tests.rs"]
mod tests;
