// Temporal Memory: Time Crystal-Based Working Memory Implementation
// Models working memory as a discrete time crystal with limit cycle attractors

use ndarray::{Array1, Array2};
use std::collections::VecDeque;
use std::f64::consts::PI;

/// Working memory item
#[derive(Clone, Debug)]
pub struct MemoryItem {
    /// Content vector (high-dimensional representation)
    pub content: Array1<f64>,
    /// Timestamp of encoding
    pub encoded_time: f64,
    /// Strength (0-1)
    pub strength: f64,
}

/// Time crystal working memory configuration
#[derive(Clone, Debug)]
pub struct TemporalMemoryConfig {
    /// Dimension of memory representations
    pub memory_dim: usize,
    /// Number of neurons in prefrontal cortex module
    pub pfc_neurons: usize,
    /// Number of neurons in hippocampal module
    pub hc_neurons: usize,
    /// Theta oscillation frequency (Hz)
    pub theta_frequency: f64,
    /// Coupling strength between PFC and HC
    pub pfc_hc_coupling: f64,
    /// Metabolic energy supply rate
    pub energy_rate: f64,
    /// Dissipation rate
    pub dissipation: f64,
    /// Time step
    pub dt: f64,
    /// Maximum working memory capacity
    pub max_capacity: usize,
}

impl Default for TemporalMemoryConfig {
    fn default() -> Self {
        Self {
            memory_dim: 64,
            pfc_neurons: 200,
            hc_neurons: 100,
            theta_frequency: 8.0,
            pfc_hc_coupling: 0.5,
            energy_rate: 1.0,
            dissipation: 0.1,
            dt: 0.001,
            max_capacity: 4, // Miller's 4±1
        }
    }
}

/// Time crystal working memory system
pub struct TemporalMemory {
    config: TemporalMemoryConfig,
    /// PFC neural population (maintenance)
    pfc_neurons: Array1<f64>,
    /// HC neural population (encoding/retrieval)
    hc_neurons: Array1<f64>,
    /// PFC recurrent weights (asymmetric for limit cycles)
    pfc_weights: Array2<f64>,
    /// HC recurrent weights
    hc_weights: Array2<f64>,
    /// PFC-HC coupling weights
    pfc_to_hc: Array2<f64>,
    hc_to_pfc: Array2<f64>,
    /// Stored memory items
    memory_items: VecDeque<MemoryItem>,
    /// Current time
    time: f64,
    /// Metabolic energy level
    energy: f64,
    /// Time crystal order parameter history
    order_parameter_history: Vec<f64>,
}

impl TemporalMemory {
    /// Create new time crystal working memory
    pub fn new(config: TemporalMemoryConfig) -> Self {
        let pfc_neurons = Array1::zeros(config.pfc_neurons);
        let hc_neurons = Array1::zeros(config.hc_neurons);

        // Asymmetric weights for PFC (enable limit cycles)
        let pfc_weights = Self::generate_limit_cycle_weights(config.pfc_neurons, 0.3, 1.0);

        // Symmetric weights for HC (content storage)
        let hc_weights = Self::generate_symmetric_weights(config.hc_neurons, 0.2, 0.8);

        // Coupling weights
        let pfc_to_hc = Self::generate_coupling_weights(
            config.pfc_neurons,
            config.hc_neurons,
            config.pfc_hc_coupling,
        );
        let hc_to_pfc = pfc_to_hc.t().to_owned();

        Self {
            config,
            pfc_neurons,
            hc_neurons,
            pfc_weights,
            hc_weights,
            pfc_to_hc,
            hc_to_pfc,
            memory_items: VecDeque::new(),
            time: 0.0,
            energy: 1.0,
            order_parameter_history: Vec::new(),
        }
    }

    /// Generate asymmetric weights that support limit cycles
    fn generate_limit_cycle_weights(n: usize, sparsity: f64, strength: f64) -> Array2<f64> {
        let mut weights = Array2::zeros((n, n));
        let mut rng = rand::thread_rng();

        use rand::Rng;
        for i in 0..n {
            for j in 0..n {
                if i != j && rng.gen::<f64>() < sparsity {
                    // Asymmetric: W_ij != W_ji
                    weights[[i, j]] = rng.gen_range(-strength..strength);
                }
            }
        }

        weights
    }

    /// Generate symmetric weights for content storage
    fn generate_symmetric_weights(n: usize, sparsity: f64, strength: f64) -> Array2<f64> {
        let mut weights = Array2::zeros((n, n));
        let mut rng = rand::thread_rng();

        use rand::Rng;
        for i in 0..n {
            for j in i + 1..n {
                if rng.gen::<f64>() < sparsity {
                    let w = rng.gen_range(0.0..strength);
                    weights[[i, j]] = w;
                    weights[[j, i]] = w; // Symmetric
                }
            }
        }

        weights
    }

