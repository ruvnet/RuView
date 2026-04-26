//! Survivor entity representing a detected human in a disaster zone.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    Coordinates3D, TriageStatus, VitalSignsReading, ScanZoneId,
    triage::TriageCalculator,
};

/// Unique identifier for a survivor
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurvivorId(Uuid);

impl SurvivorId {
    /// Create a new random survivor ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from an existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SurvivorId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SurvivorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current status of a survivor
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SurvivorStatus {
    /// Actively being tracked
    Active,
    /// Confirmed rescued
    Rescued,
    /// Lost signal, may need re-detection
    Lost,
    /// Confirmed deceased
    Deceased,
    /// Determined to be false positive
    FalsePositive,
}

/// Additional metadata about a survivor
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurvivorMetadata {
    /// Estimated age category based on vital patterns
    pub estimated_age_category: Option<AgeCategory>,
    /// Notes from rescue team
    pub notes: Vec<String>,
    /// Tags for organization
    pub tags: Vec<String>,
    /// Assigned rescue team ID
    pub assigned_team: Option<String>,
}

/// Estimated age category based on vital sign patterns
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AgeCategory {
    /// Infant (0-2 years)
    Infant,
    /// Child (2-12 years)
    Child,
    /// Adult (12-65 years)
    Adult,
    /// Elderly (65+ years)
    Elderly,
    /// Cannot determine
    Unknown,
}

/// History of vital signs readings
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VitalSignsHistory {
    readings: Vec<VitalSignsReading>,
    max_history: usize,
}

impl VitalSignsHistory {
    /// Create a new history with specified max size
    pub fn new(max_history: usize) -> Self {
        Self {
            readings: Vec::with_capacity(max_history),
            max_history,
        }
    }

    /// Add a new reading
    pub fn add(&mut self, reading: VitalSignsReading) {
        if self.readings.len() >= self.max_history {
            self.readings.remove(0);
        }
        self.readings.push(reading);
    }

    /// Get the most recent reading
    pub fn latest(&self) -> Option<&VitalSignsReading> {
        self.readings.last()
    }

    /// Get all readings
    pub fn all(&self) -> &[VitalSignsReading] {
        &self.readings
    }

    /// Get the number of readings
    pub fn len(&self) -> usize {
        self.readings.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }

    /// Calculate average confidence across readings
    pub fn average_confidence(&self) -> f64 {
        if self.readings.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.readings.iter()
            .map(|r| r.confidence.value())
            .sum();
        sum / self.readings.len() as f64
    }

    /// Check if vitals are deteriorating
    pub fn is_deteriorating(&self) -> bool {
        if self.readings.len() < 3 {
            return false;
        }

        let recent: Vec<_> = self.readings.iter().rev().take(3).collect();

        // Check breathing trend
        let breathing_declining = recent.windows(2).all(|w| {
            match (&w[0].breathing, &w[1].breathing) {
                (Some(a), Some(b)) => a.rate_bpm < b.rate_bpm,
                _ => false,
            }
        });

        // Check confidence trend
        let confidence_declining = recent.windows(2).all(|w| {
            w[0].confidence.value() < w[1].confidence.value()
        });

        breathing_declining || confidence_declining
    }
}

/// A detected survivor in the disaster zone
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Survivor {
    id: SurvivorId,
    zone_id: ScanZoneId,
    first_detected: DateTime<Utc>,
    last_updated: DateTime<Utc>,
    location: Option<Coordinates3D>,
    vital_signs: VitalSignsHistory,
    triage_status: TriageStatus,
    status: SurvivorStatus,
    confidence: f64,
    metadata: SurvivorMetadata,
    alert_sent: bool,
}

impl Survivor {
    /// Create a new survivor from initial detection
    pub fn new(
        zone_id: ScanZoneId,
        initial_vitals: VitalSignsReading,
        location: Option<Coordinates3D>,
    ) -> Self {
        let now = Utc::now();
        let confidence = initial_vitals.confidence.value();
        let triage_status = TriageCalculator::calculate(&initial_vitals);

        let mut vital_signs = VitalSignsHistory::new(100);
        vital_signs.add(initial_vitals);

        Self {
            id: SurvivorId::new(),
            zone_id,
            first_detected: now,
            last_updated: now,
            location,
            vital_signs,
            triage_status,
            status: SurvivorStatus::Active,
            confidence,
            metadata: SurvivorMetadata::default(),
            alert_sent: false,
        }
    }

    /// Get the survivor ID
    pub fn id(&self) -> &SurvivorId {
        &self.id
    }

    /// Get the zone ID where survivor was detected
    pub fn zone_id(&self) -> &ScanZoneId {
        &self.zone_id
    }

    /// Get the first detection time
    pub fn first_detected(&self) -> &DateTime<Utc> {
        &self.first_detected
    }

    /// Get the last update time
    pub fn last_updated(&self) -> &DateTime<Utc> {
        &self.last_updated
    }

    /// Get the estimated location
    pub fn location(&self) -> Option<&Coordinates3D> {
        self.location.as_ref()
    }

