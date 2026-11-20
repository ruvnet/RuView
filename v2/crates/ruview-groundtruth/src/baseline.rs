//! Session-disjoint splits and the trivial baseline for occupancy labels.
//!
//! Windows from one session are strongly correlated, so a split that puts
//! windows of the same session on both sides leaks. [`session_disjoint_split`]
//! splits by session only and refuses a split that cannot be disjoint.
//!
//! An occupancy or count accuracy means little without the score of a model
//! that ignores the CSI entirely. [`majority_baseline`] fits that model on the
//! training side (majority occupancy class, median person count) and scores it
//! on the test side. Report a model's numbers next to this baseline.
//!
//! Limits of what this baseline and split establish:
//! - A session split does not give independence across rooms or days. Two
//!   sessions recorded in the same room, or on the same day, share layout,
//!   occupants and RF environment, so a model can still look better than it
//!   would in a new room. Split by room or day when the claim needs it.
//! - The labeller's empty-hold guard (`empty_hold_ms`) is a backward-looking
//!   heuristic: it only suppresses an `Empty` label shortly after an occupied
//!   one and cannot see a person who is present but stationary or out of the
//!   radar's view.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::autolabel::{LabelledWindow, Occupancy};
use crate::error::GroundTruthError;

/// Train and test windows with no session on both sides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionSplit {
    /// Sessions on the training side, sorted.
    pub train_sessions: Vec<String>,
    /// Sessions on the test side, sorted.
    pub test_sessions: Vec<String>,
    /// Training windows.
    pub train: Vec<LabelledWindow>,
    /// Test windows.
    pub test: Vec<LabelledWindow>,
}

/// Split windows by session: sessions named in `test_sessions` go to the test
/// side, every other session to training.
///
/// # Errors
/// [`GroundTruthError::InvalidSplit`] when a named test session has no
/// windows, or when either side would be empty.
pub fn session_disjoint_split(
    windows: &[LabelledWindow],
    test_sessions: &[&str],
) -> Result<SessionSplit, GroundTruthError> {
    let present: BTreeSet<&str> = windows.iter().map(|w| w.session.as_str()).collect();
    let test: BTreeSet<&str> = test_sessions.iter().copied().collect();
    if test.iter().any(|s| !present.contains(s)) {
        return Err(GroundTruthError::InvalidSplit {
            reason: "a test session has no labelled windows",
        });
    }
    let (te, tr): (Vec<_>, Vec<_>) = windows
        .iter()
        .cloned()
        .partition(|w| test.contains(w.session.as_str()));
    if te.is_empty() || tr.is_empty() {
        return Err(GroundTruthError::InvalidSplit {
            reason: "both sides of a session split need at least one session",
        });
    }
    Ok(SessionSplit {
        train_sessions: present.difference(&test).map(|s| (*s).to_owned()).collect(),
        test_sessions: test.iter().map(|s| (*s).to_owned()).collect(),
        train: tr,
        test: te,
    })
}

/// The no-CSI baseline, fitted on training windows and scored on test windows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaselineReport {
    /// Training windows.
    pub train_windows: usize,
    /// Test windows.
    pub test_windows: usize,
    /// Test windows per occupancy class.
    pub test_class_counts: BTreeMap<Occupancy, usize>,
    /// The training majority class (ties go to `Empty`).
    pub majority_occupancy: Occupancy,
    /// Accuracy of always predicting the majority class on the test side.
    pub majority_accuracy: f64,
    /// Median training person count (mean of the two middle values for an even
    /// number of windows), when any training window has a count. The median
    /// minimises mean absolute error, so it is the strongest constant
    /// predictor for [`BaselineReport::count_mae`].
    pub median_count: Option<f64>,
    /// Mean absolute error of predicting `median_count` on test windows with a
    /// count.
    pub count_mae: Option<f64>,
    /// Test windows with a count.
    pub count_test_windows: usize,
}

/// Fit and score the majority-class / median-count baseline.
///
/// # Errors
/// [`GroundTruthError::InvalidSplit`] when either side is empty.
pub fn majority_baseline(
    train: &[LabelledWindow],
    test: &[LabelledWindow],
) -> Result<BaselineReport, GroundTruthError> {
    if train.is_empty() || test.is_empty() {
        return Err(GroundTruthError::InvalidSplit {
            reason: "baseline needs training and test windows",
        });
    }
    let occupied = train
        .iter()
        .filter(|w| w.occupancy == Occupancy::Occupied)
        .count();
    let majority_occupancy = if occupied * 2 > train.len() {
        Occupancy::Occupied
    } else {
        Occupancy::Empty
    };

    let mut test_class_counts = BTreeMap::new();
    for w in test {
        *test_class_counts.entry(w.occupancy).or_insert(0usize) += 1;
    }
    let hits = test_class_counts
        .get(&majority_occupancy)
        .copied()
        .unwrap_or(0);

    let train_counts: Vec<f64> = train
        .iter()
        .filter_map(|w| w.person_count)
        .map(f64::from)
        .collect();
    let median_count = median(&train_counts);
    let test_counts: Vec<f64> = test
        .iter()
        .filter_map(|w| w.person_count)
        .map(f64::from)
        .collect();
    let count_mae = match median_count {
        Some(m) if !test_counts.is_empty() => {
            Some(test_counts.iter().map(|c| (c - m).abs()).sum::<f64>() / test_counts.len() as f64)
        }
        _ => None,
    };

    Ok(BaselineReport {
        train_windows: train.len(),
        test_windows: test.len(),
        test_class_counts,
        majority_occupancy,
        majority_accuracy: hits as f64 / test.len() as f64,
        median_count,
        count_mae,
        count_test_windows: test_counts.len(),
    })
}

