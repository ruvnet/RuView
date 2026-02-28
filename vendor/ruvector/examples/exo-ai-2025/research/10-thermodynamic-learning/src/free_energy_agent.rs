/// Free Energy Agent: Implementation of Karl Friston's Free Energy Principle
///
/// The Free Energy Principle (FEP) states that biological systems minimize
/// variational free energy, which upper-bounds surprise (negative log probability
/// of sensory observations).
///
/// F = E_q[log q(x|s) - log p(x,s)]
///   = -log p(s) + D_KL[q(x|s) || p(x|s)]
///
/// Where:
/// - x = hidden states (beliefs about the world)
/// - s = sensory observations
/// - q(x|s) = approximate posterior (recognition model)
/// - p(x,s) = generative model
///
/// Active inference extends this: agents act to minimize *expected* free energy.

/// Generative model: p(x, s) = p(s|x) p(x)
#[derive(Debug, Clone)]
pub struct GenerativeModel {
    /// Prior distribution p(x)
    pub prior: Distribution,

    /// Likelihood p(s|x)
    pub likelihood: Likelihood,

    /// Dimensionality of hidden states
    pub dim_x: usize,

    /// Dimensionality of observations
    pub dim_s: usize,
}

/// Distribution representation (Gaussian for simplicity)
#[derive(Debug, Clone)]
pub struct Distribution {
    pub mean: Vec<f64>,
    pub variance: Vec<f64>,
}

impl Distribution {
    pub fn new(mean: Vec<f64>, variance: Vec<f64>) -> Self {
        assert_eq!(mean.len(), variance.len());
        Self { mean, variance }
    }

    /// Standard normal distribution
    pub fn standard_normal(dim: usize) -> Self {
        Self {
            mean: vec![0.0; dim],
            variance: vec![1.0; dim],
        }
    }

    /// Sample from distribution (Box-Muller method)
    pub fn sample(&self) -> Vec<f64> {
        let mut samples = Vec::new();
        for i in 0..self.mean.len() {
            let u1 = rand::random::<f64>();
            let u2 = rand::random::<f64>();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            samples.push(self.mean[i] + z * self.variance[i].sqrt());
        }
        samples
    }

    /// Log probability density
    pub fn log_prob(&self, x: &[f64]) -> f64 {
        let mut log_p = 0.0;
        for i in 0..self.mean.len() {
            let diff = x[i] - self.mean[i];
            log_p -= 0.5 * (2.0 * std::f64::consts::PI * self.variance[i]).ln();
            log_p -= 0.5 * diff * diff / self.variance[i];
        }
        log_p
    }

    /// Entropy H[q] = -E_q[log q(x)]
    pub fn entropy(&self) -> f64 {
        let mut h = 0.0;
        for &var in &self.variance {
            h += 0.5 * (2.0 * std::f64::consts::PI * std::f64::consts::E * var).ln();
        }
        h
    }

    /// KL divergence from self to other
    pub fn kl_divergence(&self, other: &Distribution) -> f64 {
        assert_eq!(self.mean.len(), other.mean.len());
        let mut kl = 0.0;
        for i in 0..self.mean.len() {
            let mean_diff = self.mean[i] - other.mean[i];
            kl += 0.5 * (other.variance[i] / self.variance[i]).ln();
            kl += 0.5 * (self.variance[i] + mean_diff * mean_diff) / other.variance[i];
            kl -= 0.5;
        }
        kl
    }
}

/// Likelihood model p(s|x)
#[derive(Debug, Clone)]
pub struct Likelihood {
    /// Linear: s = Wx + ε where ε ~ N(0, σ²)
    pub weight_matrix: Vec<Vec<f64>>,
    pub noise_variance: Vec<f64>,
}

impl Likelihood {
    pub fn new(weight_matrix: Vec<Vec<f64>>, noise_variance: Vec<f64>) -> Self {
        Self {
            weight_matrix,
            noise_variance,
        }
    }

    /// Compute p(s|x)
    pub fn predict(&self, x: &[f64]) -> Distribution {
        let mut mean = vec![0.0; self.weight_matrix.len()];
        for i in 0..self.weight_matrix.len() {
            for j in 0..x.len() {
                mean[i] += self.weight_matrix[i][j] * x[j];
            }
        }
        Distribution::new(mean, self.noise_variance.clone())
    }

