//! WiFi-Mat WASM bindings for browser-based disaster response dashboard.
//!
//! This module provides JavaScript-callable functions for:
//! - Creating and managing disaster events
//! - Adding/removing scan zones with canvas coordinates
//! - Getting survivor list with positions
//! - Subscribing to real-time updates via callbacks
//!
//! # Example Usage (JavaScript)
//!
//! ```javascript
//! import init, { MatDashboard } from './wifi_densepose_wasm.js';
//!
//! async function main() {
//!     await init();
//!
//!     const dashboard = MatDashboard.new();
//!
//!     // Create a disaster event
//!     const eventId = dashboard.createEvent('earthquake', 37.7749, -122.4194, 'Bay Area Earthquake');
//!
//!     // Add scan zones
//!     dashboard.addRectangleZone('Zone A', 0, 0, 100, 80);
//!     dashboard.addCircleZone('Zone B', 200, 150, 50);
//!
//!     // Subscribe to updates
//!     dashboard.onSurvivorDetected((survivor) => {
//!         console.log('Survivor detected:', survivor);
//!     });
//!
//!     dashboard.onAlertGenerated((alert) => {
//!         console.log('Alert:', alert);
//!     });
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// ============================================================================
// TypeScript Type Definitions (exported via JSDoc-style comments)
// ============================================================================

/// JavaScript-friendly disaster type enumeration
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsDisasterType {
    BuildingCollapse = 0,
    Earthquake = 1,
    Landslide = 2,
    Avalanche = 3,
    Flood = 4,
    MineCollapse = 5,
    Industrial = 6,
    TunnelCollapse = 7,
    Unknown = 8,
}

impl Default for JsDisasterType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// JavaScript-friendly triage status
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsTriageStatus {
    /// Immediate (Red) - Life-threatening
    Immediate = 0,
    /// Delayed (Yellow) - Serious but stable
    Delayed = 1,
    /// Minor (Green) - Walking wounded
    Minor = 2,
    /// Deceased (Black)
    Deceased = 3,
    /// Unknown
    Unknown = 4,
}

impl JsTriageStatus {
    /// Get the CSS color for this triage status
    pub fn color(&self) -> &'static str {
        match self {
            JsTriageStatus::Immediate => "#ff0000",
            JsTriageStatus::Delayed => "#ffcc00",
            JsTriageStatus::Minor => "#00cc00",
            JsTriageStatus::Deceased => "#333333",
            JsTriageStatus::Unknown => "#999999",
        }
    }

    /// Get priority (1 = highest)
    pub fn priority(&self) -> u8 {
        match self {
            JsTriageStatus::Immediate => 1,
            JsTriageStatus::Delayed => 2,
            JsTriageStatus::Minor => 3,
            JsTriageStatus::Deceased => 4,
            JsTriageStatus::Unknown => 5,
        }
    }
}

/// JavaScript-friendly zone status
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsZoneStatus {
    Active = 0,
    Paused = 1,
    Complete = 2,
    Inaccessible = 3,
}

/// JavaScript-friendly alert priority
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsAlertPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

// ============================================================================
// JavaScript-Compatible Data Structures
// ============================================================================

/// Survivor data for JavaScript consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen(getter_with_clone)]
pub struct JsSurvivor {
    /// Unique identifier
    pub id: String,
    /// Zone ID where detected
    pub zone_id: String,
    /// X position on canvas (pixels)
    pub x: f64,
    /// Y position on canvas (pixels)
    pub y: f64,
    /// Estimated depth in meters (negative = buried)
    pub depth: f64,
    /// Triage status (0=Immediate, 1=Delayed, 2=Minor, 3=Deceased, 4=Unknown)
    pub triage_status: u8,
    /// Triage color for rendering
    pub triage_color: String,
    /// Detection confidence (0.0-1.0)
    pub confidence: f64,
    /// Breathing rate (breaths per minute), -1 if not detected
    pub breathing_rate: f64,
    /// Heart rate (beats per minute), -1 if not detected
    pub heart_rate: f64,
    /// First detection timestamp (ISO 8601)
    pub first_detected: String,
    /// Last update timestamp (ISO 8601)
    pub last_updated: String,
    /// Whether survivor is deteriorating
    pub is_deteriorating: bool,
}

#[wasm_bindgen]
impl JsSurvivor {
    /// Get triage status as enum
    #[wasm_bindgen(getter)]
    pub fn triage(&self) -> JsTriageStatus {
        match self.triage_status {
            0 => JsTriageStatus::Immediate,
            1 => JsTriageStatus::Delayed,
            2 => JsTriageStatus::Minor,
            3 => JsTriageStatus::Deceased,
            _ => JsTriageStatus::Unknown,
        }
    }

    /// Check if survivor needs urgent attention
    #[wasm_bindgen]
    pub fn is_urgent(&self) -> bool {
        self.triage_status <= 1
    }
}

/// Scan zone data for JavaScript consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen(getter_with_clone)]
pub struct JsScanZone {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Zone type: "rectangle", "circle", "polygon"
    pub zone_type: String,
    /// Status (0=Active, 1=Paused, 2=Complete, 3=Inaccessible)
    pub status: u8,
    /// Number of scans completed
    pub scan_count: u32,
    /// Number of detections in this zone
    pub detection_count: u32,
    /// Zone bounds as JSON string
    pub bounds_json: String,
}

/// Alert data for JavaScript consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen(getter_with_clone)]
pub struct JsAlert {
    /// Unique identifier
    pub id: String,
    /// Related survivor ID
    pub survivor_id: String,
    /// Priority (0=Critical, 1=High, 2=Medium, 3=Low)
    pub priority: u8,
    /// Alert title
    pub title: String,
    /// Alert message
    pub message: String,
    /// Recommended action
    pub recommended_action: String,
    /// Triage status of survivor
    pub triage_status: u8,
    /// Location X (canvas pixels)
    pub location_x: f64,
    /// Location Y (canvas pixels)
    pub location_y: f64,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Priority color for rendering
    pub priority_color: String,
}