/// Median of `values`; the mean of the two middle values when the length is
/// even. `None` for an empty slice.
fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autolabel::CsiWindowSummary;

    fn w(session: &str, i: i64, count: u32) -> LabelledWindow {
        LabelledWindow {
            evidence: crate::agreement::EvidenceGrade::Claimed,
            session: session.into(),
            start_ms: i * 1000,
            end_ms: i * 1000 + 1000,
            occupancy: if count > 0 {
                Occupancy::Occupied
            } else {
                Occupancy::Empty
            },
            person_count: Some(count),
            count_low: Some(count),
            count_high: Some(count),
            radar_window_index: i as usize,
            csi: CsiWindowSummary {
                n_frames: 10,
                rssi_mean: None,
                amplitude_mean: None,
                amplitude_var: None,
            },
        }
    }

    fn windows() -> Vec<LabelledWindow> {
        let mut v = Vec::new();
        for (i, c) in [0, 1, 1, 2].iter().enumerate() {
            v.push(w("mon", i as i64, *c));
        }
        for (i, c) in [0, 0, 1].iter().enumerate() {
            v.push(w("tue", i as i64, *c));
        }
        for (i, c) in [1, 0].iter().enumerate() {
            v.push(w("wed", i as i64, *c));
        }
        v
    }

    #[test]
    fn split_keeps_sessions_on_one_side() {
        let s = session_disjoint_split(&windows(), &["tue"]).unwrap();
        assert_eq!(s.train_sessions, ["mon", "wed"]);
        assert_eq!(s.test_sessions, ["tue"]);
        assert_eq!((s.train.len(), s.test.len()), (6, 3));
        let train: BTreeSet<_> = s.train.iter().map(|w| &w.session).collect();
        assert!(s.test.iter().all(|w| !train.contains(&w.session)));
    }

    #[test]
    fn impossible_splits_are_refused() {
        let all = windows();
        assert!(session_disjoint_split(&all, &[]).is_err());
        assert!(session_disjoint_split(&all, &["mon", "tue", "wed"]).is_err());
        assert!(session_disjoint_split(&all, &["thu"]).is_err());
        let one: Vec<_> = all.into_iter().filter(|w| w.session == "mon").collect();
        assert!(session_disjoint_split(&one, &["mon"]).is_err());
    }

    #[test]
    fn baseline_matches_hand_computed_fixture() {
        let s = session_disjoint_split(&windows(), &["tue"]).unwrap();
        // Train: mon [0,1,1,2] + wed [1,0] -> 4 occupied of 6, median count 1.
        // Test: tue [0,0,1] -> majority "occupied" is right once in three.
        let b = majority_baseline(&s.train, &s.test).unwrap();
        assert_eq!(b.majority_occupancy, Occupancy::Occupied);
        assert!((b.majority_accuracy - 1.0 / 3.0).abs() < 1e-12);
        assert!((b.median_count.unwrap() - 1.0).abs() < 1e-12);
        // Test tue [0,0,1] against a constant 1: errors 1, 1, 0.
        let mae = 2.0 / 3.0;
        assert!((b.count_mae.unwrap() - mae).abs() < 1e-12);
        assert_eq!(b.count_test_windows, 3);
        assert_eq!(b.test_class_counts.get(&Occupancy::Empty), Some(&2));
    }

    #[test]
    fn median_is_mae_optimal_and_handles_even_and_odd() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[0.0, 0.0, 1.0, 9.0]), Some(0.5));
        // Skewed train set: the mean (2.0) would score worse on MAE than the
        // median (0.0) against test counts [0, 0, 0].
        let train = vec![w("a", 0, 0), w("a", 1, 0), w("a", 2, 6)];
        let test = vec![w("b", 0, 0), w("b", 1, 0), w("b", 2, 0)];
        let b = majority_baseline(&train, &test).unwrap();
        assert_eq!(b.median_count, Some(0.0));
        assert_eq!(b.count_mae, Some(0.0));
    }

    #[test]
    fn tie_goes_to_empty_and_missing_counts_are_none() {
        let mut train = vec![w("a", 0, 0), w("a", 1, 1)];
        for t in &mut train {
            t.person_count = None;
        }
        let test = vec![w("b", 0, 0)];
        let b = majority_baseline(&train, &test).unwrap();
        assert_eq!(b.majority_occupancy, Occupancy::Empty);
        assert_eq!(b.majority_accuracy, 1.0);
        assert_eq!((b.median_count, b.count_mae), (None, None));
        assert!(majority_baseline(&[], &test).is_err());
    }
}
