//! Session State Management
//!
//! Manages conversation sessions including lifecycle, state tracking,
//! and integration with KV cache and adapters.

use crate::error::{Result, RuvLLMError};
use crate::kv_cache::{KvCacheConfig, TwoTierKvCache};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Maximum session lifetime in seconds
    pub max_lifetime_secs: u64,
    /// Session idle timeout in seconds
    pub idle_timeout_secs: u64,
    /// Maximum turns per session
    pub max_turns: u32,
    /// KV cache configuration
    pub kv_cache: KvCacheConfig,
    /// Enable session persistence
    pub persist: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_lifetime_secs: 3600, // 1 hour
            idle_timeout_secs: 300,  // 5 minutes
            max_turns: 100,
            kv_cache: KvCacheConfig::default(),
            persist: true,
        }
    }
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Session is active and accepting requests
    Active,
    /// Session is paused (e.g., user inactive)
    Paused,
    /// Session has expired
    Expired,
    /// Session was terminated by user/system
    Terminated,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Custom key-value pairs
    pub custom: HashMap<String, serde_json::Value>,
    /// User agent string
    pub user_agent: Option<String>,
    /// Client IP (if available)
    pub client_ip: Option<String>,
    /// Language/locale
    pub locale: Option<String>,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            custom: HashMap::new(),
            user_agent: None,
            client_ip: None,
            locale: None,
        }
    }
}

/// A conversation session
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// User identifier (if authenticated)
    pub user_id: Option<String>,
    /// Current status
    pub status: SessionStatus,
    /// Turn count
    pub turn_count: u32,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_active: chrono::DateTime<chrono::Utc>,
    /// Active LoRA adapter ID
    pub active_adapter: Option<Uuid>,
    /// Session metadata
    pub metadata: SessionMetadata,
    /// Context embedding (for semantic search)
    pub context_embedding: Option<Vec<f32>>,
    /// KV cache for this session
    kv_cache: Arc<TwoTierKvCache>,
}

impl Session {
    /// Create a new session
    pub fn new(config: &SessionConfig, user_id: Option<&str>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.map(String::from),
            status: SessionStatus::Active,
            turn_count: 0,
            created_at: now,
            last_active: now,
            active_adapter: None,
            metadata: SessionMetadata::default(),
            context_embedding: None,
            kv_cache: Arc::new(TwoTierKvCache::new(config.kv_cache.clone())),
        }
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.status == SessionStatus::Active
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_active = chrono::Utc::now();
    }

    /// Increment turn count
    pub fn increment_turn(&mut self) {
        self.turn_count += 1;
        self.touch();
    }

    /// Check if session has expired based on config
    pub fn is_expired(&self, config: &SessionConfig) -> bool {
        let now = chrono::Utc::now();

        // Check lifetime
        let lifetime = (now - self.created_at).num_seconds() as u64;
        if lifetime > config.max_lifetime_secs {
            return true;
        }

        // Check idle timeout
        let idle = (now - self.last_active).num_seconds() as u64;
        if idle > config.idle_timeout_secs {
            return true;
        }

        // Check turn limit
        if self.turn_count >= config.max_turns {
            return true;
        }

        false
    }

    /// Get the KV cache
    pub fn kv_cache(&self) -> &Arc<TwoTierKvCache> {
        &self.kv_cache
    }

    /// Set context embedding
    pub fn set_context_embedding(&mut self, embedding: Vec<f32>) {
        self.context_embedding = Some(embedding);
    }

    /// Set active adapter
    pub fn set_adapter(&mut self, adapter_id: Option<Uuid>) {
        self.active_adapter = adapter_id;
    }

    /// Pause the session
    pub fn pause(&mut self) {
        self.status = SessionStatus::Paused;
    }

    /// Resume the session
    pub fn resume(&mut self) {
        self.status = SessionStatus::Active;
        self.touch();
    }

    /// Terminate the session
    pub fn terminate(&mut self) {
        self.status = SessionStatus::Terminated;
    }
}

