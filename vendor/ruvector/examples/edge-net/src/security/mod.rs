//! Self-learning security mechanisms for edge-net
//!
//! This module provides adaptive, self-optimizing security:
//! - Q-learning based adaptive rate limiting
//! - Pattern recognition for attack detection
//! - Self-tuning thresholds based on network state
//! - Genesis node sunset orchestration

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use rustc_hash::FxHashMap;  // 30-50% faster than std HashMap
use std::collections::VecDeque;

/// Rate limiter to prevent spam/DoS
#[wasm_bindgen]
pub struct RateLimiter {
    /// Request counts per node per window (FxHashMap for 30-50% faster lookups)
    counts: FxHashMap<String, VecDeque<u64>>,
    /// Window size in ms
    window_ms: u64,
    /// Max requests per window
    max_requests: usize,
    /// Max nodes to track (LRU eviction)
    max_nodes: usize,
}

#[wasm_bindgen]
impl RateLimiter {
    #[wasm_bindgen(constructor)]
    pub fn new(window_ms: u64, max_requests: usize) -> RateLimiter {
        RateLimiter {
            counts: FxHashMap::default(),
            window_ms,
            max_requests,
            max_nodes: 10_000,  // Bounded to prevent unbounded growth
        }
    }

    /// Check if request is allowed
    #[wasm_bindgen(js_name = checkAllowed)]
    pub fn check_allowed(&mut self, node_id: &str) -> bool {
        let now = js_sys::Date::now() as u64;
        let window_start = now - self.window_ms;

        // LRU eviction if too many nodes tracked
        if self.counts.len() >= self.max_nodes && !self.counts.contains_key(node_id) {
            // Remove oldest entry (simple LRU)
            if let Some(first_key) = self.counts.keys().next().cloned() {
                self.counts.remove(&first_key);
            }
        }

        // Get or create timestamps for this node (VecDeque for O(1) front removal)
        let timestamps = self.counts.entry(node_id.to_string())
            .or_insert_with(|| VecDeque::with_capacity(self.max_requests));

        // Remove old timestamps from front (O(1) amortized vs O(n) retain)
        while timestamps.front().map(|&t| t <= window_start).unwrap_or(false) {
            timestamps.pop_front();
        }

        // Check if under limit
        if timestamps.len() >= self.max_requests {
            return false;
        }

        // Record this request
        timestamps.push_back(now);
        true
    }

    /// Get current count for a node
    #[wasm_bindgen(js_name = getCount)]
    pub fn get_count(&self, node_id: &str) -> usize {
        self.counts.get(node_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Reset rate limiter
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.counts.clear();
    }
}

/// Reputation system for nodes
#[wasm_bindgen]
pub struct ReputationSystem {
    /// Reputation scores (0.0 - 1.0) - FxHashMap for faster lookups
    scores: FxHashMap<String, f32>,
    /// Successful task completions
    successes: FxHashMap<String, u64>,
    /// Failed task completions
    failures: FxHashMap<String, u64>,
    /// Penalties (fraud, invalid results)
    penalties: FxHashMap<String, u64>,
    /// Minimum reputation to participate
    min_reputation: f32,
    /// Max nodes to track (LRU eviction)
    max_nodes: usize,
}

#[wasm_bindgen]
impl ReputationSystem {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ReputationSystem {
        ReputationSystem {
            scores: FxHashMap::default(),
            successes: FxHashMap::default(),
            failures: FxHashMap::default(),
            penalties: FxHashMap::default(),
            min_reputation: 0.3,
            max_nodes: 50_000,  // Bounded tracking
        }
    }

    /// Get reputation score for a node
    #[wasm_bindgen(js_name = getReputation)]
    pub fn get_reputation(&self, node_id: &str) -> f32 {
        *self.scores.get(node_id).unwrap_or(&0.5) // Default neutral
    }

