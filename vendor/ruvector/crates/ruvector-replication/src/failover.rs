//! Automatic failover and high availability
//!
//! Provides failover management with health monitoring,
//! quorum-based decision making, and split-brain prevention.

use crate::{Replica, ReplicaRole, ReplicaSet, ReplicationError, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

/// Health status of a replica
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Replica is healthy
    Healthy,
    /// Replica is degraded but operational
    Degraded,
    /// Replica is unhealthy
    Unhealthy,
    /// Replica is not responding
    Unresponsive,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Replica ID
    pub replica_id: String,
    /// Health status
    pub status: HealthStatus,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Error message if unhealthy
    pub error: Option<String>,
    /// Timestamp of the check
    pub timestamp: DateTime<Utc>,
}

impl HealthCheck {
    /// Create a healthy check result
    pub fn healthy(replica_id: String, response_time_ms: u64) -> Self {
        Self {
            replica_id,
            status: HealthStatus::Healthy,
            response_time_ms,
            error: None,
            timestamp: Utc::now(),
        }
    }

    /// Create an unhealthy check result
    pub fn unhealthy(replica_id: String, error: String) -> Self {
        Self {
            replica_id,
            status: HealthStatus::Unhealthy,
            response_time_ms: 0,
            error: Some(error),
            timestamp: Utc::now(),
        }
    }

    /// Create an unresponsive check result
    pub fn unresponsive(replica_id: String) -> Self {
        Self {
            replica_id,
            status: HealthStatus::Unresponsive,
            response_time_ms: 0,
            error: Some("No response".to_string()),
            timestamp: Utc::now(),
        }
    }
}

/// Failover policy configuration
#[derive(Debug, Clone)]
pub struct FailoverPolicy {
    /// Enable automatic failover
    pub auto_failover: bool,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Timeout for health checks
    pub health_check_timeout: Duration,
    /// Number of consecutive failures before failover
    pub failure_threshold: usize,
    /// Minimum quorum size for failover
    pub min_quorum: usize,
    /// Enable split-brain prevention
    pub prevent_split_brain: bool,
}

impl Default for FailoverPolicy {
    fn default() -> Self {
        Self {
            auto_failover: true,
            health_check_interval: Duration::from_secs(5),
            health_check_timeout: Duration::from_secs(2),
            failure_threshold: 3,
            min_quorum: 2,
            prevent_split_brain: true,
        }
    }
}

/// Manages automatic failover and health monitoring
pub struct FailoverManager {
    /// The replica set
    replica_set: Arc<RwLock<ReplicaSet>>,
    /// Failover policy
    policy: Arc<RwLock<FailoverPolicy>>,
    /// Health check history
    health_history: Arc<RwLock<Vec<HealthCheck>>>,
    /// Failure counts by replica
    failure_counts: Arc<RwLock<std::collections::HashMap<String, usize>>>,
    /// Whether failover is in progress
    failover_in_progress: Arc<RwLock<bool>>,
}

impl FailoverManager {
    /// Create a new failover manager
    pub fn new(replica_set: Arc<RwLock<ReplicaSet>>) -> Self {
        Self {
            replica_set,
            policy: Arc::new(RwLock::new(FailoverPolicy::default())),
            health_history: Arc::new(RwLock::new(Vec::new())),
            failure_counts: Arc::new(RwLock::new(std::collections::HashMap::new())),
            failover_in_progress: Arc::new(RwLock::new(false)),
        }
    }

    /// Create with custom policy
    pub fn with_policy(replica_set: Arc<RwLock<ReplicaSet>>, policy: FailoverPolicy) -> Self {
        Self {
            replica_set,
            policy: Arc::new(RwLock::new(policy)),
            health_history: Arc::new(RwLock::new(Vec::new())),
            failure_counts: Arc::new(RwLock::new(std::collections::HashMap::new())),
            failover_in_progress: Arc::new(RwLock::new(false)),
        }
    }

