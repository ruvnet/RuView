//! Axum request handlers for the MAT REST API.
//!
//! This module contains all the HTTP endpoint handlers for disaster response operations.
//! Each handler is documented with OpenAPI-style documentation comments.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use geo::Point;
use uuid::Uuid;

use super::dto::*;
use super::error::{ApiError, ApiResult};
use super::state::AppState;
use crate::domain::{
    DisasterEvent, DisasterType, ScanZone, ZoneBounds,
    ScanParameters, ScanResolution, MovementType,
};

// ============================================================================
// Event Handlers
// ============================================================================

/// List all disaster events.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events:
///   get:
///     summary: List disaster events
///     description: Returns a paginated list of disaster events with optional filtering
///     tags: [Events]
///     parameters:
///       - name: status
///         in: query
///         description: Filter by event status
///         schema:
///           type: string
///           enum: [Initializing, Active, Suspended, SecondarySearch, Closed]
///       - name: event_type
///         in: query
///         description: Filter by disaster type
///         schema:
///           type: string
///       - name: page
///         in: query
///         description: Page number (0-indexed)
///         schema:
///           type: integer
///           default: 0
///       - name: page_size
///         in: query
///         description: Items per page (max 100)
///         schema:
///           type: integer
///           default: 20
///     responses:
///       200:
///         description: List of events
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/EventListResponse'
/// ```
#[tracing::instrument(skip(state))]
pub async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<ListEventsQuery>,
) -> ApiResult<Json<EventListResponse>> {
    let all_events = state.list_events();

    // Apply filters
    let filtered: Vec<_> = all_events
        .into_iter()
        .filter(|e| {
            if let Some(ref status) = query.status {
                let event_status: EventStatusDto = e.status().clone().into();
                if !matches_status(&event_status, status) {
                    return false;
                }
            }
            if let Some(ref event_type) = query.event_type {
                let et: DisasterTypeDto = e.event_type().clone().into();
                if et != *event_type {
                    return false;
                }
            }
            true
        })
        .collect();

    let total = filtered.len();

    // Apply pagination
    let page_size = query.page_size.min(100).max(1);
    let start = query.page * page_size;
    let events: Vec<_> = filtered
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(event_to_response)
        .collect();

    Ok(Json(EventListResponse {
        events,
        total,
        page: query.page,
        page_size,
    }))
}

