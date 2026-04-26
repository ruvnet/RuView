//! Alert types for emergency notifications.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{SurvivorId, TriageStatus, Coordinates3D};

/// Unique identifier for an alert
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlertId(Uuid);

impl AlertId {
    /// Create a new random alert ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for AlertId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AlertId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Alert priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Priority {
    /// Critical - immediate action required
    Critical = 1,
    /// High - urgent attention needed
    High = 2,
    /// Medium - important but not urgent
    Medium = 3,
    /// Low - informational
    Low = 4,
}

impl Priority {
    /// Create from triage status
    pub fn from_triage(status: &TriageStatus) -> Self {
        match status {
            TriageStatus::Immediate => Priority::Critical,
            TriageStatus::Delayed => Priority::High,
            TriageStatus::Minor => Priority::Medium,
            TriageStatus::Deceased => Priority::Low,
            TriageStatus::Unknown => Priority::Medium,
        }
    }

    /// Get numeric value (lower = higher priority)
    pub fn value(&self) -> u8 {
        *self as u8
    }

    /// Get display color
    pub fn color(&self) -> &'static str {
        match self {
            Priority::Critical => "red",
            Priority::High => "orange",
            Priority::Medium => "yellow",
            Priority::Low => "blue",
        }
    }

    /// Get sound pattern for audio alerts
    pub fn audio_pattern(&self) -> &'static str {
        match self {
            Priority::Critical => "rapid_beep",
            Priority::High => "double_beep",
            Priority::Medium => "single_beep",
            Priority::Low => "soft_tone",
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Critical => write!(f, "CRITICAL"),
            Priority::High => write!(f, "HIGH"),
            Priority::Medium => write!(f, "MEDIUM"),
            Priority::Low => write!(f, "LOW"),
        }
    }
}

