/// Landauer-Optimal Learning: Near-Thermodynamic-Limit Machine Learning
///
/// This module implements learning algorithms that approach the Landauer bound:
/// E_min = kT ln(2) per bit of information processed.
///
/// Key components:
/// - Energy-aware gradient descent
/// - Reversible computation tracking
/// - Thermodynamic efficiency metrics
/// - Adiabatic parameter updates
use std::f64::consts::LN_2;

/// Physical constants
pub mod constants {
    /// Boltzmann constant (J/K)
    pub const BOLTZMANN: f64 = 1.380649e-23;

    /// Room temperature (K)
    pub const ROOM_TEMP: f64 = 300.0;

    /// Landauer limit at room temperature (J)
    pub const LANDAUER_LIMIT: f64 = BOLTZMANN * ROOM_TEMP * std::f64::consts::LN_2;
    // ≈ 2.87 × 10^-21 J per bit

    /// Convert Joules to electron volts
    pub const J_TO_EV: f64 = 6.242e18;

    /// Landauer limit in eV
    pub const LANDAUER_LIMIT_EV: f64 = LANDAUER_LIMIT * J_TO_EV;
    // ≈ 0.0179 eV
}

/// Thermodynamic state tracker for learning process
#[derive(Debug, Clone)]
pub struct ThermodynamicState {
    /// Total energy dissipated (Joules)
    pub energy_dissipated: f64,

    /// Number of bits of information processed
    pub bits_processed: f64,

    /// Operating temperature (Kelvin)
    pub temperature: f64,

    /// Entropy produced (J/K)
    pub entropy_produced: f64,

    /// Number of irreversible operations
    pub irreversible_ops: usize,

    /// Number of reversible operations
    pub reversible_ops: usize,
}

impl ThermodynamicState {
    pub fn new(temperature: f64) -> Self {
        Self {
            energy_dissipated: 0.0,
            bits_processed: 0.0,
            temperature,
            entropy_produced: 0.0,
            irreversible_ops: 0,
            reversible_ops: 0,
        }
    }

    /// Calculate thermodynamic efficiency (actual energy / Landauer limit)
    pub fn efficiency(&self) -> f64 {
        let landauer_bound = constants::BOLTZMANN * self.temperature * LN_2 * self.bits_processed;
        if landauer_bound > 0.0 {
            self.energy_dissipated / landauer_bound
        } else {
            f64::INFINITY
        }
    }

    /// Energy per bit processed
    pub fn energy_per_bit(&self) -> f64 {
        if self.bits_processed > 0.0 {
            self.energy_dissipated / self.bits_processed
        } else {
            0.0
        }
    }

    /// Landauer limit for current temperature
    pub fn landauer_limit(&self) -> f64 {
        constants::BOLTZMANN * self.temperature * LN_2
    }

    /// How many times above Landauer limit we're operating
    pub fn landauer_multiple(&self) -> f64 {
        self.energy_per_bit() / self.landauer_limit()
    }

    /// Record an irreversible operation
    pub fn record_irreversible_op(&mut self, bits: f64) {
        let min_energy = self.landauer_limit() * bits;
        self.energy_dissipated += min_energy;
        self.bits_processed += bits;
        self.entropy_produced += constants::BOLTZMANN * LN_2 * bits;
        self.irreversible_ops += 1;
    }

    /// Record a reversible operation (minimal energy cost)
    pub fn record_reversible_op(&mut self, adiabatic_slowness: f64) {
        // Reversible operations have energy cost ~ 1/τ^2 where τ is time
        // For adiabatic processes, this approaches zero
        let energy_cost = self.landauer_limit() / (adiabatic_slowness * adiabatic_slowness);
        self.energy_dissipated += energy_cost;
        self.reversible_ops += 1;
    }
}

/// Thermodynamically-aware optimizer
#[derive(Debug, Clone)]
pub struct LandauerOptimizer {
    /// Learning rate
    pub learning_rate: f64,

    /// Adiabatic slowness factor (higher = slower = more reversible)
    pub adiabatic_factor: f64,

    /// Temperature (K)
    pub temperature: f64,

    /// Thermodynamic state
    pub state: ThermodynamicState,

    /// Use reversible updates when possible
    pub use_reversible: bool,
}