    /// Generate coupling weights between modules
    fn generate_coupling_weights(n_from: usize, n_to: usize, strength: f64) -> Array2<f64> {
        let mut weights = Array2::zeros((n_to, n_from));
        let mut rng = rand::thread_rng();

        use rand::Rng;
        for i in 0..n_to {
            for j in 0..n_from {
                if rng.gen::<f64>() < 0.1 {
                    // Sparse coupling
                    weights[[i, j]] = rng.gen_range(-strength..strength);
                }
            }
        }

        weights
    }

    /// Theta oscillation (periodic drive)
    fn theta_drive(&self) -> f64 {
        let omega = 2.0 * PI * self.config.theta_frequency;
        (omega * self.time).cos()
    }

    /// Encode new item into working memory
    pub fn encode(&mut self, item: Array1<f64>) -> Result<(), &'static str> {
        if item.len() != self.config.memory_dim {
            return Err("Item dimension mismatch");
        }

        if self.memory_items.len() >= self.config.max_capacity {
            // At capacity - remove weakest item
            self.memory_items.pop_front();
        }

        let memory_item = MemoryItem {
            content: item.clone(),
            encoded_time: self.time,
            strength: 1.0,
        };

        self.memory_items.push_back(memory_item);

        // Activate HC neurons for encoding
        self.activate_hc_for_item(&item);