/// Payload containing alert details
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlertPayload {
    /// Human-readable title
    pub title: String,
    /// Detailed message
    pub message: String,
    /// Triage status of survivor
    pub triage_status: TriageStatus,
    /// Location if known
    pub location: Option<Coordinates3D>,
    /// Recommended action
    pub recommended_action: String,
    /// Time-critical deadline (if any)
    pub deadline: Option<DateTime<Utc>>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl AlertPayload {
    /// Create a new alert payload
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        triage_status: TriageStatus,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            triage_status,
            location: None,
            recommended_action: String::new(),
            deadline: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set location
    pub fn with_location(mut self, location: Coordinates3D) -> Self {
        self.location = Some(location);
        self
    }

    /// Set recommended action
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.recommended_action = action.into();
        self
    }

    /// Set deadline
    pub fn with_deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Status of an alert
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlertStatus {
    /// Alert is pending acknowledgement
    Pending,
    /// Alert has been acknowledged
    Acknowledged,
    /// Alert is being worked on
    InProgress,
    /// Alert has been resolved
    Resolved,
    /// Alert was cancelled/superseded
    Cancelled,
    /// Alert expired without action
    Expired,
}

/// Resolution details for a closed alert
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlertResolution {
    /// Resolution type
    pub resolution_type: ResolutionType,
    /// Resolution notes
    pub notes: String,
    /// Team that resolved
    pub resolved_by: Option<String>,
    /// Resolution time
    pub resolved_at: DateTime<Utc>,
}

/// Types of alert resolution
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResolutionType {
    /// Survivor was rescued
    Rescued,
    /// Alert was a false positive
    FalsePositive,
    /// Survivor deceased before rescue
    Deceased,
    /// Alert superseded by new information
    Superseded,
    /// Alert timed out
    TimedOut,
    /// Other resolution
    Other,
}

/// An alert for rescue teams
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Alert {
    id: AlertId,
    survivor_id: SurvivorId,
    priority: Priority,
    payload: AlertPayload,
    status: AlertStatus,
    created_at: DateTime<Utc>,
    acknowledged_at: Option<DateTime<Utc>>,
    acknowledged_by: Option<String>,
    resolution: Option<AlertResolution>,
    escalation_count: u32,
}

impl Alert {
    /// Create a new alert
    pub fn new(survivor_id: SurvivorId, priority: Priority, payload: AlertPayload) -> Self {
        Self {
            id: AlertId::new(),
            survivor_id,
            priority,
            payload,
            status: AlertStatus::Pending,
            created_at: Utc::now(),
            acknowledged_at: None,
            acknowledged_by: None,
            resolution: None,
            escalation_count: 0,
        }
    }

    /// Get the alert ID
    pub fn id(&self) -> &AlertId {
        &self.id
    }

    /// Get the survivor ID
    pub fn survivor_id(&self) -> &SurvivorId {
        &self.survivor_id
    }

    /// Get the priority
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// Get the payload
    pub fn payload(&self) -> &AlertPayload {
        &self.payload
    }

    /// Get the status
    pub fn status(&self) -> &AlertStatus {
        &self.status
    }

    /// Get creation time
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    /// Get acknowledgement time
    pub fn acknowledged_at(&self) -> Option<&DateTime<Utc>> {
        self.acknowledged_at.as_ref()
    }

    /// Get who acknowledged
    pub fn acknowledged_by(&self) -> Option<&str> {
        self.acknowledged_by.as_deref()
    }

    /// Get resolution
    pub fn resolution(&self) -> Option<&AlertResolution> {
        self.resolution.as_ref()
    }

    /// Get escalation count
    pub fn escalation_count(&self) -> u32 {
        self.escalation_count
    }

    /// Acknowledge the alert
    pub fn acknowledge(&mut self, by: impl Into<String>) {
        self.status = AlertStatus::Acknowledged;
        self.acknowledged_at = Some(Utc::now());
        self.acknowledged_by = Some(by.into());
    }

    /// Mark as in progress
    pub fn start_work(&mut self) {
        if self.status == AlertStatus::Acknowledged {
            self.status = AlertStatus::InProgress;
        }
    }

    /// Resolve the alert
    pub fn resolve(&mut self, resolution: AlertResolution) {
        self.status = AlertStatus::Resolved;
        self.resolution = Some(resolution);
    }

    /// Cancel the alert
    pub fn cancel(&mut self, reason: &str) {
        self.status = AlertStatus::Cancelled;
        self.resolution = Some(AlertResolution {
            resolution_type: ResolutionType::Other,
            notes: reason.to_string(),
            resolved_by: None,
            resolved_at: Utc::now(),
        });
    }

    /// Escalate the alert (increase priority)
    pub fn escalate(&mut self) {
        self.escalation_count += 1;
        if self.priority != Priority::Critical {
            self.priority = match self.priority {
                Priority::Low => Priority::Medium,
                Priority::Medium => Priority::High,
                Priority::High => Priority::Critical,
                Priority::Critical => Priority::Critical,
            };
        }
    }

    /// Check if alert is pending
    pub fn is_pending(&self) -> bool {
        self.status == AlertStatus::Pending
    }

    /// Check if alert is active (not resolved/cancelled)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            AlertStatus::Pending | AlertStatus::Acknowledged | AlertStatus::InProgress
        )
    }

    /// Time since alert was created
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Time since acknowledgement
    pub fn time_since_ack(&self) -> Option<chrono::Duration> {
        self.acknowledged_at.map(|t| Utc::now() - t)
    }

    /// Check if alert needs escalation based on time
    pub fn needs_escalation(&self, max_pending_seconds: i64) -> bool {
        if !self.is_pending() {
            return false;
        }
        self.age().num_seconds() > max_pending_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_payload() -> AlertPayload {
        AlertPayload::new(
            "Survivor Detected",
            "Vital signs detected in Zone A",
            TriageStatus::Immediate,
        )
    }

    #[test]
    fn test_alert_creation() {
        let survivor_id = SurvivorId::new();
        let alert = Alert::new(
            survivor_id.clone(),
            Priority::Critical,
            create_test_payload(),
        );

        assert_eq!(alert.survivor_id(), &survivor_id);
        assert_eq!(alert.priority(), Priority::Critical);
        assert!(alert.is_pending());
        assert!(alert.is_active());
    }

    #[test]
    fn test_alert_lifecycle() {
        let mut alert = Alert::new(
            SurvivorId::new(),
            Priority::High,
            create_test_payload(),
        );

        // Initial state
        assert!(alert.is_pending());

        // Acknowledge
        alert.acknowledge("Team Alpha");
        assert_eq!(alert.status(), &AlertStatus::Acknowledged);
        assert_eq!(alert.acknowledged_by(), Some("Team Alpha"));

        // Start work
        alert.start_work();
        assert_eq!(alert.status(), &AlertStatus::InProgress);

        // Resolve
        alert.resolve(AlertResolution {
            resolution_type: ResolutionType::Rescued,
            notes: "Survivor extracted successfully".to_string(),
            resolved_by: Some("Team Alpha".to_string()),
            resolved_at: Utc::now(),
        });
        assert_eq!(alert.status(), &AlertStatus::Resolved);
        assert!(!alert.is_active());
    }

    #[test]
    fn test_alert_escalation() {
        let mut alert = Alert::new(
            SurvivorId::new(),
            Priority::Low,
            create_test_payload(),
        );

        alert.escalate();
        assert_eq!(alert.priority(), Priority::Medium);
        assert_eq!(alert.escalation_count(), 1);

        alert.escalate();
        assert_eq!(alert.priority(), Priority::High);

        alert.escalate();
        assert_eq!(alert.priority(), Priority::Critical);

        // Can't escalate beyond critical
        alert.escalate();
        assert_eq!(alert.priority(), Priority::Critical);
    }

    #[test]
    fn test_priority_from_triage() {
        assert_eq!(Priority::from_triage(&TriageStatus::Immediate), Priority::Critical);
        assert_eq!(Priority::from_triage(&TriageStatus::Delayed), Priority::High);
        assert_eq!(Priority::from_triage(&TriageStatus::Minor), Priority::Medium);
    }
}