impl LandauerOptimizer {
    pub fn new(learning_rate: f64, temperature: f64) -> Self {
        Self {
            learning_rate,
            adiabatic_factor: 10.0,
            temperature,
            state: ThermodynamicState::new(temperature),
            use_reversible: true,
        }
    }

    /// Perform gradient descent step with thermodynamic accounting
    pub fn step(&mut self, gradient: &[f64], parameters: &mut [f64]) {
        assert_eq!(gradient.len(), parameters.len());

        let n_params = parameters.len();

        // Each parameter update requires processing information
        // Estimate bits: log2(precision) per parameter
        let bits_per_param = 32.0; // Assuming 32-bit precision
        let total_bits = n_params as f64 * bits_per_param;

        if self.use_reversible {
            // Reversible update: adiabatic change
            for (param, grad) in parameters.iter_mut().zip(gradient.iter()) {
                *param -= self.learning_rate * grad;
            }
            self.state.record_reversible_op(self.adiabatic_factor);
        } else {
            // Standard irreversible update
            for (param, grad) in parameters.iter_mut().zip(gradient.iter()) {
                *param -= self.learning_rate * grad;
            }
            self.state.record_irreversible_op(total_bits);
        }
    }

    /// Information-theoretic gradient: weight by information content
    pub fn information_weighted_gradient(&self, gradient: &[f64], information: &[f64]) -> Vec<f64> {
        gradient
            .iter()
            .zip(information.iter())
            .map(|(g, i)| g * i)
            .collect()
    }

    /// Estimate mutual information between data and parameters
    pub fn estimate_mutual_information(
        &self,
        data_entropy: f64,
        param_entropy: f64,
        joint_entropy: f64,
    ) -> f64 {
        // I(D; θ) = H(D) + H(θ) - H(D, θ)
        data_entropy + param_entropy - joint_entropy
    }

    /// Get thermodynamic efficiency report
    pub fn efficiency_report(&self) -> String {
        format!(
            "Thermodynamic Efficiency Report:\n\
             --------------------------------\n\
             Temperature: {:.2} K\n\
             Energy dissipated: {:.3e} J ({:.3e} eV)\n\
             Bits processed: {:.3e}\n\
             Energy per bit: {:.3e} J ({:.3e} eV)\n\
             Landauer limit: {:.3e} J ({:.3e} eV)\n\
             Efficiency multiple: {:.2}x above Landauer\n\
             Irreversible ops: {}\n\
             Reversible ops: {}\n\
             Entropy produced: {:.3e} J/K\n",
            self.state.temperature,
            self.state.energy_dissipated,
            self.state.energy_dissipated * constants::J_TO_EV,
            self.state.bits_processed,
            self.state.energy_per_bit(),
            self.state.energy_per_bit() * constants::J_TO_EV,
            self.state.landauer_limit(),
            self.state.landauer_limit() * constants::J_TO_EV,
            self.state.landauer_multiple(),
            self.state.irreversible_ops,
            self.state.reversible_ops,
            self.state.entropy_produced
        )
    }
}

/// Information bottleneck for thermodynamically-optimal compression
#[derive(Debug)]
pub struct InformationBottleneck {
    /// Trade-off parameter between compression and prediction
    pub beta: f64,

    /// Temperature (K)
    pub temperature: f64,
}

impl InformationBottleneck {
    pub fn new(beta: f64, temperature: f64) -> Self {
        Self { beta, temperature }
    }

    /// Information bottleneck objective: min I(X;T) - β I(T;Y)
    /// X = input, T = representation, Y = target
    pub fn objective(&self, mutual_info_x_t: f64, mutual_info_t_y: f64) -> f64 {
        mutual_info_x_t - self.beta * mutual_info_t_y
    }

    /// Thermodynamic cost of achieving compression ratio r
    pub fn compression_cost(&self, compression_ratio: f64) -> f64 {
        // Cost to erase (1 - 1/r) fraction of information
        let bits_erased = compression_ratio.log2();
        constants::BOLTZMANN * self.temperature * LN_2 * bits_erased
    }
}

/// Adiabatic learning: slow parameter changes to minimize dissipation
#[derive(Debug)]
pub struct AdiabaticLearner {
    /// Number of intermediate steps for adiabatic evolution
    pub n_steps: usize,

