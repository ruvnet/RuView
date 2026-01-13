//! Disaster event aggregate root.

use chrono::{DateTime, Utc};
use uuid::Uuid;
use geo::Point;

use super::{
    Survivor, SurvivorId, ScanZone, ScanZoneId,
    VitalSignsReading, Coordinates3D,
};
use crate::MatError;

/// Unique identifier for a disaster event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisasterEventId(Uuid);

impl DisasterEventId {
    /// Create a new random event ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for DisasterEventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DisasterEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of disaster events
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DisasterType {
    /// Building collapse (explosion, structural failure)
    BuildingCollapse,
    /// Earthquake
    Earthquake,
    /// Landslide or mudslide
    Landslide,
    /// Avalanche (snow)
    Avalanche,
    /// Flood
    Flood,
    /// Mine collapse
    MineCollapse,
    /// Industrial accident
    Industrial,
    /// Tunnel collapse
    TunnelCollapse,
    /// Unknown or other
    Unknown,
}

impl DisasterType {
    /// Get typical debris profile for this disaster type
    pub fn typical_debris_profile(&self) -> super::DebrisProfile {
        use super::{DebrisProfile, DebrisMaterial, MoistureLevel, MetalContent};

        match self {
            DisasterType::BuildingCollapse => DebrisProfile {
                primary_material: DebrisMaterial::Mixed,
                void_fraction: 0.25,
                moisture_content: MoistureLevel::Dry,
                metal_content: MetalContent::Moderate,
            },
            DisasterType::Earthquake => DebrisProfile {
                primary_material: DebrisMaterial::HeavyConcrete,
                void_fraction: 0.2,
                moisture_content: MoistureLevel::Dry,
                metal_content: MetalContent::Moderate,
            },
            DisasterType::Avalanche => DebrisProfile {
                primary_material: DebrisMaterial::Snow,
                void_fraction: 0.4,
                moisture_content: MoistureLevel::Wet,
                metal_content: MetalContent::None,
            },
            DisasterType::Landslide => DebrisProfile {
                primary_material: DebrisMaterial::Soil,
                void_fraction: 0.15,
                moisture_content: MoistureLevel::Wet,
                metal_content: MetalContent::None,
            },
            DisasterType::Flood => DebrisProfile {
                primary_material: DebrisMaterial::Mixed,
                void_fraction: 0.3,
                moisture_content: MoistureLevel::Saturated,
                metal_content: MetalContent::Low,
            },
            DisasterType::MineCollapse | DisasterType::TunnelCollapse => DebrisProfile {
                primary_material: DebrisMaterial::Soil,
                void_fraction: 0.2,
                moisture_content: MoistureLevel::Damp,
                metal_content: MetalContent::Low,
            },
            DisasterType::Industrial => DebrisProfile {
                primary_material: DebrisMaterial::Metal,
                void_fraction: 0.35,
                moisture_content: MoistureLevel::Dry,
                metal_content: MetalContent::High,
            },
            DisasterType::Unknown => DebrisProfile::default(),
        }
    }

    /// Get expected maximum survival time (hours)
    pub fn expected_survival_hours(&self) -> u32 {
        match self {
            DisasterType::Avalanche => 2,        // Limited air, hypothermia
            DisasterType::Flood => 6,            // Drowning risk
            DisasterType::MineCollapse => 72,    // Air supply critical
            DisasterType::BuildingCollapse => 96,
            DisasterType::Earthquake => 120,
            DisasterType::Landslide => 48,
            DisasterType::TunnelCollapse => 72,
            DisasterType::Industrial => 72,
            DisasterType::Unknown => 72,
        }
    }
}

impl Default for DisasterType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Current status of the disaster event
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventStatus {
    /// Event just reported, setting up
    Initializing,
    /// Active search and rescue
    Active,
    /// Search suspended (weather, safety)
    Suspended,
    /// Primary rescue complete, secondary search
    SecondarySearch,
    /// Event closed
    Closed,
}

/// Aggregate root for a disaster event
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisasterEvent {
    id: DisasterEventId,
    event_type: DisasterType,
    start_time: DateTime<Utc>,
    location: Point<f64>,
    description: String,
    scan_zones: Vec<ScanZone>,
    survivors: Vec<Survivor>,
    status: EventStatus,
    metadata: EventMetadata,
}

/// Additional metadata for a disaster event
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventMetadata {
    /// Estimated number of people in area at time of disaster
    pub estimated_occupancy: Option<u32>,
    /// Known survivors (already rescued)
    pub confirmed_rescued: u32,
    /// Known fatalities
    pub confirmed_deceased: u32,
    /// Weather conditions
    pub weather: Option<String>,
    /// Lead agency
    pub lead_agency: Option<String>,
    /// Notes
    pub notes: Vec<String>,
}

impl DisasterEvent {
    /// Create a new disaster event
    pub fn new(
        event_type: DisasterType,
        location: Point<f64>,
        description: &str,
    ) -> Self {
        Self {
            id: DisasterEventId::new(),
            event_type,
            start_time: Utc::now(),
            location,
            description: description.to_string(),
            scan_zones: Vec::new(),
            survivors: Vec::new(),
            status: EventStatus::Initializing,
            metadata: EventMetadata::default(),
        }
    }

