//! Q-Learning Module for iOS WASM
//!
//! Lightweight reinforcement learning for adaptive recommendations.
//! Uses tabular Q-learning with function approximation for state generalization.

/// Maximum number of actions (content recommendations)
const MAX_ACTIONS: usize = 100;

/// State discretization buckets
const STATE_BUCKETS: usize = 16;

/// User interaction types
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum InteractionType {
    /// User viewed content
    View = 0,
    /// User liked/saved content
    Like = 1,
    /// User shared content
    Share = 2,
    /// User skipped content
    Skip = 3,
    /// User completed content (video/audio)
    Complete = 4,
    /// User dismissed/hid content
    Dismiss = 5,
}

impl InteractionType {
    /// Convert interaction to reward signal
    #[inline]
    pub fn to_reward(self) -> f32 {
        match self {
            InteractionType::View => 0.1,
            InteractionType::Like => 0.8,
            InteractionType::Share => 1.0,
            InteractionType::Skip => -0.1,
            InteractionType::Complete => 0.6,
            InteractionType::Dismiss => -0.5,
        }
    }
}

/// User interaction event
#[derive(Clone, Debug)]
pub struct UserInteraction {
    /// Content ID that was interacted with
    pub content_id: u64,
    /// Type of interaction
    pub interaction: InteractionType,
    /// Time spent in seconds
    pub time_spent: f32,
    /// Position in recommendation list (0-indexed)
    pub position: u8,
}

/// Q-Learning agent for personalized recommendations
pub struct QLearner {
    /// Q-values: state_bucket x action -> value
    q_table: Vec<f32>,
    /// Learning rate (alpha)
    learning_rate: f32,
    /// Discount factor (gamma)
    discount: f32,
    /// Exploration rate (epsilon)
    exploration: f32,
    /// Number of state buckets
    state_dim: usize,
    /// Number of actions
    action_dim: usize,
    /// Visit counts for UCB exploration
    visit_counts: Vec<u32>,
    /// Total updates
    total_updates: u64,
}

impl QLearner {
    /// Create a new Q-learner
    pub fn new(action_dim: usize) -> Self {
        let action_dim = action_dim.min(MAX_ACTIONS);
        let state_dim = STATE_BUCKETS;
        let table_size = state_dim * action_dim;

        Self {
            q_table: vec![0.0; table_size],
            learning_rate: 0.1,
            discount: 0.95,
            exploration: 0.1,
            state_dim,
            action_dim,
            visit_counts: vec![0; table_size],
            total_updates: 0,
        }
    }

    /// Create with custom hyperparameters
    pub fn with_params(
        action_dim: usize,
        learning_rate: f32,
        discount: f32,
        exploration: f32,
    ) -> Self {
        let mut learner = Self::new(action_dim);
        learner.learning_rate = learning_rate.clamp(0.001, 1.0);
        learner.discount = discount.clamp(0.0, 1.0);
        learner.exploration = exploration.clamp(0.0, 1.0);
        learner
    }

    /// Discretize state embedding to bucket index
    #[inline]
    fn discretize_state(&self, state_embedding: &[f32]) -> usize {
        if state_embedding.is_empty() {
            return 0;
        }

        // Use first few dimensions to compute hash
        let mut hash: u32 = 0;
        for (i, &val) in state_embedding.iter().take(8).enumerate() {
            let quantized = ((val + 1.0) * 127.0) as u32;
            hash = hash.wrapping_add(quantized << (i * 4));
        }

        (hash as usize) % self.state_dim
    }

    /// Get Q-value for state-action pair
    #[inline]
    fn get_q(&self, state: usize, action: usize) -> f32 {
        let idx = state * self.action_dim + action;
        if idx < self.q_table.len() {
            self.q_table[idx]
        } else {
            0.0
        }
    }

    /// Set Q-value for state-action pair
    #[inline]
    fn set_q(&mut self, state: usize, action: usize, value: f32) {
        let idx = state * self.action_dim + action;
        if idx < self.q_table.len() {
            self.q_table[idx] = value;
            self.visit_counts[idx] += 1;
        }
    }

    /// Select action using epsilon-greedy with UCB exploration bonus
    pub fn select_action(&self, state_embedding: &[f32], rng_seed: u32) -> usize {
        let state = self.discretize_state(state_embedding);

        // Epsilon-greedy exploration
        let explore_threshold = (rng_seed % 1000) as f32 / 1000.0;
        if explore_threshold < self.exploration {
            // Random action
            return (rng_seed as usize) % self.action_dim;
        }

        // Greedy action with UCB bonus
        let mut best_action = 0;
        let mut best_value = f32::NEG_INFINITY;
        let total_visits = self.total_updates.max(1) as f32;

        for action in 0..self.action_dim {
            let q_val = self.get_q(state, action);
            let visits = self.visit_counts[state * self.action_dim + action].max(1) as f32;

            // UCB exploration bonus
            let ucb_bonus = (2.0 * total_visits.ln() / visits).sqrt() * 0.5;
            let value = q_val + ucb_bonus;

            if value > best_value {
                best_value = value;
                best_action = action;
            }
        }

        best_action
    }