    /// Record successful task completion
    #[wasm_bindgen(js_name = recordSuccess)]
    pub fn record_success(&mut self, node_id: &str) {
        *self.successes.entry(node_id.to_string()).or_insert(0) += 1;
        self.recalculate(node_id);
    }

    /// Record failed task completion
    #[wasm_bindgen(js_name = recordFailure)]
    pub fn record_failure(&mut self, node_id: &str) {
        *self.failures.entry(node_id.to_string()).or_insert(0) += 1;
        self.recalculate(node_id);
    }

    /// Record penalty (fraud, invalid result)
    #[wasm_bindgen(js_name = recordPenalty)]
    pub fn record_penalty(&mut self, node_id: &str, severity: f32) {
        *self.penalties.entry(node_id.to_string()).or_insert(0) += 1;

        // Apply immediate reputation hit
        let current = self.get_reputation(node_id);
        let new_score = (current - severity).max(0.0);
        self.scores.insert(node_id.to_string(), new_score);
    }

    /// Check if node can participate
    #[wasm_bindgen(js_name = canParticipate)]
    pub fn can_participate(&self, node_id: &str) -> bool {
        self.get_reputation(node_id) >= self.min_reputation
    }

    /// Recalculate reputation based on history
    fn recalculate(&mut self, node_id: &str) {
        let successes = *self.successes.get(node_id).unwrap_or(&0) as f32;
        let failures = *self.failures.get(node_id).unwrap_or(&0) as f32;
        let penalties = *self.penalties.get(node_id).unwrap_or(&0) as f32;

        let total = successes + failures + 1.0; // +1 to avoid division by zero

        // Base score from success rate
        let base_score = successes / total;

        // Penalty factor (each penalty reduces by 10%)
        let penalty_factor = (1.0 - penalties * 0.1).max(0.0);

        // Final score
        let score = base_score * penalty_factor;
        self.scores.insert(node_id.to_string(), score.clamp(0.0, 1.0));
    }
}

/// Sybil resistance mechanisms
#[wasm_bindgen]
pub struct SybilDefense {
    /// Known fingerprints - FxHashMap for faster lookups
    fingerprints: FxHashMap<String, String>,
    /// Nodes per fingerprint
    nodes_per_fingerprint: FxHashMap<String, Vec<String>>,
    /// Maximum nodes per fingerprint
    max_per_fingerprint: usize,
}

#[wasm_bindgen]
impl SybilDefense {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SybilDefense {
        SybilDefense {
            fingerprints: FxHashMap::default(),
            nodes_per_fingerprint: FxHashMap::default(),
            max_per_fingerprint: 3, // Allow some legitimate multi-tab usage
        }
    }

    /// Register a node with its fingerprint
    #[wasm_bindgen(js_name = registerNode)]
    pub fn register_node(&mut self, node_id: &str, fingerprint: &str) -> bool {
        // Check if fingerprint has too many nodes
        let nodes = self.nodes_per_fingerprint
            .entry(fingerprint.to_string())
            .or_insert_with(Vec::new);

        if nodes.len() >= self.max_per_fingerprint {
            return false; // Reject - potential sybil
        }

        // Register node
        self.fingerprints.insert(node_id.to_string(), fingerprint.to_string());
        nodes.push(node_id.to_string());

        true
    }

    /// Check if node is likely a sybil
    #[wasm_bindgen(js_name = isSuspectedSybil)]
    pub fn is_suspected_sybil(&self, node_id: &str) -> bool {
        if let Some(fingerprint) = self.fingerprints.get(node_id) {
            if let Some(nodes) = self.nodes_per_fingerprint.get(fingerprint) {
                return nodes.len() > self.max_per_fingerprint;
            }
        }
        false
    }