    /// Temperature
    pub temperature: f64,

    /// Thermodynamic state
    pub state: ThermodynamicState,
}

impl AdiabaticLearner {
    pub fn new(n_steps: usize, temperature: f64) -> Self {
        Self {
            n_steps,
            temperature,
            state: ThermodynamicState::new(temperature),
        }
    }

    /// Adiabatically evolve parameters from initial to final
    pub fn adiabatic_update(&mut self, initial: &[f64], final_params: &[f64], params: &mut [f64]) {
        assert_eq!(initial.len(), final_params.len());
        assert_eq!(initial.len(), params.len());

        // Interpolate slowly from initial to final
        for step in 0..self.n_steps {
            let alpha = (step + 1) as f64 / self.n_steps as f64;

            for i in 0..params.len() {
                params[i] = initial[i] * (1.0 - alpha) + final_params[i] * alpha;
            }

            // Each step is reversible if done slowly enough
            self.state.record_reversible_op(self.n_steps as f64);
        }
    }

    /// Energy cost of adiabatic evolution
    pub fn adiabatic_cost(&self) -> f64 {
        // Cost scales as 1/τ^2 for process time τ
        // More steps → slower → less dissipation
        let tau = self.n_steps as f64;
        constants::BOLTZMANN * self.temperature / (tau * tau)
    }
}

/// Maxwell's demon for information-driven learning
/// Implements Sagawa-Ueda generalized second law
#[derive(Debug)]
pub struct MaxwellDemon {
    /// Information acquired about system (bits)
    pub information: f64,

    /// Work extracted using information (J)
    pub work_extracted: f64,

    /// Temperature
    pub temperature: f64,
}

impl MaxwellDemon {
    pub fn new(temperature: f64) -> Self {
        Self {
            information: 0.0,
            work_extracted: 0.0,
            temperature,
        }
    }

    /// Sagawa-Ueda bound: W ≤ kT × I
    pub fn maximum_work(&self) -> f64 {
        constants::BOLTZMANN * self.temperature * LN_2 * self.information
    }

    /// Check if extracted work violates second law
    pub fn violates_second_law(&self) -> bool {
        self.work_extracted > self.maximum_work()
    }

    /// Use information to extract work
    pub fn extract_work(&mut self, bits_used: f64) -> f64 {
        let max_work = constants::BOLTZMANN * self.temperature * LN_2 * bits_used;
        self.work_extracted += max_work;
        self.information -= bits_used;
        max_work
    }

    /// Erase memory (costs energy)
    pub fn erase_memory(&mut self, bits: f64) -> f64 {
        let cost = constants::BOLTZMANN * self.temperature * LN_2 * bits;
        self.information = 0.0;
        cost
    }
}

/// Speed-energy tradeoff for learning
/// Implements E × τ ≥ constant principle
#[derive(Debug)]
pub struct SpeedEnergyTradeoff {
    /// Minimum product E × τ
    pub min_product: f64,

    /// Temperature
    pub temperature: f64,
}

impl SpeedEnergyTradeoff {
    pub fn new(temperature: f64) -> Self {
        // Minimum from uncertainty principle-like bound
        let min_product = constants::BOLTZMANN * temperature;
        Self {
            min_product,
            temperature,
        }
    }

    /// Minimum energy for given time constraint
    pub fn min_energy(&self, time: f64) -> f64 {
        self.min_product / time
    }

    /// Minimum time for given energy budget
    pub fn min_time(&self, energy: f64) -> f64 {
        self.min_product / energy
    }

