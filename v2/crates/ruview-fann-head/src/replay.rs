//! Replay comparison on held-out sessions with fixed gates (ADR-425 D5).
//!
//! Input is one row per admitted cohort with its frozen feature vector, the
//! operator's condition label, the session it came from and a split assigned
//! per *session* (`train`, `validation`, `heldout`). For each room, a candidate
//! and a feature-matched linear control are fitted on that room's Train rows
//! and scored on its HeldOut rows, next to the incumbent's recorded
//! probability for the same rows.
//!
//! Abstentions stay in every denominator: a model cannot improve its
//! sensitivity or false-positive rate by going quiet.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::train::{fit, FitConfig};
use crate::{decide, HeadContext, HeadError, PresenceDecision};

/// Local qualification gates (room learner policy) and the pilot target.
pub const MIN_ROWS_PER_CONDITION: usize = 4;
pub const MIN_SENSITIVITY: f64 = 0.90;
pub const MAX_FALSE_POSITIVE_RATE: f64 = 0.05;
pub const MIN_COVERAGE: f64 = 0.90;
pub const MIN_ADJACENT_REDUCTION: f64 = 0.25;
const BOOTSTRAP_DRAWS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    QuietEmpty,
    AdjacentOnly,
    StillOccupied,
    MovingOccupied,
}

impl Condition {
    pub fn occupied(self) -> bool {
        matches!(self, Self::StillOccupied | Self::MovingOccupied)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    Train,
    Validation,
    Heldout,
}

/// One admitted cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRow {
    pub room: String,
    pub session: String,
    pub split: Split,
    pub condition: Condition,
    pub features: Vec<f32>,
    /// The incumbent's probability for this exact cohort, when recorded.
    #[serde(default)]
    pub incumbent_probability: Option<f32>,
    #[serde(default)]
    pub synthetic: bool,
}

/// Per-condition outcome counts for one model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ConditionScore {
    pub rows: usize,
    pub occupied: usize,
    pub empty: usize,
    pub abstain: usize,
    /// Share of rows with a decision.
    pub coverage: f64,
    /// Occupied conditions: occupied decisions over all rows (abstain is a miss).
    pub sensitivity: Option<f64>,
    /// Negative conditions: occupied decisions over all rows.
    pub false_positive_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelScore {
    pub conditions: BTreeMap<Condition, ConditionScore>,
    /// `None` when a condition has too few HeldOut rows to judge.
    pub passes_local_gates: Option<bool>,
    pub gate_failures: Vec<String>,
}

