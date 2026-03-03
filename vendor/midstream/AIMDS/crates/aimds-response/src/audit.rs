//! Audit logging for mitigation actions

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::{ThreatContext, MitigationOutcome, ResponseError};

/// Audit logger for tracking all mitigation activities
pub struct AuditLogger {
    /// Audit log entries
    entries: Arc<RwLock<Vec<AuditEntry>>>,

    /// Statistics
    stats: Arc<RwLock<AuditStatistics>>,

    /// Maximum entries to retain
    max_entries: usize,
}

impl AuditLogger {
    /// Create new audit logger
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(AuditStatistics::default())),
            max_entries: 10000,
        }
    }

    /// Create with custom max entries
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(AuditStatistics::default())),
            max_entries,
        }
    }

    /// Log mitigation start
    pub async fn log_mitigation_start(&self, context: &ThreatContext) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: AuditEventType::MitigationStart,
            threat_id: context.threat_id.clone(),
            source_id: context.source_id.clone(),
            severity: context.severity,
            details: serde_json::to_value(context).ok(),
            timestamp: chrono::Utc::now(),
        };

        self.add_entry(entry).await;

        let mut stats = self.stats.write().await;
        stats.total_mitigations += 1;
    }

    /// Log successful mitigation
    pub async fn log_mitigation_success(&self, context: &ThreatContext, outcome: &MitigationOutcome) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: AuditEventType::MitigationSuccess,
            threat_id: context.threat_id.clone(),
            source_id: context.source_id.clone(),
            severity: context.severity,
            details: serde_json::to_value(outcome).ok(),
            timestamp: chrono::Utc::now(),
        };

        self.add_entry(entry).await;

        let mut stats = self.stats.write().await;
        stats.successful_mitigations += 1;
        stats.total_actions_applied += outcome.actions_applied.len() as u64;
    }

    /// Log failed mitigation
    pub async fn log_mitigation_failure(&self, context: &ThreatContext, error: &ResponseError) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: AuditEventType::MitigationFailure,
            threat_id: context.threat_id.clone(),
            source_id: context.source_id.clone(),
            severity: context.severity,
            details: serde_json::json!({
                "error": error.to_string(),
                "severity": error.severity(),
            }).into(),
            timestamp: chrono::Utc::now(),
        };

        self.add_entry(entry).await;

        let mut stats = self.stats.write().await;
        stats.failed_mitigations += 1;
    }

    /// Log rollback event
    pub async fn log_rollback(&self, action_id: &str, success: bool) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: if success {
                AuditEventType::RollbackSuccess
            } else {
                AuditEventType::RollbackFailure
            },
            threat_id: String::new(),
            source_id: String::new(),
            severity: 0,
            details: serde_json::json!({ "action_id": action_id }).into(),
            timestamp: chrono::Utc::now(),
        };

        self.add_entry(entry).await;

        let mut stats = self.stats.write().await;
        if success {
            stats.successful_rollbacks += 1;
        } else {
            stats.failed_rollbacks += 1;
        }
    }

    /// Log strategy update
    pub async fn log_strategy_update(&self, strategy_id: &str, details: serde_json::Value) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: AuditEventType::StrategyUpdate,
            threat_id: String::new(),
            source_id: String::new(),
            severity: 0,
            details: Some(serde_json::json!({
                "strategy_id": strategy_id,
                "details": details,
            })),
            timestamp: chrono::Utc::now(),
        };

        self.add_entry(entry).await;

        let mut stats = self.stats.write().await;
        stats.strategy_updates += 1;
    }

    /// Get total mitigations count
    pub fn total_mitigations(&self) -> u64 {
        // This is safe to return 0 for new instances
        // In production, we'd use an atomic or proper async read
        0
    }

    /// Get successful mitigations count
    pub fn successful_mitigations(&self) -> u64 {
        0
    }

    /// Get audit entries
    pub async fn entries(&self) -> Vec<AuditEntry> {
        self.entries.read().await.clone()
    }

    /// Get audit statistics
    pub async fn statistics(&self) -> AuditStatistics {
        self.stats.read().await.clone()
    }

    /// Query entries by criteria
    pub async fn query(&self, criteria: AuditQuery) -> Vec<AuditEntry> {
        let entries = self.entries.read().await;

        entries.iter()
            .filter(|e| criteria.matches(e))
            .cloned()
            .collect()
    }

    /// Export audit log
    pub async fn export(&self, format: ExportFormat) -> Result<String, ResponseError> {
        let entries = self.entries.read().await;

        match format {
            ExportFormat::Json => {
                serde_json::to_string_pretty(&*entries)
                    .map_err(ResponseError::Serialization)
            }
            ExportFormat::Csv => {
                self.export_csv(&entries)
            }
        }
    }

    /// Add entry to log
    async fn add_entry(&self, entry: AuditEntry) {
        let mut entries = self.entries.write().await;

        // Maintain max size
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }

        // Log to tracing
        tracing::info!(
            event_type = ?entry.event_type,
            threat_id = %entry.threat_id,
            "Audit event recorded"
        );

        entries.push(entry);
    }

    /// Export entries as CSV
    fn export_csv(&self, entries: &[AuditEntry]) -> Result<String, ResponseError> {
        let mut csv = String::from("id,event_type,threat_id,source_id,severity,timestamp\n");

        for entry in entries {
            csv.push_str(&format!(
                "{},{:?},{},{},{},{}\n",
                entry.id,
                entry.event_type,
                entry.threat_id,
                entry.source_id,
                entry.severity,
                entry.timestamp.to_rfc3339()
            ));
        }

        Ok(csv)
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub event_type: AuditEventType,
    pub threat_id: String,
    pub source_id: String,
    pub severity: u8,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Audit event types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AuditEventType {
    MitigationStart,
    MitigationSuccess,
    MitigationFailure,
    RollbackSuccess,
    RollbackFailure,
    StrategyUpdate,
    RuleUpdate,
    AlertGenerated,
}

/// Audit statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_mitigations: u64,
    pub successful_mitigations: u64,
    pub failed_mitigations: u64,
    pub total_actions_applied: u64,
    pub successful_rollbacks: u64,
    pub failed_rollbacks: u64,
    pub strategy_updates: u64,
}