    /// Set the failover policy
    pub fn set_policy(&self, policy: FailoverPolicy) {
        *self.policy.write() = policy;
    }

    /// Get the current policy
    pub fn policy(&self) -> FailoverPolicy {
        self.policy.read().clone()
    }

    /// Start health monitoring
    pub async fn start_monitoring(&self) {
        let policy = self.policy.read().clone();
        let replica_set = self.replica_set.clone();
        let health_history = self.health_history.clone();
        let failure_counts = self.failure_counts.clone();
        let failover_in_progress = self.failover_in_progress.clone();
        let manager_policy = self.policy.clone();

        tokio::spawn(async move {
            let mut interval_timer = interval(policy.health_check_interval);

            loop {
                interval_timer.tick().await;

                let replica_ids = {
                    let set = replica_set.read();
                    set.replica_ids()
                };

                for replica_id in replica_ids {
                    let health = Self::check_replica_health(
                        &replica_set,
                        &replica_id,
                        policy.health_check_timeout,
                    )
                    .await;

                    // Record health check
                    health_history.write().push(health.clone());

                    // Update failure count and check if failover is needed
                    // Use a scope to ensure lock is dropped before any await
                    let should_failover = {
                        let mut counts = failure_counts.write();
                        let count = counts.entry(replica_id.clone()).or_insert(0);

                        match health.status {
                            HealthStatus::Healthy => {
                                *count = 0;
                                false
                            }
                            HealthStatus::Degraded => {
                                // Don't increment for degraded
                                false
                            }
                            HealthStatus::Unhealthy | HealthStatus::Unresponsive => {
                                *count += 1;

                                // Check if failover is needed
                                let current_policy = manager_policy.read();
                                *count >= current_policy.failure_threshold
                                    && current_policy.auto_failover
                            }
                        }
                    }; // Lock is dropped here

                    // Trigger failover if needed (after lock is dropped)
                    if should_failover {
                        if let Err(e) =
                            Self::trigger_failover(&replica_set, &failover_in_progress).await
                        {
                            tracing::error!("Failover failed: {}", e);
                        }
                    }
                }

                // Trim health history to last 1000 entries
                let mut history = health_history.write();
                let len = history.len();
                if len > 1000 {
                    history.drain(0..len - 1000);
                }
            }
        });
    }

    /// Check health of a specific replica
    async fn check_replica_health(
        replica_set: &Arc<RwLock<ReplicaSet>>,
        replica_id: &str,
        timeout: Duration,
    ) -> HealthCheck {
        // In a real implementation, this would make a network call
        // For now, we simulate health checks based on replica status

        let replica = {
            let set = replica_set.read();
            set.get_replica(replica_id)
        };

        match replica {
            Some(replica) => {
                if replica.is_timed_out(timeout) {
                    HealthCheck::unresponsive(replica_id.to_string())
                } else if replica.is_healthy() {
                    HealthCheck::healthy(replica_id.to_string(), 10)
                } else {
                    HealthCheck::unhealthy(replica_id.to_string(), "Replica is lagging".to_string())
                }
            }
            None => HealthCheck::unhealthy(replica_id.to_string(), "Replica not found".to_string()),
        }
    }

    /// Trigger failover to a healthy secondary
    async fn trigger_failover(
        replica_set: &Arc<RwLock<ReplicaSet>>,
        failover_in_progress: &Arc<RwLock<bool>>,
    ) -> Result<()> {
        // Check if failover is already in progress
        {
            let mut in_progress = failover_in_progress.write();
            if *in_progress {
                return Ok(());
            }
            *in_progress = true;
        }

        tracing::warn!("Initiating failover");

        // Find candidate within a scope to drop the lock before await
        let candidate_id = {
            let set = replica_set.read();

            // Check quorum
            if !set.has_quorum() {
                *failover_in_progress.write() = false;
                return Err(ReplicationError::QuorumNotMet {
                    needed: set.get_quorum_size(),
                    available: set.get_healthy_replicas().len(),
                });
            }

            // Find best candidate for promotion
            let candidate = Self::select_failover_candidate(&set)?;
            candidate.id.clone()
        }; // Lock is dropped here

        // Promote the candidate (lock re-acquired inside promote_to_primary)
        let result = {
            let mut set = replica_set.write();
            set.promote_to_primary(&candidate_id)
        };

        match &result {
            Ok(()) => tracing::info!("Failover completed: promoted {} to primary", candidate_id),
            Err(e) => tracing::error!("Failover failed: {}", e),
        }

        // Clear failover flag
        *failover_in_progress.write() = false;

        result
    }

