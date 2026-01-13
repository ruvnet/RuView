//! Data Transfer Objects (DTOs) for the MAT REST API.
//!
//! These types are used for serializing/deserializing API requests and responses.
//! They provide a clean separation between domain models and API contracts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    DisasterType, EventStatus, ZoneStatus, TriageStatus, Priority,
    AlertStatus, SurvivorStatus,
};

// ============================================================================
// Event DTOs
// ============================================================================

/// Request body for creating a new disaster event.
///
/// ## Example
///
/// ```json
/// {
///   "event_type": "Earthquake",
///   "latitude": 37.7749,
///   "longitude": -122.4194,
///   "description": "Magnitude 6.8 earthquake in San Francisco",
///   "estimated_occupancy": 500,
///   "lead_agency": "SF Fire Department"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateEventRequest {
    /// Type of disaster event
    pub event_type: DisasterTypeDto,
    /// Latitude of disaster epicenter
    pub latitude: f64,
    /// Longitude of disaster epicenter
    pub longitude: f64,
    /// Human-readable description of the event
    pub description: String,
    /// Estimated number of people in the affected area
    #[serde(default)]
    pub estimated_occupancy: Option<u32>,
    /// Lead responding agency
    #[serde(default)]
    pub lead_agency: Option<String>,
}

/// Response body for disaster event details.
///
/// ## Example Response
///
/// ```json
/// {
///   "id": "550e8400-e29b-41d4-a716-446655440000",
///   "event_type": "Earthquake",
///   "status": "Active",
///   "start_time": "2024-01-15T14:30:00Z",
///   "latitude": 37.7749,
///   "longitude": -122.4194,
///   "description": "Magnitude 6.8 earthquake",
///   "zone_count": 5,
///   "survivor_count": 12,
///   "triage_summary": {
///     "immediate": 3,
///     "delayed": 5,
///     "minor": 4,
///     "deceased": 0
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EventResponse {
    /// Unique event identifier
    pub id: Uuid,
    /// Type of disaster
    pub event_type: DisasterTypeDto,
    /// Current event status
    pub status: EventStatusDto,
    /// When the event was created/started
    pub start_time: DateTime<Utc>,
    /// Latitude of epicenter
    pub latitude: f64,
    /// Longitude of epicenter
    pub longitude: f64,
    /// Event description
    pub description: String,
    /// Number of scan zones
    pub zone_count: usize,
    /// Number of detected survivors
    pub survivor_count: usize,
    /// Summary of triage classifications
    pub triage_summary: TriageSummary,
    /// Metadata about the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<EventMetadataDto>,
}

/// Summary of triage counts across all survivors.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct TriageSummary {
    /// Immediate (Red) - life-threatening
    pub immediate: u32,
    /// Delayed (Yellow) - serious but stable
    pub delayed: u32,
    /// Minor (Green) - walking wounded
    pub minor: u32,
    /// Deceased (Black)
    pub deceased: u32,
    /// Unknown status
    pub unknown: u32,
}

/// Event metadata DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventMetadataDto {
    /// Estimated number of people in area at time of disaster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_occupancy: Option<u32>,
    /// Known survivors (already rescued)
    #[serde(default)]
    pub confirmed_rescued: u32,
    /// Known fatalities
    #[serde(default)]
    pub confirmed_deceased: u32,
    /// Weather conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    /// Lead agency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead_agency: Option<String>,
}

/// Paginated list of events.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EventListResponse {
    /// List of events
    pub events: Vec<EventResponse>,
    /// Total count of events
    pub total: usize,
    /// Current page number (0-indexed)
    pub page: usize,
    /// Number of items per page
    pub page_size: usize,
}

// ============================================================================
// Zone DTOs
// ============================================================================

/// Request body for adding a scan zone to an event.
///
/// ## Example
///
/// ```json
/// {
///   "name": "Building A - North Wing",
///   "bounds": {
///     "type": "rectangle",
///     "min_x": 0.0,
///     "min_y": 0.0,
///     "max_x": 50.0,
///     "max_y": 30.0
///   },
///   "parameters": {
///     "sensitivity": 0.85,
///     "max_depth": 5.0,
///     "heartbeat_detection": true
///   }
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateZoneRequest {
    /// Human-readable zone name
    pub name: String,
    /// Geographic bounds of the zone
    pub bounds: ZoneBoundsDto,
    /// Optional scan parameters
    #[serde(default)]
    pub parameters: Option<ScanParametersDto>,
}