/// Candidate against incumbent on adjacent-room-only rows, paired by session.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdjacentComparison {
    pub incumbent_false_occupied: usize,
    pub candidate_false_occupied: usize,
    pub rows: usize,
    pub sessions: usize,
    /// `None` when the incumbent has no false occupied rows to reduce.
    pub relative_reduction: Option<f64>,
    /// 95 % session-bootstrap interval of (incumbent - candidate) false rows.
    pub paired_difference_ci95: (f64, f64),
    pub meets_pilot_target: bool,
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoomReport {
    pub room: String,
    pub train_rows: usize,
    pub heldout_rows: usize,
    pub heldout_sessions: usize,
    pub incumbent: Option<ModelScore>,
    pub linear_control: ModelScore,
    pub candidate: ModelScore,
    pub candidate_digest: String,
    pub candidate_parameters: usize,
    pub adjacent: Option<AdjacentComparison>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplayReport {
    pub task: &'static str,
    pub evidence: &'static str,
    pub objective_note: &'static str,
    pub rooms: Vec<RoomReport>,
}

fn score_model(rows: &[(&ReplayRow, PresenceDecision)]) -> ModelScore {
    let mut conditions: BTreeMap<Condition, ConditionScore> = BTreeMap::new();
    for (row, decision) in rows {
        let c = conditions.entry(row.condition).or_default();
        c.rows += 1;
        match decision {
            PresenceDecision::Occupied => c.occupied += 1,
            PresenceDecision::Empty => c.empty += 1,
            PresenceDecision::Abstain => c.abstain += 1,
        }
    }
    let mut failures = Vec::new();
    let mut judged = true;
    for condition in [
        Condition::QuietEmpty,
        Condition::AdjacentOnly,
        Condition::StillOccupied,
        Condition::MovingOccupied,
    ] {
        let Some(c) = conditions.get_mut(&condition) else {
            judged = false;
            continue;
        };
        let n = c.rows as f64;
        c.coverage = (c.rows - c.abstain) as f64 / n;
        let rate = c.occupied as f64 / n;
        if condition.occupied() {
            c.sensitivity = Some(rate);
            if rate < MIN_SENSITIVITY {
                failures.push(format!(
                    "{condition:?}: sensitivity {rate:.3} < {MIN_SENSITIVITY}"
                ));
            }
        } else {
            c.false_positive_rate = Some(rate);
            if rate > MAX_FALSE_POSITIVE_RATE {
                failures.push(format!(
                    "{condition:?}: false positives {rate:.3} > {MAX_FALSE_POSITIVE_RATE}"
                ));
            }
        }
        if c.coverage < MIN_COVERAGE {
            failures.push(format!(
                "{condition:?}: coverage {:.3} < {MIN_COVERAGE}",
                c.coverage
            ));
        }
        judged &= c.rows >= MIN_ROWS_PER_CONDITION;
    }
    ModelScore {
        conditions,
        passes_local_gates: judged.then_some(failures.is_empty()),
        gate_failures: failures,
    }
}

fn adjacent_comparison(
    incumbent: &[(&ReplayRow, PresenceDecision)],
    candidate: &[(&ReplayRow, PresenceDecision)],
) -> Option<AdjacentComparison> {
    // Per-session (incumbent false occupied, candidate false occupied).
    let mut by_session: BTreeMap<&str, (f64, f64)> = BTreeMap::new();
    let mut rows = 0;
    for ((row, inc), (_, cand)) in incumbent.iter().zip(candidate) {
        if row.condition != Condition::AdjacentOnly {
            continue;
        }
        rows += 1;
        let entry = by_session.entry(row.session.as_str()).or_default();
        entry.0 += f64::from(u8::from(*inc == PresenceDecision::Occupied));
        entry.1 += f64::from(u8::from(*cand == PresenceDecision::Occupied));
    }
    if rows == 0 {
        return None;
    }
    let sessions: Vec<(f64, f64)> = by_session.values().copied().collect();
    let inc: f64 = sessions.iter().map(|s| s.0).sum();
    let cand: f64 = sessions.iter().map(|s| s.1).sum();

    // Sessions, not rows, are the independent unit: resample them with a
    // fixed-seed generator so the interval is reproducible.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut draws: Vec<f64> = (0..BOOTSTRAP_DRAWS)
        .map(|_| {
            (0..sessions.len())
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    let s = sessions[(state % sessions.len() as u64) as usize];
                    s.0 - s.1
                })
                .sum()
        })
        .collect();
    draws.sort_by(f64::total_cmp);
    let ci = (
        draws[BOOTSTRAP_DRAWS * 25 / 1000],
        draws[BOOTSTRAP_DRAWS * 975 / 1000 - 1],
    );

    let reduction = (inc > 0.0).then(|| (inc - cand) / inc);
    let meets = reduction.is_some_and(|r| r >= MIN_ADJACENT_REDUCTION) && ci.0 > 0.0;
    Some(AdjacentComparison {
        incumbent_false_occupied: inc as usize,
        candidate_false_occupied: cand as usize,
        rows,
        sessions: sessions.len(),
        relative_reduction: reduction,
        paired_difference_ci95: ci,
        meets_pilot_target: meets,
        note: if inc == 0.0 {
            "incumbent has no adjacent false occupied rows: report the bound and keep the simpler model"
        } else if sessions.len() < 2 {
            "one session: the interval cannot show a paired improvement"
        } else {
            "paired by session"
        },
    })
}

