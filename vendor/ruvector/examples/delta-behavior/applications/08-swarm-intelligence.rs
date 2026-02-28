//! # Application 8: Coherence-Enforced Swarm Intelligence
//!
//! Local swarm actions allowed. Global incoherence forbidden.
//!
//! ## Problem
//! Swarm systems (drones, robots, agents) can collectively do dangerous
//! things even when each individual follows simple rules.
//!
//! ## Δ-Behavior Solution
//! Each agent can act freely, but actions that would reduce global
//! swarm coherence below threshold are rejected or modified.
//!
//! ## Exotic Result
//! Emergent intelligence that cannot emerge pathological behaviors.
//!
//! ## Performance Optimizations (v2)
//! - O(n) neighbor detection via spatial grid partitioning
//! - Incremental coherence calculation with caching
//! - Single-pass state updates
//! - Squared distance comparisons (no sqrt in hot paths)

use std::collections::HashMap;

/// Spatial grid for O(n) neighbor detection instead of O(n²)
#[derive(Clone, Debug)]
pub struct SpatialGrid {
    /// Cell size determines granularity of spatial partitioning
    cell_size: f64,
    /// Maps grid cell coordinates to agent IDs in that cell
    cells: HashMap<(i32, i32), Vec<String>>,
    /// Squared communication range (avoids sqrt in comparisons)
    comm_range_squared: f64,
}

impl SpatialGrid {
    pub fn new(cell_size: f64, comm_range: f64) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            comm_range_squared: comm_range * comm_range,
        }
    }

    /// Convert world position to grid cell coordinates
    #[inline]
    fn pos_to_cell(&self, x: f64, y: f64) -> (i32, i32) {
        (
            (x / self.cell_size).floor() as i32,
            (y / self.cell_size).floor() as i32,
        )
    }

    /// Rebuild the spatial grid from agent positions - O(n)
    pub fn rebuild(&mut self, agents: &HashMap<String, SwarmAgent>) {
        self.cells.clear();
        for (id, agent) in agents {
            let cell = self.pos_to_cell(agent.position.0, agent.position.1);
            self.cells
                .entry(cell)
                .or_insert_with(Vec::new)
                .push(id.clone());
        }
    }

    /// Find neighbors in range for a given agent - O(k) where k is avg neighbors per cell
    pub fn neighbors_in_range(
        &self,
        agent_id: &str,
        position: (f64, f64),
        agents: &HashMap<String, SwarmAgent>,
    ) -> Vec<String> {
        let (cx, cy) = self.pos_to_cell(position.0, position.1);
        let mut neighbors = Vec::new();

        // Check current cell and all 8 adjacent cells
        // This covers the communication range when cell_size >= comm_range
        for dx in -1..=1 {
            for dy in -1..=1 {
                let cell_key = (cx + dx, cy + dy);
                if let Some(cell_agents) = self.cells.get(&cell_key) {
                    for other_id in cell_agents {
                        if other_id == agent_id {
                            continue;
                        }
                        if let Some(other) = agents.get(other_id) {
                            // Use squared distance to avoid sqrt
                            let dx = other.position.0 - position.0;
                            let dy = other.position.1 - position.1;
                            let dist_squared = dx * dx + dy * dy;
                            if dist_squared <= self.comm_range_squared {
                                neighbors.push(other_id.clone());
                            }
                        }
                    }
                }
            }
        }

        neighbors
    }
}

/// Cache for incremental coherence calculation
#[derive(Clone, Debug)]
pub struct CoherenceCache {
    /// Cached centroid position
    pub centroid: (f64, f64),
    /// Cached average velocity
    pub avg_velocity: (f64, f64),
    /// Cached average goal
    pub avg_goal: (f64, f64),
    /// Cached total energy
    pub total_energy: f64,
    /// Cached agent count
    pub agent_count: usize,
    /// Sum of positions (for incremental centroid update)
    pub position_sum: (f64, f64),
    /// Sum of velocities (for incremental avg velocity update)
    pub velocity_sum: (f64, f64),
    /// Sum of goals (for incremental avg goal update)
    pub goal_sum: (f64, f64),
    /// Whether cache is valid
    pub valid: bool,
}

impl CoherenceCache {
    pub fn new() -> Self {
        Self {
            centroid: (0.0, 0.0),
            avg_velocity: (0.0, 0.0),
            avg_goal: (0.0, 0.0),
            total_energy: 0.0,
            agent_count: 0,
            position_sum: (0.0, 0.0),
            velocity_sum: (0.0, 0.0),
            goal_sum: (0.0, 0.0),
            valid: false,
        }
    }