/// Zone boundary definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ZoneBoundsDto {
    /// Rectangular boundary
    Rectangle {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    },
    /// Circular boundary
    Circle {
        center_x: f64,
        center_y: f64,
        radius: f64,
    },
    /// Polygon boundary (list of vertices)
    Polygon {
        vertices: Vec<(f64, f64)>,
    },
}

/// Scan parameters for a zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ScanParametersDto {
    /// Detection sensitivity (0.0-1.0)
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f64,
    /// Maximum depth to scan in meters
    #[serde(default = "default_max_depth")]
    pub max_depth: f64,
    /// Scan resolution level
    #[serde(default)]
    pub resolution: ScanResolutionDto,
    /// Enable enhanced breathing detection
    #[serde(default = "default_true")]
    pub enhanced_breathing: bool,
    /// Enable heartbeat detection (slower but more accurate)
    #[serde(default)]
    pub heartbeat_detection: bool,
}

fn default_sensitivity() -> f64 { 0.8 }
fn default_max_depth() -> f64 { 5.0 }
fn default_true() -> bool { true }

impl Default for ScanParametersDto {
    fn default() -> Self {
        Self {
            sensitivity: default_sensitivity(),
            max_depth: default_max_depth(),
            resolution: ScanResolutionDto::default(),
            enhanced_breathing: default_true(),
            heartbeat_detection: false,
        }
    }
}

/// Scan resolution levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScanResolutionDto {
    Quick,
    #[default]
    Standard,
    High,
    Maximum,
}

/// Response for zone details.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ZoneResponse {
    /// Zone identifier
    pub id: Uuid,
    /// Zone name
    pub name: String,
    /// Zone status
    pub status: ZoneStatusDto,
    /// Zone boundaries
    pub bounds: ZoneBoundsDto,
    /// Zone area in square meters
    pub area: f64,
    /// Scan parameters
    pub parameters: ScanParametersDto,
    /// Last scan time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scan: Option<DateTime<Utc>>,
    /// Total scan count
    pub scan_count: u32,
    /// Number of detections in this zone
    pub detections_count: u32,
}

/// List of zones response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ZoneListResponse {
    /// List of zones
    pub zones: Vec<ZoneResponse>,
    /// Total count
    pub total: usize,
}

// ============================================================================
// Survivor DTOs
// ============================================================================

/// Response for survivor details.
///
/// ## Example Response
///
/// ```json
/// {
///   "id": "550e8400-e29b-41d4-a716-446655440001",
///   "zone_id": "550e8400-e29b-41d4-a716-446655440002",
///   "status": "Active",
///   "triage_status": "Immediate",
///   "location": {
///     "x": 25.5,
///     "y": 12.3,
///     "z": -2.1,
///     "uncertainty_radius": 1.5
///   },
///   "vital_signs": {
///     "breathing_rate": 22.5,
///     "has_heartbeat": true,
///     "has_movement": false
///   },
///   "confidence": 0.87,
///   "first_detected": "2024-01-15T14:32:00Z",
///   "last_updated": "2024-01-15T14:45:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SurvivorResponse {
    /// Survivor identifier
    pub id: Uuid,
    /// Zone where survivor was detected
    pub zone_id: Uuid,
    /// Current survivor status
    pub status: SurvivorStatusDto,
    /// Triage classification
    pub triage_status: TriageStatusDto,
    /// Location information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationDto>,
    /// Latest vital signs summary
    pub vital_signs: VitalSignsSummaryDto,
    /// Detection confidence (0.0-1.0)
    pub confidence: f64,
    /// When survivor was first detected
    pub first_detected: DateTime<Utc>,
    /// Last update time
    pub last_updated: DateTime<Utc>,
    /// Whether survivor is deteriorating
    pub is_deteriorating: bool,
    /// Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SurvivorMetadataDto>,
}

/// Location information DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LocationDto {
    /// X coordinate (east-west, meters)
    pub x: f64,
    /// Y coordinate (north-south, meters)
    pub y: f64,
    /// Z coordinate (depth, negative is below surface)
    pub z: f64,
    /// Estimated depth below surface (positive meters)
    pub depth: f64,
    /// Horizontal uncertainty radius in meters
    pub uncertainty_radius: f64,
    /// Location confidence score
    pub confidence: f64,
}