/// Dashboard statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen(getter_with_clone)]
pub struct JsDashboardStats {
    /// Total survivors detected
    pub total_survivors: u32,
    /// Immediate (red) count
    pub immediate_count: u32,
    /// Delayed (yellow) count
    pub delayed_count: u32,
    /// Minor (green) count
    pub minor_count: u32,
    /// Deceased (black) count
    pub deceased_count: u32,
    /// Unknown count
    pub unknown_count: u32,
    /// Total active zones
    pub active_zones: u32,
    /// Total scans performed
    pub total_scans: u32,
    /// Active alerts count
    pub active_alerts: u32,
    /// Event elapsed time in seconds
    pub elapsed_seconds: f64,
}

// ============================================================================
// Internal State Management
// ============================================================================

/// Internal survivor state
#[derive(Debug, Clone)]
struct InternalSurvivor {
    id: Uuid,
    zone_id: Uuid,
    x: f64,
    y: f64,
    depth: f64,
    triage_status: JsTriageStatus,
    confidence: f64,
    breathing_rate: Option<f64>,
    heart_rate: Option<f64>,
    first_detected: chrono::DateTime<chrono::Utc>,
    last_updated: chrono::DateTime<chrono::Utc>,
    is_deteriorating: bool,
    alert_sent: bool,
}

impl InternalSurvivor {
    fn to_js(&self) -> JsSurvivor {
        JsSurvivor {
            id: self.id.to_string(),
            zone_id: self.zone_id.to_string(),
            x: self.x,
            y: self.y,
            depth: self.depth,
            triage_status: self.triage_status as u8,
            triage_color: self.triage_status.color().to_string(),
            confidence: self.confidence,
            breathing_rate: self.breathing_rate.unwrap_or(-1.0),
            heart_rate: self.heart_rate.unwrap_or(-1.0),
            first_detected: self.first_detected.to_rfc3339(),
            last_updated: self.last_updated.to_rfc3339(),
            is_deteriorating: self.is_deteriorating,
        }
    }
}

/// Zone bounds variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ZoneBounds {
    Rectangle {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Circle {
        center_x: f64,
        center_y: f64,
        radius: f64,
    },
    Polygon {
        vertices: Vec<(f64, f64)>,
    },
}

/// Internal zone state
#[derive(Debug, Clone)]
struct InternalZone {
    id: Uuid,
    name: String,
    bounds: ZoneBounds,
    status: JsZoneStatus,
    scan_count: u32,
    detection_count: u32,
}

impl InternalZone {
    fn to_js(&self) -> JsScanZone {
        let zone_type = match &self.bounds {
            ZoneBounds::Rectangle { .. } => "rectangle",
            ZoneBounds::Circle { .. } => "circle",
            ZoneBounds::Polygon { .. } => "polygon",
        };

        JsScanZone {
            id: self.id.to_string(),
            name: self.name.clone(),
            zone_type: zone_type.to_string(),
            status: self.status as u8,
            scan_count: self.scan_count,
            detection_count: self.detection_count,
            bounds_json: serde_json::to_string(&self.bounds).unwrap_or_default(),
        }
    }

    fn contains_point(&self, x: f64, y: f64) -> bool {
        match &self.bounds {
            ZoneBounds::Rectangle {
                x: rx,
                y: ry,
                width,
                height,
            } => x >= *rx && x <= rx + width && y >= *ry && y <= ry + height,
            ZoneBounds::Circle {
                center_x,
                center_y,
                radius,
            } => {
                let dx = x - center_x;
                let dy = y - center_y;
                (dx * dx + dy * dy).sqrt() <= *radius
            }
            ZoneBounds::Polygon { vertices } => {
                if vertices.len() < 3 {
                    return false;
                }
                // Ray casting algorithm
                let mut inside = false;
                let n = vertices.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = vertices[i];
                    let (xj, yj) = vertices[j];
                    if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }
}

/// Internal alert state
#[derive(Debug, Clone)]
struct InternalAlert {
    id: Uuid,
    survivor_id: Uuid,
    priority: JsAlertPriority,
    title: String,
    message: String,
    recommended_action: String,
    triage_status: JsTriageStatus,
    location_x: f64,
    location_y: f64,
    created_at: chrono::DateTime<chrono::Utc>,
    acknowledged: bool,
}

impl InternalAlert {
    fn to_js(&self) -> JsAlert {
        let priority_color = match self.priority {
            JsAlertPriority::Critical => "#ff0000",
            JsAlertPriority::High => "#ff6600",
            JsAlertPriority::Medium => "#ffcc00",
            JsAlertPriority::Low => "#0066ff",
        };

        JsAlert {
            id: self.id.to_string(),
            survivor_id: self.survivor_id.to_string(),
            priority: self.priority as u8,
            title: self.title.clone(),
            message: self.message.clone(),
            recommended_action: self.recommended_action.clone(),
            triage_status: self.triage_status as u8,
            location_x: self.location_x,
            location_y: self.location_y,
            created_at: self.created_at.to_rfc3339(),
            priority_color: priority_color.to_string(),
        }
    }
}

/// Dashboard internal state
struct DashboardState {
    event_id: Option<Uuid>,
    disaster_type: JsDisasterType,
    event_start: Option<chrono::DateTime<chrono::Utc>>,
    location: (f64, f64),
    description: String,
    zones: HashMap<Uuid, InternalZone>,
    survivors: HashMap<Uuid, InternalSurvivor>,
    alerts: HashMap<Uuid, InternalAlert>,
    // Callbacks
    on_survivor_detected: Option<js_sys::Function>,
    on_survivor_updated: Option<js_sys::Function>,
    on_alert_generated: Option<js_sys::Function>,
    on_zone_updated: Option<js_sys::Function>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            event_id: None,
            disaster_type: JsDisasterType::Unknown,
            event_start: None,
            location: (0.0, 0.0),
            description: String::new(),
            zones: HashMap::new(),
            survivors: HashMap::new(),
            alerts: HashMap::new(),
            on_survivor_detected: None,
            on_survivor_updated: None,
            on_alert_generated: None,
            on_zone_updated: None,
        }
    }
}