/// Fit and score every room. `context` supplies the feature schema and source
/// contract; each room's name becomes the head's room revision.
pub fn evaluate(
    rows: &[ReplayRow],
    context: &HeadContext,
    config: FitConfig,
) -> Result<ReplayReport, HeadError> {
    let Some(width) = rows.first().map(|r| r.features.len()) else {
        return Err(HeadError::Invalid("no replay rows"));
    };
    if rows.iter().any(|r| r.features.len() != width) {
        return Err(HeadError::Invalid("rows disagree on the feature count"));
    }
    // A session belongs to exactly one split, or HeldOut is not held out.
    let mut session_split: BTreeMap<(&str, &str), Split> = BTreeMap::new();
    for r in rows {
        if *session_split
            .entry((&r.room, &r.session))
            .or_insert(r.split)
            != r.split
        {
            return Err(HeadError::Invalid(
                "a session appears in more than one split",
            ));
        }
    }

    let room_names: BTreeSet<&str> = rows.iter().map(|r| r.room.as_str()).collect();
    let mut rooms = Vec::new();
    for room in room_names {
        let train: Vec<&ReplayRow> = rows
            .iter()
            .filter(|r| r.room == room && r.split == Split::Train)
            .collect();
        let heldout: Vec<&ReplayRow> = rows
            .iter()
            .filter(|r| r.room == room && r.split == Split::Heldout)
            .collect();
        if train.is_empty() || heldout.is_empty() {
            return Err(HeadError::Invalid(
                "every room needs Train and HeldOut rows",
            ));
        }
        let features: Vec<f32> = train
            .iter()
            .flat_map(|r| r.features.iter().copied())
            .collect();
        let labels: Vec<bool> = train.iter().map(|r| r.condition.occupied()).collect();
        let room_context = HeadContext {
            room_revision: room.to_string(),
            ..context.clone()
        };

        let candidate = fit(&features, &labels, room_context.clone(), config)?.head;
        let control = fit(
            &features,
            &labels,
            room_context,
            FitConfig {
                hidden: 0,
                ..config
            },
        )?
        .head;

        let decisions =
            |head: &crate::FannHead| -> Result<Vec<(&ReplayRow, PresenceDecision)>, HeadError> {
                let mut scorer = head.scorer()?;
                heldout
                    .iter()
                    .map(|r| Ok((*r, decide(head.score(&mut scorer, &r.features)?))))
                    .collect()
            };
        let candidate_rows = decisions(&candidate)?;
        let control_rows = decisions(&control)?;
        let incumbent_rows: Option<Vec<(&ReplayRow, PresenceDecision)>> = heldout
            .iter()
            .map(|r| {
                r.incumbent_probability
                    .filter(|p| p.is_finite())
                    .map(|p| (*r, decide(p)))
            })
            .collect();

        rooms.push(RoomReport {
            room: room.to_string(),
            train_rows: train.len(),
            heldout_rows: heldout.len(),
            heldout_sessions: heldout
                .iter()
                .map(|r| r.session.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            adjacent: incumbent_rows
                .as_ref()
                .and_then(|inc| adjacent_comparison(inc, &candidate_rows)),
            incumbent: incumbent_rows.as_deref().map(score_model),
            linear_control: score_model(&control_rows),
            candidate: score_model(&candidate_rows),
            candidate_digest: candidate.digest().to_string(),
            candidate_parameters: candidate.parameter_count(),
        });
    }
    Ok(ReplayReport {
        task: "binary room presence, room-specific head on frozen features",
        evidence: if rows.iter().any(|r| r.synthetic) {
            "SYNTHETIC"
        } else {
            "MEASURED"
        },
        objective_note:
            "candidate and linear control are fitted with MSE on the sigmoid probability; \
                         the incumbent was fitted with binary cross-entropy",
        rooms,
    })
}

/// Parse replay rows from JSON lines.
pub fn parse_rows(text: &str) -> Result<Vec<ReplayRow>, HeadError> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).map_err(|_| HeadError::Json))
        .collect()
}
