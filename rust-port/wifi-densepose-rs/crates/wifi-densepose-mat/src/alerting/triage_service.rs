//! Triage service for calculating and updating survivor priority.

use crate::domain::{
    Priority, Survivor, TriageStatus, VitalSignsReading,
    triage::TriageCalculator,
};

/// Service for triage operations
pub struct TriageService;

impl TriageService {
    /// Calculate triage status from vital signs
    pub fn calculate_triage(vitals: &VitalSignsReading) -> TriageStatus {
        TriageCalculator::calculate(vitals)
    }

    /// Check if survivor should be upgraded
    pub fn should_upgrade(survivor: &Survivor) -> bool {
        TriageCalculator::should_upgrade(
            survivor.triage_status(),
            survivor.is_deteriorating(),
        )
    }

    /// Get upgraded status
    pub fn upgrade_status(current: &TriageStatus) -> TriageStatus {
        TriageCalculator::upgrade(current)
    }

    /// Evaluate overall severity for multiple survivors
    pub fn evaluate_mass_casualty(survivors: &[&Survivor]) -> MassCasualtyAssessment {
        let total = survivors.len() as u32;

        let mut immediate = 0u32;
        let mut delayed = 0u32;
        let mut minor = 0u32;
        let mut deceased = 0u32;
        let mut unknown = 0u32;

        for survivor in survivors {
            match survivor.triage_status() {
                TriageStatus::Immediate => immediate += 1,
                TriageStatus::Delayed => delayed += 1,
                TriageStatus::Minor => minor += 1,
                TriageStatus::Deceased => deceased += 1,
                TriageStatus::Unknown => unknown += 1,
            }
        }

        let severity = Self::calculate_severity(immediate, delayed, total);
        let resource_level = Self::calculate_resource_level(immediate, delayed, minor);

        MassCasualtyAssessment {
            total,
            immediate,
            delayed,
            minor,
            deceased,
            unknown,
            severity,
            resource_level,
        }
    }

    /// Calculate overall severity level
    fn calculate_severity(immediate: u32, delayed: u32, total: u32) -> SeverityLevel {
        if total == 0 {
            return SeverityLevel::Minimal;
        }

        let critical_ratio = (immediate + delayed) as f64 / total as f64;

        if immediate >= 10 || critical_ratio > 0.5 {
            SeverityLevel::Critical
        } else if immediate >= 5 || critical_ratio > 0.3 {
            SeverityLevel::Major
        } else if immediate >= 1 || critical_ratio > 0.1 {
            SeverityLevel::Moderate
        } else {
            SeverityLevel::Minimal
        }
    }

    /// Calculate resource level needed
    fn calculate_resource_level(immediate: u32, delayed: u32, minor: u32) -> ResourceLevel {
        // Each immediate needs ~4 rescuers
        // Each delayed needs ~2 rescuers
        // Each minor needs ~0.5 rescuers
        let rescuers_needed = immediate * 4 + delayed * 2 + minor / 2;

        if rescuers_needed >= 100 {
            ResourceLevel::MutualAid
        } else if rescuers_needed >= 50 {
            ResourceLevel::MultiAgency
        } else if rescuers_needed >= 20 {
            ResourceLevel::Enhanced
        } else if rescuers_needed >= 5 {
            ResourceLevel::Standard
        } else {
            ResourceLevel::Minimal
        }
    }
}

/// Calculator for alert priority
pub struct PriorityCalculator;

impl PriorityCalculator {
    /// Calculate priority from triage status
    pub fn from_triage(status: &TriageStatus) -> Priority {
        Priority::from_triage(status)
    }

    /// Calculate priority with additional factors
    pub fn calculate_with_factors(
        status: &TriageStatus,
        deteriorating: bool,
        time_since_detection_mins: u64,
        depth_meters: Option<f64>,
    ) -> Priority {
        let base_priority = Priority::from_triage(status);

        // Adjust for deterioration
        let priority = if deteriorating && base_priority != Priority::Critical {
            match base_priority {
                Priority::High => Priority::Critical,
                Priority::Medium => Priority::High,
                Priority::Low => Priority::Medium,
                Priority::Critical => Priority::Critical,
            }
        } else {
            base_priority
        };

        // Adjust for time (longer = more urgent)
        let priority = if time_since_detection_mins > 30 && priority == Priority::Medium {
            Priority::High
        } else {
            priority
        };

        // Adjust for depth (deeper = more complex rescue)
        if let Some(depth) = depth_meters {
            if depth > 3.0 && priority == Priority::High {
                return Priority::Critical;
            }
        }

        priority
    }
}