    /// Check if (E, τ) pair is thermodynamically feasible
    pub fn is_feasible(&self, energy: f64, time: f64) -> bool {
        energy * time >= self.min_product
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landauer_limit() {
        // At room temperature, should be ~2.87 × 10^-21 J
        let limit = constants::LANDAUER_LIMIT;
        assert!((limit - 2.87e-21).abs() < 1e-22);

        // In eV, should be ~0.018 eV
        let limit_ev = constants::LANDAUER_LIMIT_EV;
        assert!((limit_ev - 0.018).abs() < 0.001);
    }

    #[test]
    fn test_thermodynamic_state() {
        let mut state = ThermodynamicState::new(constants::ROOM_TEMP);

        // Process 1000 bits irreversibly
        state.record_irreversible_op(1000.0);

        // Energy should be ~1000 × Landauer limit
        let expected = 1000.0 * constants::LANDAUER_LIMIT;
        assert!((state.energy_dissipated - expected).abs() < 1e-18);

        // Efficiency should be ~1.0 (at Landauer limit)
        assert!((state.efficiency() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_optimizer() {
        let mut opt = LandauerOptimizer::new(0.01, constants::ROOM_TEMP);

        let gradient = vec![1.0, -0.5, 0.3];
        let mut params = vec![1.0, 2.0, 3.0];

        opt.step(&gradient, &mut params);

        // Check parameters updated
        assert!((params[0] - 0.99).abs() < 1e-6);
        assert!((params[1] - 2.005).abs() < 1e-6);

        // Check thermodynamic accounting
        assert!(opt.state.energy_dissipated > 0.0);
        assert!(opt.state.bits_processed > 0.0 || opt.state.reversible_ops > 0);
    }

    #[test]
    fn test_maxwell_demon() {
        let mut demon = MaxwellDemon::new(constants::ROOM_TEMP);
        demon.information = 100.0; // 100 bits

        // Maximum extractable work
        let max_work = demon.maximum_work();
        let expected = 100.0 * constants::LANDAUER_LIMIT;
        assert!((max_work - expected).abs() < 1e-18);

        // Extract work
        let work = demon.extract_work(50.0);
        assert!((work - 50.0 * constants::LANDAUER_LIMIT).abs() < 1e-18);

        // Should not violate second law
        assert!(!demon.violates_second_law());
    }

    #[test]
    fn test_speed_energy_tradeoff() {
        let tradeoff = SpeedEnergyTradeoff::new(constants::ROOM_TEMP);

        let energy = 1e-18; // 1 attojoule
        let min_time = tradeoff.min_time(energy);

        // Should satisfy E × τ ≥ kT
        assert!(energy * min_time >= tradeoff.min_product);

        // Check feasibility
        assert!(tradeoff.is_feasible(energy, min_time));
        assert!(!tradeoff.is_feasible(energy, min_time * 0.5));
    }

    #[test]
    fn test_information_bottleneck() {
        let ib = InformationBottleneck::new(1.0, constants::ROOM_TEMP);

        // Compression cost for 2x compression (1 bit erased)
        let cost = ib.compression_cost(2.0);
        assert!((cost - constants::LANDAUER_LIMIT).abs() < 1e-22);

        // Objective with different mutual information values
        let obj1 = ib.objective(10.0, 8.0);
        let obj2 = ib.objective(10.0, 9.0);

        // Higher I(T;Y) should give better (lower) objective
        assert!(obj2 < obj1);
    }
}

/// Example: Train a simple model with thermodynamic accounting
pub fn example_thermodynamic_training() {
    println!("=== Landauer-Optimal Learning Example ===\n");

    let mut optimizer = LandauerOptimizer::new(0.01, constants::ROOM_TEMP);
    optimizer.use_reversible = true;
    optimizer.adiabatic_factor = 100.0;

    // Simulate training
    let mut params = vec![0.5; 100]; // 100 parameters

    for epoch in 0..10 {
        let gradient: Vec<f64> = (0..100).map(|i| (i as f64 * 0.01).sin()).collect();
        optimizer.step(&gradient, &mut params);

        if epoch % 3 == 0 {
            println!(
                "Epoch {}: Energy dissipated = {:.3e} J",
                epoch, optimizer.state.energy_dissipated
            );
        }
    }

    println!("\n{}", optimizer.efficiency_report());

    // Compare to theoretical minimum
    let bits_learned = 100.0 * 32.0; // 100 params × 32 bits precision
    let theoretical_min = constants::LANDAUER_LIMIT * bits_learned;
    println!("\nTheoretical minimum: {:.3e} J", theoretical_min);
    println!("Actual energy: {:.3e} J", optimizer.state.energy_dissipated);
    println!(
        "Efficiency: {:.2}x above Landauer limit",
        optimizer.state.landauer_multiple()
    );
}