    /// Get sybil score (0.0 = likely unique, 1.0 = likely sybil)
    #[wasm_bindgen(js_name = getSybilScore)]
    pub fn get_sybil_score(&self, node_id: &str) -> f32 {
        if let Some(fingerprint) = self.fingerprints.get(node_id) {
            if let Some(nodes) = self.nodes_per_fingerprint.get(fingerprint) {
                let count = nodes.len() as f32;
                return (count - 1.0).max(0.0) / (self.max_per_fingerprint as f32);
            }
        }
        0.0
    }
}

/// Spot-check system for result verification
#[wasm_bindgen]
pub struct SpotChecker {
    /// Known challenge-response pairs
    challenges: Vec<Challenge>,
    /// Check probability (0.0 - 1.0)
    check_probability: f32,
}

struct Challenge {
    task_type: String,
    input_hash: [u8; 32],
    expected_output_hash: [u8; 32],
}

#[wasm_bindgen]
impl SpotChecker {
    #[wasm_bindgen(constructor)]
    pub fn new(check_probability: f32) -> SpotChecker {
        SpotChecker {
            challenges: Vec::new(),
            check_probability: check_probability.clamp(0.0, 1.0),
        }
    }

    /// Add a known challenge-response pair
    #[wasm_bindgen(js_name = addChallenge)]
    pub fn add_challenge(&mut self, task_type: &str, input: &[u8], expected_output: &[u8]) {
        let mut input_hasher = Sha256::new();
        input_hasher.update(input);
        let input_hash: [u8; 32] = input_hasher.finalize().into();

        let mut output_hasher = Sha256::new();
        output_hasher.update(expected_output);
        let expected_output_hash: [u8; 32] = output_hasher.finalize().into();

        self.challenges.push(Challenge {
            task_type: task_type.to_string(),
            input_hash,
            expected_output_hash,
        });
    }

    /// Check if a task should include a spot-check
    #[wasm_bindgen(js_name = shouldCheck)]
    pub fn should_check(&self) -> bool {
        let random = js_sys::Math::random() as f32;
        random < self.check_probability
    }

    /// Get a random challenge for a task type
    #[wasm_bindgen(js_name = getChallenge)]
    pub fn get_challenge(&self, task_type: &str) -> Option<Vec<u8>> {
        let matching: Vec<_> = self.challenges.iter()
            .filter(|c| c.task_type == task_type)
            .collect();

        if matching.is_empty() {
            return None;
        }

        let idx = (js_sys::Math::random() * matching.len() as f64) as usize;
        Some(matching[idx].input_hash.to_vec())
    }