    /// Log likelihood log p(s|x)
    pub fn log_likelihood(&self, s: &[f64], x: &[f64]) -> f64 {
        let predicted = self.predict(x);
        predicted.log_prob(s)
    }
}

impl GenerativeModel {
    pub fn new(dim_x: usize, dim_s: usize) -> Self {
        // Random weight matrix
        let mut weight_matrix = vec![vec![0.0; dim_x]; dim_s];
        for i in 0..dim_s {
            for j in 0..dim_x {
                weight_matrix[i][j] = (rand::random::<f64>() - 0.5) * 0.2;
            }
        }

        Self {
            prior: Distribution::standard_normal(dim_x),
            likelihood: Likelihood::new(weight_matrix, vec![0.1; dim_s]),
            dim_x,
            dim_s,
        }
    }

    /// Joint log probability log p(x, s)
    pub fn log_joint(&self, x: &[f64], s: &[f64]) -> f64 {
        self.prior.log_prob(x) + self.likelihood.log_likelihood(s, x)
    }

    /// Evidence (marginal likelihood) - approximated
    pub fn log_evidence(&self, s: &[f64], samples: usize) -> f64 {
        let mut total = 0.0;
        for _ in 0..samples {
            let x = self.prior.sample();
            total += (self.log_joint(&x, s)).exp();
        }
        (total / samples as f64).ln()
    }
}

/// Recognition model: q(x|s) approximates true posterior p(x|s)
#[derive(Debug, Clone)]
pub struct RecognitionModel {
    /// Parameters of q(x|s)
    pub mean_params: Vec<Vec<f64>>, // s -> mean(x)
    pub var_params: Vec<f64>, // variance(x)
}

impl RecognitionModel {
    pub fn new(dim_s: usize, dim_x: usize) -> Self {
        let mut mean_params = vec![vec![0.0; dim_s]; dim_x];
        for i in 0..dim_x {
            for j in 0..dim_s {
                mean_params[i][j] = (rand::random::<f64>() - 0.5) * 0.2;
            }
        }

        Self {
            mean_params,
            var_params: vec![1.0; dim_x],
        }
    }

    /// Compute q(x|s)
    pub fn infer(&self, s: &[f64]) -> Distribution {
        let mut mean = vec![0.0; self.mean_params.len()];
        for i in 0..self.mean_params.len() {
            for j in 0..s.len() {
                mean[i] += self.mean_params[i][j] * s[j];
            }
        }
        Distribution::new(mean, self.var_params.clone())
    }
}

/// Free Energy Agent
#[derive(Debug)]
pub struct FreeEnergyAgent {
    /// Generative model of the world
    pub generative: GenerativeModel,

    /// Recognition model (approximate inference)
    pub recognition: RecognitionModel,

    /// Preferred observations (goals)
    pub preferences: Option<Distribution>,

    /// Learning rate for model updates
    pub learning_rate: f64,

    /// Temperature for thermodynamic interpretation
    pub temperature: f64,
}

impl FreeEnergyAgent {
    pub fn new(dim_x: usize, dim_s: usize, temperature: f64) -> Self {
        Self {
            generative: GenerativeModel::new(dim_x, dim_s),
            recognition: RecognitionModel::new(dim_s, dim_x),
            preferences: None,
            learning_rate: 0.01,
            temperature,
        }
    }

    /// Variational free energy: F = E_q[log q(x|s) - log p(x,s)]
    pub fn free_energy(&self, s: &[f64]) -> f64 {
        let q = self.recognition.infer(s);

        // Energy term: E_q[log q(x|s)]
        let entropy_term = -q.entropy();

        // Expected log joint: E_q[log p(x,s)]
        let mut expected_log_joint = 0.0;
        let n_samples = 100;
        for _ in 0..n_samples {
            let x = q.sample();
            expected_log_joint += self.generative.log_joint(&x, s);
        }
        expected_log_joint /= n_samples as f64;

        entropy_term - expected_log_joint
    }

    /// Alternative: F = -log p(s) + D_KL[q(x|s) || p(x|s)]
    /// Approximated using samples
    pub fn free_energy_kl(&self, s: &[f64]) -> f64 {
        let q = self.recognition.infer(s);

        // KL divergence from q to prior (approximation)
        let kl_to_prior = q.kl_divergence(&self.generative.prior);

        // Reconstruction error
        let x_sample = q.sample();
        let log_likelihood = self.generative.likelihood.log_likelihood(s, &x_sample);

        -log_likelihood + kl_to_prior
    }