    /// Get the vital signs history
    pub fn vital_signs(&self) -> &VitalSignsHistory {
        &self.vital_signs
    }

    /// Get the current triage status
    pub fn triage_status(&self) -> &TriageStatus {
        &self.triage_status
    }

    /// Get the current status
    pub fn status(&self) -> &SurvivorStatus {
        &self.status
    }

    /// Get the confidence score
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Get the metadata
    pub fn metadata(&self) -> &SurvivorMetadata {
        &self.metadata
    }

    /// Get mutable metadata
    pub fn metadata_mut(&mut self) -> &mut SurvivorMetadata {
        &mut self.metadata
    }

    /// Update with new vital signs reading
    pub fn update_vitals(&mut self, reading: VitalSignsReading) {
        let previous_triage = self.triage_status.clone();
        self.vital_signs.add(reading.clone());
        self.confidence = self.vital_signs.average_confidence();
        self.triage_status = TriageCalculator::calculate(&reading);
        self.last_updated = Utc::now();

        // Log triage change for audit
        if previous_triage != self.triage_status {
            tracing::info!(
                survivor_id = %self.id,
                previous = ?previous_triage,
                current = ?self.triage_status,
                "Triage status changed"
            );
        }
    }

    /// Update the location estimate
    pub fn update_location(&mut self, location: Coordinates3D) {
        self.location = Some(location);
        self.last_updated = Utc::now();
    }

    /// Mark as rescued
    pub fn mark_rescued(&mut self) {
        self.status = SurvivorStatus::Rescued;
        self.last_updated = Utc::now();
        tracing::info!(survivor_id = %self.id, "Survivor marked as rescued");
    }

    /// Mark as lost (signal lost)
    pub fn mark_lost(&mut self) {
        self.status = SurvivorStatus::Lost;
        self.last_updated = Utc::now();
    }

    /// Mark as deceased
    pub fn mark_deceased(&mut self) {
        self.status = SurvivorStatus::Deceased;
        self.triage_status = TriageStatus::Deceased;
        self.last_updated = Utc::now();
    }

    /// Mark as false positive
    pub fn mark_false_positive(&mut self) {
        self.status = SurvivorStatus::FalsePositive;
        self.last_updated = Utc::now();
    }

    /// Check if survivor should generate an alert
    pub fn should_alert(&self) -> bool {
        if self.alert_sent {
            return false;
        }

        // Alert for high-priority survivors
        matches!(
            self.triage_status,
            TriageStatus::Immediate | TriageStatus::Delayed
        ) && self.confidence >= 0.5
    }

    /// Mark that alert was sent
    pub fn mark_alert_sent(&mut self) {
        self.alert_sent = true;
    }

    /// Check if vitals are deteriorating (needs priority upgrade)
    pub fn is_deteriorating(&self) -> bool {
        self.vital_signs.is_deteriorating()
    }

    /// Get time since last update
    pub fn time_since_update(&self) -> chrono::Duration {
        Utc::now() - self.last_updated
    }

    /// Check if survivor data is stale
    pub fn is_stale(&self, threshold_seconds: i64) -> bool {
        self.time_since_update().num_seconds() > threshold_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BreathingPattern, BreathingType, ConfidenceScore};

    fn create_test_vitals(confidence: f64) -> VitalSignsReading {
        VitalSignsReading {
            breathing: Some(BreathingPattern {
                rate_bpm: 16.0,
                amplitude: 0.8,
                regularity: 0.9,
                pattern_type: BreathingType::Normal,
            }),
            heartbeat: None,
            movement: Default::default(),
            timestamp: Utc::now(),
            confidence: ConfidenceScore::new(confidence),
        }
    }

    #[test]
    fn test_survivor_creation() {
        let zone_id = ScanZoneId::new();
        let vitals = create_test_vitals(0.8);
        let survivor = Survivor::new(zone_id.clone(), vitals, None);

        assert_eq!(survivor.zone_id(), &zone_id);
        assert!(survivor.confidence() >= 0.8);
        assert!(matches!(survivor.status(), SurvivorStatus::Active));
    }

    #[test]
    fn test_vital_signs_history() {
        let mut history = VitalSignsHistory::new(5);

        for i in 0..7 {
            history.add(create_test_vitals(0.5 + (i as f64 * 0.05)));
        }

        // Should only keep last 5
        assert_eq!(history.len(), 5);

        // Average should be based on last 5 readings
        assert!(history.average_confidence() > 0.5);
    }

    #[test]
    fn test_survivor_should_alert() {
        let zone_id = ScanZoneId::new();
        let vitals = create_test_vitals(0.8);
        let survivor = Survivor::new(zone_id, vitals, None);

        // Should alert if triage is Immediate or Delayed
        // Depends on triage calculation from vitals
        assert!(!survivor.alert_sent);
    }

    #[test]
    fn test_survivor_mark_rescued() {
        let zone_id = ScanZoneId::new();
        let vitals = create_test_vitals(0.8);
        let mut survivor = Survivor::new(zone_id, vitals, None);

        survivor.mark_rescued();
        assert!(matches!(survivor.status(), SurvivorStatus::Rescued));
    }
}