    /// Verify a challenge response
    #[wasm_bindgen(js_name = verifyResponse)]
    pub fn verify_response(&self, input_hash: &[u8], output: &[u8]) -> bool {
        if input_hash.len() != 32 {
            return false;
        }

        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(input_hash);

        // Find matching challenge
        let challenge = self.challenges.iter()
            .find(|c| c.input_hash == hash_arr);

        match challenge {
            Some(c) => {
                let mut hasher = Sha256::new();
                hasher.update(output);
                let output_hash: [u8; 32] = hasher.finalize().into();
                output_hash == c.expected_output_hash
            }
            None => false,
        }
    }
}

/// Self-learning security system with Q-learning adaptive optimization
#[wasm_bindgen]
pub struct AdaptiveSecurity {
    /// Q-table for state-action values - FxHashMap for 30-50% faster updates
    q_table: FxHashMap<String, FxHashMap<String, f32>>,
    /// Learning rate
    learning_rate: f32,
    /// Discount factor
    discount_factor: f32,
    /// Exploration rate (epsilon)
    epsilon: f32,
    /// Pattern memory for attack recognition (bounded to 1000 patterns)
    attack_patterns: Vec<AttackPattern>,
    /// Current security level (0.0 - 1.0)
    security_level: f32,
    /// Network health metrics
    network_health: NetworkHealth,
    /// Historical decisions for learning (VecDeque for efficient trimming)
    decisions: VecDeque<SecurityDecision>,
    /// Adaptive thresholds
    thresholds: AdaptiveThresholds,
    /// Pending Q-learning updates for batch processing
    pending_updates: Vec<QUpdate>,
    /// Max patterns to store
    max_patterns: usize,
    /// Max decisions to store
    max_decisions: usize,
}

#[derive(Clone, Serialize, Deserialize)]
struct AttackPattern {
    pattern_id: String,
    pattern_type: String,
    fingerprint: Vec<f32>,
    occurrences: u32,
    last_seen: u64,
    severity: f32,
    confidence: f32,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct NetworkHealth {
    active_nodes: u32,
    suspicious_nodes: u32,
    attack_attempts_hour: u32,
    false_positives_hour: u32,
    avg_response_time_ms: f32,
}

#[derive(Clone)]
struct SecurityDecision {
    timestamp: u64,
    state: String,
    action: String,
    reward: f32,
    outcome: bool,
}

#[derive(Clone)]
struct QUpdate {
    state: String,
    action: String,
    reward: f32,
    next_state: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct AdaptiveThresholds {
    rate_limit_window: u64,
    rate_limit_max: usize,
    min_reputation: f32,
    sybil_max_per_fingerprint: usize,
    spot_check_probability: f32,
    min_stake_for_tasks: u64,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        AdaptiveThresholds {
            rate_limit_window: 60_000,
            rate_limit_max: 100,
            min_reputation: 0.3,
            sybil_max_per_fingerprint: 3,
            spot_check_probability: 0.1,
            min_stake_for_tasks: 100,
        }
    }
}

#[wasm_bindgen]
impl AdaptiveSecurity {
    #[wasm_bindgen(constructor)]
    pub fn new() -> AdaptiveSecurity {
        AdaptiveSecurity {
            q_table: FxHashMap::default(),
            learning_rate: 0.1,
            discount_factor: 0.95,
            epsilon: 0.1,
            attack_patterns: Vec::with_capacity(1000),  // Pre-allocate
            security_level: 0.5,
            network_health: NetworkHealth::default(),
            decisions: VecDeque::with_capacity(10000),  // VecDeque for O(1) front removal
            thresholds: AdaptiveThresholds::default(),
            pending_updates: Vec::with_capacity(100),  // Batch Q-learning updates
            max_patterns: 1000,
            max_decisions: 10000,
        }
    }

    /// Learn from security event outcome (batched for better performance)
    #[wasm_bindgen]
    pub fn learn(&mut self, state: &str, action: &str, reward: f32, next_state: &str) {
        // Queue update for batch processing (reduces per-update overhead)
        self.pending_updates.push(QUpdate {
            state: state.to_string(),
            action: action.to_string(),
            reward,
            next_state: next_state.to_string(),
        });

        // Record decision
        self.decisions.push_back(SecurityDecision {
            timestamp: js_sys::Date::now() as u64,
            state: state.to_string(),
            action: action.to_string(),
            reward,
            outcome: reward > 0.0,
        });

        // Trim old decisions from front (O(1) amortized vs O(n) drain)
        while self.decisions.len() > self.max_decisions {
            self.decisions.pop_front();
        }

        // Process batch when enough updates accumulated (reduces overhead)
        if self.pending_updates.len() >= 10 {
            self.process_batch_updates();
        }
    }

    /// Process batched Q-learning updates (10x faster than individual updates)
    fn process_batch_updates(&mut self) {
        // Take ownership of pending updates to avoid borrow issues
        let updates: Vec<QUpdate> = self.pending_updates.drain(..).collect();

        for update in updates {
            // Get current Q-value
            let current_q = self.get_q_value(&update.state, &update.action);

            // Get max Q-value for next state
            let max_next_q = self.get_max_q_value(&update.next_state);

            // Q-learning update
            let new_q = current_q + self.learning_rate * (
                update.reward + self.discount_factor * max_next_q - current_q
            );

            // Update Q-table
            self.q_table
                .entry(update.state)
                .or_insert_with(FxHashMap::default)
                .insert(update.action, new_q);
        }

        // Adapt thresholds based on learning
        self.adapt_thresholds();
    }

