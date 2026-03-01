//! Claude Flow Memory Bridge - Integration with Claude Flow's memory system
//!
//! Provides a bridge to Claude Flow's CLI-based memory system for pattern storage,
//! search, and synchronization with the hive mind.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Result, RuvLLMError};

/// Configuration for Claude Flow bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeFlowBridgeConfig {
    /// CLI command to use (default: npx @claude-flow/cli@latest)
    pub cli_command: String,
    /// Namespace for patterns
    pub patterns_namespace: String,
    /// Namespace for tasks
    pub tasks_namespace: String,
    /// Namespace for agents
    pub agents_namespace: String,
    /// Enable caching of CLI results
    pub enable_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: i64,
    /// Timeout for CLI commands in milliseconds
    pub timeout_ms: u64,
    /// Enable hive sync
    pub enable_hive_sync: bool,
}

impl Default for ClaudeFlowBridgeConfig {
    fn default() -> Self {
        Self {
            cli_command: "npx @claude-flow/cli@latest".to_string(),
            patterns_namespace: "patterns".to_string(),
            tasks_namespace: "tasks".to_string(),
            agents_namespace: "agents".to_string(),
            enable_cache: true,
            cache_ttl_seconds: 300, // 5 minutes
            timeout_ms: 30_000,     // 30 seconds
            enable_hive_sync: true,
        }
    }
}

/// A pattern stored in Claude Flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeFlowPattern {
    /// Pattern key
    pub key: String,
    /// Pattern value/content
    pub value: String,
    /// Namespace
    pub namespace: String,
    /// Tags
    pub tags: Vec<String>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Result of a sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Number of patterns synced
    pub patterns_synced: usize,
    /// Number of tasks synced
    pub tasks_synced: usize,
    /// Sync duration in milliseconds
    pub duration_ms: u64,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Sync timestamp
    pub synced_at: DateTime<Utc>,
}

/// Bridge statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStats {
    /// Total store operations
    pub stores: u64,
    /// Total search operations
    pub searches: u64,
    /// Successful operations
    pub successes: u64,
    /// Failed operations
    pub failures: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Sync operations
    pub syncs: u64,
}

/// Internal statistics
#[derive(Debug, Default)]
struct StatsInternal {
    stores: AtomicU64,
    searches: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    cache_hits: AtomicU64,
    syncs: AtomicU64,
}

/// Cached search result
#[derive(Debug, Clone)]
struct CachedSearch {
    results: Vec<ClaudeFlowPattern>,
    cached_at: DateTime<Utc>,
}