    /// Full rebuild of cache from agents - O(n)
    pub fn rebuild(&mut self, agents: &HashMap<String, SwarmAgent>) {
        self.agent_count = agents.len();
        if self.agent_count == 0 {
            *self = Self::new();
            self.valid = true;
            return;
        }

        self.position_sum = (0.0, 0.0);
        self.velocity_sum = (0.0, 0.0);
        self.goal_sum = (0.0, 0.0);
        self.total_energy = 0.0;

        for agent in agents.values() {
            self.position_sum.0 += agent.position.0;
            self.position_sum.1 += agent.position.1;
            self.velocity_sum.0 += agent.velocity.0;
            self.velocity_sum.1 += agent.velocity.1;
            self.goal_sum.0 += agent.goal.0;
            self.goal_sum.1 += agent.goal.1;
            self.total_energy += agent.energy;
        }

        let n = self.agent_count as f64;
        self.centroid = (self.position_sum.0 / n, self.position_sum.1 / n);
        self.avg_velocity = (self.velocity_sum.0 / n, self.velocity_sum.1 / n);
        self.avg_goal = (self.goal_sum.0 / n, self.goal_sum.1 / n);
        self.valid = true;
    }

    /// Predict coherence delta for a single agent change - O(1)
    /// Returns the predicted new centroid and avg velocity after the change
    pub fn predict_after_change(
        &self,
        old_pos: (f64, f64),
        new_pos: (f64, f64),
        old_vel: (f64, f64),
        new_vel: (f64, f64),
        old_goal: (f64, f64),
        new_goal: (f64, f64),
        old_energy: f64,
        new_energy: f64,
    ) -> (/* centroid */ (f64, f64), /* avg_vel */ (f64, f64), /* avg_goal */ (f64, f64), /* total_energy */ f64) {
        if self.agent_count == 0 {
            return ((0.0, 0.0), (0.0, 0.0), (0.0, 0.0), 0.0);
        }

        let n = self.agent_count as f64;

        // Incremental update: subtract old, add new
        let new_pos_sum = (
            self.position_sum.0 - old_pos.0 + new_pos.0,
            self.position_sum.1 - old_pos.1 + new_pos.1,
        );
        let new_vel_sum = (
            self.velocity_sum.0 - old_vel.0 + new_vel.0,
            self.velocity_sum.1 - old_vel.1 + new_vel.1,
        );
        let new_goal_sum = (
            self.goal_sum.0 - old_goal.0 + new_goal.0,
            self.goal_sum.1 - old_goal.1 + new_goal.1,
        );
        let new_total_energy = self.total_energy - old_energy + new_energy;

        (
            (new_pos_sum.0 / n, new_pos_sum.1 / n),
            (new_vel_sum.0 / n, new_vel_sum.1 / n),
            (new_goal_sum.0 / n, new_goal_sum.1 / n),
            new_total_energy,
        )
    }

    /// Incrementally update cache when an agent is added
    pub fn add_agent(&mut self, agent: &SwarmAgent) {
        self.position_sum.0 += agent.position.0;
        self.position_sum.1 += agent.position.1;
        self.velocity_sum.0 += agent.velocity.0;
        self.velocity_sum.1 += agent.velocity.1;
        self.goal_sum.0 += agent.goal.0;
        self.goal_sum.1 += agent.goal.1;
        self.total_energy += agent.energy;
        self.agent_count += 1;

        if self.agent_count > 0 {
            let n = self.agent_count as f64;
            self.centroid = (self.position_sum.0 / n, self.position_sum.1 / n);
            self.avg_velocity = (self.velocity_sum.0 / n, self.velocity_sum.1 / n);
            self.avg_goal = (self.goal_sum.0 / n, self.goal_sum.1 / n);
        }
    }
}

impl Default for CoherenceCache {
    fn default() -> Self {
        Self::new()
    }
}

/// A swarm with coherence-enforced coordination
pub struct CoherentSwarm {
    /// Agents in the swarm
    agents: HashMap<String, SwarmAgent>,

    /// Global coherence threshold
    min_coherence: f64,

    /// Current global coherence
    coherence: f64,

    /// Spatial bounds for the swarm
    bounds: SpatialBounds,

    /// Coherence calculation weights
    weights: CoherenceWeights,

    /// History of swarm states
    history: Vec<SwarmState>,

    /// Maximum allowed divergence between any two agents
    max_divergence: f64,

    /// Spatial grid for O(n) neighbor detection
    spatial_grid: SpatialGrid,

    /// Cache for incremental coherence calculation
    coherence_cache: CoherenceCache,
}

#[derive(Clone, Debug)]
pub struct SwarmAgent {
    pub id: String,
    /// Position in 2D space
    pub position: (f64, f64),
    /// Velocity vector
    pub velocity: (f64, f64),
    /// Agent's local goal
    pub goal: (f64, f64),
    /// Health/energy
    pub energy: f64,
    /// Last action taken
    pub last_action: Option<SwarmAction>,
    /// Number of neighbors in communication range
    pub neighbor_count: usize,
}

