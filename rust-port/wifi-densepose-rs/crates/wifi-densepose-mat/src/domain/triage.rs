//! Triage classification following START protocol.
//!
//! The START (Simple Triage and Rapid Treatment) protocol is used to
//! quickly categorize victims in mass casualty incidents.

use super::{VitalSignsReading, BreathingType, MovementType};

/// Triage status following START protocol
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TriageStatus {
    /// Immediate (Red) - Life-threatening, requires immediate intervention
    /// RPM: Respiration >30 or <10, or absent pulse, or unable to follow commands
    Immediate,

    /// Delayed (Yellow) - Serious but stable, can wait for treatment
    /// RPM: Normal respiration, pulse present, follows commands, non-life-threatening
    Delayed,

    /// Minor (Green) - Walking wounded, minimal treatment needed
    /// Can walk, minor injuries
    Minor,

    /// Deceased (Black) - No vital signs, or not breathing after airway cleared
    Deceased,

    /// Unknown - Insufficient data for classification
    Unknown,
}

impl TriageStatus {
    /// Get the priority level (1 = highest)
    pub fn priority(&self) -> u8 {
        match self {
            TriageStatus::Immediate => 1,
            TriageStatus::Delayed => 2,
            TriageStatus::Minor => 3,
            TriageStatus::Deceased => 4,
            TriageStatus::Unknown => 5,
        }
    }

    /// Get display color
    pub fn color(&self) -> &'static str {
        match self {
            TriageStatus::Immediate => "red",
            TriageStatus::Delayed => "yellow",
            TriageStatus::Minor => "green",
            TriageStatus::Deceased => "black",
            TriageStatus::Unknown => "gray",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            TriageStatus::Immediate => "Requires immediate life-saving intervention",
            TriageStatus::Delayed => "Serious but can wait for treatment",
            TriageStatus::Minor => "Minor injuries, walking wounded",
            TriageStatus::Deceased => "No vital signs detected",
            TriageStatus::Unknown => "Unable to determine status",
        }
    }

    /// Check if this status requires urgent attention
    pub fn is_urgent(&self) -> bool {
        matches!(self, TriageStatus::Immediate | TriageStatus::Delayed)
    }
}

impl std::fmt::Display for TriageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriageStatus::Immediate => write!(f, "IMMEDIATE (Red)"),
            TriageStatus::Delayed => write!(f, "DELAYED (Yellow)"),
            TriageStatus::Minor => write!(f, "MINOR (Green)"),
            TriageStatus::Deceased => write!(f, "DECEASED (Black)"),
            TriageStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Calculator for triage status based on vital signs
pub struct TriageCalculator;

impl TriageCalculator {
    /// Calculate triage status from vital signs reading
    ///
    /// Uses modified START protocol adapted for remote sensing:
    /// 1. Check breathing (respiration)
    /// 2. Check for movement/responsiveness (proxy for perfusion/mental status)
    /// 3. Classify based on combined assessment
    pub fn calculate(vitals: &VitalSignsReading) -> TriageStatus {
        // Step 1: Check if any vitals are detected
        if !vitals.has_vitals() {
            // No vitals at all - either deceased or signal issue
            return TriageStatus::Unknown;
        }

        // Step 2: Assess breathing
        let breathing_status = Self::assess_breathing(vitals);

        // Step 3: Assess movement/responsiveness
        let movement_status = Self::assess_movement(vitals);

        // Step 4: Combine assessments
        Self::combine_assessments(breathing_status, movement_status)
    }

    /// Assess breathing status
    fn assess_breathing(vitals: &VitalSignsReading) -> BreathingAssessment {
        match &vitals.breathing {
            None => BreathingAssessment::Absent,
            Some(breathing) => {
                // Check for agonal breathing (pre-death)
                if breathing.pattern_type == BreathingType::Agonal {
                    return BreathingAssessment::Agonal;
                }

                // Check rate
                if breathing.rate_bpm < 10.0 {
                    BreathingAssessment::TooSlow
                } else if breathing.rate_bpm > 30.0 {
                    BreathingAssessment::TooFast
                } else {
                    BreathingAssessment::Normal
                }
            }
        }
    }

    /// Assess movement/responsiveness
    fn assess_movement(vitals: &VitalSignsReading) -> MovementAssessment {
        match vitals.movement.movement_type {
            MovementType::Gross if vitals.movement.is_voluntary => {
                MovementAssessment::Responsive
            }
            MovementType::Gross => MovementAssessment::Moving,
            MovementType::Fine => MovementAssessment::MinimalMovement,
            MovementType::Tremor => MovementAssessment::InvoluntaryOnly,
            MovementType::Periodic => MovementAssessment::MinimalMovement,
            MovementType::None => MovementAssessment::None,
        }
    }