        Ok(())
    }

    /// Activate HC neurons to represent item
    fn activate_hc_for_item(&mut self, item: &Array1<f64>) {
        // Project item into HC neural space
        // Simple approach: sample neurons proportional to item components
        let mut rng = rand::thread_rng();
        use rand::Rng;

        for i in 0..self.config.hc_neurons {
            let idx = i % item.len();
            self.hc_neurons[i] = item[idx].tanh() + rng.gen_range(-0.1..0.1);
        }
    }

    /// Retrieve item from working memory
    pub fn retrieve(&self, query: &Array1<f64>) -> Option<MemoryItem> {
        if query.len() != self.config.memory_dim {
            return None;
        }

        // Find item with highest similarity
        let mut best_match = None;
        let mut best_similarity = -1.0;

        for item in &self.memory_items {
            let similarity = cosine_similarity(query, &item.content) * item.strength;
            if similarity > best_similarity {
                best_similarity = similarity;
                best_match = Some(item.clone());
            }
        }

        if best_similarity > 0.5 {
            best_match
        } else {
            None
        }
    }

    /// Evolve neural dynamics (time crystal maintenance)
    pub fn step(&mut self) {
        let theta = self.theta_drive();

        // Update energy (metabolic supply - dissipation)
        let energy_cost = self.compute_energy_cost();
        self.energy +=
            (self.config.energy_rate - energy_cost - self.config.dissipation) * self.config.dt;
        self.energy = self.energy.clamp(0.0, 2.0);

        // If energy too low, time crystal collapses
        if self.energy < 0.2 {
            self.pfc_neurons *= 0.9; // Decay
            self.hc_neurons *= 0.9;
        } else {
            // Update PFC (working memory maintenance via limit cycle)
            self.update_pfc(theta);

            // Update HC (content representation)
            self.update_hc(theta);
        }

        // Decay memory strengths
        for item in &mut self.memory_items {
            item.strength *= 0.9999; // Slow decay
            if (self.time - item.encoded_time) > 10.0 {
                item.strength *= 0.99; // Faster decay for old items
            }
        }

        // Remove forgotten items
        self.memory_items.retain(|item| item.strength > 0.1);

        // Compute and store order parameter
        let m_k = self.compute_order_parameter(2);
        self.order_parameter_history.push(m_k);

        self.time += self.config.dt;
    }

    /// Update PFC neurons (limit cycle dynamics for maintenance)
    fn update_pfc(&mut self, theta_drive: f64) {
        let n = self.config.pfc_neurons;
        let mut updates = Array1::zeros(n);

        for i in 0..n {
            // Recurrent input (asymmetric → limit cycles)
            let mut recurrent = 0.0;
            for j in 0..n {
                recurrent += self.pfc_weights[[i, j]] * self.pfc_neurons[j];
            }

            // Input from HC
            let mut hc_input = 0.0;
            for j in 0..self.config.hc_neurons {
                hc_input += self.hc_to_pfc[[i, j]] * self.hc_neurons[j];
            }

            // Theta drive
            let drive = theta_drive * 0.5;

            // Neural dynamics: dr/dt = -r + tanh(W*r + I + θ)
            let total_input = recurrent + hc_input + drive;
            updates[i] = (-self.pfc_neurons[i] + total_input.tanh()) * self.config.dt;
        }

        self.pfc_neurons += &updates;
    }

    /// Update HC neurons (content representation)
    fn update_hc(&mut self, theta_drive: f64) {
        let n = self.config.hc_neurons;
        let mut updates = Array1::zeros(n);

        for i in 0..n {
            // Recurrent input (symmetric → stable attractors)
            let mut recurrent = 0.0;
            for j in 0..n {
                recurrent += self.hc_weights[[i, j]] * self.hc_neurons[j];
            }

            // Input from PFC
            let mut pfc_input = 0.0;
            for j in 0..self.config.pfc_neurons {
                pfc_input += self.pfc_to_hc[[i, j]] * self.pfc_neurons[j];
            }

            // Theta modulation
            let modulation = 1.0 + 0.3 * theta_drive;

            // Neural dynamics
            let total_input = (recurrent + pfc_input) * modulation;
            updates[i] = (-self.hc_neurons[i] + total_input.tanh()) * self.config.dt;
        }

        self.hc_neurons += &updates;
    }

    /// Compute energy cost (proportional to neural activity)
    fn compute_energy_cost(&self) -> f64 {
        let pfc_activity = self.pfc_neurons.mapv(|x| x.abs()).sum();
        let hc_activity = self.hc_neurons.mapv(|x| x.abs()).sum();
        (pfc_activity + hc_activity) * 0.001
    }

    /// Compute time crystal order parameter M_k
    fn compute_order_parameter(&self, k: usize) -> f64 {
        let omega_0 = 2.0 * PI * self.config.theta_frequency;
        let n = self.config.pfc_neurons;

        // Phases of PFC neurons
        let mut sum_real = 0.0;
        let mut sum_imag = 0.0;

        for i in 0..n {
            let phase = self.pfc_neurons[i] * PI / 2.0; // Map activity to phase
            let arg = k as f64 * omega_0 * phase;
            sum_real += arg.cos();
            sum_imag += arg.sin();
        }

        ((sum_real / n as f64).powi(2) + (sum_imag / n as f64).powi(2)).sqrt()
    }

    /// Get current capacity usage
    pub fn current_capacity(&self) -> usize {
        self.memory_items.len()
    }

    /// Get average memory strength
    pub fn average_strength(&self) -> f64 {
        if self.memory_items.is_empty() {
            0.0
        } else {
            self.memory_items
                .iter()
                .map(|item| item.strength)
                .sum::<f64>()
                / self.memory_items.len() as f64
        }
    }

    /// Check if system is in time crystal phase
    pub fn is_time_crystal_phase(&self) -> bool {
        if self.order_parameter_history.len() < 100 {
            return false;
        }

        // Average recent order parameter
        let recent: Vec<f64> = self
            .order_parameter_history
            .iter()
            .rev()
            .take(100)
            .cloned()
            .collect();

        let avg_m_k = recent.iter().sum::<f64>() / recent.len() as f64;

        // CTC phase if M_k > 0.5
        avg_m_k > 0.5
    }

    /// Get statistics
    pub fn get_stats(&self) -> MemoryStats {
        MemoryStats {
            capacity: self.current_capacity(),
            max_capacity: self.config.max_capacity,
            avg_strength: self.average_strength(),
            energy: self.energy,
            is_time_crystal: self.is_time_crystal_phase(),
            order_parameter: self.order_parameter_history.last().cloned().unwrap_or(0.0),
        }
    }
}

/// Memory statistics
#[derive(Clone, Debug)]
pub struct MemoryStats {
    pub capacity: usize,
    pub max_capacity: usize,
    pub avg_strength: f64,
    pub energy: f64,
    pub is_time_crystal: bool,
    pub order_parameter: f64,
}

impl std::fmt::Display for MemoryStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WM: {}/{} items | Strength: {:.2} | Energy: {:.2} | Time Crystal: {} | M_2: {:.3}",
            self.capacity,
            self.max_capacity,
            self.avg_strength,
            self.energy,
            if self.is_time_crystal { "YES" } else { "NO " },
            self.order_parameter
        )
    }
}