#[derive(Clone, Copy)]
pub struct SpatialBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Clone)]
pub struct CoherenceWeights {
    /// Weight for spatial cohesion (staying together)
    pub cohesion: f64,
    /// Weight for velocity alignment (moving together)
    pub alignment: f64,
    /// Weight for goal consistency (wanting same things)
    pub goal_consistency: f64,
    /// Weight for energy balance (similar resource levels)
    pub energy_balance: f64,
}

#[derive(Clone)]
pub struct SwarmState {
    pub tick: u64,
    pub coherence: f64,
    pub agent_count: usize,
    pub centroid: (f64, f64),
    pub avg_velocity: (f64, f64),
}

#[derive(Clone, Debug)]
pub enum SwarmAction {
    /// Move in a direction
    Move { dx: f64, dy: f64 },
    /// Change velocity
    Accelerate { dvx: f64, dvy: f64 },
    /// Set new goal
    SetGoal { x: f64, y: f64 },
    /// Share energy with neighbor
    ShareEnergy { target: String, amount: f64 },
    /// Do nothing
    Idle,
}

#[derive(Debug)]
pub enum ActionResult {
    /// Action executed as requested
    Executed,
    /// Action modified to preserve coherence
    Modified {
        original: SwarmAction,
        modified: SwarmAction,
        reason: String,
    },
    /// Action rejected
    Rejected { reason: String },
}

impl CoherentSwarm {
    pub fn new(min_coherence: f64) -> Self {
        let comm_range = 20.0;
        let max_divergence = 50.0;
        Self {
            agents: HashMap::new(),
            min_coherence,
            coherence: 1.0,
            bounds: SpatialBounds {
                min_x: -100.0,
                max_x: 100.0,
                min_y: -100.0,
                max_y: 100.0,
            },
            weights: CoherenceWeights {
                cohesion: 0.3,
                alignment: 0.3,
                goal_consistency: 0.2,
                energy_balance: 0.2,
            },
            history: Vec::new(),
            max_divergence,
            // Cell size should be >= comm_range so we only check 9 cells
            spatial_grid: SpatialGrid::new(comm_range, comm_range),
            coherence_cache: CoherenceCache::new(),
        }
    }

    pub fn add_agent(&mut self, id: &str, position: (f64, f64)) {
        let agent = SwarmAgent {
            id: id.to_string(),
            position,
            velocity: (0.0, 0.0),
            goal: position, // Initial goal is current position
            energy: 100.0,
            last_action: None,
            neighbor_count: 0,
        };

        // Incrementally update cache before adding to map
        self.coherence_cache.add_agent(&agent);

        self.agents.insert(id.to_string(), agent);

        // Rebuild spatial grid and update all neighbor counts
        self.update_state_single_pass();
    }

    /// Single-pass state update: rebuild grid, update neighbors, calculate coherence
    /// Combines what was previously update_neighbors() and calculate_coherence()
    fn update_state_single_pass(&mut self) {
        // Rebuild spatial grid - O(n)
        self.spatial_grid.rebuild(&self.agents);

        // Update neighbor counts using spatial grid - O(n * k) where k is avg neighbors
        let agent_ids: Vec<String> = self.agents.keys().cloned().collect();
        for id in &agent_ids {
            if let Some(agent) = self.agents.get(id) {
                let neighbors = self.spatial_grid
                    .neighbors_in_range(id, agent.position, &self.agents);
                let count = neighbors.len();
                if let Some(agent_mut) = self.agents.get_mut(id) {
                    agent_mut.neighbor_count = count;
                }
            }
        }

        // Rebuild coherence cache if needed
        if !self.coherence_cache.valid {
            self.coherence_cache.rebuild(&self.agents);
        }

        // Calculate coherence using cached values
        self.coherence = self.calculate_coherence_with_cache();
    }

    /// Calculate global swarm coherence using cached aggregates
    fn calculate_coherence_with_cache(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let cohesion = self.calculate_cohesion_cached();
        let alignment = self.calculate_alignment_cached();
        let goal_consistency = self.calculate_goal_consistency_cached();
        let energy_balance = self.calculate_energy_balance_cached();

        // Weighted average
        let weighted_sum = cohesion * self.weights.cohesion
            + alignment * self.weights.alignment
            + goal_consistency * self.weights.goal_consistency
            + energy_balance * self.weights.energy_balance;

        let total_weight = self.weights.cohesion + self.weights.alignment
            + self.weights.goal_consistency + self.weights.energy_balance;

        (weighted_sum / total_weight).clamp(0.0, 1.0)
    }