/// Create a new disaster event.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events:
///   post:
///     summary: Create a new disaster event
///     description: Creates a new disaster event for search and rescue operations
///     tags: [Events]
///     requestBody:
///       required: true
///       content:
///         application/json:
///           schema:
///             $ref: '#/components/schemas/CreateEventRequest'
///     responses:
///       201:
///         description: Event created successfully
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/EventResponse'
///       400:
///         description: Invalid request data
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/ErrorResponse'
/// ```
#[tracing::instrument(skip(state))]
pub async fn create_event(
    State(state): State<AppState>,
    Json(request): Json<CreateEventRequest>,
) -> ApiResult<(StatusCode, Json<EventResponse>)> {
    // Validate coordinates
    if request.latitude < -90.0 || request.latitude > 90.0 {
        return Err(ApiError::validation(
            "Latitude must be between -90 and 90",
            Some("latitude".to_string()),
        ));
    }
    if request.longitude < -180.0 || request.longitude > 180.0 {
        return Err(ApiError::validation(
            "Longitude must be between -180 and 180",
            Some("longitude".to_string()),
        ));
    }

    let disaster_type: DisasterType = request.event_type.into();
    let location = Point::new(request.longitude, request.latitude);
    let mut event = DisasterEvent::new(disaster_type, location, &request.description);

    // Set metadata if provided
    if let Some(occupancy) = request.estimated_occupancy {
        event.metadata_mut().estimated_occupancy = Some(occupancy);
    }
    if let Some(agency) = request.lead_agency {
        event.metadata_mut().lead_agency = Some(agency);
    }

    let response = event_to_response(event.clone());
    let event_id = *event.id().as_uuid();
    state.store_event(event);

    // Broadcast event creation
    state.broadcast(WebSocketMessage::EventStatusChanged {
        event_id,
        old_status: EventStatusDto::Initializing,
        new_status: response.status,
    });

    tracing::info!(event_id = %event_id, "Created new disaster event");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Get a specific disaster event by ID.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events/{event_id}:
///   get:
///     summary: Get event details
///     description: Returns detailed information about a specific disaster event
///     tags: [Events]
///     parameters:
///       - name: event_id
///         in: path
///         required: true
///         description: Event UUID
///         schema:
///           type: string
///           format: uuid
///     responses:
///       200:
///         description: Event details
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/EventResponse'
///       404:
///         description: Event not found
/// ```
#[tracing::instrument(skip(state))]
pub async fn get_event(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> ApiResult<Json<EventResponse>> {
    let event = state
        .get_event(event_id)
        .ok_or_else(|| ApiError::event_not_found(event_id))?;

    Ok(Json(event_to_response(event)))
}

// ============================================================================
// Zone Handlers
// ============================================================================

/// List all zones for a disaster event.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events/{event_id}/zones:
///   get:
///     summary: List zones for an event
///     description: Returns all scan zones configured for a disaster event
///     tags: [Zones]
///     parameters:
///       - name: event_id
///         in: path
///         required: true
///         schema:
///           type: string
///           format: uuid
///     responses:
///       200:
///         description: List of zones
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/ZoneListResponse'
///       404:
///         description: Event not found
/// ```
#[tracing::instrument(skip(state))]
pub async fn list_zones(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> ApiResult<Json<ZoneListResponse>> {
    let event = state
        .get_event(event_id)
        .ok_or_else(|| ApiError::event_not_found(event_id))?;

    let zones: Vec<_> = event.zones().iter().map(zone_to_response).collect();
    let total = zones.len();

    Ok(Json(ZoneListResponse { zones, total }))
}

/// Add a scan zone to a disaster event.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events/{event_id}/zones:
///   post:
///     summary: Add a scan zone
///     description: Creates a new scan zone within a disaster event area
///     tags: [Zones]
///     parameters:
///       - name: event_id
///         in: path
///         required: true
///         schema:
///           type: string
///           format: uuid
///     requestBody:
///       required: true
///       content:
///         application/json:
///           schema:
///             $ref: '#/components/schemas/CreateZoneRequest'
///     responses:
///       201:
///         description: Zone created successfully
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/ZoneResponse'
///       404:
///         description: Event not found
///       400:
///         description: Invalid zone configuration
/// ```
#[tracing::instrument(skip(state))]
pub async fn add_zone(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateZoneRequest>,
) -> ApiResult<(StatusCode, Json<ZoneResponse>)> {
    // Convert DTO to domain
    let bounds = match request.bounds {
        ZoneBoundsDto::Rectangle { min_x, min_y, max_x, max_y } => {
            if max_x <= min_x || max_y <= min_y {
                return Err(ApiError::validation(
                    "max coordinates must be greater than min coordinates",
                    Some("bounds".to_string()),
                ));
            }
            ZoneBounds::rectangle(min_x, min_y, max_x, max_y)
        }
        ZoneBoundsDto::Circle { center_x, center_y, radius } => {
            if radius <= 0.0 {
                return Err(ApiError::validation(
                    "radius must be positive",
                    Some("bounds.radius".to_string()),
                ));
            }
            ZoneBounds::circle(center_x, center_y, radius)
        }
        ZoneBoundsDto::Polygon { vertices } => {
            if vertices.len() < 3 {
                return Err(ApiError::validation(
                    "polygon must have at least 3 vertices",
                    Some("bounds.vertices".to_string()),
                ));
            }
            ZoneBounds::polygon(vertices)
        }
    };

    let params = if let Some(p) = request.parameters {
        ScanParameters {
            sensitivity: p.sensitivity.clamp(0.0, 1.0),
            max_depth: p.max_depth.max(0.0),
            resolution: match p.resolution {
                ScanResolutionDto::Quick => ScanResolution::Quick,
                ScanResolutionDto::Standard => ScanResolution::Standard,
                ScanResolutionDto::High => ScanResolution::High,
                ScanResolutionDto::Maximum => ScanResolution::Maximum,
            },
            enhanced_breathing: p.enhanced_breathing,
            heartbeat_detection: p.heartbeat_detection,
        }
    } else {
        ScanParameters::default()
    };

    let zone = ScanZone::with_parameters(&request.name, bounds, params);
    let zone_response = zone_to_response(&zone);
    let zone_id = *zone.id().as_uuid();

    // Add zone to event
    let added = state.update_event(event_id, move |e| {
        e.add_zone(zone);
        true
    });

    if added.is_none() {
        return Err(ApiError::event_not_found(event_id));
    }

    tracing::info!(event_id = %event_id, zone_id = %zone_id, "Added scan zone");

    Ok((StatusCode::CREATED, Json(zone_response)))
}

// ============================================================================
// Survivor Handlers
// ============================================================================

/// List survivors detected in a disaster event.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events/{event_id}/survivors:
///   get:
///     summary: List survivors
///     description: Returns all detected survivors in a disaster event
///     tags: [Survivors]
///     parameters:
///       - name: event_id
///         in: path
///         required: true
///         schema:
///           type: string
///           format: uuid
///       - name: triage_status
///         in: query
///         description: Filter by triage status
///         schema:
///           type: string
///           enum: [Immediate, Delayed, Minor, Deceased, Unknown]
///       - name: zone_id
///         in: query
///         description: Filter by zone
///         schema:
///           type: string
///           format: uuid
///       - name: min_confidence
///         in: query
///         description: Minimum confidence threshold
///         schema:
///           type: number
///       - name: deteriorating_only
///         in: query
///         description: Only return deteriorating survivors
///         schema:
///           type: boolean
///     responses:
///       200:
///         description: List of survivors
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/SurvivorListResponse'
///       404:
///         description: Event not found
/// ```
#[tracing::instrument(skip(state))]
pub async fn list_survivors(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListSurvivorsQuery>,
) -> ApiResult<Json<SurvivorListResponse>> {
    let event = state
        .get_event(event_id)
        .ok_or_else(|| ApiError::event_not_found(event_id))?;

    let mut triage_summary = TriageSummary::default();
    let survivors: Vec<_> = event
        .survivors()
        .into_iter()
        .filter(|s| {
            // Update triage counts for all survivors
            update_triage_summary(&mut triage_summary, s.triage_status());

            // Apply filters
            if let Some(ref ts) = query.triage_status {
                let survivor_triage: TriageStatusDto = s.triage_status().clone().into();
                if !matches_triage_status(&survivor_triage, ts) {
                    return false;
                }
            }
            if let Some(zone_id) = query.zone_id {
                if s.zone_id().as_uuid() != &zone_id {
                    return false;
                }
            }
            if let Some(min_conf) = query.min_confidence {
                if s.confidence() < min_conf {
                    return false;
                }
            }
            if query.deteriorating_only && !s.is_deteriorating() {
                return false;
            }
            true
        })
        .map(survivor_to_response)
        .collect();

    let total = survivors.len();

    Ok(Json(SurvivorListResponse {
        survivors,
        total,
        triage_summary,
    }))
}

// ============================================================================
// Alert Handlers
// ============================================================================

/// List alerts for a disaster event.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/events/{event_id}/alerts:
///   get:
///     summary: List alerts
///     description: Returns all alerts generated for a disaster event
///     tags: [Alerts]
///     parameters:
///       - name: event_id
///         in: path
///         required: true
///         schema:
///           type: string
///           format: uuid
///       - name: priority
///         in: query
///         description: Filter by priority
///         schema:
///           type: string
///           enum: [Critical, High, Medium, Low]
///       - name: status
///         in: query
///         description: Filter by status
///         schema:
///           type: string
///       - name: pending_only
///         in: query
///         description: Only return pending alerts
///         schema:
///           type: boolean
///       - name: active_only
///         in: query
///         description: Only return active alerts
///         schema:
///           type: boolean
///     responses:
///       200:
///         description: List of alerts
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/AlertListResponse'
///       404:
///         description: Event not found
/// ```
#[tracing::instrument(skip(state))]
pub async fn list_alerts(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListAlertsQuery>,
) -> ApiResult<Json<AlertListResponse>> {
    // Verify event exists
    if state.get_event(event_id).is_none() {
        return Err(ApiError::event_not_found(event_id));
    }

    let all_alerts = state.list_alerts_for_event(event_id);
    let mut priority_counts = PriorityCounts::default();

    let alerts: Vec<_> = all_alerts
        .into_iter()
        .filter(|a| {
            // Update priority counts
            update_priority_counts(&mut priority_counts, a.priority());

            // Apply filters
            if let Some(ref priority) = query.priority {
                let alert_priority: PriorityDto = a.priority().into();
                if !matches_priority(&alert_priority, priority) {
                    return false;
                }
            }
            if let Some(ref status) = query.status {
                let alert_status: AlertStatusDto = a.status().clone().into();
                if !matches_alert_status(&alert_status, status) {
                    return false;
                }
            }
            if query.pending_only && !a.is_pending() {
                return false;
            }
            if query.active_only && !a.is_active() {
                return false;
            }
            true
        })
        .map(|a| alert_to_response(&a))
        .collect();

    let total = alerts.len();

    Ok(Json(AlertListResponse {
        alerts,
        total,
        priority_counts,
    }))
}

/// Acknowledge an alert.
///
/// # OpenAPI Specification
///
/// ```yaml
/// /api/v1/mat/alerts/{alert_id}/acknowledge:
///   post:
///     summary: Acknowledge an alert
///     description: Marks an alert as acknowledged by a rescue team
///     tags: [Alerts]
///     parameters:
///       - name: alert_id
///         in: path
///         required: true
///         schema:
///           type: string
///           format: uuid
///     requestBody:
///       required: true
///       content:
///         application/json:
///           schema:
///             $ref: '#/components/schemas/AcknowledgeAlertRequest'
///     responses:
///       200:
///         description: Alert acknowledged
///         content:
///           application/json:
///             schema:
///               $ref: '#/components/schemas/AcknowledgeAlertResponse'
///       404:
///         description: Alert not found
///       409:
///         description: Alert already acknowledged
/// ```
#[tracing::instrument(skip(state))]
pub async fn acknowledge_alert(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
    Json(request): Json<AcknowledgeAlertRequest>,
) -> ApiResult<Json<AcknowledgeAlertResponse>> {
    let alert_data = state
        .get_alert(alert_id)
        .ok_or_else(|| ApiError::alert_not_found(alert_id))?;

    if !alert_data.alert.is_pending() {
        return Err(ApiError::InvalidState {
            message: "Alert is not in pending state".to_string(),
            current_state: format!("{:?}", alert_data.alert.status()),
        });
    }

    let event_id = alert_data.event_id;

    // Acknowledge the alert
    state.update_alert(alert_id, |a| {
        a.acknowledge(&request.acknowledged_by);
    });

    // Get updated alert
    let updated = state
        .get_alert(alert_id)
        .ok_or_else(|| ApiError::alert_not_found(alert_id))?;

    let response = alert_to_response(&updated.alert);

    // Broadcast update
    state.broadcast(WebSocketMessage::AlertUpdated {
        event_id,
        alert: response.clone(),
    });

    tracing::info!(
        alert_id = %alert_id,
        acknowledged_by = %request.acknowledged_by,
        "Alert acknowledged"
    );

    Ok(Json(AcknowledgeAlertResponse {
        success: true,
        alert: response,
    }))
}

// ============================================================================
// Helper Functions
// ============================================================================

fn event_to_response(event: DisasterEvent) -> EventResponse {
    let triage_counts = event.triage_counts();

    EventResponse {
        id: *event.id().as_uuid(),
        event_type: event.event_type().clone().into(),
        status: event.status().clone().into(),
        start_time: *event.start_time(),
        latitude: event.location().y(),
        longitude: event.location().x(),
        description: event.description().to_string(),
        zone_count: event.zones().len(),
        survivor_count: event.survivors().len(),
        triage_summary: TriageSummary {
            immediate: triage_counts.immediate,
            delayed: triage_counts.delayed,
            minor: triage_counts.minor,
            deceased: triage_counts.deceased,
            unknown: triage_counts.unknown,
        },
        metadata: Some(EventMetadataDto {
            estimated_occupancy: event.metadata().estimated_occupancy,
            confirmed_rescued: event.metadata().confirmed_rescued,
            confirmed_deceased: event.metadata().confirmed_deceased,
            weather: event.metadata().weather.clone(),
            lead_agency: event.metadata().lead_agency.clone(),
        }),
    }
}

fn zone_to_response(zone: &ScanZone) -> ZoneResponse {
    let bounds = match zone.bounds() {
        ZoneBounds::Rectangle { min_x, min_y, max_x, max_y } => {
            ZoneBoundsDto::Rectangle {
                min_x: *min_x,
                min_y: *min_y,
                max_x: *max_x,
                max_y: *max_y,
            }
        }
        ZoneBounds::Circle { center_x, center_y, radius } => {
            ZoneBoundsDto::Circle {
                center_x: *center_x,
                center_y: *center_y,
                radius: *radius,
            }
        }
        ZoneBounds::Polygon { vertices } => {
            ZoneBoundsDto::Polygon {
                vertices: vertices.clone(),
            }
        }
    };

    let params = zone.parameters();
    let parameters = ScanParametersDto {
        sensitivity: params.sensitivity,
        max_depth: params.max_depth,
        resolution: match params.resolution {
            ScanResolution::Quick => ScanResolutionDto::Quick,
            ScanResolution::Standard => ScanResolutionDto::Standard,
            ScanResolution::High => ScanResolutionDto::High,
            ScanResolution::Maximum => ScanResolutionDto::Maximum,
        },
        enhanced_breathing: params.enhanced_breathing,
        heartbeat_detection: params.heartbeat_detection,
    };

    ZoneResponse {
        id: *zone.id().as_uuid(),
        name: zone.name().to_string(),
        status: zone.status().clone().into(),
        bounds,
        area: zone.area(),
        parameters,
        last_scan: zone.last_scan().cloned(),
        scan_count: zone.scan_count(),
        detections_count: zone.detections_count(),
    }
}

fn survivor_to_response(survivor: &crate::Survivor) -> SurvivorResponse {
    let location = survivor.location().map(|loc| LocationDto {
        x: loc.x,
        y: loc.y,
        z: loc.z,
        depth: loc.depth(),
        uncertainty_radius: loc.uncertainty.horizontal_error,
        confidence: loc.uncertainty.confidence,
    });

    let latest_vitals = survivor.vital_signs().latest();
    let vital_signs = VitalSignsSummaryDto {
        breathing_rate: latest_vitals.and_then(|v| v.breathing.as_ref().map(|b| b.rate_bpm)),
        breathing_type: latest_vitals.and_then(|v| v.breathing.as_ref().map(|b| format!("{:?}", b.pattern_type))),
        heart_rate: latest_vitals.and_then(|v| v.heartbeat.as_ref().map(|h| h.rate_bpm)),
        has_heartbeat: latest_vitals.map(|v| v.has_heartbeat()).unwrap_or(false),
        has_movement: latest_vitals.map(|v| v.has_movement()).unwrap_or(false),
        movement_type: latest_vitals.and_then(|v| {
            if v.movement.movement_type != MovementType::None {
                Some(format!("{:?}", v.movement.movement_type))
            } else {
                None
            }
        }),
        timestamp: latest_vitals.map(|v| v.timestamp).unwrap_or_else(chrono::Utc::now),
    };

    let metadata = {
        let m = survivor.metadata();
        if m.notes.is_empty() && m.tags.is_empty() && m.assigned_team.is_none() {
            None
        } else {
            Some(SurvivorMetadataDto {
                estimated_age_category: m.estimated_age_category.as_ref().map(|a| format!("{:?}", a)),
                assigned_team: m.assigned_team.clone(),
                notes: m.notes.clone(),
                tags: m.tags.clone(),
            })
        }
    };

    SurvivorResponse {
        id: *survivor.id().as_uuid(),
        zone_id: *survivor.zone_id().as_uuid(),
        status: survivor.status().clone().into(),
        triage_status: survivor.triage_status().clone().into(),
        location,
        vital_signs,
        confidence: survivor.confidence(),
        first_detected: *survivor.first_detected(),
        last_updated: *survivor.last_updated(),
        is_deteriorating: survivor.is_deteriorating(),
        metadata,
    }
}

fn alert_to_response(alert: &crate::Alert) -> AlertResponse {
    let location = alert.payload().location.as_ref().map(|loc| LocationDto {
        x: loc.x,
        y: loc.y,
        z: loc.z,
        depth: loc.depth(),
        uncertainty_radius: loc.uncertainty.horizontal_error,
        confidence: loc.uncertainty.confidence,
    });

    AlertResponse {
        id: *alert.id().as_uuid(),
        survivor_id: *alert.survivor_id().as_uuid(),
        priority: alert.priority().into(),
        status: alert.status().clone().into(),
        title: alert.payload().title.clone(),
        message: alert.payload().message.clone(),
        triage_status: alert.payload().triage_status.clone().into(),
        location,
        recommended_action: if alert.payload().recommended_action.is_empty() {
            None
        } else {
            Some(alert.payload().recommended_action.clone())
        },
        created_at: *alert.created_at(),
        acknowledged_at: alert.acknowledged_at().cloned(),
        acknowledged_by: alert.acknowledged_by().map(String::from),
        escalation_count: alert.escalation_count(),
    }
}

fn update_triage_summary(summary: &mut TriageSummary, status: &crate::TriageStatus) {
    match status {
        crate::TriageStatus::Immediate => summary.immediate += 1,
        crate::TriageStatus::Delayed => summary.delayed += 1,
        crate::TriageStatus::Minor => summary.minor += 1,
        crate::TriageStatus::Deceased => summary.deceased += 1,
        crate::TriageStatus::Unknown => summary.unknown += 1,
    }
}

fn update_priority_counts(counts: &mut PriorityCounts, priority: crate::Priority) {
    match priority {
        crate::Priority::Critical => counts.critical += 1,
        crate::Priority::High => counts.high += 1,
        crate::Priority::Medium => counts.medium += 1,
        crate::Priority::Low => counts.low += 1,
    }
}

// Match helper functions (avoiding PartialEq on DTOs for flexibility)
fn matches_status(a: &EventStatusDto, b: &EventStatusDto) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn matches_triage_status(a: &TriageStatusDto, b: &TriageStatusDto) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn matches_priority(a: &PriorityDto, b: &PriorityDto) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn matches_alert_status(a: &AlertStatusDto, b: &AlertStatusDto) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}