/// Bridge to Claude Flow's memory system
pub struct ClaudeFlowMemoryBridge {
    /// Configuration
    config: ClaudeFlowBridgeConfig,
    /// Search cache
    search_cache: Arc<RwLock<HashMap<String, CachedSearch>>>,
    /// Statistics
    stats: StatsInternal,
    /// Last sync timestamp
    last_sync: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl ClaudeFlowMemoryBridge {
    /// Create new bridge with configuration
    pub fn new(config: ClaudeFlowBridgeConfig) -> Self {
        Self {
            config,
            search_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: StatsInternal::default(),
            last_sync: Arc::new(RwLock::new(None)),
        }
    }

    /// Store a pattern in Claude Flow memory
    pub fn store_pattern(
        &self,
        key: &str,
        value: &str,
        namespace: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> Result<()> {
        self.stats.stores.fetch_add(1, Ordering::SeqCst);

        let ns = namespace.unwrap_or(&self.config.patterns_namespace);

        // Build command
        let mut args = vec![
            "memory".to_string(),
            "store".to_string(),
            "--key".to_string(),
            key.to_string(),
            "--value".to_string(),
            value.to_string(),
            "--namespace".to_string(),
            ns.to_string(),
        ];

        if let Some(tag_list) = tags {
            if !tag_list.is_empty() {
                args.push("--tags".to_string());
                args.push(tag_list.join(","));
            }
        }

        self.execute_cli(&args)?;
        self.stats.successes.fetch_add(1, Ordering::SeqCst);

        // Invalidate cache for this namespace
        self.invalidate_cache(ns);

        Ok(())
    }

    /// Search patterns in Claude Flow memory
    pub fn search_patterns(
        &self,
        query: &str,
        namespace: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<ClaudeFlowPattern>> {
        self.stats.searches.fetch_add(1, Ordering::SeqCst);

        let ns = namespace.unwrap_or(&self.config.patterns_namespace);
        let cache_key = format!("{}:{}:{}", ns, query, limit.unwrap_or(10));

        // Check cache
        if self.config.enable_cache {
            let cache = self.search_cache.read();
            if let Some(cached) = cache.get(&cache_key) {
                let age = Utc::now() - cached.cached_at;
                if age.num_seconds() < self.config.cache_ttl_seconds {
                    self.stats.cache_hits.fetch_add(1, Ordering::SeqCst);
                    self.stats.successes.fetch_add(1, Ordering::SeqCst);
                    return Ok(cached.results.clone());
                }
            }
        }

        // Build command
        let mut args = vec![
            "memory".to_string(),
            "search".to_string(),
            "--query".to_string(),
            query.to_string(),
            "--namespace".to_string(),
            ns.to_string(),
        ];

        if let Some(lim) = limit {
            args.push("--limit".to_string());
            args.push(lim.to_string());
        }

        let output = self.execute_cli(&args)?;
        let patterns = self.parse_search_results(&output, ns)?;

        self.stats.successes.fetch_add(1, Ordering::SeqCst);

        // Update cache
        if self.config.enable_cache {
            let mut cache = self.search_cache.write();
            cache.insert(
                cache_key,
                CachedSearch {
                    results: patterns.clone(),
                    cached_at: Utc::now(),
                },
            );
        }

        Ok(patterns)
    }

    /// Retrieve a specific pattern by key
    pub fn retrieve_pattern(
        &self,
        key: &str,
        namespace: Option<&str>,
    ) -> Result<Option<ClaudeFlowPattern>> {
        let ns = namespace.unwrap_or(&self.config.patterns_namespace);

        let args = vec![
            "memory".to_string(),
            "retrieve".to_string(),
            "--key".to_string(),
            key.to_string(),
            "--namespace".to_string(),
            ns.to_string(),
        ];

        let output = self.execute_cli(&args)?;

        if output.trim().is_empty() || output.contains("not found") {
            return Ok(None);
        }

        let pattern = ClaudeFlowPattern {
            key: key.to_string(),
            value: output.trim().to_string(),
            namespace: ns.to_string(),
            tags: vec![],
            metadata: HashMap::new(),
            created_at: Utc::now(),
        };

        Ok(Some(pattern))
    }

    /// Delete a pattern
    pub fn delete_pattern(&self, key: &str, namespace: Option<&str>) -> Result<bool> {
        let ns = namespace.unwrap_or(&self.config.patterns_namespace);

        let args = vec![
            "memory".to_string(),
            "delete".to_string(),
            "--key".to_string(),
            key.to_string(),
            "--namespace".to_string(),
            ns.to_string(),
        ];

        self.execute_cli(&args)?;
        self.invalidate_cache(ns);

        Ok(true)
    }

    /// Sync with hive mind
    pub fn sync_with_hive(&self) -> Result<SyncResult> {
        if !self.config.enable_hive_sync {
            return Ok(SyncResult {
                patterns_synced: 0,
                tasks_synced: 0,
                duration_ms: 0,
                errors: vec!["Hive sync disabled".to_string()],
                synced_at: Utc::now(),
            });
        }

        self.stats.syncs.fetch_add(1, Ordering::SeqCst);
        let start = std::time::Instant::now();

        let mut errors = Vec::new();
        let mut patterns_synced = 0;
        let mut tasks_synced = 0;

        // Sync patterns
        match self.execute_cli(&[
            "hive-mind".to_string(),
            "memory".to_string(),
            "--action".to_string(),
            "list".to_string(),
        ]) {
            Ok(output) => {
                patterns_synced = output.lines().count();
            }
            Err(e) => {
                errors.push(format!("Pattern sync failed: {}", e));
            }
        }

        // Sync tasks
        match self.execute_cli(&["task".to_string(), "list".to_string()]) {
            Ok(output) => {
                tasks_synced = output.lines().filter(|l| !l.is_empty()).count();
            }
            Err(e) => {
                errors.push(format!("Task sync failed: {}", e));
            }
        }

        let duration = start.elapsed();
        let now = Utc::now();

        *self.last_sync.write() = Some(now);

        Ok(SyncResult {
            patterns_synced,
            tasks_synced,
            duration_ms: duration.as_millis() as u64,
            errors,
            synced_at: now,
        })
    }

    /// Get agent routing suggestion from Claude Flow
    pub fn get_routing_suggestion(&self, task: &str) -> Result<Option<String>> {
        let args = vec![
            "hooks".to_string(),
            "route".to_string(),
            "--task".to_string(),
            task.to_string(),
        ];

        let output = self.execute_cli(&args)?;

        if output.trim().is_empty() {
            return Ok(None);
        }

        // Parse routing suggestion from output
        // Expected format: "Recommended agent: coder (confidence: 0.85)"
        if let Some(line) = output.lines().find(|l| l.contains("Recommended agent")) {
            return Ok(Some(line.to_string()));
        }

        Ok(Some(output.trim().to_string()))
    }

    /// Record task outcome for learning
    pub fn record_outcome(&self, task_id: &str, success: bool, quality: Option<f32>) -> Result<()> {
        let mut args = vec![
            "hooks".to_string(),
            "post-task".to_string(),
            "--task-id".to_string(),
            task_id.to_string(),
            "--success".to_string(),
            success.to_string(),
        ];

        if let Some(q) = quality {
            args.push("--quality".to_string());
            args.push(q.to_string());
        }

        self.execute_cli(&args)?;
        Ok(())
    }

    /// Get bridge statistics
    pub fn stats(&self) -> BridgeStats {
        BridgeStats {
            stores: self.stats.stores.load(Ordering::SeqCst),
            searches: self.stats.searches.load(Ordering::SeqCst),
            successes: self.stats.successes.load(Ordering::SeqCst),
            failures: self.stats.failures.load(Ordering::SeqCst),
            cache_hits: self.stats.cache_hits.load(Ordering::SeqCst),
            syncs: self.stats.syncs.load(Ordering::SeqCst),
        }
    }

    /// Get last sync timestamp
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        *self.last_sync.read()
    }

    /// Clear search cache
    pub fn clear_cache(&self) {
        self.search_cache.write().clear();
    }

    /// Invalidate cache for namespace
    fn invalidate_cache(&self, namespace: &str) {
        let mut cache = self.search_cache.write();
        cache.retain(|k, _| !k.starts_with(&format!("{}:", namespace)));
    }

    /// Validate CLI argument to prevent command injection
    fn validate_cli_arg(arg: &str) -> Result<&str> {
        // Reject shell metacharacters
        const FORBIDDEN: &[char] = &[
            '$', ';', '|', '&', '`', '\n', '\r', '\\', '"', '\'', '<', '>', '(', ')', '{', '}',
            '[', ']', '*', '?', '!', '#',
        ];
        if arg.chars().any(|c| FORBIDDEN.contains(&c)) {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Invalid character in CLI argument: {}",
                arg
            )));
        }
        // Reject if starts with dash followed by dash (--) to prevent option injection
        if arg.starts_with("--")
            && arg.len() > 2
            && !arg[2..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false)
        {
            return Err(RuvLLMError::InvalidOperation(
                "Invalid CLI argument format".to_string(),
            ));
        }
        Ok(arg)
    }

    /// Execute CLI command
    fn execute_cli(&self, args: &[String]) -> Result<String> {
        let cli_parts: Vec<&str> = self.config.cli_command.split_whitespace().collect();

        if cli_parts.is_empty() {
            self.stats.failures.fetch_add(1, Ordering::SeqCst);
            return Err(RuvLLMError::Config("Empty CLI command".to_string()));
        }

        // Validate all provided arguments before execution
        for arg in args {
            Self::validate_cli_arg(arg).map_err(|e| {
                self.stats.failures.fetch_add(1, Ordering::SeqCst);
                e
            })?;
        }

        let program = cli_parts[0];
        let mut cmd = Command::new(program);

        // Add base command args (these are from config, assumed trusted)
        for part in &cli_parts[1..] {
            cmd.arg(part);
        }

        // Add provided args (already validated above)
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output().map_err(|e| {
            self.stats.failures.fetch_add(1, Ordering::SeqCst);
            RuvLLMError::Io(e)
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            self.stats.failures.fetch_add(1, Ordering::SeqCst);
            return Err(RuvLLMError::InvalidOperation(format!(
                "CLI command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Parse search results from CLI output
    fn parse_search_results(
        &self,
        output: &str,
        namespace: &str,
    ) -> Result<Vec<ClaudeFlowPattern>> {
        let mut patterns = Vec::new();

        // Try to parse as JSON first
        if let Ok(json_results) = serde_json::from_str::<Vec<serde_json::Value>>(output) {
            for item in json_results {
                let pattern = ClaudeFlowPattern {
                    key: item
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    value: item
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    namespace: namespace.to_string(),
                    tags: item
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default(),
                    metadata: HashMap::new(),
                    created_at: Utc::now(),
                };
                patterns.push(pattern);
            }
        } else {
            // Fall back to line-based parsing
            for line in output.lines() {
                if line.trim().is_empty() {
                    continue;
                }

                // Try key: value format
                if let Some(pos) = line.find(':') {
                    let key = line[..pos].trim().to_string();
                    let value = line[pos + 1..].trim().to_string();

                    patterns.push(ClaudeFlowPattern {
                        key,
                        value,
                        namespace: namespace.to_string(),
                        tags: vec![],
                        metadata: HashMap::new(),
                        created_at: Utc::now(),
                    });
                }
            }
        }

        Ok(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let config = ClaudeFlowBridgeConfig::default();
        let bridge = ClaudeFlowMemoryBridge::new(config);
        assert_eq!(bridge.stats().stores, 0);
    }

    #[test]
    fn test_config_defaults() {
        let config = ClaudeFlowBridgeConfig::default();
        assert_eq!(config.patterns_namespace, "patterns");
        assert!(config.enable_cache);
        assert!(config.enable_hive_sync);
    }

    #[test]
    fn test_cache_invalidation() {
        let config = ClaudeFlowBridgeConfig::default();
        let bridge = ClaudeFlowMemoryBridge::new(config);

        // Add some cache entries manually
        {
            let mut cache = bridge.search_cache.write();
            cache.insert(
                "patterns:test:10".to_string(),
                CachedSearch {
                    results: vec![],
                    cached_at: Utc::now(),
                },
            );
            cache.insert(
                "tasks:test:10".to_string(),
                CachedSearch {
                    results: vec![],
                    cached_at: Utc::now(),
                },
            );
        }

        assert_eq!(bridge.search_cache.read().len(), 2);

        bridge.invalidate_cache("patterns");

        assert_eq!(bridge.search_cache.read().len(), 1);
        assert!(bridge.search_cache.read().contains_key("tasks:test:10"));
    }

    #[test]
    fn test_clear_cache() {
        let config = ClaudeFlowBridgeConfig::default();
        let bridge = ClaudeFlowMemoryBridge::new(config);

        {
            let mut cache = bridge.search_cache.write();
            cache.insert(
                "test:key".to_string(),
                CachedSearch {
                    results: vec![],
                    cached_at: Utc::now(),
                },
            );
        }

        assert_eq!(bridge.search_cache.read().len(), 1);
        bridge.clear_cache();
        assert_eq!(bridge.search_cache.read().len(), 0);
    }

    #[test]
    fn test_parse_search_results_json() {
        let config = ClaudeFlowBridgeConfig::default();
        let bridge = ClaudeFlowMemoryBridge::new(config);

        let json_output = r#"[
            {"key": "pattern-1", "value": "value-1", "tags": ["rust"]},
            {"key": "pattern-2", "value": "value-2", "tags": []}
        ]"#;

        let results = bridge
            .parse_search_results(json_output, "patterns")
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].key, "pattern-1");
        assert_eq!(results[0].tags, vec!["rust"]);
    }

    #[test]
    fn test_parse_search_results_text() {
        let config = ClaudeFlowBridgeConfig::default();
        let bridge = ClaudeFlowMemoryBridge::new(config);

        let text_output = "key1: value1\nkey2: value2\n";

        let results = bridge
            .parse_search_results(text_output, "patterns")
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].key, "key1");
        assert_eq!(results[0].value, "value1");
    }

    #[test]
    fn test_sync_result_creation() {
        let result = SyncResult {
            patterns_synced: 10,
            tasks_synced: 5,
            duration_ms: 100,
            errors: vec![],
            synced_at: Utc::now(),
        };

        assert_eq!(result.patterns_synced, 10);
        assert!(result.errors.is_empty());
    }
}