    /// Combine breathing and movement assessments into triage status
    fn combine_assessments(
        breathing: BreathingAssessment,
        movement: MovementAssessment,
    ) -> TriageStatus {
        match (breathing, movement) {
            // No breathing
            (BreathingAssessment::Absent, MovementAssessment::None) => {
                TriageStatus::Deceased
            }
            (BreathingAssessment::Agonal, _) => {
                TriageStatus::Immediate
            }
            (BreathingAssessment::Absent, _) => {
                // No breathing but movement - possible airway obstruction
                TriageStatus::Immediate
            }

            // Abnormal breathing rates
            (BreathingAssessment::TooFast, _) => {
                TriageStatus::Immediate
            }
            (BreathingAssessment::TooSlow, _) => {
                TriageStatus::Immediate
            }

            // Normal breathing with movement assessment
            (BreathingAssessment::Normal, MovementAssessment::Responsive) => {
                TriageStatus::Minor
            }
            (BreathingAssessment::Normal, MovementAssessment::Moving) => {
                TriageStatus::Delayed
            }
            (BreathingAssessment::Normal, MovementAssessment::MinimalMovement) => {
                TriageStatus::Delayed
            }
            (BreathingAssessment::Normal, MovementAssessment::InvoluntaryOnly) => {
                TriageStatus::Immediate // Not following commands
            }
            (BreathingAssessment::Normal, MovementAssessment::None) => {
                TriageStatus::Immediate // Breathing but unresponsive
            }
        }
    }

    /// Check if status should be upgraded based on deterioration
    pub fn should_upgrade(current: &TriageStatus, is_deteriorating: bool) -> bool {
        if !is_deteriorating {
            return false;
        }

        // Upgrade if not already at highest priority
        matches!(current, TriageStatus::Delayed | TriageStatus::Minor)
    }

    /// Get upgraded triage status
    pub fn upgrade(current: &TriageStatus) -> TriageStatus {
        match current {
            TriageStatus::Minor => TriageStatus::Delayed,
            TriageStatus::Delayed => TriageStatus::Immediate,
            other => other.clone(),
        }
    }
}

/// Internal breathing assessment
#[derive(Debug, Clone, Copy)]
enum BreathingAssessment {
    Normal,
    TooFast,
    TooSlow,
    Agonal,
    Absent,
}

/// Internal movement assessment
#[derive(Debug, Clone, Copy)]
enum MovementAssessment {
    Responsive,      // Voluntary purposeful movement
    Moving,          // Movement but unclear if responsive
    MinimalMovement, // Small movements only
    InvoluntaryOnly, // Only tremors/involuntary
    None,            // No movement detected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BreathingPattern, ConfidenceScore, MovementProfile};
    use chrono::Utc;

    fn create_vitals(
        breathing: Option<BreathingPattern>,
        movement: MovementProfile,
    ) -> VitalSignsReading {
        VitalSignsReading {
            breathing,
            heartbeat: None,
            movement,
            timestamp: Utc::now(),
            confidence: ConfidenceScore::new(0.8),
        }
    }

    #[test]
    fn test_no_vitals_is_unknown() {
        let vitals = create_vitals(None, MovementProfile::default());
        assert_eq!(TriageCalculator::calculate(&vitals), TriageStatus::Unknown);
    }

    #[test]
    fn test_normal_breathing_responsive_is_minor() {
        let vitals = create_vitals(
            Some(BreathingPattern {
                rate_bpm: 16.0,
                amplitude: 0.8,
                regularity: 0.9,
                pattern_type: BreathingType::Normal,
            }),
            MovementProfile {
                movement_type: MovementType::Gross,
                intensity: 0.8,
                frequency: 0.5,
                is_voluntary: true,
            },
        );
        assert_eq!(TriageCalculator::calculate(&vitals), TriageStatus::Minor);
    }

    #[test]
    fn test_fast_breathing_is_immediate() {
        let vitals = create_vitals(
            Some(BreathingPattern {
                rate_bpm: 35.0,
                amplitude: 0.7,
                regularity: 0.5,
                pattern_type: BreathingType::Labored,
            }),
            MovementProfile {
                movement_type: MovementType::Fine,
                intensity: 0.3,
                frequency: 0.2,
                is_voluntary: false,
            },
        );
        assert_eq!(TriageCalculator::calculate(&vitals), TriageStatus::Immediate);
    }

    #[test]
    fn test_slow_breathing_is_immediate() {
        let vitals = create_vitals(
            Some(BreathingPattern {
                rate_bpm: 8.0,
                amplitude: 0.5,
                regularity: 0.6,
                pattern_type: BreathingType::Shallow,
            }),
            MovementProfile {
                movement_type: MovementType::None,
                intensity: 0.0,
                frequency: 0.0,
                is_voluntary: false,
            },
        );
        assert_eq!(TriageCalculator::calculate(&vitals), TriageStatus::Immediate);
    }

    #[test]
    fn test_agonal_breathing_is_immediate() {
        let vitals = create_vitals(
            Some(BreathingPattern {
                rate_bpm: 4.0,
                amplitude: 0.3,
                regularity: 0.2,
                pattern_type: BreathingType::Agonal,
            }),
            MovementProfile::default(),
        );
        assert_eq!(TriageCalculator::calculate(&vitals), TriageStatus::Immediate);
    }

    #[test]
    fn test_triage_priority() {
        assert_eq!(TriageStatus::Immediate.priority(), 1);
        assert_eq!(TriageStatus::Delayed.priority(), 2);
        assert_eq!(TriageStatus::Minor.priority(), 3);
        assert_eq!(TriageStatus::Deceased.priority(), 4);
    }

    #[test]
    fn test_upgrade_triage() {
        assert_eq!(
            TriageCalculator::upgrade(&TriageStatus::Minor),
            TriageStatus::Delayed
        );
        assert_eq!(
            TriageCalculator::upgrade(&TriageStatus::Delayed),
            TriageStatus::Immediate
        );
        assert_eq!(
            TriageCalculator::upgrade(&TriageStatus::Immediate),
            TriageStatus::Immediate
        );
    }
}