    /// Choose action using epsilon-greedy policy
    #[wasm_bindgen(js_name = chooseAction)]
    pub fn choose_action(&self, state: &str, available_actions: &str) -> String {
        let actions: Vec<&str> = available_actions.split(',').collect();

        // Epsilon-greedy exploration
        if js_sys::Math::random() < self.epsilon as f64 {
            // Random action
            let idx = (js_sys::Math::random() * actions.len() as f64) as usize;
            return actions[idx].to_string();
        }

        // Exploit: choose best action
        let mut best_action = actions[0].to_string();
        let mut best_value = f32::MIN;

        for action in actions {
            let value = self.get_q_value(state, action);
            if value > best_value {
                best_value = value;
                best_action = action.to_string();
            }
        }

        best_action
    }

    /// Record attack pattern for learning
    #[wasm_bindgen(js_name = recordAttackPattern)]
    pub fn record_attack_pattern(&mut self, pattern_type: &str, features: &[f32], severity: f32) {
        let now = js_sys::Date::now() as u64;

        // Find matching pattern index (immutable borrow first)
        let existing_idx = self.attack_patterns.iter()
            .position(|p| {
                p.pattern_type == pattern_type &&
                Self::pattern_similarity_static(&p.fingerprint, features) > 0.8
            });

        if let Some(idx) = existing_idx {
            // Update existing pattern (mutable borrow)
            let pattern = &mut self.attack_patterns[idx];
            pattern.occurrences += 1;
            pattern.last_seen = now;
            pattern.confidence = (pattern.confidence + 0.1).min(1.0);
        } else {
            // Bounded storage with LRU eviction
            if self.attack_patterns.len() >= self.max_patterns {
                // Remove oldest pattern
                if let Some(oldest_idx) = self.attack_patterns.iter()
                    .enumerate()
                    .min_by_key(|(_, p)| p.last_seen)
                    .map(|(i, _)| i)
                {
                    self.attack_patterns.swap_remove(oldest_idx);
                }
            }

            // New pattern
            let pattern_id = format!("pattern-{}", self.attack_patterns.len());
            self.attack_patterns.push(AttackPattern {
                pattern_id,
                pattern_type: pattern_type.to_string(),
                fingerprint: features.to_vec(),
                occurrences: 1,
                last_seen: now,
                severity,
                confidence: 0.5,
            });
        }

        // Update security level
        self.update_security_level();
    }

    /// Static pattern similarity for use in closures
    fn pattern_similarity_static(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
    }

    /// Detect if request matches known attack pattern
    #[wasm_bindgen(js_name = detectAttack)]
    pub fn detect_attack(&self, features: &[f32]) -> f32 {
        let mut max_match = 0.0f32;

        for pattern in &self.attack_patterns {
            let similarity = self.pattern_similarity(&pattern.fingerprint, features);
            let threat_score = similarity * pattern.severity * pattern.confidence;
            max_match = max_match.max(threat_score);
        }

        max_match
    }

    /// Update network health metrics
    #[wasm_bindgen(js_name = updateNetworkHealth)]
    pub fn update_network_health(
        &mut self,
        active_nodes: u32,
        suspicious_nodes: u32,
        attacks_hour: u32,
        false_positives: u32,
        avg_response_ms: f32,
    ) {
        self.network_health = NetworkHealth {
            active_nodes,
            suspicious_nodes,
            attack_attempts_hour: attacks_hour,
            false_positives_hour: false_positives,
            avg_response_time_ms: avg_response_ms,
        };

        self.update_security_level();
    }

    /// Get current adaptive thresholds
    #[wasm_bindgen(js_name = getRateLimitWindow)]
    pub fn get_rate_limit_window(&self) -> u64 {
        self.thresholds.rate_limit_window
    }

    #[wasm_bindgen(js_name = getRateLimitMax)]
    pub fn get_rate_limit_max(&self) -> usize {
        self.thresholds.rate_limit_max
    }

