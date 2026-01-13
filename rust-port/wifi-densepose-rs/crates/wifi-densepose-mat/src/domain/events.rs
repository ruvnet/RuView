//! Domain events for the wifi-Mat system.

use chrono::{DateTime, Utc};

use super::{
    AlertId, Coordinates3D, Priority, ScanZoneId, SurvivorId,
    TriageStatus, VitalSignsReading, AlertResolution,
};

/// All domain events in the system
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DomainEvent {
    /// Detection-related events
    Detection(DetectionEvent),
    /// Alert-related events
    Alert(AlertEvent),
    /// Zone-related events
    Zone(ZoneEvent),
    /// System-level events
    System(SystemEvent),
}

impl DomainEvent {
    /// Get the timestamp of the event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            DomainEvent::Detection(e) => e.timestamp(),
            DomainEvent::Alert(e) => e.timestamp(),
            DomainEvent::Zone(e) => e.timestamp(),
            DomainEvent::System(e) => e.timestamp(),
        }
    }

    /// Get event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            DomainEvent::Detection(e) => e.event_type(),
            DomainEvent::Alert(e) => e.event_type(),
            DomainEvent::Zone(e) => e.event_type(),
            DomainEvent::System(e) => e.event_type(),
        }
    }
}

/// Detection-related events
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DetectionEvent {
    /// New survivor detected
    SurvivorDetected {
        survivor_id: SurvivorId,
        zone_id: ScanZoneId,
        vital_signs: VitalSignsReading,
        location: Option<Coordinates3D>,
        timestamp: DateTime<Utc>,
    },

    /// Survivor vital signs updated
    VitalsUpdated {
        survivor_id: SurvivorId,
        previous_triage: TriageStatus,
        current_triage: TriageStatus,
        confidence: f64,
        timestamp: DateTime<Utc>,
    },

    /// Survivor triage status changed
    TriageStatusChanged {
        survivor_id: SurvivorId,
        previous: TriageStatus,
        current: TriageStatus,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Survivor location refined
    LocationRefined {
        survivor_id: SurvivorId,
        previous: Option<Coordinates3D>,
        current: Coordinates3D,
        uncertainty_reduced: bool,
        timestamp: DateTime<Utc>,
    },

    /// Survivor no longer detected
    SurvivorLost {
        survivor_id: SurvivorId,
        last_detection: DateTime<Utc>,
        reason: LostReason,
        timestamp: DateTime<Utc>,
    },

    /// Survivor rescued
    SurvivorRescued {
        survivor_id: SurvivorId,
        rescue_team: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// Survivor marked deceased
    SurvivorDeceased {
        survivor_id: SurvivorId,
        timestamp: DateTime<Utc>,
    },
}

impl DetectionEvent {
    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SurvivorDetected { timestamp, .. } => *timestamp,
            Self::VitalsUpdated { timestamp, .. } => *timestamp,
            Self::TriageStatusChanged { timestamp, .. } => *timestamp,
            Self::LocationRefined { timestamp, .. } => *timestamp,
            Self::SurvivorLost { timestamp, .. } => *timestamp,
            Self::SurvivorRescued { timestamp, .. } => *timestamp,
            Self::SurvivorDeceased { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SurvivorDetected { .. } => "SurvivorDetected",
            Self::VitalsUpdated { .. } => "VitalsUpdated",
            Self::TriageStatusChanged { .. } => "TriageStatusChanged",
            Self::LocationRefined { .. } => "LocationRefined",
            Self::SurvivorLost { .. } => "SurvivorLost",
            Self::SurvivorRescued { .. } => "SurvivorRescued",
            Self::SurvivorDeceased { .. } => "SurvivorDeceased",
        }
    }

    /// Get the survivor ID associated with this event
    pub fn survivor_id(&self) -> &SurvivorId {
        match self {
            Self::SurvivorDetected { survivor_id, .. } => survivor_id,
            Self::VitalsUpdated { survivor_id, .. } => survivor_id,
            Self::TriageStatusChanged { survivor_id, .. } => survivor_id,
            Self::LocationRefined { survivor_id, .. } => survivor_id,
            Self::SurvivorLost { survivor_id, .. } => survivor_id,
            Self::SurvivorRescued { survivor_id, .. } => survivor_id,
            Self::SurvivorDeceased { survivor_id, .. } => survivor_id,
        }
    }
}

/// Reasons for losing a survivor signal
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LostReason {
    /// Survivor was rescued (signal expected to stop)
    Rescued,
    /// Detection determined to be false positive
    FalsePositive,
    /// Signal lost (interference, debris shift, etc.)
    SignalLost,
    /// Zone was deactivated
    ZoneDeactivated,
    /// Sensor malfunction
    SensorFailure,
    /// Unknown reason
    Unknown,
}

/// Alert-related events
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlertEvent {
    /// New alert generated
    AlertGenerated {
        alert_id: AlertId,
        survivor_id: SurvivorId,
        priority: Priority,
        timestamp: DateTime<Utc>,
    },

    /// Alert acknowledged by rescue team
    AlertAcknowledged {
        alert_id: AlertId,
        acknowledged_by: String,
        timestamp: DateTime<Utc>,
    },

    /// Alert escalated
    AlertEscalated {
        alert_id: AlertId,
        previous_priority: Priority,
        new_priority: Priority,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Alert resolved
    AlertResolved {
        alert_id: AlertId,
        resolution: AlertResolution,
        timestamp: DateTime<Utc>,
    },

    /// Alert cancelled
    AlertCancelled {
        alert_id: AlertId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl AlertEvent {
    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::AlertGenerated { timestamp, .. } => *timestamp,
            Self::AlertAcknowledged { timestamp, .. } => *timestamp,
            Self::AlertEscalated { timestamp, .. } => *timestamp,
            Self::AlertResolved { timestamp, .. } => *timestamp,
            Self::AlertCancelled { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AlertGenerated { .. } => "AlertGenerated",
            Self::AlertAcknowledged { .. } => "AlertAcknowledged",
            Self::AlertEscalated { .. } => "AlertEscalated",
            Self::AlertResolved { .. } => "AlertResolved",
            Self::AlertCancelled { .. } => "AlertCancelled",
        }
    }

    /// Get the alert ID associated with this event
    pub fn alert_id(&self) -> &AlertId {
        match self {
            Self::AlertGenerated { alert_id, .. } => alert_id,
            Self::AlertAcknowledged { alert_id, .. } => alert_id,
            Self::AlertEscalated { alert_id, .. } => alert_id,
            Self::AlertResolved { alert_id, .. } => alert_id,
            Self::AlertCancelled { alert_id, .. } => alert_id,
        }
    }
}

/// Zone-related events
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ZoneEvent {
    /// Zone activated
    ZoneActivated {
        zone_id: ScanZoneId,
        zone_name: String,
        timestamp: DateTime<Utc>,
    },

    /// Zone scan completed
    ZoneScanCompleted {
        zone_id: ScanZoneId,
        detections_found: u32,
        scan_duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Zone paused
    ZonePaused {
        zone_id: ScanZoneId,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Zone resumed
    ZoneResumed {
        zone_id: ScanZoneId,
        timestamp: DateTime<Utc>,
    },

    /// Zone marked complete
    ZoneCompleted {
        zone_id: ScanZoneId,
        total_survivors_found: u32,
        timestamp: DateTime<Utc>,
    },

    /// Zone deactivated
    ZoneDeactivated {
        zone_id: ScanZoneId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl ZoneEvent {
    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::ZoneActivated { timestamp, .. } => *timestamp,
            Self::ZoneScanCompleted { timestamp, .. } => *timestamp,
            Self::ZonePaused { timestamp, .. } => *timestamp,
            Self::ZoneResumed { timestamp, .. } => *timestamp,
            Self::ZoneCompleted { timestamp, .. } => *timestamp,
            Self::ZoneDeactivated { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ZoneActivated { .. } => "ZoneActivated",
            Self::ZoneScanCompleted { .. } => "ZoneScanCompleted",
            Self::ZonePaused { .. } => "ZonePaused",
            Self::ZoneResumed { .. } => "ZoneResumed",
            Self::ZoneCompleted { .. } => "ZoneCompleted",
            Self::ZoneDeactivated { .. } => "ZoneDeactivated",
        }
    }

    /// Get the zone ID associated with this event
    pub fn zone_id(&self) -> &ScanZoneId {
        match self {
            Self::ZoneActivated { zone_id, .. } => zone_id,
            Self::ZoneScanCompleted { zone_id, .. } => zone_id,
            Self::ZonePaused { zone_id, .. } => zone_id,
            Self::ZoneResumed { zone_id, .. } => zone_id,
            Self::ZoneCompleted { zone_id, .. } => zone_id,
            Self::ZoneDeactivated { zone_id, .. } => zone_id,
        }
    }
}

/// System-level events
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SystemEvent {
    /// System started
    SystemStarted {
        version: String,
        timestamp: DateTime<Utc>,
    },

    /// System stopped
    SystemStopped {
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Sensor connected
    SensorConnected {
        sensor_id: String,
        zone_id: ScanZoneId,
        timestamp: DateTime<Utc>,
    },

    /// Sensor disconnected
    SensorDisconnected {
        sensor_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Configuration changed
    ConfigChanged {
        setting: String,
        previous_value: String,
        new_value: String,
        timestamp: DateTime<Utc>,
    },

    /// Error occurred
    ErrorOccurred {
        error_type: String,
        message: String,
        severity: ErrorSeverity,
        timestamp: DateTime<Utc>,
    },
}

impl SystemEvent {
    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SystemStarted { timestamp, .. } => *timestamp,
            Self::SystemStopped { timestamp, .. } => *timestamp,
            Self::SensorConnected { timestamp, .. } => *timestamp,
            Self::SensorDisconnected { timestamp, .. } => *timestamp,
            Self::ConfigChanged { timestamp, .. } => *timestamp,
            Self::ErrorOccurred { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SystemStarted { .. } => "SystemStarted",
            Self::SystemStopped { .. } => "SystemStopped",
            Self::SensorConnected { .. } => "SensorConnected",
            Self::SensorDisconnected { .. } => "SensorDisconnected",
            Self::ConfigChanged { .. } => "ConfigChanged",
            Self::ErrorOccurred { .. } => "ErrorOccurred",
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorSeverity {
    /// Warning - operation continues
    Warning,
    /// Error - operation may be affected
    Error,
    /// Critical - immediate attention required
    Critical,
}

/// Event store for persisting domain events
pub trait EventStore: Send + Sync {
    /// Append an event to the store
    fn append(&self, event: DomainEvent) -> Result<(), crate::MatError>;

    /// Get all events
    fn all(&self) -> Result<Vec<DomainEvent>, crate::MatError>;

    /// Get events since a timestamp
    fn since(&self, timestamp: DateTime<Utc>) -> Result<Vec<DomainEvent>, crate::MatError>;

    /// Get events for a specific survivor
    fn for_survivor(&self, survivor_id: &SurvivorId) -> Result<Vec<DomainEvent>, crate::MatError>;
}

/// In-memory event store implementation
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    events: parking_lot::RwLock<Vec<DomainEvent>>,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: DomainEvent) -> Result<(), crate::MatError> {
        self.events.write().push(event);
        Ok(())
    }

    fn all(&self) -> Result<Vec<DomainEvent>, crate::MatError> {
        Ok(self.events.read().clone())
    }

    fn since(&self, timestamp: DateTime<Utc>) -> Result<Vec<DomainEvent>, crate::MatError> {
        Ok(self
            .events
            .read()
            .iter()
            .filter(|e| e.timestamp() >= timestamp)
            .cloned()
            .collect())
    }

    fn for_survivor(&self, survivor_id: &SurvivorId) -> Result<Vec<DomainEvent>, crate::MatError> {
        Ok(self
            .events
            .read()
            .iter()
            .filter(|e| {
                if let DomainEvent::Detection(de) = e {
                    de.survivor_id() == survivor_id
                } else {
                    false
                }
            })
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_event_store() {
        let store = InMemoryEventStore::new();

        let event = DomainEvent::System(SystemEvent::SystemStarted {
            version: "1.0.0".to_string(),
            timestamp: Utc::now(),
        });

        store.append(event).unwrap();
        let events = store.all().unwrap();
        assert_eq!(events.len(), 1);
    }
}