/// Session manager
pub struct SessionManager {
    /// Configuration
    config: SessionConfig,
    /// Active sessions
    sessions: DashMap<String, Arc<parking_lot::RwLock<Session>>>,
    /// User to session mapping (for user-scoped lookups)
    user_sessions: DashMap<String, Vec<String>>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            sessions: DashMap::new(),
            user_sessions: DashMap::new(),
        }
    }

    /// Create a new session
    pub fn create_session(&self, user_id: Option<&str>) -> Result<Session> {
        let session = Session::new(&self.config, user_id);
        let session_id = session.id.clone();

        // Track user sessions
        if let Some(uid) = user_id {
            self.user_sessions
                .entry(uid.to_string())
                .or_default()
                .push(session_id.clone());
        }

        // Store session
        let session_ref = Arc::new(parking_lot::RwLock::new(session));
        self.sessions.insert(session_id.clone(), session_ref);

        // Return a copy
        Ok(self
            .sessions
            .get(&session_id)
            .map(|s| {
                let guard = s.read();
                Session {
                    id: guard.id.clone(),
                    user_id: guard.user_id.clone(),
                    status: guard.status,
                    turn_count: guard.turn_count,
                    created_at: guard.created_at,
                    last_active: guard.last_active,
                    active_adapter: guard.active_adapter,
                    metadata: guard.metadata.clone(),
                    context_embedding: guard.context_embedding.clone(),
                    kv_cache: guard.kv_cache.clone(),
                }
            })
            .ok_or_else(|| RuvLLMError::Session("Failed to create session".to_string()))?)
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        Ok(self.sessions.get(session_id).map(|s| {
            let guard = s.read();
            Session {
                id: guard.id.clone(),
                user_id: guard.user_id.clone(),
                status: guard.status,
                turn_count: guard.turn_count,
                created_at: guard.created_at,
                last_active: guard.last_active,
                active_adapter: guard.active_adapter,
                metadata: guard.metadata.clone(),
                context_embedding: guard.context_embedding.clone(),
                kv_cache: guard.kv_cache.clone(),
            }
        }))
    }

    /// Update session
    pub fn update_session<F>(&self, session_id: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut Session),
    {
        if let Some(session) = self.sessions.get(session_id) {
            let mut guard = session.write();
            f(&mut guard);
            Ok(())
        } else {
            Err(RuvLLMError::NotFound(format!(
                "Session not found: {}",
                session_id
            )))
        }
    }

    /// Terminate a session
    pub fn terminate_session(&self, session_id: &str) -> Result<()> {
        if let Some(session) = self.sessions.get(session_id) {
            let mut guard = session.write();
            guard.terminate();

            // Remove from user sessions
            if let Some(uid) = &guard.user_id {
                if let Some(mut sessions) = self.user_sessions.get_mut(uid) {
                    sessions.retain(|s| s != session_id);
                }
            }
        }

        // Remove from sessions map
        self.sessions.remove(session_id);

        Ok(())
    }

    /// Get sessions for a user
    pub fn get_user_sessions(&self, user_id: &str) -> Vec<String> {
        self.user_sessions
            .get(user_id)
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let mut expired = Vec::new();

        for entry in self.sessions.iter() {
            let guard = entry.value().read();
            if guard.is_expired(&self.config) {
                expired.push(guard.id.clone());
            }
        }

        let count = expired.len();
        for session_id in expired {
            let _ = self.terminate_session(&session_id);
        }

        count
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.iter().map(|e| e.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = Session::new(&config, Some("user-123"));

        assert!(session.is_active());
        assert_eq!(session.turn_count, 0);
        assert_eq!(session.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_session_lifecycle() {
        let config = SessionConfig::default();
        let mut session = Session::new(&config, None);

        session.increment_turn();
        assert_eq!(session.turn_count, 1);

        session.pause();
        assert_eq!(session.status, SessionStatus::Paused);

        session.resume();
        assert_eq!(session.status, SessionStatus::Active);

        session.terminate();
        assert_eq!(session.status, SessionStatus::Terminated);
    }

    #[test]
    fn test_session_manager() {
        let config = SessionConfig::default();
        let manager = SessionManager::new(config);

        let session = manager.create_session(Some("user-1")).unwrap();
        let session_id = session.id.clone();

        assert!(manager.get_session(&session_id).unwrap().is_some());

        let user_sessions = manager.get_user_sessions("user-1");
        assert_eq!(user_sessions.len(), 1);

        manager.terminate_session(&session_id).unwrap();
        assert!(manager.get_session(&session_id).unwrap().is_none());
    }
}