    /// Update Q-value based on interaction
    pub fn update(
        &mut self,
        state_embedding: &[f32],
        action: usize,
        interaction: &UserInteraction,
        next_state_embedding: &[f32],
    ) {
        let state = self.discretize_state(state_embedding);
        let next_state = self.discretize_state(next_state_embedding);

        // Compute reward
        let base_reward = interaction.interaction.to_reward();
        let time_bonus = (interaction.time_spent / 60.0).min(1.0) * 0.2;
        let position_bonus = (1.0 - interaction.position as f32 / 10.0).max(0.0) * 0.1;
        let reward = base_reward + time_bonus + position_bonus;

        // Find max Q-value for next state
        let mut max_next_q = f32::NEG_INFINITY;
        for a in 0..self.action_dim {
            let q = self.get_q(next_state, a);
            if q > max_next_q {
                max_next_q = q;
            }
        }
        if max_next_q == f32::NEG_INFINITY {
            max_next_q = 0.0;
        }

        // Q-learning update
        let current_q = self.get_q(state, action);
        let td_target = reward + self.discount * max_next_q;
        let new_q = current_q + self.learning_rate * (td_target - current_q);

        self.set_q(state, action, new_q);
        self.total_updates += 1;

        // Decay exploration over time
        if self.total_updates % 100 == 0 {
            self.exploration = (self.exploration * 0.99).max(0.01);
        }
    }

    /// Get action rankings for a state (returns sorted action indices)
    pub fn rank_actions(&self, state_embedding: &[f32]) -> Vec<usize> {
        let state = self.discretize_state(state_embedding);

        let mut action_values: Vec<(usize, f32)> = (0..self.action_dim)
            .map(|a| (a, self.get_q(state, a)))
            .collect();

        action_values.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));

        action_values.into_iter().map(|(a, _)| a).collect()
    }

    /// Serialize Q-table to bytes for persistence
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.q_table.len() * 4 + 32);

        // Header
        bytes.extend_from_slice(&(self.state_dim as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.action_dim as u32).to_le_bytes());
        bytes.extend_from_slice(&self.learning_rate.to_le_bytes());
        bytes.extend_from_slice(&self.discount.to_le_bytes());
        bytes.extend_from_slice(&self.exploration.to_le_bytes());
        bytes.extend_from_slice(&self.total_updates.to_le_bytes());

        // Q-table
        for &q in &self.q_table {
            bytes.extend_from_slice(&q.to_le_bytes());
        }

        bytes
    }

    /// Deserialize Q-table from bytes
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        // Header: 4+4+4+4+4+8 = 28 bytes
        const HEADER_SIZE: usize = 28;
        if bytes.len() < HEADER_SIZE {
            return None;
        }

        let state_dim = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let action_dim = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let learning_rate = f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let discount = f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let exploration = f32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let total_updates = u64::from_le_bytes([
            bytes[20], bytes[21], bytes[22], bytes[23],
            bytes[24], bytes[25], bytes[26], bytes[27],
        ]);

        let table_size = state_dim * action_dim;
        let expected_len = HEADER_SIZE + table_size * 4;

        if bytes.len() < expected_len {
            return None;
        }

        let mut q_table = Vec::with_capacity(table_size);
        for i in 0..table_size {
            let offset = HEADER_SIZE + i * 4;
            let q = f32::from_le_bytes([
                bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3],
            ]);
            q_table.push(q);
        }

        Some(Self {
            q_table,
            learning_rate,
            discount,
            exploration,
            state_dim,
            action_dim,
            visit_counts: vec![0; table_size],
            total_updates,
        })
    }

    /// Get current exploration rate
    pub fn exploration_rate(&self) -> f32 {
        self.exploration
    }

    /// Get total number of updates
    pub fn update_count(&self) -> u64 {
        self.total_updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qlearner_creation() {
        let learner = QLearner::new(50);
        assert_eq!(learner.action_dim, 50);
    }

    #[test]
    fn test_action_selection() {
        let learner = QLearner::new(10);
        let state = vec![0.5; 64];
        let action = learner.select_action(&state, 42);
        assert!(action < 10);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut learner = QLearner::with_params(10, 0.1, 0.9, 0.2);

        // Do some updates
        let state = vec![0.5; 64];
        let interaction = UserInteraction {
            content_id: 1,
            interaction: InteractionType::Like,
            time_spent: 30.0,
            position: 0,
        };
        learner.update(&state, 0, &interaction, &state);

        // Serialize and deserialize
        let bytes = learner.serialize();
        let restored = QLearner::deserialize(&bytes).unwrap();

        assert_eq!(restored.action_dim, learner.action_dim);
        assert_eq!(restored.total_updates, learner.total_updates);
    }
}