/// Summary of vital signs for API response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct VitalSignsSummaryDto {
    /// Breathing rate (breaths per minute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breathing_rate: Option<f32>,
    /// Breathing pattern type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breathing_type: Option<String>,
    /// Heart rate if detected (bpm)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heart_rate: Option<f32>,
    /// Whether heartbeat is detected
    pub has_heartbeat: bool,
    /// Whether movement is detected
    pub has_movement: bool,
    /// Movement type if present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movement_type: Option<String>,
    /// Timestamp of reading
    pub timestamp: DateTime<Utc>,
}

/// Survivor metadata DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SurvivorMetadataDto {
    /// Estimated age category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_age_category: Option<String>,
    /// Assigned rescue team
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_team: Option<String>,
    /// Notes
    pub notes: Vec<String>,
    /// Tags
    pub tags: Vec<String>,
}

/// List of survivors response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SurvivorListResponse {
    /// List of survivors
    pub survivors: Vec<SurvivorResponse>,
    /// Total count
    pub total: usize,
    /// Triage summary
    pub triage_summary: TriageSummary,
}

// ============================================================================
// Alert DTOs
// ============================================================================

/// Response for alert details.
///
/// ## Example Response
///
/// ```json
/// {
///   "id": "550e8400-e29b-41d4-a716-446655440003",
///   "survivor_id": "550e8400-e29b-41d4-a716-446655440001",
///   "priority": "Critical",
///   "status": "Pending",
///   "title": "Immediate: Survivor detected with abnormal breathing",
///   "message": "Survivor in Zone A showing signs of respiratory distress",
///   "triage_status": "Immediate",
///   "location": { ... },
///   "created_at": "2024-01-15T14:35:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AlertResponse {
    /// Alert identifier
    pub id: Uuid,
    /// Related survivor ID
    pub survivor_id: Uuid,
    /// Alert priority
    pub priority: PriorityDto,
    /// Alert status
    pub status: AlertStatusDto,
    /// Alert title
    pub title: String,
    /// Detailed message
    pub message: String,
    /// Associated triage status
    pub triage_status: TriageStatusDto,
    /// Location if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationDto>,
    /// Recommended action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_action: Option<String>,
    /// When alert was created
    pub created_at: DateTime<Utc>,
    /// When alert was acknowledged
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// Who acknowledged the alert
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_by: Option<String>,
    /// Escalation count
    pub escalation_count: u32,
}

/// Request to acknowledge an alert.
///
/// ## Example
///
/// ```json
/// {
///   "acknowledged_by": "Team Alpha",
///   "notes": "En route to location"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcknowledgeAlertRequest {
    /// Who is acknowledging the alert
    pub acknowledged_by: String,
    /// Optional notes
    #[serde(default)]
    pub notes: Option<String>,
}

/// Response after acknowledging an alert.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AcknowledgeAlertResponse {
    /// Whether acknowledgement was successful
    pub success: bool,
    /// Updated alert
    pub alert: AlertResponse,
}

/// List of alerts response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AlertListResponse {
    /// List of alerts
    pub alerts: Vec<AlertResponse>,
    /// Total count
    pub total: usize,
    /// Count by priority
    pub priority_counts: PriorityCounts,
}

/// Count of alerts by priority.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PriorityCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

// ============================================================================
// WebSocket DTOs
// ============================================================================

/// WebSocket message types for real-time streaming.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// New survivor detected
    SurvivorDetected {
        event_id: Uuid,
        survivor: SurvivorResponse,
    },
    /// Survivor status updated
    SurvivorUpdated {
        event_id: Uuid,
        survivor: SurvivorResponse,
    },
    /// Survivor lost (signal lost)
    SurvivorLost {
        event_id: Uuid,
        survivor_id: Uuid,
    },
    /// New alert generated
    AlertCreated {
        event_id: Uuid,
        alert: AlertResponse,
    },
    /// Alert status changed
    AlertUpdated {
        event_id: Uuid,
        alert: AlertResponse,
    },
    /// Zone scan completed
    ZoneScanComplete {
        event_id: Uuid,
        zone_id: Uuid,
        detections: u32,
    },
    /// Event status changed
    EventStatusChanged {
        event_id: Uuid,
        old_status: EventStatusDto,
        new_status: EventStatusDto,
    },
    /// Heartbeat/keep-alive
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
    /// Error message
    Error {
        code: String,
        message: String,
    },
}

