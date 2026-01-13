//! Alert dispatching and delivery.

use crate::domain::{Alert, AlertId, Priority, Survivor};
use crate::MatError;
use super::AlertGenerator;
use std::collections::HashMap;

/// Configuration for alert dispatch
#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// Enable audio alerts
    pub audio_enabled: bool,
    /// Enable visual alerts
    pub visual_enabled: bool,
    /// Escalation timeout in seconds
    pub escalation_timeout_secs: u64,
    /// Maximum pending alerts before forced escalation
    pub max_pending_alerts: usize,
    /// Auto-acknowledge after seconds (0 = disabled)
    pub auto_ack_secs: u64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            audio_enabled: true,
            visual_enabled: true,
            escalation_timeout_secs: 300, // 5 minutes
            max_pending_alerts: 50,
            auto_ack_secs: 0, // Disabled
        }
    }
}

/// Dispatcher for sending alerts to rescue teams
pub struct AlertDispatcher {
    config: AlertConfig,
    generator: AlertGenerator,
    pending_alerts: parking_lot::RwLock<HashMap<AlertId, Alert>>,
    handlers: Vec<Box<dyn AlertHandler>>,
}

impl AlertDispatcher {
    /// Create a new alert dispatcher
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config,
            generator: AlertGenerator::new(),
            pending_alerts: parking_lot::RwLock::new(HashMap::new()),
            handlers: Vec::new(),
        }
    }

    /// Add an alert handler
    pub fn add_handler(&mut self, handler: Box<dyn AlertHandler>) {
        self.handlers.push(handler);
    }

    /// Generate an alert for a survivor
    pub fn generate_alert(&self, survivor: &Survivor) -> Result<Alert, MatError> {
        self.generator.generate(survivor)
    }

    /// Dispatch an alert
    pub async fn dispatch(&self, alert: Alert) -> Result<(), MatError> {
        let alert_id = alert.id().clone();
        let priority = alert.priority();

        // Store in pending alerts
        self.pending_alerts.write().insert(alert_id.clone(), alert.clone());

        // Log the alert
        tracing::info!(
            alert_id = %alert_id,
            priority = ?priority,
            title = %alert.payload().title,
            "Dispatching alert"
        );

        // Send to all handlers
        for handler in &self.handlers {
            if let Err(e) = handler.handle(&alert).await {
                tracing::warn!(
                    alert_id = %alert_id,
                    handler = %handler.name(),
                    error = %e,
                    "Handler failed to process alert"
                );
            }
        }

        // Check if we're at capacity
        let pending_count = self.pending_alerts.read().len();
        if pending_count >= self.config.max_pending_alerts {
            tracing::warn!(
                pending_count,
                max = self.config.max_pending_alerts,
                "Alert capacity reached - escalating oldest alerts"
            );
            self.escalate_oldest().await?;
        }

        Ok(())
    }

    /// Acknowledge an alert
    pub fn acknowledge(&self, alert_id: &AlertId, by: &str) -> Result<(), MatError> {
        let mut alerts = self.pending_alerts.write();

        if let Some(alert) = alerts.get_mut(alert_id) {
            alert.acknowledge(by);
            tracing::info!(
                alert_id = %alert_id,
                acknowledged_by = by,
                "Alert acknowledged"
            );
            Ok(())
        } else {
            Err(MatError::Alerting(format!("Alert {} not found", alert_id)))
        }
    }

    /// Resolve an alert
    pub fn resolve(&self, alert_id: &AlertId, resolution: crate::domain::AlertResolution) -> Result<(), MatError> {
        let mut alerts = self.pending_alerts.write();

        if let Some(alert) = alerts.remove(alert_id) {
            let mut resolved_alert = alert;
            resolved_alert.resolve(resolution);
            tracing::info!(
                alert_id = %alert_id,
                "Alert resolved"
            );
            Ok(())
        } else {
            Err(MatError::Alerting(format!("Alert {} not found", alert_id)))
        }
    }

    /// Get all pending alerts
    pub fn pending(&self) -> Vec<Alert> {
        self.pending_alerts.read().values().cloned().collect()
    }

    /// Get pending alerts by priority
    pub fn pending_by_priority(&self, priority: Priority) -> Vec<Alert> {
        self.pending_alerts
            .read()
            .values()
            .filter(|a| a.priority() == priority)
            .cloned()
            .collect()
    }

    /// Get count of pending alerts
    pub fn pending_count(&self) -> usize {
        self.pending_alerts.read().len()
    }

    /// Check and escalate timed-out alerts
    pub async fn check_escalations(&self) -> Result<u32, MatError> {
        let timeout_secs = self.config.escalation_timeout_secs as i64;
        let mut escalated = 0;

        let mut to_escalate = Vec::new();
        {
            let alerts = self.pending_alerts.read();
            for (id, alert) in alerts.iter() {
                if alert.needs_escalation(timeout_secs) {
                    to_escalate.push(id.clone());
                }
            }
        }

        for id in to_escalate {
            let mut alerts = self.pending_alerts.write();
            if let Some(alert) = alerts.get_mut(&id) {
                alert.escalate();
                escalated += 1;

                tracing::warn!(
                    alert_id = %id,
                    new_priority = ?alert.priority(),
                    "Alert escalated due to timeout"
                );
            }
        }

        Ok(escalated)
    }

    /// Escalate oldest pending alerts
    async fn escalate_oldest(&self) -> Result<(), MatError> {
        let mut alerts: Vec<_> = self.pending_alerts.read()
            .iter()
            .map(|(id, alert)| (id.clone(), *alert.created_at()))
            .collect();

        // Sort by creation time (oldest first)
        alerts.sort_by_key(|(_, created)| *created);

        // Escalate oldest 10%
        let to_escalate = (alerts.len() / 10).max(1);

        let mut pending = self.pending_alerts.write();
        for (id, _) in alerts.into_iter().take(to_escalate) {
            if let Some(alert) = pending.get_mut(&id) {
                alert.escalate();
            }
        }

        Ok(())
    }

    /// Get configuration
    pub fn config(&self) -> &AlertConfig {
        &self.config
    }
}

