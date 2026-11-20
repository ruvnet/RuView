//! Candidate cohort diagnostics. Declared trials are not verified independence.
use super::super::{
    model::{PresenceDecision, RoomModel, RoomPrediction},
    observation::{CaptureCohort, DataRole, RoomLabel, MAX_COHORTS},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(crate) const CONDITIONS: [RoomLabel; 4] = [
    RoomLabel::Empty,
    RoomLabel::OccupiedStatic,
    RoomLabel::OccupiedMoving,
    RoomLabel::AdjacentOnly,
];
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BinaryConfusion {
    pub true_empty: usize,
    pub false_occupied: usize,
    pub false_empty: usize,
    pub true_occupied: usize,
    pub abstained_empty: usize,
    pub abstained_occupied: usize,
    /// Missing probabilities are a subset of abstentions, never correct results.
    pub missing_empty: usize,
    pub missing_occupied: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRates {
    pub evaluated_cohort_coverage: Option<f32>,
    pub unconditional_accuracy: Option<f32>,
    pub scored_accuracy: Option<f32>,
    /// Includes every positive cohort, including abstained/missing positives.
    pub unconditional_sensitivity: Option<f32>,
    pub scored_sensitivity: Option<f32>,
    pub unconditional_specificity: Option<f32>,
    pub scored_specificity: Option<f32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConditionEvaluation {
    pub condition: RoomLabel,
    pub samples: usize,
    pub correct: usize,
    pub abstained: usize,
    pub scored: usize,
    pub missing: usize,
    pub binary_cross_entropy: Option<f32>,
    pub binary_cross_entropy_samples: usize,
    pub declared_trial_count: usize,
    pub declared_session_count: usize,
    pub confusion: BinaryConfusion,
    pub rates: EvaluationRates,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvaluationReport {
    pub samples: usize,
    pub correct: usize,
    pub abstained: usize,
    /// Same numeric mean as before when probabilities exist; otherwise null.
    pub binary_cross_entropy: Option<f32>,
    pub binary_cross_entropy_samples: usize,
    pub scored: usize,
    pub missing: usize,
    pub declared_trial_count: usize,
    pub declared_session_count: usize,
    pub confusion: BinaryConfusion,
    pub rates: EvaluationRates,
    pub per_condition: Vec<ConditionEvaluation>,
    pub independence_verified: bool,
}
fn ratio(n: usize, d: usize) -> Option<f32> {
    if d == 0 {
        None
    } else {
        Some(n as f32 / d as f32)
    }
}
impl BinaryConfusion {
    fn scored(&self) -> usize {
        self.true_empty + self.false_occupied + self.false_empty + self.true_occupied
    }
    fn correct(&self) -> usize {
        self.true_empty + self.true_occupied
    }
    fn abstained(&self) -> usize {
        self.abstained_empty + self.abstained_occupied
    }
    fn missing(&self) -> usize {
        self.missing_empty + self.missing_occupied
    }
    fn rates(&self) -> EvaluationRates {
        let scored = self.scored();
        let samples = scored + self.abstained();
        EvaluationRates {
            evaluated_cohort_coverage: ratio(scored, samples),
            unconditional_accuracy: ratio(self.correct(), samples),
            scored_accuracy: ratio(self.correct(), scored),
            unconditional_sensitivity: ratio(
                self.true_occupied,
                self.true_occupied + self.false_empty + self.abstained_occupied,
            ),
            scored_sensitivity: ratio(self.true_occupied, self.true_occupied + self.false_empty),
            unconditional_specificity: ratio(
                self.true_empty,
                self.true_empty + self.false_occupied + self.abstained_empty,
            ),
            scored_specificity: ratio(self.true_empty, self.true_empty + self.false_occupied),
        }
    }
}
#[derive(Default)]
struct Accumulator<'a> {
    confusion: BinaryConfusion,
    loss: f64,
    probabilities: usize,
    trials: HashSet<&'a str>,
    sessions: HashSet<&'a str>,
}
impl<'a> Accumulator<'a> {
    fn observe(&mut self, row: &'a CaptureCohort, p: &RoomPrediction) {
        self.trials.insert(&row.trial_id);
        self.sessions.insert(&row.session_id);
        let occupied = matches!(
            row.label,
            RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving
        );
        let probability = p
            .occupied_probability
            .filter(|v| v.is_finite() && (0.0..=1.0).contains(v));
        if let Some(v) = probability {
            let v = v.clamp(1e-6, 1.0 - 1e-6) as f64;
            self.loss -= if occupied { v.ln() } else { (1.0 - v).ln() };
            self.probabilities += 1;
        }
        let c = &mut self.confusion;
        if p.decision == PresenceDecision::Abstain || probability.is_none() {
            if occupied {
                c.abstained_occupied += 1;
                if probability.is_none() {
                    c.missing_occupied += 1;
                }
            } else {
                c.abstained_empty += 1;
                if probability.is_none() {
                    c.missing_empty += 1;
                }
            }
        } else {
            match (occupied, p.decision) {
                (false, PresenceDecision::Empty) => c.true_empty += 1,
                (false, PresenceDecision::Occupied) => c.false_occupied += 1,
                (true, PresenceDecision::Empty) => c.false_empty += 1,
                (true, PresenceDecision::Occupied) => c.true_occupied += 1,
                _ => unreachable!(),
            }
        }
    }
    fn loss(&self) -> Option<f32> {
        if self.probabilities == 0 {
            None
        } else {
            Some((self.loss / self.probabilities as f64) as f32)
        }
    }
    fn condition(&self, condition: RoomLabel) -> ConditionEvaluation {
        ConditionEvaluation {
            condition,
            samples: self.confusion.scored() + self.confusion.abstained(),
            correct: self.confusion.correct(),
            abstained: self.confusion.abstained(),
            scored: self.confusion.scored(),
            missing: self.confusion.missing(),
            binary_cross_entropy: self.loss(),
            binary_cross_entropy_samples: self.probabilities,
            declared_trial_count: self.trials.len(),
            declared_session_count: self.sessions.len(),
            confusion: self.confusion.clone(),
            rates: self.confusion.rates(),
        }
    }
}
impl EvaluationReport {
    pub(crate) fn compute(model: &RoomModel, rows: &[CaptureCohort]) -> Result<Self, String> {
        model.validate()?;
        if rows.is_empty() || rows.len() > MAX_COHORTS {
            return Err("invalid bounded evaluation cohort count".into());
        }
        let role = rows[0].role;
        if !matches!(role, DataRole::Validation | DataRole::HeldOut) {
            return Err("evaluation requires labeled validation or held-out cohorts".into());
        }
        let mut ids = HashSet::new();
        let mut total = Accumulator::default();
        let mut conditions: [Accumulator; 4] = Default::default();
        let digest = model.context().digest();
        for row in rows {
            if row.role != role || row.label == RoomLabel::Unknown {
                return Err("evaluation requires one labeled non-inference role".into());
            }
            if row.context_digest != digest {
                return Err("evaluation context mismatch".into());
            }
            if [&row.id, &row.trial_id, &row.session_id].iter().any(|id| {
                id.is_empty()
                    || id.len() > 256
                    || !id.is_ascii()
                    || id.trim() != id.as_str()
                    || id.chars().any(char::is_control)
            }) || !ids.insert(&row.id)
            {
                return Err("invalid or duplicate evaluation cohort identity".into());
            }
            let prediction = model.infer(row);
            let condition = CONDITIONS
                .iter()
                .position(|label| *label == row.label)
                .ok_or("unknown evaluation condition")?;
            total.observe(row, &prediction);
            conditions[condition].observe(row, &prediction);
        }
        let summary = total.condition(RoomLabel::Unknown);
        let report = Self {
            samples: summary.samples,
            correct: summary.correct,
            abstained: summary.abstained,
            binary_cross_entropy: summary.binary_cross_entropy,
            binary_cross_entropy_samples: summary.binary_cross_entropy_samples,
            scored: summary.scored,
            missing: summary.missing,
            declared_trial_count: summary.declared_trial_count,
            declared_session_count: summary.declared_session_count,
            confusion: summary.confusion,
            rates: summary.rates,
            per_condition: conditions
                .iter()
                .zip(CONDITIONS)
                .map(|(acc, label)| acc.condition(label))
                .collect(),
            independence_verified: false,
        };
        report.validate()?;
        Ok(report)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_COHORTS).contains(&self.samples)
            || self.independence_verified
            || self.per_condition.len() != 4
        {
            return Err("invalid evaluation report bounds".into());
        }
        let mut sum = BinaryConfusion::default();
        let mut probability_count = 0usize;
        let mut loss = 0.0;
        for (condition, label) in self.per_condition.iter().zip(CONDITIONS) {
            if condition.condition != label {
                return Err("invalid evaluation conditions".into());
            }
            validate_summary(condition)?;
            let c = &condition.confusion;
            if (matches!(label, RoomLabel::Empty | RoomLabel::AdjacentOnly)
                && c.true_occupied + c.false_empty + c.abstained_occupied != 0)
                || (matches!(label, RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving)
                    && c.true_empty + c.false_occupied + c.abstained_empty != 0)
            {
                return Err("condition confusion target mismatch".into());
            }
            sum.true_empty += c.true_empty;
            sum.false_occupied += c.false_occupied;
            sum.false_empty += c.false_empty;
            sum.true_occupied += c.true_occupied;
            sum.abstained_empty += c.abstained_empty;
            sum.abstained_occupied += c.abstained_occupied;
            sum.missing_empty += c.missing_empty;
            sum.missing_occupied += c.missing_occupied;
            probability_count = probability_count
                .checked_add(condition.binary_cross_entropy_samples)
                .ok_or("evaluation probability count overflow")?;
            loss += condition.binary_cross_entropy.unwrap_or(0.0) as f64
                * condition.binary_cross_entropy_samples as f64;
        }
        let summary = ConditionEvaluation {
            condition: RoomLabel::Unknown,
            samples: self.samples,
            correct: self.correct,
            abstained: self.abstained,
            scored: self.scored,
            missing: self.missing,
            binary_cross_entropy: self.binary_cross_entropy,
            binary_cross_entropy_samples: self.binary_cross_entropy_samples,
            declared_trial_count: self.declared_trial_count,
            declared_session_count: self.declared_session_count,
            confusion: self.confusion.clone(),
            rates: self.rates.clone(),
        };
        validate_summary(&summary)?;
        if sum != self.confusion
            || probability_count != self.binary_cross_entropy_samples
            || (probability_count > 0
                && (self.binary_cross_entropy.unwrap_or(0.0) as f64
                    - loss / probability_count as f64)
                    .abs()
                    > 1e-4)
            || !valid_declared_union(
                self.declared_trial_count,
                self.per_condition.iter().map(|c| c.declared_trial_count),
            )
            || !valid_declared_union(
                self.declared_session_count,
                self.per_condition.iter().map(|c| c.declared_session_count),
            )
        {
            return Err("evaluation aggregate mismatch".into());
        }
        Ok(())
    }
}
fn valid_declared_union(total: usize, mut counts: impl Iterator<Item = usize>) -> bool {
    let Some((maximum, sum)) = counts.try_fold((0usize, 0usize), |(maximum, sum), count| {
        sum.checked_add(count).map(|sum| (maximum.max(count), sum))
    }) else {
        return false;
    };
    total >= maximum && total <= sum
}
fn validate_summary(s: &ConditionEvaluation) -> Result<(), String> {
    let c = &s.confusion;
    // Bound components before addition so malformed JSON cannot overflow usize.
    if [
        c.true_empty,
        c.false_occupied,
        c.false_empty,
        c.true_occupied,
        c.abstained_empty,
        c.abstained_occupied,
        c.missing_empty,
        c.missing_occupied,
    ]
    .iter()
    .any(|n| *n > MAX_COHORTS)
        || s.samples > MAX_COHORTS
        || c.scored() + c.abstained() != s.samples
        || c.correct() != s.correct
        || c.scored() != s.scored
        || c.abstained() != s.abstained
        || c.missing() != s.missing
        || c.missing_empty > c.abstained_empty
        || c.missing_occupied > c.abstained_occupied
        || s.samples.checked_sub(s.missing) != Some(s.binary_cross_entropy_samples)
        || (s.samples > 0 && (s.declared_trial_count == 0 || s.declared_session_count == 0))
        || s.declared_trial_count > s.samples
        || s.declared_session_count > s.samples
        || c.rates() != s.rates
    {
        return Err("invalid evaluation cohort metrics".into());
    }
    match (s.binary_cross_entropy_samples, s.binary_cross_entropy) {
        (0, None) => {}
        (n, Some(loss)) if n > 0 && loss.is_finite() && (0.0..=32.0).contains(&loss) => {}
        _ => return Err("invalid evaluation probability loss".into()),
    }
    Ok(())
}

#[cfg(test)]
#[path = "evaluation_report_tests.rs"]
mod tests;