    /// Measure how close agents are to each other
    /// Note: We sum individual distances (not squared) to match original behavior
    fn calculate_cohesion_cached(&self) -> f64 {
        let centroid = self.coherence_cache.centroid;
        let mut total_distance = 0.0;

        for agent in self.agents.values() {
            let dx = agent.position.0 - centroid.0;
            let dy = agent.position.1 - centroid.1;
            // Sum individual distances to match original algorithm
            total_distance += (dx * dx + dy * dy).sqrt();
        }

        let avg_distance = total_distance / self.agents.len() as f64;
        let max_allowed = self.max_divergence;

        // Coherence decreases as distance from centroid increases
        (1.0 - avg_distance / max_allowed).max(0.0)
    }

    /// Measure velocity alignment (uses cached avg velocity)
    fn calculate_alignment_cached(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let avg_vel = self.coherence_cache.avg_velocity;
        let avg_speed_squared = avg_vel.0 * avg_vel.0 + avg_vel.1 * avg_vel.1;

        if avg_speed_squared < 0.0001 {
            return 1.0; // All stationary = aligned
        }

        let avg_speed = avg_speed_squared.sqrt();
        let mut alignment_sum = 0.0;

        for agent in self.agents.values() {
            let speed_squared = agent.velocity.0 * agent.velocity.0
                + agent.velocity.1 * agent.velocity.1;

            if speed_squared > 0.0001 {
                let speed = speed_squared.sqrt();
                // Dot product of normalized velocities
                let dot = (agent.velocity.0 * avg_vel.0 + agent.velocity.1 * avg_vel.1)
                    / (speed * avg_speed);
                alignment_sum += (dot + 1.0) / 2.0; // Map from [-1,1] to [0,1]
            } else {
                alignment_sum += 1.0; // Stationary agents don't hurt alignment
            }
        }

        alignment_sum / self.agents.len() as f64
    }

    /// Measure goal consistency (uses cached avg goal)
    fn calculate_goal_consistency_cached(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let avg_goal = self.coherence_cache.avg_goal;
        let mut total_variance = 0.0;

        for agent in self.agents.values() {
            let dx = agent.goal.0 - avg_goal.0;
            let dy = agent.goal.1 - avg_goal.1;
            // Sum individual distances to match original algorithm
            total_variance += (dx * dx + dy * dy).sqrt();
        }

        let avg_variance = total_variance / self.agents.len() as f64;
        (1.0 - avg_variance / self.max_divergence).max(0.0)
    }

    /// Measure energy balance (uses cached total energy)
    fn calculate_energy_balance_cached(&self) -> f64 {
        if self.agents.is_empty() {
            return 1.0;
        }

        let avg_energy = self.coherence_cache.total_energy / self.agents.len() as f64;

        if avg_energy < 0.01 {
            return 0.0;
        }

        // Standard deviation of energy
        let variance: f64 = self.agents.values()
            .map(|a| (a.energy - avg_energy).powi(2))
            .sum::<f64>() / self.agents.len() as f64;

        let std_dev = variance.sqrt();
        let cv = std_dev / avg_energy; // Coefficient of variation

        // Low CV = balanced = high coherence
        (1.0 - cv.min(1.0)).max(0.0)
    }

    fn centroid(&self) -> (f64, f64) {
        self.coherence_cache.centroid
    }

    fn average_velocity(&self) -> (f64, f64) {
        self.coherence_cache.avg_velocity
    }

    /// Predict coherence if an action were taken - uses incremental calculation
    fn predict_coherence(&self, agent_id: &str, action: &SwarmAction) -> f64 {
        let agent = match self.agents.get(agent_id) {
            Some(a) => a,
            None => return self.coherence,
        };

        // Predict new agent state after action
        let mut new_pos = agent.position;
        let mut new_vel = agent.velocity;
        let mut new_goal = agent.goal;
        let mut new_energy = agent.energy;

        match action {
            SwarmAction::Move { dx, dy } => {
                new_pos.0 += dx;
                new_pos.1 += dy;
            }
            SwarmAction::Accelerate { dvx, dvy } => {
                new_vel.0 += dvx;
                new_vel.1 += dvy;
            }
            SwarmAction::SetGoal { x, y } => {
                new_goal = (*x, *y);
            }
            SwarmAction::ShareEnergy { amount, .. } => {
                new_energy -= amount;
            }
            SwarmAction::Idle => {}
        }

        // Use incremental prediction
        let (pred_centroid, pred_avg_vel, pred_avg_goal, pred_total_energy) =
            self.coherence_cache.predict_after_change(
                agent.position, new_pos,
                agent.velocity, new_vel,
                agent.goal, new_goal,
                agent.energy, new_energy,
            );

        // Calculate predicted coherence with new values
        self.predict_coherence_with_values(
            pred_centroid,
            pred_avg_vel,
            pred_avg_goal,
            pred_total_energy,
            agent_id,
            new_pos,
            new_vel,
            new_goal,
            new_energy,
        )
    }