    /// Perception: Update beliefs q(x|s) to minimize free energy
    pub fn perceive(&mut self, s: &[f64]) -> f64 {
        let initial_fe = self.free_energy_kl(s);

        // Gradient descent on recognition parameters
        // ∂F/∂φ where φ are recognition parameters

        let eps = 1e-4;
        for i in 0..self.recognition.mean_params.len() {
            for j in 0..self.recognition.mean_params[i].len() {
                // Numerical gradient
                let original = self.recognition.mean_params[i][j];

                self.recognition.mean_params[i][j] = original + eps;
                let fe_plus = self.free_energy_kl(s);

                self.recognition.mean_params[i][j] = original - eps;
                let fe_minus = self.free_energy_kl(s);

                let gradient = (fe_plus - fe_minus) / (2.0 * eps);
                self.recognition.mean_params[i][j] = original - self.learning_rate * gradient;
            }
        }

        let final_fe = self.free_energy_kl(s);
        initial_fe - final_fe // Reduction in free energy
    }

    /// Action: Choose action to minimize expected free energy
    /// For simplicity, return gradient of free energy w.r.t. observations
    pub fn act(&self, s: &[f64]) -> Vec<f64> {
        let eps = 1e-4;
        let mut action_gradient = vec![0.0; s.len()];

        for i in 0..s.len() {
            let mut s_plus = s.to_vec();
            s_plus[i] += eps;
            let fe_plus = self.free_energy_kl(&s_plus);

            let mut s_minus = s.to_vec();
            s_minus[i] -= eps;
            let fe_minus = self.free_energy_kl(&s_minus);

            action_gradient[i] = -(fe_plus - fe_minus) / (2.0 * eps);
        }

        action_gradient
    }

    /// Expected free energy for planning
    /// G = E[F] under policy π
    pub fn expected_free_energy(&self, s_predicted: &[f64]) -> f64 {
        // Epistemic value: expected information gain
        let q = self.recognition.infer(s_predicted);
        let epistemic = -q.entropy();

        // Pragmatic value: expected surprise under preferences
        let pragmatic = if let Some(ref pref) = self.preferences {
            -pref.log_prob(s_predicted)
        } else {
            0.0
        };

        epistemic + pragmatic
    }

    /// Learn generative model from data
    pub fn learn(&mut self, s: &[f64]) {
        // Infer hidden states
        let q = self.recognition.infer(s);
        let x = q.sample();

        // Update likelihood parameters (simplified)
        let eps = 1e-4;
        for i in 0..self.generative.likelihood.weight_matrix.len() {
            for j in 0..self.generative.likelihood.weight_matrix[i].len() {
                let original = self.generative.likelihood.weight_matrix[i][j];

                self.generative.likelihood.weight_matrix[i][j] = original + eps;
                let ll_plus = self.generative.likelihood.log_likelihood(s, &x);

                self.generative.likelihood.weight_matrix[i][j] = original - eps;
                let ll_minus = self.generative.likelihood.log_likelihood(s, &x);

                let gradient = (ll_plus - ll_minus) / (2.0 * eps);
                self.generative.likelihood.weight_matrix[i][j] =
                    original + self.learning_rate * gradient;
            }
        }
    }

    /// Set goal/preference distribution
    pub fn set_goal(&mut self, goal_mean: Vec<f64>, goal_var: Vec<f64>) {
        self.preferences = Some(Distribution::new(goal_mean, goal_var));
    }
}

/// Active inference loop
pub struct ActiveInferenceLoop {
    pub agent: FreeEnergyAgent,
    pub timestep: usize,
}

impl ActiveInferenceLoop {
    pub fn new(agent: FreeEnergyAgent) -> Self {
        Self { agent, timestep: 0 }
    }

    /// One step of perception-action cycle
    pub fn step(&mut self, observation: &[f64]) -> Vec<f64> {
        // Perception: minimize free energy w.r.t. beliefs
        let _fe_reduction = self.agent.perceive(observation);

        // Action: minimize expected free energy
        let action = self.agent.act(observation);

        // Learning: update generative model
        self.agent.learn(observation);

        self.timestep += 1;

        action
    }

    /// Report current state
    pub fn report(&self, observation: &[f64]) -> String {
        let fe = self.agent.free_energy_kl(observation);
        let q = self.agent.recognition.infer(observation);

        format!(
            "Timestep: {}\n\
             Free Energy: {:.6}\n\
             Belief mean: {:?}\n\
             Belief variance: {:?}\n",
            self.timestep, fe, q.mean, q.variance
        )
    }
}