    /// Get the event ID
    pub fn id(&self) -> &DisasterEventId {
        &self.id
    }

    /// Get the event type
    pub fn event_type(&self) -> &DisasterType {
        &self.event_type
    }

    /// Get the start time
    pub fn start_time(&self) -> &DateTime<Utc> {
        &self.start_time
    }

    /// Get the location
    pub fn location(&self) -> &Point<f64> {
        &self.location
    }

    /// Get the description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the scan zones
    pub fn zones(&self) -> &[ScanZone] {
        &self.scan_zones
    }

    /// Get mutable scan zones
    pub fn zones_mut(&mut self) -> &mut [ScanZone] {
        &mut self.scan_zones
    }

    /// Get the survivors
    pub fn survivors(&self) -> Vec<&Survivor> {
        self.survivors.iter().collect()
    }

    /// Get mutable survivors
    pub fn survivors_mut(&mut self) -> &mut [Survivor] {
        &mut self.survivors
    }

    /// Get the current status
    pub fn status(&self) -> &EventStatus {
        &self.status
    }

    /// Get metadata
    pub fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    /// Get mutable metadata
    pub fn metadata_mut(&mut self) -> &mut EventMetadata {
        &mut self.metadata
    }

    /// Add a scan zone
    pub fn add_zone(&mut self, zone: ScanZone) {
        self.scan_zones.push(zone);

        // Activate event if first zone
        if self.status == EventStatus::Initializing {
            self.status = EventStatus::Active;
        }
    }

    /// Remove a scan zone
    pub fn remove_zone(&mut self, zone_id: &ScanZoneId) {
        self.scan_zones.retain(|z| z.id() != zone_id);
    }

    /// Record a new detection
    pub fn record_detection(
        &mut self,
        zone_id: ScanZoneId,
        vitals: VitalSignsReading,
        location: Option<Coordinates3D>,
    ) -> Result<&Survivor, MatError> {
        // Check if this might be an existing survivor
        let existing_id = if let Some(loc) = &location {
            self.find_nearby_survivor(loc, 2.0).cloned()
        } else {
            None
        };

        if let Some(existing) = existing_id {
            // Update existing survivor
            let survivor = self.survivors.iter_mut()
                .find(|s| s.id() == &existing)
                .ok_or_else(|| MatError::Domain("Survivor not found".into()))?;
            survivor.update_vitals(vitals);
            if let Some(l) = location {
                survivor.update_location(l);
            }
            return Ok(survivor);
        }

        // Create new survivor
        let survivor = Survivor::new(zone_id, vitals, location);
        self.survivors.push(survivor);
        Ok(self.survivors.last().unwrap())
    }

    /// Find a survivor near a location
    fn find_nearby_survivor(&self, location: &Coordinates3D, radius: f64) -> Option<&SurvivorId> {
        for survivor in &self.survivors {
            if let Some(loc) = survivor.location() {
                if loc.distance_to(location) < radius {
                    return Some(survivor.id());
                }
            }
        }
        None
    }

    /// Get survivor by ID
    pub fn get_survivor(&self, id: &SurvivorId) -> Option<&Survivor> {
        self.survivors.iter().find(|s| s.id() == id)
    }