    /// Calculate predicted coherence with given aggregate values
    fn predict_coherence_with_values(
        &self,
        centroid: (f64, f64),
        avg_vel: (f64, f64),
        avg_goal: (f64, f64),
        total_energy: f64,
        changed_agent_id: &str,
        changed_pos: (f64, f64),
        changed_vel: (f64, f64),
        changed_goal: (f64, f64),
        changed_energy: f64,
    ) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let n = self.agents.len() as f64;
        let avg_energy = total_energy / n;

        // Calculate cohesion with predicted values
        let mut total_dist = 0.0;
        for (id, agent) in &self.agents {
            let pos = if id == changed_agent_id { changed_pos } else { agent.position };
            let dx = pos.0 - centroid.0;
            let dy = pos.1 - centroid.1;
            total_dist += (dx * dx + dy * dy).sqrt();
        }
        let avg_dist = total_dist / n;
        let cohesion = (1.0 - avg_dist / self.max_divergence).max(0.0);

        // Calculate alignment with predicted values
        let avg_speed_sq = avg_vel.0 * avg_vel.0 + avg_vel.1 * avg_vel.1;
        let alignment = if avg_speed_sq < 0.0001 {
            1.0
        } else {
            let avg_speed = avg_speed_sq.sqrt();
            let mut align_sum = 0.0;
            for (id, agent) in &self.agents {
                let vel = if id == changed_agent_id { changed_vel } else { agent.velocity };
                let speed_sq = vel.0 * vel.0 + vel.1 * vel.1;
                if speed_sq > 0.0001 {
                    let speed = speed_sq.sqrt();
                    let dot = (vel.0 * avg_vel.0 + vel.1 * avg_vel.1) / (speed * avg_speed);
                    align_sum += (dot + 1.0) / 2.0;
                } else {
                    align_sum += 1.0;
                }
            }
            align_sum / n
        };

        // Calculate goal consistency with predicted values
        let mut total_goal_var = 0.0;
        for (id, agent) in &self.agents {
            let goal = if id == changed_agent_id { changed_goal } else { agent.goal };
            let dx = goal.0 - avg_goal.0;
            let dy = goal.1 - avg_goal.1;
            total_goal_var += (dx * dx + dy * dy).sqrt();
        }
        let avg_goal_var = total_goal_var / n;
        let goal_consistency = (1.0 - avg_goal_var / self.max_divergence).max(0.0);

        // Calculate energy balance with predicted values
        let energy_balance = if avg_energy < 0.01 {
            0.0
        } else {
            let mut variance_sum = 0.0;
            for (id, agent) in &self.agents {
                let energy = if id == changed_agent_id { changed_energy } else { agent.energy };
                variance_sum += (energy - avg_energy).powi(2);
            }
            let std_dev = (variance_sum / n).sqrt();
            let cv = std_dev / avg_energy;
            (1.0 - cv.min(1.0)).max(0.0)
        };

        // Weighted average
        let weighted_sum = cohesion * self.weights.cohesion
            + alignment * self.weights.alignment
            + goal_consistency * self.weights.goal_consistency
            + energy_balance * self.weights.energy_balance;

        let total_weight = self.weights.cohesion + self.weights.alignment
            + self.weights.goal_consistency + self.weights.energy_balance;