// ============================================================================
// Main Dashboard Class
// ============================================================================

/// WiFi-Mat Disaster Response Dashboard for browser integration.
///
/// This class provides a complete interface for managing disaster response
/// operations from a web browser, including zone management, survivor tracking,
/// and real-time alert notifications.
#[wasm_bindgen]
pub struct MatDashboard {
    state: Rc<RefCell<DashboardState>>,
}

#[wasm_bindgen]
impl MatDashboard {
    /// Create a new MatDashboard instance.
    ///
    /// @returns {MatDashboard} A new dashboard instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> MatDashboard {
        // Initialize panic hook for better error messages
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        MatDashboard {
            state: Rc::new(RefCell::new(DashboardState::default())),
        }
    }

    // ========================================================================
    // Event Management
    // ========================================================================

    /// Create a new disaster event.
    ///
    /// @param {string} disaster_type - Type: "earthquake", "building_collapse", etc.
    /// @param {number} latitude - Event latitude
    /// @param {number} longitude - Event longitude
    /// @param {string} description - Event description
    /// @returns {string} The event ID
    #[wasm_bindgen(js_name = createEvent)]
    pub fn create_event(
        &self,
        disaster_type: &str,
        latitude: f64,
        longitude: f64,
        description: &str,
    ) -> String {
        let mut state = self.state.borrow_mut();

        let dtype = match disaster_type.to_lowercase().as_str() {
            "earthquake" => JsDisasterType::Earthquake,
            "building_collapse" | "buildingcollapse" => JsDisasterType::BuildingCollapse,
            "landslide" => JsDisasterType::Landslide,
            "avalanche" => JsDisasterType::Avalanche,
            "flood" => JsDisasterType::Flood,
            "mine_collapse" | "minecollapse" => JsDisasterType::MineCollapse,
            "industrial" => JsDisasterType::Industrial,
            "tunnel_collapse" | "tunnelcollapse" => JsDisasterType::TunnelCollapse,
            _ => JsDisasterType::Unknown,
        };

        let event_id = Uuid::new_v4();
        state.event_id = Some(event_id);
        state.disaster_type = dtype;
        state.event_start = Some(chrono::Utc::now());
        state.location = (latitude, longitude);
        state.description = description.to_string();

        // Clear previous data
        state.zones.clear();
        state.survivors.clear();
        state.alerts.clear();

        log::info!("Created disaster event: {} - {}", event_id, description);

        event_id.to_string()
    }

    /// Get the current event ID.
    ///
    /// @returns {string | undefined} The event ID or undefined
    #[wasm_bindgen(js_name = getEventId)]
    pub fn get_event_id(&self) -> Option<String> {
        self.state.borrow().event_id.map(|id| id.to_string())
    }

    /// Get the disaster type.
    ///
    /// @returns {number} The disaster type enum value
    #[wasm_bindgen(js_name = getDisasterType)]
    pub fn get_disaster_type(&self) -> JsDisasterType {
        self.state.borrow().disaster_type
    }

    /// Close the current event.
    #[wasm_bindgen(js_name = closeEvent)]
    pub fn close_event(&self) {
        let mut state = self.state.borrow_mut();
        state.event_id = None;
        state.event_start = None;
        log::info!("Disaster event closed");
    }

    // ========================================================================
    // Zone Management
    // ========================================================================

    /// Add a rectangular scan zone.
    ///
    /// @param {string} name - Zone name
    /// @param {number} x - Top-left X coordinate (canvas pixels)
    /// @param {number} y - Top-left Y coordinate (canvas pixels)
    /// @param {number} width - Zone width (pixels)
    /// @param {number} height - Zone height (pixels)
    /// @returns {string} The zone ID
    #[wasm_bindgen(js_name = addRectangleZone)]
    pub fn add_rectangle_zone(
        &self,
        name: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> String {
        let mut state = self.state.borrow_mut();

        let zone = InternalZone {
            id: Uuid::new_v4(),
            name: name.to_string(),
            bounds: ZoneBounds::Rectangle {
                x,
                y,
                width,
                height,
            },
            status: JsZoneStatus::Active,
            scan_count: 0,
            detection_count: 0,
        };

        let zone_id = zone.id;
        let js_zone = zone.to_js();
        state.zones.insert(zone_id, zone);

        // Fire callback
        if let Some(callback) = &state.on_zone_updated {
            let this = JsValue::NULL;
            let zone_value = serde_wasm_bindgen::to_value(&js_zone).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&this, &zone_value);
        }

        log::info!("Added rectangle zone: {} ({})", name, zone_id);
        zone_id.to_string()
    }

    /// Add a circular scan zone.
    ///
    /// @param {string} name - Zone name
    /// @param {number} centerX - Center X coordinate (canvas pixels)
    /// @param {number} centerY - Center Y coordinate (canvas pixels)
    /// @param {number} radius - Zone radius (pixels)
    /// @returns {string} The zone ID
    #[wasm_bindgen(js_name = addCircleZone)]
    pub fn add_circle_zone(&self, name: &str, center_x: f64, center_y: f64, radius: f64) -> String {
        let mut state = self.state.borrow_mut();

        let zone = InternalZone {
            id: Uuid::new_v4(),
            name: name.to_string(),
            bounds: ZoneBounds::Circle {
                center_x,
                center_y,
                radius,
            },
            status: JsZoneStatus::Active,
            scan_count: 0,
            detection_count: 0,
        };

        let zone_id = zone.id;
        let js_zone = zone.to_js();
        state.zones.insert(zone_id, zone);

        // Fire callback
        if let Some(callback) = &state.on_zone_updated {
            let this = JsValue::NULL;
            let zone_value = serde_wasm_bindgen::to_value(&js_zone).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&this, &zone_value);
        }

        log::info!("Added circle zone: {} ({})", name, zone_id);
        zone_id.to_string()
    }

    /// Add a polygon scan zone.
    ///
    /// @param {string} name - Zone name
    /// @param {Float64Array} vertices - Flat array of [x1, y1, x2, y2, ...] coordinates
    /// @returns {string} The zone ID
    #[wasm_bindgen(js_name = addPolygonZone)]
    pub fn add_polygon_zone(&self, name: &str, vertices: &[f64]) -> String {
        let mut state = self.state.borrow_mut();

        // Convert flat array to vertex pairs
        let vertex_pairs: Vec<(f64, f64)> = vertices
            .chunks(2)
            .filter(|chunk| chunk.len() == 2)
            .map(|chunk| (chunk[0], chunk[1]))
            .collect();

        let zone = InternalZone {
            id: Uuid::new_v4(),
            name: name.to_string(),
            bounds: ZoneBounds::Polygon {
                vertices: vertex_pairs,
            },
            status: JsZoneStatus::Active,
            scan_count: 0,
            detection_count: 0,
        };

        let zone_id = zone.id;
        let js_zone = zone.to_js();
        state.zones.insert(zone_id, zone);

        // Fire callback
        if let Some(callback) = &state.on_zone_updated {
            let this = JsValue::NULL;
            let zone_value = serde_wasm_bindgen::to_value(&js_zone).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&this, &zone_value);
        }

        log::info!("Added polygon zone: {} ({})", name, zone_id);
        zone_id.to_string()
    }

    /// Remove a scan zone.
    ///
    /// @param {string} zone_id - Zone ID to remove
    /// @returns {boolean} True if removed
    #[wasm_bindgen(js_name = removeZone)]
    pub fn remove_zone(&self, zone_id: &str) -> bool {
        let mut state = self.state.borrow_mut();

        if let Ok(uuid) = Uuid::parse_str(zone_id) {
            if state.zones.remove(&uuid).is_some() {
                log::info!("Removed zone: {}", zone_id);
                return true;
            }
        }
        false
    }

    /// Update zone status.
    ///
    /// @param {string} zone_id - Zone ID
    /// @param {number} status - New status (0=Active, 1=Paused, 2=Complete, 3=Inaccessible)
    /// @returns {boolean} True if updated
    #[wasm_bindgen(js_name = setZoneStatus)]
    pub fn set_zone_status(&self, zone_id: &str, status: u8) -> bool {
        let mut state = self.state.borrow_mut();

        if let Ok(uuid) = Uuid::parse_str(zone_id) {
            if let Some(zone) = state.zones.get_mut(&uuid) {
                zone.status = match status {
                    0 => JsZoneStatus::Active,
                    1 => JsZoneStatus::Paused,
                    2 => JsZoneStatus::Complete,
                    3 => JsZoneStatus::Inaccessible,
                    _ => return false,
                };

                // Get JS zone before callback
                let js_zone = zone.to_js();

                // Fire callback
                if let Some(callback) = &state.on_zone_updated {
                    let this = JsValue::NULL;
                    let zone_value = serde_wasm_bindgen::to_value(&js_zone).unwrap_or(JsValue::NULL);
                    let _ = callback.call1(&this, &zone_value);
                }

                return true;
            }
        }
        false
    }

    /// Get all zones.
    ///
    /// @returns {Array<JsScanZone>} Array of zones
    #[wasm_bindgen(js_name = getZones)]
    pub fn get_zones(&self) -> JsValue {
        let state = self.state.borrow();
        let zones: Vec<JsScanZone> = state.zones.values().map(|z| z.to_js()).collect();
        serde_wasm_bindgen::to_value(&zones).unwrap_or(JsValue::NULL)
    }

    /// Get a specific zone.
    ///
    /// @param {string} zone_id - Zone ID
    /// @returns {JsScanZone | undefined} The zone or undefined
    #[wasm_bindgen(js_name = getZone)]
    pub fn get_zone(&self, zone_id: &str) -> JsValue {
        let state = self.state.borrow();

        if let Ok(uuid) = Uuid::parse_str(zone_id) {
            if let Some(zone) = state.zones.get(&uuid) {
                return serde_wasm_bindgen::to_value(&zone.to_js()).unwrap_or(JsValue::NULL);
            }
        }
        JsValue::UNDEFINED
    }

    // ========================================================================
    // Survivor Management
    // ========================================================================

    /// Simulate a survivor detection (for testing/demo).
    ///
    /// @param {number} x - X position (canvas pixels)
    /// @param {number} y - Y position (canvas pixels)
    /// @param {number} depth - Depth in meters (negative = buried)
    /// @param {number} triage - Triage status (0-4)
    /// @param {number} confidence - Detection confidence (0.0-1.0)
    /// @returns {string} The survivor ID
    #[wasm_bindgen(js_name = simulateSurvivorDetection)]
    pub fn simulate_survivor_detection(
        &self,
        x: f64,
        y: f64,
        depth: f64,
        triage: u8,
        confidence: f64,
    ) -> String {
        let mut state = self.state.borrow_mut();

        // Find which zone contains this point
        let zone_id = state
            .zones
            .iter()
            .find(|(_, z)| z.contains_point(x, y))
            .map(|(id, _)| *id)
            .unwrap_or_else(Uuid::new_v4);

        // Update zone detection count
        if let Some(zone) = state.zones.get_mut(&zone_id) {
            zone.detection_count += 1;
        }

        let triage_status = match triage {
            0 => JsTriageStatus::Immediate,
            1 => JsTriageStatus::Delayed,
            2 => JsTriageStatus::Minor,
            3 => JsTriageStatus::Deceased,
            _ => JsTriageStatus::Unknown,
        };

        let now = chrono::Utc::now();
        let survivor = InternalSurvivor {
            id: Uuid::new_v4(),
            zone_id,
            x,
            y,
            depth,
            triage_status,
            confidence: confidence.clamp(0.0, 1.0),
            breathing_rate: Some(12.0 + (confidence * 8.0)),
            heart_rate: Some(60.0 + (confidence * 40.0)),
            first_detected: now,
            last_updated: now,
            is_deteriorating: false,
            alert_sent: false,
        };

        let survivor_id = survivor.id;
        let js_survivor = survivor.to_js();
        state.survivors.insert(survivor_id, survivor);

        // Fire callback
        if let Some(callback) = &state.on_survivor_detected {
            let this = JsValue::NULL;
            let survivor_value =
                serde_wasm_bindgen::to_value(&js_survivor).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&this, &survivor_value);
        }

        // Generate alert for urgent survivors
        if triage_status == JsTriageStatus::Immediate || triage_status == JsTriageStatus::Delayed {
            self.generate_alert_internal(&mut state, survivor_id, triage_status, x, y);
        }

        log::info!(
            "Survivor detected: {} at ({}, {}) - {:?}",
            survivor_id,
            x,
            y,
            triage_status
        );

        survivor_id.to_string()
    }

    /// Get all survivors.
    ///
    /// @returns {Array<JsSurvivor>} Array of survivors
    #[wasm_bindgen(js_name = getSurvivors)]
    pub fn get_survivors(&self) -> JsValue {
        let state = self.state.borrow();
        let survivors: Vec<JsSurvivor> = state.survivors.values().map(|s| s.to_js()).collect();
        serde_wasm_bindgen::to_value(&survivors).unwrap_or(JsValue::NULL)
    }

    /// Get survivors filtered by triage status.
    ///
    /// @param {number} triage - Triage status to filter (0-4)
    /// @returns {Array<JsSurvivor>} Filtered survivors
    #[wasm_bindgen(js_name = getSurvivorsByTriage)]
    pub fn get_survivors_by_triage(&self, triage: u8) -> JsValue {
        let state = self.state.borrow();
        let target_status = match triage {
            0 => JsTriageStatus::Immediate,
            1 => JsTriageStatus::Delayed,
            2 => JsTriageStatus::Minor,
            3 => JsTriageStatus::Deceased,
            _ => JsTriageStatus::Unknown,
        };

        let survivors: Vec<JsSurvivor> = state
            .survivors
            .values()
            .filter(|s| s.triage_status == target_status)
            .map(|s| s.to_js())
            .collect();

        serde_wasm_bindgen::to_value(&survivors).unwrap_or(JsValue::NULL)
    }

    /// Get a specific survivor.
    ///
    /// @param {string} survivor_id - Survivor ID
    /// @returns {JsSurvivor | undefined} The survivor or undefined
    #[wasm_bindgen(js_name = getSurvivor)]
    pub fn get_survivor(&self, survivor_id: &str) -> JsValue {
        let state = self.state.borrow();

        if let Ok(uuid) = Uuid::parse_str(survivor_id) {
            if let Some(survivor) = state.survivors.get(&uuid) {
                return serde_wasm_bindgen::to_value(&survivor.to_js()).unwrap_or(JsValue::NULL);
            }
        }
        JsValue::UNDEFINED
    }

    /// Mark a survivor as rescued.
    ///
    /// @param {string} survivor_id - Survivor ID
    /// @returns {boolean} True if updated
    #[wasm_bindgen(js_name = markSurvivorRescued)]
    pub fn mark_survivor_rescued(&self, survivor_id: &str) -> bool {
        let mut state = self.state.borrow_mut();

        if let Ok(uuid) = Uuid::parse_str(survivor_id) {
            if let Some(_survivor) = state.survivors.remove(&uuid) {
                log::info!("Survivor {} marked as rescued", survivor_id);
                return true;
            }
        }
        false
    }

    /// Update survivor deterioration status.
    ///
    /// @param {string} survivor_id - Survivor ID
    /// @param {boolean} is_deteriorating - Whether survivor is deteriorating
    /// @returns {boolean} True if updated
    #[wasm_bindgen(js_name = setSurvivorDeteriorating)]
    pub fn set_survivor_deteriorating(&self, survivor_id: &str, is_deteriorating: bool) -> bool {
        let mut state = self.state.borrow_mut();

        if let Ok(uuid) = Uuid::parse_str(survivor_id) {
            if let Some(survivor) = state.survivors.get_mut(&uuid) {
                survivor.is_deteriorating = is_deteriorating;
                survivor.last_updated = chrono::Utc::now();

                // Get JS survivor before callback
                let js_survivor = survivor.to_js();

                // Fire callback
                if let Some(callback) = &state.on_survivor_updated {
                    let this = JsValue::NULL;
                    let survivor_value =
                        serde_wasm_bindgen::to_value(&js_survivor).unwrap_or(JsValue::NULL);
                    let _ = callback.call1(&this, &survivor_value);
                }

                return true;
            }
        }
        false
    }

    // ========================================================================
    // Alert Management
    // ========================================================================

    fn generate_alert_internal(
        &self,
        state: &mut DashboardState,
        survivor_id: Uuid,
        triage_status: JsTriageStatus,
        x: f64,
        y: f64,
    ) {
        let priority = match triage_status {
            JsTriageStatus::Immediate => JsAlertPriority::Critical,
            JsTriageStatus::Delayed => JsAlertPriority::High,
            JsTriageStatus::Minor => JsAlertPriority::Medium,
            _ => JsAlertPriority::Low,
        };

        let title = match triage_status {
            JsTriageStatus::Immediate => "CRITICAL: Survivor needs immediate attention",
            JsTriageStatus::Delayed => "URGENT: Survivor detected - delayed priority",
            _ => "Survivor detected",
        };

        let alert = InternalAlert {
            id: Uuid::new_v4(),
            survivor_id,
            priority,
            title: title.to_string(),
            message: format!(
                "Survivor detected at position ({:.0}, {:.0}). Triage: {:?}",
                x, y, triage_status
            ),
            recommended_action: match triage_status {
                JsTriageStatus::Immediate => "Dispatch rescue team immediately".to_string(),
                JsTriageStatus::Delayed => "Schedule rescue team dispatch".to_string(),
                _ => "Monitor and assess".to_string(),
            },
            triage_status,
            location_x: x,
            location_y: y,
            created_at: chrono::Utc::now(),
            acknowledged: false,
        };

        let alert_id = alert.id;
        let js_alert = alert.to_js();
        state.alerts.insert(alert_id, alert);

        // Mark survivor alert sent
        if let Some(survivor) = state.survivors.get_mut(&survivor_id) {
            survivor.alert_sent = true;
        }

        // Fire callback
        if let Some(callback) = &state.on_alert_generated {
            let this = JsValue::NULL;
            let alert_value = serde_wasm_bindgen::to_value(&js_alert).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&this, &alert_value);
        }
    }

    /// Get all active alerts.
    ///
    /// @returns {Array<JsAlert>} Array of alerts
    #[wasm_bindgen(js_name = getAlerts)]
    pub fn get_alerts(&self) -> JsValue {
        let state = self.state.borrow();
        let alerts: Vec<JsAlert> = state
            .alerts
            .values()
            .filter(|a| !a.acknowledged)
            .map(|a| a.to_js())
            .collect();
        serde_wasm_bindgen::to_value(&alerts).unwrap_or(JsValue::NULL)
    }

    /// Acknowledge an alert.
    ///
    /// @param {string} alert_id - Alert ID
    /// @returns {boolean} True if acknowledged
    #[wasm_bindgen(js_name = acknowledgeAlert)]
    pub fn acknowledge_alert(&self, alert_id: &str) -> bool {
        let mut state = self.state.borrow_mut();

        if let Ok(uuid) = Uuid::parse_str(alert_id) {
            if let Some(alert) = state.alerts.get_mut(&uuid) {
                alert.acknowledged = true;
                log::info!("Alert {} acknowledged", alert_id);
                return true;
            }
        }
        false
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get dashboard statistics.
    ///
    /// @returns {JsDashboardStats} Current statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> JsDashboardStats {
        let state = self.state.borrow();

        let mut immediate_count = 0u32;
        let mut delayed_count = 0u32;
        let mut minor_count = 0u32;
        let mut deceased_count = 0u32;
        let mut unknown_count = 0u32;

        for survivor in state.survivors.values() {
            match survivor.triage_status {
                JsTriageStatus::Immediate => immediate_count += 1,
                JsTriageStatus::Delayed => delayed_count += 1,
                JsTriageStatus::Minor => minor_count += 1,
                JsTriageStatus::Deceased => deceased_count += 1,
                JsTriageStatus::Unknown => unknown_count += 1,
            }
        }

        let active_zones = state
            .zones
            .values()
            .filter(|z| z.status == JsZoneStatus::Active)
            .count() as u32;

        let total_scans: u32 = state.zones.values().map(|z| z.scan_count).sum();

        let active_alerts = state.alerts.values().filter(|a| !a.acknowledged).count() as u32;

        let elapsed_seconds = state
            .event_start
            .map(|start| (chrono::Utc::now() - start).num_milliseconds() as f64 / 1000.0)
            .unwrap_or(0.0);

        JsDashboardStats {
            total_survivors: state.survivors.len() as u32,
            immediate_count,
            delayed_count,
            minor_count,
            deceased_count,
            unknown_count,
            active_zones,
            total_scans,
            active_alerts,
            elapsed_seconds,
        }
    }

    // ========================================================================
    // Callback Registration
    // ========================================================================

    /// Register callback for survivor detection events.
    ///
    /// @param {Function} callback - Function to call with JsSurvivor when detected
    #[wasm_bindgen(js_name = onSurvivorDetected)]
    pub fn on_survivor_detected(&self, callback: js_sys::Function) {
        self.state.borrow_mut().on_survivor_detected = Some(callback);
    }

    /// Register callback for survivor update events.
    ///
    /// @param {Function} callback - Function to call with JsSurvivor when updated
    #[wasm_bindgen(js_name = onSurvivorUpdated)]
    pub fn on_survivor_updated(&self, callback: js_sys::Function) {
        self.state.borrow_mut().on_survivor_updated = Some(callback);
    }

    /// Register callback for alert generation events.
    ///
    /// @param {Function} callback - Function to call with JsAlert when generated
    #[wasm_bindgen(js_name = onAlertGenerated)]
    pub fn on_alert_generated(&self, callback: js_sys::Function) {
        self.state.borrow_mut().on_alert_generated = Some(callback);
    }

    /// Register callback for zone update events.
    ///
    /// @param {Function} callback - Function to call with JsScanZone when updated
    #[wasm_bindgen(js_name = onZoneUpdated)]
    pub fn on_zone_updated(&self, callback: js_sys::Function) {
        self.state.borrow_mut().on_zone_updated = Some(callback);
    }

    // ========================================================================
    // Canvas Rendering Helpers
    // ========================================================================

    /// Render all zones on a canvas context.
    ///
    /// @param {CanvasRenderingContext2D} ctx - Canvas 2D context
    #[wasm_bindgen(js_name = renderZones)]
    pub fn render_zones(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        let state = self.state.borrow();

        for zone in state.zones.values() {
            let color = match zone.status {
                JsZoneStatus::Active => "rgba(0, 150, 255, 0.3)",
                JsZoneStatus::Paused => "rgba(255, 200, 0, 0.3)",
                JsZoneStatus::Complete => "rgba(0, 200, 0, 0.3)",
                JsZoneStatus::Inaccessible => "rgba(150, 150, 150, 0.3)",
            };

            let border_color = match zone.status {
                JsZoneStatus::Active => "#0096ff",
                JsZoneStatus::Paused => "#ffc800",
                JsZoneStatus::Complete => "#00c800",
                JsZoneStatus::Inaccessible => "#969696",
            };

            ctx.set_fill_style_str(color);
            ctx.set_stroke_style_str(border_color);
            ctx.set_line_width(2.0);

            match &zone.bounds {
                ZoneBounds::Rectangle {
                    x,
                    y,
                    width,
                    height,
                } => {
                    ctx.fill_rect(*x, *y, *width, *height);
                    ctx.stroke_rect(*x, *y, *width, *height);

                    // Draw zone name
                    ctx.set_fill_style_str("#ffffff");
                    ctx.set_font("12px sans-serif");
                    let _ = ctx.fill_text(&zone.name, *x + 5.0, *y + 15.0);
                }
                ZoneBounds::Circle {
                    center_x,
                    center_y,
                    radius,
                } => {
                    ctx.begin_path();
                    let _ = ctx.arc(*center_x, *center_y, *radius, 0.0, std::f64::consts::TAU);
                    ctx.fill();
                    ctx.stroke();

                    // Draw zone name
                    ctx.set_fill_style_str("#ffffff");
                    ctx.set_font("12px sans-serif");
                    let _ = ctx.fill_text(&zone.name, *center_x - 20.0, *center_y);
                }
                ZoneBounds::Polygon { vertices } => {
                    if !vertices.is_empty() {
                        ctx.begin_path();
                        ctx.move_to(vertices[0].0, vertices[0].1);
                        for (x, y) in vertices.iter().skip(1) {
                            ctx.line_to(*x, *y);
                        }
                        ctx.close_path();
                        ctx.fill();
                        ctx.stroke();

                        // Draw zone name at centroid
                        if !vertices.is_empty() {
                            let cx: f64 =
                                vertices.iter().map(|(x, _)| x).sum::<f64>() / vertices.len() as f64;
                            let cy: f64 =
                                vertices.iter().map(|(_, y)| y).sum::<f64>() / vertices.len() as f64;
                            ctx.set_fill_style_str("#ffffff");
                            ctx.set_font("12px sans-serif");
                            let _ = ctx.fill_text(&zone.name, cx - 20.0, cy);
                        }
                    }
                }
            }
        }
    }

    /// Render all survivors on a canvas context.
    ///
    /// @param {CanvasRenderingContext2D} ctx - Canvas 2D context
    #[wasm_bindgen(js_name = renderSurvivors)]
    pub fn render_survivors(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        let state = self.state.borrow();

        for survivor in state.survivors.values() {
            let color = survivor.triage_status.color();
            let radius = if survivor.is_deteriorating { 12.0 } else { 10.0 };

            // Draw outer glow for urgent survivors
            if survivor.triage_status == JsTriageStatus::Immediate {
                ctx.set_fill_style_str("rgba(255, 0, 0, 0.3)");
                ctx.begin_path();
                let _ = ctx.arc(survivor.x, survivor.y, radius + 8.0, 0.0, std::f64::consts::TAU);
                ctx.fill();
            }

            // Draw marker
            ctx.set_fill_style_str(color);
            ctx.begin_path();
            let _ = ctx.arc(survivor.x, survivor.y, radius, 0.0, std::f64::consts::TAU);
            ctx.fill();

            // Draw border
            ctx.set_stroke_style_str("#ffffff");
            ctx.set_line_width(2.0);
            ctx.stroke();

            // Draw deterioration indicator
            if survivor.is_deteriorating {
                ctx.set_stroke_style_str("#ff0000");
                ctx.set_line_width(3.0);
                ctx.begin_path();
                let _ = ctx.arc(
                    survivor.x,
                    survivor.y,
                    radius + 4.0,
                    0.0,
                    std::f64::consts::TAU,
                );
                ctx.stroke();
            }

            // Draw depth indicator if buried
            if survivor.depth < 0.0 {
                ctx.set_fill_style_str("#ffffff");
                ctx.set_font("10px sans-serif");
                let depth_text = format!("{:.1}m", -survivor.depth);
                let _ = ctx.fill_text(&depth_text, survivor.x + radius + 2.0, survivor.y + 4.0);
            }
        }
    }

    // ========================================================================
    // WebSocket Integration
    // ========================================================================

    /// Connect to a WebSocket for real-time updates.
    ///
    /// @param {string} url - WebSocket URL
    /// @returns {Promise<void>} Promise that resolves when connected
    #[wasm_bindgen(js_name = connectWebSocket)]
    pub fn connect_websocket(&self, url: &str) -> js_sys::Promise {
        let state = Rc::clone(&self.state);
        let url = url.to_string();

        wasm_bindgen_futures::future_to_promise(async move {
            let ws = web_sys::WebSocket::new(&url)
                .map_err(|e| JsValue::from_str(&format!("Failed to create WebSocket: {:?}", e)))?;

            ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

            // Set up message handler
            let _state_clone = Rc::clone(&state);
            let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                    let msg: String = txt.into();
                    // Parse and handle incoming survivor data
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&msg) {
                        if let Some(msg_type) = data.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "survivor_detection" => {
                                    log::info!("Received survivor detection via WebSocket");
                                    // Process survivor data...
                                }
                                "zone_update" => {
                                    log::info!("Received zone update via WebSocket");
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);

            ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
            onmessage_callback.forget();

            // Set up error handler
            let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
                log::error!("WebSocket error: {:?}", e.message());
            }) as Box<dyn FnMut(_)>);

            ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
            onerror_callback.forget();

            log::info!("WebSocket connected to {}", url);

            Ok(JsValue::UNDEFINED)
        })
    }
}