// Mock rand
mod rand {
    pub fn random<T>() -> f64 {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distribution() {
        let dist = Distribution::new(vec![0.0, 1.0], vec![1.0, 0.5]);
        assert_eq!(dist.mean.len(), 2);

        let sample = dist.sample();
        assert_eq!(sample.len(), 2);

        let log_p = dist.log_prob(&vec![0.0, 1.0]);
        assert!(log_p.is_finite());

        let entropy = dist.entropy();
        assert!(entropy > 0.0);
    }

    #[test]
    fn test_kl_divergence() {
        let p = Distribution::new(vec![0.0], vec![1.0]);
        let q = Distribution::new(vec![1.0], vec![2.0]);

        let kl = p.kl_divergence(&q);
        assert!(kl >= 0.0); // KL is always non-negative
    }

    #[test]
    fn test_likelihood() {
        let likelihood = Likelihood::new(vec![vec![1.0, 0.5], vec![0.5, 1.0]], vec![0.1, 0.1]);

        let x = vec![1.0, -1.0];
        let predicted = likelihood.predict(&x);

        assert_eq!(predicted.mean.len(), 2);

        let ll = likelihood.log_likelihood(&vec![0.5, -0.5], &x);
        assert!(ll.is_finite());
    }

    #[test]
    fn test_generative_model() {
        let model = GenerativeModel::new(2, 3);
        assert_eq!(model.dim_x, 2);
        assert_eq!(model.dim_s, 3);

        let x = vec![0.0, 1.0];
        let s = vec![0.5, 0.5, 0.5];

        let log_joint = model.log_joint(&x, &s);
        assert!(log_joint.is_finite());
    }

    #[test]
    fn test_recognition_model() {
        let recognition = RecognitionModel::new(3, 2);

        let s = vec![0.5, 0.5, 0.5];
        let q = recognition.infer(&s);

        assert_eq!(q.mean.len(), 2);
        assert_eq!(q.variance.len(), 2);
    }

    #[test]
    fn test_free_energy_agent() {
        let agent = FreeEnergyAgent::new(2, 3, 300.0);

        let observation = vec![0.5, 0.5, 0.5];
        let fe = agent.free_energy_kl(&observation);

        assert!(fe.is_finite());
        assert!(fe >= 0.0); // Free energy should be non-negative
    }

    #[test]
    fn test_perception() {
        let mut agent = FreeEnergyAgent::new(2, 3, 300.0);
        let observation = vec![1.0, 0.5, 0.0];

        let initial_fe = agent.free_energy_kl(&observation);
        let reduction = agent.perceive(&observation);
        let final_fe = agent.free_energy_kl(&observation);

        // Free energy should decrease (or stay same)
        assert!(final_fe <= initial_fe || (final_fe - initial_fe).abs() < 0.1);
    }

    #[test]
    fn test_active_inference_loop() {
        let agent = FreeEnergyAgent::new(2, 3, 300.0);
        let mut loop_executor = ActiveInferenceLoop::new(agent);

        let observation = vec![1.0, 0.0, 0.5];
        let action = loop_executor.step(&observation);

        assert_eq!(action.len(), 3);
        assert!(loop_executor.timestep == 1);
    }
}

/// Example: Free energy minimization for tracking a signal
pub fn example_free_energy_tracking() {
    println!("=== Free Energy Agent: Signal Tracking ===\n");

    let mut agent = FreeEnergyAgent::new(2, 2, 300.0);

    // Set goal: prefer observations near [1.0, 1.0]
    agent.set_goal(vec![1.0, 1.0], vec![0.1, 0.1]);

    let mut loop_executor = ActiveInferenceLoop::new(agent);

    // Simulate trajectory
    let observations = vec![
        vec![0.0, 0.0],
        vec![0.2, 0.3],
        vec![0.5, 0.6],
        vec![0.8, 0.9],
        vec![1.0, 1.0],
    ];

    for (i, obs) in observations.iter().enumerate() {
        println!("Step {}:", i);
        println!("{}", loop_executor.report(obs));

        let action = loop_executor.step(obs);
        println!("Action: {:?}\n", action);
    }

    println!(
        "Final free energy: {:.6}",
        loop_executor
            .agent
            .free_energy_kl(&observations.last().unwrap())
    );
}