/// Mass casualty assessment result
#[derive(Debug, Clone)]
pub struct MassCasualtyAssessment {
    /// Total survivors detected
    pub total: u32,
    /// Immediate (Red) count
    pub immediate: u32,
    /// Delayed (Yellow) count
    pub delayed: u32,
    /// Minor (Green) count
    pub minor: u32,
    /// Deceased (Black) count
    pub deceased: u32,
    /// Unknown count
    pub unknown: u32,
    /// Overall severity level
    pub severity: SeverityLevel,
    /// Resource level needed
    pub resource_level: ResourceLevel,
}

impl MassCasualtyAssessment {
    /// Get count of living survivors
    pub fn living(&self) -> u32 {
        self.immediate + self.delayed + self.minor
    }

    /// Get count needing active rescue
    pub fn needs_rescue(&self) -> u32 {
        self.immediate + self.delayed
    }

    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "MCI Assessment:\n\
             Total: {} (Living: {}, Deceased: {})\n\
             Immediate: {}, Delayed: {}, Minor: {}\n\
             Severity: {:?}, Resources: {:?}",
            self.total, self.living(), self.deceased,
            self.immediate, self.delayed, self.minor,
            self.severity, self.resource_level
        )
    }
}

/// Severity levels for mass casualty incidents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityLevel {
    /// Few or no critical patients
    Minimal,
    /// Some critical patients, manageable
    Moderate,
    /// Many critical patients, challenging
    Major,
    /// Overwhelming number of critical patients
    Critical,
}

/// Resource levels for response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLevel {
    /// Standard response adequate
    Minimal,
    /// Standard response needed
    Standard,
    /// Enhanced response needed
    Enhanced,
    /// Multi-agency response needed
    MultiAgency,
    /// Regional mutual aid required
    MutualAid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        BreathingPattern, BreathingType, ConfidenceScore, ScanZoneId,
    };
    use chrono::Utc;

    fn create_test_vitals(rate_bpm: f32) -> VitalSignsReading {
        VitalSignsReading {
            breathing: Some(BreathingPattern {
                rate_bpm,
                amplitude: 0.8,
                regularity: 0.9,
                pattern_type: BreathingType::Normal,
            }),
            heartbeat: None,
            movement: Default::default(),
            timestamp: Utc::now(),
            confidence: ConfidenceScore::new(0.8),
        }
    }

    #[test]
    fn test_calculate_triage() {
        let normal = create_test_vitals(16.0);
        assert!(matches!(
            TriageService::calculate_triage(&normal),
            TriageStatus::Immediate | TriageStatus::Delayed | TriageStatus::Minor
        ));

        let fast = create_test_vitals(35.0);
        assert!(matches!(
            TriageService::calculate_triage(&fast),
            TriageStatus::Immediate
        ));
    }

    #[test]
    fn test_priority_from_triage() {
        assert_eq!(
            PriorityCalculator::from_triage(&TriageStatus::Immediate),
            Priority::Critical
        );
        assert_eq!(
            PriorityCalculator::from_triage(&TriageStatus::Delayed),
            Priority::High
        );
    }

    #[test]
    fn test_mass_casualty_assessment() {
        let survivors: Vec<Survivor> = (0..10)
            .map(|i| {
                let rate = if i < 3 { 35.0 } else if i < 6 { 16.0 } else { 18.0 };
                Survivor::new(
                    ScanZoneId::new(),
                    create_test_vitals(rate),
                    None,
                )
            })
            .collect();

        let survivor_refs: Vec<&Survivor> = survivors.iter().collect();
        let assessment = TriageService::evaluate_mass_casualty(&survivor_refs);

        assert_eq!(assessment.total, 10);
        assert!(assessment.living() >= assessment.needs_rescue());
    }

    #[test]
    fn test_priority_with_factors() {
        // Deteriorating patient should be upgraded
        let priority = PriorityCalculator::calculate_with_factors(
            &TriageStatus::Delayed,
            true,
            0,
            None,
        );
        assert_eq!(priority, Priority::Critical);

        // Deep burial should upgrade
        let priority = PriorityCalculator::calculate_with_factors(
            &TriageStatus::Delayed,
            false,
            0,
            Some(4.0),
        );
        assert_eq!(priority, Priority::Critical);
    }
}