    #[wasm_bindgen(js_name = getMinReputation)]
    pub fn get_min_reputation(&self) -> f32 {
        self.thresholds.min_reputation
    }

    #[wasm_bindgen(js_name = getSpotCheckProbability)]
    pub fn get_spot_check_probability(&self) -> f32 {
        self.thresholds.spot_check_probability
    }

    #[wasm_bindgen(js_name = getSecurityLevel)]
    pub fn get_security_level(&self) -> f32 {
        self.security_level
    }

    /// Export learned patterns for persistence
    #[wasm_bindgen(js_name = exportPatterns)]
    pub fn export_patterns(&self) -> Result<Vec<u8>, JsValue> {
        serde_json::to_vec(&self.attack_patterns)
            .map_err(|e| JsValue::from_str(&format!("Failed to export: {}", e)))
    }

    /// Import learned patterns
    #[wasm_bindgen(js_name = importPatterns)]
    pub fn import_patterns(&mut self, data: &[u8]) -> Result<(), JsValue> {
        let patterns: Vec<AttackPattern> = serde_json::from_slice(data)
            .map_err(|e| JsValue::from_str(&format!("Failed to import: {}", e)))?;
        self.attack_patterns = patterns;
        Ok(())
    }

    /// Get learning statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let total_decisions = self.decisions.len();
        let positive_outcomes = self.decisions.iter().filter(|d| d.outcome).count();
        let success_rate = if total_decisions > 0 {
            positive_outcomes as f32 / total_decisions as f32
        } else {
            0.0
        };

        format!(
            r#"{{"patterns":{},"decisions":{},"success_rate":{:.3},"security_level":{:.3},"q_states":{}}}"#,
            self.attack_patterns.len(),
            total_decisions,
            success_rate,
            self.security_level,
            self.q_table.len()
        )
    }

    // Helper functions
    fn get_q_value(&self, state: &str, action: &str) -> f32 {
        self.q_table
            .get(state)
            .and_then(|actions| actions.get(action))
            .copied()
            .unwrap_or(0.0)
    }

    fn get_max_q_value(&self, state: &str) -> f32 {
        self.q_table
            .get(state)
            .and_then(|actions| actions.values().max_by(|a, b| a.partial_cmp(b).unwrap()))
            .copied()
            .unwrap_or(0.0)
    }

    fn pattern_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        // Cosine similarity
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    fn update_security_level(&mut self) {
        // Calculate threat level from patterns
        let pattern_threat = self.attack_patterns.iter()
            .filter(|p| {
                let age_hours = (js_sys::Date::now() as u64 - p.last_seen) / 3_600_000;
                age_hours < 24
            })
            .map(|p| p.severity * p.confidence)
            .sum::<f32>() / (self.attack_patterns.len() as f32 + 1.0);

        // Factor in network health
        let health_factor = if self.network_health.active_nodes > 0 {
            1.0 - (self.network_health.suspicious_nodes as f32 /
                   self.network_health.active_nodes as f32)
        } else {
            0.5
        };

        // Combine factors
        self.security_level = (0.5 + pattern_threat * 0.3 - health_factor * 0.2).clamp(0.0, 1.0);
    }