/// Handler for processing alerts
#[async_trait::async_trait]
pub trait AlertHandler: Send + Sync {
    /// Handler name
    fn name(&self) -> &str;

    /// Handle an alert
    async fn handle(&self, alert: &Alert) -> Result<(), MatError>;
}

/// Console/logging alert handler
pub struct ConsoleAlertHandler;

#[async_trait::async_trait]
impl AlertHandler for ConsoleAlertHandler {
    fn name(&self) -> &str {
        "console"
    }

    async fn handle(&self, alert: &Alert) -> Result<(), MatError> {
        let priority_indicator = match alert.priority() {
            Priority::Critical => "🔴",
            Priority::High => "🟠",
            Priority::Medium => "🟡",
            Priority::Low => "🔵",
        };

        println!("\n{} ALERT {}", priority_indicator, "=".repeat(50));
        println!("ID: {}", alert.id());
        println!("Priority: {:?}", alert.priority());
        println!("Title: {}", alert.payload().title);
        println!("{}", "=".repeat(60));
        println!("{}", alert.payload().message);
        println!("{}", "=".repeat(60));
        println!("Recommended Action: {}", alert.payload().recommended_action);
        println!("{}\n", "=".repeat(60));

        Ok(())
    }
}

/// Audio alert handler (placeholder)
pub struct AudioAlertHandler;

#[async_trait::async_trait]
impl AlertHandler for AudioAlertHandler {
    fn name(&self) -> &str {
        "audio"
    }

    async fn handle(&self, alert: &Alert) -> Result<(), MatError> {
        // In production, this would trigger actual audio alerts
        let pattern = alert.priority().audio_pattern();
        tracing::debug!(
            alert_id = %alert.id(),
            pattern,
            "Would play audio alert"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SurvivorId, TriageStatus, AlertPayload};

    fn create_test_alert() -> Alert {
        Alert::new(
            SurvivorId::new(),
            Priority::High,
            AlertPayload::new("Test Alert", "Test message", TriageStatus::Delayed),
        )
    }

    #[tokio::test]
    async fn test_dispatch_alert() {
        let dispatcher = AlertDispatcher::new(AlertConfig::default());
        let alert = create_test_alert();

        let result = dispatcher.dispatch(alert).await;
        assert!(result.is_ok());
        assert_eq!(dispatcher.pending_count(), 1);
    }

    #[tokio::test]
    async fn test_acknowledge_alert() {
        let dispatcher = AlertDispatcher::new(AlertConfig::default());
        let alert = create_test_alert();
        let alert_id = alert.id().clone();

        dispatcher.dispatch(alert).await.unwrap();

        let result = dispatcher.acknowledge(&alert_id, "Team Alpha");
        assert!(result.is_ok());

        let pending = dispatcher.pending();
        assert!(pending.iter().any(|a| a.id() == &alert_id && a.acknowledged_by() == Some("Team Alpha")));
    }

    #[tokio::test]
    async fn test_resolve_alert() {
        let dispatcher = AlertDispatcher::new(AlertConfig::default());
        let alert = create_test_alert();
        let alert_id = alert.id().clone();

        dispatcher.dispatch(alert).await.unwrap();

        let resolution = crate::domain::AlertResolution {
            resolution_type: crate::domain::ResolutionType::Rescued,
            notes: "Survivor extracted successfully".to_string(),
            resolved_by: Some("Team Alpha".to_string()),
            resolved_at: chrono::Utc::now(),
        };

        dispatcher.resolve(&alert_id, resolution).unwrap();
        assert_eq!(dispatcher.pending_count(), 0);
    }
}
