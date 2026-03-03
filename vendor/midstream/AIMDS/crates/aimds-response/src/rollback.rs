//! Rollback manager for safe mitigation reversal

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::{MitigationAction, Result, ResponseError};

/// Manages rollback of mitigation actions
pub struct RollbackManager {
    /// Stack of reversible actions
    action_stack: Arc<RwLock<Vec<RollbackEntry>>>,

    /// Rollback history
    history: Arc<RwLock<Vec<RollbackRecord>>>,

    /// Maximum stack size
    max_stack_size: usize,
}

impl RollbackManager {
    /// Create new rollback manager
    pub fn new() -> Self {
        Self {
            action_stack: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_stack_size: 1000,
        }
    }

    /// Create with custom max stack size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            action_stack: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_stack_size: max_size,
        }
    }

    /// Push action onto rollback stack
    pub async fn push_action(&self, action: MitigationAction, action_id: String) -> Result<()> {
        let mut stack = self.action_stack.write().await;

        // Check stack size limit
        if stack.len() >= self.max_stack_size {
            // Remove oldest entry
            stack.remove(0);
        }

        let entry = RollbackEntry {
            action,
            action_id,
            timestamp: chrono::Utc::now(),
            context: HashMap::new(),
        };

        stack.push(entry);
        Ok(())
    }

    /// Rollback the last action
    pub async fn rollback_last(&self) -> Result<()> {
        let mut stack = self.action_stack.write().await;

        if let Some(entry) = stack.pop() {
            let result = self.execute_rollback(&entry).await;

            // Record rollback attempt
            let mut history = self.history.write().await;
            history.push(RollbackRecord {
                action_id: entry.action_id.clone(),
                success: result.is_ok(),
                timestamp: chrono::Utc::now(),
                error: result.as_ref().err().map(|e| e.to_string()),
            });

            result
        } else {
            Err(ResponseError::RollbackFailed("No actions to rollback".to_string()))
        }
    }

    /// Rollback specific action by ID
    pub async fn rollback_action(&self, action_id: &str) -> Result<()> {
        let mut stack = self.action_stack.write().await;

        // Find and remove action from stack
        if let Some(pos) = stack.iter().position(|e| e.action_id == action_id) {
            let entry = stack.remove(pos);
            let result = self.execute_rollback(&entry).await;

            // Record rollback attempt
            let mut history = self.history.write().await;
            history.push(RollbackRecord {
                action_id: entry.action_id.clone(),
                success: result.is_ok(),
                timestamp: chrono::Utc::now(),
                error: result.as_ref().err().map(|e| e.to_string()),
            });

            result
        } else {
            Err(ResponseError::RollbackFailed(
                format!("Action {} not found", action_id)
            ))
        }
    }

    /// Rollback all actions
    pub async fn rollback_all(&self) -> Result<Vec<String>> {
        let mut stack = self.action_stack.write().await;
        let mut rolled_back = Vec::new();
        let mut errors = Vec::new();

        while let Some(entry) = stack.pop() {
            match self.execute_rollback(&entry).await {
                Ok(_) => {
                    rolled_back.push(entry.action_id.clone());
                }
                Err(e) => {
                    errors.push(format!("Failed to rollback {}: {}", entry.action_id, e));
                }
            }

            // Record rollback attempt
            let mut history = self.history.write().await;
            history.push(RollbackRecord {
                action_id: entry.action_id.clone(),
                success: errors.is_empty(),
                timestamp: chrono::Utc::now(),
                error: errors.last().cloned(),
            });
        }

        if errors.is_empty() {
            Ok(rolled_back)
        } else {
            Err(ResponseError::RollbackFailed(errors.join("; ")))
        }
    }

    /// Get rollback history
    pub async fn history(&self) -> Vec<RollbackRecord> {
        self.history.read().await.clone()
    }

    /// Get current stack size
    pub async fn stack_size(&self) -> usize {
        self.action_stack.read().await.len()
    }

    /// Clear rollback stack (use with caution)
    pub async fn clear_stack(&self) {
        let mut stack = self.action_stack.write().await;
        stack.clear();
    }

    /// Execute rollback for entry
    async fn execute_rollback(&self, entry: &RollbackEntry) -> Result<()> {
        tracing::info!("Rolling back action: {}", entry.action_id);

        match entry.action.rollback(&entry.action_id) {
            Ok(_) => {
                metrics::counter!("rollback.success").increment(1);
                Ok(())
            }
            Err(e) => {
                metrics::counter!("rollback.failure").increment(1);
                Err(e)
            }
        }
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in rollback stack
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RollbackEntry {
    action: MitigationAction,
    action_id: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    context: HashMap<String, String>,
}

/// Record of rollback attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub action_id: String,
    pub success: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MitigationAction;
    use std::time::Duration;

    #[tokio::test]
    async fn test_rollback_manager_creation() {
        let manager = RollbackManager::new();
        assert_eq!(manager.stack_size().await, 0);
    }

    #[tokio::test]
    async fn test_push_action() {
        let manager = RollbackManager::new();

        let action = MitigationAction::BlockRequest {
            reason: "Test".to_string(),
        };

        manager.push_action(action, "action-1".to_string()).await.unwrap();
        assert_eq!(manager.stack_size().await, 1);
    }

    #[tokio::test]
    async fn test_rollback_last() {
        let manager = RollbackManager::new();

        let action = MitigationAction::RateLimitUser {
            duration: Duration::from_secs(60),
        };

        manager.push_action(action, "action-1".to_string()).await.unwrap();
        assert_eq!(manager.stack_size().await, 1);

        let result = manager.rollback_last().await;
        assert!(result.is_ok());
        assert_eq!(manager.stack_size().await, 0);
    }

    #[tokio::test]
    async fn test_rollback_specific_action() {
        let manager = RollbackManager::new();

        let action1 = MitigationAction::BlockRequest {
            reason: "Test 1".to_string(),
        };
        let action2 = MitigationAction::BlockRequest {
            reason: "Test 2".to_string(),
        };

        manager.push_action(action1, "action-1".to_string()).await.unwrap();
        manager.push_action(action2, "action-2".to_string()).await.unwrap();

        assert_eq!(manager.stack_size().await, 2);

        manager.rollback_action("action-1").await.unwrap();
        assert_eq!(manager.stack_size().await, 1);
    }

    #[tokio::test]
    async fn test_rollback_all() {
        let manager = RollbackManager::new();

        for i in 0..5 {
            let action = MitigationAction::BlockRequest {
                reason: format!("Test {}", i),
            };
            manager.push_action(action, format!("action-{}", i)).await.unwrap();
        }

        assert_eq!(manager.stack_size().await, 5);

        let result = manager.rollback_all().await;
        assert!(result.is_ok());
        assert_eq!(manager.stack_size().await, 0);
    }

    #[tokio::test]
    async fn test_max_stack_size() {
        let manager = RollbackManager::with_max_size(3);

        for i in 0..5 {
            let action = MitigationAction::BlockRequest {
                reason: format!("Test {}", i),
            };
            manager.push_action(action, format!("action-{}", i)).await.unwrap();
        }

        // Should only keep last 3
        assert_eq!(manager.stack_size().await, 3);
    }

    #[tokio::test]
    async fn test_rollback_history() {
        let manager = RollbackManager::new();

        let action = MitigationAction::BlockRequest {
            reason: "Test".to_string(),
        };

        manager.push_action(action, "action-1".to_string()).await.unwrap();
        manager.rollback_last().await.unwrap();

        let history = manager.history().await;
        assert_eq!(history.len(), 1);
        assert!(history[0].success);
    }
}