impl Default for MatDashboard {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TypeScript Type Definitions
// ============================================================================

/// Generate TypeScript definitions.
/// This is exported as a constant string for tooling.
#[wasm_bindgen]
pub fn get_typescript_definitions() -> String {
    r#"
// WiFi-Mat TypeScript Definitions

export enum DisasterType {
    BuildingCollapse = 0,
    Earthquake = 1,
    Landslide = 2,
    Avalanche = 3,
    Flood = 4,
    MineCollapse = 5,
    Industrial = 6,
    TunnelCollapse = 7,
    Unknown = 8,
}

export enum TriageStatus {
    Immediate = 0,  // Red
    Delayed = 1,    // Yellow
    Minor = 2,      // Green
    Deceased = 3,   // Black
    Unknown = 4,    // Gray
}

export enum ZoneStatus {
    Active = 0,
    Paused = 1,
    Complete = 2,
    Inaccessible = 3,
}

export enum AlertPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

export interface Survivor {
    id: string;
    zone_id: string;
    x: number;
    y: number;
    depth: number;
    triage_status: TriageStatus;
    triage_color: string;
    confidence: number;
    breathing_rate: number;
    heart_rate: number;
    first_detected: string;
    last_updated: string;
    is_deteriorating: boolean;
}

export interface ScanZone {
    id: string;
    name: string;
    zone_type: 'rectangle' | 'circle' | 'polygon';
    status: ZoneStatus;
    scan_count: number;
    detection_count: number;
    bounds_json: string;
}

export interface Alert {
    id: string;
    survivor_id: string;
    priority: AlertPriority;
    title: string;
    message: string;
    recommended_action: string;
    triage_status: TriageStatus;
    location_x: number;
    location_y: number;
    created_at: string;
    priority_color: string;
}

export interface DashboardStats {
    total_survivors: number;
    immediate_count: number;
    delayed_count: number;
    minor_count: number;
    deceased_count: number;
    unknown_count: number;
    active_zones: number;
    total_scans: number;
    active_alerts: number;
    elapsed_seconds: number;
}

export class MatDashboard {
    constructor();

    // Event Management
    createEvent(disasterType: string, latitude: number, longitude: number, description: string): string;
    getEventId(): string | undefined;
    getDisasterType(): DisasterType;
    closeEvent(): void;

    // Zone Management
    addRectangleZone(name: string, x: number, y: number, width: number, height: number): string;
    addCircleZone(name: string, centerX: number, centerY: number, radius: number): string;
    addPolygonZone(name: string, vertices: Float64Array): string;
    removeZone(zoneId: string): boolean;
    setZoneStatus(zoneId: string, status: ZoneStatus): boolean;
    getZones(): ScanZone[];
    getZone(zoneId: string): ScanZone | undefined;

    // Survivor Management
    simulateSurvivorDetection(x: number, y: number, depth: number, triage: TriageStatus, confidence: number): string;
    getSurvivors(): Survivor[];
    getSurvivorsByTriage(triage: TriageStatus): Survivor[];
    getSurvivor(survivorId: string): Survivor | undefined;
    markSurvivorRescued(survivorId: string): boolean;
    setSurvivorDeteriorating(survivorId: string, isDeteriorating: boolean): boolean;

    // Alert Management
    getAlerts(): Alert[];
    acknowledgeAlert(alertId: string): boolean;

    // Statistics
    getStats(): DashboardStats;

    // Callbacks
    onSurvivorDetected(callback: (survivor: Survivor) => void): void;
    onSurvivorUpdated(callback: (survivor: Survivor) => void): void;
    onAlertGenerated(callback: (alert: Alert) => void): void;
    onZoneUpdated(callback: (zone: ScanZone) => void): void;

    // Rendering
    renderZones(ctx: CanvasRenderingContext2D): void;
    renderSurvivors(ctx: CanvasRenderingContext2D): void;

    // WebSocket
    connectWebSocket(url: string): Promise<void>;
}
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_create_dashboard() {
        let dashboard = MatDashboard::new();
        assert!(dashboard.get_event_id().is_none());
    }

    #[wasm_bindgen_test]
    fn test_create_event() {
        let dashboard = MatDashboard::new();
        let event_id = dashboard.create_event("earthquake", 37.7749, -122.4194, "Test Event");
        assert!(!event_id.is_empty());
        assert!(dashboard.get_event_id().is_some());
    }

    #[wasm_bindgen_test]
    fn test_add_zone() {
        let dashboard = MatDashboard::new();
        dashboard.create_event("earthquake", 0.0, 0.0, "Test");

        let zone_id = dashboard.add_rectangle_zone("Zone A", 0.0, 0.0, 100.0, 80.0);
        assert!(!zone_id.is_empty());
    }

    #[wasm_bindgen_test]
    fn test_simulate_survivor() {
        let dashboard = MatDashboard::new();
        dashboard.create_event("earthquake", 0.0, 0.0, "Test");
        dashboard.add_rectangle_zone("Zone A", 0.0, 0.0, 100.0, 80.0);

        let survivor_id = dashboard.simulate_survivor_detection(50.0, 40.0, -2.0, 0, 0.85);
        assert!(!survivor_id.is_empty());
    }
}