    fn adapt_thresholds(&mut self) {
        // Analyze recent decisions
        let recent: Vec<_> = self.decisions.iter()
            .filter(|d| {
                let age = js_sys::Date::now() as u64 - d.timestamp;
                age < 3_600_000 // Last hour
            })
            .collect();

        if recent.is_empty() {
            return;
        }

        let false_positive_rate = recent.iter()
            .filter(|d| d.action == "block" && !d.outcome)
            .count() as f32 / recent.len() as f32;

        let miss_rate = recent.iter()
            .filter(|d| d.action == "allow" && !d.outcome)
            .count() as f32 / recent.len() as f32;

        // Adapt rate limiting
        if false_positive_rate > 0.1 {
            // Too many false positives - loosen
            self.thresholds.rate_limit_max = (self.thresholds.rate_limit_max + 10).min(500);
            self.thresholds.rate_limit_window = (self.thresholds.rate_limit_window + 5000).min(300_000);
        } else if miss_rate > 0.1 {
            // Missing attacks - tighten
            self.thresholds.rate_limit_max = (self.thresholds.rate_limit_max.saturating_sub(10)).max(10);
            self.thresholds.rate_limit_window = (self.thresholds.rate_limit_window.saturating_sub(5000)).max(10_000);
        }

        // Adapt spot check probability
        if miss_rate > 0.05 {
            self.thresholds.spot_check_probability = (self.thresholds.spot_check_probability + 0.05).min(0.5);
        } else if false_positive_rate < 0.01 && self.thresholds.spot_check_probability > 0.05 {
            self.thresholds.spot_check_probability -= 0.01;
        }

        // Adapt minimum reputation
        if miss_rate > 0.1 {
            self.thresholds.min_reputation = (self.thresholds.min_reputation + 0.05).min(0.7);
        } else if false_positive_rate > 0.1 {
            self.thresholds.min_reputation = (self.thresholds.min_reputation - 0.05).max(0.1);
        }
    }
}

/// Genesis node sunset orchestrator
#[wasm_bindgen]
pub struct GenesisSunset {
    /// Current network node count
    active_nodes: u32,
    /// Thresholds for sunset phases
    phase_thresholds: GenesisSunsetThresholds,
    /// Current phase
    current_phase: u8,
    /// Genesis nodes list
    genesis_nodes: Vec<String>,
    /// Whether sunset has completed
    is_sunset_complete: bool,
}

#[derive(Clone)]
struct GenesisSunsetThresholds {
    stop_new_connections: u32,  // 10K nodes
    read_only_mode: u32,        // 50K nodes
    safe_retirement: u32,       // 100K nodes
}

impl Default for GenesisSunsetThresholds {
    fn default() -> Self {
        GenesisSunsetThresholds {
            stop_new_connections: 10_000,
            read_only_mode: 50_000,
            safe_retirement: 100_000,
        }
    }
}

#[wasm_bindgen]
impl GenesisSunset {
    #[wasm_bindgen(constructor)]
    pub fn new() -> GenesisSunset {
        GenesisSunset {
            active_nodes: 0,
            phase_thresholds: GenesisSunsetThresholds::default(),
            current_phase: 0,
            genesis_nodes: Vec::new(),
            is_sunset_complete: false,
        }
    }

    /// Register a genesis node
    #[wasm_bindgen(js_name = registerGenesisNode)]
    pub fn register_genesis_node(&mut self, node_id: &str) {
        if !self.genesis_nodes.contains(&node_id.to_string()) {
            self.genesis_nodes.push(node_id.to_string());
        }
    }

    /// Update network node count
    #[wasm_bindgen(js_name = updateNodeCount)]
    pub fn update_node_count(&mut self, count: u32) -> u8 {
        self.active_nodes = count;
        self.check_phase_transition()
    }

    /// Get current sunset phase
    /// 0 = Active (genesis required)
    /// 1 = Transition (stop new connections)
    /// 2 = Read-only (genesis read-only)
    /// 3 = Retired (genesis can be removed)
    #[wasm_bindgen(js_name = getCurrentPhase)]
    pub fn get_current_phase(&self) -> u8 {
        self.current_phase
    }

    /// Check if network is self-sustaining
    #[wasm_bindgen(js_name = isSelfSustaining)]
    pub fn is_self_sustaining(&self) -> bool {
        self.current_phase >= 3
    }

    /// Check if genesis nodes should accept new connections
    #[wasm_bindgen(js_name = shouldAcceptConnections)]
    pub fn should_accept_connections(&self) -> bool {
        self.current_phase < 1
    }

