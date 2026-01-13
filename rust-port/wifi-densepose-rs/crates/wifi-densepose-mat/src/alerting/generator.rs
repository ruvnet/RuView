//! Alert generation from survivor detections.

use crate::domain::{
    Alert, AlertPayload, Priority, Survivor, TriageStatus, ScanZoneId,
};
use crate::MatError;

/// Generator for alerts based on survivor status
pub struct AlertGenerator {
    /// Zone name lookup (would be connected to event in production)
    zone_names: std::collections::HashMap<ScanZoneId, String>,
}

impl AlertGenerator {
    /// Create a new alert generator
    pub fn new() -> Self {
        Self {
            zone_names: std::collections::HashMap::new(),
        }
    }

    /// Register a zone name
    pub fn register_zone(&mut self, zone_id: ScanZoneId, name: String) {
        self.zone_names.insert(zone_id, name);
    }

    /// Generate an alert for a survivor
    pub fn generate(&self, survivor: &Survivor) -> Result<Alert, MatError> {
        let priority = Priority::from_triage(survivor.triage_status());
        let payload = self.create_payload(survivor);

        Ok(Alert::new(survivor.id().clone(), priority, payload))
    }

    /// Generate an escalation alert
    pub fn generate_escalation(
        &self,
        survivor: &Survivor,
        reason: &str,
    ) -> Result<Alert, MatError> {
        let mut payload = self.create_payload(survivor);
        payload.title = format!("ESCALATED: {}", payload.title);
        payload.message = format!(
            "{}\n\nReason for escalation: {}",
            payload.message, reason
        );

        // Escalated alerts are always at least high priority
        let priority = match survivor.triage_status() {
            TriageStatus::Immediate => Priority::Critical,
            _ => Priority::High,
        };

        Ok(Alert::new(survivor.id().clone(), priority, payload))
    }

    /// Generate a status change alert
    pub fn generate_status_change(
        &self,
        survivor: &Survivor,
        previous_status: &TriageStatus,
    ) -> Result<Alert, MatError> {
        let mut payload = self.create_payload(survivor);

        payload.title = format!(
            "Status Change: {} → {}",
            previous_status, survivor.triage_status()
        );

        // Determine if this is an upgrade (worse) or downgrade (better)
        let is_upgrade = survivor.triage_status().priority() < previous_status.priority();

        if is_upgrade {
            payload.message = format!(
                "URGENT: Survivor condition has WORSENED.\n{}\n\nPrevious: {}\nCurrent: {}",
                payload.message,
                previous_status,
                survivor.triage_status()
            );
        } else {
            payload.message = format!(
                "Survivor condition has improved.\n{}\n\nPrevious: {}\nCurrent: {}",
                payload.message,
                previous_status,
                survivor.triage_status()
            );
        }

        let priority = if is_upgrade {
            Priority::from_triage(survivor.triage_status())
        } else {
            Priority::Medium
        };

        Ok(Alert::new(survivor.id().clone(), priority, payload))
    }

    /// Create alert payload from survivor data
    fn create_payload(&self, survivor: &Survivor) -> AlertPayload {
        let zone_name = self.zone_names
            .get(survivor.zone_id())
            .map(String::as_str)
            .unwrap_or("Unknown Zone");

        let title = format!(
            "{} Survivor Detected - {}",
            survivor.triage_status(),
            zone_name
        );

        let vital_info = self.format_vital_signs(survivor);
        let location_info = self.format_location(survivor);

        let message = format!(
            "Survivor ID: {}\n\
             Zone: {}\n\
             Triage: {}\n\
             Confidence: {:.0}%\n\n\
             Vital Signs:\n{}\n\n\
             Location:\n{}",
            survivor.id(),
            zone_name,
            survivor.triage_status(),
            survivor.confidence() * 100.0,
            vital_info,
            location_info
        );

        let recommended_action = self.recommend_action(survivor);

        AlertPayload::new(title, message, survivor.triage_status().clone())
            .with_action(recommended_action)
            .with_metadata("zone_id", survivor.zone_id().to_string())
            .with_metadata("confidence", format!("{:.2}", survivor.confidence()))
    }