/// WebSocket subscription request.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WebSocketRequest {
    /// Subscribe to events for a disaster event
    Subscribe {
        event_id: Uuid,
    },
    /// Unsubscribe from events
    Unsubscribe {
        event_id: Uuid,
    },
    /// Subscribe to all events
    SubscribeAll,
    /// Request current state
    GetState {
        event_id: Uuid,
    },
}

// ============================================================================
// Enum DTOs (mirroring domain enums with serde)
// ============================================================================

/// Disaster type DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum DisasterTypeDto {
    BuildingCollapse,
    Earthquake,
    Landslide,
    Avalanche,
    Flood,
    MineCollapse,
    Industrial,
    TunnelCollapse,
    Unknown,
}

impl From<DisasterType> for DisasterTypeDto {
    fn from(dt: DisasterType) -> Self {
        match dt {
            DisasterType::BuildingCollapse => DisasterTypeDto::BuildingCollapse,
            DisasterType::Earthquake => DisasterTypeDto::Earthquake,
            DisasterType::Landslide => DisasterTypeDto::Landslide,
            DisasterType::Avalanche => DisasterTypeDto::Avalanche,
            DisasterType::Flood => DisasterTypeDto::Flood,
            DisasterType::MineCollapse => DisasterTypeDto::MineCollapse,
            DisasterType::Industrial => DisasterTypeDto::Industrial,
            DisasterType::TunnelCollapse => DisasterTypeDto::TunnelCollapse,
            DisasterType::Unknown => DisasterTypeDto::Unknown,
        }
    }
}

impl From<DisasterTypeDto> for DisasterType {
    fn from(dt: DisasterTypeDto) -> Self {
        match dt {
            DisasterTypeDto::BuildingCollapse => DisasterType::BuildingCollapse,
            DisasterTypeDto::Earthquake => DisasterType::Earthquake,
            DisasterTypeDto::Landslide => DisasterType::Landslide,
            DisasterTypeDto::Avalanche => DisasterType::Avalanche,
            DisasterTypeDto::Flood => DisasterType::Flood,
            DisasterTypeDto::MineCollapse => DisasterType::MineCollapse,
            DisasterTypeDto::Industrial => DisasterType::Industrial,
            DisasterTypeDto::TunnelCollapse => DisasterType::TunnelCollapse,
            DisasterTypeDto::Unknown => DisasterType::Unknown,
        }
    }
}

/// Event status DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EventStatusDto {
    Initializing,
    Active,
    Suspended,
    SecondarySearch,
    Closed,
}

impl From<EventStatus> for EventStatusDto {
    fn from(es: EventStatus) -> Self {
        match es {
            EventStatus::Initializing => EventStatusDto::Initializing,
            EventStatus::Active => EventStatusDto::Active,
            EventStatus::Suspended => EventStatusDto::Suspended,
            EventStatus::SecondarySearch => EventStatusDto::SecondarySearch,
            EventStatus::Closed => EventStatusDto::Closed,
        }
    }
}

/// Zone status DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ZoneStatusDto {
    Active,
    Paused,
    Complete,
    Inaccessible,
    Deactivated,
}

impl From<ZoneStatus> for ZoneStatusDto {
    fn from(zs: ZoneStatus) -> Self {
        match zs {
            ZoneStatus::Active => ZoneStatusDto::Active,
            ZoneStatus::Paused => ZoneStatusDto::Paused,
            ZoneStatus::Complete => ZoneStatusDto::Complete,
            ZoneStatus::Inaccessible => ZoneStatusDto::Inaccessible,
            ZoneStatus::Deactivated => ZoneStatusDto::Deactivated,
        }
    }
}

/// Triage status DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TriageStatusDto {
    Immediate,
    Delayed,
    Minor,
    Deceased,
    Unknown,
}