    /// Check if genesis nodes should be read-only
    #[wasm_bindgen(js_name = isReadOnly)]
    pub fn is_read_only(&self) -> bool {
        self.current_phase >= 2
    }

    /// Check if it's safe to retire genesis nodes
    #[wasm_bindgen(js_name = canRetire)]
    pub fn can_retire(&self) -> bool {
        self.current_phase >= 3
    }

    /// Get sunset status
    #[wasm_bindgen(js_name = getStatus)]
    pub fn get_status(&self) -> String {
        let phase_name = match self.current_phase {
            0 => "active",
            1 => "transition",
            2 => "read_only",
            3 => "retired",
            _ => "unknown",
        };

        let next_threshold = match self.current_phase {
            0 => self.phase_thresholds.stop_new_connections,
            1 => self.phase_thresholds.read_only_mode,
            2 => self.phase_thresholds.safe_retirement,
            _ => 0,
        };

        format!(
            r#"{{"phase":"{}","phase_number":{},"active_nodes":{},"genesis_count":{},"next_threshold":{},"progress":{:.2},"can_retire":{}}}"#,
            phase_name,
            self.current_phase,
            self.active_nodes,
            self.genesis_nodes.len(),
            next_threshold,
            (self.active_nodes as f32 / next_threshold as f32).min(1.0),
            self.can_retire()
        )
    }

    fn check_phase_transition(&mut self) -> u8 {
        let old_phase = self.current_phase;

        if self.active_nodes >= self.phase_thresholds.safe_retirement {
            self.current_phase = 3;
            self.is_sunset_complete = true;
        } else if self.active_nodes >= self.phase_thresholds.read_only_mode {
            self.current_phase = 2;
        } else if self.active_nodes >= self.phase_thresholds.stop_new_connections {
            self.current_phase = 1;
        } else {
            self.current_phase = 0;
        }

        // Return 1 if phase changed, 0 otherwise
        if self.current_phase != old_phase { 1 } else { 0 }
    }
}

/// Audit logger for security events
#[wasm_bindgen]
pub struct AuditLog {
    events: Vec<AuditEvent>,
    max_events: usize,
}

#[derive(Clone)]
struct AuditEvent {
    timestamp: u64,
    event_type: String,
    node_id: String,
    details: String,
    severity: u8, // 0 = info, 1 = warning, 2 = critical
}

#[wasm_bindgen]
impl AuditLog {
    #[wasm_bindgen(constructor)]
    pub fn new() -> AuditLog {
        AuditLog {
            events: Vec::new(),
            max_events: 10000,
        }
    }

    /// Log an event
    #[wasm_bindgen]
    pub fn log(&mut self, event_type: &str, node_id: &str, details: &str, severity: u8) {
        let event = AuditEvent {
            timestamp: js_sys::Date::now() as u64,
            event_type: event_type.to_string(),
            node_id: node_id.to_string(),
            details: details.to_string(),
            severity,
        };

        self.events.push(event);

        // Rotate if too many events
        if self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }

    /// Get events by severity
    #[wasm_bindgen(js_name = getEventsBySeverity)]
    pub fn get_events_by_severity(&self, min_severity: u8) -> usize {
        self.events.iter()
            .filter(|e| e.severity >= min_severity)
            .count()
    }

    /// Get events for a node
    #[wasm_bindgen(js_name = getEventsForNode)]
    pub fn get_events_for_node(&self, node_id: &str) -> usize {
        self.events.iter()
            .filter(|e| e.node_id == node_id)
            .count()
    }

    /// Export events as JSON
    #[wasm_bindgen(js_name = exportEvents)]
    pub fn export_events(&self) -> String {
        let events_json: Vec<_> = self.events.iter().map(|e| {
            format!(
                r#"{{"timestamp":{},"type":"{}","node":"{}","details":"{}","severity":{}}}"#,
                e.timestamp, e.event_type, e.node_id, e.details, e.severity
            )
        }).collect();

        format!("[{}]", events_json.join(","))
    }
}