    /// Get mutable survivor by ID
    pub fn get_survivor_mut(&mut self, id: &SurvivorId) -> Option<&mut Survivor> {
        self.survivors.iter_mut().find(|s| s.id() == id)
    }

    /// Get zone by ID
    pub fn get_zone(&self, id: &ScanZoneId) -> Option<&ScanZone> {
        self.scan_zones.iter().find(|z| z.id() == id)
    }

    /// Set event status
    pub fn set_status(&mut self, status: EventStatus) {
        self.status = status;
    }

    /// Suspend operations
    pub fn suspend(&mut self, reason: &str) {
        self.status = EventStatus::Suspended;
        self.metadata.notes.push(format!(
            "[{}] Suspended: {}",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            reason
        ));
    }

    /// Resume operations
    pub fn resume(&mut self) {
        if self.status == EventStatus::Suspended {
            self.status = EventStatus::Active;
            self.metadata.notes.push(format!(
                "[{}] Resumed operations",
                Utc::now().format("%Y-%m-%d %H:%M:%S")
            ));
        }
    }

    /// Close the event
    pub fn close(&mut self) {
        self.status = EventStatus::Closed;
    }

    /// Get time since event started
    pub fn elapsed_time(&self) -> chrono::Duration {
        Utc::now() - self.start_time
    }

    /// Get count of survivors by triage status
    pub fn triage_counts(&self) -> TriageCounts {
        use super::TriageStatus;

        let mut counts = TriageCounts::default();
        for survivor in &self.survivors {
            match survivor.triage_status() {
                TriageStatus::Immediate => counts.immediate += 1,
                TriageStatus::Delayed => counts.delayed += 1,
                TriageStatus::Minor => counts.minor += 1,
                TriageStatus::Deceased => counts.deceased += 1,
                TriageStatus::Unknown => counts.unknown += 1,
            }
        }
        counts
    }
}

/// Triage status counts
#[derive(Debug, Clone, Default)]
pub struct TriageCounts {
    /// Immediate (Red)
    pub immediate: u32,
    /// Delayed (Yellow)
    pub delayed: u32,
    /// Minor (Green)
    pub minor: u32,
    /// Deceased (Black)
    pub deceased: u32,
    /// Unknown
    pub unknown: u32,
}

impl TriageCounts {
    /// Total count
    pub fn total(&self) -> u32 {
        self.immediate + self.delayed + self.minor + self.deceased + self.unknown
    }

    /// Count of living survivors
    pub fn living(&self) -> u32 {
        self.immediate + self.delayed + self.minor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ZoneBounds, BreathingPattern, BreathingType, ConfidenceScore};

    fn create_test_vitals() -> VitalSignsReading {
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
            confidence: ConfidenceScore::new(0.8),
        }
    }

    #[test]
    fn test_event_creation() {
        let event = DisasterEvent::new(
            DisasterType::Earthquake,
            Point::new(-122.4194, 37.7749),
            "Test earthquake event",
        );

        assert!(matches!(event.event_type(), DisasterType::Earthquake));
        assert_eq!(event.status(), &EventStatus::Initializing);
    }

    #[test]
    fn test_add_zone_activates_event() {
        let mut event = DisasterEvent::new(
            DisasterType::BuildingCollapse,
            Point::new(0.0, 0.0),
            "Test",
        );

        assert_eq!(event.status(), &EventStatus::Initializing);

        let zone = ScanZone::new("Zone A", ZoneBounds::rectangle(0.0, 0.0, 10.0, 10.0));
        event.add_zone(zone);

        assert_eq!(event.status(), &EventStatus::Active);
    }

    #[test]
    fn test_record_detection() {
        let mut event = DisasterEvent::new(
            DisasterType::Earthquake,
            Point::new(0.0, 0.0),
            "Test",
        );

        let zone = ScanZone::new("Zone A", ZoneBounds::rectangle(0.0, 0.0, 10.0, 10.0));
        let zone_id = zone.id().clone();
        event.add_zone(zone);

        let vitals = create_test_vitals();
        event.record_detection(zone_id, vitals, None).unwrap();

        assert_eq!(event.survivors().len(), 1);
    }

    #[test]
    fn test_disaster_type_survival_hours() {
        assert!(DisasterType::Avalanche.expected_survival_hours() < DisasterType::Earthquake.expected_survival_hours());
    }
}