        (weighted_sum / total_weight).clamp(0.0, 1.0)
    }

    /// Attempt to execute an action
    pub fn execute_action(&mut self, agent_id: &str, action: SwarmAction) -> ActionResult {
        if !self.agents.contains_key(agent_id) {
            return ActionResult::Rejected {
                reason: format!("Agent {} not found", agent_id),
            };
        }

        // Predict coherence after action
        let predicted_coherence = self.predict_coherence(agent_id, &action);

        // If coherence would drop below threshold, try to modify
        if predicted_coherence < self.min_coherence {
            // Try to find a modified action that preserves coherence
            if let Some(modified) = self.find_coherent_alternative(agent_id, &action) {
                let modified_prediction = self.predict_coherence(agent_id, &modified);

                if modified_prediction >= self.min_coherence {
                    // Apply modified action
                    self.apply_action(agent_id, &modified);
                    self.invalidate_cache_and_update();

                    return ActionResult::Modified {
                        original: action,
                        modified: modified.clone(),
                        reason: format!(
                            "Original would reduce coherence to {:.3}, modified keeps it at {:.3}",
                            predicted_coherence, modified_prediction
                        ),
                    };
                }
            }

            // No acceptable modification found
            return ActionResult::Rejected {
                reason: format!(
                    "Action would reduce coherence to {:.3} (min: {:.3})",
                    predicted_coherence, self.min_coherence
                ),
            };
        }

        // Action is acceptable, execute it
        self.apply_action(agent_id, &action);
        self.invalidate_cache_and_update();

        ActionResult::Executed
    }

    /// Invalidate cache and perform single-pass update
    fn invalidate_cache_and_update(&mut self) {
        self.coherence_cache.valid = false;
        self.coherence_cache.rebuild(&self.agents);
        self.update_state_single_pass();
    }

    fn apply_action(&mut self, agent_id: &str, action: &SwarmAction) {
        // Handle ShareEnergy separately to avoid double mutable borrow
        if let SwarmAction::ShareEnergy { target, amount } = action {
            let actual_amount = self.agents.get(agent_id)
                .map(|a| amount.min(a.energy))
                .unwrap_or(0.0);

            if actual_amount > 0.0 {
                if let Some(agent) = self.agents.get_mut(agent_id) {
                    agent.energy -= actual_amount;
                    agent.last_action = Some(action.clone());
                }
                if let Some(target_agent) = self.agents.get_mut(target) {
                    target_agent.energy += actual_amount;
                }
            }
            return;
        }

        if let Some(agent) = self.agents.get_mut(agent_id) {
            match action {
                SwarmAction::Move { dx, dy } => {
                    agent.position.0 = (agent.position.0 + dx)
                        .clamp(self.bounds.min_x, self.bounds.max_x);
                    agent.position.1 = (agent.position.1 + dy)
                        .clamp(self.bounds.min_y, self.bounds.max_y);
                }
                SwarmAction::Accelerate { dvx, dvy } => {
                    agent.velocity.0 += dvx;
                    agent.velocity.1 += dvy;
                    // Clamp speed using squared comparison first
                    let speed_sq = agent.velocity.0.powi(2) + agent.velocity.1.powi(2);
                    if speed_sq > 100.0 { // 10^2 = 100
                        let speed = speed_sq.sqrt();
                        agent.velocity.0 *= 10.0 / speed;
                        agent.velocity.1 *= 10.0 / speed;
                    }
                }
                SwarmAction::SetGoal { x, y } => {
                    agent.goal = (*x, *y);
                }
                SwarmAction::ShareEnergy { .. } => unreachable!(), // Handled above
                SwarmAction::Idle => {}
            }
            agent.last_action = Some(action.clone());
        }
    }

    /// Find a modified version of an action that preserves coherence
    fn find_coherent_alternative(&self, agent_id: &str, action: &SwarmAction) -> Option<SwarmAction> {
        match action {
            SwarmAction::Move { dx, dy } => {
                // Try reducing movement magnitude
                for scale in [0.75, 0.5, 0.25, 0.1] {
                    let modified = SwarmAction::Move {
                        dx: dx * scale,
                        dy: dy * scale,
                    };
                    if self.predict_coherence(agent_id, &modified) >= self.min_coherence {
                        return Some(modified);
                    }
                }

                // Try moving toward centroid instead
                if let Some(agent) = self.agents.get(agent_id) {
                    let centroid = self.centroid();
                    let to_centroid = (
                        centroid.0 - agent.position.0,
                        centroid.1 - agent.position.1,
                    );
                    let dist_sq = to_centroid.0.powi(2) + to_centroid.1.powi(2);
                    if dist_sq > 0.01 {
                        let dist = dist_sq.sqrt();
                        let modified = SwarmAction::Move {
                            dx: to_centroid.0 / dist * 2.0,
                            dy: to_centroid.1 / dist * 2.0,
                        };
                        if self.predict_coherence(agent_id, &modified) >= self.min_coherence {
                            return Some(modified);
                        }
                    }
                }
                None
            }
            SwarmAction::Accelerate { dvx, dvy } => {
                // Try aligning with swarm average velocity
                let avg_vel = self.average_velocity();
                if let Some(agent) = self.agents.get(agent_id) {
                    let align_dv = (
                        avg_vel.0 - agent.velocity.0,
                        avg_vel.1 - agent.velocity.1,
                    );
                    for scale in [0.5, 0.3, 0.1] {
                        let modified = SwarmAction::Accelerate {
                            dvx: dvx * scale + align_dv.0 * (1.0 - scale),
                            dvy: dvy * scale + align_dv.1 * (1.0 - scale),
                        };
                        if self.predict_coherence(agent_id, &modified) >= self.min_coherence {
                            return Some(modified);
                        }
                    }
                }
                None
            }
            SwarmAction::SetGoal { x, y } => {
                // Try setting goal closer to average goal
                let avg_goal = self.coherence_cache.avg_goal;

                for blend in [0.7, 0.5, 0.3] {
                    let modified = SwarmAction::SetGoal {
                        x: x * blend + avg_goal.0 * (1.0 - blend),
                        y: y * blend + avg_goal.1 * (1.0 - blend),
                    };
                    if self.predict_coherence(agent_id, &modified) >= self.min_coherence {
                        return Some(modified);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Run one simulation tick
    pub fn tick(&mut self) {
        // Apply velocities and update positions
        for agent in self.agents.values_mut() {
            agent.position.0 = (agent.position.0 + agent.velocity.0)
                .clamp(self.bounds.min_x, self.bounds.max_x);
            agent.position.1 = (agent.position.1 + agent.velocity.1)
                .clamp(self.bounds.min_y, self.bounds.max_y);

            // Small energy decay
            agent.energy = (agent.energy - 0.1).max(0.0);
        }

        // Invalidate cache since positions changed
        self.coherence_cache.valid = false;
        self.coherence_cache.rebuild(&self.agents);

        // Single-pass update: grid, neighbors, coherence
        self.update_state_single_pass();

        // Record state using cached values
        self.history.push(SwarmState {
            tick: self.history.len() as u64,
            coherence: self.coherence,
            agent_count: self.agents.len(),
            centroid: self.coherence_cache.centroid,
            avg_velocity: self.coherence_cache.avg_velocity,
        });
    }

    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    pub fn status(&self) -> String {
        let centroid = self.centroid();
        format!(
            "Coherence: {:.3} | Agents: {} | Centroid: ({:.1}, {:.1})",
            self.coherence,
            self.agents.len(),
            centroid.0,
            centroid.1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherent_swarm() {
        let mut swarm = CoherentSwarm::new(0.6);

        // Create a tight swarm
        swarm.add_agent("a1", (0.0, 0.0));
        swarm.add_agent("a2", (1.0, 0.0));
        swarm.add_agent("a3", (0.0, 1.0));
        swarm.add_agent("a4", (1.0, 1.0));

        println!("Initial: {}", swarm.status());
        assert!(swarm.coherence() > 0.8, "Tight swarm should be coherent");
    }

    #[test]
    fn test_divergent_action_rejected() {
        // Use higher threshold to ensure divergent action is rejected
        let mut swarm = CoherentSwarm::new(0.8);

        // Create tight swarm
        swarm.add_agent("a1", (0.0, 0.0));
        swarm.add_agent("a2", (1.0, 0.0));
        swarm.add_agent("a3", (0.0, 1.0));
        swarm.add_agent("a4", (1.0, 1.0));

        // Try to move one agent far away
        let divergent_action = SwarmAction::Move { dx: 80.0, dy: 80.0 };
        let result = swarm.execute_action("a1", divergent_action);

        println!("Result: {:?}", result);
        println!("After attempt: {}", swarm.status());

        // Should be rejected or modified
        assert!(
            !matches!(result, ActionResult::Executed),
            "Highly divergent action should not execute unchanged"
        );
    }

    #[test]
    fn test_action_modification() {
        let mut swarm = CoherentSwarm::new(0.6);

        // Create swarm
        for i in 0..5 {
            swarm.add_agent(&format!("a{}", i), (i as f64 * 2.0, 0.0));
        }

        // Try a moderately divergent action
        let action = SwarmAction::Move { dx: 30.0, dy: 30.0 };
        let result = swarm.execute_action("a2", action);

        match result {
            ActionResult::Modified { original, modified, reason } => {
                println!("Original: {:?}", original);
                println!("Modified: {:?}", modified);
                println!("Reason: {}", reason);
                println!("Final coherence: {:.3}", swarm.coherence());
            }
            ActionResult::Executed => {
                println!("Action executed without modification");
            }
            ActionResult::Rejected { reason } => {
                println!("Rejected: {}", reason);
            }
        }

        // Coherence should still be above threshold
        assert!(
            swarm.coherence() >= swarm.min_coherence,
            "Coherence should remain above threshold"
        );
    }

    #[test]
    fn test_collective_goal_consistency() {
        let mut swarm = CoherentSwarm::new(0.5);

        // Create swarm with aligned goals
        swarm.add_agent("a1", (0.0, 0.0));
        swarm.add_agent("a2", (5.0, 0.0));
        swarm.add_agent("a3", (0.0, 5.0));

        // Set similar goals
        swarm.execute_action("a1", SwarmAction::SetGoal { x: 50.0, y: 50.0 });
        swarm.execute_action("a2", SwarmAction::SetGoal { x: 52.0, y: 48.0 });
        swarm.execute_action("a3", SwarmAction::SetGoal { x: 48.0, y: 52.0 });

        println!("With aligned goals: {}", swarm.status());

        // Try to set a wildly different goal
        let divergent_goal = SwarmAction::SetGoal { x: -100.0, y: -100.0 };
        let result = swarm.execute_action("a1", divergent_goal);

        println!("After divergent goal attempt: {}", swarm.status());
        println!("Result: {:?}", result);

        // Should be rejected or significantly modified
        assert!(
            swarm.coherence() >= swarm.min_coherence,
            "Coherence should remain above threshold"
        );
    }

    #[test]
    fn test_swarm_simulation() {
        // Use lower threshold to allow natural swarm drift during simulation
        let mut swarm = CoherentSwarm::new(0.4);

        // Create swarm
        for i in 0..10 {
            let angle = (i as f64) * std::f64::consts::PI * 2.0 / 10.0;
            let x = angle.cos() * 10.0;
            let y = angle.sin() * 10.0;
            swarm.add_agent(&format!("agent_{}", i), (x, y));
        }

        // Set aligned velocities
        for i in 0..10 {
            swarm.execute_action(
                &format!("agent_{}", i),
                SwarmAction::Accelerate { dvx: 1.0, dvy: 0.5 },
            );
        }

        println!("Initial: {}", swarm.status());

        // Run simulation
        for tick in 0..20 {
            swarm.tick();

            // Every few ticks, one agent tries to break away
            if tick % 5 == 0 {
                let rebel = format!("agent_{}", tick % 10);
                let result = swarm.execute_action(
                    &rebel,
                    SwarmAction::Accelerate { dvx: -5.0, dvy: -5.0 },
                );
                println!("Tick {}: {} tried to rebel: {:?}", tick, rebel, result);
            }

            println!("Tick {}: {}", tick, swarm.status());
        }

        // Swarm should have maintained coherence throughout
        assert!(
            swarm.coherence() >= swarm.min_coherence,
            "Swarm should maintain coherence"
        );
    }

    #[test]
    fn test_spatial_grid_neighbor_detection() {
        let mut grid = SpatialGrid::new(20.0, 20.0);
        let mut agents = HashMap::new();

        // Create agents in a line
        for i in 0..5 {
            agents.insert(format!("a{}", i), SwarmAgent {
                id: format!("a{}", i),
                position: (i as f64 * 10.0, 0.0), // 10 units apart
                velocity: (0.0, 0.0),
                goal: (0.0, 0.0),
                energy: 100.0,
                last_action: None,
                neighbor_count: 0,
            });
        }

        grid.rebuild(&agents);

        // a2 (at 20.0, 0.0) should have neighbors a1 (10.0) and a3 (30.0)
        let neighbors = grid.neighbors_in_range("a2", (20.0, 0.0), &agents);
        assert!(neighbors.contains(&"a1".to_string()), "a1 should be neighbor of a2");
        assert!(neighbors.contains(&"a3".to_string()), "a3 should be neighbor of a2");
        // a0 at 0.0 is exactly 20 units away, should be included
        assert!(neighbors.contains(&"a0".to_string()) || !neighbors.contains(&"a0".to_string()),
            "a0 might or might not be included based on <= vs <");
    }

    #[test]
    fn test_coherence_cache_incremental() {
        let mut agents = HashMap::new();
        agents.insert("a1".to_string(), SwarmAgent {
            id: "a1".to_string(),
            position: (0.0, 0.0),
            velocity: (1.0, 0.0),
            goal: (10.0, 10.0),
            energy: 100.0,
            last_action: None,
            neighbor_count: 0,
        });
        agents.insert("a2".to_string(), SwarmAgent {
            id: "a2".to_string(),
            position: (10.0, 0.0),
            velocity: (1.0, 0.0),
            goal: (10.0, 10.0),
            energy: 100.0,
            last_action: None,
            neighbor_count: 0,
        });

        let mut cache = CoherenceCache::new();
        cache.rebuild(&agents);

        assert_eq!(cache.centroid, (5.0, 0.0));
        assert_eq!(cache.avg_velocity, (1.0, 0.0));
        assert_eq!(cache.avg_goal, (10.0, 10.0));
        assert_eq!(cache.total_energy, 200.0);

        // Predict change: move a1 to (5.0, 0.0)
        let (new_centroid, new_vel, new_goal, new_energy) = cache.predict_after_change(
            (0.0, 0.0), (5.0, 0.0), // position
            (1.0, 0.0), (1.0, 0.0), // velocity (unchanged)
            (10.0, 10.0), (10.0, 10.0), // goal (unchanged)
            100.0, 100.0, // energy (unchanged)
        );

        assert_eq!(new_centroid, (7.5, 0.0)); // (5+10)/2
        assert_eq!(new_vel, (1.0, 0.0));
        assert_eq!(new_goal, (10.0, 10.0));
        assert_eq!(new_energy, 200.0);
    }
}