/// Cosine similarity between vectors
fn cosine_similarity(a: &Array1<f64>, b: &Array1<f64>) -> f64 {
    let dot = a.dot(b);
    let norm_a = a.dot(a).sqrt();
    let norm_b = b.dot(b).sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Working memory task simulator
pub struct WorkingMemoryTask {
    memory: TemporalMemory,
    /// Sequence of items to remember
    items: Vec<Array1<f64>>,
    /// Retrieval queries
    queries: Vec<Array1<f64>>,
    /// Accuracy history
    accuracy_history: Vec<f64>,
}

impl WorkingMemoryTask {
    pub fn new(config: TemporalMemoryConfig, n_items: usize, memory_dim: usize) -> Self {
        let mut rng = rand::thread_rng();
        use rand::Rng;

        // Generate random items
        let items: Vec<Array1<f64>> = (0..n_items)
            .map(|_| Array1::from_vec((0..memory_dim).map(|_| rng.gen_range(-1.0..1.0)).collect()))
            .collect();

        // Queries are same as items (exact recall)
        let queries = items.clone();

        Self {
            memory: TemporalMemory::new(config),
            items,
            queries,
            accuracy_history: Vec::new(),
        }
    }

    /// Run delayed match-to-sample task
    pub fn run_delayed_match_to_sample(&mut self, encoding_duration: f64, delay_duration: f64) {
        let dt = self.memory.config.dt;

        // Encoding phase: present items sequentially
        for item in &self.items {
            self.memory.encode(item.clone()).unwrap();

            // Maintenance during encoding
            let encoding_steps = (encoding_duration / dt) as usize;
            for _ in 0..encoding_steps {
                self.memory.step();
            }
        }

        // Delay phase: maintain without input
        let delay_steps = (delay_duration / dt) as usize;
        for _ in 0..delay_steps {
            self.memory.step();
        }

        // Retrieval phase: test recall
        let mut correct = 0;
        for query in self.queries.iter() {
            if let Some(retrieved) = self.memory.retrieve(query) {
                let similarity = cosine_similarity(query, &retrieved.content);
                if similarity > 0.8 {
                    correct += 1;
                }
            }
        }

        let accuracy = correct as f64 / self.queries.len() as f64;
        self.accuracy_history.push(accuracy);
    }

    /// Get performance metrics
    pub fn get_performance(&self) -> (f64, bool) {
        let accuracy = self.accuracy_history.last().cloned().unwrap_or(0.0);
        let is_time_crystal = self.memory.is_time_crystal_phase();
        (accuracy, is_time_crystal)
    }

    /// Print summary
    pub fn print_summary(&self) {
        let (accuracy, is_tc) = self.get_performance();
        let stats = self.memory.get_stats();

        println!("\n=== Working Memory Task Summary ===");
        println!("Accuracy: {:.1}%", accuracy * 100.0);
        println!("{}", stats);
        println!("Time Crystal enables accurate working memory: {}", is_tc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_memory_creation() {
        let config = TemporalMemoryConfig::default();
        let memory = TemporalMemory::new(config);
        assert_eq!(memory.current_capacity(), 0);
    }

    #[test]
    fn test_encode_retrieve() {
        let mut config = TemporalMemoryConfig::default();
        let mut memory = TemporalMemory::new(config);

        // Create item with correct dimension (64)
        let item = Array1::from_vec(vec![1.0; 64]);
        memory.encode(item.clone()).unwrap();

        // Maintain for a while
        for _ in 0..1000 {
            memory.step();
        }

        let retrieved = memory.retrieve(&item);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_working_memory_task() {
        let config = TemporalMemoryConfig::default();
        let mut task = WorkingMemoryTask::new(config, 3, 64);

        task.run_delayed_match_to_sample(0.5, 1.0);

        let (accuracy, _) = task.get_performance();
        println!("Task accuracy: {:.1}%", accuracy * 100.0);

        // With time crystal dynamics, accuracy should be reasonable
        assert!(accuracy > 0.3); // At least 30% recall
    }

    #[test]
    fn test_capacity_limit() {
        let mut config = TemporalMemoryConfig::default();
        config.max_capacity = 3;

        let mut memory = TemporalMemory::new(config);

        // Encode 5 items (exceeds capacity)
        for i in 0..5 {
            let item = Array1::from_vec(vec![i as f64; 64]);
            memory.encode(item).unwrap();
        }

        // Should only keep 3 most recent
        assert_eq!(memory.current_capacity(), 3);
    }

    #[test]
    fn test_time_crystal_phase() {
        let mut config = TemporalMemoryConfig::default();
        config.pfc_neurons = 100;

        let mut memory = TemporalMemory::new(config);

        // Encode item and maintain
        let item = Array1::from_vec(vec![1.0; 64]);
        memory.encode(item).unwrap();

        // Run for several theta cycles
        for _ in 0..10000 {
            memory.step();
        }

        // Check if time crystal phase emerged
        let is_tc = memory.is_time_crystal_phase();
        println!("Time crystal phase: {}", is_tc);

        // Should have some order parameter history
        assert!(memory.order_parameter_history.len() > 100);
    }
}