impl From<TriageStatus> for TriageStatusDto {
    fn from(ts: TriageStatus) -> Self {
        match ts {
            TriageStatus::Immediate => TriageStatusDto::Immediate,
            TriageStatus::Delayed => TriageStatusDto::Delayed,
            TriageStatus::Minor => TriageStatusDto::Minor,
            TriageStatus::Deceased => TriageStatusDto::Deceased,
            TriageStatus::Unknown => TriageStatusDto::Unknown,
        }
    }
}

/// Priority DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PriorityDto {
    Critical,
    High,
    Medium,
    Low,
}

impl From<Priority> for PriorityDto {
    fn from(p: Priority) -> Self {
        match p {
            Priority::Critical => PriorityDto::Critical,
            Priority::High => PriorityDto::High,
            Priority::Medium => PriorityDto::Medium,
            Priority::Low => PriorityDto::Low,
        }
    }
}

/// Alert status DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AlertStatusDto {
    Pending,
    Acknowledged,
    InProgress,
    Resolved,
    Cancelled,
    Expired,
}

impl From<AlertStatus> for AlertStatusDto {
    fn from(as_: AlertStatus) -> Self {
        match as_ {
            AlertStatus::Pending => AlertStatusDto::Pending,
            AlertStatus::Acknowledged => AlertStatusDto::Acknowledged,
            AlertStatus::InProgress => AlertStatusDto::InProgress,
            AlertStatus::Resolved => AlertStatusDto::Resolved,
            AlertStatus::Cancelled => AlertStatusDto::Cancelled,
            AlertStatus::Expired => AlertStatusDto::Expired,
        }
    }
}

/// Survivor status DTO.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SurvivorStatusDto {
    Active,
    Rescued,
    Lost,
    Deceased,
    FalsePositive,
}

impl From<SurvivorStatus> for SurvivorStatusDto {
    fn from(ss: SurvivorStatus) -> Self {
        match ss {
            SurvivorStatus::Active => SurvivorStatusDto::Active,
            SurvivorStatus::Rescued => SurvivorStatusDto::Rescued,
            SurvivorStatus::Lost => SurvivorStatusDto::Lost,
            SurvivorStatus::Deceased => SurvivorStatusDto::Deceased,
            SurvivorStatus::FalsePositive => SurvivorStatusDto::FalsePositive,
        }
    }
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for listing events.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ListEventsQuery {
    /// Filter by status
    pub status: Option<EventStatusDto>,
    /// Filter by disaster type
    pub event_type: Option<DisasterTypeDto>,
    /// Page number (0-indexed)
    #[serde(default)]
    pub page: usize,
    /// Page size (default 20, max 100)
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page_size() -> usize { 20 }

/// Query parameters for listing survivors.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ListSurvivorsQuery {
    /// Filter by triage status
    pub triage_status: Option<TriageStatusDto>,
    /// Filter by zone ID
    pub zone_id: Option<Uuid>,
    /// Filter by minimum confidence
    pub min_confidence: Option<f64>,
    /// Include only deteriorating
    #[serde(default)]
    pub deteriorating_only: bool,
}

/// Query parameters for listing alerts.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ListAlertsQuery {
    /// Filter by priority
    pub priority: Option<PriorityDto>,
    /// Filter by status
    pub status: Option<AlertStatusDto>,
    /// Only pending alerts
    #[serde(default)]
    pub pending_only: bool,
    /// Only active alerts
    #[serde(default)]
    pub active_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_event_request_deserialize() {
        let json = r#"{
            "event_type": "Earthquake",
            "latitude": 37.7749,
            "longitude": -122.4194,
            "description": "Test earthquake"
        }"#;

        let req: CreateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.event_type, DisasterTypeDto::Earthquake);
        assert!((req.latitude - 37.7749).abs() < 0.0001);
    }

    #[test]
    fn test_zone_bounds_dto_deserialize() {
        let rect_json = r#"{
            "type": "rectangle",
            "min_x": 0.0,
            "min_y": 0.0,
            "max_x": 10.0,
            "max_y": 10.0
        }"#;

        let bounds: ZoneBoundsDto = serde_json::from_str(rect_json).unwrap();
        assert!(matches!(bounds, ZoneBoundsDto::Rectangle { .. }));
    }

    #[test]
    fn test_websocket_message_serialize() {
        let msg = WebSocketMessage::Heartbeat {
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"heartbeat\""));
    }
}