    /// Select the best candidate for failover
    fn select_failover_candidate(replica_set: &ReplicaSet) -> Result<Replica> {
        let mut candidates: Vec<Replica> = replica_set
            .get_healthy_replicas()
            .into_iter()
            .filter(|r| r.role == ReplicaRole::Secondary)
            .collect();

        if candidates.is_empty() {
            return Err(ReplicationError::FailoverFailed(
                "No healthy secondary replicas available".to_string(),
            ));
        }

        // Sort by priority (highest first), then by lowest lag
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.lag_ms.cmp(&b.lag_ms)));

        Ok(candidates[0].clone())
    }

    /// Manually trigger failover
    pub async fn manual_failover(&self, target_replica_id: Option<String>) -> Result<()> {
        let mut set = self.replica_set.write();

        // Check quorum
        if !set.has_quorum() {
            return Err(ReplicationError::QuorumNotMet {
                needed: set.get_quorum_size(),
                available: set.get_healthy_replicas().len(),
            });
        }

        let target = if let Some(id) = target_replica_id {
            set.get_replica(&id)
                .ok_or_else(|| ReplicationError::ReplicaNotFound(id))?
        } else {
            Self::select_failover_candidate(&set)?
        };

        set.promote_to_primary(&target.id)?;

        tracing::info!(
            "Manual failover completed: promoted {} to primary",
            target.id
        );
        Ok(())
    }

    /// Get health check history
    pub fn health_history(&self) -> Vec<HealthCheck> {
        self.health_history.read().clone()
    }

    /// Get recent health status for a replica
    pub fn recent_health(&self, replica_id: &str, limit: usize) -> Vec<HealthCheck> {
        let history = self.health_history.read();
        history
            .iter()
            .rev()
            .filter(|h| h.replica_id == replica_id)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Check if failover is currently in progress
    pub fn is_failover_in_progress(&self) -> bool {
        *self.failover_in_progress.read()
    }

    /// Get failure count for a replica
    pub fn failure_count(&self, replica_id: &str) -> usize {
        self.failure_counts
            .read()
            .get(replica_id)
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check() {
        let check = HealthCheck::healthy("r1".to_string(), 15);
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.response_time_ms, 15);

        let check = HealthCheck::unhealthy("r2".to_string(), "Error".to_string());
        assert_eq!(check.status, HealthStatus::Unhealthy);
        assert!(check.error.is_some());
    }

    #[test]
    fn test_failover_policy() {
        let policy = FailoverPolicy::default();
        assert!(policy.auto_failover);
        assert_eq!(policy.failure_threshold, 3);
    }

    #[test]
    fn test_failover_manager() {
        let mut replica_set = ReplicaSet::new("cluster-1");
        replica_set
            .add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        replica_set
            .add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();

        let manager = FailoverManager::new(Arc::new(RwLock::new(replica_set)));
        assert!(!manager.is_failover_in_progress());
    }

    #[test]
    fn test_candidate_selection() {
        let mut replica_set = ReplicaSet::new("cluster-1");
        replica_set
            .add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        replica_set
            .add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();
        replica_set
            .add_replica("r3", "127.0.0.1:9003", ReplicaRole::Secondary)
            .unwrap();

        let candidate = FailoverManager::select_failover_candidate(&replica_set).unwrap();
        assert!(candidate.role == ReplicaRole::Secondary);
        assert!(candidate.is_healthy());
    }
}
