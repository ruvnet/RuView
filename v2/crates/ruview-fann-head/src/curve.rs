//! Learning curve: how much Train data a room needs (ADR-426 D4).
//!
//! For each room the Train rows are put in recording order and the candidate
//! and its linear control are refitted on growing prefixes, each scored on the
//! room's full HeldOut set with the replay gates. Prefixes end on session
//! boundaries when the room has at least three Train sessions, otherwise at
//! fixed fractions of the rows.
//!
//! A prefix that holds only one class cannot be fitted; it is reported as
//! such, never skipped, so the curve shows where learning becomes possible.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::replay::{check_rows, label_provenance, score_model, ModelScore, ReplayRow, Split};
use crate::train::{fit, FitConfig};
use crate::{decide, FannHead, HeadContext, HeadError, PresenceDecision};

const ROW_FRACTIONS: [(usize, usize); 6] = [(1, 16), (1, 8), (1, 4), (1, 2), (3, 4), (1, 1)];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurvePoint {
    pub train_sessions: usize,
    pub train_rows: usize,
    /// Sum of each included session's first-to-last row time, when every row
    /// in the prefix carries a timestamp.
    pub train_minutes: Option<f64>,
    /// `None` when the prefix holds one class only and cannot be fitted.
    pub candidate: Option<ModelScore>,
    pub linear_control: Option<ModelScore>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoomCurve {
    pub room: String,
    /// `sessions` or `row_fractions`.
    pub prefix_unit: &'static str,
    pub heldout_rows: usize,
    pub points: Vec<CurvePoint>,
    /// Smallest prefix from which the model passes the gates at every later
    /// point too. `None` when it never settles.
    pub candidate_passes_from_rows: Option<usize>,
    pub linear_control_passes_from_rows: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurveReport {
    pub task: &'static str,
    pub evidence: &'static str,
    pub label_sources: Vec<String>,
    pub labels_are_reference: bool,
    pub rooms: Vec<RoomCurve>,
}

fn minutes(prefix: &[&ReplayRow]) -> Option<f64> {
    let mut spans: BTreeMap<&str, (i64, i64)> = BTreeMap::new();
    for r in prefix {
        let t = r.timestamp_ms?;
        let e = spans.entry(r.session.as_str()).or_insert((t, t));
        e.0 = e.0.min(t);
        e.1 = e.1.max(t);
    }
    Some(spans.values().map(|(a, b)| (b - a) as f64).sum::<f64>() / 60_000.0)
}

fn passes_from(
    points: &[CurvePoint],
    pick: impl Fn(&CurvePoint) -> Option<&ModelScore>,
) -> Option<usize> {
    let mut from = None;
    for p in points {
        if pick(p).and_then(|m| m.passes_local_gates) == Some(true) {
            from.get_or_insert(p.train_rows);
        } else {
            from = None;
        }
    }
    from
}

/// Refit on growing Train prefixes and score each on HeldOut.
pub fn learning_curve(
    rows: &[ReplayRow],
    context: &HeadContext,
    config: FitConfig,
) -> Result<CurveReport, HeadError> {
    check_rows(rows)?;
    let room_names: BTreeSet<&str> = rows.iter().map(|r| r.room.as_str()).collect();
    let mut rooms = Vec::new();
    for room in room_names {
        let heldout: Vec<&ReplayRow> = rows
            .iter()
            .filter(|r| r.room == room && r.split == Split::Heldout)
            .collect();
        let mut train: Vec<(usize, &ReplayRow)> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.room == room && r.split == Split::Train)
            .collect();
        if train.is_empty() || heldout.is_empty() {
            return Err(HeadError::Invalid(
                "every room needs Train and HeldOut rows",
            ));
        }
        // Recording order: sessions by first timestamp (else first appearance),
        // rows within a session by timestamp (else input order).
        let mut session_key: BTreeMap<&str, (i64, usize)> = BTreeMap::new();
        for (i, r) in &train {
            let k = (r.timestamp_ms.unwrap_or(i64::MAX), *i);
            let e = session_key.entry(r.session.as_str()).or_insert(k);
            *e = (*e).min(k);
        }
        train.sort_by_key(|(i, r)| {
            (
                session_key[r.session.as_str()],
                r.timestamp_ms.unwrap_or(i64::MAX),
                *i,
            )
        });
        let train: Vec<&ReplayRow> = train.into_iter().map(|(_, r)| r).collect();

        let by_session = session_key.len() >= 3;
        let mut ends: Vec<usize> = if by_session {
            (1..=train.len())
                .filter(|&n| n == train.len() || train[n].session != train[n - 1].session)
                .collect()
        } else {
            ROW_FRACTIONS
                .iter()
                .map(|(a, b)| (train.len() * a).div_ceil(*b))
                .collect()
        };
        ends.dedup();

        let room_context = HeadContext {
            room_revision: room.to_string(),
            ..context.clone()
        };
        let score = |head: &FannHead| -> Result<ModelScore, HeadError> {
            let mut scorer = head.scorer()?;
            let decisions: Vec<(&ReplayRow, PresenceDecision)> = heldout
                .iter()
                .map(|r| Ok((*r, decide(head.score(&mut scorer, &r.features)?))))
                .collect::<Result<_, HeadError>>()?;
            Ok(score_model(&decisions))
        };

        let mut points = Vec::new();
        for end in ends {
            let prefix = &train[..end];
            let labels: Vec<bool> = prefix.iter().map(|r| r.condition.occupied()).collect();
            let both = labels.iter().any(|&l| l) && labels.iter().any(|&l| !l);
            let (candidate, linear_control) = if both {
                let features: Vec<f32> = prefix
                    .iter()
                    .flat_map(|r| r.features.iter().copied())
                    .collect();
                let cand = fit(&features, &labels, room_context.clone(), config)?.head;
                let ctrl = fit(
                    &features,
                    &labels,
                    room_context.clone(),
                    FitConfig {
                        hidden: 0,
                        ..config
                    },
                )?
                .head;
                (Some(score(&cand)?), Some(score(&ctrl)?))
            } else {
                (None, None)
            };
            points.push(CurvePoint {
                train_sessions: prefix
                    .iter()
                    .map(|r| r.session.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
                train_rows: end,
                train_minutes: minutes(prefix),
                candidate,
                linear_control,
            });
        }
        rooms.push(RoomCurve {
            room: room.to_string(),
            prefix_unit: if by_session {
                "sessions"
            } else {
                "row_fractions"
            },
            heldout_rows: heldout.len(),
            candidate_passes_from_rows: passes_from(&points, |p| p.candidate.as_ref()),
            linear_control_passes_from_rows: passes_from(&points, |p| p.linear_control.as_ref()),
            points,
        });
    }
    let (label_sources, labels_are_reference) = label_provenance(rows);
    Ok(CurveReport {
        task: "learning curve: refit on growing Train prefixes, score on HeldOut",
        evidence: if rows.iter().any(|r| r.synthetic) {
            "SYNTHETIC"
        } else {
            "MEASURED"
        },
        label_sources,
        labels_are_reference,
        rooms,
    })
}