    /// Format vital signs for display
    fn format_vital_signs(&self, survivor: &Survivor) -> String {
        let vitals = survivor.vital_signs();

        let mut lines = Vec::new();

        if let Some(reading) = vitals.latest() {
            if let Some(breathing) = &reading.breathing {
                lines.push(format!(
                    "  Breathing: {:.1} BPM ({:?})",
                    breathing.rate_bpm, breathing.pattern_type
                ));
            } else {
                lines.push("  Breathing: Not detected".to_string());
            }

            if let Some(heartbeat) = &reading.heartbeat {
                lines.push(format!(
                    "  Heartbeat: {:.0} BPM ({:?})",
                    heartbeat.rate_bpm, heartbeat.strength
                ));
            }

            lines.push(format!(
                "  Movement: {:?} (intensity: {:.1})",
                reading.movement.movement_type,
                reading.movement.intensity
            ));
        } else {
            lines.push("  No recent readings".to_string());
        }

        lines.join("\n")
    }

    /// Format location for display
    fn format_location(&self, survivor: &Survivor) -> String {
        match survivor.location() {
            Some(loc) => {
                let depth_str = if loc.is_buried() {
                    format!("{:.1}m below surface", loc.depth())
                } else {
                    "At surface level".to_string()
                };

                format!(
                    "  Position: ({:.1}, {:.1})\n\
                     Depth: {}\n\
                     Uncertainty: ±{:.1}m",
                    loc.x, loc.y,
                    depth_str,
                    loc.uncertainty.horizontal_error
                )
            }
            None => "  Position not yet determined".to_string(),
        }
    }

    /// Recommend action based on triage status
    fn recommend_action(&self, survivor: &Survivor) -> String {
        match survivor.triage_status() {
            TriageStatus::Immediate => {
                "IMMEDIATE RESCUE REQUIRED. Deploy heavy rescue team. \
                 Prepare for airway management and critical care on extraction."
            }
            TriageStatus::Delayed => {
                "Rescue team required. Mark location. Provide reassurance \
                 if communication is possible. Monitor for status changes."
            }
            TriageStatus::Minor => {
                "Lower priority. Guide to extraction if conscious and mobile. \
                 Assign walking wounded assistance team."
            }
            TriageStatus::Deceased => {
                "Mark location for recovery. Do not allocate rescue resources. \
                 Document for incident report."
            }
            TriageStatus::Unknown => {
                "Requires additional assessment. Deploy scout team with \
                 enhanced detection equipment to confirm status."
            }
        }
        .to_string()
    }
}

impl Default for AlertGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BreathingPattern, BreathingType, ConfidenceScore, VitalSignsReading};
    use chrono::Utc;

    fn create_test_survivor() -> Survivor {
        let vitals = VitalSignsReading {
            breathing: Some(BreathingPattern {
                rate_bpm: 35.0,
                amplitude: 0.7,
                regularity: 0.5,
                pattern_type: BreathingType::Labored,
            }),
            heartbeat: None,
            movement: Default::default(),
            timestamp: Utc::now(),
            confidence: ConfidenceScore::new(0.8),
        };

        Survivor::new(ScanZoneId::new(), vitals, None)
    }

    #[test]
    fn test_generate_alert() {
        let generator = AlertGenerator::new();
        let survivor = create_test_survivor();

        let result = generator.generate(&survivor);
        assert!(result.is_ok());

        let alert = result.unwrap();
        assert!(alert.is_pending());
    }

    #[test]
    fn test_escalation_alert() {
        let generator = AlertGenerator::new();
        let survivor = create_test_survivor();

        let alert = generator.generate_escalation(&survivor, "Vital signs deteriorating")
            .unwrap();

        assert!(alert.payload().title.contains("ESCALATED"));
        assert!(matches!(alert.priority(), Priority::Critical | Priority::High));
    }

    #[test]
    fn test_status_change_alert() {
        let generator = AlertGenerator::new();
        let survivor = create_test_survivor();

        let alert = generator.generate_status_change(
            &survivor,
            &TriageStatus::Minor,
        ).unwrap();

        assert!(alert.payload().title.contains("Status Change"));
    }
}