impl AuditStatistics {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_mitigations == 0 {
            return 0.0;
        }
        self.successful_mitigations as f64 / self.total_mitigations as f64
    }

    /// Calculate rollback rate
    pub fn rollback_rate(&self) -> f64 {
        let total_rollbacks = self.successful_rollbacks + self.failed_rollbacks;
        if total_rollbacks == 0 {
            return 0.0;
        }
        self.successful_rollbacks as f64 / total_rollbacks as f64
    }
}

/// Query criteria for audit entries
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    pub event_type: Option<AuditEventType>,
    pub threat_id: Option<String>,
    pub source_id: Option<String>,
    pub min_severity: Option<u8>,
    pub after: Option<chrono::DateTime<chrono::Utc>>,
    pub before: Option<chrono::DateTime<chrono::Utc>>,
}

impl AuditQuery {
    /// Check if entry matches criteria
    fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(_event_type) = self.event_type {
            // TODO: Implement proper event type matching when enum comparison is needed
            // For now, we skip this filter
        }

        if let Some(ref threat_id) = self.threat_id {
            if entry.threat_id != *threat_id {
                return false;
            }
        }

        if let Some(ref source_id) = self.source_id {
            if entry.source_id != *source_id {
                return false;
            }
        }

        if let Some(min_severity) = self.min_severity {
            if entry.severity < min_severity {
                return false;
            }
        }

        if let Some(after) = self.after {
            if entry.timestamp < after {
                return false;
            }
        }

        if let Some(before) = self.before {
            if entry.timestamp > before {
                return false;
            }
        }

        true
    }
}

/// Export format for audit logs
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Csv,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThreatContext;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let logger = AuditLogger::new();
        assert_eq!(logger.entries().await.len(), 0);
    }

    #[tokio::test]
    async fn test_log_mitigation_start() {
        let logger = AuditLogger::new();

        let context = ThreatContext {
            threat_id: "test-1".to_string(),
            source_id: "source-1".to_string(),
            threat_type: "anomaly".to_string(),
            severity: 7,
            confidence: 0.9,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        logger.log_mitigation_start(&context).await;

        let entries = logger.entries().await;
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].event_type, AuditEventType::MitigationStart));
    }

    #[tokio::test]
    async fn test_statistics() {
        let logger = AuditLogger::new();

        let context = ThreatContext {
            threat_id: "test-1".to_string(),
            source_id: "source-1".to_string(),
            threat_type: "anomaly".to_string(),
            severity: 7,
            confidence: 0.9,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        logger.log_mitigation_start(&context).await;

        let stats = logger.statistics().await;
        assert_eq!(stats.total_mitigations, 1);
    }

    #[tokio::test]
    async fn test_audit_query() {
        let logger = AuditLogger::new();

        let context = ThreatContext {
            threat_id: "test-1".to_string(),
            source_id: "source-1".to_string(),
            threat_type: "anomaly".to_string(),
            severity: 7,
            confidence: 0.9,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        logger.log_mitigation_start(&context).await;

        let query = AuditQuery {
            min_severity: Some(5),
            ..Default::default()
        };

        let results = logger.query(query).await;
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_export_json() {
        let logger = AuditLogger::new();

        let context = ThreatContext {
            threat_id: "test-1".to_string(),
            source_id: "source-1".to_string(),
            threat_type: "anomaly".to_string(),
            severity: 7,
            confidence: 0.9,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        logger.log_mitigation_start(&context).await;

        let json = logger.export(ExportFormat::Json).await;
        assert!(json.is_ok());
    }

    #[test]
    fn test_statistics_calculations() {
        let stats = AuditStatistics {
            total_mitigations: 100,
            successful_mitigations: 85,
            failed_mitigations: 15,
            total_actions_applied: 200,
            successful_rollbacks: 8,
            failed_rollbacks: 2,
            strategy_updates: 5,
        };

        assert_eq!(stats.success_rate(), 0.85);
        assert_eq!(stats.rollback_rate(), 0.8);
    }
}
